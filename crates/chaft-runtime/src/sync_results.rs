use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{BlobTransferAttempt, WorkspaceCompromiseResponse};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledWorkspace {
    pub workspace_id: String,
    #[serde(default)]
    pub requested_event_count: usize,
    pub requested_event_ids: Vec<String>,
    #[serde(default)]
    pub fetched_event_count: usize,
    pub fetched_event_ids: Vec<String>,
    #[serde(default)]
    pub fetched_blob_count: usize,
    pub fetched_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub ignored_event_count: usize,
    pub ignored_event_ids: Vec<String>,
    #[serde(default)]
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub openmls_catchup: PulledOpenMlsCatchup,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compromise_response: Option<WorkspaceCompromiseResponse>,
    #[serde(default)]
    pub gap_count: usize,
    pub gaps: Vec<PulledWorkspaceGap>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledOpenMlsCatchup {
    #[serde(default)]
    pub event_count: usize,
    pub workspace_joined_event_id: Option<String>,
    pub workspace_applied_event_ids: Vec<String>,
    pub workspace_provisioned_event_ids: Vec<String>,
    pub workspace_self_removed: bool,
    pub channel_groups: Vec<PulledOpenMlsChannelCatchup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledOpenMlsChannelCatchup {
    pub channel_id: String,
    #[serde(default)]
    pub event_count: usize,
    pub joined_event_id: Option<String>,
    pub applied_event_ids: Vec<String>,
    pub provisioned_event_ids: Vec<String>,
    pub self_removed: bool,
}

impl PulledOpenMlsCatchup {
    pub(crate) fn has_provisioned_events(&self) -> bool {
        !self.workspace_provisioned_event_ids.is_empty()
            || self
                .channel_groups
                .iter()
                .any(|group| !group.provisioned_event_ids.is_empty())
    }

    pub(crate) fn refresh_counts(&mut self) {
        for group in &mut self.channel_groups {
            group.refresh_counts();
        }
        self.event_count = usize::from(self.workspace_joined_event_id.is_some())
            + self.workspace_applied_event_ids.len()
            + self.workspace_provisioned_event_ids.len()
            + self
                .channel_groups
                .iter()
                .map(|group| group.event_count)
                .sum::<usize>();
    }
}

impl PulledOpenMlsChannelCatchup {
    pub(crate) fn refresh_counts(&mut self) {
        self.event_count = usize::from(self.joined_event_id.is_some())
            + self.applied_event_ids.len()
            + self.provisioned_event_ids.len();
    }
}

impl PulledWorkspace {
    pub(crate) fn has_local_generated_events(&self) -> bool {
        self.openmls_catchup.has_provisioned_events()
            || self
                .compromise_response
                .as_ref()
                .is_some_and(|response| response.rotated_local_secret_state)
    }

    pub(crate) fn refresh_counts(&mut self) {
        self.requested_event_count = self.requested_event_ids.len();
        self.fetched_event_count = self.fetched_event_ids.len();
        self.fetched_blob_count = self.fetched_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.ignored_event_count = self.ignored_event_ids.len();
        self.applied_event_count = self.applied_event_ids.len();
        self.gap_count = self.gaps.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedWorkspace {
    pub workspace_id: String,
    #[serde(default)]
    pub published_event_count: usize,
    pub published_event_ids: Vec<String>,
    #[serde(default)]
    pub published_blob_count: usize,
    pub published_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub skipped_gap_count: usize,
    pub skipped_gaps: Vec<PulledWorkspaceGap>,
    #[serde(default)]
    pub blob_transfer_attempt_count: usize,
    pub blob_transfer_attempts: Vec<BlobTransferAttempt>,
}

impl PublishedWorkspace {
    pub(crate) fn from_parts(
        workspace_id: String,
        published_event_ids: Vec<String>,
        published_blob_hashes: Vec<String>,
        missing_blob_hashes: Vec<String>,
        skipped_gaps: Vec<PulledWorkspaceGap>,
        blob_transfer_attempts: Vec<BlobTransferAttempt>,
    ) -> Self {
        let mut published = Self {
            workspace_id,
            published_event_count: 0,
            published_event_ids,
            published_blob_count: 0,
            published_blob_hashes,
            missing_blob_count: 0,
            missing_blob_hashes,
            skipped_gap_count: 0,
            skipped_gaps,
            blob_transfer_attempt_count: 0,
            blob_transfer_attempts,
        };
        published.refresh_counts();
        published
    }

    pub(crate) fn refresh_counts(&mut self) {
        self.published_event_count = self.published_event_ids.len();
        self.published_blob_count = self.published_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.skipped_gap_count = self.skipped_gaps.len();
        self.blob_transfer_attempt_count = self.blob_transfer_attempts.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncedWorkspace {
    pub workspace_id: String,
    pub published: PublishedWorkspace,
    pub pulled: PulledWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PulledWorkspaceGap {
    pub event_id: String,
    pub missing_parent_ids: Vec<String>,
}

pub(crate) fn merge_published_workspace(
    target: &mut PublishedWorkspace,
    source: PublishedWorkspace,
) {
    merge_unique_strings(&mut target.published_event_ids, source.published_event_ids);
    merge_unique_strings(
        &mut target.published_blob_hashes,
        source.published_blob_hashes,
    );
    merge_unique_strings(&mut target.missing_blob_hashes, source.missing_blob_hashes);
    merge_workspace_gaps(&mut target.skipped_gaps, source.skipped_gaps);
    target
        .blob_transfer_attempts
        .extend(source.blob_transfer_attempts);
    target.refresh_counts();
}

fn merge_unique_strings(target: &mut Vec<String>, source: Vec<String>) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for value in source {
        if seen.insert(value.clone()) {
            target.push(value);
        }
    }
}

fn merge_workspace_gaps(target: &mut Vec<PulledWorkspaceGap>, source: Vec<PulledWorkspaceGap>) {
    let mut seen = target
        .iter()
        .map(|gap| gap.event_id.clone())
        .collect::<BTreeSet<_>>();
    for gap in source {
        if seen.insert(gap.event_id.clone()) {
            target.push(gap);
        }
    }
}
