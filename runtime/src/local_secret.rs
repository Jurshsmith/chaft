use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use chaft_crypto::{ContentKey, SealedPayload, open_aes_256_gcm_siv, seal_aes_256_gcm_siv};
use getrandom::SysRng;
use rand_core::{Rng, UnwrapErr};
use serde::{Deserialize, Serialize};

use crate::RuntimeError;

const LOCAL_SECRET_SCHEMA_VERSION: u32 = 1;
pub(crate) const LOCAL_SECRET_STORAGE: &str = "argon2id-aes-256-gcm-siv";
const LOCAL_SECRET_KDF_ARGON2ID: &str = "argon2id";
const LOCAL_SECRET_KDF_CONTEXT: &str = "Chaft local secret file v1";
const LOCAL_SECRET_SALT_LEN: usize = 16;
const LOCAL_SECRET_KEY_LEN: usize = 32;
const LOCAL_SECRET_ARGON2_MEMORY_COST_KIB: u32 = 64 * 1024;
const LOCAL_SECRET_ARGON2_TIME_COST: u32 = 3;
const LOCAL_SECRET_ARGON2_PARALLELISM: u32 = 1;
const LOCAL_SECRET_KDF_OUTPUT_LEN: u32 = LOCAL_SECRET_KEY_LEN as u32;

pub(crate) const LOCAL_SECRET_FILE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const LOCAL_SECRET_KIND_WORKSPACE_KEY: &str = "workspace-key";
pub(crate) const LOCAL_SECRET_KIND_CHANNEL_KEY: &str = "channel-key";
pub(crate) const LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE: &str = "openmls-key-package";
pub(crate) const LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP: &str = "openmls-workspace-group";
pub(crate) const LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP: &str = "openmls-channel-group";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedEncryptedLocalSecret {
    schema_version: u32,
    storage: String,
    secret_kind: String,
    path_hint: String,
    kdf: LocalSecretKdf,
    sealed_payload: SealedPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalSecretKdf {
    name: String,
    context: String,
    salt: Vec<u8>,
    memory_cost_kib: u32,
    time_cost: u32,
    parallelism: u32,
    output_len: u32,
}

pub(crate) fn encrypt_local_secret(
    secret_kind: &str,
    path_hint: &str,
    passphrase: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, RuntimeError> {
    let mut salt = [0; LOCAL_SECRET_SALT_LEN];
    UnwrapErr(SysRng).fill_bytes(&mut salt);
    let kdf = LocalSecretKdf {
        name: LOCAL_SECRET_KDF_ARGON2ID.to_owned(),
        context: LOCAL_SECRET_KDF_CONTEXT.to_owned(),
        salt: salt.to_vec(),
        memory_cost_kib: LOCAL_SECRET_ARGON2_MEMORY_COST_KIB,
        time_cost: LOCAL_SECRET_ARGON2_TIME_COST,
        parallelism: LOCAL_SECRET_ARGON2_PARALLELISM,
        output_len: LOCAL_SECRET_KDF_OUTPUT_LEN,
    };
    let wrapping_key = derive_local_secret_key(passphrase, &kdf)?;
    let aad = local_secret_aad(secret_kind, path_hint, &kdf);
    let sealed_payload = seal_aes_256_gcm_siv(
        local_secret_key_id(secret_kind, path_hint),
        &wrapping_key,
        plaintext,
        &aad,
    )?;

    Ok(serde_json::to_vec_pretty(&PersistedEncryptedLocalSecret {
        schema_version: LOCAL_SECRET_SCHEMA_VERSION,
        storage: LOCAL_SECRET_STORAGE.to_owned(),
        secret_kind: secret_kind.to_owned(),
        path_hint: path_hint.to_owned(),
        kdf,
        sealed_payload,
    })?)
}

pub(crate) fn open_serialized_local_secret(
    bytes: &[u8],
    secret_kind: &str,
    path_hint: &str,
    passphrase: Option<&str>,
) -> Result<Option<Vec<u8>>, RuntimeError> {
    let Ok(encrypted) = serde_json::from_slice::<PersistedEncryptedLocalSecret>(bytes) else {
        return Ok(None);
    };
    if encrypted.storage != LOCAL_SECRET_STORAGE {
        return Ok(None);
    }
    open_local_secret(encrypted, secret_kind, path_hint, passphrase).map(Some)
}

fn open_local_secret(
    encrypted: PersistedEncryptedLocalSecret,
    secret_kind: &str,
    path_hint: &str,
    passphrase: Option<&str>,
) -> Result<Vec<u8>, RuntimeError> {
    if encrypted.schema_version != LOCAL_SECRET_SCHEMA_VERSION
        || encrypted.storage != LOCAL_SECRET_STORAGE
        || encrypted.secret_kind != secret_kind
    {
        return Err(RuntimeError::UnsupportedLocalSecretFile);
    }
    if encrypted.path_hint != path_hint {
        return Err(RuntimeError::InvalidLocalSecretFile);
    }
    let Some(passphrase) = passphrase.filter(|passphrase| !passphrase.trim().is_empty()) else {
        return Err(RuntimeError::LocalSecretPassphraseRequired);
    };

    let wrapping_key = derive_local_secret_key(passphrase, &encrypted.kdf)?;
    let aad = local_secret_aad(secret_kind, path_hint, &encrypted.kdf);
    if encrypted.sealed_payload.aad != aad {
        return Err(RuntimeError::InvalidLocalSecretFile);
    }
    open_aes_256_gcm_siv(&wrapping_key, &encrypted.sealed_payload).map_err(Into::into)
}

fn derive_local_secret_key(
    passphrase: &str,
    kdf: &LocalSecretKdf,
) -> Result<ContentKey, RuntimeError> {
    if kdf.name != LOCAL_SECRET_KDF_ARGON2ID
        || kdf.context != LOCAL_SECRET_KDF_CONTEXT
        || kdf.salt.len() != LOCAL_SECRET_SALT_LEN
        || kdf.memory_cost_kib != LOCAL_SECRET_ARGON2_MEMORY_COST_KIB
        || kdf.time_cost != LOCAL_SECRET_ARGON2_TIME_COST
        || kdf.parallelism != LOCAL_SECRET_ARGON2_PARALLELISM
        || kdf.output_len != LOCAL_SECRET_KDF_OUTPUT_LEN
    {
        return Err(RuntimeError::UnsupportedLocalSecretFile);
    }

    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(kdf.output_len as usize),
    )
    .map_err(|error| RuntimeError::LocalSecretKdf(format!("{error:?}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut bytes = [0; LOCAL_SECRET_KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, &mut bytes)
        .map_err(|error| RuntimeError::LocalSecretKdf(format!("{error:?}")))?;
    Ok(ContentKey::from_bytes(bytes))
}

fn local_secret_aad(secret_kind: &str, path_hint: &str, kdf: &LocalSecretKdf) -> Vec<u8> {
    let mut aad = format!(
        "chaft:v1:local_secret:{}:{}:{}:{}:",
        secret_kind, path_hint, kdf.name, kdf.context
    )
    .into_bytes();
    aad.extend_from_slice(&kdf.salt);
    aad
}

fn local_secret_key_id(secret_kind: &str, path_hint: &str) -> String {
    format!("local-secret:{secret_kind}:{path_hint}")
}

pub(crate) fn openmls_group_secret_kind(path: &Path) -> &'static str {
    if path.file_name().and_then(|name| name.to_str()) == Some("workspace.json") {
        LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP
    } else {
        LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP
    }
}
