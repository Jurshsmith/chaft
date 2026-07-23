use std::collections::{BTreeMap, BTreeSet, HashMap};

use chaft_core::WorkspaceState;
use chaft_crypto::open_message_markdown;
use chaft_search::{SearchIndex, query_has_search_terms};
use chaft_types::{ChannelId, EventId, MessageId, WorkspaceId};

use crate::{
    IndexedWorkspaceSearch, LOCAL_SEARCH_RAW_HIT_LIMIT, LOCAL_SECRET_KIND_WORKSPACE_KEY,
    LocalRuntime, RuntimeError, SearchedWorkspace, WorkspaceKey, WorkspaceSearchHit,
    validate_search_query_size, validate_workspace_id_reference, verified_local_events_for_runtime,
};

impl LocalRuntime {
    pub(crate) fn open_search_index(&self) -> Result<SearchIndex, RuntimeError> {
        Ok(SearchIndex::open(&self.paths.search_index)?)
    }

    pub(crate) fn index_message_plaintext(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        event_id: &EventId,
        physical_ms: i64,
        markdown: &str,
    ) -> Result<(), RuntimeError> {
        self.open_search_index()?.index_message(
            workspace_id,
            channel_id,
            message_id,
            event_id,
            physical_ms,
            markdown,
        )?;
        Ok(())
    }

    pub(crate) fn remove_message_from_search(
        &self,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
    ) -> Result<(), RuntimeError> {
        self.open_search_index()?
            .remove_message(workspace_id, message_id)?;
        Ok(())
    }

    pub fn reindex_workspace_search(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<IndexedWorkspaceSearch, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        if workspace_key.is_none() && !self.has_openmls_group_state(&workspace_id) {
            self.read_local_secret_file(
                &self.workspace_key_path(&workspace_id),
                LOCAL_SECRET_KIND_WORKSPACE_KEY,
            )?;
        }
        self.reindex_workspace_search_with_key(&workspace_id, workspace_key.as_ref())
    }

    pub fn search_workspace_messages(
        &self,
        workspace_id: WorkspaceId,
        query: impl AsRef<str>,
    ) -> Result<SearchedWorkspace, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let query = query.as_ref().trim().to_owned();
        validate_search_query_size(&query)?;
        if !query_has_search_terms(&query) {
            return Ok(SearchedWorkspace::empty(workspace_id, query));
        }
        let mut raw_hits = self.open_search_index()?.search_limited(
            &workspace_id,
            &query,
            LOCAL_SEARCH_RAW_HIT_LIMIT.saturating_add(1),
        )?;
        if raw_hits.is_empty() {
            return Ok(SearchedWorkspace::empty(workspace_id, query));
        }
        let has_more_raw_candidates = raw_hits.len() > LOCAL_SEARCH_RAW_HIT_LIMIT;
        raw_hits.truncate(LOCAL_SEARCH_RAW_HIT_LIMIT);
        let raw_candidate_count = raw_hits.len();
        let raw_hit_event_ids = raw_hits
            .iter()
            .map(|hit| hit.event_id.clone())
            .collect::<Vec<_>>();
        let servable_event_ids = self
            .store
            .filter_servable_event_ids_for_workspace(&workspace_id.0, &raw_hit_event_ids)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        raw_hits.retain(|hit| servable_event_ids.contains(&hit.event_id));
        if raw_hits.is_empty() {
            return Ok(SearchedWorkspace::bounded(
                workspace_id,
                query,
                Vec::new(),
                raw_candidate_count,
                has_more_raw_candidates,
            ));
        }

        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&events)?;
        let applied_event_ids = report.applied_events.into_iter().collect::<BTreeSet<_>>();
        let event_author_and_physical_ms_by_id = events
            .iter()
            .map(|event| {
                (
                    event.event_id.clone(),
                    (
                        event.event.author_device_id.clone(),
                        event.event.timestamp.physical_ms,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut hits: Vec<_> = raw_hits
            .into_iter()
            .filter(|hit| {
                applied_event_ids.contains(&hit.event_id)
                    && state.channel_accessible_to(&hit.channel_id, self.identity.device_id())
            })
            .filter_map(|hit| {
                let (author_device_id, physical_ms) =
                    event_author_and_physical_ms_by_id.get(&hit.event_id)?;
                let channel = state.channels.get(&hit.channel_id)?;
                let author_display_name = state
                    .profiles
                    .get(author_device_id)
                    .map(|profile| profile.display_name.clone());
                let author_avatar_id = state
                    .person_device_links
                    .get(author_device_id)
                    .and_then(|link| state.person_profiles.get(&link.person_id))
                    .map(|profile| profile.avatar_id.trim())
                    .filter(|avatar_id| !avatar_id.is_empty())
                    .map(str::to_owned)
                    .or_else(|| {
                        state
                            .profiles
                            .get(author_device_id)
                            .map(|profile| profile.avatar_id.trim())
                            .filter(|avatar_id| !avatar_id.is_empty())
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                Some(WorkspaceSearchHit {
                    workspace_id: workspace_id.0.clone(),
                    event_id: hit.event_id.0,
                    message_id: hit.message_id.0,
                    channel_id: hit.channel_id.0,
                    channel_name: channel.name.clone(),
                    channel_is_private: channel.is_private,
                    author_device_id: author_device_id.0.clone(),
                    author_display_name,
                    author_avatar_id,
                    physical_ms: *physical_ms,
                    body: hit.markdown,
                    body_char_count: hit.markdown_char_count,
                    body_truncated: hit.markdown_truncated,
                })
            })
            .collect();
        hits.sort_by(|left, right| {
            right
                .physical_ms
                .cmp(&left.physical_ms)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.message_id.cmp(&right.message_id))
        });
        Ok(SearchedWorkspace::bounded(
            workspace_id,
            query,
            hits,
            raw_candidate_count,
            has_more_raw_candidates,
        ))
    }

    pub(crate) fn reindex_workspace_search_if_key_available(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(), RuntimeError> {
        let workspace_key = self.load_workspace_key(workspace_id)?;
        if workspace_key.is_some() || self.has_openmls_group_state(workspace_id) {
            self.reindex_workspace_search_with_key(workspace_id, workspace_key.as_ref())?;
        }
        Ok(())
    }

    pub(crate) fn reindex_workspace_search_with_key(
        &self,
        workspace_id: &WorkspaceId,
        workspace_key: Option<&WorkspaceKey>,
    ) -> Result<IndexedWorkspaceSearch, RuntimeError> {
        let events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let events = verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let physical_ms_by_event_id = events
            .iter()
            .map(|event| (&event.event_id, event.event.timestamp.physical_ms))
            .collect::<HashMap<_, _>>();

        let index = self.open_search_index()?;
        index.clear_workspace(workspace_id)?;
        let mut indexed_message_count = 0;

        for message in state.messages.values() {
            if message.deleted {
                continue;
            }
            if !state.channel_accessible_to(&message.channel_id, self.identity.device_id()) {
                continue;
            }

            let markdown = if let Some(sealed_markdown) = message.sealed_markdown.as_ref() {
                let Some(content_key) = self.content_key_for_materialized_payload(
                    workspace_id,
                    &message.channel_id,
                    &state,
                    workspace_key,
                    &sealed_markdown.key_id,
                )?
                else {
                    continue;
                };
                open_message_markdown(
                    content_key.content_key(),
                    sealed_markdown,
                    workspace_id,
                    &message.channel_id,
                    &message.message_id,
                )?
            } else {
                message.markdown.clone()
            };
            if markdown.trim().is_empty() {
                continue;
            }

            index.index_message(
                workspace_id,
                &message.channel_id,
                &message.message_id,
                &message.author_event_id,
                physical_ms_by_event_id
                    .get(&message.author_event_id)
                    .copied()
                    .unwrap_or_default(),
                &markdown,
            )?;
            indexed_message_count += 1;
        }

        Ok(IndexedWorkspaceSearch {
            workspace_id: workspace_id.0.clone(),
            indexed_message_count,
        })
    }
}
