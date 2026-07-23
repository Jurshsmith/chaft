use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use chaft_net::NetError;
use chaft_net_direct::{JoinResponseInbox, MAX_JOIN_RESPONSE_SUBMISSION_BYTES};
use chaft_runtime::WorkspaceInviteResponse;
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::{direct_workspace_id_arg, ffi_device_id_arg},
    input::{KEY_TRANSFER_JSON_MAX_BYTES, read_c_string, read_c_string_with_max_bytes},
};

const JOIN_RESPONSE_INBOX_DIR: &str = "join-response-inbox";
const JOIN_RESPONSE_INBOX_SCHEMA_VERSION: u32 = 1;
pub(crate) const JOIN_RESPONSE_INBOX_ENTRY_MAX_BYTES: usize = KEY_TRANSFER_JSON_MAX_BYTES + 4096;
pub(crate) const JOIN_RESPONSE_INBOX_ENTRY_ID_MAX_BYTES: usize = 128;
const JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES: usize = 100;
pub(crate) const JOIN_RESPONSE_INBOX_MAX_ENTRIES: usize = 1024;
const JOIN_RESPONSE_INBOX_SCOPE_MAX_REQUEST_IDS: usize = JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES;
const JOIN_RESPONSE_INBOX_SCOPE_JSON_MAX_BYTES: usize =
    JOIN_RESPONSE_INBOX_SCOPE_MAX_REQUEST_IDS * (JOIN_RESPONSE_INBOX_ENTRY_ID_MAX_BYTES + 3) + 2;

static JOIN_RESPONSE_INBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static JOIN_RESPONSE_INBOX_WRITE_LOCK: Mutex<()> = Mutex::new(());

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
        let entries = list_join_response_inbox_entries(
            &self.data_dir,
            Some(workspace_id),
            max_entries.min(JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES),
        )
        .map_err(|error| NetError::Protocol(error.message))?;
        Ok(entries
            .into_iter()
            .map(|entry| entry.response_text.into_bytes())
            .collect())
    }

    fn list_join_responses_for_requests(
        &self,
        workspace_id: &str,
        request_ids: &[String],
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        if request_ids.is_empty() || max_entries == 0 {
            return Ok(Vec::new());
        }
        let workspace_id = direct_workspace_id_arg(workspace_id.to_owned())
            .map_err(|error| NetError::Protocol(error.message))?;
        let request_ids = request_ids
            .iter()
            .map(|request_id| {
                let request_id = request_id.trim().to_owned();
                validate_join_response_entry_id(&request_id)
                    .map_err(|error| NetError::Protocol(error.message))?;
                Ok(request_id)
            })
            .collect::<Result<HashSet<_>, NetError>>()?;
        let entries = list_join_response_inbox_entries_matching(
            &self.data_dir,
            max_entries.min(JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES),
            |entry| {
                Ok(entry.workspace_id.as_deref() == Some(workspace_id.as_str())
                    && request_ids.contains(&entry.request_id))
            },
        )
        .map_err(|error| NetError::Protocol(error.message))?;
        Ok(entries
            .into_iter()
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
        let entries =
            list_join_response_inbox_entries(&PathBuf::from(data_dir), None, max_entries)?;
        Ok(JoinResponseInboxEntries { entries })
    })
}

pub(crate) fn runtime_list_join_response_inbox_scoped_result(
    data_dir: *const std::ffi::c_char,
    local_device_id: *const std::ffi::c_char,
    pending_request_ids_json: *const std::ffi::c_char,
    max_entries: usize,
) -> FfiResult<JoinResponseInboxEntries> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let local_device_id = ffi_device_id_arg(read_c_string(local_device_id, "device_id")?)?;
        let pending_request_ids_json = read_c_string_with_max_bytes(
            pending_request_ids_json,
            "pending_request_ids_json",
            JOIN_RESPONSE_INBOX_SCOPE_JSON_MAX_BYTES,
            "join_response_inbox_scope_too_large",
            "pending join response request IDs",
        )?;
        let pending_request_ids =
            parse_pending_join_response_request_ids(&pending_request_ids_json)?;
        let max_entries = if max_entries == 0 {
            JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_RESPONSE_INBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_response_inbox_entries_matching(
            &PathBuf::from(data_dir),
            max_entries,
            |entry| {
                if !pending_request_ids.contains(&entry.request_id) {
                    return Ok(false);
                }
                let metadata = validate_join_response_payload(&entry.response_text)?;
                Ok(metadata
                    .invitee_device_id
                    .as_deref()
                    .is_none_or(|invitee_device_id| invitee_device_id == local_device_id))
            },
        )?;
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
    let workspace_id = resolve_join_response_workspace_id(workspace_id, metadata.workspace_id)?;
    let _write_guard = JOIN_RESPONSE_INBOX_WRITE_LOCK.lock().map_err(|_| {
        ffi_error(
            "join_response_inbox_write_failed",
            "join response inbox write lock is unavailable",
        )
    })?;
    let inbox_dir = inbox_dir(data_dir);
    fs::create_dir_all(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_response_inbox_write_failed",
            format!("could not create join response inbox: {error}"),
        )
    })?;
    let entry_id = metadata.request_id;
    let existing_path = inbox_entry_path(data_dir, &entry_id);
    if existing_path.exists() {
        let existing = read_join_response_inbox_entry(data_dir, &entry_id)?;
        if existing.workspace_id == workspace_id && existing.response_text == response_text {
            return Ok(existing);
        }
        return Err(ffi_error(
            "join_response_inbox_entry_conflict",
            format!("join response inbox entry {entry_id} conflicts with an existing response"),
        ));
    }
    let entry = JoinResponseInboxEntry {
        schema_version: JOIN_RESPONSE_INBOX_SCHEMA_VERSION,
        entry_id: entry_id.clone(),
        request_id: entry_id,
        workspace_id,
        received_at_unix_ms: current_unix_ms(),
        response_text: response_text.to_owned(),
    };
    let bytes = encode_join_response_inbox_entry(&entry)?;
    let sequence = JOIN_RESPONSE_INBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = inbox_dir.join(format!(
        ".{}.{}.{}.tmp",
        entry.entry_id,
        std::process::id(),
        sequence
    ));
    let final_path = inbox_entry_path(data_dir, &entry.entry_id);
    if let Err(error) = write_private_file(&temp_path, &bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    let capacity_result = (|| {
        let entry_count = inbox_json_entry_count(&inbox_dir)?;
        let prune_count = entry_count
            .saturating_sub(JOIN_RESPONSE_INBOX_MAX_ENTRIES)
            .saturating_add(1);
        if entry_count >= JOIN_RESPONSE_INBOX_MAX_ENTRIES {
            for _ in 0..prune_count {
                prune_oldest_valid_join_response_inbox_entry(data_dir)?;
            }
        }
        Ok::<(), FfiError>(())
    })();
    if let Err(error) = capacity_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    match fs::hard_link(&temp_path, &final_path) {
        Ok(()) => {
            let _ = fs::remove_file(&temp_path);
            Ok(entry)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temp_path);
            let existing = read_join_response_inbox_entry(data_dir, &entry.entry_id)?;
            if existing.workspace_id == entry.workspace_id
                && existing.response_text == entry.response_text
            {
                Ok(existing)
            } else {
                Err(ffi_error(
                    "join_response_inbox_entry_conflict",
                    format!(
                        "join response inbox entry {} conflicts with an existing response",
                        entry.entry_id
                    ),
                ))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(ffi_error(
                "join_response_inbox_write_failed",
                format!("could not commit join response inbox entry: {error}"),
            ))
        }
    }
}

fn list_join_response_inbox_entries(
    data_dir: &Path,
    workspace_id: Option<&str>,
    max_entries: usize,
) -> Result<Vec<JoinResponseInboxEntry>, FfiError> {
    let workspace_id = workspace_id.map(str::trim);
    list_join_response_inbox_entries_matching(data_dir, max_entries, |entry| {
        Ok(workspace_id
            .is_none_or(|workspace_id| entry.workspace_id.as_deref() == Some(workspace_id)))
    })
}

fn list_join_response_inbox_entries_matching(
    data_dir: &Path,
    max_entries: usize,
    mut matches: impl FnMut(&JoinResponseInboxEntry) -> Result<bool, FfiError>,
) -> Result<Vec<JoinResponseInboxEntry>, FfiError> {
    if max_entries == 0 {
        return Ok(Vec::new());
    }
    let inbox_dir = inbox_dir(data_dir);
    let paths = match fs::read_dir(&inbox_dir) {
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
    let mut entries = Vec::with_capacity(max_entries.min(JOIN_RESPONSE_INBOX_MAX_ENTRIES));
    for path in paths {
        let entry = read_join_response_inbox_entry_path(&path)?;
        if !matches(&entry)? {
            continue;
        }
        entries.push(entry);
        if entries.len() > max_entries {
            sort_join_response_inbox_entries_newest_first(&mut entries);
            entries.truncate(max_entries);
        }
    }
    sort_join_response_inbox_entries_newest_first(&mut entries);
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

fn encode_join_response_inbox_entry(entry: &JoinResponseInboxEntry) -> Result<Vec<u8>, FfiError> {
    validate_join_response_inbox_entry(entry)?;
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
    Ok(bytes)
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
    if metadata.workspace_id != entry.workspace_id {
        return Err(ffi_error(
            "join_response_inbox_payload_workspace_mismatch",
            "join response payload workspace must match the inbox entry",
        ));
    }
    Ok(())
}

pub(crate) struct JoinResponseMetadata {
    pub(crate) request_id: String,
    pub(crate) workspace_id: Option<String>,
    invitee_device_id: Option<String>,
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
    if object.get("schemaVersion").and_then(|value| value.as_u64()) != Some(1) {
        return Err(ffi_error(
            "join_response_payload_invalid",
            "join response payload schema version must be 1",
        ));
    }
    let workspace_id = object
        .get("workspaceId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ffi_error(
                "join_response_workspace_id_required",
                "join response payload must include workspaceId",
            )
        })?
        .to_owned();
    direct_workspace_id_arg(workspace_id.clone())?;
    let mut invitee_device_id = None;
    if kind == "chaft.workspace-invite.v1" {
        for field in ["inviteId", "inviteeDeviceId", "role"] {
            if object
                .get(field)
                .and_then(|value| value.as_str())
                .map(str::trim)
                .is_none_or(str::is_empty)
            {
                return Err(ffi_error(
                    "join_response_payload_invalid",
                    format!("legacy invite response payload must include {field}"),
                ));
            }
        }
        invitee_device_id = Some(ffi_device_id_arg(
            object["inviteeDeviceId"]
                .as_str()
                .unwrap()
                .trim()
                .to_owned(),
        )?);
    }
    if kind == "chaft.workspace-invite-response.v1" {
        let response =
            serde_json::from_value::<WorkspaceInviteResponse>(value.clone()).map_err(|error| {
                ffi_error(
                    "join_response_payload_invalid",
                    format!("secure invite response payload is incomplete: {error}"),
                )
            })?;
        if response.request_id().trim().is_empty()
            || response.workspace_id().trim().is_empty()
            || response.invitee_device_id().trim().is_empty()
            || response.responder_signature.trim().is_empty()
        {
            return Err(ffi_error(
                "join_response_payload_invalid",
                "secure invite response payload is incomplete",
            ));
        }
        invitee_device_id = Some(ffi_device_id_arg(response.invitee_device_id().to_owned())?);
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
    Ok(JoinResponseMetadata {
        request_id,
        workspace_id: Some(workspace_id),
        invitee_device_id,
    })
}

fn parse_pending_join_response_request_ids(value: &str) -> Result<HashSet<String>, FfiError> {
    let request_ids = serde_json::from_str::<Vec<String>>(value).map_err(|error| {
        ffi_error(
            "join_response_inbox_scope_invalid",
            format!("pending join response request IDs must be a JSON string array: {error}"),
        )
    })?;
    if request_ids.len() > JOIN_RESPONSE_INBOX_SCOPE_MAX_REQUEST_IDS {
        return Err(ffi_error(
            "join_response_inbox_scope_too_many_request_ids",
            format!(
                "pending join response request IDs exceed the maximum of {JOIN_RESPONSE_INBOX_SCOPE_MAX_REQUEST_IDS}"
            ),
        ));
    }
    request_ids
        .into_iter()
        .map(|request_id| {
            let request_id = request_id.trim().to_owned();
            validate_join_response_entry_id(&request_id)?;
            Ok(request_id)
        })
        .collect()
}

fn resolve_join_response_workspace_id(
    transport_workspace_id: Option<&str>,
    payload_workspace_id: Option<String>,
) -> Result<Option<String>, FfiError> {
    let transport_workspace_id = transport_workspace_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .map(direct_workspace_id_arg)
        .transpose()?;
    if let (Some(transport_workspace_id), Some(payload_workspace_id)) =
        (&transport_workspace_id, &payload_workspace_id)
        && transport_workspace_id != payload_workspace_id
    {
        return Err(ffi_error(
            "join_response_workspace_id_mismatch",
            "join response transport workspace does not match its payload",
        ));
    }
    Ok(transport_workspace_id.or(payload_workspace_id))
}

fn sort_join_response_inbox_entries_newest_first(entries: &mut [JoinResponseInboxEntry]) {
    entries.sort_by(|left, right| {
        right
            .received_at_unix_ms
            .cmp(&left.received_at_unix_ms)
            .then_with(|| right.entry_id.cmp(&left.entry_id))
    });
}

fn prune_oldest_valid_join_response_inbox_entry(data_dir: &Path) -> Result<(), FfiError> {
    let inbox_dir = inbox_dir(data_dir);
    let paths = fs::read_dir(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_response_inbox_read_failed",
            format!("could not read join response inbox: {error}"),
        )
    })?;
    let mut oldest: Option<(u64, String, PathBuf)> = None;
    for path in paths
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
    {
        let Ok(entry) = read_join_response_inbox_entry_path(&path) else {
            continue;
        };
        let replace = oldest.as_ref().is_none_or(|(received_at, entry_id, _)| {
            (entry.received_at_unix_ms, entry.entry_id.as_str()) < (*received_at, entry_id.as_str())
        });
        if replace {
            oldest = Some((entry.received_at_unix_ms, entry.entry_id, path));
        }
    }
    let (_, _, path) = oldest.ok_or_else(|| {
        ffi_error(
            "join_response_inbox_full",
            "join response inbox is full and has no valid entry to prune",
        )
    })?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ffi_error(
            "join_response_inbox_prune_failed",
            format!("could not prune the oldest join response inbox entry: {error}"),
        )),
    }
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

fn inbox_json_entry_count(inbox_dir: &Path) -> Result<usize, FfiError> {
    let mut count = 0;
    for entry in fs::read_dir(inbox_dir).map_err(|error| {
        ffi_error(
            "join_response_inbox_read_failed",
            format!("could not read join response inbox: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            ffi_error(
                "join_response_inbox_read_failed",
                format!("could not inspect join response inbox: {error}"),
            )
        })?;
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            count += 1;
        }
    }
    Ok(count)
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
