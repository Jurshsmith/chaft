use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use argon2::{Algorithm, Argon2, Params, Version};
use chaft_crypto::{
    ContentKey, CryptoError, SealedPayload, open_aes_256_gcm_siv, seal_aes_256_gcm_siv,
};
use chaft_types::{
    DeviceId, EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES, EVENT_SIGNATURE_MAX_BYTES, EventBody,
    SignableEvent, SignedEvent, SignedTrustSnapshot, TrustSnapshot,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use getrandom::SysRng;
use rand_core::{Rng, UnwrapErr};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const ENCRYPTED_IDENTITY_SCHEMA_VERSION: u32 = 1;
const ENCRYPTED_IDENTITY_STORAGE: &str = "argon2id-aes-256-gcm-siv";
const ENCRYPTED_IDENTITY_KDF_ARGON2ID: &str = "argon2id";
const ENCRYPTED_IDENTITY_KDF_CONTEXT: &str = "Chaft device identity file v1";
const ENCRYPTED_IDENTITY_SALT_LEN: usize = 16;
const ENCRYPTED_IDENTITY_KEY_LEN: usize = 32;
const ENCRYPTED_IDENTITY_ARGON2_MEMORY_COST_KIB: u32 = 64 * 1024;
const ENCRYPTED_IDENTITY_ARGON2_TIME_COST: u32 = 3;
const ENCRYPTED_IDENTITY_ARGON2_PARALLELISM: u32 = 1;
const ENCRYPTED_IDENTITY_KDF_OUTPUT_LEN: u32 = ENCRYPTED_IDENTITY_KEY_LEN as u32;
const DEVICE_IDENTITY_FILE_MAX_BYTES: usize = 64 * 1024;
const DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES: usize = 16 * 1024;
const DEVICE_IDENTITY_PATH_MAX_BYTES: usize = 64 * 1024;

static IDENTITY_FILE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("identity serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("invalid signing key")]
    InvalidSigningKey,
    #[error("invalid verifying key")]
    InvalidVerifyingKey,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("event id does not match event content and signature")]
    EventIdMismatch,
    #[error("author public key does not match author device id")]
    DeviceIdMismatch,
    #[error("persisted identity device id does not match signing key")]
    PersistedDeviceIdMismatch,
    #[error("trust snapshot root event is invalid")]
    InvalidTrustSnapshotRoot,
    #[error("trust snapshot signer does not match workspace root owner")]
    TrustSnapshotSignerMismatch,
    #[error("encrypted identity passphrase is required")]
    EncryptedIdentityPassphraseRequired,
    #[error("encrypted identity passphrase is too large ({actual_bytes} bytes, max {max_bytes})")]
    EncryptedIdentityPassphraseTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("encrypted identity schema or KDF is unsupported")]
    UnsupportedEncryptedIdentity,
    #[error("encrypted identity contents do not match metadata")]
    InvalidEncryptedIdentity,
    #[error("identity encryption error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("identity KDF failed: {0}")]
    Kdf(String),
    #[error("identity file is too large ({actual_bytes} bytes, max {max_bytes})")]
    IdentityFileTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("identity file path is required")]
    IdentityPathRequired,
    #[error("identity file path is too large ({actual_bytes} bytes, max {max_bytes})")]
    IdentityPathTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    device_id: DeviceId,
    signing_key: SigningKey,
}

#[derive(Clone)]
pub struct InvitationCapability {
    signing_key: SigningKey,
}

impl InvitationCapability {
    pub fn generate() -> Self {
        let mut rng = UnwrapErr(SysRng);
        Self {
            signing_key: SigningKey::generate(&mut rng),
        }
    }

    pub fn from_secret_bytes(bytes: [u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&bytes),
        }
    }

    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        self.signing_key.sign(bytes).to_bytes().to_vec()
    }
}

impl DeviceIdentity {
    pub fn generate() -> Self {
        let mut rng = UnwrapErr(SysRng);
        let signing_key = SigningKey::generate(&mut rng);
        Self::from_signing_key(signing_key)
    }

    pub fn from_signing_key_bytes(bytes: [u8; 32]) -> Self {
        Self::from_signing_key(SigningKey::from_bytes(&bytes))
    }

    fn from_signing_key(signing_key: SigningKey) -> Self {
        let device_id = DeviceId::from_public_key_bytes(&signing_key.verifying_key().to_bytes());

        Self {
            device_id,
            signing_key,
        }
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    pub fn signing_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    pub fn sign_bytes(&self, bytes: &[u8]) -> Vec<u8> {
        self.signing_key.sign(bytes).to_bytes().to_vec()
    }

    pub fn load_or_generate(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        Self::load_or_generate_with_passphrase(path, None)
    }

    pub fn load_or_generate_with_passphrase(
        path: impl AsRef<Path>,
        passphrase: Option<&str>,
    ) -> Result<Self, IdentityError> {
        let path = path.as_ref();
        validate_identity_path(path)?;
        validate_identity_passphrase(passphrase)?;
        if path.exists() {
            Self::load_from_file_with_passphrase(path, passphrase)
        } else {
            let identity = Self::generate();
            let bytes = identity.persisted_bytes(passphrase)?;
            if write_new_identity_file(path, &bytes)? {
                Ok(identity)
            } else {
                Self::load_from_file_with_passphrase(path, passphrase)
            }
        }
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        Self::load_from_file_with_passphrase(path, None)
    }

    pub fn load_from_file_with_passphrase(
        path: impl AsRef<Path>,
        passphrase: Option<&str>,
    ) -> Result<Self, IdentityError> {
        validate_identity_path(path.as_ref())?;
        validate_identity_passphrase(passphrase)?;
        let bytes = read_identity_file(path.as_ref())?;
        if let Ok(encrypted) = serde_json::from_slice::<PersistedEncryptedDeviceIdentity>(&bytes)
            && encrypted.storage == ENCRYPTED_IDENTITY_STORAGE
        {
            return Self::from_encrypted_persisted(encrypted, passphrase);
        }

        let persisted: PersistedDeviceIdentity = serde_json::from_slice(&bytes)?;
        Self::from_persisted(persisted)
    }

    fn from_persisted(persisted: PersistedDeviceIdentity) -> Result<Self, IdentityError> {
        if persisted.schema_version != 1 {
            return Err(IdentityError::InvalidSigningKey);
        }

        let identity =
            Self::from_signing_key_bytes(decode_hex_32(&persisted.ed25519_signing_key_hex)?);
        if identity.device_id != persisted.device_id {
            return Err(IdentityError::PersistedDeviceIdMismatch);
        }
        Ok(identity)
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), IdentityError> {
        self.save_to_file_with_passphrase(path, None)
    }

    pub fn save_to_file_with_passphrase(
        &self,
        path: impl AsRef<Path>,
        passphrase: Option<&str>,
    ) -> Result<(), IdentityError> {
        let path = path.as_ref();
        validate_identity_path(path)?;
        validate_identity_passphrase(passphrase)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bytes = self.persisted_bytes(passphrase)?;
        write_identity_file(path, &bytes)
    }

    fn persisted_bytes(&self, passphrase: Option<&str>) -> Result<Vec<u8>, IdentityError> {
        let persisted = PersistedDeviceIdentity {
            schema_version: 1,
            device_id: self.device_id.clone(),
            ed25519_signing_key_hex: encode_hex(&self.signing_key_bytes()),
        };
        Ok(match passphrase {
            Some(passphrase) if !passphrase.trim().is_empty() => {
                serde_json::to_vec_pretty(&encrypt_persisted_identity(&persisted, passphrase)?)?
            }
            Some(_) => return Err(IdentityError::EncryptedIdentityPassphraseRequired),
            None => serde_json::to_vec_pretty(&persisted)?,
        })
    }

    fn from_encrypted_persisted(
        encrypted: PersistedEncryptedDeviceIdentity,
        passphrase: Option<&str>,
    ) -> Result<Self, IdentityError> {
        if encrypted.schema_version != ENCRYPTED_IDENTITY_SCHEMA_VERSION {
            return Err(IdentityError::UnsupportedEncryptedIdentity);
        }
        validate_identity_passphrase(passphrase)?;
        let Some(passphrase) = passphrase.filter(|passphrase| !passphrase.trim().is_empty()) else {
            return Err(IdentityError::EncryptedIdentityPassphraseRequired);
        };

        let wrapping_key = derive_encrypted_identity_key(passphrase, &encrypted.kdf)?;
        let aad = encrypted_identity_aad(
            &encrypted.device_id,
            encrypted.kdf.name.as_str(),
            encrypted.kdf.context.as_str(),
            &encrypted.kdf.salt,
        );
        if encrypted.sealed_payload.aad != aad {
            return Err(IdentityError::InvalidEncryptedIdentity);
        }
        let plaintext = open_aes_256_gcm_siv(&wrapping_key, &encrypted.sealed_payload)?;
        let persisted = serde_json::from_slice::<PersistedDeviceIdentity>(&plaintext)?;
        if persisted.device_id != encrypted.device_id {
            return Err(IdentityError::InvalidEncryptedIdentity);
        }
        Self::from_persisted(persisted)
    }
}

fn validate_identity_path(path: &Path) -> Result<(), IdentityError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(IdentityError::IdentityPathRequired);
    }
    if actual_bytes > DEVICE_IDENTITY_PATH_MAX_BYTES {
        return Err(IdentityError::IdentityPathTooLarge {
            actual_bytes,
            max_bytes: DEVICE_IDENTITY_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_identity_passphrase(passphrase: Option<&str>) -> Result<(), IdentityError> {
    if let Some(passphrase) = passphrase
        && passphrase.len() > DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES
    {
        return Err(IdentityError::EncryptedIdentityPassphraseTooLarge {
            actual_bytes: passphrase.len(),
            max_bytes: DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES,
        });
    }
    Ok(())
}

fn read_identity_file(path: &Path) -> Result<Vec<u8>, IdentityError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > DEVICE_IDENTITY_FILE_MAX_BYTES as u64 {
        return Err(IdentityError::IdentityFileTooLarge {
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes: DEVICE_IDENTITY_FILE_MAX_BYTES,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(DEVICE_IDENTITY_FILE_MAX_BYTES as u64 + 1);
    let mut bytes =
        Vec::with_capacity(metadata.len().min(DEVICE_IDENTITY_FILE_MAX_BYTES as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() > DEVICE_IDENTITY_FILE_MAX_BYTES {
        return Err(IdentityError::IdentityFileTooLarge {
            actual_bytes: bytes.len(),
            max_bytes: DEVICE_IDENTITY_FILE_MAX_BYTES,
        });
    }
    Ok(bytes)
}

fn write_identity_file(path: &Path, bytes: &[u8]) -> Result<(), IdentityError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_identity_temp_file(path)?;
    let result = (|| -> Result<(), IdentityError> {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_identity_parent_directory(parent)?;
        }
        Ok(())
    })();

    if result.is_err() {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    result?;

    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn write_new_identity_file(path: &Path, bytes: &[u8]) -> Result<bool, IdentityError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_identity_temp_file(path)?;
    let result = (|| -> Result<bool, IdentityError> {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        #[cfg(unix)]
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;

        let created = match fs::hard_link(&temp_path, path) {
            Ok(()) => true,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(error) => return Err(error.into()),
        };
        fs::remove_file(&temp_path)?;
        if created
            && let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_identity_parent_directory(parent)?;
        }
        Ok(created)
    })();

    if result.is_err() {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    result
}

fn create_unique_identity_temp_file(path: &Path) -> Result<(PathBuf, fs::File), IdentityError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            IdentityError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "identity file path has no file name",
            ))
        })?;

    for _ in 0..32 {
        let counter = IDENTITY_FILE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_file_name(format!(".{file_name}.tmp.{}.{}", process::id(), counter));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(IdentityError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique identity temp file",
    )))
}

fn sync_identity_parent_directory(parent: &Path) -> Result<(), IdentityError> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
    }
    Ok(())
}

impl DeviceIdentity {
    pub fn sign_event(&self, event: SignableEvent) -> SignedEvent {
        let signature = self.signing_key.sign(&event.signing_bytes());
        SignedEvent::from_author_signature(
            event,
            self.verifying_key_bytes().to_vec(),
            signature.to_bytes().to_vec(),
        )
    }

    pub fn sign_trust_snapshot(
        &self,
        snapshot: TrustSnapshot,
        root_event: SignedEvent,
    ) -> Result<SignedTrustSnapshot, IdentityError> {
        if self.device_id != snapshot.root_author_device_id {
            return Err(IdentityError::TrustSnapshotSignerMismatch);
        }
        validate_trust_snapshot_root(&snapshot, &root_event)?;
        let signature = self.signing_key.sign(&snapshot.signing_bytes());
        Ok(SignedTrustSnapshot {
            snapshot,
            root_event,
            author_public_key: self.verifying_key_bytes().to_vec(),
            signature: signature.to_bytes().to_vec(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedDeviceIdentity {
    schema_version: u32,
    device_id: DeviceId,
    ed25519_signing_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEncryptedDeviceIdentity {
    schema_version: u32,
    storage: String,
    device_id: DeviceId,
    kdf: EncryptedIdentityKdf,
    sealed_payload: SealedPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedIdentityKdf {
    name: String,
    context: String,
    salt: Vec<u8>,
    memory_cost_kib: u32,
    time_cost: u32,
    parallelism: u32,
    output_len: u32,
}

fn encrypt_persisted_identity(
    persisted: &PersistedDeviceIdentity,
    passphrase: &str,
) -> Result<PersistedEncryptedDeviceIdentity, IdentityError> {
    let mut salt = [0; ENCRYPTED_IDENTITY_SALT_LEN];
    UnwrapErr(SysRng).fill_bytes(&mut salt);
    let kdf = EncryptedIdentityKdf {
        name: ENCRYPTED_IDENTITY_KDF_ARGON2ID.to_owned(),
        context: ENCRYPTED_IDENTITY_KDF_CONTEXT.to_owned(),
        salt: salt.to_vec(),
        memory_cost_kib: ENCRYPTED_IDENTITY_ARGON2_MEMORY_COST_KIB,
        time_cost: ENCRYPTED_IDENTITY_ARGON2_TIME_COST,
        parallelism: ENCRYPTED_IDENTITY_ARGON2_PARALLELISM,
        output_len: ENCRYPTED_IDENTITY_KDF_OUTPUT_LEN,
    };
    let wrapping_key = derive_encrypted_identity_key(passphrase, &kdf)?;
    let aad = encrypted_identity_aad(
        &persisted.device_id,
        kdf.name.as_str(),
        kdf.context.as_str(),
        &kdf.salt,
    );
    let plaintext = serde_json::to_vec(persisted)?;
    let sealed_payload = seal_aes_256_gcm_siv(
        encrypted_identity_key_id(&persisted.device_id),
        &wrapping_key,
        &plaintext,
        &aad,
    )?;

    Ok(PersistedEncryptedDeviceIdentity {
        schema_version: ENCRYPTED_IDENTITY_SCHEMA_VERSION,
        storage: ENCRYPTED_IDENTITY_STORAGE.to_owned(),
        device_id: persisted.device_id.clone(),
        kdf,
        sealed_payload,
    })
}

fn derive_encrypted_identity_key(
    passphrase: &str,
    kdf: &EncryptedIdentityKdf,
) -> Result<ContentKey, IdentityError> {
    if kdf.name != ENCRYPTED_IDENTITY_KDF_ARGON2ID
        || kdf.context != ENCRYPTED_IDENTITY_KDF_CONTEXT
        || kdf.salt.len() != ENCRYPTED_IDENTITY_SALT_LEN
        || kdf.memory_cost_kib != ENCRYPTED_IDENTITY_ARGON2_MEMORY_COST_KIB
        || kdf.time_cost != ENCRYPTED_IDENTITY_ARGON2_TIME_COST
        || kdf.parallelism != ENCRYPTED_IDENTITY_ARGON2_PARALLELISM
        || kdf.output_len != ENCRYPTED_IDENTITY_KDF_OUTPUT_LEN
    {
        return Err(IdentityError::UnsupportedEncryptedIdentity);
    }

    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(kdf.output_len as usize),
    )
    .map_err(|error| IdentityError::Kdf(format!("{error:?}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut bytes = [0; ENCRYPTED_IDENTITY_KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, &mut bytes)
        .map_err(|error| IdentityError::Kdf(format!("{error:?}")))?;
    Ok(ContentKey::from_bytes(bytes))
}

fn encrypted_identity_aad(
    device_id: &DeviceId,
    kdf_name: &str,
    kdf_context: &str,
    salt: &[u8],
) -> Vec<u8> {
    let mut aad = format!(
        "chaft:v1:device_identity:{}:{}:{}:",
        device_id.0, kdf_name, kdf_context
    )
    .into_bytes();
    aad.extend_from_slice(salt);
    aad
}

fn encrypted_identity_key_id(device_id: &DeviceId) -> String {
    format!("device-identity:{}", device_id.0)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex_32(input: &str) -> Result<[u8; 32], IdentityError> {
    if input.len() != 64 {
        return Err(IdentityError::InvalidSigningKey);
    }

    let mut out = [0; 32];
    for (index, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (decode_hex_nibble(chunk[0])? << 4) | decode_hex_nibble(chunk[1])?;
    }
    Ok(out)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, IdentityError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(IdentityError::InvalidSigningKey),
    }
}

pub fn verify_event(
    event: &SignedEvent,
    verifying_key_bytes: &[u8; 32],
) -> Result<(), IdentityError> {
    validate_signature_len(&event.signature)?;
    let verifying_key = VerifyingKey::from_bytes(verifying_key_bytes)
        .map_err(|_| IdentityError::InvalidVerifyingKey)?;
    let signature =
        Signature::from_slice(&event.signature).map_err(|_| IdentityError::InvalidSignature)?;

    verifying_key
        .verify(&event.event.signing_bytes(), &signature)
        .map_err(|_| IdentityError::InvalidSignature)
}

pub fn verify_detached_signature(
    verifying_key_bytes: &[u8; 32],
    bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), IdentityError> {
    if signature_bytes.len() != 64 {
        return Err(IdentityError::InvalidSignature);
    }
    let verifying_key = VerifyingKey::from_bytes(verifying_key_bytes)
        .map_err(|_| IdentityError::InvalidVerifyingKey)?;
    let signature =
        Signature::from_slice(signature_bytes).map_err(|_| IdentityError::InvalidSignature)?;
    verifying_key
        .verify(bytes, &signature)
        .map_err(|_| IdentityError::InvalidSignature)
}

pub fn verify_device_detached_signature(
    device_id: &DeviceId,
    verifying_key_bytes: &[u8; 32],
    bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<(), IdentityError> {
    if DeviceId::from_public_key_bytes(verifying_key_bytes) != *device_id {
        return Err(IdentityError::DeviceIdMismatch);
    }
    verify_detached_signature(verifying_key_bytes, bytes, signature_bytes)
}

pub fn verify_self_contained_event(event: &SignedEvent) -> Result<(), IdentityError> {
    validate_author_public_key_len(&event.author_public_key)?;
    validate_signature_len(&event.signature)?;
    let verifying_key_bytes: [u8; 32] = event
        .author_public_key
        .as_slice()
        .try_into()
        .map_err(|_| IdentityError::InvalidVerifyingKey)?;
    let derived_device_id = DeviceId::from_public_key_bytes(&verifying_key_bytes);
    if derived_device_id != event.event.author_device_id {
        return Err(IdentityError::DeviceIdMismatch);
    }
    let canonical_event_id = SignedEvent::from_author_signature(
        event.event.clone(),
        event.author_public_key.clone(),
        event.signature.clone(),
    )
    .event_id;
    if canonical_event_id != event.event_id {
        return Err(IdentityError::EventIdMismatch);
    }

    verify_event(event, &verifying_key_bytes)
}

pub fn verify_self_contained_trust_snapshot(
    snapshot: &SignedTrustSnapshot,
) -> Result<(), IdentityError> {
    validate_trust_snapshot_root(&snapshot.snapshot, &snapshot.root_event)?;
    validate_author_public_key_len(&snapshot.author_public_key)?;
    validate_signature_len(&snapshot.signature)?;
    let verifying_key_bytes: [u8; 32] = snapshot
        .author_public_key
        .as_slice()
        .try_into()
        .map_err(|_| IdentityError::InvalidVerifyingKey)?;
    let derived_device_id = DeviceId::from_public_key_bytes(&verifying_key_bytes);
    if derived_device_id != snapshot.snapshot.root_author_device_id {
        return Err(IdentityError::TrustSnapshotSignerMismatch);
    }
    if snapshot.root_event.author_public_key != snapshot.author_public_key {
        return Err(IdentityError::TrustSnapshotSignerMismatch);
    }

    let verifying_key = VerifyingKey::from_bytes(&verifying_key_bytes)
        .map_err(|_| IdentityError::InvalidVerifyingKey)?;
    let signature =
        Signature::from_slice(&snapshot.signature).map_err(|_| IdentityError::InvalidSignature)?;
    verifying_key
        .verify(&snapshot.snapshot.signing_bytes(), &signature)
        .map_err(|_| IdentityError::InvalidSignature)
}

fn validate_author_public_key_len(bytes: &[u8]) -> Result<(), IdentityError> {
    if bytes.len() > EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES {
        return Err(IdentityError::InvalidVerifyingKey);
    }
    Ok(())
}

fn validate_signature_len(bytes: &[u8]) -> Result<(), IdentityError> {
    if bytes.len() > EVENT_SIGNATURE_MAX_BYTES {
        return Err(IdentityError::InvalidSignature);
    }
    Ok(())
}

fn validate_trust_snapshot_root(
    snapshot: &TrustSnapshot,
    root_event: &SignedEvent,
) -> Result<(), IdentityError> {
    verify_self_contained_event(root_event)?;
    if snapshot.schema_version != 1
        || root_event.event_id != snapshot.root_event_id
        || root_event.event.workspace_id != snapshot.workspace_id
        || root_event.event.author_device_id != snapshot.root_author_device_id
        || !matches!(root_event.event.body, EventBody::WorkspaceCreated { .. })
    {
        return Err(IdentityError::InvalidTrustSnapshotRoot);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
    };

    use chaft_types::{ChannelId, EventBody, EventId, MessageId, SignableEvent, WorkspaceId};

    use super::*;

    const RFC8032_TEST_1_SIGNING_KEY_HEX: &str =
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    const RFC8032_TEST_1_VERIFYING_KEY_HEX: &str =
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const RFC8032_TEST_1_DEVICE_ID: &str =
        "dev_6c31041268f471609c79f5f2dbcc38e4a4ab2f4d416109a4e09fcf50fd0f0062";
    const RFC8032_TEST_1_EMPTY_MESSAGE_SIGNATURE_HEX: &str = concat!(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
        "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    );

    fn identity_temp_artifacts_under(root: &Path) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();
        collect_identity_temp_artifacts(root, &mut artifacts);
        artifacts.sort();
        artifacts
    }

    fn collect_identity_temp_artifacts(root: &Path, artifacts: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                collect_identity_temp_artifacts(&path, artifacts);
                continue;
            }
            if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.contains(".tmp."))
            {
                artifacts.push(path);
            }
        }
    }

    fn assert_identity_passphrase_too_large<T>(result: Result<T, IdentityError>) {
        match result {
            Err(IdentityError::EncryptedIdentityPassphraseTooLarge {
                actual_bytes,
                max_bytes,
            }) if actual_bytes > DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES
                && max_bytes == DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized identity passphrase error, got ok"),
            Err(error) => panic!("expected oversized identity passphrase error, got {error}"),
        }
    }

    fn assert_identity_path_too_large<T>(result: Result<T, IdentityError>) {
        match result {
            Err(IdentityError::IdentityPathTooLarge {
                actual_bytes,
                max_bytes,
            }) if actual_bytes > DEVICE_IDENTITY_PATH_MAX_BYTES
                && max_bytes == DEVICE_IDENTITY_PATH_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized identity path error, got ok"),
            Err(error) => panic!("expected oversized identity path error, got {error}"),
        }
    }

    #[test]
    fn generated_device_can_sign_and_verify_event() {
        let identity = DeviceIdentity::generate();
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "p2p hello".to_owned(),
                attachments: Vec::new(),
            },
        );

        let signed = identity.sign_event(event);

        verify_event(&signed, &identity.verifying_key_bytes()).unwrap();
        verify_self_contained_event(&signed).unwrap();
    }

    #[test]
    fn signing_key_bytes_recreate_same_device_identity() {
        let identity = DeviceIdentity::generate();
        let reloaded = DeviceIdentity::from_signing_key_bytes(identity.signing_key_bytes());

        assert_eq!(identity.device_id(), reloaded.device_id());
        assert_eq!(
            identity.verifying_key_bytes(),
            reloaded.verifying_key_bytes()
        );
    }

    #[test]
    fn rfc8032_seed_preserves_identity_and_signature_bytes() {
        fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<SigningKey>();

        let identity = DeviceIdentity::from_signing_key_bytes(
            decode_hex_32(RFC8032_TEST_1_SIGNING_KEY_HEX).unwrap(),
        );

        assert_eq!(
            encode_hex(&identity.verifying_key_bytes()),
            RFC8032_TEST_1_VERIFYING_KEY_HEX
        );
        assert_eq!(identity.device_id().0, RFC8032_TEST_1_DEVICE_ID);
        assert_eq!(
            encode_hex(&identity.sign_bytes(b"")),
            RFC8032_TEST_1_EMPTY_MESSAGE_SIGNATURE_HEX
        );
        verify_device_detached_signature(
            identity.device_id(),
            &identity.verifying_key_bytes(),
            b"",
            &identity.sign_bytes(b""),
        )
        .unwrap();
    }

    #[test]
    fn frozen_v1_plaintext_identity_file_still_loads() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let frozen_identity = format!(
            concat!(
                r#"{{"schema_version":1,"device_id":"{}","#,
                r#""ed25519_signing_key_hex":"{}"}}"#,
            ),
            RFC8032_TEST_1_DEVICE_ID, RFC8032_TEST_1_SIGNING_KEY_HEX,
        );
        fs::write(&identity_path, frozen_identity).unwrap();

        let identity = DeviceIdentity::load_from_file(&identity_path).unwrap();

        assert_eq!(identity.device_id().0, RFC8032_TEST_1_DEVICE_ID);
        assert_eq!(
            encode_hex(&identity.signing_key_bytes()),
            RFC8032_TEST_1_SIGNING_KEY_HEX
        );
        assert_eq!(
            encode_hex(&identity.verifying_key_bytes()),
            RFC8032_TEST_1_VERIFYING_KEY_HEX
        );
        assert_eq!(
            encode_hex(&identity.sign_bytes(b"")),
            RFC8032_TEST_1_EMPTY_MESSAGE_SIGNATURE_HEX
        );
    }

    #[test]
    fn file_backed_identity_survives_restart() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let first = DeviceIdentity::load_or_generate(&identity_path).unwrap();
        let second = DeviceIdentity::load_or_generate(&identity_path).unwrap();

        assert_eq!(first.device_id(), second.device_id());
        assert_eq!(first.verifying_key_bytes(), second.verifying_key_bytes());

        #[cfg(unix)]
        {
            let mode = fs::metadata(&identity_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn concurrent_identity_file_writes_use_unique_temp_files() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let identities = (0..8)
            .map(|_| DeviceIdentity::generate())
            .collect::<Vec<_>>();
        let expected_device_ids = identities
            .iter()
            .map(|identity| identity.device_id().clone())
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(identities.len()));
        let handles = identities
            .into_iter()
            .map(|identity| {
                let identity_path = identity_path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    identity.save_to_file(&identity_path).unwrap();
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        let persisted = DeviceIdentity::load_from_file(&identity_path).unwrap();
        assert!(expected_device_ids.contains(persisted.device_id()));
        assert!(identity_temp_artifacts_under(tempdir.path()).is_empty());
        #[cfg(unix)]
        {
            let mode = fs::metadata(&identity_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn concurrent_load_or_generate_calls_converge_on_one_identity() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let worker_count = 8;
        let barrier = Arc::new(Barrier::new(worker_count));
        let handles = (0..worker_count)
            .map(|_| {
                let identity_path = identity_path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    DeviceIdentity::load_or_generate(identity_path)
                        .unwrap()
                        .device_id()
                        .clone()
                })
            })
            .collect::<Vec<_>>();

        let device_ids = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        let persisted = DeviceIdentity::load_from_file(&identity_path).unwrap();

        assert!(
            device_ids
                .iter()
                .all(|device_id| device_id == persisted.device_id())
        );
        assert!(identity_temp_artifacts_under(tempdir.path()).is_empty());
    }

    #[test]
    fn oversized_identity_file_is_rejected_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let file = fs::File::create(&identity_path).unwrap();
        file.set_len(DEVICE_IDENTITY_FILE_MAX_BYTES as u64 + 1)
            .unwrap();

        assert!(matches!(
            DeviceIdentity::load_from_file(&identity_path),
            Err(IdentityError::IdentityFileTooLarge {
                actual_bytes,
                max_bytes: DEVICE_IDENTITY_FILE_MAX_BYTES,
            }) if actual_bytes == DEVICE_IDENTITY_FILE_MAX_BYTES + 1
        ));
    }

    #[test]
    fn blank_identity_path_is_rejected_before_filesystem_work() {
        assert!(matches!(
            DeviceIdentity::load_or_generate(PathBuf::new()),
            Err(IdentityError::IdentityPathRequired)
        ));
        assert!(matches!(
            DeviceIdentity::load_from_file(PathBuf::new()),
            Err(IdentityError::IdentityPathRequired)
        ));
        assert!(matches!(
            DeviceIdentity::generate().save_to_file(PathBuf::new()),
            Err(IdentityError::IdentityPathRequired)
        ));
    }

    #[test]
    fn oversized_identity_path_is_rejected_before_generate_or_file_work() {
        assert_identity_path_too_large(DeviceIdentity::load_or_generate(PathBuf::from(
            "d".repeat(DEVICE_IDENTITY_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn oversized_identity_path_is_rejected_before_identity_file_read() {
        assert_identity_path_too_large(DeviceIdentity::load_from_file(PathBuf::from(
            "d".repeat(DEVICE_IDENTITY_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn oversized_identity_path_is_rejected_before_identity_file_write() {
        assert_identity_path_too_large(DeviceIdentity::generate().save_to_file(PathBuf::from(
            "d".repeat(DEVICE_IDENTITY_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn oversized_identity_passphrase_is_rejected_before_generate_or_file_work() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("missing").join("device.json");
        let parent = identity_path.parent().unwrap().to_path_buf();
        let passphrase = "p".repeat(DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES + 1);

        assert_identity_passphrase_too_large(DeviceIdentity::load_or_generate_with_passphrase(
            &identity_path,
            Some(&passphrase),
        ));
        assert!(!parent.exists());
    }

    #[test]
    fn oversized_identity_passphrase_is_rejected_before_identity_file_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("missing-device.json");
        let passphrase = "p".repeat(DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES + 1);

        assert_identity_passphrase_too_large(DeviceIdentity::load_from_file_with_passphrase(
            &identity_path,
            Some(&passphrase),
        ));
    }

    #[test]
    fn oversized_identity_passphrase_is_rejected_before_identity_file_write() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity = DeviceIdentity::generate();
        let identity_path = tempdir.path().join("missing").join("device.json");
        let parent = identity_path.parent().unwrap().to_path_buf();
        let passphrase = "p".repeat(DEVICE_IDENTITY_PASSPHRASE_MAX_BYTES + 1);

        assert_identity_passphrase_too_large(
            identity.save_to_file_with_passphrase(&identity_path, Some(&passphrase)),
        );
        assert!(!parent.exists());
    }

    #[test]
    fn encrypted_identity_file_round_trips_with_passphrase_and_hides_seed() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        let first =
            DeviceIdentity::load_or_generate_with_passphrase(&identity_path, Some("correct horse"))
                .unwrap();
        let signing_key_hex = encode_hex(&first.signing_key_bytes());
        let second =
            DeviceIdentity::load_or_generate_with_passphrase(&identity_path, Some("correct horse"))
                .unwrap();
        let file = fs::read_to_string(&identity_path).unwrap();

        assert_eq!(first.device_id(), second.device_id());
        assert_eq!(first.verifying_key_bytes(), second.verifying_key_bytes());
        assert!(file.contains(ENCRYPTED_IDENTITY_STORAGE));
        assert!(!file.contains("ed25519_signing_key_hex"));
        assert!(!file.contains(&signing_key_hex));

        #[cfg(unix)]
        {
            let mode = fs::metadata(&identity_path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn encrypted_identity_requires_passphrase() {
        let tempdir = tempfile::tempdir().unwrap();
        let identity_path = tempdir.path().join("device.json");
        DeviceIdentity::load_or_generate_with_passphrase(&identity_path, Some("correct horse"))
            .unwrap();

        assert!(matches!(
            DeviceIdentity::load_from_file(&identity_path),
            Err(IdentityError::EncryptedIdentityPassphraseRequired)
        ));
        assert!(matches!(
            DeviceIdentity::load_from_file_with_passphrase(&identity_path, Some("wrong horse")),
            Err(IdentityError::Crypto(CryptoError::OpenFailed))
        ));
    }

    #[test]
    fn self_contained_verification_rejects_mismatched_author_device() {
        let identity = DeviceIdentity::generate();
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            DeviceId("dev_wrong".to_owned()),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "p2p hello".to_owned(),
                attachments: Vec::new(),
            },
        );

        let signed = identity.sign_event(event);

        assert!(matches!(
            verify_self_contained_event(&signed),
            Err(IdentityError::DeviceIdMismatch)
        ));
    }

    #[test]
    fn self_contained_verification_rejects_mismatched_event_id() {
        let identity = DeviceIdentity::generate();
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "p2p hello".to_owned(),
                attachments: Vec::new(),
            },
        );
        let mut signed = identity.sign_event(event);
        signed.event_id = EventId("evt_not_the_canonical_hash".to_owned());

        assert!(matches!(
            verify_self_contained_event(&signed),
            Err(IdentityError::EventIdMismatch)
        ));
    }

    #[test]
    fn self_contained_verification_rejects_oversized_signature_material() {
        let identity = DeviceIdentity::generate();
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "p2p hello".to_owned(),
                attachments: Vec::new(),
            },
        );

        let mut oversized_public_key = identity.sign_event(event.clone());
        oversized_public_key.author_public_key = vec![0; EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES + 1];
        assert!(matches!(
            verify_self_contained_event(&oversized_public_key),
            Err(IdentityError::InvalidVerifyingKey)
        ));

        let mut oversized_signature = identity.sign_event(event);
        oversized_signature.signature = vec![0; EVENT_SIGNATURE_MAX_BYTES + 1];
        assert!(matches!(
            verify_self_contained_event(&oversized_signature),
            Err(IdentityError::InvalidSignature)
        ));
    }

    #[test]
    fn trust_snapshot_verification_rejects_oversized_signature_material() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let root_event = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let snapshot = TrustSnapshot {
            schema_version: 1,
            workspace_id,
            root_event_id: root_event.event_id.clone(),
            root_author_device_id: identity.device_id().clone(),
            roles: Vec::new(),
            channels: Vec::new(),
            messages: Vec::new(),
            event_channels: Vec::new(),
            person_device_links: Vec::new(),
        };

        let mut oversized_public_key = identity
            .sign_trust_snapshot(snapshot.clone(), root_event.clone())
            .unwrap();
        oversized_public_key.author_public_key = vec![0; EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES + 1];
        assert!(matches!(
            verify_self_contained_trust_snapshot(&oversized_public_key),
            Err(IdentityError::InvalidVerifyingKey)
        ));

        let mut oversized_signature = identity.sign_trust_snapshot(snapshot, root_event).unwrap();
        oversized_signature.signature = vec![0; EVENT_SIGNATURE_MAX_BYTES + 1];
        assert!(matches!(
            verify_self_contained_trust_snapshot(&oversized_signature),
            Err(IdentityError::InvalidSignature)
        ));
    }
}
