use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use chaft_types::{ChannelId, MessageId, WorkspaceId};
pub use chaft_types::{EncryptedBlobRef, PayloadEncryption, SealedPayload};
use getrandom::SysRng;
use rand_core::{Rng, UnwrapErr};
use std::string::FromUtf8Error;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

const AES_256_GCM_SIV_KEY_LEN: usize = 32;
const AES_256_GCM_SIV_NONCE_LEN: usize = 12;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("payload is not development plaintext")]
    NotDevelopmentPlaintext,
    #[error("payload is not AES-256-GCM-SIV")]
    NotAes256GcmSiv,
    #[error("AES-256-GCM-SIV nonce must be 12 bytes")]
    InvalidNonceLength,
    #[error("associated data does not match sealed payload context")]
    AssociatedDataMismatch,
    #[error("authenticated encryption failed")]
    SealFailed,
    #[error("authenticated decryption failed")]
    OpenFailed,
    #[error("decrypted payload is not valid UTF-8")]
    InvalidUtf8(#[from] FromUtf8Error),
}

#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ContentKey([u8; AES_256_GCM_SIV_KEY_LEN]);

impl ContentKey {
    pub fn generate() -> Self {
        let mut bytes = [0; AES_256_GCM_SIV_KEY_LEN];
        UnwrapErr(SysRng).fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn from_bytes(bytes: [u8; AES_256_GCM_SIV_KEY_LEN]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; AES_256_GCM_SIV_KEY_LEN] {
        &self.0
    }
}

pub fn seal_development_plaintext(bytes: impl Into<Vec<u8>>) -> SealedPayload {
    SealedPayload {
        mode: PayloadEncryption::DevelopmentPlaintext,
        key_id: String::new(),
        nonce: Vec::new(),
        aad: Vec::new(),
        bytes: bytes.into(),
    }
}

pub fn open_development_plaintext(payload: &SealedPayload) -> Result<&[u8], CryptoError> {
    match payload.mode {
        PayloadEncryption::DevelopmentPlaintext => Ok(&payload.bytes),
        PayloadEncryption::Aes256GcmSiv | PayloadEncryption::OpenMlsPending => {
            Err(CryptoError::NotDevelopmentPlaintext)
        }
    }
}

pub fn seal_aes_256_gcm_siv(
    key_id: impl Into<String>,
    key: &ContentKey,
    plaintext: &[u8],
    aad: &[u8],
) -> Result<SealedPayload, CryptoError> {
    let cipher =
        Aes256GcmSiv::new_from_slice(key.as_bytes()).map_err(|_| CryptoError::SealFailed)?;
    let mut nonce = [0; AES_256_GCM_SIV_NONCE_LEN];
    UnwrapErr(SysRng).fill_bytes(&mut nonce);
    let bytes = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::SealFailed)?;

    Ok(SealedPayload {
        mode: PayloadEncryption::Aes256GcmSiv,
        key_id: key_id.into(),
        nonce: nonce.to_vec(),
        aad: aad.to_vec(),
        bytes,
    })
}

pub fn open_aes_256_gcm_siv(
    key: &ContentKey,
    payload: &SealedPayload,
) -> Result<Vec<u8>, CryptoError> {
    open_aes_256_gcm_siv_with_aad(key, payload, &payload.aad)
}

pub fn open_aes_256_gcm_siv_with_aad(
    key: &ContentKey,
    payload: &SealedPayload,
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if payload.mode != PayloadEncryption::Aes256GcmSiv {
        return Err(CryptoError::NotAes256GcmSiv);
    }
    if payload.nonce.len() != AES_256_GCM_SIV_NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength);
    }
    if payload.aad != aad {
        return Err(CryptoError::AssociatedDataMismatch);
    }

    let cipher =
        Aes256GcmSiv::new_from_slice(key.as_bytes()).map_err(|_| CryptoError::OpenFailed)?;
    cipher
        .decrypt(
            Nonce::from_slice(&payload.nonce),
            Payload {
                msg: payload.bytes.as_slice(),
                aad,
            },
        )
        .map_err(|_| CryptoError::OpenFailed)
}

pub fn message_markdown_aad(
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
) -> Vec<u8> {
    format!(
        "chaft:v1:message_markdown:{}:{}:{}",
        workspace_id.0, channel_id.0, message_id.0
    )
    .into_bytes()
}

pub fn seal_message_markdown(
    key_id: impl Into<String>,
    key: &ContentKey,
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
    markdown: &str,
) -> Result<SealedPayload, CryptoError> {
    seal_aes_256_gcm_siv(
        key_id,
        key,
        markdown.as_bytes(),
        &message_markdown_aad(workspace_id, channel_id, message_id),
    )
}

pub fn open_message_markdown(
    key: &ContentKey,
    payload: &SealedPayload,
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
) -> Result<String, CryptoError> {
    let bytes = open_aes_256_gcm_siv_with_aad(
        key,
        payload,
        &message_markdown_aad(workspace_id, channel_id, message_id),
    )?;
    Ok(String::from_utf8(bytes)?)
}

pub fn attachment_blob_aad(
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
    attachment_index: u32,
) -> Vec<u8> {
    format!(
        "chaft:v1:attachment_blob:{}:{}:{}:{}",
        workspace_id.0, channel_id.0, message_id.0, attachment_index
    )
    .into_bytes()
}

pub fn seal_attachment_blob(
    key_id: impl Into<String>,
    key: &ContentKey,
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
    attachment_index: u32,
    bytes: &[u8],
) -> Result<SealedPayload, CryptoError> {
    seal_aes_256_gcm_siv(
        key_id,
        key,
        bytes,
        &attachment_blob_aad(workspace_id, channel_id, message_id, attachment_index),
    )
}

pub fn open_attachment_blob(
    key: &ContentKey,
    payload: &SealedPayload,
    workspace_id: &WorkspaceId,
    channel_id: &ChannelId,
    message_id: &MessageId,
    attachment_index: u32,
) -> Result<Vec<u8>, CryptoError> {
    open_aes_256_gcm_siv_with_aad(
        key,
        payload,
        &attachment_blob_aad(workspace_id, channel_id, message_id, attachment_index),
    )
}

pub fn encrypted_blob_ref_from_payload(
    payload: &SealedPayload,
    plaintext_byte_len: u64,
) -> Result<EncryptedBlobRef, CryptoError> {
    if payload.mode != PayloadEncryption::Aes256GcmSiv {
        return Err(CryptoError::NotAes256GcmSiv);
    }
    if payload.nonce.len() != AES_256_GCM_SIV_NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength);
    }
    Ok(EncryptedBlobRef {
        mode: payload.mode.clone(),
        key_id: payload.key_id.clone(),
        nonce: payload.nonce.clone(),
        aad: payload.aad.clone(),
        plaintext_byte_len,
    })
}

pub fn sealed_payload_from_encrypted_blob_ref(
    encrypted: &EncryptedBlobRef,
    ciphertext: Vec<u8>,
) -> SealedPayload {
    SealedPayload {
        mode: encrypted.mode.clone(),
        key_id: encrypted.key_id.clone(),
        nonce: encrypted.nonce.clone(),
        aad: encrypted.aad.clone(),
        bytes: ciphertext,
    }
}

#[cfg(test)]
mod tests {
    use chaft_types::{EventBody, PayloadEncryption};

    use super::*;

    #[test]
    fn development_plaintext_round_trips_for_bootstrap_only() {
        let sealed = seal_development_plaintext("hello");

        assert_eq!(open_development_plaintext(&sealed).unwrap(), b"hello");
    }

    #[test]
    fn aes_256_gcm_siv_round_trips_and_hides_plaintext() {
        let key = ContentKey::from_bytes([9; AES_256_GCM_SIV_KEY_LEN]);
        let plaintext = b"replicas must not see this markdown";
        let sealed = seal_aes_256_gcm_siv("workspace-key-1", &key, plaintext, b"msg:123").unwrap();

        assert_eq!(sealed.mode, PayloadEncryption::Aes256GcmSiv);
        assert_eq!(sealed.key_id, "workspace-key-1");
        assert_ne!(sealed.bytes, plaintext);
        assert!(!String::from_utf8_lossy(&sealed.bytes).contains("markdown"));

        let opened = open_aes_256_gcm_siv(&key, &sealed).unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn aes_256_gcm_siv_rejects_wrong_associated_data() {
        let key = ContentKey::from_bytes([7; AES_256_GCM_SIV_KEY_LEN]);
        let sealed = seal_aes_256_gcm_siv("workspace-key-1", &key, b"hello", b"msg:123").unwrap();

        assert!(matches!(
            open_aes_256_gcm_siv_with_aad(&key, &sealed, b"msg:124"),
            Err(CryptoError::AssociatedDataMismatch)
        ));
    }

    #[test]
    fn aes_256_gcm_siv_rejects_wrong_key_and_tamper() {
        let key = ContentKey::from_bytes([1; AES_256_GCM_SIV_KEY_LEN]);
        let wrong_key = ContentKey::from_bytes([2; AES_256_GCM_SIV_KEY_LEN]);
        let sealed = seal_aes_256_gcm_siv("workspace-key-1", &key, b"hello", b"msg:123").unwrap();

        assert!(matches!(
            open_aes_256_gcm_siv(&wrong_key, &sealed),
            Err(CryptoError::OpenFailed)
        ));

        let mut tampered = sealed.clone();
        tampered.bytes[0] ^= 0x80;

        assert!(matches!(
            open_aes_256_gcm_siv(&key, &tampered),
            Err(CryptoError::OpenFailed)
        ));
    }

    #[test]
    fn encrypted_message_event_body_does_not_expose_plaintext() {
        let key = ContentKey::from_bytes([3; AES_256_GCM_SIV_KEY_LEN]);
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let sealed = seal_message_markdown(
            "workspace-key-1",
            &key,
            &workspace_id,
            &channel_id,
            &message_id,
            "private launch plan",
        )
        .unwrap();
        let body = EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: sealed.clone(),
            attachments: Vec::new(),
        };

        let visible_json = serde_json::to_string(&body).unwrap();

        assert!(!visible_json.contains("private launch plan"));
        assert!(visible_json.contains("aes256_gcm_siv"));
        assert_eq!(
            open_message_markdown(&key, &sealed, &workspace_id, &channel_id, &message_id).unwrap(),
            "private launch plan"
        );
    }

    #[test]
    fn encrypted_message_payload_is_bound_to_message_context() {
        let key = ContentKey::from_bytes([4; AES_256_GCM_SIV_KEY_LEN]);
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let sealed = seal_message_markdown(
            "workspace-key-1",
            &key,
            &workspace_id,
            &channel_id,
            &message_id,
            "hello",
        )
        .unwrap();
        let wrong_message_id = MessageId::new();

        assert!(matches!(
            open_message_markdown(&key, &sealed, &workspace_id, &channel_id, &wrong_message_id),
            Err(CryptoError::AssociatedDataMismatch)
        ));
    }

    #[test]
    fn encrypted_attachment_blob_round_trips_from_blob_ref_metadata() {
        let key = ContentKey::from_bytes([5; AES_256_GCM_SIV_KEY_LEN]);
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let plaintext = b"private attachment bytes";
        let sealed = seal_attachment_blob(
            "workspace-key-1",
            &key,
            &workspace_id,
            &channel_id,
            &message_id,
            0,
            plaintext,
        )
        .unwrap();
        let encrypted = encrypted_blob_ref_from_payload(&sealed, plaintext.len() as u64).unwrap();

        assert_ne!(sealed.bytes, plaintext);
        assert!(!String::from_utf8_lossy(&sealed.bytes).contains("attachment"));
        assert_eq!(encrypted.plaintext_byte_len, plaintext.len() as u64);

        let reconstructed = sealed_payload_from_encrypted_blob_ref(&encrypted, sealed.bytes);
        let opened = open_attachment_blob(
            &key,
            &reconstructed,
            &workspace_id,
            &channel_id,
            &message_id,
            0,
        )
        .unwrap();

        assert_eq!(opened, plaintext);
    }

    #[test]
    fn encrypted_attachment_blob_is_bound_to_attachment_slot() {
        let key = ContentKey::from_bytes([6; AES_256_GCM_SIV_KEY_LEN]);
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let sealed = seal_attachment_blob(
            "workspace-key-1",
            &key,
            &workspace_id,
            &channel_id,
            &message_id,
            0,
            b"hello",
        )
        .unwrap();

        assert!(matches!(
            open_attachment_blob(&key, &sealed, &workspace_id, &channel_id, &message_id, 1),
            Err(CryptoError::AssociatedDataMismatch)
        ));
    }
}
