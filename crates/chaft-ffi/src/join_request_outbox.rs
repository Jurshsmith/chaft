use std::{
    ffi::c_char,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_net_direct::MAX_JOIN_REQUEST_SUBMISSION_BYTES;
use chaft_net_iroh::IrohTransport;
use chaft_runtime::WorkspaceInviteClaim;
use chaft_types::{DEVICE_DISPLAY_NAME_MAX_BYTES, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::direct_workspace_id_arg,
    input::{optional_c_string, read_c_string, read_c_string_with_max_bytes},
    peer_endpoint::direct_peer_address,
    worker::{run_network_future, run_on_worker_thread},
};

const JOIN_REQUEST_OUTBOX_DIR: &str = "join-request-outbox";
const JOIN_REQUEST_OUTBOX_SCHEMA_VERSION: u32 = 1;
const JOIN_REQUEST_OUTBOX_ENTRY_MAX_BYTES: usize = 48 * 1024;
const JOIN_REQUEST_OUTBOX_ENTRY_ID_MAX_BYTES: usize = 128;
const JOIN_REQUEST_OUTBOX_ERROR_MAX_BYTES: usize = 512;
const JOIN_REQUEST_OUTBOX_LIST_MAX_ENTRIES: usize = 100;
const JOIN_REQUEST_OUTBOX_MAX_RETRY_DELAY_MS: u64 = 5 * 60 * 1000;

static JOIN_REQUEST_OUTBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JoinRequestOutboxStatus {
    Pending,
    Delivered,
    Failed,
    Acknowledged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinRequestOutboxEntry {
    schema_version: u32,
    entry_id: String,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peer_endpoint: Option<String>,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_attempt_at_unix_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivered_at_unix_ms: Option<u64>,
    #[serde(default)]
    attempt_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_attempt_after_unix_ms: Option<u64>,
    status: JoinRequestOutboxStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    request_text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueuedJoinRequestOutboxEntry {
    entry: JoinRequestOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinRequestOutboxEntries {
    entries: Vec<JoinRequestOutboxEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkedJoinRequestOutboxEntry {
    entry: JoinRequestOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmittedJoinRequestOutboxEntry {
    entry: JoinRequestOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcknowledgedJoinRequestOutboxEntry {
    entry_id: String,
}

pub(crate) fn runtime_queue_join_request_outbox_result(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    request_json: *const c_char,
) -> FfiResult<QueuedJoinRequestOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let peer_endpoint = optional_c_string(peer_endpoint, "peer_endpoint")?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if let Some(peer_endpoint) = &peer_endpoint {
            direct_peer_address(peer_endpoint.clone())?;
        }
        let workspace_id = optional_c_string(workspace_id, "workspace_id")?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(direct_workspace_id_arg)
            .transpose()?;
        let request_text = read_c_string_with_max_bytes(
            request_json,
            "request_json",
            MAX_JOIN_REQUEST_SUBMISSION_BYTES,
            "join_request_too_large",
            "join request",
        )?;
        let metadata = validated_join_request_metadata(&request_text)?;
        if workspace_id.is_some()
            && metadata.workspace_id.is_some()
            && workspace_id != metadata.workspace_id
        {
            return Err(ffi_error(
                "join_request_workspace_id_mismatch",
                "join request payload workspace ID must match the requested workspace",
            ));
        }
        let workspace_id = workspace_id
            .or(metadata.workspace_id)
            .map(direct_workspace_id_arg)
            .transpose()?;
        let entry = queue_join_request_outbox_entry(
            &data_dir,
            peer_endpoint,
            workspace_id,
            metadata.request_id,
            request_text,
        )?;
        Ok(QueuedJoinRequestOutboxEntry { entry })
    })
}

pub(crate) fn runtime_list_join_request_outbox_result(
    data_dir: *const c_char,
    max_entries: usize,
) -> FfiResult<JoinRequestOutboxEntries> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let max_entries = if max_entries == 0 {
            JOIN_REQUEST_OUTBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_REQUEST_OUTBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_request_outbox_entries(&data_dir, max_entries)?;
        Ok(JoinRequestOutboxEntries { entries })
    })
}

pub(crate) fn runtime_list_due_join_request_outbox_result(
    data_dir: *const c_char,
    max_entries: usize,
) -> FfiResult<JoinRequestOutboxEntries> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let max_entries = if max_entries == 0 {
            JOIN_REQUEST_OUTBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_REQUEST_OUTBOX_LIST_MAX_ENTRIES)
        };
        let now = current_unix_ms();
        // Terminal entries must not consume the scan window and starve newer
        // pending work. The public result is still bounded below.
        let mut entries = Vec::new();
        for entry in list_join_request_outbox_entries(&data_dir, usize::MAX)? {
            if is_join_request_outbox_entry_terminal(&entry) {
                // Survive a crash between a successful submit and the desktop
                // ACK, and clean up entries created by older clients.
                let _ = fs::remove_file(outbox_entry_path(&data_dir, &entry.entry_id));
                continue;
            }
            if entries.len() < max_entries && is_join_request_outbox_entry_due(&entry, now) {
                entries.push(entry);
            }
        }
        Ok(JoinRequestOutboxEntries { entries })
    })
}

pub(crate) fn runtime_mark_join_request_outbox_entry_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
    status: *const c_char,
    error: *const c_char,
) -> FfiResult<MarkedJoinRequestOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_outbox_entry_id(&entry_id)?;
        let status = outbox_status_arg(&read_c_string(status, "status")?)?;
        let error = optional_outbox_error(error)?;
        let entry = mark_join_request_outbox_entry(&data_dir, &entry_id, status, error)?;
        Ok(MarkedJoinRequestOutboxEntry { entry })
    })
}

pub(crate) fn runtime_submit_join_request_outbox_entry_direct_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> FfiResult<SubmittedJoinRequestOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_outbox_entry_id(&entry_id)?;
        let entry = read_join_request_outbox_entry(&data_dir, &entry_id)?;
        let peer_endpoint = entry.peer_endpoint.clone().ok_or_else(|| {
            ffi_error(
                "join_request_outbox_peer_endpoint_required",
                "queued join request has no peer endpoint",
            )
        })?;
        let peer = direct_peer_address(peer_endpoint)?;
        let workspace_id = entry.workspace_id.clone().map(WorkspaceId);
        let request_bytes = entry.request_text.clone().into_bytes();

        run_on_worker_thread(move || {
            let transport = IrohTransport::shared_from_environment();
            let submit_result = run_network_future(transport.submit_join_request(
                &peer,
                workspace_id.as_ref(),
                request_bytes,
            ))?;
            let entry = match submit_result {
                Ok(()) => mark_join_request_outbox_entry(
                    &data_dir,
                    &entry_id,
                    JoinRequestOutboxStatus::Delivered,
                    None,
                )?,
                Err(error) => {
                    let message = error.to_string();
                    let _ = mark_join_request_outbox_entry(
                        &data_dir,
                        &entry_id,
                        JoinRequestOutboxStatus::Failed,
                        Some(message.clone()),
                    );
                    return Err(ffi_error("runtime_submit_join_request_failed", message));
                }
            };
            Ok(SubmittedJoinRequestOutboxEntry { entry })
        })
    })
}

pub(crate) fn runtime_ack_join_request_outbox_entry_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> FfiResult<AcknowledgedJoinRequestOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_outbox_entry_id(&entry_id)?;
        match fs::remove_file(outbox_entry_path(&data_dir, &entry_id)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ffi_error(
                    "join_request_outbox_ack_failed",
                    format!("could not acknowledge join request outbox entry: {error}"),
                ));
            }
        }
        Ok(AcknowledgedJoinRequestOutboxEntry { entry_id })
    })
}

fn queue_join_request_outbox_entry(
    data_dir: &Path,
    peer_endpoint: Option<String>,
    workspace_id: Option<String>,
    request_id: String,
    request_text: String,
) -> Result<JoinRequestOutboxEntry, FfiError> {
    validate_outbox_entry_id(&request_id)?;
    let now = current_unix_ms();
    let existing = read_join_request_outbox_entry(data_dir, &request_id).ok();
    let entry = JoinRequestOutboxEntry {
        schema_version: JOIN_REQUEST_OUTBOX_SCHEMA_VERSION,
        entry_id: request_id.clone(),
        request_id,
        workspace_id,
        peer_endpoint,
        created_at_unix_ms: existing
            .as_ref()
            .map(|entry| entry.created_at_unix_ms)
            .unwrap_or(now),
        updated_at_unix_ms: now,
        last_attempt_at_unix_ms: existing
            .as_ref()
            .and_then(|entry| entry.last_attempt_at_unix_ms),
        delivered_at_unix_ms: existing
            .as_ref()
            .and_then(|entry| entry.delivered_at_unix_ms),
        attempt_count: existing
            .as_ref()
            .map(|entry| entry.attempt_count)
            .unwrap_or(0),
        next_attempt_after_unix_ms: existing
            .as_ref()
            .and_then(|entry| entry.next_attempt_after_unix_ms),
        status: existing
            .as_ref()
            .map(|entry| entry.status)
            .unwrap_or(JoinRequestOutboxStatus::Pending),
        error: existing.and_then(|entry| entry.error),
        request_text,
    };
    write_join_request_outbox_entry(data_dir, &entry)?;
    Ok(entry)
}

fn list_join_request_outbox_entries(
    data_dir: &Path,
    max_entries: usize,
) -> Result<Vec<JoinRequestOutboxEntry>, FfiError> {
    let outbox_dir = outbox_dir(data_dir);
    let paths = match fs::read_dir(&outbox_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(ffi_error(
                "join_request_outbox_read_failed",
                format!("could not read join request outbox: {error}"),
            ));
        }
    };
    let mut entries = Vec::new();
    for path in paths {
        match read_join_request_outbox_entry_path(&path) {
            Ok(entry) => entries.push(entry),
            Err(_) => {
                // Older builds could persist requests that no longer satisfy
                // the outbound security contract. Preserve the raw entry for
                // diagnostics, but keep it out of future retry scans so one
                // invalid request cannot starve every valid handoff.
                let _ = fs::rename(&path, path.with_extension("invalid"));
            }
        }
    }
    entries.sort_by(|left, right| {
        left.created_at_unix_ms
            .cmp(&right.created_at_unix_ms)
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    entries.truncate(max_entries);
    Ok(entries)
}

fn mark_join_request_outbox_entry(
    data_dir: &Path,
    entry_id: &str,
    status: JoinRequestOutboxStatus,
    error: Option<String>,
) -> Result<JoinRequestOutboxEntry, FfiError> {
    let mut entry = read_join_request_outbox_entry(data_dir, entry_id)?;
    let now = current_unix_ms();
    entry.status = status;
    entry.updated_at_unix_ms = now;
    if matches!(
        status,
        JoinRequestOutboxStatus::Delivered | JoinRequestOutboxStatus::Failed
    ) {
        entry.attempt_count = entry.attempt_count.saturating_add(1);
        entry.last_attempt_at_unix_ms = Some(now);
    }
    if status == JoinRequestOutboxStatus::Delivered {
        entry.delivered_at_unix_ms = Some(now);
        entry.next_attempt_after_unix_ms = None;
        entry.error = None;
    } else if status == JoinRequestOutboxStatus::Failed {
        entry.next_attempt_after_unix_ms =
            Some(now.saturating_add(join_request_retry_delay_ms(entry.attempt_count)));
        entry.error = error;
    } else {
        entry.next_attempt_after_unix_ms = None;
        entry.error = None;
    }
    write_join_request_outbox_entry(data_dir, &entry)?;
    Ok(entry)
}

fn read_join_request_outbox_entry(
    data_dir: &Path,
    entry_id: &str,
) -> Result<JoinRequestOutboxEntry, FfiError> {
    validate_outbox_entry_id(entry_id)?;
    read_join_request_outbox_entry_path(&outbox_entry_path(data_dir, entry_id))
}

fn read_join_request_outbox_entry_path(path: &Path) -> Result<JoinRequestOutboxEntry, FfiError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ffi_error(
            "join_request_outbox_read_failed",
            format!("could not inspect join request outbox entry: {error}"),
        )
    })?;
    if metadata.len() as usize > JOIN_REQUEST_OUTBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_request_outbox_entry_too_large",
            format!("join request outbox entry {} is too large", path.display()),
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| {
        ffi_error(
            "join_request_outbox_read_failed",
            format!("could not read join request outbox entry: {error}"),
        )
    })?;
    let entry: JoinRequestOutboxEntry = serde_json::from_str(&text).map_err(|error| {
        ffi_error(
            "join_request_outbox_read_failed",
            format!("could not parse join request outbox entry: {error}"),
        )
    })?;
    validate_join_request_outbox_entry(&entry)?;
    Ok(entry)
}

fn write_join_request_outbox_entry(
    data_dir: &Path,
    entry: &JoinRequestOutboxEntry,
) -> Result<(), FfiError> {
    validate_join_request_outbox_entry(entry)?;
    let outbox_dir = outbox_dir(data_dir);
    fs::create_dir_all(&outbox_dir).map_err(|error| {
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not create join request outbox: {error}"),
        )
    })?;
    let bytes = serde_json::to_vec_pretty(entry).map_err(|error| {
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not encode join request outbox entry: {error}"),
        )
    })?;
    if bytes.len() > JOIN_REQUEST_OUTBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_request_outbox_entry_too_large",
            format!(
                "join request outbox entry is too large: {} bytes",
                bytes.len()
            ),
        ));
    }
    let sequence = JOIN_REQUEST_OUTBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = outbox_dir.join(format!(".{}.{}.tmp", entry.entry_id, sequence));
    let final_path = outbox_entry_path(data_dir, &entry.entry_id);
    write_private_file(&temp_path, &bytes)?;
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not commit join request outbox entry: {error}"),
        )
    })
}

fn validate_join_request_outbox_entry(entry: &JoinRequestOutboxEntry) -> Result<(), FfiError> {
    if entry.schema_version != JOIN_REQUEST_OUTBOX_SCHEMA_VERSION {
        return Err(ffi_error(
            "join_request_outbox_schema_unsupported",
            format!(
                "join request outbox entry schema {} is unsupported",
                entry.schema_version
            ),
        ));
    }
    validate_outbox_entry_id(&entry.entry_id)?;
    validate_outbox_entry_id(&entry.request_id)?;
    if entry.entry_id != entry.request_id {
        return Err(ffi_error(
            "join_request_outbox_entry_id_mismatch",
            "join request outbox entry ID must match request ID",
        ));
    }
    if let Some(workspace_id) = &entry.workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    if let Some(peer_endpoint) = &entry.peer_endpoint {
        direct_peer_address(peer_endpoint.clone())?;
    }
    if let Some(error) = &entry.error {
        validate_outbox_error(error)?;
    }
    let metadata = validated_join_request_metadata(&entry.request_text)?;
    if metadata.request_id != entry.request_id {
        return Err(ffi_error(
            "join_request_outbox_payload_id_mismatch",
            "join request payload request ID must match the outbox entry",
        ));
    }
    if metadata.workspace_id != entry.workspace_id {
        return Err(ffi_error(
            "join_request_outbox_payload_workspace_mismatch",
            "join request payload workspace must match the outbox entry",
        ));
    }
    Ok(())
}

fn join_request_retry_delay_ms(attempt_count: u32) -> u64 {
    let exponent = attempt_count.saturating_sub(1).min(5);
    15_000u64
        .saturating_mul(1u64 << exponent)
        .min(JOIN_REQUEST_OUTBOX_MAX_RETRY_DELAY_MS)
}

fn is_join_request_outbox_entry_due(entry: &JoinRequestOutboxEntry, now_unix_ms: u64) -> bool {
    if entry
        .peer_endpoint
        .as_ref()
        .map(|endpoint| endpoint.trim().is_empty())
        .unwrap_or(true)
    {
        return false;
    }
    if matches!(
        entry.status,
        JoinRequestOutboxStatus::Delivered | JoinRequestOutboxStatus::Acknowledged
    ) {
        return false;
    }
    entry
        .next_attempt_after_unix_ms
        .map(|next_attempt_after| next_attempt_after <= now_unix_ms)
        .unwrap_or(true)
}

fn is_join_request_outbox_entry_terminal(entry: &JoinRequestOutboxEntry) -> bool {
    matches!(
        entry.status,
        JoinRequestOutboxStatus::Delivered | JoinRequestOutboxStatus::Acknowledged
    )
}

pub(crate) struct JoinRequestMetadata {
    pub(crate) request_id: String,
    pub(crate) workspace_id: Option<String>,
}

pub(crate) fn validated_join_request_metadata(
    request_text: &str,
) -> Result<JoinRequestMetadata, FfiError> {
    if request_text.trim().is_empty() {
        return Err(ffi_error(
            "join_request_payload_empty",
            "join request payload is empty",
        ));
    }
    if request_text.len() > MAX_JOIN_REQUEST_SUBMISSION_BYTES {
        return Err(ffi_error(
            "join_request_payload_too_large",
            format!(
                "join request payload is too large: {} bytes",
                request_text.len()
            ),
        ));
    }
    let value = serde_json::from_str::<serde_json::Value>(request_text).map_err(|error| {
        ffi_error(
            "join_request_payload_invalid",
            format!("join request payload is not valid JSON: {error}"),
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        ffi_error(
            "join_request_payload_invalid",
            "join request payload must be a JSON object",
        )
    })?;
    let kind = object
        .get("kind")
        .and_then(|value| value.as_str())
        .map(str::trim);
    if !matches!(
        kind,
        Some("chaft.workspace-join-request.v1" | "chaft.workspace-invite-claim.v1")
    ) {
        return Err(ffi_error(
            "join_request_payload_invalid",
            "join request payload kind is unsupported",
        ));
    }
    if object.get("schemaVersion").and_then(|value| value.as_u64()) != Some(1) {
        return Err(ffi_error(
            "join_request_payload_invalid",
            "join request payload schema version must be 1",
        ));
    }
    let display_name = object
        .get("displayName")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ffi_error(
                "join_request_display_name_required",
                "join request payload must include the joiner's display name",
            )
        })?;
    if display_name.len() > DEVICE_DISPLAY_NAME_MAX_BYTES {
        return Err(ffi_error(
            "join_request_display_name_too_large",
            format!(
                "join request display name is too large: {} bytes",
                display_name.len()
            ),
        ));
    }
    if kind == Some("chaft.workspace-invite-claim.v1") {
        let claim =
            serde_json::from_value::<WorkspaceInviteClaim>(value.clone()).map_err(|error| {
                ffi_error(
                    "join_request_payload_invalid",
                    format!("invite claim payload is incomplete: {error}"),
                )
            })?;
        if claim.payload.schema_version != 1
            || claim.payload.request_id.trim().is_empty()
            || claim.payload.workspace_id.trim().is_empty()
            || claim.payload.invite_id.trim().is_empty()
            || claim.payload.device_id.trim().is_empty()
            || claim.payload.device_public_key.trim().is_empty()
            || claim.payload.delivery_device_id.trim().is_empty()
            || claim
                .payload
                .response_encryption_public_key
                .trim()
                .is_empty()
            || claim.payload.source_type != "invite_claim"
            || claim.payload.source_invite_id != claim.payload.invite_id
            || claim.payload.source_approval_policy != "preapproved"
            || claim.device_signature.trim().is_empty()
            || claim.capability_signature.trim().is_empty()
        {
            return Err(ffi_error(
                "join_request_payload_invalid",
                "invite claim payload is incomplete",
            ));
        }
    }
    let request_id = object
        .get("requestId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ffi_error(
                "join_request_id_required",
                "join request payload must include requestId",
            )
        })?
        .to_owned();
    validate_outbox_entry_id(&request_id)?;
    let workspace_id = object
        .get("workspaceId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ffi_error(
                "join_request_workspace_id_required",
                "join request payload must include workspaceId",
            )
        })?
        .to_owned();
    direct_workspace_id_arg(workspace_id.clone())?;
    Ok(JoinRequestMetadata {
        request_id,
        workspace_id: Some(workspace_id),
    })
}

fn outbox_status_arg(status: &str) -> Result<JoinRequestOutboxStatus, FfiError> {
    match status.trim() {
        "pending" => Ok(JoinRequestOutboxStatus::Pending),
        "delivered" => Ok(JoinRequestOutboxStatus::Delivered),
        "failed" => Ok(JoinRequestOutboxStatus::Failed),
        "acknowledged" => Ok(JoinRequestOutboxStatus::Acknowledged),
        _ => Err(ffi_error(
            "join_request_outbox_status_invalid",
            "join request outbox status must be pending, delivered, failed, or acknowledged",
        )),
    }
}

fn optional_outbox_error(error: *const c_char) -> Result<Option<String>, FfiError> {
    let error = optional_c_string(error, "error")?
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(error) = &error {
        validate_outbox_error(error)?;
    }
    Ok(error)
}

fn validate_outbox_error(error: &str) -> Result<(), FfiError> {
    if error.len() > JOIN_REQUEST_OUTBOX_ERROR_MAX_BYTES {
        return Err(ffi_error(
            "join_request_outbox_error_too_large",
            "join request outbox error is too large",
        ));
    }
    Ok(())
}

fn validate_outbox_entry_id(entry_id: &str) -> Result<(), FfiError> {
    if entry_id.is_empty() {
        return Err(ffi_error(
            "join_request_outbox_entry_id_required",
            "join request outbox entry ID is required",
        ));
    }
    if entry_id.len() > JOIN_REQUEST_OUTBOX_ENTRY_ID_MAX_BYTES
        || !entry_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(ffi_error(
            "join_request_outbox_entry_id_invalid",
            "join request outbox entry ID is invalid",
        ));
    }
    Ok(())
}

fn outbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(JOIN_REQUEST_OUTBOX_DIR)
}

fn outbox_entry_path(data_dir: &Path, entry_id: &str) -> PathBuf {
    outbox_dir(data_dir).join(format!("{entry_id}.json"))
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), FfiError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not create join request outbox entry: {error}"),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not write join request outbox entry: {error}"),
        )
    })?;
    file.sync_all().map_err(|error| {
        ffi_error(
            "join_request_outbox_write_failed",
            format!("could not flush join request outbox entry: {error}"),
        )
    })
}
