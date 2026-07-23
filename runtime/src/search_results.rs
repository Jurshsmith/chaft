use chaft_types::WorkspaceId;
use serde::{Deserialize, Serialize};

pub(crate) const LOCAL_SEARCH_RAW_HIT_LIMIT: usize = 500;
pub(crate) const LOCAL_SEARCH_VISIBLE_HIT_LIMIT: usize = 50;
pub(crate) const SEARCH_QUERY_MAX_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedWorkspaceSearch {
    pub workspace_id: String,
    pub indexed_message_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchedWorkspace {
    pub workspace_id: String,
    pub query: String,
    #[serde(default)]
    pub item_count: usize,
    #[serde(default)]
    pub hit_count: usize,
    #[serde(default)]
    pub raw_candidate_count: usize,
    #[serde(default = "default_local_search_raw_hit_limit")]
    pub raw_candidate_limit: usize,
    #[serde(default = "default_local_search_visible_hit_limit")]
    pub visible_hit_limit: usize,
    #[serde(default)]
    pub has_more_hits: bool,
    pub hits: Vec<WorkspaceSearchHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchHit {
    pub workspace_id: String,
    pub event_id: String,
    pub message_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub channel_is_private: bool,
    pub author_device_id: String,
    pub author_display_name: Option<String>,
    #[serde(default)]
    pub author_avatar_id: String,
    pub physical_ms: i64,
    pub body: String,
    #[serde(default)]
    pub body_char_count: usize,
    #[serde(default)]
    pub body_truncated: bool,
}

const fn default_local_search_raw_hit_limit() -> usize {
    LOCAL_SEARCH_RAW_HIT_LIMIT
}

const fn default_local_search_visible_hit_limit() -> usize {
    LOCAL_SEARCH_VISIBLE_HIT_LIMIT
}

impl SearchedWorkspace {
    pub(crate) fn empty(workspace_id: WorkspaceId, query: String) -> Self {
        Self {
            workspace_id: workspace_id.0,
            query,
            item_count: 0,
            hit_count: 0,
            raw_candidate_count: 0,
            raw_candidate_limit: LOCAL_SEARCH_RAW_HIT_LIMIT,
            visible_hit_limit: LOCAL_SEARCH_VISIBLE_HIT_LIMIT,
            has_more_hits: false,
            hits: Vec::new(),
        }
    }

    pub(crate) fn bounded(
        workspace_id: WorkspaceId,
        query: String,
        mut hits: Vec<WorkspaceSearchHit>,
        raw_candidate_count: usize,
        has_more_raw_candidates: bool,
    ) -> Self {
        let hit_count = hits.len();
        hits.truncate(LOCAL_SEARCH_VISIBLE_HIT_LIMIT);
        Self {
            workspace_id: workspace_id.0,
            query,
            item_count: hits.len(),
            hit_count,
            raw_candidate_count,
            raw_candidate_limit: LOCAL_SEARCH_RAW_HIT_LIMIT,
            visible_hit_limit: LOCAL_SEARCH_VISIBLE_HIT_LIMIT,
            has_more_hits: has_more_raw_candidates,
            hits,
        }
    }
}
