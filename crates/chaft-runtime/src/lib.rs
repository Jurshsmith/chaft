use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_app::{
    WorkspaceChannelPage, WorkspaceChannelSearch, WorkspaceMemberPage,
    query_has_channel_search_terms,
};
use chaft_core::{
    AuthorizationError, CoreError, MaterializationReport, MessageView, WorkspaceState,
    authorize_event_with_history,
};
use chaft_crypto::CryptoError;
use chaft_identity::{DeviceIdentity, IdentityError, verify_self_contained_event};
use chaft_media::MediaError;
use chaft_mls::MlsError;
use chaft_net::NetError;
use chaft_search::SearchError;
use chaft_store::{EventStore, StoreError};
use chaft_sync::SyncError;
use chaft_types::{
    AttachmentRef, ChannelId, DeviceId, DeviceKeyPackageId, EventBody, EventId, MessageId,
    SignableEvent, SignedEvent, WorkspaceId, WorkspaceRole,
};
pub use chaft_types::{
    PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES,
};
use thiserror::Error;

mod attachment_runtime;
mod blob_transfer;
mod blob_transfer_planning;
mod blob_transfer_runtime;
mod compromise;
mod compromise_runtime;
mod content_keys;
mod local_file_io;
mod local_secret;
mod local_secret_store;
mod openmls_actions;
mod openmls_provisioning;
mod paths;
mod publish_queue;
mod recovery_bundle;
mod runtime_validation;
mod search_results;
mod search_runtime;
mod snapshot_runtime;
mod storage_diagnostics;
mod sync_results;
mod sync_runtime;
mod trust_snapshot;
mod workspace_actions;
mod workspace_listing;

pub(crate) use attachment_runtime::PendingAttachment;
#[cfg(test)]
pub(crate) use attachment_runtime::{
    ATTACHMENT_FILE_MAX_BYTES, read_attachment_file_with_limit, write_attachment_export_file,
};
pub(crate) use blob_transfer::{
    BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES,
    BLOB_TRANSFER_LEDGER_MAX_BYTES, BLOB_TRANSFER_LEDGER_MAX_ENTRIES,
    BLOB_TRANSFER_LEDGER_SCHEMA_VERSION, blob_transfer_peer_error,
};
pub use blob_transfer::{
    BlobTransferAttempt, BlobTransferLedger, BlobTransferMode, BlobTransferPeerError,
    BlobTransferRetryReport, BlobTransferStatus,
};
pub(crate) use blob_transfer_planning::{ordered_retry_peers, planned_chunk_upload};
pub(crate) use compromise::{
    COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES,
    COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE,
    COMPROMISE_RESPONSE_LEDGER_MAX_BYTES, COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING,
    COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED,
    COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS,
    COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW,
    COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE, CompromiseResponseLedger,
    workspace_compromise_signal_from_event,
};
pub use compromise::{
    RotatedWorkspaceForSuspectedCompromise, WorkspaceCompromiseReport, WorkspaceCompromiseResponse,
    WorkspaceCompromiseSignal,
};
#[cfg(test)]
pub(crate) use content_keys::CONTENT_KEY_EXPORT_SCHEMA_VERSION;
pub(crate) use content_keys::{
    ChannelKey, ResolvedContentKey, WORKSPACE_KEY_LEN, WorkspaceKey, content_key_from_mls_export,
};
pub use content_keys::{
    ChannelKeyExport, ExportedContentKeyMaterial, ImportedChannelKey, ImportedWorkspaceKey,
    RotatedChannelKey, RotatedWorkspaceKey, RotatedWorkspaceManualKeys, WorkspaceKeyExport,
};
pub(crate) use local_file_io::{read_local_metadata_file_with_limit, write_secret_file};
#[cfg(test)]
pub(crate) use local_secret::LOCAL_SECRET_STORAGE;
pub(crate) use local_secret::{
    LOCAL_SECRET_FILE_MAX_BYTES, LOCAL_SECRET_KIND_CHANNEL_KEY,
    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP, LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP, LOCAL_SECRET_KIND_WORKSPACE_KEY,
    encrypt_local_secret, open_serialized_local_secret, openmls_group_secret_kind,
};
pub use openmls_actions::{
    AddedOpenMlsChannelGroupMember, AddedOpenMlsWorkspaceGroupMember,
    AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits,
    CreatedOpenMlsChannelGroup, CreatedOpenMlsWorkspaceGroup, JoinedOpenMlsChannelGroup,
    JoinedOpenMlsWorkspaceGroup, PublishedOpenMlsKeyPackage, RemovedOpenMlsChannelGroupMember,
    RemovedOpenMlsWorkspaceGroupMember, UpdatedOpenMlsChannelGroup, UpdatedOpenMlsWorkspaceGroup,
    UpdatedWorkspaceOpenMlsGroups,
};
pub(crate) use openmls_provisioning::{
    OpenMlsAutoProvisionIndex, ProvisionedOpenMlsChannelMembers,
    current_private_channel_member_ids_from_events,
};
pub use paths::RuntimePaths;
#[cfg(test)]
pub(crate) use paths::{RUNTIME_PASSPHRASE_MAX_BYTES, RUNTIME_PATH_MAX_BYTES};
pub(crate) use paths::{
    normalize_runtime_identity_passphrase, validate_runtime_path, validate_runtime_paths,
};
pub(crate) use publish_queue::{
    MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS, MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS,
    MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS, attachment_blob_hashes,
    workspace_publish_queue_summary,
};
pub use publish_queue::{
    WorkspacePublishQueue, WorkspacePublishQueueChannelSummary, WorkspacePublishQueueSummary,
};
pub use recovery_bundle::{
    ImportedWorkspaceRecoveryBundle, WorkspaceRecoveryBundle, WorkspaceRecoveryBundleKdf,
};
#[cfg(test)]
pub(crate) use recovery_bundle::{
    RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB, RECOVERY_BUNDLE_ARGON2_PARALLELISM,
    RECOVERY_BUNDLE_ARGON2_TIME_COST, RECOVERY_BUNDLE_KDF_ARGON2ID,
    RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY, RECOVERY_BUNDLE_KDF_CONTEXT,
    RECOVERY_BUNDLE_KDF_OUTPUT_LEN, RECOVERY_BUNDLE_SALT_LEN, RECOVERY_BUNDLE_SCHEMA_VERSION,
    WorkspaceRecoveryBundlePlaintext, derive_recovery_bundle_key, recovery_bundle_aad,
    recovery_bundle_key_id,
};
#[cfg(test)]
pub(crate) use runtime_validation::DEVICE_ID_REFERENCE_MAX_BYTES;
pub(crate) use runtime_validation::{
    validate_channel_id_reference, validate_device_key_package_id_reference,
    validate_event_id_reference, validate_message_id_reference, validate_message_markdown_size,
    validate_peer_address, validate_peer_addresses, validate_search_query_size,
    validate_workspace_id_reference,
};
#[cfg(test)]
pub(crate) use search_results::LOCAL_SEARCH_VISIBLE_HIT_LIMIT;
pub use search_results::{IndexedWorkspaceSearch, SearchedWorkspace, WorkspaceSearchHit};
pub(crate) use search_results::{LOCAL_SEARCH_RAW_HIT_LIMIT, SEARCH_QUERY_MAX_BYTES};
pub use storage_diagnostics::{WorkspaceStorageHealth, WorkspaceStorageRepair};
pub(crate) use sync_results::merge_published_workspace;
pub use sync_results::{
    PublishedWorkspace, PulledOpenMlsCatchup, PulledOpenMlsChannelCatchup, PulledWorkspace,
    PulledWorkspaceGap, SyncedWorkspace,
};
pub use workspace_actions::{
    AddedChannelMember, AddedReaction, CreatedChannel, CreatedMessage, CreatedWorkspace,
    DeletedMessage, EditedMessage, InvitedMember, MarkedChannelRead, PrunedBlobCache,
    PublishedDeviceKeyPackage, PublishedPeerEndpoint, RemovedChannelMember,
    RemovedChannelMemberWithKeyRotation, RemovedChannelMemberWithOpenMls, RemovedMember,
    RemovedMemberWithKeyRotation, RemovedMemberWithOpenMls, RemovedReaction, SavedAttachment,
    UpdatedDeviceProfile,
};
#[cfg(test)]
pub(crate) use workspace_listing::MAX_WORKSPACE_SUMMARY_PAGE_ROWS;
pub use workspace_listing::{LocalWorkspaceSummary, LocalWorkspaceSummaryPage};

#[cfg(test)]
use chaft_app::WorkspaceSnapshotOptions;
#[cfg(test)]
use chaft_crypto::seal_message_markdown;
#[cfg(test)]
use chaft_media::{BLOB_DESCRIPTOR_MAX_CHUNKS, BlobAvailability};
#[cfg(test)]
use chaft_net_direct::{
    AuthorizedPublishTransport, BlobSyncTransport, MAX_PUBLISH_EVENTS_PER_REQUEST,
};
#[cfg(test)]
use chaft_types::{MESSAGE_MARKDOWN_MAX_BYTES, REACTION_TEXT_MAX_BYTES};

const DIRECT_WHOLE_BLOB_SYNC_LIMIT: usize = 4 * 1024 * 1024;
const DIRECT_BLOB_CHUNK_SIZE: usize = 4 * 1024 * 1024;
pub(crate) const DEVICE_KEY_PACKAGE_MAX_LEN: usize = 64 * 1024;
const MAX_WORKSPACE_MEMBER_PAGE_ROWS: usize = 128;
const MAX_WORKSPACE_CHANNEL_PAGE_ROWS: usize = 128;
const MAX_WORKSPACE_CHANNEL_SEARCH_ROWS: usize = 128;
const CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV: &str = "aes-256-gcm-siv";
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

    pub fn device_id(&self) -> &DeviceId {
        self.identity.device_id()
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

        Ok(Some(ResolvedContentKey::new(exported.key_id, content_key)))
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

        Ok(Some(ResolvedContentKey::new(exported.key_id, content_key)))
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

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Barrier, Mutex,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
        },
        thread,
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use async_trait::async_trait;
    use chaft_crypto::{
        CryptoError, PayloadEncryption, SealedPayload, open_attachment_blob, open_message_markdown,
        seal_aes_256_gcm_siv, sealed_payload_from_encrypted_blob_ref,
    };
    use chaft_media::{BlobDescriptor, BlobStore, blob_hash, describe_blob};
    use chaft_net::{ChaftTransport, PeerAddress, PeerId};
    use chaft_net_direct::{DirectPeerServer, DirectTransport};
    use chaft_net_iroh::IrohTransport;
    use chaft_store::EventStore;
    use chaft_types::{
        ATTACHMENT_BLOB_HASH_MAX_BYTES, CHANNEL_NAME_MAX_BYTES, ContentKeyScope,
        DEVICE_DISPLAY_NAME_MAX_BYTES, DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, EncryptedBlobRef,
        EventBody, PEER_ENDPOINT_TRANSPORT_MAX_BYTES, SignedTrustSnapshot, WORKSPACE_ID_MAX_BYTES,
        WORKSPACE_NAME_MAX_BYTES,
    };
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
