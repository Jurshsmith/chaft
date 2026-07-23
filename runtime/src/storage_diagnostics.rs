use std::path::Path;

use chaft_store::{WorkspaceEventStorageHealth, WorkspaceEventStorageRepair};
use chaft_types::{SignedEvent, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{LocalRuntime, RuntimeError, validate_workspace_id_reference};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageHealth {
    pub workspace_id: String,
    pub total_event_count: usize,
    pub parseable_event_count: usize,
    pub corrupt_event_count: usize,
    pub signature_valid_metadata_count: usize,
    pub servable_event_count: usize,
    pub poisoned_servable_metadata_count: usize,
    pub promotable_servable_metadata_count: usize,
    pub non_servable_parseable_event_count: usize,
}

impl From<WorkspaceEventStorageHealth> for WorkspaceStorageHealth {
    fn from(health: WorkspaceEventStorageHealth) -> Self {
        Self {
            workspace_id: health.workspace_id,
            total_event_count: health.total_event_count,
            parseable_event_count: health.parseable_event_count,
            corrupt_event_count: health.corrupt_event_count,
            signature_valid_metadata_count: health.signature_valid_metadata_count,
            servable_event_count: health.servable_event_count,
            poisoned_servable_metadata_count: health.poisoned_servable_metadata_count,
            promotable_servable_metadata_count: health.promotable_servable_metadata_count,
            non_servable_parseable_event_count: health.non_servable_parseable_event_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStorageRepair {
    pub workspace_id: String,
    pub total_event_count: usize,
    pub parseable_event_count: usize,
    pub corrupt_event_count: usize,
    pub signature_valid_metadata_before_count: usize,
    pub signature_valid_metadata_after_count: usize,
    pub repaired_metadata_count: usize,
    pub promoted_servable_metadata_count: usize,
    pub cleared_unservable_metadata_count: usize,
}

impl From<WorkspaceEventStorageRepair> for WorkspaceStorageRepair {
    fn from(repair: WorkspaceEventStorageRepair) -> Self {
        Self {
            workspace_id: repair.workspace_id,
            total_event_count: repair.total_event_count,
            parseable_event_count: repair.parseable_event_count,
            corrupt_event_count: repair.corrupt_event_count,
            signature_valid_metadata_before_count: repair.signature_valid_metadata_before_count,
            signature_valid_metadata_after_count: repair.signature_valid_metadata_after_count,
            repaired_metadata_count: repair.repaired_metadata_count,
            promoted_servable_metadata_count: repair.promoted_servable_metadata_count,
            cleared_unservable_metadata_count: repair.cleared_unservable_metadata_count,
        }
    }
}

impl LocalRuntime {
    pub fn workspace_events(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<SignedEvent>, RuntimeError> {
        validate_workspace_id_reference(workspace_id)?;
        Ok(self.store.list_events_for_workspace(&workspace_id.0)?)
    }

    pub fn workspace_storage_health(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceStorageHealth, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        Ok(self
            .store
            .workspace_event_storage_health(&workspace_id.0)?
            .into())
    }

    pub fn repair_workspace_storage_metadata(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceStorageRepair, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        Ok(self
            .store
            .repair_workspace_event_storage_metadata(&workspace_id.0)?
            .into())
    }

    pub fn event_store_path(&self) -> &Path {
        &self.paths.event_store
    }
}
