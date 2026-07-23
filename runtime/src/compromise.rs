use std::collections::BTreeSet;

use chaft_identity::verify_self_contained_event;
use chaft_types::{DeviceId, SignedEvent};
use serde::{Deserialize, Serialize};

use crate::{
    content_keys::RotatedWorkspaceManualKeys, openmls_actions::UpdatedWorkspaceOpenMlsGroups,
};

pub(crate) const COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE: &str =
    "invalid_self_contained_signature";
pub(crate) const COMPROMISE_SIGNAL_SEVERITY_SUSPECTED: &str = "suspected";
pub(crate) const COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE: &str =
    "rotate_workspace_for_suspected_compromise";
pub(crate) const COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES: &str = "review_invalid_signatures";
pub(crate) const COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS: &str = "no_signals";
pub(crate) const COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW: &str =
    "remote_signals_require_review";
pub(crate) const COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED: &str =
    "local_signals_already_handled";
pub(crate) const COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING: &str =
    "local_secret_state_missing";

const COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION: u32 = 1;
const COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES: usize = 512;
pub(crate) const COMPROMISE_RESPONSE_LEDGER_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceForSuspectedCompromise {
    pub workspace_id: String,
    pub openmls_updates: Option<UpdatedWorkspaceOpenMlsGroups>,
    pub manual_key_rotation: Option<RotatedWorkspaceManualKeys>,
    #[serde(default)]
    pub rotated_event_count: usize,
    pub rotated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseReport {
    pub workspace_id: String,
    pub has_signals: bool,
    pub signal_count: usize,
    pub invalid_signature_count: usize,
    pub local_device_signal_count: usize,
    pub should_rotate_local_secret_state: bool,
    pub recommended_action: Option<String>,
    pub signals: Vec<WorkspaceCompromiseSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseSignal {
    pub kind: String,
    pub severity: String,
    pub event_id: String,
    pub channel_id: Option<String>,
    pub author_device_id: String,
    pub local_device: bool,
    pub physical_ms: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompromiseResponse {
    pub workspace_id: String,
    pub report: WorkspaceCompromiseReport,
    pub action_taken: Option<String>,
    pub rotated_local_secret_state: bool,
    pub skipped_reason: Option<String>,
    #[serde(default)]
    pub responded_signal_count: usize,
    pub responded_signal_event_ids: Vec<String>,
    #[serde(default)]
    pub already_handled_signal_count: usize,
    pub already_handled_signal_event_ids: Vec<String>,
    pub rotation: Option<RotatedWorkspaceForSuspectedCompromise>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompromiseResponseLedger {
    schema_version: u32,
    entries: Vec<CompromiseResponseLedgerEntry>,
}

impl Default for CompromiseResponseLedger {
    fn default() -> Self {
        Self {
            schema_version: COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

impl CompromiseResponseLedger {
    pub(crate) fn into_current_schema(self) -> Self {
        if self.schema_version == COMPROMISE_RESPONSE_LEDGER_SCHEMA_VERSION {
            self
        } else {
            Self::default()
        }
    }

    pub(crate) fn handled_signal_event_ids_for_workspace(
        self,
        workspace_id: &str,
    ) -> BTreeSet<String> {
        self.entries
            .into_iter()
            .filter(|entry| entry.workspace_id == workspace_id)
            .flat_map(|entry| entry.signal_event_ids)
            .collect()
    }

    pub(crate) fn record_response(
        &mut self,
        workspace_id: String,
        signal_event_ids: Vec<String>,
        rotated_event_ids: Vec<String>,
        responded_at_unix_ms: u64,
    ) {
        self.entries.push(CompromiseResponseLedgerEntry {
            workspace_id,
            signal_event_ids,
            rotated_event_ids,
            responded_at_unix_ms,
        });
        if self.entries.len() > COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES {
            let remove_count = self.entries.len() - COMPROMISE_RESPONSE_LEDGER_MAX_ENTRIES;
            self.entries.drain(..remove_count);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompromiseResponseLedgerEntry {
    workspace_id: String,
    signal_event_ids: Vec<String>,
    rotated_event_ids: Vec<String>,
    responded_at_unix_ms: u64,
}

pub(crate) fn workspace_compromise_signal_from_event(
    event: &SignedEvent,
    local_device_id: &DeviceId,
) -> Option<WorkspaceCompromiseSignal> {
    if event.author_public_key.is_empty() {
        return None;
    }

    verify_self_contained_event(event)
        .err()
        .map(|error| WorkspaceCompromiseSignal {
            kind: COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE.to_owned(),
            severity: COMPROMISE_SIGNAL_SEVERITY_SUSPECTED.to_owned(),
            event_id: event.event_id.0.clone(),
            channel_id: event
                .event
                .channel_id
                .as_ref()
                .map(|channel_id| channel_id.0.clone()),
            author_device_id: event.event.author_device_id.0.clone(),
            local_device: &event.event.author_device_id == local_device_id,
            physical_ms: event.event.timestamp.physical_ms,
            reason: error.to_string(),
        })
}
