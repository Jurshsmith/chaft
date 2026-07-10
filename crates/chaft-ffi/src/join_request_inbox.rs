use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_net::NetError;
use chaft_net_direct::JoinRequestInbox;
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    input::read_c_string,
};

const JOIN_REQUEST_INBOX_DIR: &str = "join-request-inbox";
const JOIN_REQUEST_INBOX_SCHEMA_VERSION: u32 = 1;
pub(crate) const JOIN_REQUEST_INBOX_ENTRY_MAX_BYTES: usize = 32 * 1024;
const JOIN_REQUEST_INBOX_ENTRY_ID_MAX_BYTES: usize = 128;
const JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES: usize = 100;

static JOIN_REQUEST_INBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub(crate) struct FileJoinRequestInbox {
    data_dir: PathBuf,
}

impl FileJoinRequestInbox {
    pub(crate) fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }
}

impl JoinRequestInbox for FileJoinRequestInbox {
    fn submit_join_request(
        &self,
        workspace_id: Option<&str>,
        request: Vec<u8>,
    ) -> Result<(), NetError> {
        let request_text = String::from_utf8(request).map_err(|_| {
            NetError::Protocol("join request payload must be UTF-8 JSON".to_owned())
        })?;
        validate_join_request_payload(&request_text)
            .map_err(|error| NetError::Protocol(error.message))?;
        write_join_request_inbox_entry(&self.data_dir, workspace_id, &request_text)
            .map(|_| ())
            .map_err(|error| NetError::Protocol(error.message))
    }

    fn list_join_requests(
        &self,
        workspace_id: &str,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let entries =
            list_join_request_inbox_entries(&self.data_dir, JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES)
                .map_err(|error| NetError::Protocol(error.message))?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.workspace_id.as_deref() == Some(workspace_id))
            .take(max_entries)
            .map(|entry| entry.request_text.into_bytes())
            .collect())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinRequestInboxEntry {
    schema_version: u32,
    entry_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<String>,
    received_at_unix_ms: u64,
    request_text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JoinRequestInboxEntries {
    entries: Vec<JoinRequestInboxEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcknowledgedJoinRequestInboxEntry {
    entry_id: String,
}

pub(crate) fn runtime_list_join_request_inbox_result(
    data_dir: *const std::ffi::c_char,
    max_entries: usize,
) -> FfiResult<JoinRequestInboxEntries> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let max_entries = if max_entries == 0 {
            JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_request_inbox_entries(&PathBuf::from(data_dir), max_entries)?;
        Ok(JoinRequestInboxEntries { entries })
    })
}

pub(crate) fn runtime_ack_join_request_inbox_entry_result(
    data_dir: *const std::ffi::c_char,
    entry_id: *const std::ffi::c_char,
) -> FfiResult<AcknowledgedJoinRequestInboxEntry> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let entry_id = read_c_string(entry_id, "entry_id")?;
        validate_join_request_inbox_entry_id(&entry_id)?;
        let path = inbox_entry_path(&PathBuf::from(data_dir), &entry_id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ffi_error(
                    "join_request_inbox_ack_failed",
                    format!("could not acknowledge join request inbox entry: {error}"),
                ));
            }
        }
        Ok(AcknowledgedJoinRequestInboxEntry { entry_id })
    })
}

fn write_join_request_inbox_entry(
    data_dir: &Path,
    workspace_id: Option<&str>,
    request_text: &str,
) -> Result<JoinRequestInboxEntry, FfiError> {
    let metadata = validate_join_request_payload(request_text)?;
    let inbox_dir = inbox_dir(data_dir);
    fs::create_dir_all(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not create join request inbox: {error}"),
        )
    })?;
    let received_at_unix_ms = current_unix_ms();
    let entry_id = metadata.request_id.unwrap_or_else(|| {
        format!(
            "jr_{}_{}",
            received_at_unix_ms,
            JOIN_REQUEST_INBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    });
    if let Ok(existing) = read_join_request_inbox_entry(data_dir, &entry_id) {
        return Ok(existing);
    }
    let entry = JoinRequestInboxEntry {
        schema_version: JOIN_REQUEST_INBOX_SCHEMA_VERSION,
        entry_id: entry_id.clone(),
        workspace_id: workspace_id.map(ToOwned::to_owned),
        received_at_unix_ms,
        request_text: request_text.to_owned(),
    };
    let bytes = serde_json::to_vec_pretty(&entry).map_err(|error| {
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not encode join request inbox entry: {error}"),
        )
    })?;
    if bytes.len() > JOIN_REQUEST_INBOX_ENTRY_MAX_BYTES {
        return Err(ffi_error(
            "join_request_inbox_entry_too_large",
            format!(
                "join request inbox entry is too large: {} bytes",
                bytes.len()
            ),
        ));
    }

    let temp_path = inbox_dir.join(format!(".{entry_id}.tmp"));
    let final_path = inbox_dir.join(format!("{entry_id}.json"));
    write_private_file(&temp_path, &bytes)?;
    fs::rename(&temp_path, &final_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not commit join request inbox entry: {error}"),
        )
    })?;
    Ok(entry)
}

fn list_join_request_inbox_entries(
    data_dir: &Path,
    max_entries: usize,
) -> Result<Vec<JoinRequestInboxEntry>, FfiError> {
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
                "join_request_inbox_read_failed",
                format!("could not read join request inbox: {error}"),
            ));
        }
    };
    paths.sort();

    let mut entries = Vec::new();
    for path in paths.into_iter().take(max_entries) {
        let metadata = fs::metadata(&path).map_err(|error| {
            ffi_error(
                "join_request_inbox_read_failed",
                format!("could not inspect join request inbox entry: {error}"),
            )
        })?;
        if metadata.len() as usize > JOIN_REQUEST_INBOX_ENTRY_MAX_BYTES {
            return Err(ffi_error(
                "join_request_inbox_entry_too_large",
                format!("join request inbox entry {} is too large", path.display()),
            ));
        }
        let text = fs::read_to_string(&path).map_err(|error| {
            ffi_error(
                "join_request_inbox_read_failed",
                format!("could not read join request inbox entry: {error}"),
            )
        })?;
        let entry: JoinRequestInboxEntry = serde_json::from_str(&text).map_err(|error| {
            ffi_error(
                "join_request_inbox_read_failed",
                format!("could not parse join request inbox entry: {error}"),
            )
        })?;
        if entry.schema_version != JOIN_REQUEST_INBOX_SCHEMA_VERSION {
            return Err(ffi_error(
                "join_request_inbox_schema_unsupported",
                format!(
                    "join request inbox entry schema {} is unsupported",
                    entry.schema_version
                ),
            ));
        }
        validate_join_request_inbox_entry_id(&entry.entry_id)?;
        validate_join_request_payload(&entry.request_text)?;
        entries.push(entry);
    }
    Ok(entries)
}

struct JoinRequestPayloadMetadata {
    request_id: Option<String>,
}

fn validate_join_request_payload(
    request_text: &str,
) -> Result<JoinRequestPayloadMetadata, FfiError> {
    if request_text.trim().is_empty() {
        return Err(ffi_error(
            "join_request_payload_empty",
            "join request payload is empty",
        ));
    }
    if request_text.len() > JOIN_REQUEST_INBOX_ENTRY_MAX_BYTES {
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
    let request_id = object
        .get("requestId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let Some(request_id) = &request_id {
        validate_join_request_inbox_entry_id(request_id)?;
    }
    Ok(JoinRequestPayloadMetadata { request_id })
}

fn read_join_request_inbox_entry(
    data_dir: &Path,
    entry_id: &str,
) -> Result<JoinRequestInboxEntry, FfiError> {
    validate_join_request_inbox_entry_id(entry_id)?;
    let path = inbox_entry_path(data_dir, entry_id);
    let text = fs::read_to_string(&path).map_err(|error| {
        ffi_error(
            "join_request_inbox_read_failed",
            format!("could not read join request inbox entry: {error}"),
        )
    })?;
    let entry: JoinRequestInboxEntry = serde_json::from_str(&text).map_err(|error| {
        ffi_error(
            "join_request_inbox_read_failed",
            format!("could not parse join request inbox entry: {error}"),
        )
    })?;
    if entry.schema_version != JOIN_REQUEST_INBOX_SCHEMA_VERSION {
        return Err(ffi_error(
            "join_request_inbox_schema_unsupported",
            format!(
                "join request inbox entry schema {} is unsupported",
                entry.schema_version
            ),
        ));
    }
    validate_join_request_inbox_entry_id(&entry.entry_id)?;
    validate_join_request_payload(&entry.request_text)?;
    Ok(entry)
}

fn validate_join_request_inbox_entry_id(entry_id: &str) -> Result<(), FfiError> {
    if entry_id.is_empty() {
        return Err(ffi_error(
            "join_request_inbox_entry_id_required",
            "join request inbox entry ID is required",
        ));
    }
    if entry_id.len() > JOIN_REQUEST_INBOX_ENTRY_ID_MAX_BYTES
        || !entry_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(ffi_error(
            "join_request_inbox_entry_id_invalid",
            "join request inbox entry ID is invalid",
        ));
    }
    Ok(())
}

fn inbox_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(JOIN_REQUEST_INBOX_DIR)
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
            "join_request_inbox_write_failed",
            format!("could not create join request inbox entry: {error}"),
        )
    })?;
    file.write_all(bytes).map_err(|error| {
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not write join request inbox entry: {error}"),
        )
    })?;
    file.sync_all().map_err(|error| {
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not flush join request inbox entry: {error}"),
        )
    })
}
