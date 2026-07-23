use std::path::Path;

use chaft_types::{ChannelId, EventId, MessageId, WorkspaceId};
use rusqlite::{Connection, params};
use thiserror::Error;

pub const SEARCH_INDEX_PATH_MAX_BYTES: usize = 64 * 1024;
pub const MAX_SEARCH_HIT_LIMIT: usize = 512;
const DEFAULT_SEARCH_HIT_LIMIT: usize = 50;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("sqlite error")]
    Sqlite(#[from] rusqlite::Error),
    #[error("search index path is required")]
    SearchPathRequired,
    #[error("search index path is too large ({actual_bytes} bytes, max {max_bytes})")]
    SearchPathTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub event_id: EventId,
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub markdown: String,
    pub markdown_char_count: usize,
    pub markdown_truncated: bool,
}

pub struct SearchIndex {
    connection: Connection,
}

impl SearchIndex {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SearchError> {
        let path = path.as_ref();
        validate_search_index_path(path)?;
        let connection = Connection::open(path)?;
        let index = Self { connection };
        index.configure()?;
        index.migrate()?;
        Ok(index)
    }

    pub fn open_in_memory() -> Result<Self, SearchError> {
        let connection = Connection::open_in_memory()?;
        let index = Self { connection };
        index.configure()?;
        index.migrate()?;
        Ok(index)
    }

    fn configure(&self) -> Result<(), SearchError> {
        self.connection.pragma_update(None, "journal_mode", "WAL")?;
        self.connection
            .pragma_update(None, "synchronous", "NORMAL")?;
        Ok(())
    }

    fn migrate(&self) -> Result<(), SearchError> {
        if !self.messages_table_is_current()? {
            self.migrate_legacy_messages_table()?;
            return Ok(());
        }
        self.create_messages_table()
    }

    fn create_messages_table(&self) -> Result<(), SearchError> {
        self.connection.execute_batch(
            "
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                event_id UNINDEXED,
                message_id UNINDEXED,
                workspace_id UNINDEXED,
                channel_id UNINDEXED,
                physical_ms UNINDEXED,
                markdown
            );
            ",
        )?;
        Ok(())
    }

    fn migrate_legacy_messages_table(&self) -> Result<(), SearchError> {
        self.connection.execute_batch(
            "
            DROP TABLE IF EXISTS temp.legacy_messages_fts;
            CREATE TEMP TABLE legacy_messages_fts AS
                SELECT event_id, message_id, workspace_id, channel_id, markdown
                FROM messages_fts;
            DROP TABLE messages_fts;
            CREATE VIRTUAL TABLE messages_fts USING fts5(
                event_id UNINDEXED,
                message_id UNINDEXED,
                workspace_id UNINDEXED,
                channel_id UNINDEXED,
                physical_ms UNINDEXED,
                markdown
            );
            INSERT INTO messages_fts(
                event_id,
                message_id,
                workspace_id,
                channel_id,
                physical_ms,
                markdown
            )
                SELECT event_id, message_id, workspace_id, channel_id, 0, markdown
                FROM temp.legacy_messages_fts;
            DROP TABLE temp.legacy_messages_fts;
            ",
        )?;
        Ok(())
    }

    fn messages_table_is_current(&self) -> Result<bool, SearchError> {
        let mut statement = self
            .connection
            .prepare("PRAGMA table_xinfo(messages_fts)")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
        let mut saw_table = false;
        for row in rows {
            saw_table = true;
            if row? == "physical_ms" {
                return Ok(true);
            }
        }
        Ok(!saw_table)
    }

    pub fn index_message(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        event_id: &EventId,
        physical_ms: i64,
        markdown: &str,
    ) -> Result<(), SearchError> {
        self.connection.execute(
            "DELETE FROM messages_fts WHERE event_id = ?1",
            params![event_id.0],
        )?;
        self.connection.execute(
            "
            INSERT INTO messages_fts(
                event_id,
                message_id,
                workspace_id,
                channel_id,
                physical_ms,
                markdown
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                event_id.0,
                message_id.0,
                workspace_id.0,
                channel_id.0,
                physical_ms,
                markdown
            ],
        )?;
        Ok(())
    }

    pub fn clear_workspace(&self, workspace_id: &WorkspaceId) -> Result<(), SearchError> {
        self.connection.execute(
            "DELETE FROM messages_fts WHERE workspace_id = ?1",
            params![workspace_id.0],
        )?;
        Ok(())
    }

    pub fn remove_message(
        &self,
        workspace_id: &WorkspaceId,
        message_id: &MessageId,
    ) -> Result<(), SearchError> {
        self.connection.execute(
            "DELETE FROM messages_fts WHERE workspace_id = ?1 AND message_id = ?2",
            params![workspace_id.0, message_id.0],
        )?;
        Ok(())
    }

    pub fn search(
        &self,
        workspace_id: &WorkspaceId,
        query: &str,
    ) -> Result<Vec<SearchHit>, SearchError> {
        self.search_limited(workspace_id, query, DEFAULT_SEARCH_HIT_LIMIT)
    }

    pub fn search_limited(
        &self,
        workspace_id: &WorkspaceId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>, SearchError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let Some(query) = normalize_query(query) else {
            return Ok(Vec::new());
        };
        let limit = limit.min(MAX_SEARCH_HIT_LIMIT) as i64;

        let mut statement = self.connection.prepare(
            "
            SELECT
                event_id,
                message_id,
                channel_id,
                snippet(messages_fts, -1, '', '', '...', 64) AS markdown_snippet,
                length(markdown) AS markdown_char_count,
                snippet(messages_fts, -1, '', '', '...', 64) <> markdown AS markdown_truncated
            FROM messages_fts
            WHERE workspace_id = ?1 AND messages_fts MATCH ?2
            ORDER BY CAST(physical_ms AS INTEGER) DESC, event_id ASC
            LIMIT ?3
            ",
        )?;
        let rows = statement.query_map(params![workspace_id.0, query, limit], |row| {
            Ok(SearchHit {
                event_id: EventId(row.get(0)?),
                message_id: MessageId(row.get(1)?),
                channel_id: ChannelId(row.get(2)?),
                markdown: row.get(3)?,
                markdown_char_count: row.get::<_, i64>(4)?.max(0) as usize,
                markdown_truncated: row.get::<_, bool>(5)?,
            })
        })?;

        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }
}

fn validate_search_index_path(path: &Path) -> Result<(), SearchError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(SearchError::SearchPathRequired);
    }
    if actual_bytes > SEARCH_INDEX_PATH_MAX_BYTES {
        return Err(SearchError::SearchPathTooLarge {
            actual_bytes,
            max_bytes: SEARCH_INDEX_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

pub fn query_has_search_terms(query: &str) -> bool {
    normalize_query(query).is_some()
}

fn normalize_query(query: &str) -> Option<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    for character in query.chars() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
        } else if !current.is_empty() {
            terms.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        terms.push(current);
    }

    if terms.is_empty() {
        return None;
    }

    for term in &mut terms {
        term.push('*');
    }
    Some(terms.join(" "))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn assert_search_path_too_large<T>(result: Result<T, SearchError>) {
        match result {
            Err(SearchError::SearchPathTooLarge {
                actual_bytes,
                max_bytes,
            }) if actual_bytes > SEARCH_INDEX_PATH_MAX_BYTES
                && max_bytes == SEARCH_INDEX_PATH_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized search index path error, got ok"),
            Err(error) => panic!("expected oversized search index path error, got {error}"),
        }
    }

    #[test]
    fn search_index_rejects_blank_path_before_sqlite_open() {
        assert!(matches!(
            SearchIndex::open(PathBuf::new()),
            Err(SearchError::SearchPathRequired)
        ));
    }

    #[test]
    fn search_index_rejects_oversized_path_before_sqlite_open() {
        assert_search_path_too_large(SearchIndex::open(PathBuf::from(
            "s".repeat(SEARCH_INDEX_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn indexes_and_searches_messages() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let event_id = EventId("evt_message".to_owned());

        index
            .index_message(
                &workspace_id,
                &channel_id,
                &message_id,
                &event_id,
                1_000,
                "local first peer to peer chat",
            )
            .unwrap();

        let hits = index.search(&workspace_id, "peer").unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].event_id, event_id);
        assert_eq!(hits[0].message_id, message_id);
    }

    #[test]
    fn search_index_persists_to_disk_and_sanitizes_query() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("search.db");
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let event_id = EventId("evt_search".to_owned());

        let index = SearchIndex::open(&path).unwrap();
        index
            .index_message(
                &workspace_id,
                &channel_id,
                &message_id,
                &event_id,
                1_000,
                "fast local workspace search",
            )
            .unwrap();
        drop(index);

        let reopened = SearchIndex::open(&path).unwrap();
        let hits = reopened.search(&workspace_id, "\"local\"!").unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].event_id, event_id);
        assert!(reopened.search(&workspace_id, "!!!").unwrap().is_empty());
    }

    #[test]
    fn query_term_detection_matches_search_normalization() {
        assert!(!query_has_search_terms(""));
        assert!(!query_has_search_terms(" \t !!! --- "));
        assert!(query_has_search_terms("peer"));
        assert!(query_has_search_terms("\"peer\" sync"));
        assert!(query_has_search_terms("こんにちは"));
    }

    #[test]
    fn open_migrates_legacy_search_table_without_physical_ms() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("search.db");
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();

        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "
                    CREATE VIRTUAL TABLE messages_fts USING fts5(
                        event_id UNINDEXED,
                        message_id UNINDEXED,
                        workspace_id UNINDEXED,
                        channel_id UNINDEXED,
                        markdown
                    );
                    INSERT INTO messages_fts(
                        event_id,
                        message_id,
                        workspace_id,
                        channel_id,
                        markdown
                    )
                    VALUES (
                        'evt_legacy',
                        'msg_legacy',
                        'wrk_legacy',
                        'chn_legacy',
                        'legacy needle'
                    );
                    ",
                )
                .unwrap();
        }

        let index = SearchIndex::open(&path).unwrap();
        let legacy_hits = index
            .search(&WorkspaceId("wrk_legacy".to_owned()), "legacy")
            .unwrap();
        assert_eq!(legacy_hits.len(), 1);
        assert_eq!(legacy_hits[0].event_id, EventId("evt_legacy".to_owned()));

        index
            .index_message(
                &workspace_id,
                &channel_id,
                &MessageId("msg_new".to_owned()),
                &EventId("evt_new".to_owned()),
                10,
                "new needle",
            )
            .unwrap();

        let hits = index.search(&workspace_id, "new").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].event_id, EventId("evt_new".to_owned()));
    }

    #[test]
    fn search_limit_is_configurable() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();

        for index_value in 0..3 {
            index
                .index_message(
                    &workspace_id,
                    &channel_id,
                    &MessageId(format!("msg_{index_value}")),
                    &EventId(format!("evt_{index_value}")),
                    i64::from(index_value),
                    "needle bounded search",
                )
                .unwrap();
        }

        let hits = index.search_limited(&workspace_id, "needle", 2).unwrap();

        assert_eq!(hits.len(), 2);
        assert!(
            index
                .search_limited(&workspace_id, "needle", 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn search_limited_clamps_oversized_limits_to_newest_rows() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();

        for index_value in 0..(MAX_SEARCH_HIT_LIMIT + 5) {
            index
                .index_message(
                    &workspace_id,
                    &channel_id,
                    &MessageId(format!("msg_{index_value:04}")),
                    &EventId(format!("evt_{index_value:04}")),
                    index_value as i64,
                    "needle capped result",
                )
                .unwrap();
        }

        let hits = index
            .search_limited(&workspace_id, "needle", usize::MAX)
            .unwrap();

        assert_eq!(hits.len(), MAX_SEARCH_HIT_LIMIT);
        assert_eq!(
            hits[0].event_id,
            EventId(format!("evt_{:04}", MAX_SEARCH_HIT_LIMIT + 4))
        );
        assert_eq!(
            hits.last().unwrap().event_id,
            EventId("evt_0005".to_owned())
        );
    }

    #[test]
    fn search_supports_prefix_terms_for_incremental_ui() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();

        index
            .index_message(
                &workspace_id,
                &channel_id,
                &MessageId("msg_peer".to_owned()),
                &EventId("evt_peer".to_owned()),
                2_000,
                "peer sync feels instant",
            )
            .unwrap();
        index
            .index_message(
                &workspace_id,
                &channel_id,
                &MessageId("msg_other".to_owned()),
                &EventId("evt_other".to_owned()),
                1_000,
                "local draft cache",
            )
            .unwrap();

        let hits = index.search_limited(&workspace_id, "pee ins", 10).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].event_id, EventId("evt_peer".to_owned()));
    }

    #[test]
    fn search_returns_bounded_snippet_metadata_for_long_matches() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let long_prefix = (0..120)
            .map(|index| format!("prefix{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let long_suffix = (0..120)
            .map(|index| format!("suffix{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let markdown = format!("{long_prefix} needle-search-context {long_suffix}");

        index
            .index_message(
                &workspace_id,
                &channel_id,
                &MessageId("msg_long".to_owned()),
                &EventId("evt_long".to_owned()),
                1_000,
                &markdown,
            )
            .unwrap();

        let hits = index.search_limited(&workspace_id, "needle", 10).unwrap();

        assert_eq!(hits.len(), 1);
        assert!(hits[0].markdown.contains("needle-search-context"));
        assert!(hits[0].markdown.len() < markdown.len());
        assert_eq!(hits[0].markdown_char_count, markdown.chars().count());
        assert!(hits[0].markdown_truncated);
    }

    #[test]
    fn search_limited_returns_newest_indexed_hits_first() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();

        for (event_id, physical_ms) in [("evt_oldest", 2), ("evt_newest", 10), ("evt_middle", 9)] {
            index
                .index_message(
                    &workspace_id,
                    &channel_id,
                    &MessageId(format!("msg_{event_id}")),
                    &EventId(event_id.to_owned()),
                    physical_ms,
                    "needle ordered result",
                )
                .unwrap();
        }

        let hits = index.search_limited(&workspace_id, "needle", 2).unwrap();

        assert_eq!(
            hits.iter()
                .map(|hit| hit.event_id.0.as_str())
                .collect::<Vec<_>>(),
            vec!["evt_newest", "evt_middle"]
        );
    }

    #[test]
    fn removes_one_message_from_workspace_index() {
        let index = SearchIndex::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let other_workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId("msg_remove".to_owned());
        let other_message_id = MessageId("msg_keep".to_owned());

        index
            .index_message(
                &workspace_id,
                &channel_id,
                &message_id,
                &EventId("evt_remove".to_owned()),
                1_000,
                "needle removed",
            )
            .unwrap();
        index
            .index_message(
                &workspace_id,
                &channel_id,
                &other_message_id,
                &EventId("evt_keep".to_owned()),
                2_000,
                "needle kept",
            )
            .unwrap();
        index
            .index_message(
                &other_workspace_id,
                &channel_id,
                &message_id,
                &EventId("evt_other".to_owned()),
                3_000,
                "needle other workspace",
            )
            .unwrap();

        index.remove_message(&workspace_id, &message_id).unwrap();

        let workspace_hits = index.search(&workspace_id, "needle").unwrap();
        let other_hits = index.search(&other_workspace_id, "needle").unwrap();
        assert_eq!(workspace_hits.len(), 1);
        assert_eq!(workspace_hits[0].message_id, other_message_id);
        assert_eq!(other_hits.len(), 1);
        assert_eq!(other_hits[0].message_id, message_id);
    }
}
