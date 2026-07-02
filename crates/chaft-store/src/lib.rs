use std::{collections::BTreeMap, path::Path};

use chaft_identity::verify_self_contained_event;
use chaft_types::{EventId, SignedEvent};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error")]
    Sqlite(#[from] rusqlite::Error),
    #[error("event serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("event JSON is too large ({actual_bytes} bytes, max {max_bytes})")]
    EventJsonTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("event store path is required")]
    StorePathRequired,
    #[error("event store path is too large ({actual_bytes} bytes, max {max_bytes})")]
    StorePathTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

pub const EVENT_JSON_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_EVENT_STORE_PAGE_ROWS: usize = 1024;
pub const MAX_EVENT_STORE_CANDIDATE_FILTER_ROWS: usize = 1024;
pub const EVENT_STORE_PATH_MAX_BYTES: usize = 64 * 1024;
const EVENT_JSON_MAX_BYTES_SQL: i64 = EVENT_JSON_MAX_BYTES as i64;

pub struct EventStore {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEventStorageHealth {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEventStorageRepair {
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

impl EventStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        validate_event_store_path(path)?;
        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        let store = Self { connection };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    fn configure(&self) -> Result<(), StoreError> {
        self.connection.pragma_update(None, "journal_mode", "WAL")?;
        self.connection.pragma_update(None, "foreign_keys", "ON")?;
        self.connection
            .pragma_update(None, "synchronous", "NORMAL")?;
        Ok(())
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS events (
                event_id TEXT PRIMARY KEY NOT NULL,
                workspace_id TEXT NOT NULL,
                channel_id TEXT,
                author_device_id TEXT NOT NULL,
                physical_ms INTEGER NOT NULL,
                logical INTEGER NOT NULL,
                self_contained_signature_valid INTEGER,
                event_json BLOB NOT NULL
            );
            ",
        )?;

        self.ensure_self_contained_signature_valid_column()?;
        self.backfill_self_contained_signature_valid()?;

        self.connection.execute_batch(
            "

            CREATE INDEX IF NOT EXISTS idx_events_workspace_time
            ON events(workspace_id, physical_ms, logical);

            CREATE INDEX IF NOT EXISTS idx_events_channel_time
            ON events(channel_id, physical_ms, logical);

            CREATE INDEX IF NOT EXISTS idx_events_workspace_servable
            ON events(workspace_id, self_contained_signature_valid);
            ",
        )?;
        Ok(())
    }

    fn ensure_self_contained_signature_valid_column(&self) -> Result<(), StoreError> {
        let mut statement = self.connection.prepare("PRAGMA table_info(events)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for column in columns {
            if column? == "self_contained_signature_valid" {
                return Ok(());
            }
        }

        self.connection.execute_batch(
            "
            ALTER TABLE events
            ADD COLUMN self_contained_signature_valid INTEGER;
            ",
        )?;
        Ok(())
    }

    fn backfill_self_contained_signature_valid(&self) -> Result<(), StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                event_id,
                length(event_json),
                CASE
                    WHEN length(event_json) <= ?1 THEN event_json
                    ELSE NULL
                END
            FROM events
            WHERE self_contained_signature_valid IS NULL
            ",
        )?;
        let rows = statement.query_map(params![EVENT_JSON_MAX_BYTES_SQL], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            pending.push(row?);
        }
        drop(statement);

        for (event_id, actual_bytes, bytes) in pending {
            let is_servable = bounded_event_json_or_none(actual_bytes, bytes)
                .and_then(|bytes| serde_json::from_slice::<SignedEvent>(&bytes).ok())
                .is_some_and(|event| is_servable_event(&event));
            self.connection.execute(
                "
                UPDATE events
                SET self_contained_signature_valid = ?2
                WHERE event_id = ?1
                ",
                params![event_id, bool_to_sqlite_integer(is_servable)],
            )?;
        }

        Ok(())
    }

    pub fn append_event(&self, event: &SignedEvent) -> Result<(), StoreError> {
        let event_json = serde_json::to_vec(event)?;
        validate_event_json_size(event_json.len())?;
        let is_servable = is_servable_event(event);
        self.connection.execute(
            "
            INSERT INTO events (
                event_id,
                workspace_id,
                channel_id,
                author_device_id,
                physical_ms,
                logical,
                self_contained_signature_valid,
                event_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(event_id) DO UPDATE SET
                workspace_id = excluded.workspace_id,
                channel_id = excluded.channel_id,
                author_device_id = excluded.author_device_id,
                physical_ms = excluded.physical_ms,
                logical = excluded.logical,
                self_contained_signature_valid = excluded.self_contained_signature_valid,
                event_json = excluded.event_json
            WHERE COALESCE(events.self_contained_signature_valid, 0) != 1
              AND excluded.self_contained_signature_valid = 1
            ",
            params![
                event.event_id.0,
                event.event.workspace_id.0,
                event.event.channel_id.as_ref().map(|id| id.0.as_str()),
                event.event.author_device_id.0,
                event.event.timestamp.physical_ms,
                event.event.timestamp.logical,
                bool_to_sqlite_integer(is_servable),
                event_json
            ],
        )?;
        Ok(())
    }

    pub fn get_event(&self, event_id: &EventId) -> Result<Option<SignedEvent>, StoreError> {
        let row: Option<(i64, Option<Vec<u8>>)> = self
            .connection
            .query_row(
                "
                SELECT
                    length(event_json),
                    CASE
                        WHEN length(event_json) <= ?2 THEN event_json
                        ELSE NULL
                    END
                FROM events
                WHERE event_id = ?1
                ",
                params![event_id.0, EVENT_JSON_MAX_BYTES_SQL],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        row.map(|(actual_bytes, bytes)| {
            let bytes = bounded_event_json_or_error(actual_bytes, bytes)?;
            serde_json::from_slice(&bytes).map_err(StoreError::from)
        })
        .transpose()
    }

    pub fn get_servable_event(
        &self,
        event_id: &EventId,
    ) -> Result<Option<SignedEvent>, StoreError> {
        let bytes: Option<Vec<u8>> = self
            .connection
            .query_row(
                "
                SELECT event_json FROM events
                WHERE event_id = ?1
                  AND self_contained_signature_valid = 1
                  AND length(event_json) <= ?2
                ",
                params![event_id.0, EVENT_JSON_MAX_BYTES_SQL],
                |row| row.get(0),
            )
            .optional()?;

        Ok(bytes.and_then(|bytes| serde_json::from_slice(&bytes).ok()))
    }

    pub fn list_events(&self) -> Result<Vec<SignedEvent>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                length(event_json),
                CASE
                    WHEN length(event_json) <= ?1 THEN event_json
                    ELSE NULL
                END
            FROM events
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![EVENT_JSON_MAX_BYTES_SQL], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
        })?;
        let mut events = Vec::new();

        for row in rows {
            let (actual_bytes, bytes) = row?;
            let bytes = bounded_event_json_or_error(actual_bytes, bytes)?;
            events.push(serde_json::from_slice(&bytes)?);
        }

        Ok(events)
    }

    pub fn list_event_ids(&self) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn list_workspace_ids(&self) -> Result<Vec<String>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT workspace_id FROM events
            GROUP BY workspace_id
            ORDER BY MIN(rowid) ASC
            ",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let mut workspace_ids = Vec::new();

        for row in rows {
            workspace_ids.push(row?);
        }

        Ok(workspace_ids)
    }

    pub fn count_workspaces(&self) -> Result<usize, StoreError> {
        let count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*) FROM (
                SELECT workspace_id FROM events
                GROUP BY workspace_id
            )
            ",
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn list_workspace_ids_page(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<String>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT workspace_id FROM events
            GROUP BY workspace_id
            ORDER BY MIN(rowid) ASC
            LIMIT ?1 OFFSET ?2
            ",
        )?;
        let rows = statement.query_map(
            params![
                sqlite_page_limit_value(limit),
                sqlite_page_value(start_index)
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut workspace_ids = Vec::new();

        for row in rows {
            workspace_ids.push(row?);
        }

        Ok(workspace_ids)
    }

    pub fn list_events_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SignedEvent>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                length(event_json),
                CASE
                    WHEN length(event_json) <= ?2 THEN event_json
                    ELSE NULL
                END
            FROM events
            WHERE workspace_id = ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
        })?;
        let mut events = Vec::new();

        for row in rows {
            let (actual_bytes, bytes) = row?;
            let bytes = bounded_event_json_or_error(actual_bytes, bytes)?;
            events.push(serde_json::from_slice(&bytes)?);
        }

        Ok(events)
    }

    pub fn count_events_for_workspace(&self, workspace_id: &str) -> Result<usize, StoreError> {
        let count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*) FROM events
            WHERE workspace_id = ?1
            ",
            params![workspace_id],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn workspace_event_storage_health(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceEventStorageHealth, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                length(event_json),
                CASE
                    WHEN length(event_json) <= ?2 THEN event_json
                    ELSE NULL
                END,
                COALESCE(self_contained_signature_valid, 0)
            FROM events
            WHERE workspace_id = ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        let mut total_event_count = 0usize;
        let mut parseable_event_count = 0usize;
        let mut corrupt_event_count = 0usize;
        let mut signature_valid_metadata_count = 0usize;
        let mut servable_event_count = 0usize;
        let mut poisoned_servable_metadata_count = 0usize;
        let mut promotable_servable_metadata_count = 0usize;
        let mut non_servable_parseable_event_count = 0usize;

        for row in rows {
            let (actual_bytes, bytes, signature_valid_metadata) = row?;
            let has_signature_valid_metadata = signature_valid_metadata == 1;
            total_event_count += 1;
            if has_signature_valid_metadata {
                signature_valid_metadata_count += 1;
            }

            let Some(bytes) = bounded_event_json_or_none(actual_bytes, bytes) else {
                corrupt_event_count += 1;
                if has_signature_valid_metadata {
                    poisoned_servable_metadata_count += 1;
                }
                continue;
            };

            match serde_json::from_slice::<SignedEvent>(&bytes) {
                Ok(event) => {
                    parseable_event_count += 1;
                    let is_servable = is_servable_event(&event);
                    if is_servable {
                        servable_event_count += 1;
                    } else {
                        non_servable_parseable_event_count += 1;
                    }
                    if has_signature_valid_metadata && !is_servable {
                        poisoned_servable_metadata_count += 1;
                    } else if !has_signature_valid_metadata && is_servable {
                        promotable_servable_metadata_count += 1;
                    }
                }
                Err(_) => {
                    corrupt_event_count += 1;
                    if has_signature_valid_metadata {
                        poisoned_servable_metadata_count += 1;
                    }
                }
            }
        }

        Ok(WorkspaceEventStorageHealth {
            workspace_id: workspace_id.to_owned(),
            total_event_count,
            parseable_event_count,
            corrupt_event_count,
            signature_valid_metadata_count,
            servable_event_count,
            poisoned_servable_metadata_count,
            promotable_servable_metadata_count,
            non_servable_parseable_event_count,
        })
    }

    pub fn repair_workspace_event_storage_metadata(
        &self,
        workspace_id: &str,
    ) -> Result<WorkspaceEventStorageRepair, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
                event_id,
                length(event_json),
                CASE
                    WHEN length(event_json) <= ?2 THEN event_json
                    ELSE NULL
                END,
                COALESCE(self_contained_signature_valid, 0)
            FROM events
            WHERE workspace_id = ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        let mut total_event_count = 0usize;
        let mut parseable_event_count = 0usize;
        let mut corrupt_event_count = 0usize;
        let mut signature_valid_metadata_before_count = 0usize;
        let mut signature_valid_metadata_after_count = 0usize;
        let mut promoted_servable_metadata_count = 0usize;
        let mut cleared_unservable_metadata_count = 0usize;
        let mut updates = Vec::new();

        for row in rows {
            let (event_id, actual_bytes, bytes, signature_valid_metadata) = row?;
            let current = signature_valid_metadata == 1;
            total_event_count += 1;
            if current {
                signature_valid_metadata_before_count += 1;
            }

            let repaired = if let Some(bytes) = bounded_event_json_or_none(actual_bytes, bytes) {
                match serde_json::from_slice::<SignedEvent>(&bytes) {
                    Ok(event) => {
                        parseable_event_count += 1;
                        is_servable_event(&event)
                    }
                    Err(_) => {
                        corrupt_event_count += 1;
                        false
                    }
                }
            } else {
                corrupt_event_count += 1;
                false
            };

            if repaired {
                signature_valid_metadata_after_count += 1;
            }
            if current != repaired {
                if repaired {
                    promoted_servable_metadata_count += 1;
                } else {
                    cleared_unservable_metadata_count += 1;
                }
                updates.push((event_id, repaired));
            }
        }
        drop(statement);

        for (event_id, repaired) in &updates {
            self.connection.execute(
                "
                UPDATE events
                SET self_contained_signature_valid = ?2
                WHERE event_id = ?1
                ",
                params![event_id, bool_to_sqlite_integer(*repaired)],
            )?;
        }

        Ok(WorkspaceEventStorageRepair {
            workspace_id: workspace_id.to_owned(),
            total_event_count,
            parseable_event_count,
            corrupt_event_count,
            signature_valid_metadata_before_count,
            signature_valid_metadata_after_count,
            repaired_metadata_count: updates.len(),
            promoted_servable_metadata_count,
            cleared_unservable_metadata_count,
        })
    }

    pub fn list_parseable_events_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SignedEvent>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_json FROM events
            WHERE workspace_id = ?1
              AND length(event_json) <= ?2
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        let mut events = Vec::new();

        for row in rows {
            let bytes = row?;
            if let Ok(event) = serde_json::from_slice(&bytes) {
                events.push(event);
            }
        }

        Ok(events)
    }

    pub fn list_event_ids_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            WHERE workspace_id = ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id], |row| row.get::<_, String>(0))?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn list_servable_event_ids(&self) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            WHERE self_contained_signature_valid = 1
              AND length(event_json) <= ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![EVENT_JSON_MAX_BYTES_SQL], |row| {
            row.get::<_, String>(0)
        })?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn count_servable_events(&self) -> Result<usize, StoreError> {
        let count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*) FROM events
            WHERE self_contained_signature_valid = 1
              AND length(event_json) <= ?1
            ",
            params![EVENT_JSON_MAX_BYTES_SQL],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn list_servable_event_ids_page(
        &self,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            WHERE self_contained_signature_valid = 1
              AND length(event_json) <= ?1
            ORDER BY rowid ASC
            LIMIT ?2 OFFSET ?3
            ",
        )?;
        let rows = statement.query_map(
            params![
                EVENT_JSON_MAX_BYTES_SQL,
                sqlite_page_limit_value(limit),
                sqlite_page_value(start_index)
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn count_servable_events_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<usize, StoreError> {
        let count: i64 = self.connection.query_row(
            "
            SELECT COUNT(*) FROM events
            WHERE workspace_id = ?1
              AND self_contained_signature_valid = 1
              AND length(event_json) <= ?2
            ",
            params![workspace_id, EVENT_JSON_MAX_BYTES_SQL],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    pub fn list_servable_event_ids_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            WHERE workspace_id = ?1
              AND self_contained_signature_valid = 1
              AND length(event_json) <= ?2
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            row.get::<_, String>(0)
        })?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn filter_servable_event_ids_for_workspace(
        &self,
        workspace_id: &str,
        candidate_ids: &[EventId],
    ) -> Result<Vec<EventId>, StoreError> {
        if candidate_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut event_ids_by_rowid = BTreeMap::new();
        for candidate_chunk in candidate_ids.chunks(MAX_EVENT_STORE_CANDIDATE_FILTER_ROWS) {
            let placeholders = std::iter::repeat_n("?", candidate_chunk.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "
                SELECT rowid, event_id FROM events
                WHERE workspace_id = ?
                  AND self_contained_signature_valid = 1
                  AND length(event_json) <= ?
                  AND event_id IN ({placeholders})
                ORDER BY rowid ASC
                "
            );
            let mut query_params = Vec::with_capacity(candidate_chunk.len() + 2);
            query_params.push(Value::Text(workspace_id.to_owned()));
            query_params.push(Value::Integer(EVENT_JSON_MAX_BYTES_SQL));
            query_params.extend(
                candidate_chunk
                    .iter()
                    .map(|event_id| Value::Text(event_id.0.clone())),
            );

            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(query_params), |row| {
                Ok((row.get::<_, i64>(0)?, EventId(row.get::<_, String>(1)?)))
            })?;

            for row in rows {
                let (rowid, event_id) = row?;
                event_ids_by_rowid.insert(rowid, event_id);
            }
        }

        Ok(event_ids_by_rowid.into_values().collect())
    }

    pub fn list_servable_event_ids_for_workspace_page(
        &self,
        workspace_id: &str,
        start_index: usize,
        limit: usize,
    ) -> Result<Vec<EventId>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_id FROM events
            WHERE workspace_id = ?1
              AND self_contained_signature_valid = 1
              AND length(event_json) <= ?2
            ORDER BY rowid ASC
            LIMIT ?3 OFFSET ?4
            ",
        )?;
        let rows = statement.query_map(
            params![
                workspace_id,
                EVENT_JSON_MAX_BYTES_SQL,
                sqlite_page_limit_value(limit),
                sqlite_page_value(start_index)
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut event_ids = Vec::new();

        for row in rows {
            event_ids.push(EventId(row?));
        }

        Ok(event_ids)
    }

    pub fn list_servable_events_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<SignedEvent>, StoreError> {
        let mut statement = self.connection.prepare(
            "
            SELECT event_json FROM events
            WHERE workspace_id = ?1
              AND self_contained_signature_valid = 1
              AND length(event_json) <= ?2
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![workspace_id, EVENT_JSON_MAX_BYTES_SQL], |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        let mut events = Vec::new();

        for row in rows {
            let bytes = row?;
            if let Ok(event) = serde_json::from_slice(&bytes) {
                events.push(event);
            }
        }

        Ok(events)
    }
}

fn is_servable_event(event: &SignedEvent) -> bool {
    verify_self_contained_event(event).is_ok()
}

fn bool_to_sqlite_integer(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn sqlite_page_value(value: usize) -> i64 {
    value.min(i64::MAX as usize) as i64
}

fn sqlite_page_limit_value(value: usize) -> i64 {
    sqlite_page_value(value.min(MAX_EVENT_STORE_PAGE_ROWS))
}

pub fn validate_signed_event_json_size(event: &SignedEvent) -> Result<(), StoreError> {
    let event_json = serde_json::to_vec(event)?;
    validate_event_json_size(event_json.len())
}

fn sqlite_blob_len_to_usize(actual_bytes: i64) -> usize {
    usize::try_from(actual_bytes.max(0)).unwrap_or(usize::MAX)
}

fn validate_event_json_size(actual_bytes: usize) -> Result<(), StoreError> {
    if actual_bytes > EVENT_JSON_MAX_BYTES {
        return Err(StoreError::EventJsonTooLarge {
            actual_bytes,
            max_bytes: EVENT_JSON_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_event_store_path(path: &Path) -> Result<(), StoreError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(StoreError::StorePathRequired);
    }
    if actual_bytes > EVENT_STORE_PATH_MAX_BYTES {
        return Err(StoreError::StorePathTooLarge {
            actual_bytes,
            max_bytes: EVENT_STORE_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

fn bounded_event_json_or_error(
    actual_bytes: i64,
    bytes: Option<Vec<u8>>,
) -> Result<Vec<u8>, StoreError> {
    let actual_bytes = sqlite_blob_len_to_usize(actual_bytes);
    validate_event_json_size(actual_bytes)?;
    let bytes = bytes.unwrap_or_default();
    validate_event_json_size(bytes.len())?;
    Ok(bytes)
}

fn bounded_event_json_or_none(actual_bytes: i64, bytes: Option<Vec<u8>>) -> Option<Vec<u8>> {
    if sqlite_blob_len_to_usize(actual_bytes) > EVENT_JSON_MAX_BYTES {
        return None;
    }
    bytes.filter(|bytes| bytes.len() <= EVENT_JSON_MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chaft_identity::DeviceIdentity;
    use chaft_types::{
        ChannelId, DeviceId, EventBody, MessageId, SignableEvent, SignedEvent, WorkspaceId,
    };

    use super::*;

    fn assert_store_path_too_large<T>(result: Result<T, StoreError>) {
        match result {
            Err(StoreError::StorePathTooLarge {
                actual_bytes,
                max_bytes,
            }) if actual_bytes > EVENT_STORE_PATH_MAX_BYTES
                && max_bytes == EVENT_STORE_PATH_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized event store path error, got ok"),
            Err(error) => panic!("expected oversized event store path error, got {error}"),
        }
    }

    #[test]
    fn event_store_rejects_blank_path_before_sqlite_open() {
        assert!(matches!(
            EventStore::open(PathBuf::new()),
            Err(StoreError::StorePathRequired)
        ));
    }

    #[test]
    fn event_store_rejects_oversized_path_before_sqlite_open() {
        assert_store_path_too_large(EventStore::open(PathBuf::from(
            "e".repeat(EVENT_STORE_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn stores_and_reads_event() {
        let store = EventStore::open_in_memory().unwrap();
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            DeviceId("dev_test".to_owned()),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "stored".to_owned(),
                attachments: Vec::new(),
            },
        );
        let signed = SignedEvent::from_signed_bytes(event, vec![4, 5, 6]);

        store.append_event(&signed).unwrap();

        assert_eq!(store.get_event(&signed.event_id).unwrap(), Some(signed));
        assert_eq!(store.list_events().unwrap().len(), 1);
    }

    #[test]
    fn lists_events_in_insertion_order() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let first = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                workspace_id.clone(),
                None,
                DeviceId("dev_test".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "Chaft".to_owned(),
                },
            ),
            vec![1],
        );
        let second = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                workspace_id,
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "second".to_owned(),
                    attachments: Vec::new(),
                },
            ),
            vec![2],
        );

        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();

        let listed = store.list_events().unwrap();

        assert_eq!(listed[0].event_id, first.event_id);
        assert_eq!(listed[1].event_id, second.event_id);
        assert_eq!(
            store.list_event_ids().unwrap(),
            vec![first.event_id, second.event_id]
        );
    }

    #[test]
    fn lists_event_ids_without_deserializing_event_json() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 1, 0, ?4)
                ",
                params![
                    "evt_corrupt",
                    workspace_id.0,
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        assert_eq!(
            store.list_event_ids().unwrap(),
            vec![EventId("evt_corrupt".to_owned())]
        );
        assert_eq!(
            store.list_event_ids_for_workspace(&workspace_id.0).unwrap(),
            vec![EventId("evt_corrupt".to_owned())]
        );
        assert_eq!(
            store.count_events_for_workspace(&workspace_id.0).unwrap(),
            1
        );
        assert!(store.list_servable_event_ids().unwrap().is_empty());
        assert!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .get_servable_event(&EventId("evt_corrupt".to_owned()))
                .unwrap(),
            None
        );
        assert!(store.list_events().is_err());
    }

    #[test]
    fn oversized_event_json_rows_are_bounded_and_repairable() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let event_id = EventId("evt_oversized".to_owned());
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 1, 0, 1, zeroblob(?4))
                ",
                params![
                    event_id.0.as_str(),
                    workspace_id.0.as_str(),
                    "dev_test",
                    EVENT_JSON_MAX_BYTES_SQL + 1
                ],
            )
            .unwrap();

        assert_eq!(
            store.list_event_ids_for_workspace(&workspace_id.0).unwrap(),
            vec![event_id.clone()]
        );
        assert!(matches!(
            store.get_event(&event_id),
            Err(StoreError::EventJsonTooLarge {
                actual_bytes,
                max_bytes: EVENT_JSON_MAX_BYTES,
            }) if actual_bytes == EVENT_JSON_MAX_BYTES + 1
        ));
        assert!(matches!(
            store.list_events_for_workspace(&workspace_id.0),
            Err(StoreError::EventJsonTooLarge {
                actual_bytes,
                max_bytes: EVENT_JSON_MAX_BYTES,
            }) if actual_bytes == EVENT_JSON_MAX_BYTES + 1
        ));
        assert_eq!(store.get_servable_event(&event_id).unwrap(), None);
        assert!(
            store
                .list_parseable_events_for_workspace(&workspace_id.0)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            store
                .count_servable_events_for_workspace(&workspace_id.0)
                .unwrap(),
            0
        );

        let health = store
            .workspace_event_storage_health(&workspace_id.0)
            .unwrap();
        assert_eq!(health.total_event_count, 1);
        assert_eq!(health.parseable_event_count, 0);
        assert_eq!(health.corrupt_event_count, 1);
        assert_eq!(health.signature_valid_metadata_count, 1);
        assert_eq!(health.poisoned_servable_metadata_count, 1);

        let repaired = store
            .repair_workspace_event_storage_metadata(&workspace_id.0)
            .unwrap();
        assert_eq!(repaired.total_event_count, 1);
        assert_eq!(repaired.parseable_event_count, 0);
        assert_eq!(repaired.corrupt_event_count, 1);
        assert_eq!(repaired.signature_valid_metadata_before_count, 1);
        assert_eq!(repaired.signature_valid_metadata_after_count, 0);
        assert_eq!(repaired.repaired_metadata_count, 1);
        assert_eq!(repaired.cleared_unservable_metadata_count, 1);
        assert_eq!(
            store.list_event_ids_for_workspace(&workspace_id.0).unwrap(),
            vec![event_id]
        );
    }

    #[test]
    fn append_rejects_oversized_event_json() {
        let store = EventStore::open_in_memory().unwrap();
        let signed = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                WorkspaceId::new(),
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "x".repeat(EVENT_JSON_MAX_BYTES),
                    attachments: Vec::new(),
                },
            ),
            vec![1, 2, 3],
        );

        assert!(matches!(
            store.append_event(&signed),
            Err(StoreError::EventJsonTooLarge {
                actual_bytes,
                max_bytes: EVENT_JSON_MAX_BYTES,
            }) if actual_bytes > EVENT_JSON_MAX_BYTES
        ));
        assert!(store.list_event_ids().unwrap().is_empty());
    }

    #[test]
    fn lists_events_for_one_workspace_in_insertion_order() {
        let store = EventStore::open_in_memory().unwrap();
        let first_workspace_id = WorkspaceId::new();
        let second_workspace_id = WorkspaceId::new();
        let first = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                first_workspace_id.clone(),
                None,
                DeviceId("dev_test".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "First".to_owned(),
                },
            ),
            vec![1],
        );
        let second = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                second_workspace_id,
                None,
                DeviceId("dev_test".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "Second".to_owned(),
                },
            ),
            vec![2],
        );
        let third = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                first_workspace_id.clone(),
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "first workspace message".to_owned(),
                    attachments: Vec::new(),
                },
            ),
            vec![3],
        );

        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();
        store.append_event(&third).unwrap();

        let listed = store
            .list_events_for_workspace(&first_workspace_id.0)
            .unwrap();

        assert_eq!(
            listed
                .iter()
                .map(|event| &event.event_id)
                .collect::<Vec<_>>(),
            vec![&first.event_id, &third.event_id]
        );
        assert_eq!(
            store
                .list_event_ids_for_workspace(&first_workspace_id.0)
                .unwrap(),
            vec![first.event_id, third.event_id]
        );
    }

    #[test]
    fn servable_event_ids_exclude_invalid_self_contained_signatures() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Valid".to_owned(),
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Mallory".to_owned(),
            },
        ));
        forged.signature[0] ^= 0x01;

        store.append_event(&valid).unwrap();
        store.append_event(&forged).unwrap();

        assert_eq!(
            store.list_event_ids_for_workspace(&workspace_id.0).unwrap(),
            vec![valid.event_id.clone(), forged.event_id]
        );
        assert_eq!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.event_id]
        );
    }

    #[test]
    fn pages_servable_event_ids_and_counts_without_deserializing_event_json() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let first_workspace_id = WorkspaceId::new();
        let second_workspace_id = WorkspaceId::new();
        let first = identity.sign_event(SignableEvent::new(
            first_workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "First".to_owned(),
            },
        ));
        let second = identity.sign_event(SignableEvent::new(
            first_workspace_id.clone(),
            Some(ChannelId::new()),
            identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "hello".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let third = identity.sign_event(SignableEvent::new(
            second_workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Second".to_owned(),
            },
        ));

        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();
        store.append_event(&third).unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 4, 0, 0, ?4)
                ",
                params![
                    "evt_corrupt_same_workspace",
                    first_workspace_id.0.clone(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        assert_eq!(store.count_servable_events().unwrap(), 3);
        assert_eq!(
            store.list_servable_event_ids_page(1, 2).unwrap(),
            vec![second.event_id.clone(), third.event_id]
        );
        assert_eq!(
            store
                .count_servable_events_for_workspace(&first_workspace_id.0)
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .list_servable_event_ids_for_workspace_page(&first_workspace_id.0, 1, 1)
                .unwrap(),
            vec![second.event_id]
        );
        assert!(
            store
                .list_events_for_workspace(&first_workspace_id.0)
                .is_err()
        );
    }

    #[test]
    fn filters_servable_workspace_candidates_without_deserializing_event_json() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let other_workspace_id = WorkspaceId::new();
        let first = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "First".to_owned(),
            },
        ));
        let second = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            Some(ChannelId::new()),
            identity.device_id().clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "hello".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let other = identity.sign_event(SignableEvent::new(
            other_workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Other".to_owned(),
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Forged".to_owned(),
            },
        ));
        forged.signature[0] ^= 0x01;

        store.append_event(&first).unwrap();
        store.append_event(&other).unwrap();
        store.append_event(&forged).unwrap();
        store.append_event(&second).unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 5, 0, 0, ?4)
                ",
                params![
                    "evt_corrupt_candidate",
                    workspace_id.0.clone(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        let filtered = store
            .filter_servable_event_ids_for_workspace(
                &workspace_id.0,
                &[
                    EventId("evt_missing_candidate".to_owned()),
                    other.event_id.clone(),
                    forged.event_id.clone(),
                    EventId("evt_corrupt_candidate".to_owned()),
                    second.event_id.clone(),
                    first.event_id.clone(),
                ],
            )
            .unwrap();

        assert_eq!(filtered, vec![first.event_id, second.event_id]);
        assert!(store.list_events_for_workspace(&workspace_id.0).is_err());
    }

    #[test]
    fn filter_servable_workspace_candidates_chunks_oversized_inputs_in_store_order() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = "wrk_filter_candidates";
        let oversized_count = MAX_EVENT_STORE_CANDIDATE_FILTER_ROWS + 3;

        for index in 0..oversized_count {
            store
                .connection
                .execute(
                    "
                    INSERT INTO events (
                        event_id,
                        workspace_id,
                        channel_id,
                        author_device_id,
                        physical_ms,
                        logical,
                        self_contained_signature_valid,
                        event_json
                    ) VALUES (?1, ?2, NULL, ?3, ?4, 0, 1, ?5)
                    ",
                    params![
                        format!("evt_filter_{index:04}"),
                        workspace_id,
                        "dev_test",
                        index as i64,
                        b"{not valid json}".as_slice()
                    ],
                )
                .unwrap();
        }
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, ?4, 0, 1, ?5)
                ",
                params![
                    "evt_filter_other_workspace",
                    "wrk_other_filter_candidates",
                    "dev_test",
                    oversized_count as i64,
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        let mut candidates = (0..oversized_count)
            .rev()
            .map(|index| EventId(format!("evt_filter_{index:04}")))
            .collect::<Vec<_>>();
        candidates.insert(0, EventId("evt_filter_0001".to_owned()));
        candidates.push(EventId("evt_missing_filter_candidate".to_owned()));
        candidates.push(EventId("evt_filter_other_workspace".to_owned()));

        let filtered = store
            .filter_servable_event_ids_for_workspace(workspace_id, &candidates)
            .unwrap();

        assert_eq!(filtered.len(), oversized_count);
        assert_eq!(filtered[0], EventId("evt_filter_0000".to_owned()));
        assert_eq!(
            filtered[MAX_EVENT_STORE_CANDIDATE_FILTER_ROWS],
            EventId(format!(
                "evt_filter_{:04}",
                MAX_EVENT_STORE_CANDIDATE_FILTER_ROWS
            ))
        );
        assert_eq!(
            filtered.last().unwrap(),
            &EventId(format!("evt_filter_{:04}", oversized_count - 1))
        );
    }

    #[test]
    fn valid_append_repairs_existing_invalid_row_with_same_event_id() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Repairable".to_owned(),
            },
        ));
        let mut poisoned = valid.clone();
        poisoned.signature[0] ^= 0x01;

        store.append_event(&poisoned).unwrap();
        assert!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .is_empty()
        );

        store.append_event(&valid).unwrap();

        assert_eq!(
            store.get_event(&valid.event_id).unwrap(),
            Some(valid.clone())
        );
        assert_eq!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.event_id]
        );
    }

    #[test]
    fn servable_events_for_workspace_skip_corrupt_rows() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Scoped".to_owned(),
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Forged".to_owned(),
            },
        ));
        forged.signature[0] ^= 0x01;

        store.append_event(&valid).unwrap();
        store.append_event(&forged).unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 2, 0, 0, ?4)
                ",
                params![
                    "evt_corrupt_same_workspace",
                    workspace_id.0.as_str(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 3, 0, 1, ?4)
                ",
                params![
                    "evt_corrupt_poisoned_metadata",
                    workspace_id.0.as_str(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        assert!(store.list_events_for_workspace(&workspace_id.0).is_err());
        assert_eq!(
            store
                .list_parseable_events_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.clone(), forged]
        );
        assert_eq!(
            store
                .get_servable_event(&EventId("evt_corrupt_poisoned_metadata".to_owned()))
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .list_servable_events_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid]
        );
    }

    #[test]
    fn workspace_event_storage_health_counts_corrupt_and_poisoned_rows() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Healthy".to_owned(),
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Forged".to_owned(),
            },
        ));
        forged.signature[0] ^= 0x01;

        store.append_event(&valid).unwrap();
        store.append_event(&forged).unwrap();
        store
            .connection
            .execute(
                "
                UPDATE events
                SET self_contained_signature_valid = 0
                WHERE event_id = ?1
                ",
                params![valid.event_id.0.as_str()],
            )
            .unwrap();
        store
            .connection
            .execute(
                "
                UPDATE events
                SET self_contained_signature_valid = 1
                WHERE event_id = ?1
                ",
                params![forged.event_id.0.as_str()],
            )
            .unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 2, 0, 0, ?4)
                ",
                params![
                    "evt_corrupt",
                    workspace_id.0.as_str(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 3, 0, 1, ?4)
                ",
                params![
                    "evt_corrupt_poisoned_metadata",
                    workspace_id.0.as_str(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        let health = store
            .workspace_event_storage_health(&workspace_id.0)
            .unwrap();

        assert_eq!(health.workspace_id, workspace_id.0);
        assert_eq!(health.total_event_count, 4);
        assert_eq!(health.parseable_event_count, 2);
        assert_eq!(health.corrupt_event_count, 2);
        assert_eq!(health.signature_valid_metadata_count, 2);
        assert_eq!(health.servable_event_count, 1);
        assert_eq!(health.poisoned_servable_metadata_count, 2);
        assert_eq!(health.promotable_servable_metadata_count, 1);
        assert_eq!(health.non_servable_parseable_event_count, 1);
    }

    #[test]
    fn repairs_workspace_event_storage_metadata_without_deleting_rows() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Repair".to_owned(),
            },
        ));

        store.append_event(&valid).unwrap();
        store
            .connection
            .execute(
                "
                UPDATE events
                SET self_contained_signature_valid = 0
                WHERE event_id = ?1
                ",
                params![valid.event_id.0.as_str()],
            )
            .unwrap();
        store
            .connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, 2, 0, 1, ?4)
                ",
                params![
                    "evt_corrupt_poisoned_metadata",
                    workspace_id.0.as_str(),
                    "dev_test",
                    b"{not valid json}".as_slice()
                ],
            )
            .unwrap();

        assert!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .contains(&EventId("evt_corrupt_poisoned_metadata".to_owned()))
        );
        assert!(
            !store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .contains(&valid.event_id)
        );

        let repaired = store
            .repair_workspace_event_storage_metadata(&workspace_id.0)
            .unwrap();

        assert_eq!(repaired.workspace_id, workspace_id.0);
        assert_eq!(repaired.total_event_count, 2);
        assert_eq!(repaired.parseable_event_count, 1);
        assert_eq!(repaired.corrupt_event_count, 1);
        assert_eq!(repaired.signature_valid_metadata_before_count, 1);
        assert_eq!(repaired.signature_valid_metadata_after_count, 1);
        assert_eq!(repaired.repaired_metadata_count, 2);
        assert_eq!(repaired.promoted_servable_metadata_count, 1);
        assert_eq!(repaired.cleared_unservable_metadata_count, 1);
        assert_eq!(
            store
                .list_event_ids_for_workspace(&workspace_id.0)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.event_id]
        );
    }

    #[test]
    fn invalid_append_does_not_replace_existing_valid_row() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Durable".to_owned(),
            },
        ));
        let mut poisoned = valid.clone();
        poisoned.signature[0] ^= 0x01;

        store.append_event(&valid).unwrap();
        store.append_event(&poisoned).unwrap();

        assert_eq!(
            store.get_event(&valid.event_id).unwrap(),
            Some(valid.clone())
        );
        assert_eq!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.event_id]
        );
    }

    #[test]
    fn servable_event_id_backfill_handles_legacy_rows() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let valid = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Legacy valid".to_owned(),
            },
        ));
        let mut forged = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Legacy forged".to_owned(),
            },
        ));
        forged.signature[0] ^= 0x01;

        store.append_event(&valid).unwrap();
        store.append_event(&forged).unwrap();
        store
            .connection
            .execute(
                "
                UPDATE events
                SET self_contained_signature_valid = NULL
                ",
                [],
            )
            .unwrap();

        store.backfill_self_contained_signature_valid().unwrap();

        assert_eq!(
            store
                .list_servable_event_ids_for_workspace(&workspace_id.0)
                .unwrap(),
            vec![valid.event_id]
        );
    }

    #[test]
    fn lists_workspace_ids_by_first_seen_order() {
        let store = EventStore::open_in_memory().unwrap();
        let first_workspace_id = WorkspaceId::new();
        let second_workspace_id = WorkspaceId::new();
        let first = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                first_workspace_id.clone(),
                None,
                DeviceId("dev_test".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "First".to_owned(),
                },
            ),
            vec![1],
        );
        let second = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                second_workspace_id.clone(),
                None,
                DeviceId("dev_test".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "Second".to_owned(),
                },
            ),
            vec![2],
        );
        let third = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                first_workspace_id.clone(),
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "first workspace message".to_owned(),
                    attachments: Vec::new(),
                },
            ),
            vec![3],
        );

        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();
        store.append_event(&third).unwrap();

        assert_eq!(
            store.list_workspace_ids().unwrap(),
            vec![first_workspace_id.0, second_workspace_id.0]
        );
    }

    #[test]
    fn pages_workspace_ids_and_counts_without_deserializing_event_json() {
        let store = EventStore::open_in_memory().unwrap();
        for index in 0..4 {
            store
                .connection
                .execute(
                    "
                    INSERT INTO events (
                        event_id,
                        workspace_id,
                        channel_id,
                        author_device_id,
                        physical_ms,
                        logical,
                        event_json
                    ) VALUES (?1, ?2, NULL, ?3, ?4, 0, ?5)
                    ",
                    params![
                        format!("evt_corrupt_{index}"),
                        format!("wrk_corrupt_{index}"),
                        "dev_test",
                        index as i64,
                        b"{not valid json}".as_slice()
                    ],
                )
                .unwrap();
        }

        assert_eq!(store.count_workspaces().unwrap(), 4);
        assert_eq!(
            store.list_workspace_ids_page(1, 2).unwrap(),
            vec!["wrk_corrupt_1".to_owned(), "wrk_corrupt_2".to_owned()]
        );
        assert_eq!(
            store.list_workspace_ids_page(4, 2).unwrap(),
            Vec::<String>::new()
        );
        assert!(store.list_events().is_err());
    }

    #[test]
    fn page_helpers_clamp_oversized_limits_before_sqlite() {
        let store = EventStore::open_in_memory().unwrap();
        let oversized_count = MAX_EVENT_STORE_PAGE_ROWS + 3;
        for index in 0..oversized_count {
            store
                .connection
                .execute(
                    "
                    INSERT INTO events (
                        event_id,
                        workspace_id,
                        channel_id,
                        author_device_id,
                        physical_ms,
                        logical,
                        self_contained_signature_valid,
                        event_json
                    ) VALUES (?1, ?2, NULL, ?3, ?4, 0, 1, ?5)
                    ",
                    params![
                        format!("evt_global_{index:04}"),
                        format!("wrk_global_{index:04}"),
                        "dev_test",
                        index as i64,
                        b"{not valid json}".as_slice()
                    ],
                )
                .unwrap();
        }

        let workspace_ids = store.list_workspace_ids_page(0, usize::MAX).unwrap();
        let event_ids = store.list_servable_event_ids_page(0, usize::MAX).unwrap();

        assert_eq!(workspace_ids.len(), MAX_EVENT_STORE_PAGE_ROWS);
        assert_eq!(workspace_ids[0], "wrk_global_0000");
        assert_eq!(
            workspace_ids.last().unwrap(),
            &format!("wrk_global_{:04}", MAX_EVENT_STORE_PAGE_ROWS - 1)
        );
        assert_eq!(event_ids.len(), MAX_EVENT_STORE_PAGE_ROWS);
        assert_eq!(event_ids[0], EventId("evt_global_0000".to_owned()));
        assert_eq!(
            event_ids.last().unwrap(),
            &EventId(format!("evt_global_{:04}", MAX_EVENT_STORE_PAGE_ROWS - 1))
        );

        for index in 0..oversized_count {
            store
                .connection
                .execute(
                    "
                    INSERT INTO events (
                        event_id,
                        workspace_id,
                        channel_id,
                        author_device_id,
                        physical_ms,
                        logical,
                        self_contained_signature_valid,
                        event_json
                    ) VALUES (?1, ?2, NULL, ?3, ?4, 0, 1, ?5)
                    ",
                    params![
                        format!("evt_workspace_{index:04}"),
                        "wrk_shared",
                        "dev_test",
                        index as i64,
                        b"{not valid json}".as_slice()
                    ],
                )
                .unwrap();
        }

        let workspace_event_ids = store
            .list_servable_event_ids_for_workspace_page("wrk_shared", 0, usize::MAX)
            .unwrap();
        let workspace_event_tail = store
            .list_servable_event_ids_for_workspace_page(
                "wrk_shared",
                MAX_EVENT_STORE_PAGE_ROWS,
                usize::MAX,
            )
            .unwrap();

        assert_eq!(workspace_event_ids.len(), MAX_EVENT_STORE_PAGE_ROWS);
        assert_eq!(
            workspace_event_ids[0],
            EventId("evt_workspace_0000".to_owned())
        );
        assert_eq!(
            workspace_event_ids.last().unwrap(),
            &EventId(format!(
                "evt_workspace_{:04}",
                MAX_EVENT_STORE_PAGE_ROWS - 1
            ))
        );
        assert_eq!(
            workspace_event_tail.len(),
            oversized_count - MAX_EVENT_STORE_PAGE_ROWS
        );
        assert_eq!(
            workspace_event_tail[0],
            EventId(format!("evt_workspace_{:04}", MAX_EVENT_STORE_PAGE_ROWS))
        );
    }
}
