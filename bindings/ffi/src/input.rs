use std::{ffi::c_char, slice};

use chaft_types::{
    ATTACHMENT_ID_MAX_BYTES, ATTACHMENT_MEDIA_TYPE_MAX_BYTES, CHANNEL_ID_MAX_BYTES,
    CHANNEL_NAME_MAX_BYTES, DEVICE_DISPLAY_NAME_MAX_BYTES, DEVICE_ID_MAX_BYTES,
    DEVICE_KEY_PACKAGE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, EVENT_ID_MAX_BYTES,
    MESSAGE_ID_MAX_BYTES, MESSAGE_MARKDOWN_MAX_BYTES, PEER_ENDPOINT_ID_MAX_BYTES,
    PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES, PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
    REACTION_TEXT_MAX_BYTES, WORKSPACE_ACCESS_POLICY_MAX_BYTES, WORKSPACE_ID_MAX_BYTES,
    WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES, WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
    WORKSPACE_INVITE_ID_MAX_BYTES, WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES,
    WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES, WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
    WORKSPACE_NAME_MAX_BYTES, WorkspaceAccessPolicy, WorkspaceInviteResolution,
    WorkspaceJoinRequestResolution, WorkspaceRole,
};

use crate::envelope::{FfiError, ffi_error};

pub(crate) const WORKSPACE_EVENTS_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const KEY_TRANSFER_JSON_MAX_BYTES: usize = 256 * 1024;
pub(crate) const RECOVERY_BUNDLE_JSON_MAX_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const PEER_ENDPOINT_LIST_TEXT_MAX_BYTES: usize =
    PEER_ENDPOINT_LIST_MAX_ITEMS * (PEER_ENDPOINT_MAX_BYTES + 1);
pub(crate) const SEARCH_QUERY_MAX_BYTES: usize = 512;
pub(crate) const FFI_PATH_MAX_BYTES: usize = 64 * 1024;
pub(crate) const FFI_PASSPHRASE_MAX_BYTES: usize = 16 * 1024;
pub(crate) const WORKSPACE_ROLE_TEXT_MAX_BYTES: usize = 16;
pub(crate) const FFI_GENERIC_STRING_MAX_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn read_c_string(
    value: *const c_char,
    field_name: &'static str,
) -> Result<String, FfiError> {
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

pub(crate) fn read_c_string_with_max_bytes(
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

pub(crate) fn optional_c_string(
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
        "access_policy" => Some((
            WORKSPACE_ACCESS_POLICY_MAX_BYTES,
            "workspace_access_policy_too_large",
            "workspace access policy",
        )),
        "join_request_resolution" => Some((
            WORKSPACE_ROLE_TEXT_MAX_BYTES,
            "join_request_resolution_too_large",
            "join request resolution",
        )),
        "invite_resolution" => Some((
            WORKSPACE_ROLE_TEXT_MAX_BYTES,
            "invite_resolution_too_large",
            "invite resolution",
        )),
        "invite_id" => Some((
            WORKSPACE_INVITE_ID_MAX_BYTES,
            "invite_id_too_large",
            "invite ID",
        )),
        "timestamp" => Some((
            WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
            "timestamp_too_large",
            "timestamp",
        )),
        "invite_approval_policy" => Some((
            WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES,
            "invite_approval_policy_too_large",
            "invite approval policy",
        )),
        "invite_sync_expectation" => Some((
            WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES,
            "invite_sync_expectation_too_large",
            "invite sync expectation",
        )),
        "request_id" => Some((
            WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
            "join_request_id_too_large",
            "join request ID",
        )),
        "join_request_note" => Some((
            WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
            "join_request_note_too_large",
            "join request note",
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

pub(crate) fn optional_c_string_with_max_bytes(
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

pub(crate) fn validate_json_payload_size(
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

pub(crate) fn parse_workspace_role(input: &str) -> Result<WorkspaceRole, FfiError> {
    let quoted = format!("\"{}\"", input);
    serde_json::from_str::<WorkspaceRole>(&quoted).map_err(|_| {
        ffi_error(
            "invalid_workspace_role",
            "expected owner, admin, member, or guest",
        )
    })
}

pub(crate) fn parse_workspace_access_policy(
    input: &str,
) -> Result<WorkspaceAccessPolicy, FfiError> {
    let quoted = format!("\"{}\"", input);
    serde_json::from_str::<WorkspaceAccessPolicy>(&quoted).map_err(|_| {
        ffi_error(
            "invalid_workspace_access_policy",
            "expected invite_only, request_access, or discoverable",
        )
    })
}

pub(crate) fn parse_workspace_join_request_resolution(
    input: &str,
) -> Result<WorkspaceJoinRequestResolution, FfiError> {
    let quoted = format!("\"{}\"", input);
    serde_json::from_str::<WorkspaceJoinRequestResolution>(&quoted).map_err(|_| {
        ffi_error(
            "invalid_join_request_resolution",
            "expected approved, declined, or revoked",
        )
    })
}

pub(crate) fn parse_workspace_invite_resolution(
    input: &str,
) -> Result<WorkspaceInviteResolution, FfiError> {
    let quoted = format!("\"{}\"", input);
    serde_json::from_str::<WorkspaceInviteResolution>(&quoted)
        .map_err(|_| ffi_error("invalid_invite_resolution", "expected revoked"))
}
