use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{Algorithm, Argon2, Params, Version};
use chaft_app::{
    WorkspaceChannelPage, WorkspaceChannelSearch, WorkspaceMemberPage, WorkspaceSnapshot,
    WorkspaceSnapshotOptions, body_override_event_ids_for_snapshot_window,
    query_has_channel_search_terms,
};
use chaft_core::{
    AuthorizationError, CoreError, MaterializationReport, MessageView, WorkspaceState,
    authorize_event_with_history, trust_snapshot_for_event_from_events,
    trust_snapshot_for_events_from_events, trust_snapshot_from_events,
};
use chaft_crypto::{
    ContentKey, CryptoError, SealedPayload, encrypted_blob_ref_from_payload, open_aes_256_gcm_siv,
    open_attachment_blob, open_message_markdown, seal_aes_256_gcm_siv, seal_attachment_blob,
    seal_message_markdown, sealed_payload_from_encrypted_blob_ref,
};
use chaft_identity::{DeviceIdentity, IdentityError, verify_self_contained_event};
use chaft_media::{
    BLOB_DESCRIPTOR_MAX_CHUNKS, BlobAvailability, BlobPruneReport, BlobStore, MediaError,
    describe_blob, validate_blob_availability,
};
use chaft_mls::{MlsError, OPENMLS_KEY_PACKAGE_PROTOCOL, OPENMLS_WORKSPACE_GROUP_PROTOCOL};
use chaft_net::{ChaftTransport, NetError, PeerAddress};
use chaft_net_direct::{
    AuthorizedPublishTransport, BlobSyncTransport, MAX_PUBLISH_EVENTS_PER_REQUEST,
};
use chaft_search::{SearchError, SearchIndex, query_has_search_terms};
use chaft_store::{
    EventStore, StoreError, WorkspaceEventStorageHealth, WorkspaceEventStorageRepair,
};
use chaft_sync::{
    PullSyncReport, SyncError, pull_workspace_from_peer, pull_workspace_from_peer_with_inventory,
    validate_remote_inventory_event_ids,
};
use chaft_types::{
    ATTACHMENT_BLOB_HASH_MAX_BYTES, ATTACHMENT_PLAINTEXT_MAX_BYTES, AttachmentRef,
    CHANNEL_NAME_MAX_BYTES, ChannelId, ContentKeyScope, DEVICE_DISPLAY_NAME_MAX_BYTES,
    DEVICE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, DeviceId, DeviceKeyPackageId,
    EventBody, EventId, IdValidationError, MESSAGE_MARKDOWN_MAX_BYTES, MessageId,
    PEER_ENDPOINT_TRANSPORT_MAX_BYTES, REACTION_TEXT_MAX_BYTES, SignableEvent, SignedEvent,
    SignedTrustSnapshot, WORKSPACE_ID_MAX_BYTES, WORKSPACE_NAME_MAX_BYTES, WorkspaceId,
    WorkspaceRole, peer_endpoint_hint_is_supported, peer_endpoint_hint_transport_is_consistent,
    validate_channel_id as validate_type_channel_id,
    validate_device_key_package_id as validate_type_device_key_package_id,
    validate_event_id as validate_type_event_id, validate_message_id as validate_type_message_id,
    validate_workspace_id as validate_type_workspace_id,
};
pub use chaft_types::{
    PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const WORKSPACE_KEY_LEN: usize = 32;
const DIRECT_WHOLE_BLOB_SYNC_LIMIT: usize = 4 * 1024 * 1024;
const DIRECT_BLOB_CHUNK_SIZE: usize = 4 * 1024 * 1024;
const LOCAL_SEARCH_RAW_HIT_LIMIT: usize = 500;
const LOCAL_SEARCH_VISIBLE_HIT_LIMIT: usize = 50;
const SEARCH_QUERY_MAX_BYTES: usize = 512;
const ATTACHMENT_FILE_MAX_BYTES: u64 = ATTACHMENT_PLAINTEXT_MAX_BYTES;
const DEVICE_KEY_PACKAGE_MAX_LEN: usize = 64 * 1024;
const DEVICE_ID_REFERENCE_MAX_BYTES: usize = DEVICE_ID_MAX_BYTES;
const MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS: usize = 128;
const MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS: usize = 64;
const MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS: usize = 64;
const MAX_WORKSPACE_SUMMARY_PAGE_ROWS: usize = 128;
const MAX_WORKSPACE_MEMBER_PAGE_ROWS: usize = 128;
const MAX_WORKSPACE_CHANNEL_PAGE_ROWS: usize = 128;
const MAX_WORKSPACE_CHANNEL_SEARCH_ROWS: usize = 128;
const CONTENT_KEY_EXPORT_SCHEMA_VERSION: u32 = 2;
const CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV: &str = "aes-256-gcm-siv";
const RECOVERY_BUNDLE_SCHEMA_VERSION: u32 = 1;
const RECOVERY_BUNDLE_SALT_LEN: usize = 16;
const RECOVERY_BUNDLE_KDF_ARGON2ID: &str = "argon2id";
const RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY: &str = "blake3-derive-key";
const RECOVERY_BUNDLE_KDF_CONTEXT: &str = "Chaft workspace recovery bundle v1";
const RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB: u32 = 64 * 1024;
const RECOVERY_BUNDLE_ARGON2_TIME_COST: u32 = 3;
const RECOVERY_BUNDLE_ARGON2_PARALLELISM: u32 = 1;
const RECOVERY_BUNDLE_KDF_OUTPUT_LEN: u32 = WORKSPACE_KEY_LEN as u32;
const LOCAL_SECRET_SCHEMA_VERSION: u32 = 1;
const LOCAL_SECRET_STORAGE: &str = "argon2id-aes-256-gcm-siv";
const LOCAL_SECRET_KDF_ARGON2ID: &str = "argon2id";
const LOCAL_SECRET_KDF_CONTEXT: &str = "Chaft local secret file v1";
const LOCAL_SECRET_SALT_LEN: usize = 16;
const LOCAL_SECRET_KEY_LEN: usize = 32;
const LOCAL_SECRET_ARGON2_MEMORY_COST_KIB: u32 = 64 * 1024;
const LOCAL_SECRET_ARGON2_TIME_COST: u32 = 3;
const LOCAL_SECRET_ARGON2_PARALLELISM: u32 = 1;
const LOCAL_SECRET_KDF_OUTPUT_LEN: u32 = LOCAL_SECRET_KEY_LEN as u32;
const LOCAL_SECRET_FILE_MAX_BYTES: usize = 16 * 1024 * 1024;
const LOCAL_SECRET_KIND_WORKSPACE_KEY: &str = "workspace-key";
const LOCAL_SECRET_KIND_CHANNEL_KEY: &str = "channel-key";
const LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE: &str = "openmls-key-package";
const LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP: &str = "openmls-workspace-group";
const LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP: &str = "openmls-channel-group";
const BLOB_TRANSFER_LEDGER_SCHEMA_VERSION: u32 = 1;
const BLOB_TRANSFER_LEDGER_MAX_ENTRIES: usize = 512;
const BLOB_TRANSFER_LEDGER_MAX_BYTES: usize = 16 * 1024 * 1024;
const BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES: usize =
    20 + 1 + 10 + 1 + PEER_ENDPOINT_ID_MAX_BYTES + 1 + ATTACHMENT_BLOB_HASH_MAX_BYTES;
const BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES: usize = 2 * 1024;
const COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION: u32 = 1;
const COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES: usize = 512;
const COMPROMISE_RESPONSE_LEDGER_MAX_BYTES: usize = 4 * 1024 * 1024;
static SECRET_FILE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static ATTACHMENT_EXPORT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE: &str = "invalid_self_contained_signature";
const COMPROMISE_SIGNAL_SEVERITY_SUSPECTED: &str = "suspected";
const COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE: &str =
    "rotate_workspace_for_suspected_compromise";
const COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES: &str = "review_invalid_signatures";
const COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS: &str = "no_signals";
const COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW: &str =
    "remote_signals_require_review";
const COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED: &str =
    "local_signals_already_handled";
const COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING: &str = "local_secret_state_missing";
const RUNTIME_PATH_MAX_BYTES: usize = 64 * 1024;
const RUNTIME_PASSPHRASE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("identity error: {0}")]
    Identity(#[from] IdentityError),
    #[error("event store error")]
    Store(#[from] StoreError),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("runtime metadata serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("workspace materialization error")]
    Core(#[from] CoreError),
    #[error("workspace authorization error: {0}")]
    Authorization(#[from] AuthorizationError),
    #[error("network error")]
    Net(#[from] NetError),
    #[error("sync error")]
    Sync(#[from] SyncError),
    #[error("search index error")]
    Search(#[from] SearchError),
    #[error("media storage error")]
    Media(#[from] MediaError),
    #[error("MLS error")]
    Mls(#[from] MlsError),
    #[error("recovery bundle KDF failed: {0}")]
    RecoveryBundleKdf(String),
    #[error("workspace {workspace_id:?} has no local events")]
    WorkspaceHasNoEvents { workspace_id: WorkspaceId },
    #[error("message {message_id:?} was not found in workspace {workspace_id:?}")]
    MessageNotFound {
        workspace_id: WorkspaceId,
        message_id: MessageId,
    },
    #[error("event {event_id:?} was not found in workspace {workspace_id:?}")]
    EventNotFound {
        workspace_id: WorkspaceId,
        event_id: EventId,
    },
    #[error(
        "attachment {blob_hash} was not found on message {message_id:?} in workspace {workspace_id:?}"
    )]
    AttachmentNotFound {
        workspace_id: WorkspaceId,
        message_id: MessageId,
        blob_hash: String,
    },
    #[error("attachment blob {blob_hash} is not available in the local blob store")]
    AttachmentBlobMissing { blob_hash: String },
    #[error("attachment {blob_hash} does not include encryption metadata")]
    AttachmentNotEncrypted { blob_hash: String },
    #[error("channel {channel_id:?} was not found in workspace {workspace_id:?}")]
    ChannelNotFound {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    },
    #[error(
        "device {device_id:?} cannot access channel {channel_id:?} in workspace {workspace_id:?}"
    )]
    ChannelAccessDenied {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        device_id: DeviceId,
    },
    #[error("reaction is required")]
    ReactionRequired,
    #[error("message markdown is too large ({actual_bytes} bytes, max {max_bytes})")]
    MessageMarkdownTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("{field} is too large ({actual_bytes} bytes, max {max_bytes})")]
    MetadataFieldTooLarge {
        field: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("{field} is required")]
    MetadataFieldRequired { field: &'static str },
    #[error("search query is too large ({actual_bytes} bytes, max {max_bytes})")]
    SearchQueryTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("attachment file is too large ({actual_bytes} bytes, max {max_bytes})")]
    AttachmentFileTooLarge { actual_bytes: u64, max_bytes: u64 },
    #[error("display name is required")]
    DisplayNameRequired,
    #[error("device key package protocol is required")]
    DeviceKeyPackageProtocolRequired,
    #[error("device key package bytes are required")]
    DeviceKeyPackageRequired,
    #[error("device key package is too large")]
    DeviceKeyPackageTooLarge,
    #[error("peer endpoint id is required")]
    PeerEndpointIdRequired,
    #[error("peer endpoint is required")]
    PeerEndpointRequired,
    #[error("peer endpoint uses an unsupported P2P route")]
    UnsupportedPeerEndpoint,
    #[error("peer endpoint transport is required")]
    PeerEndpointTransportRequired,
    #[error("peer endpoint transport does not match its route")]
    PeerEndpointTransportMismatch,
    #[error("peer endpoint list is too large ({actual_count} endpoints, max {max_count})")]
    PeerEndpointListTooLarge {
        actual_count: usize,
        max_count: usize,
    },
    #[error("OpenMLS workspace group already exists for {workspace_id:?}")]
    OpenMlsWorkspaceGroupAlreadyExists { workspace_id: WorkspaceId },
    #[error("OpenMLS workspace group is missing for {workspace_id:?}")]
    OpenMlsWorkspaceGroupMissing { workspace_id: WorkspaceId },
    #[error("no local OpenMLS group state exists for {workspace_id:?}")]
    OpenMlsLocalGroupMissing { workspace_id: WorkspaceId },
    #[error("OpenMLS channel group already exists for {channel_id:?} in {workspace_id:?}")]
    OpenMlsChannelGroupAlreadyExists {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    },
    #[error("OpenMLS channel group is missing for {channel_id:?} in {workspace_id:?}")]
    OpenMlsChannelGroupMissing {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    },
    #[error("device key package {key_package_id:?} was not found in workspace {workspace_id:?}")]
    DeviceKeyPackageNotFound {
        workspace_id: WorkspaceId,
        key_package_id: DeviceKeyPackageId,
    },
    #[error(
        "OpenMLS private key package {key_package_ref} is missing in workspace {workspace_id:?}"
    )]
    OpenMlsPrivateKeyPackageMissing {
        workspace_id: WorkspaceId,
        key_package_ref: String,
    },
    #[error("OpenMLS workspace group invite was not found for {device_id:?} in {workspace_id:?}")]
    OpenMlsWorkspaceGroupInviteNotFound {
        workspace_id: WorkspaceId,
        device_id: DeviceId,
    },
    #[error("OpenMLS channel group invite was not found for {device_id:?} in {channel_id:?}")]
    OpenMlsChannelGroupInviteNotFound {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        device_id: DeviceId,
    },
    #[error("workspace content key is invalid")]
    InvalidWorkspaceKey,
    #[error("workspace key export schema version is unsupported")]
    UnsupportedWorkspaceKeyExport,
    #[error("channel content key is missing for {channel_id:?} in workspace {workspace_id:?}")]
    ChannelKeyMissing {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    },
    #[error("channel content key is invalid")]
    InvalidChannelKey,
    #[error("channel key export schema version is unsupported")]
    UnsupportedChannelKeyExport,
    #[error("recovery bundle passphrase is required")]
    RecoveryBundlePassphraseRequired,
    #[error("workspace recovery bundle schema or KDF is unsupported")]
    UnsupportedWorkspaceRecoveryBundle,
    #[error("workspace recovery bundle contents do not match bundle metadata")]
    InvalidWorkspaceRecoveryBundle,
    #[error("local secret file passphrase is required")]
    LocalSecretPassphraseRequired,
    #[error("local secret file schema or KDF is unsupported")]
    UnsupportedLocalSecretFile,
    #[error("local secret file contents do not match metadata")]
    InvalidLocalSecretFile,
    #[error("local secret file KDF failed: {0}")]
    LocalSecretKdf(String),
    #[error(
        "content key {key_id} is missing for channel {channel_id:?} in workspace {workspace_id:?}"
    )]
    ContentKeyMissing {
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        key_id: String,
    },
}

impl RuntimeError {
    pub fn is_peer_protocol_error(&self) -> bool {
        matches!(
            self,
            RuntimeError::Net(NetError::Protocol(_))
                | RuntimeError::Sync(SyncError::Net(NetError::Protocol(_)))
        )
    }

    pub fn peer_protocol_error_message(&self) -> Option<String> {
        match self {
            RuntimeError::Net(NetError::Protocol(message))
            | RuntimeError::Sync(SyncError::Net(NetError::Protocol(message))) => {
                Some(format!("protocol error: {message}"))
            }
            _ => None,
        }
    }
}

pub struct LocalRuntime {
    paths: RuntimePaths,
    identity: DeviceIdentity,
    identity_passphrase: Option<String>,
    store: EventStore,
}

struct WorkspaceWriteContext {
    events: Vec<SignedEvent>,
    state: WorkspaceState,
    report: MaterializationReport,
    head_event_ids: Vec<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePaths {
    pub data_dir: PathBuf,
    pub identity_file: PathBuf,
    pub event_store: PathBuf,
    pub search_index: PathBuf,
    pub blob_store: PathBuf,
    pub workspace_keys_dir: PathBuf,
    pub blob_transfer_ledger: PathBuf,
    pub compromise_response_ledger: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWorkspace {
    pub workspace_id: String,
    pub channel_id: String,
    pub owner_device_id: String,
    pub workspace_event_id: String,
    pub channel_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorkspaceSummary {
    pub workspace_id: String,
    pub name: String,
    pub channel_count: usize,
    pub member_count: usize,
    pub event_count: usize,
    pub has_workspace_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorkspaceSummaryPage {
    pub start_index: usize,
    pub item_count: usize,
    pub total_count: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
    pub workspaces: Vec<LocalWorkspaceSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageHealth {
    pub workspace_id: String,
    pub total_event_count: usize,
    pub parseable_event_count: usize,
    pub corrupt_event_count: usize,
    pub signature_valid_metadata_count: usize,
    pub servable_event_count: usize,
    pub poisoned_servable_metadata_count: usize,
    pub promotable_servable_metadata_count: usize,
    pub non_servable_parseable_event_count: usize,
}

impl From<WorkspaceEventStorageHealth> for WorkspaceStorageHealth {
    fn from(health: WorkspaceEventStorageHealth) -> Self {
        Self {
            workspace_id: health.workspace_id,
            total_event_count: health.total_event_count,
            parseable_event_count: health.parseable_event_count,
            corrupt_event_count: health.corrupt_event_count,
            signature_valid_metadata_count: health.signature_valid_metadata_count,
            servable_event_count: health.servable_event_count,
            poisoned_servable_metadata_count: health.poisoned_servable_metadata_count,
            promotable_servable_metadata_count: health.promotable_servable_metadata_count,
            non_servable_parseable_event_count: health.non_servable_parseable_event_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageRepair {
    pub workspace_id: String,
    pub total_event_count: usize,
    pub parseable_event_count: usize,
    pub corrupt_event_count: usize,
    pub signature_valid_metadata_before_count: usize,
    pub signature_valid_metadata_after_count: usize,
    pub repaired_metadata_count: usize,
    pub promoted_servable_metadata_count: usize,
    pub cleared_unservable_metadata_count: usize,
}

impl From<WorkspaceEventStorageRepair> for WorkspaceStorageRepair {
    fn from(repair: WorkspaceEventStorageRepair) -> Self {
        Self {
            workspace_id: repair.workspace_id,
            total_event_count: repair.total_event_count,
            parseable_event_count: repair.parseable_event_count,
            corrupt_event_count: repair.corrupt_event_count,
            signature_valid_metadata_before_count: repair.signature_valid_metadata_before_count,
            signature_valid_metadata_after_count: repair.signature_valid_metadata_after_count,
            repaired_metadata_count: repair.repaired_metadata_count,
            promoted_servable_metadata_count: repair.promoted_servable_metadata_count,
            cleared_unservable_metadata_count: repair.cleared_unservable_metadata_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedChannel {
    pub workspace_id: String,
    pub channel_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedDeviceProfile {
    pub workspace_id: String,
    pub device_id: String,
    pub display_name: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDeviceKeyPackage {
    pub workspace_id: String,
    pub device_id: String,
    pub key_package_id: String,
    pub protocol: String,
    pub byte_len: usize,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPeerEndpoint {
    pub workspace_id: String,
    pub device_id: String,
    pub endpoint_id: String,
    pub endpoint: String,
    pub transport: String,
    pub is_backup_peer: bool,
    pub expires_at_ms: Option<i64>,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedOpenMlsKeyPackage {
    pub workspace_id: String,
    pub device_id: String,
    pub key_package_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub key_package_ref: String,
    pub byte_len: usize,
    pub private_bundle_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedOpenMlsWorkspaceGroupMember {
    pub workspace_id: String,
    pub device_id: String,
    pub invitee_device_id: String,
    pub invitee_key_package_id: String,
    pub invitee_key_package_ref: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub welcome_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedOpenMlsWorkspaceGroupMember {
    pub workspace_id: String,
    pub device_id: String,
    pub removed_device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
    pub source_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOpenMlsWorkspaceGroupCommits {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub self_removed: bool,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedOpenMlsChannelGroupMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub invitee_device_id: String,
    pub invitee_key_package_id: String,
    pub invitee_key_package_ref: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub welcome_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedOpenMlsChannelGroupMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub removed_device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
    pub source_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedWorkspaceOpenMlsGroups {
    pub workspace_id: String,
    pub workspace_update: Option<UpdatedOpenMlsWorkspaceGroup>,
    #[serde(default)]
    pub channel_update_count: usize,
    pub channel_updates: Vec<UpdatedOpenMlsChannelGroup>,
    #[serde(default)]
    pub updated_event_count: usize,
    pub updated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceForSuspectedCompromise {
    pub workspace_id: String,
    pub openmls_updates: Option<UpdatedWorkspaceOpenMlsGroups>,
    pub manual_key_rotation: Option<RotatedWorkspaceManualKeys>,
    #[serde(default)]
    pub rotated_event_count: usize,
    pub rotated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseReport {
    pub workspace_id: String,
    pub has_signals: bool,
    pub signal_count: usize,
    pub invalid_signature_count: usize,
    pub local_device_signal_count: usize,
    pub should_rotate_local_secret_state: bool,
    pub recommended_action: Option<String>,
    pub signals: Vec<WorkspaceCompromiseSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseSignal {
    pub kind: String,
    pub severity: String,
    pub event_id: String,
    pub channel_id: Option<String>,
    pub author_device_id: String,
    pub local_device: bool,
    pub physical_ms: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseResponse {
    pub workspace_id: String,
    pub report: WorkspaceCompromiseReport,
    pub action_taken: Option<String>,
    pub rotated_local_secret_state: bool,
    pub skipped_reason: Option<String>,
    #[serde(default)]
    pub responded_signal_count: usize,
    pub responded_signal_event_ids: Vec<String>,
    #[serde(default)]
    pub already_handled_signal_count: usize,
    pub already_handled_signal_event_ids: Vec<String>,
    pub rotation: Option<RotatedWorkspaceForSuspectedCompromise>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOpenMlsChannelGroupCommits {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub self_removed: bool,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<String>,
    pub event_id: String,
    pub encrypted: bool,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAttachment {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub blob_hash: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub attachment_id: String,
    pub display_name: String,
    pub media_type: String,
    pub byte_len: u64,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrunedBlobCache {
    #[serde(default)]
    pub workspace_count: usize,
    pub workspace_ids: Vec<String>,
    #[serde(default)]
    pub referenced_blob_count: usize,
    pub referenced_blob_hashes: Vec<String>,
    #[serde(default)]
    pub removed_blob_count: usize,
    pub removed_blob_hashes: Vec<String>,
    #[serde(default)]
    pub removed_manifest_count: usize,
    pub removed_manifest_hashes: Vec<String>,
    #[serde(default)]
    pub removed_chunk_count: usize,
    pub removed_chunk_hashes: Vec<String>,
    #[serde(default)]
    pub removed_temp_file_count: usize,
    #[serde(default)]
    pub removed_temp_file_paths: Vec<String>,
}

impl PrunedBlobCache {
    fn from_parts(
        workspace_ids: Vec<String>,
        referenced_blob_hashes: Vec<String>,
        removed_blob_hashes: Vec<String>,
        removed_manifest_hashes: Vec<String>,
        removed_chunk_hashes: Vec<String>,
        removed_temp_file_paths: Vec<String>,
    ) -> Self {
        Self {
            workspace_count: workspace_ids.len(),
            workspace_ids,
            referenced_blob_count: referenced_blob_hashes.len(),
            referenced_blob_hashes,
            removed_blob_count: removed_blob_hashes.len(),
            removed_blob_hashes,
            removed_manifest_count: removed_manifest_hashes.len(),
            removed_manifest_hashes,
            removed_chunk_count: removed_chunk_hashes.len(),
            removed_chunk_hashes,
            removed_temp_file_count: removed_temp_file_paths.len(),
            removed_temp_file_paths,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferLedger {
    pub schema_version: u32,
    pub entries: Vec<BlobTransferAttempt>,
}

impl Default for BlobTransferLedger {
    fn default() -> Self {
        Self {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferAttempt {
    pub attempt_id: String,
    pub workspace_id: String,
    pub peer_id: String,
    pub peer_endpoint: String,
    pub blob_hash: String,
    pub mode: BlobTransferMode,
    pub status: BlobTransferStatus,
    pub attempt_count: u32,
    pub total_byte_len: u64,
    pub chunk_size: Option<u64>,
    #[serde(default)]
    pub chunk_count: usize,
    pub chunk_hashes: Vec<String>,
    #[serde(default)]
    pub planned_chunk_count: usize,
    pub planned_chunk_hashes: Vec<String>,
    #[serde(default)]
    pub remote_available_chunk_count: usize,
    pub remote_available_chunk_hashes: Vec<String>,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: Option<u64>,
    pub error: Option<String>,
}

impl BlobTransferAttempt {
    fn refresh_counts(&mut self) {
        self.chunk_count = self.chunk_hashes.len();
        self.planned_chunk_count = self.planned_chunk_hashes.len();
        self.remote_available_chunk_count = self.remote_available_chunk_hashes.len();
    }

    fn normalize_after_read(&mut self) {
        truncate_string_bytes(&mut self.attempt_id, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.workspace_id, WORKSPACE_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.peer_id, PEER_ENDPOINT_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.peer_endpoint, PEER_ENDPOINT_MAX_BYTES);
        truncate_string_bytes(&mut self.blob_hash, ATTACHMENT_BLOB_HASH_MAX_BYTES);
        self.chunk_hashes.truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        self.planned_chunk_hashes
            .truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        self.remote_available_chunk_hashes
            .truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        truncate_string_list_bytes(&mut self.chunk_hashes, ATTACHMENT_BLOB_HASH_MAX_BYTES);
        truncate_string_list_bytes(
            &mut self.planned_chunk_hashes,
            ATTACHMENT_BLOB_HASH_MAX_BYTES,
        );
        truncate_string_list_bytes(
            &mut self.remote_available_chunk_hashes,
            ATTACHMENT_BLOB_HASH_MAX_BYTES,
        );
        if self.mode == BlobTransferMode::WholeBlob {
            self.chunk_size = None;
            self.chunk_hashes.clear();
            self.planned_chunk_hashes.clear();
            self.remote_available_chunk_hashes.clear();
        }
        truncate_string_option_bytes(&mut self.error, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
        self.refresh_counts();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompromiseResponseLedger {
    schema_version: u32,
    entries: Vec<CompromiseResponseLedgerEntry>,
}

impl Default for CompromiseResponseLedger {
    fn default() -> Self {
        Self {
            schema_version: COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompromiseResponseLedgerEntry {
    workspace_id: String,
    signal_event_ids: Vec<String>,
    rotated_event_ids: Vec<String>,
    responded_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferRetryReport {
    pub workspace_id: String,
    #[serde(default)]
    pub pending_attempt_count: usize,
    pub pending_attempt_ids: Vec<String>,
    #[serde(default)]
    pub retried_blob_count: usize,
    pub retried_blob_hashes: Vec<String>,
    #[serde(default)]
    pub reconciled_blob_count: usize,
    pub reconciled_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub skipped_blob_count: usize,
    pub skipped_blob_hashes: Vec<String>,
    #[serde(default)]
    pub peer_error_count: usize,
    pub peer_errors: Vec<BlobTransferPeerError>,
    #[serde(default)]
    pub blob_transfer_attempt_count: usize,
    pub blob_transfer_attempts: Vec<BlobTransferAttempt>,
}

impl BlobTransferRetryReport {
    fn refresh_counts(&mut self) {
        self.pending_attempt_count = self.pending_attempt_ids.len();
        self.retried_blob_count = self.retried_blob_hashes.len();
        self.reconciled_blob_count = self.reconciled_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.skipped_blob_count = self.skipped_blob_hashes.len();
        self.peer_error_count = self.peer_errors.len();
        self.blob_transfer_attempt_count = self.blob_transfer_attempts.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferPeerError {
    pub peer_id: String,
    pub peer_endpoint: String,
    pub blob_hash: String,
    pub message: String,
    pub suspect_protocol_error: bool,
}

fn blob_transfer_peer_error(
    peer_id: &str,
    peer_endpoint: &str,
    blob_hash: &str,
    mut message: String,
    suspect_protocol_error: bool,
) -> BlobTransferPeerError {
    let mut peer_id = peer_id.to_owned();
    let mut peer_endpoint = peer_endpoint.to_owned();
    let mut blob_hash = blob_hash.to_owned();
    truncate_string_bytes(&mut peer_id, PEER_ENDPOINT_ID_MAX_BYTES);
    truncate_string_bytes(&mut peer_endpoint, PEER_ENDPOINT_MAX_BYTES);
    truncate_string_bytes(&mut blob_hash, ATTACHMENT_BLOB_HASH_MAX_BYTES);
    truncate_string_bytes(&mut message, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
    BlobTransferPeerError {
        peer_id,
        peer_endpoint,
        blob_hash,
        message,
        suspect_protocol_error,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobTransferMode {
    WholeBlob,
    ChunkedBlob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobTransferStatus {
    InProgress,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub event_id: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedReaction {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub reaction: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedReaction {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub reaction: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkedChannelRead {
    pub workspace_id: String,
    pub channel_id: String,
    pub read_through_event_id: String,
    pub marker_event_id: Option<String>,
    pub already_read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitedMember {
    pub workspace_id: String,
    pub invitee_device_id: String,
    pub role: WorkspaceRole,
    pub event_id: String,
    pub openmls_member_add_event_id: Option<String>,
    pub openmls_epoch: Option<u64>,
    pub openmls_member_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMember {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMemberWithOpenMls {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub openmls_event_id: String,
    pub removal_event_id: String,
    pub openmls_epoch: u64,
    pub openmls_member_count: usize,
    pub openmls_private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMemberWithKeyRotation {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub removal_event_id: String,
    pub workspace_key_rotation: RotatedWorkspaceKey,
    #[serde(default)]
    pub channel_key_rotation_count: usize,
    pub channel_key_rotations: Vec<RotatedChannelKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedChannelMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub event_id: String,
    pub openmls_member_add_event_id: Option<String>,
    pub openmls_epoch: Option<u64>,
    pub openmls_member_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMemberWithOpenMls {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub openmls_event_id: String,
    pub removal_event_id: String,
    pub openmls_epoch: u64,
    pub openmls_member_count: usize,
    pub openmls_private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMemberWithKeyRotation {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub removal_event_id: String,
    pub channel_key_rotation: RotatedChannelKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledWorkspace {
    pub workspace_id: String,
    #[serde(default)]
    pub requested_event_count: usize,
    pub requested_event_ids: Vec<String>,
    #[serde(default)]
    pub fetched_event_count: usize,
    pub fetched_event_ids: Vec<String>,
    #[serde(default)]
    pub fetched_blob_count: usize,
    pub fetched_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub ignored_event_count: usize,
    pub ignored_event_ids: Vec<String>,
    #[serde(default)]
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub openmls_catchup: PulledOpenMlsCatchup,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compromise_response: Option<WorkspaceCompromiseResponse>,
    #[serde(default)]
    pub gap_count: usize,
    pub gaps: Vec<PulledWorkspaceGap>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledOpenMlsCatchup {
    #[serde(default)]
    pub event_count: usize,
    pub workspace_joined_event_id: Option<String>,
    pub workspace_applied_event_ids: Vec<String>,
    pub workspace_provisioned_event_ids: Vec<String>,
    pub workspace_self_removed: bool,
    pub channel_groups: Vec<PulledOpenMlsChannelCatchup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledOpenMlsChannelCatchup {
    pub channel_id: String,
    #[serde(default)]
    pub event_count: usize,
    pub joined_event_id: Option<String>,
    pub applied_event_ids: Vec<String>,
    pub provisioned_event_ids: Vec<String>,
    pub self_removed: bool,
}

impl PulledOpenMlsCatchup {
    fn has_provisioned_events(&self) -> bool {
        !self.workspace_provisioned_event_ids.is_empty()
            || self
                .channel_groups
                .iter()
                .any(|group| !group.provisioned_event_ids.is_empty())
    }

    fn refresh_counts(&mut self) {
        for group in &mut self.channel_groups {
            group.refresh_counts();
        }
        self.event_count = usize::from(self.workspace_joined_event_id.is_some())
            + self.workspace_applied_event_ids.len()
            + self.workspace_provisioned_event_ids.len()
            + self
                .channel_groups
                .iter()
                .map(|group| group.event_count)
                .sum::<usize>();
    }
}

impl PulledOpenMlsChannelCatchup {
    fn refresh_counts(&mut self) {
        self.event_count = usize::from(self.joined_event_id.is_some())
            + self.applied_event_ids.len()
            + self.provisioned_event_ids.len();
    }
}

impl PulledWorkspace {
    fn has_local_generated_events(&self) -> bool {
        self.openmls_catchup.has_provisioned_events()
            || self
                .compromise_response
                .as_ref()
                .is_some_and(|response| response.rotated_local_secret_state)
    }

    fn refresh_counts(&mut self) {
        self.requested_event_count = self.requested_event_ids.len();
        self.fetched_event_count = self.fetched_event_ids.len();
        self.fetched_blob_count = self.fetched_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.ignored_event_count = self.ignored_event_ids.len();
        self.applied_event_count = self.applied_event_ids.len();
        self.gap_count = self.gaps.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedWorkspace {
    pub workspace_id: String,
    #[serde(default)]
    pub published_event_count: usize,
    pub published_event_ids: Vec<String>,
    #[serde(default)]
    pub published_blob_count: usize,
    pub published_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub skipped_gap_count: usize,
    pub skipped_gaps: Vec<PulledWorkspaceGap>,
    #[serde(default)]
    pub blob_transfer_attempt_count: usize,
    pub blob_transfer_attempts: Vec<BlobTransferAttempt>,
}

impl PublishedWorkspace {
    fn from_parts(
        workspace_id: String,
        published_event_ids: Vec<String>,
        published_blob_hashes: Vec<String>,
        missing_blob_hashes: Vec<String>,
        skipped_gaps: Vec<PulledWorkspaceGap>,
        blob_transfer_attempts: Vec<BlobTransferAttempt>,
    ) -> Self {
        let mut published = Self {
            workspace_id,
            published_event_count: 0,
            published_event_ids,
            published_blob_count: 0,
            published_blob_hashes,
            missing_blob_count: 0,
            missing_blob_hashes,
            skipped_gap_count: 0,
            skipped_gaps,
            blob_transfer_attempt_count: 0,
            blob_transfer_attempts,
        };
        published.refresh_counts();
        published
    }

    fn refresh_counts(&mut self) {
        self.published_event_count = self.published_event_ids.len();
        self.published_blob_count = self.published_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.skipped_gap_count = self.skipped_gaps.len();
        self.blob_transfer_attempt_count = self.blob_transfer_attempts.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueue {
    pub workspace_id: String,
    pub summary: WorkspacePublishQueueSummary,
    pub publishable_event_ids: Vec<String>,
    pub backup_event_ids: Vec<String>,
    pub available_blob_hashes: Vec<String>,
    pub missing_blob_hashes: Vec<String>,
    pub skipped_gaps: Vec<PulledWorkspaceGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueueSummary {
    pub publishable_event_count: usize,
    pub backup_event_count: usize,
    pub available_blob_count: usize,
    pub missing_blob_count: usize,
    pub skipped_gap_count: usize,
    pub queued_message_event_count: usize,
    pub queued_attachment_blob_count: usize,
    pub oldest_event_physical_ms: Option<i64>,
    pub newest_event_physical_ms: Option<i64>,
    pub has_missing_local_blobs: bool,
    pub has_skipped_gaps: bool,
    pub is_complete: bool,
    pub channels: Vec<WorkspacePublishQueueChannelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueueChannelSummary {
    pub channel_id: Option<String>,
    pub publishable_event_count: usize,
    pub backup_event_count: usize,
    pub queued_message_event_count: usize,
    pub queued_attachment_blob_count: usize,
    pub missing_blob_count: usize,
}

#[derive(Default)]
struct WorkspacePublishQueueChannelSummaryBuilder {
    publishable_event_count: usize,
    backup_event_count: usize,
    queued_message_event_count: usize,
    attachment_blob_hashes: BTreeSet<String>,
    missing_blob_hashes: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedWorkspace {
    pub workspace_id: String,
    pub published: PublishedWorkspace,
    pub pulled: PulledWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledWorkspaceGap {
    pub event_id: String,
    pub missing_parent_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceKeyExport {
    pub schema_version: u32,
    pub workspace_id: String,
    #[serde(default = "default_content_key_epoch")]
    pub epoch: u64,
    pub key_id: String,
    pub exporter_device_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    pub previous_keys: Vec<ExportedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspaceKey {
    pub workspace_id: String,
    pub key_id: String,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceKey {
    pub workspace_id: String,
    pub previous_key_id: String,
    pub key_id: String,
    pub epoch: u64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelKeyExport {
    pub schema_version: u32,
    pub workspace_id: String,
    pub channel_id: String,
    #[serde(default = "default_content_key_epoch")]
    pub epoch: u64,
    pub key_id: String,
    pub exporter_device_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    pub previous_keys: Vec<ExportedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedChannelKey {
    pub workspace_id: String,
    pub channel_id: String,
    pub key_id: String,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryBundleKdf {
    pub name: String,
    pub context: String,
    pub salt: Vec<u8>,
    #[serde(default)]
    pub memory_cost_kib: u32,
    #[serde(default)]
    pub time_cost: u32,
    #[serde(default)]
    pub parallelism: u32,
    #[serde(default)]
    pub output_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryBundle {
    pub schema_version: u32,
    pub workspace_id: String,
    pub exporter_device_id: String,
    pub kdf: WorkspaceRecoveryBundleKdf,
    pub sealed_payload: SealedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspaceRecoveryBundle {
    pub workspace_id: String,
    pub workspace_key_id: String,
    #[serde(default)]
    pub imported_channel_count: usize,
    pub imported_channel_ids: Vec<String>,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRecoveryBundlePlaintext {
    schema_version: u32,
    workspace_key: WorkspaceKeyExport,
    #[serde(default)]
    channel_keys: Vec<ChannelKeyExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedChannelKey {
    pub workspace_id: String,
    pub channel_id: String,
    pub previous_key_id: String,
    pub key_id: String,
    pub epoch: u64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceManualKeys {
    pub workspace_id: String,
    pub workspace_key_rotation: RotatedWorkspaceKey,
    #[serde(default)]
    pub channel_key_rotation_count: usize,
    pub channel_key_rotations: Vec<RotatedChannelKey>,
    #[serde(default)]
    pub rotated_event_count: usize,
    pub rotated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedContentKeyMaterial {
    pub key_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedWorkspaceSearch {
    pub workspace_id: String,
    pub indexed_message_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchedWorkspace {
    pub workspace_id: String,
    pub query: String,
    #[serde(default)]
    pub item_count: usize,
    #[serde(default)]
    pub hit_count: usize,
    #[serde(default)]
    pub raw_candidate_count: usize,
    #[serde(default = "default_local_search_raw_hit_limit")]
    pub raw_candidate_limit: usize,
    #[serde(default = "default_local_search_visible_hit_limit")]
    pub visible_hit_limit: usize,
    #[serde(default)]
    pub has_more_hits: bool,
    pub hits: Vec<WorkspaceSearchHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchHit {
    pub workspace_id: String,
    pub event_id: String,
    pub message_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub channel_is_private: bool,
    pub author_device_id: String,
    pub author_display_name: Option<String>,
    pub physical_ms: i64,
    pub body: String,
    #[serde(default)]
    pub body_char_count: usize,
    #[serde(default)]
    pub body_truncated: bool,
}

const fn default_local_search_raw_hit_limit() -> usize {
    LOCAL_SEARCH_RAW_HIT_LIMIT
}

const fn default_local_search_visible_hit_limit() -> usize {
    LOCAL_SEARCH_VISIBLE_HIT_LIMIT
}

impl SearchedWorkspace {
    fn empty(workspace_id: WorkspaceId, query: String) -> Self {
        Self {
            workspace_id: workspace_id.0,
            query,
            item_count: 0,
            hit_count: 0,
            raw_candidate_count: 0,
            raw_candidate_limit: LOCAL_SEARCH_RAW_HIT_LIMIT,
            visible_hit_limit: LOCAL_SEARCH_VISIBLE_HIT_LIMIT,
            has_more_hits: false,
            hits: Vec::new(),
        }
    }

    fn bounded(
        workspace_id: WorkspaceId,
        query: String,
        mut hits: Vec<WorkspaceSearchHit>,
        raw_candidate_count: usize,
        has_more_raw_candidates: bool,
    ) -> Self {
        let hit_count = hits.len();
        hits.truncate(LOCAL_SEARCH_VISIBLE_HIT_LIMIT);
        Self {
            workspace_id: workspace_id.0,
            query,
            item_count: hits.len(),
            hit_count,
            raw_candidate_count,
            raw_candidate_limit: LOCAL_SEARCH_RAW_HIT_LIMIT,
            visible_hit_limit: LOCAL_SEARCH_VISIBLE_HIT_LIMIT,
            has_more_hits: has_more_raw_candidates,
            hits,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedWorkspaceKey {
    schema_version: u32,
    workspace_id: WorkspaceId,
    #[serde(default = "default_content_key_epoch")]
    epoch: u64,
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    previous_keys: Vec<PersistedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedChannelKey {
    schema_version: u32,
    workspace_id: WorkspaceId,
    channel_id: ChannelId,
    #[serde(default = "default_content_key_epoch")]
    epoch: u64,
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    previous_keys: Vec<PersistedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedContentKeyMaterial {
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
}

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

struct PendingAttachment {
    display_name: String,
    media_type: String,
    plaintext: Vec<u8>,
}

struct ProvisionedOpenMlsChannelMembers {
    channel_id: String,
    event_ids: Vec<String>,
}

struct ResolvedContentKey {
    key_id: String,
    content_key: ContentKey,
}

impl ResolvedContentKey {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn content_key(&self) -> &ContentKey {
        &self.content_key
    }
}

fn default_content_key_epoch() -> u64 {
    1
}

fn event_attachment_blob_hashes(event: &SignedEvent) -> Vec<String> {
    match &event.event.body {
        EventBody::MessageCreated { attachments, .. }
        | EventBody::MessageCreatedEncrypted { attachments, .. }
        | EventBody::MessageReplyCreated { attachments, .. }
        | EventBody::MessageReplyCreatedEncrypted { attachments, .. } => attachments
            .iter()
            .map(|attachment| attachment.blob_hash.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn attachment_blob_hashes(events: &[SignedEvent]) -> Vec<String> {
    let mut hashes = BTreeSet::new();
    for event in events {
        hashes.extend(event_attachment_blob_hashes(event));
    }
    hashes.into_iter().collect()
}

fn is_queued_message_event(event: &SignedEvent) -> bool {
    matches!(
        &event.event.body,
        EventBody::MessageCreated { .. }
            | EventBody::MessageCreatedEncrypted { .. }
            | EventBody::MessageReplyCreated { .. }
            | EventBody::MessageReplyCreatedEncrypted { .. }
            | EventBody::MessageEdited { .. }
            | EventBody::MessageEditedEncrypted { .. }
            | EventBody::MessageDeleted { .. }
            | EventBody::ReactionAdded { .. }
            | EventBody::ReactionRemoved { .. }
    )
}

fn workspace_publish_queue_summary(
    events: &[SignedEvent],
    backup_event_ids: &BTreeSet<String>,
    available_blob_hashes: &[String],
    missing_blob_hashes: &[String],
    skipped_gaps: &[PulledWorkspaceGap],
) -> WorkspacePublishQueueSummary {
    let missing_blob_hashes = missing_blob_hashes.iter().cloned().collect::<BTreeSet<_>>();
    let mut channel_summaries =
        BTreeMap::<Option<String>, WorkspacePublishQueueChannelSummaryBuilder>::new();
    let mut queued_message_event_count = 0;
    let mut queued_attachment_blob_hashes = BTreeSet::new();
    let mut oldest_event_physical_ms = None;
    let mut newest_event_physical_ms = None;

    for event in events {
        let physical_ms = event.event.timestamp.physical_ms;
        oldest_event_physical_ms = Some(
            oldest_event_physical_ms
                .map(|oldest: i64| oldest.min(physical_ms))
                .unwrap_or(physical_ms),
        );
        newest_event_physical_ms = Some(
            newest_event_physical_ms
                .map(|newest: i64| newest.max(physical_ms))
                .unwrap_or(physical_ms),
        );

        let is_backup_event = backup_event_ids.contains(&event.event_id.0);
        let is_message_event = is_queued_message_event(event);
        if is_message_event {
            queued_message_event_count += 1;
        }

        let channel_id = event.event.channel_id.as_ref().map(|id| id.0.clone());
        let channel_summary = channel_summaries.entry(channel_id).or_default();
        channel_summary.publishable_event_count += 1;
        if is_backup_event {
            channel_summary.backup_event_count += 1;
        }
        if is_message_event {
            channel_summary.queued_message_event_count += 1;
        }

        for blob_hash in event_attachment_blob_hashes(event) {
            queued_attachment_blob_hashes.insert(blob_hash.clone());
            channel_summary
                .attachment_blob_hashes
                .insert(blob_hash.clone());
            if missing_blob_hashes.contains(&blob_hash) {
                channel_summary.missing_blob_hashes.insert(blob_hash);
            }
        }
    }

    let channels = channel_summaries
        .into_iter()
        .map(
            |(channel_id, summary)| WorkspacePublishQueueChannelSummary {
                channel_id,
                publishable_event_count: summary.publishable_event_count,
                backup_event_count: summary.backup_event_count,
                queued_message_event_count: summary.queued_message_event_count,
                queued_attachment_blob_count: summary.attachment_blob_hashes.len(),
                missing_blob_count: summary.missing_blob_hashes.len(),
            },
        )
        .collect::<Vec<_>>();

    WorkspacePublishQueueSummary {
        publishable_event_count: events.len(),
        backup_event_count: backup_event_ids.len(),
        available_blob_count: available_blob_hashes.len(),
        missing_blob_count: missing_blob_hashes.len(),
        skipped_gap_count: skipped_gaps.len(),
        queued_message_event_count,
        queued_attachment_blob_count: queued_attachment_blob_hashes.len(),
        oldest_event_physical_ms,
        newest_event_physical_ms,
        has_missing_local_blobs: !missing_blob_hashes.is_empty(),
        has_skipped_gaps: !skipped_gaps.is_empty(),
        is_complete: missing_blob_hashes.is_empty() && skipped_gaps.is_empty(),
        channels,
    }
}

fn attachment_media_type_for_path(file_path: &Path, requested_media_type: &str) -> String {
    let requested_media_type = requested_media_type.trim();
    if !requested_media_type.is_empty() {
        return requested_media_type.to_owned();
    }

    let extension = file_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
    .to_owned()
}

fn attachment_id_for_message_slot(message_id: &MessageId, attachment_index: usize) -> String {
    format!("att_{}_{}", message_id.0, attachment_index)
}

fn validate_message_markdown_size(markdown: &str) -> Result<(), RuntimeError> {
    let actual_bytes = markdown.len();
    if actual_bytes > MESSAGE_MARKDOWN_MAX_BYTES {
        return Err(RuntimeError::MessageMarkdownTooLarge {
            actual_bytes,
            max_bytes: MESSAGE_MARKDOWN_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_metadata_field_size(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), RuntimeError> {
    let actual_bytes = value.len();
    if actual_bytes > max_bytes {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}

fn validate_identifier_size(result: Result<(), IdValidationError>) -> Result<(), RuntimeError> {
    result.map_err(|error| RuntimeError::MetadataFieldTooLarge {
        field: error.field,
        actual_bytes: error.actual_bytes,
        max_bytes: error.max_bytes,
    })
}

fn validate_workspace_id_reference(workspace_id: &WorkspaceId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_workspace_id(workspace_id))
}

fn validate_channel_id_reference(channel_id: &ChannelId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_channel_id(channel_id))
}

fn validate_message_id_reference(message_id: &MessageId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_message_id(message_id))
}

fn validate_device_key_package_id_reference(
    key_package_id: &DeviceKeyPackageId,
) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_device_key_package_id(key_package_id))
}

fn validate_event_id_reference(event_id: &EventId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_event_id(event_id))
}

fn read_local_metadata_file_with_limit(
    path: &Path,
    max_bytes: usize,
    field: &'static str,
) -> Result<Option<Vec<u8>>, RuntimeError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() > max_bytes as u64 {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(max_bytes as u64 + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(max_bytes as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes: bytes.len(),
            max_bytes,
        });
    }
    Ok(Some(bytes))
}

fn validate_search_query_size(query: &str) -> Result<(), RuntimeError> {
    let actual_bytes = query.len();
    if actual_bytes > SEARCH_QUERY_MAX_BYTES {
        return Err(RuntimeError::SearchQueryTooLarge {
            actual_bytes,
            max_bytes: SEARCH_QUERY_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_device_id_reference(device_id: &DeviceId) -> Result<(), RuntimeError> {
    validate_metadata_field_size("device ID", &device_id.0, DEVICE_ID_REFERENCE_MAX_BYTES)
}

fn validate_peer_endpoint_input(endpoint: &str) -> Result<(), RuntimeError> {
    if endpoint.trim().is_empty() {
        return Err(RuntimeError::PeerEndpointRequired);
    }
    validate_metadata_field_size("peer endpoint", endpoint, PEER_ENDPOINT_MAX_BYTES)?;
    if !peer_endpoint_hint_is_supported(endpoint) {
        return Err(RuntimeError::UnsupportedPeerEndpoint);
    }
    Ok(())
}

fn validate_peer_address(peer: &PeerAddress) -> Result<(), RuntimeError> {
    validate_peer_endpoint_input(&peer.endpoint)?;
    validate_metadata_field_size("peer ID", &peer.peer_id.0, PEER_ENDPOINT_ID_MAX_BYTES)
}

fn validate_peer_addresses(peers: &[PeerAddress]) -> Result<(), RuntimeError> {
    if peers.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        return Err(RuntimeError::PeerEndpointListTooLarge {
            actual_count: peers.len(),
            max_count: PEER_ENDPOINT_LIST_MAX_ITEMS,
        });
    }
    for peer in peers {
        validate_peer_address(peer)?;
    }
    Ok(())
}

fn validate_attachment_plaintext_size(actual_bytes: u64) -> Result<(), RuntimeError> {
    if actual_bytes > ATTACHMENT_FILE_MAX_BYTES {
        return Err(RuntimeError::AttachmentFileTooLarge {
            actual_bytes,
            max_bytes: ATTACHMENT_FILE_MAX_BYTES,
        });
    }
    Ok(())
}

fn read_attachment_file_with_limit(file_path: &Path) -> Result<Vec<u8>, RuntimeError> {
    validate_runtime_path(file_path, "attachment file path")?;
    let metadata = fs::metadata(file_path)?;
    validate_attachment_plaintext_size(metadata.len())?;

    let file = fs::File::open(file_path)?;
    let capacity = metadata.len().min(ATTACHMENT_FILE_MAX_BYTES) as usize;
    let mut plaintext = Vec::with_capacity(capacity);
    let mut limited_file = file.take(ATTACHMENT_FILE_MAX_BYTES + 1);
    limited_file.read_to_end(&mut plaintext)?;
    validate_attachment_plaintext_size(plaintext.len() as u64)?;
    Ok(plaintext)
}

fn current_private_channel_member_ids_from_events(
    events: &[SignedEvent],
    expected_channel_id: &ChannelId,
) -> BTreeSet<String> {
    let mut member_ids = BTreeSet::new();
    for event in events {
        match &event.event.body {
            EventBody::ChannelCreated {
                channel_id,
                is_private,
                ..
            } if channel_id == expected_channel_id && *is_private => {
                member_ids.insert(event.event.author_device_id.0.clone());
            }
            EventBody::ChannelMemberAdded {
                channel_id,
                member_device_id,
            } if channel_id == expected_channel_id => {
                member_ids.insert(member_device_id.0.clone());
            }
            EventBody::ChannelMemberRemoved {
                channel_id,
                member_device_id,
            } if channel_id == expected_channel_id => {
                member_ids.remove(&member_device_id.0);
            }
            EventBody::MemberRemoved { removed_device_id } => {
                member_ids.remove(&removed_device_id.0);
            }
            _ => {}
        }
    }
    member_ids
}

#[derive(Default)]
struct OpenMlsAutoProvisionIndex {
    used_key_package_ids: BTreeSet<String>,
    key_package_ids_by_device_id: BTreeMap<String, Vec<DeviceKeyPackageId>>,
    workspace_group_member_ids: BTreeSet<String>,
    channel_group_member_ids_by_channel_id: BTreeMap<String, BTreeSet<String>>,
}

impl OpenMlsAutoProvisionIndex {
    fn from_events(events: &[SignedEvent]) -> Self {
        let mut index = Self::default();
        for event in events {
            match &event.event.body {
                EventBody::DeviceKeyPackagePublished {
                    key_package_id,
                    protocol,
                    ..
                } if protocol == OPENMLS_KEY_PACKAGE_PROTOCOL => {
                    index
                        .key_package_ids_by_device_id
                        .entry(event.event.author_device_id.0.clone())
                        .or_default()
                        .push(key_package_id.clone());
                }
                EventBody::OpenMlsWorkspaceGroupMemberAdded {
                    invitee_device_id,
                    invitee_key_package_id,
                    ..
                } => {
                    index
                        .workspace_group_member_ids
                        .insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                    removed_device_id, ..
                } => {
                    index
                        .workspace_group_member_ids
                        .remove(&removed_device_id.0);
                }
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id,
                    invitee_device_id,
                    invitee_key_package_id,
                    ..
                } => {
                    index
                        .channel_group_member_ids_by_channel_id
                        .entry(channel_id.0.clone())
                        .or_default()
                        .insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsChannelGroupMemberRemoved {
                    channel_id,
                    removed_device_id,
                    ..
                } => {
                    if let Some(member_ids) = index
                        .channel_group_member_ids_by_channel_id
                        .get_mut(&channel_id.0)
                    {
                        member_ids.remove(&removed_device_id.0);
                    }
                }
                _ => {}
            }
        }
        index
    }

    fn workspace_group_has_device(&self, device_id: &DeviceId) -> bool {
        self.workspace_group_member_ids.contains(&device_id.0)
    }

    fn channel_group_has_device(&self, channel_id: &ChannelId, device_id: &DeviceId) -> bool {
        self.channel_group_member_ids_by_channel_id
            .get(&channel_id.0)
            .is_some_and(|member_ids| member_ids.contains(&device_id.0))
    }

    fn latest_unused_key_package_id_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Option<DeviceKeyPackageId> {
        self.key_package_ids_by_device_id
            .get(&device_id.0)?
            .iter()
            .rev()
            .find(|key_package_id| !self.used_key_package_ids.contains(&key_package_id.0))
            .cloned()
    }

    fn mark_workspace_group_member_added(&mut self, device_id: &str, key_package_id: &str) {
        self.workspace_group_member_ids.insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }

    fn mark_channel_group_member_added(
        &mut self,
        channel_id: &str,
        device_id: &str,
        key_package_id: &str,
    ) {
        self.channel_group_member_ids_by_channel_id
            .entry(channel_id.to_owned())
            .or_default()
            .insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }
}

fn merge_unique_strings(target: &mut Vec<String>, source: Vec<String>) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for value in source {
        if seen.insert(value.clone()) {
            target.push(value);
        }
    }
}

fn merge_published_workspace(target: &mut PublishedWorkspace, source: PublishedWorkspace) {
    merge_unique_strings(&mut target.published_event_ids, source.published_event_ids);
    merge_unique_strings(
        &mut target.published_blob_hashes,
        source.published_blob_hashes,
    );
    merge_unique_strings(&mut target.missing_blob_hashes, source.missing_blob_hashes);
    merge_workspace_gaps(&mut target.skipped_gaps, source.skipped_gaps);
    target
        .blob_transfer_attempts
        .extend(source.blob_transfer_attempts);
    target.refresh_counts();
}

fn merge_workspace_gaps(target: &mut Vec<PulledWorkspaceGap>, source: Vec<PulledWorkspaceGap>) {
    let mut seen = target
        .iter()
        .map(|gap| gap.event_id.clone())
        .collect::<BTreeSet<_>>();
    for gap in source {
        if seen.insert(gap.event_id.clone()) {
            target.push(gap);
        }
    }
}

fn now_unix_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.min(u128::from(u64::MAX)) as u64
}

fn truncate_string_bytes(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

fn truncate_string_option_bytes(value: &mut Option<String>, max_bytes: usize) {
    if let Some(value) = value {
        truncate_string_bytes(value, max_bytes);
    }
}

fn truncate_string_list_bytes(values: &mut [String], max_bytes: usize) {
    for value in values {
        truncate_string_bytes(value, max_bytes);
    }
}

fn planned_chunk_upload(
    bytes: &[u8],
    remote_availability: Option<&BlobAvailability>,
) -> (u64, Vec<String>, Vec<String>, Vec<String>) {
    let descriptor = describe_blob(bytes, DIRECT_BLOB_CHUNK_SIZE);
    let remote_available = remote_availability
        .filter(|availability| {
            availability.descriptor.as_ref() == Some(&descriptor)
                && validate_blob_availability(availability).is_ok()
        })
        .map(|availability| {
            availability
                .available_chunk_hashes
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut remote_available_chunk_hashes = Vec::new();
    let mut planned_chunk_hashes = Vec::new();
    let mut seen_remote = BTreeSet::new();
    let mut seen_planned = BTreeSet::new();
    for chunk_hash in &descriptor.chunk_hashes {
        if remote_available.contains(chunk_hash) {
            if seen_remote.insert(chunk_hash.clone()) {
                remote_available_chunk_hashes.push(chunk_hash.clone());
            }
        } else if seen_planned.insert(chunk_hash.clone()) {
            planned_chunk_hashes.push(chunk_hash.clone());
        }
    }

    (
        descriptor.chunk_size as u64,
        descriptor.chunk_hashes,
        planned_chunk_hashes,
        remote_available_chunk_hashes,
    )
}

fn ordered_retry_peers(peers: &[PeerAddress]) -> Vec<&PeerAddress> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for peer in peers {
        if seen.insert(peer.endpoint.clone()) {
            ordered.push(peer);
        }
    }
    ordered
}

fn workspace_openmls_commit_event(event: &SignedEvent) -> Option<(&str, u64, &[u8])> {
    match &event.event.body {
        EventBody::OpenMlsWorkspaceGroupMemberAdded {
            group_id,
            epoch,
            commit,
            ..
        }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved {
            group_id,
            epoch,
            commit,
            ..
        }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated {
            group_id,
            epoch,
            commit,
            ..
        } => Some((group_id.as_str(), *epoch, commit.as_slice())),
        _ => None,
    }
}

fn channel_openmls_commit_event<'a>(
    event: &'a SignedEvent,
    expected_channel_id: &ChannelId,
) -> Option<(&'a str, u64, &'a [u8])> {
    match &event.event.body {
        EventBody::OpenMlsChannelGroupMemberAdded {
            channel_id,
            group_id,
            epoch,
            commit,
            ..
        }
        | EventBody::OpenMlsChannelGroupMemberRemoved {
            channel_id,
            group_id,
            epoch,
            commit,
            ..
        }
        | EventBody::OpenMlsChannelGroupSelfUpdated {
            channel_id,
            group_id,
            epoch,
            commit,
            ..
        } if channel_id == expected_channel_id => {
            Some((group_id.as_str(), *epoch, commit.as_slice()))
        }
        _ => None,
    }
}

fn is_backup_slice_event(event: &SignedEvent) -> bool {
    matches!(
        &event.event.body,
        EventBody::MessageCreatedEncrypted { .. }
            | EventBody::MessageReplyCreatedEncrypted { .. }
            | EventBody::MessageEditedEncrypted { .. }
            | EventBody::MessageDeleted { .. }
            | EventBody::ReactionAdded { .. }
            | EventBody::ReactionRemoved { .. }
            | EventBody::MemberRemoved { .. }
            | EventBody::ChannelMemberRemoved { .. }
            | EventBody::DeviceProfileUpdated { .. }
            | EventBody::DeviceKeyPackagePublished { .. }
            | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
            | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
            | EventBody::OpenMlsChannelGroupMemberAdded { .. }
            | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
            | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
            | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
            | EventBody::ContentKeyEpochPublished { .. }
            | EventBody::ReadMarkerUpdated { .. }
    )
}

impl RuntimePaths {
    pub fn new(data_dir: impl AsRef<Path>, identity_file: Option<PathBuf>) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        Self {
            identity_file: identity_file.unwrap_or_else(|| data_dir.join("device.json")),
            event_store: data_dir.join("events.db"),
            search_index: data_dir.join("search.db"),
            blob_store: data_dir.join("blobs"),
            workspace_keys_dir: data_dir.join("keys"),
            blob_transfer_ledger: data_dir.join("blob-transfer-ledger.json"),
            compromise_response_ledger: data_dir.join("compromise-response-ledger.json"),
            data_dir,
        }
    }
}

fn validate_runtime_paths(paths: &RuntimePaths) -> Result<(), RuntimeError> {
    validate_runtime_path(&paths.data_dir, "data directory")?;
    validate_runtime_path(&paths.identity_file, "identity file")?;
    validate_runtime_path(&paths.event_store, "event store path")?;
    validate_runtime_path(&paths.search_index, "search index path")?;
    validate_runtime_path(&paths.blob_store, "blob store path")?;
    validate_runtime_path(&paths.workspace_keys_dir, "workspace keys path")?;
    validate_runtime_path(&paths.blob_transfer_ledger, "blob transfer ledger path")?;
    validate_runtime_path(
        &paths.compromise_response_ledger,
        "compromise response ledger path",
    )?;
    Ok(())
}

fn validate_runtime_path(path: &Path, field: &'static str) -> Result<(), RuntimeError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(RuntimeError::MetadataFieldRequired { field });
    }
    if actual_bytes > RUNTIME_PATH_MAX_BYTES {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes,
            max_bytes: RUNTIME_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

fn normalize_runtime_identity_passphrase(
    passphrase: Option<&str>,
) -> Result<Option<String>, RuntimeError> {
    match passphrase {
        Some(passphrase) if passphrase.len() > RUNTIME_PASSPHRASE_MAX_BYTES => {
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "identity passphrase",
                actual_bytes: passphrase.len(),
                max_bytes: RUNTIME_PASSPHRASE_MAX_BYTES,
            })
        }
        Some(passphrase) if passphrase.trim().is_empty() => Ok(None),
        Some(passphrase) => Ok(Some(passphrase.to_owned())),
        None => Ok(None),
    }
}

impl LocalRuntime {
    pub fn open(
        data_dir: impl AsRef<Path>,
        identity_file: Option<PathBuf>,
    ) -> Result<Self, RuntimeError> {
        Self::open_with_identity_passphrase(data_dir, identity_file, None)
    }

    pub fn open_with_identity_passphrase(
        data_dir: impl AsRef<Path>,
        identity_file: Option<PathBuf>,
        identity_passphrase: Option<&str>,
    ) -> Result<Self, RuntimeError> {
        let paths = RuntimePaths::new(data_dir, identity_file);
        validate_runtime_paths(&paths)?;
        let identity_passphrase = normalize_runtime_identity_passphrase(identity_passphrase)?;
        fs::create_dir_all(&paths.data_dir)?;
        fs::create_dir_all(&paths.workspace_keys_dir)?;
        let identity = DeviceIdentity::load_or_generate_with_passphrase(
            &paths.identity_file,
            identity_passphrase.as_deref(),
        )?;
        let store = EventStore::open(&paths.event_store)?;

        Ok(Self {
            paths,
            identity,
            identity_passphrase,
            store,
        })
    }

    pub fn paths(&self) -> &RuntimePaths {
        &self.paths
    }

    pub fn blob_transfer_ledger(&self) -> Result<BlobTransferLedger, RuntimeError> {
        self.read_blob_transfer_ledger()
    }

    pub fn device_id(&self) -> &DeviceId {
        self.identity.device_id()
    }

    pub fn list_workspaces(&self) -> Result<Vec<LocalWorkspaceSummary>, RuntimeError> {
        let total_count = self.store.count_workspaces()?;
        let mut workspaces = Vec::with_capacity(total_count.min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS));
        let mut start_index = 0usize;

        while start_index < total_count {
            let page = self.list_workspace_page_uncapped(
                start_index,
                (total_count - start_index).min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS),
            )?;
            let item_count = page.item_count;
            workspaces.extend(page.workspaces);
            if item_count == 0 {
                break;
            }
            start_index = start_index.saturating_add(item_count);
        }

        Ok(workspaces)
    }

    pub fn list_workspace_page(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<LocalWorkspaceSummaryPage, RuntimeError> {
        self.list_workspace_page_uncapped(start_index, limit.min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS))
    }

    fn list_workspace_page_uncapped(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<LocalWorkspaceSummaryPage, RuntimeError> {
        let total_count = self.store.count_workspaces()?;
        let start_index = start_index.min(total_count);
        let end_index = start_index.saturating_add(limit).min(total_count);
        let workspace_ids = self
            .store
            .list_workspace_ids_page(start_index, end_index - start_index)?;
        let mut summaries = Vec::new();
        for workspace_id in workspace_ids {
            let workspace_id = WorkspaceId(workspace_id);
            let event_count = self.store.count_events_for_workspace(&workspace_id.0)?;
            let events = self
                .store
                .list_servable_events_for_workspace(&workspace_id.0)?;
            summaries.push(self.local_workspace_summary(&workspace_id, &events, event_count)?);
        }
        Ok(LocalWorkspaceSummaryPage {
            start_index,
            item_count: summaries.len(),
            total_count,
            has_more_before: start_index > 0,
            has_more_after: end_index < total_count,
            workspaces: summaries,
        })
    }

    pub fn list_workspace_member_page(
        &self,
        workspace_id: WorkspaceId,
        start_index: usize,
        limit: usize,
    ) -> Result<WorkspaceMemberPage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let limit = limit.min(MAX_WORKSPACE_MEMBER_PAGE_ROWS);
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        Ok(WorkspaceMemberPage::from_events(
            workspace_id,
            &events,
            start_index,
            limit,
        )?)
    }

    pub fn list_workspace_channel_page(
        &self,
        workspace_id: WorkspaceId,
        start_index: usize,
        limit: usize,
    ) -> Result<WorkspaceChannelPage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let limit = limit.min(MAX_WORKSPACE_CHANNEL_PAGE_ROWS);
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        let empty_body_overrides = HashMap::new();
        let raw_page = WorkspaceChannelPage::from_state_report_for_device_and_body_overrides(
            &state,
            &report,
            &raw_events,
            self.identity.device_id(),
            &empty_body_overrides,
            start_index,
            limit,
        );
        let body_override_event_ids = Self::channel_page_body_override_event_ids(&state, &raw_page);
        let body_overrides = self.decrypted_body_overrides_for_event_ids(
            &workspace_id,
            &state,
            workspace_key.as_ref(),
            &body_override_event_ids,
        )?;

        Ok(
            WorkspaceChannelPage::from_state_report_for_device_and_body_overrides(
                &state,
                &report,
                &raw_events,
                self.identity.device_id(),
                &body_overrides,
                start_index,
                limit,
            ),
        )
    }

    pub fn list_workspace_channel_page_containing(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        limit: usize,
    ) -> Result<WorkspaceChannelPage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        let limit = limit.min(MAX_WORKSPACE_CHANNEL_PAGE_ROWS);
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        if !state.channels.contains_key(&channel_id) {
            return Err(RuntimeError::ChannelNotFound {
                workspace_id,
                channel_id,
            });
        }
        if !state.channel_accessible_to(&channel_id, self.identity.device_id()) {
            return Err(RuntimeError::ChannelAccessDenied {
                workspace_id,
                channel_id,
                device_id: self.identity.device_id().clone(),
            });
        }

        let empty_body_overrides = HashMap::new();
        let raw_page =
            WorkspaceChannelPage::from_state_report_for_device_and_body_overrides_containing_channel(
                &state,
                &report,
                &raw_events,
                self.identity.device_id(),
                &empty_body_overrides,
                &channel_id,
                limit,
            )
            .ok_or_else(|| RuntimeError::ChannelNotFound {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            })?;
        let body_override_event_ids = Self::channel_page_body_override_event_ids(&state, &raw_page);
        let body_overrides = self.decrypted_body_overrides_for_event_ids(
            &workspace_id,
            &state,
            workspace_key.as_ref(),
            &body_override_event_ids,
        )?;

        WorkspaceChannelPage::from_state_report_for_device_and_body_overrides_containing_channel(
            &state,
            &report,
            &raw_events,
            self.identity.device_id(),
            &body_overrides,
            &channel_id,
            limit,
        )
        .ok_or(RuntimeError::ChannelNotFound {
            workspace_id,
            channel_id,
        })
    }

    pub fn search_workspace_channels(
        &self,
        workspace_id: WorkspaceId,
        query: impl AsRef<str>,
        limit: usize,
    ) -> Result<WorkspaceChannelSearch, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let limit = limit.min(MAX_WORKSPACE_CHANNEL_SEARCH_ROWS);
        let query = query.as_ref().trim().to_owned();
        validate_search_query_size(&query)?;
        if !query_has_channel_search_terms(&query) {
            return Ok(WorkspaceChannelSearch {
                query,
                item_count: 0,
                total_count: 0,
                channels: Vec::new(),
            });
        }

        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        let empty_body_overrides = HashMap::new();
        let raw_search = WorkspaceChannelSearch::from_state_report_for_device_and_body_overrides(
            &state,
            &report,
            &raw_events,
            self.identity.device_id(),
            &empty_body_overrides,
            &query,
            limit,
        );
        let body_override_event_ids =
            Self::channel_rows_body_override_event_ids(&state, &raw_search.channels);
        let body_overrides = self.decrypted_body_overrides_for_event_ids(
            &workspace_id,
            &state,
            workspace_key.as_ref(),
            &body_override_event_ids,
        )?;

        Ok(
            WorkspaceChannelSearch::from_state_report_for_device_and_body_overrides(
                &state,
                &report,
                &raw_events,
                self.identity.device_id(),
                &body_overrides,
                &query,
                limit,
            ),
        )
    }

    pub fn create_workspace(
        &self,
        name: impl Into<String>,
        default_channel_name: impl Into<String>,
    ) -> Result<CreatedWorkspace, RuntimeError> {
        let name = name.into();
        let default_channel_name = default_channel_name.into();
        validate_metadata_field_size("workspace name", &name, WORKSPACE_NAME_MAX_BYTES)?;
        validate_metadata_field_size(
            "default channel name",
            &default_channel_name,
            CHANNEL_NAME_MAX_BYTES,
        )?;

        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let workspace_key = WorkspaceKey::generate(workspace_id.clone());
        self.save_workspace_key(&workspace_key)?;

        let workspace = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceCreated { name },
        );
        let workspace = self.identity.sign_event(workspace);
        self.store.append_event(&workspace)?;

        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: default_channel_name,
                is_private: false,
            },
        );
        channel.parents = vec![workspace.event_id.clone()];
        let channel = self.sign_authorize_and_append(channel)?;

        Ok(CreatedWorkspace {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            owner_device_id: self.identity.device_id().0.clone(),
            workspace_event_id: workspace.event_id.0,
            channel_event_id: channel.event_id.0,
        })
    }

    pub fn create_channel(
        &self,
        workspace_id: WorkspaceId,
        name: impl Into<String>,
        is_private: bool,
    ) -> Result<CreatedChannel, RuntimeError> {
        let name = name.into();
        validate_metadata_field_size("channel name", &name, CHANNEL_NAME_MAX_BYTES)?;

        let channel_id = ChannelId::new();
        let context = self.workspace_write_context(&workspace_id)?;
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name,
                is_private,
            },
        );
        channel.parents = context.head_event_ids.clone();
        let channel_key =
            is_private.then(|| ChannelKey::generate(workspace_id.clone(), channel_id.clone()));
        let channel = self.identity.sign_event(channel);
        authorize_event_with_history(&context.events, &channel)?;
        if let Some(channel_key) = channel_key.as_ref() {
            self.save_channel_key(channel_key)?;
        }
        self.store.append_event(&channel)?;

        Ok(CreatedChannel {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            event_id: channel.event_id.0,
        })
    }

    pub fn update_device_profile(
        &self,
        workspace_id: WorkspaceId,
        display_name: impl AsRef<str>,
    ) -> Result<UpdatedDeviceProfile, RuntimeError> {
        let display_name = display_name.as_ref().trim().to_owned();
        if display_name.is_empty() {
            return Err(RuntimeError::DisplayNameRequired);
        }
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: display_name.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(UpdatedDeviceProfile {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            display_name,
            event_id: event.event_id.0,
        })
    }

    pub fn publish_device_key_package(
        &self,
        workspace_id: WorkspaceId,
        protocol: impl AsRef<str>,
        key_package: Vec<u8>,
    ) -> Result<PublishedDeviceKeyPackage, RuntimeError> {
        let protocol = protocol.as_ref().trim().to_owned();
        if protocol.is_empty() {
            return Err(RuntimeError::DeviceKeyPackageProtocolRequired);
        }
        validate_metadata_field_size(
            "device key package protocol",
            &protocol,
            DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
        )?;
        if key_package.is_empty() {
            return Err(RuntimeError::DeviceKeyPackageRequired);
        }
        if key_package.len() > DEVICE_KEY_PACKAGE_MAX_LEN {
            return Err(RuntimeError::DeviceKeyPackageTooLarge);
        }

        let key_package_id = DeviceKeyPackageId::new();
        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: protocol.clone(),
                key_package,
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        let byte_len = match &event.event.body {
            EventBody::DeviceKeyPackagePublished { key_package, .. } => key_package.len(),
            _ => 0,
        };

        Ok(PublishedDeviceKeyPackage {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            key_package_id: key_package_id.0,
            protocol,
            byte_len,
            event_id: event.event_id.0,
        })
    }

    pub fn publish_peer_endpoint(
        &self,
        workspace_id: WorkspaceId,
        endpoint_id: impl AsRef<str>,
        endpoint: impl AsRef<str>,
        transport: impl AsRef<str>,
        is_backup_peer: bool,
        expires_at_ms: Option<i64>,
    ) -> Result<PublishedPeerEndpoint, RuntimeError> {
        let endpoint_id = endpoint_id.as_ref().trim().to_owned();
        if endpoint_id.is_empty() {
            return Err(RuntimeError::PeerEndpointIdRequired);
        }
        validate_metadata_field_size("peer endpoint ID", &endpoint_id, PEER_ENDPOINT_ID_MAX_BYTES)?;
        let endpoint = endpoint.as_ref().trim().to_owned();
        if endpoint.is_empty() {
            return Err(RuntimeError::PeerEndpointRequired);
        }
        validate_metadata_field_size("peer endpoint", &endpoint, PEER_ENDPOINT_MAX_BYTES)?;
        if !peer_endpoint_hint_is_supported(&endpoint) {
            return Err(RuntimeError::UnsupportedPeerEndpoint);
        }
        let transport = transport.as_ref().trim().to_owned();
        if transport.is_empty() {
            return Err(RuntimeError::PeerEndpointTransportRequired);
        }
        validate_metadata_field_size(
            "peer endpoint transport",
            &transport,
            PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
        )?;
        if !peer_endpoint_hint_transport_is_consistent(&endpoint, &transport) {
            return Err(RuntimeError::PeerEndpointTransportMismatch);
        }

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: endpoint_id.clone(),
                endpoint: endpoint.clone(),
                transport: transport.clone(),
                is_backup_peer,
                expires_at_ms,
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(PublishedPeerEndpoint {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            expires_at_ms,
            event_id: event.event_id.0,
        })
    }

    pub fn publish_openmls_device_key_package(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedOpenMlsKeyPackage, RuntimeError> {
        let generated = chaft_mls::generate_device_key_package(&self.identity.device_id().0)?;
        debug_assert_eq!(generated.protocol, OPENMLS_KEY_PACKAGE_PROTOCOL);
        let key_package_id = DeviceKeyPackageId::new();
        let private_bundle_path =
            self.openmls_key_package_path(&workspace_id, &generated.key_package_ref);
        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: generated.protocol.clone(),
                key_package: generated.key_package.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_bundle_path,
                    LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
                    &generated.private_bundle,
                )
            },
        )?;

        Ok(PublishedOpenMlsKeyPackage {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            key_package_id: key_package_id.0,
            protocol: generated.protocol,
            ciphersuite: generated.ciphersuite,
            key_package_ref: generated.key_package_ref,
            byte_len: generated.key_package.len(),
            private_bundle_path: private_bundle_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn create_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<CreatedOpenMlsWorkspaceGroup, RuntimeError> {
        self.require_workspace_admin(&workspace_id, "create_openmls_workspace_group")?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { workspace_id });
        }

        let created =
            chaft_mls::create_workspace_group(&self.identity.device_id().0, &workspace_id.0)?;
        debug_assert_eq!(created.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
            &created.private_group_state,
        )?;

        Ok(CreatedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: created.protocol,
            ciphersuite: created.ciphersuite,
            group_id: created.group_id,
            epoch: created.epoch,
            member_count: created.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn add_openmls_workspace_group_member(
        &self,
        workspace_id: WorkspaceId,
        key_package_id: DeviceKeyPackageId,
    ) -> Result<AddedOpenMlsWorkspaceGroupMember, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_device_key_package_id_reference(&key_package_id)?;
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_workspace_admin_in_state(
            &context.state,
            "add_openmls_workspace_group_member",
        )?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let key_package = context
            .state
            .key_packages
            .get(&key_package_id)
            .ok_or_else(|| RuntimeError::DeviceKeyPackageNotFound {
                workspace_id: workspace_id.clone(),
                key_package_id: key_package_id.clone(),
            })?;
        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let added = chaft_mls::add_member_to_workspace_group(
            &private_group_state,
            &key_package.key_package,
        )?;
        debug_assert_eq!(added.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: key_package.device_id.clone(),
                invitee_key_package_id: key_package_id.clone(),
                invitee_key_package_ref: added.invitee_key_package_ref.clone(),
                protocol: added.protocol.clone(),
                ciphersuite: added.ciphersuite.clone(),
                group_id: added.group_id.clone(),
                epoch: added.epoch,
                commit: added.commit.clone(),
                welcome: added.welcome.clone(),
                ratchet_tree: added.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &added.updated_private_group_state,
                )
            },
        )?;

        Ok(AddedOpenMlsWorkspaceGroupMember {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            invitee_device_id: key_package.device_id.0.clone(),
            invitee_key_package_id: key_package_id.0,
            invitee_key_package_ref: added.invitee_key_package_ref,
            protocol: added.protocol,
            ciphersuite: added.ciphersuite,
            group_id: added.group_id,
            epoch: added.epoch,
            member_count: added.member_count,
            commit_byte_len: added.commit.len(),
            welcome_byte_len: added.welcome.len(),
            ratchet_tree_byte_len: added.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn remove_openmls_workspace_group_member(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedOpenMlsWorkspaceGroupMember, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_workspace_admin_in_state(
            &context.state,
            "remove_openmls_workspace_group_member",
        )?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let removed =
            chaft_mls::remove_member_from_group(&private_group_state, &removed_device_id.0)?;
        debug_assert_eq!(removed.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                removed_device_id: removed_device_id.clone(),
                protocol: removed.protocol.clone(),
                ciphersuite: removed.ciphersuite.clone(),
                group_id: removed.group_id.clone(),
                epoch: removed.epoch,
                commit: removed.commit.clone(),
                ratchet_tree: removed.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &removed.updated_private_group_state,
                )
            },
        )?;

        Ok(RemovedOpenMlsWorkspaceGroupMember {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            removed_device_id: removed_device_id.0,
            protocol: removed.protocol,
            ciphersuite: removed.ciphersuite,
            group_id: removed.group_id,
            epoch: removed.epoch,
            member_count: removed.member_count,
            commit_byte_len: removed.commit.len(),
            ratchet_tree_byte_len: removed.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn join_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
        source_event_id: Option<EventId>,
    ) -> Result<JoinedOpenMlsWorkspaceGroup, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists {
                workspace_id: workspace_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let selected = events.iter().rev().find_map(|event| {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                return None;
            }
            match &event.event.body {
                EventBody::OpenMlsWorkspaceGroupMemberAdded {
                    invitee_device_id,
                    invitee_key_package_ref,
                    welcome,
                    ratchet_tree,
                    ..
                } if invitee_device_id == self.identity.device_id() => Some((
                    event.event_id.clone(),
                    invitee_key_package_ref.clone(),
                    welcome.clone(),
                    ratchet_tree.clone(),
                )),
                _ => None,
            }
        });
        let Some((event_id, key_package_ref, welcome, ratchet_tree)) = selected else {
            return Err(RuntimeError::OpenMlsWorkspaceGroupInviteNotFound {
                workspace_id,
                device_id: self.identity.device_id().clone(),
            });
        };

        let private_bundle_path = self.openmls_key_package_path(&workspace_id, &key_package_ref);
        if !private_bundle_path.exists() {
            return Err(RuntimeError::OpenMlsPrivateKeyPackageMissing {
                workspace_id,
                key_package_ref,
            });
        }

        let joined = chaft_mls::join_workspace_group_from_welcome(
            &self.read_openmls_secret_file(
                &private_bundle_path,
                LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
            )?,
            &welcome,
            &ratchet_tree,
        )?;
        debug_assert_eq!(joined.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
            &joined.private_group_state,
        )?;

        Ok(JoinedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: joined.protocol,
            ciphersuite: joined.ciphersuite,
            group_id: joined.group_id,
            epoch: joined.epoch,
            member_count: joined.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            source_event_id: event_id.0,
        })
    }

    pub fn update_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<UpdatedOpenMlsWorkspaceGroup, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let updated = chaft_mls::update_own_leaf_in_group(&private_group_state)?;
        debug_assert_eq!(updated.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupSelfUpdated {
                protocol: updated.protocol.clone(),
                ciphersuite: updated.ciphersuite.clone(),
                group_id: updated.group_id.clone(),
                epoch: updated.epoch,
                commit: updated.commit.clone(),
                ratchet_tree: updated.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &updated.updated_private_group_state,
                )
            },
        )?;

        Ok(UpdatedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: updated.protocol,
            ciphersuite: updated.ciphersuite,
            group_id: updated.group_id,
            epoch: updated.epoch,
            member_count: updated.member_count,
            commit_byte_len: updated.commit.len(),
            ratchet_tree_byte_len: updated.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn apply_openmls_workspace_group_commits(
        &self,
        workspace_id: WorkspaceId,
        source_event_id: Option<EventId>,
    ) -> Result<AppliedOpenMlsWorkspaceGroupCommits, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let mut validated =
            chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
        let mut protocol = validated.protocol.clone();
        let mut ciphersuite = validated.ciphersuite.clone();
        let mut group_id = validated.group_id.clone();
        let mut epoch = validated.epoch;
        let mut member_count = validated.member_count;
        let mut applied_event_ids = Vec::new();
        let mut self_removed = false;
        let mut selected_event_found = source_event_id.is_none();

        for event in &events {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                continue;
            }
            if source_event_id.is_some() {
                selected_event_found = true;
            }

            let Some((event_group_id, event_epoch, commit)) = workspace_openmls_commit_event(event)
            else {
                continue;
            };
            if event_group_id != validated.group_id || event_epoch <= validated.epoch {
                continue;
            }

            let applied = chaft_mls::apply_group_commit(&private_group_state, commit)?;
            private_group_state = applied.updated_private_group_state.clone();
            self_removed |= applied.self_removed;
            applied_event_ids.push(event.event_id.0.clone());
            protocol = applied.protocol.clone();
            ciphersuite = applied.ciphersuite.clone();
            group_id = applied.group_id.clone();
            epoch = applied.epoch;
            member_count = applied.member_count;
            if applied.self_removed {
                break;
            }
            validated = chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
            protocol = validated.protocol.clone();
            ciphersuite = validated.ciphersuite.clone();
            group_id = validated.group_id.clone();
            epoch = validated.epoch;
            member_count = validated.member_count;
        }

        if !selected_event_found && let Some(source_event_id) = source_event_id {
            return Err(RuntimeError::EventNotFound {
                workspace_id,
                event_id: source_event_id,
            });
        }

        if !applied_event_ids.is_empty() {
            self.write_openmls_secret_file(
                &private_group_state_path,
                LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                &private_group_state,
            )?;
        }

        Ok(AppliedOpenMlsWorkspaceGroupCommits {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol,
            ciphersuite,
            group_id,
            epoch,
            member_count,
            applied_event_count: applied_event_ids.len(),
            applied_event_ids,
            self_removed,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn create_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<CreatedOpenMlsChannelGroup, RuntimeError> {
        self.require_local_channel_access(&workspace_id, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupAlreadyExists {
                workspace_id,
                channel_id,
            });
        }

        let created = chaft_mls::create_channel_group(
            &self.identity.device_id().0,
            &workspace_id.0,
            &channel_id.0,
        )?;
        debug_assert_eq!(created.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
            &created.private_group_state,
        )?;

        Ok(CreatedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: created.protocol,
            ciphersuite: created.ciphersuite,
            group_id: created.group_id,
            epoch: created.epoch,
            member_count: created.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn add_openmls_channel_group_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        key_package_id: DeviceKeyPackageId,
    ) -> Result<AddedOpenMlsChannelGroupMember, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        validate_device_key_package_id_reference(&key_package_id)?;
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let key_package = context
            .state
            .key_packages
            .get(&key_package_id)
            .ok_or_else(|| RuntimeError::DeviceKeyPackageNotFound {
                workspace_id: workspace_id.clone(),
                key_package_id: key_package_id.clone(),
            })?;
        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let added = chaft_mls::add_member_to_workspace_group(
            &private_group_state,
            &key_package.key_package,
        )?;
        debug_assert_eq!(added.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id: channel_id.clone(),
                invitee_device_id: key_package.device_id.clone(),
                invitee_key_package_id: key_package_id.clone(),
                invitee_key_package_ref: added.invitee_key_package_ref.clone(),
                protocol: added.protocol.clone(),
                ciphersuite: added.ciphersuite.clone(),
                group_id: added.group_id.clone(),
                epoch: added.epoch,
                commit: added.commit.clone(),
                welcome: added.welcome.clone(),
                ratchet_tree: added.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &added.updated_private_group_state,
                )
            },
        )?;

        Ok(AddedOpenMlsChannelGroupMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            invitee_device_id: key_package.device_id.0.clone(),
            invitee_key_package_id: key_package_id.0,
            invitee_key_package_ref: added.invitee_key_package_ref,
            protocol: added.protocol,
            ciphersuite: added.ciphersuite,
            group_id: added.group_id,
            epoch: added.epoch,
            member_count: added.member_count,
            commit_byte_len: added.commit.len(),
            welcome_byte_len: added.welcome.len(),
            ratchet_tree_byte_len: added.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn remove_openmls_channel_group_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedOpenMlsChannelGroupMember, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_local_channel_access_in_state(&context.state, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let removed =
            chaft_mls::remove_member_from_group(&private_group_state, &removed_device_id.0)?;
        debug_assert_eq!(removed.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupMemberRemoved {
                channel_id: channel_id.clone(),
                removed_device_id: removed_device_id.clone(),
                protocol: removed.protocol.clone(),
                ciphersuite: removed.ciphersuite.clone(),
                group_id: removed.group_id.clone(),
                epoch: removed.epoch,
                commit: removed.commit.clone(),
                ratchet_tree: removed.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &removed.updated_private_group_state,
                )
            },
        )?;

        Ok(RemovedOpenMlsChannelGroupMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            removed_device_id: removed_device_id.0,
            protocol: removed.protocol,
            ciphersuite: removed.ciphersuite,
            group_id: removed.group_id,
            epoch: removed.epoch,
            member_count: removed.member_count,
            commit_byte_len: removed.commit.len(),
            ratchet_tree_byte_len: removed.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn join_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        source_event_id: Option<EventId>,
    ) -> Result<JoinedOpenMlsChannelGroup, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupAlreadyExists {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let selected = events.iter().rev().find_map(|event| {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                return None;
            }
            match &event.event.body {
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id: event_channel_id,
                    invitee_device_id,
                    invitee_key_package_ref,
                    welcome,
                    ratchet_tree,
                    ..
                } if event_channel_id == &channel_id
                    && invitee_device_id == self.identity.device_id() =>
                {
                    Some((
                        event.event_id.clone(),
                        invitee_key_package_ref.clone(),
                        welcome.clone(),
                        ratchet_tree.clone(),
                    ))
                }
                _ => None,
            }
        });
        let Some((event_id, key_package_ref, welcome, ratchet_tree)) = selected else {
            return Err(RuntimeError::OpenMlsChannelGroupInviteNotFound {
                workspace_id,
                channel_id,
                device_id: self.identity.device_id().clone(),
            });
        };

        let private_bundle_path = self.openmls_key_package_path(&workspace_id, &key_package_ref);
        if !private_bundle_path.exists() {
            return Err(RuntimeError::OpenMlsPrivateKeyPackageMissing {
                workspace_id,
                key_package_ref,
            });
        }

        let joined = chaft_mls::join_channel_group_from_welcome(
            &self.read_openmls_secret_file(
                &private_bundle_path,
                LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
            )?,
            &welcome,
            &ratchet_tree,
        )?;
        debug_assert_eq!(joined.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
            &joined.private_group_state,
        )?;

        Ok(JoinedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: joined.protocol,
            ciphersuite: joined.ciphersuite,
            group_id: joined.group_id,
            epoch: joined.epoch,
            member_count: joined.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            source_event_id: event_id.0,
        })
    }

    pub fn update_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<UpdatedOpenMlsChannelGroup, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_local_channel_access_in_state(&context.state, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let updated = chaft_mls::update_own_leaf_in_group(&private_group_state)?;
        debug_assert_eq!(updated.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupSelfUpdated {
                channel_id: channel_id.clone(),
                protocol: updated.protocol.clone(),
                ciphersuite: updated.ciphersuite.clone(),
                group_id: updated.group_id.clone(),
                epoch: updated.epoch,
                commit: updated.commit.clone(),
                ratchet_tree: updated.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &updated.updated_private_group_state,
                )
            },
        )?;

        Ok(UpdatedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: updated.protocol,
            ciphersuite: updated.ciphersuite,
            group_id: updated.group_id,
            epoch: updated.epoch,
            member_count: updated.member_count,
            commit_byte_len: updated.commit.len(),
            ratchet_tree_byte_len: updated.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn update_workspace_openmls_groups(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<UpdatedWorkspaceOpenMlsGroups, RuntimeError> {
        let has_workspace_group = self.openmls_workspace_group_path(&workspace_id).exists();
        let channel_ids = self.local_updatable_openmls_channel_group_ids(&workspace_id)?;
        if !has_workspace_group && channel_ids.is_empty() {
            return Err(RuntimeError::OpenMlsLocalGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let workspace_update = if has_workspace_group {
            Some(self.update_openmls_workspace_group(workspace_id.clone())?)
        } else {
            None
        };
        let mut channel_updates = Vec::with_capacity(channel_ids.len());
        for channel_id in channel_ids {
            channel_updates
                .push(self.update_openmls_channel_group(workspace_id.clone(), channel_id)?);
        }

        let mut updated_event_ids = Vec::with_capacity(
            if workspace_update.is_some() { 1 } else { 0 } + channel_updates.len(),
        );
        if let Some(workspace_update) = &workspace_update {
            updated_event_ids.push(workspace_update.event_id.clone());
        }
        updated_event_ids.extend(channel_updates.iter().map(|update| update.event_id.clone()));

        Ok(UpdatedWorkspaceOpenMlsGroups {
            workspace_id: workspace_id.0,
            workspace_update,
            channel_update_count: channel_updates.len(),
            channel_updates,
            updated_event_count: updated_event_ids.len(),
            updated_event_ids,
        })
    }

    pub fn apply_openmls_channel_group_commits(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        source_event_id: Option<EventId>,
    ) -> Result<AppliedOpenMlsChannelGroupCommits, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let mut validated =
            chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
        let mut protocol = validated.protocol.clone();
        let mut ciphersuite = validated.ciphersuite.clone();
        let mut group_id = validated.group_id.clone();
        let mut epoch = validated.epoch;
        let mut member_count = validated.member_count;
        let mut applied_event_ids = Vec::new();
        let mut self_removed = false;
        let mut selected_event_found = source_event_id.is_none();

        for event in &events {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                continue;
            }
            if source_event_id.is_some() {
                selected_event_found = true;
            }

            let Some((event_group_id, event_epoch, commit)) =
                channel_openmls_commit_event(event, &channel_id)
            else {
                continue;
            };
            if event_group_id != validated.group_id || event_epoch <= validated.epoch {
                continue;
            }

            let applied = chaft_mls::apply_group_commit(&private_group_state, commit)?;
            private_group_state = applied.updated_private_group_state.clone();
            self_removed |= applied.self_removed;
            applied_event_ids.push(event.event_id.0.clone());
            protocol = applied.protocol.clone();
            ciphersuite = applied.ciphersuite.clone();
            group_id = applied.group_id.clone();
            epoch = applied.epoch;
            member_count = applied.member_count;
            if applied.self_removed {
                break;
            }
            validated = chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
            protocol = validated.protocol.clone();
            ciphersuite = validated.ciphersuite.clone();
            group_id = validated.group_id.clone();
            epoch = validated.epoch;
            member_count = validated.member_count;
        }

        if !selected_event_found && let Some(source_event_id) = source_event_id {
            return Err(RuntimeError::EventNotFound {
                workspace_id,
                event_id: source_event_id,
            });
        }

        if !applied_event_ids.is_empty() {
            self.write_openmls_secret_file(
                &private_group_state_path,
                LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                &private_group_state,
            )?;
        }

        Ok(AppliedOpenMlsChannelGroupCommits {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol,
            ciphersuite,
            group_id,
            epoch,
            member_count,
            applied_event_count: applied_event_ids.len(),
            applied_event_ids,
            self_removed,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn rotate_workspace_key(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceKey, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let mut workspace_key = self
            .load_workspace_key(&workspace_id)?
            .ok_or(RuntimeError::InvalidWorkspaceKey)?;
        let previous_key_id = workspace_key.key_id.clone();
        workspace_key.rotate();

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: workspace_key.epoch,
                key_id: workspace_key.key_id.clone(),
                previous_key_id: Some(previous_key_id.clone()),
                algorithm: CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV.to_owned(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| runtime.save_workspace_key(&workspace_key),
        )?;

        Ok(RotatedWorkspaceKey {
            workspace_id: workspace_id.0,
            previous_key_id,
            key_id: workspace_key.key_id,
            epoch: workspace_key.epoch,
            event_id: event.event_id.0,
        })
    }

    pub fn rotate_channel_key(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<RotatedChannelKey, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let mut channel_key = self
            .load_channel_key(&workspace_id, &channel_id)?
            .ok_or_else(|| RuntimeError::ChannelKeyMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            })?;
        let previous_key_id = channel_key.key_id.clone();
        channel_key.rotate();

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Channel {
                    channel_id: channel_id.clone(),
                },
                epoch: channel_key.epoch,
                key_id: channel_key.key_id.clone(),
                previous_key_id: Some(previous_key_id.clone()),
                algorithm: CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV.to_owned(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| runtime.save_channel_key(&channel_key),
        )?;

        Ok(RotatedChannelKey {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            previous_key_id,
            key_id: channel_key.key_id,
            epoch: channel_key.epoch,
            event_id: event.event_id.0,
        })
    }

    pub fn rotate_workspace_manual_keys(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceManualKeys, RuntimeError> {
        let workspace_key_rotation = self.rotate_workspace_key(workspace_id.clone())?;
        let mut channel_key_rotations = Vec::new();
        for channel_id in self.local_private_channel_key_ids(&workspace_id)? {
            channel_key_rotations.push(self.rotate_channel_key(workspace_id.clone(), channel_id)?);
        }
        let mut rotated_event_ids = Vec::with_capacity(1 + channel_key_rotations.len());
        rotated_event_ids.push(workspace_key_rotation.event_id.clone());
        rotated_event_ids.extend(
            channel_key_rotations
                .iter()
                .map(|rotation| rotation.event_id.clone()),
        );

        Ok(RotatedWorkspaceManualKeys {
            workspace_id: workspace_id.0,
            workspace_key_rotation,
            channel_key_rotation_count: channel_key_rotations.len(),
            channel_key_rotations,
            rotated_event_count: rotated_event_ids.len(),
            rotated_event_ids,
        })
    }

    pub fn rotate_workspace_for_suspected_compromise(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceForSuspectedCompromise, RuntimeError> {
        let openmls_updates = match self.update_workspace_openmls_groups(workspace_id.clone()) {
            Ok(updated) => Some(updated),
            Err(RuntimeError::OpenMlsLocalGroupMissing { .. }) => None,
            Err(error) => return Err(error),
        };
        let manual_key_rotation = if self.workspace_key_path(&workspace_id).exists() {
            Some(self.rotate_workspace_manual_keys(workspace_id.clone())?)
        } else {
            None
        };
        if openmls_updates.is_none() && manual_key_rotation.is_none() {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }

        let mut rotated_event_ids = Vec::new();
        if let Some(openmls_updates) = &openmls_updates {
            rotated_event_ids.extend(openmls_updates.updated_event_ids.iter().cloned());
        }
        if let Some(manual_key_rotation) = &manual_key_rotation {
            rotated_event_ids.extend(manual_key_rotation.rotated_event_ids.iter().cloned());
        }

        Ok(RotatedWorkspaceForSuspectedCompromise {
            workspace_id: workspace_id.0,
            openmls_updates,
            manual_key_rotation,
            rotated_event_count: rotated_event_ids.len(),
            rotated_event_ids,
        })
    }

    pub fn detect_workspace_compromise_signals(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceCompromiseReport, RuntimeError> {
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let signals = events
            .iter()
            .filter_map(|event| {
                workspace_compromise_signal_from_event(event, self.identity.device_id())
            })
            .collect::<Vec<_>>();
        let local_device_signal_count = signals.iter().filter(|signal| signal.local_device).count();
        let invalid_signature_count = signals
            .iter()
            .filter(|signal| signal.kind == COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE)
            .count();
        let should_rotate_local_secret_state = local_device_signal_count > 0;
        let recommended_action = if should_rotate_local_secret_state {
            Some(COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE.to_owned())
        } else if !signals.is_empty() {
            Some(COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES.to_owned())
        } else {
            None
        };

        Ok(WorkspaceCompromiseReport {
            workspace_id: workspace_id.0,
            has_signals: !signals.is_empty(),
            signal_count: signals.len(),
            invalid_signature_count,
            local_device_signal_count,
            should_rotate_local_secret_state,
            recommended_action,
            signals,
        })
    }

    pub fn respond_to_workspace_compromise_signals(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceCompromiseResponse, RuntimeError> {
        let report = self.detect_workspace_compromise_signals(workspace_id.clone())?;
        self.respond_to_workspace_compromise_report(workspace_id, report)
    }

    fn automatic_compromise_response_if_needed(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceCompromiseResponse>, RuntimeError> {
        let report = self.detect_workspace_compromise_signals(workspace_id.clone())?;
        if !report.has_signals {
            return Ok(None);
        }

        self.respond_to_workspace_compromise_report(workspace_id.clone(), report)
            .map(Some)
    }

    fn respond_to_workspace_compromise_report(
        &self,
        workspace_id: WorkspaceId,
        report: WorkspaceCompromiseReport,
    ) -> Result<WorkspaceCompromiseResponse, RuntimeError> {
        let handled_signal_event_ids =
            self.handled_compromise_signal_event_ids_for_workspace(&workspace_id)?;

        let mut already_handled_signal_event_ids = Vec::new();
        let mut responded_signal_event_ids = Vec::new();
        for signal in report.signals.iter().filter(|signal| signal.local_device) {
            if handled_signal_event_ids.contains(&signal.event_id) {
                already_handled_signal_event_ids.push(signal.event_id.clone());
            } else {
                responded_signal_event_ids.push(signal.event_id.clone());
            }
        }

        let mut action_taken = None;
        let mut rotated_local_secret_state = false;
        let mut skipped_reason = None;
        let mut rotation = None;

        if responded_signal_event_ids.is_empty() {
            skipped_reason = if !report.has_signals {
                Some(COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS.to_owned())
            } else if report.local_device_signal_count == 0 {
                Some(COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW.to_owned())
            } else {
                Some(COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED.to_owned())
            };
        } else {
            match self.rotate_workspace_for_suspected_compromise(workspace_id.clone()) {
                Ok(rotated) => {
                    self.record_compromise_response(
                        &workspace_id,
                        responded_signal_event_ids.clone(),
                        rotated.rotated_event_ids.clone(),
                    )?;
                    action_taken = Some(
                        COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE.to_owned(),
                    );
                    rotated_local_secret_state = true;
                    rotation = Some(rotated);
                }
                Err(RuntimeError::InvalidWorkspaceKey) => {
                    responded_signal_event_ids.clear();
                    skipped_reason =
                        Some(COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING.to_owned());
                }
                Err(error) => return Err(error),
            }
        }

        Ok(WorkspaceCompromiseResponse {
            workspace_id: workspace_id.0,
            report,
            action_taken,
            rotated_local_secret_state,
            skipped_reason,
            responded_signal_count: responded_signal_event_ids.len(),
            responded_signal_event_ids,
            already_handled_signal_count: already_handled_signal_event_ids.len(),
            already_handled_signal_event_ids,
            rotation,
        })
    }

    pub fn send_message(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        self.send_message_with_attachments(workspace_id, channel_id, markdown, None, Vec::new())
    }

    pub fn send_message_reply(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        reply_to_message_id: MessageId,
        markdown: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        self.send_message_with_attachments(
            workspace_id,
            channel_id,
            markdown,
            Some(reply_to_message_id),
            Vec::new(),
        )
    }

    pub fn send_message_with_attachment_file(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
        file_path: impl AsRef<Path>,
        media_type: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        self.send_message_with_attachment_file_reply(
            workspace_id,
            channel_id,
            None,
            markdown,
            file_path,
            media_type,
        )
    }

    pub fn send_message_with_attachment_file_reply(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        reply_to_message_id: Option<MessageId>,
        markdown: impl AsRef<str>,
        file_path: impl AsRef<Path>,
        media_type: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        let markdown = markdown.as_ref();
        validate_message_markdown_size(markdown)?;
        let file_path = file_path.as_ref();
        validate_runtime_path(file_path, "attachment file path")?;
        let display_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("attachment")
            .to_owned();
        let attachment = PendingAttachment {
            display_name,
            media_type: attachment_media_type_for_path(file_path, media_type.as_ref()),
            plaintext: read_attachment_file_with_limit(file_path)?,
        };
        self.send_message_with_attachments(
            workspace_id,
            channel_id,
            markdown,
            reply_to_message_id,
            vec![attachment],
        )
    }

    fn send_message_with_attachments(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
        reply_to_message_id: Option<MessageId>,
        pending_attachments: Vec<PendingAttachment>,
    ) -> Result<CreatedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        if let Some(reply_to_message_id) = reply_to_message_id.as_ref() {
            validate_message_id_reference(reply_to_message_id)?;
        }
        let markdown = markdown.as_ref().to_owned();
        validate_message_markdown_size(&markdown)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let content_key =
            self.content_key_for_local_write_in_state(&workspace_id, &channel_id, &context.state)?;
        let message_id = MessageId::new();
        let attachments = self.seal_and_store_attachments(
            &workspace_id,
            &channel_id,
            &message_id,
            &content_key,
            pending_attachments,
        )?;
        let sealed_markdown = seal_message_markdown(
            content_key.key_id(),
            content_key.content_key(),
            &workspace_id,
            &channel_id,
            &message_id,
            &markdown,
        )?;
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            match reply_to_message_id.clone() {
                Some(reply_to_message_id) => EventBody::MessageReplyCreatedEncrypted {
                    message_id: message_id.clone(),
                    reply_to_message_id,
                    sealed_markdown,
                    attachments,
                },
                None => EventBody::MessageCreatedEncrypted {
                    message_id: message_id.clone(),
                    sealed_markdown,
                    attachments,
                },
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.index_message_plaintext(
            &workspace_id,
            &channel_id,
            &message_id,
            &message.event_id,
            message.event.timestamp.physical_ms,
            &markdown,
        );

        Ok(CreatedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reply_to_message_id: reply_to_message_id.map(|message_id| message_id.0),
            event_id: message.event_id.0,
            encrypted: true,
            attachment_count: match &message.event.body {
                EventBody::MessageCreatedEncrypted { attachments, .. }
                | EventBody::MessageReplyCreatedEncrypted { attachments, .. } => attachments.len(),
                _ => 0,
            },
        })
    }

    pub fn save_attachment_to_file(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        attachment_selector: impl AsRef<str>,
        output_path: impl AsRef<Path>,
    ) -> Result<SavedAttachment, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let attachment_selector = attachment_selector.as_ref().to_owned();
        let output_path = output_path.as_ref();
        validate_runtime_path(output_path, "attachment output path")?;
        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let (channel_id, attachment, attachment_index) = Self::message_attachment_from_state(
            &state,
            self.identity.device_id(),
            &workspace_id,
            &message_id,
            &attachment_selector,
        )?;
        let encrypted =
            attachment
                .encryption
                .as_ref()
                .ok_or_else(|| RuntimeError::AttachmentNotEncrypted {
                    blob_hash: attachment.blob_hash.clone(),
                })?;
        let ciphertext = self
            .open_blob_store()?
            .get_complete_bytes(&attachment.blob_hash)?
            .ok_or_else(|| RuntimeError::AttachmentBlobMissing {
                blob_hash: attachment.blob_hash.clone(),
            })?;
        let sealed = sealed_payload_from_encrypted_blob_ref(encrypted, ciphertext);
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        let content_key = self
            .content_key_for_materialized_payload(
                &workspace_id,
                &channel_id,
                &state,
                workspace_key.as_ref(),
                &sealed.key_id,
            )?
            .ok_or_else(|| RuntimeError::ContentKeyMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
                key_id: sealed.key_id.clone(),
            })?;
        let plaintext = open_attachment_blob(
            content_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &message_id,
            attachment_index as u32,
        )?;

        write_attachment_export_file(output_path, &plaintext)?;

        Ok(SavedAttachment {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            blob_hash: attachment.blob_hash,
            attachment_id: attachment.attachment_id,
            display_name: attachment.display_name,
            media_type: attachment.media_type,
            byte_len: attachment.byte_len,
            output_path: output_path.to_string_lossy().into_owned(),
        })
    }

    pub fn prune_unreferenced_blobs(&self) -> Result<PrunedBlobCache, RuntimeError> {
        let workspace_ids = self
            .store
            .list_workspace_ids()?
            .into_iter()
            .map(WorkspaceId)
            .collect::<Vec<_>>();
        let mut referenced_blob_hashes = BTreeSet::new();

        for workspace_id in &workspace_ids {
            let events = self.materialized_workspace_events(workspace_id)?;
            referenced_blob_hashes.extend(attachment_blob_hashes(&events));
        }

        let BlobPruneReport {
            referenced_blob_hashes,
            removed_blob_hashes,
            removed_manifest_hashes,
            removed_chunk_hashes,
            removed_temp_file_paths,
        } = self
            .open_blob_store()?
            .prune_unreferenced(&referenced_blob_hashes)?;

        Ok(PrunedBlobCache::from_parts(
            workspace_ids
                .into_iter()
                .map(|workspace_id| workspace_id.0)
                .collect(),
            referenced_blob_hashes,
            removed_blob_hashes,
            removed_manifest_hashes,
            removed_chunk_hashes,
            removed_temp_file_paths,
        ))
    }

    pub fn edit_message(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        markdown: impl AsRef<str>,
    ) -> Result<EditedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let markdown = markdown.as_ref().to_owned();
        validate_message_markdown_size(&markdown)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let message_view =
            Self::message_view_from_state(&context.state, &workspace_id, &message_id)?;
        let channel_id = message_view.channel_id.clone();
        let indexed_event_id = message_view.author_event_id.clone();
        let indexed_physical_ms = context
            .events
            .iter()
            .find(|event| event.event_id == indexed_event_id)
            .map(|event| event.event.timestamp.physical_ms)
            .unwrap_or_default();
        let content_key =
            self.content_key_for_local_write_in_state(&workspace_id, &channel_id, &context.state)?;
        let sealed_markdown = seal_message_markdown(
            content_key.key_id(),
            content_key.content_key(),
            &workspace_id,
            &channel_id,
            &message_id,
            &markdown,
        )?;
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::MessageEditedEncrypted {
                message_id: message_id.clone(),
                sealed_markdown,
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.index_message_plaintext(
            &workspace_id,
            &channel_id,
            &message_id,
            &indexed_event_id,
            indexed_physical_ms,
            &markdown,
        );

        Ok(EditedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            event_id: message.event_id.0,
            encrypted: true,
        })
    }

    pub fn delete_message(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
    ) -> Result<DeletedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let message_view =
            Self::message_view_from_state(&context.state, &workspace_id, &message_id)?;
        let channel_id = message_view.channel_id.clone();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::MessageDeleted {
                message_id: message_id.clone(),
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.remove_message_from_search(&workspace_id, &message_id);

        Ok(DeletedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            event_id: message.event_id.0,
        })
    }

    pub fn invite_member(
        &self,
        workspace_id: WorkspaceId,
        invitee_device_id: DeviceId,
        role: WorkspaceRole,
    ) -> Result<InvitedMember, RuntimeError> {
        validate_device_id_reference(&invitee_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut invite = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberInvited {
                invitee_device_id: invitee_device_id.clone(),
                role,
            },
        );
        invite.parents = context.head_event_ids.clone();
        let invite = self.sign_authorize_and_append_with_history(invite, &context.events)?;
        let openmls =
            self.auto_add_openmls_workspace_member_if_ready(&workspace_id, &invitee_device_id);

        Ok(InvitedMember {
            workspace_id: workspace_id.0,
            invitee_device_id: invitee_device_id.0,
            role,
            event_id: invite.event_id.0,
            openmls_member_add_event_id: openmls.as_ref().map(|added| added.event_id.clone()),
            openmls_epoch: openmls.as_ref().map(|added| added.epoch),
            openmls_member_count: openmls.as_ref().map(|added| added.member_count),
        })
    }

    pub fn remove_member(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMember, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut removal = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberRemoved {
                removed_device_id: removed_device_id.clone(),
            },
        );
        removal.parents = context.head_event_ids.clone();
        let removal = self.sign_authorize_and_append_with_history(removal, &context.events)?;

        Ok(RemovedMember {
            workspace_id: workspace_id.0,
            removed_device_id: removed_device_id.0,
            event_id: removal.event_id.0,
        })
    }

    pub fn remove_member_with_openmls(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMemberWithOpenMls, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let openmls = self.remove_openmls_workspace_group_member(
            workspace_id.clone(),
            removed_device_id.clone(),
        )?;
        let removal = self.remove_member(workspace_id, removed_device_id)?;

        Ok(RemovedMemberWithOpenMls {
            workspace_id: removal.workspace_id,
            removed_device_id: removal.removed_device_id,
            openmls_event_id: openmls.event_id,
            removal_event_id: removal.event_id,
            openmls_epoch: openmls.epoch,
            openmls_member_count: openmls.member_count,
            openmls_private_group_state_path: openmls.private_group_state_path,
        })
    }

    pub fn remove_member_with_key_rotation(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMemberWithKeyRotation, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let removal = self.remove_member(workspace_id.clone(), removed_device_id)?;
        let key_rotations = self.rotate_workspace_manual_keys(workspace_id)?;

        Ok(RemovedMemberWithKeyRotation {
            workspace_id: removal.workspace_id,
            removed_device_id: removal.removed_device_id,
            removal_event_id: removal.event_id,
            workspace_key_rotation: key_rotations.workspace_key_rotation,
            channel_key_rotation_count: key_rotations.channel_key_rotation_count,
            channel_key_rotations: key_rotations.channel_key_rotations,
        })
    }

    pub fn add_channel_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<AddedChannelMember, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut grant = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelMemberAdded {
                channel_id: channel_id.clone(),
                member_device_id: member_device_id.clone(),
            },
        );
        grant.parents = context.head_event_ids.clone();
        let grant = self.sign_authorize_and_append_with_history(grant, &context.events)?;
        let openmls = self.auto_add_openmls_channel_member_if_ready(
            &workspace_id,
            &channel_id,
            &member_device_id,
        );

        Ok(AddedChannelMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            member_device_id: member_device_id.0,
            event_id: grant.event_id.0,
            openmls_member_add_event_id: openmls.as_ref().map(|added| added.event_id.clone()),
            openmls_epoch: openmls.as_ref().map(|added| added.epoch),
            openmls_member_count: openmls.as_ref().map(|added| added.member_count),
        })
    }

    pub fn remove_channel_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMember, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut removal = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelMemberRemoved {
                channel_id: channel_id.clone(),
                member_device_id: member_device_id.clone(),
            },
        );
        removal.parents = context.head_event_ids.clone();
        let removal = self.sign_authorize_and_append_with_history(removal, &context.events)?;

        Ok(RemovedChannelMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            member_device_id: member_device_id.0,
            event_id: removal.event_id.0,
        })
    }

    pub fn remove_channel_member_with_openmls(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMemberWithOpenMls, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let openmls = self.remove_openmls_channel_group_member(
            workspace_id.clone(),
            channel_id.clone(),
            member_device_id.clone(),
        )?;
        let removal = self.remove_channel_member(workspace_id, channel_id, member_device_id)?;

        Ok(RemovedChannelMemberWithOpenMls {
            workspace_id: removal.workspace_id,
            channel_id: removal.channel_id,
            member_device_id: removal.member_device_id,
            openmls_event_id: openmls.event_id,
            removal_event_id: removal.event_id,
            openmls_epoch: openmls.epoch,
            openmls_member_count: openmls.member_count,
            openmls_private_group_state_path: openmls.private_group_state_path,
        })
    }

    pub fn remove_channel_member_with_key_rotation(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMemberWithKeyRotation, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let removal =
            self.remove_channel_member(workspace_id.clone(), channel_id.clone(), member_device_id)?;
        let channel_key_rotation = self.rotate_channel_key(workspace_id, channel_id)?;

        Ok(RemovedChannelMemberWithKeyRotation {
            workspace_id: removal.workspace_id,
            channel_id: removal.channel_id,
            member_device_id: removal.member_device_id,
            removal_event_id: removal.event_id,
            channel_key_rotation,
        })
    }

    pub fn add_reaction(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        reaction: impl AsRef<str>,
    ) -> Result<AddedReaction, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let reaction = reaction.as_ref().trim().to_owned();
        if reaction.is_empty() {
            return Err(RuntimeError::ReactionRequired);
        }
        validate_metadata_field_size("reaction", &reaction, REACTION_TEXT_MAX_BYTES)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let channel_id =
            Self::message_channel_id_from_state(&context.state, &workspace_id, &message_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: reaction.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(AddedReaction {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reaction,
            event_id: event.event_id.0,
        })
    }

    pub fn remove_reaction(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        reaction: impl AsRef<str>,
    ) -> Result<RemovedReaction, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let reaction = reaction.as_ref().trim().to_owned();
        if reaction.is_empty() {
            return Err(RuntimeError::ReactionRequired);
        }
        validate_metadata_field_size("reaction", &reaction, REACTION_TEXT_MAX_BYTES)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let channel_id =
            Self::message_channel_id_from_state(&context.state, &workspace_id, &message_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReactionRemoved {
                message_id: message_id.clone(),
                reaction: reaction.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(RemovedReaction {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reaction,
            event_id: event.event_id.0,
        })
    }

    pub fn mark_channel_read(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<MarkedChannelRead, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let read_through_event_id =
            Self::latest_channel_read_event_id_from_context(&context, &workspace_id, &channel_id)?;
        if Self::channel_is_read_through_in_state(
            &context.state,
            self.identity.device_id(),
            &channel_id,
            &read_through_event_id,
        ) {
            return Ok(MarkedChannelRead {
                workspace_id: workspace_id.0,
                channel_id: channel_id.0,
                read_through_event_id: read_through_event_id.0,
                marker_event_id: None,
                already_read: true,
            });
        }

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: read_through_event_id.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(MarkedChannelRead {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            read_through_event_id: read_through_event_id.0,
            marker_event_id: Some(event.event_id.0),
            already_read: false,
        })
    }

    pub fn workspace_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.workspace_snapshot_with_options(workspace_id, &WorkspaceSnapshotOptions::full())
    }

    pub fn workspace_snapshot_with_options(
        &self,
        workspace_id: WorkspaceId,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        if let Some(channel_id) = options.timeline_channel_id.as_ref() {
            validate_channel_id_reference(channel_id)?;
        }
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let mut snapshot = WorkspaceSnapshot::from_events_for_device_with_options(
            workspace_id,
            &events,
            self.identity.device_id(),
            options,
        )?;
        self.annotate_attachment_availability(&mut snapshot)?;
        Ok(snapshot)
    }

    pub fn decrypted_workspace_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::full(),
        )
    }

    pub fn decrypted_workspace_snapshot_with_options(
        &self,
        workspace_id: WorkspaceId,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        if let Some(channel_id) = options.timeline_channel_id.as_ref() {
            validate_channel_id_reference(channel_id)?;
        }
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        self.validate_snapshot_channel_scope(&workspace_id, &state, options)?;
        let body_override_event_ids = body_override_event_ids_for_snapshot_window(
            &state,
            &report,
            &raw_events,
            self.identity.device_id(),
            options,
        );
        let body_overrides = self.decrypted_body_overrides_for_event_ids(
            &workspace_id,
            &state,
            workspace_key.as_ref(),
            &body_override_event_ids,
        )?;

        let mut snapshot =
            WorkspaceSnapshot::from_state_report_for_device_and_body_overrides_with_options(
                workspace_id,
                &state,
                &report,
                &raw_events,
                self.identity.device_id(),
                &body_overrides,
                options,
            );
        self.annotate_attachment_availability(&mut snapshot)?;
        Ok(snapshot)
    }

    pub fn decrypted_workspace_channel_snapshot_latest(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        timeline_limit: usize,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::latest_for_channel(channel_id, timeline_limit),
        )
    }

    pub fn decrypted_workspace_channel_snapshot_window(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        timeline_start: usize,
        timeline_limit: usize,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::window_for_channel(
                channel_id,
                timeline_start,
                timeline_limit,
            ),
        )
    }

    fn validate_snapshot_channel_scope(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<(), RuntimeError> {
        let Some(channel_id) = options.timeline_channel_id.as_ref() else {
            return Ok(());
        };
        validate_channel_id_reference(channel_id)?;
        if !state.channels.contains_key(channel_id) {
            return Err(RuntimeError::ChannelNotFound {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }
        if !state.channel_accessible_to(channel_id, self.identity.device_id()) {
            return Err(RuntimeError::ChannelAccessDenied {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
                device_id: self.identity.device_id().clone(),
            });
        }
        Ok(())
    }

    fn decrypted_body_overrides_for_event_ids(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        workspace_key: Option<&WorkspaceKey>,
        body_override_event_ids: &BTreeSet<EventId>,
    ) -> Result<HashMap<String, String>, RuntimeError> {
        let mut body_overrides = HashMap::new();
        for message in state.messages.values() {
            if !body_override_event_ids.contains(&message.author_event_id) {
                continue;
            }
            if !state.channel_accessible_to(&message.channel_id, self.identity.device_id()) {
                continue;
            }
            if let Some(sealed_markdown) = message.sealed_markdown.as_ref() {
                let Some(content_key) = self.content_key_for_materialized_payload(
                    workspace_id,
                    &message.channel_id,
                    state,
                    workspace_key,
                    &sealed_markdown.key_id,
                )?
                else {
                    continue;
                };
                let plaintext = open_message_markdown(
                    content_key.content_key(),
                    sealed_markdown,
                    workspace_id,
                    &message.channel_id,
                    &message.message_id,
                )?;
                body_overrides.insert(message.author_event_id.0.clone(), plaintext);
            }
        }
        Ok(body_overrides)
    }

    fn channel_page_body_override_event_ids(
        state: &WorkspaceState,
        page: &WorkspaceChannelPage,
    ) -> BTreeSet<EventId> {
        Self::channel_rows_body_override_event_ids(state, &page.channels)
    }

    fn channel_rows_body_override_event_ids(
        state: &WorkspaceState,
        channels: &[chaft_app::ChannelSnapshot],
    ) -> BTreeSet<EventId> {
        let mut event_ids = BTreeSet::new();
        for channel in channels {
            let Some(activity) = channel.latest_activity.as_ref() else {
                continue;
            };
            event_ids.insert(EventId(activity.event_id.clone()));
            let Some(message_id) = activity.message_id.as_ref() else {
                continue;
            };
            let Some(message) = state.messages.get(&MessageId(message_id.clone())) else {
                continue;
            };
            if !message.deleted && message.sealed_markdown.is_some() {
                event_ids.insert(message.author_event_id.clone());
            }
        }
        event_ids
    }

    pub fn workspace_events(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<SignedEvent>, RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        Ok(self.store.list_events_for_workspace(&workspace_id.0)?)
    }

    pub fn workspace_storage_health(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceStorageHealth, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        Ok(self
            .store
            .workspace_event_storage_health(&workspace_id.0)?
            .into())
    }

    pub fn repair_workspace_storage_metadata(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceStorageRepair, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        Ok(self
            .store
            .repair_workspace_event_storage_metadata(&workspace_id.0)?
            .into())
    }

    pub fn export_workspace_key(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceKeyExport, RuntimeError> {
        let workspace_key = self
            .load_workspace_key(&workspace_id)?
            .ok_or_else(|| RuntimeError::InvalidWorkspaceKey)?;
        Ok(WorkspaceKeyExport {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: workspace_key.workspace_id.0.clone(),
            epoch: workspace_key.epoch,
            key_id: workspace_key.key_id.clone(),
            exporter_device_id: self.identity.device_id().0.clone(),
            aes_256_gcm_siv_key: workspace_key.content_key.as_bytes().to_vec(),
            previous_keys: workspace_key.exported_previous_keys(),
        })
    }

    pub fn import_workspace_key(
        &self,
        exported: WorkspaceKeyExport,
    ) -> Result<ImportedWorkspaceKey, RuntimeError> {
        let workspace_key = WorkspaceKey::from_export(exported)?;
        let imported = ImportedWorkspaceKey {
            workspace_id: workspace_key.workspace_id.0.clone(),
            key_id: workspace_key.key_id.clone(),
            importer_device_id: self.identity.device_id().0.clone(),
        };
        self.save_workspace_key(&workspace_key)?;
        let _ = self.reindex_workspace_search(workspace_key.workspace_id.clone());
        Ok(imported)
    }

    pub fn export_channel_key(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<ChannelKeyExport, RuntimeError> {
        let channel_key = self
            .load_channel_key(&workspace_id, &channel_id)?
            .ok_or_else(|| RuntimeError::InvalidChannelKey)?;
        Ok(ChannelKeyExport {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: channel_key.workspace_id.0.clone(),
            channel_id: channel_key.channel_id.0.clone(),
            epoch: channel_key.epoch,
            key_id: channel_key.key_id.clone(),
            exporter_device_id: self.identity.device_id().0.clone(),
            aes_256_gcm_siv_key: channel_key.content_key.as_bytes().to_vec(),
            previous_keys: channel_key.exported_previous_keys(),
        })
    }

    pub fn import_channel_key(
        &self,
        exported: ChannelKeyExport,
    ) -> Result<ImportedChannelKey, RuntimeError> {
        let channel_key = ChannelKey::from_export(exported)?;
        let imported = ImportedChannelKey {
            workspace_id: channel_key.workspace_id.0.clone(),
            channel_id: channel_key.channel_id.0.clone(),
            key_id: channel_key.key_id.clone(),
            importer_device_id: self.identity.device_id().0.clone(),
        };
        self.save_channel_key(&channel_key)?;
        let _ = self.reindex_workspace_search(channel_key.workspace_id.clone());
        Ok(imported)
    }

    pub fn export_workspace_recovery_bundle(
        &self,
        workspace_id: WorkspaceId,
        passphrase: &str,
    ) -> Result<WorkspaceRecoveryBundle, RuntimeError> {
        if passphrase.trim().is_empty() {
            return Err(RuntimeError::RecoveryBundlePassphraseRequired);
        }

        let workspace_key = self.export_workspace_key(workspace_id.clone())?;
        let channel_keys = self
            .local_private_channel_key_ids(&workspace_id)?
            .into_iter()
            .map(|channel_id| self.export_channel_key(workspace_id.clone(), channel_id))
            .collect::<Result<Vec<_>, _>>()?;
        let plaintext = WorkspaceRecoveryBundlePlaintext {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_key,
            channel_keys,
        };
        let plaintext = serde_json::to_vec(&plaintext)?;
        let mut salt = vec![0; RECOVERY_BUNDLE_SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let kdf = WorkspaceRecoveryBundleKdf {
            name: RECOVERY_BUNDLE_KDF_ARGON2ID.to_owned(),
            context: RECOVERY_BUNDLE_KDF_CONTEXT.to_owned(),
            salt,
            memory_cost_kib: RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB,
            time_cost: RECOVERY_BUNDLE_ARGON2_TIME_COST,
            parallelism: RECOVERY_BUNDLE_ARGON2_PARALLELISM,
            output_len: RECOVERY_BUNDLE_KDF_OUTPUT_LEN,
        };
        let wrapping_key = derive_recovery_bundle_key(passphrase, &kdf)?;
        let sealed_payload = seal_aes_256_gcm_siv(
            recovery_bundle_key_id(&workspace_id),
            &wrapping_key,
            &plaintext,
            &recovery_bundle_aad(
                &workspace_id,
                self.identity.device_id(),
                kdf.name.as_str(),
                kdf.context.as_str(),
                &kdf.salt,
            ),
        )?;

        Ok(WorkspaceRecoveryBundle {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_id: workspace_id.0,
            exporter_device_id: self.identity.device_id().0.clone(),
            kdf,
            sealed_payload,
        })
    }

    pub fn import_workspace_recovery_bundle(
        &self,
        bundle: WorkspaceRecoveryBundle,
        passphrase: &str,
    ) -> Result<ImportedWorkspaceRecoveryBundle, RuntimeError> {
        if passphrase.trim().is_empty() {
            return Err(RuntimeError::RecoveryBundlePassphraseRequired);
        }
        if bundle.schema_version != RECOVERY_BUNDLE_SCHEMA_VERSION {
            return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
        }

        let workspace_id = WorkspaceId(bundle.workspace_id.clone());
        let exporter_device_id = DeviceId(bundle.exporter_device_id.clone());
        let wrapping_key = derive_recovery_bundle_key(passphrase, &bundle.kdf)?;
        let aad = recovery_bundle_aad(
            &workspace_id,
            &exporter_device_id,
            bundle.kdf.name.as_str(),
            bundle.kdf.context.as_str(),
            &bundle.kdf.salt,
        );
        if bundle.sealed_payload.aad != aad {
            return Err(RuntimeError::InvalidWorkspaceRecoveryBundle);
        }
        let plaintext = open_aes_256_gcm_siv(&wrapping_key, &bundle.sealed_payload)?;
        let plaintext = serde_json::from_slice::<WorkspaceRecoveryBundlePlaintext>(&plaintext)?;
        if plaintext.schema_version != RECOVERY_BUNDLE_SCHEMA_VERSION
            || plaintext.workspace_key.workspace_id != bundle.workspace_id
            || plaintext
                .channel_keys
                .iter()
                .any(|channel_key| channel_key.workspace_id != bundle.workspace_id)
        {
            return Err(RuntimeError::InvalidWorkspaceRecoveryBundle);
        }

        let workspace_key = WorkspaceKey::from_export(plaintext.workspace_key)?;
        let channel_keys = plaintext
            .channel_keys
            .into_iter()
            .map(ChannelKey::from_export)
            .collect::<Result<Vec<_>, _>>()?;
        let imported = ImportedWorkspaceRecoveryBundle {
            workspace_id: workspace_key.workspace_id.0.clone(),
            workspace_key_id: workspace_key.key_id.clone(),
            imported_channel_count: channel_keys.len(),
            imported_channel_ids: channel_keys
                .iter()
                .map(|channel_key| channel_key.channel_id.0.clone())
                .collect(),
            importer_device_id: self.identity.device_id().0.clone(),
        };

        self.save_workspace_key(&workspace_key)?;
        for channel_key in channel_keys {
            self.save_channel_key(&channel_key)?;
        }
        let _ = self.reindex_workspace_search(WorkspaceId(imported.workspace_id.clone()));
        Ok(imported)
    }

    pub fn export_trust_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let events = self.materialized_workspace_events(&workspace_id)?;
        self.sign_trust_snapshot_from_materialized_events(workspace_id, &events)
    }

    fn sign_trust_snapshot_from_materialized_events(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) = trust_snapshot_from_events(workspace_id, events)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }

    fn sign_trust_snapshot_for_materialized_event(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        event: &SignedEvent,
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) =
            trust_snapshot_for_event_from_events(workspace_id, events, event)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }

    fn sign_trust_snapshot_for_materialized_event_slice(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        target_events: &[SignedEvent],
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) =
            trust_snapshot_for_events_from_events(workspace_id, events, target_events)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }

    pub fn reindex_workspace_search(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<IndexedWorkspaceSearch, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        self.reindex_workspace_search_with_key(&workspace_id, workspace_key.as_ref())
    }

    pub fn search_workspace_messages(
        &self,
        workspace_id: WorkspaceId,
        query: impl AsRef<str>,
    ) -> Result<SearchedWorkspace, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let query = query.as_ref().trim().to_owned();
        validate_search_query_size(&query)?;
        if !query_has_search_terms(&query) {
            return Ok(SearchedWorkspace::empty(workspace_id, query));
        }
        let mut raw_hits = self.open_search_index()?.search_limited(
            &workspace_id,
            &query,
            LOCAL_SEARCH_RAW_HIT_LIMIT.saturating_add(1),
        )?;
        if raw_hits.is_empty() {
            return Ok(SearchedWorkspace::empty(workspace_id, query));
        }
        let has_more_raw_candidates = raw_hits.len() > LOCAL_SEARCH_RAW_HIT_LIMIT;
        raw_hits.truncate(LOCAL_SEARCH_RAW_HIT_LIMIT);
        let raw_candidate_count = raw_hits.len();
        let raw_hit_event_ids = raw_hits
            .iter()
            .map(|hit| hit.event_id.clone())
            .collect::<Vec<_>>();
        let servable_event_ids = self
            .store
            .filter_servable_event_ids_for_workspace(&workspace_id.0, &raw_hit_event_ids)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        raw_hits.retain(|hit| servable_event_ids.contains(&hit.event_id));
        if raw_hits.is_empty() {
            return Ok(SearchedWorkspace::bounded(
                workspace_id,
                query,
                Vec::new(),
                raw_candidate_count,
                has_more_raw_candidates,
            ));
        }

        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        let applied_event_ids = report.applied_events.into_iter().collect::<BTreeSet<_>>();
        let event_author_and_physical_ms_by_id = events
            .iter()
            .map(|event| {
                (
                    event.event_id.clone(),
                    (
                        event.event.author_device_id.clone(),
                        event.event.timestamp.physical_ms,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut hits: Vec<_> = raw_hits
            .into_iter()
            .filter(|hit| {
                applied_event_ids.contains(&hit.event_id)
                    && state.channel_accessible_to(&hit.channel_id, self.identity.device_id())
            })
            .filter_map(|hit| {
                let (author_device_id, physical_ms) =
                    event_author_and_physical_ms_by_id.get(&hit.event_id)?;
                let channel = state.channels.get(&hit.channel_id)?;
                let author_display_name = state
                    .profiles
                    .get(author_device_id)
                    .map(|profile| profile.display_name.clone());
                Some(WorkspaceSearchHit {
                    workspace_id: workspace_id.0.clone(),
                    event_id: hit.event_id.0,
                    message_id: hit.message_id.0,
                    channel_id: hit.channel_id.0,
                    channel_name: channel.name.clone(),
                    channel_is_private: channel.is_private,
                    author_device_id: author_device_id.0.clone(),
                    author_display_name,
                    physical_ms: *physical_ms,
                    body: hit.markdown,
                    body_char_count: hit.markdown_char_count,
                    body_truncated: hit.markdown_truncated,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .physical_ms
                .cmp(&left.physical_ms)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.message_id.cmp(&right.message_id))
        });
        Ok(SearchedWorkspace::bounded(
            workspace_id,
            query,
            hits,
            raw_candidate_count,
            has_more_raw_candidates,
        ))
    }

    pub fn workspace_publish_queue(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspacePublishQueue, RuntimeError> {
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let mut publishable_event_ids = events
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        let mut backup_event_ids = events
            .iter()
            .filter(|event| is_backup_slice_event(event))
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        let backup_event_id_set = backup_event_ids.iter().cloned().collect::<BTreeSet<_>>();
        let blob_store = self.open_blob_store()?;
        let mut available_blob_hashes = Vec::new();
        let mut missing_blob_hashes = Vec::new();
        for blob_hash in attachment_blob_hashes(&events) {
            if blob_store.has_complete_blob(&blob_hash)? {
                available_blob_hashes.push(blob_hash);
            } else {
                missing_blob_hashes.push(blob_hash);
            }
        }
        let summary = workspace_publish_queue_summary(
            &events,
            &backup_event_id_set,
            &available_blob_hashes,
            &missing_blob_hashes,
            &skipped_gaps,
        );
        publishable_event_ids.truncate(MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS);
        backup_event_ids.truncate(MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS);
        available_blob_hashes.truncate(MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS);
        missing_blob_hashes.truncate(MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS);
        let mut skipped_gaps = skipped_gaps;
        skipped_gaps.truncate(MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS);

        Ok(WorkspacePublishQueue {
            workspace_id: workspace_id.0,
            summary,
            publishable_event_ids,
            backup_event_ids,
            available_blob_hashes,
            missing_blob_hashes,
            skipped_gaps,
        })
    }

    pub async fn publish_workspace_to_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let mut published_event_ids = Vec::with_capacity(events.len());
        for event in events {
            let event_id = event.event_id.0.clone();
            transport.publish_event(peer, event).await?;
            published_event_ids.push(event_id);
        }

        Ok(PublishedWorkspace::from_parts(
            workspace_id.0,
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        ))
    }

    pub async fn publish_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        validate_remote_inventory_event_ids(&remote_event_ids)?;
        let remote_event_ids = remote_event_ids.into_iter().collect::<BTreeSet<_>>();
        let events_to_publish = events
            .iter()
            .filter(|event| !remote_event_ids.contains(&event.event_id))
            .cloned()
            .collect::<Vec<_>>();
        let published_event_ids = events_to_publish
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        transport
            .publish_events_with_authorization(peer, events_to_publish, Vec::new(), Vec::new())
            .await?;

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0.clone(),
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(transport, peer, &events, &mut published)
            .await?;
        published.refresh_counts();

        Ok(published)
    }

    pub async fn publish_event_direct_with_trust_snapshot<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
        event_id: EventId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_event_id_reference(&event_id)?;
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let event = events
            .iter()
            .find(|event| event.event_id == event_id)
            .cloned()
            .ok_or_else(|| RuntimeError::EventNotFound {
                workspace_id: workspace_id.clone(),
                event_id: event_id.clone(),
            })?;
        let trust_snapshot =
            self.sign_trust_snapshot_for_materialized_event(workspace_id.clone(), &events, &event)?;
        transport
            .publish_events_with_authorization(
                peer,
                vec![event.clone()],
                Vec::new(),
                vec![trust_snapshot],
            )
            .await?;

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0,
            vec![event.event_id.0.clone()],
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(transport, peer, &[event], &mut published)
            .await?;
        published.refresh_counts();
        Ok(published)
    }

    pub async fn backup_workspace_direct_with_trust_snapshot<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        validate_remote_inventory_event_ids(&remote_event_ids)?;
        let remote_event_ids = remote_event_ids.into_iter().collect::<BTreeSet<_>>();
        let backup_events = events
            .iter()
            .filter(|event| is_backup_slice_event(event))
            .cloned()
            .collect::<Vec<_>>();
        let events_to_publish = backup_events
            .iter()
            .filter(|event| !remote_event_ids.contains(&event.event_id))
            .cloned()
            .collect::<Vec<_>>();
        let published_event_ids = events_to_publish
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();

        for event_chunk in events_to_publish.chunks(MAX_PUBLISH_EVENTS_PER_REQUEST) {
            let trust_snapshot = self.sign_trust_snapshot_for_materialized_event_slice(
                workspace_id.clone(),
                &events,
                event_chunk,
            )?;
            transport
                .publish_events_with_authorization(
                    peer,
                    event_chunk.to_vec(),
                    Vec::new(),
                    vec![trust_snapshot],
                )
                .await?;
        }

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0,
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(
            transport,
            peer,
            &backup_events,
            &mut published,
        )
        .await?;
        published.refresh_counts();
        Ok(published)
    }

    pub async fn retry_pending_blob_transfers_direct<T>(
        &self,
        transport: &T,
        workspace_id: WorkspaceId,
        peers: &[PeerAddress],
    ) -> Result<BlobTransferRetryReport, RuntimeError>
    where
        T: BlobSyncTransport,
    {
        validate_peer_addresses(peers)?;
        let materialized_blob_hashes =
            attachment_blob_hashes(&self.materialized_workspace_events(&workspace_id)?)
                .into_iter()
                .collect::<BTreeSet<_>>();
        let ledger = self.read_blob_transfer_ledger()?;
        let pending_entries = ledger
            .entries
            .into_iter()
            .filter(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.status != BlobTransferStatus::Succeeded
            })
            .collect::<Vec<_>>();
        let pending_attempt_ids = pending_entries
            .iter()
            .map(|entry| entry.attempt_id.clone())
            .collect::<Vec<_>>();
        let blob_store = self.open_blob_store()?;
        let mut report = BlobTransferRetryReport {
            workspace_id: workspace_id.0.clone(),
            pending_attempt_count: 0,
            pending_attempt_ids,
            retried_blob_count: 0,
            retried_blob_hashes: Vec::new(),
            reconciled_blob_count: 0,
            reconciled_blob_hashes: Vec::new(),
            missing_blob_count: 0,
            missing_blob_hashes: Vec::new(),
            skipped_blob_count: 0,
            skipped_blob_hashes: Vec::new(),
            peer_error_count: 0,
            peer_errors: Vec::new(),
            blob_transfer_attempt_count: 0,
            blob_transfer_attempts: Vec::new(),
        };
        let mut retried = BTreeSet::new();
        let mut reconciled = BTreeSet::new();
        let mut missing = BTreeSet::new();
        let mut skipped = BTreeSet::new();
        let mut processed = BTreeSet::new();
        let retry_peers = ordered_retry_peers(peers);

        for pending in pending_entries {
            if !processed.insert(pending.blob_hash.clone()) {
                continue;
            }
            if reconciled.contains(&pending.blob_hash) {
                continue;
            }
            if !materialized_blob_hashes.contains(&pending.blob_hash) {
                if skipped.insert(pending.blob_hash.clone()) {
                    report.skipped_blob_hashes.push(pending.blob_hash.clone());
                }
                continue;
            }
            let Some(bytes) = blob_store.get_complete_bytes(&pending.blob_hash)? else {
                if missing.insert(pending.blob_hash.clone()) {
                    report.missing_blob_hashes.push(pending.blob_hash.clone());
                }
                continue;
            };

            for &peer in &retry_peers {
                let remote_blob_availability = match transport
                    .fetch_blob_availabilities(peer, vec![pending.blob_hash.clone()])
                    .await
                {
                    Ok(availability) => availability,
                    Err(error) => {
                        let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                        report.peer_errors.push(blob_transfer_peer_error(
                            &peer.peer_id.0,
                            &peer.endpoint,
                            &pending.blob_hash,
                            error.to_string(),
                            suspect_protocol_error,
                        ));
                        continue;
                    }
                };
                if remote_blob_availability
                    .get(&pending.blob_hash)
                    .is_some_and(|availability| availability.is_complete())
                {
                    let reconciled_attempts = self.reconcile_satisfied_blob_transfer_attempts(
                        &workspace_id.0,
                        &pending.blob_hash,
                    )?;
                    if !reconciled_attempts.is_empty()
                        && reconciled.insert(pending.blob_hash.clone())
                    {
                        report
                            .reconciled_blob_hashes
                            .push(pending.blob_hash.clone());
                    }
                    report.blob_transfer_attempts.extend(reconciled_attempts);
                    break;
                }

                let (upload, suspect_protocol_error) = self
                    .retry_blob_transfer_to_peer(
                        transport,
                        peer,
                        &workspace_id.0,
                        &pending.blob_hash,
                        bytes.clone(),
                        remote_blob_availability.get(&pending.blob_hash),
                    )
                    .await?;
                if upload.status == BlobTransferStatus::Succeeded {
                    let upload_blob_hash = upload.blob_hash.clone();
                    if retried.insert(upload_blob_hash.clone()) {
                        report.retried_blob_hashes.push(upload_blob_hash.clone());
                    }
                    report.blob_transfer_attempts.push(upload);
                    let reconciled_attempts = self.reconcile_satisfied_blob_transfer_attempts(
                        &workspace_id.0,
                        &upload_blob_hash,
                    )?;
                    if !reconciled_attempts.is_empty()
                        && reconciled.insert(upload_blob_hash.clone())
                    {
                        report.reconciled_blob_hashes.push(upload_blob_hash);
                    }
                    report.blob_transfer_attempts.extend(reconciled_attempts);
                    break;
                }
                if let Some(message) = upload.error.clone() {
                    report.peer_errors.push(blob_transfer_peer_error(
                        &upload.peer_id,
                        &upload.peer_endpoint,
                        &upload.blob_hash,
                        message,
                        suspect_protocol_error,
                    ));
                }
                report.blob_transfer_attempts.push(upload);
            }
        }

        report.refresh_counts();
        Ok(report)
    }

    pub async fn pull_workspace_from_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PulledWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let report =
            pull_workspace_from_peer(transport, peer, &self.store, workspace_id.clone()).await?;
        let openmls_catchup = self.apply_local_openmls_catchup(&workspace_id)?;
        let compromise_response = self.automatic_compromise_response_if_needed(&workspace_id)?;
        let _ = self.reindex_workspace_search_if_key_available(&workspace_id);
        let mut pulled = Self::pulled_workspace_from_report(workspace_id, report);
        pulled.openmls_catchup = openmls_catchup;
        pulled.compromise_response = compromise_response;
        Ok(pulled)
    }

    pub async fn pull_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PulledWorkspace, RuntimeError>
    where
        T: ChaftTransport + BlobSyncTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        let report = pull_workspace_from_peer_with_inventory(
            transport,
            peer,
            &self.store,
            workspace_id.clone(),
            remote_event_ids,
        )
        .await?;
        let openmls_catchup = self.apply_local_openmls_catchup(&workspace_id)?;
        let compromise_response = self.automatic_compromise_response_if_needed(&workspace_id)?;
        let _ = self.reindex_workspace_search_if_key_available(&workspace_id);
        let mut pulled = Self::pulled_workspace_from_report(workspace_id.clone(), report);
        pulled.openmls_catchup = openmls_catchup;
        pulled.compromise_response = compromise_response;
        let events = self.materialized_workspace_events(&workspace_id)?;
        let blob_hashes = attachment_blob_hashes(&events);
        let blob_store = self.open_blob_store()?;
        let mut missing_local_blob_hashes = Vec::new();

        for blob_hash in blob_hashes {
            if blob_store.has_complete_blob(&blob_hash)? {
                continue;
            }
            missing_local_blob_hashes.push(blob_hash);
        }

        let fetched_blobs = transport
            .fetch_blobs(peer, missing_local_blob_hashes.clone())
            .await?;
        for blob_hash in missing_local_blob_hashes {
            match fetched_blobs.get(&blob_hash) {
                Some(bytes) => {
                    blob_store.put_bytes_with_hash(&blob_hash, bytes)?;
                    pulled.fetched_blob_hashes.push(blob_hash);
                }
                None => match transport.fetch_blob_chunked(peer, &blob_hash).await? {
                    Some(bytes) => {
                        blob_store.put_bytes_with_hash(&blob_hash, &bytes)?;
                        pulled.fetched_blob_hashes.push(blob_hash);
                    }
                    None => pulled.missing_blob_hashes.push(blob_hash),
                },
            }
        }
        pulled.refresh_counts();

        Ok(pulled)
    }

    fn pulled_workspace_from_report(
        workspace_id: WorkspaceId,
        report: PullSyncReport,
    ) -> PulledWorkspace {
        let mut pulled = PulledWorkspace {
            workspace_id: workspace_id.0,
            requested_event_count: 0,
            requested_event_ids: report
                .requested_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            fetched_event_count: 0,
            fetched_event_ids: report
                .fetched_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            fetched_blob_count: 0,
            fetched_blob_hashes: Vec::new(),
            missing_blob_count: 0,
            missing_blob_hashes: Vec::new(),
            ignored_event_count: 0,
            ignored_event_ids: report
                .ignored_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            applied_event_count: 0,
            applied_event_ids: report
                .materialization
                .applied_events
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            openmls_catchup: PulledOpenMlsCatchup::default(),
            compromise_response: None,
            gap_count: 0,
            gaps: report
                .materialization
                .gaps
                .into_iter()
                .map(|gap| PulledWorkspaceGap {
                    event_id: gap.event_id.0,
                    missing_parent_ids: gap
                        .missing_parent_ids
                        .into_iter()
                        .map(|event_id| event_id.0)
                        .collect(),
                })
                .collect(),
        };
        pulled.refresh_counts();
        pulled
    }

    fn apply_local_openmls_catchup(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<PulledOpenMlsCatchup, RuntimeError> {
        let mut catchup = PulledOpenMlsCatchup::default();

        if !self.openmls_workspace_group_path(workspace_id).exists() {
            match self.join_openmls_workspace_group(workspace_id.clone(), None) {
                Ok(joined) => catchup.workspace_joined_event_id = Some(joined.source_event_id),
                Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { .. })
                | Err(RuntimeError::OpenMlsWorkspaceGroupInviteNotFound { .. })
                | Err(RuntimeError::OpenMlsPrivateKeyPackageMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        if self.openmls_workspace_group_path(workspace_id).exists() {
            match self.apply_openmls_workspace_group_commits(workspace_id.clone(), None) {
                Ok(applied) => {
                    catchup.workspace_applied_event_ids = applied.applied_event_ids;
                    catchup.workspace_self_removed = applied.self_removed;
                }
                Err(RuntimeError::OpenMlsWorkspaceGroupMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        for channel_id in self.joinable_openmls_channel_group_ids(workspace_id)? {
            match self.join_openmls_channel_group(workspace_id.clone(), channel_id.clone(), None) {
                Ok(joined) => {
                    catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                        channel_id: channel_id.0,
                        event_count: 0,
                        joined_event_id: Some(joined.source_event_id),
                        applied_event_ids: Vec::new(),
                        provisioned_event_ids: Vec::new(),
                        self_removed: false,
                    });
                }
                Err(RuntimeError::OpenMlsChannelGroupAlreadyExists { .. })
                | Err(RuntimeError::OpenMlsChannelGroupInviteNotFound { .. })
                | Err(RuntimeError::OpenMlsPrivateKeyPackageMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        for channel_id in self.local_openmls_channel_group_ids(workspace_id)? {
            match self.apply_openmls_channel_group_commits(
                workspace_id.clone(),
                channel_id.clone(),
                None,
            ) {
                Ok(applied) => {
                    let channel_id_string = channel_id.0;
                    let Some(existing) = catchup
                        .channel_groups
                        .iter_mut()
                        .find(|group| group.channel_id == channel_id_string)
                    else {
                        if applied.applied_event_ids.is_empty() && !applied.self_removed {
                            continue;
                        }
                        catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                            channel_id: channel_id_string,
                            event_count: 0,
                            joined_event_id: None,
                            applied_event_ids: applied.applied_event_ids,
                            provisioned_event_ids: Vec::new(),
                            self_removed: applied.self_removed,
                        });
                        continue;
                    };
                    existing.applied_event_ids = applied.applied_event_ids;
                    existing.self_removed |= applied.self_removed;
                }
                Err(RuntimeError::OpenMlsChannelGroupMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        catchup.workspace_provisioned_event_ids =
            self.auto_provision_openmls_workspace_members(workspace_id);
        for provisioned in self.auto_provision_openmls_channel_members(workspace_id) {
            let Some(existing) = catchup
                .channel_groups
                .iter_mut()
                .find(|group| group.channel_id == provisioned.channel_id)
            else {
                catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                    channel_id: provisioned.channel_id,
                    event_count: 0,
                    joined_event_id: None,
                    applied_event_ids: Vec::new(),
                    provisioned_event_ids: provisioned.event_ids,
                    self_removed: false,
                });
                continue;
            };
            existing.provisioned_event_ids.extend(provisioned.event_ids);
        }

        catchup.refresh_counts();
        Ok(catchup)
    }

    fn auto_add_openmls_workspace_member_if_ready(
        &self,
        workspace_id: &WorkspaceId,
        device_id: &DeviceId,
    ) -> Option<AddedOpenMlsWorkspaceGroupMember> {
        let events = self.materialized_workspace_events(workspace_id).ok()?;
        let mut index = OpenMlsAutoProvisionIndex::from_events(&events);
        self.auto_add_openmls_workspace_member_if_ready_with_index(
            workspace_id,
            device_id,
            &mut index,
        )
    }

    fn auto_add_openmls_workspace_member_if_ready_with_index(
        &self,
        workspace_id: &WorkspaceId,
        device_id: &DeviceId,
        index: &mut OpenMlsAutoProvisionIndex,
    ) -> Option<AddedOpenMlsWorkspaceGroupMember> {
        if device_id == self.identity.device_id()
            || !self.openmls_workspace_group_path(workspace_id).exists()
            || index.workspace_group_has_device(device_id)
        {
            return None;
        }

        let key_package_id = index.latest_unused_key_package_id_for_device(device_id)?;
        let added = self
            .add_openmls_workspace_group_member(workspace_id.clone(), key_package_id)
            .ok()?;
        index.mark_workspace_group_member_added(
            &added.invitee_device_id,
            &added.invitee_key_package_id,
        );
        Some(added)
    }

    fn auto_add_openmls_channel_member_if_ready(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> Option<AddedOpenMlsChannelGroupMember> {
        let events = self.materialized_workspace_events(workspace_id).ok()?;
        let mut index = OpenMlsAutoProvisionIndex::from_events(&events);
        self.auto_add_openmls_channel_member_if_ready_with_index(
            workspace_id,
            channel_id,
            device_id,
            &mut index,
        )
    }

    fn auto_add_openmls_channel_member_if_ready_with_index(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        device_id: &DeviceId,
        index: &mut OpenMlsAutoProvisionIndex,
    ) -> Option<AddedOpenMlsChannelGroupMember> {
        if device_id == self.identity.device_id()
            || !self
                .openmls_channel_group_path(workspace_id, channel_id)
                .exists()
            || index.channel_group_has_device(channel_id, device_id)
        {
            return None;
        }

        let key_package_id = index.latest_unused_key_package_id_for_device(device_id)?;
        let added = self
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                channel_id.clone(),
                key_package_id,
            )
            .ok()?;
        index.mark_channel_group_member_added(
            &added.channel_id,
            &added.invitee_device_id,
            &added.invitee_key_package_id,
        );
        Some(added)
    }

    fn auto_provision_openmls_workspace_members(&self, workspace_id: &WorkspaceId) -> Vec<String> {
        if !self.openmls_workspace_group_path(workspace_id).exists() {
            return Vec::new();
        }

        let Ok(events) = self.materialized_workspace_events(workspace_id) else {
            return Vec::new();
        };
        let mut state = WorkspaceState::new(workspace_id.clone());
        if state.apply_batch(&events).is_err() {
            return Vec::new();
        }
        let Some(local_member) = state.members.get(self.identity.device_id()) else {
            return Vec::new();
        };
        if !matches!(
            local_member.role,
            WorkspaceRole::Owner | WorkspaceRole::Admin
        ) {
            return Vec::new();
        }

        let mut provision_index = OpenMlsAutoProvisionIndex::from_events(&events);
        let mut device_ids = state.members.keys().cloned().collect::<Vec<_>>();
        device_ids.sort_by(|left, right| left.0.cmp(&right.0));

        device_ids
            .into_iter()
            .filter_map(|device_id| {
                self.auto_add_openmls_workspace_member_if_ready_with_index(
                    workspace_id,
                    &device_id,
                    &mut provision_index,
                )
                .map(|added| added.event_id)
            })
            .collect()
    }

    fn auto_provision_openmls_channel_members(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Vec<ProvisionedOpenMlsChannelMembers> {
        let Ok(events) = self.materialized_workspace_events(workspace_id) else {
            return Vec::new();
        };
        let Ok(channel_ids) = self.local_openmls_channel_group_ids(workspace_id) else {
            return Vec::new();
        };

        let mut provision_index = OpenMlsAutoProvisionIndex::from_events(&events);
        let mut provisioned = Vec::new();
        for channel_id in channel_ids {
            let mut device_ids =
                current_private_channel_member_ids_from_events(&events, &channel_id)
                    .into_iter()
                    .map(DeviceId)
                    .collect::<Vec<_>>();
            device_ids.sort_by(|left, right| left.0.cmp(&right.0));

            let event_ids = device_ids
                .into_iter()
                .filter_map(|device_id| {
                    self.auto_add_openmls_channel_member_if_ready_with_index(
                        workspace_id,
                        &channel_id,
                        &device_id,
                        &mut provision_index,
                    )
                    .map(|added| added.event_id)
                })
                .collect::<Vec<_>>();

            if !event_ids.is_empty() {
                provisioned.push(ProvisionedOpenMlsChannelMembers {
                    channel_id: channel_id.0,
                    event_ids,
                });
            }
        }

        provisioned
    }

    pub async fn sync_workspace_with_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<SyncedWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_peer_address(peer)?;
        let mut published = self
            .publish_workspace_to_peer(transport, peer, workspace_id.clone())
            .await?;
        let pulled = self
            .pull_workspace_from_peer(transport, peer, workspace_id.clone())
            .await?;
        if pulled.has_local_generated_events() {
            let followup = self
                .publish_workspace_to_peer(transport, peer, workspace_id.clone())
                .await?;
            merge_published_workspace(&mut published, followup);
        }

        Ok(SyncedWorkspace {
            workspace_id: workspace_id.0,
            published,
            pulled,
        })
    }

    pub async fn sync_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<SyncedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_peer_address(peer)?;
        let mut published = self
            .publish_workspace_direct(transport, peer, workspace_id.clone())
            .await?;
        let pulled = self
            .pull_workspace_direct(transport, peer, workspace_id.clone())
            .await?;
        if pulled.has_local_generated_events() {
            let followup = self
                .publish_workspace_direct(transport, peer, workspace_id.clone())
                .await?;
            merge_published_workspace(&mut published, followup);
        }

        Ok(SyncedWorkspace {
            workspace_id: workspace_id.0,
            published,
            pulled,
        })
    }

    pub fn event_store_path(&self) -> &Path {
        &self.paths.event_store
    }

    fn annotate_attachment_availability(
        &self,
        snapshot: &mut WorkspaceSnapshot,
    ) -> Result<(), RuntimeError> {
        let blob_store = self.open_blob_store()?;
        for item in &mut snapshot.timeline {
            for attachment in &mut item.attachments {
                attachment.local_blob_available =
                    Some(blob_store.has_complete_blob(&attachment.blob_hash)?);
            }
        }
        Ok(())
    }

    fn materialized_workspace_events(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<SignedEvent>, RuntimeError> {
        Ok(self
            .materialized_workspace_events_with_gaps(workspace_id)?
            .0)
    }

    fn materialized_workspace_events_with_gaps(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(Vec<SignedEvent>, Vec<PulledWorkspaceGap>), RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        let mut events_by_id = events
            .iter()
            .cloned()
            .map(|event| (event.event_id.clone(), event))
            .collect::<HashMap<_, _>>();

        let applied_events = report
            .applied_events
            .into_iter()
            .filter_map(|event_id| events_by_id.remove(&event_id))
            .collect();
        let gaps = report
            .gaps
            .into_iter()
            .map(|gap| PulledWorkspaceGap {
                event_id: gap.event_id.0,
                missing_parent_ids: gap
                    .missing_parent_ids
                    .into_iter()
                    .map(|event_id| event_id.0)
                    .collect(),
            })
            .collect();

        Ok((applied_events, gaps))
    }

    fn read_blob_transfer_ledger(&self) -> Result<BlobTransferLedger, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            &self.paths.blob_transfer_ledger,
            BLOB_TRANSFER_LEDGER_MAX_BYTES,
            "blob transfer ledger",
        )?
        else {
            return Ok(BlobTransferLedger::default());
        };
        let mut ledger = serde_json::from_slice::<BlobTransferLedger>(&bytes)?;
        if ledger.schema_version != BLOB_TRANSFER_LEDGER_SCHEMA_VERSION {
            ledger = BlobTransferLedger::default();
        } else {
            if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
                let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
                ledger.entries.drain(0..remove_count);
            }
            for entry in &mut ledger.entries {
                entry.normalize_after_read();
            }
        }
        Ok(ledger)
    }

    fn write_blob_transfer_ledger(&self, ledger: &BlobTransferLedger) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec_pretty(ledger)?;
        write_secret_file(&self.paths.blob_transfer_ledger, &bytes)
    }

    fn read_compromise_response_ledger(&self) -> Result<CompromiseResponseLedger, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            &self.paths.compromise_response_ledger,
            COMPROMISE_RESPONSE_LEDGER_MAX_BYTES,
            "compromise response ledger",
        )?
        else {
            return Ok(CompromiseResponseLedger::default());
        };
        let mut ledger = serde_json::from_slice::<CompromiseResponseLedger>(&bytes)?;
        if ledger.schema_version != COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION {
            ledger = CompromiseResponseLedger::default();
        }
        Ok(ledger)
    }

    fn write_compromise_response_ledger(
        &self,
        ledger: &CompromiseResponseLedger,
    ) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec_pretty(ledger)?;
        write_secret_file(&self.paths.compromise_response_ledger, &bytes)
    }

    fn handled_compromise_signal_event_ids_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<BTreeSet<String>, RuntimeError> {
        Ok(self
            .read_compromise_response_ledger()?
            .entries
            .into_iter()
            .filter(|entry| entry.workspace_id == workspace_id.0)
            .flat_map(|entry| entry.signal_event_ids)
            .collect())
    }

    fn record_compromise_response(
        &self,
        workspace_id: &WorkspaceId,
        signal_event_ids: Vec<String>,
        rotated_event_ids: Vec<String>,
    ) -> Result<(), RuntimeError> {
        let mut ledger = self.read_compromise_response_ledger()?;
        ledger.entries.push(CompromiseResponseLedgerEntry {
            workspace_id: workspace_id.0.clone(),
            signal_event_ids,
            rotated_event_ids,
            responded_at_unix_ms: now_unix_ms(),
        });
        if ledger.entries.len() > COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES;
            ledger.entries.drain(..remove_count);
        }
        self.write_compromise_response_ledger(&ledger)
    }

    #[allow(clippy::too_many_arguments)]
    fn record_blob_transfer_started(
        &self,
        workspace_id: &str,
        peer: &PeerAddress,
        blob_hash: &str,
        mode: BlobTransferMode,
        total_byte_len: u64,
        chunk_size: Option<u64>,
        chunk_hashes: Vec<String>,
        planned_chunk_hashes: Vec<String>,
        remote_available_chunk_hashes: Vec<String>,
    ) -> Result<BlobTransferAttempt, RuntimeError> {
        validate_peer_address(peer)?;
        let mut ledger = self.read_blob_transfer_ledger()?;
        let attempt_count = ledger
            .entries
            .iter()
            .filter(|entry| {
                entry.workspace_id == workspace_id
                    && entry.peer_id == peer.peer_id.0
                    && entry.peer_endpoint == peer.endpoint
                    && entry.blob_hash == blob_hash
            })
            .count() as u32
            + 1;
        let started_at_unix_ms = now_unix_ms();
        let mut attempt_id = format!(
            "{}:{}:{}:{}",
            started_at_unix_ms, attempt_count, peer.peer_id.0, blob_hash
        );
        truncate_string_bytes(&mut attempt_id, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES);
        let attempt = BlobTransferAttempt {
            attempt_id,
            workspace_id: workspace_id.to_owned(),
            peer_id: peer.peer_id.0.clone(),
            peer_endpoint: peer.endpoint.clone(),
            blob_hash: blob_hash.to_owned(),
            mode,
            status: BlobTransferStatus::InProgress,
            attempt_count,
            total_byte_len,
            chunk_size,
            chunk_count: chunk_hashes.len(),
            chunk_hashes,
            planned_chunk_count: planned_chunk_hashes.len(),
            planned_chunk_hashes,
            remote_available_chunk_count: remote_available_chunk_hashes.len(),
            remote_available_chunk_hashes,
            started_at_unix_ms,
            finished_at_unix_ms: None,
            error: None,
        };
        ledger.entries.push(attempt.clone());
        if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        self.write_blob_transfer_ledger(&ledger)?;
        Ok(attempt)
    }

    fn record_blob_transfer_finished(
        &self,
        started: &BlobTransferAttempt,
        status: BlobTransferStatus,
        error: Option<String>,
    ) -> Result<BlobTransferAttempt, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let mut finished = started.clone();
        finished.status = status;
        finished.finished_at_unix_ms = Some(now_unix_ms());
        finished.error = error;
        truncate_string_option_bytes(&mut finished.error, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);

        if let Some(entry) = ledger
            .entries
            .iter_mut()
            .find(|entry| entry.attempt_id == started.attempt_id)
        {
            *entry = finished.clone();
        } else {
            ledger.entries.push(finished.clone());
        }
        if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        self.write_blob_transfer_ledger(&ledger)?;
        Ok(finished)
    }

    fn reconcile_completed_blob_transfer_attempts(
        &self,
        workspace_id: &str,
        peer: &PeerAddress,
        blob_hash: &str,
    ) -> Result<Vec<BlobTransferAttempt>, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let finished_at_unix_ms = now_unix_ms();
        let mut reconciled = Vec::new();

        for entry in &mut ledger.entries {
            if entry.workspace_id == workspace_id
                && entry.peer_id == peer.peer_id.0
                && entry.peer_endpoint == peer.endpoint
                && entry.blob_hash == blob_hash
                && entry.status != BlobTransferStatus::Succeeded
            {
                entry.status = BlobTransferStatus::Succeeded;
                entry.finished_at_unix_ms = Some(finished_at_unix_ms);
                entry.error = None;
                reconciled.push(entry.clone());
            }
        }

        if !reconciled.is_empty() {
            self.write_blob_transfer_ledger(&ledger)?;
        }
        Ok(reconciled)
    }

    fn reconcile_satisfied_blob_transfer_attempts(
        &self,
        workspace_id: &str,
        blob_hash: &str,
    ) -> Result<Vec<BlobTransferAttempt>, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let finished_at_unix_ms = now_unix_ms();
        let mut reconciled = Vec::new();

        for entry in &mut ledger.entries {
            if entry.workspace_id == workspace_id
                && entry.blob_hash == blob_hash
                && entry.status != BlobTransferStatus::Succeeded
            {
                entry.status = BlobTransferStatus::Succeeded;
                entry.finished_at_unix_ms = Some(finished_at_unix_ms);
                entry.error = None;
                reconciled.push(entry.clone());
            }
        }

        if !reconciled.is_empty() {
            self.write_blob_transfer_ledger(&ledger)?;
        }
        Ok(reconciled)
    }

    async fn retry_blob_transfer_to_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: &str,
        blob_hash: &str,
        bytes: Vec<u8>,
        remote_availability: Option<&BlobAvailability>,
    ) -> Result<(BlobTransferAttempt, bool), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        if bytes.len() > DIRECT_WHOLE_BLOB_SYNC_LIMIT {
            let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
                planned_chunk_upload(&bytes, remote_availability);
            let attempt = self.record_blob_transfer_started(
                workspace_id,
                peer,
                blob_hash,
                BlobTransferMode::ChunkedBlob,
                bytes.len() as u64,
                Some(chunk_size),
                chunk_hashes,
                planned_chunk_hashes,
                remote_available_chunk_hashes,
            )?;
            return match transport
                .put_blob_chunked(peer, bytes, DIRECT_BLOB_CHUNK_SIZE)
                .await
            {
                Ok(_) => self
                    .record_blob_transfer_finished(&attempt, BlobTransferStatus::Succeeded, None)
                    .map(|attempt| (attempt, false)),
                Err(error) => {
                    let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                    self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Failed,
                        Some(error.to_string()),
                    )
                    .map(|attempt| (attempt, suspect_protocol_error))
                }
            };
        }

        let attempt = self.record_blob_transfer_started(
            workspace_id,
            peer,
            blob_hash,
            BlobTransferMode::WholeBlob,
            bytes.len() as u64,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )?;
        match transport.put_blobs(peer, vec![bytes]).await {
            Ok(_) => self
                .record_blob_transfer_finished(&attempt, BlobTransferStatus::Succeeded, None)
                .map(|attempt| (attempt, false)),
            Err(error) => {
                let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                self.record_blob_transfer_finished(
                    &attempt,
                    BlobTransferStatus::Failed,
                    Some(error.to_string()),
                )
                .map(|attempt| (attempt, suspect_protocol_error))
            }
        }
    }

    async fn publish_materialized_event_blobs_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        events: &[SignedEvent],
        published: &mut PublishedWorkspace,
    ) -> Result<(), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        let blob_hashes = attachment_blob_hashes(events);
        if blob_hashes.is_empty() {
            return Ok(());
        }

        let blob_store = self.open_blob_store()?;
        let remote_blob_availability = transport
            .fetch_blob_availabilities(peer, blob_hashes.clone())
            .await?;
        let mut blobs_to_publish = Vec::new();
        let mut chunked_blobs_to_publish = Vec::new();

        for blob_hash in blob_hashes {
            if remote_blob_availability
                .get(&blob_hash)
                .is_some_and(|availability| availability.is_complete())
            {
                published.blob_transfer_attempts.extend(
                    self.reconcile_completed_blob_transfer_attempts(
                        &published.workspace_id,
                        peer,
                        &blob_hash,
                    )?,
                );
                continue;
            }
            match blob_store.get_complete_bytes(&blob_hash)? {
                Some(bytes) if bytes.len() > DIRECT_WHOLE_BLOB_SYNC_LIMIT => {
                    chunked_blobs_to_publish.push((blob_hash, bytes));
                }
                Some(bytes) => blobs_to_publish.push((blob_hash, bytes)),
                None => published.missing_blob_hashes.push(blob_hash),
            }
        }
        if !blobs_to_publish.is_empty() {
            let attempts = blobs_to_publish
                .iter()
                .map(|(blob_hash, bytes)| {
                    self.record_blob_transfer_started(
                        &published.workspace_id,
                        peer,
                        blob_hash,
                        BlobTransferMode::WholeBlob,
                        bytes.len() as u64,
                        None,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            match transport
                .put_blobs(
                    peer,
                    blobs_to_publish
                        .iter()
                        .map(|(_, bytes)| bytes.clone())
                        .collect(),
                )
                .await
            {
                Ok(_) => {
                    for attempt in &attempts {
                        let finished = self.record_blob_transfer_finished(
                            attempt,
                            BlobTransferStatus::Succeeded,
                            None,
                        )?;
                        published.blob_transfer_attempts.push(finished);
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    for attempt in &attempts {
                        let finished = self.record_blob_transfer_finished(
                            attempt,
                            BlobTransferStatus::Failed,
                            Some(message.clone()),
                        )?;
                        published.blob_transfer_attempts.push(finished);
                    }
                    return Err(error.into());
                }
            }
            published
                .published_blob_hashes
                .extend(blobs_to_publish.into_iter().map(|(hash, _)| hash));
        }
        for (blob_hash, bytes) in chunked_blobs_to_publish {
            let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
                planned_chunk_upload(&bytes, remote_blob_availability.get(&blob_hash));
            let attempt = self.record_blob_transfer_started(
                &published.workspace_id,
                peer,
                &blob_hash,
                BlobTransferMode::ChunkedBlob,
                bytes.len() as u64,
                Some(chunk_size),
                chunk_hashes,
                planned_chunk_hashes,
                remote_available_chunk_hashes,
            )?;
            match transport
                .put_blob_chunked(peer, bytes, DIRECT_BLOB_CHUNK_SIZE)
                .await
            {
                Ok(_) => {
                    let finished = self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Succeeded,
                        None,
                    )?;
                    published.blob_transfer_attempts.push(finished);
                }
                Err(error) => {
                    let message = error.to_string();
                    let finished = self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Failed,
                        Some(message),
                    )?;
                    published.blob_transfer_attempts.push(finished);
                    return Err(error.into());
                }
            }
            published.published_blob_hashes.push(blob_hash);
        }

        Ok(())
    }

    fn open_search_index(&self) -> Result<SearchIndex, RuntimeError> {
        Ok(SearchIndex::open(&self.paths.search_index)?)
    }

    fn open_blob_store(&self) -> Result<BlobStore, RuntimeError> {
        Ok(BlobStore::open(&self.paths.blob_store)?)
    }

    fn seal_and_store_attachments(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        content_key: &ResolvedContentKey,
        pending_attachments: Vec<PendingAttachment>,
    ) -> Result<Vec<AttachmentRef>, RuntimeError> {
        if pending_attachments.is_empty() {
            return Ok(Vec::new());
        }

        let blob_store = self.open_blob_store()?;
        let mut attachments = Vec::with_capacity(pending_attachments.len());
        for (index, pending) in pending_attachments.into_iter().enumerate() {
            validate_attachment_plaintext_size(pending.plaintext.len() as u64)?;
            let sealed = seal_attachment_blob(
                content_key.key_id(),
                content_key.content_key(),
                workspace_id,
                channel_id,
                message_id,
                index as u32,
                &pending.plaintext,
            )?;
            let encryption =
                encrypted_blob_ref_from_payload(&sealed, pending.plaintext.len() as u64)?;
            let descriptor = blob_store.put_bytes(&sealed.bytes)?;
            attachments.push(AttachmentRef {
                blob_hash: descriptor.hash,
                media_type: if pending.media_type.is_empty() {
                    "application/octet-stream".to_owned()
                } else {
                    pending.media_type
                },
                byte_len: descriptor.byte_len,
                display_name: pending.display_name,
                attachment_id: attachment_id_for_message_slot(message_id, index),
                encryption: Some(encryption),
            });
        }

        Ok(attachments)
    }

    fn index_message_plaintext(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        event_id: &EventId,
        physical_ms: i64,
        markdown: &str,
    ) -> Result<(), RuntimeError> {
        self.open_search_index()?.index_message(
            workspace_id,
            channel_id,
            message_id,
            event_id,
            physical_ms,
            markdown,
        )?;
        Ok(())
    }

    fn remove_message_from_search(
        &self,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
    ) -> Result<(), RuntimeError> {
        self.open_search_index()?
            .remove_message(workspace_id, message_id)?;
        Ok(())
    }

    fn reindex_workspace_search_if_key_available(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(), RuntimeError> {
        let workspace_key = self.load_workspace_key(workspace_id)?;
        if workspace_key.is_some() || self.has_openmls_group_state(workspace_id) {
            self.reindex_workspace_search_with_key(workspace_id, workspace_key.as_ref())?;
        }
        Ok(())
    }

    fn reindex_workspace_search_with_key(
        &self,
        workspace_id: &WorkspaceId,
        workspace_key: Option<&WorkspaceKey>,
    ) -> Result<IndexedWorkspaceSearch, RuntimeError> {
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let physical_ms_by_event_id = events
            .iter()
            .map(|event| (&event.event_id, event.event.timestamp.physical_ms))
            .collect::<HashMap<_, _>>();

        let index = self.open_search_index()?;
        index.clear_workspace(workspace_id)?;
        let mut indexed_message_count = 0;

        for message in state.messages.values() {
            if message.deleted {
                continue;
            }
            if !state.channel_accessible_to(&message.channel_id, self.identity.device_id()) {
                continue;
            }

            let markdown = if let Some(sealed_markdown) = message.sealed_markdown.as_ref() {
                let Some(content_key) = self.content_key_for_materialized_payload(
                    workspace_id,
                    &message.channel_id,
                    &state,
                    workspace_key,
                    &sealed_markdown.key_id,
                )?
                else {
                    continue;
                };
                open_message_markdown(
                    content_key.content_key(),
                    sealed_markdown,
                    workspace_id,
                    &message.channel_id,
                    &message.message_id,
                )?
            } else {
                message.markdown.clone()
            };
            if markdown.trim().is_empty() {
                continue;
            }

            index.index_message(
                workspace_id,
                &message.channel_id,
                &message.message_id,
                &message.author_event_id,
                physical_ms_by_event_id
                    .get(&message.author_event_id)
                    .copied()
                    .unwrap_or_default(),
                &markdown,
            )?;
            indexed_message_count += 1;
        }

        Ok(IndexedWorkspaceSearch {
            workspace_id: workspace_id.0.clone(),
            indexed_message_count,
        })
    }

    fn content_key_for_local_write_in_state(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        state: &WorkspaceState,
    ) -> Result<ResolvedContentKey, RuntimeError> {
        let channel = state.channels.get(channel_id).ok_or_else(|| {
            RuntimeError::Authorization(AuthorizationError::ChannelNotFound {
                channel_id: channel_id.clone(),
            })
        })?;

        if channel.is_private {
            if !state.channel_accessible_to(channel_id, self.identity.device_id()) {
                return Err(RuntimeError::Authorization(
                    AuthorizationError::PrivateChannelAccessDenied {
                        channel_id: channel_id.clone(),
                        device_id: self.identity.device_id().clone(),
                    },
                ));
            }
            if let Some(content_key) = self.openmls_channel_content_key(workspace_id, channel_id)? {
                return Ok(content_key);
            }
            return self
                .load_channel_key(workspace_id, channel_id)?
                .ok_or_else(|| RuntimeError::ChannelKeyMissing {
                    workspace_id: workspace_id.clone(),
                    channel_id: channel_id.clone(),
                })
                .map(ResolvedContentKey::from);
        }

        if let Some(content_key) = self.openmls_workspace_content_key(workspace_id)? {
            return Ok(content_key);
        }
        self.load_workspace_key(workspace_id)?
            .ok_or(RuntimeError::InvalidWorkspaceKey)
            .map(ResolvedContentKey::from)
    }

    fn content_key_for_materialized_payload(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        state: &WorkspaceState,
        workspace_key: Option<&WorkspaceKey>,
        key_id: &str,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let Some(channel) = state.channels.get(channel_id) else {
            return Ok(None);
        };
        if channel.is_private {
            if let Some(openmls_key) =
                self.openmls_channel_content_key_for_key_id(workspace_id, channel_id, key_id)?
            {
                return Ok(Some(openmls_key));
            }
            return self
                .load_channel_key(workspace_id, channel_id)
                .map(|key| key.and_then(|key| key.resolve_content_key(key_id)));
        }

        if let Some(openmls_key) =
            self.openmls_workspace_content_key_for_key_id(workspace_id, key_id)?
        {
            return Ok(Some(openmls_key));
        }

        Ok(workspace_key.and_then(|key| key.resolve_content_key(key_id)))
    }

    fn openmls_workspace_content_key(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(workspace_id);
        self.openmls_group_content_key(&private_group_state_path)
    }

    fn openmls_channel_content_key(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(workspace_id, channel_id);
        self.openmls_group_content_key(&private_group_state_path)
    }

    fn openmls_workspace_content_key_for_key_id(
        &self,
        workspace_id: &WorkspaceId,
        key_id: &str,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(workspace_id);
        self.openmls_group_content_key_for_key_id(&private_group_state_path, key_id)
    }

    fn openmls_channel_content_key_for_key_id(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        key_id: &str,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(workspace_id, channel_id);
        self.openmls_group_content_key_for_key_id(&private_group_state_path, key_id)
    }

    fn openmls_group_content_key(
        &self,
        private_group_state_path: &Path,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let Some(private_group_state) = self.read_optional_openmls_secret_file(
            private_group_state_path,
            openmls_group_secret_kind(private_group_state_path),
        )?
        else {
            return Ok(None);
        };
        let exported = chaft_mls::export_group_content_key(&private_group_state)?;
        let content_key = content_key_from_mls_export(exported.content_key)?;

        Ok(Some(ResolvedContentKey {
            key_id: exported.key_id,
            content_key,
        }))
    }

    fn openmls_group_content_key_for_key_id(
        &self,
        private_group_state_path: &Path,
        key_id: &str,
    ) -> Result<Option<ResolvedContentKey>, RuntimeError> {
        let Some(private_group_state) = self.read_optional_openmls_secret_file(
            private_group_state_path,
            openmls_group_secret_kind(private_group_state_path),
        )?
        else {
            return Ok(None);
        };
        let Some(exported) =
            chaft_mls::export_group_content_key_for_key_id(&private_group_state, key_id)?
        else {
            return Ok(None);
        };
        let content_key = content_key_from_mls_export(exported.content_key)?;

        Ok(Some(ResolvedContentKey {
            key_id: exported.key_id,
            content_key,
        }))
    }

    fn has_openmls_group_state(&self, workspace_id: &WorkspaceId) -> bool {
        self.openmls_workspace_group_path(workspace_id).exists()
            || self
                .openmls_channel_groups_dir(workspace_id)
                .read_dir()
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
    }

    fn local_openmls_channel_group_ids(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<ChannelId>, RuntimeError> {
        let channel_groups_dir = self.openmls_channel_groups_dir(workspace_id);
        if !channel_groups_dir.exists() {
            return Ok(Vec::new());
        }

        let mut channel_ids = Vec::new();
        for entry in channel_groups_dir.read_dir()? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if file_stem.is_empty() {
                continue;
            }
            channel_ids.push(ChannelId(file_stem.to_owned()));
        }
        channel_ids.sort_by(|left, right| left.0.cmp(&right.0));
        channel_ids.dedup_by(|left, right| left.0 == right.0);
        Ok(channel_ids)
    }

    fn local_updatable_openmls_channel_group_ids(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<ChannelId>, RuntimeError> {
        let events = self.materialized_workspace_events(workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let mut channel_ids = state
            .channels
            .values()
            .filter(|channel| {
                channel.is_private
                    && state.channel_accessible_to(&channel.channel_id, self.identity.device_id())
                    && self
                        .openmls_channel_group_path(workspace_id, &channel.channel_id)
                        .exists()
            })
            .map(|channel| channel.channel_id.clone())
            .collect::<Vec<_>>();
        channel_ids.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(channel_ids)
    }

    fn joinable_openmls_channel_group_ids(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<ChannelId>, RuntimeError> {
        let events = self.materialized_workspace_events(workspace_id)?;
        let mut channel_ids = BTreeSet::new();
        for event in events {
            let EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id,
                invitee_device_id,
                ..
            } = event.event.body
            else {
                continue;
            };
            if invitee_device_id != *self.identity.device_id() {
                continue;
            }
            if self
                .openmls_channel_group_path(workspace_id, &channel_id)
                .exists()
            {
                continue;
            }
            channel_ids.insert(channel_id.0);
        }
        Ok(channel_ids.into_iter().map(ChannelId).collect())
    }

    fn local_private_channel_key_ids(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<ChannelId>, RuntimeError> {
        let events = self.materialized_workspace_events(workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let mut channel_ids = state
            .channels
            .values()
            .filter(|channel| {
                channel.is_private
                    && self
                        .channel_key_path(workspace_id, &channel.channel_id)
                        .exists()
            })
            .map(|channel| channel.channel_id.clone())
            .collect::<Vec<_>>();
        channel_ids.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(channel_ids)
    }

    fn message_channel_id_from_state(
        state: &WorkspaceState,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
    ) -> Result<ChannelId, RuntimeError> {
        Ok(
            Self::message_view_from_state(state, workspace_id, message_id)?
                .channel_id
                .clone(),
        )
    }

    fn message_view_from_state<'a>(
        state: &'a WorkspaceState,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
    ) -> Result<&'a MessageView, RuntimeError> {
        validate_message_id_reference(message_id)?;
        state
            .messages
            .get(message_id)
            .ok_or_else(|| RuntimeError::MessageNotFound {
                workspace_id: workspace_id.clone(),
                message_id: message_id.clone(),
            })
    }

    fn message_attachment_from_state(
        state: &WorkspaceState,
        device_id: &DeviceId,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
        attachment_selector: &str,
    ) -> Result<(ChannelId, AttachmentRef, usize), RuntimeError> {
        validate_message_id_reference(message_id)?;
        let message =
            state
                .messages
                .get(message_id)
                .ok_or_else(|| RuntimeError::MessageNotFound {
                    workspace_id: workspace_id.clone(),
                    message_id: message_id.clone(),
                })?;
        if !state.channel_accessible_to(&message.channel_id, device_id) {
            return Err(RuntimeError::ChannelAccessDenied {
                workspace_id: workspace_id.clone(),
                channel_id: message.channel_id.clone(),
                device_id: device_id.clone(),
            });
        }
        let (attachment_index, attachment) = message
            .attachments
            .iter()
            .enumerate()
            .find(|(_, attachment)| {
                !attachment.attachment_id.is_empty()
                    && attachment.attachment_id == attachment_selector
            })
            .or_else(|| {
                message
                    .attachments
                    .iter()
                    .enumerate()
                    .find(|(_, attachment)| attachment.blob_hash == attachment_selector)
            })
            .ok_or_else(|| RuntimeError::AttachmentNotFound {
                workspace_id: workspace_id.clone(),
                message_id: message_id.clone(),
                blob_hash: attachment_selector.to_owned(),
            })?;

        Ok((
            message.channel_id.clone(),
            attachment.clone(),
            attachment_index,
        ))
    }

    fn local_workspace_summary(
        &self,
        workspace_id: &WorkspaceId,
        events: &[SignedEvent],
        event_count: usize,
    ) -> Result<LocalWorkspaceSummary, RuntimeError> {
        let verified_events = verified_local_events_for_runtime(events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&verified_events)?;
        let channel_count = state
            .channels
            .values()
            .filter(|channel| {
                state.channel_accessible_to(&channel.channel_id, self.identity.device_id())
            })
            .count();

        Ok(LocalWorkspaceSummary {
            workspace_id: workspace_id.0.clone(),
            name: state.name.clone().unwrap_or_else(|| "Chaft".to_owned()),
            channel_count,
            member_count: state.members.len(),
            event_count,
            has_workspace_key: self.workspace_key_path(workspace_id).exists(),
        })
    }

    fn latest_channel_read_event_id_from_context(
        context: &WorkspaceWriteContext,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> Result<EventId, RuntimeError> {
        validate_channel_id_reference(channel_id)?;
        if !context.state.channels.contains_key(channel_id) {
            return Err(RuntimeError::ChannelNotFound {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        for event_id in context.report.applied_events.iter().rev() {
            let Some(event) = context
                .events
                .iter()
                .find(|event| event.event_id == *event_id)
            else {
                continue;
            };
            match &event.event.body {
                EventBody::MessageCreated { .. }
                | EventBody::MessageCreatedEncrypted { .. }
                | EventBody::MessageReplyCreated { .. }
                | EventBody::MessageReplyCreatedEncrypted { .. }
                    if event.event.channel_id.as_ref() == Some(channel_id) =>
                {
                    return Ok(event_id.clone());
                }
                EventBody::ChannelCreated {
                    channel_id: created_channel_id,
                    ..
                } if created_channel_id == channel_id => {
                    return Ok(event_id.clone());
                }
                _ => {}
            }
        }

        Err(RuntimeError::ChannelNotFound {
            workspace_id: workspace_id.clone(),
            channel_id: channel_id.clone(),
        })
    }

    fn channel_is_read_through_in_state(
        state: &WorkspaceState,
        device_id: &DeviceId,
        channel_id: &ChannelId,
        read_through_event_id: &EventId,
    ) -> bool {
        state
            .read_markers
            .get(device_id)
            .and_then(|channels| channels.get(channel_id))
            == Some(read_through_event_id)
    }

    fn workspace_write_context(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<WorkspaceWriteContext, RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        if raw_events.is_empty() {
            return Err(RuntimeError::WorkspaceHasNoEvents {
                workspace_id: workspace_id.clone(),
            });
        }

        let verified_events = verified_local_events_for_runtime(&raw_events);
        Self::workspace_write_context_from_materialized_events(workspace_id, verified_events)
    }

    fn materialized_workspace_write_context(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<WorkspaceWriteContext, RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let verified_events = verified_local_events_for_runtime(&raw_events);
        Self::workspace_write_context_from_materialized_events(workspace_id, verified_events)
    }

    fn workspace_write_context_from_materialized_events(
        workspace_id: &WorkspaceId,
        verified_events: Cow<'_, [SignedEvent]>,
    ) -> Result<WorkspaceWriteContext, RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&verified_events)?;
        if report.applied_events.is_empty() {
            return Err(RuntimeError::WorkspaceHasNoEvents {
                workspace_id: workspace_id.clone(),
            });
        }

        let mut events_by_id = verified_events
            .iter()
            .cloned()
            .map(|event| (event.event_id.clone(), event))
            .collect::<HashMap<_, _>>();
        let events = report
            .applied_events
            .iter()
            .filter_map(|event_id| events_by_id.remove(event_id))
            .collect::<Vec<_>>();
        let head_event_ids = Self::workspace_head_event_ids_from_materialization(&events, &report);

        Ok(WorkspaceWriteContext {
            events,
            state,
            report,
            head_event_ids,
        })
    }

    fn workspace_head_event_ids_from_materialization(
        events: &[SignedEvent],
        report: &MaterializationReport,
    ) -> Vec<EventId> {
        let applied_event_ids = report
            .applied_events
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut referenced_parent_ids = BTreeSet::new();
        for event in events
            .iter()
            .filter(|event| applied_event_ids.contains(&event.event_id))
        {
            referenced_parent_ids.extend(
                event
                    .event
                    .parents
                    .iter()
                    .filter(|parent_id| applied_event_ids.contains(parent_id))
                    .cloned(),
            );
        }
        let heads = report
            .applied_events
            .iter()
            .filter(|event_id| !referenced_parent_ids.contains(event_id))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        if heads.is_empty() {
            return vec![
                report
                    .applied_events
                    .last()
                    .expect("applied events is not empty")
                    .clone(),
            ];
        }

        heads
    }

    fn workspace_key_path(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(format!("{}.json", workspace_id.0))
    }

    fn channel_key_path(&self, workspace_id: &WorkspaceId, channel_id: &ChannelId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("channels")
            .join(format!("{}.json", channel_id.0))
    }

    fn openmls_key_package_path(
        &self,
        workspace_id: &WorkspaceId,
        key_package_ref: &str,
    ) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-key-packages")
            .join(format!("{key_package_ref}.json"))
    }

    fn openmls_workspace_group_path(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-groups")
            .join("workspace.json")
    }

    fn openmls_channel_groups_dir(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-groups")
            .join("channels")
    }

    fn openmls_channel_group_path(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> PathBuf {
        self.openmls_channel_groups_dir(workspace_id)
            .join(format!("{}.json", channel_id.0))
    }

    fn local_secret_path_hint(&self, path: &Path) -> String {
        path.strip_prefix(&self.paths.data_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }

    fn read_local_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Vec<u8>, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            path,
            LOCAL_SECRET_FILE_MAX_BYTES,
            "local secret file",
        )?
        else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("local secret file not found: {}", path.display()),
            )
            .into());
        };
        if let Ok(encrypted) = serde_json::from_slice::<PersistedEncryptedLocalSecret>(&bytes)
            && encrypted.storage == LOCAL_SECRET_STORAGE
        {
            let path_hint = self.local_secret_path_hint(path);
            return open_local_secret(
                encrypted,
                secret_kind,
                &path_hint,
                self.identity_passphrase.as_deref(),
            );
        }
        Ok(bytes)
    }

    fn write_local_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
        plaintext: &[u8],
    ) -> Result<(), RuntimeError> {
        let bytes = match self.identity_passphrase.as_deref() {
            Some(passphrase) => {
                let path_hint = self.local_secret_path_hint(path);
                serde_json::to_vec_pretty(&encrypt_local_secret(
                    secret_kind,
                    &path_hint,
                    passphrase,
                    plaintext,
                )?)?
            }
            None => plaintext.to_vec(),
        };
        write_secret_file(path, &bytes)
    }

    fn load_workspace_key(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceKey>, RuntimeError> {
        let path = self.workspace_key_path(workspace_id);
        match self.read_local_secret_file(&path, LOCAL_SECRET_KIND_WORKSPACE_KEY) {
            Ok(bytes) => Ok(Some(WorkspaceKey::from_bytes(&bytes)?)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn save_workspace_key(&self, key: &WorkspaceKey) -> Result<(), RuntimeError> {
        let path = self.workspace_key_path(&key.workspace_id);
        self.write_key_file(&path, LOCAL_SECRET_KIND_WORKSPACE_KEY, &key.persisted())
    }

    fn load_channel_key(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> Result<Option<ChannelKey>, RuntimeError> {
        let path = self.channel_key_path(workspace_id, channel_id);
        match self.read_local_secret_file(&path, LOCAL_SECRET_KIND_CHANNEL_KEY) {
            Ok(bytes) => Ok(Some(ChannelKey::from_bytes(&bytes)?)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn save_channel_key(&self, key: &ChannelKey) -> Result<(), RuntimeError> {
        let path = self.channel_key_path(&key.workspace_id, &key.channel_id);
        self.write_key_file(&path, LOCAL_SECRET_KIND_CHANNEL_KEY, &key.persisted())
    }

    fn read_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.read_local_secret_file(path, secret_kind)
    }

    fn read_optional_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        match self.read_openmls_secret_file(path, secret_kind) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn write_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
        plaintext: &[u8],
    ) -> Result<(), RuntimeError> {
        self.write_local_secret_file(path, secret_kind, plaintext)
    }

    fn write_key_file<T>(
        &self,
        path: &Path,
        secret_kind: &str,
        persisted: &T,
    ) -> Result<(), RuntimeError>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec_pretty(persisted)?;
        self.write_local_secret_file(path, secret_kind, &bytes)
    }

    fn require_local_channel_access(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> Result<(), RuntimeError> {
        let events = self.materialized_workspace_events(workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        self.require_local_channel_access_in_state(&state, channel_id)
    }

    fn require_local_channel_access_in_state(
        &self,
        state: &WorkspaceState,
        channel_id: &ChannelId,
    ) -> Result<(), RuntimeError> {
        if !state.channels.contains_key(channel_id) {
            return Err(AuthorizationError::ChannelNotFound {
                channel_id: channel_id.clone(),
            }
            .into());
        }
        if !state.channel_accessible_to(channel_id, self.identity.device_id()) {
            return Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: channel_id.clone(),
                device_id: self.identity.device_id().clone(),
            }
            .into());
        }
        Ok(())
    }

    fn require_workspace_admin(
        &self,
        workspace_id: &WorkspaceId,
        action: &'static str,
    ) -> Result<(), RuntimeError> {
        let events = self.materialized_workspace_events(workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        self.require_workspace_admin_in_state(&state, action)
    }

    fn require_workspace_admin_in_state(
        &self,
        state: &WorkspaceState,
        action: &'static str,
    ) -> Result<(), RuntimeError> {
        let role = state
            .members
            .get(self.identity.device_id())
            .map(|member| member.role)
            .ok_or_else(|| AuthorizationError::NotAMember {
                device_id: self.identity.device_id().clone(),
            })?;

        if matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
            Ok(())
        } else {
            Err(AuthorizationError::InsufficientRole { role, action }.into())
        }
    }

    fn sign_authorize_and_append(&self, event: SignableEvent) -> Result<SignedEvent, RuntimeError> {
        let history = self
            .store
            .list_parseable_events_for_workspace(&event.workspace_id.0)?;
        self.sign_authorize_and_append_with_history(event, &history)
    }

    fn sign_authorize_and_append_with_history(
        &self,
        event: SignableEvent,
        history: &[SignedEvent],
    ) -> Result<SignedEvent, RuntimeError> {
        let signed = self.identity.sign_event(event);
        authorize_event_with_history(history, &signed)?;
        self.store.append_event(&signed)?;
        Ok(signed)
    }

    fn sign_authorize_save_key_and_append_with_history<F>(
        &self,
        event: SignableEvent,
        history: &[SignedEvent],
        save_key: F,
    ) -> Result<SignedEvent, RuntimeError>
    where
        F: FnOnce(&Self) -> Result<(), RuntimeError>,
    {
        let signed = self.identity.sign_event(event);
        authorize_event_with_history(history, &signed)?;
        save_key(self)?;
        self.store.append_event(&signed)?;
        Ok(signed)
    }
}

impl From<WorkspaceKey> for ResolvedContentKey {
    fn from(key: WorkspaceKey) -> Self {
        Self {
            key_id: key.key_id,
            content_key: key.content_key,
        }
    }
}

impl From<ChannelKey> for ResolvedContentKey {
    fn from(key: ChannelKey) -> Self {
        Self {
            key_id: key.key_id,
            content_key: key.content_key,
        }
    }
}

#[derive(Clone)]
struct ContentKeyMaterial {
    key_id: String,
    content_key: ContentKey,
}

impl ContentKeyMaterial {
    fn exported(&self) -> ExportedContentKeyMaterial {
        ExportedContentKeyMaterial {
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
        }
    }

    fn persisted(&self) -> PersistedContentKeyMaterial {
        PersistedContentKeyMaterial {
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
        }
    }

    fn resolved(&self) -> ResolvedContentKey {
        ResolvedContentKey {
            key_id: self.key_id.clone(),
            content_key: self.content_key.clone(),
        }
    }
}

#[derive(Clone)]
struct WorkspaceKey {
    workspace_id: WorkspaceId,
    epoch: u64,
    key_id: String,
    content_key: ContentKey,
    previous_keys: Vec<ContentKeyMaterial>,
}

impl WorkspaceKey {
    fn generate(workspace_id: WorkspaceId) -> Self {
        let epoch = 1;
        let key_id = Self::key_id_for_epoch(&workspace_id, epoch);
        Self {
            workspace_id,
            epoch,
            key_id,
            content_key: ContentKey::generate(),
            previous_keys: Vec::new(),
        }
    }

    fn key_id_for_epoch(workspace_id: &WorkspaceId, epoch: u64) -> String {
        format!("{}:content:v{}", workspace_id.0, epoch)
    }

    #[cfg(test)]
    fn key_id(&self) -> &str {
        &self.key_id
    }

    #[cfg(test)]
    fn content_key(&self) -> &ContentKey {
        &self.content_key
    }

    #[cfg(test)]
    fn load(path: &Path) -> Result<Self, RuntimeError> {
        Self::from_bytes(&fs::read(path)?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let persisted: PersistedWorkspaceKey = serde_json::from_slice(bytes)?;
        if !content_key_schema_supported(persisted.schema_version) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let epoch = persisted.epoch.max(1);
        if persisted.key_id != Self::key_id_for_epoch(&persisted.workspace_id, epoch) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let content_key =
            decode_workspace_key_material(persisted.key_id.clone(), persisted.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = persisted
            .previous_keys
            .into_iter()
            .map(|key| decode_workspace_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id: persisted.workspace_id,
            epoch,
            key_id: persisted.key_id,
            content_key,
            previous_keys,
        })
    }

    fn from_export(exported: WorkspaceKeyExport) -> Result<Self, RuntimeError> {
        if !content_key_schema_supported(exported.schema_version) {
            return Err(RuntimeError::UnsupportedWorkspaceKeyExport);
        }
        let workspace_id = WorkspaceId(exported.workspace_id);
        let epoch = exported.epoch.max(1);
        if exported.key_id != Self::key_id_for_epoch(&workspace_id, epoch) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let content_key =
            decode_workspace_key_material(exported.key_id.clone(), exported.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = exported
            .previous_keys
            .into_iter()
            .map(|key| decode_workspace_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id,
            epoch,
            key_id: exported.key_id,
            content_key,
            previous_keys,
        })
    }

    fn rotate(&mut self) {
        let next_epoch = self.epoch + 1;
        let next_key_id = Self::key_id_for_epoch(&self.workspace_id, next_epoch);
        let previous_key_id = std::mem::replace(&mut self.key_id, next_key_id);
        let previous_content_key = std::mem::replace(&mut self.content_key, ContentKey::generate());
        self.previous_keys.push(ContentKeyMaterial {
            key_id: previous_key_id,
            content_key: previous_content_key,
        });
        self.epoch = next_epoch;
    }

    fn resolve_content_key(&self, key_id: &str) -> Option<ResolvedContentKey> {
        if self.key_id == key_id {
            return Some(ResolvedContentKey {
                key_id: self.key_id.clone(),
                content_key: self.content_key.clone(),
            });
        }
        self.previous_keys
            .iter()
            .find(|key| key.key_id == key_id)
            .map(ContentKeyMaterial::resolved)
    }

    fn exported_previous_keys(&self) -> Vec<ExportedContentKeyMaterial> {
        self.previous_keys
            .iter()
            .map(ContentKeyMaterial::exported)
            .collect()
    }

    fn persisted(&self) -> PersistedWorkspaceKey {
        PersistedWorkspaceKey {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: self.workspace_id.clone(),
            epoch: self.epoch,
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
            previous_keys: self
                .previous_keys
                .iter()
                .map(ContentKeyMaterial::persisted)
                .collect(),
        }
    }
}

#[derive(Clone)]
struct ChannelKey {
    workspace_id: WorkspaceId,
    channel_id: ChannelId,
    epoch: u64,
    key_id: String,
    content_key: ContentKey,
    previous_keys: Vec<ContentKeyMaterial>,
}

impl ChannelKey {
    fn generate(workspace_id: WorkspaceId, channel_id: ChannelId) -> Self {
        let epoch = 1;
        let key_id = Self::key_id_for_epoch(&workspace_id, &channel_id, epoch);
        Self {
            workspace_id,
            channel_id,
            epoch,
            key_id,
            content_key: ContentKey::generate(),
            previous_keys: Vec::new(),
        }
    }

    fn key_id_for_epoch(workspace_id: &WorkspaceId, channel_id: &ChannelId, epoch: u64) -> String {
        format!("{}:{}:content:v{}", workspace_id.0, channel_id.0, epoch)
    }

    #[cfg(test)]
    fn load(path: &Path) -> Result<Self, RuntimeError> {
        Self::from_bytes(&fs::read(path)?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let persisted: PersistedChannelKey = serde_json::from_slice(bytes)?;
        if !content_key_schema_supported(persisted.schema_version) {
            return Err(RuntimeError::InvalidChannelKey);
        }
        let epoch = persisted.epoch.max(1);
        let expected_key_id =
            Self::key_id_for_epoch(&persisted.workspace_id, &persisted.channel_id, epoch);
        if persisted.key_id != expected_key_id {
            return Err(RuntimeError::InvalidChannelKey);
        }

        let content_key =
            decode_channel_key_material(persisted.key_id.clone(), persisted.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = persisted
            .previous_keys
            .into_iter()
            .map(|key| decode_channel_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id: persisted.workspace_id,
            channel_id: persisted.channel_id,
            epoch,
            key_id: persisted.key_id,
            content_key,
            previous_keys,
        })
    }

    fn from_export(exported: ChannelKeyExport) -> Result<Self, RuntimeError> {
        if !content_key_schema_supported(exported.schema_version) {
            return Err(RuntimeError::UnsupportedChannelKeyExport);
        }

        let workspace_id = WorkspaceId(exported.workspace_id);
        let channel_id = ChannelId(exported.channel_id);
        let epoch = exported.epoch.max(1);
        if exported.key_id != Self::key_id_for_epoch(&workspace_id, &channel_id, epoch) {
            return Err(RuntimeError::InvalidChannelKey);
        }
        let content_key =
            decode_channel_key_material(exported.key_id.clone(), exported.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = exported
            .previous_keys
            .into_iter()
            .map(|key| decode_channel_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id,
            channel_id,
            epoch,
            key_id: exported.key_id,
            content_key,
            previous_keys,
        })
    }

    fn rotate(&mut self) {
        let next_epoch = self.epoch + 1;
        let next_key_id = Self::key_id_for_epoch(&self.workspace_id, &self.channel_id, next_epoch);
        let previous_key_id = std::mem::replace(&mut self.key_id, next_key_id);
        let previous_content_key = std::mem::replace(&mut self.content_key, ContentKey::generate());
        self.previous_keys.push(ContentKeyMaterial {
            key_id: previous_key_id,
            content_key: previous_content_key,
        });
        self.epoch = next_epoch;
    }

    fn resolve_content_key(&self, key_id: &str) -> Option<ResolvedContentKey> {
        if self.key_id == key_id {
            return Some(ResolvedContentKey {
                key_id: self.key_id.clone(),
                content_key: self.content_key.clone(),
            });
        }
        self.previous_keys
            .iter()
            .find(|key| key.key_id == key_id)
            .map(ContentKeyMaterial::resolved)
    }

    fn exported_previous_keys(&self) -> Vec<ExportedContentKeyMaterial> {
        self.previous_keys
            .iter()
            .map(ContentKeyMaterial::exported)
            .collect()
    }

    fn persisted(&self) -> PersistedChannelKey {
        PersistedChannelKey {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: self.workspace_id.clone(),
            channel_id: self.channel_id.clone(),
            epoch: self.epoch,
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
            previous_keys: self
                .previous_keys
                .iter()
                .map(ContentKeyMaterial::persisted)
                .collect(),
        }
    }
}

fn content_key_schema_supported(schema_version: u32) -> bool {
    schema_version == 1 || schema_version == CONTENT_KEY_EXPORT_SCHEMA_VERSION
}

fn decode_workspace_key_material(
    key_id: String,
    raw_key: Vec<u8>,
) -> Result<ContentKeyMaterial, RuntimeError> {
    if raw_key.len() != WORKSPACE_KEY_LEN {
        return Err(RuntimeError::InvalidWorkspaceKey);
    }
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidWorkspaceKey)?;
    Ok(ContentKeyMaterial {
        key_id,
        content_key: ContentKey::from_bytes(bytes),
    })
}

fn content_key_from_mls_export(raw_key: Vec<u8>) -> Result<ContentKey, RuntimeError> {
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidWorkspaceKey)?;
    Ok(ContentKey::from_bytes(bytes))
}

fn decode_channel_key_material(
    key_id: String,
    raw_key: Vec<u8>,
) -> Result<ContentKeyMaterial, RuntimeError> {
    if raw_key.len() != WORKSPACE_KEY_LEN {
        return Err(RuntimeError::InvalidChannelKey);
    }
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidChannelKey)?;
    Ok(ContentKeyMaterial {
        key_id,
        content_key: ContentKey::from_bytes(bytes),
    })
}

fn derive_recovery_bundle_key(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    match kdf.name.as_str() {
        RECOVERY_BUNDLE_KDF_ARGON2ID => derive_recovery_bundle_key_argon2id(passphrase, kdf),
        RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY => derive_recovery_bundle_key_blake3(passphrase, kdf),
        _ => Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle),
    }
}

fn derive_recovery_bundle_key_argon2id(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    if kdf.context != RECOVERY_BUNDLE_KDF_CONTEXT
        || kdf.salt.len() != RECOVERY_BUNDLE_SALT_LEN
        || kdf.memory_cost_kib != RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB
        || kdf.time_cost != RECOVERY_BUNDLE_ARGON2_TIME_COST
        || kdf.parallelism != RECOVERY_BUNDLE_ARGON2_PARALLELISM
        || kdf.output_len != RECOVERY_BUNDLE_KDF_OUTPUT_LEN
    {
        return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
    }

    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(WORKSPACE_KEY_LEN),
    )
    .map_err(|error| RuntimeError::RecoveryBundleKdf(format!("{error:?}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut bytes = [0; WORKSPACE_KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, &mut bytes)
        .map_err(|error| RuntimeError::RecoveryBundleKdf(format!("{error:?}")))?;
    Ok(ContentKey::from_bytes(bytes))
}

fn derive_recovery_bundle_key_blake3(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    if kdf.context != RECOVERY_BUNDLE_KDF_CONTEXT || kdf.salt.len() != RECOVERY_BUNDLE_SALT_LEN {
        return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
    }
    let mut input = Vec::with_capacity(kdf.salt.len() + passphrase.len());
    input.extend_from_slice(&kdf.salt);
    input.extend_from_slice(passphrase.as_bytes());
    Ok(ContentKey::from_bytes(blake3::derive_key(
        RECOVERY_BUNDLE_KDF_CONTEXT,
        &input,
    )))
}

fn recovery_bundle_key_id(workspace_id: &WorkspaceId) -> String {
    format!(
        "{}:recovery:v{}",
        workspace_id.0, RECOVERY_BUNDLE_SCHEMA_VERSION
    )
}

fn recovery_bundle_aad(
    workspace_id: &WorkspaceId,
    exporter_device_id: &DeviceId,
    kdf_name: &str,
    kdf_context: &str,
    salt: &[u8],
) -> Vec<u8> {
    let salt_hash = blake3::hash(salt);
    format!(
        "chaft:v1:workspace_recovery_bundle:{}:{}:{}:{}:{}",
        workspace_id.0, exporter_device_id.0, RECOVERY_BUNDLE_SCHEMA_VERSION, kdf_name, kdf_context
    )
    .into_bytes()
    .into_iter()
    .chain(salt_hash.as_bytes().iter().copied())
    .collect()
}

fn encrypt_local_secret(
    secret_kind: &str,
    path_hint: &str,
    passphrase: &str,
    plaintext: &[u8],
) -> Result<PersistedEncryptedLocalSecret, RuntimeError> {
    let mut salt = [0; LOCAL_SECRET_SALT_LEN];
    OsRng.fill_bytes(&mut salt);
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

    Ok(PersistedEncryptedLocalSecret {
        schema_version: LOCAL_SECRET_SCHEMA_VERSION,
        storage: LOCAL_SECRET_STORAGE.to_owned(),
        secret_kind: secret_kind.to_owned(),
        path_hint: path_hint.to_owned(),
        kdf,
        sealed_payload,
    })
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

fn openmls_group_secret_kind(path: &Path) -> &'static str {
    if path.file_name().and_then(|name| name.to_str()) == Some("workspace.json") {
        LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP
    } else {
        LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP
    }
}

fn workspace_compromise_signal_from_event(
    event: &SignedEvent,
    local_device_id: &DeviceId,
) -> Option<WorkspaceCompromiseSignal> {
    if event.author_public_key.is_empty() {
        return None;
    }

    verify_self_contained_event(event)
        .err()
        .map(|error| WorkspaceCompromiseSignal {
            kind: COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE.to_owned(),
            severity: COMPROMISE_SIGNAL_SEVERITY_SUSPECTED.to_owned(),
            event_id: event.event_id.0.clone(),
            channel_id: event
                .event
                .channel_id
                .as_ref()
                .map(|channel_id| channel_id.0.clone()),
            author_device_id: event.event.author_device_id.0.clone(),
            local_device: &event.event.author_device_id == local_device_id,
            physical_ms: event.event.timestamp.physical_ms,
            reason: error.to_string(),
        })
}

fn verified_local_events_for_runtime(events: &[SignedEvent]) -> Cow<'_, [SignedEvent]> {
    let mut verified_events = Vec::new();
    let mut found_invalid = false;

    for (index, event) in events.iter().enumerate() {
        let invalid_self_contained_signature =
            !event.author_public_key.is_empty() && verify_self_contained_event(event).is_err();
        if invalid_self_contained_signature {
            if !found_invalid {
                verified_events.extend_from_slice(&events[..index]);
                found_invalid = true;
            }
        } else if found_invalid {
            verified_events.push(event.clone());
        }
    }

    if found_invalid {
        Cow::Owned(verified_events)
    } else {
        Cow::Borrowed(events)
    }
}

fn write_attachment_export_file(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    validate_runtime_path(path, "attachment output path")?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_attachment_export_temp_file(path)?;
    let result = (|| -> Result<(), RuntimeError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_attachment_export_parent_directory(parent)?;
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

    result
}

fn create_unique_attachment_export_temp_file(
    path: &Path,
) -> Result<(PathBuf, fs::File), RuntimeError> {
    let file_name = path.file_name().ok_or_else(|| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "attachment export path has no file name",
        ))
    })?;

    for _ in 0..32 {
        let counter = ATTACHMENT_EXPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut temp_file_name = OsString::from(".");
        temp_file_name.push(file_name);
        temp_file_name.push(format!(".tmp.{}.{}", process::id(), counter));
        let temp_path = path.with_file_name(temp_file_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique attachment export temp file",
    )))
}

fn sync_attachment_export_parent_directory(parent: &Path) -> Result<(), RuntimeError> {
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

fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_secret_temp_file(path)?;
    let result = (|| -> Result<(), RuntimeError> {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_secret_parent_directory(parent)?;
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

fn create_unique_secret_temp_file(path: &Path) -> Result<(PathBuf, fs::File), RuntimeError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "secret file path has no file name",
            ))
        })?;

    for _ in 0..32 {
        let counter = SECRET_FILE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
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

    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique secret temp file",
    )))
}

fn sync_secret_parent_directory(parent: &Path) -> Result<(), RuntimeError> {
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

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Barrier, Mutex,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
        },
        thread,
    };

    use async_trait::async_trait;
    use chaft_crypto::{
        CryptoError, PayloadEncryption, SealedPayload, open_attachment_blob, open_message_markdown,
        sealed_payload_from_encrypted_blob_ref,
    };
    use chaft_media::{BlobDescriptor, BlobStore, blob_hash, describe_blob};
    use chaft_net::{ChaftTransport, PeerAddress, PeerId};
    use chaft_net_direct::{DirectPeerServer, DirectTransport};
    use chaft_net_iroh::IrohTransport;
    use chaft_store::EventStore;
    use chaft_types::{ContentKeyScope, EncryptedBlobRef, EventBody};
    use tokio::sync::oneshot;

    use super::*;

    fn secret_temp_artifacts_under(root: &Path) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();
        collect_secret_temp_artifacts(root, &mut artifacts);
        artifacts.sort();
        artifacts
    }

    fn collect_secret_temp_artifacts(root: &Path, artifacts: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                collect_secret_temp_artifacts(&path, artifacts);
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

    #[derive(Clone)]
    struct CapturedBackupPublish {
        events: Vec<SignedEvent>,
        snapshot: SignedTrustSnapshot,
    }

    #[derive(Clone, Default)]
    struct CapturingBackupTransport {
        publishes: Arc<Mutex<Vec<CapturedBackupPublish>>>,
    }

    impl CapturingBackupTransport {
        fn publishes(&self) -> Vec<CapturedBackupPublish> {
            self.publishes.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ChaftTransport for CapturingBackupTransport {
        async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
            Ok(())
        }

        async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
            Ok(Vec::new())
        }

        async fn publish_event(
            &self,
            _peer: &PeerAddress,
            _event: SignedEvent,
        ) -> Result<(), NetError> {
            Err(NetError::Protocol(
                "unexpected legacy publish in capture transport".to_owned(),
            ))
        }

        async fn fetch_events(
            &self,
            _peer: &PeerAddress,
            _event_ids: Vec<EventId>,
        ) -> Result<Vec<SignedEvent>, NetError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl AuthorizedPublishTransport for CapturingBackupTransport {
        async fn publish_events_with_authorization(
            &self,
            _peer: &PeerAddress,
            events: Vec<SignedEvent>,
            authorization_events: Vec<SignedEvent>,
            mut authorization_snapshots: Vec<SignedTrustSnapshot>,
        ) -> Result<(), NetError> {
            assert!(!events.is_empty());
            assert!(events.len() <= MAX_PUBLISH_EVENTS_PER_REQUEST);
            assert!(authorization_events.is_empty());
            assert_eq!(authorization_snapshots.len(), 1);
            self.publishes.lock().unwrap().push(CapturedBackupPublish {
                events,
                snapshot: authorization_snapshots.remove(0),
            });
            Ok(())
        }
    }

    #[async_trait]
    impl BlobSyncTransport for CapturingBackupTransport {
        async fn put_blobs(
            &self,
            _peer: &PeerAddress,
            blobs: Vec<Vec<u8>>,
        ) -> Result<Vec<String>, NetError> {
            assert!(blobs.is_empty());
            Ok(Vec::new())
        }

        async fn fetch_blobs(
            &self,
            _peer: &PeerAddress,
            _hashes: Vec<String>,
        ) -> Result<HashMap<String, Vec<u8>>, NetError> {
            Ok(HashMap::new())
        }

        async fn fetch_blob_availabilities(
            &self,
            _peer: &PeerAddress,
            hashes: Vec<String>,
        ) -> Result<HashMap<String, BlobAvailability>, NetError> {
            assert!(hashes.is_empty());
            Ok(HashMap::new())
        }

        async fn put_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _bytes: Vec<u8>,
            _chunk_size: usize,
        ) -> Result<BlobDescriptor, NetError> {
            Err(NetError::Protocol(
                "unexpected chunked blob upload in capture transport".to_owned(),
            ))
        }

        async fn fetch_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _hash: &str,
        ) -> Result<Option<Vec<u8>>, NetError> {
            Ok(None)
        }
    }

    struct RemoteInventoryPublishTransport {
        inventory: Vec<EventId>,
        publish_count: AtomicUsize,
    }

    impl RemoteInventoryPublishTransport {
        fn new(inventory: Vec<EventId>) -> Self {
            Self {
                inventory,
                publish_count: AtomicUsize::new(0),
            }
        }

        fn publish_count(&self) -> usize {
            self.publish_count.load(AtomicOrdering::SeqCst)
        }
    }

    #[async_trait]
    impl ChaftTransport for RemoteInventoryPublishTransport {
        async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
            Ok(())
        }

        async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
            Ok(self.inventory.clone())
        }

        async fn publish_event(
            &self,
            _peer: &PeerAddress,
            _event: SignedEvent,
        ) -> Result<(), NetError> {
            Err(NetError::Protocol(
                "unexpected legacy publish in remote inventory transport".to_owned(),
            ))
        }

        async fn fetch_events(
            &self,
            _peer: &PeerAddress,
            _event_ids: Vec<EventId>,
        ) -> Result<Vec<SignedEvent>, NetError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl AuthorizedPublishTransport for RemoteInventoryPublishTransport {
        async fn publish_events_with_authorization(
            &self,
            _peer: &PeerAddress,
            _events: Vec<SignedEvent>,
            _authorization_events: Vec<SignedEvent>,
            _authorization_snapshots: Vec<SignedTrustSnapshot>,
        ) -> Result<(), NetError> {
            self.publish_count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl BlobSyncTransport for RemoteInventoryPublishTransport {
        async fn put_blobs(
            &self,
            _peer: &PeerAddress,
            blobs: Vec<Vec<u8>>,
        ) -> Result<Vec<String>, NetError> {
            assert!(blobs.is_empty());
            Ok(Vec::new())
        }

        async fn fetch_blobs(
            &self,
            _peer: &PeerAddress,
            _hashes: Vec<String>,
        ) -> Result<HashMap<String, Vec<u8>>, NetError> {
            Ok(HashMap::new())
        }

        async fn fetch_blob_availabilities(
            &self,
            _peer: &PeerAddress,
            hashes: Vec<String>,
        ) -> Result<HashMap<String, BlobAvailability>, NetError> {
            assert!(hashes.is_empty());
            Ok(HashMap::new())
        }

        async fn put_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _bytes: Vec<u8>,
            _chunk_size: usize,
        ) -> Result<BlobDescriptor, NetError> {
            Err(NetError::Protocol(
                "unexpected chunked blob upload in remote inventory transport".to_owned(),
            ))
        }

        async fn fetch_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _hash: &str,
        ) -> Result<Option<Vec<u8>>, NetError> {
            Ok(None)
        }
    }

    struct CountingCompleteAvailabilityTransport {
        blob_hash: String,
        fetch_count: AtomicUsize,
        error_message: Option<String>,
    }

    impl CountingCompleteAvailabilityTransport {
        fn new(blob_hash: String) -> Self {
            Self {
                blob_hash,
                fetch_count: AtomicUsize::new(0),
                error_message: None,
            }
        }

        fn failing(blob_hash: String, error_message: String) -> Self {
            Self {
                error_message: Some(error_message),
                ..Self::new(blob_hash)
            }
        }

        fn fetch_count(&self) -> usize {
            self.fetch_count.load(AtomicOrdering::SeqCst)
        }
    }

    #[async_trait]
    impl ChaftTransport for CountingCompleteAvailabilityTransport {
        async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
            Ok(())
        }

        async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
            Ok(Vec::new())
        }

        async fn publish_event(
            &self,
            _peer: &PeerAddress,
            _event: SignedEvent,
        ) -> Result<(), NetError> {
            Err(NetError::Protocol(
                "unexpected legacy publish in complete availability transport".to_owned(),
            ))
        }

        async fn fetch_events(
            &self,
            _peer: &PeerAddress,
            _event_ids: Vec<EventId>,
        ) -> Result<Vec<SignedEvent>, NetError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl BlobSyncTransport for CountingCompleteAvailabilityTransport {
        async fn put_blobs(
            &self,
            _peer: &PeerAddress,
            _blobs: Vec<Vec<u8>>,
        ) -> Result<Vec<String>, NetError> {
            Err(NetError::Protocol(
                "unexpected whole blob upload in complete availability transport".to_owned(),
            ))
        }

        async fn fetch_blobs(
            &self,
            _peer: &PeerAddress,
            _hashes: Vec<String>,
        ) -> Result<HashMap<String, Vec<u8>>, NetError> {
            Ok(HashMap::new())
        }

        async fn fetch_blob_availabilities(
            &self,
            _peer: &PeerAddress,
            hashes: Vec<String>,
        ) -> Result<HashMap<String, BlobAvailability>, NetError> {
            self.fetch_count.fetch_add(1, AtomicOrdering::SeqCst);
            assert_eq!(hashes, vec![self.blob_hash.clone()]);
            if let Some(error_message) = &self.error_message {
                return Err(NetError::Protocol(error_message.clone()));
            }
            Ok(HashMap::from([(
                self.blob_hash.clone(),
                BlobAvailability {
                    hash: self.blob_hash.clone(),
                    has_whole_blob: true,
                    descriptor: None,
                    available_chunk_hashes: Vec::new(),
                    missing_chunk_hashes: Vec::new(),
                },
            )]))
        }

        async fn put_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _bytes: Vec<u8>,
            _chunk_size: usize,
        ) -> Result<BlobDescriptor, NetError> {
            Err(NetError::Protocol(
                "unexpected chunked blob upload in complete availability transport".to_owned(),
            ))
        }

        async fn fetch_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _hash: &str,
        ) -> Result<Option<Vec<u8>>, NetError> {
            Ok(None)
        }
    }

    struct CountingWholeBlobUploadTransport {
        blob_hash: String,
        fetch_count: AtomicUsize,
        upload_count: AtomicUsize,
        fail_uploads: bool,
    }

    impl CountingWholeBlobUploadTransport {
        fn new(blob_hash: String) -> Self {
            Self {
                blob_hash,
                fetch_count: AtomicUsize::new(0),
                upload_count: AtomicUsize::new(0),
                fail_uploads: false,
            }
        }

        fn failing(blob_hash: String) -> Self {
            Self {
                fail_uploads: true,
                ..Self::new(blob_hash)
            }
        }

        fn fetch_count(&self) -> usize {
            self.fetch_count.load(AtomicOrdering::SeqCst)
        }

        fn upload_count(&self) -> usize {
            self.upload_count.load(AtomicOrdering::SeqCst)
        }
    }

    #[async_trait]
    impl ChaftTransport for CountingWholeBlobUploadTransport {
        async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
            Ok(())
        }

        async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
            Ok(Vec::new())
        }

        async fn publish_event(
            &self,
            _peer: &PeerAddress,
            _event: SignedEvent,
        ) -> Result<(), NetError> {
            Err(NetError::Protocol(
                "unexpected legacy publish in whole blob upload transport".to_owned(),
            ))
        }

        async fn fetch_events(
            &self,
            _peer: &PeerAddress,
            _event_ids: Vec<EventId>,
        ) -> Result<Vec<SignedEvent>, NetError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl BlobSyncTransport for CountingWholeBlobUploadTransport {
        async fn put_blobs(
            &self,
            _peer: &PeerAddress,
            blobs: Vec<Vec<u8>>,
        ) -> Result<Vec<String>, NetError> {
            self.upload_count.fetch_add(1, AtomicOrdering::SeqCst);
            assert_eq!(blobs.len(), 1);
            assert_eq!(blob_hash(&blobs[0]), self.blob_hash);
            if self.fail_uploads {
                return Err(NetError::Protocol(
                    "forced whole blob upload failure".to_owned(),
                ));
            }
            Ok(vec![self.blob_hash.clone()])
        }

        async fn fetch_blobs(
            &self,
            _peer: &PeerAddress,
            _hashes: Vec<String>,
        ) -> Result<HashMap<String, Vec<u8>>, NetError> {
            Ok(HashMap::new())
        }

        async fn fetch_blob_availabilities(
            &self,
            _peer: &PeerAddress,
            hashes: Vec<String>,
        ) -> Result<HashMap<String, BlobAvailability>, NetError> {
            self.fetch_count.fetch_add(1, AtomicOrdering::SeqCst);
            assert_eq!(hashes, vec![self.blob_hash.clone()]);
            Ok(HashMap::new())
        }

        async fn put_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _bytes: Vec<u8>,
            _chunk_size: usize,
        ) -> Result<BlobDescriptor, NetError> {
            Err(NetError::Protocol(
                "unexpected chunked blob upload in whole blob upload transport".to_owned(),
            ))
        }

        async fn fetch_blob_chunked(
            &self,
            _peer: &PeerAddress,
            _hash: &str,
        ) -> Result<Option<Vec<u8>>, NetError> {
            Ok(None)
        }
    }

    fn attachment_media_type_for_message(events: &[SignedEvent], event_id: &str) -> String {
        let event = events
            .iter()
            .find(|event| event.event_id.0 == event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &event.event.body else {
            panic!("expected encrypted message event");
        };
        attachments[0].media_type.clone()
    }

    fn insert_corrupt_event_json(
        data_dir: &std::path::Path,
        workspace_id: &WorkspaceId,
        event_id: &str,
    ) {
        let connection = rusqlite::Connection::open(data_dir.join("events.db")).unwrap();
        connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)
                ",
                rusqlite::params![
                    event_id,
                    workspace_id.0.as_str(),
                    "dev_corrupt",
                    1_i64,
                    0_i64,
                    1_i64,
                    b"not valid signed event json".as_slice()
                ],
            )
            .unwrap();
    }

    fn assert_oversized_peer_endpoint_error<T>(result: Result<T, RuntimeError>) {
        match result {
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "peer endpoint",
                actual_bytes,
                max_bytes: PEER_ENDPOINT_MAX_BYTES,
            }) if actual_bytes == PEER_ENDPOINT_MAX_BYTES + 1 => {}
            Ok(_) => panic!("expected oversized peer endpoint error, got ok"),
            Err(error) => panic!("expected oversized peer endpoint error, got {error}"),
        }
    }

    fn assert_unsupported_peer_endpoint_error<T>(result: Result<T, RuntimeError>) {
        match result {
            Err(RuntimeError::UnsupportedPeerEndpoint) => {}
            Ok(_) => panic!("expected unsupported peer endpoint error, got ok"),
            Err(error) => panic!("expected unsupported peer endpoint error, got {error}"),
        }
    }

    fn assert_peer_protocol_error_contains<T>(result: Result<T, RuntimeError>, expected: &str) {
        match result {
            Err(error) if error.is_peer_protocol_error() => {
                let message = error
                    .peer_protocol_error_message()
                    .expect("protocol error message should be present");
                assert!(
                    message.contains(expected),
                    "expected protocol error containing {expected:?}, got {message:?}"
                );
            }
            Ok(_) => panic!("expected peer protocol error containing {expected:?}, got ok"),
            Err(error) => {
                panic!("expected peer protocol error containing {expected:?}, got {error}")
            }
        }
    }

    fn assert_oversized_identifier_error<T>(
        result: Result<T, RuntimeError>,
        expected_field: &'static str,
        expected_max_bytes: usize,
    ) {
        match result {
            Err(RuntimeError::MetadataFieldTooLarge {
                field,
                actual_bytes,
                max_bytes,
            }) if field == expected_field
                && actual_bytes == expected_max_bytes + 1
                && max_bytes == expected_max_bytes => {}
            Ok(_) => panic!("expected oversized {expected_field} error, got ok"),
            Err(error) => panic!("expected oversized {expected_field} error, got {error}"),
        }
    }

    fn assert_oversized_metadata_file_error<T>(
        result: Result<T, RuntimeError>,
        expected_field: &'static str,
        expected_max_bytes: usize,
    ) {
        match result {
            Err(RuntimeError::MetadataFieldTooLarge {
                field,
                actual_bytes,
                max_bytes,
            }) if field == expected_field
                && actual_bytes == expected_max_bytes + 1
                && max_bytes == expected_max_bytes => {}
            Ok(_) => panic!("expected oversized {expected_field} error, got ok"),
            Err(error) => panic!("expected oversized {expected_field} error, got {error}"),
        }
    }

    fn assert_required_metadata_field_error<T>(
        result: Result<T, RuntimeError>,
        expected_field: &'static str,
    ) {
        match result {
            Err(RuntimeError::MetadataFieldRequired { field }) if field == expected_field => {}
            Ok(_) => panic!("expected required {expected_field} error, got ok"),
            Err(error) => panic!("expected required {expected_field} error, got {error}"),
        }
    }

    fn assert_oversized_runtime_path_error<T>(
        result: Result<T, RuntimeError>,
        expected_field: &'static str,
    ) {
        match result {
            Err(RuntimeError::MetadataFieldTooLarge {
                field,
                actual_bytes,
                max_bytes,
            }) if field == expected_field
                && actual_bytes > RUNTIME_PATH_MAX_BYTES
                && max_bytes == RUNTIME_PATH_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized {expected_field} error, got ok"),
            Err(error) => panic!("expected oversized {expected_field} error, got {error}"),
        }
    }

    fn assert_oversized_runtime_passphrase_error<T>(result: Result<T, RuntimeError>) {
        match result {
            Err(RuntimeError::MetadataFieldTooLarge {
                field,
                actual_bytes,
                max_bytes,
            }) if field == "identity passphrase"
                && actual_bytes > RUNTIME_PASSPHRASE_MAX_BYTES
                && max_bytes == RUNTIME_PASSPHRASE_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized identity passphrase error, got ok"),
            Err(error) => panic!("expected oversized identity passphrase error, got {error}"),
        }
    }

    #[test]
    fn runtime_open_rejects_blank_data_dir_before_filesystem_work() {
        assert_required_metadata_field_error(
            LocalRuntime::open(PathBuf::new(), None),
            "data directory",
        );
    }

    #[test]
    fn runtime_open_rejects_oversized_data_dir_before_filesystem_work() {
        assert_oversized_runtime_path_error(
            LocalRuntime::open(PathBuf::from("d".repeat(RUNTIME_PATH_MAX_BYTES + 1)), None),
            "data directory",
        );
    }

    #[test]
    fn runtime_open_rejects_oversized_identity_file_before_filesystem_work() {
        assert_oversized_runtime_path_error(
            LocalRuntime::open(
                "runtime",
                Some(PathBuf::from("i".repeat(RUNTIME_PATH_MAX_BYTES + 1))),
            ),
            "identity file",
        );
    }

    #[test]
    fn runtime_open_treats_blank_identity_passphrase_as_absent() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some(" \t\n "))
                .unwrap();

        assert!(runtime.identity_passphrase.is_none());
        let identity_json = fs::read_to_string(tempdir.path().join("device.json")).unwrap();
        assert!(identity_json.contains("ed25519_signing_key_hex"));
        assert!(!identity_json.contains("argon2id-aes-256-gcm-siv"));
    }

    #[test]
    fn runtime_open_rejects_oversized_identity_passphrase_before_filesystem_work() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = tempdir.path().join("runtime");
        let passphrase = "p".repeat(RUNTIME_PASSPHRASE_MAX_BYTES + 1);

        assert_oversized_runtime_passphrase_error(LocalRuntime::open_with_identity_passphrase(
            &data_dir,
            None,
            Some(&passphrase),
        ));
        assert!(!data_dir.exists());
    }

    #[test]
    fn runtime_open_rejects_oversized_derived_paths_before_filesystem_work() {
        assert_oversized_runtime_path_error(
            LocalRuntime::open(PathBuf::from("d".repeat(RUNTIME_PATH_MAX_BYTES)), None),
            "identity file",
        );
    }

    #[test]
    fn runtime_persists_identity_and_creates_encrypted_workspace_events() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let first_device_id = runtime.device_id().clone();
        let created = runtime
            .create_workspace("Chaft Runtime", "general")
            .unwrap();
        let sent = runtime
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id.clone()),
                "private runtime message",
            )
            .unwrap();

        assert!(sent.encrypted);

        let reopened = LocalRuntime::open(tempdir.path(), None).unwrap();
        assert_eq!(reopened.device_id(), &first_device_id);

        let events = reopened
            .workspace_events(&WorkspaceId(created.workspace_id.clone()))
            .unwrap();
        let events_json = serde_json::to_string(&events).unwrap();

        assert_eq!(events.len(), 3);
        assert!(!events_json.contains("private runtime message"));
        assert!(events_json.contains("aes256_gcm_siv"));
        assert_eq!(events[1].event.parents, vec![events[0].event_id.clone()]);
        assert_eq!(events[2].event.parents, vec![events[1].event_id.clone()]);

        let snapshot = reopened
            .workspace_snapshot(WorkspaceId(created.workspace_id))
            .unwrap();

        assert_eq!(snapshot.name, "Chaft Runtime");
        assert_eq!(snapshot.channels[0].name, "general");
        assert_eq!(snapshot.timeline[0].body, "Encrypted message");
        assert!(snapshot.timeline[0].encrypted);
    }

    #[test]
    fn runtime_detects_invalid_signature_compromise_signals() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Signals", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        let clean = runtime
            .detect_workspace_compromise_signals(workspace_id.clone())
            .unwrap();
        assert!(!clean.has_signals);
        assert_eq!(clean.recommended_action, None);

        let sent = runtime
            .send_message(workspace_id.clone(), channel_id, "signed before tamper")
            .unwrap();
        let mut forged = runtime
            .workspace_events(&workspace_id)
            .unwrap()
            .into_iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        forged.signature[0] ^= 1;
        let forged = SignedEvent::from_author_signature(
            forged.event,
            forged.author_public_key,
            forged.signature,
        );
        runtime.store.append_event(&forged).unwrap();

        let report = runtime
            .detect_workspace_compromise_signals(workspace_id.clone())
            .unwrap();
        assert!(report.has_signals);
        assert_eq!(report.signal_count, 1);
        assert_eq!(report.invalid_signature_count, 1);
        assert_eq!(report.local_device_signal_count, 1);
        assert!(report.should_rotate_local_secret_state);
        assert_eq!(
            report.recommended_action.as_deref(),
            Some(COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE)
        );
        assert_eq!(report.signals[0].event_id, forged.event_id.0);
        assert_eq!(
            report.signals[0].kind,
            COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE
        );
        assert!(report.signals[0].local_device);
        assert_eq!(report.signals[0].reason, "invalid signature");
    }

    #[test]
    fn runtime_reports_remote_invalid_signature_without_local_rotation_trigger() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Remote Signals", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let remote_identity = DeviceIdentity::generate();

        let mut remote_event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            remote_identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged remote".to_owned(),
                attachments: Vec::new(),
            },
        );
        remote_event.parents = vec![EventId(created.channel_event_id)];
        let mut forged = remote_identity.sign_event(remote_event);
        forged.signature[0] ^= 1;
        let forged = SignedEvent::from_author_signature(
            forged.event,
            forged.author_public_key,
            forged.signature,
        );
        runtime.store.append_event(&forged).unwrap();

        let report = runtime
            .detect_workspace_compromise_signals(workspace_id)
            .unwrap();
        assert_eq!(report.signal_count, 1);
        assert_eq!(report.local_device_signal_count, 0);
        assert!(!report.should_rotate_local_secret_state);
        assert_eq!(
            report.recommended_action.as_deref(),
            Some(COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES)
        );
        assert!(!report.signals[0].local_device);
        assert_eq!(
            report.signals[0].author_device_id,
            remote_identity.device_id().0
        );
    }

    #[test]
    fn runtime_responds_to_local_compromise_signal_once() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Respond Signals", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "local signal before response",
            )
            .unwrap();
        let mut forged = runtime
            .workspace_events(&workspace_id)
            .unwrap()
            .into_iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        forged.signature[0] ^= 1;
        let forged = SignedEvent::from_author_signature(
            forged.event,
            forged.author_public_key,
            forged.signature,
        );
        runtime.store.append_event(&forged).unwrap();

        let first_response = runtime
            .respond_to_workspace_compromise_signals(workspace_id.clone())
            .unwrap();
        assert!(first_response.rotated_local_secret_state);
        assert_eq!(
            first_response.action_taken.as_deref(),
            Some(COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE)
        );
        assert_eq!(
            first_response.responded_signal_event_ids,
            vec![forged.event_id.0.clone()]
        );
        assert!(first_response.already_handled_signal_event_ids.is_empty());
        assert!(first_response.skipped_reason.is_none());
        assert!(first_response.rotation.is_some());

        let event_count_after_first_response =
            runtime.workspace_events(&workspace_id).unwrap().len();
        let second_response = runtime
            .respond_to_workspace_compromise_signals(workspace_id.clone())
            .unwrap();
        assert!(!second_response.rotated_local_secret_state);
        assert!(second_response.action_taken.is_none());
        assert_eq!(
            second_response.skipped_reason.as_deref(),
            Some(COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED)
        );
        assert!(second_response.responded_signal_event_ids.is_empty());
        assert_eq!(
            second_response.already_handled_signal_event_ids,
            vec![forged.event_id.0]
        );
        assert_eq!(
            runtime.workspace_events(&workspace_id).unwrap().len(),
            event_count_after_first_response
        );
    }

    #[test]
    fn runtime_compromise_response_does_not_rotate_for_remote_signal() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Remote Response Signals", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let remote_identity = DeviceIdentity::generate();

        let mut remote_event = SignableEvent::new(
            workspace_id.clone(),
            Some(ChannelId(created.channel_id.clone())),
            remote_identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "remote signal".to_owned(),
                attachments: Vec::new(),
            },
        );
        remote_event.parents = vec![EventId(created.channel_event_id)];
        let mut forged = remote_identity.sign_event(remote_event);
        forged.signature[0] ^= 1;
        let forged = SignedEvent::from_author_signature(
            forged.event,
            forged.author_public_key,
            forged.signature,
        );
        runtime.store.append_event(&forged).unwrap();
        let event_count_before_response = runtime.workspace_events(&workspace_id).unwrap().len();

        let response = runtime
            .respond_to_workspace_compromise_signals(workspace_id.clone())
            .unwrap();
        assert!(!response.rotated_local_secret_state);
        assert!(response.action_taken.is_none());
        assert_eq!(
            response.skipped_reason.as_deref(),
            Some(COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW)
        );
        assert!(response.responded_signal_event_ids.is_empty());
        assert!(response.already_handled_signal_event_ids.is_empty());
        assert_eq!(response.report.signal_count, 1);
        assert_eq!(response.report.local_device_signal_count, 0);
        assert_eq!(
            runtime.workspace_events(&workspace_id).unwrap().len(),
            event_count_before_response
        );
    }

    #[test]
    fn runtime_persists_in_progress_blob_transfer_attempts() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("backup-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };

        let started = runtime
            .record_blob_transfer_started(
                "wrk_transfer",
                &peer,
                "blob_hash",
                BlobTransferMode::ChunkedBlob,
                8,
                Some(4),
                vec!["chunk_a".to_owned(), "chunk_b".to_owned()],
                vec!["chunk_b".to_owned()],
                vec!["chunk_a".to_owned()],
            )
            .unwrap();

        assert_eq!(started.status, BlobTransferStatus::InProgress);
        assert!(started.finished_at_unix_ms.is_none());
        assert_eq!(started.chunk_count, 2);
        assert_eq!(started.planned_chunk_count, 1);
        assert_eq!(started.remote_available_chunk_count, 1);
        let reopened = LocalRuntime::open(tempdir.path(), None).unwrap();
        assert_eq!(
            reopened.blob_transfer_ledger().unwrap().entries,
            vec![started.clone()]
        );

        let failed = reopened
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some("network unavailable".to_owned()),
            )
            .unwrap();
        assert_eq!(failed.status, BlobTransferStatus::Failed);
        assert_eq!(failed.error.as_deref(), Some("network unavailable"));
        assert_eq!(
            LocalRuntime::open(tempdir.path(), None)
                .unwrap()
                .blob_transfer_ledger()
                .unwrap()
                .entries,
            vec![failed]
        );
    }

    #[test]
    fn runtime_caps_blob_transfer_ledger_entries_after_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let entry_count = BLOB_TRANSFER_LEDGER_MAX_ENTRIES + 3;
        let entries = (0..entry_count)
            .map(|index| BlobTransferAttempt {
                attempt_id: format!("attempt-{index}"),
                workspace_id: "wrk_transfer".to_owned(),
                peer_id: "backup-node".to_owned(),
                peer_endpoint: "127.0.0.1:7777".to_owned(),
                blob_hash: format!("blob-{index}"),
                mode: BlobTransferMode::ChunkedBlob,
                status: BlobTransferStatus::Failed,
                attempt_count: index as u32,
                total_byte_len: 8,
                chunk_size: Some(4),
                chunk_count: 999,
                chunk_hashes: vec![format!("chunk-{index}-a"), format!("chunk-{index}-b")],
                planned_chunk_count: 999,
                planned_chunk_hashes: vec![format!("chunk-{index}-b")],
                remote_available_chunk_count: 999,
                remote_available_chunk_hashes: vec![format!("chunk-{index}-a")],
                started_at_unix_ms: index as u64,
                finished_at_unix_ms: Some(index as u64 + 1),
                error: Some("needs retry".to_owned()),
            })
            .collect::<Vec<_>>();
        let ledger = BlobTransferLedger {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries,
        };
        fs::write(
            runtime.paths().blob_transfer_ledger.clone(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let read = runtime.blob_transfer_ledger().unwrap();

        assert_eq!(read.entries.len(), BLOB_TRANSFER_LEDGER_MAX_ENTRIES);
        assert_eq!(read.entries[0].attempt_id, "attempt-3");
        assert_eq!(
            read.entries[BLOB_TRANSFER_LEDGER_MAX_ENTRIES - 1].attempt_id,
            format!("attempt-{}", entry_count - 1)
        );
        assert_eq!(read.entries[0].chunk_count, 2);
        assert_eq!(read.entries[0].planned_chunk_count, 1);
        assert_eq!(read.entries[0].remote_available_chunk_count, 1);
    }

    #[test]
    fn runtime_caps_blob_transfer_ledger_chunk_lists_after_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let chunk_count = BLOB_DESCRIPTOR_MAX_CHUNKS + 3;
        let chunk_hashes = (0..chunk_count)
            .map(|index| format!("chunk-{index}"))
            .collect::<Vec<_>>();
        let planned_chunk_hashes = (0..chunk_count)
            .map(|index| format!("planned-{index}"))
            .collect::<Vec<_>>();
        let remote_available_chunk_hashes = (0..chunk_count)
            .map(|index| format!("remote-{index}"))
            .collect::<Vec<_>>();
        let ledger = BlobTransferLedger {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: vec![BlobTransferAttempt {
                attempt_id: "attempt-big-chunks".to_owned(),
                workspace_id: "wrk_transfer".to_owned(),
                peer_id: "backup-node".to_owned(),
                peer_endpoint: "127.0.0.1:7777".to_owned(),
                blob_hash: "blob-big-chunks".to_owned(),
                mode: BlobTransferMode::ChunkedBlob,
                status: BlobTransferStatus::Failed,
                attempt_count: 1,
                total_byte_len: 8,
                chunk_size: Some(4),
                chunk_count: chunk_count + 100,
                chunk_hashes,
                planned_chunk_count: chunk_count + 100,
                planned_chunk_hashes,
                remote_available_chunk_count: chunk_count + 100,
                remote_available_chunk_hashes,
                started_at_unix_ms: 1,
                finished_at_unix_ms: Some(2),
                error: Some("needs retry".to_owned()),
            }],
        };
        fs::write(
            runtime.paths().blob_transfer_ledger.clone(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let read = runtime.blob_transfer_ledger().unwrap();
        let entry = &read.entries[0];

        assert_eq!(entry.chunk_count, BLOB_DESCRIPTOR_MAX_CHUNKS);
        assert_eq!(entry.chunk_hashes.len(), BLOB_DESCRIPTOR_MAX_CHUNKS);
        assert_eq!(
            entry.chunk_hashes[BLOB_DESCRIPTOR_MAX_CHUNKS - 1],
            format!("chunk-{}", BLOB_DESCRIPTOR_MAX_CHUNKS - 1)
        );
        assert_eq!(entry.planned_chunk_count, BLOB_DESCRIPTOR_MAX_CHUNKS);
        assert_eq!(entry.planned_chunk_hashes.len(), BLOB_DESCRIPTOR_MAX_CHUNKS);
        assert_eq!(
            entry.remote_available_chunk_count,
            BLOB_DESCRIPTOR_MAX_CHUNKS
        );
        assert_eq!(
            entry.remote_available_chunk_hashes.len(),
            BLOB_DESCRIPTOR_MAX_CHUNKS
        );
    }

    #[test]
    fn runtime_caps_blob_transfer_error_strings() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("backup-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let started = runtime
            .record_blob_transfer_started(
                "wrk_transfer",
                &peer,
                "blob_hash",
                BlobTransferMode::WholeBlob,
                8,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let oversized_error = "é".repeat(BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);

        let failed = runtime
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some(oversized_error.clone()),
            )
            .unwrap();

        let written_error = failed.error.as_ref().unwrap();
        assert_eq!(written_error.len(), BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
        assert!(written_error.is_char_boundary(written_error.len()));
        assert_eq!(
            runtime.blob_transfer_ledger().unwrap().entries[0]
                .error
                .as_ref()
                .unwrap()
                .len(),
            BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES
        );

        let ledger = BlobTransferLedger {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: vec![BlobTransferAttempt {
                attempt_id: "attempt-big-error".to_owned(),
                workspace_id: "wrk_transfer".to_owned(),
                peer_id: "backup-node".to_owned(),
                peer_endpoint: "127.0.0.1:7777".to_owned(),
                blob_hash: "blob-big-error".to_owned(),
                mode: BlobTransferMode::WholeBlob,
                status: BlobTransferStatus::Failed,
                attempt_count: 1,
                total_byte_len: 8,
                chunk_size: None,
                chunk_count: 0,
                chunk_hashes: Vec::new(),
                planned_chunk_count: 0,
                planned_chunk_hashes: Vec::new(),
                remote_available_chunk_count: 0,
                remote_available_chunk_hashes: Vec::new(),
                started_at_unix_ms: 1,
                finished_at_unix_ms: Some(2),
                error: Some(oversized_error),
            }],
        };
        fs::write(
            runtime.paths().blob_transfer_ledger.clone(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let read_error = runtime.blob_transfer_ledger().unwrap().entries[0]
            .error
            .clone()
            .unwrap();
        assert_eq!(read_error.len(), BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
        assert!(read_error.is_char_boundary(read_error.len()));
    }

    #[test]
    fn runtime_caps_blob_transfer_ledger_identifying_strings_after_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let oversized_chunk_hash = "é".repeat(ATTACHMENT_BLOB_HASH_MAX_BYTES);
        let ledger = BlobTransferLedger {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: vec![BlobTransferAttempt {
                attempt_id: "é".repeat(BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES),
                workspace_id: "w".repeat(WORKSPACE_ID_MAX_BYTES + 3),
                peer_id: "p".repeat(PEER_ENDPOINT_ID_MAX_BYTES + 3),
                peer_endpoint: "e".repeat(PEER_ENDPOINT_MAX_BYTES + 3),
                blob_hash: "b".repeat(ATTACHMENT_BLOB_HASH_MAX_BYTES + 3),
                mode: BlobTransferMode::ChunkedBlob,
                status: BlobTransferStatus::Failed,
                attempt_count: 1,
                total_byte_len: 8,
                chunk_size: Some(4),
                chunk_count: 1,
                chunk_hashes: vec![oversized_chunk_hash.clone()],
                planned_chunk_count: 1,
                planned_chunk_hashes: vec![oversized_chunk_hash.clone()],
                remote_available_chunk_count: 1,
                remote_available_chunk_hashes: vec![oversized_chunk_hash],
                started_at_unix_ms: 1,
                finished_at_unix_ms: Some(2),
                error: Some("needs retry".to_owned()),
            }],
        };
        fs::write(
            runtime.paths().blob_transfer_ledger.clone(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let read = runtime.blob_transfer_ledger().unwrap();
        let entry = &read.entries[0];

        assert!(entry.attempt_id.len() <= BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES);
        assert!(entry.attempt_id.is_char_boundary(entry.attempt_id.len()));
        assert_eq!(entry.workspace_id.len(), WORKSPACE_ID_MAX_BYTES);
        assert_eq!(entry.peer_id.len(), PEER_ENDPOINT_ID_MAX_BYTES);
        assert_eq!(entry.peer_endpoint.len(), PEER_ENDPOINT_MAX_BYTES);
        assert_eq!(entry.blob_hash.len(), ATTACHMENT_BLOB_HASH_MAX_BYTES);
        assert_eq!(entry.chunk_hashes[0].len(), ATTACHMENT_BLOB_HASH_MAX_BYTES);
        assert!(entry.chunk_hashes[0].is_char_boundary(entry.chunk_hashes[0].len()));
        assert_eq!(
            entry.planned_chunk_hashes[0].len(),
            ATTACHMENT_BLOB_HASH_MAX_BYTES
        );
        assert!(
            entry.planned_chunk_hashes[0].is_char_boundary(entry.planned_chunk_hashes[0].len())
        );
        assert_eq!(
            entry.remote_available_chunk_hashes[0].len(),
            ATTACHMENT_BLOB_HASH_MAX_BYTES
        );
        assert!(
            entry.remote_available_chunk_hashes[0]
                .is_char_boundary(entry.remote_available_chunk_hashes[0].len())
        );
    }

    #[test]
    fn runtime_clears_whole_blob_transfer_chunk_metadata_after_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let ledger = BlobTransferLedger {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: vec![BlobTransferAttempt {
                attempt_id: "attempt-whole-with-chunks".to_owned(),
                workspace_id: "wrk_transfer".to_owned(),
                peer_id: "backup-node".to_owned(),
                peer_endpoint: "127.0.0.1:7777".to_owned(),
                blob_hash: "blob-with-fake-chunks".to_owned(),
                mode: BlobTransferMode::WholeBlob,
                status: BlobTransferStatus::Failed,
                attempt_count: 1,
                total_byte_len: 8,
                chunk_size: Some(4),
                chunk_count: 2,
                chunk_hashes: vec!["chunk-a".to_owned(), "chunk-b".to_owned()],
                planned_chunk_count: 1,
                planned_chunk_hashes: vec!["chunk-b".to_owned()],
                remote_available_chunk_count: 1,
                remote_available_chunk_hashes: vec!["chunk-a".to_owned()],
                started_at_unix_ms: 1,
                finished_at_unix_ms: Some(2),
                error: Some("needs retry".to_owned()),
            }],
        };
        fs::write(
            runtime.paths().blob_transfer_ledger.clone(),
            serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();

        let read = runtime.blob_transfer_ledger().unwrap();
        let entry = &read.entries[0];

        assert_eq!(entry.mode, BlobTransferMode::WholeBlob);
        assert_eq!(entry.chunk_size, None);
        assert_eq!(entry.chunk_count, 0);
        assert!(entry.chunk_hashes.is_empty());
        assert_eq!(entry.planned_chunk_count, 0);
        assert!(entry.planned_chunk_hashes.is_empty());
        assert_eq!(entry.remote_available_chunk_count, 0);
        assert!(entry.remote_available_chunk_hashes.is_empty());
    }

    #[test]
    fn runtime_secret_file_concurrent_writes_use_unique_temp_files() {
        let tempdir = tempfile::tempdir().unwrap();
        let secret_path = tempdir.path().join("secret.json");
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|index| {
                let secret_path = secret_path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    write_secret_file(&secret_path, format!("secret {index}").as_bytes()).unwrap();
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        let contents = fs::read_to_string(&secret_path).unwrap();
        assert!(contents.starts_with("secret "));
        assert!(contents.ends_with('\n'));
        assert!(secret_temp_artifacts_under(tempdir.path()).is_empty());
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&secret_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn runtime_rejects_oversized_blob_transfer_ledger_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let file = fs::File::create(&runtime.paths().blob_transfer_ledger).unwrap();
        file.set_len(BLOB_TRANSFER_LEDGER_MAX_BYTES as u64 + 1)
            .unwrap();

        assert_oversized_metadata_file_error(
            runtime.blob_transfer_ledger(),
            "blob transfer ledger",
            BLOB_TRANSFER_LEDGER_MAX_BYTES,
        );
    }

    #[test]
    fn runtime_rejects_oversized_compromise_response_ledger_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let file = fs::File::create(&runtime.paths().compromise_response_ledger).unwrap();
        file.set_len(COMPROMISE_RESPONSE_LEDGER_MAX_BYTES as u64 + 1)
            .unwrap();

        assert_oversized_metadata_file_error(
            runtime.read_compromise_response_ledger(),
            "compromise response ledger",
            COMPROMISE_RESPONSE_LEDGER_MAX_BYTES,
        );
    }

    #[test]
    fn runtime_rejects_oversized_blob_transfer_peer_id_before_ledger_write() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("p".repeat(PEER_ENDPOINT_ID_MAX_BYTES + 1)),
            endpoint: "127.0.0.1:7777".to_owned(),
        };

        match runtime.record_blob_transfer_started(
            "wrk_transfer",
            &peer,
            "blob_hash",
            BlobTransferMode::WholeBlob,
            8,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ) {
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "peer ID",
                actual_bytes,
                max_bytes: PEER_ENDPOINT_ID_MAX_BYTES,
            }) if actual_bytes == PEER_ENDPOINT_ID_MAX_BYTES + 1 => {}
            Ok(_) => panic!("expected oversized peer ID error, got ok"),
            Err(error) => panic!("expected oversized peer ID error, got {error}"),
        }

        assert!(runtime.blob_transfer_ledger().unwrap().entries.is_empty());
    }

    #[test]
    fn planned_chunk_upload_records_remote_chunks_in_descriptor_order() {
        let mut bytes = vec![0; (DIRECT_BLOB_CHUNK_SIZE * 2) + 512];
        for (index, chunk) in bytes.chunks_mut(DIRECT_BLOB_CHUNK_SIZE).enumerate() {
            chunk.fill(index as u8);
        }
        let descriptor = describe_blob(&bytes, DIRECT_BLOB_CHUNK_SIZE);
        let expected_chunk_hashes = descriptor.chunk_hashes.clone();
        let remote_availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: vec![
                expected_chunk_hashes[2].clone(),
                expected_chunk_hashes[0].clone(),
            ],
            missing_chunk_hashes: vec![expected_chunk_hashes[1].clone()],
        };

        let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
            planned_chunk_upload(&bytes, Some(&remote_availability));

        assert_eq!(chunk_size, DIRECT_BLOB_CHUNK_SIZE as u64);
        assert_eq!(chunk_hashes, expected_chunk_hashes.clone());
        assert_eq!(planned_chunk_hashes, vec![expected_chunk_hashes[1].clone()]);
        assert_eq!(
            remote_available_chunk_hashes,
            vec![
                expected_chunk_hashes[0].clone(),
                expected_chunk_hashes[2].clone()
            ]
        );
    }

    #[test]
    fn planned_chunk_upload_ignores_invalid_remote_availability() {
        let mut bytes = vec![0; (DIRECT_BLOB_CHUNK_SIZE * 2) + 512];
        for (index, chunk) in bytes.chunks_mut(DIRECT_BLOB_CHUNK_SIZE).enumerate() {
            chunk.fill(index as u8);
        }
        let descriptor = describe_blob(&bytes, DIRECT_BLOB_CHUNK_SIZE);
        let expected_chunk_hashes = descriptor.chunk_hashes.clone();
        let remote_availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: vec![
                expected_chunk_hashes[0].clone(),
                blob_hash(b"foreign runtime availability chunk"),
            ],
            missing_chunk_hashes: expected_chunk_hashes[1..].to_vec(),
        };

        let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
            planned_chunk_upload(&bytes, Some(&remote_availability));

        assert_eq!(chunk_size, DIRECT_BLOB_CHUNK_SIZE as u64);
        assert_eq!(chunk_hashes, expected_chunk_hashes.clone());
        assert_eq!(planned_chunk_hashes, expected_chunk_hashes);
        assert!(remote_available_chunk_hashes.is_empty());
    }

    #[test]
    fn planned_chunk_upload_records_repeated_chunk_hash_once() {
        let bytes = vec![7; DIRECT_BLOB_CHUNK_SIZE * 2];
        let descriptor = describe_blob(&bytes, DIRECT_BLOB_CHUNK_SIZE);
        assert_eq!(descriptor.chunk_hashes.len(), 2);
        assert_eq!(descriptor.chunk_hashes[0], descriptor.chunk_hashes[1]);

        let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
            planned_chunk_upload(&bytes, None);

        assert_eq!(chunk_size, DIRECT_BLOB_CHUNK_SIZE as u64);
        assert_eq!(chunk_hashes, descriptor.chunk_hashes.clone());
        assert_eq!(
            planned_chunk_hashes,
            vec![descriptor.chunk_hashes[0].clone()]
        );
        assert!(remote_available_chunk_hashes.is_empty());

        let remote_availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: descriptor.chunk_hashes.clone(),
            missing_chunk_hashes: Vec::new(),
        };
        let (_, _, planned_chunk_hashes, remote_available_chunk_hashes) =
            planned_chunk_upload(&bytes, Some(&remote_availability));

        assert!(planned_chunk_hashes.is_empty());
        assert_eq!(
            remote_available_chunk_hashes,
            vec![descriptor.chunk_hashes[0].clone()]
        );
    }

    #[test]
    fn planned_chunk_upload_ignores_remote_chunks_from_mismatched_descriptor() {
        let mut bytes = vec![0; (DIRECT_BLOB_CHUNK_SIZE * 2) + 512];
        for (index, chunk) in bytes.chunks_mut(DIRECT_BLOB_CHUNK_SIZE).enumerate() {
            chunk.fill(index as u8);
        }
        let descriptor = describe_blob(&bytes, DIRECT_BLOB_CHUNK_SIZE);
        let expected_chunk_hashes = descriptor.chunk_hashes.clone();
        let mut mismatched_descriptor = descriptor.clone();
        mismatched_descriptor.chunk_hashes[1] = blob_hash(b"wrong runtime descriptor chunk");
        let remote_availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(mismatched_descriptor.clone()),
            available_chunk_hashes: vec![mismatched_descriptor.chunk_hashes[0].clone()],
            missing_chunk_hashes: mismatched_descriptor.chunk_hashes[1..].to_vec(),
        };

        let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
            planned_chunk_upload(&bytes, Some(&remote_availability));

        assert_eq!(chunk_size, DIRECT_BLOB_CHUNK_SIZE as u64);
        assert_eq!(chunk_hashes, expected_chunk_hashes.clone());
        assert_eq!(planned_chunk_hashes, expected_chunk_hashes);
        assert!(remote_available_chunk_hashes.is_empty());
    }

    #[test]
    fn ordered_retry_peers_preserves_first_endpoint_occurrence() {
        let first = PeerAddress {
            peer_id: PeerId("first".to_owned()),
            endpoint: "127.0.0.1:7001".to_owned(),
        };
        let duplicate = PeerAddress {
            peer_id: PeerId("duplicate".to_owned()),
            endpoint: first.endpoint.clone(),
        };
        let second = PeerAddress {
            peer_id: PeerId("second".to_owned()),
            endpoint: "127.0.0.1:7002".to_owned(),
        };
        let peers = vec![first.clone(), duplicate, second.clone()];

        let ordered = ordered_retry_peers(&peers)
            .into_iter()
            .map(|peer| peer.peer_id.0.clone())
            .collect::<Vec<_>>();

        assert_eq!(ordered, vec![first.peer_id.0, second.peer_id.0]);
    }

    #[test]
    fn runtime_reopens_passphrase_encrypted_identity_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open_with_identity_passphrase(
            tempdir.path(),
            None,
            Some("identity pass"),
        )
        .unwrap();
        let first_device_id = runtime.device_id().clone();
        let created = runtime
            .create_workspace("Encrypted Identity Runtime", "general")
            .unwrap();
        let identity_json = fs::read_to_string(tempdir.path().join("device.json")).unwrap();

        assert!(identity_json.contains("argon2id-aes-256-gcm-siv"));
        assert!(!identity_json.contains("ed25519_signing_key_hex"));

        let reopened = LocalRuntime::open_with_identity_passphrase(
            tempdir.path(),
            None,
            Some("identity pass"),
        )
        .unwrap();
        assert_eq!(reopened.device_id(), &first_device_id);
        assert_eq!(reopened.list_workspaces().unwrap().len(), 1);

        assert!(matches!(
            LocalRuntime::open(tempdir.path(), None),
            Err(RuntimeError::Identity(
                IdentityError::EncryptedIdentityPassphraseRequired
            ))
        ));
        assert!(matches!(
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("wrong pass")),
            Err(RuntimeError::Identity(IdentityError::Crypto(
                CryptoError::OpenFailed
            )))
        ));
        assert!(!created.workspace_id.is_empty());
    }

    #[test]
    fn runtime_encrypts_manual_key_files_when_identity_passphrase_is_supplied() {
        let tempdir = tempfile::tempdir().unwrap();
        LocalRuntime::open(tempdir.path(), None).unwrap();
        let runtime =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("key pass"))
                .unwrap();
        let created = runtime
            .create_workspace("Encrypted Key Files", "general")
            .unwrap();
        let private_channel = runtime
            .create_channel(WorkspaceId(created.workspace_id.clone()), "strategy", true)
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel_id = ChannelId(private_channel.channel_id.clone());
        let workspace_key_path = runtime.workspace_key_path(&workspace_id);
        let channel_key_path = runtime.channel_key_path(&workspace_id, &private_channel_id);
        let workspace_key_json = fs::read_to_string(&workspace_key_path).unwrap();
        let channel_key_json = fs::read_to_string(&channel_key_path).unwrap();

        assert!(workspace_key_json.contains(LOCAL_SECRET_STORAGE));
        assert!(workspace_key_json.contains(LOCAL_SECRET_KIND_WORKSPACE_KEY));
        assert!(!workspace_key_json.contains("aes_256_gcm_siv_key"));
        assert!(channel_key_json.contains(LOCAL_SECRET_STORAGE));
        assert!(channel_key_json.contains(LOCAL_SECRET_KIND_CHANNEL_KEY));
        assert!(!channel_key_json.contains("aes_256_gcm_siv_key"));

        let reopened =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("key pass"))
                .unwrap();
        let exported = reopened.export_workspace_key(workspace_id.clone()).unwrap();
        assert_eq!(exported.workspace_id, workspace_id.0);
        let exported_channel = reopened
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        assert_eq!(exported_channel.channel_id, private_channel_id.0);

        let missing_passphrase = LocalRuntime::open(tempdir.path(), None).unwrap();
        assert!(matches!(
            missing_passphrase.export_workspace_key(workspace_id.clone()),
            Err(RuntimeError::LocalSecretPassphraseRequired)
        ));

        let wrong_passphrase =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("wrong pass"))
                .unwrap();
        assert!(matches!(
            wrong_passphrase.export_workspace_key(workspace_id),
            Err(RuntimeError::Crypto(CryptoError::OpenFailed))
        ));
    }

    #[test]
    fn runtime_rejects_oversized_local_secret_file_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Oversized Local Secret", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let workspace_key_path = runtime.workspace_key_path(&workspace_id);
        let file = fs::File::create(&workspace_key_path).unwrap();
        file.set_len(LOCAL_SECRET_FILE_MAX_BYTES as u64 + 1)
            .unwrap();

        assert_oversized_metadata_file_error(
            runtime.export_workspace_key(workspace_id),
            "local secret file",
            LOCAL_SECRET_FILE_MAX_BYTES,
        );
    }

    #[test]
    fn runtime_encrypts_openmls_private_state_when_identity_passphrase_is_supplied() {
        let tempdir = tempfile::tempdir().unwrap();
        LocalRuntime::open(tempdir.path(), None).unwrap();
        let runtime =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("mls pass"))
                .unwrap();
        let created = runtime
            .create_workspace("Encrypted OpenMLS State", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let package = runtime
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        let group = runtime
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let private_bundle_path =
            runtime.openmls_key_package_path(&workspace_id, &package.key_package_ref);
        let private_group_path = runtime.openmls_workspace_group_path(&workspace_id);
        let private_bundle_json = fs::read_to_string(&private_bundle_path).unwrap();
        let private_group_json = fs::read_to_string(&private_group_path).unwrap();

        assert!(private_bundle_json.contains(LOCAL_SECRET_STORAGE));
        assert!(private_bundle_json.contains(LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE));
        assert!(!private_bundle_json.contains("signatureKeyPair"));
        assert!(!private_bundle_json.contains("keyPackageBundle"));
        assert!(private_group_json.contains(LOCAL_SECRET_STORAGE));
        assert!(private_group_json.contains(LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP));
        assert!(!private_group_json.contains("signatureKeyPair"));
        assert!(!private_group_json.contains("storageEntries"));

        let reopened =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("mls pass"))
                .unwrap();
        let updated = reopened
            .update_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        assert_eq!(updated.workspace_id, workspace_id.0);

        let missing_passphrase = LocalRuntime::open(tempdir.path(), None).unwrap();
        assert!(matches!(
            missing_passphrase.update_openmls_workspace_group(workspace_id.clone()),
            Err(RuntimeError::LocalSecretPassphraseRequired)
        ));

        let wrong_passphrase =
            LocalRuntime::open_with_identity_passphrase(tempdir.path(), None, Some("wrong pass"))
                .unwrap();
        assert!(matches!(
            wrong_passphrase.update_openmls_workspace_group(workspace_id),
            Err(RuntimeError::Crypto(CryptoError::OpenFailed))
        ));
        assert_eq!(group.epoch, 0);
    }

    #[test]
    fn runtime_lists_local_workspaces_for_desktop_switching() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let first = runtime
            .create_workspace("First Workspace", "general")
            .unwrap();
        let second = runtime.create_workspace("Second Workspace", "ops").unwrap();
        runtime
            .send_message(
                WorkspaceId(first.workspace_id.clone()),
                ChannelId(first.channel_id),
                "first message",
            )
            .unwrap();

        let summaries = runtime.list_workspaces().unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].workspace_id, first.workspace_id);
        assert_eq!(summaries[0].name, "First Workspace");
        assert_eq!(summaries[0].channel_count, 1);
        assert_eq!(summaries[0].member_count, 1);
        assert_eq!(summaries[0].event_count, 3);
        assert!(summaries[0].has_workspace_key);
        assert_eq!(summaries[1].workspace_id, second.workspace_id);
        assert_eq!(summaries[1].name, "Second Workspace");
        assert_eq!(summaries[1].channel_count, 1);
        assert_eq!(summaries[1].event_count, 2);
    }

    #[test]
    fn runtime_pages_workspace_summaries_without_materializing_every_workspace() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let first = runtime
            .create_workspace("First Workspace", "general")
            .unwrap();
        let second = runtime.create_workspace("Second Workspace", "ops").unwrap();
        let third = runtime
            .create_workspace("Third Workspace", "design")
            .unwrap();

        let page = runtime.list_workspace_page(1, 1).unwrap();

        assert_eq!(page.start_index, 1);
        assert_eq!(page.item_count, 1);
        assert_eq!(page.total_count, 3);
        assert!(page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(page.workspaces.len(), 1);
        assert_eq!(page.workspaces[0].workspace_id, second.workspace_id);
        assert_eq!(page.workspaces[0].name, "Second Workspace");

        let empty_tail = runtime.list_workspace_page(10, 2).unwrap();
        assert_eq!(empty_tail.start_index, 3);
        assert_eq!(empty_tail.item_count, 0);
        assert_eq!(empty_tail.total_count, 3);
        assert!(empty_tail.has_more_before);
        assert!(!empty_tail.has_more_after);
        assert!(empty_tail.workspaces.is_empty());

        let full = runtime.list_workspaces().unwrap();
        assert_eq!(
            full.into_iter()
                .map(|summary| summary.workspace_id)
                .collect::<Vec<_>>(),
            vec![first.workspace_id, second.workspace_id, third.workspace_id]
        );
    }

    #[test]
    fn runtime_caps_workspace_summary_page_limit_but_full_list_remains_complete() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let workspace_count = MAX_WORKSPACE_SUMMARY_PAGE_ROWS + 2;
        let mut workspace_ids = Vec::new();
        for index in 0..workspace_count {
            let created = runtime
                .create_workspace(format!("Summary Page Cap {index:03}"), "general")
                .unwrap();
            workspace_ids.push(created.workspace_id);
        }

        let page = runtime.list_workspace_page(0, usize::MAX).unwrap();

        assert_eq!(page.start_index, 0);
        assert_eq!(page.item_count, MAX_WORKSPACE_SUMMARY_PAGE_ROWS);
        assert_eq!(page.total_count, workspace_count);
        assert!(!page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(page.workspaces.len(), MAX_WORKSPACE_SUMMARY_PAGE_ROWS);
        assert_eq!(page.workspaces[0].workspace_id, workspace_ids[0]);
        assert_eq!(
            page.workspaces
                .last()
                .map(|summary| summary.workspace_id.as_str()),
            Some(workspace_ids[MAX_WORKSPACE_SUMMARY_PAGE_ROWS - 1].as_str())
        );

        let tail = runtime
            .list_workspace_page(MAX_WORKSPACE_SUMMARY_PAGE_ROWS, usize::MAX)
            .unwrap();
        assert_eq!(tail.start_index, MAX_WORKSPACE_SUMMARY_PAGE_ROWS);
        assert_eq!(tail.item_count, 2);
        assert_eq!(tail.total_count, workspace_count);
        assert!(tail.has_more_before);
        assert!(!tail.has_more_after);

        let full = runtime.list_workspaces().unwrap();
        assert_eq!(full.len(), workspace_count);
    }

    #[test]
    fn runtime_workspace_summary_page_ignores_corrupt_nonservable_rows() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let first = runtime
            .create_workspace("First Workspace", "general")
            .unwrap();
        let second = runtime.create_workspace("Second Workspace", "ops").unwrap();
        let third = runtime
            .create_workspace("Third Workspace", "design")
            .unwrap();

        insert_corrupt_event_json(
            tempdir.path(),
            &WorkspaceId(first.workspace_id.clone()),
            "evt_corrupt_first_off_page",
        );
        insert_corrupt_event_json(
            tempdir.path(),
            &WorkspaceId(second.workspace_id.clone()),
            "evt_corrupt_second_visible_page",
        );
        insert_corrupt_event_json(
            tempdir.path(),
            &WorkspaceId(third.workspace_id.clone()),
            "evt_corrupt_third_off_page",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&second.workspace_id)
                .is_err()
        );

        let page = runtime.list_workspace_page(1, 1).unwrap();

        assert_eq!(page.start_index, 1);
        assert_eq!(page.item_count, 1);
        assert_eq!(page.total_count, 3);
        assert!(page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(page.workspaces[0].workspace_id, second.workspace_id);
        assert_eq!(page.workspaces[0].name, "Second Workspace");
        assert_eq!(page.workspaces[0].event_count, 3);

        let full = runtime.list_workspaces().unwrap();
        assert_eq!(full.len(), 3);
        assert_eq!(full[0].event_count, 3);
        assert_eq!(full[1].event_count, 3);
        assert_eq!(full[2].event_count, 3);
    }

    #[test]
    fn runtime_desktop_read_paths_skip_corrupt_rows_and_keep_invalid_signature_rows() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Desktop Corrupt Read", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "desktop searchable message",
            )
            .unwrap();
        let mut forged_event = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Forged Profile".to_owned(),
            },
        );
        forged_event.parents = vec![EventId(sent.event_id.clone())];
        let mut forged = runtime.identity.sign_event(forged_event);
        forged.signature[0] ^= 1;
        runtime.store.append_event(&forged).unwrap();
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_desktop_read_path",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let snapshot = runtime.workspace_snapshot(workspace_id.clone()).unwrap();
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let member_page = runtime
            .list_workspace_member_page(workspace_id.clone(), 0, 10)
            .unwrap();
        let channel_page = runtime
            .list_workspace_channel_page(workspace_id.clone(), 0, 10)
            .unwrap();
        let channel_search = runtime
            .search_workspace_channels(workspace_id.clone(), "general", 10)
            .unwrap();
        let message_search = runtime
            .search_workspace_messages(workspace_id.clone(), "desktop searchable")
            .unwrap();

        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.invalid_signatures[0].event_id, forged.event_id.0);
        assert_eq!(snapshot.invalid_signature_count, 1);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(member_page.total_count, 1);
        assert_eq!(channel_page.total_count, 1);
        assert_eq!(channel_search.total_count, 1);
        assert_eq!(message_search.hits.len(), 1);
        assert_eq!(message_search.hits[0].message_id, sent.message_id);
        assert!(decrypted.timeline.iter().any(|item| {
            item.message_id.as_deref() == Some(sent.message_id.as_str())
                && item.body == "desktop searchable message"
        }));
    }

    #[test]
    fn runtime_workspace_storage_health_reports_corrupt_rows() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Storage Health", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let mut forged_event = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Forged Profile".to_owned(),
            },
        );
        forged_event.parents = vec![EventId(created.channel_event_id)];
        let mut forged = runtime.identity.sign_event(forged_event);
        forged.signature[0] ^= 1;
        runtime.store.append_event(&forged).unwrap();
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_storage_health_tripwire",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let health = runtime
            .workspace_storage_health(workspace_id.clone())
            .unwrap();

        assert_eq!(health.workspace_id, workspace_id.0);
        assert_eq!(health.total_event_count, 4);
        assert_eq!(health.parseable_event_count, 3);
        assert_eq!(health.corrupt_event_count, 1);
        assert_eq!(health.signature_valid_metadata_count, 3);
        assert_eq!(health.servable_event_count, 2);
        assert_eq!(health.poisoned_servable_metadata_count, 1);
        assert_eq!(health.promotable_servable_metadata_count, 0);
        assert_eq!(health.non_servable_parseable_event_count, 1);
    }

    #[test]
    fn runtime_repairs_workspace_storage_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Storage Repair", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_storage_repair_tripwire",
        );
        assert_eq!(
            runtime
                .workspace_storage_health(workspace_id.clone())
                .unwrap()
                .poisoned_servable_metadata_count,
            1
        );

        let repaired = runtime
            .repair_workspace_storage_metadata(workspace_id.clone())
            .unwrap();
        let health = runtime.workspace_storage_health(workspace_id).unwrap();

        assert_eq!(repaired.workspace_id, created.workspace_id);
        assert_eq!(repaired.total_event_count, 3);
        assert_eq!(repaired.corrupt_event_count, 1);
        assert_eq!(repaired.signature_valid_metadata_before_count, 3);
        assert_eq!(repaired.signature_valid_metadata_after_count, 2);
        assert_eq!(repaired.repaired_metadata_count, 1);
        assert_eq!(repaired.promoted_servable_metadata_count, 0);
        assert_eq!(repaired.cleared_unservable_metadata_count, 1);
        assert_eq!(health.corrupt_event_count, 1);
        assert_eq!(health.poisoned_servable_metadata_count, 0);
        assert_eq!(health.promotable_servable_metadata_count, 0);
        assert_eq!(health.servable_event_count, 2);
    }

    #[test]
    fn runtime_write_paths_skip_corrupt_local_event_json() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Corrupt Write Context", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_write_context_tripwire",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let profile = runtime
            .update_device_profile(workspace_id.clone(), "Local Writer")
            .unwrap();
        let invited = runtime
            .invite_member(
                workspace_id.clone(),
                DeviceId("dev_invited_after_corrupt".to_owned()),
                WorkspaceRole::Member,
            )
            .unwrap();
        let created_channel = runtime
            .create_channel(workspace_id.clone(), "after-corrupt", false)
            .unwrap();
        let rotated = runtime.rotate_workspace_key(workspace_id.clone()).unwrap();
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                channel_id,
                "message after corrupt write context",
            )
            .unwrap();
        let indexed = runtime
            .reindex_workspace_search(workspace_id.clone())
            .unwrap();
        let search = runtime
            .search_workspace_messages(workspace_id.clone(), "corrupt write")
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(profile.display_name, "Local Writer");
        assert_eq!(invited.invitee_device_id, "dev_invited_after_corrupt");
        assert_eq!(created_channel.workspace_id, workspace_id.0);
        assert_eq!(rotated.workspace_id, workspace_id.0);
        assert_eq!(sent.workspace_id, workspace_id.0);
        assert_eq!(indexed.indexed_message_count, 1);
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].message_id, sent.message_id);
        assert!(snapshot.timeline.iter().any(|item| {
            item.message_id.as_deref() == Some(sent.message_id.as_str())
                && item.body == "message after corrupt write context"
        }));
    }

    #[test]
    fn runtime_pages_workspace_members_for_desktop_management() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Member Page", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .invite_member(
                workspace_id.clone(),
                DeviceId("dev_admin".to_owned()),
                WorkspaceRole::Admin,
            )
            .unwrap();
        runtime
            .invite_member(
                workspace_id.clone(),
                DeviceId("dev_a".to_owned()),
                WorkspaceRole::Member,
            )
            .unwrap();
        runtime
            .invite_member(
                workspace_id.clone(),
                DeviceId("dev_b".to_owned()),
                WorkspaceRole::Member,
            )
            .unwrap();

        let page = runtime
            .list_workspace_member_page(workspace_id.clone(), 1, 2)
            .unwrap();

        assert_eq!(page.start_index, 1);
        assert_eq!(page.item_count, 2);
        assert_eq!(page.total_count, 4);
        assert!(page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(
            page.members
                .iter()
                .map(|member| member.device_id.as_str())
                .collect::<Vec<_>>(),
            vec!["dev_admin", "dev_a"]
        );

        let empty_tail = runtime
            .list_workspace_member_page(workspace_id, 10, 2)
            .unwrap();
        assert_eq!(empty_tail.start_index, 4);
        assert_eq!(empty_tail.item_count, 0);
        assert_eq!(empty_tail.total_count, 4);
        assert!(empty_tail.has_more_before);
        assert!(!empty_tail.has_more_after);
        assert!(empty_tail.members.is_empty());
    }

    #[test]
    fn runtime_pages_workspace_channels_for_desktop_sidebar() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Channel Page", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .create_channel(workspace_id.clone(), "alpha", false)
            .unwrap();
        let beta = runtime
            .create_channel(workspace_id.clone(), "beta", false)
            .unwrap();
        let gamma = runtime
            .create_channel(workspace_id.clone(), "gamma", false)
            .unwrap();
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(beta.channel_id.clone()),
                "beta latest",
            )
            .unwrap();
        runtime
            .edit_message(
                workspace_id.clone(),
                MessageId(sent.message_id),
                "beta edited",
            )
            .unwrap();

        let page = runtime
            .list_workspace_channel_page(workspace_id.clone(), 0, 2)
            .unwrap();

        assert_eq!(page.start_index, 0);
        assert_eq!(page.item_count, 2);
        assert_eq!(page.total_count, 4);
        assert!(!page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(page.channels[0].channel_id, beta.channel_id);
        assert_eq!(
            page.channels[0]
                .latest_activity
                .as_ref()
                .map(|activity| activity.preview.as_str()),
            Some("Edited: beta edited")
        );
        assert_eq!(page.channels[1].name, "alpha");

        let containing_page = runtime
            .list_workspace_channel_page_containing(
                workspace_id.clone(),
                ChannelId(gamma.channel_id.clone()),
                2,
            )
            .unwrap();
        assert_eq!(containing_page.start_index, 2);
        assert_eq!(containing_page.item_count, 2);
        assert_eq!(containing_page.total_count, 4);
        assert!(containing_page.has_more_before);
        assert!(!containing_page.has_more_after);
        assert_eq!(containing_page.channels[0].channel_id, gamma.channel_id);

        let gamma_sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(gamma.channel_id.clone()),
                "gamma search latest",
            )
            .unwrap();
        runtime
            .edit_message(
                workspace_id.clone(),
                MessageId(gamma_sent.message_id),
                "gamma search edited",
            )
            .unwrap();
        let channel_search = runtime
            .search_workspace_channels(workspace_id.clone(), "gam", 2)
            .unwrap();
        assert_eq!(channel_search.query, "gam");
        assert_eq!(channel_search.item_count, 1);
        assert_eq!(channel_search.total_count, 1);
        assert_eq!(channel_search.channels[0].channel_id, gamma.channel_id);
        assert_eq!(
            channel_search.channels[0]
                .latest_activity
                .as_ref()
                .map(|activity| activity.preview.as_str()),
            Some("Edited: gamma search edited")
        );

        let missing = runtime
            .list_workspace_channel_page_containing(
                workspace_id.clone(),
                ChannelId("chn_missing".to_owned()),
                2,
            )
            .unwrap_err();
        assert!(matches!(
            missing,
            RuntimeError::ChannelNotFound { channel_id, .. }
                if channel_id == ChannelId("chn_missing".to_owned())
        ));

        let empty_tail = runtime
            .list_workspace_channel_page(workspace_id, 10, 2)
            .unwrap();
        assert_eq!(empty_tail.start_index, 4);
        assert_eq!(empty_tail.item_count, 0);
        assert_eq!(empty_tail.total_count, 4);
        assert!(empty_tail.has_more_before);
        assert!(!empty_tail.has_more_after);
        assert!(empty_tail.channels.is_empty());
    }

    #[test]
    fn runtime_caps_member_channel_and_channel_search_page_limits() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Runtime Page Caps", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let extra_count = MAX_WORKSPACE_CHANNEL_PAGE_ROWS + 2;
        let mut channel_ids = Vec::new();
        for index in 0..extra_count {
            let channel = runtime
                .create_channel(workspace_id.clone(), format!("channel-{index:03}"), false)
                .unwrap();
            channel_ids.push(channel.channel_id);
            runtime
                .invite_member(
                    workspace_id.clone(),
                    DeviceId(format!("dev_{index:03}")),
                    WorkspaceRole::Member,
                )
                .unwrap();
        }

        let member_page = runtime
            .list_workspace_member_page(workspace_id.clone(), 0, usize::MAX)
            .unwrap();
        assert_eq!(member_page.start_index, 0);
        assert_eq!(member_page.item_count, MAX_WORKSPACE_MEMBER_PAGE_ROWS);
        assert_eq!(member_page.total_count, extra_count + 1);
        assert!(member_page.has_more_after);
        assert_eq!(member_page.members.len(), MAX_WORKSPACE_MEMBER_PAGE_ROWS);

        let member_tail = runtime
            .list_workspace_member_page(
                workspace_id.clone(),
                MAX_WORKSPACE_MEMBER_PAGE_ROWS,
                usize::MAX,
            )
            .unwrap();
        assert_eq!(member_tail.start_index, MAX_WORKSPACE_MEMBER_PAGE_ROWS);
        assert_eq!(member_tail.item_count, 3);
        assert_eq!(member_tail.total_count, extra_count + 1);
        assert!(!member_tail.has_more_after);

        let channel_page = runtime
            .list_workspace_channel_page(workspace_id.clone(), 0, usize::MAX)
            .unwrap();
        assert_eq!(channel_page.start_index, 0);
        assert_eq!(channel_page.item_count, MAX_WORKSPACE_CHANNEL_PAGE_ROWS);
        assert_eq!(channel_page.total_count, extra_count + 1);
        assert!(channel_page.has_more_after);
        assert_eq!(channel_page.channels.len(), MAX_WORKSPACE_CHANNEL_PAGE_ROWS);

        let target_channel_id = ChannelId(channel_ids[MAX_WORKSPACE_CHANNEL_PAGE_ROWS + 1].clone());
        let containing_page = runtime
            .list_workspace_channel_page_containing(
                workspace_id.clone(),
                target_channel_id.clone(),
                usize::MAX,
            )
            .unwrap();
        assert_eq!(containing_page.start_index, MAX_WORKSPACE_CHANNEL_PAGE_ROWS);
        assert_eq!(containing_page.item_count, 3);
        assert_eq!(containing_page.total_count, extra_count + 1);
        assert!(!containing_page.has_more_after);
        assert!(
            containing_page
                .channels
                .iter()
                .any(|channel| channel.channel_id == target_channel_id.0)
        );

        let channel_search = runtime
            .search_workspace_channels(workspace_id, "channel-", usize::MAX)
            .unwrap();
        assert_eq!(channel_search.item_count, MAX_WORKSPACE_CHANNEL_SEARCH_ROWS);
        assert_eq!(channel_search.total_count, extra_count);
        assert_eq!(
            channel_search.channels.len(),
            MAX_WORKSPACE_CHANNEL_SEARCH_ROWS
        );
    }

    #[test]
    fn runtime_channel_search_skips_termless_queries_without_opening_local_keys() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let key_path = runtime.workspace_key_path(&workspace_id);
        assert!(key_path.exists());
        fs::remove_file(&key_path).unwrap();

        let search = runtime
            .search_workspace_channels(workspace_id.clone(), " \t--- ___ ", 10)
            .unwrap();

        assert_eq!(search.query, "--- ___");
        assert_eq!(search.item_count, 0);
        assert_eq!(search.total_count, 0);
        assert!(search.channels.is_empty());
        assert!(!key_path.exists());
    }

    #[test]
    fn runtime_channel_search_rejects_oversized_query_before_opening_local_keys() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let key_path = runtime.workspace_key_path(&workspace_id);
        assert!(key_path.exists());
        fs::remove_file(&key_path).unwrap();
        let oversized_query = "q".repeat(SEARCH_QUERY_MAX_BYTES + 1);

        let error = runtime
            .search_workspace_channels(workspace_id, oversized_query, 10)
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::SearchQueryTooLarge {
                actual_bytes,
                max_bytes
            } if actual_bytes == SEARCH_QUERY_MAX_BYTES + 1
                && max_bytes == SEARCH_QUERY_MAX_BYTES
        ));
        assert!(!key_path.exists());
    }

    #[test]
    fn runtime_workspace_summary_ignores_invalid_self_contained_signature_events() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Summary Integrity", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let forged_channel_id = ChannelId::new();
        let mut forged_event = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: forged_channel_id,
                name: "forged".to_owned(),
                is_private: false,
            },
        );
        forged_event.parents = vec![EventId(created.channel_event_id.clone())];
        let mut forged = runtime.identity.sign_event(forged_event);
        forged.signature[0] ^= 1;
        runtime.store.append_event(&forged).unwrap();

        let summaries = runtime.list_workspaces().unwrap();
        let snapshot = runtime.workspace_snapshot(workspace_id).unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Summary Integrity");
        assert_eq!(summaries[0].channel_count, 1);
        assert_eq!(summaries[0].event_count, 3);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.invalid_signatures.len(), 1);
    }

    #[test]
    fn decrypted_runtime_snapshot_shows_plaintext_without_leaking_store() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "local plaintext view",
            )
            .unwrap();

        let placeholder = runtime.workspace_snapshot(workspace_id.clone()).unwrap();
        let decrypted = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();
        let events_json = serde_json::to_string(
            &runtime
                .workspace_events(&WorkspaceId(created.workspace_id))
                .unwrap(),
        )
        .unwrap();

        assert_eq!(placeholder.timeline[0].body, "Encrypted message");
        assert_eq!(decrypted.timeline[0].body, "local plaintext view");
        assert!(decrypted.timeline[0].encrypted);
        assert!(!events_json.contains("local plaintext view"));
    }

    #[test]
    fn decrypted_latest_snapshot_skips_off_window_ciphertext() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Windowed Decrypt", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let workspace_key = WorkspaceKey::load(&runtime.workspace_key_path(&workspace_id)).unwrap();
        let stale_message_id = MessageId::new();
        let mut stale_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            runtime.identity.device_id().clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: stale_message_id,
                sealed_markdown: SealedPayload {
                    mode: PayloadEncryption::Aes256GcmSiv,
                    key_id: workspace_key.key_id,
                    nonce: vec![0; 12],
                    aad: b"invalid sealed message aad".to_vec(),
                    bytes: vec![0; 16],
                },
                attachments: Vec::new(),
            },
        );
        stale_message.parents = vec![EventId(created.channel_event_id)];
        let stale_message = runtime.identity.sign_event(stale_message);
        runtime.store.append_event(&stale_message).unwrap();
        runtime
            .send_message(workspace_id.clone(), channel_id, "visible latest")
            .unwrap();

        let latest = runtime
            .decrypted_workspace_snapshot_with_options(
                workspace_id.clone(),
                &WorkspaceSnapshotOptions::latest(1),
            )
            .unwrap();
        let full = runtime.decrypted_workspace_snapshot(workspace_id);

        assert_eq!(latest.timeline.len(), 1);
        assert_eq!(latest.timeline[0].body, "visible latest");
        assert_eq!(latest.timeline_window.total_count, 2);
        assert!(latest.timeline_window.has_more_before);
        assert!(matches!(
            full,
            Err(RuntimeError::Crypto(CryptoError::AssociatedDataMismatch))
        ));
    }

    #[test]
    fn decrypted_channel_snapshot_windows_only_selected_channel() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Channel Windowed", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let general_id = ChannelId(created.channel_id);
        let beta = runtime
            .create_channel(workspace_id.clone(), "beta", false)
            .unwrap();
        let beta_id = ChannelId(beta.channel_id.clone());
        runtime
            .send_message(workspace_id.clone(), general_id.clone(), "general first")
            .unwrap();
        runtime
            .send_message(workspace_id.clone(), beta_id.clone(), "beta first")
            .unwrap();
        runtime
            .send_message(workspace_id.clone(), general_id.clone(), "general second")
            .unwrap();
        runtime
            .send_message(workspace_id.clone(), beta_id.clone(), "beta second")
            .unwrap();
        runtime
            .send_message(workspace_id.clone(), beta_id.clone(), "beta third")
            .unwrap();

        let latest = runtime
            .decrypted_workspace_channel_snapshot_latest(workspace_id.clone(), beta_id.clone(), 2)
            .unwrap();

        assert_eq!(
            latest.timeline_channel_id.as_deref(),
            Some(beta.channel_id.as_str())
        );
        assert_eq!(
            latest
                .timeline
                .iter()
                .map(|item| item.body.as_str())
                .collect::<Vec<_>>(),
            vec!["beta second", "beta third"]
        );
        assert_eq!(latest.timeline_window.start_index, 1);
        assert_eq!(latest.timeline_window.item_count, 2);
        assert_eq!(latest.timeline_window.total_count, 3);
        assert!(latest.timeline_window.has_more_before);
        assert!(!latest.timeline_window.has_more_after);

        let first_page = runtime
            .decrypted_workspace_channel_snapshot_window(workspace_id, beta_id, 0, 2)
            .unwrap();
        assert_eq!(
            first_page
                .timeline
                .iter()
                .map(|item| item.body.as_str())
                .collect::<Vec<_>>(),
            vec!["beta first", "beta second"]
        );
        assert!(!first_page.timeline_window.has_more_before);
        assert!(first_page.timeline_window.has_more_after);
    }

    #[test]
    fn runtime_updates_signed_device_profile_for_timeline_authors() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let updated = runtime
            .update_device_profile(workspace_id.clone(), " Mira ")
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "profiled plaintext",
            )
            .unwrap();

        let events = runtime.workspace_events(&workspace_id).unwrap();
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(updated.display_name, "Mira");
        assert_eq!(updated.device_id, runtime.device_id().0);
        assert_eq!(snapshot.profiles[0].display_name, "Mira");
        assert_eq!(
            snapshot.timeline[0].author_display_name.as_deref(),
            Some("Mira")
        );
        assert!(matches!(
            &events[2].event.body,
            EventBody::DeviceProfileUpdated { display_name } if display_name == "Mira"
        ));
        assert_eq!(events[2].event.parents, vec![events[1].event_id.clone()]);
    }

    #[test]
    fn runtime_publishes_signed_device_key_package_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("MLS Prep", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        let published = runtime
            .publish_device_key_package(
                workspace_id.clone(),
                " openmls/key-package ",
                vec![9, 8, 7, 6],
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert_eq!(published.workspace_id, created.workspace_id);
        assert_eq!(published.device_id, runtime.device_id().0);
        assert!(published.key_package_id.starts_with("dkp_"));
        assert_eq!(published.protocol, "openmls/key-package");
        assert_eq!(published.byte_len, 4);
        assert_eq!(events.len(), 3);
        assert_eq!(events[2].event_id.0, published.event_id);
        assert_eq!(events[2].event.parents, vec![events[1].event_id.clone()]);
        assert!(matches!(
            &events[2].event.body,
            EventBody::DeviceKeyPackagePublished {
                protocol,
                key_package,
                ..
            } if protocol == "openmls/key-package" && key_package == &vec![9, 8, 7, 6]
        ));
    }

    #[test]
    fn runtime_publishes_signed_peer_endpoint_hint() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("P2P Prep", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .update_device_profile(workspace_id.clone(), "Mira")
            .unwrap();

        let published = runtime
            .publish_peer_endpoint(
                workspace_id.clone(),
                " desktop ",
                " direct+tcp://127.0.0.1:7777 ",
                " direct-tcp ",
                true,
                Some(1_700_000_600_000),
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(published.workspace_id, created.workspace_id);
        assert_eq!(published.device_id, runtime.device_id().0);
        assert_eq!(published.endpoint_id, "desktop");
        assert_eq!(published.endpoint, "direct+tcp://127.0.0.1:7777");
        assert_eq!(published.transport, "direct-tcp");
        assert!(published.is_backup_peer);
        assert_eq!(published.expires_at_ms, Some(1_700_000_600_000));
        assert_eq!(events.len(), 4);
        assert_eq!(events[3].event_id.0, published.event_id);
        assert_eq!(events[3].event.parents, vec![events[2].event_id.clone()]);
        assert!(matches!(
            &events[3].event.body,
            EventBody::PeerEndpointPublished {
                endpoint_id,
                endpoint,
                transport,
                is_backup_peer,
                expires_at_ms,
            } if endpoint_id == "desktop"
                && endpoint == "direct+tcp://127.0.0.1:7777"
                && transport == "direct-tcp"
                && *is_backup_peer
                && *expires_at_ms == Some(1_700_000_600_000)
        ));
        assert_eq!(snapshot.peer_endpoints.len(), 1);
        assert_eq!(
            snapshot.peer_endpoints[0].display_name.as_deref(),
            Some("Mira")
        );
        assert_eq!(
            snapshot.peer_endpoints[0].endpoint,
            "direct+tcp://127.0.0.1:7777"
        );
        assert!(snapshot.peer_endpoints[0].is_backup_peer);
    }

    #[test]
    fn runtime_generates_and_publishes_valid_openmls_device_key_package() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("OpenMLS Prep", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        let published = runtime
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::DeviceKeyPackagePublished {
            protocol,
            key_package,
            ..
        } = &events[2].event.body
        else {
            panic!("expected device key package event");
        };
        let public = chaft_mls::validate_key_package(key_package).unwrap();
        let private = chaft_mls::validate_private_key_package_bundle(
            &fs::read(&published.private_bundle_path).unwrap(),
        )
        .unwrap();

        assert_eq!(published.workspace_id, created.workspace_id);
        assert_eq!(published.device_id, runtime.device_id().0);
        assert_eq!(published.protocol, chaft_mls::OPENMLS_KEY_PACKAGE_PROTOCOL);
        assert_eq!(protocol, chaft_mls::OPENMLS_KEY_PACKAGE_PROTOCOL);
        assert_eq!(published.ciphersuite, public.ciphersuite);
        assert_eq!(published.key_package_ref, public.key_package_ref);
        assert_eq!(published.key_package_ref, private.key_package_ref);
        assert_eq!(public.identity, runtime.device_id().0);
        assert_eq!(private.identity, runtime.device_id().0);
        assert_eq!(published.byte_len, key_package.len());
        assert_eq!(events[2].event_id.0, published.event_id);
        assert_eq!(events[2].event.parents, vec![events[1].event_id.clone()]);
    }

    #[test]
    fn runtime_creates_private_openmls_workspace_group_state() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("OpenMLS Group", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        let group = runtime
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let saved_group_state = fs::read(&group.private_group_state_path).unwrap();
        let validated =
            chaft_mls::validate_private_workspace_group_state(&saved_group_state).unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert_eq!(group.workspace_id, created.workspace_id);
        assert_eq!(group.device_id, runtime.device_id().0);
        assert_eq!(group.protocol, chaft_mls::OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(group.ciphersuite, validated.ciphersuite);
        assert_eq!(group.group_id, validated.group_id);
        assert_eq!(group.epoch, 0);
        assert_eq!(group.member_count, 1);
        assert_eq!(validated.identity, runtime.device_id().0);
        assert_eq!(events.len(), 2);
        assert!(matches!(
            runtime.create_openmls_workspace_group(workspace_id),
            Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { .. })
        ));
    }

    #[test]
    fn runtime_adds_and_joins_openmls_workspace_group_member() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Members", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id.clone()),
            )
            .unwrap();
        let events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::OpenMlsWorkspaceGroupMemberAdded {
            invitee_device_id,
            invitee_key_package_id,
            invitee_key_package_ref,
            protocol,
            epoch,
            commit,
            welcome,
            ratchet_tree,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == added.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected OpenMLS member-add event");
        };

        assert_eq!(added.workspace_id, created.workspace_id);
        assert_eq!(added.invitee_device_id, bob.device_id().0);
        assert_eq!(added.invitee_key_package_id, bob_package.key_package_id);
        assert_eq!(added.invitee_key_package_ref, bob_package.key_package_ref);
        assert_eq!(added.protocol, chaft_mls::OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(added.epoch, 1);
        assert_eq!(added.member_count, 2);
        assert_eq!(invitee_device_id, bob.device_id());
        assert_eq!(invitee_key_package_id.0, bob_package.key_package_id);
        assert_eq!(invitee_key_package_ref, &bob_package.key_package_ref);
        assert_eq!(protocol, chaft_mls::OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        assert_eq!(*epoch, 1);
        assert_eq!(commit.len(), added.commit_byte_len);
        assert_eq!(welcome.len(), added.welcome_byte_len);
        assert_eq!(ratchet_tree.len(), added.ratchet_tree_byte_len);
        assert!(!commit.is_empty());
        assert!(!welcome.is_empty());
        assert!(!ratchet_tree.is_empty());
        let alice_group_state = fs::read(&added.private_group_state_path).unwrap();
        let alice_validated =
            chaft_mls::validate_private_workspace_group_state(&alice_group_state).unwrap();
        assert_eq!(alice_validated.member_count, 2);

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let joined = bob
            .join_openmls_workspace_group(
                workspace_id.clone(),
                Some(EventId(added.event_id.clone())),
            )
            .unwrap();
        let bob_group_state = fs::read(&joined.private_group_state_path).unwrap();
        let bob_validated =
            chaft_mls::validate_private_workspace_group_state(&bob_group_state).unwrap();

        assert_eq!(joined.workspace_id, created.workspace_id);
        assert_eq!(joined.device_id, bob.device_id().0);
        assert_eq!(joined.source_event_id, added.event_id);
        assert_eq!(joined.group_id, added.group_id);
        assert_eq!(joined.epoch, 1);
        assert_eq!(joined.member_count, 2);
        assert_eq!(bob_validated.identity, bob.device_id().0);
        assert_eq!(bob_validated.member_count, 2);
    }

    #[test]
    fn runtime_uses_openmls_workspace_content_key_for_public_messages() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Payloads", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(
            workspace_id.clone(),
            Some(EventId(added.event_id.clone())),
        )
        .unwrap();

        let sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "mls derived plaintext",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown, ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(sealed_markdown.key_id.starts_with("openmls:workspace:"));

        for event in alice_events {
            bob.store.append_event(&event).unwrap();
        }
        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let indexed = bob.reindex_workspace_search(workspace_id.clone()).unwrap();
        let search = bob
            .search_workspace_messages(workspace_id, "derived plaintext")
            .unwrap();

        assert_eq!(snapshot.timeline[0].body, "mls derived plaintext");
        assert_eq!(indexed.indexed_message_count, 1);
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "mls derived plaintext");
    }

    #[test]
    fn runtime_applies_openmls_workspace_add_commit_for_existing_member() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let charlie_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Add Catchup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .invite_member(
                workspace_id.clone(),
                charlie.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        let charlie_package = charlie
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        for event in charlie.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let bob_added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(
            workspace_id.clone(),
            Some(EventId(bob_added.event_id.clone())),
        )
        .unwrap();

        let first_sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "workspace add commit epoch one",
            )
            .unwrap();
        let first_alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: first_sealed_markdown,
            ..
        } = &first_alice_events
            .iter()
            .find(|event| event.event_id.0 == first_sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(first_sealed_markdown.key_id.ends_with(":content:v1"));
        for event in first_alice_events {
            bob.store.append_event(&event).unwrap();
        }

        let charlie_added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(charlie_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }
        charlie
            .join_openmls_workspace_group(
                workspace_id.clone(),
                Some(EventId(charlie_added.event_id.clone())),
            )
            .unwrap();

        let applied = bob
            .apply_openmls_workspace_group_commits(workspace_id.clone(), None)
            .unwrap();
        assert_eq!(applied.applied_event_count, 1);
        assert_eq!(applied.applied_event_ids, vec![charlie_added.event_id]);
        assert_eq!(applied.epoch, 2);
        assert_eq!(applied.member_count, 3);

        let sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "workspace add commit epoch two",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown, ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(sealed_markdown.key_id.ends_with(":content:v2"));

        for event in alice_events {
            bob.store.append_event(&event).unwrap();
        }
        let snapshot = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "workspace add commit epoch one");
        assert_eq!(snapshot.timeline[1].body, "workspace add commit epoch two");
    }

    #[test]
    fn runtime_applies_openmls_workspace_remove_commit() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let charlie_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Remove Catchup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .invite_member(
                workspace_id.clone(),
                charlie.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        let charlie_package = charlie
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        for event in charlie.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let bob_added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(workspace_id.clone(), Some(EventId(bob_added.event_id)))
            .unwrap();

        let charlie_added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(charlie_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }
        bob.apply_openmls_workspace_group_commits(workspace_id.clone(), None)
            .unwrap();
        charlie
            .join_openmls_workspace_group(
                workspace_id.clone(),
                Some(EventId(charlie_added.event_id)),
            )
            .unwrap();

        let before_removal = alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "workspace remove commit epoch two",
            )
            .unwrap();
        let removed = alice
            .remove_openmls_workspace_group_member(workspace_id.clone(), bob.device_id().clone())
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }

        let bob_applied = bob
            .apply_openmls_workspace_group_commits(
                workspace_id.clone(),
                Some(EventId(removed.event_id.clone())),
            )
            .unwrap();
        let charlie_applied = charlie
            .apply_openmls_workspace_group_commits(
                workspace_id.clone(),
                Some(EventId(removed.event_id.clone())),
            )
            .unwrap();
        let after_removal = alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "workspace remove commit epoch three",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: before_sealed,
            ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == before_removal.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: after_sealed,
            ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == after_removal.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        let before_key_id = before_sealed.key_id.clone();
        let after_key_id = after_sealed.key_id.clone();
        for event in alice_events {
            charlie.store.append_event(&event).unwrap();
        }
        let snapshot = charlie.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(removed.removed_device_id, bob.device_id().0);
        assert_eq!(removed.epoch, 3);
        assert_eq!(removed.member_count, 2);
        assert!(bob_applied.self_removed);
        assert_eq!(charlie_applied.applied_event_ids, vec![removed.event_id]);
        assert_eq!(charlie_applied.epoch, 3);
        assert!(before_key_id.ends_with(":content:v2"));
        assert!(after_key_id.ends_with(":content:v3"));
        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(
            snapshot.timeline[0].body,
            "workspace remove commit epoch two"
        );
        assert_eq!(
            snapshot.timeline[1].body,
            "workspace remove commit epoch three"
        );
    }

    #[test]
    fn runtime_applies_openmls_workspace_self_update_commit() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Update Catchup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(
            workspace_id.clone(),
            Some(EventId(added.event_id.clone())),
        )
        .unwrap();

        let first_sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "workspace self update epoch one",
            )
            .unwrap();
        let first_alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: first_sealed_markdown,
            ..
        } = &first_alice_events
            .iter()
            .find(|event| event.event_id.0 == first_sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(first_sealed_markdown.key_id.ends_with(":content:v1"));
        for event in first_alice_events {
            bob.store.append_event(&event).unwrap();
        }

        let updated = alice
            .update_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        assert_eq!(updated.epoch, 2);
        assert_eq!(updated.member_count, 2);
        assert!(updated.commit_byte_len > 0);
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let applied = bob
            .apply_openmls_workspace_group_commits(
                workspace_id.clone(),
                Some(EventId(updated.event_id.clone())),
            )
            .unwrap();
        assert_eq!(applied.applied_event_count, 1);
        assert_eq!(applied.applied_event_ids, vec![updated.event_id]);
        assert_eq!(applied.epoch, 2);

        let sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "workspace self update epoch two",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown, ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(sealed_markdown.key_id.ends_with(":content:v2"));

        for event in alice_events {
            bob.store.append_event(&event).unwrap();
        }
        let snapshot = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "workspace self update epoch one");
        assert_eq!(snapshot.timeline[1].body, "workspace self update epoch two");
    }

    #[test]
    fn runtime_uses_openmls_channel_content_key_for_private_messages() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Private Payloads", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        let channel_group = alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        assert_eq!(
            channel_group.protocol,
            chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL
        );
        assert_eq!(added.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let joined = bob
            .join_openmls_channel_group(
                workspace_id.clone(),
                private_channel_id.clone(),
                Some(EventId(added.event_id.clone())),
            )
            .unwrap();
        assert_eq!(joined.protocol, chaft_mls::OPENMLS_CHANNEL_GROUP_PROTOCOL);
        assert_eq!(joined.group_id, added.group_id);

        let sent = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private mls derived plaintext",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown, ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(sealed_markdown.key_id.starts_with("openmls:channel:"));

        for event in alice_events {
            bob.store.append_event(&event).unwrap();
        }
        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let indexed = bob.reindex_workspace_search(workspace_id.clone()).unwrap();
        let search = bob
            .search_workspace_messages(workspace_id, "private mls")
            .unwrap();

        assert_eq!(snapshot.timeline[0].body, "private mls derived plaintext");
        assert_eq!(indexed.indexed_message_count, 1);
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "private mls derived plaintext");
    }

    #[test]
    fn runtime_applies_openmls_channel_self_update_and_keeps_prior_epoch_readable() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Channel Update Catchup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_channel_group(
            workspace_id.clone(),
            private_channel_id.clone(),
            Some(EventId(added.event_id.clone())),
        )
        .unwrap();

        let first_sent = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "channel self update epoch one",
            )
            .unwrap();
        let first_alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: first_sealed_markdown,
            ..
        } = &first_alice_events
            .iter()
            .find(|event| event.event_id.0 == first_sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(first_sealed_markdown.key_id.starts_with("openmls:channel:"));
        assert!(first_sealed_markdown.key_id.ends_with(":content:v1"));
        for event in first_alice_events {
            bob.store.append_event(&event).unwrap();
        }

        let updated = alice
            .update_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        assert_eq!(updated.epoch, 2);
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let applied = bob
            .apply_openmls_channel_group_commits(
                workspace_id.clone(),
                private_channel_id.clone(),
                Some(EventId(updated.event_id.clone())),
            )
            .unwrap();
        assert_eq!(applied.applied_event_count, 1);
        assert_eq!(applied.epoch, 2);

        let second_sent = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "channel self update epoch two",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: second_sealed_markdown,
            ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == second_sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        assert!(
            second_sealed_markdown
                .key_id
                .starts_with("openmls:channel:")
        );
        assert!(second_sealed_markdown.key_id.ends_with(":content:v2"));

        for event in alice_events {
            bob.store.append_event(&event).unwrap();
        }
        let snapshot = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "channel self update epoch one");
        assert_eq!(snapshot.timeline[1].body, "channel self update epoch two");
    }

    #[test]
    fn runtime_updates_all_local_openmls_groups_for_suspected_compromise() {
        let runtime_dir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("OpenMLS Local Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        runtime
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        runtime
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id.clone(),
                "public before MLS compromise rotation",
            )
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private before MLS compromise rotation",
            )
            .unwrap();

        let updated = runtime
            .update_workspace_openmls_groups(workspace_id.clone())
            .unwrap();
        let public_after = runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id,
                "public after MLS compromise rotation",
            )
            .unwrap();
        let private_after = runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id,
                "private after MLS compromise rotation",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let workspace_update = updated.workspace_update.as_ref().unwrap();
        let workspace_update_index = events
            .iter()
            .position(|event| event.event_id.0 == workspace_update.event_id)
            .unwrap();
        let channel_update_index = events
            .iter()
            .position(|event| event.event_id.0 == updated.channel_updates[0].event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: public_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == public_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted public message");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: private_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == private_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted private message");
        };
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(updated.workspace_id, workspace_id.0);
        assert_eq!(workspace_update.epoch, 1);
        assert_eq!(updated.channel_updates.len(), 1);
        assert_eq!(updated.channel_updates[0].epoch, 1);
        assert_eq!(
            updated.updated_event_ids,
            vec![
                workspace_update.event_id.clone(),
                updated.channel_updates[0].event_id.clone()
            ]
        );
        assert!(workspace_update_index < channel_update_index);
        assert!(matches!(
            events[workspace_update_index].event.body,
            EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        ));
        assert!(matches!(
            events[channel_update_index].event.body,
            EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        ));
        assert!(public_sealed.key_id.starts_with("openmls:workspace:"));
        assert!(public_sealed.key_id.ends_with(":content:v1"));
        assert!(private_sealed.key_id.starts_with("openmls:channel:"));
        assert!(private_sealed.key_id.ends_with(":content:v1"));
        assert_eq!(snapshot.timeline.len(), 4);
        assert_eq!(
            snapshot.timeline[2].body,
            "public after MLS compromise rotation"
        );
        assert_eq!(
            snapshot.timeline[3].body,
            "private after MLS compromise rotation"
        );
    }

    #[test]
    fn runtime_suspected_compromise_policy_rotates_openmls_and_manual_keys() {
        let runtime_dir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Comprehensive Compromise Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let openmls_private_channel = runtime
            .create_channel(workspace_id.clone(), "mls-private", true)
            .unwrap();
        let openmls_private_channel_id = ChannelId(openmls_private_channel.channel_id.clone());
        let manual_private_channel = runtime
            .create_channel(workspace_id.clone(), "manual-private", true)
            .unwrap();
        let manual_private_channel_id = ChannelId(manual_private_channel.channel_id.clone());

        runtime
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        runtime
            .create_openmls_channel_group(workspace_id.clone(), openmls_private_channel_id.clone())
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id.clone(),
                "public before comprehensive rotation",
            )
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                openmls_private_channel_id.clone(),
                "mls private before comprehensive rotation",
            )
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                manual_private_channel_id.clone(),
                "manual private before comprehensive rotation",
            )
            .unwrap();

        let rotated = runtime
            .rotate_workspace_for_suspected_compromise(workspace_id.clone())
            .unwrap();
        let public_after = runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id,
                "public after comprehensive rotation",
            )
            .unwrap();
        let openmls_private_after = runtime
            .send_message(
                workspace_id.clone(),
                openmls_private_channel_id.clone(),
                "mls private after comprehensive rotation",
            )
            .unwrap();
        let manual_private_after = runtime
            .send_message(
                workspace_id.clone(),
                manual_private_channel_id.clone(),
                "manual private after comprehensive rotation",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let openmls_updates = rotated.openmls_updates.as_ref().unwrap();
        let manual_rotation = rotated.manual_key_rotation.as_ref().unwrap();
        let manual_private_rotation = manual_rotation
            .channel_key_rotations
            .iter()
            .find(|rotation| rotation.channel_id == manual_private_channel_id.0)
            .unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: public_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == public_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted public message");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: openmls_private_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == openmls_private_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted OpenMLS private message");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: manual_private_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == manual_private_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted manual private message");
        };
        let expected_event_ids = openmls_updates
            .updated_event_ids
            .iter()
            .chain(manual_rotation.rotated_event_ids.iter())
            .cloned()
            .collect::<Vec<_>>();
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(rotated.workspace_id, workspace_id.0);
        assert_eq!(rotated.rotated_event_ids, expected_event_ids);
        assert_eq!(openmls_updates.workspace_update.as_ref().unwrap().epoch, 1);
        assert_eq!(openmls_updates.channel_updates.len(), 1);
        assert_eq!(openmls_updates.channel_updates[0].epoch, 1);
        assert_eq!(manual_rotation.workspace_key_rotation.epoch, 2);
        assert_eq!(manual_rotation.channel_key_rotations.len(), 2);
        assert!(public_sealed.key_id.starts_with("openmls:workspace:"));
        assert!(public_sealed.key_id.ends_with(":content:v1"));
        assert!(
            openmls_private_sealed
                .key_id
                .starts_with("openmls:channel:")
        );
        assert!(openmls_private_sealed.key_id.ends_with(":content:v1"));
        assert_eq!(manual_private_sealed.key_id, manual_private_rotation.key_id);
        assert!(
            decrypted
                .timeline
                .iter()
                .any(|item| item.body == "manual private after comprehensive rotation")
        );
    }

    #[test]
    fn runtime_updates_channel_openmls_groups_without_workspace_group() {
        let runtime_dir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("OpenMLS Channel Only Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        runtime
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id)
            .unwrap();
        let updated = runtime
            .update_workspace_openmls_groups(workspace_id.clone())
            .unwrap();

        assert_eq!(updated.workspace_id, workspace_id.0);
        assert!(updated.workspace_update.is_none());
        assert_eq!(updated.channel_updates.len(), 1);
        assert_eq!(updated.channel_updates[0].epoch, 1);
        assert_eq!(
            updated.updated_event_ids,
            vec![updated.channel_updates[0].event_id.clone()]
        );
    }

    #[test]
    fn runtime_applies_openmls_channel_remove_commit() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let charlie_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Channel Remove", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        for device_id in [bob.device_id(), charlie.device_id()] {
            alice
                .invite_member(
                    workspace_id.clone(),
                    device_id.clone(),
                    WorkspaceRole::Member,
                )
                .unwrap();
            alice
                .add_channel_member(
                    workspace_id.clone(),
                    private_channel_id.clone(),
                    device_id.clone(),
                )
                .unwrap();
        }
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        let charlie_package = charlie
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        for event in charlie.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let bob_added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_channel_group(
            workspace_id.clone(),
            private_channel_id.clone(),
            Some(EventId(bob_added.event_id)),
        )
        .unwrap();

        let charlie_added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(charlie_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }
        bob.apply_openmls_channel_group_commits(
            workspace_id.clone(),
            private_channel_id.clone(),
            None,
        )
        .unwrap();
        charlie
            .join_openmls_channel_group(
                workspace_id.clone(),
                private_channel_id.clone(),
                Some(EventId(charlie_added.event_id)),
            )
            .unwrap();

        let before_removal = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "channel remove commit epoch two",
            )
            .unwrap();
        let removed = alice
            .remove_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
            charlie.store.append_event(&event).unwrap();
        }

        let bob_applied = bob
            .apply_openmls_channel_group_commits(
                workspace_id.clone(),
                private_channel_id.clone(),
                Some(EventId(removed.event_id.clone())),
            )
            .unwrap();
        let charlie_applied = charlie
            .apply_openmls_channel_group_commits(
                workspace_id.clone(),
                private_channel_id.clone(),
                Some(EventId(removed.event_id.clone())),
            )
            .unwrap();
        let after_removal = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id,
                "channel remove commit epoch three",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: before_sealed,
            ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == before_removal.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: after_sealed,
            ..
        } = &alice_events
            .iter()
            .find(|event| event.event_id.0 == after_removal.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted message event");
        };
        let before_key_id = before_sealed.key_id.clone();
        let after_key_id = after_sealed.key_id.clone();
        for event in alice_events {
            charlie.store.append_event(&event).unwrap();
        }
        let snapshot = charlie.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(removed.removed_device_id, bob.device_id().0);
        assert_eq!(removed.epoch, 3);
        assert_eq!(removed.member_count, 2);
        assert!(bob_applied.self_removed);
        assert_eq!(charlie_applied.applied_event_ids, vec![removed.event_id]);
        assert_eq!(charlie_applied.epoch, 3);
        assert!(before_key_id.starts_with("openmls:channel:"));
        assert!(before_key_id.ends_with(":content:v2"));
        assert!(after_key_id.starts_with("openmls:channel:"));
        assert!(after_key_id.ends_with(":content:v3"));
        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "channel remove commit epoch two");
        assert_eq!(
            snapshot.timeline[1].body,
            "channel remove commit epoch three"
        );
    }

    #[test]
    fn runtime_rejects_invalid_device_key_package_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("MLS Prep", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        assert!(matches!(
            runtime.publish_device_key_package(workspace_id.clone(), " ", vec![1]),
            Err(RuntimeError::DeviceKeyPackageProtocolRequired)
        ));
        assert!(matches!(
            runtime.publish_device_key_package(workspace_id.clone(), "openmls/key-package", vec![]),
            Err(RuntimeError::DeviceKeyPackageRequired)
        ));
        assert!(matches!(
            runtime.publish_device_key_package(
                workspace_id,
                "openmls/key-package",
                vec![1; DEVICE_KEY_PACKAGE_MAX_LEN + 1],
            ),
            Err(RuntimeError::DeviceKeyPackageTooLarge)
        ));
    }

    #[test]
    fn runtime_rejects_oversized_workspace_channel_profile_and_key_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();

        let oversized_workspace_name = "w".repeat(WORKSPACE_NAME_MAX_BYTES + 1);
        assert!(matches!(
            runtime.create_workspace(oversized_workspace_name, "general"),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace name",
                actual_bytes,
                max_bytes: WORKSPACE_NAME_MAX_BYTES,
            }) if actual_bytes == WORKSPACE_NAME_MAX_BYTES + 1
        ));
        assert_eq!(runtime.store.list_events().unwrap().len(), 0);

        let oversized_default_channel = "c".repeat(CHANNEL_NAME_MAX_BYTES + 1);
        assert!(matches!(
            runtime.create_workspace("Metadata Limits", oversized_default_channel),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "default channel name",
                actual_bytes,
                max_bytes: CHANNEL_NAME_MAX_BYTES,
            }) if actual_bytes == CHANNEL_NAME_MAX_BYTES + 1
        ));
        assert_eq!(runtime.store.list_events().unwrap().len(), 0);

        let created = runtime
            .create_workspace("Metadata Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        let oversized_channel = "c".repeat(CHANNEL_NAME_MAX_BYTES + 1);
        assert!(matches!(
            runtime.create_channel(workspace_id.clone(), oversized_channel, false),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "channel name",
                actual_bytes,
                max_bytes: CHANNEL_NAME_MAX_BYTES,
            }) if actual_bytes == CHANNEL_NAME_MAX_BYTES + 1
        ));

        let oversized_display_name = "d".repeat(DEVICE_DISPLAY_NAME_MAX_BYTES + 1);
        assert!(matches!(
            runtime.update_device_profile(workspace_id.clone(), oversized_display_name),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "display name",
                actual_bytes,
                max_bytes: DEVICE_DISPLAY_NAME_MAX_BYTES,
            }) if actual_bytes == DEVICE_DISPLAY_NAME_MAX_BYTES + 1
        ));

        let oversized_protocol = "p".repeat(DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES + 1);
        assert!(matches!(
            runtime.publish_device_key_package(workspace_id.clone(), oversized_protocol, vec![1]),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "device key package protocol",
                actual_bytes,
                max_bytes: DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
            }) if actual_bytes == DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES + 1
        ));
        assert_eq!(runtime.workspace_events(&workspace_id).unwrap().len(), 2);
    }

    #[test]
    fn runtime_rejects_oversized_workspace_identifier_before_store_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let workspace_id = WorkspaceId("w".repeat(chaft_types::WORKSPACE_ID_MAX_BYTES + 1));

        assert_oversized_identifier_error(
            runtime.workspace_snapshot(workspace_id),
            "workspace ID",
            chaft_types::WORKSPACE_ID_MAX_BYTES,
        );
    }

    #[test]
    fn runtime_rejects_oversized_channel_and_message_identifiers_before_workspace_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let workspace_id = WorkspaceId("wrk_missing".to_owned());

        assert_oversized_identifier_error(
            runtime.send_message(
                workspace_id.clone(),
                ChannelId("c".repeat(chaft_types::CHANNEL_ID_MAX_BYTES + 1)),
                "hello",
            ),
            "channel ID",
            chaft_types::CHANNEL_ID_MAX_BYTES,
        );
        assert_oversized_identifier_error(
            runtime.edit_message(
                workspace_id,
                MessageId("m".repeat(chaft_types::MESSAGE_ID_MAX_BYTES + 1)),
                "hello",
            ),
            "message ID",
            chaft_types::MESSAGE_ID_MAX_BYTES,
        );
    }

    #[test]
    fn runtime_rejects_invalid_peer_endpoint_hint() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("P2P Prep", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                " ",
                "direct+tcp://127.0.0.1:7777",
                "direct-tcp",
                false,
                None,
            ),
            Err(RuntimeError::PeerEndpointIdRequired)
        ));
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "desktop",
                " ",
                "direct-tcp",
                false,
                None,
            ),
            Err(RuntimeError::PeerEndpointRequired)
        ));
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "desktop",
                "direct+tcp://127.0.0.1:7777",
                " ",
                false,
                None,
            ),
            Err(RuntimeError::PeerEndpointTransportRequired)
        ));
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "bad-label",
                "direct+tcp://127.0.0.1:7777",
                "iroh",
                false,
                None,
            ),
            Err(RuntimeError::PeerEndpointTransportMismatch)
        ));
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "bad-direct",
                "direct+tcp://not-a-socket",
                "direct-tcp",
                false,
                None,
            ),
            Err(RuntimeError::UnsupportedPeerEndpoint)
        ));
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id,
                "centralized-ws",
                "wss://central.example.invalid/sync",
                "wss",
                false,
                None,
            ),
            Err(RuntimeError::UnsupportedPeerEndpoint)
        ));
    }

    #[test]
    fn runtime_rejects_oversized_peer_endpoint_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("P2P Metadata", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        let oversized_endpoint_id = "p".repeat(PEER_ENDPOINT_ID_MAX_BYTES + 1);
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                oversized_endpoint_id,
                "direct+tcp://127.0.0.1:7777",
                "direct-tcp",
                false,
                None,
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "peer endpoint ID",
                actual_bytes,
                max_bytes: PEER_ENDPOINT_ID_MAX_BYTES,
            }) if actual_bytes == PEER_ENDPOINT_ID_MAX_BYTES + 1
        ));

        let oversized_endpoint = "e".repeat(PEER_ENDPOINT_MAX_BYTES + 1);
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "desktop",
                oversized_endpoint,
                "direct-tcp",
                false,
                None,
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "peer endpoint",
                actual_bytes,
                max_bytes: PEER_ENDPOINT_MAX_BYTES,
            }) if actual_bytes == PEER_ENDPOINT_MAX_BYTES + 1
        ));

        let oversized_transport = "t".repeat(PEER_ENDPOINT_TRANSPORT_MAX_BYTES + 1);
        assert!(matches!(
            runtime.publish_peer_endpoint(
                workspace_id.clone(),
                "desktop",
                "direct+tcp://127.0.0.1:7777",
                oversized_transport,
                false,
                None,
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "peer endpoint transport",
                actual_bytes,
                max_bytes: PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
            }) if actual_bytes == PEER_ENDPOINT_TRANSPORT_MAX_BYTES + 1
        ));
        assert_eq!(runtime.workspace_events(&workspace_id).unwrap().len(), 2);
    }

    #[test]
    fn runtime_rejects_oversized_member_device_id_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Member Metadata", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        let oversized_device_id = DeviceId("d".repeat(DEVICE_ID_REFERENCE_MAX_BYTES + 1));

        assert!(matches!(
            runtime.invite_member(
                workspace_id.clone(),
                oversized_device_id.clone(),
                WorkspaceRole::Member,
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "device ID",
                actual_bytes,
                max_bytes: DEVICE_ID_REFERENCE_MAX_BYTES,
            }) if actual_bytes == DEVICE_ID_REFERENCE_MAX_BYTES + 1
        ));
        assert!(matches!(
            runtime.add_channel_member(
                workspace_id.clone(),
                channel_id,
                oversized_device_id.clone(),
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "device ID",
                actual_bytes,
                max_bytes: DEVICE_ID_REFERENCE_MAX_BYTES,
            }) if actual_bytes == DEVICE_ID_REFERENCE_MAX_BYTES + 1
        ));
        assert!(matches!(
            runtime.remove_member_with_key_rotation(workspace_id.clone(), oversized_device_id),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "device ID",
                actual_bytes,
                max_bytes: DEVICE_ID_REFERENCE_MAX_BYTES,
            }) if actual_bytes == DEVICE_ID_REFERENCE_MAX_BYTES + 1
        ));
        assert_eq!(runtime.workspace_events(&workspace_id).unwrap().len(), 2);
    }

    #[test]
    fn runtime_rejects_message_for_unknown_channel() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let missing_channel_id = ChannelId("chn_missing".to_owned());
        let error = runtime
            .send_message(
                workspace_id.clone(),
                missing_channel_id.clone(),
                "wrong channel",
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::Authorization(AuthorizationError::ChannelNotFound { channel_id })
                if channel_id == missing_channel_id
        ));
        assert_eq!(runtime.workspace_events(&workspace_id).unwrap().len(), 2);
    }

    #[test]
    fn runtime_sends_encrypted_reply_with_snapshot_context() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Replies", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let parent = runtime
            .send_message(workspace_id.clone(), channel_id.clone(), "parent body")
            .unwrap();
        let reply = runtime
            .send_message_reply(
                workspace_id.clone(),
                channel_id,
                MessageId(parent.message_id.clone()),
                "reply body",
            )
            .unwrap();

        let events = runtime.workspace_events(&workspace_id).unwrap();
        let reply_event = events
            .iter()
            .find(|event| event.event_id.0 == reply.event_id)
            .unwrap();
        let EventBody::MessageReplyCreatedEncrypted {
            reply_to_message_id,
            ..
        } = &reply_event.event.body
        else {
            panic!("expected encrypted reply event");
        };
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(
            reply.reply_to_message_id.as_deref(),
            Some(parent.message_id.as_str())
        );
        assert_eq!(reply_to_message_id.0, parent.message_id);
        assert_eq!(
            snapshot.timeline[1].reply_to_message_id.as_deref(),
            Some(parent.message_id.as_str())
        );
        assert_eq!(
            snapshot.timeline[1]
                .reply_preview
                .as_ref()
                .map(|preview| preview.body.as_str()),
            Some("parent body")
        );
    }

    #[test]
    fn runtime_adds_reaction_to_existing_message() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Reactions", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "reactable message",
            )
            .unwrap();

        let reaction = runtime
            .add_reaction(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                "+1",
            )
            .unwrap();
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(reaction.channel_id, created.channel_id);
        assert_eq!(reaction.message_id, sent.message_id);
        assert_eq!(reaction.reaction, "+1");
        assert_eq!(snapshot.timeline[0].reactions.get("+1"), Some(&1));
        assert_eq!(snapshot.timeline[0].my_reactions, vec!["+1".to_owned()]);

        let duplicate_reaction = runtime
            .add_reaction(
                WorkspaceId(created.workspace_id.clone()),
                MessageId(sent.message_id.clone()),
                "+1",
            )
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(WorkspaceId(created.workspace_id.clone()))
            .unwrap();
        assert_eq!(duplicate_reaction.reaction, "+1");
        assert_eq!(snapshot.timeline[0].reactions.get("+1"), Some(&1));
        assert_eq!(snapshot.timeline[0].my_reactions, vec!["+1".to_owned()]);

        let removed = runtime
            .remove_reaction(
                WorkspaceId(created.workspace_id.clone()),
                MessageId(sent.message_id.clone()),
                "+1",
            )
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(WorkspaceId(created.workspace_id.clone()))
            .unwrap();
        assert_eq!(removed.channel_id, created.channel_id);
        assert_eq!(removed.message_id, sent.message_id);
        assert_eq!(removed.reaction, "+1");
        assert_eq!(snapshot.timeline[0].reactions.get("+1"), None);
        assert!(snapshot.timeline[0].my_reactions.is_empty());

        let duplicate_removal = runtime
            .remove_reaction(
                WorkspaceId(created.workspace_id.clone()),
                MessageId(sent.message_id),
                "+1",
            )
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(WorkspaceId(created.workspace_id))
            .unwrap();
        assert_eq!(duplicate_removal.reaction, "+1");
        assert_eq!(snapshot.timeline[0].reactions.get("+1"), None);
        assert!(snapshot.timeline[0].my_reactions.is_empty());
    }

    #[test]
    fn runtime_rejects_oversized_reaction_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Reaction Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "reactable message",
            )
            .unwrap();
        let message_id = MessageId(sent.message_id);
        let oversized_reaction = "r".repeat(REACTION_TEXT_MAX_BYTES + 1);

        assert!(matches!(
            runtime.add_reaction(
                workspace_id.clone(),
                message_id.clone(),
                oversized_reaction.clone(),
            ),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "reaction",
                actual_bytes,
                max_bytes: REACTION_TEXT_MAX_BYTES,
            }) if actual_bytes == REACTION_TEXT_MAX_BYTES + 1
        ));
        assert!(matches!(
            runtime.remove_reaction(workspace_id.clone(), message_id, oversized_reaction),
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "reaction",
                actual_bytes,
                max_bytes: REACTION_TEXT_MAX_BYTES,
            }) if actual_bytes == REACTION_TEXT_MAX_BYTES + 1
        ));
        assert_eq!(runtime.workspace_events(&workspace_id).unwrap().len(), 3);
    }

    #[test]
    fn runtime_sends_encrypted_attachment_without_plaintext_blob_leak() {
        const ATTACHMENT_TEXT: &str = "private attachment bytes";
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("note.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime.create_workspace("Attachments", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "see attached",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &events[2].event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = &attachments[0];
        let blob_store = BlobStore::open(runtime.paths().blob_store.clone()).unwrap();
        let ciphertext = blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let workspace_key = WorkspaceKey::load(&runtime.workspace_key_path(&workspace_id)).unwrap();
        let sealed = sealed_payload_from_encrypted_blob_ref(
            attachment.encryption.as_ref().unwrap(),
            ciphertext.clone(),
        );
        let opened = open_attachment_blob(
            workspace_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &MessageId(sent.message_id.clone()),
            0,
        )
        .unwrap();
        let saved_path = tempdir.path().join("saved-note.txt");
        let saved = runtime
            .save_attachment_to_file(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                &attachment.attachment_id,
                &saved_path,
            )
            .unwrap();

        assert_eq!(sent.attachment_count, 1);
        assert_eq!(snapshot.timeline[0].attachments.len(), 1);
        assert_eq!(
            snapshot.timeline[0].attachments[0].attachment_id,
            attachment.attachment_id
        );
        assert_eq!(snapshot.timeline[0].attachments[0].display_name, "note.txt");
        assert_eq!(snapshot.timeline[0].attachments[0].media_type, "text/plain");
        assert!(snapshot.timeline[0].attachments[0].encrypted);
        assert_eq!(
            snapshot.timeline[0].attachments[0].local_blob_available,
            Some(true)
        );
        assert_eq!(attachment.display_name, "note.txt");
        assert_eq!(attachment.media_type, "text/plain");
        assert_eq!(
            attachment.attachment_id,
            format!("att_{}_0", sent.message_id)
        );
        assert_eq!(opened, ATTACHMENT_TEXT.as_bytes());
        assert_eq!(saved.workspace_id, workspace_id.0);
        assert_eq!(saved.channel_id, channel_id.0);
        assert_eq!(saved.message_id, sent.message_id);
        assert_eq!(saved.blob_hash, attachment.blob_hash);
        assert_eq!(saved.attachment_id, attachment.attachment_id);
        assert_eq!(saved.display_name, "note.txt");
        assert_eq!(fs::read_to_string(saved_path).unwrap(), ATTACHMENT_TEXT);
        assert!(!String::from_utf8_lossy(&ciphertext).contains("attachment bytes"));
        assert!(
            !serde_json::to_string(&events)
                .unwrap()
                .contains(ATTACHMENT_TEXT)
        );
    }

    #[test]
    fn runtime_attachment_export_replaces_existing_file_without_temp_artifacts() {
        const ATTACHMENT_TEXT: &str = "replacement attachment bytes";
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("replace-note.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Attachment Replace", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "see replacement",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &events[2].event.body else {
            panic!("expected encrypted message event");
        };
        let attachment_id = attachments[0].attachment_id.clone();
        let export_dir = tempdir.path().join("exports");
        let saved_path = export_dir.join("replace-note.txt");
        fs::create_dir_all(&export_dir).unwrap();
        fs::write(&saved_path, "old bytes").unwrap();

        let saved = runtime
            .save_attachment_to_file(
                workspace_id,
                MessageId(sent.message_id),
                attachment_id,
                &saved_path,
            )
            .unwrap();

        assert_eq!(saved.output_path, saved_path.to_string_lossy());
        assert_eq!(fs::read_to_string(&saved_path).unwrap(), ATTACHMENT_TEXT);
        assert_eq!(
            secret_temp_artifacts_under(&export_dir),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn runtime_attachment_export_cleans_temp_file_when_destination_is_directory() {
        const ATTACHMENT_TEXT: &str = "failed export attachment bytes";
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("failed-note.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Attachment Failure", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "see failed export",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &events[2].event.body else {
            panic!("expected encrypted message event");
        };
        let attachment_id = attachments[0].attachment_id.clone();
        let output_dir = tempdir.path().join("directory-output.txt");
        fs::create_dir_all(&output_dir).unwrap();

        let error = runtime
            .save_attachment_to_file(
                workspace_id,
                MessageId(sent.message_id),
                attachment_id,
                &output_dir,
            )
            .unwrap_err();

        assert!(matches!(error, RuntimeError::Io(_)));
        assert!(output_dir.is_dir());
        assert_eq!(
            secret_temp_artifacts_under(tempdir.path()),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn runtime_rejects_oversized_attachment_file_before_append() {
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("too-large.bin");
        let attachment_file = fs::File::create(&attachment_path).unwrap();
        attachment_file
            .set_len(ATTACHMENT_FILE_MAX_BYTES + 1)
            .unwrap();
        drop(attachment_file);
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Attachment Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        let error = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "oversized attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap_err();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert!(matches!(
            error,
            RuntimeError::AttachmentFileTooLarge {
                actual_bytes,
                max_bytes
            } if actual_bytes == ATTACHMENT_FILE_MAX_BYTES + 1
                && max_bytes == ATTACHMENT_FILE_MAX_BYTES
        ));
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn runtime_rejects_blank_attachment_file_path_before_file_stat() {
        assert_required_metadata_field_error(
            read_attachment_file_with_limit(Path::new("")),
            "attachment file path",
        );
    }

    #[test]
    fn runtime_rejects_oversized_attachment_file_path_before_file_stat() {
        assert_oversized_runtime_path_error(
            read_attachment_file_with_limit(&PathBuf::from("a".repeat(RUNTIME_PATH_MAX_BYTES + 1))),
            "attachment file path",
        );
    }

    #[test]
    fn runtime_rejects_blank_attachment_output_path_before_file_write() {
        assert_required_metadata_field_error(
            write_attachment_export_file(Path::new(""), b"bytes"),
            "attachment output path",
        );
    }

    #[test]
    fn runtime_rejects_oversized_attachment_output_path_before_file_write() {
        assert_oversized_runtime_path_error(
            write_attachment_export_file(
                &PathBuf::from("o".repeat(RUNTIME_PATH_MAX_BYTES + 1)),
                b"bytes",
            ),
            "attachment output path",
        );
    }

    #[test]
    fn runtime_attachment_export_ignores_invalid_signature_messages() {
        const ATTACHMENT_TEXT: &[u8] = b"forged encrypted attachment bytes";
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Invalid Attachment", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let context = runtime.workspace_write_context(&workspace_id).unwrap();
        let content_key = runtime
            .content_key_for_local_write_in_state(&workspace_id, &channel_id, &context.state)
            .unwrap();
        let forged_message_id = MessageId::new();
        let sealed_markdown = seal_message_markdown(
            content_key.key_id(),
            content_key.content_key(),
            &workspace_id,
            &channel_id,
            &forged_message_id,
            "forged encrypted attachment",
        )
        .unwrap();
        let attachments = runtime
            .seal_and_store_attachments(
                &workspace_id,
                &channel_id,
                &forged_message_id,
                &content_key,
                vec![PendingAttachment {
                    display_name: "forged.txt".to_owned(),
                    media_type: "text/plain".to_owned(),
                    plaintext: ATTACHMENT_TEXT.to_vec(),
                }],
            )
            .unwrap();
        let forged_blob_hash = attachments[0].blob_hash.clone();
        let mut forged_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            runtime.device_id().clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: forged_message_id.clone(),
                sealed_markdown,
                attachments,
            },
        );
        forged_message.parents = context.head_event_ids;
        let mut forged_message = runtime.identity.sign_event(forged_message);
        forged_message.signature[0] ^= 1;
        let forged_event_id = forged_message.event_id.clone();
        runtime.store.append_event(&forged_message).unwrap();

        let output_path = tempdir.path().join("forged-output.txt");
        let error = runtime
            .save_attachment_to_file(
                workspace_id.clone(),
                forged_message_id.clone(),
                &forged_blob_hash,
                &output_path,
            )
            .unwrap_err();
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        match error {
            RuntimeError::MessageNotFound {
                workspace_id: missing_workspace_id,
                message_id: missing_message_id,
            } => {
                assert_eq!(missing_workspace_id, workspace_id);
                assert_eq!(missing_message_id, forged_message_id);
            }
            other => panic!("expected forged message to be ignored, got {other:?}"),
        }
        assert!(!output_path.exists());
        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.invalid_signatures[0].event_id, forged_event_id.0);
        assert!(
            !snapshot
                .timeline
                .iter()
                .any(|item| item.kind == chaft_app::TimelineItemKind::EncryptedMessage)
        );
    }

    #[test]
    fn runtime_infers_attachment_media_type_when_not_supplied() {
        let tempdir = tempfile::tempdir().unwrap();
        let markdown_path = tempdir.path().join("readme.MD");
        let unknown_path = tempdir.path().join("payload.unknown");
        fs::write(&markdown_path, "# attachment metadata").unwrap();
        fs::write(&unknown_path, "opaque bytes").unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Attachment Media", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let inferred = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "inferred",
                &markdown_path,
                "",
            )
            .unwrap();
        let fallback = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "fallback",
                &unknown_path,
                "",
            )
            .unwrap();
        let explicit = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "explicit",
                &markdown_path,
                "application/x-chaft-test",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert_eq!(
            attachment_media_type_for_message(&events, &inferred.event_id),
            "text/markdown"
        );
        assert_eq!(
            attachment_media_type_for_message(&events, &fallback.event_id),
            "application/octet-stream"
        );
        assert_eq!(
            attachment_media_type_for_message(&events, &explicit.event_id),
            "application/x-chaft-test"
        );
    }

    #[test]
    fn runtime_marks_missing_local_attachment_blobs_in_snapshot() {
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("note.txt");
        fs::write(&attachment_path, "cached attachment").unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Blob Availability", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "see attached",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &events[2].event.body else {
            panic!("expected encrypted message event");
        };
        let blob_hash = attachments[0].blob_hash.clone();
        let blob_store = BlobStore::open(runtime.paths().blob_store.clone()).unwrap();
        let ciphertext = blob_store.get_bytes(&blob_hash).unwrap().unwrap();
        let available_snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let blob_path = runtime
            .paths()
            .blob_store
            .join(&blob_hash[..2])
            .join(&blob_hash);

        fs::remove_file(blob_path).unwrap();
        let missing_snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let descriptor = blob_store.put_bytes_chunked(&ciphertext, 8).unwrap();
        let chunked_snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let saved_path = tempdir.path().join("saved-from-chunks.txt");
        runtime
            .save_attachment_to_file(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                blob_hash.clone(),
                &saved_path,
            )
            .unwrap();
        let first_chunk_path = runtime
            .paths()
            .blob_store
            .join("chunks")
            .join(&descriptor.chunk_hashes[0][..2])
            .join(&descriptor.chunk_hashes[0]);

        fs::remove_file(first_chunk_path).unwrap();
        let partial_chunk_snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(sent.attachment_count, 1);
        assert_eq!(descriptor.hash, blob_hash);
        assert_eq!(
            available_snapshot.timeline[0].attachments[0].local_blob_available,
            Some(true)
        );
        assert_eq!(
            missing_snapshot.timeline[0].attachments[0].local_blob_available,
            Some(false)
        );
        assert_eq!(
            chunked_snapshot.timeline[0].attachments[0].local_blob_available,
            Some(true)
        );
        assert_eq!(fs::read_to_string(saved_path).unwrap(), "cached attachment");
        assert_eq!(
            partial_chunk_snapshot.timeline[0].attachments[0].local_blob_available,
            Some(false)
        );
    }

    #[test]
    fn runtime_prunes_unreferenced_local_blob_cache() {
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("note.txt");
        fs::write(&attachment_path, "kept attachment").unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Blob Retention", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "see attached",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &events[2].event.body else {
            panic!("expected encrypted message event");
        };
        let kept_blob_hash = attachments[0].blob_hash.clone();
        let blob_store = BlobStore::open(runtime.paths().blob_store.clone()).unwrap();
        let orphan = blob_store.put_bytes(b"orphan ciphertext").unwrap();
        let stale_temp_path = runtime.paths().blob_store.join(".orphan.tmp.999999.0");
        fs::write(&stale_temp_path, b"stale temp").unwrap();

        let pruned = runtime.prune_unreferenced_blobs().unwrap();

        assert_eq!(sent.attachment_count, 1);
        assert_eq!(pruned.workspace_ids, vec![workspace_id.0]);
        assert_eq!(pruned.referenced_blob_hashes, vec![kept_blob_hash.clone()]);
        assert_eq!(pruned.removed_blob_hashes, vec![orphan.hash.clone()]);
        assert_eq!(pruned.removed_temp_file_count, 1);
        assert_eq!(
            pruned.removed_temp_file_paths,
            vec![".orphan.tmp.999999.0".to_owned()]
        );
        assert!(blob_store.has_blob(&kept_blob_hash).unwrap());
        assert!(!blob_store.has_blob(&orphan.hash).unwrap());
        assert!(!stale_temp_path.exists());
    }

    #[test]
    fn runtime_edits_encrypted_message_and_rebuilds_search() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Edits", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message(workspace_id.clone(), channel_id.clone(), "original needle")
            .unwrap();

        let edited = runtime
            .edit_message(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                "edited needle",
            )
            .unwrap();
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let original_hits = runtime
            .search_workspace_messages(workspace_id.clone(), "original")
            .unwrap();
        let edited_hits = runtime
            .search_workspace_messages(workspace_id.clone(), "edited")
            .unwrap();
        let events_json =
            serde_json::to_string(&runtime.workspace_events(&workspace_id).unwrap()).unwrap();

        assert_eq!(edited.channel_id, created.channel_id);
        assert_eq!(edited.message_id, sent.message_id);
        assert!(edited.encrypted);
        assert_eq!(decrypted.timeline[0].body, "edited needle");
        assert_eq!(original_hits.hits.len(), 0);
        assert_eq!(edited_hits.hits.len(), 1);
        assert_eq!(edited_hits.hits[0].body, "edited needle");
        assert_eq!(edited_hits.hits[0].event_id, sent.event_id);
        assert_eq!(edited_hits.hits[0].message_id, sent.message_id);
        assert!(!events_json.contains("original needle"));
        assert!(!events_json.contains("edited needle"));
    }

    #[test]
    fn runtime_rejects_oversized_message_markdown_before_append() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Message Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let oversized = "x".repeat(MESSAGE_MARKDOWN_MAX_BYTES + 1);

        let error = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                &oversized,
            )
            .unwrap_err();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert!(matches!(
            error,
            RuntimeError::MessageMarkdownTooLarge {
                actual_bytes,
                max_bytes
            } if actual_bytes == MESSAGE_MARKDOWN_MAX_BYTES + 1
                && max_bytes == MESSAGE_MARKDOWN_MAX_BYTES
        ));
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn runtime_rejects_oversized_message_edit_without_reindexing() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Edit Limits", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message(workspace_id.clone(), channel_id, "original searchable")
            .unwrap();
        let oversized = "x".repeat(MESSAGE_MARKDOWN_MAX_BYTES + 1);

        let error = runtime
            .edit_message(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                &oversized,
            )
            .unwrap_err();
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let hits = runtime
            .search_workspace_messages(workspace_id.clone(), "original")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert!(matches!(
            error,
            RuntimeError::MessageMarkdownTooLarge {
                actual_bytes,
                max_bytes
            } if actual_bytes == MESSAGE_MARKDOWN_MAX_BYTES + 1
                && max_bytes == MESSAGE_MARKDOWN_MAX_BYTES
        ));
        assert_eq!(snapshot.timeline[0].body, "original searchable");
        assert_eq!(hits.hits.len(), 1);
        assert_eq!(hits.hits[0].message_id, sent.message_id);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn runtime_deletes_message_and_removes_it_from_search() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Deletes", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "deleted needle",
            )
            .unwrap();
        let kept = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "deleted needle kept",
            )
            .unwrap();

        let deleted = runtime
            .delete_message(workspace_id.clone(), MessageId(sent.message_id.clone()))
            .unwrap();
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let hits = runtime
            .search_workspace_messages(workspace_id.clone(), "deleted")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert_eq!(deleted.channel_id, created.channel_id);
        assert_eq!(deleted.message_id, sent.message_id);
        assert_eq!(decrypted.timeline[0].body, "Message deleted");
        assert!(decrypted.timeline[0].deleted);
        assert_eq!(hits.hits.len(), 1);
        assert_eq!(hits.hits[0].message_id, kept.message_id);
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn runtime_marks_channel_read_idempotently() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Read Markers", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "read marker target",
            )
            .unwrap();

        let first = runtime
            .mark_channel_read(workspace_id.clone(), channel_id.clone())
            .unwrap();
        let second = runtime
            .mark_channel_read(workspace_id.clone(), channel_id.clone())
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();

        assert_eq!(first.channel_id, created.channel_id);
        assert_eq!(first.read_through_event_id, sent.event_id);
        assert!(first.marker_event_id.is_some());
        assert!(!first.already_read);
        assert_eq!(second.marker_event_id, None);
        assert!(second.already_read);
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn runtime_indexes_sent_messages_in_private_search_database() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Searchable", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .update_device_profile(workspace_id.clone(), "Mira")
            .unwrap();
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "needle lives only in the local search index",
            )
            .unwrap();

        let hits = runtime
            .search_workspace_messages(workspace_id.clone(), "needle")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let sent_event_physical_ms = events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .timestamp
            .physical_ms;
        let events_json = serde_json::to_string(&events).unwrap();
        let device_id = runtime.device_id().0.clone();
        drop(runtime);

        let reopened = LocalRuntime::open(tempdir.path(), None).unwrap();
        let reopened_hits = reopened
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(hits.hits.len(), 1);
        assert_eq!(
            hits.hits[0].body,
            "needle lives only in the local search index"
        );
        assert_eq!(hits.hits[0].author_device_id, device_id);
        assert_eq!(hits.hits[0].author_display_name.as_deref(), Some("Mira"));
        assert_eq!(hits.hits[0].channel_name, "general");
        assert_eq!(hits.hits[0].physical_ms, sent_event_physical_ms);
        assert_eq!(reopened_hits.hits.len(), 1);
        assert_eq!(reopened_hits.hits[0].channel_name, "general");
        assert_eq!(
            reopened_hits.hits[0].author_device_id,
            hits.hits[0].author_device_id
        );
        assert_eq!(
            reopened_hits.hits[0].author_display_name.as_deref(),
            Some("Mira")
        );
        assert_eq!(reopened_hits.hits[0].physical_ms, sent_event_physical_ms);
        assert!(!events_json.contains("needle lives only in the local search index"));
        assert!(tempdir.path().join("search.db").exists());
    }

    #[test]
    fn runtime_search_skips_termless_queries_without_opening_index() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Empty Query", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let search_path = tempdir.path().join("search.db");
        assert!(!search_path.exists());

        let search = runtime
            .search_workspace_messages(workspace_id.clone(), " \t!!! --- ")
            .unwrap();

        assert_eq!(search.workspace_id, workspace_id.0);
        assert_eq!(search.query, "!!! ---");
        assert!(search.hits.is_empty());
        assert!(!search_path.exists());
    }

    #[test]
    fn runtime_search_rejects_oversized_query_before_opening_index_or_history() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Query Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let search_path = tempdir.path().join("search.db");
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_oversized_search_tripwire",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );
        assert!(!search_path.exists());
        let oversized_query = "!".repeat(SEARCH_QUERY_MAX_BYTES + 1);

        let error = runtime
            .search_workspace_messages(workspace_id, oversized_query)
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::SearchQueryTooLarge {
                actual_bytes,
                max_bytes
            } if actual_bytes == SEARCH_QUERY_MAX_BYTES + 1
                && max_bytes == SEARCH_QUERY_MAX_BYTES
        ));
        assert!(!search_path.exists());
    }

    #[test]
    fn runtime_search_skips_history_materialization_when_index_has_no_hits() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search No Raw Hits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        insert_corrupt_event_json(tempdir.path(), &workspace_id, "evt_corrupt_no_hit_tripwire");
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let search = runtime
            .search_workspace_messages(workspace_id.clone(), "definitelyabsent")
            .unwrap();

        assert_eq!(search.workspace_id, workspace_id.0);
        assert_eq!(search.query, "definitelyabsent");
        assert!(search.hits.is_empty());
        assert!(tempdir.path().join("search.db").exists());
    }

    #[test]
    fn runtime_search_skips_history_materialization_when_raw_hits_are_stale() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Stale Raw Hits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        runtime
            .open_search_index()
            .unwrap()
            .index_message(
                &workspace_id,
                &ChannelId(created.channel_id),
                &MessageId("msg_stale_only".to_owned()),
                &EventId("evt_stale_only".to_owned()),
                1_000,
                "staleonly raw hit",
            )
            .unwrap();
        insert_corrupt_event_json(
            tempdir.path(),
            &workspace_id,
            "evt_corrupt_stale_hit_tripwire",
        );
        assert!(
            runtime
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let search = runtime
            .search_workspace_messages(workspace_id.clone(), "staleonly")
            .unwrap();

        assert_eq!(search.workspace_id, workspace_id.0);
        assert_eq!(search.query, "staleonly");
        assert!(search.hits.is_empty());
    }

    #[test]
    fn runtime_search_returns_newest_hits_first() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Search Order", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "needle older result",
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        runtime
            .send_message(workspace_id.clone(), channel_id, "needle newer result")
            .unwrap();

        let search = runtime
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(search.hits.len(), 2);
        assert_eq!(search.hits[0].body, "needle newer result");
        assert_eq!(search.hits[1].body, "needle older result");
        assert!(search.hits[0].physical_ms > search.hits[1].physical_ms);
    }

    #[test]
    fn runtime_search_limit_is_applied_after_stale_rows_are_filtered() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Stale Rows", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let index = runtime.open_search_index().unwrap();
        for index_value in 0..60 {
            index
                .index_message(
                    &workspace_id,
                    &channel_id,
                    &MessageId(format!("msg_stale_{index_value:03}")),
                    &EventId(format!("evt_stale_{index_value:03}")),
                    i64::from(index_value),
                    "needle stale row",
                )
                .unwrap();
        }
        drop(index);
        runtime
            .send_message(workspace_id.clone(), channel_id, "needle visible result")
            .unwrap();

        let search = runtime
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "needle visible result");
    }

    #[test]
    fn runtime_search_caps_visible_hits_and_preserves_bounded_counts() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Visible Cap", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        for index in 0..(LOCAL_SEARCH_VISIBLE_HIT_LIMIT + 5) {
            runtime
                .send_message(
                    workspace_id.clone(),
                    channel_id.clone(),
                    format!("needle visible result {index:03}"),
                )
                .unwrap();
        }

        let search = runtime
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(search.item_count, LOCAL_SEARCH_VISIBLE_HIT_LIMIT);
        assert_eq!(search.hits.len(), LOCAL_SEARCH_VISIBLE_HIT_LIMIT);
        assert_eq!(search.hit_count, LOCAL_SEARCH_VISIBLE_HIT_LIMIT + 5);
        assert_eq!(
            search.raw_candidate_count,
            LOCAL_SEARCH_VISIBLE_HIT_LIMIT + 5
        );
        assert_eq!(search.raw_candidate_limit, LOCAL_SEARCH_RAW_HIT_LIMIT);
        assert_eq!(search.visible_hit_limit, LOCAL_SEARCH_VISIBLE_HIT_LIMIT);
        assert!(!search.has_more_hits);
    }

    #[test]
    fn runtime_search_returns_bounded_body_snippets_with_length_metadata() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Body Preview", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let long_prefix = (0..120)
            .map(|index| format!("prefix{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let long_suffix = (0..120)
            .map(|index| format!("suffix{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let body = format!("{long_prefix} needle-search-context {long_suffix}");

        runtime
            .send_message(workspace_id.clone(), channel_id, &body)
            .unwrap();

        let search = runtime
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(search.hits.len(), 1);
        assert!(search.hits[0].body.contains("needle-search-context"));
        assert!(search.hits[0].body.len() < body.len());
        assert_eq!(search.hits[0].body_char_count, body.chars().count());
        assert!(search.hits[0].body_truncated);
    }

    #[test]
    fn runtime_search_ignores_invalid_self_contained_signature_events() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Integrity", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let message_id = MessageId::new();
        let mut forged_event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            runtime.identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "forged searchable needle".to_owned(),
                attachments: Vec::new(),
            },
        );
        forged_event.parents = vec![EventId(created.channel_event_id)];
        let mut forged = runtime.identity.sign_event(forged_event);
        forged.signature[0] ^= 1;
        let forged_event_id = forged.event_id.clone();
        runtime.store.append_event(&forged).unwrap();
        runtime
            .open_search_index()
            .unwrap()
            .index_message(
                &workspace_id,
                &channel_id,
                &message_id,
                &forged_event_id,
                forged.event.timestamp.physical_ms,
                "forged searchable needle",
            )
            .unwrap();

        let stale_hits = runtime
            .search_workspace_messages(workspace_id.clone(), "forged")
            .unwrap();
        let indexed = runtime
            .reindex_workspace_search(workspace_id.clone())
            .unwrap();
        let reindexed_hits = runtime
            .search_workspace_messages(workspace_id.clone(), "forged")
            .unwrap();
        let snapshot = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert!(stale_hits.hits.is_empty());
        assert_eq!(indexed.indexed_message_count, 0);
        assert!(reindexed_hits.hits.is_empty());
        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.invalid_signatures[0].event_id, forged_event_id.0);
        assert_eq!(snapshot.timeline[0].body, "Failed signature verification");
    }

    #[test]
    fn runtime_search_raw_limit_prefers_newest_indexed_rows() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Search Raw Ordering", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let index = runtime.open_search_index().unwrap();
        for index_value in 0..LOCAL_SEARCH_RAW_HIT_LIMIT {
            index
                .index_message(
                    &workspace_id,
                    &channel_id,
                    &MessageId(format!("msg_stale_{index_value:03}")),
                    &EventId(format!("evt_stale_{index_value:03}")),
                    i64::from(index_value as u32),
                    "needle stale row",
                )
                .unwrap();
        }
        drop(index);
        let first_visible = runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "needle visible older",
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let second_visible = runtime
            .send_message(workspace_id.clone(), channel_id, "needle visible newer")
            .unwrap();

        let search = runtime
            .search_workspace_messages(workspace_id, "needle")
            .unwrap();

        assert_eq!(search.hits.len(), 2);
        assert_eq!(search.item_count, 2);
        assert_eq!(search.hit_count, 2);
        assert_eq!(search.raw_candidate_count, LOCAL_SEARCH_RAW_HIT_LIMIT);
        assert!(search.has_more_hits);
        assert_eq!(search.hits[0].event_id, second_visible.event_id);
        assert_eq!(search.hits[0].body, "needle visible newer");
        assert_eq!(search.hits[1].event_id, first_visible.event_id);
        assert_eq!(search.hits[1].body, "needle visible older");
    }

    #[test]
    fn runtime_reuses_workspace_key_for_messages_after_restart() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "first encrypted body",
            )
            .unwrap();
        drop(runtime);

        let reopened = LocalRuntime::open(tempdir.path(), None).unwrap();
        reopened
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "second encrypted body",
            )
            .unwrap();

        let workspace_key =
            WorkspaceKey::load(&reopened.workspace_key_path(&workspace_id)).unwrap();
        let events = reopened.workspace_events(&workspace_id).unwrap();
        let encrypted_messages = events
            .iter()
            .filter_map(|event| match &event.event.body {
                EventBody::MessageCreatedEncrypted {
                    message_id,
                    sealed_markdown,
                    ..
                } => Some((message_id, sealed_markdown)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(encrypted_messages.len(), 2);
        for (_message_id, sealed_markdown) in &encrypted_messages {
            assert_eq!(sealed_markdown.mode, PayloadEncryption::Aes256GcmSiv);
            assert_eq!(sealed_markdown.key_id, workspace_key.key_id());
        }

        let first_plaintext = open_message_markdown(
            workspace_key.content_key(),
            encrypted_messages[0].1,
            &workspace_id,
            &channel_id,
            encrypted_messages[0].0,
        )
        .unwrap();
        let second_plaintext = open_message_markdown(
            workspace_key.content_key(),
            encrypted_messages[1].1,
            &workspace_id,
            &channel_id,
            encrypted_messages[1].0,
        )
        .unwrap();

        assert_eq!(first_plaintext, "first encrypted body");
        assert_eq!(second_plaintext, "second encrypted body");
    }

    #[test]
    fn runtime_rotates_workspace_key_and_keeps_prior_ciphertext_readable() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Rotating Keys", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        let first = runtime
            .send_message(workspace_id.clone(), channel_id.clone(), "before rotation")
            .unwrap();
        let rotated = runtime.rotate_workspace_key(workspace_id.clone()).unwrap();
        let second = runtime
            .send_message(workspace_id.clone(), channel_id.clone(), "after rotation")
            .unwrap();

        let workspace_key = WorkspaceKey::load(&runtime.workspace_key_path(&workspace_id)).unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let rotation_event = events
            .iter()
            .find(|event| event.event_id.0 == rotated.event_id)
            .unwrap();
        let encrypted_messages = events
            .iter()
            .filter_map(|event| match &event.event.body {
                EventBody::MessageCreatedEncrypted {
                    message_id,
                    sealed_markdown,
                    ..
                } => Some((event.event_id.0.as_str(), message_id, sealed_markdown)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(rotated.epoch, 2);
        assert_eq!(rotated.previous_key_id, encrypted_messages[0].2.key_id);
        assert_eq!(rotated.key_id, workspace_key.key_id());
        assert_eq!(encrypted_messages[1].2.key_id, rotated.key_id);
        assert!(matches!(
            &rotation_event.event.body,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: 2,
                key_id,
                previous_key_id,
                algorithm,
            } if key_id == &rotated.key_id
                && previous_key_id.as_deref() == Some(rotated.previous_key_id.as_str())
                && algorithm == CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV
        ));

        let first_key = workspace_key
            .resolve_content_key(&encrypted_messages[0].2.key_id)
            .unwrap();
        let second_key = workspace_key
            .resolve_content_key(&encrypted_messages[1].2.key_id)
            .unwrap();
        let first_opened = open_message_markdown(
            first_key.content_key(),
            encrypted_messages[0].2,
            &workspace_id,
            &channel_id,
            encrypted_messages[0].1,
        )
        .unwrap();
        let second_opened = open_message_markdown(
            second_key.content_key(),
            encrypted_messages[1].2,
            &workspace_id,
            &channel_id,
            encrypted_messages[1].1,
        )
        .unwrap();
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(encrypted_messages[0].0, first.event_id);
        assert_eq!(encrypted_messages[1].0, second.event_id);
        assert_eq!(first_opened, "before rotation");
        assert_eq!(second_opened, "after rotation");
        assert_eq!(decrypted.timeline[0].body, "before rotation");
        assert_eq!(decrypted.timeline[1].body, "after rotation");
    }

    #[test]
    fn exported_rotated_workspace_key_includes_history_for_imported_devices() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Shared Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "first shared epoch",
            )
            .unwrap();
        alice.rotate_workspace_key(workspace_id.clone()).unwrap();
        alice
            .send_message(workspace_id.clone(), channel_id, "second shared epoch")
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        assert_eq!(
            exported_key.schema_version,
            CONTENT_KEY_EXPORT_SCHEMA_VERSION
        );
        assert_eq!(exported_key.epoch, 2);
        assert_eq!(exported_key.previous_keys.len(), 1);
        assert!(
            bob.decrypted_workspace_snapshot(workspace_id.clone())
                .is_err()
        );

        bob.import_workspace_key(exported_key).unwrap();
        let decrypted = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(decrypted.timeline.len(), 2);
        assert_eq!(decrypted.timeline[0].body, "first shared epoch");
        assert_eq!(decrypted.timeline[1].body, "second shared epoch");
    }

    #[test]
    fn workspace_recovery_bundle_imports_workspace_and_private_channel_keys() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Recovery", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id);
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id);

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                public_channel_id,
                "recover public note",
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "recover private note",
            )
            .unwrap();
        let bundle = alice
            .export_workspace_recovery_bundle(workspace_id.clone(), "correct horse battery staple")
            .unwrap();
        assert_eq!(bundle.kdf.name, RECOVERY_BUNDLE_KDF_ARGON2ID);
        assert_eq!(
            bundle.kdf.memory_cost_kib,
            RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB
        );
        assert_eq!(bundle.kdf.time_cost, RECOVERY_BUNDLE_ARGON2_TIME_COST);
        assert_eq!(bundle.kdf.parallelism, RECOVERY_BUNDLE_ARGON2_PARALLELISM);
        assert_eq!(bundle.kdf.output_len, RECOVERY_BUNDLE_KDF_OUTPUT_LEN);

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        assert!(
            bob.decrypted_workspace_snapshot(workspace_id.clone())
                .is_err()
        );

        let imported = bob
            .import_workspace_recovery_bundle(bundle, "correct horse battery staple")
            .unwrap();
        let decrypted = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(imported.imported_channel_count, 1);
        assert_eq!(imported.imported_channel_ids, vec![private_channel_id.0]);
        assert!(
            decrypted
                .timeline
                .iter()
                .any(|item| item.body == "recover public note")
        );
        assert!(
            decrypted
                .timeline
                .iter()
                .any(|item| item.body == "recover private note")
        );
    }

    #[test]
    fn workspace_recovery_bundle_imports_legacy_blake3_kdf_bundle() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Legacy Recovery", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "legacy bundle plaintext",
            )
            .unwrap();
        let plaintext = WorkspaceRecoveryBundlePlaintext {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_key: alice.export_workspace_key(workspace_id.clone()).unwrap(),
            channel_keys: Vec::new(),
        };
        let plaintext = serde_json::to_vec(&plaintext).unwrap();
        let kdf = WorkspaceRecoveryBundleKdf {
            name: RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY.to_owned(),
            context: RECOVERY_BUNDLE_KDF_CONTEXT.to_owned(),
            salt: vec![9; RECOVERY_BUNDLE_SALT_LEN],
            memory_cost_kib: 0,
            time_cost: 0,
            parallelism: 0,
            output_len: 0,
        };
        let exporter_device_id = alice.device_id().clone();
        let wrapping_key = derive_recovery_bundle_key("legacy passphrase", &kdf).unwrap();
        let sealed_payload = seal_aes_256_gcm_siv(
            recovery_bundle_key_id(&workspace_id),
            &wrapping_key,
            &plaintext,
            &recovery_bundle_aad(
                &workspace_id,
                &exporter_device_id,
                kdf.name.as_str(),
                kdf.context.as_str(),
                &kdf.salt,
            ),
        )
        .unwrap();
        let bundle = WorkspaceRecoveryBundle {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_id: workspace_id.0.clone(),
            exporter_device_id: exporter_device_id.0,
            kdf,
            sealed_payload,
        };

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let imported = bob
            .import_workspace_recovery_bundle(bundle, "legacy passphrase")
            .unwrap();
        let decrypted = bob.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(imported.imported_channel_count, 0);
        assert_eq!(imported.imported_channel_ids, Vec::<String>::new());
        assert_eq!(decrypted.timeline[0].body, "legacy bundle plaintext");
    }

    #[test]
    fn workspace_recovery_bundle_wrong_passphrase_does_not_install_keys() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Recovery Failure", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "unavailable without key",
            )
            .unwrap();
        let bundle = alice
            .export_workspace_recovery_bundle(workspace_id.clone(), "right passphrase")
            .unwrap();

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let error = bob
            .import_workspace_recovery_bundle(bundle, "wrong passphrase")
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::Crypto(CryptoError::OpenFailed)
        ));
        assert!(bob.decrypted_workspace_snapshot(workspace_id).is_err());
    }

    #[test]
    fn workspace_rotation_keeps_old_attachments_savable() {
        const ATTACHMENT_TEXT: &str = "attachment from prior epoch";
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("epoch-one.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Attachment Epochs", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "old attachment",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let events_before_rotation = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } =
            &events_before_rotation[2].event.body
        else {
            panic!("expected encrypted message event");
        };
        let old_attachment = attachments[0].clone();

        runtime.rotate_workspace_key(workspace_id.clone()).unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "new key message",
            )
            .unwrap();
        let saved_path = tempdir.path().join("saved-old-attachment.txt");
        runtime
            .save_attachment_to_file(
                workspace_id,
                MessageId(sent.message_id),
                old_attachment.blob_hash,
                &saved_path,
            )
            .unwrap();

        assert_eq!(fs::read_to_string(saved_path).unwrap(), ATTACHMENT_TEXT);
    }

    #[test]
    fn private_channel_messages_use_channel_key_not_workspace_key() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Private Keys", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "channel scoped secret",
            )
            .unwrap();

        let workspace_key = WorkspaceKey::load(&runtime.workspace_key_path(&workspace_id)).unwrap();
        let channel_key =
            ChannelKey::load(&runtime.channel_key_path(&workspace_id, &private_channel_id))
                .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted private message event");
        };

        assert_eq!(sealed_markdown.key_id, channel_key.key_id);
        assert_ne!(sealed_markdown.key_id, workspace_key.key_id);
        assert!(
            open_message_markdown(
                &workspace_key.content_key,
                sealed_markdown,
                &workspace_id,
                &private_channel_id,
                message_id,
            )
            .is_err()
        );
        let opened = open_message_markdown(
            &channel_key.content_key,
            sealed_markdown,
            &workspace_id,
            &private_channel_id,
            message_id,
        )
        .unwrap();

        assert_eq!(opened, "channel scoped secret");
    }

    #[test]
    fn runtime_rotates_private_channel_key_independently() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Private Key Epochs", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private first epoch",
            )
            .unwrap();
        let rotated = runtime
            .rotate_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private second epoch",
            )
            .unwrap();

        let workspace_key = WorkspaceKey::load(&runtime.workspace_key_path(&workspace_id)).unwrap();
        let channel_key =
            ChannelKey::load(&runtime.channel_key_path(&workspace_id, &private_channel_id))
                .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let rotation_event = events
            .iter()
            .find(|event| event.event_id.0 == rotated.event_id)
            .unwrap();
        let encrypted_messages = events
            .iter()
            .filter_map(|event| match &event.event.body {
                EventBody::MessageCreatedEncrypted {
                    message_id,
                    sealed_markdown,
                    ..
                } if event.event.channel_id.as_ref() == Some(&private_channel_id) => {
                    Some((message_id, sealed_markdown))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(rotated.epoch, 2);
        assert_eq!(rotated.key_id, channel_key.key_id);
        assert_eq!(encrypted_messages[0].1.key_id, rotated.previous_key_id);
        assert_eq!(encrypted_messages[1].1.key_id, rotated.key_id);
        assert!(matches!(
            &rotation_event.event.body,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Channel { channel_id },
                epoch: 2,
                key_id,
                previous_key_id,
                algorithm,
            } if channel_id == &private_channel_id
                && key_id == &rotated.key_id
                && previous_key_id.as_deref() == Some(rotated.previous_key_id.as_str())
                && algorithm == CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV
        ));
        assert!(
            workspace_key
                .resolve_content_key(&encrypted_messages[0].1.key_id)
                .is_none()
        );

        let first_key = channel_key
            .resolve_content_key(&encrypted_messages[0].1.key_id)
            .unwrap();
        let second_key = channel_key
            .resolve_content_key(&encrypted_messages[1].1.key_id)
            .unwrap();
        let first_opened = open_message_markdown(
            first_key.content_key(),
            encrypted_messages[0].1,
            &workspace_id,
            &private_channel_id,
            encrypted_messages[0].0,
        )
        .unwrap();
        let second_opened = open_message_markdown(
            second_key.content_key(),
            encrypted_messages[1].1,
            &workspace_id,
            &private_channel_id,
            encrypted_messages[1].0,
        )
        .unwrap();
        let decrypted = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(first_opened, "private first epoch");
        assert_eq!(second_opened, "private second epoch");
        assert!(
            decrypted
                .timeline
                .iter()
                .any(|item| item.body == "private first epoch")
        );
        assert!(
            decrypted
                .timeline
                .iter()
                .any(|item| item.body == "private second epoch")
        );
    }

    #[test]
    fn runtime_rotates_workspace_manual_keys_for_suspected_compromise() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Manual Compromise Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id.clone(),
                "public before compromise rotation",
            )
            .unwrap();
        runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private before compromise rotation",
            )
            .unwrap();
        let old_workspace_key = runtime.export_workspace_key(workspace_id.clone()).unwrap();
        let old_channel_key = runtime
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();

        let rotated = runtime
            .rotate_workspace_manual_keys(workspace_id.clone())
            .unwrap();
        let public_after = runtime
            .send_message(
                workspace_id.clone(),
                public_channel_id,
                "public after compromise rotation",
            )
            .unwrap();
        let private_after = runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private after compromise rotation",
            )
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let workspace_rotation_index = events
            .iter()
            .position(|event| event.event_id.0 == rotated.workspace_key_rotation.event_id)
            .unwrap();
        let channel_rotation_index = events
            .iter()
            .position(|event| event.event_id.0 == rotated.channel_key_rotations[0].event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: public_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == public_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted public message");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: private_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == private_after.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted private message");
        };
        let decrypted = runtime
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let timeline_bodies = decrypted
            .timeline
            .iter()
            .map(|item| item.body.as_str())
            .collect::<Vec<_>>();

        assert_eq!(rotated.workspace_id, workspace_id.0);
        assert_eq!(
            rotated.workspace_key_rotation.previous_key_id,
            old_workspace_key.key_id
        );
        assert_eq!(rotated.workspace_key_rotation.epoch, 2);
        assert_eq!(rotated.channel_key_rotations.len(), 1);
        assert_eq!(
            rotated.channel_key_rotations[0].previous_key_id,
            old_channel_key.key_id
        );
        assert_eq!(rotated.channel_key_rotations[0].epoch, 2);
        assert_eq!(
            rotated.rotated_event_ids,
            vec![
                rotated.workspace_key_rotation.event_id.clone(),
                rotated.channel_key_rotations[0].event_id.clone()
            ]
        );
        assert!(workspace_rotation_index < channel_rotation_index);
        assert_eq!(public_sealed.key_id, rotated.workspace_key_rotation.key_id);
        assert_eq!(
            private_sealed.key_id,
            rotated.channel_key_rotations[0].key_id
        );
        assert!(timeline_bodies.contains(&"public before compromise rotation"));
        assert!(timeline_bodies.contains(&"private before compromise rotation"));
        assert!(timeline_bodies.contains(&"public after compromise rotation"));
        assert!(timeline_bodies.contains(&"private after compromise rotation"));
        assert!(matches!(
            &events[workspace_rotation_index].event.body,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: 2,
                key_id,
                previous_key_id,
                algorithm,
            } if key_id == &rotated.workspace_key_rotation.key_id
                && previous_key_id.as_deref()
                    == Some(rotated.workspace_key_rotation.previous_key_id.as_str())
                && algorithm == CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV
        ));
        assert!(matches!(
            &events[channel_rotation_index].event.body,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Channel { channel_id },
                epoch: 2,
                key_id,
                previous_key_id,
                algorithm,
            } if channel_id == &private_channel_id
                && key_id == &rotated.channel_key_rotations[0].key_id
                && previous_key_id.as_deref()
                    == Some(rotated.channel_key_rotations[0].previous_key_id.as_str())
                && algorithm == CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV
        ));
    }

    #[test]
    fn runtime_creates_additional_channel_with_causal_parent() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Chaft", "general").unwrap();
        let channel = runtime
            .create_channel(WorkspaceId(created.workspace_id.clone()), "ops", true)
            .unwrap();
        let snapshot = runtime
            .workspace_snapshot(WorkspaceId(created.workspace_id.clone()))
            .unwrap();
        let events = runtime
            .workspace_events(&WorkspaceId(created.workspace_id))
            .unwrap();

        assert_eq!(channel.workspace_id, snapshot.workspace_id);
        assert!(
            snapshot
                .channels
                .iter()
                .any(|channel| channel.name == "ops")
        );
        assert_eq!(events[2].event.parents, vec![events[1].event_id.clone()]);
    }

    #[test]
    fn runtime_new_events_join_concurrent_workspace_heads() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Concurrent", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let shared_parent = events[1].event_id.clone();

        let mut ops_channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_ops".to_owned()),
                name: "ops".to_owned(),
                is_private: false,
            },
        );
        ops_channel.parents = vec![shared_parent.clone()];
        let ops_channel = runtime.identity.sign_event(ops_channel);
        runtime.store.append_event(&ops_channel).unwrap();

        let mut design_channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_design".to_owned()),
                name: "design".to_owned(),
                is_private: false,
            },
        );
        design_channel.parents = vec![shared_parent];
        let design_channel = runtime.identity.sign_event(design_channel);
        runtime.store.append_event(&design_channel).unwrap();

        let sent = runtime
            .send_message(workspace_id.clone(), channel_id, "joins both heads")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let sent_event = events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();

        let mut expected_parents = vec![
            design_channel.event_id.clone(),
            ops_channel.event_id.clone(),
        ];
        expected_parents.sort();

        assert_eq!(sent_event.event.parents, expected_parents);
    }

    #[test]
    fn runtime_new_events_ignore_incomplete_history_heads() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Partial Heads", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let first = runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "complete local head",
            )
            .unwrap();

        let mut incomplete_channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            runtime.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_incomplete_slice".to_owned()),
                name: "incomplete-slice".to_owned(),
                is_private: false,
            },
        );
        incomplete_channel.parents = vec![EventId("evt_missing_parent".to_owned())];
        let incomplete_channel = runtime.identity.sign_event(incomplete_channel);
        runtime.store.append_event(&incomplete_channel).unwrap();

        let second = runtime
            .send_message(workspace_id.clone(), channel_id, "after partial gap")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let second_event = events
            .iter()
            .find(|event| event.event_id.0 == second.event_id)
            .unwrap();
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(
            second_event.event.parents,
            vec![EventId(first.event_id.clone())]
        );
        assert!(
            !second_event
                .event
                .parents
                .contains(&incomplete_channel.event_id)
        );
        let message_bodies = snapshot
            .timeline
            .iter()
            .filter(|item| item.kind == chaft_app::TimelineItemKind::EncryptedMessage)
            .map(|item| item.body.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            message_bodies,
            vec!["complete local head", "after partial gap"]
        );
        assert_eq!(snapshot.gaps.len(), 1);
    }

    #[test]
    fn runtime_new_events_ignore_invalid_signature_heads() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Invalid Signature Heads", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let first = runtime
            .send_message(workspace_id.clone(), channel_id.clone(), "valid local head")
            .unwrap();

        let mut forged_head = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            runtime.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged local head".to_owned(),
                attachments: Vec::new(),
            },
        );
        forged_head.parents = vec![EventId(first.event_id.clone())];
        let mut forged_head = runtime.identity.sign_event(forged_head);
        forged_head.signature[0] ^= 1;
        let forged_head_event_id = forged_head.event_id.clone();
        runtime.store.append_event(&forged_head).unwrap();

        let second = runtime
            .send_message(workspace_id.clone(), channel_id, "after forged head")
            .unwrap();
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let second_event = events
            .iter()
            .find(|event| event.event_id.0 == second.event_id)
            .unwrap();
        let snapshot = runtime.decrypted_workspace_snapshot(workspace_id).unwrap();

        assert_eq!(
            second_event.event.parents,
            vec![EventId(first.event_id.clone())]
        );
        assert!(!second_event.event.parents.contains(&forged_head_event_id));
        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(
            snapshot.invalid_signatures[0].event_id,
            forged_head_event_id.0
        );
        let message_bodies = snapshot
            .timeline
            .iter()
            .filter(|item| item.kind == chaft_app::TimelineItemKind::EncryptedMessage)
            .map(|item| item.body.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            message_bodies,
            vec!["valid local head", "after forged head"]
        );
    }

    #[tokio::test]
    async fn runtime_pulls_workspace_from_peer_and_ignores_other_workspaces() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let primary = alice.create_workspace("Primary", "general").unwrap();
        alice
            .invite_member(
                WorkspaceId(primary.workspace_id.clone()),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .send_message(
                WorkspaceId(primary.workspace_id.clone()),
                ChannelId(primary.channel_id.clone()),
                "bob should not receive plaintext",
            )
            .unwrap();
        let other = alice.create_workspace("Other", "random").unwrap();
        drop(alice);

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

        let transport = DirectTransport;
        let pulled = bob
            .pull_workspace_from_peer(&transport, &peer, WorkspaceId(primary.workspace_id.clone()))
            .await
            .unwrap();

        assert_eq!(pulled.workspace_id, primary.workspace_id);
        assert_eq!(pulled.fetched_event_ids.len(), 4);
        assert!(pulled.ignored_event_ids.is_empty());
        assert!(pulled.gaps.is_empty());

        let snapshot = bob
            .workspace_snapshot(WorkspaceId(primary.workspace_id.clone()))
            .unwrap();
        assert_eq!(snapshot.name, "Primary");
        assert_eq!(snapshot.channels[0].name, "general");
        assert_eq!(
            snapshot.timeline[0].kind,
            chaft_app::TimelineItemKind::EncryptedMessage
        );
        assert_eq!(snapshot.timeline[0].body, "Encrypted message");

        let primary_events = bob
            .workspace_events(&WorkspaceId(primary.workspace_id.clone()))
            .unwrap();
        let other_events = bob
            .workspace_events(&WorkspaceId(other.workspace_id))
            .unwrap();
        let primary_json = serde_json::to_string(&primary_events).unwrap();
        assert_eq!(primary_events.len(), 4);
        assert!(other_events.is_empty());
        assert!(!primary_json.contains("bob should not receive plaintext"));

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn runtime_exports_root_signed_trust_snapshot_from_materialized_history() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Trust Snapshot", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();

        let snapshot = runtime.export_trust_snapshot(workspace_id.clone()).unwrap();

        assert_eq!(snapshot.snapshot.workspace_id, workspace_id);
        assert_eq!(
            snapshot.root_event.event_id,
            snapshot.snapshot.root_event_id
        );
        assert_eq!(
            snapshot.snapshot.root_author_device_id,
            runtime.device_id().clone()
        );
        assert!(
            snapshot
                .snapshot
                .channels
                .iter()
                .any(|channel| channel.channel_id.0 == created.channel_id && !channel.is_private)
        );
        assert!(snapshot.snapshot.channels.iter().any(|channel| {
            channel.channel_id.0 == private_channel.channel_id && channel.is_private
        }));
    }

    #[tokio::test]
    async fn runtime_publishes_single_event_with_trust_snapshot_to_partial_replica() {
        let alice_dir = tempfile::tempdir().unwrap();
        let replica_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Partial Snapshot Publish", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "later slice with compact proof",
            )
            .unwrap();

        let replica_store = EventStore::open(replica_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", replica_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("partial-replica".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_event_direct_with_trust_snapshot(
                &transport,
                &peer,
                workspace_id,
                EventId(sent.event_id.clone()),
            )
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();

        assert_eq!(published.published_event_ids, vec![sent.event_id.clone()]);
        assert_eq!(inventory, vec![EventId(sent.event_id)]);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_backs_up_content_slices_with_trust_snapshot() {
        let alice_dir = tempfile::tempdir().unwrap();
        let replica_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Partial Snapshot Backup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        let profile = alice
            .update_device_profile(workspace_id.clone(), "Replica Mira")
            .unwrap();
        let sent = alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "backup slice with compact proof",
            )
            .unwrap();
        let reaction = alice
            .add_reaction(
                workspace_id.clone(),
                MessageId(sent.message_id.clone()),
                "+1",
            )
            .unwrap();

        let replica_store = EventStore::open(replica_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", replica_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("partial-backup".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let repeated = alice
            .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id)
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();

        assert_eq!(
            published.published_event_ids,
            vec![
                profile.event_id.clone(),
                sent.event_id.clone(),
                reaction.event_id.clone()
            ]
        );
        assert!(repeated.published_event_ids.is_empty());
        assert_eq!(
            inventory,
            vec![
                EventId(profile.event_id),
                EventId(sent.event_id),
                EventId(reaction.event_id)
            ]
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_backup_chunks_large_slices_with_chunk_scoped_trust_snapshots() {
        let alice_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chunked Snapshot Backup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let general_channel_id = ChannelId(created.channel_id.clone());
        let mut expected_event_ids = Vec::new();

        for index in 0..MAX_PUBLISH_EVENTS_PER_REQUEST {
            let sent = alice
                .send_message(
                    workspace_id.clone(),
                    general_channel_id.clone(),
                    format!("general backup event {index}"),
                )
                .unwrap();
            expected_event_ids.push(sent.event_id);
        }

        let overflow_channel = alice
            .create_channel(workspace_id.clone(), "overflow", false)
            .unwrap();
        let overflow_channel_id = ChannelId(overflow_channel.channel_id.clone());
        let overflow = alice
            .send_message(
                workspace_id.clone(),
                overflow_channel_id.clone(),
                "overflow backup event",
            )
            .unwrap();
        expected_event_ids.push(overflow.event_id);

        let transport = CapturingBackupTransport::default();
        let peer = PeerAddress {
            peer_id: PeerId("captured-backup".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };

        let published = alice
            .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let publishes = transport.publishes();
        let captured_event_ids = publishes
            .iter()
            .flat_map(|publish| publish.events.iter().map(|event| event.event_id.0.clone()))
            .collect::<Vec<_>>();

        assert_eq!(published.published_event_ids, expected_event_ids);
        assert_eq!(captured_event_ids, published.published_event_ids);
        assert_eq!(publishes.len(), 2);
        assert_eq!(publishes[0].events.len(), MAX_PUBLISH_EVENTS_PER_REQUEST);
        assert_eq!(publishes[1].events.len(), 1);
        assert_eq!(publishes[0].snapshot.snapshot.channels.len(), 1);
        assert_eq!(
            publishes[0].snapshot.snapshot.channels[0].channel_id,
            general_channel_id
        );
        assert_eq!(publishes[1].snapshot.snapshot.channels.len(), 1);
        assert_eq!(
            publishes[1].snapshot.snapshot.channels[0].channel_id,
            overflow_channel_id
        );
        for publish in publishes {
            chaft_identity::verify_self_contained_trust_snapshot(&publish.snapshot).unwrap();
            for event in &publish.events {
                assert!(
                    chaft_core::authorize_event_with_trust_snapshot(
                        &publish.snapshot.snapshot,
                        event
                    )
                    .is_ok()
                );
            }
        }
    }

    #[tokio::test]
    async fn runtime_backup_repairs_blobs_for_already_backed_up_events() {
        const ATTACHMENT_TEXT: &str = "backup repair attachment secret";
        let alice_dir = tempfile::tempdir().unwrap();
        let replica_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("repair.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Partial Blob Backup Repair", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "backup should repair this blob",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let message_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .cloned()
            .expect("sent attachment event should exist");
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &message_event.event.body
        else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();

        let replica_store = EventStore::open(replica_dir.path().join("events.db")).unwrap();
        replica_store.append_event(&message_event).unwrap();
        let replica_blob_path = replica_dir.path().join("blobs");
        let replica_blobs = BlobStore::open(&replica_blob_path).unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", replica_store, replica_blobs)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("partial-blob-backup".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let repaired = alice
            .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id)
            .await
            .unwrap();

        assert!(repaired.published_event_ids.is_empty());
        assert_eq!(
            repaired.published_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(repaired.missing_blob_hashes.is_empty());
        assert_eq!(
            transport
                .fetch_blob(&peer, &attachment.blob_hash)
                .await
                .unwrap(),
            BlobStore::open(alice.paths().blob_store.clone())
                .unwrap()
                .get_bytes(&attachment.blob_hash)
                .unwrap()
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_backup_includes_openmls_update_slices() {
        let alice_dir = tempfile::tempdir().unwrap();
        let replica_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Backup Slices", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id);

        let key_package = alice
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id)
            .unwrap();
        let updated = alice
            .update_workspace_openmls_groups(workspace_id.clone())
            .unwrap();

        let replica_store = EventStore::open(replica_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", replica_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("partial-openmls-backup".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id)
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();
        let expected_event_ids = std::iter::once(key_package.event_id.clone())
            .chain(updated.updated_event_ids.clone())
            .collect::<Vec<_>>();

        assert_eq!(published.published_event_ids, expected_event_ids);
        assert_eq!(
            inventory,
            published
                .published_event_ids
                .into_iter()
                .map(EventId)
                .collect::<Vec<_>>()
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_direct_pull_uses_workspace_scoped_inventory() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let primary = alice.create_workspace("Primary", "general").unwrap();
        alice
            .send_message(
                WorkspaceId(primary.workspace_id.clone()),
                ChannelId(primary.channel_id.clone()),
                "bob should receive only this workspace",
            )
            .unwrap();
        let other = alice.create_workspace("Other", "random").unwrap();
        alice
            .send_message(
                WorkspaceId(other.workspace_id.clone()),
                ChannelId(other.channel_id.clone()),
                "unrelated workspace event",
            )
            .unwrap();
        drop(alice);

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let transport = DirectTransport;
        let pulled = bob
            .pull_workspace_direct(&transport, &peer, WorkspaceId(primary.workspace_id.clone()))
            .await
            .unwrap();

        assert_eq!(pulled.workspace_id, primary.workspace_id);
        assert_eq!(pulled.fetched_event_ids.len(), 3);
        assert!(pulled.ignored_event_ids.is_empty());
        assert!(pulled.gaps.is_empty());
        assert!(
            bob.workspace_events(&WorkspaceId(other.workspace_id))
                .unwrap()
                .is_empty()
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_rejects_oversized_direct_peer_endpoint_before_network() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Direct Endpoint Limits", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "direct endpoint validation",
            )
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("oversized-node".to_owned()),
            endpoint: "e".repeat(PEER_ENDPOINT_MAX_BYTES + 1),
        };
        let transport = DirectTransport;

        assert_oversized_peer_endpoint_error(
            runtime
                .publish_workspace_direct(&transport, &peer, workspace_id.clone())
                .await,
        );
        assert_oversized_peer_endpoint_error(
            runtime
                .publish_event_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    workspace_id.clone(),
                    EventId(sent.event_id),
                )
                .await,
        );
        assert_oversized_peer_endpoint_error(
            runtime
                .backup_workspace_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    workspace_id.clone(),
                )
                .await,
        );
        assert_oversized_peer_endpoint_error(
            runtime
                .pull_workspace_direct(&transport, &peer, workspace_id.clone())
                .await,
        );
        assert_oversized_peer_endpoint_error(
            runtime
                .sync_workspace_direct(&transport, &peer, workspace_id)
                .await,
        );
    }

    #[tokio::test]
    async fn runtime_rejects_unsupported_direct_peer_endpoint_before_network() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Direct Endpoint Policy", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "direct endpoint policy",
            )
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("central-node".to_owned()),
            endpoint: "https://central.example.invalid/sync".to_owned(),
        };
        let transport = DirectTransport;

        assert_unsupported_peer_endpoint_error(
            runtime
                .publish_workspace_direct(&transport, &peer, workspace_id.clone())
                .await,
        );
        assert_unsupported_peer_endpoint_error(
            runtime
                .publish_event_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    workspace_id.clone(),
                    EventId(sent.event_id),
                )
                .await,
        );
        assert_unsupported_peer_endpoint_error(
            runtime
                .backup_workspace_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    workspace_id.clone(),
                )
                .await,
        );
        assert_unsupported_peer_endpoint_error(
            runtime
                .pull_workspace_direct(&transport, &peer, workspace_id.clone())
                .await,
        );
        assert_unsupported_peer_endpoint_error(
            runtime
                .sync_workspace_direct(&transport, &peer, workspace_id.clone())
                .await,
        );
        assert_unsupported_peer_endpoint_error(
            runtime
                .retry_pending_blob_transfers_direct(&transport, workspace_id, &[peer])
                .await,
        );
    }

    #[tokio::test]
    async fn runtime_direct_publish_and_backup_reject_malformed_remote_inventory_before_publish() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Direct Inventory Validation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "remote inventory must be canonical",
            )
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("scripted-inventory".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let non_canonical =
            RemoteInventoryPublishTransport::new(vec![EventId("evt_NOT_CANONICAL".to_owned())]);

        assert_peer_protocol_error_contains(
            runtime
                .publish_workspace_direct(&non_canonical, &peer, workspace_id.clone())
                .await,
            "non-canonical inventory event id",
        );
        assert_eq!(non_canonical.publish_count(), 0);

        let duplicate_event_id = EventId(sent.event_id);
        let duplicate = RemoteInventoryPublishTransport::new(vec![
            duplicate_event_id.clone(),
            duplicate_event_id,
        ]);

        assert_peer_protocol_error_contains(
            runtime
                .backup_workspace_direct_with_trust_snapshot(&duplicate, &peer, workspace_id)
                .await,
            "duplicate inventory event id",
        );
        assert_eq!(duplicate.publish_count(), 0);
    }

    #[tokio::test]
    async fn runtime_rejects_oversized_direct_event_id_before_network() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let transport = DirectTransport;
        let peer = PeerAddress {
            peer_id: PeerId("peer".to_owned()),
            endpoint: "127.0.0.1:9".to_owned(),
        };

        assert_oversized_identifier_error(
            runtime
                .publish_event_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    WorkspaceId("wrk_missing".to_owned()),
                    EventId("e".repeat(chaft_types::EVENT_ID_MAX_BYTES + 1)),
                )
                .await,
            "event ID",
            chaft_types::EVENT_ID_MAX_BYTES,
        );
    }

    #[tokio::test]
    async fn runtime_rejects_oversized_direct_retry_peer_list_before_workspace_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let peers = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| {
                let endpoint = format!("direct+tcp://127.0.0.1:{}", 10_000 + index);
                PeerAddress {
                    peer_id: PeerId(endpoint.clone()),
                    endpoint,
                }
            })
            .collect::<Vec<_>>();
        let transport = DirectTransport;

        match runtime
            .retry_pending_blob_transfers_direct(
                &transport,
                WorkspaceId("wrk_missing_retry_limit".to_owned()),
                &peers,
            )
            .await
        {
            Err(RuntimeError::PeerEndpointListTooLarge {
                actual_count,
                max_count,
            }) => {
                assert_eq!(actual_count, PEER_ENDPOINT_LIST_MAX_ITEMS + 1);
                assert_eq!(max_count, PEER_ENDPOINT_LIST_MAX_ITEMS);
            }
            Ok(_) => panic!("expected oversized peer endpoint list error, got ok"),
            Err(error) => panic!("expected oversized peer endpoint list error, got {error}"),
        }
    }

    #[tokio::test]
    async fn runtime_direct_pull_skips_unmaterialized_gap_event_blobs() {
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let author = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let missing_parent_id = EventId("evt_missing_pull_parent".to_owned());
        let gap_blob_bytes = b"gap attachment bytes".to_vec();

        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let gap_blob_hash = node_blob_store.put_bytes(&gap_blob_bytes).unwrap().hash;
        let mut gap_event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            author.device_id().clone(),
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: SealedPayload {
                    mode: PayloadEncryption::DevelopmentPlaintext,
                    key_id: "gap-key".to_owned(),
                    nonce: Vec::new(),
                    aad: Vec::new(),
                    bytes: b"gap message".to_vec(),
                },
                attachments: vec![AttachmentRef {
                    blob_hash: gap_blob_hash.clone(),
                    media_type: "text/plain".to_owned(),
                    byte_len: gap_blob_bytes.len() as u64,
                    display_name: "gap.txt".to_owned(),
                    attachment_id: String::new(),
                    encryption: None,
                }],
            },
        );
        gap_event.parents = vec![missing_parent_id.clone()];
        let gap_event = author.sign_event(gap_event);
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        node_store.append_event(&gap_event).unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("partial-node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let transport = DirectTransport;
        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let bob_blob_store = BlobStore::open(bob.paths().blob_store.clone()).unwrap();

        assert_eq!(pulled.fetched_event_ids, vec![gap_event.event_id.0.clone()]);
        assert_eq!(pulled.gaps.len(), 1);
        assert_eq!(pulled.gaps[0].event_id, gap_event.event_id.0);
        assert_eq!(pulled.gaps[0].missing_parent_ids, vec![missing_parent_id.0]);
        assert!(pulled.fetched_blob_hashes.is_empty());
        assert!(pulled.missing_blob_hashes.is_empty());
        assert!(!bob_blob_store.has_blob(&gap_blob_hash).unwrap());

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn imported_workspace_key_allows_pulled_profile_to_decrypt() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Shared", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "shared secret body",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        assert_eq!(
            exported_key.schema_version,
            CONTENT_KEY_EXPORT_SCHEMA_VERSION
        );
        assert_eq!(exported_key.epoch, 1);
        assert_eq!(exported_key.workspace_id, created.workspace_id);
        assert_eq!(exported_key.aes_256_gcm_siv_key.len(), WORKSPACE_KEY_LEN);
        assert!(exported_key.previous_keys.is_empty());
        drop(alice);

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

        let transport = DirectTransport;
        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();

        let raw_snapshot = bob.workspace_snapshot(workspace_id.clone()).unwrap();
        assert_eq!(raw_snapshot.timeline[0].body, "Encrypted message");
        assert!(
            bob.decrypted_workspace_snapshot(workspace_id.clone())
                .is_err()
        );

        let imported = bob.import_workspace_key(exported_key).unwrap();
        assert_eq!(imported.workspace_id, created.workspace_id);
        let decrypted = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id.clone(), "shared secret")
            .unwrap();
        let bob_events_json =
            serde_json::to_string(&bob.workspace_events(&workspace_id).unwrap()).unwrap();

        assert_eq!(decrypted.timeline[0].body, "shared secret body");
        assert!(decrypted.timeline[0].encrypted);
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "shared secret body");
        assert!(!bob_events_json.contains("shared secret body"));

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_publish_and_pull_replicates_attachment_blobs() {
        const ATTACHMENT_TEXT: &str = "replicated attachment secret";
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("secret.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice.create_workspace("Blob Sync", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "attachment sync",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &alice_events[2].event.body
        else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();

        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let second_publish = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            published.published_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(published.missing_blob_hashes.is_empty());
        assert!(second_publish.published_event_ids.is_empty());
        assert!(second_publish.published_blob_hashes.is_empty());
        assert!(second_publish.missing_blob_hashes.is_empty());
        assert!(
            transport
                .fetch_blob(&peer, &attachment.blob_hash)
                .await
                .unwrap()
                .is_some()
        );

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            pulled.fetched_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(pulled.missing_blob_hashes.is_empty());
        bob.import_workspace_key(exported_key).unwrap();
        let bob_blob_store = BlobStore::open(bob.paths().blob_store.clone()).unwrap();
        let ciphertext = bob_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let workspace_key = WorkspaceKey::load(&bob.workspace_key_path(&workspace_id)).unwrap();
        let sealed = sealed_payload_from_encrypted_blob_ref(
            attachment.encryption.as_ref().unwrap(),
            ciphertext,
        );
        let opened = open_attachment_blob(
            workspace_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &MessageId(sent.message_id),
            0,
        )
        .unwrap();

        assert_eq!(opened, ATTACHMENT_TEXT.as_bytes());
        assert!(
            !serde_json::to_string(&bob.workspace_events(&workspace_id).unwrap())
                .unwrap()
                .contains(ATTACHMENT_TEXT)
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_pull_repairs_missing_blob_for_existing_local_event() {
        const ATTACHMENT_TEXT: &str = "locally missing replicated attachment";
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("repair-pull.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Pull Blob Repair", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "pull should repair this attachment",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &alice_events[2].event.body
        else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        for event in &alice_events {
            bob.store.append_event(event).unwrap();
        }
        assert!(
            BlobStore::open(bob.paths().blob_store.clone())
                .unwrap()
                .get_bytes(&attachment.blob_hash)
                .unwrap()
                .is_none()
        );

        let alice_store = EventStore::open(alice.paths().event_store.clone()).unwrap();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", alice_store, alice_blob_store)
                .await
                .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice-source".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id)
            .await
            .unwrap();

        assert!(pulled.requested_event_ids.is_empty());
        assert!(pulled.fetched_event_ids.is_empty());
        assert_eq!(
            pulled.fetched_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(pulled.missing_blob_hashes.is_empty());
        assert_eq!(
            BlobStore::open(bob.paths().blob_store.clone())
                .unwrap()
                .get_bytes(&attachment.blob_hash)
                .unwrap(),
            BlobStore::open(alice.paths().blob_store.clone())
                .unwrap()
                .get_bytes(&attachment.blob_hash)
                .unwrap()
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn iroh_bridge_publish_and_pull_replicates_attachment_blobs() {
        const ATTACHMENT_TEXT: &str = "iroh bridge attachment secret";
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("secret.txt");
        fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Iroh Bridge Blob Sync", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "bridge attachment sync",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &alice_events[2].event.body
        else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("iroh-bridge-node".to_owned()),
            endpoint: format!("direct+tcp://{}", server.local_addr().unwrap()),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = IrohTransport::default();

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            published.published_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(published.missing_blob_hashes.is_empty());

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            pulled.fetched_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(pulled.missing_blob_hashes.is_empty());

        bob.import_workspace_key(exported_key).unwrap();
        let bob_blob_store = BlobStore::open(bob.paths().blob_store.clone()).unwrap();
        let ciphertext = bob_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let workspace_key = WorkspaceKey::load(&bob.workspace_key_path(&workspace_id)).unwrap();
        let sealed = sealed_payload_from_encrypted_blob_ref(
            attachment.encryption.as_ref().unwrap(),
            ciphertext,
        );
        let opened = open_attachment_blob(
            workspace_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &MessageId(sent.message_id),
            0,
        )
        .unwrap();

        assert_eq!(opened, ATTACHMENT_TEXT.as_bytes());

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_publish_and_pull_replicates_large_attachment_as_chunks() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let large_attachment = vec![42; DIRECT_WHOLE_BLOB_SYNC_LIMIT + 1024];
        let attachment_path = alice_dir.path().join("large.bin");
        fs::write(&attachment_path, &large_attachment).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Large Blob Sync", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "large attachment sync",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &alice_events[2].event.body
        else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();

        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let second_publish = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let replica_blobs = BlobStore::open(&node_blob_path).unwrap();
        let replica_availability = replica_blobs
            .availability(&attachment.blob_hash)
            .unwrap()
            .unwrap();

        assert_eq!(
            published.published_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(second_publish.published_event_ids.is_empty());
        assert!(second_publish.published_blob_hashes.is_empty());
        assert!(second_publish.missing_blob_hashes.is_empty());
        assert!(!replica_blobs.has_blob(&attachment.blob_hash).unwrap());
        assert!(replica_availability.is_complete());

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            pulled.fetched_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(pulled.missing_blob_hashes.is_empty());

        bob.import_workspace_key(exported_key).unwrap();
        let bob_blob_store = BlobStore::open(bob.paths().blob_store.clone()).unwrap();
        let ciphertext = bob_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let workspace_key = WorkspaceKey::load(&bob.workspace_key_path(&workspace_id)).unwrap();
        let sealed = sealed_payload_from_encrypted_blob_ref(
            attachment.encryption.as_ref().unwrap(),
            ciphertext,
        );
        let opened = open_attachment_blob(
            workspace_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &MessageId(sent.message_id),
            0,
        )
        .unwrap();

        assert_eq!(opened, large_attachment);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_publish_resumes_partial_chunked_attachment_upload() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let large_attachment = vec![7; DIRECT_WHOLE_BLOB_SYNC_LIMIT + 1024];
        let attachment_path = alice_dir.path().join("resume-large.bin");
        fs::write(&attachment_path, &large_attachment).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Resumable Blob Publish", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id.clone(),
                "resume large attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let descriptor = describe_blob(&ciphertext, DIRECT_BLOB_CHUNK_SIZE);

        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        node_blob_store.put_manifest(&descriptor).unwrap();
        node_blob_store
            .put_chunk_with_hash(
                &descriptor.chunk_hashes[0],
                &ciphertext[..descriptor.chunk_size],
            )
            .unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let replica_blobs = BlobStore::open(&node_blob_path).unwrap();
        let replica_availability = replica_blobs
            .availability(&attachment.blob_hash)
            .unwrap()
            .unwrap();

        assert_eq!(
            published.published_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(published.missing_blob_hashes.is_empty());
        assert_eq!(published.blob_transfer_attempts.len(), 1);
        let transfer_attempt = &published.blob_transfer_attempts[0];
        assert_eq!(transfer_attempt.workspace_id, workspace_id.0);
        assert_eq!(transfer_attempt.peer_endpoint, peer.endpoint);
        assert_eq!(transfer_attempt.blob_hash, attachment.blob_hash);
        assert_eq!(transfer_attempt.mode, BlobTransferMode::ChunkedBlob);
        assert_eq!(transfer_attempt.status, BlobTransferStatus::Succeeded);
        assert_eq!(transfer_attempt.attempt_count, 1);
        assert_eq!(
            transfer_attempt.chunk_size,
            Some(DIRECT_BLOB_CHUNK_SIZE as u64)
        );
        assert_eq!(transfer_attempt.chunk_count, descriptor.chunk_hashes.len());
        assert_eq!(transfer_attempt.chunk_hashes, descriptor.chunk_hashes);
        assert_eq!(transfer_attempt.remote_available_chunk_count, 1);
        assert_eq!(
            transfer_attempt.remote_available_chunk_hashes,
            vec![descriptor.chunk_hashes[0].clone()]
        );
        assert_eq!(
            transfer_attempt.planned_chunk_count,
            descriptor.chunk_hashes.len() - 1
        );
        assert_eq!(
            transfer_attempt.planned_chunk_hashes,
            descriptor.chunk_hashes[1..].to_vec()
        );
        assert!(transfer_attempt.finished_at_unix_ms.is_some());
        assert!(transfer_attempt.error.is_none());
        assert!(!replica_blobs.has_blob(&attachment.blob_hash).unwrap());
        assert!(replica_availability.is_complete());
        assert_eq!(
            replica_blobs
                .get_bytes_chunked(&attachment.blob_hash)
                .unwrap(),
            Some(ciphertext)
        );
        let reopened_alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let transfer_ledger = reopened_alice.blob_transfer_ledger().unwrap();
        assert_eq!(
            transfer_ledger.schema_version,
            BLOB_TRANSFER_LEDGER_SCHEMA_VERSION
        );
        assert_eq!(transfer_ledger.entries, published.blob_transfer_attempts);

        let pulled = bob
            .pull_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        let saved_path = bob_dir.path().join("restored-large.bin");
        bob.save_attachment_to_file(
            workspace_id,
            MessageId(sent.message_id),
            attachment.blob_hash.clone(),
            &saved_path,
        )
        .unwrap();

        assert_eq!(pulled.fetched_blob_hashes, vec![attachment.blob_hash]);
        assert_eq!(fs::read(saved_path).unwrap(), large_attachment);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_publish_reconciles_completed_stale_chunked_blob_transfer() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let large_attachment = vec![9; DIRECT_WHOLE_BLOB_SYNC_LIMIT + 1024];
        let attachment_path = alice_dir.path().join("already-uploaded-large.bin");
        fs::write(&attachment_path, &large_attachment).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Reconcile Blob Publish", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "already uploaded large attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let descriptor = describe_blob(&ciphertext, DIRECT_BLOB_CHUNK_SIZE);

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        node_blob_store
            .put_bytes_chunked(&ciphertext, DIRECT_BLOB_CHUNK_SIZE)
            .unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let stale_attempt = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &peer,
                &attachment.blob_hash,
                BlobTransferMode::ChunkedBlob,
                ciphertext.len() as u64,
                Some(DIRECT_BLOB_CHUNK_SIZE as u64),
                descriptor.chunk_hashes.clone(),
                descriptor.chunk_hashes.clone(),
                Vec::new(),
            )
            .unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();

        assert!(published.published_blob_hashes.is_empty());
        assert!(published.missing_blob_hashes.is_empty());
        assert_eq!(published.blob_transfer_attempts.len(), 1);
        let reconciled = &published.blob_transfer_attempts[0];
        assert_eq!(reconciled.attempt_id, stale_attempt.attempt_id);
        assert_eq!(reconciled.status, BlobTransferStatus::Succeeded);
        assert!(reconciled.finished_at_unix_ms.is_some());
        assert!(reconciled.error.is_none());
        assert_eq!(
            alice.blob_transfer_ledger().unwrap().entries,
            vec![reconciled.clone()]
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_reconciles_failed_attempt_when_peer_has_blob() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"already recovered blob".to_vec();
        let attachment_path = alice_dir.path().join("already-recovered.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Reconcile Failed Blob Retry", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry already recovered attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        node_blob_store
            .put_bytes_with_hash(&attachment.blob_hash, &ciphertext)
            .unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let failed = alice
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some("connection reset before ack".to_owned()),
            )
            .unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id.clone(),
                std::slice::from_ref(&peer),
            )
            .await
            .unwrap();

        assert_eq!(retry.pending_attempt_ids, vec![failed.attempt_id.clone()]);
        assert!(retry.retried_blob_hashes.is_empty());
        assert_eq!(retry.reconciled_blob_hashes, vec![attachment.blob_hash]);
        assert!(retry.missing_blob_hashes.is_empty());
        assert!(retry.skipped_blob_hashes.is_empty());
        assert!(retry.peer_errors.is_empty());
        assert_eq!(retry.blob_transfer_attempts.len(), 1);
        let reconciled = &retry.blob_transfer_attempts[0];
        assert_eq!(reconciled.attempt_id, failed.attempt_id);
        assert_eq!(reconciled.status, BlobTransferStatus::Succeeded);
        assert!(reconciled.finished_at_unix_ms.is_some());
        assert!(reconciled.error.is_none());
        assert_eq!(
            alice.blob_transfer_ledger().unwrap().entries,
            vec![reconciled.clone()]
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_marks_protocol_peer_errors() {
        let alice_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"protocol failed retry blob".to_vec();
        let attachment_path = alice_dir.path().join("protocol-retry.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Protocol Retry Peer", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry protocol error attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", EventStore::open_in_memory().unwrap())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("protocol-error-node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        alice
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some("needs retry".to_owned()),
            )
            .unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id,
                std::slice::from_ref(&peer),
            )
            .await
            .unwrap();

        assert_eq!(retry.peer_errors.len(), 1);
        let peer_error = &retry.peer_errors[0];
        assert_eq!(peer_error.peer_endpoint, peer.endpoint);
        assert_eq!(peer_error.blob_hash, attachment.blob_hash);
        assert!(peer_error.message.contains("blob store unavailable"));
        assert!(peer_error.suspect_protocol_error);
        assert!(retry.retried_blob_hashes.is_empty());
        assert!(retry.reconciled_blob_hashes.is_empty());

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_caps_peer_error_messages() {
        let alice_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"oversized peer error retry blob".to_vec();
        let attachment_path = alice_dir.path().join("oversized-peer-error.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Oversized Retry Peer Error", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry oversized peer error attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("oversized-error-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        alice
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some("needs retry".to_owned()),
            )
            .unwrap();
        let transport = CountingCompleteAvailabilityTransport::failing(
            attachment.blob_hash.clone(),
            "é".repeat(BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES),
        );

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id,
                std::slice::from_ref(&peer),
            )
            .await
            .unwrap();

        assert_eq!(transport.fetch_count(), 1);
        assert_eq!(retry.peer_errors.len(), 1);
        assert_eq!(
            retry.peer_errors[0].message.len(),
            BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES
        );
        assert!(
            retry.peer_errors[0]
                .message
                .is_char_boundary(retry.peer_errors[0].message.len())
        );
        assert!(retry.peer_errors[0].suspect_protocol_error);
        assert!(retry.blob_transfer_attempts.is_empty());
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_reconciles_failed_attempt_from_fallback_peer() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"fallback already has blob".to_vec();
        let attachment_path = alice_dir.path().join("fallback-recovered.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Fallback Reconcile Blob Retry", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry fallback recovered attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        node_blob_store
            .put_bytes_with_hash(&attachment.blob_hash, &ciphertext)
            .unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let fallback_peer = PeerAddress {
            peer_id: PeerId("fallback-node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let stale_peer = PeerAddress {
            peer_id: PeerId("offline-node".to_owned()),
            endpoint: "127.0.0.1:1".to_owned(),
        };
        let started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let failed = alice
            .record_blob_transfer_finished(
                &started,
                BlobTransferStatus::Failed,
                Some("offline peer".to_owned()),
            )
            .unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let retry_peers = vec![fallback_peer.clone(), stale_peer.clone()];
        let retry = alice
            .retry_pending_blob_transfers_direct(&transport, workspace_id.clone(), &retry_peers)
            .await
            .unwrap();

        assert_eq!(retry.pending_attempt_ids, vec![failed.attempt_id.clone()]);
        assert!(retry.retried_blob_hashes.is_empty());
        assert_eq!(retry.reconciled_blob_hashes, vec![attachment.blob_hash]);
        assert!(retry.missing_blob_hashes.is_empty());
        assert!(retry.skipped_blob_hashes.is_empty());
        assert!(retry.peer_errors.is_empty());
        assert_eq!(retry.blob_transfer_attempts.len(), 1);
        let reconciled = &retry.blob_transfer_attempts[0];
        assert_eq!(reconciled.attempt_id, failed.attempt_id);
        assert_eq!(reconciled.peer_endpoint, stale_peer.endpoint);
        assert_eq!(reconciled.status, BlobTransferStatus::Succeeded);
        assert!(reconciled.finished_at_unix_ms.is_some());
        assert!(reconciled.error.is_none());
        assert_eq!(
            alice.blob_transfer_ledger().unwrap().entries,
            vec![reconciled.clone()]
        );

        let second_retry = alice
            .retry_pending_blob_transfers_direct(&transport, workspace_id, &[fallback_peer])
            .await
            .unwrap();
        assert!(second_retry.pending_attempt_ids.is_empty());
        assert!(second_retry.reconciled_blob_hashes.is_empty());
        assert!(second_retry.blob_transfer_attempts.is_empty());

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_reconciles_duplicate_blob_attempts_once() {
        let alice_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"duplicate attempts already recovered".to_vec();
        let attachment_path = alice_dir.path().join("duplicate-recovered.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Duplicate Reconcile Blob Retry", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry duplicate recovered attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let first_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-a".to_owned()),
            endpoint: "127.0.0.1:1".to_owned(),
        };
        let second_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-b".to_owned()),
            endpoint: "127.0.0.1:2".to_owned(),
        };
        let first_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &first_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let first_failed = alice
            .record_blob_transfer_finished(
                &first_started,
                BlobTransferStatus::Failed,
                Some("offline peer a".to_owned()),
            )
            .unwrap();
        let second_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &second_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let second_failed = alice
            .record_blob_transfer_finished(
                &second_started,
                BlobTransferStatus::Failed,
                Some("offline peer b".to_owned()),
            )
            .unwrap();
        let fallback_peer = PeerAddress {
            peer_id: PeerId("fallback-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let transport = CountingCompleteAvailabilityTransport::new(attachment.blob_hash.clone());

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id.clone(),
                std::slice::from_ref(&fallback_peer),
            )
            .await
            .unwrap();

        assert_eq!(transport.fetch_count(), 1);
        assert_eq!(
            retry.pending_attempt_ids,
            vec![
                first_failed.attempt_id.clone(),
                second_failed.attempt_id.clone()
            ]
        );
        assert!(retry.retried_blob_hashes.is_empty());
        assert_eq!(
            retry.reconciled_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(retry.peer_errors.is_empty());
        assert_eq!(retry.blob_transfer_attempts.len(), 2);
        assert_eq!(
            retry
                .blob_transfer_attempts
                .iter()
                .map(|attempt| attempt.attempt_id.clone())
                .collect::<Vec<_>>(),
            vec![first_failed.attempt_id.clone(), second_failed.attempt_id]
        );
        assert!(
            retry
                .blob_transfer_attempts
                .iter()
                .all(|attempt| attempt.status == BlobTransferStatus::Succeeded
                    && attempt.error.is_none()
                    && attempt.finished_at_unix_ms.is_some())
        );
        assert!(
            alice
                .blob_transfer_ledger()
                .unwrap()
                .entries
                .iter()
                .all(|attempt| attempt.status == BlobTransferStatus::Succeeded)
        );

        let second_retry = alice
            .retry_pending_blob_transfers_direct(&transport, workspace_id, &[fallback_peer])
            .await
            .unwrap();
        assert!(second_retry.pending_attempt_ids.is_empty());
        assert_eq!(transport.fetch_count(), 1);
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_reconciles_duplicate_blob_attempts_after_upload() {
        let alice_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"duplicate attempts need one upload".to_vec();
        let attachment_path = alice_dir.path().join("duplicate-upload.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Duplicate Upload Blob Retry", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry duplicate upload attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let first_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-a".to_owned()),
            endpoint: "127.0.0.1:1".to_owned(),
        };
        let second_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-b".to_owned()),
            endpoint: "127.0.0.1:2".to_owned(),
        };
        let first_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &first_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let first_failed = alice
            .record_blob_transfer_finished(
                &first_started,
                BlobTransferStatus::Failed,
                Some("offline peer a".to_owned()),
            )
            .unwrap();
        let second_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &second_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let second_failed = alice
            .record_blob_transfer_finished(
                &second_started,
                BlobTransferStatus::Failed,
                Some("offline peer b".to_owned()),
            )
            .unwrap();
        let fallback_peer = PeerAddress {
            peer_id: PeerId("fallback-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let transport = CountingWholeBlobUploadTransport::new(attachment.blob_hash.clone());

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id.clone(),
                std::slice::from_ref(&fallback_peer),
            )
            .await
            .unwrap();

        assert_eq!(transport.fetch_count(), 1);
        assert_eq!(transport.upload_count(), 1);
        assert_eq!(
            retry.pending_attempt_ids,
            vec![
                first_failed.attempt_id.clone(),
                second_failed.attempt_id.clone()
            ]
        );
        assert_eq!(
            retry.retried_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert_eq!(
            retry.reconciled_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(retry.peer_errors.is_empty());
        assert_eq!(retry.blob_transfer_attempts.len(), 3);
        let upload_attempt = retry
            .blob_transfer_attempts
            .iter()
            .find(|attempt| attempt.peer_endpoint == fallback_peer.endpoint)
            .unwrap();
        assert_eq!(upload_attempt.status, BlobTransferStatus::Succeeded);
        assert_eq!(upload_attempt.blob_hash, attachment.blob_hash);
        let reconciled_attempt_ids = retry
            .blob_transfer_attempts
            .iter()
            .filter(|attempt| attempt.peer_endpoint != fallback_peer.endpoint)
            .map(|attempt| attempt.attempt_id.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            reconciled_attempt_ids,
            BTreeSet::from([first_failed.attempt_id, second_failed.attempt_id])
        );
        assert!(
            alice
                .blob_transfer_ledger()
                .unwrap()
                .entries
                .iter()
                .all(|attempt| attempt.status == BlobTransferStatus::Succeeded)
        );

        let second_retry = alice
            .retry_pending_blob_transfers_direct(&transport, workspace_id, &[fallback_peer])
            .await
            .unwrap();
        assert!(second_retry.pending_attempt_ids.is_empty());
        assert_eq!(transport.fetch_count(), 1);
        assert_eq!(transport.upload_count(), 1);
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_deduplicates_failed_duplicate_blob_attempts() {
        let alice_dir = tempfile::tempdir().unwrap();
        let attachment_bytes = b"duplicate attempts still failing".to_vec();
        let attachment_path = alice_dir.path().join("duplicate-failed-upload.bin");
        fs::write(&attachment_path, &attachment_bytes).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Duplicate Failed Blob Retry", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry duplicate failed attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let first_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-a".to_owned()),
            endpoint: "127.0.0.1:1".to_owned(),
        };
        let second_stale_peer = PeerAddress {
            peer_id: PeerId("offline-node-b".to_owned()),
            endpoint: "127.0.0.1:2".to_owned(),
        };
        let first_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &first_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let first_failed = alice
            .record_blob_transfer_finished(
                &first_started,
                BlobTransferStatus::Failed,
                Some("offline peer a".to_owned()),
            )
            .unwrap();
        let second_started = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &second_stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::WholeBlob,
                ciphertext.len() as u64,
                None,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let second_failed = alice
            .record_blob_transfer_finished(
                &second_started,
                BlobTransferStatus::Failed,
                Some("offline peer b".to_owned()),
            )
            .unwrap();
        let fallback_peer = PeerAddress {
            peer_id: PeerId("fallback-node".to_owned()),
            endpoint: "127.0.0.1:7777".to_owned(),
        };
        let transport = CountingWholeBlobUploadTransport::failing(attachment.blob_hash.clone());

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id,
                std::slice::from_ref(&fallback_peer),
            )
            .await
            .unwrap();

        assert_eq!(transport.fetch_count(), 1);
        assert_eq!(transport.upload_count(), 1);
        assert_eq!(
            retry.pending_attempt_ids,
            vec![first_failed.attempt_id, second_failed.attempt_id]
        );
        assert!(retry.retried_blob_hashes.is_empty());
        assert!(retry.reconciled_blob_hashes.is_empty());
        assert_eq!(retry.peer_errors.len(), 1);
        assert!(retry.peer_errors[0].suspect_protocol_error);
        assert_eq!(retry.blob_transfer_attempts.len(), 1);
        assert_eq!(
            retry.blob_transfer_attempts[0].peer_endpoint,
            fallback_peer.endpoint
        );
        assert_eq!(
            retry.blob_transfer_attempts[0].status,
            BlobTransferStatus::Failed
        );

        let ledger = alice.blob_transfer_ledger().unwrap();
        assert_eq!(
            ledger
                .entries
                .iter()
                .filter(|attempt| attempt.blob_hash == attachment.blob_hash
                    && attempt.status == BlobTransferStatus::Failed)
                .count(),
            3
        );
    }

    #[tokio::test]
    async fn retry_pending_blob_transfers_uploads_missing_chunks_to_fallback_peer() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let large_attachment = vec![11; DIRECT_WHOLE_BLOB_SYNC_LIMIT + 1024];
        let attachment_path = alice_dir.path().join("retry-large.bin");
        fs::write(&attachment_path, &large_attachment).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Retry Blob Transfers", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let sent = alice
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "retry pending large attachment",
                &attachment_path,
                "application/octet-stream",
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let sent_event = alice_events
            .iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted { attachments, .. } = &sent_event.event.body else {
            panic!("expected encrypted message event");
        };
        let attachment = attachments[0].clone();
        let alice_blob_store = BlobStore::open(alice.paths().blob_store.clone()).unwrap();
        let ciphertext = alice_blob_store
            .get_bytes(&attachment.blob_hash)
            .unwrap()
            .unwrap();
        let descriptor = describe_blob(&ciphertext, DIRECT_BLOB_CHUNK_SIZE);

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_path = node_dir.path().join("blobs");
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        node_blob_store.put_manifest(&descriptor).unwrap();
        node_blob_store
            .put_chunk_with_hash(
                &descriptor.chunk_hashes[0],
                &ciphertext[..descriptor.chunk_size],
            )
            .unwrap();
        let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", node_store, node_blob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let stale_peer = PeerAddress {
            peer_id: PeerId("offline-node".to_owned()),
            endpoint: "127.0.0.1:1".to_owned(),
        };
        let stale_attempt = alice
            .record_blob_transfer_started(
                &workspace_id.0,
                &stale_peer,
                &attachment.blob_hash,
                BlobTransferMode::ChunkedBlob,
                ciphertext.len() as u64,
                Some(DIRECT_BLOB_CHUNK_SIZE as u64),
                descriptor.chunk_hashes.clone(),
                descriptor.chunk_hashes[1..].to_vec(),
                vec![descriptor.chunk_hashes[0].clone()],
            )
            .unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let retry = alice
            .retry_pending_blob_transfers_direct(
                &transport,
                workspace_id.clone(),
                std::slice::from_ref(&peer),
            )
            .await
            .unwrap();

        assert_eq!(
            retry.pending_attempt_ids,
            vec![stale_attempt.attempt_id.clone()]
        );
        assert_eq!(
            retry.retried_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert_eq!(
            retry.reconciled_blob_hashes,
            vec![attachment.blob_hash.clone()]
        );
        assert!(retry.missing_blob_hashes.is_empty());
        assert!(retry.skipped_blob_hashes.is_empty());
        assert!(retry.peer_errors.is_empty());
        assert_eq!(retry.blob_transfer_attempts.len(), 2);
        let retried = retry
            .blob_transfer_attempts
            .iter()
            .find(|attempt| attempt.peer_endpoint == peer.endpoint)
            .unwrap();
        assert_eq!(retried.status, BlobTransferStatus::Succeeded);
        assert_eq!(retried.peer_endpoint, peer.endpoint);
        assert_eq!(retried.attempt_count, 1);
        assert_eq!(retried.blob_hash, attachment.blob_hash);
        assert_eq!(retried.chunk_count, descriptor.chunk_hashes.len());
        assert_eq!(retried.remote_available_chunk_count, 1);
        assert_eq!(
            retried.remote_available_chunk_hashes,
            vec![descriptor.chunk_hashes[0].clone()]
        );
        assert_eq!(
            retried.planned_chunk_count,
            descriptor.chunk_hashes.len() - 1
        );
        assert_eq!(
            retried.planned_chunk_hashes,
            descriptor.chunk_hashes[1..].to_vec()
        );
        let reconciled_stale = retry
            .blob_transfer_attempts
            .iter()
            .find(|attempt| attempt.attempt_id == stale_attempt.attempt_id)
            .unwrap();
        assert_eq!(reconciled_stale.status, BlobTransferStatus::Succeeded);
        assert!(reconciled_stale.error.is_none());
        assert!(reconciled_stale.finished_at_unix_ms.is_some());
        let replica_blobs = BlobStore::open(&node_blob_path).unwrap();
        assert!(
            replica_blobs
                .availability(&attachment.blob_hash)
                .unwrap()
                .unwrap()
                .is_complete()
        );
        let ledger = alice.blob_transfer_ledger().unwrap();
        assert_eq!(ledger.entries.len(), 2);
        assert_eq!(ledger.entries[0].attempt_id, stale_attempt.attempt_id);
        assert_eq!(ledger.entries[0].status, BlobTransferStatus::Succeeded);
        assert_eq!(ledger.entries[1].attempt_id, retried.attempt_id);
        assert_eq!(ledger.entries[1].status, BlobTransferStatus::Succeeded);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_direct_publish_skips_events_the_peer_already_has() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice.create_workspace("Delta Publish", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id.clone()),
                "publish once",
            )
            .unwrap();

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let first = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let second = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();

        assert_eq!(first.published_event_ids.len(), 3);
        assert!(second.published_event_ids.is_empty());
        assert_eq!(inventory.len(), 3);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_direct_publish_skips_invalid_self_contained_signature_events() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Publish Integrity", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let mut forged_event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            alice.identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged publish payload".to_owned(),
                attachments: Vec::new(),
            },
        );
        forged_event.parents = vec![EventId(created.channel_event_id.clone())];
        let mut forged = alice.identity.sign_event(forged_event);
        forged.signature[0] ^= 1;
        let forged_event_id = forged.event_id.clone();
        alice.store.append_event(&forged).unwrap();
        insert_corrupt_event_json(
            alice_dir.path(),
            &workspace_id,
            "evt_corrupt_direct_publish_tripwire",
        );
        assert!(
            alice
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();
        let snapshot = alice
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();

        assert_eq!(
            published.published_event_ids,
            vec![created.workspace_event_id, created.channel_event_id]
        );
        assert!(!published.published_event_ids.contains(&forged_event_id.0));
        assert_eq!(
            inventory,
            published
                .published_event_ids
                .into_iter()
                .map(EventId)
                .collect::<Vec<_>>()
        );
        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.invalid_signatures[0].event_id, forged_event_id.0);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn runtime_direct_publish_skips_unmaterialized_gap_events_and_their_blobs() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice.create_workspace("Gap Publish", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        alice
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                "publish materialized only",
            )
            .unwrap();

        let gap_blob_hash = "f".repeat(64);
        let missing_parent_id = EventId("evt_missing_parent".to_owned());
        let mut gap_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            alice.device_id().clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: SealedPayload {
                    mode: PayloadEncryption::DevelopmentPlaintext,
                    key_id: "gap-key".to_owned(),
                    nonce: Vec::new(),
                    aad: Vec::new(),
                    bytes: b"stored but incomplete".to_vec(),
                },
                attachments: vec![AttachmentRef {
                    blob_hash: gap_blob_hash.clone(),
                    media_type: "text/plain".to_owned(),
                    byte_len: 5,
                    display_name: "gap.txt".to_owned(),
                    attachment_id: String::new(),
                    encryption: None,
                }],
            },
        );
        gap_message.parents = vec![missing_parent_id.clone()];
        let gap_message = alice.identity.sign_event(gap_message);
        alice.store.append_event(&gap_message).unwrap();
        insert_corrupt_event_json(
            alice_dir.path(),
            &workspace_id,
            "evt_corrupt_publish_queue_tripwire",
        );
        assert!(
            alice
                .store
                .list_events_for_workspace(&workspace_id.0)
                .is_err()
        );

        let queue = alice.workspace_publish_queue(workspace_id.clone()).unwrap();

        assert_eq!(queue.workspace_id, workspace_id.0);
        assert_eq!(queue.publishable_event_ids.len(), 3);
        assert_eq!(queue.backup_event_ids.len(), 1);
        assert!(queue.available_blob_hashes.is_empty());
        assert!(queue.missing_blob_hashes.is_empty());
        assert!(!queue.missing_blob_hashes.contains(&gap_blob_hash));
        assert_eq!(queue.summary.publishable_event_count, 3);
        assert_eq!(queue.summary.backup_event_count, 1);
        assert_eq!(queue.summary.available_blob_count, 0);
        assert_eq!(queue.summary.missing_blob_count, 0);
        assert_eq!(queue.summary.skipped_gap_count, 1);
        assert_eq!(queue.summary.queued_message_event_count, 1);
        assert_eq!(queue.summary.queued_attachment_blob_count, 0);
        assert!(!queue.summary.has_missing_local_blobs);
        assert!(queue.summary.has_skipped_gaps);
        assert!(!queue.summary.is_complete);
        assert!(queue.summary.oldest_event_physical_ms.is_some());
        assert!(queue.summary.newest_event_physical_ms.is_some());
        let message_channel_summary = queue
            .summary
            .channels
            .iter()
            .find(|summary| summary.channel_id.as_deref() == Some(channel_id.0.as_str()))
            .unwrap();
        assert_eq!(message_channel_summary.publishable_event_count, 1);
        assert_eq!(message_channel_summary.backup_event_count, 1);
        assert_eq!(message_channel_summary.queued_message_event_count, 1);
        assert_eq!(message_channel_summary.queued_attachment_blob_count, 0);
        assert_eq!(message_channel_summary.missing_blob_count, 0);
        assert_eq!(
            queue.skipped_gaps,
            vec![PulledWorkspaceGap {
                event_id: gap_message.event_id.0.clone(),
                missing_parent_ids: vec![missing_parent_id.0.clone()],
            }]
        );

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let inventory = transport
            .fetch_workspace_inventory(&peer, &workspace_id)
            .await
            .unwrap();

        assert_eq!(published.published_event_ids.len(), 3);
        assert!(
            !published
                .published_event_ids
                .contains(&gap_message.event_id.0)
        );
        assert!(published.missing_blob_hashes.is_empty());
        assert!(!published.missing_blob_hashes.contains(&gap_blob_hash));
        assert_eq!(
            published.skipped_gaps,
            vec![PulledWorkspaceGap {
                event_id: gap_message.event_id.0.clone(),
                missing_parent_ids: vec![missing_parent_id.0],
            }]
        );
        assert_eq!(inventory.len(), 3);
        assert!(!inventory.contains(&gap_message.event_id));

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn runtime_publish_queue_caps_samples_and_preserves_summary_counts() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime.create_workspace("Queue Cap", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        let queued_message_count = MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS + 5;
        let missing_blob_count = MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS + 3;
        let skipped_gap_count = MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS + 2;

        for index in 0..queued_message_count {
            let attachment = (index < missing_blob_count).then(|| AttachmentRef {
                blob_hash: format!("{index:064x}"),
                media_type: "application/octet-stream".to_owned(),
                byte_len: 8,
                display_name: format!("missing-{index}.bin"),
                attachment_id: String::new(),
                encryption: Some(EncryptedBlobRef {
                    mode: PayloadEncryption::Aes256GcmSiv,
                    key_id: "test-key".to_owned(),
                    nonce: vec![0; 12],
                    aad: Vec::new(),
                    plaintext_byte_len: 8,
                }),
            });
            let event = runtime.identity.sign_event(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                runtime.device_id().clone(),
                EventBody::MessageCreatedEncrypted {
                    message_id: MessageId::new(),
                    sealed_markdown: SealedPayload {
                        mode: PayloadEncryption::Aes256GcmSiv,
                        key_id: "test-key".to_owned(),
                        nonce: vec![0; 12],
                        aad: Vec::new(),
                        bytes: b"ciphertext".to_vec(),
                    },
                    attachments: attachment.into_iter().collect(),
                },
            ));
            runtime.store.append_event(&event).unwrap();
        }

        for index in 0..skipped_gap_count {
            let mut gap = SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                runtime.device_id().clone(),
                EventBody::MessageCreatedEncrypted {
                    message_id: MessageId::new(),
                    sealed_markdown: SealedPayload {
                        mode: PayloadEncryption::Aes256GcmSiv,
                        key_id: "test-key".to_owned(),
                        nonce: vec![0; 12],
                        aad: Vec::new(),
                        bytes: b"gap-ciphertext".to_vec(),
                    },
                    attachments: Vec::new(),
                },
            );
            gap.parents = vec![EventId(format!("evt_missing_parent_{index:03}"))];
            let gap = runtime.identity.sign_event(gap);
            runtime.store.append_event(&gap).unwrap();
        }

        let queue = runtime.workspace_publish_queue(workspace_id).unwrap();

        assert_eq!(
            queue.publishable_event_ids.len(),
            MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(
            queue.backup_event_ids.len(),
            MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(
            queue.missing_blob_hashes.len(),
            MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(
            queue.skipped_gaps.len(),
            MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS
        );
        assert!(queue.available_blob_hashes.is_empty());
        assert!(queue.summary.publishable_event_count > queue.publishable_event_ids.len());
        assert!(queue.summary.backup_event_count > queue.backup_event_ids.len());
        assert_eq!(queue.summary.missing_blob_count, missing_blob_count);
        assert_eq!(queue.summary.skipped_gap_count, skipped_gap_count);
        assert!(queue.summary.has_missing_local_blobs);
        assert!(queue.summary.has_skipped_gaps);
        assert!(!queue.summary.is_complete);
    }

    #[tokio::test]
    async fn invited_member_can_publish_encrypted_reply_to_replica() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Invited", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();

        let node_store_path = node_dir.path().join("events.db");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let published = alice
            .publish_workspace_to_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(published.workspace_id, workspace_id.0);
        assert_eq!(published.published_event_ids.len(), 3);

        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        bob.send_message(workspace_id.clone(), channel_id, "invited member reply")
            .unwrap();

        let bob_published = bob
            .publish_workspace_to_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(bob_published.workspace_id, workspace_id.0);
        assert_eq!(bob_published.published_event_ids.len(), 4);

        let inventory = transport.fetch_inventory(&peer).await.unwrap();
        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        assert_eq!(inventory.len(), 4);
        assert_eq!(snapshot.timeline[0].body, "invited member reply");
        assert!(snapshot.timeline[0].encrypted);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();

        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_events_json = serde_json::to_string(
            &node_store
                .list_events_for_workspace(&workspace_id.0)
                .unwrap(),
        )
        .unwrap();
        assert!(!node_events_json.contains("invited member reply"));
    }

    #[tokio::test]
    async fn invited_member_private_channel_reply_is_rejected_by_local_runtime() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Private Channel", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());
        alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "owner private secret",
            )
            .unwrap();
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        bob.send_message(workspace_id.clone(), public_channel_id, "public reply")
            .unwrap();
        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id.clone(), "owner private secret")
            .unwrap();
        let error = bob
            .send_message(workspace_id.clone(), private_channel_id, "private reply")
            .unwrap_err();
        let events = bob.workspace_events(&workspace_id).unwrap();

        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.channels[0].name, "general");
        assert!(
            !snapshot
                .timeline
                .iter()
                .any(|item| item.body == "owner private secret")
        );
        assert!(search.hits.is_empty());
        assert!(
            error
                .to_string()
                .contains("not authorized for private channel")
        );
        assert_eq!(events.len(), 6);
        assert!(
            !serde_json::to_string(&events)
                .unwrap()
                .contains("private reply")
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn removed_workspace_member_cannot_append_future_messages() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Removed", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.import_workspace_key(exported_key).unwrap();
        bob.send_message(
            workspace_id.clone(),
            channel_id.clone(),
            "before workspace removal",
        )
        .unwrap();

        let removed = alice
            .remove_member(workspace_id.clone(), bob.device_id().clone())
            .unwrap();
        let removal_event = alice
            .workspace_events(&workspace_id)
            .unwrap()
            .into_iter()
            .find(|event| event.event_id.0 == removed.event_id)
            .unwrap();
        bob.store.append_event(&removal_event).unwrap();

        let error = bob
            .send_message(workspace_id.clone(), channel_id, "after workspace removal")
            .unwrap_err();
        let snapshot = bob.workspace_snapshot(workspace_id).unwrap();
        let events_json = serde_json::to_string(&bob.store.list_events().unwrap()).unwrap();

        assert_eq!(removed.removed_device_id, bob.device_id().0);
        assert!(error.to_string().contains("not a workspace member"));
        assert!(
            !snapshot
                .members
                .iter()
                .any(|member| member.device_id == bob.device_id().0)
        );
        assert!(!events_json.contains("after workspace removal"));
    }

    #[test]
    fn remove_member_with_key_rotation_rekeys_workspace_and_private_channels() {
        let alice_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob_id = DeviceId("dev_removed_member".to_owned());
        let created = alice
            .create_workspace("Rotated Removal", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(workspace_id.clone(), bob_id.clone(), WorkspaceRole::Member)
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob_id.clone(),
            )
            .unwrap();
        let old_workspace_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let old_channel_key = alice
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();

        let removed = alice
            .remove_member_with_key_rotation(workspace_id.clone(), bob_id.clone())
            .unwrap();
        let public_message = alice
            .send_message(
                workspace_id.clone(),
                public_channel_id,
                "after rotated workspace removal",
            )
            .unwrap();
        let private_message = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "after rotated private removal",
            )
            .unwrap();
        let events = alice.workspace_events(&workspace_id).unwrap();
        let removal_index = events
            .iter()
            .position(|event| event.event_id.0 == removed.removal_event_id)
            .unwrap();
        let workspace_rotation_index = events
            .iter()
            .position(|event| event.event_id.0 == removed.workspace_key_rotation.event_id)
            .unwrap();
        let channel_rotation_index = events
            .iter()
            .position(|event| event.event_id.0 == removed.channel_key_rotations[0].event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: public_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == public_message.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted public message");
        };
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown: private_sealed,
            ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == private_message.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted private message");
        };

        assert_eq!(removed.removed_device_id, bob_id.0);
        assert_eq!(
            removed.workspace_key_rotation.previous_key_id,
            old_workspace_key.key_id
        );
        assert_eq!(removed.channel_key_rotations.len(), 1);
        assert_eq!(
            removed.channel_key_rotations[0].previous_key_id,
            old_channel_key.key_id
        );
        assert!(removal_index < workspace_rotation_index);
        assert!(workspace_rotation_index < channel_rotation_index);
        assert_eq!(public_sealed.key_id, removed.workspace_key_rotation.key_id);
        assert_eq!(
            private_sealed.key_id,
            removed.channel_key_rotations[0].key_id
        );
    }

    #[test]
    fn composite_member_removal_revokes_openmls_before_workspace_membership() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Composite Removal", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(workspace_id.clone(), Some(EventId(added.event_id)))
            .unwrap();

        let removed = alice
            .remove_member_with_openmls(workspace_id.clone(), bob.device_id().clone())
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let openmls_index = alice_events
            .iter()
            .position(|event| event.event_id.0 == removed.openmls_event_id)
            .unwrap();
        let removal_index = alice_events
            .iter()
            .position(|event| event.event_id.0 == removed.removal_event_id)
            .unwrap();
        for event in &alice_events {
            bob.store.append_event(event).unwrap();
        }

        let send_error = bob
            .send_message(
                workspace_id.clone(),
                channel_id,
                "after composite workspace removal",
            )
            .unwrap_err();
        let applied = bob
            .apply_openmls_workspace_group_commits(
                workspace_id.clone(),
                Some(EventId(removed.openmls_event_id.clone())),
            )
            .unwrap();
        let snapshot = bob.workspace_snapshot(workspace_id).unwrap();

        assert_eq!(removed.removed_device_id, bob.device_id().0);
        assert!(openmls_index < removal_index);
        assert!(matches!(
            alice_events[openmls_index].event.body,
            EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        ));
        assert!(matches!(
            alice_events[removal_index].event.body,
            EventBody::MemberRemoved { .. }
        ));
        assert!(send_error.to_string().contains("not a workspace member"));
        assert!(applied.self_removed);
        assert_eq!(applied.applied_event_ids, vec![removed.openmls_event_id]);
        assert!(
            !snapshot
                .members
                .iter()
                .any(|member| member.device_id == bob.device_id().0)
        );
    }

    #[tokio::test]
    async fn channel_member_grant_allows_private_channel_reply_after_pull() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Private Grant", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let added = alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "owner private note",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let exported_channel_key = alice
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        let keyless_snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let keyless_search = bob
            .search_workspace_messages(workspace_id.clone(), "owner private note")
            .unwrap();
        let keyless_send_error = bob
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "missing key reply",
            )
            .unwrap_err();

        assert_eq!(keyless_snapshot.timeline[0].body, "Encrypted message");
        assert!(keyless_search.hits.is_empty());
        assert!(matches!(
            keyless_send_error,
            RuntimeError::ChannelKeyMissing { .. }
        ));

        bob.import_channel_key(exported_channel_key).unwrap();
        bob.send_message(
            workspace_id.clone(),
            private_channel_id.clone(),
            "authorized private reply",
        )
        .unwrap();
        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id.clone(), "owner private note")
            .unwrap();

        assert_eq!(added.member_device_id, bob.device_id().0);
        assert!(
            snapshot
                .timeline
                .iter()
                .any(|item| item.body == "owner private note")
        );
        assert!(
            snapshot
                .timeline
                .iter()
                .any(|item| item.body == "authorized private reply")
        );
        assert_eq!(search.hits.len(), 1);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn removed_channel_member_keeps_workspace_access_but_loses_private_channel() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Private Removal", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id);

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        let workspace_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        let channel_key = alice
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.import_workspace_key(workspace_key).unwrap();
        bob.import_channel_key(channel_key).unwrap();
        bob.send_message(
            workspace_id.clone(),
            private_channel_id.clone(),
            "before channel removal",
        )
        .unwrap();

        let removed = alice
            .remove_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        let removal_event = alice
            .workspace_events(&workspace_id)
            .unwrap()
            .into_iter()
            .find(|event| event.event_id.0 == removed.event_id)
            .unwrap();
        bob.store.append_event(&removal_event).unwrap();

        let private_error = bob
            .send_message(
                workspace_id.clone(),
                private_channel_id,
                "after channel removal",
            )
            .unwrap_err();
        bob.send_message(
            workspace_id.clone(),
            public_channel_id,
            "public still allowed",
        )
        .unwrap();
        let snapshot = bob.decrypted_workspace_snapshot(workspace_id).unwrap();
        let events_json = serde_json::to_string(&bob.store.list_events().unwrap()).unwrap();

        assert_eq!(removed.member_device_id, bob.device_id().0);
        assert!(
            private_error
                .to_string()
                .contains("not authorized for private channel")
        );
        assert!(
            snapshot
                .timeline
                .iter()
                .any(|item| item.body == "public still allowed")
        );
        assert!(!events_json.contains("after channel removal"));
    }

    #[test]
    fn remove_channel_member_with_key_rotation_rekeys_private_channel() {
        let alice_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob_id = DeviceId("dev_removed_channel_member".to_owned());
        let created = alice
            .create_workspace("Rotated Channel Removal", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(workspace_id.clone(), bob_id.clone(), WorkspaceRole::Member)
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob_id.clone(),
            )
            .unwrap();
        let old_channel_key = alice
            .export_channel_key(workspace_id.clone(), private_channel_id.clone())
            .unwrap();

        let removed = alice
            .remove_channel_member_with_key_rotation(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob_id.clone(),
            )
            .unwrap();
        let private_message = alice
            .send_message(
                workspace_id.clone(),
                private_channel_id,
                "after rotated channel removal",
            )
            .unwrap();
        let events = alice.workspace_events(&workspace_id).unwrap();
        let removal_index = events
            .iter()
            .position(|event| event.event_id.0 == removed.removal_event_id)
            .unwrap();
        let rotation_index = events
            .iter()
            .position(|event| event.event_id.0 == removed.channel_key_rotation.event_id)
            .unwrap();
        let EventBody::MessageCreatedEncrypted {
            sealed_markdown, ..
        } = &events
            .iter()
            .find(|event| event.event_id.0 == private_message.event_id)
            .unwrap()
            .event
            .body
        else {
            panic!("expected encrypted private message");
        };

        assert_eq!(removed.member_device_id, bob_id.0);
        assert_eq!(
            removed.channel_key_rotation.previous_key_id,
            old_channel_key.key_id
        );
        assert!(removal_index < rotation_index);
        assert_eq!(sealed_markdown.key_id, removed.channel_key_rotation.key_id);
    }

    #[test]
    fn composite_channel_member_removal_revokes_openmls_before_channel_access() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Composite Channel Removal", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let public_channel_id = ChannelId(created.channel_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        let workspace_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.import_workspace_key(workspace_key).unwrap();
        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_channel_group(
            workspace_id.clone(),
            private_channel_id.clone(),
            Some(EventId(added.event_id)),
        )
        .unwrap();

        let removed = alice
            .remove_channel_member_with_openmls(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        let alice_events = alice.workspace_events(&workspace_id).unwrap();
        let openmls_index = alice_events
            .iter()
            .position(|event| event.event_id.0 == removed.openmls_event_id)
            .unwrap();
        let removal_index = alice_events
            .iter()
            .position(|event| event.event_id.0 == removed.removal_event_id)
            .unwrap();
        for event in &alice_events {
            bob.store.append_event(event).unwrap();
        }

        let private_error = bob
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "after composite channel removal",
            )
            .unwrap_err();
        bob.send_message(
            workspace_id.clone(),
            public_channel_id,
            "public after composite channel removal",
        )
        .unwrap();
        let applied = bob
            .apply_openmls_channel_group_commits(
                workspace_id.clone(),
                private_channel_id,
                Some(EventId(removed.openmls_event_id.clone())),
            )
            .unwrap();

        assert_eq!(removed.member_device_id, bob.device_id().0);
        assert!(openmls_index < removal_index);
        assert!(matches!(
            alice_events[openmls_index].event.body,
            EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        ));
        assert!(matches!(
            alice_events[removal_index].event.body,
            EventBody::ChannelMemberRemoved { .. }
        ));
        assert!(
            private_error
                .to_string()
                .contains("not authorized for private channel")
        );
        assert!(applied.self_removed);
        assert_eq!(applied.applied_event_ids, vec![removed.openmls_event_id]);
    }

    #[test]
    fn add_channel_member_auto_provisions_openmls_channel_member_when_key_package_exists() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Auto Channel MLS Provision", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let added = alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        let openmls_event_id = added.openmls_member_add_event_id.clone().unwrap();

        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        let joined = bob
            .join_openmls_channel_group(
                workspace_id,
                private_channel_id,
                Some(EventId(openmls_event_id.clone())),
            )
            .unwrap();

        assert_eq!(added.member_device_id, bob.device_id().0);
        assert_eq!(added.openmls_epoch, Some(1));
        assert_eq!(added.openmls_member_count, Some(2));
        assert_eq!(joined.source_event_id, openmls_event_id);
    }

    #[tokio::test]
    async fn sync_workspace_direct_auto_provisions_openmls_workspace_member_after_key_package_pull()
    {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Auto Workspace MLS Provision", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();

        let bob_store = EventStore::open(bob_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", bob_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("bob".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let synced = alice
            .sync_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let provisioned_event_id = synced
            .pulled
            .openmls_catchup
            .workspace_provisioned_event_ids
            .first()
            .cloned()
            .unwrap();
        let bob_has_welcome = bob
            .workspace_events(&workspace_id)
            .unwrap()
            .iter()
            .any(|event| event.event_id.0 == provisioned_event_id);
        let joined = bob
            .join_openmls_workspace_group(workspace_id, Some(EventId(provisioned_event_id.clone())))
            .unwrap();

        assert_eq!(
            synced
                .pulled
                .openmls_catchup
                .workspace_provisioned_event_ids,
            vec![provisioned_event_id.clone()]
        );
        assert!(
            synced
                .published
                .published_event_ids
                .contains(&provisioned_event_id)
        );
        assert!(bob_has_welcome);
        assert_eq!(joined.source_event_id, provisioned_event_id);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn sync_workspace_direct_auto_responds_to_local_compromise_signal() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Compromise Auto Response", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "before local compromise signal",
            )
            .unwrap();
        let mut forged = alice
            .workspace_events(&workspace_id)
            .unwrap()
            .into_iter()
            .find(|event| event.event_id.0 == sent.event_id)
            .unwrap();
        forged.signature[0] ^= 1;
        let forged = SignedEvent::from_author_signature(
            forged.event,
            forged.author_public_key,
            forged.signature,
        );
        alice.store.append_event(&forged).unwrap();

        let node_store_path = node_dir.path().join("events.db");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let synced = alice
            .sync_workspace_direct(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        let compromise_response = synced
            .pulled
            .compromise_response
            .as_ref()
            .expect("local signal should trigger automatic response");
        let rotated_event_ids = compromise_response
            .rotation
            .as_ref()
            .unwrap()
            .rotated_event_ids
            .clone();

        assert!(compromise_response.rotated_local_secret_state);
        assert_eq!(
            compromise_response.responded_signal_event_ids,
            vec![forged.event_id.0.clone()]
        );
        assert!(
            rotated_event_ids
                .iter()
                .all(|event_id| synced.published.published_event_ids.contains(event_id))
        );
        assert!(
            !synced
                .published
                .published_event_ids
                .contains(&forged.event_id.0)
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();

        let node_events = EventStore::open(node_store_path)
            .unwrap()
            .list_events_for_workspace(&workspace_id.0)
            .unwrap();
        let node_event_ids = node_events
            .into_iter()
            .map(|event| event.event_id.0)
            .collect::<BTreeSet<_>>();
        assert!(
            rotated_event_ids
                .iter()
                .all(|event_id| node_event_ids.contains(event_id))
        );
        assert!(!node_event_ids.contains(&forged.event_id.0));
    }

    #[tokio::test]
    async fn sync_workspace_with_peer_publishes_local_and_pulls_remote_events() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Synced", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();

        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let alice_sync = alice
            .sync_workspace_with_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(alice_sync.workspace_id, workspace_id.0);
        assert_eq!(alice_sync.published.published_event_ids.len(), 3);
        assert_eq!(alice_sync.pulled.fetched_event_ids.len(), 0);

        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        bob.send_message(workspace_id.clone(), channel_id, "sync reply")
            .unwrap();

        let bob_sync = bob
            .sync_workspace_with_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(bob_sync.published.published_event_ids.len(), 4);
        assert_eq!(bob_sync.pulled.fetched_event_ids.len(), 0);

        let alice_after = alice
            .sync_workspace_with_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(alice_after.published.published_event_ids.len(), 3);
        assert_eq!(alice_after.pulled.fetched_event_ids.len(), 1);
        let snapshot = alice
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        assert_eq!(snapshot.timeline[0].body, "sync reply");

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn pull_workspace_from_peer_applies_openmls_workspace_commits_before_reindex() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Auto Pull", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.join_openmls_workspace_group(
            workspace_id.clone(),
            Some(EventId(added.event_id.clone())),
        )
        .unwrap();

        let updated = alice
            .update_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "pulled after automatic MLS catch-up",
            )
            .unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let pulled = bob
            .pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            pulled.openmls_catchup.workspace_applied_event_ids,
            vec![updated.event_id]
        );

        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id, "automatic MLS")
            .unwrap();
        assert_eq!(
            snapshot.timeline[0].body,
            "pulled after automatic MLS catch-up"
        );
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "pulled after automatic MLS catch-up");

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn pull_workspace_from_peer_auto_joins_openmls_workspace_group() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Auto Join", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_workspace_group_member(
                workspace_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                channel_id,
                "pulled after automatic MLS join",
            )
            .unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let pulled = bob
            .pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(
            pulled.openmls_catchup.workspace_joined_event_id,
            Some(added.event_id)
        );

        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id, "automatic MLS join")
            .unwrap();
        assert_eq!(snapshot.timeline[0].body, "pulled after automatic MLS join");
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "pulled after automatic MLS join");

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn pull_workspace_from_peer_auto_joins_openmls_channel_group() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("OpenMLS Channel Auto Join", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = alice
            .create_channel(workspace_id.clone(), "vault", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());

        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        alice
            .add_channel_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                bob.device_id().clone(),
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let bob_package = bob
            .publish_openmls_device_key_package(workspace_id.clone())
            .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }

        alice
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id.clone())
            .unwrap();
        let added = alice
            .add_openmls_channel_group_member(
                workspace_id.clone(),
                private_channel_id.clone(),
                DeviceKeyPackageId(bob_package.key_package_id),
            )
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private pull after automatic MLS join",
            )
            .unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("alice".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        let pulled = bob
            .pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        assert_eq!(pulled.openmls_catchup.channel_groups.len(), 1);
        assert_eq!(
            pulled.openmls_catchup.channel_groups[0].channel_id,
            private_channel_id.0
        );
        assert_eq!(
            pulled.openmls_catchup.channel_groups[0].joined_event_id,
            Some(added.event_id)
        );

        let snapshot = bob
            .decrypted_workspace_snapshot(workspace_id.clone())
            .unwrap();
        let search = bob
            .search_workspace_messages(workspace_id, "private pull")
            .unwrap();
        assert_eq!(
            snapshot.timeline[0].body,
            "private pull after automatic MLS join"
        );
        assert_eq!(search.hits.len(), 1);
        assert_eq!(search.hits[0].body, "private pull after automatic MLS join");

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn uninvited_member_reply_is_rejected_by_local_runtime() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("No Invite", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id.clone());
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();

        let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("node".to_owned()),
            endpoint: server.local_addr().unwrap().to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = DirectTransport;

        bob.pull_workspace_from_peer(&transport, &peer, workspace_id.clone())
            .await
            .unwrap();
        bob.import_workspace_key(exported_key).unwrap();
        let error = bob
            .send_message(workspace_id.clone(), channel_id, "uninvited reply")
            .unwrap_err();
        let events = bob.workspace_events(&workspace_id).unwrap();

        assert!(error.to_string().contains("not a workspace member"));
        assert_eq!(events.len(), 2);
        assert!(
            !serde_json::to_string(&events)
                .unwrap()
                .contains("uninvited reply")
        );

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }
}
