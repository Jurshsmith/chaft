use chaft_core::{
    trust_snapshot_for_event_from_events, trust_snapshot_for_events_from_events,
    trust_snapshot_from_events,
};
use chaft_types::{SignedEvent, SignedTrustSnapshot, WorkspaceId};

use crate::{LocalRuntime, RuntimeError};

impl LocalRuntime {
    pub fn export_trust_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let events = self.materialized_workspace_events(&workspace_id)?;
        self.sign_trust_snapshot_from_materialized_events(workspace_id, &events)
    }

    pub(crate) fn sign_trust_snapshot_from_materialized_events(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) = trust_snapshot_from_events(workspace_id, events)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }

    pub(crate) fn sign_trust_snapshot_for_materialized_event(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        event: &SignedEvent,
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) =
            trust_snapshot_for_event_from_events(workspace_id, events, event)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }

    pub(crate) fn sign_trust_snapshot_for_materialized_event_slice(
        &self,
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        target_events: &[SignedEvent],
    ) -> Result<SignedTrustSnapshot, RuntimeError> {
        let (snapshot, root_event) =
            trust_snapshot_for_events_from_events(workspace_id, events, target_events)?;
        Ok(self.identity.sign_trust_snapshot(snapshot, root_event)?)
    }
}
