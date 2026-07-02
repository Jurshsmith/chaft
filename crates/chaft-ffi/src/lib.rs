use std::{
    collections::{BTreeSet, HashMap},
    ffi::{CString, c_char},
    future::Future,
    io::Read,
    path::{Path, PathBuf},
    ptr, slice,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

use chaft_app::{
    ChannelSnapshot, DeviceKeyPackageSnapshot, DeviceProfileSnapshot, MAX_TIMELINE_WINDOW_ROWS,
    TimelineItem, TimelineItemKind, WorkspaceChannelPage, WorkspaceChannelSearch,
    WorkspaceMemberPage, WorkspaceMemberSnapshot, WorkspaceSnapshot, WorkspaceSnapshotOptions,
};
use chaft_media::BlobStore;
use chaft_net::{PeerAddress, PeerId};
#[cfg(test)]
use chaft_net_direct::DirectTransport;
use chaft_net_direct::{DirectPeerServer, SyncPeerStore};
use chaft_net_iroh::{IrohSyncPeer, IrohTransport, IrohTransportConfig};
use chaft_runtime::{
    AddedChannelMember, AddedOpenMlsChannelGroupMember, AddedOpenMlsWorkspaceGroupMember,
    AddedReaction, AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits,
    BlobTransferAttempt, BlobTransferRetryReport, ChannelKeyExport, CreatedChannel, CreatedMessage,
    CreatedOpenMlsChannelGroup, CreatedOpenMlsWorkspaceGroup, CreatedWorkspace, DeletedMessage,
    EditedMessage, ImportedChannelKey, ImportedWorkspaceKey, ImportedWorkspaceRecoveryBundle,
    IndexedWorkspaceSearch, InvitedMember, JoinedOpenMlsChannelGroup, JoinedOpenMlsWorkspaceGroup,
    LocalRuntime, LocalWorkspaceSummary, LocalWorkspaceSummaryPage, MarkedChannelRead,
    PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES,
    PrunedBlobCache, PublishedDeviceKeyPackage, PublishedOpenMlsKeyPackage, PublishedPeerEndpoint,
    PublishedWorkspace, PulledOpenMlsCatchup, PulledWorkspace, RemovedChannelMember,
    RemovedChannelMemberWithKeyRotation, RemovedChannelMemberWithOpenMls, RemovedMember,
    RemovedMemberWithKeyRotation, RemovedMemberWithOpenMls, RemovedOpenMlsChannelGroupMember,
    RemovedOpenMlsWorkspaceGroupMember, RemovedReaction, RotatedChannelKey,
    RotatedWorkspaceForSuspectedCompromise, RotatedWorkspaceKey, RotatedWorkspaceManualKeys,
    RuntimeError, SavedAttachment, SearchedWorkspace, SyncedWorkspace, UpdatedDeviceProfile,
    UpdatedOpenMlsChannelGroup, UpdatedOpenMlsWorkspaceGroup, UpdatedWorkspaceOpenMlsGroups,
    WorkspaceCompromiseReport, WorkspaceCompromiseResponse, WorkspaceKeyExport,
    WorkspacePublishQueue, WorkspaceRecoveryBundle, WorkspaceStorageHealth, WorkspaceStorageRepair,
};
use chaft_store::EventStore;
use chaft_types::{
    ATTACHMENT_ID_MAX_BYTES, ATTACHMENT_MEDIA_TYPE_MAX_BYTES, CHANNEL_ID_MAX_BYTES,
    CHANNEL_NAME_MAX_BYTES, ChannelId, DEVICE_DISPLAY_NAME_MAX_BYTES, DEVICE_ID_MAX_BYTES,
    DEVICE_KEY_PACKAGE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, DeviceId,
    DeviceKeyPackageId, EVENT_ID_MAX_BYTES, EventId, MESSAGE_ID_MAX_BYTES,
    MESSAGE_MARKDOWN_MAX_BYTES, MessageId, PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
    REACTION_TEXT_MAX_BYTES, SignedEvent, SignedTrustSnapshot, WORKSPACE_ID_MAX_BYTES,
    WORKSPACE_NAME_MAX_BYTES, WorkspaceId, WorkspaceRole, direct_tcp_peer_listen_address_is_valid,
    is_canonical_event_id_str, peer_endpoint_hint_is_supported,
    peer_endpoint_hint_transport_is_consistent, validate_channel_id_str, validate_device_id_str,
    validate_device_key_package_id_str, validate_event_id_str, validate_message_id_str,
    validate_workspace_id_str,
};
use serde::Serialize;
use tokio::sync::oneshot;
use zeroize::Zeroizing;

static HOSTED_PEERS: OnceLock<Mutex<HashMap<String, RunningPeer>>> = OnceLock::new();
static HOSTED_PEER_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static RUNTIME_IDENTITY_PASSPHRASES: OnceLock<Mutex<HashMap<String, Zeroizing<String>>>> =
    OnceLock::new();

const MAX_RESULT_EVENT_ID_SAMPLE_ROWS: usize = 128;
const MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS: usize = 128;
const MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS: usize = 128;
const MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS: usize = 128;
const MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS: usize = 128;
const MAX_RESULT_BLOB_HASH_SAMPLE_ROWS: usize = 64;
const MAX_RESULT_GAP_SAMPLE_ROWS: usize = 64;
const MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS: usize = 64;
const MAX_RESULT_PEER_ERROR_SAMPLE_ROWS: usize = 64;
const MAX_RESULT_PEER_ERROR_MESSAGE_BYTES: usize = 2 * 1024;
const MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS: usize = 32;
const MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS: usize = 64;
const MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS: usize = 64;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FfiResult<T>
where
    T: Serialize,
{
    ok: bool,
    value: Option<T>,
    error: Option<FfiError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FfiError {
    code: &'static str,
    message: String,
}

const DEVICE_KEY_PACKAGE_FILE_MAX_BYTES: u64 = 64 * 1024;
const WORKSPACE_EVENTS_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
const KEY_TRANSFER_JSON_MAX_BYTES: usize = 256 * 1024;
const RECOVERY_BUNDLE_JSON_MAX_BYTES: usize = 4 * 1024 * 1024;
const PEER_ENDPOINT_LIST_TEXT_MAX_BYTES: usize =
    PEER_ENDPOINT_LIST_MAX_ITEMS * (PEER_ENDPOINT_MAX_BYTES + 1);
const SEARCH_QUERY_MAX_BYTES: usize = 512;
const FFI_PATH_MAX_BYTES: usize = 64 * 1024;
const FFI_PASSPHRASE_MAX_BYTES: usize = 16 * 1024;
const WORKSPACE_ROLE_TEXT_MAX_BYTES: usize = 16;
const FFI_GENERIC_STRING_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDevice {
    device_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostedPeer {
    peer_id: String,
    endpoint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoppedPeer {
    peer_id: String,
    endpoint: String,
}

struct RunningPeer {
    endpoint: String,
    shutdown: oneshot::Sender<()>,
    thread: thread::JoinHandle<()>,
}

#[unsafe(no_mangle)]
pub extern "C" fn chaft_core_version() -> *const c_char {
    c"0.1.0".as_ptr()
}

/// Stores a runtime unlock passphrase in process memory for subsequent FFI calls.
///
/// This is keyed by `data_dir` and takes precedence over the
/// `CHAFT_IDENTITY_PASSPHRASE` development fallback. The passphrase is never
/// written to `desktop.json`; callers should clear it with
/// `chaft_runtime_clear_identity_passphrase` when locking the runtime.
///
/// # Safety
///
/// `data_dir` and `passphrase` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_set_identity_passphrase(
    data_dir: *const c_char,
    passphrase: *const c_char,
) -> bool {
    set_runtime_identity_passphrase_result(data_dir, passphrase).is_ok()
}

/// Clears the in-process runtime unlock passphrase for `data_dir`.
///
/// # Safety
///
/// `data_dir` must be a valid, non-null pointer to a NUL-terminated UTF-8 string
/// for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_clear_identity_passphrase(data_dir: *const c_char) -> bool {
    clear_runtime_identity_passphrase_result(data_dir).is_ok()
}

/// Returns an owned JSON `WorkspaceSnapshot` for desktop bootstrap previews.
///
/// The caller owns the returned string and must release it with
/// `chaft_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn chaft_demo_workspace_snapshot_json() -> *mut c_char {
    into_c_string(&demo_workspace_snapshot())
}

/// Builds a workspace snapshot from a UTF-8 workspace ID and JSON array of
/// signed events.
///
/// The returned string is a JSON result envelope:
/// `{ "ok": true, "value": WorkspaceSnapshot, "error": null }` or
/// `{ "ok": false, "value": null, "error": { "code": ..., "message": ... } }`.
/// The caller owns the returned string and must release it with
/// `chaft_string_free`.
///
/// # Safety
///
/// `workspace_id` and `events_json` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_workspace_snapshot_from_events_result_json(
    workspace_id: *const c_char,
    events_json: *const c_char,
) -> *mut c_char {
    let result = workspace_snapshot_from_events_result(workspace_id, events_json);
    into_c_string(&result)
}

/// Builds a workspace snapshot from a local SQLite event store.
///
/// The returned string is a JSON result envelope with the same shape as
/// `chaft_workspace_snapshot_from_events_result_json`. The caller owns the
/// returned string and must release it with `chaft_string_free`.
///
/// # Safety
///
/// `store_path` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_workspace_snapshot_from_store_result_json(
    store_path: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = workspace_snapshot_from_store_result(store_path, workspace_id);
    into_c_string(&result)
}

/// Builds a local-store workspace snapshot containing only the latest
/// `timeline_limit` timeline rows plus `timelineWindow` metadata.
/// The effective timeline limit is capped by the app view model window budget.
///
/// The returned string is a JSON result envelope with the same shape as
/// `chaft_workspace_snapshot_from_events_result_json`. The caller owns the
/// returned string and must release it with `chaft_string_free`.
///
/// # Safety
///
/// `store_path` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_workspace_snapshot_from_store_latest_result_json(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> *mut c_char {
    let result =
        workspace_snapshot_from_store_latest_result(store_path, workspace_id, timeline_limit);
    into_c_string(&result)
}

/// Builds a local-store workspace snapshot for the requested timeline window.
///
/// `timeline_start` is the zero-based row index inside the full materialized
/// timeline, and `timeline_limit` is the maximum number of rows to serialize.
/// The effective timeline limit is capped by the app view model window budget.
///
/// # Safety
///
/// `store_path` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_workspace_snapshot_from_store_window_result_json(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> *mut c_char {
    let result = workspace_snapshot_from_store_window_result(
        store_path,
        workspace_id,
        timeline_start,
        timeline_limit,
    );
    into_c_string(&result)
}

/// Builds a decrypted workspace snapshot from a Chaft runtime data directory.
///
/// The returned string is a JSON result envelope with the same shape as
/// `chaft_workspace_snapshot_from_events_result_json`. Unlike the raw store
/// function, this opens the local workspace key from the runtime directory and
/// renders locally decrypted message bodies where possible. The caller owns the
/// returned string and must release it with `chaft_string_free`.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call. `identity_file`
/// may be null, in which case the runtime default identity path is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_decrypted_workspace_snapshot_from_runtime_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        decrypted_workspace_snapshot_from_runtime_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Builds a decrypted runtime snapshot containing only the latest
/// `timeline_limit` timeline rows plus `timelineWindow` metadata.
/// The effective timeline limit is capped by the app view model window budget.
///
/// The returned string is a JSON result envelope with the same shape as
/// `chaft_workspace_snapshot_from_events_result_json`. The caller owns the
/// returned string and must release it with `chaft_string_free`.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call. `identity_file`
/// may be null, in which case the runtime default identity path is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_decrypted_workspace_snapshot_from_runtime_latest_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> *mut c_char {
    let result = decrypted_workspace_snapshot_from_runtime_latest_result(
        data_dir,
        identity_file,
        workspace_id,
        timeline_limit,
    );
    into_c_string(&result)
}

/// Builds a decrypted runtime snapshot for the requested timeline window.
///
/// `timeline_start` is the zero-based row index inside the full materialized
/// timeline, and `timeline_limit` is the maximum number of rows to serialize.
/// The effective timeline limit is capped by the app view model window budget.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call. `identity_file`
/// may be null, in which case the runtime default identity path is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_decrypted_workspace_snapshot_from_runtime_window_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> *mut c_char {
    let result = decrypted_workspace_snapshot_from_runtime_window_result(
        data_dir,
        identity_file,
        workspace_id,
        timeline_start,
        timeline_limit,
    );
    into_c_string(&result)
}

/// Builds a decrypted runtime snapshot containing only the latest timeline rows
/// for one channel.
/// The effective timeline limit is capped by the app view model window budget.
///
/// # Safety
///
/// `data_dir`, `workspace_id`, and `channel_id` must be valid, non-null
/// pointers to NUL-terminated UTF-8 strings for the duration of this call.
/// `identity_file` may be null, in which case the runtime default identity path
/// is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_decrypted_workspace_channel_snapshot_from_runtime_latest_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_limit: usize,
) -> *mut c_char {
    let result = decrypted_workspace_channel_snapshot_from_runtime_latest_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        timeline_limit,
    );
    into_c_string(&result)
}

/// Builds a decrypted runtime snapshot for one channel timeline window.
/// The effective timeline limit is capped by the app view model window budget.
///
/// # Safety
///
/// `data_dir`, `workspace_id`, and `channel_id` must be valid, non-null
/// pointers to NUL-terminated UTF-8 strings for the duration of this call.
/// `identity_file` may be null, in which case the runtime default identity path
/// is used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_decrypted_workspace_channel_snapshot_from_runtime_window_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> *mut c_char {
    let result = decrypted_workspace_channel_snapshot_from_runtime_window_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        timeline_start,
        timeline_limit,
    );
    into_c_string(&result)
}

/// Opens or creates a local runtime and returns the device ID.
///
/// # Safety
///
/// `data_dir` must be a valid, non-null pointer to a NUL-terminated UTF-8
/// string. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_device_id_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> *mut c_char {
    let result = runtime_device_id_result(data_dir, identity_file);
    into_c_string(&result)
}

/// Lists locally known workspaces in a runtime data directory.
///
/// # Safety
///
/// `data_dir` must be a valid, non-null pointer to a NUL-terminated UTF-8
/// string. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_workspaces_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> *mut c_char {
    let result = runtime_list_workspaces_result(data_dir, identity_file);
    into_c_string(&result)
}

/// Lists one bounded page of locally known workspace summaries.
///
/// # Safety
///
/// `data_dir` must be a valid, non-null pointer to a NUL-terminated UTF-8
/// string. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_workspace_page_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    start_index: usize,
    limit: usize,
) -> *mut c_char {
    let result = runtime_list_workspace_page_result(data_dir, identity_file, start_index, limit);
    into_c_string(&result)
}

/// Reports compact local event-store health for one runtime workspace.
///
/// This diagnostic counts total local rows, parseable rows, corrupt rows,
/// signature-valid metadata rows, and readable servable rows. It does not
/// contact peers or require workspace content keys.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call. `identity_file`
/// may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_workspace_storage_health_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_workspace_storage_health_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Repairs local event-store servable metadata for one runtime workspace.
///
/// This diagnostic repair recomputes each row's self-contained-signature-valid
/// metadata from the stored signed event bytes. It does not delete event rows,
/// contact peers, or require workspace content keys.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid, non-null pointers to
/// NUL-terminated UTF-8 strings for the duration of this call. `identity_file`
/// may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_repair_workspace_storage_metadata_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_repair_workspace_storage_metadata_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Lists one bounded page of workspace members for desktop management.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_workspace_member_page_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> *mut c_char {
    let result = runtime_list_workspace_member_page_result(
        data_dir,
        identity_file,
        workspace_id,
        start_index,
        limit,
    );
    into_c_string(&result)
}

/// Lists one bounded page of workspace channels for desktop navigation.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_workspace_channel_page_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> *mut c_char {
    let result = runtime_list_workspace_channel_page_result(
        data_dir,
        identity_file,
        workspace_id,
        start_index,
        limit,
    );
    into_c_string(&result)
}

/// Lists the bounded workspace channel page that contains `channel_id`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_workspace_channel_page_containing_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    limit: usize,
) -> *mut c_char {
    let result = runtime_list_workspace_channel_page_containing_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        limit,
    );
    into_c_string(&result)
}

/// Searches accessible workspace channels by name or channel ID.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_search_workspace_channels_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
    limit: usize,
) -> *mut c_char {
    let result = runtime_search_workspace_channels_result(
        data_dir,
        identity_file,
        workspace_id,
        query,
        limit,
    );
    into_c_string(&result)
}

/// Creates a workspace and default channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_workspace_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    name: *const c_char,
    default_channel_name: *const c_char,
) -> *mut c_char {
    let result =
        runtime_create_workspace_result(data_dir, identity_file, name, default_channel_name);
    into_c_string(&result)
}

/// Creates a channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_channel_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    name: *const c_char,
    is_private: bool,
) -> *mut c_char {
    let result =
        runtime_create_channel_result(data_dir, identity_file, workspace_id, name, is_private);
    into_c_string(&result)
}

/// Updates this device's signed display profile in a local workspace.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_device_profile_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
) -> *mut c_char {
    let result =
        runtime_update_device_profile_result(data_dir, identity_file, workspace_id, display_name);
    into_c_string(&result)
}

/// Publishes an opaque device key package file as signed workspace metadata.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_device_key_package_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    protocol: *const c_char,
    key_package_file: *const c_char,
) -> *mut c_char {
    let result = runtime_publish_device_key_package_result(
        data_dir,
        identity_file,
        workspace_id,
        protocol,
        key_package_file,
    );
    into_c_string(&result)
}

/// Publishes a signed peer endpoint hint for this workspace member.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_peer_endpoint_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    endpoint_id: *const c_char,
    endpoint: *const c_char,
    transport: *const c_char,
    is_backup_peer: bool,
    has_expires_at_ms: bool,
    expires_at_ms: i64,
) -> *mut c_char {
    let result = runtime_publish_peer_endpoint_result(PeerEndpointFfiArgs {
        data_dir,
        identity_file,
        workspace_id,
        endpoint_id,
        endpoint,
        transport,
        is_backup_peer,
        has_expires_at_ms,
        expires_at_ms,
    });
    into_c_string(&result)
}

/// Generates and publishes an OpenMLS device key package as signed workspace
/// metadata, storing the corresponding private bundle in the local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_openmls_device_key_package_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_publish_openmls_device_key_package_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Creates the local private OpenMLS workspace group state for a workspace.
///
/// This writes local MLS state only; it does not publish key material or group
/// epoch secrets to peers.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_openmls_workspace_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_create_openmls_workspace_group_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Adds an invited member to the local OpenMLS workspace group using their
/// published key package.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_add_openmls_workspace_group_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    key_package_id: *const c_char,
) -> *mut c_char {
    let result = runtime_add_openmls_workspace_group_member_result(
        data_dir,
        identity_file,
        workspace_id,
        key_package_id,
    );
    into_c_string(&result)
}

/// Removes a member from the local OpenMLS workspace group and publishes the
/// remove commit.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_openmls_workspace_group_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_openmls_workspace_group_member_result(
        data_dir,
        identity_file,
        workspace_id,
        device_id,
    );
    into_c_string(&result)
}

/// Joins the local device to an OpenMLS workspace group from a replicated
/// welcome event.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and
/// `source_event_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_join_openmls_workspace_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> *mut c_char {
    let result = runtime_join_openmls_workspace_group_result(
        data_dir,
        identity_file,
        workspace_id,
        source_event_id,
    );
    into_c_string(&result)
}

/// Publishes an OpenMLS workspace self-update commit and advances local group state.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_openmls_workspace_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_update_openmls_workspace_group_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Publishes OpenMLS self-update commits for every local group in a workspace.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_workspace_openmls_groups_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_update_workspace_openmls_groups_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Applies replicated OpenMLS workspace group commits to the local group state.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and
/// `source_event_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_apply_openmls_workspace_group_commits_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> *mut c_char {
    let result = runtime_apply_openmls_workspace_group_commits_result(
        data_dir,
        identity_file,
        workspace_id,
        source_event_id,
    );
    into_c_string(&result)
}

/// Creates local private OpenMLS group state for a channel.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_openmls_channel_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> *mut c_char {
    let result = runtime_create_openmls_channel_group_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
    );
    into_c_string(&result)
}

/// Adds an invited channel member to a channel OpenMLS group using their
/// published key package.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_add_openmls_channel_group_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    key_package_id: *const c_char,
) -> *mut c_char {
    let result = runtime_add_openmls_channel_group_member_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        key_package_id,
    );
    into_c_string(&result)
}

/// Removes a member from the local channel OpenMLS group and publishes the
/// remove commit.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_openmls_channel_group_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_openmls_channel_group_member_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        device_id,
    );
    into_c_string(&result)
}

/// Joins the local device to a channel OpenMLS group from a replicated welcome
/// event.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and
/// `source_event_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_join_openmls_channel_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> *mut c_char {
    let result = runtime_join_openmls_channel_group_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        source_event_id,
    );
    into_c_string(&result)
}

/// Publishes an OpenMLS channel self-update commit and advances local group state.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_openmls_channel_group_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> *mut c_char {
    let result = runtime_update_openmls_channel_group_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
    );
    into_c_string(&result)
}

/// Applies replicated OpenMLS channel group commits to the local group state.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and
/// `source_event_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_apply_openmls_channel_group_commits_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> *mut c_char {
    let result = runtime_apply_openmls_channel_group_commits_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        source_event_id,
    );
    into_c_string(&result)
}

/// Sends an encrypted message in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_send_message_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
) -> *mut c_char {
    let result =
        runtime_send_message_result(data_dir, identity_file, workspace_id, channel_id, text);
    into_c_string(&result)
}

/// Sends an encrypted message reply in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_send_message_reply_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
) -> *mut c_char {
    let result = runtime_send_message_reply_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        reply_to_message_id,
        text,
    );
    into_c_string(&result)
}

/// Sends an encrypted message with one encrypted local file attachment.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_send_attachment_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> *mut c_char {
    let result = runtime_send_attachment_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        text,
        file_path,
        media_type,
    );
    into_c_string(&result)
}

/// Sends an encrypted message reply with one encrypted local file attachment.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
/// `reply_to_message_id` may be null or empty to send a normal attachment
/// message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_send_attachment_reply_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> *mut c_char {
    let result = runtime_send_attachment_reply_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        reply_to_message_id,
        text,
        file_path,
        media_type,
    );
    into_c_string(&result)
}

/// Decrypts a locally available encrypted attachment to an output file.
///
/// `blob_hash` is kept for ABI compatibility and may contain either a stable
/// attachment ID or a legacy blob hash selector.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_save_attachment_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    blob_hash: *const c_char,
    output_path: *const c_char,
) -> *mut c_char {
    let result = runtime_save_attachment_result(
        data_dir,
        identity_file,
        workspace_id,
        message_id,
        blob_hash,
        output_path,
    );
    into_c_string(&result)
}

/// Prunes unreferenced local ciphertext blobs from a runtime blob cache.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_prune_blobs_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> *mut c_char {
    let result = runtime_prune_blobs_result(data_dir, identity_file);
    into_c_string(&result)
}

/// Edits a message with a new encrypted body in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_edit_message_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    text: *const c_char,
) -> *mut c_char {
    let result =
        runtime_edit_message_result(data_dir, identity_file, workspace_id, message_id, text);
    into_c_string(&result)
}

/// Deletes a message in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_delete_message_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
) -> *mut c_char {
    let result = runtime_delete_message_result(data_dir, identity_file, workspace_id, message_id);
    into_c_string(&result)
}

/// Adds a reaction to a message in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_add_reaction_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> *mut c_char {
    let result =
        runtime_add_reaction_result(data_dir, identity_file, workspace_id, message_id, reaction);
    into_c_string(&result)
}

/// Removes one reaction count from a message in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_reaction_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> *mut c_char {
    let result =
        runtime_remove_reaction_result(data_dir, identity_file, workspace_id, message_id, reaction);
    into_c_string(&result)
}

/// Marks a channel read through its latest locally known message in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_mark_channel_read_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_mark_channel_read_result(data_dir, identity_file, workspace_id, channel_id);
    into_c_string(&result)
}

/// Invites a device to a workspace in a local runtime.
///
/// `role` accepts `owner`, `admin`, `member`, or `guest`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_invite_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
    role: *const c_char,
) -> *mut c_char {
    let result =
        runtime_invite_member_result(data_dir, identity_file, workspace_id, device_id, role);
    into_c_string(&result)
}

/// Removes a device from a workspace in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_member_result(data_dir, identity_file, workspace_id, device_id);
    into_c_string(&result)
}

/// Removes a device from the OpenMLS workspace group first, then revokes
/// workspace membership in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_member_with_openmls_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_remove_member_with_openmls_result(data_dir, identity_file, workspace_id, device_id);
    into_c_string(&result)
}

/// Removes a device from a workspace, then rotates local manual workspace and
/// private-channel content keys in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_member_with_key_rotation_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_member_with_key_rotation_result(
        data_dir,
        identity_file,
        workspace_id,
        device_id,
    );
    into_c_string(&result)
}

/// Adds an invited workspace device to a private channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_add_channel_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_add_channel_member_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        device_id,
    );
    into_c_string(&result)
}

/// Removes a device from a private channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_channel_member_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_channel_member_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        device_id,
    );
    into_c_string(&result)
}

/// Removes a device from the channel OpenMLS group first, then revokes private
/// channel access in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_channel_member_with_openmls_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_channel_member_with_openmls_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        device_id,
    );
    into_c_string(&result)
}

/// Removes a device from a private channel, then rotates that channel's local
/// manual content key in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_remove_channel_member_with_key_rotation_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_remove_channel_member_with_key_rotation_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        device_id,
    );
    into_c_string(&result)
}

/// Exports a plaintext workspace key bundle for explicit manual transfer.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_export_workspace_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_export_workspace_key_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Rotates the local workspace content key and publishes signed epoch metadata.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_rotate_workspace_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_rotate_workspace_key_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Rotates the local manual workspace content key and every local manual
/// private-channel content key for the workspace.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_rotate_workspace_manual_keys_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_rotate_workspace_manual_keys_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Rotates all local workspace key material for suspected compromise.
///
/// This publishes OpenMLS self-update events for local OpenMLS groups when they
/// exist, then rotates manual fallback workspace/private-channel key rings when
/// present.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_rotate_workspace_for_suspected_compromise_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_rotate_workspace_for_suspected_compromise_result(
        data_dir,
        identity_file,
        workspace_id,
    );
    into_c_string(&result)
}

/// Reports conservative suspected-compromise signals for a workspace.
///
/// This scans stored workspace events for invalid self-contained signatures and
/// returns a local-device rotation trigger flag without modifying local keys.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_detect_compromise_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_detect_compromise_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Runs the local suspected-compromise response policy for a workspace.
///
/// The policy rotates local secret state only for unhandled local-device
/// compromise signals; remote-only signals remain review-only.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_respond_compromise_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_respond_compromise_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Exports a root-signed trust snapshot proving the materialized workspace
/// authorization graph.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_export_trust_snapshot_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_export_trust_snapshot_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Imports a plaintext workspace key bundle JSON into a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_import_workspace_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> *mut c_char {
    let result = runtime_import_workspace_key_result(data_dir, identity_file, key_json);
    into_c_string(&result)
}

/// Exports a plaintext private-channel key bundle for explicit manual transfer.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_export_channel_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_export_channel_key_result(data_dir, identity_file, workspace_id, channel_id);
    into_c_string(&result)
}

/// Rotates the local private-channel content key and publishes signed epoch
/// metadata scoped to that channel.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_rotate_channel_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> *mut c_char {
    let result =
        runtime_rotate_channel_key_result(data_dir, identity_file, workspace_id, channel_id);
    into_c_string(&result)
}

/// Imports a plaintext private-channel key bundle JSON into a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_import_channel_key_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> *mut c_char {
    let result = runtime_import_channel_key_result(data_dir, identity_file, key_json);
    into_c_string(&result)
}

/// Exports a passphrase-encrypted recovery bundle containing workspace and
/// local private-channel keys for explicit manual recovery or device transfer.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_export_recovery_bundle_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    passphrase: *const c_char,
) -> *mut c_char {
    let result =
        runtime_export_recovery_bundle_result(data_dir, identity_file, workspace_id, passphrase);
    into_c_string(&result)
}

/// Imports a passphrase-encrypted recovery bundle into a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_import_recovery_bundle_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    bundle_json: *const c_char,
    passphrase: *const c_char,
) -> *mut c_char {
    let result =
        runtime_import_recovery_bundle_result(data_dir, identity_file, bundle_json, passphrase);
    into_c_string(&result)
}

/// Rebuilds the private local search index for a workspace using local keys.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_reindex_workspace_search_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_reindex_workspace_search_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Searches the private local search index for a workspace.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_search_workspace_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
) -> *mut c_char {
    let result = runtime_search_workspace_result(data_dir, identity_file, workspace_id, query);
    into_c_string(&result)
}

/// Reports locally materialized events and blobs that are ready for a future
/// publish/backup without contacting a peer.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_workspace_publish_queue_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_workspace_publish_queue_result(data_dir, identity_file, workspace_id);
    into_c_string(&result)
}

/// Publishes all local events for a workspace to a peer endpoint.
///
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_workspace_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> *mut c_char {
    let result = runtime_publish_workspace_direct_result(
        data_dir,
        identity_file,
        workspace_id,
        peer_endpoint,
    );
    into_c_string(&result)
}

/// Publishes materialized content/activity events for a workspace plus a compact
/// trust snapshot to a backup peer endpoint.
///
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_backup_workspace_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> *mut c_char {
    let result = runtime_backup_workspace_direct_result(
        data_dir,
        identity_file,
        workspace_id,
        peer_endpoint,
    );
    into_c_string(&result)
}

/// Publishes one local workspace event plus a compact trust snapshot to a
/// peer endpoint.
///
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_event_with_trust_snapshot_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    event_id: *const c_char,
    peer_endpoint: *const c_char,
) -> *mut c_char {
    let result = runtime_publish_event_with_trust_snapshot_direct_result(
        data_dir,
        identity_file,
        workspace_id,
        event_id,
        peer_endpoint,
    );
    into_c_string(&result)
}

/// Pulls a workspace from a peer endpoint into the local runtime store.
///
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_pull_workspace_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> *mut c_char {
    let result =
        runtime_pull_workspace_direct_result(data_dir, identity_file, workspace_id, peer_endpoint);
    into_c_string(&result)
}

/// Publishes local workspace events, then pulls missing workspace events from a
/// peer endpoint.
///
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_sync_workspace_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> *mut c_char {
    let result =
        runtime_sync_workspace_direct_result(data_dir, identity_file, workspace_id, peer_endpoint);
    into_c_string(&result)
}

/// Retries pending or failed blob transfer ledger entries for a workspace.
///
/// `peer_endpoints` is a comma- or semicolon-separated list of peer endpoints.
/// This is a synchronous C ABI wrapper around async network I/O. Desktop apps
/// should call it from a worker thread, not the UI thread.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_retry_blob_transfers_direct_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoints: *const c_char,
) -> *mut c_char {
    let result = runtime_retry_blob_transfers_direct_result(
        data_dir,
        identity_file,
        workspace_id,
        peer_endpoints,
    );
    into_c_string(&result)
}

/// Starts a background direct TCP peer serving a local runtime event/blob store.
///
/// `listen` may be null or empty to use `127.0.0.1:0`. The returned peer ID can
/// be passed to `chaft_runtime_stop_direct_peer_result_json`.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
/// `identity_file` and `listen` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_start_direct_peer_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    listen: *const c_char,
) -> *mut c_char {
    let result = runtime_start_direct_peer_result(data_dir, identity_file, listen);
    into_c_string(&result)
}

/// Starts a background native Iroh peer serving a local runtime event/blob store.
///
/// The returned peer ID can be passed to
/// `chaft_runtime_stop_direct_peer_result_json`.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
/// `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_start_iroh_peer_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> *mut c_char {
    let result = runtime_start_iroh_peer_result(data_dir, identity_file);
    into_c_string(&result)
}

/// Stops a background peer previously started by this process.
///
/// # Safety
///
/// `peer_id` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_stop_direct_peer_result_json(
    peer_id: *const c_char,
) -> *mut c_char {
    let result = runtime_stop_direct_peer_result(peer_id);
    into_c_string(&result)
}

/// Releases strings returned by Chaft FFI functions.
///
/// # Safety
///
/// `value` must be either null or a pointer previously returned by a Chaft FFI
/// function that transfers string ownership to the caller. Passing a static
/// string such as `chaft_core_version()` or a pointer from another allocator is
/// undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    unsafe {
        drop(CString::from_raw(value));
    }
}

fn workspace_snapshot_from_events_result(
    workspace_id: *const c_char,
    events_json: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let events_json = read_c_string(events_json, "events_json")?;
        validate_json_payload_size(
            &events_json,
            WORKSPACE_EVENTS_JSON_MAX_BYTES,
            "events_json_too_large",
            "events JSON",
        )?;
        let events = serde_json::from_str::<Vec<SignedEvent>>(&events_json)
            .map_err(|error| ffi_error("invalid_events_json", error.to_string()))?;
        WorkspaceSnapshot::from_events(WorkspaceId(workspace_id), &events)
            .map_err(|error| ffi_error("snapshot_materialization_failed", error.to_string()))
    })
}

fn workspace_snapshot_from_store_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::full(),
    )
}

fn workspace_snapshot_from_store_latest_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::latest(bounded_timeline_limit(timeline_limit)),
    )
}

fn workspace_snapshot_from_store_window_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::window(timeline_start, bounded_timeline_limit(timeline_limit)),
    )
}

fn workspace_snapshot_from_store_with_options_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let store_path = read_c_string(store_path, "store_path")?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let store = EventStore::open(&store_path)
            .map_err(|error| ffi_error("store_open_failed", error.to_string()))?;
        let events = store
            .list_events_for_workspace(&workspace_id)
            .map_err(|error| ffi_error("store_read_failed", error.to_string()))?;
        WorkspaceSnapshot::from_events_with_options(WorkspaceId(workspace_id), &events, options)
            .map_err(|error| ffi_error("snapshot_materialization_failed", error.to_string()))
    })
}

fn decrypted_workspace_snapshot_from_runtime_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::full(),
    )
}

fn decrypted_workspace_snapshot_from_runtime_latest_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::latest(bounded_timeline_limit(timeline_limit)),
    )
}

fn decrypted_workspace_snapshot_from_runtime_window_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::window(timeline_start, bounded_timeline_limit(timeline_limit)),
    )
}

fn decrypted_workspace_channel_snapshot_from_runtime_latest_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            &WorkspaceSnapshotOptions::latest_for_channel(
                channel_id,
                bounded_timeline_limit(timeline_limit),
            ),
        )
    })
}

fn decrypted_workspace_channel_snapshot_from_runtime_window_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            &WorkspaceSnapshotOptions::window_for_channel(
                channel_id,
                timeline_start,
                bounded_timeline_limit(timeline_limit),
            ),
        )
    })
}

fn decrypted_workspace_snapshot_from_runtime_with_options_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            options,
        )
    })
}

fn decrypted_workspace_snapshot_from_runtime_with_options(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> Result<WorkspaceSnapshot, FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let identity_file = if identity_file.is_null() {
        None
    } else {
        Some(read_c_string(identity_file, "identity_file")?.into())
    };
    let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
    let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
    runtime
        .decrypted_workspace_snapshot_with_options(WorkspaceId(workspace_id), options)
        .map_err(|error| ffi_error("runtime_snapshot_failed", error.to_string()))
}

fn runtime_device_id_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<RuntimeDevice> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        Ok(RuntimeDevice {
            device_id: runtime.device_id().0.clone(),
        })
    })
}

fn runtime_list_workspaces_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<Vec<LocalWorkspaceSummary>> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .list_workspace_page(0, MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS)
            .map(|page| page.workspaces)
            .map_err(|error| ffi_error("runtime_list_workspaces_failed", error.to_string()))
    })
}

fn runtime_list_workspace_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<LocalWorkspaceSummaryPage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .list_workspace_page(start_index, limit)
            .map_err(|error| ffi_error("runtime_list_workspace_page_failed", error.to_string()))
    })
}

fn runtime_workspace_storage_health_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceStorageHealth> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .workspace_storage_health(workspace_id)
            .map_err(|error| {
                ffi_error("runtime_workspace_storage_health_failed", error.to_string())
            })
    })
}

fn runtime_repair_workspace_storage_metadata_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceStorageRepair> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .repair_workspace_storage_metadata(workspace_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_repair_workspace_storage_metadata_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_list_workspace_member_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<WorkspaceMemberPage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .list_workspace_member_page(workspace_id, start_index, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_member_page_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_list_workspace_channel_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<WorkspaceChannelPage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .list_workspace_channel_page(workspace_id, start_index, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_channel_page_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_list_workspace_channel_page_containing_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    limit: usize,
) -> FfiResult<WorkspaceChannelPage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        runtime
            .list_workspace_channel_page_containing(workspace_id, channel_id, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_channel_page_containing_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_search_workspace_channels_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
    limit: usize,
) -> FfiResult<WorkspaceChannelSearch> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        let query = read_c_string(query, "query")?;
        runtime
            .search_workspace_channels(workspace_id, query, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_search_workspace_channels_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_create_workspace_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    name: *const c_char,
    default_channel_name: *const c_char,
) -> FfiResult<CreatedWorkspace> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let name = read_c_string(name, "name")?;
        let default_channel_name = read_c_string(default_channel_name, "default_channel_name")?;
        runtime
            .create_workspace(name, default_channel_name)
            .map_err(|error| ffi_error("runtime_create_workspace_failed", error.to_string()))
    })
}

fn runtime_create_channel_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    name: *const c_char,
    is_private: bool,
) -> FfiResult<CreatedChannel> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let name = read_c_string(name, "name")?;
        runtime
            .create_channel(WorkspaceId(workspace_id), name, is_private)
            .map_err(|error| ffi_error("runtime_create_channel_failed", error.to_string()))
    })
}

fn runtime_update_device_profile_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
) -> FfiResult<UpdatedDeviceProfile> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        runtime
            .update_device_profile(WorkspaceId(workspace_id), display_name)
            .map_err(|error| ffi_error("runtime_update_device_profile_failed", error.to_string()))
    })
}

fn runtime_publish_device_key_package_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    protocol: *const c_char,
    key_package_file: *const c_char,
) -> FfiResult<PublishedDeviceKeyPackage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let protocol = read_c_string(protocol, "protocol")?;
        let key_package_file = read_c_string(key_package_file, "key_package_file")?;
        let key_package = read_device_key_package_file(Path::new(&key_package_file))?;
        runtime
            .publish_device_key_package(WorkspaceId(workspace_id), protocol, key_package)
            .map_err(|error| {
                ffi_error(
                    "runtime_publish_device_key_package_failed",
                    error.to_string(),
                )
            })
    })
}

struct PeerEndpointFfiArgs {
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    endpoint_id: *const c_char,
    endpoint: *const c_char,
    transport: *const c_char,
    is_backup_peer: bool,
    has_expires_at_ms: bool,
    expires_at_ms: i64,
}

fn runtime_publish_peer_endpoint_result(
    args: PeerEndpointFfiArgs,
) -> FfiResult<PublishedPeerEndpoint> {
    result_envelope(|| {
        let workspace_id = ffi_workspace_id_arg(read_c_string(args.workspace_id, "workspace_id")?)?;
        let endpoint_id = read_c_string(args.endpoint_id, "endpoint_id")?;
        let endpoint = read_c_string(args.endpoint, "endpoint")?;
        let transport = read_c_string(args.transport, "transport")?;
        let (endpoint_id, endpoint, transport) =
            validate_peer_endpoint_hint_inputs(endpoint_id, endpoint, transport)?;
        let runtime = open_runtime_from_ffi(args.data_dir, args.identity_file)?;
        runtime
            .publish_peer_endpoint(
                WorkspaceId(workspace_id),
                endpoint_id,
                endpoint,
                transport,
                args.is_backup_peer,
                args.has_expires_at_ms.then_some(args.expires_at_ms),
            )
            .map_err(|error| ffi_error("runtime_publish_peer_endpoint_failed", error.to_string()))
    })
}

fn runtime_publish_openmls_device_key_package_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<PublishedOpenMlsKeyPackage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .publish_openmls_device_key_package(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_publish_openmls_device_key_package_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_create_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<CreatedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .create_openmls_workspace_group(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_create_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_add_openmls_workspace_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    key_package_id: *const c_char,
) -> FfiResult<AddedOpenMlsWorkspaceGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let key_package_id =
            ffi_device_key_package_id_arg(read_c_string(key_package_id, "key_package_id")?)?;
        runtime
            .add_openmls_workspace_group_member(
                WorkspaceId(workspace_id),
                DeviceKeyPackageId(key_package_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_add_openmls_workspace_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_remove_openmls_workspace_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedOpenMlsWorkspaceGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_openmls_workspace_group_member(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_openmls_workspace_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_join_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<JoinedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .join_openmls_workspace_group(WorkspaceId(workspace_id), source_event_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_join_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_update_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<UpdatedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .update_openmls_workspace_group(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_update_workspace_openmls_groups_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<UpdatedWorkspaceOpenMlsGroups> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .update_workspace_openmls_groups(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_workspace_openmls_groups_failed",
                    error.to_string(),
                )
            })
            .map(sample_updated_workspace_openmls_groups_report)
    })
}

fn runtime_apply_openmls_workspace_group_commits_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<AppliedOpenMlsWorkspaceGroupCommits> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .apply_openmls_workspace_group_commits(WorkspaceId(workspace_id), source_event_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_apply_openmls_workspace_group_commits_failed",
                    error.to_string(),
                )
            })
            .map(sample_applied_openmls_workspace_commits_report)
    })
}

fn runtime_create_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<CreatedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .create_openmls_channel_group(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_create_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_add_openmls_channel_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    key_package_id: *const c_char,
) -> FfiResult<AddedOpenMlsChannelGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let key_package_id =
            ffi_device_key_package_id_arg(read_c_string(key_package_id, "key_package_id")?)?;
        runtime
            .add_openmls_channel_group_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceKeyPackageId(key_package_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_add_openmls_channel_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_remove_openmls_channel_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedOpenMlsChannelGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_openmls_channel_group_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_openmls_channel_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_join_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<JoinedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .join_openmls_channel_group(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                source_event_id,
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_join_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_update_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<UpdatedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .update_openmls_channel_group(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_apply_openmls_channel_group_commits_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<AppliedOpenMlsChannelGroupCommits> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .apply_openmls_channel_group_commits(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                source_event_id,
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_apply_openmls_channel_group_commits_failed",
                    error.to_string(),
                )
            })
            .map(sample_applied_openmls_channel_commits_report)
    })
}

fn runtime_send_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .send_message(WorkspaceId(workspace_id), ChannelId(channel_id), text)
            .map_err(|error| ffi_error("runtime_send_message_failed", error.to_string()))
    })
}

fn runtime_send_message_reply_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let reply_to_message_id =
            ffi_message_id_arg(read_c_string(reply_to_message_id, "reply_to_message_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .send_message_reply(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                MessageId(reply_to_message_id),
                text,
            )
            .map_err(|error| ffi_error("runtime_send_message_failed", error.to_string()))
    })
}

fn runtime_send_attachment_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let text = read_c_string(text, "text")?;
        let file_path = read_c_string(file_path, "file_path")?;
        let media_type = read_c_string(media_type, "media_type")?;
        runtime
            .send_message_with_attachment_file(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                text,
                PathBuf::from(file_path),
                media_type,
            )
            .map_err(|error| ffi_error("runtime_send_attachment_failed", error.to_string()))
    })
}

#[allow(clippy::too_many_arguments)]
fn runtime_send_attachment_reply_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let reply_to_message_id = ffi_optional_message_id_arg(optional_c_string(
            reply_to_message_id,
            "reply_to_message_id",
        )?)?;
        let text = read_c_string(text, "text")?;
        let file_path = read_c_string(file_path, "file_path")?;
        let media_type = read_c_string(media_type, "media_type")?;
        runtime
            .send_message_with_attachment_file_reply(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                reply_to_message_id,
                text,
                PathBuf::from(file_path),
                media_type,
            )
            .map_err(|error| ffi_error("runtime_send_attachment_failed", error.to_string()))
    })
}

fn runtime_save_attachment_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    blob_hash: *const c_char,
    output_path: *const c_char,
) -> FfiResult<SavedAttachment> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let attachment_selector = read_c_string(blob_hash, "blob_hash")?;
        let output_path = read_c_string(output_path, "output_path")?;
        runtime
            .save_attachment_to_file(
                WorkspaceId(workspace_id),
                MessageId(message_id),
                attachment_selector,
                PathBuf::from(output_path),
            )
            .map_err(|error| ffi_error("runtime_save_attachment_failed", error.to_string()))
    })
}

fn sample_pruned_blob_cache_report(mut report: PrunedBlobCache) -> PrunedBlobCache {
    report
        .workspace_ids
        .truncate(MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS);
    report
        .referenced_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_manifest_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .removed_temp_file_paths
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
}

fn runtime_prune_blobs_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<PrunedBlobCache> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .prune_unreferenced_blobs()
            .map(sample_pruned_blob_cache_report)
            .map_err(|error| ffi_error("runtime_prune_blobs_failed", error.to_string()))
    })
}

fn runtime_edit_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    text: *const c_char,
) -> FfiResult<EditedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .edit_message(WorkspaceId(workspace_id), MessageId(message_id), text)
            .map_err(|error| ffi_error("runtime_edit_message_failed", error.to_string()))
    })
}

fn runtime_delete_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
) -> FfiResult<DeletedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        runtime
            .delete_message(WorkspaceId(workspace_id), MessageId(message_id))
            .map_err(|error| ffi_error("runtime_delete_message_failed", error.to_string()))
    })
}

fn runtime_add_reaction_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> FfiResult<AddedReaction> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let reaction = read_c_string(reaction, "reaction")?;
        runtime
            .add_reaction(WorkspaceId(workspace_id), MessageId(message_id), reaction)
            .map_err(|error| ffi_error("runtime_add_reaction_failed", error.to_string()))
    })
}

fn runtime_remove_reaction_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> FfiResult<RemovedReaction> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let reaction = read_c_string(reaction, "reaction")?;
        runtime
            .remove_reaction(WorkspaceId(workspace_id), MessageId(message_id), reaction)
            .map_err(|error| ffi_error("runtime_remove_reaction_failed", error.to_string()))
    })
}

fn runtime_mark_channel_read_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<MarkedChannelRead> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .mark_channel_read(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_mark_channel_read_failed", error.to_string()))
    })
}

fn runtime_invite_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
    role: *const c_char,
) -> FfiResult<InvitedMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        let role = parse_workspace_role(&read_c_string(role, "role")?)?;
        runtime
            .invite_member(WorkspaceId(workspace_id), DeviceId(device_id), role)
            .map_err(|error| ffi_error("runtime_invite_member_failed", error.to_string()))
    })
}

fn runtime_remove_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| ffi_error("runtime_remove_member_failed", error.to_string()))
    })
}

fn runtime_remove_member_with_openmls_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMemberWithOpenMls> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member_with_openmls(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_member_with_openmls_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_remove_member_with_key_rotation_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMemberWithKeyRotation> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member_with_key_rotation(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_member_with_key_rotation_failed",
                    error.to_string(),
                )
            })
            .map(sample_removed_member_with_key_rotation_report)
    })
}

fn runtime_add_channel_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<AddedChannelMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .add_channel_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| ffi_error("runtime_add_channel_member_failed", error.to_string()))
    })
}

fn runtime_remove_channel_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| ffi_error("runtime_remove_channel_member_failed", error.to_string()))
    })
}

fn runtime_remove_channel_member_with_openmls_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMemberWithOpenMls> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member_with_openmls(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_channel_member_with_openmls_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_remove_channel_member_with_key_rotation_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMemberWithKeyRotation> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member_with_key_rotation(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_channel_member_with_key_rotation_failed",
                    error.to_string(),
                )
            })
    })
}

fn runtime_export_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceKeyExport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .export_workspace_key(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_export_workspace_key_failed", error.to_string()))
    })
}

fn runtime_rotate_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_key(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_rotate_workspace_key_failed", error.to_string()))
    })
}

fn runtime_rotate_workspace_manual_keys_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceManualKeys> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_manual_keys(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_rotate_workspace_manual_keys_failed",
                    error.to_string(),
                )
            })
            .map(sample_rotated_workspace_manual_keys_report)
    })
}

fn runtime_rotate_workspace_for_suspected_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceForSuspectedCompromise> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_for_suspected_compromise(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_rotate_workspace_for_suspected_compromise_failed",
                    error.to_string(),
                )
            })
            .map(sample_rotated_workspace_for_suspected_compromise_report)
    })
}

fn runtime_detect_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceCompromiseReport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .detect_workspace_compromise_signals(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_detect_compromise_failed", error.to_string()))
            .map(sample_workspace_compromise_report)
    })
}

fn runtime_respond_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceCompromiseResponse> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .respond_to_workspace_compromise_signals(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_respond_compromise_failed", error.to_string()))
            .map(sample_compromise_response_report_with_rotation_samples)
    })
}

fn runtime_export_trust_snapshot_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<SignedTrustSnapshot> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .export_trust_snapshot(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_export_trust_snapshot_failed", error.to_string()))
    })
}

fn runtime_import_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> FfiResult<ImportedWorkspaceKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let key_json = read_c_string_with_max_bytes(
            key_json,
            "key_json",
            KEY_TRANSFER_JSON_MAX_BYTES,
            "workspace_key_json_too_large",
            "workspace key JSON",
        )?;
        validate_json_payload_size(
            &key_json,
            KEY_TRANSFER_JSON_MAX_BYTES,
            "workspace_key_json_too_large",
            "workspace key JSON",
        )?;
        let key = serde_json::from_str::<WorkspaceKeyExport>(&key_json)
            .map_err(|error| ffi_error("invalid_workspace_key_json", error.to_string()))?;
        runtime
            .import_workspace_key(key)
            .map_err(|error| ffi_error("runtime_import_workspace_key_failed", error.to_string()))
    })
}

fn runtime_export_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<ChannelKeyExport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .export_channel_key(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_export_channel_key_failed", error.to_string()))
    })
}

fn runtime_rotate_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<RotatedChannelKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .rotate_channel_key(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_rotate_channel_key_failed", error.to_string()))
    })
}

fn runtime_import_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> FfiResult<ImportedChannelKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let key_json = read_c_string_with_max_bytes(
            key_json,
            "key_json",
            KEY_TRANSFER_JSON_MAX_BYTES,
            "channel_key_json_too_large",
            "channel key JSON",
        )?;
        validate_json_payload_size(
            &key_json,
            KEY_TRANSFER_JSON_MAX_BYTES,
            "channel_key_json_too_large",
            "channel key JSON",
        )?;
        let key = serde_json::from_str::<ChannelKeyExport>(&key_json)
            .map_err(|error| ffi_error("invalid_channel_key_json", error.to_string()))?;
        runtime
            .import_channel_key(key)
            .map_err(|error| ffi_error("runtime_import_channel_key_failed", error.to_string()))
    })
}

fn runtime_export_recovery_bundle_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    passphrase: *const c_char,
) -> FfiResult<WorkspaceRecoveryBundle> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let passphrase = read_c_string(passphrase, "passphrase")?;
        runtime
            .export_workspace_recovery_bundle(WorkspaceId(workspace_id), &passphrase)
            .map_err(|error| ffi_error("runtime_export_recovery_bundle_failed", error.to_string()))
    })
}

fn runtime_import_recovery_bundle_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    bundle_json: *const c_char,
    passphrase: *const c_char,
) -> FfiResult<ImportedWorkspaceRecoveryBundle> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let bundle_json = read_c_string(bundle_json, "bundle_json")?;
        validate_json_payload_size(
            &bundle_json,
            RECOVERY_BUNDLE_JSON_MAX_BYTES,
            "recovery_bundle_json_too_large",
            "recovery bundle JSON",
        )?;
        let passphrase = read_c_string(passphrase, "passphrase")?;
        let bundle = serde_json::from_str::<WorkspaceRecoveryBundle>(&bundle_json)
            .map_err(|error| ffi_error("invalid_recovery_bundle_json", error.to_string()))?;
        runtime
            .import_workspace_recovery_bundle(bundle, &passphrase)
            .map_err(|error| ffi_error("runtime_import_recovery_bundle_failed", error.to_string()))
            .map(sample_imported_workspace_recovery_bundle_report)
    })
}

fn runtime_reindex_workspace_search_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<IndexedWorkspaceSearch> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .reindex_workspace_search(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error("runtime_reindex_workspace_search_failed", error.to_string())
            })
    })
}

fn runtime_search_workspace_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
) -> FfiResult<SearchedWorkspace> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let query = read_c_string(query, "query")?;
        runtime
            .search_workspace_messages(WorkspaceId(workspace_id), query)
            .map_err(|error| ffi_error("runtime_search_workspace_failed", error.to_string()))
    })
}

fn runtime_workspace_publish_queue_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspacePublishQueue> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .workspace_publish_queue(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_publish_queue_failed", error.to_string()))
    })
}

fn sample_published_workspace_report(mut report: PublishedWorkspace) -> PublishedWorkspace {
    report
        .published_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .published_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report.skipped_gaps.truncate(MAX_RESULT_GAP_SAMPLE_ROWS);
    report
        .blob_transfer_attempts
        .truncate(MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS);
    sample_blob_transfer_attempt_reports(&mut report.blob_transfer_attempts);
    report
}

fn sample_blob_transfer_attempt_report(attempt: &mut BlobTransferAttempt) {
    attempt
        .chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    attempt
        .planned_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    attempt
        .remote_available_chunk_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
}

fn sample_blob_transfer_attempt_reports(attempts: &mut [BlobTransferAttempt]) {
    for attempt in attempts {
        sample_blob_transfer_attempt_report(attempt);
    }
}

fn truncate_string_bytes(value: &mut String, max_bytes: usize) {
    if value.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
}

fn sample_pulled_openmls_catchup_report(catchup: &mut PulledOpenMlsCatchup) {
    catchup
        .workspace_applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    catchup
        .workspace_provisioned_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    catchup
        .channel_groups
        .truncate(MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS);
    for group in &mut catchup.channel_groups {
        group
            .applied_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
        group
            .provisioned_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    }
}

fn sample_applied_openmls_workspace_commits_report(
    mut report: AppliedOpenMlsWorkspaceGroupCommits,
) -> AppliedOpenMlsWorkspaceGroupCommits {
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
}

fn sample_applied_openmls_channel_commits_report(
    mut report: AppliedOpenMlsChannelGroupCommits,
) -> AppliedOpenMlsChannelGroupCommits {
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
}

fn sample_updated_workspace_openmls_groups_report(
    mut report: UpdatedWorkspaceOpenMlsGroups,
) -> UpdatedWorkspaceOpenMlsGroups {
    sample_updated_workspace_openmls_groups_report_in_place(&mut report);
    report
}

fn sample_updated_workspace_openmls_groups_report_in_place(
    report: &mut UpdatedWorkspaceOpenMlsGroups,
) {
    report
        .channel_updates
        .truncate(MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS);
    report
        .updated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

fn sample_rotated_workspace_manual_keys_report(
    mut report: RotatedWorkspaceManualKeys,
) -> RotatedWorkspaceManualKeys {
    sample_rotated_workspace_manual_keys_report_in_place(&mut report);
    report
}

fn sample_rotated_workspace_manual_keys_report_in_place(report: &mut RotatedWorkspaceManualKeys) {
    report
        .channel_key_rotations
        .truncate(MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS);
    report
        .rotated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

fn sample_removed_member_with_key_rotation_report(
    mut report: RemovedMemberWithKeyRotation,
) -> RemovedMemberWithKeyRotation {
    report
        .channel_key_rotations
        .truncate(MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS);
    report
}

fn sample_rotated_workspace_for_suspected_compromise_report(
    mut report: RotatedWorkspaceForSuspectedCompromise,
) -> RotatedWorkspaceForSuspectedCompromise {
    sample_rotated_workspace_for_suspected_compromise_report_in_place(&mut report);
    report
}

fn sample_rotated_workspace_for_suspected_compromise_report_in_place(
    report: &mut RotatedWorkspaceForSuspectedCompromise,
) {
    report
        .rotated_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    if let Some(openmls_updates) = &mut report.openmls_updates {
        sample_updated_workspace_openmls_groups_report_in_place(openmls_updates);
    }
    if let Some(manual_key_rotation) = &mut report.manual_key_rotation {
        sample_rotated_workspace_manual_keys_report_in_place(manual_key_rotation);
    }
}

fn sample_workspace_compromise_report(
    mut report: WorkspaceCompromiseReport,
) -> WorkspaceCompromiseReport {
    sample_workspace_compromise_report_in_place(&mut report);
    report
}

fn sample_workspace_compromise_report_in_place(report: &mut WorkspaceCompromiseReport) {
    report
        .signals
        .truncate(MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS);
}

fn sample_compromise_response_lists(response: &mut WorkspaceCompromiseResponse) {
    sample_workspace_compromise_report_in_place(&mut response.report);
    response
        .responded_signal_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    response
        .already_handled_signal_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
}

fn sample_compromise_response_report(response: &mut WorkspaceCompromiseResponse) {
    sample_compromise_response_lists(response);
    if let Some(rotation) = &mut response.rotation {
        rotation
            .rotated_event_ids
            .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
        rotation.openmls_updates = None;
        rotation.manual_key_rotation = None;
    }
}

fn sample_compromise_response_report_with_rotation_samples(
    mut response: WorkspaceCompromiseResponse,
) -> WorkspaceCompromiseResponse {
    sample_compromise_response_lists(&mut response);
    if let Some(rotation) = &mut response.rotation {
        sample_rotated_workspace_for_suspected_compromise_report_in_place(rotation);
    }
    response
}

fn sample_pulled_workspace_report(mut report: PulledWorkspace) -> PulledWorkspace {
    report
        .requested_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .fetched_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .fetched_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .ignored_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report
        .applied_event_ids
        .truncate(MAX_RESULT_EVENT_ID_SAMPLE_ROWS);
    report.gaps.truncate(MAX_RESULT_GAP_SAMPLE_ROWS);
    sample_pulled_openmls_catchup_report(&mut report.openmls_catchup);
    if let Some(response) = &mut report.compromise_response {
        sample_compromise_response_report(response);
    }
    report
}

fn sample_synced_workspace_report(mut report: SyncedWorkspace) -> SyncedWorkspace {
    report.published = sample_published_workspace_report(report.published);
    report.pulled = sample_pulled_workspace_report(report.pulled);
    report
}

fn sample_blob_transfer_retry_report(
    mut report: BlobTransferRetryReport,
) -> BlobTransferRetryReport {
    report
        .pending_attempt_ids
        .truncate(MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS);
    report
        .retried_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .reconciled_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .missing_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .skipped_blob_hashes
        .truncate(MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
    report
        .peer_errors
        .truncate(MAX_RESULT_PEER_ERROR_SAMPLE_ROWS);
    for peer_error in &mut report.peer_errors {
        truncate_string_bytes(&mut peer_error.message, MAX_RESULT_PEER_ERROR_MESSAGE_BYTES);
    }
    report
        .blob_transfer_attempts
        .truncate(MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS);
    sample_blob_transfer_attempt_reports(&mut report.blob_transfer_attempts);
    report
}

fn sample_imported_workspace_recovery_bundle_report(
    mut report: ImportedWorkspaceRecoveryBundle,
) -> ImportedWorkspaceRecoveryBundle {
    report
        .imported_channel_ids
        .truncate(MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS);
    report
}

fn runtime_publish_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.publish_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_publish_workspace_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

fn runtime_backup_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.backup_workspace_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    WorkspaceId(workspace_id),
                ),
                "runtime_backup_workspace_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

fn runtime_publish_event_with_trust_snapshot_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    event_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let event_id = direct_event_id_arg(read_c_string(event_id, "event_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.publish_event_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    WorkspaceId(workspace_id),
                    EventId(event_id),
                ),
                "runtime_publish_event_with_trust_snapshot_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

fn runtime_pull_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PulledWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.pull_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_pull_workspace_failed",
            )
            .map(sample_pulled_workspace_report)
        })
    })
}

fn runtime_sync_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<SyncedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.sync_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_sync_workspace_failed",
            )
            .map(sample_synced_workspace_report)
        })
    })
}

fn runtime_retry_blob_transfers_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoints: *const c_char,
) -> FfiResult<BlobTransferRetryReport> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoints = read_c_string_with_max_bytes(
            peer_endpoints,
            "peer_endpoints",
            PEER_ENDPOINT_LIST_TEXT_MAX_BYTES,
            "peer_endpoint_list_too_large",
            "peer endpoint list",
        )?;
        let peers = direct_peer_addresses(&peer_endpoints)?;

        run_on_worker_thread(move || {
            let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
            let transport = IrohTransport::from_environment();
            run_runtime_future(
                runtime.retry_pending_blob_transfers_direct(
                    &transport,
                    WorkspaceId(workspace_id),
                    &peers,
                ),
                "runtime_retry_blob_transfers_failed",
            )
            .map(sample_blob_transfer_retry_report)
        })
    })
}

fn runtime_start_direct_peer_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    listen: *const c_char,
) -> FfiResult<HostedPeer> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let listen = optional_c_string_with_max_bytes(
            listen,
            "listen",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?
        .map(|listen| listen.trim().to_owned())
        .filter(|listen| !listen.is_empty())
        .unwrap_or_else(|| "127.0.0.1:0".to_owned());
        validate_peer_endpoint_text(&listen, "listen")?;
        validate_direct_listen_endpoint_text(&listen)?;
        let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
        let paths = runtime.paths().clone();
        let peer_id = format!(
            "direct-peer-{}",
            HOSTED_PEER_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        let thread = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = ready_tx.send(Err("tokio runtime creation failed".to_owned()));
                return;
            };

            runtime.block_on(async move {
                let store = match EventStore::open(&paths.event_store) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let blob_store = match BlobStore::open(&paths.blob_store) {
                    Ok(blob_store) => blob_store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let server =
                    match DirectPeerServer::bind_with_blobs(&listen, store, blob_store).await {
                        Ok(server) => server,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return;
                        }
                    };
                let endpoint = match server.local_addr() {
                    Ok(endpoint) => endpoint.to_string(),
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(endpoint));
                let _ = server.serve_until_shutdown(shutdown_rx).await;
            });
        });

        let endpoint = match ready_rx
            .recv()
            .map_err(|_| ffi_error("runtime_direct_peer_start_failed", "peer thread exited"))?
        {
            Ok(endpoint) => endpoint,
            Err(message) => {
                let _ = thread.join();
                return Err(ffi_error("runtime_direct_peer_start_failed", message));
            }
        };

        hosted_peer_registry()
            .lock()
            .map_err(|_| ffi_error("runtime_direct_peer_registry_failed", "registry poisoned"))?
            .insert(
                peer_id.clone(),
                RunningPeer {
                    endpoint: endpoint.clone(),
                    shutdown: shutdown_tx,
                    thread,
                },
            );

        Ok(HostedPeer { peer_id, endpoint })
    })
}

fn runtime_start_iroh_peer_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<HostedPeer> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let paths = runtime.paths().clone();
        let peer_id = format!(
            "iroh-peer-{}",
            HOSTED_PEER_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        let thread = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = ready_tx.send(Err("tokio runtime creation failed".to_owned()));
                return;
            };

            runtime.block_on(async move {
                let store = match EventStore::open(&paths.event_store) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let blob_store = match BlobStore::open(&paths.blob_store) {
                    Ok(blob_store) => blob_store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let sync_store = SyncPeerStore::with_blobs(store, blob_store);
                let server =
                    match IrohSyncPeer::bind(sync_store, IrohTransportConfig::from_environment())
                        .await
                    {
                        Ok(server) => server,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return;
                        }
                    };
                let endpoint = server.endpoint_url();
                let _ = ready_tx.send(Ok(endpoint));
                let _ = shutdown_rx.await;
                let _ = server.close().await;
            });
        });

        let endpoint = match ready_rx
            .recv()
            .map_err(|_| ffi_error("runtime_iroh_peer_start_failed", "peer thread exited"))?
        {
            Ok(endpoint) => endpoint,
            Err(message) => {
                let _ = thread.join();
                return Err(ffi_error("runtime_iroh_peer_start_failed", message));
            }
        };

        hosted_peer_registry()
            .lock()
            .map_err(|_| ffi_error("runtime_iroh_peer_registry_failed", "registry poisoned"))?
            .insert(
                peer_id.clone(),
                RunningPeer {
                    endpoint: endpoint.clone(),
                    shutdown: shutdown_tx,
                    thread,
                },
            );

        Ok(HostedPeer { peer_id, endpoint })
    })
}

fn runtime_stop_direct_peer_result(peer_id: *const c_char) -> FfiResult<StoppedPeer> {
    result_envelope(|| {
        let peer_id = read_c_string(peer_id, "peer_id")?;
        let running = hosted_peer_registry()
            .lock()
            .map_err(|_| ffi_error("runtime_direct_peer_registry_failed", "registry poisoned"))?
            .remove(&peer_id)
            .ok_or_else(|| ffi_error("runtime_direct_peer_not_found", "peer is not running"))?;
        let endpoint = running.endpoint;
        let _ = running.shutdown.send(());
        running
            .thread
            .join()
            .map_err(|_| ffi_error("runtime_direct_peer_stop_failed", "peer thread panicked"))?;

        Ok(StoppedPeer { peer_id, endpoint })
    })
}

fn open_runtime_from_ffi(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> Result<LocalRuntime, FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
    open_runtime_from_paths(&data_dir, identity_file)
}

fn open_runtime_from_paths(
    data_dir: impl AsRef<std::path::Path>,
    identity_file: Option<PathBuf>,
) -> Result<LocalRuntime, FfiError> {
    let data_dir = data_dir.as_ref();
    let identity_passphrase =
        runtime_identity_passphrase_for_path(data_dir).or_else(identity_passphrase_from_env);
    LocalRuntime::open_with_identity_passphrase(
        data_dir,
        identity_file,
        identity_passphrase
            .as_ref()
            .map(|passphrase| passphrase.as_str()),
    )
    .map_err(|error| ffi_error("runtime_open_failed", error.to_string()))
}

fn identity_passphrase_from_env() -> Option<Zeroizing<String>> {
    std::env::var("CHAFT_IDENTITY_PASSPHRASE")
        .ok()
        .filter(|passphrase| env_identity_passphrase_is_usable(passphrase))
        .map(Zeroizing::new)
}

fn env_identity_passphrase_is_usable(passphrase: &str) -> bool {
    !passphrase.trim().is_empty() && passphrase.len() <= FFI_PASSPHRASE_MAX_BYTES
}

fn set_runtime_identity_passphrase_result(
    data_dir: *const c_char,
    passphrase: *const c_char,
) -> Result<(), FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let passphrase = read_c_string(passphrase, "passphrase")?;
    if passphrase.trim().is_empty() {
        return Err(ffi_error(
            "runtime_passphrase_required",
            "passphrase is required",
        ));
    }

    runtime_identity_passphrase_registry()
        .lock()
        .map_err(|_| ffi_error("runtime_passphrase_registry_failed", "registry poisoned"))?
        .insert(
            runtime_passphrase_key(std::path::Path::new(&data_dir)),
            Zeroizing::new(passphrase),
        );
    Ok(())
}

fn clear_runtime_identity_passphrase_result(data_dir: *const c_char) -> Result<(), FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    runtime_identity_passphrase_registry()
        .lock()
        .map_err(|_| ffi_error("runtime_passphrase_registry_failed", "registry poisoned"))?
        .remove(&runtime_passphrase_key(std::path::Path::new(&data_dir)));
    Ok(())
}

fn runtime_identity_passphrase_for_path(data_dir: &std::path::Path) -> Option<Zeroizing<String>> {
    runtime_identity_passphrase_registry()
        .lock()
        .ok()
        .and_then(|passphrases| passphrases.get(&runtime_passphrase_key(data_dir)).cloned())
}

fn runtime_identity_passphrase_registry() -> &'static Mutex<HashMap<String, Zeroizing<String>>> {
    RUNTIME_IDENTITY_PASSPHRASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime_passphrase_key(data_dir: &std::path::Path) -> String {
    data_dir.to_string_lossy().into_owned()
}

fn validate_peer_endpoint_text(endpoint: &str, field_name: &'static str) -> Result<(), FfiError> {
    if endpoint.trim().is_empty() {
        return Err(ffi_error(field_name, "peer endpoint is required"));
    }
    if endpoint.len() > PEER_ENDPOINT_MAX_BYTES {
        return Err(ffi_error(
            "peer_endpoint_too_large",
            format!(
                "peer endpoint is too large ({} bytes, max {})",
                endpoint.len(),
                PEER_ENDPOINT_MAX_BYTES
            ),
        ));
    }
    Ok(())
}

fn validate_direct_listen_endpoint_text(endpoint: &str) -> Result<(), FfiError> {
    if direct_tcp_peer_listen_address_is_valid(endpoint) {
        return Ok(());
    }
    Err(ffi_error(
        "peer_endpoint_unsupported",
        "direct listen endpoint must be host:port with numeric port",
    ))
}

fn direct_workspace_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "workspace_id_required",
            "workspace ID is required",
        ));
    }
    validate_workspace_id_str(&value)
        .map_err(|error| ffi_error("workspace_id_too_large", error.to_string()))?;
    Ok(value)
}

fn direct_event_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("event_id_required", "event ID is required"));
    }
    validate_event_id_str(&value)
        .map_err(|error| ffi_error("event_id_too_large", error.to_string()))?;
    if !is_canonical_event_id_str(&value) {
        return Err(ffi_error(
            "event_id_not_canonical",
            "event ID must be canonical",
        ));
    }
    Ok(value)
}

fn ffi_workspace_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "workspace_id_required",
            "workspace ID is required",
        ));
    }
    validate_workspace_id_str(&value)
        .map_err(|error| ffi_error("workspace_id_too_large", error.to_string()))?;
    Ok(value)
}

fn ffi_channel_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("channel_id_required", "channel ID is required"));
    }
    validate_channel_id_str(&value)
        .map_err(|error| ffi_error("channel_id_too_large", error.to_string()))?;
    Ok(value)
}

fn ffi_message_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("message_id_required", "message ID is required"));
    }
    validate_message_id_str(&value)
        .map_err(|error| ffi_error("message_id_too_large", error.to_string()))?;
    Ok(value)
}

fn ffi_optional_message_id_arg(value: Option<String>) -> Result<Option<MessageId>, FfiError> {
    value
        .map(|value| {
            let value = value.trim().to_owned();
            if value.is_empty() {
                Ok(None)
            } else {
                ffi_message_id_arg(value).map(MessageId).map(Some)
            }
        })
        .transpose()
        .map(Option::flatten)
}

fn ffi_event_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("event_id_required", "event ID is required"));
    }
    validate_event_id_str(&value)
        .map_err(|error| ffi_error("event_id_too_large", error.to_string()))?;
    if !is_canonical_event_id_str(&value) {
        return Err(ffi_error(
            "event_id_not_canonical",
            "event ID must be canonical",
        ));
    }
    Ok(value)
}

fn ffi_optional_event_id_arg(value: Option<String>) -> Result<Option<EventId>, FfiError> {
    value
        .map(|value| ffi_event_id_arg(value).map(EventId))
        .transpose()
}

fn ffi_device_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("device_id_required", "device ID is required"));
    }
    validate_device_id_str(&value)
        .map_err(|error| ffi_error("device_id_too_large", error.to_string()))?;
    Ok(value)
}

fn ffi_device_key_package_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "key_package_id_required",
            "device key package ID is required",
        ));
    }
    validate_device_key_package_id_str(&value)
        .map_err(|error| ffi_error("key_package_id_too_large", error.to_string()))?;
    Ok(value)
}

fn direct_peer_address(endpoint: String) -> Result<PeerAddress, FfiError> {
    let endpoint = endpoint.trim().to_owned();
    validate_peer_endpoint_text(&endpoint, "peer_endpoint")?;
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(ffi_error(
            "peer_endpoint_unsupported",
            "peer endpoint must be a direct TCP or native Iroh direct route",
        ));
    }
    Ok(PeerAddress {
        peer_id: PeerId(endpoint.clone()),
        endpoint,
    })
}

fn split_peer_endpoints(endpoints: &str) -> Vec<String> {
    endpoints
        .split([',', ';'])
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .map(str::to_owned)
        .collect()
}

fn direct_peer_addresses(endpoints: &str) -> Result<Vec<PeerAddress>, FfiError> {
    let endpoints = deduplicate_peer_endpoints(split_peer_endpoints(endpoints));
    if endpoints.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        return Err(ffi_error(
            "peer_endpoint_list_too_large",
            format!(
                "peer endpoint list is too large ({} endpoints, max {})",
                endpoints.len(),
                PEER_ENDPOINT_LIST_MAX_ITEMS
            ),
        ));
    }
    endpoints.into_iter().map(direct_peer_address).collect()
}

fn deduplicate_peer_endpoints(endpoints: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    endpoints
        .into_iter()
        .filter(|endpoint| seen.insert(endpoint.clone()))
        .collect()
}

fn validate_peer_endpoint_hint_inputs(
    endpoint_id: String,
    endpoint: String,
    transport: String,
) -> Result<(String, String, String), FfiError> {
    let endpoint_id = endpoint_id.trim().to_owned();
    if endpoint_id.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_id_required",
            "peer endpoint ID is required",
        ));
    }

    let endpoint = endpoint.trim().to_owned();
    if endpoint.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_required",
            "peer endpoint is required",
        ));
    }
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(ffi_error(
            "peer_endpoint_unsupported",
            "peer endpoint must be a direct TCP or native Iroh direct route",
        ));
    }

    let transport = transport.trim().to_owned();
    if transport.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_transport_required",
            "peer endpoint transport is required",
        ));
    }
    if !peer_endpoint_hint_transport_is_consistent(&endpoint, &transport) {
        return Err(ffi_error(
            "peer_endpoint_transport_mismatch",
            "peer endpoint transport does not match the endpoint route",
        ));
    }

    Ok((endpoint_id, endpoint, transport))
}

fn hosted_peer_registry() -> &'static Mutex<HashMap<String, RunningPeer>> {
    HOSTED_PEERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn run_on_worker_thread<T, F>(operation: F) -> Result<T, FfiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, FfiError> + Send + 'static,
{
    thread::spawn(operation)
        .join()
        .map_err(|_| ffi_error("runtime_network_worker_panicked", "network worker panicked"))?
}

fn run_runtime_future<T, F>(future: F, failure_code: &'static str) -> Result<T, FfiError>
where
    F: Future<Output = Result<T, RuntimeError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| ffi_error("tokio_runtime_failed", error.to_string()))?;
    runtime.block_on(future).map_err(|error| {
        let message = runtime_error_message(&error);
        ffi_error(runtime_error_code(&error, failure_code), message)
    })
}

fn runtime_error_code(error: &RuntimeError, fallback_code: &'static str) -> &'static str {
    if error.is_peer_protocol_error() {
        "runtime_peer_protocol_failed"
    } else {
        fallback_code
    }
}

fn runtime_error_message(error: &RuntimeError) -> String {
    error
        .peer_protocol_error_message()
        .unwrap_or_else(|| error.to_string())
}

fn bounded_timeline_limit(limit: usize) -> usize {
    limit.min(MAX_TIMELINE_WINDOW_ROWS)
}

fn result_envelope<T, F>(build: F) -> FfiResult<T>
where
    T: Serialize,
    F: FnOnce() -> Result<T, FfiError>,
{
    match build() {
        Ok(value) => FfiResult {
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => FfiResult {
            ok: false,
            value: None,
            error: Some(error),
        },
    }
}

fn read_c_string(value: *const c_char, field_name: &'static str) -> Result<String, FfiError> {
    if let Some((max_bytes, error_code, label)) = bounded_c_string_field(field_name) {
        return read_c_string_with_max_bytes(value, field_name, max_bytes, error_code, label);
    }

    read_c_string_with_max_bytes(
        value,
        field_name,
        FFI_GENERIC_STRING_MAX_BYTES,
        "ffi_string_too_large",
        "FFI string",
    )
}

fn read_c_string_with_max_bytes(
    value: *const c_char,
    field_name: &'static str,
    max_bytes: usize,
    error_code: &'static str,
    label: &'static str,
) -> Result<String, FfiError> {
    if value.is_null() {
        return Err(ffi_error(field_name, "null pointer"));
    }

    let mut actual_bytes = 0;
    while actual_bytes <= max_bytes {
        // SAFETY: Callers promise a valid C string pointer. Bounded fields scan at most
        // max_bytes + 1 bytes so an oversized input is rejected before an unbounded walk.
        let byte = unsafe { *value.add(actual_bytes) };
        if byte == 0 {
            // SAFETY: We only build a slice over the non-NUL prefix already scanned above.
            let bytes = unsafe { slice::from_raw_parts(value.cast::<u8>(), actual_bytes) };
            let value = std::str::from_utf8(bytes)
                .map_err(|error| ffi_error(field_name, error.to_string()))?;
            return Ok(value.to_owned());
        }
        actual_bytes += 1;
    }

    let actual_bytes = max_bytes.saturating_add(1);
    Err(ffi_error(
        error_code,
        format!("{label} is too large ({actual_bytes} bytes, max {max_bytes})"),
    ))
}

fn optional_c_string(
    value: *const c_char,
    field_name: &'static str,
) -> Result<Option<String>, FfiError> {
    if value.is_null() {
        return Ok(None);
    }
    read_c_string(value, field_name).map(Some)
}

fn bounded_c_string_field(field_name: &'static str) -> Option<(usize, &'static str, &'static str)> {
    match field_name {
        "workspace_id" => Some((
            WORKSPACE_ID_MAX_BYTES,
            "workspace_id_too_large",
            "workspace ID",
        )),
        "data_dir" => Some((FFI_PATH_MAX_BYTES, "data_dir_too_large", "data directory")),
        "identity_file" => Some((
            FFI_PATH_MAX_BYTES,
            "identity_file_too_large",
            "identity file path",
        )),
        "store_path" => Some((FFI_PATH_MAX_BYTES, "store_path_too_large", "store path")),
        "file_path" => Some((FFI_PATH_MAX_BYTES, "file_path_too_large", "file path")),
        "output_path" => Some((FFI_PATH_MAX_BYTES, "output_path_too_large", "output path")),
        "key_package_file" => Some((
            FFI_PATH_MAX_BYTES,
            "key_package_file_too_large",
            "key package file path",
        )),
        "passphrase" => Some((
            FFI_PASSPHRASE_MAX_BYTES,
            "passphrase_too_large",
            "passphrase",
        )),
        "role" => Some((
            WORKSPACE_ROLE_TEXT_MAX_BYTES,
            "workspace_role_too_large",
            "workspace role",
        )),
        "channel_id" => Some((CHANNEL_ID_MAX_BYTES, "channel_id_too_large", "channel ID")),
        "message_id" | "reply_to_message_id" => {
            Some((MESSAGE_ID_MAX_BYTES, "message_id_too_large", "message ID"))
        }
        "event_id" | "source_event_id" => {
            Some((EVENT_ID_MAX_BYTES, "event_id_too_large", "event ID"))
        }
        "device_id" => Some((DEVICE_ID_MAX_BYTES, "device_id_too_large", "device ID")),
        "key_package_id" => Some((
            DEVICE_KEY_PACKAGE_ID_MAX_BYTES,
            "key_package_id_too_large",
            "key package ID",
        )),
        "name" => Some((WORKSPACE_NAME_MAX_BYTES, "name_too_large", "name")),
        "default_channel_name" => Some((
            CHANNEL_NAME_MAX_BYTES,
            "channel_name_too_large",
            "channel name",
        )),
        "display_name" => Some((
            DEVICE_DISPLAY_NAME_MAX_BYTES,
            "display_name_too_large",
            "display name",
        )),
        "protocol" => Some((
            DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
            "key_package_protocol_too_large",
            "key package protocol",
        )),
        "text" => Some((
            MESSAGE_MARKDOWN_MAX_BYTES,
            "message_markdown_too_large",
            "message markdown",
        )),
        "reaction" => Some((REACTION_TEXT_MAX_BYTES, "reaction_too_large", "reaction")),
        "query" => Some((
            SEARCH_QUERY_MAX_BYTES,
            "search_query_too_large",
            "search query",
        )),
        "media_type" => Some((
            ATTACHMENT_MEDIA_TYPE_MAX_BYTES,
            "attachment_media_type_too_large",
            "attachment media type",
        )),
        "blob_hash" => Some((
            ATTACHMENT_ID_MAX_BYTES,
            "attachment_selector_too_large",
            "attachment selector",
        )),
        "endpoint" | "peer_endpoint" => Some((
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )),
        "endpoint_id" => Some((
            PEER_ENDPOINT_ID_MAX_BYTES,
            "peer_endpoint_id_too_large",
            "peer endpoint ID",
        )),
        "transport" => Some((
            PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
            "peer_endpoint_transport_too_large",
            "peer endpoint transport",
        )),
        "peer_id" => Some((PEER_ENDPOINT_ID_MAX_BYTES, "peer_id_too_large", "peer ID")),
        "events_json" => Some((
            WORKSPACE_EVENTS_JSON_MAX_BYTES,
            "events_json_too_large",
            "events JSON",
        )),
        "bundle_json" => Some((
            RECOVERY_BUNDLE_JSON_MAX_BYTES,
            "recovery_bundle_json_too_large",
            "recovery bundle JSON",
        )),
        _ => None,
    }
}

fn optional_c_string_with_max_bytes(
    value: *const c_char,
    field_name: &'static str,
    max_bytes: usize,
    error_code: &'static str,
    label: &'static str,
) -> Result<Option<String>, FfiError> {
    if value.is_null() {
        return Ok(None);
    }
    read_c_string_with_max_bytes(value, field_name, max_bytes, error_code, label).map(Some)
}

fn read_device_key_package_file(file_path: &Path) -> Result<Vec<u8>, FfiError> {
    let metadata = std::fs::metadata(file_path)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    if metadata.len() > DEVICE_KEY_PACKAGE_FILE_MAX_BYTES {
        return Err(ffi_error(
            "runtime_publish_device_key_package_failed",
            format!(
                "device key package is too large ({} bytes, max {})",
                metadata.len(),
                DEVICE_KEY_PACKAGE_FILE_MAX_BYTES
            ),
        ));
    }

    let file = std::fs::File::open(file_path)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    let mut limited_file = file.take(DEVICE_KEY_PACKAGE_FILE_MAX_BYTES + 1);
    let mut bytes =
        Vec::with_capacity(metadata.len().min(DEVICE_KEY_PACKAGE_FILE_MAX_BYTES) as usize);
    limited_file
        .read_to_end(&mut bytes)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    if bytes.len() as u64 > DEVICE_KEY_PACKAGE_FILE_MAX_BYTES {
        return Err(ffi_error(
            "runtime_publish_device_key_package_failed",
            format!(
                "device key package is too large ({} bytes, max {})",
                bytes.len(),
                DEVICE_KEY_PACKAGE_FILE_MAX_BYTES
            ),
        ));
    }
    Ok(bytes)
}

fn validate_json_payload_size(
    value: &str,
    max_bytes: usize,
    error_code: &'static str,
    label: &str,
) -> Result<(), FfiError> {
    let actual_bytes = value.len();
    if actual_bytes > max_bytes {
        return Err(ffi_error(
            error_code,
            format!("{label} is too large ({actual_bytes} bytes, max {max_bytes})"),
        ));
    }
    Ok(())
}

fn parse_workspace_role(input: &str) -> Result<WorkspaceRole, FfiError> {
    let quoted = format!("\"{}\"", input);
    serde_json::from_str::<WorkspaceRole>(&quoted).map_err(|_| {
        ffi_error(
            "invalid_workspace_role",
            "expected owner, admin, member, or guest",
        )
    })
}

fn ffi_error(code: &'static str, message: impl Into<String>) -> FfiError {
    FfiError {
        code,
        message: message.into(),
    }
}

fn into_c_string<T>(value: &T) -> *mut c_char
where
    T: Serialize,
{
    match serde_json::to_string(value)
        .ok()
        .and_then(|json| CString::new(json).ok())
    {
        Some(value) => value.into_raw(),
        None => ptr::null_mut(),
    }
}

fn demo_workspace_snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        workspace_id: "wrk_demo".to_owned(),
        name: "Chaft Labs".to_owned(),
        channels: vec![
            ChannelSnapshot {
                channel_id: "chn_general".to_owned(),
                name: "general".to_owned(),
                is_private: false,
                unread_count: 0,
                latest_activity: None,
            },
            ChannelSnapshot {
                channel_id: "chn_runtime".to_owned(),
                name: "p2p-runtime".to_owned(),
                is_private: false,
                unread_count: 2,
                latest_activity: None,
            },
            ChannelSnapshot {
                channel_id: "chn_design".to_owned(),
                name: "design-system".to_owned(),
                is_private: false,
                unread_count: 0,
                latest_activity: None,
            },
            ChannelSnapshot {
                channel_id: "chn_replicas".to_owned(),
                name: "replica-nodes".to_owned(),
                is_private: true,
                unread_count: 1,
                latest_activity: None,
            },
        ],
        profiles: vec![DeviceProfileSnapshot {
            device_id: "dev_mira".to_owned(),
            display_name: "Mira".to_owned(),
            updated_event_id: "evt_profile_mira".to_owned(),
        }],
        members: vec![WorkspaceMemberSnapshot {
            device_id: "dev_mira".to_owned(),
            role: WorkspaceRole::Owner,
            display_name: Some("Mira".to_owned()),
            profile_event_id: Some("evt_profile_mira".to_owned()),
            membership_event_id: "evt_workspace".to_owned(),
        }],
        key_packages: vec![DeviceKeyPackageSnapshot {
            device_id: "dev_mira".to_owned(),
            key_package_id: "dkp_mira_demo".to_owned(),
            protocol: "openmls/key-package".to_owned(),
            byte_len: 512,
            published_event_id: "evt_key_package_mira".to_owned(),
            physical_ms: 1_700_000_000_010,
        }],
        peer_endpoints: Vec::new(),
        channel_count: 4,
        profile_count: 1,
        member_count: 1,
        key_package_count: 1,
        peer_endpoint_count: 0,
        timeline_channel_id: None,
        timeline_window: chaft_app::TimelineWindowSnapshot {
            start_index: 0,
            item_count: 2,
            total_count: 2,
            has_more_before: false,
            has_more_after: false,
        },
        timeline: vec![
            TimelineItem {
                kind: TimelineItemKind::EncryptedMessage,
                event_id: "evt_ciphertext".to_owned(),
                message_id: Some("msg_ciphertext".to_owned()),
                reply_to_message_id: None,
                reply_preview: None,
                thread_reply_count: 0,
                thread_latest_reply: None,
                thread_reply_previews: Vec::new(),
                channel_id: Some("chn_general".to_owned()),
                author_device_id: Some("dev_mira".to_owned()),
                author_display_name: Some("Mira".to_owned()),
                physical_ms: Some(1_700_000_000_000),
                body: "Encrypted message".to_owned(),
                attachment_count: 0,
                attachments: Vec::new(),
                reaction_count: 0,
                reactions: Default::default(),
                my_reactions: Vec::new(),
                encrypted: true,
                deleted: false,
                missing_parent_ids: Vec::new(),
            },
            TimelineItem {
                kind: TimelineItemKind::MissingHistoryGap,
                event_id: "evt_later_slice".to_owned(),
                message_id: None,
                reply_to_message_id: None,
                reply_preview: None,
                thread_reply_count: 0,
                thread_latest_reply: None,
                thread_reply_previews: Vec::new(),
                channel_id: None,
                author_device_id: None,
                author_display_name: None,
                physical_ms: None,
                body: "Missing 2 parent event(s)".to_owned(),
                attachment_count: 0,
                attachments: Vec::new(),
                reaction_count: 0,
                reactions: Default::default(),
                my_reactions: Vec::new(),
                encrypted: false,
                deleted: false,
                missing_parent_ids: vec!["evt_parent_a".to_owned(), "evt_parent_b".to_owned()],
            },
        ],
        gap_count: 0,
        gaps: Vec::new(),
        invalid_signature_count: 0,
        invalid_signatures: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::CStr,
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        time::Duration,
    };

    use chaft_net_direct::DirectPeerServer;
    use chaft_runtime::{
        BlobTransferAttempt, BlobTransferMode, BlobTransferPeerError, BlobTransferStatus,
        PulledOpenMlsChannelCatchup, PulledWorkspaceGap, RotatedWorkspaceForSuspectedCompromise,
        WorkspaceCompromiseSignal,
    };
    use chaft_store::EventStore;
    use chaft_types::{
        ChannelId, DeviceId, EventBody, MessageId, PayloadEncryption, SealedPayload, SignableEvent,
    };
    use serde_json::Value;
    use tokio::sync::oneshot;

    use super::*;

    fn signed(event: SignableEvent) -> SignedEvent {
        SignedEvent::from_signed_bytes(event, vec![1, 2, 3])
    }

    fn sealed_payload() -> SealedPayload {
        SealedPayload {
            mode: PayloadEncryption::Aes256GcmSiv,
            key_id: "workspace-key-1".to_owned(),
            nonce: vec![1; 12],
            aad: b"message aad".to_vec(),
            bytes: b"ciphertext".to_vec(),
        }
    }

    fn sample_strings(prefix: &str, count: usize) -> Vec<String> {
        (0..count)
            .map(|index| format!("{prefix}_{index:03}"))
            .collect()
    }

    fn sample_blob_transfer_attempt(index: usize) -> BlobTransferAttempt {
        let chunk_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 3;
        let planned_chunk_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 5;
        let remote_available_chunk_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 7;
        BlobTransferAttempt {
            attempt_id: format!("attempt_{index:03}"),
            workspace_id: "wrk_sample".to_owned(),
            peer_id: "peer_sample".to_owned(),
            peer_endpoint: "direct+tcp://127.0.0.1:1".to_owned(),
            blob_hash: format!("blob_{index:03}"),
            mode: BlobTransferMode::ChunkedBlob,
            status: BlobTransferStatus::Succeeded,
            attempt_count: 1,
            total_byte_len: 128,
            chunk_size: Some(32),
            chunk_count,
            chunk_hashes: sample_strings(&format!("chunk_{index:03}"), chunk_count),
            planned_chunk_count,
            planned_chunk_hashes: sample_strings(
                &format!("chunk_planned_{index:03}"),
                planned_chunk_count,
            ),
            remote_available_chunk_count,
            remote_available_chunk_hashes: sample_strings(
                &format!("chunk_remote_{index:03}"),
                remote_available_chunk_count,
            ),
            started_at_unix_ms: 1_700_000_000_000 + index as u64,
            finished_at_unix_ms: Some(1_700_000_000_010 + index as u64),
            error: None,
        }
    }

    fn assert_sampled_blob_transfer_attempt_chunks(attempt: &BlobTransferAttempt) {
        assert_eq!(attempt.chunk_count, MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 3);
        assert_eq!(attempt.chunk_hashes.len(), MAX_RESULT_BLOB_HASH_SAMPLE_ROWS);
        assert_eq!(
            attempt.planned_chunk_count,
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 5
        );
        assert_eq!(
            attempt.planned_chunk_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(
            attempt.remote_available_chunk_count,
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 7
        );
        assert_eq!(
            attempt.remote_available_chunk_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
    }

    fn sample_workspace_gap(index: usize) -> PulledWorkspaceGap {
        PulledWorkspaceGap {
            event_id: format!("evt_gap_{index:03}"),
            missing_parent_ids: vec![format!("evt_missing_parent_{index:03}")],
        }
    }

    fn insert_corrupt_event_json(data_dir: &std::path::Path, workspace_id: &str, event_id: &str) {
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
                    workspace_id,
                    "dev_corrupt",
                    1_i64,
                    0_i64,
                    1_i64,
                    b"not valid signed event json".as_slice()
                ],
            )
            .unwrap();
    }

    fn sample_blob_transfer_peer_error(index: usize) -> BlobTransferPeerError {
        BlobTransferPeerError {
            peer_id: format!("peer_{index:03}"),
            peer_endpoint: format!("direct+tcp://127.0.0.1:{}", 10_000 + index),
            blob_hash: format!("blob_error_{index:03}"),
            message: "é".repeat(MAX_RESULT_PEER_ERROR_MESSAGE_BYTES),
            suspect_protocol_error: index.is_multiple_of(2),
        }
    }

    fn sample_compromise_signal(index: usize) -> WorkspaceCompromiseSignal {
        WorkspaceCompromiseSignal {
            kind: "invalidSelfContainedSignature".to_owned(),
            severity: "critical".to_owned(),
            event_id: format!("evt_signal_{index:03}"),
            channel_id: Some("chn_general".to_owned()),
            author_device_id: "dev_sample".to_owned(),
            local_device: true,
            physical_ms: 1_700_000_000_000 + index as i64,
            reason: "sample".to_owned(),
        }
    }

    fn sample_workspace_key_rotation(index: usize) -> RotatedWorkspaceKey {
        RotatedWorkspaceKey {
            workspace_id: "wrk_sample".to_owned(),
            previous_key_id: format!("wrk_key_prev_{index:03}"),
            key_id: format!("wrk_key_{index:03}"),
            epoch: index as u64 + 1,
            event_id: format!("evt_workspace_key_{index:03}"),
        }
    }

    fn sample_channel_key_rotation(index: usize) -> RotatedChannelKey {
        RotatedChannelKey {
            workspace_id: "wrk_sample".to_owned(),
            channel_id: format!("chn_{index:03}"),
            previous_key_id: format!("chn_key_prev_{index:03}"),
            key_id: format!("chn_key_{index:03}"),
            epoch: index as u64 + 1,
            event_id: format!("evt_channel_key_{index:03}"),
        }
    }

    fn sample_openmls_workspace_update(index: usize) -> UpdatedOpenMlsWorkspaceGroup {
        UpdatedOpenMlsWorkspaceGroup {
            workspace_id: "wrk_sample".to_owned(),
            device_id: "dev_sample".to_owned(),
            protocol: "openmls".to_owned(),
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
            group_id: format!("mls_workspace_{index:03}"),
            epoch: index as u64 + 1,
            member_count: 2,
            commit_byte_len: 128,
            ratchet_tree_byte_len: 256,
            private_group_state_path: format!("/tmp/workspace_group_{index:03}.bin"),
            event_id: format!("evt_openmls_workspace_{index:03}"),
        }
    }

    fn sample_openmls_channel_update(index: usize) -> UpdatedOpenMlsChannelGroup {
        UpdatedOpenMlsChannelGroup {
            workspace_id: "wrk_sample".to_owned(),
            channel_id: format!("chn_{index:03}"),
            device_id: "dev_sample".to_owned(),
            protocol: "openmls".to_owned(),
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
            group_id: format!("mls_channel_{index:03}"),
            epoch: index as u64 + 1,
            member_count: 2,
            commit_byte_len: 128,
            ratchet_tree_byte_len: 256,
            private_group_state_path: format!("/tmp/channel_group_{index:03}.bin"),
            event_id: format!("evt_openmls_channel_{index:03}"),
        }
    }

    fn sample_events() -> (WorkspaceId, Vec<SignedEvent>) {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId("chn_general".to_owned());
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft FFI".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));

        (workspace_id, vec![workspace, channel, message])
    }

    unsafe fn take_ffi_string(value: *mut c_char) -> String {
        assert!(!value.is_null());
        let text = unsafe { CStr::from_ptr(value) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe {
            chaft_string_free(value);
        }
        text
    }

    #[test]
    fn version_is_static_c_string() {
        let version = unsafe { CStr::from_ptr(chaft_core_version()) }
            .to_str()
            .unwrap();

        assert_eq!(version, "0.1.0");
    }

    #[test]
    fn demo_snapshot_returns_plain_workspace_snapshot_json() {
        let json = unsafe { take_ffi_string(chaft_demo_workspace_snapshot_json()) };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["workspaceId"], "wrk_demo");
        assert_eq!(value["channelCount"], 4);
        assert_eq!(value["profileCount"], 1);
        assert_eq!(value["memberCount"], 1);
        assert_eq!(value["keyPackageCount"], 1);
        assert_eq!(value["peerEndpointCount"], 0);
        assert_eq!(value["channels"][0]["channelId"], "chn_general");
        assert_eq!(value["members"][0]["displayName"], "Mira");
        assert_eq!(value["timeline"][0]["kind"], "encrypted_message");
    }

    #[test]
    fn runtime_detect_compromise_ffi_reports_local_rotation_trigger() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Signals", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "ffi signal before tamper",
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
        let event_store_path = runtime.paths().event_store.clone();
        drop(runtime);

        EventStore::open(event_store_path)
            .unwrap()
            .append_event(&forged)
            .unwrap();

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id).unwrap();
        let report_json = unsafe {
            take_ffi_string(chaft_runtime_detect_compromise_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let report = serde_json::from_str::<Value>(&report_json).unwrap();
        assert_eq!(report["ok"], true);
        assert_eq!(report["value"]["hasSignals"], true);
        assert_eq!(report["value"]["signalCount"], 1);
        assert_eq!(report["value"]["localDeviceSignalCount"], 1);
        assert_eq!(report["value"]["shouldRotateLocalSecretState"], true);
        assert_eq!(
            report["value"]["recommendedAction"],
            "rotate_workspace_for_suspected_compromise"
        );
        assert_eq!(report["value"]["signals"][0]["eventId"], forged.event_id.0);

        let response_json = unsafe {
            take_ffi_string(chaft_runtime_respond_compromise_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let response = serde_json::from_str::<Value>(&response_json).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["value"]["rotatedLocalSecretState"], true);
        assert_eq!(
            response["value"]["actionTaken"],
            "rotate_workspace_for_suspected_compromise"
        );
        assert_eq!(
            response["value"]["respondedSignalEventIds"],
            Value::Array(vec![Value::String(forged.event_id.0.clone())])
        );

        let second_response_json = unsafe {
            take_ffi_string(chaft_runtime_respond_compromise_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let second_response = serde_json::from_str::<Value>(&second_response_json).unwrap();
        assert_eq!(second_response["ok"], true);
        assert_eq!(second_response["value"]["rotatedLocalSecretState"], false);
        assert_eq!(
            second_response["value"]["skippedReason"],
            "local_signals_already_handled"
        );
    }

    #[test]
    fn runtime_identity_passphrase_ffi_cache_unlocks_without_environment() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let passphrase = CString::new("cache unlock passphrase").unwrap();
        let wrong_passphrase = CString::new("wrong cache unlock passphrase").unwrap();
        let workspace_name = CString::new("Chaft Locked Runtime").unwrap();
        let channel_name = CString::new("general").unwrap();

        assert!(unsafe {
            chaft_runtime_set_identity_passphrase(data_dir.as_ptr(), passphrase.as_ptr())
        });

        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        assert_eq!(created["ok"], true);

        let direct_open_error = LocalRuntime::open(tempdir.path(), None)
            .err()
            .expect("encrypted runtime should require a passphrase");
        assert!(
            direct_open_error
                .to_string()
                .contains("encrypted identity passphrase is required")
        );

        assert!(unsafe { chaft_runtime_clear_identity_passphrase(data_dir.as_ptr()) });
        assert!(unsafe {
            chaft_runtime_set_identity_passphrase(data_dir.as_ptr(), wrong_passphrase.as_ptr())
        });

        let wrong_device_json = unsafe {
            take_ffi_string(chaft_runtime_device_id_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let wrong_device = serde_json::from_str::<Value>(&wrong_device_json).unwrap();
        assert_eq!(wrong_device["ok"], false);
        assert!(
            wrong_device["error"]["message"]
                .as_str()
                .unwrap()
                .contains("authenticated decryption failed")
        );

        assert!(unsafe {
            chaft_runtime_set_identity_passphrase(data_dir.as_ptr(), passphrase.as_ptr())
        });

        let device_json = unsafe {
            take_ffi_string(chaft_runtime_device_id_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let device = serde_json::from_str::<Value>(&device_json).unwrap();
        assert_eq!(device["ok"], true);
        assert!(
            device["value"]["deviceId"]
                .as_str()
                .unwrap()
                .starts_with("dev_")
        );

        assert!(unsafe { chaft_runtime_clear_identity_passphrase(data_dir.as_ptr()) });
        let cleared_device_json = unsafe {
            take_ffi_string(chaft_runtime_device_id_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let cleared_device = serde_json::from_str::<Value>(&cleared_device_json).unwrap();
        assert_eq!(cleared_device["ok"], false);
        assert!(
            cleared_device["error"]["message"]
                .as_str()
                .unwrap()
                .contains("encrypted identity passphrase is required")
        );
    }

    #[test]
    fn snapshot_from_events_returns_result_envelope() {
        let (workspace_id, events) = sample_events();
        let workspace_id = CString::new(workspace_id.0).unwrap();
        let events_json = CString::new(serde_json::to_string(&events).unwrap()).unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["name"], "Chaft FFI");
        assert_eq!(value["value"]["channels"][0]["channelId"], "chn_general");
        assert_eq!(value["value"]["timeline"][0]["kind"], "encrypted_message");
        assert!(value["error"].is_null());
    }

    #[test]
    fn snapshot_from_events_reports_invalid_json() {
        let workspace_id = CString::new("wrk_test").unwrap();
        let events_json = CString::new("not-json").unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "invalid_events_json");
    }

    #[test]
    fn snapshot_from_events_rejects_oversized_events_json_before_parse() {
        let workspace_id = CString::new("wrk_test").unwrap();
        let events_json = CString::new("x".repeat(WORKSPACE_EVENTS_JSON_MAX_BYTES + 1)).unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "events_json_too_large");
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("events JSON is too large")
        );
    }

    #[test]
    fn ffi_reader_rejects_oversized_identifier_fields() {
        let cases = [
            (
                "workspace_id",
                WORKSPACE_ID_MAX_BYTES,
                "workspace_id_too_large",
            ),
            ("channel_id", CHANNEL_ID_MAX_BYTES, "channel_id_too_large"),
            ("message_id", MESSAGE_ID_MAX_BYTES, "message_id_too_large"),
            (
                "reply_to_message_id",
                MESSAGE_ID_MAX_BYTES,
                "message_id_too_large",
            ),
            ("event_id", EVENT_ID_MAX_BYTES, "event_id_too_large"),
            ("source_event_id", EVENT_ID_MAX_BYTES, "event_id_too_large"),
            ("device_id", DEVICE_ID_MAX_BYTES, "device_id_too_large"),
            (
                "key_package_id",
                DEVICE_KEY_PACKAGE_ID_MAX_BYTES,
                "key_package_id_too_large",
            ),
        ];

        for (field_name, max_bytes, expected_code) in cases {
            let value = CString::new("x".repeat(max_bytes + 1)).unwrap();
            let error = read_c_string(value.as_ptr(), field_name).unwrap_err();
            assert_eq!(error.code, expected_code);
            assert!(
                error
                    .message
                    .contains(&format!("{} bytes, max {}", max_bytes + 1, max_bytes)),
                "unexpected error message for {field_name}: {}",
                error.message
            );
        }
    }

    #[test]
    fn ffi_id_args_trim_required_values() {
        let canonical_event_id = format!("evt_{}", "1".repeat(64));

        assert_eq!(
            ffi_workspace_id_arg("  wrk_ffi  ".to_owned()).unwrap(),
            "wrk_ffi"
        );
        assert_eq!(
            ffi_channel_id_arg("  chn_ffi  ".to_owned()).unwrap(),
            "chn_ffi"
        );
        assert_eq!(
            ffi_message_id_arg("  msg_ffi  ".to_owned()).unwrap(),
            "msg_ffi"
        );
        assert_eq!(
            ffi_device_id_arg("  dev_ffi  ".to_owned()).unwrap(),
            "dev_ffi"
        );
        assert_eq!(
            ffi_device_key_package_id_arg("  dkp_ffi  ".to_owned()).unwrap(),
            "dkp_ffi"
        );
        assert_eq!(
            ffi_event_id_arg(format!("  {canonical_event_id}  ")).unwrap(),
            canonical_event_id
        );
    }

    #[test]
    fn ffi_id_args_reject_blank_required_values() {
        let cases = [
            (
                ffi_workspace_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "workspace_id_required",
            ),
            (
                ffi_channel_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "channel_id_required",
            ),
            (
                ffi_message_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "message_id_required",
            ),
            (
                ffi_device_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "device_id_required",
            ),
            (
                ffi_device_key_package_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "key_package_id_required",
            ),
            (
                ffi_event_id_arg(" \t\n ".to_owned()).unwrap_err(),
                "event_id_required",
            ),
        ];

        for (error, expected_code) in cases {
            assert_eq!(error.code, expected_code);
        }
    }

    #[test]
    fn ffi_optional_id_args_apply_selector_rules() {
        let canonical_event_id = format!("evt_{}", "2".repeat(64));

        assert!(ffi_optional_message_id_arg(None).unwrap().is_none());
        assert!(
            ffi_optional_message_id_arg(Some(" \t\n ".to_owned()))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            ffi_optional_message_id_arg(Some("  msg_reply  ".to_owned()))
                .unwrap()
                .unwrap()
                .0,
            "msg_reply"
        );

        assert!(ffi_optional_event_id_arg(None).unwrap().is_none());
        assert_eq!(
            ffi_optional_event_id_arg(Some(format!("  {canonical_event_id}  ")))
                .unwrap()
                .unwrap()
                .0,
            canonical_event_id
        );
        assert_eq!(
            ffi_optional_event_id_arg(Some("evt_NOT_CANONICAL".to_owned()))
                .unwrap_err()
                .code,
            "event_id_not_canonical"
        );
        assert_eq!(
            ffi_optional_event_id_arg(Some(" \t\n ".to_owned()))
                .unwrap_err()
                .code,
            "event_id_required"
        );
    }

    #[test]
    fn ffi_env_identity_passphrase_uses_passphrase_budget() {
        assert!(!env_identity_passphrase_is_usable(""));
        assert!(!env_identity_passphrase_is_usable(" \t\n "));
        assert!(env_identity_passphrase_is_usable("valid passphrase"));
        assert!(env_identity_passphrase_is_usable(
            &"p".repeat(FFI_PASSPHRASE_MAX_BYTES)
        ));
        assert!(!env_identity_passphrase_is_usable(
            &"p".repeat(FFI_PASSPHRASE_MAX_BYTES + 1)
        ));
    }

    #[test]
    fn ffi_reader_rejects_oversized_bounded_payload_fields() {
        let cases = [
            ("data_dir", FFI_PATH_MAX_BYTES, "data_dir_too_large"),
            (
                "identity_file",
                FFI_PATH_MAX_BYTES,
                "identity_file_too_large",
            ),
            ("store_path", FFI_PATH_MAX_BYTES, "store_path_too_large"),
            ("file_path", FFI_PATH_MAX_BYTES, "file_path_too_large"),
            ("output_path", FFI_PATH_MAX_BYTES, "output_path_too_large"),
            (
                "key_package_file",
                FFI_PATH_MAX_BYTES,
                "key_package_file_too_large",
            ),
            (
                "passphrase",
                FFI_PASSPHRASE_MAX_BYTES,
                "passphrase_too_large",
            ),
            (
                "role",
                WORKSPACE_ROLE_TEXT_MAX_BYTES,
                "workspace_role_too_large",
            ),
            ("name", WORKSPACE_NAME_MAX_BYTES, "name_too_large"),
            (
                "default_channel_name",
                CHANNEL_NAME_MAX_BYTES,
                "channel_name_too_large",
            ),
            (
                "display_name",
                DEVICE_DISPLAY_NAME_MAX_BYTES,
                "display_name_too_large",
            ),
            (
                "protocol",
                DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
                "key_package_protocol_too_large",
            ),
            (
                "text",
                MESSAGE_MARKDOWN_MAX_BYTES,
                "message_markdown_too_large",
            ),
            ("reaction", REACTION_TEXT_MAX_BYTES, "reaction_too_large"),
            ("query", SEARCH_QUERY_MAX_BYTES, "search_query_too_large"),
            (
                "media_type",
                ATTACHMENT_MEDIA_TYPE_MAX_BYTES,
                "attachment_media_type_too_large",
            ),
            (
                "blob_hash",
                ATTACHMENT_ID_MAX_BYTES,
                "attachment_selector_too_large",
            ),
            (
                "endpoint",
                PEER_ENDPOINT_MAX_BYTES,
                "peer_endpoint_too_large",
            ),
            (
                "peer_endpoint",
                PEER_ENDPOINT_MAX_BYTES,
                "peer_endpoint_too_large",
            ),
            (
                "transport",
                PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
                "peer_endpoint_transport_too_large",
            ),
            (
                "bundle_json",
                RECOVERY_BUNDLE_JSON_MAX_BYTES,
                "recovery_bundle_json_too_large",
            ),
        ];

        for (field_name, max_bytes, expected_code) in cases {
            let value = CString::new("x".repeat(max_bytes + 1)).unwrap();
            let error = read_c_string(value.as_ptr(), field_name).unwrap_err();
            assert_eq!(error.code, expected_code);
            assert!(
                error
                    .message
                    .contains(&format!("{} bytes, max {}", max_bytes + 1, max_bytes)),
                "unexpected error message for {field_name}: {}",
                error.message
            );
        }
    }

    #[test]
    fn bounded_ffi_reader_rejects_after_limit_without_waiting_for_nul() {
        let bytes = [b'x' as c_char; 4];
        let error =
            read_c_string_with_max_bytes(bytes.as_ptr(), "field", 3, "field_too_large", "field")
                .unwrap_err();

        assert_eq!(error.code, "field_too_large");
        assert!(error.message.contains("4 bytes, max 3"));
    }

    #[test]
    fn generic_ffi_reader_fallback_is_bounded() {
        let bytes = vec![b'x' as c_char; FFI_GENERIC_STRING_MAX_BYTES + 1];
        let error = read_c_string(bytes.as_ptr(), "future_field").unwrap_err();

        assert_eq!(error.code, "ffi_string_too_large");
        assert!(error.message.contains(&format!(
            "{} bytes, max {}",
            FFI_GENERIC_STRING_MAX_BYTES + 1,
            FFI_GENERIC_STRING_MAX_BYTES
        )));
    }

    #[test]
    fn snapshot_from_events_rejects_oversized_workspace_id_before_parse() {
        let workspace_id = CString::new("x".repeat(WORKSPACE_ID_MAX_BYTES + 1)).unwrap();
        let events_json = CString::new("not-json").unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "workspace_id_too_large");
    }

    #[test]
    fn snapshot_from_events_rejects_blank_workspace_id_before_parse() {
        let workspace_id = CString::new(" \t\n ").unwrap();
        let events_json = CString::new("not-json").unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "workspace_id_required");
    }

    #[test]
    fn snapshot_from_events_trims_workspace_id() {
        let (workspace_id, events) = sample_events();
        let workspace_id = CString::new(format!("  {}  ", workspace_id.0)).unwrap();
        let events_json = CString::new(serde_json::to_string(&events).unwrap()).unwrap();

        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_events_result_json(
                workspace_id.as_ptr(),
                events_json.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["name"], "Chaft FFI");
    }

    #[test]
    fn runtime_publish_queue_ffi_reports_local_publishable_events() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Queue", "general")
            .unwrap();
        runtime
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id),
                "queued local message",
            )
            .unwrap();
        drop(runtime);
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id.clone()).unwrap();

        let json = unsafe {
            take_ffi_string(chaft_runtime_workspace_publish_queue_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["workspaceId"], created.workspace_id);
        assert_eq!(
            value["value"]["publishableEventIds"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            value["value"]["backupEventIds"].as_array().unwrap().len(),
            1
        );
        assert_eq!(value["value"]["summary"]["publishableEventCount"], 3);
        assert_eq!(value["value"]["summary"]["backupEventCount"], 1);
        assert_eq!(value["value"]["summary"]["queuedMessageEventCount"], 1);
        assert_eq!(value["value"]["summary"]["missingBlobCount"], 0);
        assert_eq!(value["value"]["summary"]["skippedGapCount"], 0);
        assert_eq!(value["value"]["summary"]["isComplete"], true);
        assert!(value["value"]["skippedGaps"].as_array().unwrap().is_empty());
    }

    #[test]
    fn openmls_apply_result_ffi_samples_arrays_without_changing_counts() {
        let applied_event_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 13;
        let workspace_report = AppliedOpenMlsWorkspaceGroupCommits {
            workspace_id: "wrk_sample".to_owned(),
            device_id: "dev_sample".to_owned(),
            protocol: "openmls".to_owned(),
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
            group_id: "mls_workspace_sample".to_owned(),
            epoch: 42,
            member_count: 3,
            applied_event_count,
            applied_event_ids: sample_strings("evt_openmls_workspace_applied", applied_event_count),
            self_removed: false,
            private_group_state_path: "/tmp/workspace_group.bin".to_owned(),
        };
        let channel_report = AppliedOpenMlsChannelGroupCommits {
            workspace_id: "wrk_sample".to_owned(),
            channel_id: "chn_sample".to_owned(),
            device_id: "dev_sample".to_owned(),
            protocol: "openmls".to_owned(),
            ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
            group_id: "mls_channel_sample".to_owned(),
            epoch: 42,
            member_count: 3,
            applied_event_count,
            applied_event_ids: sample_strings("evt_openmls_channel_applied", applied_event_count),
            self_removed: false,
            private_group_state_path: "/tmp/channel_group.bin".to_owned(),
        };

        let sampled_workspace = sample_applied_openmls_workspace_commits_report(workspace_report);
        let sampled_channel = sample_applied_openmls_channel_commits_report(channel_report);

        assert_eq!(sampled_workspace.applied_event_count, applied_event_count);
        assert_eq!(
            sampled_workspace.applied_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled_channel.applied_event_count, applied_event_count);
        assert_eq!(
            sampled_channel.applied_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
    }

    #[test]
    fn recovery_import_result_ffi_samples_arrays_without_changing_counts() {
        let imported_channel_count = MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS + 17;
        let report = ImportedWorkspaceRecoveryBundle {
            workspace_id: "wrk_sample".to_owned(),
            workspace_key_id: "workspace_key_sample".to_owned(),
            imported_channel_count,
            imported_channel_ids: sample_strings("chn_imported", imported_channel_count),
            importer_device_id: "dev_importer".to_owned(),
        };

        let sampled = sample_imported_workspace_recovery_bundle_report(report);

        assert_eq!(sampled.imported_channel_count, imported_channel_count);
        assert_eq!(
            sampled.imported_channel_ids.len(),
            MAX_RESULT_CHANNEL_ID_SAMPLE_ROWS
        );
    }

    #[test]
    fn openmls_update_result_ffi_samples_arrays_without_changing_counts() {
        let channel_update_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 5;
        let updated_event_count = channel_update_count + 1;
        let report = UpdatedWorkspaceOpenMlsGroups {
            workspace_id: "wrk_sample".to_owned(),
            workspace_update: Some(sample_openmls_workspace_update(0)),
            channel_update_count,
            channel_updates: (0..channel_update_count)
                .map(sample_openmls_channel_update)
                .collect(),
            updated_event_count,
            updated_event_ids: sample_strings("evt_openmls_updated", updated_event_count),
        };

        let sampled = sample_updated_workspace_openmls_groups_report(report);

        assert_eq!(sampled.channel_update_count, channel_update_count);
        assert_eq!(
            sampled.channel_updates.len(),
            MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS
        );
        assert_eq!(sampled.updated_event_count, updated_event_count);
        assert_eq!(
            sampled.updated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
    }

    #[test]
    fn manual_rotation_result_ffi_samples_arrays_without_changing_counts() {
        let channel_key_rotation_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 9;
        let rotated_event_count = channel_key_rotation_count + 1;
        let report = RotatedWorkspaceManualKeys {
            workspace_id: "wrk_sample".to_owned(),
            workspace_key_rotation: sample_workspace_key_rotation(0),
            channel_key_rotation_count,
            channel_key_rotations: (0..channel_key_rotation_count)
                .map(sample_channel_key_rotation)
                .collect(),
            rotated_event_count,
            rotated_event_ids: sample_strings("evt_manual_rotated", rotated_event_count),
        };

        let sampled = sample_rotated_workspace_manual_keys_report(report);

        assert_eq!(
            sampled.channel_key_rotation_count,
            channel_key_rotation_count
        );
        assert_eq!(
            sampled.channel_key_rotations.len(),
            MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS
        );
        assert_eq!(sampled.rotated_event_count, rotated_event_count);
        assert_eq!(
            sampled.rotated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
    }

    #[test]
    fn member_rotation_result_ffi_samples_arrays_without_changing_counts() {
        let channel_key_rotation_count = MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS + 11;
        let report = RemovedMemberWithKeyRotation {
            workspace_id: "wrk_sample".to_owned(),
            removed_device_id: "dev_removed".to_owned(),
            removal_event_id: "evt_removed".to_owned(),
            workspace_key_rotation: sample_workspace_key_rotation(0),
            channel_key_rotation_count,
            channel_key_rotations: (0..channel_key_rotation_count)
                .map(sample_channel_key_rotation)
                .collect(),
        };

        let sampled = sample_removed_member_with_key_rotation_report(report);

        assert_eq!(
            sampled.channel_key_rotation_count,
            channel_key_rotation_count
        );
        assert_eq!(
            sampled.channel_key_rotations.len(),
            MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS
        );
    }

    #[test]
    fn compromise_response_ffi_samples_nested_rotation_without_changing_counts() {
        let signal_count = MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS + 5;
        let event_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 7;
        let channel_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 3;
        let response = WorkspaceCompromiseResponse {
            workspace_id: "wrk_sample".to_owned(),
            report: WorkspaceCompromiseReport {
                workspace_id: "wrk_sample".to_owned(),
                has_signals: true,
                signal_count,
                invalid_signature_count: signal_count,
                local_device_signal_count: signal_count,
                should_rotate_local_secret_state: true,
                recommended_action: Some("rotateWorkspaceForSuspectedCompromise".to_owned()),
                signals: (0..signal_count).map(sample_compromise_signal).collect(),
            },
            action_taken: Some("rotateWorkspaceForSuspectedCompromise".to_owned()),
            rotated_local_secret_state: true,
            skipped_reason: None,
            responded_signal_count: event_count,
            responded_signal_event_ids: sample_strings("evt_signal_responded", event_count),
            already_handled_signal_count: event_count,
            already_handled_signal_event_ids: sample_strings("evt_signal_handled", event_count),
            rotation: Some(RotatedWorkspaceForSuspectedCompromise {
                workspace_id: "wrk_sample".to_owned(),
                openmls_updates: Some(UpdatedWorkspaceOpenMlsGroups {
                    workspace_id: "wrk_sample".to_owned(),
                    workspace_update: Some(sample_openmls_workspace_update(0)),
                    channel_update_count: channel_count,
                    channel_updates: (0..channel_count)
                        .map(sample_openmls_channel_update)
                        .collect(),
                    updated_event_count: event_count,
                    updated_event_ids: sample_strings("evt_openmls_updated", event_count),
                }),
                manual_key_rotation: Some(RotatedWorkspaceManualKeys {
                    workspace_id: "wrk_sample".to_owned(),
                    workspace_key_rotation: sample_workspace_key_rotation(0),
                    channel_key_rotation_count: channel_count,
                    channel_key_rotations: (0..channel_count)
                        .map(sample_channel_key_rotation)
                        .collect(),
                    rotated_event_count: event_count,
                    rotated_event_ids: sample_strings("evt_manual_rotated", event_count),
                }),
                rotated_event_count: event_count,
                rotated_event_ids: sample_strings("evt_compromise_rotated", event_count),
            }),
        };

        let sampled = sample_compromise_response_report_with_rotation_samples(response);

        assert_eq!(sampled.report.signal_count, signal_count);
        assert_eq!(
            sampled.report.signals.len(),
            MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS
        );
        assert_eq!(sampled.responded_signal_count, event_count);
        assert_eq!(
            sampled.responded_signal_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        let rotation = sampled.rotation.unwrap();
        assert_eq!(rotation.rotated_event_count, event_count);
        assert_eq!(
            rotation.rotated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        let openmls_updates = rotation.openmls_updates.unwrap();
        assert_eq!(openmls_updates.channel_update_count, channel_count);
        assert_eq!(
            openmls_updates.channel_updates.len(),
            MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS
        );
        assert_eq!(openmls_updates.updated_event_count, event_count);
        assert_eq!(
            openmls_updates.updated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        let manual_rotation = rotation.manual_key_rotation.unwrap();
        assert_eq!(manual_rotation.channel_key_rotation_count, channel_count);
        assert_eq!(
            manual_rotation.channel_key_rotations.len(),
            MAX_RESULT_KEY_ROTATION_SAMPLE_ROWS
        );
        assert_eq!(manual_rotation.rotated_event_count, event_count);
        assert_eq!(
            manual_rotation.rotated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
    }

    #[test]
    fn direct_result_ffi_samples_arrays_without_changing_counts() {
        let published_event_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 7;
        let published_blob_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 5;
        let skipped_gap_count = MAX_RESULT_GAP_SAMPLE_ROWS + 3;
        let blob_transfer_attempt_count = MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS + 4;
        let published = PublishedWorkspace {
            workspace_id: "wrk_sample".to_owned(),
            published_event_count,
            published_event_ids: sample_strings("evt_published", published_event_count),
            published_blob_count,
            published_blob_hashes: sample_strings("blob_published", published_blob_count),
            missing_blob_count: published_blob_count,
            missing_blob_hashes: sample_strings("blob_missing", published_blob_count),
            skipped_gap_count,
            skipped_gaps: (0..skipped_gap_count).map(sample_workspace_gap).collect(),
            blob_transfer_attempt_count,
            blob_transfer_attempts: (0..blob_transfer_attempt_count)
                .map(sample_blob_transfer_attempt)
                .collect(),
        };

        let requested_event_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS + 9;
        let fetched_blob_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 6;
        let gap_count = MAX_RESULT_GAP_SAMPLE_ROWS + 2;
        let openmls_event_count = MAX_RESULT_EVENT_ID_SAMPLE_ROWS * 2 + 5;
        let signal_count = MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS + 3;
        let pulled = PulledWorkspace {
            workspace_id: "wrk_sample".to_owned(),
            requested_event_count,
            requested_event_ids: sample_strings("evt_requested", requested_event_count),
            fetched_event_count: requested_event_count,
            fetched_event_ids: sample_strings("evt_fetched", requested_event_count),
            fetched_blob_count,
            fetched_blob_hashes: sample_strings("blob_fetched", fetched_blob_count),
            missing_blob_count: fetched_blob_count,
            missing_blob_hashes: sample_strings("blob_pull_missing", fetched_blob_count),
            ignored_event_count: requested_event_count,
            ignored_event_ids: sample_strings("evt_ignored", requested_event_count),
            applied_event_count: requested_event_count,
            applied_event_ids: sample_strings("evt_applied", requested_event_count),
            openmls_catchup: PulledOpenMlsCatchup {
                event_count: openmls_event_count,
                workspace_joined_event_id: Some("evt_workspace_joined".to_owned()),
                workspace_applied_event_ids: sample_strings(
                    "evt_mls_workspace_applied",
                    openmls_event_count,
                ),
                workspace_provisioned_event_ids: sample_strings(
                    "evt_mls_workspace_provisioned",
                    openmls_event_count,
                ),
                workspace_self_removed: false,
                channel_groups: (0..(MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS + 2))
                    .map(|index| PulledOpenMlsChannelCatchup {
                        channel_id: format!("chn_{index:03}"),
                        event_count: openmls_event_count,
                        joined_event_id: Some(format!("evt_channel_joined_{index:03}")),
                        applied_event_ids: sample_strings(
                            &format!("evt_mls_channel_applied_{index:03}"),
                            openmls_event_count,
                        ),
                        provisioned_event_ids: sample_strings(
                            &format!("evt_mls_channel_provisioned_{index:03}"),
                            openmls_event_count,
                        ),
                        self_removed: false,
                    })
                    .collect(),
            },
            compromise_response: Some(WorkspaceCompromiseResponse {
                workspace_id: "wrk_sample".to_owned(),
                report: WorkspaceCompromiseReport {
                    workspace_id: "wrk_sample".to_owned(),
                    has_signals: true,
                    signal_count,
                    invalid_signature_count: signal_count,
                    local_device_signal_count: signal_count,
                    should_rotate_local_secret_state: true,
                    recommended_action: Some("rotateWorkspaceForSuspectedCompromise".to_owned()),
                    signals: (0..signal_count).map(sample_compromise_signal).collect(),
                },
                action_taken: Some("rotateWorkspaceForSuspectedCompromise".to_owned()),
                rotated_local_secret_state: true,
                skipped_reason: None,
                responded_signal_count: requested_event_count,
                responded_signal_event_ids: sample_strings(
                    "evt_signal_responded",
                    requested_event_count,
                ),
                already_handled_signal_count: requested_event_count,
                already_handled_signal_event_ids: sample_strings(
                    "evt_signal_handled",
                    requested_event_count,
                ),
                rotation: Some(RotatedWorkspaceForSuspectedCompromise {
                    workspace_id: "wrk_sample".to_owned(),
                    openmls_updates: None,
                    manual_key_rotation: None,
                    rotated_event_count: requested_event_count,
                    rotated_event_ids: sample_strings("evt_rotated", requested_event_count),
                }),
            }),
            gap_count,
            gaps: (0..gap_count).map(sample_workspace_gap).collect(),
        };

        let sampled = sample_synced_workspace_report(SyncedWorkspace {
            workspace_id: "wrk_sample".to_owned(),
            published,
            pulled,
        });

        assert_eq!(
            sampled.published.published_event_count,
            published_event_count
        );
        assert_eq!(
            sampled.published.published_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.published.published_blob_count, published_blob_count);
        assert_eq!(
            sampled.published.published_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.published.missing_blob_count, published_blob_count);
        assert_eq!(
            sampled.published.missing_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.published.skipped_gap_count, skipped_gap_count);
        assert_eq!(
            sampled.published.skipped_gaps.len(),
            MAX_RESULT_GAP_SAMPLE_ROWS
        );
        assert_eq!(
            sampled.published.blob_transfer_attempt_count,
            blob_transfer_attempt_count
        );
        assert_eq!(
            sampled.published.blob_transfer_attempts.len(),
            MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS
        );
        assert_sampled_blob_transfer_attempt_chunks(&sampled.published.blob_transfer_attempts[0]);

        assert_eq!(sampled.pulled.requested_event_count, requested_event_count);
        assert_eq!(
            sampled.pulled.requested_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.fetched_event_count, requested_event_count);
        assert_eq!(
            sampled.pulled.fetched_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.fetched_blob_count, fetched_blob_count);
        assert_eq!(
            sampled.pulled.fetched_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.missing_blob_count, fetched_blob_count);
        assert_eq!(
            sampled.pulled.missing_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.ignored_event_count, requested_event_count);
        assert_eq!(
            sampled.pulled.ignored_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.applied_event_count, requested_event_count);
        assert_eq!(
            sampled.pulled.applied_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.pulled.gap_count, gap_count);
        assert_eq!(sampled.pulled.gaps.len(), MAX_RESULT_GAP_SAMPLE_ROWS);

        assert_eq!(
            sampled.pulled.openmls_catchup.event_count,
            openmls_event_count
        );
        assert_eq!(
            sampled
                .pulled
                .openmls_catchup
                .workspace_applied_event_ids
                .len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(
            sampled.pulled.openmls_catchup.channel_groups.len(),
            MAX_RESULT_OPENMLS_CHANNEL_GROUP_SAMPLE_ROWS
        );
        assert_eq!(
            sampled.pulled.openmls_catchup.channel_groups[0].event_count,
            openmls_event_count
        );
        assert_eq!(
            sampled.pulled.openmls_catchup.channel_groups[0]
                .applied_event_ids
                .len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );

        let compromise = sampled.pulled.compromise_response.unwrap();
        assert_eq!(compromise.report.signal_count, signal_count);
        assert_eq!(
            compromise.report.signals.len(),
            MAX_RESULT_COMPROMISE_SIGNAL_SAMPLE_ROWS
        );
        assert_eq!(compromise.responded_signal_count, requested_event_count);
        assert_eq!(
            compromise.responded_signal_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert_eq!(
            compromise.already_handled_signal_count,
            requested_event_count
        );
        assert_eq!(
            compromise.already_handled_signal_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        let rotation = compromise.rotation.unwrap();
        assert_eq!(rotation.rotated_event_count, requested_event_count);
        assert_eq!(
            rotation.rotated_event_ids.len(),
            MAX_RESULT_EVENT_ID_SAMPLE_ROWS
        );
        assert!(rotation.openmls_updates.is_none());
        assert!(rotation.manual_key_rotation.is_none());
    }

    #[test]
    fn retry_result_ffi_samples_arrays_without_changing_counts() {
        let pending_attempt_count = MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS + 5;
        let blob_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 7;
        let peer_error_count = MAX_RESULT_PEER_ERROR_SAMPLE_ROWS + 3;
        let blob_transfer_attempt_count = MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS + 4;
        let report = BlobTransferRetryReport {
            workspace_id: "wrk_sample".to_owned(),
            pending_attempt_count,
            pending_attempt_ids: sample_strings("attempt_pending", pending_attempt_count),
            retried_blob_count: blob_count,
            retried_blob_hashes: sample_strings("blob_retried", blob_count),
            reconciled_blob_count: blob_count,
            reconciled_blob_hashes: sample_strings("blob_reconciled", blob_count),
            missing_blob_count: blob_count,
            missing_blob_hashes: sample_strings("blob_missing", blob_count),
            skipped_blob_count: blob_count,
            skipped_blob_hashes: sample_strings("blob_skipped", blob_count),
            peer_error_count,
            peer_errors: (0..peer_error_count)
                .map(sample_blob_transfer_peer_error)
                .collect(),
            blob_transfer_attempt_count,
            blob_transfer_attempts: (0..blob_transfer_attempt_count)
                .map(sample_blob_transfer_attempt)
                .collect(),
        };

        let sampled = sample_blob_transfer_retry_report(report);

        assert_eq!(sampled.pending_attempt_count, pending_attempt_count);
        assert_eq!(
            sampled.pending_attempt_ids.len(),
            MAX_RESULT_ATTEMPT_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.retried_blob_count, blob_count);
        assert_eq!(
            sampled.retried_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.reconciled_blob_count, blob_count);
        assert_eq!(
            sampled.reconciled_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.missing_blob_count, blob_count);
        assert_eq!(
            sampled.missing_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.skipped_blob_count, blob_count);
        assert_eq!(
            sampled.skipped_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.peer_error_count, peer_error_count);
        assert_eq!(sampled.peer_errors.len(), MAX_RESULT_PEER_ERROR_SAMPLE_ROWS);
        assert_eq!(
            sampled.peer_errors[0].message.len(),
            MAX_RESULT_PEER_ERROR_MESSAGE_BYTES
        );
        assert!(
            sampled.peer_errors[0]
                .message
                .is_char_boundary(sampled.peer_errors[0].message.len())
        );
        assert_eq!(
            sampled.blob_transfer_attempt_count,
            blob_transfer_attempt_count
        );
        assert_eq!(
            sampled.blob_transfer_attempts.len(),
            MAX_RESULT_BLOB_TRANSFER_ATTEMPT_SAMPLE_ROWS
        );
        assert_sampled_blob_transfer_attempt_chunks(&sampled.blob_transfer_attempts[0]);
    }

    #[test]
    fn prune_result_ffi_samples_arrays_without_changing_counts() {
        let workspace_count = MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS + 4;
        let blob_count = MAX_RESULT_BLOB_HASH_SAMPLE_ROWS + 6;
        let report = PrunedBlobCache {
            workspace_count,
            workspace_ids: sample_strings("wrk", workspace_count),
            referenced_blob_count: blob_count,
            referenced_blob_hashes: sample_strings("blob_referenced", blob_count),
            removed_blob_count: blob_count,
            removed_blob_hashes: sample_strings("blob_removed", blob_count),
            removed_manifest_count: blob_count,
            removed_manifest_hashes: sample_strings("manifest_removed", blob_count),
            removed_chunk_count: blob_count,
            removed_chunk_hashes: sample_strings("chunk_removed", blob_count),
            removed_temp_file_count: blob_count,
            removed_temp_file_paths: sample_strings("temp_removed", blob_count),
        };

        let sampled = sample_pruned_blob_cache_report(report);

        assert_eq!(sampled.workspace_count, workspace_count);
        assert_eq!(
            sampled.workspace_ids.len(),
            MAX_RESULT_WORKSPACE_ID_SAMPLE_ROWS
        );
        assert_eq!(sampled.referenced_blob_count, blob_count);
        assert_eq!(
            sampled.referenced_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.removed_blob_count, blob_count);
        assert_eq!(
            sampled.removed_blob_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.removed_manifest_count, blob_count);
        assert_eq!(
            sampled.removed_manifest_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.removed_chunk_count, blob_count);
        assert_eq!(
            sampled.removed_chunk_hashes.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(sampled.removed_temp_file_count, blob_count);
        assert_eq!(
            sampled.removed_temp_file_paths.len(),
            MAX_RESULT_BLOB_HASH_SAMPLE_ROWS
        );
    }

    #[test]
    fn snapshot_from_store_reads_only_requested_workspace() {
        let tempdir = tempfile::tempdir().unwrap();
        let store_path = tempdir.path().join("events.db");
        let store = EventStore::open(&store_path).unwrap();
        let (workspace_id, events) = sample_events();
        let other_workspace_id = WorkspaceId::new();
        let other_workspace = signed(SignableEvent::new(
            other_workspace_id,
            None,
            DeviceId("dev_test".to_owned()),
            EventBody::WorkspaceCreated {
                name: "Other".to_owned(),
            },
        ));

        for event in &events {
            store.append_event(event).unwrap();
        }
        store.append_event(&other_workspace).unwrap();
        drop(store);

        let store_path = CString::new(store_path.to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(workspace_id.0).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_store_result_json(
                store_path.as_ptr(),
                workspace_id.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["name"], "Chaft FFI");
        assert_eq!(value["value"]["channels"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn snapshot_from_store_latest_limits_timeline() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        for body in ["first", "second", "third"] {
            runtime
                .send_message(workspace_id.clone(), channel_id.clone(), body)
                .unwrap();
        }
        drop(runtime);

        let store_path = CString::new(
            tempdir
                .path()
                .join("events.db")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_store_latest_result_json(
                store_path.as_ptr(),
                workspace_id.as_ptr(),
                2,
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["timeline"].as_array().unwrap().len(), 2);
        assert_eq!(value["value"]["timelineWindow"]["startIndex"], 1);
        assert_eq!(value["value"]["timelineWindow"]["itemCount"], 2);
        assert_eq!(value["value"]["timelineWindow"]["totalCount"], 3);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreBefore"], true);
    }

    #[test]
    fn snapshot_from_store_window_loads_requested_page() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        for body in ["first", "second", "third", "fourth"] {
            runtime
                .send_message(workspace_id.clone(), channel_id.clone(), body)
                .unwrap();
        }
        drop(runtime);

        let store_path = CString::new(
            tempdir
                .path()
                .join("events.db")
                .to_string_lossy()
                .as_bytes(),
        )
        .unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_workspace_snapshot_from_store_window_result_json(
                store_path.as_ptr(),
                workspace_id.as_ptr(),
                1,
                2,
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["timeline"].as_array().unwrap().len(), 2);
        assert_eq!(value["value"]["timeline"][0]["encrypted"], true);
        assert_eq!(value["value"]["timeline"][1]["encrypted"], true);
        assert_eq!(value["value"]["timelineWindow"]["startIndex"], 1);
        assert_eq!(value["value"]["timelineWindow"]["itemCount"], 2);
        assert_eq!(value["value"]["timelineWindow"]["totalCount"], 4);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreBefore"], true);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreAfter"], true);
    }

    #[test]
    fn decrypted_snapshot_from_runtime_reads_local_workspace_key() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime", "general")
            .unwrap();
        runtime
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id),
                "ffi local plaintext",
            )
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["name"], "Chaft FFI Runtime");
        assert_eq!(value["value"]["timeline"][0]["body"], "ffi local plaintext");
        assert_eq!(value["value"]["timeline"][0]["encrypted"], true);
    }

    #[test]
    fn runtime_action_ffi_lists_bounded_workspace_summary_page() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let first = runtime
            .create_workspace("First Workspace", "general")
            .unwrap();
        let second = runtime.create_workspace("Second Workspace", "ops").unwrap();
        let third = runtime
            .create_workspace("Third Workspace", "design")
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                1,
                1,
            ))
        };
        let page = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(page["ok"], true);
        assert_eq!(page["value"]["startIndex"], 1);
        assert_eq!(page["value"]["itemCount"], 1);
        assert_eq!(page["value"]["totalCount"], 3);
        assert_eq!(page["value"]["hasMoreBefore"], true);
        assert_eq!(page["value"]["hasMoreAfter"], true);
        assert_eq!(page["value"]["workspaces"].as_array().unwrap().len(), 1);
        assert_eq!(
            page["value"]["workspaces"][0]["workspaceId"],
            second.workspace_id
        );
        assert_eq!(page["value"]["workspaces"][0]["name"], "Second Workspace");

        let tail_json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                3,
                4,
            ))
        };
        let tail = serde_json::from_str::<Value>(&tail_json).unwrap();
        assert_eq!(tail["ok"], true);
        assert_eq!(tail["value"]["startIndex"], 3);
        assert_eq!(tail["value"]["itemCount"], 0);
        assert_eq!(tail["value"]["totalCount"], 3);
        assert_eq!(tail["value"]["hasMoreBefore"], true);
        assert_eq!(tail["value"]["hasMoreAfter"], false);
        assert!(tail["value"]["workspaces"].as_array().unwrap().is_empty());
        assert_ne!(first.workspace_id, third.workspace_id);
    }

    #[test]
    fn runtime_action_ffi_legacy_workspace_list_returns_bounded_first_page() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let workspace_count = MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS + 2;
        let mut workspace_ids = Vec::new();
        for index in 0..workspace_count {
            let created = runtime
                .create_workspace(format!("Legacy Summary {index:03}"), "general")
                .unwrap();
            workspace_ids.push(created.workspace_id);
        }
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let legacy_json = unsafe {
            take_ffi_string(chaft_runtime_list_workspaces_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let legacy = serde_json::from_str::<Value>(&legacy_json).unwrap();

        assert_eq!(legacy["ok"], true);
        let summaries = legacy["value"].as_array().unwrap();
        assert_eq!(summaries.len(), MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS);
        assert_eq!(summaries[0]["workspaceId"], workspace_ids[0]);
        assert_eq!(
            summaries
                .last()
                .and_then(|summary| summary["workspaceId"].as_str())
                .unwrap(),
            workspace_ids[MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS - 1]
        );

        let tail_json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS,
                4,
            ))
        };
        let tail = serde_json::from_str::<Value>(&tail_json).unwrap();

        assert_eq!(tail["ok"], true);
        assert_eq!(
            tail["value"]["startIndex"],
            MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS
        );
        assert_eq!(tail["value"]["itemCount"], 2);
        assert_eq!(tail["value"]["totalCount"], workspace_count);
        assert_eq!(tail["value"]["hasMoreBefore"], true);
        assert_eq!(tail["value"]["hasMoreAfter"], false);
        assert_eq!(tail["value"]["workspaces"].as_array().unwrap().len(), 2);
        assert_eq!(
            tail["value"]["workspaces"][0]["workspaceId"],
            workspace_ids[MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS]
        );
    }

    #[test]
    fn runtime_action_ffi_lists_bounded_workspace_member_page() {
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
                workspace_id,
                DeviceId("dev_b".to_owned()),
                WorkspaceRole::Member,
            )
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_member_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                1,
                2,
            ))
        };
        let page = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(page["ok"], true);
        assert_eq!(page["value"]["startIndex"], 1);
        assert_eq!(page["value"]["itemCount"], 2);
        assert_eq!(page["value"]["totalCount"], 4);
        assert_eq!(page["value"]["hasMoreBefore"], true);
        assert_eq!(page["value"]["hasMoreAfter"], true);
        assert_eq!(page["value"]["members"].as_array().unwrap().len(), 2);
        assert_eq!(page["value"]["members"][0]["deviceId"], "dev_admin");
        assert_eq!(page["value"]["members"][1]["deviceId"], "dev_a");

        let tail_json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_member_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                10,
                2,
            ))
        };
        let tail = serde_json::from_str::<Value>(&tail_json).unwrap();
        assert_eq!(tail["ok"], true);
        assert_eq!(tail["value"]["startIndex"], 4);
        assert_eq!(tail["value"]["itemCount"], 0);
        assert_eq!(tail["value"]["totalCount"], 4);
        assert_eq!(tail["value"]["hasMoreBefore"], true);
        assert_eq!(tail["value"]["hasMoreAfter"], false);
        assert!(tail["value"]["members"].as_array().unwrap().is_empty());
    }

    #[test]
    fn runtime_action_ffi_lists_bounded_workspace_channel_page() {
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
            .edit_message(workspace_id, MessageId(sent.message_id), "beta edited")
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_runtime_list_workspace_channel_page_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                0,
                2,
            ))
        };
        let page = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(page["ok"], true);
        assert_eq!(page["value"]["startIndex"], 0);
        assert_eq!(page["value"]["itemCount"], 2);
        assert_eq!(page["value"]["totalCount"], 4);
        assert_eq!(page["value"]["hasMoreBefore"], false);
        assert_eq!(page["value"]["hasMoreAfter"], true);
        assert_eq!(page["value"]["channels"].as_array().unwrap().len(), 2);
        assert_eq!(page["value"]["channels"][0]["channelId"], beta.channel_id);
        assert_eq!(
            page["value"]["channels"][0]["latestActivity"]["preview"],
            "Edited: beta edited"
        );
        assert_eq!(page["value"]["channels"][1]["name"], "alpha");

        let gamma_id = CString::new(gamma.channel_id.clone()).unwrap();
        let containing_json = unsafe {
            take_ffi_string(
                chaft_runtime_list_workspace_channel_page_containing_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    gamma_id.as_ptr(),
                    2,
                ),
            )
        };
        let containing = serde_json::from_str::<Value>(&containing_json).unwrap();
        assert_eq!(containing["ok"], true);
        assert_eq!(containing["value"]["startIndex"], 2);
        assert_eq!(containing["value"]["itemCount"], 2);
        assert_eq!(containing["value"]["totalCount"], 4);
        assert_eq!(containing["value"]["hasMoreBefore"], true);
        assert_eq!(containing["value"]["hasMoreAfter"], false);
        assert_eq!(
            containing["value"]["channels"][0]["channelId"],
            gamma.channel_id
        );

        let query = CString::new("gam").unwrap();
        let search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_channels_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                query.as_ptr(),
                2,
            ))
        };
        let search = serde_json::from_str::<Value>(&search_json).unwrap();
        assert_eq!(search["ok"], true);
        assert_eq!(search["value"]["query"], "gam");
        assert_eq!(search["value"]["itemCount"], 1);
        assert_eq!(search["value"]["totalCount"], 1);
        assert_eq!(
            search["value"]["channels"][0]["channelId"],
            gamma.channel_id
        );
        assert_eq!(search["value"]["channels"][0]["name"], "gamma");
    }

    #[test]
    fn runtime_action_ffi_write_paths_skip_corrupt_local_event_json() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Corrupt Writes").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        assert_eq!(created["ok"], true);
        let workspace_id = created["value"]["workspaceId"].as_str().unwrap();
        let channel_id = created["value"]["channelId"].as_str().unwrap();
        insert_corrupt_event_json(
            tempdir.path(),
            workspace_id,
            "evt_corrupt_ffi_write_context_tripwire",
        );
        let strict_store = EventStore::open(tempdir.path().join("events.db")).unwrap();
        assert!(
            strict_store
                .list_events_for_workspace(workspace_id)
                .is_err()
        );
        drop(strict_store);

        let workspace_id_c = CString::new(workspace_id).unwrap();
        let channel_id_c = CString::new(channel_id).unwrap();
        let display_name = CString::new("FFI Writer").unwrap();
        let profile_json = unsafe {
            take_ffi_string(chaft_runtime_update_device_profile_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                display_name.as_ptr(),
            ))
        };
        let profile = serde_json::from_str::<Value>(&profile_json).unwrap();
        assert_eq!(profile["ok"], true);
        assert_eq!(profile["value"]["displayName"], "FFI Writer");

        let channel_name = CString::new("after-corrupt").unwrap();
        let channel_json = unsafe {
            take_ffi_string(chaft_runtime_create_channel_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_name.as_ptr(),
                false,
            ))
        };
        let channel = serde_json::from_str::<Value>(&channel_json).unwrap();
        assert_eq!(channel["ok"], true);
        assert_eq!(channel["value"]["workspaceId"], workspace_id);

        let text = CString::new("ffi message after corrupt row").unwrap();
        let sent_json = unsafe {
            take_ffi_string(chaft_runtime_send_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
                text.as_ptr(),
            ))
        };
        let sent = serde_json::from_str::<Value>(&sent_json).unwrap();
        assert_eq!(sent["ok"], true);
        assert_eq!(sent["value"]["encrypted"], true);

        let rotated_json = unsafe {
            take_ffi_string(chaft_runtime_rotate_workspace_key_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let rotated = serde_json::from_str::<Value>(&rotated_json).unwrap();
        assert_eq!(rotated["ok"], true);
        assert_eq!(rotated["value"]["workspaceId"], workspace_id);

        let reindexed_json = unsafe {
            take_ffi_string(chaft_runtime_reindex_workspace_search_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let reindexed = serde_json::from_str::<Value>(&reindexed_json).unwrap();
        assert_eq!(reindexed["ok"], true);
        assert_eq!(reindexed["value"]["indexedMessageCount"], 1);

        let query = CString::new("corrupt row").unwrap();
        let search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                query.as_ptr(),
            ))
        };
        let search = serde_json::from_str::<Value>(&search_json).unwrap();
        assert_eq!(search["ok"], true);
        assert_eq!(search["value"]["hits"].as_array().unwrap().len(), 1);
        assert_eq!(
            search["value"]["hits"][0]["messageId"],
            sent["value"]["messageId"]
        );

        let snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let snapshot = serde_json::from_str::<Value>(&snapshot_json).unwrap();
        assert_eq!(snapshot["ok"], true);
        assert!(
            snapshot["value"]["timeline"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["messageId"] == sent["value"]["messageId"]
                    && item["body"] == "ffi message after corrupt row")
        );
    }

    #[test]
    fn runtime_workspace_storage_health_ffi_reports_corrupt_rows() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Storage Health").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        assert_eq!(created["ok"], true);
        let workspace_id = created["value"]["workspaceId"].as_str().unwrap();
        insert_corrupt_event_json(
            tempdir.path(),
            workspace_id,
            "evt_corrupt_ffi_storage_health_tripwire",
        );
        let workspace_id = CString::new(workspace_id).unwrap();

        let health_json = unsafe {
            take_ffi_string(chaft_runtime_workspace_storage_health_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let health = serde_json::from_str::<Value>(&health_json).unwrap();

        assert_eq!(health["ok"], true);
        assert_eq!(health["value"]["totalEventCount"], 3);
        assert_eq!(health["value"]["parseableEventCount"], 2);
        assert_eq!(health["value"]["corruptEventCount"], 1);
        assert_eq!(health["value"]["signatureValidMetadataCount"], 3);
        assert_eq!(health["value"]["servableEventCount"], 2);
        assert_eq!(health["value"]["poisonedServableMetadataCount"], 1);
        assert_eq!(health["value"]["promotableServableMetadataCount"], 0);
        assert_eq!(health["value"]["nonServableParseableEventCount"], 0);

        let repair_json = unsafe {
            take_ffi_string(chaft_runtime_repair_workspace_storage_metadata_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let repair = serde_json::from_str::<Value>(&repair_json).unwrap();
        assert_eq!(repair["ok"], true);
        assert_eq!(repair["value"]["totalEventCount"], 3);
        assert_eq!(repair["value"]["repairedMetadataCount"], 1);
        assert_eq!(repair["value"]["clearedUnservableMetadataCount"], 1);
        assert_eq!(repair["value"]["signatureValidMetadataBeforeCount"], 3);
        assert_eq!(repair["value"]["signatureValidMetadataAfterCount"], 2);

        let repaired_health_json = unsafe {
            take_ffi_string(chaft_runtime_workspace_storage_health_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let repaired_health = serde_json::from_str::<Value>(&repaired_health_json).unwrap();
        assert_eq!(repaired_health["ok"], true);
        assert_eq!(repaired_health["value"]["poisonedServableMetadataCount"], 0);
        assert_eq!(
            repaired_health["value"]["promotableServableMetadataCount"],
            0
        );
        assert_eq!(repaired_health["value"]["corruptEventCount"], 1);
        assert_eq!(repaired_health["value"]["servableEventCount"], 2);
    }

    #[test]
    fn decrypted_snapshot_from_runtime_latest_limits_timeline() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        for body in ["first", "second", "third"] {
            runtime
                .send_message(workspace_id.clone(), channel_id.clone(), body)
                .unwrap();
        }
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(
                chaft_decrypted_workspace_snapshot_from_runtime_latest_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    2,
                ),
            )
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["timeline"].as_array().unwrap().len(), 2);
        assert_eq!(value["value"]["timeline"][0]["body"], "second");
        assert_eq!(value["value"]["timeline"][1]["body"], "third");
        assert_eq!(value["value"]["timelineWindow"]["startIndex"], 1);
        assert_eq!(value["value"]["timelineWindow"]["itemCount"], 2);
        assert_eq!(value["value"]["timelineWindow"]["totalCount"], 3);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreBefore"], true);
    }

    #[test]
    fn decrypted_snapshot_from_runtime_latest_caps_oversized_timeline_limit() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime Cap", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        let message_count = MAX_TIMELINE_WINDOW_ROWS + 2;
        for index in 0..message_count {
            runtime
                .send_message(
                    workspace_id.clone(),
                    channel_id.clone(),
                    format!("message {index:03}"),
                )
                .unwrap();
        }
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(
                chaft_decrypted_workspace_snapshot_from_runtime_latest_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    usize::MAX,
                ),
            )
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();
        let timeline = value["value"]["timeline"].as_array().unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(timeline.len(), MAX_TIMELINE_WINDOW_ROWS);
        assert_eq!(timeline[0]["body"], "message 002");
        assert_eq!(
            timeline
                .last()
                .and_then(|row| row["body"].as_str())
                .unwrap(),
            format!("message {:03}", message_count - 1)
        );
        assert_eq!(value["value"]["timelineWindow"]["startIndex"], 2);
        assert_eq!(
            value["value"]["timelineWindow"]["itemCount"],
            MAX_TIMELINE_WINDOW_ROWS
        );
        assert_eq!(
            value["value"]["timelineWindow"]["totalCount"],
            message_count
        );
        assert_eq!(value["value"]["timelineWindow"]["hasMoreBefore"], true);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreAfter"], false);
    }

    #[test]
    fn decrypted_snapshot_from_runtime_window_loads_requested_page() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Runtime", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let channel_id = ChannelId(created.channel_id);
        for body in ["first", "second", "third", "fourth"] {
            runtime
                .send_message(workspace_id.clone(), channel_id.clone(), body)
                .unwrap();
        }
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let json = unsafe {
            take_ffi_string(
                chaft_decrypted_workspace_snapshot_from_runtime_window_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    1,
                    2,
                ),
            )
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["timeline"].as_array().unwrap().len(), 2);
        assert_eq!(value["value"]["timeline"][0]["body"], "second");
        assert_eq!(value["value"]["timeline"][1]["body"], "third");
        assert_eq!(value["value"]["timelineWindow"]["startIndex"], 1);
        assert_eq!(value["value"]["timelineWindow"]["itemCount"], 2);
        assert_eq!(value["value"]["timelineWindow"]["totalCount"], 4);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreBefore"], true);
        assert_eq!(value["value"]["timelineWindow"]["hasMoreAfter"], true);
    }

    #[test]
    fn decrypted_channel_snapshot_from_runtime_loads_channel_windows() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Channel Runtime", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let general_id = ChannelId(created.channel_id);
        let beta = runtime
            .create_channel(workspace_id.clone(), "beta", false)
            .unwrap();
        let beta_id = ChannelId(beta.channel_id.clone());
        runtime
            .send_message(workspace_id.clone(), general_id, "general first")
            .unwrap();
        for body in ["beta first", "beta second", "beta third"] {
            runtime
                .send_message(workspace_id.clone(), beta_id.clone(), body)
                .unwrap();
        }
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let beta_id = CString::new(beta.channel_id).unwrap();
        let latest_json = unsafe {
            take_ffi_string(
                chaft_decrypted_workspace_channel_snapshot_from_runtime_latest_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    beta_id.as_ptr(),
                    2,
                ),
            )
        };
        let latest = serde_json::from_str::<Value>(&latest_json).unwrap();
        assert_eq!(latest["ok"], true);
        assert_eq!(
            latest["value"]["timelineChannelId"],
            beta_id.as_c_str().to_str().unwrap()
        );
        assert_eq!(latest["value"]["timeline"][0]["body"], "beta second");
        assert_eq!(latest["value"]["timeline"][1]["body"], "beta third");
        assert_eq!(latest["value"]["timelineWindow"]["startIndex"], 1);
        assert_eq!(latest["value"]["timelineWindow"]["totalCount"], 3);
        assert_eq!(latest["value"]["timelineWindow"]["hasMoreBefore"], true);

        let window_json = unsafe {
            take_ffi_string(
                chaft_decrypted_workspace_channel_snapshot_from_runtime_window_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    beta_id.as_ptr(),
                    0,
                    2,
                ),
            )
        };
        let window = serde_json::from_str::<Value>(&window_json).unwrap();
        assert_eq!(window["ok"], true);
        assert_eq!(window["value"]["timeline"][0]["body"], "beta first");
        assert_eq!(window["value"]["timeline"][1]["body"], "beta second");
        assert_eq!(window["value"]["timelineWindow"]["startIndex"], 0);
        assert_eq!(window["value"]["timelineWindow"]["hasMoreAfter"], true);
    }

    #[test]
    fn runtime_action_ffi_creates_workspace_sends_and_decrypts_message() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Actions").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        assert_eq!(created["ok"], true);
        let workspace_id = created["value"]["workspaceId"].as_str().unwrap();
        let channel_id = created["value"]["channelId"].as_str().unwrap();

        let workspace_id_c = CString::new(workspace_id).unwrap();
        let channel_id_c = CString::new(channel_id).unwrap();
        let workspaces_json = unsafe {
            take_ffi_string(chaft_runtime_list_workspaces_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let workspaces = serde_json::from_str::<Value>(&workspaces_json).unwrap();
        assert_eq!(workspaces["ok"], true);
        assert_eq!(workspaces["value"][0]["workspaceId"], workspace_id);
        assert_eq!(workspaces["value"][0]["name"], "Chaft FFI Actions");
        assert_eq!(workspaces["value"][0]["channelCount"], 1);
        assert_eq!(workspaces["value"][0]["memberCount"], 1);
        assert_eq!(workspaces["value"][0]["hasWorkspaceKey"], true);

        let display_name = CString::new("Mira").unwrap();
        let profile_json = unsafe {
            take_ffi_string(chaft_runtime_update_device_profile_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                display_name.as_ptr(),
            ))
        };
        let profile = serde_json::from_str::<Value>(&profile_json).unwrap();
        assert_eq!(profile["ok"], true);
        assert_eq!(profile["value"]["workspaceId"], workspace_id);
        assert_eq!(profile["value"]["displayName"], "Mira");

        let key_package_path = tempdir.path().join("openmls-key-package.bin");
        std::fs::write(&key_package_path, [1_u8, 2, 3, 4]).unwrap();
        let key_package_protocol = CString::new("openmls/key-package").unwrap();
        let key_package_file = CString::new(key_package_path.to_string_lossy().as_bytes()).unwrap();
        let key_package_json = unsafe {
            take_ffi_string(chaft_runtime_publish_device_key_package_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                key_package_protocol.as_ptr(),
                key_package_file.as_ptr(),
            ))
        };
        let key_package = serde_json::from_str::<Value>(&key_package_json).unwrap();
        assert_eq!(key_package["ok"], true);
        assert_eq!(key_package["value"]["workspaceId"], workspace_id);
        assert_eq!(key_package["value"]["protocol"], "openmls/key-package");
        assert_eq!(key_package["value"]["byteLen"], 4);

        let endpoint_id = CString::new("desktop").unwrap();
        let endpoint = CString::new("direct+tcp://127.0.0.1:7777").unwrap();
        let transport = CString::new("direct-tcp").unwrap();
        let peer_endpoint_json = unsafe {
            take_ffi_string(chaft_runtime_publish_peer_endpoint_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                endpoint_id.as_ptr(),
                endpoint.as_ptr(),
                transport.as_ptr(),
                true,
                true,
                1_700_000_600_000,
            ))
        };
        let peer_endpoint = serde_json::from_str::<Value>(&peer_endpoint_json).unwrap();
        assert_eq!(peer_endpoint["ok"], true);
        assert_eq!(peer_endpoint["value"]["workspaceId"], workspace_id);
        assert_eq!(peer_endpoint["value"]["endpointId"], "desktop");
        assert_eq!(
            peer_endpoint["value"]["endpoint"],
            "direct+tcp://127.0.0.1:7777"
        );
        assert_eq!(peer_endpoint["value"]["transport"], "direct-tcp");
        assert_eq!(peer_endpoint["value"]["isBackupPeer"], true);
        assert_eq!(peer_endpoint["value"]["expiresAtMs"], 1_700_000_600_000_i64);

        let openmls_key_package_json = unsafe {
            take_ffi_string(
                chaft_runtime_publish_openmls_device_key_package_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                ),
            )
        };
        let openmls_key_package = serde_json::from_str::<Value>(&openmls_key_package_json).unwrap();
        assert_eq!(openmls_key_package["ok"], true);
        assert_eq!(openmls_key_package["value"]["workspaceId"], workspace_id);
        assert_eq!(
            openmls_key_package["value"]["protocol"],
            "openmls/key-package/rfc9420"
        );
        assert!(openmls_key_package["value"]["keyPackageRef"].is_string());
        let private_bundle_path = openmls_key_package["value"]["privateBundlePath"]
            .as_str()
            .unwrap();
        assert!(std::path::Path::new(private_bundle_path).exists());

        let openmls_group_json = unsafe {
            take_ffi_string(chaft_runtime_create_openmls_workspace_group_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let openmls_group = serde_json::from_str::<Value>(&openmls_group_json).unwrap();
        assert_eq!(openmls_group["ok"], true);
        assert_eq!(openmls_group["value"]["workspaceId"], workspace_id);
        assert_eq!(
            openmls_group["value"]["protocol"],
            "openmls/workspace-group/rfc9420"
        );
        assert_eq!(openmls_group["value"]["epoch"], 0);
        assert_eq!(openmls_group["value"]["memberCount"], 1);
        let private_group_state_path = openmls_group["value"]["privateGroupStatePath"]
            .as_str()
            .unwrap();
        assert!(std::path::Path::new(private_group_state_path).exists());

        let private_channel_name = CString::new("strategy").unwrap();
        let private_channel_json = unsafe {
            take_ffi_string(chaft_runtime_create_channel_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                private_channel_name.as_ptr(),
                true,
            ))
        };
        let private_channel = serde_json::from_str::<Value>(&private_channel_json).unwrap();
        assert_eq!(private_channel["ok"], true);

        let device_json = unsafe {
            take_ffi_string(chaft_runtime_device_id_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let device = serde_json::from_str::<Value>(&device_json).unwrap();
        let device_id = CString::new(device["value"]["deviceId"].as_str().unwrap()).unwrap();
        let private_channel_id =
            CString::new(private_channel["value"]["channelId"].as_str().unwrap()).unwrap();
        let channel_member_json = unsafe {
            take_ffi_string(chaft_runtime_add_channel_member_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                private_channel_id.as_ptr(),
                device_id.as_ptr(),
            ))
        };
        let channel_member = serde_json::from_str::<Value>(&channel_member_json).unwrap();
        assert_eq!(channel_member["ok"], true);
        assert_eq!(
            channel_member["value"]["channelId"],
            private_channel["value"]["channelId"]
        );

        let rotated_channel_json = unsafe {
            take_ffi_string(chaft_runtime_rotate_channel_key_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                private_channel_id.as_ptr(),
            ))
        };
        let rotated_channel = serde_json::from_str::<Value>(&rotated_channel_json).unwrap();
        assert_eq!(rotated_channel["ok"], true);
        assert_eq!(rotated_channel["value"]["workspaceId"], workspace_id);
        assert_eq!(
            rotated_channel["value"]["channelId"],
            private_channel["value"]["channelId"]
        );
        assert_eq!(rotated_channel["value"]["epoch"], 2);

        let text = CString::new("ffi action plaintext").unwrap();
        let sent_json = unsafe {
            take_ffi_string(chaft_runtime_send_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
                text.as_ptr(),
            ))
        };
        let sent = serde_json::from_str::<Value>(&sent_json).unwrap();
        assert_eq!(sent["ok"], true);
        assert_eq!(sent["value"]["encrypted"], true);

        let rotated_workspace_json = unsafe {
            take_ffi_string(chaft_runtime_rotate_workspace_key_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let rotated_workspace = serde_json::from_str::<Value>(&rotated_workspace_json).unwrap();
        assert_eq!(rotated_workspace["ok"], true);
        assert_eq!(rotated_workspace["value"]["workspaceId"], workspace_id);
        assert_eq!(rotated_workspace["value"]["epoch"], 2);
        assert!(rotated_workspace["value"]["previousKeyId"].is_string());
        assert!(rotated_workspace["value"]["keyId"].is_string());

        let rotated_manual_json = unsafe {
            take_ffi_string(chaft_runtime_rotate_workspace_manual_keys_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let rotated_manual = serde_json::from_str::<Value>(&rotated_manual_json).unwrap();
        assert_eq!(rotated_manual["ok"], true);
        assert_eq!(rotated_manual["value"]["workspaceId"], workspace_id);
        assert_eq!(rotated_manual["value"]["workspaceKeyRotation"]["epoch"], 3);
        assert_eq!(
            rotated_manual["value"]["channelKeyRotations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            rotated_manual["value"]["channelKeyRotations"][0]["channelId"],
            private_channel["value"]["channelId"]
        );
        assert_eq!(
            rotated_manual["value"]["channelKeyRotations"][0]["epoch"],
            3
        );
        assert_eq!(
            rotated_manual["value"]["rotatedEventIds"][0],
            rotated_manual["value"]["workspaceKeyRotation"]["eventId"]
        );
        assert_eq!(
            rotated_manual["value"]["rotatedEventIds"][1],
            rotated_manual["value"]["channelKeyRotations"][0]["eventId"]
        );

        let compromise_rotation_json = unsafe {
            take_ffi_string(
                chaft_runtime_rotate_workspace_for_suspected_compromise_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                ),
            )
        };
        let compromise_rotation = serde_json::from_str::<Value>(&compromise_rotation_json).unwrap();
        assert_eq!(compromise_rotation["ok"], true);
        assert_eq!(compromise_rotation["value"]["workspaceId"], workspace_id);
        assert_eq!(
            compromise_rotation["value"]["openmlsUpdates"]["workspaceUpdate"]["epoch"],
            1
        );
        assert_eq!(
            compromise_rotation["value"]["openmlsUpdates"]["updatedEventIds"][0],
            compromise_rotation["value"]["openmlsUpdates"]["workspaceUpdate"]["eventId"]
        );
        assert_eq!(
            compromise_rotation["value"]["manualKeyRotation"]["workspaceKeyRotation"]["epoch"],
            4
        );
        assert_eq!(
            compromise_rotation["value"]["rotatedEventIds"][0],
            compromise_rotation["value"]["openmlsUpdates"]["workspaceUpdate"]["eventId"]
        );
        assert_eq!(
            compromise_rotation["value"]["rotatedEventIds"][1],
            compromise_rotation["value"]["manualKeyRotation"]["workspaceKeyRotation"]["eventId"]
        );

        let trust_snapshot_json = unsafe {
            take_ffi_string(chaft_runtime_export_trust_snapshot_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let trust_snapshot = serde_json::from_str::<Value>(&trust_snapshot_json).unwrap();
        assert_eq!(trust_snapshot["ok"], true);
        assert_eq!(
            trust_snapshot["value"]["snapshot"]["workspace_id"],
            workspace_id
        );
        assert_eq!(
            trust_snapshot["value"]["snapshot"]["root_event_id"],
            created["value"]["workspaceEventId"]
        );
        assert_eq!(
            trust_snapshot["value"]["root_event"]["event_id"],
            created["value"]["workspaceEventId"]
        );

        let message_id = CString::new(sent["value"]["messageId"].as_str().unwrap()).unwrap();
        let reaction_c = CString::new("+1").unwrap();
        let reaction_json = unsafe {
            take_ffi_string(chaft_runtime_add_reaction_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                message_id.as_ptr(),
                reaction_c.as_ptr(),
            ))
        };
        let reaction = serde_json::from_str::<Value>(&reaction_json).unwrap();
        assert_eq!(reaction["ok"], true);
        assert_eq!(reaction["value"]["reaction"], "+1");
        assert_eq!(reaction["value"]["messageId"], sent["value"]["messageId"]);

        let marked_json = unsafe {
            take_ffi_string(chaft_runtime_mark_channel_read_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
            ))
        };
        let marked = serde_json::from_str::<Value>(&marked_json).unwrap();
        assert_eq!(marked["ok"], true);
        assert_eq!(marked["value"]["channelId"], channel_id);
        assert_eq!(
            marked["value"]["readThroughEventId"],
            sent["value"]["eventId"]
        );
        assert_eq!(marked["value"]["alreadyRead"], false);
        assert!(marked["value"]["markerEventId"].is_string());

        let already_marked_json = unsafe {
            take_ffi_string(chaft_runtime_mark_channel_read_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
            ))
        };
        let already_marked = serde_json::from_str::<Value>(&already_marked_json).unwrap();
        assert_eq!(already_marked["ok"], true);
        assert_eq!(already_marked["value"]["alreadyRead"], true);
        assert_eq!(already_marked["value"]["markerEventId"], Value::Null);

        let reindexed_json = unsafe {
            take_ffi_string(chaft_runtime_reindex_workspace_search_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let reindexed = serde_json::from_str::<Value>(&reindexed_json).unwrap();
        assert_eq!(reindexed["ok"], true);
        assert_eq!(reindexed["value"]["workspaceId"], workspace_id);
        assert_eq!(reindexed["value"]["indexedMessageCount"], 1);

        let query = CString::new("action").unwrap();
        let search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                query.as_ptr(),
            ))
        };
        let search = serde_json::from_str::<Value>(&search_json).unwrap();
        assert_eq!(search["ok"], true);
        assert_eq!(search["value"]["workspaceId"], workspace_id);
        assert_eq!(search["value"]["query"], "action");
        assert_eq!(search["value"]["itemCount"], 1);
        assert_eq!(search["value"]["hitCount"], 1);
        assert_eq!(search["value"]["rawCandidateCount"], 1);
        assert!(
            search["value"]["rawCandidateLimit"]
                .as_u64()
                .is_some_and(|limit| limit >= 1)
        );
        assert!(
            search["value"]["visibleHitLimit"]
                .as_u64()
                .is_some_and(|limit| limit >= 1)
        );
        assert_eq!(search["value"]["hasMoreHits"], false);
        assert_eq!(search["value"]["hits"].as_array().unwrap().len(), 1);
        assert_eq!(search["value"]["hits"][0]["body"], "ffi action plaintext");
        assert_eq!(
            search["value"]["hits"][0]["bodyCharCount"],
            "ffi action plaintext".chars().count()
        );
        assert_eq!(search["value"]["hits"][0]["bodyTruncated"], false);
        assert_eq!(search["value"]["hits"][0]["channelId"], channel_id);
        assert_eq!(search["value"]["hits"][0]["channelName"], "general");
        assert_eq!(
            search["value"]["hits"][0]["authorDeviceId"],
            profile["value"]["deviceId"]
        );
        assert_eq!(search["value"]["hits"][0]["authorDisplayName"], "Mira");
        assert!(
            search["value"]["hits"][0]["physicalMs"]
                .as_i64()
                .is_some_and(|physical_ms| physical_ms > 0)
        );
        assert_eq!(
            search["value"]["hits"][0]["eventId"],
            sent["value"]["eventId"]
        );

        let oversized_query = CString::new("q".repeat(600)).unwrap();
        let oversized_search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                oversized_query.as_ptr(),
            ))
        };
        let oversized_search = serde_json::from_str::<Value>(&oversized_search_json).unwrap();
        assert_eq!(oversized_search["ok"], false);
        assert_eq!(oversized_search["error"]["code"], "search_query_too_large");
        assert!(
            oversized_search["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("search query is too large"))
        );

        let edited_text = CString::new("ffi edited plaintext").unwrap();
        let edited_json = unsafe {
            take_ffi_string(chaft_runtime_edit_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                message_id.as_ptr(),
                edited_text.as_ptr(),
            ))
        };
        let edited = serde_json::from_str::<Value>(&edited_json).unwrap();
        assert_eq!(edited["ok"], true);
        assert_eq!(edited["value"]["messageId"], sent["value"]["messageId"]);
        assert_eq!(edited["value"]["encrypted"], true);

        let edited_query = CString::new("edited").unwrap();
        let edited_search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                edited_query.as_ptr(),
            ))
        };
        let edited_search = serde_json::from_str::<Value>(&edited_search_json).unwrap();
        assert_eq!(edited_search["ok"], true);
        assert_eq!(
            edited_search["value"]["hits"][0]["body"],
            "ffi edited plaintext"
        );
        assert_eq!(edited_search["value"]["hits"][0]["channelName"], "general");

        let snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let snapshot = serde_json::from_str::<Value>(&snapshot_json).unwrap();
        assert_eq!(snapshot["ok"], true);
        assert_eq!(
            snapshot["value"]["timeline"][0]["body"],
            "ffi edited plaintext"
        );
        assert_eq!(
            snapshot["value"]["timeline"][0]["authorDisplayName"],
            "Mira"
        );
        assert_eq!(snapshot["value"]["profiles"][0]["displayName"], "Mira");
        assert_eq!(
            snapshot["value"]["keyPackages"][0]["keyPackageId"],
            key_package["value"]["keyPackageId"]
        );
        assert_eq!(
            snapshot["value"]["keyPackages"][0]["byteLen"],
            key_package["value"]["byteLen"]
        );
        assert_eq!(
            snapshot["value"]["peerEndpoints"][0]["endpointId"],
            peer_endpoint["value"]["endpointId"]
        );
        assert_eq!(
            snapshot["value"]["peerEndpoints"][0]["endpoint"],
            peer_endpoint["value"]["endpoint"]
        );
        assert_eq!(snapshot["value"]["peerEndpoints"][0]["isBackupPeer"], true);
        assert_eq!(snapshot["value"]["timeline"][0]["reactions"]["+1"], 1);
        assert_eq!(
            snapshot["value"]["timeline"][0]["myReactions"],
            serde_json::json!(["+1"])
        );
        assert_eq!(snapshot["value"]["timeline"][0]["encrypted"], true);

        let removed_reaction_json = unsafe {
            take_ffi_string(chaft_runtime_remove_reaction_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                message_id.as_ptr(),
                reaction_c.as_ptr(),
            ))
        };
        let removed_reaction = serde_json::from_str::<Value>(&removed_reaction_json).unwrap();
        assert_eq!(removed_reaction["ok"], true);
        assert_eq!(removed_reaction["value"]["reaction"], "+1");
        let snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let snapshot = serde_json::from_str::<Value>(&snapshot_json).unwrap();
        assert_eq!(snapshot["ok"], true);
        assert_eq!(
            snapshot["value"]["timeline"][0]["reactions"]["+1"],
            Value::Null
        );
        assert_eq!(
            snapshot["value"]["timeline"][0]["myReactions"],
            serde_json::json!([])
        );

        let deleted_json = unsafe {
            take_ffi_string(chaft_runtime_delete_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                message_id.as_ptr(),
            ))
        };
        let deleted = serde_json::from_str::<Value>(&deleted_json).unwrap();
        assert_eq!(deleted["ok"], true);
        assert_eq!(deleted["value"]["messageId"], sent["value"]["messageId"]);

        let deleted_search_json = unsafe {
            take_ffi_string(chaft_runtime_search_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                edited_query.as_ptr(),
            ))
        };
        let deleted_search = serde_json::from_str::<Value>(&deleted_search_json).unwrap();
        assert_eq!(deleted_search["value"]["hits"].as_array().unwrap().len(), 0);

        let deleted_snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let deleted_snapshot = serde_json::from_str::<Value>(&deleted_snapshot_json).unwrap();
        assert_eq!(
            deleted_snapshot["value"]["timeline"][0]["body"],
            "Message deleted"
        );
        assert_eq!(deleted_snapshot["value"]["timeline"][0]["deleted"], true);

        let store = EventStore::open(tempdir.path().join("events.db")).unwrap();
        let events_json = serde_json::to_string(
            &store
                .list_events_for_workspace(snapshot["value"]["workspaceId"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(!events_json.contains("ffi action plaintext"));
        assert!(!events_json.contains("ffi edited plaintext"));
    }

    #[test]
    fn runtime_publish_peer_endpoint_ffi_rejects_invalid_hint_policy_before_append() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Endpoint Policy", "general")
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id.clone()).unwrap();
        let endpoint_id = CString::new("desktop").unwrap();
        let unsupported_endpoint = CString::new("relay://relay.example.invalid/device").unwrap();
        let unsupported_transport = CString::new("iroh-relay").unwrap();
        let mismatched_endpoint = CString::new("direct+tcp://127.0.0.1:7777").unwrap();
        let mismatched_transport = CString::new("iroh").unwrap();
        let before_event_count = EventStore::open(tempdir.path().join("events.db"))
            .unwrap()
            .list_events()
            .unwrap()
            .len();

        let unsupported_json = unsafe {
            take_ffi_string(chaft_runtime_publish_peer_endpoint_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                endpoint_id.as_ptr(),
                unsupported_endpoint.as_ptr(),
                unsupported_transport.as_ptr(),
                true,
                false,
                0,
            ))
        };
        let unsupported = serde_json::from_str::<Value>(&unsupported_json).unwrap();
        assert_eq!(unsupported["ok"], false);
        assert_eq!(unsupported["error"]["code"], "peer_endpoint_unsupported");

        let mismatched_json = unsafe {
            take_ffi_string(chaft_runtime_publish_peer_endpoint_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                endpoint_id.as_ptr(),
                mismatched_endpoint.as_ptr(),
                mismatched_transport.as_ptr(),
                true,
                false,
                0,
            ))
        };
        let mismatched = serde_json::from_str::<Value>(&mismatched_json).unwrap();
        assert_eq!(mismatched["ok"], false);
        assert_eq!(
            mismatched["error"]["code"],
            "peer_endpoint_transport_mismatch"
        );

        let after_event_count = EventStore::open(tempdir.path().join("events.db"))
            .unwrap()
            .list_events()
            .unwrap()
            .len();
        assert_eq!(after_event_count, before_event_count);
    }

    #[test]
    fn runtime_action_ffi_rejects_oversized_device_key_package_file_before_publish() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Key Package Limits").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        let workspace_id = created["value"]["workspaceId"].as_str().unwrap();
        let workspace_id_c = CString::new(workspace_id).unwrap();
        let protocol = CString::new("openmls/key-package").unwrap();
        let key_package_path = tempdir.path().join("oversized-key-package.bin");
        let key_package_file = std::fs::File::create(&key_package_path).unwrap();
        key_package_file
            .set_len(DEVICE_KEY_PACKAGE_FILE_MAX_BYTES + 1)
            .unwrap();
        drop(key_package_file);
        let key_package_file_c =
            CString::new(key_package_path.to_string_lossy().as_bytes()).unwrap();

        let published_json = unsafe {
            take_ffi_string(chaft_runtime_publish_device_key_package_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                protocol.as_ptr(),
                key_package_file_c.as_ptr(),
            ))
        };
        let published = serde_json::from_str::<Value>(&published_json).unwrap();
        let store = EventStore::open(tempdir.path().join("events.db")).unwrap();
        let events = store.list_events_for_workspace(workspace_id).unwrap();

        assert_eq!(published["ok"], false);
        assert_eq!(
            published["error"]["code"],
            "runtime_publish_device_key_package_failed"
        );
        assert!(
            published["error"]["message"]
                .as_str()
                .unwrap()
                .contains("device key package is too large")
        );
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn runtime_action_ffi_rejects_oversized_key_and_recovery_import_json_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let oversized_key_json = CString::new("x".repeat(KEY_TRANSFER_JSON_MAX_BYTES + 1)).unwrap();
        let oversized_recovery_json =
            CString::new("x".repeat(RECOVERY_BUNDLE_JSON_MAX_BYTES + 1)).unwrap();
        let passphrase = CString::new("correct horse battery staple").unwrap();

        let workspace_key_json = unsafe {
            take_ffi_string(chaft_runtime_import_workspace_key_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                oversized_key_json.as_ptr(),
            ))
        };
        let workspace_key = serde_json::from_str::<Value>(&workspace_key_json).unwrap();
        let channel_key_json = unsafe {
            take_ffi_string(chaft_runtime_import_channel_key_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                oversized_key_json.as_ptr(),
            ))
        };
        let channel_key = serde_json::from_str::<Value>(&channel_key_json).unwrap();
        let recovery_json = unsafe {
            take_ffi_string(chaft_runtime_import_recovery_bundle_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                oversized_recovery_json.as_ptr(),
                passphrase.as_ptr(),
            ))
        };
        let recovery = serde_json::from_str::<Value>(&recovery_json).unwrap();

        assert_eq!(workspace_key["ok"], false);
        assert_eq!(
            workspace_key["error"]["code"],
            "workspace_key_json_too_large"
        );
        assert!(
            workspace_key["error"]["message"]
                .as_str()
                .unwrap()
                .contains("workspace key JSON is too large")
        );
        assert_eq!(channel_key["ok"], false);
        assert_eq!(channel_key["error"]["code"], "channel_key_json_too_large");
        assert!(
            channel_key["error"]["message"]
                .as_str()
                .unwrap()
                .contains("channel key JSON is too large")
        );
        assert_eq!(recovery["ok"], false);
        assert_eq!(recovery["error"]["code"], "recovery_bundle_json_too_large");
        assert!(
            recovery["error"]["message"]
                .as_str()
                .unwrap()
                .contains("recovery bundle JSON is too large")
        );
    }

    #[test]
    fn runtime_action_ffi_sends_reply_and_projects_context() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Replies").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        assert_eq!(created["ok"], true);

        let workspace_id = CString::new(created["value"]["workspaceId"].as_str().unwrap()).unwrap();
        let channel_id = CString::new(created["value"]["channelId"].as_str().unwrap()).unwrap();
        let parent_text = CString::new("ffi parent body").unwrap();
        let parent_json = unsafe {
            take_ffi_string(chaft_runtime_send_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                channel_id.as_ptr(),
                parent_text.as_ptr(),
            ))
        };
        let parent = serde_json::from_str::<Value>(&parent_json).unwrap();
        assert_eq!(parent["ok"], true);

        let reply_to = CString::new(parent["value"]["messageId"].as_str().unwrap()).unwrap();
        let reply_text = CString::new("ffi reply body").unwrap();
        let reply_json = unsafe {
            take_ffi_string(chaft_runtime_send_message_reply_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                channel_id.as_ptr(),
                reply_to.as_ptr(),
                reply_text.as_ptr(),
            ))
        };
        let reply = serde_json::from_str::<Value>(&reply_json).unwrap();
        assert_eq!(reply["ok"], true);
        assert_eq!(
            reply["value"]["replyToMessageId"],
            parent["value"]["messageId"]
        );

        let snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
            ))
        };
        let snapshot = serde_json::from_str::<Value>(&snapshot_json).unwrap();
        assert_eq!(snapshot["ok"], true);
        assert_eq!(
            snapshot["value"]["timeline"][1]["replyToMessageId"],
            parent["value"]["messageId"]
        );
        assert_eq!(
            snapshot["value"]["timeline"][1]["replyPreview"]["body"],
            "ffi parent body"
        );
        assert_eq!(snapshot["value"]["timeline"][0]["threadReplyCount"], 1);
        assert_eq!(
            snapshot["value"]["timeline"][0]["threadLatestReply"]["body"],
            "ffi reply body"
        );
        assert_eq!(
            snapshot["value"]["timeline"][0]["threadReplyPreviews"][0]["body"],
            "ffi reply body"
        );
    }

    #[test]
    fn runtime_openmls_member_add_ffi_round_trips_welcome() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let bob_dir_c = CString::new(bob_dir.path().to_string_lossy().as_bytes()).unwrap();
        let created;
        let bob_device_id;

        {
            let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
            let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
            created = alice
                .create_workspace("Chaft FFI OpenMLS", "general")
                .unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            bob_device_id = bob.device_id().0.clone();
            alice
                .invite_member(
                    workspace_id.clone(),
                    bob.device_id().clone(),
                    WorkspaceRole::Member,
                )
                .unwrap();
            let bob_store = EventStore::open(bob_dir.path().join("events.db")).unwrap();
            for event in alice.workspace_events(&workspace_id).unwrap() {
                bob_store.append_event(&event).unwrap();
            }
        }

        let workspace_id_c = CString::new(created.workspace_id.as_str()).unwrap();
        let bob_package_json = unsafe {
            take_ffi_string(
                chaft_runtime_publish_openmls_device_key_package_result_json(
                    bob_dir_c.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                ),
            )
        };
        let bob_package = serde_json::from_str::<Value>(&bob_package_json).unwrap();
        assert_eq!(bob_package["ok"], true);
        let key_package_id_c =
            CString::new(bob_package["value"]["keyPackageId"].as_str().unwrap()).unwrap();

        {
            let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
            for event in bob.workspace_events(&workspace_id).unwrap() {
                alice_store.append_event(&event).unwrap();
            }
        }

        let group_json = unsafe {
            take_ffi_string(chaft_runtime_create_openmls_workspace_group_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let group = serde_json::from_str::<Value>(&group_json).unwrap();
        assert_eq!(group["ok"], true);
        assert_eq!(group["value"]["memberCount"], 1);

        let added_json = unsafe {
            take_ffi_string(
                chaft_runtime_add_openmls_workspace_group_member_result_json(
                    alice_dir_c.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                    key_package_id_c.as_ptr(),
                ),
            )
        };
        let added = serde_json::from_str::<Value>(&added_json).unwrap();
        assert_eq!(added["ok"], true);
        assert_eq!(added["value"]["inviteeDeviceId"], bob_device_id);
        assert_eq!(added["value"]["epoch"], 1);
        assert_eq!(added["value"]["memberCount"], 2);
        assert!(added["value"]["welcomeByteLen"].as_u64().unwrap() > 0);
        let source_event_id_c = CString::new(added["value"]["eventId"].as_str().unwrap()).unwrap();

        {
            let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            let bob_store = EventStore::open(bob_dir.path().join("events.db")).unwrap();
            for event in alice.workspace_events(&workspace_id).unwrap() {
                bob_store.append_event(&event).unwrap();
            }
        }

        let joined_json = unsafe {
            take_ffi_string(chaft_runtime_join_openmls_workspace_group_result_json(
                bob_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                source_event_id_c.as_ptr(),
            ))
        };
        let joined = serde_json::from_str::<Value>(&joined_json).unwrap();
        assert_eq!(joined["ok"], true);
        assert_eq!(joined["value"]["deviceId"], bob_device_id);
        assert_eq!(joined["value"]["sourceEventId"], added["value"]["eventId"]);
        assert_eq!(joined["value"]["groupId"], added["value"]["groupId"]);
        assert_eq!(joined["value"]["epoch"], 1);
        assert_eq!(joined["value"]["memberCount"], 2);
    }

    #[test]
    fn runtime_openmls_channel_group_ffi_round_trips_welcome() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let bob_dir_c = CString::new(bob_dir.path().to_string_lossy().as_bytes()).unwrap();
        let created;
        let private_channel;
        let bob_device_id;

        {
            let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
            let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
            created = alice
                .create_workspace("Chaft FFI OpenMLS Channel", "general")
                .unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            private_channel = alice
                .create_channel(workspace_id.clone(), "strategy", true)
                .unwrap();
            bob_device_id = bob.device_id().0.clone();
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
                    ChannelId(private_channel.channel_id.clone()),
                    bob.device_id().clone(),
                )
                .unwrap();
            let bob_store = EventStore::open(bob_dir.path().join("events.db")).unwrap();
            for event in alice.workspace_events(&workspace_id).unwrap() {
                bob_store.append_event(&event).unwrap();
            }
        }

        let workspace_id_c = CString::new(created.workspace_id.as_str()).unwrap();
        let channel_id_c = CString::new(private_channel.channel_id.as_str()).unwrap();
        let bob_package_json = unsafe {
            take_ffi_string(
                chaft_runtime_publish_openmls_device_key_package_result_json(
                    bob_dir_c.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                ),
            )
        };
        let bob_package = serde_json::from_str::<Value>(&bob_package_json).unwrap();
        assert_eq!(bob_package["ok"], true);
        let key_package_id_c =
            CString::new(bob_package["value"]["keyPackageId"].as_str().unwrap()).unwrap();

        {
            let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            let alice_store = EventStore::open(alice_dir.path().join("events.db")).unwrap();
            for event in bob.workspace_events(&workspace_id).unwrap() {
                alice_store.append_event(&event).unwrap();
            }
        }

        let group_json = unsafe {
            take_ffi_string(chaft_runtime_create_openmls_channel_group_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
            ))
        };
        let group = serde_json::from_str::<Value>(&group_json).unwrap();
        assert_eq!(group["ok"], true);
        assert_eq!(group["value"]["channelId"], private_channel.channel_id);
        assert_eq!(group["value"]["memberCount"], 1);

        let added_json = unsafe {
            take_ffi_string(chaft_runtime_add_openmls_channel_group_member_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
                key_package_id_c.as_ptr(),
            ))
        };
        let added = serde_json::from_str::<Value>(&added_json).unwrap();
        assert_eq!(added["ok"], true);
        assert_eq!(added["value"]["channelId"], private_channel.channel_id);
        assert_eq!(added["value"]["inviteeDeviceId"], bob_device_id);
        assert_eq!(added["value"]["epoch"], 1);
        assert_eq!(added["value"]["memberCount"], 2);
        assert!(added["value"]["welcomeByteLen"].as_u64().unwrap() > 0);
        let source_event_id_c = CString::new(added["value"]["eventId"].as_str().unwrap()).unwrap();

        {
            let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
            let workspace_id = WorkspaceId(created.workspace_id.clone());
            let bob_store = EventStore::open(bob_dir.path().join("events.db")).unwrap();
            for event in alice.workspace_events(&workspace_id).unwrap() {
                bob_store.append_event(&event).unwrap();
            }
        }

        let joined_json = unsafe {
            take_ffi_string(chaft_runtime_join_openmls_channel_group_result_json(
                bob_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
                source_event_id_c.as_ptr(),
            ))
        };
        let joined = serde_json::from_str::<Value>(&joined_json).unwrap();
        assert_eq!(joined["ok"], true);
        assert_eq!(joined["value"]["channelId"], private_channel.channel_id);
        assert_eq!(joined["value"]["deviceId"], bob_device_id);
        assert_eq!(joined["value"]["sourceEventId"], added["value"]["eventId"]);
        assert_eq!(joined["value"]["groupId"], added["value"]["groupId"]);
        assert_eq!(joined["value"]["epoch"], 1);
        assert_eq!(joined["value"]["memberCount"], 2);
    }

    #[test]
    fn runtime_update_workspace_openmls_groups_ffi_updates_workspace_and_channels() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir =
            CString::new(tempdir.path().join("runtime").to_string_lossy().as_bytes()).unwrap();
        let runtime = LocalRuntime::open(tempdir.path().join("runtime"), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI OpenMLS Rotation", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let private_channel = runtime
            .create_channel(workspace_id.clone(), "strategy", true)
            .unwrap();
        let private_channel_id = ChannelId(private_channel.channel_id.clone());
        runtime
            .create_openmls_workspace_group(workspace_id.clone())
            .unwrap();
        runtime
            .create_openmls_channel_group(workspace_id.clone(), private_channel_id)
            .unwrap();
        let workspace_id_c = CString::new(created.workspace_id.as_str()).unwrap();

        let updated_json = unsafe {
            take_ffi_string(chaft_runtime_update_workspace_openmls_groups_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let updated = serde_json::from_str::<Value>(&updated_json).unwrap();

        assert_eq!(updated["ok"], true);
        assert_eq!(updated["value"]["workspaceId"], created.workspace_id);
        assert_eq!(updated["value"]["workspaceUpdate"]["epoch"], 1);
        assert_eq!(
            updated["value"]["channelUpdates"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            updated["value"]["channelUpdates"][0]["channelId"],
            private_channel.channel_id
        );
        assert_eq!(updated["value"]["channelUpdates"][0]["epoch"], 1);
        assert_eq!(
            updated["value"]["updatedEventIds"][0],
            updated["value"]["workspaceUpdate"]["eventId"]
        );
        assert_eq!(
            updated["value"]["updatedEventIds"][1],
            updated["value"]["channelUpdates"][0]["eventId"]
        );
    }

    #[test]
    fn runtime_attachment_ffi_sends_encrypted_file_metadata() {
        const ATTACHMENT_TEXT: &str = "ffi attachment plaintext";
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("brief.txt");
        std::fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let data_dir =
            CString::new(tempdir.path().join("runtime").to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Attachments").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        let workspace_id = created["value"]["workspaceId"].as_str().unwrap();
        let channel_id = created["value"]["channelId"].as_str().unwrap();
        let workspace_id_c = CString::new(workspace_id).unwrap();
        let channel_id_c = CString::new(channel_id).unwrap();
        let text = CString::new("see attachment").unwrap();
        let file_path = CString::new(attachment_path.to_string_lossy().as_bytes()).unwrap();
        let media_type = CString::new("text/plain").unwrap();

        let sent_json = unsafe {
            take_ffi_string(chaft_runtime_send_attachment_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                channel_id_c.as_ptr(),
                text.as_ptr(),
                file_path.as_ptr(),
                media_type.as_ptr(),
            ))
        };
        let sent = serde_json::from_str::<Value>(&sent_json).unwrap();
        assert_eq!(sent["ok"], true);
        assert_eq!(sent["value"]["attachmentCount"], 1);

        let snapshot_json = unsafe {
            take_ffi_string(chaft_decrypted_workspace_snapshot_from_runtime_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
            ))
        };
        let snapshot = serde_json::from_str::<Value>(&snapshot_json).unwrap();
        assert_eq!(
            snapshot["value"]["timeline"][0]["attachments"][0]["displayName"],
            "brief.txt"
        );
        assert_eq!(
            snapshot["value"]["timeline"][0]["attachments"][0]["mediaType"],
            "text/plain"
        );
        assert_eq!(
            snapshot["value"]["timeline"][0]["attachments"][0]["encrypted"],
            true
        );
        let message_id = sent["value"]["messageId"].as_str().unwrap().to_owned();
        let attachment_id = snapshot["value"]["timeline"][0]["attachments"][0]["attachmentId"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(attachment_id.starts_with("att_"));
        let blob_hash = snapshot["value"]["timeline"][0]["attachments"][0]["blobHash"]
            .as_str()
            .unwrap()
            .to_owned();
        let output_path = tempdir.path().join("saved-brief.txt");
        let message_id_c = CString::new(message_id).unwrap();
        let attachment_id_c = CString::new(attachment_id.as_str()).unwrap();
        let output_path_c = CString::new(output_path.to_string_lossy().as_bytes()).unwrap();
        let saved_json = unsafe {
            take_ffi_string(chaft_runtime_save_attachment_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                message_id_c.as_ptr(),
                attachment_id_c.as_ptr(),
                output_path_c.as_ptr(),
            ))
        };
        let saved = serde_json::from_str::<Value>(&saved_json).unwrap();
        assert_eq!(saved["ok"], true);
        assert_eq!(saved["value"]["workspaceId"], workspace_id);
        assert_eq!(saved["value"]["blobHash"], blob_hash);
        assert_eq!(saved["value"]["attachmentId"], attachment_id);
        assert_eq!(saved["value"]["displayName"], "brief.txt");
        assert_eq!(
            std::fs::read_to_string(&output_path).unwrap(),
            ATTACHMENT_TEXT
        );

        let blob_store = BlobStore::open(tempdir.path().join("runtime").join("blobs")).unwrap();
        let orphan = blob_store.put_bytes(b"ffi orphan ciphertext").unwrap();
        let pruned_json = unsafe {
            take_ffi_string(chaft_runtime_prune_blobs_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
            ))
        };
        let pruned = serde_json::from_str::<Value>(&pruned_json).unwrap();
        let referenced = pruned["value"]["referencedBlobHashes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        let removed = pruned["value"]["removedBlobHashes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(pruned["ok"], true);
        assert_eq!(pruned["value"]["workspaceCount"], 1);
        assert_eq!(pruned["value"]["referencedBlobCount"], 1);
        assert_eq!(pruned["value"]["removedBlobCount"], 1);
        assert_eq!(pruned["value"]["removedManifestCount"], 0);
        assert_eq!(pruned["value"]["removedChunkCount"], 0);
        assert!(referenced.contains(&blob_hash.as_str()));
        assert_eq!(removed, vec![orphan.hash.as_str()]);
        assert!(blob_store.has_blob(&blob_hash).unwrap());
        assert!(!blob_store.has_blob(&orphan.hash).unwrap());

        let store = EventStore::open(tempdir.path().join("runtime").join("events.db")).unwrap();
        let events_json = serde_json::to_string(
            &store
                .list_events_for_workspace(snapshot["value"]["workspaceId"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(!events_json.contains(ATTACHMENT_TEXT));
    }

    #[test]
    fn runtime_attachment_ffi_rejects_oversized_file() {
        const ATTACHMENT_FILE_MAX_BYTES: u64 = 128 * 1024 * 1024;
        let tempdir = tempfile::tempdir().unwrap();
        let attachment_path = tempdir.path().join("too-large.bin");
        let attachment_file = std::fs::File::create(&attachment_path).unwrap();
        attachment_file
            .set_len(ATTACHMENT_FILE_MAX_BYTES + 1)
            .unwrap();
        drop(attachment_file);
        let data_dir =
            CString::new(tempdir.path().join("runtime").to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Attachment Limits").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        let workspace_id = CString::new(created["value"]["workspaceId"].as_str().unwrap()).unwrap();
        let channel_id = CString::new(created["value"]["channelId"].as_str().unwrap()).unwrap();
        let text = CString::new("oversized attachment").unwrap();
        let file_path = CString::new(attachment_path.to_string_lossy().as_bytes()).unwrap();
        let media_type = CString::new("application/octet-stream").unwrap();

        let sent_json = unsafe {
            take_ffi_string(chaft_runtime_send_attachment_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                channel_id.as_ptr(),
                text.as_ptr(),
                file_path.as_ptr(),
                media_type.as_ptr(),
            ))
        };
        let sent = serde_json::from_str::<Value>(&sent_json).unwrap();

        assert_eq!(sent["ok"], false);
        assert_eq!(sent["error"]["code"], "runtime_send_attachment_failed");
        assert!(
            sent["error"]["message"]
                .as_str()
                .unwrap()
                .contains("attachment file is too large")
        );
    }

    #[test]
    fn runtime_direct_peer_ffi_hosts_runtime_store_and_blobs() {
        const ATTACHMENT_TEXT: &str = "hosted peer attachment plaintext";
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("hosted.txt");
        std::fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Hosted Peer", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
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
                ChannelId(created.channel_id.clone()),
                "hosted attachment",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        drop(alice);

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let listen = CString::new("127.0.0.1:0").unwrap();
        let started_json = unsafe {
            take_ffi_string(chaft_runtime_start_direct_peer_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                listen.as_ptr(),
            ))
        };
        let started = serde_json::from_str::<Value>(&started_json).unwrap();
        assert_eq!(started["ok"], true);
        let peer_id = started["value"]["peerId"].as_str().unwrap().to_owned();
        let endpoint = started["value"]["endpoint"].as_str().unwrap().to_owned();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let pulled = runtime
            .block_on(bob.pull_workspace_direct(
                &DirectTransport,
                &PeerAddress {
                    peer_id: PeerId(endpoint.clone()),
                    endpoint: endpoint.clone(),
                },
                workspace_id.clone(),
            ))
            .unwrap();
        assert_eq!(pulled.fetched_event_ids.len(), 4);
        assert_eq!(pulled.fetched_blob_hashes.len(), 1);

        bob.import_workspace_key(exported_key).unwrap();
        let saved_path = bob_dir.path().join("saved-hosted.txt");
        bob.save_attachment_to_file(
            workspace_id,
            MessageId(sent.message_id),
            &pulled.fetched_blob_hashes[0],
            &saved_path,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&saved_path).unwrap(),
            ATTACHMENT_TEXT
        );

        let peer_id_c = CString::new(peer_id).unwrap();
        let stopped_json = unsafe {
            take_ffi_string(chaft_runtime_stop_direct_peer_result_json(
                peer_id_c.as_ptr(),
            ))
        };
        let stopped = serde_json::from_str::<Value>(&stopped_json).unwrap();
        assert_eq!(stopped["ok"], true);
        assert_eq!(stopped["value"]["endpoint"], endpoint);
    }

    #[test]
    fn runtime_iroh_peer_ffi_hosts_runtime_store_and_blobs() {
        const ATTACHMENT_TEXT: &str = "hosted iroh peer attachment plaintext";
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let attachment_path = alice_dir.path().join("hosted-iroh.txt");
        std::fs::write(&attachment_path, ATTACHMENT_TEXT).unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Iroh Hosted Peer", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
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
                ChannelId(created.channel_id.clone()),
                "hosted iroh attachment",
                &attachment_path,
                "text/plain",
            )
            .unwrap();
        let exported_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        drop(alice);

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let started_json = unsafe {
            take_ffi_string(chaft_runtime_start_iroh_peer_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
            ))
        };
        let started = serde_json::from_str::<Value>(&started_json).unwrap();
        assert_eq!(started["ok"], true);
        let peer_id = started["value"]["peerId"].as_str().unwrap().to_owned();
        let endpoint = started["value"]["endpoint"].as_str().unwrap().to_owned();
        assert!(endpoint.starts_with("iroh://"));

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let transport = IrohTransport::default();
        let pulled = runtime
            .block_on(bob.pull_workspace_direct(
                &transport,
                &PeerAddress {
                    peer_id: PeerId(endpoint.clone()),
                    endpoint: endpoint.clone(),
                },
                workspace_id.clone(),
            ))
            .unwrap();
        assert_eq!(pulled.fetched_event_ids.len(), 4);
        assert_eq!(pulled.fetched_blob_hashes.len(), 1);

        bob.import_workspace_key(exported_key).unwrap();
        let saved_path = bob_dir.path().join("saved-hosted-iroh.txt");
        bob.save_attachment_to_file(
            workspace_id,
            MessageId(sent.message_id),
            &pulled.fetched_blob_hashes[0],
            &saved_path,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&saved_path).unwrap(),
            ATTACHMENT_TEXT
        );

        let peer_id_c = CString::new(peer_id).unwrap();
        let stopped_json = unsafe {
            take_ffi_string(chaft_runtime_stop_direct_peer_result_json(
                peer_id_c.as_ptr(),
            ))
        };
        let stopped = serde_json::from_str::<Value>(&stopped_json).unwrap();
        assert_eq!(stopped["ok"], true);
        assert_eq!(stopped["value"]["endpoint"], endpoint);
    }

    #[test]
    fn runtime_direct_network_ffi_publishes_and_pulls_workspace() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Chaft FFI Sync", "general").unwrap();
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
                workspace_id,
                ChannelId(created.channel_id),
                "ffi network plaintext",
            )
            .unwrap();
        drop(alice);
        drop(bob);

        let (endpoint_tx, endpoint_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let node_store_path = node_dir.path().join("events.db");
        let server_thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let node_store = EventStore::open(&node_store_path).unwrap();
                let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
                    .await
                    .unwrap();
                endpoint_tx
                    .send(server.local_addr().unwrap().to_string())
                    .unwrap();
                server.serve_until_shutdown(shutdown_rx).await.unwrap();
            });
        });
        let endpoint = format!(
            "direct+tcp://{}",
            endpoint_rx.recv_timeout(Duration::from_secs(5)).unwrap()
        );

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let bob_dir_c = CString::new(bob_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let endpoint_c = CString::new(endpoint).unwrap();

        let published_json = unsafe {
            take_ffi_string(chaft_runtime_publish_workspace_direct_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                endpoint_c.as_ptr(),
            ))
        };
        let published = serde_json::from_str::<Value>(&published_json).unwrap();
        assert_eq!(published["ok"], true);
        assert_eq!(published["value"]["workspaceId"], created.workspace_id);
        assert_eq!(published["value"]["publishedEventCount"], 4);
        assert_eq!(
            published["value"]["publishedEventIds"]
                .as_array()
                .unwrap()
                .len(),
            4
        );

        let pulled_json = unsafe {
            take_ffi_string(chaft_runtime_pull_workspace_direct_result_json(
                bob_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                endpoint_c.as_ptr(),
            ))
        };
        let pulled = serde_json::from_str::<Value>(&pulled_json).unwrap();
        assert_eq!(pulled["ok"], true);
        assert_eq!(pulled["value"]["workspaceId"], created.workspace_id);
        assert_eq!(pulled["value"]["fetchedEventCount"], 4);
        assert_eq!(
            pulled["value"]["fetchedEventIds"].as_array().unwrap().len(),
            4
        );
        assert_eq!(pulled["value"]["gapCount"], 0);
        assert!(pulled["value"]["gaps"].as_array().unwrap().is_empty());

        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let snapshot = bob
            .workspace_snapshot(WorkspaceId(created.workspace_id))
            .unwrap();
        assert_eq!(snapshot.name, "Chaft FFI Sync");
        assert_eq!(snapshot.channels[0].name, "general");
        assert_eq!(snapshot.timeline[0].body, "Encrypted message");

        shutdown_tx.send(()).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn runtime_direct_network_ffi_classifies_peer_protocol_error() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Protocol Error", "general")
            .unwrap();
        drop(runtime);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let server_thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut len = [0u8; 4];
            stream.read_exact(&mut len).unwrap();
            let request_len = u32::from_be_bytes(len) as usize;
            let mut request = vec![0; request_len];
            stream.read_exact(&mut request).unwrap();
            stream
                .write_all(&((chaft_net_direct::MAX_FRAME_LEN + 1) as u32).to_be_bytes())
                .unwrap();
        });

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let endpoint = CString::new(endpoint).unwrap();
        let json = unsafe {
            take_ffi_string(chaft_runtime_pull_workspace_direct_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                endpoint.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "runtime_peer_protocol_failed");
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("frame length")
        );
        server_thread.join().unwrap();
    }

    #[test]
    fn runtime_direct_network_ffi_publishes_event_with_trust_snapshot() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Partial Publish", "general")
            .unwrap();
        let sent = alice
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id),
                "ffi proof publish plaintext",
            )
            .unwrap();
        let sent_event_id = sent.event_id.clone();
        drop(alice);

        let (endpoint_tx, endpoint_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let node_store_path = node_dir.path().join("events.db");
        let node_store_for_assert = node_store_path.clone();
        let server_thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let node_store = EventStore::open(&node_store_path).unwrap();
                let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
                    .await
                    .unwrap();
                endpoint_tx
                    .send(server.local_addr().unwrap().to_string())
                    .unwrap();
                server.serve_until_shutdown(shutdown_rx).await.unwrap();
            });
        });
        let endpoint = endpoint_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let event_id_c = CString::new(sent_event_id.clone()).unwrap();
        let endpoint_c = CString::new(endpoint).unwrap();
        let published_json = unsafe {
            take_ffi_string(
                chaft_runtime_publish_event_with_trust_snapshot_direct_result_json(
                    alice_dir_c.as_ptr(),
                    std::ptr::null(),
                    workspace_id_c.as_ptr(),
                    event_id_c.as_ptr(),
                    endpoint_c.as_ptr(),
                ),
            )
        };
        let published = serde_json::from_str::<Value>(&published_json).unwrap();
        assert_eq!(published["ok"], true);
        assert_eq!(published["value"]["workspaceId"], created.workspace_id);
        assert_eq!(
            published["value"]["publishedEventIds"][0],
            Value::String(sent_event_id.clone())
        );

        shutdown_tx.send(()).unwrap();
        server_thread.join().unwrap();

        let node_store = EventStore::open(node_store_for_assert).unwrap();
        let node_events = node_store
            .list_events_for_workspace(&created.workspace_id)
            .unwrap();
        assert_eq!(node_events.len(), 1);
        assert_eq!(node_events[0].event_id.0, sent_event_id);
    }

    #[test]
    fn runtime_direct_network_ffi_backs_up_workspace_content_slices() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Partial Backup", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());
        let sent = alice
            .send_message(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "ffi backup slice plaintext",
            )
            .unwrap();
        let reaction = alice
            .add_reaction(workspace_id, MessageId(sent.message_id.clone()), "+1")
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
        let openmls_updates = alice.update_workspace_openmls_groups(workspace_id).unwrap();
        let sent_event_id = sent.event_id.clone();
        let reaction_event_id = reaction.event_id.clone();
        let expected_event_ids = vec![
            sent_event_id.clone(),
            reaction_event_id.clone(),
            key_package.event_id.clone(),
            openmls_updates.updated_event_ids[0].clone(),
            openmls_updates.updated_event_ids[1].clone(),
        ];
        drop(alice);

        let (endpoint_tx, endpoint_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let node_store_path = node_dir.path().join("events.db");
        let node_store_for_assert = node_store_path.clone();
        let server_thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let node_store = EventStore::open(&node_store_path).unwrap();
                let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
                    .await
                    .unwrap();
                endpoint_tx
                    .send(server.local_addr().unwrap().to_string())
                    .unwrap();
                server.serve_until_shutdown(shutdown_rx).await.unwrap();
            });
        });
        let endpoint = endpoint_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let endpoint_c = CString::new(endpoint).unwrap();
        let backed_up_json = unsafe {
            take_ffi_string(chaft_runtime_backup_workspace_direct_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                endpoint_c.as_ptr(),
            ))
        };
        let backed_up = serde_json::from_str::<Value>(&backed_up_json).unwrap();
        assert_eq!(backed_up["ok"], true);
        assert_eq!(backed_up["value"]["workspaceId"], created.workspace_id);
        assert_eq!(
            backed_up["value"]["publishedEventIds"].as_array().unwrap(),
            &expected_event_ids
                .iter()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>()
        );

        shutdown_tx.send(()).unwrap();
        server_thread.join().unwrap();

        let node_store = EventStore::open(node_store_for_assert).unwrap();
        let node_events = node_store
            .list_events_for_workspace(&created.workspace_id)
            .unwrap();
        assert_eq!(
            node_events
                .into_iter()
                .map(|event| event.event_id.0)
                .collect::<Vec<_>>(),
            expected_event_ids
        );
    }

    #[test]
    fn runtime_direct_network_ffi_retries_blob_transfer_ledger() {
        let alice_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Blob Retry", "general")
            .unwrap();
        drop(alice);

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let peers_c = CString::new("127.0.0.1:7777;127.0.0.1:7778").unwrap();
        let retried_json = unsafe {
            take_ffi_string(chaft_runtime_retry_blob_transfers_direct_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                peers_c.as_ptr(),
            ))
        };
        let retried = serde_json::from_str::<Value>(&retried_json).unwrap();
        assert_eq!(retried["ok"], true);
        assert_eq!(retried["value"]["workspaceId"], created.workspace_id);
        assert_eq!(retried["value"]["pendingAttemptCount"], 0);
        assert!(
            retried["value"]["pendingAttemptIds"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(retried["value"]["blobTransferAttemptCount"], 0);
        assert!(
            retried["value"]["blobTransferAttempts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn runtime_direct_network_ffi_deduplicates_retry_peer_endpoints_before_limit() {
        let alice_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Retry Dedupe", "general")
            .unwrap();
        drop(alice);

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let repeated_peer_list = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|_| "127.0.0.1:7777")
            .collect::<Vec<_>>()
            .join(";");
        let repeated_peer_list = CString::new(repeated_peer_list).unwrap();
        let retried_json = unsafe {
            take_ffi_string(chaft_runtime_retry_blob_transfers_direct_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                repeated_peer_list.as_ptr(),
            ))
        };
        let retried = serde_json::from_str::<Value>(&retried_json).unwrap();

        assert_eq!(retried["ok"], true);
        assert_eq!(retried["value"]["workspaceId"], created.workspace_id);
        assert_eq!(retried["value"]["pendingAttemptCount"], 0);
    }

    #[test]
    fn runtime_direct_network_ffi_rejects_oversized_peer_endpoint_inputs() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Endpoint Limit", "general")
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let oversized_endpoint = CString::new("e".repeat(PEER_ENDPOINT_MAX_BYTES + 1)).unwrap();
        let published_json = unsafe {
            take_ffi_string(chaft_runtime_publish_workspace_direct_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                oversized_endpoint.as_ptr(),
            ))
        };
        let published = serde_json::from_str::<Value>(&published_json).unwrap();
        assert_eq!(published["ok"], false);
        assert_eq!(published["error"]["code"], "peer_endpoint_too_large");
        assert!(
            published["error"]["message"]
                .as_str()
                .unwrap()
                .contains("peer endpoint is too large")
        );

        let peer_list = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| format!("direct+tcp://127.0.0.1:{}", 10_000 + index))
            .collect::<Vec<_>>()
            .join(";");
        let peer_list = CString::new(peer_list).unwrap();
        let retried_json = unsafe {
            take_ffi_string(chaft_runtime_retry_blob_transfers_direct_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                peer_list.as_ptr(),
            ))
        };
        let retried = serde_json::from_str::<Value>(&retried_json).unwrap();
        assert_eq!(retried["ok"], false);
        assert_eq!(retried["error"]["code"], "peer_endpoint_list_too_large");

        let oversized_listen = CString::new("l".repeat(PEER_ENDPOINT_MAX_BYTES + 1)).unwrap();
        let started_json = unsafe {
            take_ffi_string(chaft_runtime_start_direct_peer_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                oversized_listen.as_ptr(),
            ))
        };
        let started = serde_json::from_str::<Value>(&started_json).unwrap();
        assert_eq!(started["ok"], false);
        assert_eq!(started["error"]["code"], "peer_endpoint_too_large");
    }

    #[test]
    fn runtime_direct_network_ffi_rejects_unsupported_peer_endpoint_inputs() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Chaft FFI Endpoint Policy", "general")
            .unwrap();
        drop(runtime);

        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let unsupported_endpoint = CString::new("https://central.example.invalid/sync").unwrap();
        let published_json = unsafe {
            take_ffi_string(chaft_runtime_publish_workspace_direct_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                unsupported_endpoint.as_ptr(),
            ))
        };
        let published = serde_json::from_str::<Value>(&published_json).unwrap();

        assert_eq!(published["ok"], false);
        assert_eq!(published["error"]["code"], "peer_endpoint_unsupported");
        assert!(
            published["error"]["message"]
                .as_str()
                .unwrap()
                .contains("direct TCP or native Iroh direct route")
        );
    }

    #[test]
    fn runtime_direct_network_ffi_rejects_unsupported_peer_before_runtime_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let unsupported_endpoint = CString::new("https://central.example.invalid/sync").unwrap();
        let workspace_id = CString::new("wrk_reject_before_open").unwrap();
        let event_id = CString::new(format!(
            "evt_{}",
            "0".repeat(chaft_types::EVENT_ID_HASH_HEX_BYTES)
        ))
        .unwrap();

        let calls: Vec<Box<dyn Fn(*const c_char) -> String>> = vec![
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_publish_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    unsupported_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_backup_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    unsupported_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(
                    chaft_runtime_publish_event_with_trust_snapshot_direct_result_json(
                        data_dir,
                        std::ptr::null(),
                        workspace_id.as_ptr(),
                        event_id.as_ptr(),
                        unsupported_endpoint.as_ptr(),
                    ),
                )
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_pull_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    unsupported_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_sync_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    unsupported_endpoint.as_ptr(),
                ))
            }),
        ];

        for (index, call) in calls.into_iter().enumerate() {
            let data_path = tempdir.path().join(format!("missing-runtime-{index}"));
            let data_dir = CString::new(data_path.to_string_lossy().as_bytes()).unwrap();
            let json = call(data_dir.as_ptr());
            let value = serde_json::from_str::<Value>(&json).unwrap();

            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "peer_endpoint_unsupported");
            assert!(
                value["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("direct TCP or native Iroh direct route")
            );
            assert!(
                !data_path.exists(),
                "unsupported peer endpoint should be rejected before runtime open"
            );
        }
    }

    #[test]
    fn runtime_direct_network_ffi_rejects_blank_workspace_before_runtime_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let workspace_id = CString::new("   ").unwrap();
        let event_id = CString::new(format!(
            "evt_{}",
            "0".repeat(chaft_types::EVENT_ID_HASH_HEX_BYTES)
        ))
        .unwrap();
        let peer_endpoint = CString::new("direct+tcp://127.0.0.1:1").unwrap();

        let calls: Vec<Box<dyn Fn(*const c_char) -> String>> = vec![
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_publish_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_backup_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(
                    chaft_runtime_publish_event_with_trust_snapshot_direct_result_json(
                        data_dir,
                        std::ptr::null(),
                        workspace_id.as_ptr(),
                        event_id.as_ptr(),
                        peer_endpoint.as_ptr(),
                    ),
                )
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_pull_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_sync_workspace_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ))
            }),
            Box::new(|data_dir| unsafe {
                take_ffi_string(chaft_runtime_retry_blob_transfers_direct_result_json(
                    data_dir,
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ))
            }),
        ];

        for (index, call) in calls.into_iter().enumerate() {
            let data_path = tempdir
                .path()
                .join(format!("missing-runtime-blank-{index}"));
            let data_dir = CString::new(data_path.to_string_lossy().as_bytes()).unwrap();
            let json = call(data_dir.as_ptr());
            let value = serde_json::from_str::<Value>(&json).unwrap();

            assert_eq!(value["ok"], false);
            assert_eq!(value["error"]["code"], "workspace_id_required");
            assert!(
                !data_path.exists(),
                "blank workspace ID should be rejected before runtime open"
            );
        }
    }

    #[test]
    fn runtime_direct_network_ffi_rejects_noncanonical_event_before_runtime_open() {
        let tempdir = tempfile::tempdir().unwrap();
        let workspace_id = CString::new("wrk_reject_before_open").unwrap();
        let event_id = CString::new("evt_NOT_CANONICAL").unwrap();
        let peer_endpoint = CString::new("direct+tcp://127.0.0.1:1").unwrap();
        let data_path = tempdir.path().join("missing-runtime-noncanonical-event");
        let data_dir = CString::new(data_path.to_string_lossy().as_bytes()).unwrap();

        let json = unsafe {
            take_ffi_string(
                chaft_runtime_publish_event_with_trust_snapshot_direct_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    event_id.as_ptr(),
                    peer_endpoint.as_ptr(),
                ),
            )
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "event_id_not_canonical");
        assert!(
            !data_path.exists(),
            "non-canonical event ID should be rejected before runtime open"
        );
    }

    #[test]
    fn runtime_direct_peer_ffi_rejects_invalid_listen_endpoint_before_runtime_open() {
        let data_file = tempfile::NamedTempFile::new().unwrap();
        let data_dir = CString::new(data_file.path().to_string_lossy().as_bytes()).unwrap();
        let unsupported_listen = CString::new("https://central.example.invalid/listen").unwrap();
        let started_json = unsafe {
            take_ffi_string(chaft_runtime_start_direct_peer_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                unsupported_listen.as_ptr(),
            ))
        };
        let started = serde_json::from_str::<Value>(&started_json).unwrap();

        assert_eq!(started["ok"], false);
        assert_eq!(started["error"]["code"], "peer_endpoint_unsupported");
        assert!(
            started["error"]["message"]
                .as_str()
                .unwrap()
                .contains("direct listen endpoint must be host:port")
        );
    }

    #[test]
    fn runtime_direct_network_ffi_syncs_workspace() {
        let alice_dir = tempfile::tempdir().unwrap();
        let node_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Chaft FFI Full Sync", "general")
            .unwrap();
        alice
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id),
                "ffi sync plaintext",
            )
            .unwrap();
        drop(alice);

        let (endpoint_tx, endpoint_rx) = mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let node_store_path = node_dir.path().join("events.db");
        let server_thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let node_store = EventStore::open(&node_store_path).unwrap();
                let server = DirectPeerServer::bind("127.0.0.1:0", node_store)
                    .await
                    .unwrap();
                endpoint_tx
                    .send(server.local_addr().unwrap().to_string())
                    .unwrap();
                server.serve_until_shutdown(shutdown_rx).await.unwrap();
            });
        });
        let endpoint = endpoint_rx.recv_timeout(Duration::from_secs(5)).unwrap();

        let alice_dir_c = CString::new(alice_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id_c = CString::new(created.workspace_id.clone()).unwrap();
        let endpoint_c = CString::new(endpoint).unwrap();
        let synced_json = unsafe {
            take_ffi_string(chaft_runtime_sync_workspace_direct_result_json(
                alice_dir_c.as_ptr(),
                std::ptr::null(),
                workspace_id_c.as_ptr(),
                endpoint_c.as_ptr(),
            ))
        };
        let synced = serde_json::from_str::<Value>(&synced_json).unwrap();
        assert_eq!(synced["ok"], true);
        assert_eq!(synced["value"]["workspaceId"], created.workspace_id);
        assert_eq!(synced["value"]["published"]["publishedEventCount"], 3);
        assert_eq!(
            synced["value"]["published"]["publishedEventIds"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(synced["value"]["pulled"]["fetchedEventCount"], 0);
        assert_eq!(
            synced["value"]["pulled"]["fetchedEventIds"]
                .as_array()
                .unwrap()
                .len(),
            0
        );

        shutdown_tx.send(()).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn runtime_action_ffi_reports_authorization_errors() {
        let alice_dir = tempfile::tempdir().unwrap();
        let bob_dir = tempfile::tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Chaft", "general").unwrap();
        let exported = alice
            .export_workspace_key(WorkspaceId(created.workspace_id.clone()))
            .unwrap();
        bob.import_workspace_key(exported).unwrap();
        drop(alice);
        drop(bob);

        let bob_dir = CString::new(bob_dir.path().to_string_lossy().as_bytes()).unwrap();
        let workspace_id = CString::new(created.workspace_id).unwrap();
        let channel_id = CString::new(created.channel_id).unwrap();
        let text = CString::new("should fail").unwrap();
        let json = unsafe {
            take_ffi_string(chaft_runtime_send_message_result_json(
                bob_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                channel_id.as_ptr(),
                text.as_ptr(),
            ))
        };
        let value = serde_json::from_str::<Value>(&json).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "runtime_send_message_failed");
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("no local events")
        );
    }

    #[test]
    fn runtime_action_ffi_rejects_oversized_message_markdown() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = CString::new(tempdir.path().to_string_lossy().as_bytes()).unwrap();
        let name = CString::new("Chaft FFI Message Limit").unwrap();
        let channel_name = CString::new("general").unwrap();
        let created_json = unsafe {
            take_ffi_string(chaft_runtime_create_workspace_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                name.as_ptr(),
                channel_name.as_ptr(),
            ))
        };
        let created = serde_json::from_str::<Value>(&created_json).unwrap();
        let workspace_id = CString::new(created["value"]["workspaceId"].as_str().unwrap()).unwrap();
        let channel_id = CString::new(created["value"]["channelId"].as_str().unwrap()).unwrap();
        let oversized_text = CString::new("x".repeat(70 * 1024)).unwrap();

        let sent_json = unsafe {
            take_ffi_string(chaft_runtime_send_message_result_json(
                data_dir.as_ptr(),
                std::ptr::null(),
                workspace_id.as_ptr(),
                channel_id.as_ptr(),
                oversized_text.as_ptr(),
            ))
        };
        let sent = serde_json::from_str::<Value>(&sent_json).unwrap();

        assert_eq!(sent["ok"], false);
        assert_eq!(sent["error"]["code"], "message_markdown_too_large");
        assert!(
            sent["error"]["message"]
                .as_str()
                .unwrap()
                .contains("message markdown is too large")
        );
    }
}
