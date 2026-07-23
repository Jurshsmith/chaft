use std::collections::BTreeSet;

use chaft_types::WorkspaceId;

use crate::{
    COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES,
    COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE,
    COMPROMISE_RESPONSE_LEDGER_MAX_BYTES, COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING,
    COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED,
    COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS,
    COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW,
    COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE, CompromiseResponseLedger, LocalRuntime,
    RotatedWorkspaceForSuspectedCompromise, RuntimeError, WorkspaceCompromiseReport,
    WorkspaceCompromiseResponse, now_unix_ms, read_local_metadata_file_with_limit,
    workspace_compromise_signal_from_event, write_secret_file,
};

impl LocalRuntime {
    pub fn rotate_workspace_for_suspected_compromise(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceForSuspectedCompromise, RuntimeError> {
        let openmls_updates = match self.update_workspace_openmls_groups(workspace_id.clone()) {
            Ok(updated) => Some(updated),
            Err(RuntimeError::OpenMlsLocalGroupMissing { .. }) => None,
            Err(error) => return Err(error),
        };
        let manual_key_rotation = if self.workspace_key_path(&workspace_id).exists() {
            Some(self.rotate_workspace_manual_keys(workspace_id.clone())?)
        } else {
            None
        };
        if openmls_updates.is_none() && manual_key_rotation.is_none() {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }

        let mut rotated_event_ids = Vec::new();
        if let Some(openmls_updates) = &openmls_updates {
            rotated_event_ids.extend(openmls_updates.updated_event_ids.iter().cloned());
        }
        if let Some(manual_key_rotation) = &manual_key_rotation {
            rotated_event_ids.extend(manual_key_rotation.rotated_event_ids.iter().cloned());
        }

        Ok(RotatedWorkspaceForSuspectedCompromise {
            workspace_id: workspace_id.0,
            openmls_updates,
            manual_key_rotation,
            rotated_event_count: rotated_event_ids.len(),
            rotated_event_ids,
        })
    }

    pub fn detect_workspace_compromise_signals(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceCompromiseReport, RuntimeError> {
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let signals = events
            .iter()
            .filter_map(|event| {
                workspace_compromise_signal_from_event(event, self.identity.device_id())
            })
            .collect::<Vec<_>>();
        let local_device_signal_count = signals.iter().filter(|signal| signal.local_device).count();
        let invalid_signature_count = signals
            .iter()
            .filter(|signal| signal.kind == COMPROMISE_SIGNAL_INVALID_SELF_CONTAINED_SIGNATURE)
            .count();
        let should_rotate_local_secret_state = local_device_signal_count > 0;
        let recommended_action = if should_rotate_local_secret_state {
            Some(COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE.to_owned())
        } else if !signals.is_empty() {
            Some(COMPROMISE_ACTION_REVIEW_INVALID_SIGNATURES.to_owned())
        } else {
            None
        };

        Ok(WorkspaceCompromiseReport {
            workspace_id: workspace_id.0,
            has_signals: !signals.is_empty(),
            signal_count: signals.len(),
            invalid_signature_count,
            local_device_signal_count,
            should_rotate_local_secret_state,
            recommended_action,
            signals,
        })
    }

    pub fn respond_to_workspace_compromise_signals(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceCompromiseResponse, RuntimeError> {
        let report = self.detect_workspace_compromise_signals(workspace_id.clone())?;
        self.respond_to_workspace_compromise_report(workspace_id, report)
    }

    pub(crate) fn automatic_compromise_response_if_needed(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceCompromiseResponse>, RuntimeError> {
        let report = self.detect_workspace_compromise_signals(workspace_id.clone())?;
        if !report.has_signals {
            return Ok(None);
        }

        self.respond_to_workspace_compromise_report(workspace_id.clone(), report)
            .map(Some)
    }

    fn respond_to_workspace_compromise_report(
        &self,
        workspace_id: WorkspaceId,
        report: WorkspaceCompromiseReport,
    ) -> Result<WorkspaceCompromiseResponse, RuntimeError> {
        let handled_signal_event_ids =
            self.handled_compromise_signal_event_ids_for_workspace(&workspace_id)?;

        let mut already_handled_signal_event_ids = Vec::new();
        let mut responded_signal_event_ids = Vec::new();
        for signal in report.signals.iter().filter(|signal| signal.local_device) {
            if handled_signal_event_ids.contains(&signal.event_id) {
                already_handled_signal_event_ids.push(signal.event_id.clone());
            } else {
                responded_signal_event_ids.push(signal.event_id.clone());
            }
        }

        let mut action_taken = None;
        let mut rotated_local_secret_state = false;
        let mut skipped_reason = None;
        let mut rotation = None;

        if responded_signal_event_ids.is_empty() {
            skipped_reason = if !report.has_signals {
                Some(COMPROMISE_RESPONSE_SKIPPED_NO_SIGNALS.to_owned())
            } else if report.local_device_signal_count == 0 {
                Some(COMPROMISE_RESPONSE_SKIPPED_REMOTE_SIGNALS_REQUIRE_REVIEW.to_owned())
            } else {
                Some(COMPROMISE_RESPONSE_SKIPPED_LOCAL_SIGNALS_ALREADY_HANDLED.to_owned())
            };
        } else {
            match self.rotate_workspace_for_suspected_compromise(workspace_id.clone()) {
                Ok(rotated) => {
                    self.record_compromise_response(
                        &workspace_id,
                        responded_signal_event_ids.clone(),
                        rotated.rotated_event_ids.clone(),
                    )?;
                    action_taken = Some(
                        COMPROMISE_ACTION_ROTATE_WORKSPACE_FOR_SUSPECTED_COMPROMISE.to_owned(),
                    );
                    rotated_local_secret_state = true;
                    rotation = Some(rotated);
                }
                Err(RuntimeError::InvalidWorkspaceKey) => {
                    responded_signal_event_ids.clear();
                    skipped_reason =
                        Some(COMPROMISE_RESPONSE_SKIPPED_LOCAL_SECRET_STATE_MISSING.to_owned());
                }
                Err(error) => return Err(error),
            }
        }

        Ok(WorkspaceCompromiseResponse {
            workspace_id: workspace_id.0,
            report,
            action_taken,
            rotated_local_secret_state,
            skipped_reason,
            responded_signal_count: responded_signal_event_ids.len(),
            responded_signal_event_ids,
            already_handled_signal_count: already_handled_signal_event_ids.len(),
            already_handled_signal_event_ids,
            rotation,
        })
    }

    pub(crate) fn read_compromise_response_ledger(
        &self,
    ) -> Result<CompromiseResponseLedger, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            &self.paths.compromise_response_ledger,
            COMPROMISE_RESPONSE_LEDGER_MAX_BYTES,
            "compromise response ledger",
        )?
        else {
            return Ok(CompromiseResponseLedger::default());
        };
        Ok(serde_json::from_slice::<CompromiseResponseLedger>(&bytes)?.into_current_schema())
    }

    pub(crate) fn write_compromise_response_ledger(
        &self,
        ledger: &CompromiseResponseLedger,
    ) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec_pretty(ledger)?;
        write_secret_file(&self.paths.compromise_response_ledger, &bytes)
    }

    pub(crate) fn handled_compromise_signal_event_ids_for_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<BTreeSet<String>, RuntimeError> {
        Ok(self
            .read_compromise_response_ledger()?
            .handled_signal_event_ids_for_workspace(&workspace_id.0))
    }

    pub(crate) fn record_compromise_response(
        &self,
        workspace_id: &WorkspaceId,
        signal_event_ids: Vec<String>,
        rotated_event_ids: Vec<String>,
    ) -> Result<(), RuntimeError> {
        let mut ledger = self.read_compromise_response_ledger()?;
        ledger.record_response(
            workspace_id.0.clone(),
            signal_event_ids,
            rotated_event_ids,
            now_unix_ms(),
        );
        self.write_compromise_response_ledger(&ledger)
    }
}
