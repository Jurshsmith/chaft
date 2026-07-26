use std::{collections::HashMap, sync::RwLock};

use openmls::prelude::{
    BasicCredential, Ciphersuite, CredentialWithKey, GroupId, KeyPackage, KeyPackageBundle,
    KeyPackageIn, LeafNodeParameters, MlsGroup, MlsGroupCreateConfig, MlsGroupJoinConfig,
    MlsMessageBodyIn, MlsMessageIn, OpenMlsProvider, ProcessedMessageContent, ProtocolVersion,
    RatchetTreeIn, StagedWelcome,
    tls_codec::{Deserialize, Serialize},
};
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::{MemoryStorage, OpenMlsRustCrypto};
use openmls_traits::storage::StorageProvider;
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use thiserror::Error;

pub use chaft_types::{
    OPENMLS_COMMIT_MAX_BYTES, OPENMLS_KEY_PACKAGE_MAX_BYTES, OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES, OPENMLS_RATCHET_TREE_MAX_BYTES,
    OPENMLS_WELCOME_MAX_BYTES,
};

pub const OPENMLS_KEY_PACKAGE_PROTOCOL: &str = "openmls/key-package/rfc9420";
pub const OPENMLS_WORKSPACE_GROUP_PROTOCOL: &str = "openmls/workspace-group/rfc9420";
pub const OPENMLS_CHANNEL_GROUP_PROTOCOL: &str = "openmls/channel-group/rfc9420";
pub const DEFAULT_OPENMLS_CIPHERSUITE: &str = "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519";
pub const OPENMLS_WORKSPACE_CONTENT_KEY_PROTOCOL: &str = "openmls/workspace-content-key/rfc9420";
pub const OPENMLS_CHANNEL_CONTENT_KEY_PROTOCOL: &str = "openmls/channel-content-key/rfc9420";

const PERSISTED_KEY_PACKAGE_SCHEMA_VERSION: u32 = 1;
const PERSISTED_WORKSPACE_GROUP_SCHEMA_VERSION: u32 = 1;
const OPENMLS_WORKSPACE_CONTENT_KEY_LABEL: &str = "chaft workspace content key v1";
const OPENMLS_WORKSPACE_CONTENT_KEY_CONTEXT: &[u8] =
    b"chaft:v1:workspace-message-and-attachment-content-key";
const OPENMLS_WORKSPACE_CONTENT_KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum MlsError {
    #[error("OpenMLS key package error: {0}")]
    KeyPackage(String),
    #[error("OpenMLS credential error: {0}")]
    Credential(String),
    #[error("OpenMLS group error: {0}")]
    Group(String),
    #[error("OpenMLS storage error: {0}")]
    Storage(String),
    #[error("OpenMLS TLS codec error: {0}")]
    TlsCodec(String),
    #[error("OpenMLS private bundle serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("OpenMLS persisted hex data is invalid")]
    InvalidHex,
    #[error("OpenMLS key package identity is not valid UTF-8")]
    InvalidIdentityUtf8(#[from] std::string::FromUtf8Error),
    #[error("OpenMLS private bundle schema version is unsupported")]
    UnsupportedPersistedSchema,
    #[error("OpenMLS private workspace group schema version is unsupported")]
    UnsupportedPersistedWorkspaceGroupSchema,
    #[error("OpenMLS private bundle does not match its public key package")]
    PersistedBundleMismatch,
    #[error("OpenMLS private workspace group does not contain a loadable group")]
    PersistedWorkspaceGroupMissing,
    #[error("OpenMLS private workspace group does not match its metadata")]
    PersistedWorkspaceGroupMismatch,
    #[error("OpenMLS message is not a welcome message")]
    ExpectedWelcomeMessage,
    #[error("OpenMLS message is not a commit message")]
    ExpectedCommitMessage,
    #[error("OpenMLS group member {identity} was not found")]
    GroupMemberNotFound { identity: String },
    #[error("OpenMLS {label} is too large ({actual_bytes} bytes, max {max_bytes})")]
    PayloadTooLarge {
        label: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedMlsKeyPackage {
    pub protocol: String,
    pub ciphersuite: String,
    pub key_package_ref: String,
    pub identity: String,
    pub key_package: Vec<u8>,
    pub private_bundle: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedMlsKeyPackage {
    pub protocol: String,
    pub ciphersuite: String,
    pub key_package_ref: String,
    pub identity: String,
    pub signature_public_key: Vec<u8>,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedMlsWorkspaceGroup {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub identity: String,
    pub member_count: usize,
    pub private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatedMlsWorkspaceGroup {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub identity: String,
    pub member_count: usize,
    pub storage_entry_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedMlsWorkspaceGroupMember {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub invitee_identity: String,
    pub invitee_key_package_ref: String,
    pub commit: Vec<u8>,
    pub welcome: Vec<u8>,
    pub ratchet_tree: Vec<u8>,
    pub updated_private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMlsWorkspaceGroupMember {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub removed_identity: String,
    pub commit: Vec<u8>,
    pub ratchet_tree: Vec<u8>,
    pub updated_private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedMlsWorkspaceGroup {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub identity: String,
    pub member_count: usize,
    pub private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedMlsWorkspaceGroup {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit: Vec<u8>,
    pub ratchet_tree: Vec<u8>,
    pub updated_private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedMlsWorkspaceGroupCommit {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub identity: String,
    pub member_count: usize,
    pub self_removed: bool,
    pub updated_private_group_state: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedMlsWorkspaceContentKey {
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub key_id: String,
    pub content_key: Vec<u8>,
}

#[derive(Debug, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMlsKeyPackageBundle {
    schema_version: u32,
    protocol: String,
    ciphersuite: String,
    key_package_ref: String,
    identity: String,
    key_package: Vec<u8>,
    signature_key_pair: SignatureKeyPair,
    key_package_bundle: KeyPackageBundle,
}

#[derive(Debug, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMlsWorkspaceGroupState {
    schema_version: u32,
    protocol: String,
    ciphersuite: String,
    group_id: String,
    epoch: u64,
    identity: String,
    signature_key_pair: SignatureKeyPair,
    #[serde(default)]
    previous_content_keys: Vec<PersistedMlsContentKey>,
    storage_entries: Vec<PersistedOpenMlsStorageEntry>,
}

#[derive(Debug, Clone, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedMlsContentKey {
    protocol: String,
    ciphersuite: String,
    group_id: String,
    epoch: u64,
    key_id: String,
    content_key: Vec<u8>,
}

#[derive(Debug, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedOpenMlsStorageEntry {
    key: String,
    value: String,
}

pub fn generate_device_key_package(
    identity: impl AsRef<str>,
) -> Result<GeneratedMlsKeyPackage, MlsError> {
    let identity = identity.as_ref().to_owned();
    let ciphersuite = default_ciphersuite();
    let provider = OpenMlsRustCrypto::default();
    let (credential_with_key, signature_key_pair) =
        credential_with_signer(&identity, &provider, ciphersuite)?;
    let key_package_bundle = KeyPackage::builder()
        .build(
            ciphersuite,
            &provider,
            &signature_key_pair,
            credential_with_key,
        )
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    let key_package = key_package_bundle
        .key_package()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size("key package", &key_package, OPENMLS_KEY_PACKAGE_MAX_BYTES)?;
    let key_package_ref = key_package_ref_hex(key_package_bundle.key_package(), &provider)?;
    let persisted = PersistedMlsKeyPackageBundle {
        schema_version: PERSISTED_KEY_PACKAGE_SCHEMA_VERSION,
        protocol: OPENMLS_KEY_PACKAGE_PROTOCOL.to_owned(),
        ciphersuite: DEFAULT_OPENMLS_CIPHERSUITE.to_owned(),
        key_package_ref: key_package_ref.clone(),
        identity: identity.clone(),
        key_package: key_package.clone(),
        signature_key_pair,
        key_package_bundle,
    };
    let private_bundle = serde_json::to_vec_pretty(&persisted)?;
    validate_payload_size(
        "private key package bundle",
        &private_bundle,
        OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES,
    )?;

    Ok(GeneratedMlsKeyPackage {
        protocol: OPENMLS_KEY_PACKAGE_PROTOCOL.to_owned(),
        ciphersuite: DEFAULT_OPENMLS_CIPHERSUITE.to_owned(),
        key_package_ref,
        identity,
        key_package,
        private_bundle,
    })
}

pub fn create_workspace_group(
    identity: impl AsRef<str>,
    workspace_id: impl AsRef<str>,
) -> Result<CreatedMlsWorkspaceGroup, MlsError> {
    create_group(
        identity,
        OPENMLS_WORKSPACE_GROUP_PROTOCOL,
        format!("chaft/workspace/{}", workspace_id.as_ref()),
    )
}

pub fn create_channel_group(
    identity: impl AsRef<str>,
    workspace_id: impl AsRef<str>,
    channel_id: impl AsRef<str>,
) -> Result<CreatedMlsWorkspaceGroup, MlsError> {
    create_group(
        identity,
        OPENMLS_CHANNEL_GROUP_PROTOCOL,
        format!(
            "chaft/workspace/{}/channel/{}",
            workspace_id.as_ref(),
            channel_id.as_ref()
        ),
    )
}

fn create_group(
    identity: impl AsRef<str>,
    protocol: &str,
    group_id: String,
) -> Result<CreatedMlsWorkspaceGroup, MlsError> {
    let identity = identity.as_ref().to_owned();
    let ciphersuite = default_ciphersuite();
    let provider = OpenMlsRustCrypto::default();
    let (credential_with_key, signature_key_pair) =
        credential_with_signer(&identity, &provider, ciphersuite)?;
    let group_id = GroupId::from_slice(group_id.as_bytes());
    let group_config = MlsGroupCreateConfig::builder()
        .ciphersuite(ciphersuite)
        .build();
    let group = MlsGroup::new_with_group_id(
        &provider,
        &signature_key_pair,
        &group_config,
        group_id,
        credential_with_key,
    )
    .map_err(|error| MlsError::Group(error.to_string()))?;
    let group_id = hex_lower(group.group_id().as_slice());
    let epoch = group.epoch().as_u64();
    let member_count = group.members().count();
    let storage_entries = storage_entries(provider.storage())?;
    let persisted = PersistedMlsWorkspaceGroupState {
        schema_version: PERSISTED_WORKSPACE_GROUP_SCHEMA_VERSION,
        protocol: protocol.to_owned(),
        ciphersuite: DEFAULT_OPENMLS_CIPHERSUITE.to_owned(),
        group_id: group_id.clone(),
        epoch,
        identity: identity.clone(),
        signature_key_pair,
        previous_content_keys: Vec::new(),
        storage_entries,
    };
    let private_group_state = serde_json::to_vec_pretty(&persisted)?;
    validate_payload_size(
        "private group state",
        &private_group_state,
        OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    )?;

    Ok(CreatedMlsWorkspaceGroup {
        protocol: protocol.to_owned(),
        ciphersuite: DEFAULT_OPENMLS_CIPHERSUITE.to_owned(),
        group_id,
        epoch,
        identity,
        member_count,
        private_group_state,
    })
}

pub fn validate_key_package(mut bytes: &[u8]) -> Result<ValidatedMlsKeyPackage, MlsError> {
    validate_payload_size("key package", bytes, OPENMLS_KEY_PACKAGE_MAX_BYTES)?;
    let byte_len = bytes.len();
    let provider = OpenMlsRustCrypto::default();
    let key_package_in = KeyPackageIn::tls_deserialize(&mut bytes)
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    let key_package = key_package_in
        .validate(provider.crypto(), ProtocolVersion::Mls10)
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    let ciphersuite = key_package.ciphersuite();
    let credential = BasicCredential::try_from(key_package.leaf_node().credential().clone())
        .map_err(|error| MlsError::Credential(error.to_string()))?;
    let identity = String::from_utf8(credential.identity().to_vec())?;
    let key_package_ref = key_package_ref_hex(&key_package, &provider)?;

    Ok(ValidatedMlsKeyPackage {
        protocol: OPENMLS_KEY_PACKAGE_PROTOCOL.to_owned(),
        ciphersuite: ciphersuite_label(ciphersuite).to_owned(),
        key_package_ref,
        identity,
        signature_public_key: key_package.leaf_node().signature_key().as_slice().to_vec(),
        byte_len,
    })
}

pub fn validate_private_key_package_bundle(
    bytes: &[u8],
) -> Result<ValidatedMlsKeyPackage, MlsError> {
    validate_payload_size(
        "private key package bundle",
        bytes,
        OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES,
    )?;
    let persisted: PersistedMlsKeyPackageBundle = serde_json::from_slice(bytes)?;
    if persisted.schema_version != PERSISTED_KEY_PACKAGE_SCHEMA_VERSION {
        return Err(MlsError::UnsupportedPersistedSchema);
    }
    let public = validate_key_package(&persisted.key_package)?;
    let bundled_key_package = persisted
        .key_package_bundle
        .key_package()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;

    if persisted.protocol != public.protocol
        || persisted.ciphersuite != public.ciphersuite
        || persisted.key_package_ref != public.key_package_ref
        || persisted.identity != public.identity
        || persisted.key_package != bundled_key_package
    {
        return Err(MlsError::PersistedBundleMismatch);
    }

    Ok(public)
}

pub fn validate_private_workspace_group_state(
    bytes: &[u8],
) -> Result<ValidatedMlsWorkspaceGroup, MlsError> {
    validate_payload_size(
        "private group state",
        bytes,
        OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    )?;
    let persisted: PersistedMlsWorkspaceGroupState = serde_json::from_slice(bytes)?;
    if persisted.schema_version != PERSISTED_WORKSPACE_GROUP_SCHEMA_VERSION {
        return Err(MlsError::UnsupportedPersistedWorkspaceGroupSchema);
    }
    if !group_protocol_supported(&persisted.protocol) {
        return Err(MlsError::PersistedWorkspaceGroupMismatch);
    }
    let group_id_bytes = hex_decode(&persisted.group_id)?;
    let group_id = GroupId::from_slice(&group_id_bytes);
    let storage = storage_from_entries(&persisted.storage_entries)?;
    let group = MlsGroup::load(&storage, &group_id)
        .map_err(|error| MlsError::Storage(error.to_string()))?
        .ok_or(MlsError::PersistedWorkspaceGroupMissing)?;
    let credential = group
        .credential()
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let credential = BasicCredential::try_from(credential.clone())
        .map_err(|error| MlsError::Credential(error.to_string()))?;
    let identity = String::from_utf8(credential.identity().to_vec())?;
    let signature_public_key = group
        .own_leaf_node()
        .ok_or(MlsError::PersistedWorkspaceGroupMismatch)?
        .signature_key()
        .as_slice()
        .to_vec();

    if !group_protocol_supported(&persisted.protocol)
        || persisted.ciphersuite != ciphersuite_label(group.ciphersuite())
        || persisted.group_id != hex_lower(group.group_id().as_slice())
        || persisted.epoch != group.epoch().as_u64()
        || persisted.identity != identity
        || persisted.signature_key_pair.to_public_vec() != signature_public_key
    {
        return Err(MlsError::PersistedWorkspaceGroupMismatch);
    }

    Ok(ValidatedMlsWorkspaceGroup {
        protocol: persisted.protocol,
        ciphersuite: persisted.ciphersuite,
        group_id: persisted.group_id,
        epoch: persisted.epoch,
        identity,
        member_count: group.members().count(),
        storage_entry_count: persisted.storage_entries.len(),
    })
}

pub fn add_member_to_workspace_group(
    private_group_state: &[u8],
    invitee_key_package: &[u8],
) -> Result<AddedMlsWorkspaceGroupMember, MlsError> {
    let (mut persisted, provider, mut group) = load_persisted_workspace_group(private_group_state)?;
    let previous_content_key = export_loaded_group_content_key(&persisted, &provider, &group)?;
    validate_payload_size(
        "key package",
        invitee_key_package,
        OPENMLS_KEY_PACKAGE_MAX_BYTES,
    )?;
    let mut invitee_key_package = invitee_key_package;
    let invitee_key_package = KeyPackageIn::tls_deserialize(&mut invitee_key_package)
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?
        .validate(provider.crypto(), ProtocolVersion::Mls10)
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    let invitee_key_package_ref = key_package_ref_hex(&invitee_key_package, &provider)?;
    let invitee_credential =
        BasicCredential::try_from(invitee_key_package.leaf_node().credential().clone())
            .map_err(|error| MlsError::Credential(error.to_string()))?;
    let invitee_identity = String::from_utf8(invitee_credential.identity().to_vec())?;
    let (commit, welcome, _) = group
        .add_members(
            &provider,
            &persisted.signature_key_pair,
            std::slice::from_ref(&invitee_key_package),
        )
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let commit = commit
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    let welcome = welcome
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size("commit", &commit, OPENMLS_COMMIT_MAX_BYTES)?;
    validate_payload_size("welcome", &welcome, OPENMLS_WELCOME_MAX_BYTES)?;

    group
        .merge_pending_commit(&provider)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let ratchet_tree = group
        .export_ratchet_tree()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size(
        "ratchet tree",
        &ratchet_tree,
        OPENMLS_RATCHET_TREE_MAX_BYTES,
    )?;
    append_previous_content_key(&mut persisted, previous_content_key);
    let updated_private_group_state =
        persist_workspace_group_state(&mut persisted, &provider, &group)?;

    Ok(AddedMlsWorkspaceGroupMember {
        protocol: persisted.protocol,
        ciphersuite: persisted.ciphersuite,
        group_id: persisted.group_id,
        epoch: persisted.epoch,
        member_count: group.members().count(),
        invitee_identity,
        invitee_key_package_ref,
        commit,
        welcome,
        ratchet_tree,
        updated_private_group_state,
    })
}

pub fn remove_member_from_group(
    private_group_state: &[u8],
    removed_identity: impl AsRef<str>,
) -> Result<RemovedMlsWorkspaceGroupMember, MlsError> {
    let removed_identity = removed_identity.as_ref().to_owned();
    let (mut persisted, provider, mut group) = load_persisted_workspace_group(private_group_state)?;
    let previous_content_key = export_loaded_group_content_key(&persisted, &provider, &group)?;
    let removed_index =
        group_member_index_by_identity(&group, &removed_identity)?.ok_or_else(|| {
            MlsError::GroupMemberNotFound {
                identity: removed_identity.clone(),
            }
        })?;
    let (commit, _, _) = group
        .remove_members(&provider, &persisted.signature_key_pair, &[removed_index])
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let commit = commit
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size("commit", &commit, OPENMLS_COMMIT_MAX_BYTES)?;

    group
        .merge_pending_commit(&provider)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let ratchet_tree = group
        .export_ratchet_tree()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size(
        "ratchet tree",
        &ratchet_tree,
        OPENMLS_RATCHET_TREE_MAX_BYTES,
    )?;
    append_previous_content_key(&mut persisted, previous_content_key);
    let updated_private_group_state =
        persist_workspace_group_state(&mut persisted, &provider, &group)?;

    Ok(RemovedMlsWorkspaceGroupMember {
        protocol: persisted.protocol,
        ciphersuite: persisted.ciphersuite,
        group_id: persisted.group_id,
        epoch: persisted.epoch,
        member_count: group.members().count(),
        removed_identity,
        commit,
        ratchet_tree,
        updated_private_group_state,
    })
}

pub fn join_workspace_group_from_welcome(
    private_key_package_bundle: &[u8],
    welcome: &[u8],
    ratchet_tree: &[u8],
) -> Result<JoinedMlsWorkspaceGroup, MlsError> {
    join_group_from_welcome(
        private_key_package_bundle,
        welcome,
        ratchet_tree,
        OPENMLS_WORKSPACE_GROUP_PROTOCOL,
    )
}

pub fn join_channel_group_from_welcome(
    private_key_package_bundle: &[u8],
    welcome: &[u8],
    ratchet_tree: &[u8],
) -> Result<JoinedMlsWorkspaceGroup, MlsError> {
    join_group_from_welcome(
        private_key_package_bundle,
        welcome,
        ratchet_tree,
        OPENMLS_CHANNEL_GROUP_PROTOCOL,
    )
}

fn join_group_from_welcome(
    private_key_package_bundle: &[u8],
    mut welcome: &[u8],
    mut ratchet_tree: &[u8],
    group_protocol: &str,
) -> Result<JoinedMlsWorkspaceGroup, MlsError> {
    validate_payload_size(
        "private key package bundle",
        private_key_package_bundle,
        OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES,
    )?;
    validate_payload_size("welcome", welcome, OPENMLS_WELCOME_MAX_BYTES)?;
    validate_payload_size("ratchet tree", ratchet_tree, OPENMLS_RATCHET_TREE_MAX_BYTES)?;
    let persisted_bundle: PersistedMlsKeyPackageBundle =
        serde_json::from_slice(private_key_package_bundle)?;
    if persisted_bundle.schema_version != PERSISTED_KEY_PACKAGE_SCHEMA_VERSION {
        return Err(MlsError::UnsupportedPersistedSchema);
    }

    let provider = OpenMlsRustCrypto::default();
    persisted_bundle
        .signature_key_pair
        .store(provider.storage())
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    let key_package_ref = persisted_bundle
        .key_package_bundle
        .key_package()
        .hash_ref(provider.crypto())
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    provider
        .storage()
        .write_key_package(&key_package_ref, &persisted_bundle.key_package_bundle)
        .map_err(|error| MlsError::Storage(error.to_string()))?;

    let welcome = MlsMessageIn::tls_deserialize(&mut welcome)
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    let welcome = match welcome.extract() {
        MlsMessageBodyIn::Welcome(welcome) => welcome,
        _ => return Err(MlsError::ExpectedWelcomeMessage),
    };
    let ratchet_tree = RatchetTreeIn::tls_deserialize(&mut ratchet_tree)
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    let staged_join = StagedWelcome::new_from_welcome(
        &provider,
        &MlsGroupJoinConfig::default(),
        welcome,
        Some(ratchet_tree),
    )
    .map_err(|error| MlsError::Group(error.to_string()))?;
    let group = staged_join
        .into_group(&provider)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let group_id = hex_lower(group.group_id().as_slice());
    let epoch = group.epoch().as_u64();
    let member_count = group.members().count();
    let ciphersuite = ciphersuite_label(group.ciphersuite()).to_owned();
    let persisted = PersistedMlsWorkspaceGroupState {
        schema_version: PERSISTED_WORKSPACE_GROUP_SCHEMA_VERSION,
        protocol: group_protocol.to_owned(),
        ciphersuite: ciphersuite.clone(),
        group_id: group_id.clone(),
        epoch,
        identity: persisted_bundle.identity.clone(),
        signature_key_pair: persisted_bundle.signature_key_pair,
        previous_content_keys: Vec::new(),
        storage_entries: storage_entries(provider.storage())?,
    };
    let private_group_state = serde_json::to_vec_pretty(&persisted)?;
    validate_payload_size(
        "private group state",
        &private_group_state,
        OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    )?;

    Ok(JoinedMlsWorkspaceGroup {
        protocol: group_protocol.to_owned(),
        ciphersuite,
        group_id,
        epoch,
        identity: persisted_bundle.identity,
        member_count,
        private_group_state,
    })
}

pub fn update_own_leaf_in_group(
    private_group_state: &[u8],
) -> Result<UpdatedMlsWorkspaceGroup, MlsError> {
    let (mut persisted, provider, mut group) = load_persisted_workspace_group(private_group_state)?;
    let previous_content_key = export_loaded_group_content_key(&persisted, &provider, &group)?;
    let bundle = group
        .self_update(
            &provider,
            &persisted.signature_key_pair,
            LeafNodeParameters::default(),
        )
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let commit = bundle
        .into_commit()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size("commit", &commit, OPENMLS_COMMIT_MAX_BYTES)?;

    group
        .merge_pending_commit(&provider)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let ratchet_tree = group
        .export_ratchet_tree()
        .tls_serialize_detached()
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?;
    validate_payload_size(
        "ratchet tree",
        &ratchet_tree,
        OPENMLS_RATCHET_TREE_MAX_BYTES,
    )?;
    append_previous_content_key(&mut persisted, previous_content_key);
    let updated_private_group_state =
        persist_workspace_group_state(&mut persisted, &provider, &group)?;

    Ok(UpdatedMlsWorkspaceGroup {
        protocol: persisted.protocol,
        ciphersuite: persisted.ciphersuite,
        group_id: persisted.group_id,
        epoch: persisted.epoch,
        member_count: group.members().count(),
        commit,
        ratchet_tree,
        updated_private_group_state,
    })
}

pub fn apply_group_commit(
    private_group_state: &[u8],
    mut commit: &[u8],
) -> Result<AppliedMlsWorkspaceGroupCommit, MlsError> {
    let (mut persisted, provider, mut group) = load_persisted_workspace_group(private_group_state)?;
    let previous_content_key = export_loaded_group_content_key(&persisted, &provider, &group)?;
    validate_payload_size("commit", commit, OPENMLS_COMMIT_MAX_BYTES)?;
    let commit = MlsMessageIn::tls_deserialize(&mut commit)
        .map_err(|error| MlsError::TlsCodec(error.to_string()))?
        .try_into_protocol_message()
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let processed = group
        .process_message(&provider, commit)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let ProcessedMessageContent::StagedCommitMessage(staged_commit) = processed.into_content()
    else {
        return Err(MlsError::ExpectedCommitMessage);
    };
    let self_removed = staged_commit.self_removed();

    group
        .merge_staged_commit(&provider, *staged_commit)
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let member_count = group.members().count();
    append_previous_content_key(&mut persisted, previous_content_key);
    let updated_private_group_state =
        persist_workspace_group_state(&mut persisted, &provider, &group)?;

    Ok(AppliedMlsWorkspaceGroupCommit {
        protocol: persisted.protocol,
        ciphersuite: persisted.ciphersuite,
        group_id: persisted.group_id,
        epoch: persisted.epoch,
        identity: persisted.identity,
        member_count,
        self_removed,
        updated_private_group_state,
    })
}

pub fn export_workspace_content_key(
    private_group_state: &[u8],
) -> Result<ExportedMlsWorkspaceContentKey, MlsError> {
    export_group_content_key(private_group_state)
}

pub fn export_group_content_key(
    private_group_state: &[u8],
) -> Result<ExportedMlsWorkspaceContentKey, MlsError> {
    let (persisted, provider, group) = load_persisted_workspace_group(private_group_state)?;
    export_loaded_group_content_key(&persisted, &provider, &group)
}

pub fn export_group_content_key_for_key_id(
    private_group_state: &[u8],
    key_id: &str,
) -> Result<Option<ExportedMlsWorkspaceContentKey>, MlsError> {
    let (persisted, provider, group) = load_persisted_workspace_group(private_group_state)?;
    let current = export_loaded_group_content_key(&persisted, &provider, &group)?;
    if current.key_id == key_id {
        return Ok(Some(current));
    }

    Ok(persisted
        .previous_content_keys
        .iter()
        .find(|previous| previous.key_id == key_id)
        .map(|previous| ExportedMlsWorkspaceContentKey {
            protocol: previous.protocol.clone(),
            ciphersuite: previous.ciphersuite.clone(),
            group_id: previous.group_id.clone(),
            epoch: previous.epoch,
            key_id: previous.key_id.clone(),
            content_key: previous.content_key.clone(),
        }))
}

fn export_loaded_group_content_key(
    persisted: &PersistedMlsWorkspaceGroupState,
    provider: &OpenMlsRustCrypto,
    group: &MlsGroup,
) -> Result<ExportedMlsWorkspaceContentKey, MlsError> {
    let group_id = hex_lower(group.group_id().as_slice());
    let epoch = group.epoch().as_u64();
    let content_key = group
        .export_secret(
            provider.crypto(),
            OPENMLS_WORKSPACE_CONTENT_KEY_LABEL,
            OPENMLS_WORKSPACE_CONTENT_KEY_CONTEXT,
            OPENMLS_WORKSPACE_CONTENT_KEY_LEN,
        )
        .map_err(|error| MlsError::Group(error.to_string()))?;
    let (protocol, key_id) = if persisted.protocol == OPENMLS_CHANNEL_GROUP_PROTOCOL {
        (
            OPENMLS_CHANNEL_CONTENT_KEY_PROTOCOL,
            channel_content_key_id(&group_id, epoch),
        )
    } else {
        (
            OPENMLS_WORKSPACE_CONTENT_KEY_PROTOCOL,
            workspace_content_key_id(&group_id, epoch),
        )
    };

    Ok(ExportedMlsWorkspaceContentKey {
        protocol: protocol.to_owned(),
        ciphersuite: ciphersuite_label(group.ciphersuite()).to_owned(),
        group_id,
        epoch,
        key_id,
        content_key,
    })
}

fn append_previous_content_key(
    persisted: &mut PersistedMlsWorkspaceGroupState,
    previous: ExportedMlsWorkspaceContentKey,
) {
    if persisted
        .previous_content_keys
        .iter()
        .any(|existing| existing.key_id == previous.key_id)
    {
        return;
    }

    persisted
        .previous_content_keys
        .push(PersistedMlsContentKey {
            protocol: previous.protocol,
            ciphersuite: previous.ciphersuite,
            group_id: previous.group_id,
            epoch: previous.epoch,
            key_id: previous.key_id,
            content_key: previous.content_key,
        });
    persisted
        .previous_content_keys
        .sort_by_key(|content_key| content_key.epoch);
}

fn group_member_index_by_identity(
    group: &MlsGroup,
    identity: &str,
) -> Result<Option<openmls::prelude::LeafNodeIndex>, MlsError> {
    for member in group.members() {
        let credential = BasicCredential::try_from(member.credential.clone())
            .map_err(|error| MlsError::Credential(error.to_string()))?;
        if credential.identity() == identity.as_bytes() {
            return Ok(Some(member.index));
        }
    }

    Ok(None)
}

pub fn workspace_content_key_id(group_id: &str, epoch: u64) -> String {
    format!("openmls:workspace:{group_id}:content:v{epoch}")
}

pub fn channel_content_key_id(group_id: &str, epoch: u64) -> String {
    format!("openmls:channel:{group_id}:content:v{epoch}")
}

fn key_package_ref_hex(
    key_package: &KeyPackage,
    provider: &OpenMlsRustCrypto,
) -> Result<String, MlsError> {
    key_package
        .hash_ref(provider.crypto())
        .map(|reference| hex_lower(reference.as_slice()))
        .map_err(|error| MlsError::KeyPackage(error.to_string()))
}

fn default_ciphersuite() -> Ciphersuite {
    Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
}

fn credential_with_signer(
    identity: &str,
    provider: &OpenMlsRustCrypto,
    ciphersuite: Ciphersuite,
) -> Result<(CredentialWithKey, SignatureKeyPair), MlsError> {
    let credential = BasicCredential::new(identity.as_bytes().to_vec());
    let signature_key_pair = SignatureKeyPair::new(ciphersuite.signature_algorithm())
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    signature_key_pair
        .store(provider.storage())
        .map_err(|error| MlsError::KeyPackage(error.to_string()))?;
    let credential_with_key = CredentialWithKey {
        credential: credential.into(),
        signature_key: signature_key_pair.to_public_vec().into(),
    };

    Ok((credential_with_key, signature_key_pair))
}

fn storage_entries(storage: &MemoryStorage) -> Result<Vec<PersistedOpenMlsStorageEntry>, MlsError> {
    let values = storage
        .values
        .read()
        .map_err(|error| MlsError::Storage(error.to_string()))?;
    let mut entries = values
        .iter()
        .map(|(key, value)| PersistedOpenMlsStorageEntry {
            key: hex_lower(key),
            value: hex_lower(value),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(entries)
}

fn load_persisted_workspace_group(
    private_group_state: &[u8],
) -> Result<(PersistedMlsWorkspaceGroupState, OpenMlsRustCrypto, MlsGroup), MlsError> {
    validate_payload_size(
        "private group state",
        private_group_state,
        OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    )?;
    let persisted: PersistedMlsWorkspaceGroupState = serde_json::from_slice(private_group_state)?;
    if persisted.schema_version != PERSISTED_WORKSPACE_GROUP_SCHEMA_VERSION {
        return Err(MlsError::UnsupportedPersistedWorkspaceGroupSchema);
    }
    if !group_protocol_supported(&persisted.protocol) {
        return Err(MlsError::PersistedWorkspaceGroupMismatch);
    }
    let group_id_bytes = hex_decode(&persisted.group_id)?;
    let group_id = GroupId::from_slice(&group_id_bytes);
    let provider = OpenMlsRustCrypto::default();
    let values = persisted
        .storage_entries
        .iter()
        .map(|entry| Ok((hex_decode(&entry.key)?, hex_decode(&entry.value)?)))
        .collect::<Result<HashMap<_, _>, MlsError>>()?;
    *provider
        .storage()
        .values
        .write()
        .map_err(|error| MlsError::Storage(error.to_string()))? = values;
    let group = MlsGroup::load(provider.storage(), &group_id)
        .map_err(|error| MlsError::Storage(error.to_string()))?
        .ok_or(MlsError::PersistedWorkspaceGroupMissing)?;

    Ok((persisted, provider, group))
}

fn persist_workspace_group_state(
    persisted: &mut PersistedMlsWorkspaceGroupState,
    provider: &OpenMlsRustCrypto,
    group: &MlsGroup,
) -> Result<Vec<u8>, MlsError> {
    if !group_protocol_supported(&persisted.protocol) {
        return Err(MlsError::PersistedWorkspaceGroupMismatch);
    }
    persisted.ciphersuite = ciphersuite_label(group.ciphersuite()).to_owned();
    persisted.group_id = hex_lower(group.group_id().as_slice());
    persisted.epoch = group.epoch().as_u64();
    persisted.storage_entries = storage_entries(provider.storage())?;
    let private_group_state = serde_json::to_vec_pretty(persisted)?;
    validate_payload_size(
        "private group state",
        &private_group_state,
        OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
    )?;
    Ok(private_group_state)
}

fn storage_from_entries(
    entries: &[PersistedOpenMlsStorageEntry],
) -> Result<MemoryStorage, MlsError> {
    let values = entries
        .iter()
        .map(|entry| Ok((hex_decode(&entry.key)?, hex_decode(&entry.value)?)))
        .collect::<Result<HashMap<_, _>, MlsError>>()?;

    Ok(MemoryStorage {
        values: RwLock::new(values),
    })
}

fn validate_payload_size(
    label: &'static str,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), MlsError> {
    if bytes.len() > max_bytes {
        return Err(MlsError::PayloadTooLarge {
            label,
            actual_bytes: bytes.len(),
            max_bytes,
        });
    }
    Ok(())
}

fn ciphersuite_label(ciphersuite: Ciphersuite) -> &'static str {
    match ciphersuite {
        Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519 => DEFAULT_OPENMLS_CIPHERSUITE,
        _ => "unsupported",
    }
}

fn group_protocol_supported(protocol: &str) -> bool {
    matches!(
        protocol,
        OPENMLS_WORKSPACE_GROUP_PROTOCOL | OPENMLS_CHANNEL_GROUP_PROTOCOL
    )
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(value: &str) -> Result<Vec<u8>, MlsError> {
    if !value.len().is_multiple_of(2) {
        return Err(MlsError::InvalidHex);
    }

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_value(value: u8) -> Result<u8, MlsError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(MlsError::InvalidHex),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_device_key_package_is_valid_openmls_tls() {
        let generated = generate_device_key_package("dev_alice").unwrap();
        let validated = validate_key_package(&generated.key_package).unwrap();

        assert_eq!(generated.protocol, OPENMLS_KEY_PACKAGE_PROTOCOL);
        assert_eq!(generated.ciphersuite, DEFAULT_OPENMLS_CIPHERSUITE);
        assert_eq!(generated.identity, "dev_alice");
        assert_eq!(generated.key_package_ref, validated.key_package_ref);
        assert_eq!(validated.identity, "dev_alice");
        assert_eq!(validated.byte_len, generated.key_package.len());
        assert!(!generated.private_bundle.is_empty());
        assert!(generated.key_package.len() <= OPENMLS_KEY_PACKAGE_MAX_BYTES);
        assert!(generated.private_bundle.len() <= OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES);
    }

    #[test]
    fn persisted_private_bundle_matches_public_key_package() {
        let generated = generate_device_key_package("dev_bob").unwrap();
        let validated = validate_private_key_package_bundle(&generated.private_bundle).unwrap();

        assert_eq!(validated.identity, "dev_bob");
        assert_eq!(validated.key_package_ref, generated.key_package_ref);
    }

    #[test]
    fn created_workspace_group_persists_loadable_private_state() {
        let created = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let validated =
            validate_private_workspace_group_state(&created.private_group_state).unwrap();

        assert_eq!(created.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(created.ciphersuite, DEFAULT_OPENMLS_CIPHERSUITE);
        assert_eq!(created.identity, "dev_alice");
        assert_eq!(created.epoch, 0);
        assert_eq!(created.member_count, 1);
        assert_eq!(validated.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(validated.group_id, created.group_id);
        assert_eq!(validated.identity, "dev_alice");
        assert_eq!(validated.member_count, 1);
        assert!(validated.storage_entry_count > 0);
        assert!(created.private_group_state.len() <= OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES);
    }

    #[test]
    fn oversized_openmls_payloads_are_rejected_before_parsing() {
        assert_payload_too_large(
            validate_key_package(&vec![0; OPENMLS_KEY_PACKAGE_MAX_BYTES + 1]).unwrap_err(),
            "key package",
            OPENMLS_KEY_PACKAGE_MAX_BYTES + 1,
            OPENMLS_KEY_PACKAGE_MAX_BYTES,
        );
        assert_payload_too_large(
            validate_private_key_package_bundle(&vec![
                0;
                OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES
                    + 1
            ])
            .unwrap_err(),
            "private key package bundle",
            OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES,
        );
        assert_payload_too_large(
            validate_private_workspace_group_state(&vec![
                0;
                OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1
            ])
            .unwrap_err(),
            "private group state",
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
        );

        let generated = generate_device_key_package("dev_limit").unwrap();
        let group = create_workspace_group("dev_limit", "wrk_limit").unwrap();

        assert_payload_too_large(
            add_member_to_workspace_group(
                &group.private_group_state,
                &vec![0; OPENMLS_KEY_PACKAGE_MAX_BYTES + 1],
            )
            .unwrap_err(),
            "key package",
            OPENMLS_KEY_PACKAGE_MAX_BYTES + 1,
            OPENMLS_KEY_PACKAGE_MAX_BYTES,
        );
        assert_payload_too_large(
            join_workspace_group_from_welcome(
                &generated.private_bundle,
                &vec![0; OPENMLS_WELCOME_MAX_BYTES + 1],
                &[],
            )
            .unwrap_err(),
            "welcome",
            OPENMLS_WELCOME_MAX_BYTES + 1,
            OPENMLS_WELCOME_MAX_BYTES,
        );
        assert_payload_too_large(
            join_workspace_group_from_welcome(
                &generated.private_bundle,
                &[],
                &vec![0; OPENMLS_RATCHET_TREE_MAX_BYTES + 1],
            )
            .unwrap_err(),
            "ratchet tree",
            OPENMLS_RATCHET_TREE_MAX_BYTES + 1,
            OPENMLS_RATCHET_TREE_MAX_BYTES,
        );
        assert_payload_too_large(
            apply_group_commit(
                &group.private_group_state,
                &vec![0; OPENMLS_COMMIT_MAX_BYTES + 1],
            )
            .unwrap_err(),
            "commit",
            OPENMLS_COMMIT_MAX_BYTES + 1,
            OPENMLS_COMMIT_MAX_BYTES,
        );
    }

    #[test]
    fn group_state_operations_reject_oversized_private_state_before_parsing() {
        let oversized = vec![0; OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1];

        assert_payload_too_large(
            add_member_to_workspace_group(&oversized, &[]).unwrap_err(),
            "private group state",
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
        );
        assert_payload_too_large(
            update_own_leaf_in_group(&oversized).unwrap_err(),
            "private group state",
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
        );
        assert_payload_too_large(
            apply_group_commit(&oversized, &[]).unwrap_err(),
            "private group state",
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
        );
        assert_payload_too_large(
            export_group_content_key(&oversized).unwrap_err(),
            "private group state",
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES + 1,
            OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES,
        );
    }

    #[test]
    fn workspace_group_add_member_welcome_can_be_joined() {
        let alice_group = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let alice_validated =
            validate_private_workspace_group_state(&added.updated_private_group_state).unwrap();

        assert_eq!(added.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(added.epoch, 1);
        assert_eq!(added.member_count, 2);
        assert_eq!(added.invitee_identity, "dev_bob");
        assert_eq!(added.invitee_key_package_ref, bob_package.key_package_ref);
        assert_eq!(alice_validated.group_id, alice_group.group_id);
        assert_eq!(alice_validated.epoch, 1);
        assert_eq!(alice_validated.member_count, 2);
        assert!(!added.commit.is_empty());
        assert!(!added.welcome.is_empty());
        assert!(!added.ratchet_tree.is_empty());
        assert_group_artifacts_within_limits(
            &added.commit,
            Some(&added.welcome),
            &added.ratchet_tree,
        );

        let joined = join_workspace_group_from_welcome(
            &bob_package.private_bundle,
            &added.welcome,
            &added.ratchet_tree,
        )
        .unwrap();
        let bob_validated =
            validate_private_workspace_group_state(&joined.private_group_state).unwrap();

        assert_eq!(joined.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(joined.identity, "dev_bob");
        assert_eq!(joined.group_id, added.group_id);
        assert_eq!(joined.epoch, 1);
        assert_eq!(joined.member_count, 2);
        assert_eq!(bob_validated.identity, "dev_bob");
        assert_eq!(bob_validated.member_count, 2);
    }

    #[test]
    fn workspace_content_key_export_matches_after_member_join() {
        let alice_group = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let joined = join_workspace_group_from_welcome(
            &bob_package.private_bundle,
            &added.welcome,
            &added.ratchet_tree,
        )
        .unwrap();

        let alice_key = export_workspace_content_key(&added.updated_private_group_state).unwrap();
        let bob_key = export_workspace_content_key(&joined.private_group_state).unwrap();

        assert_eq!(alice_key.protocol, OPENMLS_WORKSPACE_CONTENT_KEY_PROTOCOL);
        assert_eq!(alice_key.group_id, added.group_id);
        assert_eq!(alice_key.epoch, 1);
        assert_eq!(
            alice_key.key_id,
            workspace_content_key_id(&added.group_id, 1)
        );
        assert_eq!(
            alice_key.content_key.len(),
            OPENMLS_WORKSPACE_CONTENT_KEY_LEN
        );
        assert_eq!(alice_key, bob_key);
    }

    #[test]
    fn channel_group_content_key_export_matches_after_member_join() {
        let alice_group = create_channel_group("dev_alice", "wrk_alpha", "chn_private").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let joined = join_channel_group_from_welcome(
            &bob_package.private_bundle,
            &added.welcome,
            &added.ratchet_tree,
        )
        .unwrap();

        let alice_key = export_group_content_key(&added.updated_private_group_state).unwrap();
        let bob_key = export_group_content_key(&joined.private_group_state).unwrap();

        assert_eq!(alice_group.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);
        assert_eq!(added.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);
        assert_eq!(joined.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);
        assert_eq!(alice_key.protocol, OPENMLS_CHANNEL_CONTENT_KEY_PROTOCOL);
        assert_eq!(alice_key.group_id, added.group_id);
        assert_eq!(alice_key.epoch, 1);
        assert_eq!(alice_key.key_id, channel_content_key_id(&added.group_id, 1));
        assert_eq!(
            alice_key.content_key.len(),
            OPENMLS_WORKSPACE_CONTENT_KEY_LEN
        );
        assert_eq!(alice_key, bob_key);
    }

    #[test]
    fn group_content_key_export_matches_after_self_update_commit_is_applied() {
        let alice_group = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let bob_joined = join_workspace_group_from_welcome(
            &bob_package.private_bundle,
            &added.welcome,
            &added.ratchet_tree,
        )
        .unwrap();
        let epoch_one_key =
            export_workspace_content_key(&added.updated_private_group_state).unwrap();

        let alice_updated = update_own_leaf_in_group(&added.updated_private_group_state).unwrap();
        let bob_applied =
            apply_group_commit(&bob_joined.private_group_state, &alice_updated.commit).unwrap();
        let alice_key =
            export_workspace_content_key(&alice_updated.updated_private_group_state).unwrap();
        let bob_key =
            export_workspace_content_key(&bob_applied.updated_private_group_state).unwrap();
        let alice_epoch_one_key = export_group_content_key_for_key_id(
            &alice_updated.updated_private_group_state,
            &epoch_one_key.key_id,
        )
        .unwrap();
        let bob_epoch_one_key = export_group_content_key_for_key_id(
            &bob_applied.updated_private_group_state,
            &epoch_one_key.key_id,
        )
        .unwrap();

        assert_eq!(alice_updated.epoch, 2);
        assert_eq!(bob_applied.epoch, 2);
        assert!(!alice_updated.commit.is_empty());
        assert!(!alice_updated.ratchet_tree.is_empty());
        assert_group_artifacts_within_limits(
            &alice_updated.commit,
            None,
            &alice_updated.ratchet_tree,
        );
        assert_eq!(alice_key, bob_key);
        assert_eq!(alice_epoch_one_key, Some(epoch_one_key.clone()));
        assert_eq!(bob_epoch_one_key, Some(epoch_one_key));
    }

    #[test]
    fn existing_member_applies_add_commit_for_later_invitee() {
        let alice_group = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let bob_added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let bob_joined = join_workspace_group_from_welcome(
            &bob_package.private_bundle,
            &bob_added.welcome,
            &bob_added.ratchet_tree,
        )
        .unwrap();
        let charlie_package = generate_device_key_package("dev_charlie").unwrap();

        let charlie_added = add_member_to_workspace_group(
            &bob_added.updated_private_group_state,
            &charlie_package.key_package,
        )
        .unwrap();
        let charlie_joined = join_workspace_group_from_welcome(
            &charlie_package.private_bundle,
            &charlie_added.welcome,
            &charlie_added.ratchet_tree,
        )
        .unwrap();
        let epoch_one_key = export_workspace_content_key(&bob_joined.private_group_state).unwrap();
        let bob_applied =
            apply_group_commit(&bob_joined.private_group_state, &charlie_added.commit).unwrap();

        let alice_key =
            export_workspace_content_key(&charlie_added.updated_private_group_state).unwrap();
        let bob_key =
            export_workspace_content_key(&bob_applied.updated_private_group_state).unwrap();
        let charlie_key =
            export_workspace_content_key(&charlie_joined.private_group_state).unwrap();
        let alice_epoch_one_key = export_group_content_key_for_key_id(
            &charlie_added.updated_private_group_state,
            &epoch_one_key.key_id,
        )
        .unwrap();
        let bob_epoch_one_key = export_group_content_key_for_key_id(
            &bob_applied.updated_private_group_state,
            &epoch_one_key.key_id,
        )
        .unwrap();
        let charlie_epoch_one_key = export_group_content_key_for_key_id(
            &charlie_joined.private_group_state,
            &epoch_one_key.key_id,
        )
        .unwrap();

        assert_eq!(charlie_added.epoch, 2);
        assert_eq!(bob_applied.epoch, 2);
        assert_eq!(bob_applied.member_count, 3);
        assert_eq!(alice_key, bob_key);
        assert_eq!(alice_key, charlie_key);
        assert_eq!(alice_epoch_one_key, Some(epoch_one_key.clone()));
        assert_eq!(bob_epoch_one_key, Some(epoch_one_key));
        assert_eq!(charlie_epoch_one_key, None);
    }

    #[test]
    fn surviving_member_applies_remove_commit_and_keeps_prior_epoch_readable() {
        let alice_group = create_workspace_group("dev_alice", "wrk_alpha").unwrap();
        let bob_package = generate_device_key_package("dev_bob").unwrap();
        let bob_added = add_member_to_workspace_group(
            &alice_group.private_group_state,
            &bob_package.key_package,
        )
        .unwrap();
        let bob_joined = join_workspace_group_from_welcome(
            &bob_package.private_bundle,
            &bob_added.welcome,
            &bob_added.ratchet_tree,
        )
        .unwrap();
        let charlie_package = generate_device_key_package("dev_charlie").unwrap();
        let charlie_added = add_member_to_workspace_group(
            &bob_added.updated_private_group_state,
            &charlie_package.key_package,
        )
        .unwrap();
        let bob_at_epoch_two =
            apply_group_commit(&bob_joined.private_group_state, &charlie_added.commit).unwrap();
        let charlie_joined = join_workspace_group_from_welcome(
            &charlie_package.private_bundle,
            &charlie_added.welcome,
            &charlie_added.ratchet_tree,
        )
        .unwrap();
        let epoch_two_key =
            export_workspace_content_key(&charlie_added.updated_private_group_state).unwrap();

        let bob_removed =
            remove_member_from_group(&charlie_added.updated_private_group_state, "dev_bob")
                .unwrap();
        let charlie_applied =
            apply_group_commit(&charlie_joined.private_group_state, &bob_removed.commit).unwrap();
        let bob_applied = apply_group_commit(
            &bob_at_epoch_two.updated_private_group_state,
            &bob_removed.commit,
        )
        .unwrap();
        let alice_key =
            export_workspace_content_key(&bob_removed.updated_private_group_state).unwrap();
        let charlie_key =
            export_workspace_content_key(&charlie_applied.updated_private_group_state).unwrap();
        let alice_epoch_two_key = export_group_content_key_for_key_id(
            &bob_removed.updated_private_group_state,
            &epoch_two_key.key_id,
        )
        .unwrap();
        let charlie_epoch_two_key = export_group_content_key_for_key_id(
            &charlie_applied.updated_private_group_state,
            &epoch_two_key.key_id,
        )
        .unwrap();

        assert_eq!(bob_removed.removed_identity, "dev_bob");
        assert_eq!(bob_removed.epoch, 3);
        assert_eq!(bob_removed.member_count, 2);
        assert_eq!(charlie_applied.epoch, 3);
        assert_eq!(charlie_applied.member_count, 2);
        assert!(bob_applied.self_removed);
        assert_group_artifacts_within_limits(&bob_removed.commit, None, &bob_removed.ratchet_tree);
        assert_eq!(alice_key, charlie_key);
        assert_eq!(alice_epoch_two_key, Some(epoch_two_key.clone()));
        assert_eq!(charlie_epoch_two_key, Some(epoch_two_key));
    }

    #[test]
    fn tampered_key_package_is_rejected() {
        let mut generated = generate_device_key_package("dev_eve").unwrap();
        let last = generated.key_package.last_mut().unwrap();
        *last ^= 0x80;

        assert!(validate_key_package(&generated.key_package).is_err());
    }

    fn assert_payload_too_large(
        error: MlsError,
        expected_label: &'static str,
        expected_actual_bytes: usize,
        expected_max_bytes: usize,
    ) {
        match error {
            MlsError::PayloadTooLarge {
                label,
                actual_bytes,
                max_bytes,
            } => {
                assert_eq!(label, expected_label);
                assert_eq!(actual_bytes, expected_actual_bytes);
                assert_eq!(max_bytes, expected_max_bytes);
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    fn assert_group_artifacts_within_limits(
        commit: &[u8],
        welcome: Option<&[u8]>,
        ratchet_tree: &[u8],
    ) {
        assert!(commit.len() <= OPENMLS_COMMIT_MAX_BYTES);
        if let Some(welcome) = welcome {
            assert!(welcome.len() <= OPENMLS_WELCOME_MAX_BYTES);
        }
        assert!(ratchet_tree.len() <= OPENMLS_RATCHET_TREE_MAX_BYTES);
    }
}
