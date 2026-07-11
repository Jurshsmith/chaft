use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_net::NetError;
use chaft_net_direct::{JoinResponseInbox, MAX_JOIN_RESPONSE_SUBMISSION_BYTES};
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::direct_workspace_id_arg,
    input::{KEY_TRANSFER_JSON_MAX_BYTES, read_c_string},
};

const JOIN_RESPONSE_INBOX_DIR: &str = "join-response-inbox";
const JOIN_RESPONSE_INBOX_SCHEMA_VERSION: u32 = 1;
pub(crate) const JOIN_RESPONSE_INBOX_ENTRY_MAX_BYTES: usize = KEY_TRANSFER_JSON_MAX_BYTES + 4096;
const JOIN_RESPONSE_INBOX_ENTRY_ID_MAX_BYTES: usize = 128;
const JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES: usize = 100;

static JOIN_RESPONSE_INBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(crate) struct FileJoinResponseInbox {
    data_dir: PathBuf,
}

impl FileJoinResponseInbox {
    pub(crate) fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }
}

impl JoinResponseInbox for FileJoinResponseInbox {
    fn submit_join_response(
        &self,
        workspace_id: Option<&str>,
        response: Vec<u8>,
    ) -> Result<(), NetError> {
        let response_text = String::from_utf8(response).map_err(|_| {
            NetError::Protocol("join response payload must be UTF-8 JSON".to_owned())
        })?;
        validate_join_response_payload(&response_text)
            .map_err(|error| NetError::Protocol(error.message))?;
        write_join_response_inbox_entry(&self.data_dir, workspace_id, &response_text)
            .map(|_| ())
            .map_err(|error| NetError::Protocol(error.message))
    }

    fn list_join_responses(
        &self,
        workspace_id: &str,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let entries =
            list_join_response_inbox_entries(&self.data_dir, JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES)
                .map_err(|error| NetError::Protocol(error.message))?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.workspace_id.as_deref() == Some(workspace_id))
            .take(max_entries)
            .map(|entry| entry.response_text.into_bytes())
            .collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinResponseInboxEntry {
    schema_version: u32,
    entry_id: String,
    request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<String>,
    received_at_unix_ms: u64,
    response_text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinResponseInboxEntries {
    entries: Vec<JoinResponseInboxEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcknowledgedJoinResponseInboxEntry {
    entry_id: String,
}

pub(crate) fn runtime_list_join_response_inbox_result(
    data_dir: *const std::ffi::c_char,
    max_entries: usize,
) -> FfiResult<JoinResponseInboxEntries> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let max_entries = if max_entries == 0 {
            JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_response_inbox_entries(&PathBuf::from(data_dir), max_entries)?;
        Ok(JoinResponseInboxEntries { entries })
    })
}

pub(crate) fn runtime_stage_join_response_inbox_result(
    data_dir: *const std::ffi::c_char,
    workspace_id: *const std::ffi::c_char,
    response_json: *const std::ffi::c_char,
) -> FfiResult<JoinResponseInboxEntry> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let workspace_id = read_c_string(workspace_id, "workspace_id")?;
        let response_json = read_c_string(response_json, "response_json")?;
        write_join_response_inbox_entry(
            &PathBuf::from(data_dir),
            Some(&workspace_id),
            &response_json,
        )
    })
}

pub(crate) fn runtime_ack_join_response_inbox_entry_result(
    data_dir: *const std::ffi::c_char,
    entry_id: *const std::ffi::c_char,
) -> FfiResult<AcknowledgedJoinResponseInboxEntry> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_join_response_entry_id(&entry_id)?;
        let path = inbox_entry_path(&PathBuf::from(data_dir), &entry_id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ffi_error(
                    "join_response_inbox_ack_failed",
                    format!("could not acknowledge join response inbox entry: {error}"),
                ));
            }
        }
        Ok(AcknowledgedJoinResponseInboxEntry { entry_id })
    })
}

fn write_join_response_inbox_entry(
    data_dir: &Path,
    workspace_id: Option<&str>,
    response_text: &str,
) -> Result<JoinResponseInboxEntry, FfiError> {
    let metadata = validate_join_response_payload(response_text)?;
    let inbox_dir = inbox_dir(data_dir);
    fs::create_dir_all(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not create join response inbox: {error}"),
        )
    })?;
    let received_at_unix_ms = current_unix_ms();
    let entry_id = if metadata.request_id.is_empty() {
        format!(
            "jrsp_{}_{}",
            received_at_unix_ms,
            JOIN_RESPONSE_INBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    } else {
        metadata.request_id.clone()
    };
    if let Ok(existing) = read_join_response_inbox_entry(data_dir, &entry_id) {
        return Ok(existing);
    }
    let workspace_id = workspace_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or(metadata.workspace_id);
    if let Some(workspace_id) = &workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    let entry = JoinResponseInboxEntry {
        schema_version: JOIN_RESPONSE_INBOX_SCHEMA_VERSION,
        entry_id: entry_id.clone(),
        request_id: metadata.request_id,
        workspace_id,
        received_at_unix_ms,
        response_text: response_text.to_owned(),
    };
    write_join_response_inbox_entry_file(data_dir, &entry)?;
    Ok(entry)
}

fn list_join_response_inbox_entries(
    data_dir: &Path,
    max_entries: usize,
) -> Result<Vec<JoinResponseInboxEntry>, FfiError> {
    let inbox_dir = inbox_dir(data_dir);
    let mut paths = match fs::read_dir(&inbox_dir) {
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
                "join_response_inbox_read_failed",
                format!("could not read join response inbox: {error}"),
            ));
        }
    };
    paths.sort();

    let mut entries = Vec::new();
    for path in paths.into_iter().take(max_entries) {
        entries.push(read_join_response_inbox_entry_path(&path)?);
    }
    Ok(entries)
}

fn read_join_response_inbox_entry(
    data_dir: &Path,
    entry_id: &str,
) -> Result<JoinResponseInboxEntry, FfiError> {
    validate_join_response_entry_id(entry_id)?;
    read_join_response_inbox_entry_path(&inbox_entry_path(data_dir, entry_id))
}

fn read_join_response_inbox_entry_path(path: &Path) -> Result<JoinResponseInboxEntry, FfiError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ffi_error(
            "join_response_inbox_read_failed",
            format!("could not inspect join response inbox entry: {error}"),
        )
    })?;
    if metadata.len() as usize > JOIN_RESPONSE_INBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_response_inbox_entry_too_large",
            format!("join response inbox entry {} is too large", path.display()),
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| {
        ffi_error(
            "join_response_inbox_read_failed",
            format!("could not read join response inbox entry: {error}"),
        )
    })?;
    let entry: JoinResponseInboxEntry = serde_json::from_str(&text).map_err(|error| {
        ffi_error(
            "join_response_inbox_read_failed",
            format!("could not parse join response inbox entry: {error}"),
        )
    })?;
    validate_join_response_inbox_entry(&entry)?;
    Ok(entry)
}

fn write_join_response_inbox_entry_file(
    data_dir: &Path,
    entry: &JoinResponseInboxEntry,
) -> Result<(), FfiError> {
    validate_join_response_inbox_entry(entry)?;
    let inbox_dir = inbox_dir(data_dir);
    fs::create_dir_all(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not create join response inbox: {error}"),
        )
    })?;
    let bytes = serde_json::to_vec_pretty(entry).map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not encode join response inbox entry: {error}"),
        )
    })?;
    if bytes.len() > JOIN_RESPONSE_INBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_response_inbox_entry_too_large",
            format!(
                "join response inbox entry is too large: {} bytes",
                bytes.len()
            ),
        ));
    }

    let sequence = JOIN_RESPONSE_INBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = inbox_dir.join(format!(".{}.{}.tmp", entry.entry_id, sequence));
    let final_path = inbox_entry_path(data_dir, &entry.entry_id);
    write_private_file(&temp_path, &bytes)?;
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not commit join response inbox entry: {error}"),
        )
    })
}

fn validate_join_response_inbox_entry(entry: &JoinResponseInboxEntry) -> Result<(), FfiError> {
    if entry.schema_version != JOIN_RESPONSE_INBOX_SCHEMA_VERSION {
        return Err(ffi_error(
            "join_response_inbox_schema_unsupported",
            format!(
                "join response inbox entry schema {} is unsupported",
                entry.schema_version
            ),
        ));
    }
    validate_join_response_entry_id(&entry.entry_id)?;
    validate_join_response_entry_id(&entry.request_id)?;
    if entry.entry_id != entry.request_id {
        return Err(ffi_error(
            "join_response_inbox_entry_id_mismatch",
            "join response inbox entry ID must match request ID",
        ));
    }
    if let Some(workspace_id) = &entry.workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    let metadata = validate_join_response_payload(&entry.response_text)?;
    if metadata.request_id != entry.request_id {
        return Err(ffi_error(
            "join_response_inbox_payload_id_mismatch",
            "join response payload request ID must match the inbox entry",
        ));
    }
    Ok(())
}

pub(crate) struct JoinResponseMetadata {
    pub(crate) request_id: String,
    pub(crate) workspace_id: Option<String>,
}

pub(crate) fn validate_join_response_payload(
    response_text: &str,
) -> Result<JoinResponseMetadata, FfiError> {
    if response_text.trim().is_empty() {
        return Err(ffi_error(
            "join_response_payload_empty",
            "join response payload is empty",
        ));
    }
    if response_text.len() > MAX_JOIN_RESPONSE_SUBMISSION_BYTES {
        return Err(ffi_error(
            "join_response_payload_too_large",
            format!(
                "join response payload is too large: {} bytes",
                response_text.len()
            ),
        ));
    }
    let value = serde_json::from_str::<serde_json::Value>(response_text).map_err(|error| {
        ffi_error(
            "join_response_payload_invalid",
            format!("join response payload is not valid JSON: {error}"),
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        ffi_error(
            "join_response_payload_invalid",
            "join response payload must be a JSON object",
        )
    })?;
    let kind = object
        .get("kind")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .ok_or_else(|| {
            ffi_error(
                "join_response_payload_invalid",
                "join response payload kind is required",
            )
        })?;
    if kind != "chaft.workspace-invite.v1"
        && kind != "chaft.workspace-invite-response.v1"
        && kind != "chaft.workspace-join-response.v1"
    {
        return Err(ffi_error(
            "join_response_payload_invalid",
            "join response payload kind is unsupported",
        ));
    }
    if kind == "chaft.workspace-join-response.v1" {
        let resolution = object
            .get("resolution")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ffi_error(
                    "join_response_resolution_required",
                    "join response payload must include resolution",
                )
            })?;
        if !matches!(resolution, "approved" | "declined" | "revoked") {
            return Err(ffi_error(
                "join_response_resolution_invalid",
                "join response payload resolution is unsupported",
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
                "join_response_request_id_required",
                "join response payload must include requestId",
            )
        })?
        .to_owned();
    validate_join_response_entry_id(&request_id)?;
    let workspace_id = object
        .get("workspaceId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(workspace_id) = &workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    Ok(JoinResponseMetadata {
        request_id,
        workspace_id,
    })
}

pub(crate) fn validate_join_response_entry_id(entry_id: &str) -> Result<(), FfiError> {
    if entry_id.is_empty() {
        return Err(ffi_error(
            "join_response_entry_id_required",
            "join response entry ID is required",
        ));
    }
    if entry_id.len() > JOIN_RESPONSE_INBOX_ENTRY_ID_MAX_BYTES
        || !entry_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(ffi_error(
            "join_response_entry_id_invalid",
            "join response entry ID is invalid",
        ));
    }
    Ok(())
}

fn inbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(JOIN_RESPONSE_INBOX_DIR)
}

fn inbox_entry_path(data_dir: &Path, entry_id: &str) -> PathBuf {
    inbox_dir(data_dir).join(format!("{entry_id}.json"))
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
            "join_response_inbox_write_failed",
            format!("could not create join response inbox entry: {error}"),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not write join response inbox entry: {error}"),
        )
    })?;
    file.sync_all().map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not flush join response inbox entry: {error}"),
        )
    })
}
