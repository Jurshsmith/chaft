use std::ffi::c_char;

use chaft_app::{
    ChannelSnapshot, DeviceKeyPackageSnapshot, DeviceProfileSnapshot, MAX_TIMELINE_WINDOW_ROWS,
    TimelineItem, TimelineItemKind, WorkspaceMemberSnapshot, WorkspaceSnapshot,
    WorkspaceSnapshotOptions,
};
use chaft_store::EventStore;
use chaft_types::{ChannelId, SignedEvent, WorkspaceAccessPolicy, WorkspaceId, WorkspaceRole};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::{ffi_channel_id_arg, ffi_workspace_id_arg},
    input::{WORKSPACE_EVENTS_JSON_MAX_BYTES, read_c_string, validate_json_payload_size},
};

pub(crate) fn workspace_snapshot_from_events_result(
    workspace_id: *const c_char,
    events_json: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let events_json = read_c_string(events_json, "events_json")?;
        validate_json_payload_size(
            &events_json,
            WORKSPACE_EVENTS_JSON_MAX_BYTES,
            "events_json_too_large",
            "events JSON",
        )?;
        let events = serde_json::from_str::<Vec<SignedEvent>>(&events_json)
            .map_err(|error| ffi_error("invalid_events_json", error.to_string()))?;
        WorkspaceSnapshot::from_events(WorkspaceId(workspace_id), &events)
            .map_err(|error| ffi_error("snapshot_materialization_failed", error.to_string()))
    })
}

pub(crate) fn workspace_snapshot_from_store_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::full(),
    )
}

pub(crate) fn workspace_snapshot_from_store_latest_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::latest(bounded_timeline_limit(timeline_limit)),
    )
}

pub(crate) fn workspace_snapshot_from_store_window_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    workspace_snapshot_from_store_with_options_result(
        store_path,
        workspace_id,
        &WorkspaceSnapshotOptions::window(timeline_start, bounded_timeline_limit(timeline_limit)),
    )
}

pub(crate) fn decrypted_workspace_snapshot_from_runtime_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::full(),
    )
}

pub(crate) fn decrypted_workspace_snapshot_from_runtime_latest_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::latest(bounded_timeline_limit(timeline_limit)),
    )
}

pub(crate) fn decrypted_workspace_snapshot_from_runtime_window_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    decrypted_workspace_snapshot_from_runtime_with_options_result(
        data_dir,
        identity_file,
        workspace_id,
        &WorkspaceSnapshotOptions::window(timeline_start, bounded_timeline_limit(timeline_limit)),
    )
}

pub(crate) fn decrypted_workspace_channel_snapshot_from_runtime_latest_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            &WorkspaceSnapshotOptions::latest_for_channel(
                channel_id,
                bounded_timeline_limit(timeline_limit),
            ),
        )
    })
}

pub(crate) fn decrypted_workspace_channel_snapshot_from_runtime_window_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    timeline_start: usize,
    timeline_limit: usize,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            &WorkspaceSnapshotOptions::window_for_channel(
                channel_id,
                timeline_start,
                bounded_timeline_limit(timeline_limit),
            ),
        )
    })
}

pub(crate) fn demo_workspace_snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot {
        workspace_id: "wrk_demo".to_owned(),
        name: "Chaft Labs".to_owned(),
        access_policy: WorkspaceAccessPolicy::InviteOnly,
        channels: vec![
            ChannelSnapshot {
                channel_id: "chn_general".to_owned(),
                name: "general".to_owned(),
                topic: "Daily coordination and launch notes".to_owned(),
                archived: false,
                is_private: false,
                member_count: 3,
                member_device_ids: Vec::new(),
                direct_message: false,
                direct_message_participant_device_ids: Vec::new(),
                unread_count: 0,
                latest_activity: None,
                access_history: Vec::new(),
            },
            ChannelSnapshot {
                channel_id: "chn_runtime".to_owned(),
                name: "p2p-runtime".to_owned(),
                topic: "Sync health, transport checks, and reachable devices".to_owned(),
                archived: false,
                is_private: false,
                member_count: 3,
                member_device_ids: Vec::new(),
                direct_message: false,
                direct_message_participant_device_ids: Vec::new(),
                unread_count: 2,
                latest_activity: None,
                access_history: Vec::new(),
            },
            ChannelSnapshot {
                channel_id: "chn_design".to_owned(),
                name: "design-system".to_owned(),
                topic: "Desktop polish and user-facing flows".to_owned(),
                archived: false,
                is_private: false,
                member_count: 3,
                member_device_ids: Vec::new(),
                direct_message: false,
                direct_message_participant_device_ids: Vec::new(),
                unread_count: 0,
                latest_activity: None,
                access_history: Vec::new(),
            },
            ChannelSnapshot {
                channel_id: "chn_replicas".to_owned(),
                name: "replica-nodes".to_owned(),
                topic: "Private replica and backup-device work".to_owned(),
                archived: false,
                is_private: true,
                member_count: 1,
                member_device_ids: vec!["dev_alex".to_owned()],
                direct_message: false,
                direct_message_participant_device_ids: Vec::new(),
                unread_count: 1,
                latest_activity: None,
                access_history: Vec::new(),
            },
        ],
        profiles: vec![DeviceProfileSnapshot {
            device_id: "dev_mira".to_owned(),
            display_name: "Mira".to_owned(),
            updated_event_id: "evt_profile_mira".to_owned(),
        }],
        person_profiles: Vec::new(),
        person_device_links: Vec::new(),
        members: vec![WorkspaceMemberSnapshot {
            device_id: "dev_mira".to_owned(),
            role: WorkspaceRole::Owner,
            display_name: Some("Mira".to_owned()),
            profile_event_id: Some("evt_profile_mira".to_owned()),
            membership_event_id: "evt_workspace".to_owned(),
        }],
        invites: Vec::new(),
        join_requests: Vec::new(),
        key_packages: vec![DeviceKeyPackageSnapshot {
            device_id: "dev_mira".to_owned(),
            key_package_id: "dkp_mira_demo".to_owned(),
            protocol: "openmls/key-package".to_owned(),
            byte_len: 512,
            published_event_id: "evt_key_package_mira".to_owned(),
            physical_ms: 1_700_000_000_010,
        }],
        peer_endpoints: Vec::new(),
        channel_count: 4,
        profile_count: 1,
        person_profile_count: 0,
        person_device_link_count: 0,
        member_count: 1,
        invite_count: 0,
        join_request_count: 0,
        key_package_count: 1,
        peer_endpoint_count: 0,
        timeline_channel_id: None,
        timeline_window: chaft_app::TimelineWindowSnapshot {
            start_index: 0,
            item_count: 2,
            total_count: 2,
            has_more_before: false,
            has_more_after: false,
        },
        timeline: vec![
            TimelineItem {
                kind: TimelineItemKind::EncryptedMessage,
                event_id: "evt_ciphertext".to_owned(),
                message_id: Some("msg_ciphertext".to_owned()),
                reply_to_message_id: None,
                reply_preview: None,
                thread_reply_count: 0,
                thread_latest_reply: None,
                thread_reply_previews: Vec::new(),
                channel_id: Some("chn_general".to_owned()),
                author_device_id: Some("dev_mira".to_owned()),
                author_display_name: Some("Mira".to_owned()),
                physical_ms: Some(1_700_000_000_000),
                body: "Encrypted message".to_owned(),
                attachment_count: 0,
                attachments: Vec::new(),
                reaction_count: 0,
                reactions: Default::default(),
                my_reactions: Vec::new(),
                encrypted: true,
                deleted: false,
                missing_parent_ids: Vec::new(),
                grouped_with_previous: false,
                day_boundary: true,
                body_decrypted: false,
            },
            TimelineItem {
                kind: TimelineItemKind::MissingHistoryGap,
                event_id: "evt_later_slice".to_owned(),
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
                body: "Missing 2 parent event(s)".to_owned(),
                attachment_count: 0,
                attachments: Vec::new(),
                reaction_count: 0,
                reactions: Default::default(),
                my_reactions: Vec::new(),
                encrypted: false,
                deleted: false,
                missing_parent_ids: vec!["evt_parent_a".to_owned(), "evt_parent_b".to_owned()],
                grouped_with_previous: false,
                day_boundary: false,
                body_decrypted: false,
            },
        ],
        gap_count: 0,
        gaps: Vec::new(),
        invalid_signature_count: 0,
        invalid_signatures: Vec::new(),
    }
}

fn workspace_snapshot_from_store_with_options_result(
    store_path: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        let store_path = read_c_string(store_path, "store_path")?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let store = EventStore::open(&store_path)
            .map_err(|error| ffi_error("store_open_failed", error.to_string()))?;
        let events = store
            .list_events_for_workspace(&workspace_id)
            .map_err(|error| ffi_error("store_read_failed", error.to_string()))?;
        WorkspaceSnapshot::from_events_with_options(WorkspaceId(workspace_id), &events, options)
            .map_err(|error| ffi_error("snapshot_materialization_failed", error.to_string()))
    })
}

fn decrypted_workspace_snapshot_from_runtime_with_options_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> FfiResult<WorkspaceSnapshot> {
    result_envelope(|| {
        decrypted_workspace_snapshot_from_runtime_with_options(
            data_dir,
            identity_file,
            workspace_id,
            options,
        )
    })
}

fn decrypted_workspace_snapshot_from_runtime_with_options(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    options: &WorkspaceSnapshotOptions,
) -> Result<WorkspaceSnapshot, FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let identity_file = if identity_file.is_null() {
        None
    } else {
        Some(read_c_string(identity_file, "identity_file")?.into())
    };
    let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
    let runtime = crate::open_runtime_from_paths(&data_dir, identity_file)?;
    runtime
        .decrypted_workspace_snapshot_with_options(WorkspaceId(workspace_id), options)
        .map_err(|error| ffi_error("runtime_snapshot_failed", error.to_string()))
}

fn bounded_timeline_limit(limit: usize) -> usize {
    limit.min(MAX_TIMELINE_WINDOW_ROWS)
}
