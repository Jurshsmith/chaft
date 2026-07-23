use std::{
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
use chaft_net_direct::JoinRequestInbox;
use chaft_runtime::WorkspaceInviteClaim;
use serde::{Deserialize, Serialize};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::{direct_workspace_id_arg, ffi_device_id_arg},
    input::read_c_string,
};

const JOIN_REQUEST_INBOX_DIR: &str = "join-request-inbox";
const JOIN_REQUEST_INBOX_SCHEMA_VERSION: u32 = 1;
pub(crate) const JOIN_REQUEST_INBOX_ENTRY_MAX_BYTES: usize = 32 * 1024;
const JOIN_REQUEST_INBOX_ENTRY_ID_MAX_BYTES: usize = 128;
const JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES: usize = 100;
pub(crate) const JOIN_REQUEST_INBOX_MAX_ENTRIES: usize = 1024;

static JOIN_REQUEST_INBOX_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static JOIN_REQUEST_INBOX_WRITE_LOCK: Mutex<()> = Mutex::new(());

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
        let entries = list_join_request_inbox_entries(
            &self.data_dir,
            Some(workspace_id),
            max_entries.min(JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES),
        )
        .map_err(|error| NetError::Protocol(error.message))?;
        Ok(entries
            .into_iter()
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
        let entries = list_join_request_inbox_entries(&PathBuf::from(data_dir), None, max_entries)?;
        Ok(JoinRequestInboxEntries { entries })
    })
}

pub(crate) fn runtime_list_join_request_inbox_for_workspace_result(
    data_dir: *const std::ffi::c_char,
    workspace_id: *const std::ffi::c_char,
    max_entries: usize,
) -> FfiResult<JoinRequestInboxEntries> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let max_entries = if max_entries == 0 {
            JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES
        } else {
            max_entries.min(JOIN_REQUEST_INBOX_LIST_MAX_ENTRIES)
        };
        let entries = list_join_request_inbox_entries(
            &PathBuf::from(data_dir),
            Some(&workspace_id),
            max_entries,
        )?;
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
    let workspace_id = resolve_join_request_workspace_id(workspace_id, metadata.workspace_id)?;
    let _write_guard = JOIN_REQUEST_INBOX_WRITE_LOCK.lock().map_err(|_| {
        ffi_error(
            "join_request_inbox_write_failed",
            "join request inbox write lock is unavailable",
        )
    })?;
    let inbox_dir = inbox_dir(data_dir);
    fs::create_dir_all(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_request_inbox_write_failed",
            format!("could not create join request inbox: {error}"),
        )
    })?;
    let entry_id = metadata.request_id;
    let existing_path = inbox_entry_path(data_dir, &entry_id);
    if existing_path.exists() {
        let existing = read_join_request_inbox_entry(data_dir, &entry_id)?;
        if existing.workspace_id == workspace_id && existing.request_text == request_text {
            return Ok(existing);
        }
        return Err(ffi_error(
            "join_request_inbox_entry_conflict",
            format!("join request inbox entry {entry_id} conflicts with an existing request"),
        ));
    }
    let entry = JoinRequestInboxEntry {
        schema_version: JOIN_REQUEST_INBOX_SCHEMA_VERSION,
        entry_id: entry_id.clone(),
        workspace_id,
        received_at_unix_ms: current_unix_ms(),
        request_text: request_text.to_owned(),
    };
    validate_join_request_inbox_entry(&entry)?;
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

    let sequence = JOIN_REQUEST_INBOX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_path = inbox_dir.join(format!(".{entry_id}.{}.{sequence}.tmp", std::process::id()));
    let final_path = inbox_dir.join(format!("{entry_id}.json"));
    if let Err(error) = write_private_file(&temp_path, &bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    let capacity_result = (|| {
        let entry_count = inbox_json_entry_count(&inbox_dir)?;
        let prune_count = entry_count
            .saturating_sub(JOIN_REQUEST_INBOX_MAX_ENTRIES)
            .saturating_add(1);
        if entry_count >= JOIN_REQUEST_INBOX_MAX_ENTRIES {
            for _ in 0..prune_count {
                prune_oldest_valid_join_request_inbox_entry(data_dir)?;
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
            let existing = read_join_request_inbox_entry(data_dir, &entry_id)?;
            if existing.workspace_id == entry.workspace_id
                && existing.request_text == entry.request_text
            {
                Ok(existing)
            } else {
                Err(ffi_error(
                    "join_request_inbox_entry_conflict",
                    format!(
                        "join request inbox entry {entry_id} conflicts with an existing request"
                    ),
                ))
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(ffi_error(
                "join_request_inbox_write_failed",
                format!("could not commit join request inbox entry: {error}"),
            ))
        }
    }
}

fn list_join_request_inbox_entries(
    data_dir: &Path,
    workspace_id: Option<&str>,
    max_entries: usize,
) -> Result<Vec<JoinRequestInboxEntry>, FfiError> {
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
                "join_request_inbox_read_failed",
                format!("could not read join request inbox: {error}"),
            ));
        }
    };
    let workspace_id = workspace_id.map(str::trim);
    let mut entries = Vec::with_capacity(max_entries);
    for path in paths {
        let entry = read_join_request_inbox_entry_path(&path)?;
        if workspace_id
            .is_some_and(|workspace_id| entry.workspace_id.as_deref() != Some(workspace_id))
        {
            continue;
        }
        entries.push(entry);
        if entries.len() > max_entries {
            sort_join_request_inbox_entries_newest_first(&mut entries);
            entries.truncate(max_entries);
        }
    }
    sort_join_request_inbox_entries_newest_first(&mut entries);
    Ok(entries)
}

struct JoinRequestPayloadMetadata {
    request_id: String,
    workspace_id: Option<String>,
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
    let kind = object
        .get("kind")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ffi_error(
                "join_request_payload_invalid",
                "join request payload kind is required",
            )
        })?;
    if !matches!(
        kind,
        "chaft.workspace-join-request.v1" | "chaft.workspace-invite-claim.v1"
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
    validate_join_request_inbox_entry_id(&request_id)?;
    let workspace_id = object
        .get("workspaceId")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .map(direct_workspace_id_arg)
        .transpose()?;

    if kind == "chaft.workspace-join-request.v1" {
        let device_id = object
            .get("deviceId")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ffi_error(
                    "join_request_device_id_required",
                    "join request payload must include deviceId",
                )
            })?;
        ffi_device_id_arg(device_id.to_owned())?;
    } else {
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
        ffi_device_id_arg(claim.payload.device_id.clone())?;
        ffi_device_id_arg(claim.payload.delivery_device_id.clone())?;
    }
    Ok(JoinRequestPayloadMetadata {
        request_id,
        workspace_id,
    })
}

fn resolve_join_request_workspace_id(
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
            "join_request_workspace_id_mismatch",
            "join request transport workspace does not match its payload",
        ));
    }
    Ok(transport_workspace_id.or(payload_workspace_id))
}

fn validate_join_request_inbox_entry(entry: &JoinRequestInboxEntry) -> Result<(), FfiError> {
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
    if let Some(workspace_id) = &entry.workspace_id {
        direct_workspace_id_arg(workspace_id.clone())?;
    }
    let metadata = validate_join_request_payload(&entry.request_text)?;
    if metadata.request_id != entry.entry_id {
        return Err(ffi_error(
            "join_request_inbox_payload_id_mismatch",
            "join request payload request ID must match the inbox entry",
        ));
    }
    if metadata.workspace_id.is_some() && metadata.workspace_id != entry.workspace_id {
        return Err(ffi_error(
            "join_request_inbox_payload_workspace_mismatch",
            "join request payload workspace must match the inbox entry",
        ));
    }
    Ok(())
}

fn sort_join_request_inbox_entries_newest_first(entries: &mut [JoinRequestInboxEntry]) {
    entries.sort_by(|left, right| {
        right
            .received_at_unix_ms
            .cmp(&left.received_at_unix_ms)
            .then_with(|| right.entry_id.cmp(&left.entry_id))
    });
}

fn read_join_request_inbox_entry(
    data_dir: &Path,
    entry_id: &str,
) -> Result<JoinRequestInboxEntry, FfiError> {
    validate_join_request_inbox_entry_id(entry_id)?;
    read_join_request_inbox_entry_path(&inbox_entry_path(data_dir, entry_id))
}

fn read_join_request_inbox_entry_path(path: &Path) -> Result<JoinRequestInboxEntry, FfiError> {
    let metadata = fs::metadata(path).map_err(|error| {
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
    let text = fs::read_to_string(path).map_err(|error| {
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
    validate_join_request_inbox_entry(&entry)?;
    Ok(entry)
}

fn prune_oldest_valid_join_request_inbox_entry(data_dir: &Path) -> Result<(), FfiError> {
    let inbox_dir = inbox_dir(data_dir);
    let paths = fs::read_dir(&inbox_dir).map_err(|error| {
        ffi_error(
            "join_request_inbox_read_failed",
            format!("could not read join request inbox: {error}"),
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
        let Ok(entry) = read_join_request_inbox_entry_path(&path) else {
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
            "join_request_inbox_full",
            "join request inbox is full and has no valid entry to prune",
        )
    })?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ffi_error(
            "join_request_inbox_prune_failed",
            format!("could not prune the oldest join request inbox entry: {error}"),
        )),
    }
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

fn inbox_json_entry_count(inbox_dir: &Path) -> Result<usize, FfiError> {
    let mut count = 0;
    for entry in fs::read_dir(inbox_dir).map_err(|error| {
        ffi_error(
            "join_request_inbox_read_failed",
            format!("could not read join request inbox: {error}"),
        )
    })? {
        let entry = entry.map_err(|error| {
            ffi_error(
                "join_request_inbox_read_failed",
                format!("could not inspect join request inbox: {error}"),
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
