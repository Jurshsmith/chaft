use std::ffi::{CString, c_char};

#[cfg(test)]
use chaft_app::MAX_TIMELINE_WINDOW_ROWS;
#[cfg(test)]
use chaft_media::BlobStore;
#[cfg(test)]
use chaft_net::{PeerAddress, PeerId};
#[cfg(test)]
use chaft_net_direct::DirectTransport;
#[cfg(test)]
use chaft_net_iroh::IrohTransport;
#[cfg(test)]
use chaft_runtime::LocalRuntime;
#[cfg(test)]
use chaft_runtime::PEER_ENDPOINT_LIST_MAX_ITEMS;
#[cfg(test)]
use chaft_runtime::PEER_ENDPOINT_MAX_BYTES;
#[cfg(test)]
use chaft_runtime::PulledOpenMlsCatchup;
#[cfg(test)]
use chaft_runtime::{
    AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits,
    BlobTransferRetryReport, ImportedWorkspaceRecoveryBundle, PrunedBlobCache, PublishedWorkspace,
    PulledWorkspace, RemovedMemberWithKeyRotation, RotatedChannelKey, RotatedWorkspaceKey,
    RotatedWorkspaceManualKeys, SyncedWorkspace, UpdatedOpenMlsChannelGroup,
    UpdatedOpenMlsWorkspaceGroup, UpdatedWorkspaceOpenMlsGroups, WorkspaceCompromiseReport,
    WorkspaceCompromiseResponse,
};
#[cfg(test)]
use chaft_types::WorkspaceId;
#[cfg(test)]
use chaft_types::{
    ATTACHMENT_ID_MAX_BYTES, ATTACHMENT_MEDIA_TYPE_MAX_BYTES, CHANNEL_ID_MAX_BYTES,
    CHANNEL_NAME_MAX_BYTES, DEVICE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_ID_MAX_BYTES,
    DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, EVENT_ID_MAX_BYTES, MESSAGE_ID_MAX_BYTES,
    MESSAGE_MARKDOWN_MAX_BYTES, PEER_ENDPOINT_TRANSPORT_MAX_BYTES, REACTION_TEXT_MAX_BYTES,
    WORKSPACE_ID_MAX_BYTES, WORKSPACE_NAME_MAX_BYTES,
};
#[cfg(test)]
use chaft_types::{SignedEvent, WorkspaceRole};

mod direct_network;
mod envelope;
mod id_args;
mod identity_passphrase;
mod input;
mod join_request_inbox;
mod join_request_outbox;
mod join_response_inbox;
mod join_response_outbox;
mod peer_endpoint;
mod peer_host;
mod portable_export;
mod result_sampling;
mod runtime_actions;
mod runtime_direct;
mod runtime_open;
mod runtime_peer;
mod runtime_query;
mod runtime_security;
mod snapshot;
mod worker;

use envelope::into_c_string;
#[cfg(test)]
use id_args::*;
#[cfg(test)]
use identity_passphrase::env_identity_passphrase_is_usable;
#[cfg(test)]
use input::read_c_string;
#[cfg(test)]
use input::read_c_string_with_max_bytes;
#[cfg(test)]
use input::{
    FFI_GENERIC_STRING_MAX_BYTES, FFI_PASSPHRASE_MAX_BYTES, FFI_PATH_MAX_BYTES,
    KEY_TRANSFER_JSON_MAX_BYTES, RECOVERY_BUNDLE_JSON_MAX_BYTES, SEARCH_QUERY_MAX_BYTES,
    WORKSPACE_EVENTS_JSON_MAX_BYTES, WORKSPACE_ROLE_TEXT_MAX_BYTES,
};
use join_request_inbox::*;
use join_request_outbox::*;
use join_response_inbox::*;
use join_response_outbox::*;
use portable_export::export_portable_workspace_archive_result;
#[cfg(test)]
use result_sampling::*;
use runtime_actions::*;
use runtime_direct::*;
use runtime_open::{
    clear_runtime_identity_passphrase_result, set_runtime_identity_passphrase_result,
};
pub(crate) use runtime_open::{open_runtime_from_ffi, open_runtime_from_paths};
use runtime_peer::*;
use runtime_query::*;
use runtime_security::*;
use snapshot::{
    decrypted_workspace_channel_snapshot_from_runtime_latest_result,
    decrypted_workspace_channel_snapshot_from_runtime_window_result,
    decrypted_workspace_snapshot_from_runtime_latest_result,
    decrypted_workspace_snapshot_from_runtime_result,
    decrypted_workspace_snapshot_from_runtime_window_result, demo_workspace_snapshot,
    workspace_snapshot_from_events_result, workspace_snapshot_from_store_latest_result,
    workspace_snapshot_from_store_result, workspace_snapshot_from_store_window_result,
};

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

/// Creates a workspace and default channel with an explicit access policy.
///
/// `access_policy` accepts `invite_only`, `request_access`, or `discoverable`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_workspace_with_access_policy_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    name: *const c_char,
    default_channel_name: *const c_char,
    access_policy: *const c_char,
) -> *mut c_char {
    let result = runtime_create_workspace_with_access_policy_result(
        data_dir,
        identity_file,
        name,
        default_channel_name,
        access_policy,
    );
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

/// Creates a direct-message channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_direct_message_channel_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    name: *const c_char,
    participant_device_id: *const c_char,
) -> *mut c_char {
    let result = runtime_create_direct_message_channel_result(
        data_dir,
        identity_file,
        workspace_id,
        name,
        participant_device_id,
    );
    into_c_string(&result)
}

/// Updates signed channel name/topic metadata in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file`, `name`, and `topic`
/// may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_channel_details_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    name: *const c_char,
    topic: *const c_char,
) -> *mut c_char {
    let result = runtime_update_channel_details_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        name,
        topic,
    );
    into_c_string(&result)
}

/// Archives or restores a signed channel in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_channel_archive_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    archived: bool,
) -> *mut c_char {
    let result = runtime_update_channel_archive_result(
        data_dir,
        identity_file,
        workspace_id,
        channel_id,
        archived,
    );
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

/// Atomically updates this device's signed display name and avatar selection.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_device_profile_with_avatar_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    avatar_id: *const c_char,
) -> *mut c_char {
    let result = runtime_update_device_profile_with_avatar_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
        avatar_id,
    );
    into_c_string(&result)
}

/// Updates this device's signed person profile in a local workspace.
///
/// If the local device has not yet linked itself to a person in this workspace,
/// the runtime creates that self-link before writing the profile update.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_local_person_profile_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
) -> *mut c_char {
    let result = runtime_update_local_person_profile_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
    );
    into_c_string(&result)
}

/// Atomically updates this device's linked person display name and avatar selection.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_local_person_profile_with_avatar_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    avatar_id: *const c_char,
) -> *mut c_char {
    let result = runtime_update_local_person_profile_with_avatar_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
        avatar_id,
    );
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
        replica_storage_class: std::ptr::null(),
        replica_retention_hint: std::ptr::null(),
    });
    into_c_string(&result)
}

/// Publishes a signed peer endpoint hint with optional replica capability
/// metadata for this workspace member.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file`,
/// `replica_storage_class`, and `replica_retention_hint` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_publish_peer_endpoint_with_replica_capability_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    endpoint_id: *const c_char,
    endpoint: *const c_char,
    transport: *const c_char,
    is_backup_peer: bool,
    has_expires_at_ms: bool,
    expires_at_ms: i64,
    replica_storage_class: *const c_char,
    replica_retention_hint: *const c_char,
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
        replica_storage_class,
        replica_retention_hint,
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

/// Reconciles this device's OpenMLS access after workspace events have changed.
///
/// The operation is idempotent: it replenishes device key packages, joins and
/// advances available groups, and provisions eligible private-room members.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_reconcile_openmls_access_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> *mut c_char {
    let result = runtime_reconcile_openmls_access_result(data_dir, identity_file, workspace_id);
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

/// Writes a decrypted, portable workspace archive directly to `output_path`.
///
/// The returned string is a compact JSON result envelope containing export
/// metadata, counts, completeness warnings, the archive byte length, and its
/// SHA-256 digest. Archive bytes are never copied through the FFI boundary.
/// The caller owns the returned string and must release it with
/// `chaft_string_free`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_export_portable_workspace_archive(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    output_path: *const c_char,
) -> *mut c_char {
    let result = export_portable_workspace_archive_result(
        data_dir,
        identity_file,
        workspace_id,
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

/// Creates a one-time, claimable workspace invite.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_workspace_invite_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    expires_at: *const c_char,
    peer_endpoint: *const c_char,
    sync_expectation: *const c_char,
) -> *mut c_char {
    let result = runtime_create_workspace_invite_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
        role,
        expires_at,
        peer_endpoint,
        sync_expectation,
    );
    into_c_string(&result)
}

/// Creates a claimable workspace invite with a bounded claim capacity.
///
/// `max_claims` is capped by the runtime. A value of zero uses the safe
/// one-claim default.
///
/// # Safety
///
/// All non-null pointer arguments must be valid pointers to NUL-terminated
/// UTF-8 strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_create_workspace_invite_with_max_claims_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    max_claims: u32,
    expires_at: *const c_char,
    peer_endpoint: *const c_char,
    sync_expectation: *const c_char,
) -> *mut c_char {
    let result = runtime_create_workspace_invite_with_max_claims_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
        role,
        max_claims,
        expires_at,
        peer_endpoint,
        sync_expectation,
    );
    into_c_string(&result)
}

/// Prepares a device-bound claim for a claimable workspace invite.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_prepare_workspace_invite_claim_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    artifact_json: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    response_peer_endpoint: *const c_char,
) -> *mut c_char {
    let result = runtime_prepare_workspace_invite_claim_result(
        data_dir,
        identity_file,
        artifact_json,
        display_name,
        note,
        response_peer_endpoint,
    );
    into_c_string(&result)
}

/// Claims a workspace invite and creates its encrypted response.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_claim_workspace_invite_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    claim_json: *const c_char,
) -> *mut c_char {
    let result = runtime_claim_workspace_invite_result(data_dir, identity_file, claim_json);
    into_c_string(&result)
}

/// Imports the encrypted response for a previously claimed workspace invite.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_import_workspace_invite_response_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    response_json: *const c_char,
) -> *mut c_char {
    let result =
        runtime_import_workspace_invite_response_result(data_dir, identity_file, response_json);
    into_c_string(&result)
}

/// Records a workspace join request in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_record_workspace_join_request_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    source_type: *const c_char,
    source_invite_id: *const c_char,
    source_display_name: *const c_char,
    source_approval_policy: *const c_char,
) -> *mut c_char {
    let result = runtime_record_workspace_join_request_result(
        data_dir,
        identity_file,
        workspace_id,
        request_id,
        device_id,
        display_name,
        note,
        source_type,
        source_invite_id,
        source_display_name,
        source_approval_policy,
    );
    into_c_string(&result)
}

/// Records a workspace join request with an optional requester response route.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and
/// `response_peer_endpoint` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_record_workspace_join_request_with_response_route_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    source_type: *const c_char,
    source_invite_id: *const c_char,
    source_display_name: *const c_char,
    source_approval_policy: *const c_char,
    response_peer_endpoint: *const c_char,
) -> *mut c_char {
    let result = runtime_record_workspace_join_request_with_response_route_result(
        data_dir,
        identity_file,
        workspace_id,
        request_id,
        device_id,
        display_name,
        note,
        source_type,
        source_invite_id,
        source_display_name,
        source_approval_policy,
        response_peer_endpoint,
    );
    into_c_string(&result)
}

/// Records a workspace invite handoff in a local runtime.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` and `request_id` may
/// be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_record_workspace_invite_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    invite_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    request_id: *const c_char,
    expires_at: *const c_char,
    approval_policy: *const c_char,
    sync_expectation: *const c_char,
) -> *mut c_char {
    let result = runtime_record_workspace_invite_result(
        data_dir,
        identity_file,
        workspace_id,
        invite_id,
        device_id,
        display_name,
        role,
        request_id,
        expires_at,
        approval_policy,
        sync_expectation,
    );
    into_c_string(&result)
}

/// Resolves a workspace invite handoff in a local runtime.
///
/// `resolution` accepts `revoked`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_resolve_workspace_invite_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    invite_id: *const c_char,
    resolution: *const c_char,
) -> *mut c_char {
    let result = runtime_resolve_workspace_invite_result(
        data_dir,
        identity_file,
        workspace_id,
        invite_id,
        resolution,
    );
    into_c_string(&result)
}

/// Resolves a workspace join request in a local runtime.
///
/// `resolution` accepts `approved`, `declined`, or `revoked`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_resolve_workspace_join_request_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    resolution: *const c_char,
) -> *mut c_char {
    let result = runtime_resolve_workspace_join_request_result(
        data_dir,
        identity_file,
        workspace_id,
        request_id,
        resolution,
    );
    into_c_string(&result)
}

/// Updates a workspace member's role in a local runtime.
///
/// `role` accepts `owner`, `admin`, `member`, or `guest`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_member_role_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
    role: *const c_char,
) -> *mut c_char {
    let result =
        runtime_update_member_role_result(data_dir, identity_file, workspace_id, device_id, role);
    into_c_string(&result)
}

/// Updates a workspace's access policy in a local runtime.
///
/// `access_policy` accepts `invite_only`, `request_access`, or `discoverable`.
///
/// # Safety
///
/// All non-null arguments must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call. `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_update_workspace_access_policy_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    access_policy: *const c_char,
) -> *mut c_char {
    let result = runtime_update_workspace_access_policy_result(
        data_dir,
        identity_file,
        workspace_id,
        access_policy,
    );
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

/// Submits a prepared workspace join request to a reachable admin peer.
///
/// `workspace_id` may be null or empty when the request itself carries the
/// target workspace context. The endpoint may use direct TCP or native Iroh;
/// the exported name is retained for ABI compatibility.
///
/// # Safety
///
/// `peer_endpoint` and `request_json` must be valid pointers to NUL-terminated
/// UTF-8 strings for the duration of this call. `workspace_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_submit_join_request_direct_result_json(
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    request_json: *const c_char,
) -> *mut c_char {
    let result =
        runtime_submit_join_request_direct_result(peer_endpoint, workspace_id, request_json);
    into_c_string(&result)
}

/// Pulls pending workspace join-request envelopes from a known peer into the
/// local runtime inbox.
///
/// This is a workspace-scoped known-peer exchange; it does not discover
/// workspaces or peers.
///
/// # Safety
///
/// All pointers must be valid pointers to NUL-terminated UTF-8 strings for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_pull_join_requests_direct_result_json(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_pull_join_requests_direct_result(
        data_dir,
        peer_endpoint,
        workspace_id,
        max_entries,
    );
    into_c_string(&result)
}

/// Pulls pending workspace join-response envelopes from a known peer into the
/// local runtime inbox.
///
/// This is a workspace-scoped known-peer exchange; it does not discover
/// workspaces or peers.
///
/// # Safety
///
/// All pointers must be valid pointers to NUL-terminated UTF-8 strings for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_pull_join_responses_direct_result_json(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_pull_join_responses_direct_result(
        data_dir,
        peer_endpoint,
        workspace_id,
        max_entries,
    );
    into_c_string(&result)
}

/// Pulls only the requested workspace join-response envelopes from a known
/// peer into the local runtime inbox.
///
/// `request_ids_json` must encode an array of request-ID strings. The array,
/// each ID, and `max_entries` are bounded by the access-envelope protocol.
/// The remote peer applies request-ID filtering before the result limit, and
/// the caller rejects a response whose request ID was not requested.
///
/// # Safety
///
/// All pointers must be valid pointers to NUL-terminated UTF-8 strings for the
/// duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_pull_join_responses_for_requests_direct_result_json(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    request_ids_json: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_pull_join_responses_for_requests_direct_result(
        data_dir,
        peer_endpoint,
        workspace_id,
        request_ids_json,
        max_entries,
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

/// Starts a background native Iroh peer with explicit public-service policy.
///
/// This variant avoids changing process environment variables after other
/// runtime workers have started.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
/// `identity_file` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_start_iroh_peer_with_policy_result_json(
    data_dir: *const c_char,
    identity_file: *const c_char,
    allow_public_relays: bool,
    allow_public_discovery: bool,
) -> *mut c_char {
    let result = runtime_start_iroh_peer_with_policy_result(
        data_dir,
        identity_file,
        allow_public_relays,
        allow_public_discovery,
    );
    into_c_string(&result)
}

/// Lists incoming join requests received by runtime-hosted peers.
///
/// The returned string is a JSON result envelope. Passing `max_entries` as `0`
/// uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_request_inbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_join_request_inbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Lists incoming join requests for one workspace.
///
/// Workspace filtering is applied before `max_entries`, so traffic for other
/// workspaces cannot consume this result window. Passing `max_entries` as `0`
/// uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` and `workspace_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_request_inbox_for_workspace_result_json(
    data_dir: *const c_char,
    workspace_id: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result =
        runtime_list_join_request_inbox_for_workspace_result(data_dir, workspace_id, max_entries);
    into_c_string(&result)
}

/// Acknowledges and removes an incoming join request from the runtime inbox.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_ack_join_request_inbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_ack_join_request_inbox_entry_result(data_dir, entry_id);
    into_c_string(&result)
}

/// Queues a prepared workspace join request for durable retry/delivery.
///
/// `workspace_id` and `peer_endpoint` may be null or empty when the request
/// itself carries the target workspace context or must be handed off manually.
///
/// # Safety
///
/// `data_dir` and `request_json` must be valid pointers to NUL-terminated
/// UTF-8 strings for the duration of this call. `workspace_id` and
/// `peer_endpoint` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_queue_join_request_outbox_result_json(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    request_json: *const c_char,
) -> *mut c_char {
    let result = runtime_queue_join_request_outbox_result(
        data_dir,
        peer_endpoint,
        workspace_id,
        request_json,
    );
    into_c_string(&result)
}

/// Lists queued workspace join requests waiting for delivery or acknowledgement.
///
/// Passing `max_entries` as `0` uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_request_outbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_join_request_outbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Lists queued workspace join requests that are due for direct retry.
///
/// Delivered, acknowledged, peerless, and backoff-delayed entries are omitted.
/// Passing `max_entries` as `0` uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_due_join_request_outbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_due_join_request_outbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Updates queued join-request delivery state.
///
/// `status` accepts `pending`, `delivered`, `failed`, or `acknowledged`.
/// `error` may be null or empty.
///
/// # Safety
///
/// All non-null pointers must be valid pointers to NUL-terminated UTF-8 strings
/// for the duration of this call. `error` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_mark_join_request_outbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
    status: *const c_char,
    error: *const c_char,
) -> *mut c_char {
    let result = runtime_mark_join_request_outbox_entry_result(data_dir, entry_id, status, error);
    into_c_string(&result)
}

/// Submits one queued join request to its stored peer endpoint.
///
/// The outbox entry is marked `delivered` on success and `failed` on transport
/// failure. Direct TCP and native Iroh endpoints are both supported; the
/// exported name is retained for ABI compatibility.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_submit_join_request_outbox_entry_direct_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_submit_join_request_outbox_entry_direct_result(data_dir, entry_id);
    into_c_string(&result)
}

/// Acknowledges and removes an outbound join request from the durable outbox.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_ack_join_request_outbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_ack_join_request_outbox_entry_result(data_dir, entry_id);
    into_c_string(&result)
}

/// Lists incoming join responses received by runtime-hosted peers.
///
/// The returned string is a JSON result envelope. Passing `max_entries` as `0`
/// uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_response_inbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_join_response_inbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Lists incoming join responses relevant to the local desktop session.
///
/// Every response must match a request ID in `pending_request_ids_json`.
/// Legacy and secure invite responses must additionally target
/// `local_device_id`. Filtering is applied before `max_entries`.
///
/// # Safety
///
/// `data_dir`, `local_device_id`, and `pending_request_ids_json` must be valid
/// pointers to NUL-terminated UTF-8 strings for the duration of this call.
/// `pending_request_ids_json` must encode an array of request-ID strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_response_inbox_scoped_result_json(
    data_dir: *const c_char,
    local_device_id: *const c_char,
    pending_request_ids_json: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_join_response_inbox_scoped_result(
        data_dir,
        local_device_id,
        pending_request_ids_json,
        max_entries,
    );
    into_c_string(&result)
}

/// Stages an incoming workspace join response in the runtime inbox.
///
/// # Safety
///
/// All arguments must be valid pointers to NUL-terminated UTF-8 strings for
/// the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_stage_join_response_inbox_result_json(
    data_dir: *const c_char,
    workspace_id: *const c_char,
    response_json: *const c_char,
) -> *mut c_char {
    let result = runtime_stage_join_response_inbox_result(data_dir, workspace_id, response_json);
    into_c_string(&result)
}

/// Acknowledges and removes an incoming join response from the runtime inbox.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_ack_join_response_inbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_ack_join_response_inbox_entry_result(data_dir, entry_id);
    into_c_string(&result)
}

/// Queues a prepared workspace join response for durable retry/delivery.
///
/// `response_json` is an approval invite package or a join-response envelope.
///
/// # Safety
///
/// All non-null pointers must be valid pointers to NUL-terminated UTF-8 strings
/// for the duration of this call. `workspace_id` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_queue_join_response_outbox_result_json(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    response_json: *const c_char,
) -> *mut c_char {
    let result = runtime_queue_join_response_outbox_result(
        data_dir,
        peer_endpoint,
        workspace_id,
        response_json,
    );
    into_c_string(&result)
}

/// Lists queued workspace join responses waiting for delivery or acknowledgement.
///
/// Passing `max_entries` as `0` uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_join_response_outbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_join_response_outbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Lists queued workspace join responses that are due for direct retry.
///
/// Delivered, acknowledged, and backoff-delayed entries are omitted. Passing
/// `max_entries` as `0` uses the runtime default limit.
///
/// # Safety
///
/// `data_dir` must be a valid pointer to a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_list_due_join_response_outbox_result_json(
    data_dir: *const c_char,
    max_entries: usize,
) -> *mut c_char {
    let result = runtime_list_due_join_response_outbox_result(data_dir, max_entries);
    into_c_string(&result)
}

/// Updates queued join-response delivery state.
///
/// `status` accepts `pending`, `delivered`, `failed`, or `acknowledged`.
/// `error` may be null or empty.
///
/// # Safety
///
/// All non-null pointers must be valid pointers to NUL-terminated UTF-8 strings
/// for the duration of this call. `error` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_mark_join_response_outbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
    status: *const c_char,
    error: *const c_char,
) -> *mut c_char {
    let result = runtime_mark_join_response_outbox_entry_result(data_dir, entry_id, status, error);
    into_c_string(&result)
}

/// Submits one queued join response to its stored peer endpoint.
///
/// The outbox entry is marked `delivered` on success and `failed` on transport
/// failure. Direct TCP and native Iroh endpoints are both supported; the
/// exported name is retained for ABI compatibility.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_submit_join_response_outbox_entry_direct_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_submit_join_response_outbox_entry_direct_result(data_dir, entry_id);
    into_c_string(&result)
}

/// Acknowledges and removes an outbound join response from the durable outbox.
///
/// # Safety
///
/// `data_dir` and `entry_id` must be valid pointers to NUL-terminated UTF-8
/// strings for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chaft_runtime_ack_join_response_outbox_entry_result_json(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> *mut c_char {
    let result = runtime_ack_join_response_outbox_entry_result(data_dir, entry_id);
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

#[cfg(test)]
mod tests;
