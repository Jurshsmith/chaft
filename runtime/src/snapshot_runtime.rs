use std::collections::{BTreeSet, HashMap};

use chaft_app::{
    ChannelSnapshot, WorkspaceChannelPage, WorkspaceSnapshot, WorkspaceSnapshotOptions,
    body_override_event_ids_for_snapshot_window,
};
use chaft_core::WorkspaceState;
use chaft_crypto::open_message_markdown;
use chaft_types::{ChannelId, EventId, MessageId, WorkspaceId};

use crate::{
    LOCAL_SECRET_KIND_WORKSPACE_KEY, LocalRuntime, RuntimeError, WorkspaceKey,
    validate_channel_id_reference, validate_workspace_id_reference,
    verified_local_events_for_runtime,
};

impl LocalRuntime {
    pub fn workspace_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.workspace_snapshot_with_options(workspace_id, &WorkspaceSnapshotOptions::full())
    }

    pub fn workspace_snapshot_with_options(
        &self,
        workspace_id: WorkspaceId,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        if let Some(channel_id) = options.timeline_channel_id.as_ref() {
            validate_channel_id_reference(channel_id)?;
        }
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let mut snapshot = WorkspaceSnapshot::from_events_for_device_with_options(
            workspace_id.clone(),
            &events,
            self.identity.device_id(),
            options,
        )?;
        self.annotate_channel_content_readiness(&workspace_id, &mut snapshot.channels)?;
        self.annotate_attachment_availability(&mut snapshot)?;
        Ok(snapshot)
    }

    pub fn decrypted_workspace_snapshot(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::full(),
        )
    }

    pub fn decrypted_workspace_snapshot_with_options(
        &self,
        workspace_id: WorkspaceId,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        if let Some(channel_id) = options.timeline_channel_id.as_ref() {
            validate_channel_id_reference(channel_id)?;
        }
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        let raw_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        self.validate_snapshot_channel_scope(&workspace_id, &state, options)?;
        let body_override_event_ids = body_override_event_ids_for_snapshot_window(
            &state,
            &report,
            &raw_events,
            self.identity.device_id(),
            options,
        );
        let body_overrides = self.decrypted_body_overrides_for_event_ids(
            &workspace_id,
            &state,
            workspace_key.as_ref(),
            &body_override_event_ids,
        )?;

        let mut snapshot =
            WorkspaceSnapshot::from_state_report_for_device_and_body_overrides_with_options(
                workspace_id.clone(),
                &state,
                &report,
                &raw_events,
                self.identity.device_id(),
                &body_overrides,
                options,
            );
        self.annotate_channel_content_readiness(&workspace_id, &mut snapshot.channels)?;
        self.annotate_attachment_availability(&mut snapshot)?;
        Ok(snapshot)
    }

    pub fn decrypted_workspace_channel_snapshot_latest(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        timeline_limit: usize,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::latest_for_channel(channel_id, timeline_limit),
        )
    }

    pub fn decrypted_workspace_channel_snapshot_window(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        timeline_start: usize,
        timeline_limit: usize,
    ) -> Result<WorkspaceSnapshot, RuntimeError> {
        self.decrypted_workspace_snapshot_with_options(
            workspace_id,
            &WorkspaceSnapshotOptions::window_for_channel(
                channel_id,
                timeline_start,
                timeline_limit,
            ),
        )
    }

    pub(crate) fn validate_snapshot_channel_scope(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        options: &WorkspaceSnapshotOptions,
    ) -> Result<(), RuntimeError> {
        let Some(channel_id) = options.timeline_channel_id.as_ref() else {
            return Ok(());
        };
        validate_channel_id_reference(channel_id)?;
        if !state.channels.contains_key(channel_id) {
            return Err(RuntimeError::ChannelNotFound {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }
        if !state.channel_accessible_to(channel_id, self.identity.device_id()) {
            return Err(RuntimeError::ChannelAccessDenied {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
                device_id: self.identity.device_id().clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn decrypted_body_overrides_for_event_ids(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        workspace_key: Option<&WorkspaceKey>,
        body_override_event_ids: &BTreeSet<EventId>,
    ) -> Result<HashMap<String, String>, RuntimeError> {
        let mut body_overrides = HashMap::new();
        // Loading an MLS group decrypts and validates its persisted state.
        // Reuse the resolved epoch key for every visible message that shares
        // the same channel/key pair instead of repeating that I/O per row.
        let mut resolved_content_keys = HashMap::new();
        for message in state.messages.values() {
            if !body_override_event_ids.contains(&message.author_event_id) {
                continue;
            }
            if !state.channel_accessible_to(&message.channel_id, self.identity.device_id()) {
                continue;
            }
            if let Some(sealed_markdown) = message.sealed_markdown.as_ref() {
                let cache_key = (message.channel_id.0.clone(), sealed_markdown.key_id.clone());
                if !resolved_content_keys.contains_key(&cache_key) {
                    let content_key = self.content_key_for_materialized_payload(
                        workspace_id,
                        &message.channel_id,
                        state,
                        workspace_key,
                        &sealed_markdown.key_id,
                    )?;
                    resolved_content_keys.insert(cache_key.clone(), content_key);
                }
                let Some(content_key) = resolved_content_keys
                    .get(&cache_key)
                    .and_then(Option::as_ref)
                else {
                    continue;
                };
                let plaintext = open_message_markdown(
                    content_key.content_key(),
                    sealed_markdown,
                    workspace_id,
                    &message.channel_id,
                    &message.message_id,
                )?;
                body_overrides.insert(message.author_event_id.0.clone(), plaintext);
            }
        }
        Ok(body_overrides)
    }

    pub(crate) fn channel_page_body_override_event_ids(
        state: &WorkspaceState,
        page: &WorkspaceChannelPage,
    ) -> BTreeSet<EventId> {
        Self::channel_rows_body_override_event_ids(state, &page.channels)
    }

    pub(crate) fn channel_rows_body_override_event_ids(
        state: &WorkspaceState,
        channels: &[ChannelSnapshot],
    ) -> BTreeSet<EventId> {
        let mut event_ids = BTreeSet::new();
        for channel in channels {
            let Some(activity) = channel.latest_activity.as_ref() else {
                continue;
            };
            event_ids.insert(EventId(activity.event_id.clone()));
            let Some(message_id) = activity.message_id.as_ref() else {
                continue;
            };
            let Some(message) = state.messages.get(&MessageId(message_id.clone())) else {
                continue;
            };
            if !message.deleted && message.sealed_markdown.is_some() {
                event_ids.insert(message.author_event_id.clone());
            }
        }
        event_ids
    }

    pub(crate) fn annotate_attachment_availability(
        &self,
        snapshot: &mut WorkspaceSnapshot,
    ) -> Result<(), RuntimeError> {
        let blob_store = self.open_blob_store()?;
        for item in &mut snapshot.timeline {
            for attachment in &mut item.attachments {
                attachment.local_blob_available =
                    Some(blob_store.has_complete_blob(&attachment.blob_hash)?);
            }
        }
        Ok(())
    }
}
