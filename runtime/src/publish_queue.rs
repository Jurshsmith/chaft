use std::collections::{BTreeMap, BTreeSet};

use chaft_types::{EventBody, SignedEvent};
use serde::{Deserialize, Serialize};

use crate::PulledWorkspaceGap;

pub(crate) const MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS: usize = 128;
pub(crate) const MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS: usize = 64;
pub(crate) const MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueue {
    pub workspace_id: String,
    pub summary: WorkspacePublishQueueSummary,
    pub publishable_event_ids: Vec<String>,
    pub backup_event_ids: Vec<String>,
    pub available_blob_hashes: Vec<String>,
    pub missing_blob_hashes: Vec<String>,
    pub skipped_gaps: Vec<PulledWorkspaceGap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueueSummary {
    pub publishable_event_count: usize,
    pub backup_event_count: usize,
    pub available_blob_count: usize,
    pub missing_blob_count: usize,
    pub skipped_gap_count: usize,
    pub queued_message_event_count: usize,
    pub queued_attachment_blob_count: usize,
    pub oldest_event_physical_ms: Option<i64>,
    pub newest_event_physical_ms: Option<i64>,
    pub has_missing_local_blobs: bool,
    pub has_skipped_gaps: bool,
    pub is_complete: bool,
    pub channels: Vec<WorkspacePublishQueueChannelSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePublishQueueChannelSummary {
    pub channel_id: Option<String>,
    pub publishable_event_count: usize,
    pub backup_event_count: usize,
    pub queued_message_event_count: usize,
    pub queued_attachment_blob_count: usize,
    pub missing_blob_count: usize,
}

#[derive(Default)]
struct WorkspacePublishQueueChannelSummaryBuilder {
    publishable_event_count: usize,
    backup_event_count: usize,
    queued_message_event_count: usize,
    attachment_blob_hashes: BTreeSet<String>,
    missing_blob_hashes: BTreeSet<String>,
}

pub(crate) fn attachment_blob_hashes(events: &[SignedEvent]) -> Vec<String> {
    let mut hashes = BTreeSet::new();
    for event in events {
        hashes.extend(event_attachment_blob_hashes(event));
    }
    hashes.into_iter().collect()
}

pub(crate) fn workspace_publish_queue_summary(
    events: &[SignedEvent],
    backup_event_ids: &BTreeSet<String>,
    available_blob_hashes: &[String],
    missing_blob_hashes: &[String],
    skipped_gaps: &[PulledWorkspaceGap],
) -> WorkspacePublishQueueSummary {
    let missing_blob_hashes = missing_blob_hashes.iter().cloned().collect::<BTreeSet<_>>();
    let mut channel_summaries =
        BTreeMap::<Option<String>, WorkspacePublishQueueChannelSummaryBuilder>::new();
    let mut queued_message_event_count = 0;
    let mut queued_attachment_blob_hashes = BTreeSet::new();
    let mut oldest_event_physical_ms = None;
    let mut newest_event_physical_ms = None;

    for event in events {
        let physical_ms = event.event.timestamp.physical_ms;
        oldest_event_physical_ms = Some(
            oldest_event_physical_ms
                .map(|oldest: i64| oldest.min(physical_ms))
                .unwrap_or(physical_ms),
        );
        newest_event_physical_ms = Some(
            newest_event_physical_ms
                .map(|newest: i64| newest.max(physical_ms))
                .unwrap_or(physical_ms),
        );

        let is_backup_event = backup_event_ids.contains(&event.event_id.0);
        let is_message_event = is_queued_message_event(event);
        if is_message_event {
            queued_message_event_count += 1;
        }

        let channel_id = event.event.channel_id.as_ref().map(|id| id.0.clone());
        let channel_summary = channel_summaries.entry(channel_id).or_default();
        channel_summary.publishable_event_count += 1;
        if is_backup_event {
            channel_summary.backup_event_count += 1;
        }
        if is_message_event {
            channel_summary.queued_message_event_count += 1;
        }

        for blob_hash in event_attachment_blob_hashes(event) {
            queued_attachment_blob_hashes.insert(blob_hash.clone());
            channel_summary
                .attachment_blob_hashes
                .insert(blob_hash.clone());
            if missing_blob_hashes.contains(&blob_hash) {
                channel_summary.missing_blob_hashes.insert(blob_hash);
            }
        }
    }

    let channels = channel_summaries
        .into_iter()
        .map(
            |(channel_id, summary)| WorkspacePublishQueueChannelSummary {
                channel_id,
                publishable_event_count: summary.publishable_event_count,
                backup_event_count: summary.backup_event_count,
                queued_message_event_count: summary.queued_message_event_count,
                queued_attachment_blob_count: summary.attachment_blob_hashes.len(),
                missing_blob_count: summary.missing_blob_hashes.len(),
            },
        )
        .collect::<Vec<_>>();

    WorkspacePublishQueueSummary {
        publishable_event_count: events.len(),
        backup_event_count: backup_event_ids.len(),
        available_blob_count: available_blob_hashes.len(),
        missing_blob_count: missing_blob_hashes.len(),
        skipped_gap_count: skipped_gaps.len(),
        queued_message_event_count,
        queued_attachment_blob_count: queued_attachment_blob_hashes.len(),
        oldest_event_physical_ms,
        newest_event_physical_ms,
        has_missing_local_blobs: !missing_blob_hashes.is_empty(),
        has_skipped_gaps: !skipped_gaps.is_empty(),
        is_complete: missing_blob_hashes.is_empty() && skipped_gaps.is_empty(),
        channels,
    }
}

fn event_attachment_blob_hashes(event: &SignedEvent) -> Vec<String> {
    match &event.event.body {
        EventBody::MessageCreated { attachments, .. }
        | EventBody::MessageCreatedEncrypted { attachments, .. }
        | EventBody::MessageReplyCreated { attachments, .. }
        | EventBody::MessageReplyCreatedEncrypted { attachments, .. } => attachments
            .iter()
            .map(|attachment| attachment.blob_hash.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn is_queued_message_event(event: &SignedEvent) -> bool {
    matches!(
        &event.event.body,
        EventBody::MessageCreated { .. }
            | EventBody::MessageCreatedEncrypted { .. }
            | EventBody::MessageReplyCreated { .. }
            | EventBody::MessageReplyCreatedEncrypted { .. }
            | EventBody::MessageEdited { .. }
            | EventBody::MessageEditedEncrypted { .. }
            | EventBody::MessageDeleted { .. }
            | EventBody::ReactionAdded { .. }
            | EventBody::ReactionRemoved { .. }
    )
}
