use chaft_core::WorkspaceState;
use chaft_types::{SignedEvent, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{LocalRuntime, RuntimeError, verified_local_events_for_runtime};

pub(crate) const MAX_WORKSPACE_SUMMARY_PAGE_ROWS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorkspaceSummary {
    pub workspace_id: String,
    pub name: String,
    pub channel_count: usize,
    pub member_count: usize,
    pub event_count: usize,
    pub has_workspace_key: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWorkspaceSummaryPage {
    pub start_index: usize,
    pub item_count: usize,
    pub total_count: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
    pub workspaces: Vec<LocalWorkspaceSummary>,
}

impl LocalRuntime {
    pub fn list_workspaces(&self) -> Result<Vec<LocalWorkspaceSummary>, RuntimeError> {
        let total_count = self.store.count_workspaces()?;
        let mut workspaces = Vec::with_capacity(total_count.min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS));
        let mut start_index = 0usize;

        while start_index < total_count {
            let page = self.list_workspace_page_uncapped(
                start_index,
                (total_count - start_index).min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS),
            )?;
            let item_count = page.item_count;
            workspaces.extend(page.workspaces);
            if item_count == 0 {
                break;
            }
            start_index = start_index.saturating_add(item_count);
        }

        Ok(workspaces)
    }

    pub fn list_workspace_page(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<LocalWorkspaceSummaryPage, RuntimeError> {
        self.list_workspace_page_uncapped(start_index, limit.min(MAX_WORKSPACE_SUMMARY_PAGE_ROWS))
    }

    fn list_workspace_page_uncapped(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<LocalWorkspaceSummaryPage, RuntimeError> {
        let total_count = self.store.count_workspaces()?;
        let start_index = start_index.min(total_count);
        let end_index = start_index.saturating_add(limit).min(total_count);
        let workspace_ids = self
            .store
            .list_workspace_ids_page(start_index, end_index - start_index)?;
        let mut summaries = Vec::new();
        for workspace_id in workspace_ids {
            let workspace_id = WorkspaceId(workspace_id);
            let event_count = self.store.count_events_for_workspace(&workspace_id.0)?;
            let events = self
                .store
                .list_servable_events_for_workspace(&workspace_id.0)?;
            summaries.push(self.local_workspace_summary(&workspace_id, &events, event_count)?);
        }
        Ok(LocalWorkspaceSummaryPage {
            start_index,
            item_count: summaries.len(),
            total_count,
            has_more_before: start_index > 0,
            has_more_after: end_index < total_count,
            workspaces: summaries,
        })
    }

    fn local_workspace_summary(
        &self,
        workspace_id: &WorkspaceId,
        events: &[SignedEvent],
        event_count: usize,
    ) -> Result<LocalWorkspaceSummary, RuntimeError> {
        let verified_events = verified_local_events_for_runtime(events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&verified_events)?;
        let channel_count = state
            .channels
            .values()
            .filter(|channel| {
                state.channel_accessible_to(&channel.channel_id, self.identity.device_id())
            })
            .count();

        Ok(LocalWorkspaceSummary {
            workspace_id: workspace_id.0.clone(),
            name: state.name.clone().unwrap_or_else(|| "Chaft".to_owned()),
            channel_count,
            member_count: state.members.len(),
            event_count,
            has_workspace_key: self.workspace_key_path(workspace_id).exists(),
        })
    }
}
