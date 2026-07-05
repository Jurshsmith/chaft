use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
};

use chaft_core::{
    CoreError, MaterializationReport, MessageView, MissingHistoryGap, WorkspaceState,
};
use chaft_identity::verify_self_contained_event;
use chaft_types::{
    ChannelId, DeviceId, EventBody, EventId, MessageId, SignedEvent, WorkspaceId, WorkspaceRole,
};
use serde::{Deserialize, Serialize};

const MAX_KEY_PACKAGE_SNAPSHOT_ROWS_PER_DEVICE_PROTOCOL: usize = 4;
const MAX_CHANNEL_SNAPSHOT_ROWS: usize = 128;
const MAX_PROFILE_SNAPSHOT_ROWS: usize = 256;
const MAX_MEMBER_SNAPSHOT_ROWS: usize = 128;
const MAX_MISSING_HISTORY_SNAPSHOT_ROWS: usize = 64;
const MAX_INVALID_SIGNATURE_SNAPSHOT_ROWS: usize = 64;
const MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND: usize = 32;
pub const MAX_TIMELINE_WINDOW_ROWS: usize = 500;
const MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS: usize = 8;
const MAX_TIMELINE_REACTION_SNAPSHOT_ROWS: usize = 12;
const MAX_GROUPED_TIMELINE_ROW_GAP_MS: i64 = 300_000;
const MS_PER_UTC_DAY: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub workspace_id: String,
    pub name: String,
    pub channels: Vec<ChannelSnapshot>,
    pub profiles: Vec<DeviceProfileSnapshot>,
    pub members: Vec<WorkspaceMemberSnapshot>,
    pub key_packages: Vec<DeviceKeyPackageSnapshot>,
    pub peer_endpoints: Vec<PeerEndpointSnapshot>,
    pub channel_count: usize,
    pub profile_count: usize,
    pub member_count: usize,
    pub key_package_count: usize,
    pub peer_endpoint_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeline_channel_id: Option<String>,
    pub timeline_window: TimelineWindowSnapshot,
    pub timeline: Vec<TimelineItem>,
    pub gap_count: usize,
    pub gaps: Vec<MissingHistorySnapshot>,
    pub invalid_signature_count: usize,
    pub invalid_signatures: Vec<InvalidSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelSnapshot {
    pub channel_id: String,
    pub name: String,
    pub is_private: bool,
    pub unread_count: u32,
    pub latest_activity: Option<ChannelActivitySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChannelPage {
    pub start_index: usize,
    pub item_count: usize,
    pub total_count: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
    pub channels: Vec<ChannelSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChannelSearch {
    pub query: String,
    pub item_count: usize,
    pub total_count: usize,
    pub channels: Vec<ChannelSnapshot>,
}

pub fn query_has_channel_search_terms(query: &str) -> bool {
    query.chars().any(char::is_alphanumeric)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelActivitySnapshot {
    pub event_id: String,
    pub message_id: Option<String>,
    pub author_device_id: String,
    pub author_display_name: Option<String>,
    pub physical_ms: i64,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfileSnapshot {
    pub device_id: String,
    pub display_name: String,
    pub updated_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberSnapshot {
    pub device_id: String,
    pub role: WorkspaceRole,
    pub display_name: Option<String>,
    pub profile_event_id: Option<String>,
    pub membership_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMemberPage {
    pub start_index: usize,
    pub item_count: usize,
    pub total_count: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
    pub members: Vec<WorkspaceMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceKeyPackageSnapshot {
    pub device_id: String,
    pub key_package_id: String,
    pub protocol: String,
    pub byte_len: usize,
    pub published_event_id: String,
    pub physical_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerEndpointSnapshot {
    pub device_id: String,
    pub display_name: Option<String>,
    pub endpoint_id: String,
    pub endpoint: String,
    pub transport: String,
    pub is_backup_peer: bool,
    pub expires_at_ms: Option<i64>,
    pub replica_storage_class: Option<String>,
    pub replica_retention_hint: Option<String>,
    pub published_event_id: String,
    pub physical_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineItemKind {
    Message,
    EncryptedMessage,
    MissingHistoryGap,
    InvalidSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub kind: TimelineItemKind,
    pub event_id: String,
    pub message_id: Option<String>,
    pub reply_to_message_id: Option<String>,
    pub reply_preview: Option<ReplyPreviewSnapshot>,
    pub thread_reply_count: u32,
    pub thread_latest_reply: Option<ReplyPreviewSnapshot>,
    pub thread_reply_previews: Vec<ReplyPreviewSnapshot>,
    pub channel_id: Option<String>,
    pub author_device_id: Option<String>,
    pub author_display_name: Option<String>,
    pub physical_ms: Option<i64>,
    pub body: String,
    #[serde(default)]
    pub attachment_count: usize,
    pub attachments: Vec<AttachmentSnapshot>,
    #[serde(default)]
    pub reaction_count: usize,
    pub reactions: BTreeMap<String, u32>,
    pub my_reactions: Vec<String>,
    pub encrypted: bool,
    pub deleted: bool,
    pub missing_parent_ids: Vec<String>,
    #[serde(default)]
    pub grouped_with_previous: bool,
    #[serde(default)]
    pub day_boundary: bool,
}

const THREAD_REPLY_PREVIEW_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyPreviewSnapshot {
    pub message_id: String,
    pub author_device_id: String,
    pub author_display_name: Option<String>,
    pub body: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentSnapshot {
    pub blob_hash: String,
    pub attachment_id: String,
    pub media_type: String,
    pub byte_len: u64,
    pub display_name: String,
    pub encrypted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_blob_available: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingHistorySnapshot {
    pub event_id: String,
    pub missing_parent_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidSignatureSnapshot {
    pub event_id: String,
    pub channel_id: Option<String>,
    pub author_device_id: String,
    pub physical_ms: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineWindowSnapshot {
    pub start_index: usize,
    pub item_count: usize,
    pub total_count: usize,
    pub has_more_before: bool,
    pub has_more_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshotOptions {
    pub timeline_start: Option<usize>,
    pub timeline_limit: Option<usize>,
    pub timeline_channel_id: Option<ChannelId>,
}

impl WorkspaceSnapshotOptions {
    pub fn full() -> Self {
        Self {
            timeline_start: None,
            timeline_limit: None,
            timeline_channel_id: None,
        }
    }

    pub fn latest(timeline_limit: usize) -> Self {
        Self {
            timeline_start: None,
            timeline_limit: Some(timeline_limit.min(MAX_TIMELINE_WINDOW_ROWS)),
            timeline_channel_id: None,
        }
    }

    pub fn window(timeline_start: usize, timeline_limit: usize) -> Self {
        Self {
            timeline_start: Some(timeline_start),
            timeline_limit: Some(timeline_limit.min(MAX_TIMELINE_WINDOW_ROWS)),
            timeline_channel_id: None,
        }
    }

    pub fn latest_for_channel(channel_id: ChannelId, timeline_limit: usize) -> Self {
        Self {
            timeline_start: None,
            timeline_limit: Some(timeline_limit.min(MAX_TIMELINE_WINDOW_ROWS)),
            timeline_channel_id: Some(channel_id),
        }
    }

    pub fn window_for_channel(
        channel_id: ChannelId,
        timeline_start: usize,
        timeline_limit: usize,
    ) -> Self {
        Self {
            timeline_start: Some(timeline_start),
            timeline_limit: Some(timeline_limit.min(MAX_TIMELINE_WINDOW_ROWS)),
            timeline_channel_id: Some(channel_id),
        }
    }
}

struct SnapshotRenderOptions<'a> {
    reader_device_id: Option<&'a DeviceId>,
    body_overrides_by_event_id: &'a HashMap<String, String>,
    window: &'a WorkspaceSnapshotOptions,
    invalid_signatures: &'a [InvalidSignatureSnapshot],
}

impl WorkspaceSnapshot {
    pub fn from_events(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
    ) -> Result<Self, CoreError> {
        Self::from_events_with_body_overrides(workspace_id, events, &HashMap::new())
    }

    pub fn from_events_with_options(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        options: &WorkspaceSnapshotOptions,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides_and_options(
            workspace_id,
            events,
            None,
            &HashMap::new(),
            options,
        )
    }

    pub fn from_events_for_device(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
    ) -> Result<Self, CoreError> {
        Self::from_events_for_device_with_body_overrides(
            workspace_id,
            events,
            reader_device_id,
            &HashMap::new(),
        )
    }

    pub fn from_events_for_device_with_options(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides_and_options(
            workspace_id,
            events,
            Some(reader_device_id),
            &HashMap::new(),
            options,
        )
    }

    pub fn from_events_with_body_overrides(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        body_overrides_by_event_id: &HashMap<String, String>,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides_and_options(
            workspace_id,
            events,
            None,
            body_overrides_by_event_id,
            &WorkspaceSnapshotOptions::full(),
        )
    }

    pub fn from_events_for_device_with_body_overrides(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides_and_options(
            workspace_id,
            events,
            Some(reader_device_id),
            body_overrides_by_event_id,
            &WorkspaceSnapshotOptions::full(),
        )
    }

    fn from_events_with_optional_reader_body_overrides_and_options(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: Option<&DeviceId>,
        body_overrides_by_event_id: &HashMap<String, String>,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<Self, CoreError> {
        let (events, invalid_signatures) = verified_events_for_snapshot(events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        Ok(Self::from_state_report_reader_body_overrides_and_options(
            workspace_id,
            &state,
            &report,
            &events,
            SnapshotRenderOptions {
                reader_device_id,
                body_overrides_by_event_id,
                window: options,
                invalid_signatures: &invalid_signatures,
            },
        ))
    }

    pub fn from_state_and_report(
        workspace_id: WorkspaceId,
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
    ) -> Self {
        Self::from_state_report_and_body_overrides(
            workspace_id,
            state,
            report,
            events,
            &HashMap::new(),
        )
    }

    pub fn from_state_report_and_body_overrides(
        workspace_id: WorkspaceId,
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        body_overrides_by_event_id: &HashMap<String, String>,
    ) -> Self {
        let invalid_signatures = invalid_signatures_for_snapshot(events);
        Self::from_state_report_reader_body_overrides_and_options(
            workspace_id,
            state,
            report,
            events,
            SnapshotRenderOptions {
                reader_device_id: None,
                body_overrides_by_event_id,
                window: &WorkspaceSnapshotOptions::full(),
                invalid_signatures: &invalid_signatures,
            },
        )
    }

    pub fn from_state_report_for_device_and_body_overrides(
        workspace_id: WorkspaceId,
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
    ) -> Self {
        Self::from_state_report_for_device_and_body_overrides_with_options(
            workspace_id,
            state,
            report,
            events,
            reader_device_id,
            body_overrides_by_event_id,
            &WorkspaceSnapshotOptions::full(),
        )
    }

    pub fn from_state_report_for_device_and_body_overrides_with_options(
        workspace_id: WorkspaceId,
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
        options: &WorkspaceSnapshotOptions,
    ) -> Self {
        let invalid_signatures = invalid_signatures_for_snapshot(events);
        Self::from_state_report_reader_body_overrides_and_options(
            workspace_id,
            state,
            report,
            events,
            SnapshotRenderOptions {
                reader_device_id: Some(reader_device_id),
                body_overrides_by_event_id,
                window: options,
                invalid_signatures: &invalid_signatures,
            },
        )
    }

    fn from_state_report_reader_body_overrides_and_options(
        workspace_id: WorkspaceId,
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        render_options: SnapshotRenderOptions<'_>,
    ) -> Self {
        let events_by_id = events
            .iter()
            .map(|event| (event.event_id.0.as_str(), event))
            .collect::<HashMap<_, _>>();
        let mut channels = channel_snapshots_from_state_report(
            state,
            report,
            &events_by_id,
            render_options.reader_device_id,
            render_options.body_overrides_by_event_id,
        );
        let channel_count = channels.len();
        retain_bounded_channels(&mut channels);

        let mut profiles = state
            .profiles
            .values()
            .map(|profile| DeviceProfileSnapshot {
                device_id: profile.device_id.0.clone(),
                display_name: profile.display_name.clone(),
                updated_event_id: profile.updated_event_id.0.clone(),
            })
            .collect::<Vec<_>>();
        sort_profile_snapshots(&mut profiles);
        let profile_count = profiles.len();
        retain_bounded_profiles(&mut profiles, render_options.reader_device_id);

        let mut members = member_snapshots_from_state(state);
        let member_count = members.len();
        retain_bounded_members(&mut members);

        let mut key_packages = state
            .key_packages
            .values()
            .map(|package| DeviceKeyPackageSnapshot {
                device_id: package.device_id.0.clone(),
                key_package_id: package.key_package_id.0.clone(),
                protocol: package.protocol.clone(),
                byte_len: package.key_package.len(),
                published_event_id: package.published_event_id.0.clone(),
                physical_ms: package.physical_ms,
            })
            .collect::<Vec<_>>();
        key_packages.sort_by(|left, right| {
            left.device_id
                .cmp(&right.device_id)
                .then_with(|| left.protocol.cmp(&right.protocol))
                .then_with(|| right.physical_ms.cmp(&left.physical_ms))
                .then_with(|| left.key_package_id.cmp(&right.key_package_id))
        });
        let key_package_count = key_packages.len();
        let mut key_package_rows_by_device_protocol = BTreeMap::new();
        key_packages.retain(|package| {
            let rows = key_package_rows_by_device_protocol
                .entry((package.device_id.clone(), package.protocol.clone()))
                .or_insert(0);
            if *rows >= MAX_KEY_PACKAGE_SNAPSHOT_ROWS_PER_DEVICE_PROTOCOL {
                return false;
            }
            *rows += 1;
            true
        });

        let mut peer_endpoints = state
            .peer_endpoints
            .values()
            .map(|endpoint| PeerEndpointSnapshot {
                device_id: endpoint.device_id.0.clone(),
                display_name: state
                    .profiles
                    .get(&endpoint.device_id)
                    .map(|profile| profile.display_name.clone()),
                endpoint_id: endpoint.endpoint_id.clone(),
                endpoint: endpoint.endpoint.clone(),
                transport: endpoint.transport.clone(),
                is_backup_peer: endpoint.is_backup_peer,
                expires_at_ms: endpoint.expires_at_ms,
                replica_storage_class: endpoint
                    .replica_storage_class
                    .map(|storage_class| storage_class.as_str().to_owned()),
                replica_retention_hint: endpoint.replica_retention_hint.clone(),
                published_event_id: endpoint.published_event_id.0.clone(),
                physical_ms: endpoint.physical_ms,
            })
            .collect::<Vec<_>>();
        peer_endpoints.sort_by(|left, right| {
            left.is_backup_peer
                .cmp(&right.is_backup_peer)
                .then_with(|| right.physical_ms.cmp(&left.physical_ms))
                .then_with(|| left.device_id.cmp(&right.device_id))
                .then_with(|| left.endpoint_id.cmp(&right.endpoint_id))
        });
        let peer_endpoint_count = peer_endpoints.len();
        let mut member_peer_rows = 0;
        let mut backup_peer_rows = 0;
        peer_endpoints.retain(|endpoint| {
            let rows = if endpoint.is_backup_peer {
                &mut backup_peer_rows
            } else {
                &mut member_peer_rows
            };
            if *rows >= MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND {
                return false;
            }
            *rows += 1;
            true
        });

        let gap_count = report.gaps.len();
        let gaps = bounded_gap_snapshots(report, &events_by_id);
        let timeline_rows = timeline_rows_for_window(
            report,
            &events_by_id,
            state,
            render_options.reader_device_id,
            render_options.invalid_signatures,
            render_options.window.timeline_channel_id.as_ref(),
            render_options.window,
        );
        let thread_parent_message_ids =
            timeline_window_message_ids(&timeline_rows.rows, &events_by_id);
        let thread_reply_index =
            thread_reply_index(state, &events_by_id, &thread_parent_message_ids);
        let timeline = render_timeline_rows(
            &timeline_rows.rows,
            timeline_rows.row_before_window,
            &events_by_id,
            &thread_reply_index,
            state,
            render_options.reader_device_id,
            render_options.body_overrides_by_event_id,
        );

        Self {
            workspace_id: workspace_id.0,
            name: state.name.clone().unwrap_or_else(|| "Chaft".to_owned()),
            channels,
            profiles,
            members,
            key_packages,
            peer_endpoints,
            channel_count,
            profile_count,
            member_count,
            key_package_count,
            peer_endpoint_count,
            timeline_channel_id: render_options
                .window
                .timeline_channel_id
                .as_ref()
                .map(|channel_id| channel_id.0.clone()),
            timeline_window: timeline_rows.window,
            timeline,
            gap_count,
            gaps,
            invalid_signature_count: render_options.invalid_signatures.len(),
            invalid_signatures: bounded_invalid_signatures(render_options.invalid_signatures),
        }
    }
}

impl WorkspaceChannelPage {
    pub fn from_events(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        start_index: usize,
        limit: usize,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides(
            workspace_id,
            events,
            None,
            &HashMap::new(),
            start_index,
            limit,
        )
    }

    pub fn from_events_for_device(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        start_index: usize,
        limit: usize,
    ) -> Result<Self, CoreError> {
        Self::from_events_with_optional_reader_body_overrides(
            workspace_id,
            events,
            Some(reader_device_id),
            &HashMap::new(),
            start_index,
            limit,
        )
    }

    pub fn from_events_containing_channel(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        channel_id: &ChannelId,
        limit: usize,
    ) -> Result<Option<Self>, CoreError> {
        let (events, _) = verified_events_for_snapshot(events);
        let mut state = WorkspaceState::new(workspace_id);
        let report = state.apply_batch(&events)?;
        Ok(
            Self::from_state_report_for_optional_reader_and_body_overrides_containing_channel(
                &state,
                &report,
                &events,
                None,
                &HashMap::new(),
                channel_id,
                limit,
            ),
        )
    }

    fn from_events_with_optional_reader_body_overrides(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        reader_device_id: Option<&DeviceId>,
        body_overrides_by_event_id: &HashMap<String, String>,
        start_index: usize,
        limit: usize,
    ) -> Result<Self, CoreError> {
        let (events, _) = verified_events_for_snapshot(events);
        let mut state = WorkspaceState::new(workspace_id);
        let report = state.apply_batch(&events)?;
        Ok(
            Self::from_state_report_for_optional_reader_and_body_overrides(
                &state,
                &report,
                &events,
                reader_device_id,
                body_overrides_by_event_id,
                start_index,
                limit,
            ),
        )
    }

    pub fn from_state_report_for_device_and_body_overrides(
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
        start_index: usize,
        limit: usize,
    ) -> Self {
        Self::from_state_report_for_optional_reader_and_body_overrides(
            state,
            report,
            events,
            Some(reader_device_id),
            body_overrides_by_event_id,
            start_index,
            limit,
        )
    }

    pub fn from_state_report_for_device_and_body_overrides_containing_channel(
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
        channel_id: &ChannelId,
        limit: usize,
    ) -> Option<Self> {
        Self::from_state_report_for_optional_reader_and_body_overrides_containing_channel(
            state,
            report,
            events,
            Some(reader_device_id),
            body_overrides_by_event_id,
            channel_id,
            limit,
        )
    }

    fn from_state_report_for_optional_reader_and_body_overrides(
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: Option<&DeviceId>,
        body_overrides_by_event_id: &HashMap<String, String>,
        start_index: usize,
        limit: usize,
    ) -> Self {
        let events_by_id = events
            .iter()
            .map(|event| (event.event_id.0.as_str(), event))
            .collect::<HashMap<_, _>>();
        channel_page_from_sorted_channels(
            channel_snapshots_from_state_report(
                state,
                report,
                &events_by_id,
                reader_device_id,
                body_overrides_by_event_id,
            ),
            start_index,
            limit,
        )
    }

    fn from_state_report_for_optional_reader_and_body_overrides_containing_channel(
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: Option<&DeviceId>,
        body_overrides_by_event_id: &HashMap<String, String>,
        channel_id: &ChannelId,
        limit: usize,
    ) -> Option<Self> {
        let events_by_id = events
            .iter()
            .map(|event| (event.event_id.0.as_str(), event))
            .collect::<HashMap<_, _>>();
        channel_page_containing_channel_from_sorted_channels(
            channel_snapshots_from_state_report(
                state,
                report,
                &events_by_id,
                reader_device_id,
                body_overrides_by_event_id,
            ),
            channel_id,
            limit,
        )
    }
}

impl WorkspaceChannelSearch {
    pub fn from_state_report_for_device_and_body_overrides(
        state: &WorkspaceState,
        report: &MaterializationReport,
        events: &[SignedEvent],
        reader_device_id: &DeviceId,
        body_overrides_by_event_id: &HashMap<String, String>,
        query: &str,
        limit: usize,
    ) -> Self {
        Self::from_sorted_channels(
            channel_snapshots_from_state_report(
                state,
                report,
                &events
                    .iter()
                    .map(|event| (event.event_id.0.as_str(), event))
                    .collect::<HashMap<_, _>>(),
                Some(reader_device_id),
                body_overrides_by_event_id,
            ),
            query,
            limit,
        )
    }

    fn from_sorted_channels(
        sorted_channels: Vec<ChannelSnapshot>,
        query: &str,
        limit: usize,
    ) -> Self {
        let query = query.trim().to_owned();
        if !query_has_channel_search_terms(&query) {
            return Self {
                query,
                item_count: 0,
                total_count: 0,
                channels: Vec::new(),
            };
        }

        let normalized_query = query.to_lowercase();
        let mut total_count = 0usize;
        let mut channels = Vec::with_capacity(limit.min(sorted_channels.len()));
        for channel in sorted_channels {
            if !channel_matches_query(&channel, &normalized_query) {
                continue;
            }
            total_count = total_count.saturating_add(1);
            if channels.len() < limit {
                channels.push(channel);
            }
        }

        Self {
            query,
            item_count: channels.len(),
            total_count,
            channels,
        }
    }
}

fn channel_snapshots_from_state_report(
    state: &WorkspaceState,
    report: &MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
    reader_device_id: Option<&DeviceId>,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> Vec<ChannelSnapshot> {
    let channel_projection = channel_projection_by_channel(
        state,
        report,
        events_by_id,
        reader_device_id,
        body_overrides_by_event_id,
    );
    let mut channels = state
        .channels
        .values()
        .filter(|channel| {
            reader_device_id.is_none_or(|reader_device_id| {
                state.channel_accessible_to(&channel.channel_id, reader_device_id)
            })
        })
        .map(|channel| ChannelSnapshot {
            channel_id: channel.channel_id.0.clone(),
            name: channel.name.clone(),
            is_private: channel.is_private,
            unread_count: channel_projection
                .unread_counts
                .get(&channel.channel_id)
                .copied()
                .unwrap_or_default(),
            latest_activity: channel_projection
                .latest_activity
                .get(&channel.channel_id)
                .cloned(),
        })
        .collect::<Vec<_>>();
    sort_channel_snapshots(&mut channels);
    channels
}

fn sort_channel_snapshots(channels: &mut [ChannelSnapshot]) {
    channels.sort_by(|left, right| {
        right
            .latest_activity
            .as_ref()
            .map(|activity| activity.physical_ms)
            .cmp(
                &left
                    .latest_activity
                    .as_ref()
                    .map(|activity| activity.physical_ms),
            )
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.channel_id.cmp(&right.channel_id))
    });
}

fn channel_matches_query(channel: &ChannelSnapshot, normalized_query: &str) -> bool {
    channel.name.to_lowercase().contains(normalized_query)
        || channel.channel_id.to_lowercase().contains(normalized_query)
}

fn retain_bounded_channels(channels: &mut Vec<ChannelSnapshot>) {
    channels.truncate(MAX_CHANNEL_SNAPSHOT_ROWS);
}

fn channel_page_from_sorted_channels(
    channels: Vec<ChannelSnapshot>,
    start_index: usize,
    limit: usize,
) -> WorkspaceChannelPage {
    let total_count = channels.len();
    let start_index = start_index.min(total_count);
    let end_index = start_index.saturating_add(limit).min(total_count);
    let page_channels = channels
        .into_iter()
        .skip(start_index)
        .take(end_index - start_index)
        .collect::<Vec<_>>();

    WorkspaceChannelPage {
        start_index,
        item_count: page_channels.len(),
        total_count,
        has_more_before: start_index > 0,
        has_more_after: end_index < total_count,
        channels: page_channels,
    }
}

fn channel_page_containing_channel_from_sorted_channels(
    channels: Vec<ChannelSnapshot>,
    channel_id: &ChannelId,
    limit: usize,
) -> Option<WorkspaceChannelPage> {
    let index = channels
        .iter()
        .position(|channel| channel.channel_id == channel_id.0)?;
    let limit = limit.max(1);
    let start_index = (index / limit) * limit;
    Some(channel_page_from_sorted_channels(
        channels,
        start_index,
        limit,
    ))
}

impl WorkspaceMemberPage {
    pub fn from_events(
        workspace_id: WorkspaceId,
        events: &[SignedEvent],
        start_index: usize,
        limit: usize,
    ) -> Result<Self, CoreError> {
        let (events, _) = verified_events_for_snapshot(events);
        let mut state = WorkspaceState::new(workspace_id);
        state.apply_batch(&events)?;
        Ok(Self::from_state(&state, start_index, limit))
    }

    pub fn from_state(state: &WorkspaceState, start_index: usize, limit: usize) -> Self {
        member_page_from_sorted_members(member_snapshots_from_state(state), start_index, limit)
    }
}

fn member_snapshots_from_state(state: &WorkspaceState) -> Vec<WorkspaceMemberSnapshot> {
    let mut members = state
        .members
        .values()
        .map(|member| {
            let profile = state.profiles.get(&member.device_id);
            WorkspaceMemberSnapshot {
                device_id: member.device_id.0.clone(),
                role: member.role,
                display_name: profile.map(|profile| profile.display_name.clone()),
                profile_event_id: profile.map(|profile| profile.updated_event_id.0.clone()),
                membership_event_id: member.membership_event_id.0.clone(),
            }
        })
        .collect::<Vec<_>>();
    sort_member_snapshots(&mut members);
    members
}

fn sort_member_snapshots(members: &mut [WorkspaceMemberSnapshot]) {
    members.sort_by(|left, right| {
        role_rank(left.role)
            .cmp(&role_rank(right.role))
            .then_with(|| {
                member_sort_label(left)
                    .to_lowercase()
                    .cmp(&member_sort_label(right).to_lowercase())
            })
            .then_with(|| left.device_id.cmp(&right.device_id))
    });
}

fn retain_bounded_members(members: &mut Vec<WorkspaceMemberSnapshot>) {
    members.truncate(MAX_MEMBER_SNAPSHOT_ROWS);
}

fn member_page_from_sorted_members(
    members: Vec<WorkspaceMemberSnapshot>,
    start_index: usize,
    limit: usize,
) -> WorkspaceMemberPage {
    let total_count = members.len();
    let start_index = start_index.min(total_count);
    let end_index = start_index.saturating_add(limit).min(total_count);
    let page_members = members
        .into_iter()
        .skip(start_index)
        .take(end_index - start_index)
        .collect::<Vec<_>>();

    WorkspaceMemberPage {
        start_index,
        item_count: page_members.len(),
        total_count,
        has_more_before: start_index > 0,
        has_more_after: end_index < total_count,
        members: page_members,
    }
}

fn sort_profile_snapshots(profiles: &mut [DeviceProfileSnapshot]) {
    profiles.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then_with(|| left.device_id.cmp(&right.device_id))
    });
}

fn retain_bounded_profiles(
    profiles: &mut Vec<DeviceProfileSnapshot>,
    reader_device_id: Option<&DeviceId>,
) {
    if profiles.len() <= MAX_PROFILE_SNAPSHOT_ROWS {
        return;
    }

    let reader_profile = reader_device_id.and_then(|reader_device_id| {
        let reader_device_id = reader_device_id.0.as_str();
        profiles
            .iter()
            .find(|profile| profile.device_id == reader_device_id)
            .cloned()
    });

    profiles.truncate(MAX_PROFILE_SNAPSHOT_ROWS);

    let Some(reader_profile) = reader_profile else {
        return;
    };
    if profiles
        .iter()
        .any(|profile| profile.device_id == reader_profile.device_id)
    {
        return;
    }

    if let Some(last_profile) = profiles.last_mut() {
        *last_profile = reader_profile;
        sort_profile_snapshots(profiles);
    }
}

pub fn body_override_event_ids_for_snapshot_window(
    state: &WorkspaceState,
    report: &MaterializationReport,
    events: &[SignedEvent],
    reader_device_id: &DeviceId,
    options: &WorkspaceSnapshotOptions,
) -> BTreeSet<EventId> {
    let events_by_id = events
        .iter()
        .map(|event| (event.event_id.0.as_str(), event))
        .collect::<HashMap<_, _>>();
    let invalid_signatures = invalid_signatures_for_snapshot(events);
    let timeline_rows = timeline_rows_for_window(
        report,
        &events_by_id,
        state,
        Some(reader_device_id),
        &invalid_signatures,
        options.timeline_channel_id.as_ref(),
        options,
    );
    let thread_parent_message_ids = timeline_window_message_ids(&timeline_rows.rows, &events_by_id);
    let thread_reply_index = thread_reply_index(state, &events_by_id, &thread_parent_message_ids);
    let mut event_ids = latest_activity_body_override_event_ids(
        state,
        report,
        &events_by_id,
        Some(reader_device_id),
    );

    for row in timeline_rows.rows.iter().copied() {
        let TimelineRowRef::Applied(event_id) = row else {
            continue;
        };
        collect_applied_timeline_body_override_event_ids(
            event_id,
            &events_by_id,
            &thread_reply_index,
            state,
            Some(reader_device_id),
            &mut event_ids,
        );
    }

    event_ids
}

#[derive(Clone, Copy)]
enum TimelineRowRef<'a> {
    Applied(&'a EventId),
    Gap(&'a MissingHistoryGap),
    Invalid(&'a InvalidSignatureSnapshot),
}

#[derive(Clone, Copy)]
struct ThreadReplyRef<'state, 'event> {
    message: &'state MessageView,
    event: &'event SignedEvent,
}

type ThreadReplyIndex<'state, 'event> =
    HashMap<&'state MessageId, Vec<ThreadReplyRef<'state, 'event>>>;

struct TimelineRowsWindow<'a> {
    window: TimelineWindowSnapshot,
    rows: Vec<TimelineRowRef<'a>>,
    row_before_window: Option<TimelineRowRef<'a>>,
}

fn verified_events_for_snapshot(
    events: &[SignedEvent],
) -> (Cow<'_, [SignedEvent]>, Vec<InvalidSignatureSnapshot>) {
    let mut verified_events = Vec::new();
    let mut invalid_signatures = Vec::new();
    let mut found_invalid = false;

    for (index, event) in events.iter().enumerate() {
        if let Some(invalid_signature) = invalid_signature_for_snapshot(event) {
            if !found_invalid {
                verified_events.extend_from_slice(&events[..index]);
                found_invalid = true;
            }
            invalid_signatures.push(invalid_signature);
        } else if found_invalid {
            verified_events.push(event.clone());
        }
    }

    if found_invalid {
        (Cow::Owned(verified_events), invalid_signatures)
    } else {
        (Cow::Borrowed(events), invalid_signatures)
    }
}

fn invalid_signatures_for_snapshot(events: &[SignedEvent]) -> Vec<InvalidSignatureSnapshot> {
    events
        .iter()
        .filter_map(invalid_signature_for_snapshot)
        .collect()
}

fn invalid_signature_for_snapshot(event: &SignedEvent) -> Option<InvalidSignatureSnapshot> {
    self_contained_signature_failure(event).map(|reason| InvalidSignatureSnapshot {
        event_id: event.event_id.0.clone(),
        channel_id: event
            .event
            .channel_id
            .as_ref()
            .map(|channel_id| channel_id.0.clone()),
        author_device_id: event.event.author_device_id.0.clone(),
        physical_ms: event.event.timestamp.physical_ms,
        reason,
    })
}

fn self_contained_signature_failure(event: &SignedEvent) -> Option<String> {
    if event.author_public_key.is_empty() {
        return None;
    }
    verify_self_contained_event(event)
        .err()
        .map(|error| error.to_string())
}

fn for_each_timeline_row_ref<'a>(
    report: &'a MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    invalid_signatures: &'a [InvalidSignatureSnapshot],
    timeline_channel_id: Option<&ChannelId>,
    mut visit: impl FnMut(TimelineRowRef<'a>),
) {
    for event_id in &report.applied_events {
        if applied_event_has_timeline_item(
            event_id,
            events_by_id,
            state,
            reader_device_id,
            timeline_channel_id,
        ) {
            visit(TimelineRowRef::Applied(event_id));
        }
    }
    for gap in &report.gaps {
        if gap_matches_timeline_channel(gap, events_by_id, timeline_channel_id) {
            visit(TimelineRowRef::Gap(gap));
        }
    }
    for invalid in invalid_signatures {
        if invalid_signature_matches_timeline_channel(invalid, timeline_channel_id) {
            visit(TimelineRowRef::Invalid(invalid));
        }
    }
}

fn timeline_rows_for_window<'a>(
    report: &'a MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    invalid_signatures: &'a [InvalidSignatureSnapshot],
    timeline_channel_id: Option<&ChannelId>,
    options: &WorkspaceSnapshotOptions,
) -> TimelineRowsWindow<'a> {
    let Some(timeline_limit) = options.timeline_limit else {
        let mut rows = Vec::with_capacity(
            report.applied_events.len() + report.gaps.len() + invalid_signatures.len(),
        );
        for_each_timeline_row_ref(
            report,
            events_by_id,
            state,
            reader_device_id,
            invalid_signatures,
            timeline_channel_id,
            |row| rows.push(row),
        );
        let window = timeline_window_for_count(rows.len(), options);
        return TimelineRowsWindow {
            window,
            rows,
            row_before_window: None,
        };
    };

    if let Some(timeline_start) = options.timeline_start {
        let mut rows = Vec::new();
        let mut row_before_window = None;
        let mut total_count = 0usize;
        let end_index = timeline_start.saturating_add(timeline_limit);
        for_each_timeline_row_ref(
            report,
            events_by_id,
            state,
            reader_device_id,
            invalid_signatures,
            timeline_channel_id,
            |row| {
                if total_count < timeline_start {
                    row_before_window = Some(row);
                } else if total_count < end_index {
                    rows.push(row);
                }
                total_count = total_count.saturating_add(1);
            },
        );
        let window = timeline_window_for_count(total_count, options);
        rows.truncate(window.item_count);
        return TimelineRowsWindow {
            window,
            rows,
            row_before_window,
        };
    }

    let max_possible_rows = report
        .applied_events
        .len()
        .saturating_add(report.gaps.len())
        .saturating_add(invalid_signatures.len());
    let mut rows = VecDeque::with_capacity(timeline_limit.min(max_possible_rows));
    let mut row_before_window = None;
    let mut total_count = 0usize;
    for_each_timeline_row_ref(
        report,
        events_by_id,
        state,
        reader_device_id,
        invalid_signatures,
        timeline_channel_id,
        |row| {
            if timeline_limit > 0 {
                if rows.len() == timeline_limit {
                    row_before_window = rows.pop_front();
                }
                rows.push_back(row);
            }
            total_count = total_count.saturating_add(1);
        },
    );
    let window = timeline_window_for_count(total_count, options);
    TimelineRowsWindow {
        window,
        rows: rows.into_iter().collect(),
        row_before_window,
    }
}

fn applied_event_has_timeline_item(
    event_id: &EventId,
    events_by_id: &HashMap<&str, &SignedEvent>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    timeline_channel_id: Option<&ChannelId>,
) -> bool {
    let Some(event) = events_by_id.get(event_id.0.as_str()) else {
        return false;
    };
    let message_id = match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => message_id,
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::MessageEdited { .. }
        | EventBody::MessageEditedEncrypted { .. }
        | EventBody::MessageDeleted { .. }
        | EventBody::ReactionAdded { .. }
        | EventBody::ReactionRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => return false,
    };
    let Some(message) = state.messages.get(message_id) else {
        return false;
    };
    if timeline_channel_id.is_some_and(|channel_id| &message.channel_id != channel_id) {
        return false;
    }
    !reader_device_id.is_some_and(|reader_device_id| {
        !state.channel_accessible_to(&message.channel_id, reader_device_id)
    })
}

fn gap_matches_timeline_channel(
    gap: &MissingHistoryGap,
    events_by_id: &HashMap<&str, &SignedEvent>,
    timeline_channel_id: Option<&ChannelId>,
) -> bool {
    let Some(timeline_channel_id) = timeline_channel_id else {
        return true;
    };
    events_by_id
        .get(gap.event_id.0.as_str())
        .and_then(|event| event.event.channel_id.as_ref())
        .is_none_or(|channel_id| channel_id == timeline_channel_id)
}

fn invalid_signature_matches_timeline_channel(
    invalid: &InvalidSignatureSnapshot,
    timeline_channel_id: Option<&ChannelId>,
) -> bool {
    let Some(timeline_channel_id) = timeline_channel_id else {
        return true;
    };
    invalid
        .channel_id
        .as_ref()
        .is_none_or(|channel_id| channel_id == &timeline_channel_id.0)
}

fn timeline_window_for_count(
    total_count: usize,
    options: &WorkspaceSnapshotOptions,
) -> TimelineWindowSnapshot {
    let Some(timeline_limit) = options.timeline_limit else {
        return TimelineWindowSnapshot {
            start_index: 0,
            item_count: total_count,
            total_count,
            has_more_before: false,
            has_more_after: false,
        };
    };

    if let Some(timeline_start) = options.timeline_start {
        let start_index = timeline_start.min(total_count);
        let end_index = start_index.saturating_add(timeline_limit).min(total_count);
        return TimelineWindowSnapshot {
            start_index,
            item_count: end_index - start_index,
            total_count,
            has_more_before: start_index > 0,
            has_more_after: end_index < total_count,
        };
    }

    if timeline_limit >= total_count {
        return TimelineWindowSnapshot {
            start_index: 0,
            item_count: total_count,
            total_count,
            has_more_before: false,
            has_more_after: false,
        };
    }

    let start_index = total_count - timeline_limit;
    TimelineWindowSnapshot {
        start_index,
        item_count: timeline_limit,
        total_count,
        has_more_before: start_index > 0,
        has_more_after: false,
    }
}

fn render_timeline_rows(
    timeline_rows: &[TimelineRowRef<'_>],
    row_before_window: Option<TimelineRowRef<'_>>,
    events_by_id: &HashMap<&str, &SignedEvent>,
    thread_reply_index: &ThreadReplyIndex<'_, '_>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> Vec<TimelineItem> {
    let render_row = |row: TimelineRowRef<'_>| match row {
        TimelineRowRef::Applied(event_id) => timeline_item_for_applied_event(
            event_id,
            events_by_id,
            thread_reply_index,
            state,
            reader_device_id,
            body_overrides_by_event_id,
        )
        .expect("selected applied timeline row should render"),
        TimelineRowRef::Gap(gap) => gap_timeline_item(gap),
        TimelineRowRef::Invalid(invalid) => invalid_signature_timeline_item(invalid),
    };
    let row_before_window = row_before_window.map(&render_row);
    let mut timeline = timeline_rows
        .iter()
        .copied()
        .map(&render_row)
        .collect::<Vec<_>>();
    annotate_timeline_row_grouping(&mut timeline, row_before_window.as_ref());
    timeline
}

// Grouping and day boundaries must stay stable across window pages, so they
// are computed against the row immediately before the window when one exists;
// only a window that truly starts the timeline treats its first row as first.
// Day comparisons use UTC day indexes (physical_ms / MS_PER_UTC_DAY), not the
// viewer's local calendar, and rows without a physical timestamp carry the
// previous known UTC day forward instead of forcing a boundary.
fn annotate_timeline_row_grouping(
    timeline: &mut [TimelineItem],
    row_before_window: Option<&TimelineItem>,
) {
    let mut previous_group_key = row_before_window.and_then(timeline_row_group_key);
    let mut previous_utc_day = row_before_window.and_then(timeline_row_utc_day);
    let mut first_visible_row = row_before_window.is_none();
    for item in timeline {
        let group_key = timeline_row_group_key(item);
        let utc_day = timeline_row_utc_day(item);
        item.grouped_with_previous = match (&previous_group_key, &group_key) {
            (Some((previous_author, previous_physical_ms)), Some((author, physical_ms))) => {
                author == previous_author
                    && physical_ms
                        .checked_sub(*previous_physical_ms)
                        .is_some_and(|elapsed_ms| {
                            (0..=MAX_GROUPED_TIMELINE_ROW_GAP_MS).contains(&elapsed_ms)
                        })
                    && physical_ms.div_euclid(MS_PER_UTC_DAY)
                        == previous_physical_ms.div_euclid(MS_PER_UTC_DAY)
            }
            _ => false,
        };
        item.day_boundary = first_visible_row || (utc_day.is_some() && utc_day != previous_utc_day);
        previous_group_key = group_key;
        if utc_day.is_some() {
            previous_utc_day = utc_day;
        }
        first_visible_row = false;
    }
}

fn timeline_row_group_key(item: &TimelineItem) -> Option<(String, i64)> {
    let message_kind = matches!(
        item.kind,
        TimelineItemKind::Message | TimelineItemKind::EncryptedMessage
    );
    if !message_kind || item.deleted {
        return None;
    }
    let author_device_id = item.author_device_id.clone()?;
    let physical_ms = item.physical_ms?;
    Some((author_device_id, physical_ms))
}

fn timeline_row_utc_day(item: &TimelineItem) -> Option<i64> {
    item.physical_ms
        .map(|physical_ms| physical_ms.div_euclid(MS_PER_UTC_DAY))
}

fn timeline_window_message_ids(
    timeline_rows: &[TimelineRowRef<'_>],
    events_by_id: &HashMap<&str, &SignedEvent>,
) -> HashSet<MessageId> {
    timeline_rows
        .iter()
        .copied()
        .filter_map(|row| match row {
            TimelineRowRef::Applied(event_id) => events_by_id
                .get(event_id.0.as_str())
                .and_then(|event| event_timeline_message_id(event))
                .cloned(),
            TimelineRowRef::Gap(_) | TimelineRowRef::Invalid(_) => None,
        })
        .collect()
}

fn thread_reply_index<'state, 'event>(
    state: &'state WorkspaceState,
    events_by_id: &HashMap<&str, &'event SignedEvent>,
    parent_message_ids: &HashSet<MessageId>,
) -> ThreadReplyIndex<'state, 'event> {
    let mut index = ThreadReplyIndex::new();
    for message in state.messages.values() {
        let Some(parent_message_id) = message.reply_to_message_id.as_ref() else {
            continue;
        };
        if !parent_message_ids.contains(parent_message_id) {
            continue;
        }
        let Some(event) = events_by_id.get(message.author_event_id.0.as_str()) else {
            continue;
        };
        index
            .entry(parent_message_id)
            .or_default()
            .push(ThreadReplyRef { message, event });
    }

    for replies in index.values_mut() {
        replies.sort_by(|left, right| {
            left.event
                .event
                .timestamp
                .cmp(&right.event.event.timestamp)
                .then_with(|| left.event.event_id.0.cmp(&right.event.event_id.0))
        });
    }

    index
}

fn event_timeline_message_id(event: &SignedEvent) -> Option<&MessageId> {
    match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => Some(message_id),
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::MessageEdited { .. }
        | EventBody::MessageEditedEncrypted { .. }
        | EventBody::MessageDeleted { .. }
        | EventBody::ReactionAdded { .. }
        | EventBody::ReactionRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => None,
    }
}

fn latest_activity_body_override_event_ids(
    state: &WorkspaceState,
    report: &MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
    reader_device_id: Option<&DeviceId>,
) -> BTreeSet<EventId> {
    let mut latest_by_channel = HashMap::<ChannelId, Option<EventId>>::new();
    for event_id in &report.applied_events {
        let Some(event) = events_by_id.get(event_id.0.as_str()) else {
            continue;
        };
        let Some((channel_id, event_id)) =
            channel_activity_body_override_event_id(event, state, reader_device_id)
        else {
            continue;
        };
        latest_by_channel.insert(channel_id, event_id);
    }

    latest_by_channel
        .into_values()
        .flatten()
        .collect::<BTreeSet<_>>()
}

fn channel_activity_body_override_event_id(
    event: &SignedEvent,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
) -> Option<(ChannelId, Option<EventId>)> {
    let (message_id, needs_message_body) = match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. }
        | EventBody::MessageEdited { message_id, .. }
        | EventBody::MessageEditedEncrypted { message_id, .. }
        | EventBody::ReactionAdded { message_id, .. }
        | EventBody::ReactionRemoved { message_id, .. } => (message_id, true),
        EventBody::MessageDeleted { message_id } => (message_id, false),
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => return None,
    };

    let message = state.messages.get(message_id)?;
    if reader_device_id.is_some_and(|reader_device_id| {
        !state.channel_accessible_to(&message.channel_id, reader_device_id)
    }) {
        return None;
    }

    let event_id = needs_message_body
        .then(|| body_override_event_id_for_message(message))
        .flatten();
    Some((message.channel_id.clone(), event_id))
}

fn collect_applied_timeline_body_override_event_ids(
    event_id: &EventId,
    events_by_id: &HashMap<&str, &SignedEvent>,
    thread_reply_index: &ThreadReplyIndex<'_, '_>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    event_ids: &mut BTreeSet<EventId>,
) {
    let Some(event) = events_by_id.get(event_id.0.as_str()) else {
        return;
    };
    let message_id = match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => message_id,
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::MessageEdited { .. }
        | EventBody::MessageEditedEncrypted { .. }
        | EventBody::MessageDeleted { .. }
        | EventBody::ReactionAdded { .. }
        | EventBody::ReactionRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => return,
    };
    let Some(message) = state.messages.get(message_id) else {
        return;
    };
    if reader_device_id.is_some_and(|reader_device_id| {
        !state.channel_accessible_to(&message.channel_id, reader_device_id)
    }) {
        return;
    }

    if let Some(event_id) = body_override_event_id_for_message(message) {
        event_ids.insert(event_id);
    }
    if let Some(reply_to_message_id) = message.reply_to_message_id.as_ref()
        && let Some(reply_to) = state.messages.get(reply_to_message_id)
        && let Some(event_id) = body_override_event_id_for_message(reply_to)
    {
        event_ids.insert(event_id);
    }
    for event_id in thread_reply_preview_body_override_event_ids(message, thread_reply_index) {
        event_ids.insert(event_id);
    }
}

fn thread_reply_preview_body_override_event_ids(
    message: &MessageView,
    thread_reply_index: &ThreadReplyIndex<'_, '_>,
) -> Vec<EventId> {
    let Some(replies) = thread_reply_index.get(&message.message_id) else {
        return Vec::new();
    };

    let preview_start = replies.len().saturating_sub(THREAD_REPLY_PREVIEW_LIMIT);
    replies[preview_start..]
        .iter()
        .filter_map(|reply| body_override_event_id_for_message(reply.message))
        .collect()
}

fn body_override_event_id_for_message(message: &MessageView) -> Option<EventId> {
    (!message.deleted && message.sealed_markdown.is_some()).then(|| message.author_event_id.clone())
}

fn role_rank(role: WorkspaceRole) -> u8 {
    match role {
        WorkspaceRole::Owner => 0,
        WorkspaceRole::Admin => 1,
        WorkspaceRole::Member => 2,
        WorkspaceRole::Guest => 3,
    }
}

fn member_sort_label(member: &WorkspaceMemberSnapshot) -> &str {
    member
        .display_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(&member.device_id)
}

struct ChannelProjection {
    latest_activity: HashMap<ChannelId, ChannelActivitySnapshot>,
    unread_counts: HashMap<ChannelId, u32>,
}

fn channel_projection_by_channel(
    state: &WorkspaceState,
    report: &MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
    reader_device_id: Option<&DeviceId>,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> ChannelProjection {
    let marker_index_by_channel = reader_device_id
        .map(|reader_device_id| {
            let applied_index_by_event_id = report
                .applied_events
                .iter()
                .enumerate()
                .map(|(index, event_id)| (event_id, index))
                .collect::<HashMap<_, _>>();
            state
                .read_markers
                .get(reader_device_id)
                .into_iter()
                .flat_map(|channels| channels.iter())
                .filter_map(|(channel_id, marker_event_id)| {
                    applied_index_by_event_id
                        .get(marker_event_id)
                        .copied()
                        .map(|index| (channel_id, index))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut projection = ChannelProjection {
        latest_activity: HashMap::new(),
        unread_counts: HashMap::new(),
    };
    for (index, event_id) in report.applied_events.iter().enumerate() {
        let Some(event) = events_by_id.get(event_id.0.as_str()) else {
            continue;
        };

        if let Some((channel_id, activity)) =
            channel_activity_for_event(event, state, reader_device_id, body_overrides_by_event_id)
        {
            projection.latest_activity.insert(channel_id, activity);
        }

        let Some(reader_device_id) = reader_device_id else {
            continue;
        };
        if &event.event.author_device_id == reader_device_id {
            continue;
        }
        if !matches!(
            &event.event.body,
            EventBody::MessageCreated { .. }
                | EventBody::MessageCreatedEncrypted { .. }
                | EventBody::MessageReplyCreated { .. }
                | EventBody::MessageReplyCreatedEncrypted { .. }
        ) {
            continue;
        }
        let Some(channel_id) = event.event.channel_id.as_ref() else {
            continue;
        };
        if !state.channel_accessible_to(channel_id, reader_device_id) {
            continue;
        }
        if marker_index_by_channel
            .get(channel_id)
            .is_some_and(|read_index| index <= *read_index)
        {
            continue;
        }
        let count = projection
            .unread_counts
            .entry(channel_id.clone())
            .or_default();
        *count = count.saturating_add(1);
    }

    projection
}

fn channel_activity_for_event(
    event: &SignedEvent,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> Option<(ChannelId, ChannelActivitySnapshot)> {
    let (message_id, preview) = match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => {
            let message = state.messages.get(message_id)?;
            (
                message_id,
                message_activity_preview(message, &event.event_id, body_overrides_by_event_id),
            )
        }
        EventBody::MessageEdited { message_id, .. }
        | EventBody::MessageEditedEncrypted { message_id, .. } => {
            let message = state.messages.get(message_id)?;
            (
                message_id,
                format!(
                    "Edited: {}",
                    message_activity_preview(message, &event.event_id, body_overrides_by_event_id)
                ),
            )
        }
        EventBody::MessageDeleted { message_id } => (message_id, "Message deleted".to_owned()),
        EventBody::ReactionAdded {
            message_id,
            reaction,
        } => (message_id, format!("Reacted {}", compact_preview(reaction))),
        EventBody::ReactionRemoved {
            message_id,
            reaction,
        } => (
            message_id,
            format!("Removed reaction {}", compact_preview(reaction)),
        ),
        _ => return None,
    };

    let message = state.messages.get(message_id)?;
    if reader_device_id.is_some_and(|reader_device_id| {
        !state.channel_accessible_to(&message.channel_id, reader_device_id)
    }) {
        return None;
    }

    Some((
        message.channel_id.clone(),
        ChannelActivitySnapshot {
            event_id: event.event_id.0.clone(),
            message_id: Some(message.message_id.0.clone()),
            author_device_id: event.event.author_device_id.0.clone(),
            author_display_name: state
                .profiles
                .get(&event.event.author_device_id)
                .map(|profile| profile.display_name.clone()),
            physical_ms: event.event.timestamp.physical_ms,
            preview,
        },
    ))
}

fn message_activity_preview(
    message: &MessageView,
    event_id: &EventId,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> String {
    if message.deleted {
        return "Message deleted".to_owned();
    }

    if let Some(body) = body_overrides_by_event_id
        .get(&event_id.0)
        .or_else(|| body_overrides_by_event_id.get(&message.author_event_id.0))
    {
        return compact_preview(body);
    }

    if message.sealed_markdown.is_some() {
        return "Encrypted message".to_owned();
    }

    if !message.markdown.trim().is_empty() {
        return compact_preview(&message.markdown);
    }

    match message.attachments.len() {
        0 => "Message".to_owned(),
        1 => format!(
            "Attachment: {}",
            compact_preview(&message.attachments[0].display_name)
        ),
        count => format!("{count} attachments"),
    }
}

fn compact_preview(value: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 96;
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= MAX_PREVIEW_CHARS {
        return compact;
    }

    let mut preview = compact
        .chars()
        .take(MAX_PREVIEW_CHARS.saturating_sub(3))
        .collect::<String>();
    preview.push_str("...");
    preview
}

fn reaction_preview_map(
    reactions: &BTreeMap<String, u32>,
    my_reactions: &[String],
) -> BTreeMap<String, u32> {
    if reactions.len() <= MAX_TIMELINE_REACTION_SNAPSHOT_ROWS {
        return reactions.clone();
    }

    let my_reactions = my_reactions
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut ranked = reactions
        .iter()
        .map(|(reaction, count)| {
            (
                reaction.as_str(),
                *count,
                my_reactions.contains(reaction.as_str()),
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.0.cmp(right.0))
    });

    ranked
        .into_iter()
        .take(MAX_TIMELINE_REACTION_SNAPSHOT_ROWS)
        .map(|(reaction, count, _)| (reaction.to_owned(), count))
        .collect()
}

fn timeline_item_for_applied_event(
    event_id: &EventId,
    events_by_id: &HashMap<&str, &SignedEvent>,
    thread_reply_index: &ThreadReplyIndex<'_, '_>,
    state: &WorkspaceState,
    reader_device_id: Option<&DeviceId>,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> Option<TimelineItem> {
    let event = events_by_id.get(event_id.0.as_str())?;
    match &event.event.body {
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => {
            let message = state.messages.get(message_id)?;
            if reader_device_id.is_some_and(|reader_device_id| {
                !state.channel_accessible_to(&message.channel_id, reader_device_id)
            }) {
                return None;
            }

            let encrypted = message.sealed_markdown.is_some();
            let deleted = message.deleted;
            let body = if deleted {
                "Message deleted".to_owned()
            } else if let Some(body) = body_overrides_by_event_id.get(&event.event_id.0) {
                body.clone()
            } else if encrypted {
                "Encrypted message".to_owned()
            } else {
                message.markdown.clone()
            };
            let (thread_reply_count, thread_latest_reply, thread_reply_previews) =
                thread_summary_for_message(
                    message,
                    thread_reply_index,
                    state,
                    body_overrides_by_event_id,
                );
            let my_reactions = reader_device_id
                .map(|device_id| message.reactions_for_device(device_id))
                .unwrap_or_default();
            let reactions = reaction_preview_map(&message.reactions, &my_reactions);
            Some(TimelineItem {
                kind: if encrypted {
                    TimelineItemKind::EncryptedMessage
                } else {
                    TimelineItemKind::Message
                },
                event_id: event.event_id.0.clone(),
                message_id: Some(message.message_id.0.clone()),
                reply_to_message_id: message
                    .reply_to_message_id
                    .as_ref()
                    .map(|message_id| message_id.0.clone()),
                reply_preview: reply_preview_for_message(
                    message,
                    events_by_id,
                    state,
                    body_overrides_by_event_id,
                ),
                thread_reply_count,
                thread_latest_reply,
                thread_reply_previews,
                channel_id: Some(message.channel_id.0.clone()),
                author_device_id: Some(event.event.author_device_id.0.clone()),
                author_display_name: state
                    .profiles
                    .get(&event.event.author_device_id)
                    .map(|profile| profile.display_name.clone()),
                physical_ms: Some(event.event.timestamp.physical_ms),
                body,
                attachment_count: message.attachments.len(),
                attachments: message
                    .attachments
                    .iter()
                    .take(MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS)
                    .map(|attachment| AttachmentSnapshot {
                        blob_hash: attachment.blob_hash.clone(),
                        attachment_id: attachment.attachment_id.clone(),
                        media_type: attachment.media_type.clone(),
                        byte_len: attachment.byte_len,
                        display_name: attachment.display_name.clone(),
                        encrypted: attachment.encryption.is_some(),
                        local_blob_available: None,
                    })
                    .collect(),
                reaction_count: message.reactions.len(),
                reactions,
                my_reactions,
                encrypted,
                deleted,
                missing_parent_ids: Vec::new(),
                grouped_with_previous: false,
                day_boundary: false,
            })
        }
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::MessageEdited { .. }
        | EventBody::MessageEditedEncrypted { .. }
        | EventBody::MessageDeleted { .. }
        | EventBody::ReactionAdded { .. }
        | EventBody::ReactionRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => None,
    }
}

fn thread_summary_for_message(
    message: &MessageView,
    thread_reply_index: &ThreadReplyIndex<'_, '_>,
    state: &WorkspaceState,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> (u32, Option<ReplyPreviewSnapshot>, Vec<ReplyPreviewSnapshot>) {
    let Some(replies) = thread_reply_index.get(&message.message_id) else {
        return (0, None, Vec::new());
    };

    let reply_count = u32::try_from(replies.len()).unwrap_or(u32::MAX);
    let preview_start = replies.len().saturating_sub(THREAD_REPLY_PREVIEW_LIMIT);
    let thread_reply_previews = replies[preview_start..]
        .iter()
        .map(|reply| {
            reply_preview_snapshot(
                reply.message,
                reply.event,
                state,
                body_overrides_by_event_id,
            )
        })
        .collect::<Vec<_>>();
    let latest_reply = thread_reply_previews.last().cloned();

    (reply_count, latest_reply, thread_reply_previews)
}

fn reply_preview_for_message(
    message: &MessageView,
    events_by_id: &HashMap<&str, &SignedEvent>,
    state: &WorkspaceState,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> Option<ReplyPreviewSnapshot> {
    let reply_to_message_id = message.reply_to_message_id.as_ref()?;
    let reply_to = state.messages.get(reply_to_message_id)?;
    let reply_event = events_by_id.get(reply_to.author_event_id.0.as_str())?;
    Some(reply_preview_snapshot(
        reply_to,
        reply_event,
        state,
        body_overrides_by_event_id,
    ))
}

fn reply_preview_snapshot(
    message: &MessageView,
    event: &SignedEvent,
    state: &WorkspaceState,
    body_overrides_by_event_id: &HashMap<String, String>,
) -> ReplyPreviewSnapshot {
    ReplyPreviewSnapshot {
        message_id: message.message_id.0.clone(),
        author_device_id: event.event.author_device_id.0.clone(),
        author_display_name: state
            .profiles
            .get(&event.event.author_device_id)
            .map(|profile| profile.display_name.clone()),
        body: message_activity_preview(message, &event.event_id, body_overrides_by_event_id),
        deleted: message.deleted,
    }
}

fn gap_snapshot(gap: &MissingHistoryGap) -> MissingHistorySnapshot {
    MissingHistorySnapshot {
        event_id: gap.event_id.0.clone(),
        missing_parent_ids: gap
            .missing_parent_ids
            .iter()
            .map(|id| id.0.clone())
            .collect(),
    }
}

fn bounded_gap_snapshots(
    report: &MaterializationReport,
    events_by_id: &HashMap<&str, &SignedEvent>,
) -> Vec<MissingHistorySnapshot> {
    let mut gaps = report.gaps.iter().collect::<Vec<_>>();
    gaps.sort_by(|left, right| {
        gap_physical_ms(right, events_by_id)
            .cmp(&gap_physical_ms(left, events_by_id))
            .then_with(|| left.event_id.0.cmp(&right.event_id.0))
    });
    gaps.into_iter()
        .take(MAX_MISSING_HISTORY_SNAPSHOT_ROWS)
        .map(gap_snapshot)
        .collect()
}

fn gap_physical_ms(gap: &MissingHistoryGap, events_by_id: &HashMap<&str, &SignedEvent>) -> i64 {
    events_by_id
        .get(gap.event_id.0.as_str())
        .map(|event| event.event.timestamp.physical_ms)
        .unwrap_or(i64::MIN)
}

fn bounded_invalid_signatures(
    invalid_signatures: &[InvalidSignatureSnapshot],
) -> Vec<InvalidSignatureSnapshot> {
    let mut invalid_signatures = invalid_signatures.to_vec();
    invalid_signatures.sort_by(|left, right| {
        right
            .physical_ms
            .cmp(&left.physical_ms)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    invalid_signatures.truncate(MAX_INVALID_SIGNATURE_SNAPSHOT_ROWS);
    invalid_signatures
}

fn gap_timeline_item(gap: &MissingHistoryGap) -> TimelineItem {
    TimelineItem {
        kind: TimelineItemKind::MissingHistoryGap,
        event_id: gap.event_id.0.clone(),
        message_id: None,
        reply_to_message_id: None,
        reply_preview: None,
        thread_reply_count: 0,
        thread_latest_reply: None,
        thread_reply_previews: Vec::new(),
        channel_id: None,
        author_device_id: None,
        author_display_name: None,
        physical_ms: None,
        body: if gap.missing_parent_ids.is_empty() {
            "Missing authorization context".to_owned()
        } else {
            format!("Missing {} parent event(s)", gap.missing_parent_ids.len())
        },
        attachment_count: 0,
        attachments: Vec::new(),
        reaction_count: 0,
        reactions: BTreeMap::new(),
        my_reactions: Vec::new(),
        encrypted: false,
        deleted: false,
        missing_parent_ids: gap
            .missing_parent_ids
            .iter()
            .map(|id| id.0.clone())
            .collect(),
        grouped_with_previous: false,
        day_boundary: false,
    }
}

fn invalid_signature_timeline_item(invalid: &InvalidSignatureSnapshot) -> TimelineItem {
    TimelineItem {
        kind: TimelineItemKind::InvalidSignature,
        event_id: invalid.event_id.clone(),
        message_id: None,
        reply_to_message_id: None,
        reply_preview: None,
        thread_reply_count: 0,
        thread_latest_reply: None,
        thread_reply_previews: Vec::new(),
        channel_id: invalid.channel_id.clone(),
        author_device_id: Some(invalid.author_device_id.clone()),
        author_display_name: None,
        physical_ms: Some(invalid.physical_ms),
        body: "Failed signature verification".to_owned(),
        attachment_count: 0,
        attachments: Vec::new(),
        reaction_count: 0,
        reactions: BTreeMap::new(),
        my_reactions: Vec::new(),
        encrypted: false,
        deleted: false,
        missing_parent_ids: Vec::new(),
        grouped_with_previous: false,
        day_boundary: false,
    }
}

#[cfg(test)]
mod tests {
    use chaft_identity::DeviceIdentity;
    use chaft_types::{
        AttachmentRef, ChannelId, DeviceId, DeviceKeyPackageId, EncryptedBlobRef, EventBody,
        EventId, HybridTimestamp, MessageId, PayloadEncryption, SealedPayload, SignableEvent,
        WorkspaceRole,
    };

    use super::*;

    fn signed(event: SignableEvent) -> SignedEvent {
        SignedEvent::from_signed_bytes(event, vec![9, 9, 9])
    }

    fn sealed_payload() -> SealedPayload {
        SealedPayload {
            mode: PayloadEncryption::Aes256GcmSiv,
            key_id: "workspace-key-1".to_owned(),
            nonce: vec![1; 12],
            aad: b"message aad".to_vec(),
            bytes: b"ciphertext".to_vec(),
        }
    }

    fn workspace_with_channel(
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        owner_device_id: &DeviceId,
    ) -> Vec<SignedEvent> {
        vec![
            signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner_device_id.clone(),
                EventBody::WorkspaceCreated {
                    name: "Grouped Rows".to_owned(),
                },
            )),
            signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner_device_id.clone(),
                EventBody::ChannelCreated {
                    channel_id: channel_id.clone(),
                    name: "general".to_owned(),
                    is_private: false,
                },
            )),
        ]
    }

    fn timestamped_message(
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        author_device_id: &DeviceId,
        markdown: &str,
        physical_ms: i64,
    ) -> SignedEvent {
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            author_device_id.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: markdown.to_owned(),
                attachments: Vec::new(),
            },
        );
        message.timestamp = HybridTimestamp {
            physical_ms,
            logical: 0,
        };
        signed(message)
    }

    #[test]
    fn snapshot_contains_channels_and_encrypted_timeline_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));
        let message_physical_ms = message.event.timestamp.physical_ms;

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, channel, message]).unwrap();

        assert_eq!(snapshot.name, "Chaft Labs");
        assert_eq!(snapshot.channels[0].name, "general");
        let latest_activity = snapshot.channels[0].latest_activity.as_ref().unwrap();
        assert_eq!(latest_activity.event_id, snapshot.timeline[0].event_id);
        assert_eq!(latest_activity.message_id, snapshot.timeline[0].message_id);
        assert_eq!(latest_activity.preview, "Encrypted message");
        assert_eq!(snapshot.timeline.len(), 1);
        assert_eq!(snapshot.timeline[0].physical_ms, Some(message_physical_ms));
        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::EncryptedMessage
        );
        assert_eq!(snapshot.timeline[0].body, "Encrypted message");
        assert!(snapshot.timeline[0].encrypted);
        assert!(snapshot.gaps.is_empty());
    }

    #[test]
    fn snapshot_includes_reply_context_on_timeline_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let parent_message_id = MessageId::new();
        let reply_message_id = MessageId::new();
        let device_id = DeviceId("dev_mira".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Mira".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let parent = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id: parent_message_id.clone(),
                markdown: "parent message with enough context".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let reply = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageReplyCreated {
                message_id: reply_message_id.clone(),
                reply_to_message_id: parent_message_id.clone(),
                markdown: "reply body".to_owned(),
                attachments: Vec::new(),
            },
        ));

        let snapshot = WorkspaceSnapshot::from_events(
            workspace_id,
            &[workspace, profile, channel, parent, reply],
        )
        .unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].thread_reply_count, 1);
        let latest_thread_reply = snapshot.timeline[0].thread_latest_reply.as_ref().unwrap();
        assert_eq!(latest_thread_reply.message_id, reply_message_id.0);
        assert_eq!(
            latest_thread_reply.author_display_name.as_deref(),
            Some("Mira")
        );
        assert_eq!(latest_thread_reply.body, "reply body");
        assert_eq!(snapshot.timeline[0].thread_reply_previews.len(), 1);
        assert_eq!(
            snapshot.timeline[0].thread_reply_previews[0].message_id,
            reply_message_id.0
        );
        assert_eq!(
            snapshot.timeline[1].reply_to_message_id.as_deref(),
            Some(parent_message_id.0.as_str())
        );
        let reply_preview = snapshot.timeline[1].reply_preview.as_ref().unwrap();
        assert_eq!(reply_preview.message_id, parent_message_id.0);
        assert_eq!(reply_preview.author_display_name.as_deref(), Some("Mira"));
        assert_eq!(reply_preview.body, "parent message with enough context");
        assert!(!reply_preview.deleted);
    }

    #[test]
    fn snapshot_bounds_thread_reply_previews_to_latest_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let parent_message_id = MessageId::new();
        let device_id = DeviceId("dev_mira".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Mira".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let parent = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id: parent_message_id.clone(),
                markdown: "parent".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let mut events = vec![workspace, profile, channel, parent];
        let mut reply_message_ids = Vec::new();
        for index in 0..6 {
            let reply_message_id = MessageId::new();
            let mut reply = SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::MessageReplyCreated {
                    message_id: reply_message_id.clone(),
                    reply_to_message_id: parent_message_id.clone(),
                    markdown: format!("reply {index}"),
                    attachments: Vec::new(),
                },
            );
            reply.timestamp = HybridTimestamp {
                physical_ms: 2_000 + i64::from(index),
                logical: 0,
            };
            reply_message_ids.push(reply_message_id);
            events.push(signed(reply));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let parent_row = snapshot
            .timeline
            .iter()
            .find(|item| item.message_id.as_deref() == Some(parent_message_id.0.as_str()))
            .unwrap();

        assert_eq!(parent_row.thread_reply_count, 6);
        assert_eq!(parent_row.thread_reply_previews.len(), 5);
        assert_eq!(
            parent_row
                .thread_reply_previews
                .iter()
                .map(|preview| preview.body.as_str())
                .collect::<Vec<_>>(),
            vec!["reply 1", "reply 2", "reply 3", "reply 4", "reply 5"]
        );
        assert_eq!(
            parent_row
                .thread_reply_previews
                .iter()
                .map(|preview| preview.message_id.as_str())
                .collect::<Vec<_>>(),
            reply_message_ids[1..]
                .iter()
                .map(|message_id| message_id.0.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            parent_row.thread_latest_reply.as_ref().unwrap().body,
            "reply 5"
        );

        let windowed_snapshot = WorkspaceSnapshot::from_events_with_options(
            WorkspaceId(snapshot.workspace_id.clone()),
            &events,
            &WorkspaceSnapshotOptions::window(0, 1),
        )
        .unwrap();
        assert_eq!(windowed_snapshot.timeline.len(), 1);
        assert_eq!(windowed_snapshot.timeline[0].thread_reply_count, 6);
        assert_eq!(
            windowed_snapshot.timeline[0]
                .thread_reply_previews
                .last()
                .unwrap()
                .body,
            "reply 5"
        );
    }

    #[test]
    fn body_override_selection_bounds_thread_preview_plaintext() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let parent_message_id = MessageId::new();
        let device_id = DeviceId("dev_mira".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let parent = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: parent_message_id.clone(),
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));
        let parent_event_id = parent.event_id.clone();
        let mut events = vec![workspace, channel, parent];
        let mut reply_event_ids = Vec::new();
        for index in 0..6 {
            let mut reply = SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::MessageReplyCreatedEncrypted {
                    message_id: MessageId::new(),
                    reply_to_message_id: parent_message_id.clone(),
                    sealed_markdown: sealed_payload(),
                    attachments: Vec::new(),
                },
            );
            reply.timestamp = HybridTimestamp {
                physical_ms: 2_000 + i64::from(index),
                logical: 0,
            };
            let reply = signed(reply);
            reply_event_ids.push(reply.event_id.clone());
            events.push(reply);
        }
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events).unwrap();

        let selected = body_override_event_ids_for_snapshot_window(
            &state,
            &report,
            &events,
            &device_id,
            &WorkspaceSnapshotOptions::window(0, 1),
        );

        assert!(selected.contains(&parent_event_id));
        assert!(!selected.contains(&reply_event_ids[0]));
        for reply_event_id in &reply_event_ids[1..] {
            assert!(selected.contains(reply_event_id));
        }
    }

    #[test]
    fn snapshot_sorts_channels_by_latest_activity_and_includes_preview() {
        let workspace_id = WorkspaceId::new();
        let alpha_id = ChannelId::new();
        let beta_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Activity".to_owned(),
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: alpha_id.clone(),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: beta_id.clone(),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let mut alpha_message = SignableEvent::new(
            workspace_id.clone(),
            Some(alpha_id),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "older alpha message".to_owned(),
                attachments: Vec::new(),
            },
        );
        alpha_message.timestamp = HybridTimestamp {
            physical_ms: 1_000,
            logical: 0,
        };
        let mut beta_message = SignableEvent::new(
            workspace_id.clone(),
            Some(beta_id),
            device_id,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "newer beta message with preview".to_owned(),
                attachments: Vec::new(),
            },
        );
        beta_message.timestamp = HybridTimestamp {
            physical_ms: 2_000,
            logical: 0,
        };

        let snapshot = WorkspaceSnapshot::from_events(
            workspace_id,
            &[
                workspace,
                alpha,
                beta,
                signed(alpha_message),
                signed(beta_message),
            ],
        )
        .unwrap();

        assert_eq!(snapshot.channels[0].name, "beta");
        assert_eq!(snapshot.channels[1].name, "alpha");
        let latest_activity = snapshot.channels[0].latest_activity.as_ref().unwrap();
        assert_eq!(latest_activity.preview, "newer beta message with preview");
        assert_eq!(latest_activity.physical_ms, 2_000);
    }

    #[test]
    fn snapshot_latest_window_keeps_tail_rows_and_reports_total() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Windowed".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let first = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "first".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let second = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "second".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let third = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "third".to_owned(),
                attachments: Vec::new(),
            },
        ));

        let snapshot = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &[workspace, channel, first, second, third],
            &WorkspaceSnapshotOptions::latest(2),
        )
        .unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "second");
        assert_eq!(snapshot.timeline[1].body, "third");
        assert_eq!(snapshot.timeline_window.start_index, 1);
        assert_eq!(snapshot.timeline_window.item_count, 2);
        assert_eq!(snapshot.timeline_window.total_count, 3);
        assert!(snapshot.timeline_window.has_more_before);
        assert!(!snapshot.timeline_window.has_more_after);
    }

    #[test]
    fn snapshot_options_cap_explicit_timeline_windows() {
        let channel_id = ChannelId::new();

        assert_eq!(
            WorkspaceSnapshotOptions::latest(usize::MAX).timeline_limit,
            Some(MAX_TIMELINE_WINDOW_ROWS)
        );
        assert_eq!(
            WorkspaceSnapshotOptions::window(0, usize::MAX).timeline_limit,
            Some(MAX_TIMELINE_WINDOW_ROWS)
        );
        assert_eq!(
            WorkspaceSnapshotOptions::latest_for_channel(channel_id.clone(), usize::MAX)
                .timeline_limit,
            Some(MAX_TIMELINE_WINDOW_ROWS)
        );
        assert_eq!(
            WorkspaceSnapshotOptions::window_for_channel(channel_id, 0, usize::MAX).timeline_limit,
            Some(MAX_TIMELINE_WINDOW_ROWS)
        );
        assert_eq!(WorkspaceSnapshotOptions::full().timeline_limit, None);
    }

    #[test]
    fn snapshot_window_keeps_requested_rows_and_reports_edges() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Windowed".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let mut events = vec![workspace, channel];
        for body in ["first", "second", "third", "fourth"] {
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: body.to_owned(),
                    attachments: Vec::new(),
                },
            )));
        }

        let snapshot = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &events,
            &WorkspaceSnapshotOptions::window(1, 2),
        )
        .unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(snapshot.timeline[0].body, "second");
        assert_eq!(snapshot.timeline[1].body, "third");
        assert_eq!(snapshot.timeline_window.start_index, 1);
        assert_eq!(snapshot.timeline_window.item_count, 2);
        assert_eq!(snapshot.timeline_window.total_count, 4);
        assert!(snapshot.timeline_window.has_more_before);
        assert!(snapshot.timeline_window.has_more_after);
    }

    #[test]
    fn snapshot_empty_timeline_windows_preserve_total_counts() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Empty Windows".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let mut events = vec![workspace, channel];
        for body in ["first", "second", "third"] {
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: body.to_owned(),
                    attachments: Vec::new(),
                },
            )));
        }

        let latest_empty = WorkspaceSnapshot::from_events_with_options(
            workspace_id.clone(),
            &events,
            &WorkspaceSnapshotOptions::latest(0),
        )
        .unwrap();
        assert!(latest_empty.timeline.is_empty());
        assert_eq!(latest_empty.timeline_window.start_index, 3);
        assert_eq!(latest_empty.timeline_window.item_count, 0);
        assert_eq!(latest_empty.timeline_window.total_count, 3);
        assert!(latest_empty.timeline_window.has_more_before);
        assert!(!latest_empty.timeline_window.has_more_after);

        let out_of_range = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &events,
            &WorkspaceSnapshotOptions::window(99, 5),
        )
        .unwrap();
        assert!(out_of_range.timeline.is_empty());
        assert_eq!(out_of_range.timeline_window.start_index, 3);
        assert_eq!(out_of_range.timeline_window.item_count, 0);
        assert_eq!(out_of_range.timeline_window.total_count, 3);
        assert!(out_of_range.timeline_window.has_more_before);
        assert!(!out_of_range.timeline_window.has_more_after);
    }

    #[test]
    fn snapshot_channel_window_counts_only_selected_channel_rows() {
        let workspace_id = WorkspaceId::new();
        let alpha_id = ChannelId("chn_alpha".to_owned());
        let beta_id = ChannelId("chn_beta".to_owned());
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Channel Windowed".to_owned(),
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: alpha_id.clone(),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: beta_id.clone(),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let mut events = vec![workspace, alpha, beta];
        for (channel_id, body) in [
            (&alpha_id, "alpha first"),
            (&beta_id, "beta first"),
            (&alpha_id, "alpha second"),
            (&beta_id, "beta second"),
            (&alpha_id, "alpha third"),
        ] {
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: body.to_owned(),
                    attachments: Vec::new(),
                },
            )));
        }

        let latest = WorkspaceSnapshot::from_events_with_options(
            workspace_id.clone(),
            &events,
            &WorkspaceSnapshotOptions::latest_for_channel(alpha_id.clone(), 2),
        )
        .unwrap();

        assert_eq!(latest.timeline_channel_id.as_deref(), Some("chn_alpha"));
        assert_eq!(latest.channels.len(), 2);
        assert_eq!(
            latest
                .timeline
                .iter()
                .map(|item| item.body.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha second", "alpha third"]
        );
        assert_eq!(latest.timeline_window.start_index, 1);
        assert_eq!(latest.timeline_window.item_count, 2);
        assert_eq!(latest.timeline_window.total_count, 3);
        assert!(latest.timeline_window.has_more_before);
        assert!(!latest.timeline_window.has_more_after);

        let first_page = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &events,
            &WorkspaceSnapshotOptions::window_for_channel(alpha_id, 0, 2),
        )
        .unwrap();

        assert_eq!(
            first_page
                .timeline
                .iter()
                .map(|item| item.body.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha first", "alpha second"]
        );
        assert_eq!(first_page.timeline_window.start_index, 0);
        assert_eq!(first_page.timeline_window.item_count, 2);
        assert_eq!(first_page.timeline_window.total_count, 3);
        assert!(!first_page.timeline_window.has_more_before);
        assert!(first_page.timeline_window.has_more_after);
    }

    #[test]
    fn snapshot_window_counts_messages_gaps_and_invalid_signatures() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let identity = DeviceIdentity::generate();
        let owner = identity.device_id().clone();
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Mixed Window".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let first = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "first".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let mut gap = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "gap".to_owned(),
                attachments: Vec::new(),
            },
        );
        gap.parents = vec![EventId("evt_missing_parent".to_owned())];
        let gap = signed(gap);
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged".to_owned(),
                attachments: Vec::new(),
            },
        ));
        forged.signature[0] ^= 1;

        let snapshot = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &[workspace, channel, first, gap, forged],
            &WorkspaceSnapshotOptions::window(1, 2),
        )
        .unwrap();

        assert_eq!(snapshot.gap_count, 1);
        assert_eq!(snapshot.invalid_signature_count, 1);
        assert_eq!(snapshot.gaps.len(), 1);
        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.timeline.len(), 2);
        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::MissingHistoryGap
        );
        assert_eq!(
            snapshot.timeline[1].kind,
            TimelineItemKind::InvalidSignature
        );
        assert_eq!(snapshot.timeline_window.start_index, 1);
        assert_eq!(snapshot.timeline_window.item_count, 2);
        assert_eq!(snapshot.timeline_window.total_count, 3);
        assert!(snapshot.timeline_window.has_more_before);
        assert!(!snapshot.timeline_window.has_more_after);
    }

    #[test]
    fn snapshot_caps_diagnostic_arrays_and_preserves_total_counts() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let identity = DeviceIdentity::generate();
        let owner = identity.device_id().clone();
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Capped Diagnostics".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let total_gap_count = MAX_MISSING_HISTORY_SNAPSHOT_ROWS + 3;
        let total_invalid_signature_count = MAX_INVALID_SIGNATURE_SNAPSHOT_ROWS + 5;
        let mut events = vec![workspace, channel];
        let mut gap_event_ids = Vec::new();
        let mut invalid_event_ids = Vec::new();

        for index in 0..total_gap_count {
            let mut gap = SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: format!("gap {index}"),
                    attachments: Vec::new(),
                },
            );
            gap.timestamp = HybridTimestamp {
                physical_ms: index as i64,
                logical: 0,
            };
            gap.parents = vec![EventId(format!("evt_missing_parent_{index:03}"))];
            let gap = signed(gap);
            gap_event_ids.push(gap.event_id.0.clone());
            events.push(gap);
        }

        for index in 0..total_invalid_signature_count {
            let mut invalid = SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: format!("invalid {index}"),
                    attachments: Vec::new(),
                },
            );
            invalid.timestamp = HybridTimestamp {
                physical_ms: 10_000 + index as i64,
                logical: 0,
            };
            let mut invalid = identity.sign_event(invalid);
            invalid.signature[0] ^= 1;
            invalid_event_ids.push(invalid.event_id.0.clone());
            events.push(invalid);
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.gap_count, total_gap_count);
        assert_eq!(
            snapshot.invalid_signature_count,
            total_invalid_signature_count
        );
        assert_eq!(snapshot.gaps.len(), MAX_MISSING_HISTORY_SNAPSHOT_ROWS);
        assert_eq!(
            snapshot.invalid_signatures.len(),
            MAX_INVALID_SIGNATURE_SNAPSHOT_ROWS
        );
        assert_eq!(
            snapshot.gaps.first().map(|gap| gap.event_id.as_str()),
            Some(gap_event_ids[total_gap_count - 1].as_str())
        );
        assert_eq!(
            snapshot.gaps.last().map(|gap| gap.event_id.as_str()),
            Some(gap_event_ids[total_gap_count - MAX_MISSING_HISTORY_SNAPSHOT_ROWS].as_str())
        );
        assert_eq!(
            snapshot
                .invalid_signatures
                .first()
                .map(|invalid| invalid.event_id.as_str()),
            Some(invalid_event_ids[total_invalid_signature_count - 1].as_str())
        );
        assert_eq!(
            snapshot
                .invalid_signatures
                .last()
                .map(|invalid| invalid.event_id.as_str()),
            Some(
                invalid_event_ids
                    [total_invalid_signature_count - MAX_INVALID_SIGNATURE_SNAPSHOT_ROWS]
                    .as_str()
            )
        );
    }

    #[test]
    fn snapshot_projects_device_profiles_onto_timeline_authors() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_mira".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Mira".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, profile, channel, message])
                .unwrap();

        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.profiles[0].display_name, "Mira");
        assert_eq!(snapshot.members.len(), 1);
        assert_eq!(snapshot.members[0].role, WorkspaceRole::Owner);
        assert_eq!(snapshot.members[0].display_name.as_deref(), Some("Mira"));
        assert_eq!(
            snapshot.timeline[0].author_display_name.as_deref(),
            Some("Mira")
        );
    }

    #[test]
    fn snapshot_caps_channel_rows_after_sorting_while_preserving_total_count() {
        let workspace_id = WorkspaceId("wrk_capped_channels".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let total_channel_count = MAX_CHANNEL_SNAPSHOT_ROWS + 3;
        let mut events = vec![workspace];

        for index in 0..total_channel_count {
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::ChannelCreated {
                    channel_id: ChannelId(format!("chn_{index:03}")),
                    name: format!("Channel {index:03}"),
                    is_private: false,
                },
            )));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let expected_last_channel_name = format!("Channel {:03}", MAX_CHANNEL_SNAPSHOT_ROWS - 1);

        assert_eq!(snapshot.channel_count, total_channel_count);
        assert_eq!(snapshot.channels.len(), MAX_CHANNEL_SNAPSHOT_ROWS);
        assert_eq!(
            snapshot
                .channels
                .first()
                .map(|channel| channel.name.as_str()),
            Some("Channel 000")
        );
        assert_eq!(
            snapshot
                .channels
                .last()
                .map(|channel| channel.name.as_str()),
            Some(expected_last_channel_name.as_str())
        );
    }

    #[test]
    fn workspace_channel_page_returns_exact_sorted_window() {
        let workspace_id = WorkspaceId("wrk_channel_page".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_beta".to_owned()),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_alpha".to_owned()),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let gamma = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_gamma".to_owned()),
                name: "gamma".to_owned(),
                is_private: false,
            },
        ));

        let page =
            WorkspaceChannelPage::from_events(workspace_id, &[workspace, beta, alpha, gamma], 1, 2)
                .unwrap();

        assert_eq!(page.start_index, 1);
        assert_eq!(page.item_count, 2);
        assert_eq!(page.total_count, 3);
        assert!(page.has_more_before);
        assert!(!page.has_more_after);
        assert_eq!(
            page.channels
                .iter()
                .map(|channel| channel.channel_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chn_beta", "chn_gamma"]
        );
    }

    #[test]
    fn workspace_channel_page_containing_channel_returns_sorted_window() {
        let workspace_id = WorkspaceId("wrk_channel_page_containing".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_beta".to_owned()),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_alpha".to_owned()),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let gamma = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_gamma".to_owned()),
                name: "gamma".to_owned(),
                is_private: false,
            },
        ));

        let page = WorkspaceChannelPage::from_events_containing_channel(
            workspace_id.clone(),
            &[
                workspace.clone(),
                beta.clone(),
                alpha.clone(),
                gamma.clone(),
            ],
            &ChannelId("chn_gamma".to_owned()),
            2,
        )
        .unwrap()
        .unwrap();

        assert_eq!(page.start_index, 2);
        assert_eq!(page.item_count, 1);
        assert_eq!(page.total_count, 3);
        assert!(page.has_more_before);
        assert!(!page.has_more_after);
        assert_eq!(page.channels[0].channel_id, "chn_gamma");

        let zero_limit_page = WorkspaceChannelPage::from_events_containing_channel(
            workspace_id.clone(),
            &[
                workspace.clone(),
                beta.clone(),
                alpha.clone(),
                gamma.clone(),
            ],
            &ChannelId("chn_beta".to_owned()),
            0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(zero_limit_page.item_count, 1);
        assert_eq!(zero_limit_page.channels[0].channel_id, "chn_beta");

        let missing = WorkspaceChannelPage::from_events_containing_channel(
            workspace_id,
            &[workspace, beta, alpha, gamma],
            &ChannelId("chn_missing".to_owned()),
            2,
        )
        .unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn workspace_channel_search_filters_sorted_channels_with_bound() {
        let workspace_id = WorkspaceId("wrk_channel_search".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_beta".to_owned()),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_alpha".to_owned()),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let gamma = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_gamma".to_owned()),
                name: "gamma".to_owned(),
                is_private: false,
            },
        ));
        let events = [workspace, beta, alpha, gamma];
        let (events, _) = verified_events_for_snapshot(&events);
        let mut state = WorkspaceState::new(workspace_id);
        let report = state.apply_batch(&events).unwrap();

        let search = WorkspaceChannelSearch::from_state_report_for_device_and_body_overrides(
            &state,
            &report,
            &events,
            &DeviceId("dev_owner".to_owned()),
            &HashMap::new(),
            "a",
            2,
        );

        assert_eq!(search.query, "a");
        assert_eq!(search.item_count, 2);
        assert_eq!(search.total_count, 3);
        assert_eq!(
            search
                .channels
                .iter()
                .map(|channel| channel.channel_id.as_str())
                .collect::<Vec<_>>(),
            vec!["chn_alpha", "chn_beta"]
        );

        let count_only = WorkspaceChannelSearch::from_state_report_for_device_and_body_overrides(
            &state,
            &report,
            &events,
            &DeviceId("dev_owner".to_owned()),
            &HashMap::new(),
            "a",
            0,
        );
        assert_eq!(count_only.item_count, 0);
        assert_eq!(count_only.total_count, 3);
        assert!(count_only.channels.is_empty());
    }

    #[test]
    fn workspace_channel_search_ignores_termless_queries() {
        assert!(!query_has_channel_search_terms(""));
        assert!(!query_has_channel_search_terms(" \t --- ___ "));
        assert!(query_has_channel_search_terms("gen"));
        assert!(query_has_channel_search_terms("general-2"));
        assert!(query_has_channel_search_terms("こんにちは"));

        let workspace_id = WorkspaceId("wrk_channel_termless_search".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let general = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_general".to_owned()),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let events = [workspace, general];
        let (events, _) = verified_events_for_snapshot(&events);
        let mut state = WorkspaceState::new(workspace_id);
        let report = state.apply_batch(&events).unwrap();

        let search = WorkspaceChannelSearch::from_state_report_for_device_and_body_overrides(
            &state,
            &report,
            &events,
            &DeviceId("dev_owner".to_owned()),
            &HashMap::new(),
            " \t --- ___ ",
            10,
        );

        assert_eq!(search.query, "--- ___");
        assert_eq!(search.item_count, 0);
        assert_eq!(search.total_count, 0);
        assert!(search.channels.is_empty());
    }

    #[test]
    fn snapshot_caps_profile_rows_after_sorting_while_preserving_total_count() {
        let workspace_id = WorkspaceId("wrk_capped_profiles".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let total_profile_count = MAX_PROFILE_SNAPSHOT_ROWS + 3;
        let mut events = vec![workspace];

        for index in 0..total_profile_count {
            let profile_device_id = DeviceId(format!("dev_profile_{index:03}"));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::MemberInvited {
                    invitee_device_id: profile_device_id.clone(),
                    role: WorkspaceRole::Member,
                },
            )));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                profile_device_id,
                EventBody::DeviceProfileUpdated {
                    display_name: format!("Member {index:03}"),
                },
            )));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let expected_last_display_name = format!("Member {:03}", MAX_PROFILE_SNAPSHOT_ROWS - 1);

        assert_eq!(snapshot.profile_count, total_profile_count);
        assert_eq!(snapshot.profiles.len(), MAX_PROFILE_SNAPSHOT_ROWS);
        assert_eq!(
            snapshot
                .profiles
                .first()
                .map(|profile| profile.display_name.as_str()),
            Some("Member 000")
        );
        assert_eq!(
            snapshot
                .profiles
                .last()
                .map(|profile| profile.display_name.as_str()),
            Some(expected_last_display_name.as_str())
        );
    }

    #[test]
    fn reader_snapshot_keeps_local_profile_when_profile_rows_are_capped() {
        let workspace_id = WorkspaceId("wrk_reader_capped_profiles".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let reader = DeviceId("dev_reader".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let total_remote_profile_count = MAX_PROFILE_SNAPSHOT_ROWS + 3;
        let mut events = vec![workspace];

        for index in 0..total_remote_profile_count {
            let profile_device_id = DeviceId(format!("dev_profile_{index:03}"));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::MemberInvited {
                    invitee_device_id: profile_device_id.clone(),
                    role: WorkspaceRole::Member,
                },
            )));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                profile_device_id,
                EventBody::DeviceProfileUpdated {
                    display_name: format!("Member {index:03}"),
                },
            )));
        }
        events.push(signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: reader.clone(),
                role: WorkspaceRole::Member,
            },
        )));
        events.push(signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Zzz Local".to_owned(),
            },
        )));

        let snapshot =
            WorkspaceSnapshot::from_events_for_device(workspace_id, &events, &reader).unwrap();
        let replaced_profile_device_id =
            format!("dev_profile_{:03}", MAX_PROFILE_SNAPSHOT_ROWS - 1);

        assert_eq!(snapshot.profile_count, total_remote_profile_count + 1);
        assert_eq!(snapshot.profiles.len(), MAX_PROFILE_SNAPSHOT_ROWS);
        assert!(
            snapshot
                .profiles
                .iter()
                .any(|profile| profile.device_id == reader.0.as_str())
        );
        assert!(
            !snapshot
                .profiles
                .iter()
                .any(|profile| profile.device_id == replaced_profile_device_id.as_str())
        );
    }

    #[test]
    fn snapshot_caps_member_rows_after_sorting_while_preserving_total_count() {
        let workspace_id = WorkspaceId("wrk_capped_members".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let total_member_count = MAX_MEMBER_SNAPSHOT_ROWS + 3;
        let mut events = vec![workspace];

        for index in 0..(total_member_count - 1) {
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::MemberInvited {
                    invitee_device_id: DeviceId(format!("dev_member_{index:03}")),
                    role: WorkspaceRole::Member,
                },
            )));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let expected_last_member_id = format!("dev_member_{:03}", MAX_MEMBER_SNAPSHOT_ROWS - 2);

        assert_eq!(snapshot.member_count, total_member_count);
        assert_eq!(snapshot.members.len(), MAX_MEMBER_SNAPSHOT_ROWS);
        assert_eq!(
            snapshot
                .members
                .first()
                .map(|member| member.device_id.as_str()),
            Some("dev_owner")
        );
        assert_eq!(
            snapshot
                .members
                .last()
                .map(|member| member.device_id.as_str()),
            Some(expected_last_member_id.as_str())
        );
    }

    #[test]
    fn workspace_member_page_returns_exact_sorted_window() {
        let workspace_id = WorkspaceId("wrk_member_page".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let admin = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: DeviceId("dev_admin".to_owned()),
                role: WorkspaceRole::Admin,
            },
        ));
        let member_a = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: DeviceId("dev_a".to_owned()),
                role: WorkspaceRole::Member,
            },
        ));
        let member_b = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: DeviceId("dev_b".to_owned()),
                role: WorkspaceRole::Member,
            },
        ));

        let page = WorkspaceMemberPage::from_events(
            workspace_id,
            &[workspace, member_b, member_a, admin],
            1,
            2,
        )
        .unwrap();

        assert_eq!(page.start_index, 1);
        assert_eq!(page.item_count, 2);
        assert_eq!(page.total_count, 4);
        assert!(page.has_more_before);
        assert!(page.has_more_after);
        assert_eq!(
            page.members
                .iter()
                .map(|member| member.device_id.as_str())
                .collect::<Vec<_>>(),
            vec!["dev_admin", "dev_a"]
        );
    }

    #[test]
    fn snapshot_includes_workspace_members_with_roles_and_profile_names() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Admin,
            },
        ));
        let profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member,
            EventBody::DeviceProfileUpdated {
                display_name: "Nia".to_owned(),
            },
        ));

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, invite, profile]).unwrap();

        assert_eq!(snapshot.members.len(), 2);
        assert_eq!(snapshot.members[0].device_id, "dev_owner");
        assert_eq!(snapshot.members[0].role, WorkspaceRole::Owner);
        assert_eq!(snapshot.members[0].display_name, None);
        assert_eq!(snapshot.members[1].device_id, "dev_member");
        assert_eq!(snapshot.members[1].role, WorkspaceRole::Admin);
        assert_eq!(snapshot.members[1].display_name.as_deref(), Some("Nia"));
        assert!(snapshot.members[1].profile_event_id.is_some());
    }

    #[test]
    fn snapshot_includes_device_key_package_metadata() {
        let workspace_id = WorkspaceId("wrk_key_packages".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let key_package_id = DeviceKeyPackageId("dkp_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let mut package = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: "openmls/key-package".to_owned(),
                key_package: vec![1, 2, 3, 4, 5],
            },
        );
        package.timestamp = HybridTimestamp {
            physical_ms: 1_700_000_000_025,
            logical: 0,
        };
        let package = signed(package);

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, package.clone()]).unwrap();

        assert_eq!(snapshot.key_package_count, 1);
        assert_eq!(snapshot.key_packages.len(), 1);
        assert_eq!(snapshot.key_packages[0].device_id, owner.0);
        assert_eq!(snapshot.key_packages[0].key_package_id, key_package_id.0);
        assert_eq!(snapshot.key_packages[0].protocol, "openmls/key-package");
        assert_eq!(snapshot.key_packages[0].byte_len, 5);
        assert_eq!(
            snapshot.key_packages[0].published_event_id,
            package.event_id.0
        );
        assert_eq!(snapshot.key_packages[0].physical_ms, 1_700_000_000_025);
        assert!(snapshot.timeline.is_empty());
    }

    #[test]
    fn snapshot_caps_key_package_hints_per_device_protocol_after_priority_sorting() {
        let workspace_id = WorkspaceId("wrk_capped_key_packages".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let total_key_package_count = MAX_KEY_PACKAGE_SNAPSHOT_ROWS_PER_DEVICE_PROTOCOL + 3;
        let mut events = vec![workspace];

        for index in 0..total_key_package_count {
            let mut package = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::DeviceKeyPackagePublished {
                    key_package_id: DeviceKeyPackageId(format!("dkp-{index:03}")),
                    protocol: "openmls/key-package".to_owned(),
                    key_package: vec![index as u8],
                },
            );
            package.timestamp = HybridTimestamp {
                physical_ms: index as i64,
                logical: 0,
            };
            events.push(signed(package));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let expected_first_key_package_id = format!("dkp-{:03}", total_key_package_count - 1);
        let expected_last_key_package_id = format!(
            "dkp-{:03}",
            total_key_package_count - MAX_KEY_PACKAGE_SNAPSHOT_ROWS_PER_DEVICE_PROTOCOL
        );

        assert_eq!(
            snapshot.key_packages.len(),
            MAX_KEY_PACKAGE_SNAPSHOT_ROWS_PER_DEVICE_PROTOCOL
        );
        assert_eq!(snapshot.key_package_count, total_key_package_count);
        assert_eq!(
            snapshot
                .key_packages
                .first()
                .map(|package| package.key_package_id.as_str()),
            Some(expected_first_key_package_id.as_str())
        );
        assert_eq!(
            snapshot
                .key_packages
                .last()
                .map(|package| package.key_package_id.as_str()),
            Some(expected_last_key_package_id.as_str())
        );
        assert_eq!(
            snapshot
                .key_packages
                .iter()
                .map(|package| package.physical_ms)
                .collect::<Vec<_>>(),
            vec![6, 5, 4, 3]
        );
    }

    #[test]
    fn snapshot_includes_latest_peer_endpoint_hints() {
        let workspace_id = WorkspaceId("wrk_peer_endpoints".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Mira".to_owned(),
            },
        ));
        let first = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "desktop".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: Some(1_700_000_600_000),
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));
        let replacement = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "desktop".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:8888".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: Some(chaft_types::ReplicaStorageClass::FullHistoryWithBlobs),
                replica_retention_hint: Some("30d".to_owned()),
            },
        ));
        let replacement_event_id = replacement.event_id.0.clone();

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, profile, first, replacement])
                .unwrap();

        assert_eq!(snapshot.peer_endpoint_count, 1);
        assert_eq!(snapshot.peer_endpoints.len(), 1);
        assert_eq!(snapshot.peer_endpoints[0].device_id, owner.0);
        assert_eq!(
            snapshot.peer_endpoints[0].display_name.as_deref(),
            Some("Mira")
        );
        assert_eq!(snapshot.peer_endpoints[0].endpoint_id, "desktop");
        assert_eq!(
            snapshot.peer_endpoints[0].endpoint,
            "direct+tcp://127.0.0.1:8888"
        );
        assert_eq!(snapshot.peer_endpoints[0].transport, "direct-tcp");
        assert!(snapshot.peer_endpoints[0].is_backup_peer);
        assert_eq!(snapshot.peer_endpoints[0].expires_at_ms, None);
        assert_eq!(
            snapshot.peer_endpoints[0].replica_storage_class.as_deref(),
            Some("full_history_with_blobs")
        );
        assert_eq!(
            snapshot.peer_endpoints[0].replica_retention_hint.as_deref(),
            Some("30d")
        );
        assert_eq!(
            snapshot.peer_endpoints[0].published_event_id,
            replacement_event_id
        );
        assert!(snapshot.timeline.is_empty());
    }

    #[test]
    fn snapshot_caps_peer_endpoint_hints_per_kind_after_priority_sorting() {
        let workspace_id = WorkspaceId("wrk_capped_peer_endpoints".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let member_endpoint_count = MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND + 5;
        let backup_endpoint_count = MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND + 7;
        let mut events = vec![workspace];

        for index in 0..member_endpoint_count {
            let mut endpoint = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: format!("desktop-{index:03}"),
                    endpoint: format!("direct+tcp://127.0.0.1:{}", 7000 + index),
                    transport: "direct-tcp".to_owned(),
                    is_backup_peer: false,
                    expires_at_ms: None,
                    replica_storage_class: None,
                    replica_retention_hint: None,
                },
            );
            endpoint.timestamp = HybridTimestamp {
                physical_ms: index as i64,
                logical: 0,
            };
            events.push(signed(endpoint));
        }

        for index in 0..backup_endpoint_count {
            let mut endpoint = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: format!("backup-{index:03}"),
                    endpoint: format!("direct+tcp://127.0.0.1:{}", 8000 + index),
                    transport: "direct-tcp".to_owned(),
                    is_backup_peer: true,
                    expires_at_ms: None,
                    replica_storage_class: None,
                    replica_retention_hint: None,
                },
            );
            endpoint.timestamp = HybridTimestamp {
                physical_ms: 10_000 + index as i64,
                logical: 0,
            };
            events.push(signed(endpoint));
        }

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();
        let member_endpoints = snapshot
            .peer_endpoints
            .iter()
            .filter(|endpoint| !endpoint.is_backup_peer)
            .collect::<Vec<_>>();
        let backup_endpoints = snapshot
            .peer_endpoints
            .iter()
            .filter(|endpoint| endpoint.is_backup_peer)
            .collect::<Vec<_>>();

        let expected_first_member = format!(
            "direct+tcp://127.0.0.1:{}",
            7000 + member_endpoint_count - 1
        );
        let expected_last_member = format!(
            "direct+tcp://127.0.0.1:{}",
            7000 + member_endpoint_count - MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND
        );
        let expected_first_backup = format!(
            "direct+tcp://127.0.0.1:{}",
            8000 + backup_endpoint_count - 1
        );
        let expected_last_backup = format!(
            "direct+tcp://127.0.0.1:{}",
            8000 + backup_endpoint_count - MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND
        );

        assert_eq!(
            snapshot.peer_endpoints.len(),
            MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND * 2
        );
        assert_eq!(
            snapshot.peer_endpoint_count,
            member_endpoint_count + backup_endpoint_count
        );
        assert_eq!(
            member_endpoints.len(),
            MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND
        );
        assert_eq!(
            backup_endpoints.len(),
            MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND
        );
        assert!(
            snapshot.peer_endpoints[..MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND]
                .iter()
                .all(|endpoint| !endpoint.is_backup_peer)
        );
        assert!(
            snapshot.peer_endpoints[MAX_PEER_ENDPOINT_SNAPSHOT_ROWS_PER_KIND..]
                .iter()
                .all(|endpoint| endpoint.is_backup_peer)
        );
        assert_eq!(
            member_endpoints
                .first()
                .map(|endpoint| endpoint.endpoint.as_str()),
            Some(expected_first_member.as_str())
        );
        assert_eq!(
            member_endpoints
                .last()
                .map(|endpoint| endpoint.endpoint.as_str()),
            Some(expected_last_member.as_str())
        );
        assert_eq!(
            backup_endpoints
                .first()
                .map(|endpoint| endpoint.endpoint.as_str()),
            Some(expected_first_backup.as_str())
        );
        assert_eq!(
            backup_endpoints
                .last()
                .map(|endpoint| endpoint.endpoint.as_str()),
            Some(expected_last_backup.as_str())
        );
    }

    #[test]
    fn snapshot_can_render_local_plaintext_override_for_encrypted_message() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));
        let body_overrides =
            HashMap::from([(message.event_id.0.clone(), "locally opened".to_owned())]);

        let snapshot = WorkspaceSnapshot::from_events_with_body_overrides(
            workspace_id,
            &[workspace, channel, message],
            &body_overrides,
        )
        .unwrap();

        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::EncryptedMessage
        );
        assert_eq!(snapshot.timeline[0].body, "locally opened");
        assert!(snapshot.timeline[0].encrypted);
    }

    #[test]
    fn snapshot_includes_reaction_counts_on_message_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: message_id.clone(),
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));
        let reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));
        let duplicate_reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));
        let removal = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id.clone(),
            EventBody::ReactionRemoved {
                message_id,
                reaction: "+1".to_owned(),
            },
        ));

        let snapshot = WorkspaceSnapshot::from_events(
            workspace_id.clone(),
            &[
                workspace.clone(),
                channel.clone(),
                message.clone(),
                reaction.clone(),
                duplicate_reaction.clone(),
            ],
        )
        .unwrap();

        assert_eq!(snapshot.timeline[0].reactions.get("+1"), Some(&1));
        assert_eq!(snapshot.timeline[0].reaction_count, 1);
        assert!(snapshot.timeline[0].my_reactions.is_empty());

        let device_snapshot = WorkspaceSnapshot::from_events_for_device(
            workspace_id.clone(),
            &[
                workspace.clone(),
                channel.clone(),
                message.clone(),
                reaction.clone(),
                duplicate_reaction.clone(),
            ],
            &device_id,
        )
        .unwrap();
        assert_eq!(
            device_snapshot.timeline[0].my_reactions,
            vec!["+1".to_owned()]
        );
        assert_eq!(device_snapshot.timeline[0].reaction_count, 1);

        let device_snapshot_after_removal = WorkspaceSnapshot::from_events_for_device(
            workspace_id.clone(),
            &[
                workspace.clone(),
                channel.clone(),
                message.clone(),
                reaction.clone(),
                duplicate_reaction.clone(),
                removal.clone(),
            ],
            &device_id,
        )
        .unwrap();
        assert!(
            device_snapshot_after_removal.timeline[0]
                .my_reactions
                .is_empty()
        );

        let snapshot = WorkspaceSnapshot::from_events(
            workspace_id,
            &[
                workspace,
                channel,
                message,
                reaction,
                duplicate_reaction,
                removal,
            ],
        )
        .unwrap();
        assert_eq!(snapshot.timeline[0].reactions.get("+1"), None);
        assert_eq!(snapshot.timeline[0].reaction_count, 0);
    }

    #[test]
    fn snapshot_caps_reaction_rows_and_preserves_count() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "reaction fanout".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let local_reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "zz-local".to_owned(),
            },
        ));
        let remote_reaction_count = MAX_TIMELINE_REACTION_SNAPSHOT_ROWS + 4;
        let mut events = vec![workspace, channel, message, local_reaction];
        for index in 0..remote_reaction_count {
            let peer = DeviceId(format!("dev_peer_{index:02}"));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::MemberInvited {
                    invitee_device_id: peer.clone(),
                    role: WorkspaceRole::Member,
                },
            )));
            events.push(signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                peer,
                EventBody::ReactionAdded {
                    message_id: message_id.clone(),
                    reaction: format!("r_{index:02}"),
                },
            )));
        }

        let snapshot =
            WorkspaceSnapshot::from_events_for_device(workspace_id, &events, &owner).unwrap();
        let row = &snapshot.timeline[0];

        assert_eq!(
            row.reaction_count,
            remote_reaction_count + 1,
            "full distinct reaction count should be preserved"
        );
        assert_eq!(row.reactions.len(), MAX_TIMELINE_REACTION_SNAPSHOT_ROWS);
        assert_eq!(row.my_reactions, vec!["zz-local".to_owned()]);
        assert!(row.reactions.contains_key("zz-local"));
        assert!(!row.reactions.contains_key("r_11"));
    }

    #[test]
    fn snapshot_includes_attachment_metadata_on_message_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: vec![AttachmentRef {
                    blob_hash: "b".repeat(64),
                    media_type: "text/plain".to_owned(),
                    byte_len: 42,
                    display_name: "note.txt".to_owned(),
                    attachment_id: "att_snapshot_0".to_owned(),
                    encryption: Some(EncryptedBlobRef {
                        mode: PayloadEncryption::Aes256GcmSiv,
                        key_id: "workspace-key-1".to_owned(),
                        nonce: vec![1; 12],
                        aad: b"attachment aad".to_vec(),
                        plaintext_byte_len: 17,
                    }),
                }],
            },
        ));

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, channel, message]).unwrap();

        assert_eq!(snapshot.timeline[0].attachments.len(), 1);
        assert_eq!(snapshot.timeline[0].attachment_count, 1);
        assert_eq!(
            snapshot.timeline[0].attachments[0].attachment_id,
            "att_snapshot_0"
        );
        assert_eq!(snapshot.timeline[0].attachments[0].display_name, "note.txt");
        assert_eq!(snapshot.timeline[0].attachments[0].media_type, "text/plain");
        assert_eq!(snapshot.timeline[0].attachments[0].byte_len, 42);
        assert!(snapshot.timeline[0].attachments[0].encrypted);
        assert_eq!(
            snapshot.timeline[0].attachments[0].local_blob_available,
            None
        );
    }

    #[test]
    fn snapshot_caps_timeline_attachment_rows_and_preserves_count() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let attachment_count = MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS + 3;
        let attachments = (0..attachment_count)
            .map(|index| AttachmentRef {
                blob_hash: format!("{index:064x}"),
                media_type: "application/octet-stream".to_owned(),
                byte_len: u64::try_from(index).unwrap_or(u64::MAX),
                display_name: format!("file-{index:02}.bin"),
                attachment_id: format!("att_capped_{index:02}"),
                encryption: None,
            })
            .collect::<Vec<_>>();
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreated {
                message_id,
                markdown: "many files".to_owned(),
                attachments,
            },
        ));

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, channel, message]).unwrap();

        assert_eq!(snapshot.timeline[0].attachment_count, attachment_count);
        assert_eq!(
            snapshot.timeline[0].attachments.len(),
            MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS
        );
        assert_eq!(
            snapshot.timeline[0].attachments[0].attachment_id,
            "att_capped_00"
        );
        assert_eq!(
            snapshot.timeline[0].attachments[MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS - 1]
                .attachment_id,
            format!(
                "att_capped_{:02}",
                MAX_TIMELINE_ATTACHMENT_SNAPSHOT_ROWS - 1
            )
        );
    }

    #[test]
    fn snapshot_counts_unread_messages_for_reader_device() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let reader = DeviceId("dev_reader".to_owned());
        let peer = DeviceId("dev_peer".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let invite_peer = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::MemberInvited {
                invitee_device_id: peer.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let first_peer_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            peer.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "first unread".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let read_marker = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            reader.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: first_peer_message.event_id.clone(),
            },
        ));
        let second_peer_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            peer,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "second unread".to_owned(),
                attachments: Vec::new(),
            },
        ));

        let keyless = WorkspaceSnapshot::from_events(
            workspace_id.clone(),
            &[
                workspace.clone(),
                channel.clone(),
                invite_peer.clone(),
                first_peer_message.clone(),
                second_peer_message.clone(),
            ],
        )
        .unwrap();
        let unread_before_marker = WorkspaceSnapshot::from_events_for_device(
            workspace_id.clone(),
            &[
                workspace.clone(),
                channel.clone(),
                invite_peer.clone(),
                first_peer_message.clone(),
                second_peer_message.clone(),
            ],
            &reader,
        )
        .unwrap();
        let unread_after_marker = WorkspaceSnapshot::from_events_for_device(
            workspace_id,
            &[
                workspace,
                channel,
                invite_peer,
                first_peer_message,
                read_marker,
                second_peer_message,
            ],
            &reader,
        )
        .unwrap();

        assert_eq!(keyless.channels[0].unread_count, 0);
        assert_eq!(unread_before_marker.channels[0].unread_count, 2);
        assert_eq!(unread_after_marker.channels[0].unread_count, 1);
    }

    #[test]
    fn snapshot_counts_unread_messages_across_channels() {
        let workspace_id = WorkspaceId::new();
        let alpha_channel_id = ChannelId::new();
        let beta_channel_id = ChannelId::new();
        let reader = DeviceId("dev_reader".to_owned());
        let peer = DeviceId("dev_peer".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::ChannelCreated {
                channel_id: alpha_channel_id.clone(),
                name: "alpha".to_owned(),
                is_private: false,
            },
        ));
        let beta = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::ChannelCreated {
                channel_id: beta_channel_id.clone(),
                name: "beta".to_owned(),
                is_private: false,
            },
        ));
        let invite_peer = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            reader.clone(),
            EventBody::MemberInvited {
                invitee_device_id: peer.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let first_alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(alpha_channel_id.clone()),
            peer.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "read alpha".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let first_beta = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(beta_channel_id.clone()),
            peer.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "unread beta one".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let alpha_marker = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(alpha_channel_id.clone()),
            reader.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: alpha_channel_id.clone(),
                event_id: first_alpha.event_id.clone(),
            },
        ));
        let second_alpha = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(alpha_channel_id),
            peer.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "unread alpha".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let own_beta = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(beta_channel_id.clone()),
            reader.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "own beta".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let second_beta = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(beta_channel_id),
            peer,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "unread beta two".to_owned(),
                attachments: Vec::new(),
            },
        ));

        let snapshot = WorkspaceSnapshot::from_events_for_device(
            workspace_id,
            &[
                workspace,
                alpha,
                beta,
                invite_peer,
                first_alpha,
                first_beta,
                alpha_marker,
                second_alpha,
                own_beta,
                second_beta,
            ],
            &reader,
        )
        .unwrap();
        let unread_by_name = snapshot
            .channels
            .iter()
            .map(|channel| (channel.name.as_str(), channel.unread_count))
            .collect::<HashMap<_, _>>();
        let latest_preview_by_name = snapshot
            .channels
            .iter()
            .map(|channel| {
                (
                    channel.name.as_str(),
                    channel
                        .latest_activity
                        .as_ref()
                        .map(|activity| activity.preview.as_str()),
                )
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(unread_by_name.get("alpha"), Some(&1));
        assert_eq!(unread_by_name.get("beta"), Some(&2));
        assert_eq!(
            latest_preview_by_name.get("alpha"),
            Some(&Some("unread alpha"))
        );
        assert_eq!(
            latest_preview_by_name.get("beta"),
            Some(&Some("unread beta two"))
        );
    }

    #[test]
    fn reader_snapshot_hides_private_channel_until_granted() {
        let workspace_id = WorkspaceId::new();
        let public_channel_id = ChannelId::new();
        let private_channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let public_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: public_channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let private_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: private_channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let private_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(private_channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "private plan".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let grant = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelMemberAdded {
                channel_id: private_channel_id.clone(),
                member_device_id: member.clone(),
            },
        ));

        let hidden = WorkspaceSnapshot::from_events_for_device(
            workspace_id.clone(),
            &[
                workspace.clone(),
                invite.clone(),
                public_channel.clone(),
                private_channel.clone(),
                private_message.clone(),
            ],
            &member,
        )
        .unwrap();
        let visible = WorkspaceSnapshot::from_events_for_device(
            workspace_id,
            &[
                workspace,
                invite,
                public_channel,
                private_channel,
                grant,
                private_message,
            ],
            &member,
        )
        .unwrap();

        assert_eq!(hidden.channels.len(), 1);
        assert_eq!(hidden.channels[0].channel_id, public_channel_id.0);
        assert!(hidden.timeline.is_empty());
        assert_eq!(visible.channels.len(), 2);
        assert!(visible.channels.iter().any(|channel| channel.is_private));
        assert_eq!(visible.timeline[0].body, "private plan");
    }

    #[test]
    fn snapshot_json_uses_qml_friendly_camel_case_fields() {
        let snapshot = WorkspaceSnapshot {
            workspace_id: "wrk_test".to_owned(),
            name: "Chaft".to_owned(),
            channels: vec![ChannelSnapshot {
                channel_id: "chn_general".to_owned(),
                name: "general".to_owned(),
                is_private: false,
                unread_count: 3,
                latest_activity: None,
            }],
            profiles: Vec::new(),
            members: vec![WorkspaceMemberSnapshot {
                device_id: "dev_test".to_owned(),
                role: WorkspaceRole::Owner,
                display_name: Some("Mira".to_owned()),
                profile_event_id: Some("evt_profile".to_owned()),
                membership_event_id: "evt_workspace".to_owned(),
            }],
            key_packages: vec![DeviceKeyPackageSnapshot {
                device_id: "dev_test".to_owned(),
                key_package_id: "dkp_test".to_owned(),
                protocol: "openmls/key-package".to_owned(),
                byte_len: 42,
                published_event_id: "evt_key_package".to_owned(),
                physical_ms: 1_700_000_000_025,
            }],
            peer_endpoints: vec![PeerEndpointSnapshot {
                device_id: "dev_test".to_owned(),
                display_name: Some("Mira".to_owned()),
                endpoint_id: "desktop".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: Some(1_700_000_600_000),
                replica_storage_class: Some("full_history".to_owned()),
                replica_retention_hint: Some("best-effort".to_owned()),
                published_event_id: "evt_peer_endpoint".to_owned(),
                physical_ms: 1_700_000_000_050,
            }],
            channel_count: 1,
            profile_count: 0,
            member_count: 1,
            key_package_count: 1,
            peer_endpoint_count: 1,
            timeline_channel_id: None,
            timeline_window: TimelineWindowSnapshot {
                start_index: 0,
                item_count: 1,
                total_count: 1,
                has_more_before: false,
                has_more_after: false,
            },
            timeline: vec![TimelineItem {
                kind: TimelineItemKind::EncryptedMessage,
                event_id: "evt_test".to_owned(),
                message_id: Some("msg_test".to_owned()),
                reply_to_message_id: None,
                reply_preview: None,
                thread_reply_count: 0,
                thread_latest_reply: None,
                thread_reply_previews: Vec::new(),
                channel_id: Some("chn_general".to_owned()),
                author_device_id: Some("dev_test".to_owned()),
                author_display_name: Some("Mira".to_owned()),
                physical_ms: Some(1_700_000_000_000),
                body: "Encrypted message".to_owned(),
                attachment_count: 0,
                attachments: Vec::new(),
                reaction_count: 0,
                reactions: BTreeMap::new(),
                my_reactions: Vec::new(),
                encrypted: true,
                deleted: false,
                missing_parent_ids: Vec::new(),
                grouped_with_previous: true,
                day_boundary: false,
            }],
            gap_count: 1,
            gaps: vec![MissingHistorySnapshot {
                event_id: "evt_gap".to_owned(),
                missing_parent_ids: vec!["evt_parent".to_owned()],
            }],
            invalid_signature_count: 1,
            invalid_signatures: vec![InvalidSignatureSnapshot {
                event_id: "evt_bad_sig".to_owned(),
                channel_id: Some("chn_general".to_owned()),
                author_device_id: "dev_test".to_owned(),
                physical_ms: 1_700_000_000_100,
                reason: "invalid signature".to_owned(),
            }],
        };

        let value = serde_json::to_value(snapshot).unwrap();

        assert_eq!(value["workspaceId"], "wrk_test");
        assert_eq!(value["channelCount"], 1);
        assert_eq!(value["profileCount"], 0);
        assert_eq!(value["memberCount"], 1);
        assert_eq!(value["keyPackageCount"], 1);
        assert_eq!(value["peerEndpointCount"], 1);
        assert_eq!(value["channels"][0]["channelId"], "chn_general");
        assert_eq!(value["channels"][0]["isPrivate"], false);
        assert_eq!(value["channels"][0]["unreadCount"], 3);
        assert_eq!(value["timeline"][0]["eventId"], "evt_test");
        assert_eq!(value["timeline"][0]["messageId"], "msg_test");
        assert_eq!(value["timeline"][0]["threadReplyCount"], 0);
        assert_eq!(
            value["timeline"][0]["threadLatestReply"],
            serde_json::Value::Null
        );
        assert_eq!(
            value["timeline"][0]["threadReplyPreviews"],
            serde_json::json!([])
        );
        assert_eq!(value["timeline"][0]["authorDeviceId"], "dev_test");
        assert_eq!(value["timeline"][0]["authorDisplayName"], "Mira");
        assert_eq!(value["timeline"][0]["physicalMs"], 1_700_000_000_000_i64);
        assert_eq!(value["timeline"][0]["groupedWithPrevious"], true);
        assert_eq!(value["timeline"][0]["dayBoundary"], false);
        assert_eq!(value["profiles"], serde_json::json!([]));
        assert_eq!(value["members"][0]["deviceId"], "dev_test");
        assert_eq!(value["members"][0]["role"], "owner");
        assert_eq!(value["members"][0]["displayName"], "Mira");
        assert_eq!(value["members"][0]["profileEventId"], "evt_profile");
        assert_eq!(value["members"][0]["membershipEventId"], "evt_workspace");
        assert_eq!(value["keyPackages"][0]["deviceId"], "dev_test");
        assert_eq!(value["keyPackages"][0]["keyPackageId"], "dkp_test");
        assert_eq!(value["keyPackages"][0]["protocol"], "openmls/key-package");
        assert_eq!(value["keyPackages"][0]["byteLen"], 42);
        assert_eq!(
            value["keyPackages"][0]["publishedEventId"],
            "evt_key_package"
        );
        assert_eq!(value["keyPackages"][0]["physicalMs"], 1_700_000_000_025_i64);
        assert_eq!(value["peerEndpoints"][0]["deviceId"], "dev_test");
        assert_eq!(value["peerEndpoints"][0]["displayName"], "Mira");
        assert_eq!(value["peerEndpoints"][0]["endpointId"], "desktop");
        assert_eq!(
            value["peerEndpoints"][0]["endpoint"],
            "direct+tcp://127.0.0.1:7777"
        );
        assert_eq!(value["peerEndpoints"][0]["transport"], "direct-tcp");
        assert_eq!(value["peerEndpoints"][0]["isBackupPeer"], true);
        assert_eq!(
            value["peerEndpoints"][0]["expiresAtMs"],
            1_700_000_600_000_i64
        );
        assert_eq!(
            value["peerEndpoints"][0]["publishedEventId"],
            "evt_peer_endpoint"
        );
        assert_eq!(
            value["peerEndpoints"][0]["physicalMs"],
            1_700_000_000_050_i64
        );
        assert_eq!(value["timelineWindow"]["startIndex"], 0);
        assert_eq!(value["timelineWindow"]["itemCount"], 1);
        assert_eq!(value["timelineWindow"]["totalCount"], 1);
        assert_eq!(value["timelineWindow"]["hasMoreBefore"], false);
        assert_eq!(value["timelineWindow"]["hasMoreAfter"], false);
        assert_eq!(value["timeline"][0]["attachments"], serde_json::json!([]));
        assert_eq!(value["timeline"][0]["reactionCount"], 0);
        assert_eq!(value["timeline"][0]["reactions"], serde_json::json!({}));
        assert_eq!(value["timeline"][0]["myReactions"], serde_json::json!([]));
        assert_eq!(
            value["timeline"][0]["missingParentIds"],
            serde_json::json!([])
        );
        assert_eq!(value["gapCount"], 1);
        assert_eq!(value["gaps"][0]["missingParentIds"][0], "evt_parent");
        assert_eq!(value["invalidSignatureCount"], 1);
        assert_eq!(
            value["invalidSignatures"][0]["physicalMs"],
            1_700_000_000_100_i64
        );
        assert_eq!(value["invalidSignatures"][0]["eventId"], "evt_bad_sig");
        assert_eq!(value["invalidSignatures"][0]["channelId"], "chn_general");
        assert!(value.get("workspace_id").is_none());
    }

    #[test]
    fn snapshot_reports_missing_history_as_timeline_gap() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let missing_parent_id = EventId("evt_missing".to_owned());
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            DeviceId("dev_test".to_owned()),
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        );
        message.parents = vec![missing_parent_id.clone()];
        let message = signed(message);

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, std::slice::from_ref(&message)).unwrap();

        assert!(snapshot.channels.is_empty());
        assert_eq!(snapshot.gaps.len(), 1);
        assert_eq!(snapshot.gaps[0].event_id, message.event_id.0);
        assert_eq!(
            snapshot.gaps[0].missing_parent_ids,
            vec![missing_parent_id.0]
        );
        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::MissingHistoryGap
        );
    }

    #[test]
    fn snapshot_reports_unauthorized_ready_event_as_gap_without_rendering_message() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let outsider = DeviceId("dev_outsider".to_owned());
        let message_id = MessageId::new();
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            outsider,
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, channel, message]).unwrap();

        assert_eq!(snapshot.timeline.len(), 1);
        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::MissingHistoryGap
        );
        assert_eq!(snapshot.timeline[0].body, "Missing authorization context");
        assert!(snapshot.timeline[0].missing_parent_ids.is_empty());
    }

    #[test]
    fn snapshot_reports_invalid_signature_without_rendering_forged_message() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let identity = DeviceIdentity::generate();
        let owner = identity.device_id().clone();
        let workspace = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft Labs".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: sealed_payload(),
                attachments: Vec::new(),
            },
        ));
        forged.signature[0] ^= 1;
        let forged_event_id = forged.event_id.0.clone();

        let snapshot =
            WorkspaceSnapshot::from_events(workspace_id, &[workspace, channel, forged]).unwrap();

        assert_eq!(snapshot.invalid_signatures.len(), 1);
        assert_eq!(snapshot.invalid_signatures[0].event_id, forged_event_id);
        assert_eq!(
            snapshot.invalid_signatures[0].channel_id,
            Some(channel_id.0)
        );
        assert_eq!(snapshot.invalid_signatures[0].author_device_id, owner.0);
        assert_eq!(snapshot.timeline.len(), 1);
        assert_eq!(
            snapshot.timeline[0].kind,
            TimelineItemKind::InvalidSignature
        );
        assert_eq!(snapshot.timeline[0].body, "Failed signature verification");
        assert!(snapshot.timeline[0].attachments.is_empty());
    }

    #[test]
    fn snapshot_groups_same_author_rows_within_five_minutes() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let author = DeviceId("dev_author".to_owned());
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &author);
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "first",
            1_700_000_000_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "second",
            1_700_000_000_000 + MAX_GROUPED_TIMELINE_ROW_GAP_MS,
        ));

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert!(!snapshot.timeline[0].grouped_with_previous);
        assert!(snapshot.timeline[0].day_boundary);
        assert!(snapshot.timeline[1].grouped_with_previous);
        assert!(!snapshot.timeline[1].day_boundary);
    }

    #[test]
    fn snapshot_breaks_row_grouping_on_author_change() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let other = DeviceId("dev_other".to_owned());
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &owner);
        events.push(signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: other.clone(),
                role: WorkspaceRole::Member,
            },
        )));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &owner,
            "owner message",
            1_700_000_000_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &other,
            "other message",
            1_700_000_001_000,
        ));

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert!(!snapshot.timeline[1].grouped_with_previous);
        assert!(!snapshot.timeline[1].day_boundary);
    }

    #[test]
    fn snapshot_breaks_row_grouping_after_five_minute_gap() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let author = DeviceId("dev_author".to_owned());
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &author);
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "first",
            1_700_000_000_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "too late",
            1_700_000_000_000 + MAX_GROUPED_TIMELINE_ROW_GAP_MS + 1,
        ));

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 2);
        assert!(!snapshot.timeline[1].grouped_with_previous);
        assert!(!snapshot.timeline[1].day_boundary);
    }

    #[test]
    fn snapshot_marks_day_boundary_on_utc_day_change() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let author = DeviceId("dev_author".to_owned());
        let utc_midnight_ms = 19_676 * MS_PER_UTC_DAY;
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &author);
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "yesterday first",
            utc_midnight_ms - 120_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "yesterday second",
            utc_midnight_ms - 60_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "today",
            utc_midnight_ms + 60_000,
        ));

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 3);
        assert!(snapshot.timeline[0].day_boundary);
        assert!(!snapshot.timeline[1].day_boundary);
        assert!(snapshot.timeline[1].grouped_with_previous);
        assert!(snapshot.timeline[2].day_boundary);
        assert!(!snapshot.timeline[2].grouped_with_previous);
    }

    #[test]
    fn snapshot_never_groups_gap_or_invalid_signature_rows() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let identity = DeviceIdentity::generate();
        let owner = identity.device_id().clone();
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &owner);
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &owner,
            "first",
            1_700_000_000_000,
        ));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &owner,
            "second",
            1_700_000_001_000,
        ));
        let mut forged = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged".to_owned(),
                attachments: Vec::new(),
            },
        );
        forged.timestamp = HybridTimestamp {
            physical_ms: 1_700_000_002_000,
            logical: 0,
        };
        let mut forged = identity.sign_event(forged);
        forged.signature[0] ^= 1;
        let mut gap = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "gap".to_owned(),
                attachments: Vec::new(),
            },
        );
        gap.timestamp = HybridTimestamp {
            physical_ms: 1_700_000_003_000,
            logical: 0,
        };
        gap.parents = vec![EventId("evt_missing_parent".to_owned())];
        events.push(signed(gap));
        events.push(forged);

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 4);
        assert!(snapshot.timeline[1].grouped_with_previous);
        assert_eq!(
            snapshot.timeline[2].kind,
            TimelineItemKind::MissingHistoryGap
        );
        assert!(!snapshot.timeline[2].grouped_with_previous);
        assert!(!snapshot.timeline[2].day_boundary);
        assert_eq!(
            snapshot.timeline[3].kind,
            TimelineItemKind::InvalidSignature
        );
        assert!(!snapshot.timeline[3].grouped_with_previous);
        assert!(!snapshot.timeline[3].day_boundary);
    }

    #[test]
    fn snapshot_breaks_row_grouping_across_deleted_message_tombstones() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let author = DeviceId("dev_author".to_owned());
        let deleted_message_id = MessageId::new();
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &author);
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "first",
            1_700_000_000_000,
        ));
        let mut tombstone = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            author.clone(),
            EventBody::MessageCreated {
                message_id: deleted_message_id.clone(),
                markdown: "soon deleted".to_owned(),
                attachments: Vec::new(),
            },
        );
        tombstone.timestamp = HybridTimestamp {
            physical_ms: 1_700_000_001_000,
            logical: 0,
        };
        events.push(signed(tombstone));
        events.push(signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            author.clone(),
            EventBody::MessageDeleted {
                message_id: deleted_message_id,
            },
        )));
        events.push(timestamped_message(
            &workspace_id,
            &channel_id,
            &author,
            "after tombstone",
            1_700_000_002_000,
        ));

        let snapshot = WorkspaceSnapshot::from_events(workspace_id, &events).unwrap();

        assert_eq!(snapshot.timeline.len(), 3);
        assert!(!snapshot.timeline[0].grouped_with_previous);
        assert!(snapshot.timeline[1].deleted);
        assert!(!snapshot.timeline[1].grouped_with_previous);
        assert!(!snapshot.timeline[2].grouped_with_previous);
        assert!(!snapshot.timeline[2].day_boundary);
    }

    #[test]
    fn snapshot_window_pages_group_against_row_before_window() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let author = DeviceId("dev_author".to_owned());
        let mut events = workspace_with_channel(&workspace_id, &channel_id, &author);
        for index in 0..4 {
            events.push(timestamped_message(
                &workspace_id,
                &channel_id,
                &author,
                &format!("message {index}"),
                1_700_000_000_000 + i64::from(index) * 1_000,
            ));
        }

        let first_page = WorkspaceSnapshot::from_events_with_options(
            workspace_id.clone(),
            &events,
            &WorkspaceSnapshotOptions::window(0, 2),
        )
        .unwrap();
        let later_page = WorkspaceSnapshot::from_events_with_options(
            workspace_id.clone(),
            &events,
            &WorkspaceSnapshotOptions::window(2, 2),
        )
        .unwrap();
        let latest_page = WorkspaceSnapshot::from_events_with_options(
            workspace_id,
            &events,
            &WorkspaceSnapshotOptions::latest(2),
        )
        .unwrap();

        assert_eq!(first_page.timeline.len(), 2);
        assert!(!first_page.timeline[0].grouped_with_previous);
        assert!(first_page.timeline[0].day_boundary);
        assert_eq!(later_page.timeline.len(), 2);
        assert!(later_page.timeline[0].grouped_with_previous);
        assert!(!later_page.timeline[0].day_boundary);
        assert!(later_page.timeline[1].grouped_with_previous);
        assert_eq!(latest_page.timeline.len(), 2);
        assert!(latest_page.timeline[0].grouped_with_previous);
        assert!(!latest_page.timeline[0].day_boundary);
    }
}
