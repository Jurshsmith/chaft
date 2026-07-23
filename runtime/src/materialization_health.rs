use std::sync::Mutex;

use chaft_types::{EventId, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    LocalRuntime, PulledWorkspaceGap, RuntimeError, read_local_metadata_file_with_limit,
    write_derived_cache_file,
};

const MATERIALIZATION_HEALTH_CACHE_SCHEMA_VERSION: u32 = 1;
const MATERIALIZATION_HEALTH_CACHE_MAX_BYTES: usize = 256 * 1024;
const MATERIALIZATION_HEALTH_CACHE_MAX_WORKSPACES: usize = 256;
const MATERIALIZATION_HEALTH_CACHE_MAX_GAPS_PER_WORKSPACE: usize = 1_024;
const MATERIALIZATION_HEALTH_SCAN_MAX_ATTEMPTS: usize = 2;
pub(crate) const MATERIALIZATION_HEALTH_CACHE_FILE: &str = "materialization-health-cache.json";

static MATERIALIZATION_HEALTH_CACHE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterializationHealthCache {
    schema_version: u32,
    workspaces: Vec<CachedWorkspaceMaterializationHealth>,
}

impl Default for MaterializationHealthCache {
    fn default() -> Self {
        Self {
            schema_version: MATERIALIZATION_HEALTH_CACHE_SCHEMA_VERSION,
            workspaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedWorkspaceMaterializationHealth {
    workspace_id: String,
    event_count: usize,
    event_inventory_fingerprint: String,
    gaps: Vec<PulledWorkspaceGap>,
}

impl LocalRuntime {
    /// Returns the complete materialization-gap health for the exact current
    /// local sync inventory without turning a no-change pull into a repeated
    /// full-history scan.
    ///
    /// Only gaps come from this health scan. `PulledWorkspace.applied_event_ids`
    /// remains the pull report's value and is never repopulated from cached or
    /// locally re-materialized history on a no-change pull.
    pub(crate) fn current_workspace_materialization_gaps(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<PulledWorkspaceGap>, RuntimeError> {
        let mut inventory = self.workspace_materialization_inventory(workspace_id)?;

        if let Some(gaps) = self.cached_workspace_materialization_gaps(workspace_id, &inventory) {
            let inventory_after_lookup = self.workspace_materialization_inventory(workspace_id)?;
            if inventory_after_lookup == inventory {
                return Ok(gaps);
            }
            inventory = inventory_after_lookup;
        }

        let mut latest_gaps = Vec::new();
        for _ in 0..MATERIALIZATION_HEALTH_SCAN_MAX_ATTEMPTS {
            let (_, gaps) = self.materialized_workspace_events_with_gaps(workspace_id)?;
            let inventory_after_scan = self.workspace_materialization_inventory(workspace_id)?;
            latest_gaps = gaps;
            if inventory_after_scan == inventory {
                self.record_workspace_materialization_gaps(workspace_id, &inventory, &latest_gaps);
                return Ok(latest_gaps);
            }
            inventory = inventory_after_scan;
        }

        // A different process may be actively appending to the same store. Do
        // not let derived health caching fail event sync: return the last
        // complete point-in-time scan, but leave the cache stale so the next
        // sync checks again.
        Ok(latest_gaps)
    }

    fn workspace_materialization_inventory(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, RuntimeError> {
        let mut event_ids = self
            .store
            .list_servable_event_ids_for_workspace(&workspace_id.0)?;
        event_ids.sort_unstable();
        Ok(event_ids)
    }

    fn cached_workspace_materialization_gaps(
        &self,
        workspace_id: &WorkspaceId,
        event_ids: &[EventId],
    ) -> Option<Vec<PulledWorkspaceGap>> {
        let _guard = MATERIALIZATION_HEALTH_CACHE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let cache = self.read_materialization_health_cache().ok()?;
        let fingerprint = event_inventory_fingerprint(event_ids);
        cache
            .workspaces
            .iter()
            .find(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.event_count == event_ids.len()
                    && entry.event_inventory_fingerprint == fingerprint
            })
            .map(|entry| entry.gaps.clone())
    }

    fn record_workspace_materialization_gaps(
        &self,
        workspace_id: &WorkspaceId,
        event_ids: &[EventId],
        gaps: &[PulledWorkspaceGap],
    ) {
        if gaps.len() > MATERIALIZATION_HEALTH_CACHE_MAX_GAPS_PER_WORKSPACE {
            return;
        }

        let entry = CachedWorkspaceMaterializationHealth {
            workspace_id: workspace_id.0.clone(),
            event_count: event_ids.len(),
            event_inventory_fingerprint: event_inventory_fingerprint(event_ids),
            gaps: gaps.to_vec(),
        };
        let Ok(entry_bytes) = serde_json::to_vec(&entry) else {
            return;
        };
        if entry_bytes.len() >= MATERIALIZATION_HEALTH_CACHE_MAX_BYTES {
            return;
        }

        let _guard = MATERIALIZATION_HEALTH_CACHE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut cache = self.read_materialization_health_cache().unwrap_or_default();
        cache
            .workspaces
            .retain(|cached| cached.workspace_id != workspace_id.0);
        cache.workspaces.push(entry);
        while cache.workspaces.len() > MATERIALIZATION_HEALTH_CACHE_MAX_WORKSPACES {
            cache.workspaces.remove(0);
        }

        let bytes = loop {
            let Ok(bytes) = serde_json::to_vec(&cache) else {
                return;
            };
            // `write_derived_cache_file` appends one newline byte.
            if bytes.len() < MATERIALIZATION_HEALTH_CACHE_MAX_BYTES {
                break bytes;
            }
            if cache.workspaces.len() <= 1 {
                return;
            }
            cache.workspaces.remove(0);
        };

        // This is a derived cache. If persistence is unavailable, the next
        // sync performs a fresh health scan and still returns truthful gaps.
        let _ = write_derived_cache_file(&self.materialization_health_cache_path(), &bytes);
    }

    fn read_materialization_health_cache(
        &self,
    ) -> Result<MaterializationHealthCache, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            &self.materialization_health_cache_path(),
            MATERIALIZATION_HEALTH_CACHE_MAX_BYTES,
            "materialization health cache",
        )?
        else {
            return Ok(MaterializationHealthCache::default());
        };
        let cache = serde_json::from_slice::<MaterializationHealthCache>(&bytes)?;
        if cache.schema_version != MATERIALIZATION_HEALTH_CACHE_SCHEMA_VERSION
            || cache.workspaces.len() > MATERIALIZATION_HEALTH_CACHE_MAX_WORKSPACES
            || cache.workspaces.iter().any(|entry| {
                entry.gaps.len() > MATERIALIZATION_HEALTH_CACHE_MAX_GAPS_PER_WORKSPACE
                    || entry.event_inventory_fingerprint.len() != blake3::OUT_LEN * 2
                    || !entry
                        .event_inventory_fingerprint
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            return Ok(MaterializationHealthCache::default());
        }
        Ok(cache)
    }

    fn materialization_health_cache_path(&self) -> std::path::PathBuf {
        self.paths.data_dir.join(MATERIALIZATION_HEALTH_CACHE_FILE)
    }
}

fn event_inventory_fingerprint(event_ids: &[EventId]) -> String {
    let mut hasher = blake3::Hasher::new();
    let mut ordered_event_ids = event_ids.iter().collect::<Vec<_>>();
    ordered_event_ids.sort_unstable();
    for event_id in ordered_event_ids {
        hasher.update(&(event_id.0.len() as u64).to_le_bytes());
        hasher.update(event_id.0.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::event_inventory_fingerprint;
    use chaft_types::EventId;

    #[test]
    fn inventory_fingerprint_covers_exact_set_length_and_content() {
        let first = vec![EventId("evt_a".to_owned()), EventId("evt_bc".to_owned())];
        let reordered = vec![EventId("evt_bc".to_owned()), EventId("evt_a".to_owned())];
        let repartitioned = vec![EventId("evt_ab".to_owned()), EventId("evt_c".to_owned())];

        assert_eq!(
            event_inventory_fingerprint(&first),
            event_inventory_fingerprint(&reordered)
        );
        assert_ne!(
            event_inventory_fingerprint(&first),
            event_inventory_fingerprint(&repartitioned)
        );
    }
}
