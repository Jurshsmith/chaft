use std::{
    ffi::c_char,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_net_direct::{DirectTransport, MAX_JOIN_RESPONSE_SUBMISSION_BYTES};
use chaft_types::WorkspaceId;
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::direct_workspace_id_arg,
    input::{
        KEY_TRANSFER_JSON_MAX_BYTES, optional_c_string, read_c_string, read_c_string_with_max_bytes,
    },
    join_response_inbox::{validate_join_response_entry_id, validate_join_response_payload},
    peer_endpoint::direct_peer_address,
    worker::run_on_worker_thread,
};

const JOIN_RESPONSE_OUTBOX_DIR: &str = "join-response-outbox";
const JOIN_RESPONSE_OUTBOX_SCHEMA_VERSION: u32 = 1;
const JOIN_RESPONSE_OUTBOX_ENTRY_MAX_BYTES: usize = KEY_TRANSFER_JSON_MAX_BYTES + 4096;
const JOIN_RESPONSE_OUTBOX_ERROR_MAX_BYTES: usize = 512;
const JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES: usize = 100;
const JOIN_RESPONSE_OUTBOX_MAX_RETRY_DELAY_MS: u64 = 5 * 60 * 1000;

static JOIN_RESPONSE_OUTBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JoinResponseOutboxStatus {
    Pending,
    Delivered,
    Failed,
    Acknowledged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinResponseOutboxEntry {
    schema_version: u32,
    entry_id: String,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<String>,
    peer_endpoint: String,
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
    status: JoinResponseOutboxStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    response_text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueuedJoinResponseOutboxEntry {
    entry: JoinResponseOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinResponseOutboxEntries {
    entries: Vec<JoinResponseOutboxEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkedJoinResponseOutboxEntry {
    entry: JoinResponseOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmittedJoinResponseOutboxEntry {
    entry: JoinResponseOutboxEntry,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcknowledgedJoinResponseOutboxEntry {
    entry_id: String,
}

pub(crate) fn runtime_queue_join_response_outbox_result(
    data_dir: *const c_char,
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    response_json: *const c_char,
) -> FfiResult<QueuedJoinResponseOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let peer_endpoint = read_c_string(peer_endpoint, "peer_endpoint")?
            .trim()
            .to_owned();
        direct_peer_address(peer_endpoint.clone())?;
        let workspace_id = optional_c_string(workspace_id, "workspace_id")?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(direct_workspace_id_arg)
            .transpose()?;
        let response_text = read_c_string_with_max_bytes(
            response_json,
            "response_json",
            MAX_JOIN_RESPONSE_SUBMISSION_BYTES,
            "join_response_too_large",
            "join response",
        )?;
        let metadata = validate_join_response_payload(&response_text)?;
        let workspace_id = workspace_id
            .or(metadata.workspace_id)
            .map(direct_workspace_id_arg)
            .transpose()?;
        let entry = queue_join_response_outbox_entry(
            &data_dir,
            peer_endpoint,
            workspace_id,
            metadata.request_id,
            response_text,
        )?;
        Ok(QueuedJoinResponseOutboxEntry { entry })
    })
}

pub(crate) fn runtime_list_join_response_outbox_result(
    data_dir: *const c_char,
    max_entries: usize,
) -> FfiResult<JoinResponseOutboxEntries> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let max_entries = if max_entries == 0 {
            JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_response_outbox_entries(&data_dir, max_entries)?;
        Ok(JoinResponseOutboxEntries { entries })
    })
}

pub(crate) fn runtime_list_due_join_response_outbox_result(
    data_dir: *const c_char,
    max_entries: usize,
) -> FfiResult<JoinResponseOutboxEntries> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let max_entries = if max_entries == 0 {
            JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES)
        };
        let now = current_unix_ms();
        let entries =
            list_join_response_outbox_entries(&data_dir, JOIN_RESPONSE_OUTBOX_LIST_MAX_ENTRIES)?
                .into_iter()
                .filter(|entry| is_join_response_outbox_entry_due(entry, now))
                .take(max_entries)
                .collect();
        Ok(JoinResponseOutboxEntries { entries })
    })
}

pub(crate) fn runtime_mark_join_response_outbox_entry_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
    status: *const c_char,
    error: *const c_char,
) -> FfiResult<MarkedJoinResponseOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_join_response_entry_id(&entry_id)?;
        let status = outbox_status_arg(&read_c_string(status, "status")?)?;
        let error = optional_outbox_error(error)?;
        let entry = mark_join_response_outbox_entry(&data_dir, &entry_id, status, error)?;
        Ok(MarkedJoinResponseOutboxEntry { entry })
    })
}

pub(crate) fn runtime_submit_join_response_outbox_entry_direct_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> FfiResult<SubmittedJoinResponseOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_join_response_entry_id(&entry_id)?;
        let entry = read_join_response_outbox_entry(&data_dir, &entry_id)?;
        let peer = direct_peer_address(entry.peer_endpoint.clone())?;
        let workspace_id = entry.workspace_id.clone().map(WorkspaceId);
        let response_bytes = entry.response_text.clone().into_bytes();

        run_on_worker_thread(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ffi_error("tokio_runtime_failed", error.to_string()))?;
            let submit_result = runtime.block_on(DirectTransport.submit_join_response(
                &peer,
                workspace_id.as_ref(),
                response_bytes,
            ));
            let entry = match submit_result {
                Ok(()) => mark_join_response_outbox_entry(
                    &data_dir,
                    &entry_id,
                    JoinResponseOutboxStatus::Delivered,
                    None,
                )?,
                Err(error) => {
                    let message = error.to_string();
                    let _ = mark_join_response_outbox_entry(
                        &data_dir,
                        &entry_id,
                        JoinResponseOutboxStatus::Failed,
                        Some(message.clone()),
                    );
                    return Err(ffi_error("runtime_submit_join_response_failed", message));
                }
            };
            Ok(SubmittedJoinResponseOutboxEntry { entry })
        })
    })
}

pub(crate) fn runtime_ack_join_response_outbox_entry_result(
    data_dir: *const c_char,
    entry_id: *const c_char,
) -> FfiResult<AcknowledgedJoinResponseOutboxEntry> {
    result_envelope(|| {
        let data_dir = PathBuf::from(read_c_string(data_dir, "data_dir")?);
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_join_response_entry_id(&entry_id)?;
        match fs::remove_file(outbox_entry_path(&data_dir, &entry_id)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ffi_error(
                    "join_response_outbox_ack_failed",
                    format!("could not acknowledge join response outbox entry: {error}"),
                ));
            }
        }
        Ok(AcknowledgedJoinResponseOutboxEntry { entry_id })
    })
}

fn queue_join_response_outbox_entry(
    data_dir: &Path,
    peer_endpoint: String,
    workspace_id: Option<String>,
    request_id: String,
    response_text: String,
) -> Result<JoinResponseOutboxEntry, FfiError> {
    validate_join_response_entry_id(&request_id)?;
    let now = current_unix_ms();
    let existing = read_join_response_outbox_entry(data_dir, &request_id).ok();
    let entry = JoinResponseOutboxEntry {
        schema_version: JOIN_RESPONSE_OUTBOX_SCHEMA_VERSION,
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
            .unwrap_or(JoinResponseOutboxStatus::Pending),
        error: existing.and_then(|entry| entry.error),
        response_text,
    };
    write_join_response_outbox_entry(data_dir, &entry)?;
    Ok(entry)
}

fn list_join_response_outbox_entries(
    data_dir: &Path,
    max_entries: usize,
) -> Result<Vec<JoinResponseOutboxEntry>, FfiError> {
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
                "join_response_outbox_read_failed",
                format!("could not read join response outbox: {error}"),
            ));
        }
    };
    let mut entries = Vec::new();
    for path in paths {
        entries.push(read_join_response_outbox_entry_path(&path)?);
    }
    entries.sort_by(|left, right| {
        left.created_at_unix_ms
            .cmp(&right.created_at_unix_ms)
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    entries.truncate(max_entries);
    Ok(entries)
}

fn mark_join_response_outbox_entry(
    data_dir: &Path,
    entry_id: &str,
    status: JoinResponseOutboxStatus,
    error: Option<String>,
) -> Result<JoinResponseOutboxEntry, FfiError> {
    let mut entry = read_join_response_outbox_entry(data_dir, entry_id)?;
    let now = current_unix_ms();
    entry.status = status;
    entry.updated_at_unix_ms = now;
    if matches!(
        status,
        JoinResponseOutboxStatus::Delivered | JoinResponseOutboxStatus::Failed
    ) {
        entry.attempt_count = entry.attempt_count.saturating_add(1);
        entry.last_attempt_at_unix_ms = Some(now);
    }
    if status == JoinResponseOutboxStatus::Delivered {
        entry.delivered_at_unix_ms = Some(now);
        entry.next_attempt_after_unix_ms = None;
        entry.error = None;
    } else if status == JoinResponseOutboxStatus::Failed {
        entry.next_attempt_after_unix_ms =
            Some(now.saturating_add(join_response_retry_delay_ms(entry.attempt_count)));
        entry.error = error;
    } else {
        entry.next_attempt_after_unix_ms = None;
        entry.error = None;
    }
    write_join_response_outbox_entry(data_dir, &entry)?;
    Ok(entry)
}

fn read_join_response_outbox_entry(
    data_dir: &Path,
    entry_id: &str,
) -> Result<JoinResponseOutboxEntry, FfiError> {
    validate_join_response_entry_id(entry_id)?;
    read_join_response_outbox_entry_path(&outbox_entry_path(data_dir, entry_id))
}

fn read_join_response_outbox_entry_path(path: &Path) -> Result<JoinResponseOutboxEntry, FfiError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ffi_error(
            "join_response_outbox_read_failed",
            format!("could not inspect join response outbox entry: {error}"),
        )
    })?;
    if metadata.len() as usize > JOIN_RESPONSE_OUTBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_response_outbox_entry_too_large",
            format!("join response outbox entry {} is too large", path.display()),
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| {
        ffi_error(
            "join_response_outbox_read_failed",
            format!("could not read join response outbox entry: {error}"),
        )
    })?;
    let entry: JoinResponseOutboxEntry = serde_json::from_str(&text).map_err(|error| {
        ffi_error(
            "join_response_outbox_read_failed",
            format!("could not parse join response outbox entry: {error}"),
        )
    })?;
    validate_join_response_outbox_entry(&entry)?;
    Ok(entry)
}

fn write_join_response_outbox_entry(
    data_dir: &Path,
    entry: &JoinResponseOutboxEntry,
) -> Result<(), FfiError> {
    validate_join_response_outbox_entry(entry)?;
    let outbox_dir = outbox_dir(data_dir);
    fs::create_dir_all(&outbox_dir).map_err(|error| {
        ffi_error(
            "join_response_outbox_write_failed",
            format!("could not create join response outbox: {error}"),
        )
    })?;
    let bytes = serde_json::to_vec_pretty(entry).map_err(|error| {
        ffi_error(
            "join_response_outbox_write_failed",
            format!("could not encode join response outbox entry: {error}"),
        )
    })?;
    if bytes.len() > JOIN_RESPONSE_OUTBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_response_outbox_entry_too_large",
            format!(
                "join response outbox entry is too large: {} bytes",
                bytes.len()
            ),
        ));
    }
    let sequence = JOIN_RESPONSE_OUTBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = outbox_dir.join(format!(".{}.{}.tmp", entry.entry_id, sequence));
    let final_path = outbox_entry_path(data_dir, &entry.entry_id);
    write_private_file(&temp_path, &bytes)?;
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        ffi_error(
            "join_response_outbox_write_failed",
            format!("could not commit join response outbox entry: {error}"),
        )
    })
}

fn validate_join_response_outbox_entry(entry: &JoinResponseOutboxEntry) -> Result<(), FfiError> {
    if entry.schema_version != JOIN_RESPONSE_OUTBOX_SCHEMA_VERSION {
        return Err(ffi_error(
            "join_response_outbox_schema_unsupported",
            format!(
                "join response outbox entry schema {} is unsupported",
                entry.schema_version
            ),
        ));
    }
    validate_join_response_entry_id(&entry.entry_id)?;
    validate_join_response_entry_id(&entry.request_id)?;
    if entry.entry_id != entry.request_id {
        return Err(ffi_error(
            "join_response_outbox_entry_id_mismatch",
            "join response outbox entry ID must match request ID",
        ));
    }
    if let Some(workspace_id) = &entry.workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    direct_peer_address(entry.peer_endpoint.clone())?;
    if let Some(error) = &entry.error {
        validate_outbox_error(error)?;
    }
    let metadata = validate_join_response_payload(&entry.response_text)?;
    if metadata.request_id != entry.request_id {
        return Err(ffi_error(
            "join_response_outbox_payload_id_mismatch",
            "join response payload request ID must match the outbox entry",
        ));
    }
    Ok(())
}

fn join_response_retry_delay_ms(attempt_count: u32) -> u64 {
    let exponent = attempt_count.saturating_sub(1).min(5);
    15_000u64
        .saturating_mul(1u64 << exponent)
        .min(JOIN_RESPONSE_OUTBOX_MAX_RETRY_DELAY_MS)
}

fn is_join_response_outbox_entry_due(entry: &JoinResponseOutboxEntry, now_unix_ms: u64) -> bool {
    if entry.peer_endpoint.trim().is_empty() {
        return false;
    }
    if matches!(
        entry.status,
        JoinResponseOutboxStatus::Delivered | JoinResponseOutboxStatus::Acknowledged
    ) {
        return false;
    }
    entry
        .next_attempt_after_unix_ms
        .map(|next_attempt_after| next_attempt_after <= now_unix_ms)
        .unwrap_or(true)
}

fn outbox_status_arg(status: &str) -> Result<JoinResponseOutboxStatus, FfiError> {
    match status.trim() {
        "pending" => Ok(JoinResponseOutboxStatus::Pending),
        "delivered" => Ok(JoinResponseOutboxStatus::Delivered),
        "failed" => Ok(JoinResponseOutboxStatus::Failed),
        "acknowledged" => Ok(JoinResponseOutboxStatus::Acknowledged),
        _ => Err(ffi_error(
            "join_response_outbox_status_invalid",
            "join response outbox status must be pending, delivered, failed, or acknowledged",
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
    if error.len() > JOIN_RESPONSE_OUTBOX_ERROR_MAX_BYTES {
        return Err(ffi_error(
            "join_response_outbox_error_too_large",
            "join response outbox error is too large",
        ));
    }
    Ok(())
}

fn outbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(JOIN_RESPONSE_OUTBOX_DIR)
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
            "join_response_outbox_write_failed",
            format!("could not create join response outbox entry: {error}"),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        ffi_error(
            "join_response_outbox_write_failed",
            format!("could not write join response outbox entry: {error}"),
        )
    })?;
    file.sync_all().map_err(|error| {
        ffi_error(
            "join_response_outbox_write_failed",
            format!("could not flush join response outbox entry: {error}"),
        )
    })
}
