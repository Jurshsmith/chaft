use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata},
    io::Read,
    path::Path,
    sync::Mutex,
    time::UNIX_EPOCH,
};

use chaft_core::WorkspaceState;
use chaft_media::BlobAvailability;
use chaft_net::{ChaftTransport, NetError, PeerAddress};
use chaft_net_direct::{
    AuthorizedPublishTransport, BlobSyncTransport, MAX_PUBLISH_EVENTS_PER_REQUEST,
};
use chaft_sync::{
    PullSyncReport, WorkspaceSyncPlan, plan_workspace_sync, pull_workspace_from_peer,
    pull_workspace_from_peer_with_plan, validate_remote_inventory_event_ids,
};
use chaft_types::{EventId, SignedEvent, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{
    BlobTransferAttempt, BlobTransferMode, BlobTransferRetryReport, BlobTransferStatus,
    DIRECT_BLOB_CHUNK_SIZE, DIRECT_WHOLE_BLOB_SYNC_LIMIT, LocalRuntime,
    MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS, MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS,
    MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS, OpenMlsAutoProvisionIndex, PublishedWorkspace,
    PulledOpenMlsCatchup, PulledOpenMlsChannelCatchup, PulledWorkspace, PulledWorkspaceGap,
    RuntimeError, SyncedWorkspace, WorkspacePublishQueue, attachment_blob_hashes,
    blob_transfer_peer_error, is_backup_slice_event, merge_published_workspace, now_unix_ms,
    planned_chunk_upload, planned_retry_peers, read_local_metadata_file_with_limit,
    validate_event_id_reference, validate_peer_address, validate_peer_addresses,
    validate_workspace_id_reference, workspace_creator_device_id_from_events,
    workspace_publish_queue_summary, write_derived_cache_file,
};

const INBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION: u32 = 1;
const INBOUND_BLOB_REPAIR_LEDGER_MAX_BYTES: usize = 256 * 1024;
const INBOUND_BLOB_REPAIR_MAX_WORKSPACES: usize = 256;
const INBOUND_BLOB_REPAIR_MAX_HASHES_PER_WORKSPACE: usize = 512;
const INBOUND_BLOB_FULL_REPAIR_INTERVAL_MS: u64 = 60_000;
const INBOUND_BLOB_REPAIR_LEDGER_FILE: &str = "inbound-blob-repair-ledger.json";
const OUTBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION: u32 = 1;
const OUTBOUND_BLOB_REPAIR_LEDGER_MAX_BYTES: usize = 256 * 1024;
const OUTBOUND_BLOB_REPAIR_MAX_PEERS: usize = 512;
const OUTBOUND_BLOB_FULL_REPAIR_INTERVAL_MS: u64 = 60_000;
const OUTBOUND_BLOB_REPAIR_LEDGER_FILE: &str = "outbound-blob-repair-ledger.json";
const OPENMLS_RECONCILE_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
const OPENMLS_RECONCILE_CHECKPOINT_MAX_BYTES: usize = 256 * 1024;
const OPENMLS_RECONCILE_CHECKPOINT_MAX_ENTRIES: usize = 256;
const OPENMLS_RECONCILE_SECRET_MAX_FILES: usize = 2_048;
const OPENMLS_RECONCILE_SECRET_MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const OPENMLS_RECONCILE_CHECKPOINT_FILE: &str = "openmls-reconcile-checkpoint.json";

static INBOUND_BLOB_REPAIR_LEDGER_LOCK: Mutex<()> = Mutex::new(());
static OUTBOUND_BLOB_REPAIR_LEDGER_LOCK: Mutex<()> = Mutex::new(());
static OPENMLS_RECONCILE_CHECKPOINT_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundBlobRepairWorkspace {
    workspace_id: String,
    last_full_scan_unix_ms: u64,
    pending_blob_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InboundBlobRepairLedger {
    schema_version: u32,
    workspaces: Vec<InboundBlobRepairWorkspace>,
}

impl Default for InboundBlobRepairLedger {
    fn default() -> Self {
        Self {
            schema_version: INBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION,
            workspaces: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundBlobRepairPeer {
    workspace_id: String,
    peer_id: String,
    peer_endpoint: String,
    local_inventory_fingerprint: String,
    last_full_scan_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutboundBlobRepairLedger {
    schema_version: u32,
    peers: Vec<OutboundBlobRepairPeer>,
}

impl Default for OutboundBlobRepairLedger {
    fn default() -> Self {
        Self {
            schema_version: OUTBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION,
            peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenMlsReconcileCheckpointEntry {
    workspace_id: String,
    device_id: String,
    event_inventory_fingerprint: String,
    local_secret_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenMlsReconcileCheckpointLedger {
    schema_version: u32,
    entries: Vec<OpenMlsReconcileCheckpointEntry>,
}

impl Default for OpenMlsReconcileCheckpointLedger {
    fn default() -> Self {
        Self {
            schema_version: OPENMLS_RECONCILE_CHECKPOINT_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenMlsReconcileSnapshot {
    event_ids: Vec<EventId>,
    event_inventory_fingerprint: String,
    local_secret_fingerprint: String,
}

impl LocalRuntime {
    pub fn workspace_publish_queue(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspacePublishQueue, RuntimeError> {
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let mut publishable_event_ids = events
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        let mut backup_event_ids = events
            .iter()
            .filter(|event| is_backup_slice_event(event))
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        let backup_event_id_set = backup_event_ids.iter().cloned().collect::<BTreeSet<_>>();
        let blob_store = self.open_blob_store()?;
        let mut available_blob_hashes = Vec::new();
        let mut missing_blob_hashes = Vec::new();
        for blob_hash in attachment_blob_hashes(&events) {
            if blob_store.has_complete_blob(&blob_hash)? {
                available_blob_hashes.push(blob_hash);
            } else {
                missing_blob_hashes.push(blob_hash);
            }
        }
        let summary = workspace_publish_queue_summary(
            &events,
            &backup_event_id_set,
            &available_blob_hashes,
            &missing_blob_hashes,
            &skipped_gaps,
        );
        publishable_event_ids.truncate(MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS);
        backup_event_ids.truncate(MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS);
        available_blob_hashes.truncate(MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS);
        missing_blob_hashes.truncate(MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS);
        let mut skipped_gaps = skipped_gaps;
        skipped_gaps.truncate(MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS);

        Ok(WorkspacePublishQueue {
            workspace_id: workspace_id.0,
            summary,
            publishable_event_ids,
            backup_event_ids,
            available_blob_hashes,
            missing_blob_hashes,
            skipped_gaps,
        })
    }

    pub async fn publish_workspace_to_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let mut published_event_ids = Vec::with_capacity(events.len());
        for event in events {
            let event_id = event.event_id.0.clone();
            transport.publish_event(peer, event).await?;
            published_event_ids.push(event_id);
        }

        Ok(PublishedWorkspace::from_parts(
            workspace_id.0,
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        ))
    }

    async fn publish_workspace_to_peer_with_plan<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
        plan: &WorkspaceSyncPlan,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        debug_assert_eq!(plan.workspace_id(), &workspace_id);
        let publish_event_ids = plan
            .publish_event_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if publish_event_ids.is_empty() {
            return Ok(Self::empty_published_workspace(workspace_id));
        }

        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let events_to_publish = events
            .into_iter()
            .filter(|event| publish_event_ids.contains(&event.event_id))
            .collect::<Vec<_>>();
        let mut published_event_ids = Vec::with_capacity(events_to_publish.len());
        for event in events_to_publish {
            let event_id = event.event_id.0.clone();
            transport.publish_event(peer, event).await?;
            published_event_ids.push(event_id);
        }

        Ok(PublishedWorkspace::from_parts(
            workspace_id.0,
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        ))
    }

    pub async fn publish_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_peer_address(peer)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        let plan = plan_workspace_sync(&self.store, &workspace_id, remote_event_ids)?;
        let published = self
            .publish_workspace_direct_with_plan(transport, peer, workspace_id.clone(), &plan)
            .await?;
        self.record_outbound_blob_full_repair(&workspace_id, peer, plan.local_event_ids())?;
        Ok(published)
    }

    async fn publish_workspace_direct_with_plan<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
        plan: &WorkspaceSyncPlan,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: AuthorizedPublishTransport + BlobSyncTransport,
    {
        debug_assert_eq!(plan.workspace_id(), &workspace_id);
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let publish_event_ids = plan
            .publish_event_ids()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let events_to_publish = events
            .iter()
            .filter(|event| publish_event_ids.contains(&event.event_id))
            .cloned()
            .collect::<Vec<_>>();
        let published_event_ids = events_to_publish
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        if !events_to_publish.is_empty() {
            transport
                .publish_events_with_authorization(peer, events_to_publish, Vec::new(), Vec::new())
                .await?;
        }

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0.clone(),
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(transport, peer, &events, &mut published)
            .await?;
        published.refresh_counts();

        Ok(published)
    }

    pub async fn publish_event_direct_with_trust_snapshot<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
        event_id: EventId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_event_id_reference(&event_id)?;
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let event = events
            .iter()
            .find(|event| event.event_id == event_id)
            .cloned()
            .ok_or_else(|| RuntimeError::EventNotFound {
                workspace_id: workspace_id.clone(),
                event_id: event_id.clone(),
            })?;
        let trust_snapshot =
            self.sign_trust_snapshot_for_materialized_event(workspace_id.clone(), &events, &event)?;
        transport
            .publish_events_with_authorization(
                peer,
                vec![event.clone()],
                Vec::new(),
                vec![trust_snapshot],
            )
            .await?;

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0,
            vec![event.event_id.0.clone()],
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(transport, peer, &[event], &mut published)
            .await?;
        published.refresh_counts();
        Ok(published)
    }

    pub async fn backup_workspace_direct_with_trust_snapshot<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_peer_address(peer)?;
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        validate_remote_inventory_event_ids(&remote_event_ids)?;
        let remote_event_ids = remote_event_ids.into_iter().collect::<BTreeSet<_>>();
        let backup_events = events
            .iter()
            .filter(|event| is_backup_slice_event(event))
            .cloned()
            .collect::<Vec<_>>();
        let events_to_publish = backup_events
            .iter()
            .filter(|event| !remote_event_ids.contains(&event.event_id))
            .cloned()
            .collect::<Vec<_>>();
        let published_event_ids = events_to_publish
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();

        for event_chunk in events_to_publish.chunks(MAX_PUBLISH_EVENTS_PER_REQUEST) {
            let trust_snapshot = self.sign_trust_snapshot_for_materialized_event_slice(
                workspace_id.clone(),
                &events,
                event_chunk,
            )?;
            transport
                .publish_events_with_authorization(
                    peer,
                    event_chunk.to_vec(),
                    Vec::new(),
                    vec![trust_snapshot],
                )
                .await?;
        }

        let mut published = PublishedWorkspace::from_parts(
            workspace_id.0,
            published_event_ids,
            Vec::new(),
            Vec::new(),
            skipped_gaps,
            Vec::new(),
        );
        self.publish_materialized_event_blobs_direct(
            transport,
            peer,
            &backup_events,
            &mut published,
        )
        .await?;
        published.refresh_counts();
        Ok(published)
    }

    pub async fn retry_pending_blob_transfers_direct<T>(
        &self,
        transport: &T,
        workspace_id: WorkspaceId,
        peers: &[PeerAddress],
    ) -> Result<BlobTransferRetryReport, RuntimeError>
    where
        T: BlobSyncTransport,
    {
        validate_peer_addresses(peers)?;
        let materialized_blob_hashes =
            attachment_blob_hashes(&self.materialized_workspace_events(&workspace_id)?)
                .into_iter()
                .collect::<BTreeSet<_>>();
        let ledger_entries = self.read_blob_transfer_ledger()?.entries;
        let pending_entries = ledger_entries
            .iter()
            .filter(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.status != BlobTransferStatus::Succeeded
            })
            .cloned()
            .collect::<Vec<_>>();
        let pending_attempt_ids = pending_entries
            .iter()
            .map(|entry| entry.attempt_id.clone())
            .collect::<Vec<_>>();
        let blob_store = self.open_blob_store()?;
        let mut report = BlobTransferRetryReport {
            workspace_id: workspace_id.0.clone(),
            pending_attempt_count: 0,
            pending_attempt_ids,
            retried_blob_count: 0,
            retried_blob_hashes: Vec::new(),
            reconciled_blob_count: 0,
            reconciled_blob_hashes: Vec::new(),
            missing_blob_count: 0,
            missing_blob_hashes: Vec::new(),
            skipped_blob_count: 0,
            skipped_blob_hashes: Vec::new(),
            peer_error_count: 0,
            peer_errors: Vec::new(),
            blob_transfer_attempt_count: 0,
            blob_transfer_attempts: Vec::new(),
        };
        let mut retried = BTreeSet::new();
        let mut reconciled = BTreeSet::new();
        let mut missing = BTreeSet::new();
        let mut skipped = BTreeSet::new();
        let mut processed = BTreeSet::new();

        for pending in pending_entries {
            if !processed.insert(pending.blob_hash.clone()) {
                continue;
            }
            if reconciled.contains(&pending.blob_hash) {
                continue;
            }
            if !materialized_blob_hashes.contains(&pending.blob_hash) {
                if skipped.insert(pending.blob_hash.clone()) {
                    report.skipped_blob_hashes.push(pending.blob_hash.clone());
                }
                continue;
            }
            let Some(bytes) = blob_store.get_complete_bytes(&pending.blob_hash)? else {
                if missing.insert(pending.blob_hash.clone()) {
                    report.missing_blob_hashes.push(pending.blob_hash.clone());
                }
                continue;
            };

            let retry_peers =
                planned_retry_peers(peers, &ledger_entries, &workspace_id.0, &pending.blob_hash);
            for peer in retry_peers {
                let remote_blob_availability = match transport
                    .fetch_blob_availabilities(peer, vec![pending.blob_hash.clone()])
                    .await
                {
                    Ok(availability) => availability,
                    Err(error) => {
                        let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                        report.peer_errors.push(blob_transfer_peer_error(
                            &peer.peer_id.0,
                            &peer.endpoint,
                            &pending.blob_hash,
                            error.to_string(),
                            suspect_protocol_error,
                        ));
                        continue;
                    }
                };
                if remote_blob_availability
                    .get(&pending.blob_hash)
                    .is_some_and(|availability| availability.is_complete())
                {
                    let reconciled_attempts = self.reconcile_satisfied_blob_transfer_attempts(
                        &workspace_id.0,
                        &pending.blob_hash,
                    )?;
                    if !reconciled_attempts.is_empty()
                        && reconciled.insert(pending.blob_hash.clone())
                    {
                        report
                            .reconciled_blob_hashes
                            .push(pending.blob_hash.clone());
                    }
                    report.blob_transfer_attempts.extend(reconciled_attempts);
                    break;
                }

                let (upload, suspect_protocol_error) = self
                    .retry_blob_transfer_to_peer(
                        transport,
                        peer,
                        &workspace_id.0,
                        &pending.blob_hash,
                        bytes.clone(),
                        remote_blob_availability.get(&pending.blob_hash),
                    )
                    .await?;
                if upload.status == BlobTransferStatus::Succeeded {
                    let upload_blob_hash = upload.blob_hash.clone();
                    if retried.insert(upload_blob_hash.clone()) {
                        report.retried_blob_hashes.push(upload_blob_hash.clone());
                    }
                    report.blob_transfer_attempts.push(upload);
                    let reconciled_attempts = self.reconcile_satisfied_blob_transfer_attempts(
                        &workspace_id.0,
                        &upload_blob_hash,
                    )?;
                    if !reconciled_attempts.is_empty()
                        && reconciled.insert(upload_blob_hash.clone())
                    {
                        report.reconciled_blob_hashes.push(upload_blob_hash);
                    }
                    report.blob_transfer_attempts.extend(reconciled_attempts);
                    break;
                }
                if let Some(message) = upload.error.clone() {
                    report.peer_errors.push(blob_transfer_peer_error(
                        &upload.peer_id,
                        &upload.peer_endpoint,
                        &upload.blob_hash,
                        message,
                        suspect_protocol_error,
                    ));
                }
                report.blob_transfer_attempts.push(upload);
            }
        }

        report.refresh_counts();
        Ok(report)
    }

    pub async fn pull_workspace_from_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PulledWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let report =
            pull_workspace_from_peer(transport, peer, &self.store, workspace_id.clone()).await?;
        self.finish_workspace_pull(workspace_id, report)
    }

    pub async fn pull_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<PulledWorkspace, RuntimeError>
    where
        T: ChaftTransport + BlobSyncTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        let plan = plan_workspace_sync(&self.store, &workspace_id, remote_event_ids)?;
        self.pull_workspace_direct_with_plan(transport, peer, workspace_id, &plan, true)
            .await
    }

    async fn pull_workspace_direct_with_plan<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
        plan: &WorkspaceSyncPlan,
        repair_existing_blobs: bool,
    ) -> Result<PulledWorkspace, RuntimeError>
    where
        T: ChaftTransport + BlobSyncTransport,
    {
        debug_assert_eq!(plan.workspace_id(), &workspace_id);
        let report = pull_workspace_from_peer_with_plan(
            transport,
            peer,
            &self.store,
            workspace_id.clone(),
            plan,
        )
        .await?;
        let fetched_event_delta = report.has_fetched_events();
        let mut pulled = self.finish_workspace_pull(workspace_id.clone(), report)?;
        if fetched_event_delta || repair_existing_blobs {
            self.reconcile_missing_workspace_blobs(transport, peer, &workspace_id, &mut pulled)
                .await?;
        } else {
            self.reconcile_pending_or_due_workspace_blobs(
                transport,
                peer,
                &workspace_id,
                &mut pulled,
            )
            .await?;
        }
        pulled.refresh_counts();
        Ok(pulled)
    }

    fn finish_workspace_pull(
        &self,
        workspace_id: WorkspaceId,
        report: PullSyncReport,
    ) -> Result<PulledWorkspace, RuntimeError> {
        let fetched_event_delta = report.has_fetched_events();
        let local_membership_hint = fetched_event_delta.then(|| {
            report
                .materialized_member_device_ids
                .contains(self.identity.device_id())
        });
        let invite_profile_event_ids =
            self.finalize_pending_workspace_invite_profile(&workspace_id)?;
        let openmls_catchup = self.reconcile_openmls_access_with_membership_hint(
            workspace_id.clone(),
            local_membership_hint,
        )?;
        let compromise_response = self.automatic_compromise_response_if_needed(&workspace_id)?;
        let local_state_changed = !invite_profile_event_ids.is_empty()
            || openmls_catchup.event_count > 0
            || compromise_response
                .as_ref()
                .is_some_and(|response| response.rotated_local_secret_state);
        if fetched_event_delta || local_state_changed {
            let _ = self.reindex_workspace_search_if_key_available(&workspace_id);
        }
        let mut pulled = Self::pulled_workspace_from_report(workspace_id.clone(), report);
        pulled.invite_profile_event_ids = invite_profile_event_ids;
        pulled.openmls_catchup = openmls_catchup;
        pulled.compromise_response = compromise_response;
        if !fetched_event_delta {
            pulled.gaps = self.current_workspace_materialization_gaps(&workspace_id)?;
        }
        pulled.refresh_counts();
        Ok(pulled)
    }

    async fn reconcile_missing_workspace_blobs<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        pulled: &mut PulledWorkspace,
    ) -> Result<(), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        let events = self.materialized_workspace_events(workspace_id)?;
        let blob_hashes = attachment_blob_hashes(&events);
        let blob_store = self.open_blob_store()?;
        let mut missing_local_blob_hashes = Vec::new();

        for blob_hash in &blob_hashes {
            if blob_store.has_complete_blob(blob_hash)? {
                continue;
            }
            missing_local_blob_hashes.push(blob_hash.clone());
        }

        let pending_blob_hashes = self
            .fetch_missing_workspace_blobs(transport, peer, missing_local_blob_hashes, pulled)
            .await?;
        self.update_inbound_blob_repair_workspace(
            workspace_id,
            Some(now_unix_ms()),
            pending_blob_hashes,
        )?;
        Ok(())
    }

    async fn reconcile_pending_or_due_workspace_blobs<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        pulled: &mut PulledWorkspace,
    ) -> Result<(), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        let repair = self.inbound_blob_repair_workspace(workspace_id)?;
        let full_scan_due = repair.as_ref().is_none_or(|repair| {
            now_unix_ms().saturating_sub(repair.last_full_scan_unix_ms)
                >= INBOUND_BLOB_FULL_REPAIR_INTERVAL_MS
        });
        if full_scan_due {
            return self
                .reconcile_missing_workspace_blobs(transport, peer, workspace_id, pulled)
                .await;
        }

        let Some(repair) = repair else {
            return Ok(());
        };
        if repair.pending_blob_hashes.is_empty() {
            return Ok(());
        }
        let pending_blob_hashes = self
            .fetch_missing_workspace_blobs(transport, peer, repair.pending_blob_hashes, pulled)
            .await?;
        self.update_inbound_blob_repair_workspace(workspace_id, None, pending_blob_hashes)
    }

    async fn fetch_missing_workspace_blobs<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        blob_hashes: Vec<String>,
        pulled: &mut PulledWorkspace,
    ) -> Result<Vec<String>, RuntimeError>
    where
        T: BlobSyncTransport,
    {
        if blob_hashes.is_empty() {
            return Ok(Vec::new());
        }
        let blob_store = self.open_blob_store()?;
        let mut missing_local_blob_hashes = Vec::new();
        for blob_hash in blob_hashes {
            if blob_store.has_complete_blob(&blob_hash)? {
                continue;
            }
            missing_local_blob_hashes.push(blob_hash);
        }
        if missing_local_blob_hashes.is_empty() {
            return Ok(Vec::new());
        }
        let fetched_blobs = transport
            .fetch_blobs(peer, missing_local_blob_hashes.clone())
            .await?;
        let mut pending_blob_hashes = Vec::new();
        for blob_hash in missing_local_blob_hashes {
            match fetched_blobs.get(&blob_hash) {
                Some(bytes) => {
                    blob_store.put_bytes_with_hash(&blob_hash, bytes)?;
                    pulled.fetched_blob_hashes.push(blob_hash);
                }
                None => match transport.fetch_blob_chunked(peer, &blob_hash).await? {
                    Some(bytes) => {
                        blob_store.put_bytes_with_hash(&blob_hash, &bytes)?;
                        pulled.fetched_blob_hashes.push(blob_hash);
                    }
                    None => {
                        pulled.missing_blob_hashes.push(blob_hash.clone());
                        pending_blob_hashes.push(blob_hash);
                    }
                },
            }
        }
        Ok(pending_blob_hashes)
    }

    fn inbound_blob_repair_workspace(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<InboundBlobRepairWorkspace>, RuntimeError> {
        let _guard = INBOUND_BLOB_REPAIR_LEDGER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ledger = self.read_inbound_blob_repair_ledger().unwrap_or_default();
        Ok(ledger
            .workspaces
            .into_iter()
            .find(|workspace| workspace.workspace_id == workspace_id.0))
    }

    fn update_inbound_blob_repair_workspace(
        &self,
        workspace_id: &WorkspaceId,
        last_full_scan_unix_ms: Option<u64>,
        pending_blob_hashes: Vec<String>,
    ) -> Result<(), RuntimeError> {
        let _guard = INBOUND_BLOB_REPAIR_LEDGER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut ledger = self.read_inbound_blob_repair_ledger().unwrap_or_default();
        let workspace = match ledger
            .workspaces
            .iter_mut()
            .find(|workspace| workspace.workspace_id == workspace_id.0)
        {
            Some(workspace) => workspace,
            None => {
                if ledger.workspaces.len() >= INBOUND_BLOB_REPAIR_MAX_WORKSPACES {
                    ledger.workspaces.remove(0);
                }
                ledger.workspaces.push(InboundBlobRepairWorkspace {
                    workspace_id: workspace_id.0.clone(),
                    ..InboundBlobRepairWorkspace::default()
                });
                ledger
                    .workspaces
                    .last_mut()
                    .expect("workspace was inserted")
            }
        };
        if let Some(last_full_scan_unix_ms) = last_full_scan_unix_ms {
            workspace.last_full_scan_unix_ms = last_full_scan_unix_ms;
        }
        workspace.pending_blob_hashes = pending_blob_hashes
            .into_iter()
            .filter(|hash| valid_blob_hash_for_repair_ledger(hash))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(INBOUND_BLOB_REPAIR_MAX_HASHES_PER_WORKSPACE)
            .collect();
        // This ledger is derived retry-scheduler metadata. A read-only data
        // directory or interrupted cache write must not fail event sync; the
        // next poll safely falls back to a bounded full blob scan.
        let _ = self.write_inbound_blob_repair_ledger(&ledger);
        Ok(())
    }

    fn read_inbound_blob_repair_ledger(&self) -> Result<InboundBlobRepairLedger, RuntimeError> {
        let path = self.paths.data_dir.join(INBOUND_BLOB_REPAIR_LEDGER_FILE);
        let Some(bytes) = read_local_metadata_file_with_limit(
            &path,
            INBOUND_BLOB_REPAIR_LEDGER_MAX_BYTES,
            "inbound blob repair ledger",
        )?
        else {
            return Ok(InboundBlobRepairLedger::default());
        };
        let Ok(mut ledger) = serde_json::from_slice::<InboundBlobRepairLedger>(&bytes) else {
            return Ok(InboundBlobRepairLedger::default());
        };
        if ledger.schema_version != INBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION {
            return Ok(InboundBlobRepairLedger::default());
        }
        if ledger.workspaces.len() > INBOUND_BLOB_REPAIR_MAX_WORKSPACES {
            let remove_count = ledger.workspaces.len() - INBOUND_BLOB_REPAIR_MAX_WORKSPACES;
            ledger.workspaces.drain(0..remove_count);
        }
        for workspace in &mut ledger.workspaces {
            workspace
                .pending_blob_hashes
                .retain(|hash| valid_blob_hash_for_repair_ledger(hash));
            workspace
                .pending_blob_hashes
                .truncate(INBOUND_BLOB_REPAIR_MAX_HASHES_PER_WORKSPACE);
        }
        Ok(ledger)
    }

    fn write_inbound_blob_repair_ledger(
        &self,
        ledger: &InboundBlobRepairLedger,
    ) -> Result<(), RuntimeError> {
        let path = self.paths.data_dir.join(INBOUND_BLOB_REPAIR_LEDGER_FILE);
        let bytes = serde_json::to_vec(ledger)?;
        write_derived_cache_file(&path, &bytes)
    }

    pub async fn sync_workspace_with_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<SyncedWorkspace, RuntimeError>
    where
        T: ChaftTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        let initial_plan = plan_workspace_sync(&self.store, &workspace_id, remote_event_ids)?;
        let mut published = if initial_plan.publish_event_ids().is_empty() {
            Self::empty_published_workspace(workspace_id.clone())
        } else {
            self.publish_workspace_to_peer_with_plan(
                transport,
                peer,
                workspace_id.clone(),
                &initial_plan,
            )
            .await?
        };
        let report = pull_workspace_from_peer_with_plan(
            transport,
            peer,
            &self.store,
            workspace_id.clone(),
            &initial_plan,
        )
        .await?;
        let pulled = self.finish_workspace_pull(workspace_id.clone(), report)?;
        if pulled.has_local_generated_events() {
            let followup_plan = self.followup_sync_plan(&workspace_id, &initial_plan)?;
            let followup = self
                .publish_workspace_to_peer_with_plan(
                    transport,
                    peer,
                    workspace_id.clone(),
                    &followup_plan,
                )
                .await?;
            merge_published_workspace(&mut published, followup);
        }
        Ok(SyncedWorkspace {
            workspace_id: workspace_id.0,
            published,
            pulled,
        })
    }

    pub async fn sync_workspace_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: WorkspaceId,
    ) -> Result<SyncedWorkspace, RuntimeError>
    where
        T: ChaftTransport + AuthorizedPublishTransport + BlobSyncTransport,
    {
        validate_workspace_id_reference(&workspace_id)?;
        validate_peer_address(peer)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        let initial_plan = plan_workspace_sync(&self.store, &workspace_id, remote_event_ids)?;
        let outbound_blob_repair_due =
            self.outbound_blob_full_repair_due(&workspace_id, peer, initial_plan.local_event_ids());
        let ran_outbound_blob_scan =
            !initial_plan.publish_event_ids().is_empty() || outbound_blob_repair_due;
        let mut published = if !ran_outbound_blob_scan {
            Self::empty_published_workspace(workspace_id.clone())
        } else {
            let published = self
                .publish_workspace_direct_with_plan(
                    transport,
                    peer,
                    workspace_id.clone(),
                    &initial_plan,
                )
                .await?;
            self.record_outbound_blob_full_repair(
                &workspace_id,
                peer,
                initial_plan.local_event_ids(),
            )?;
            published
        };
        let pulled = self
            .pull_workspace_direct_with_plan(
                transport,
                peer,
                workspace_id.clone(),
                &initial_plan,
                false,
            )
            .await?;
        if pulled.has_local_generated_events() {
            let followup_plan = self.followup_sync_plan(&workspace_id, &initial_plan)?;
            let followup = self
                .publish_workspace_direct_with_plan(
                    transport,
                    peer,
                    workspace_id.clone(),
                    &followup_plan,
                )
                .await?;
            self.record_outbound_blob_full_repair(
                &workspace_id,
                peer,
                followup_plan.local_event_ids(),
            )?;
            merge_published_workspace(&mut published, followup);
        }
        if self.has_pending_outbound_blob_transfers(&workspace_id)? {
            let retry = self
                .retry_pending_blob_transfers_direct(
                    transport,
                    workspace_id.clone(),
                    std::slice::from_ref(peer),
                )
                .await?;
            Self::merge_blob_retry_into_published(&mut published, retry);
        }

        Ok(SyncedWorkspace {
            workspace_id: workspace_id.0,
            published,
            pulled,
        })
    }

    fn empty_published_workspace(workspace_id: WorkspaceId) -> PublishedWorkspace {
        PublishedWorkspace::from_parts(
            workspace_id.0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn followup_sync_plan(
        &self,
        workspace_id: &WorkspaceId,
        initial_plan: &WorkspaceSyncPlan,
    ) -> Result<WorkspaceSyncPlan, RuntimeError> {
        // Treat the initial local inventory as already considered by the first
        // publish pass. This makes the follow-up plan contain only events that
        // local pull finalization generated, while still avoiding another
        // remote inventory request.
        let known_remote_event_ids = initial_plan
            .remote_event_ids()
            .iter()
            .chain(initial_plan.local_event_ids())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        plan_workspace_sync(&self.store, workspace_id, known_remote_event_ids).map_err(Into::into)
    }

    fn has_pending_outbound_blob_transfers(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<bool, RuntimeError> {
        Ok(self
            .read_blob_transfer_ledger()?
            .entries
            .iter()
            .any(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.status != BlobTransferStatus::Succeeded
            }))
    }

    fn outbound_blob_full_repair_due(
        &self,
        workspace_id: &WorkspaceId,
        peer: &PeerAddress,
        local_event_ids: &[EventId],
    ) -> bool {
        let _guard = OUTBOUND_BLOB_REPAIR_LEDGER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ledger = self.read_outbound_blob_repair_ledger().unwrap_or_default();
        let expected_fingerprint = event_inventory_fingerprint(local_event_ids);
        ledger
            .peers
            .iter()
            .find(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.peer_id == peer.peer_id.0
                    && entry.peer_endpoint == peer.endpoint
            })
            .is_none_or(|entry| {
                entry.local_inventory_fingerprint != expected_fingerprint
                    || now_unix_ms().saturating_sub(entry.last_full_scan_unix_ms)
                        >= OUTBOUND_BLOB_FULL_REPAIR_INTERVAL_MS
            })
    }

    fn record_outbound_blob_full_repair(
        &self,
        workspace_id: &WorkspaceId,
        peer: &PeerAddress,
        local_event_ids: &[EventId],
    ) -> Result<(), RuntimeError> {
        let _guard = OUTBOUND_BLOB_REPAIR_LEDGER_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut ledger = self.read_outbound_blob_repair_ledger().unwrap_or_default();
        let entry = match ledger.peers.iter_mut().find(|entry| {
            entry.workspace_id == workspace_id.0
                && entry.peer_id == peer.peer_id.0
                && entry.peer_endpoint == peer.endpoint
        }) {
            Some(entry) => entry,
            None => {
                if ledger.peers.len() >= OUTBOUND_BLOB_REPAIR_MAX_PEERS {
                    ledger.peers.remove(0);
                }
                ledger.peers.push(OutboundBlobRepairPeer {
                    workspace_id: workspace_id.0.clone(),
                    peer_id: peer.peer_id.0.clone(),
                    peer_endpoint: peer.endpoint.clone(),
                    ..OutboundBlobRepairPeer::default()
                });
                ledger.peers.last_mut().expect("peer was inserted")
            }
        };
        entry.local_inventory_fingerprint = event_inventory_fingerprint(local_event_ids);
        entry.last_full_scan_unix_ms = now_unix_ms();
        // This file only schedules a repeatable repair scan. If persistence is
        // unavailable, leave the scan due rather than failing event sync.
        let _ = self.write_outbound_blob_repair_ledger(&ledger);
        Ok(())
    }

    fn read_outbound_blob_repair_ledger(&self) -> Result<OutboundBlobRepairLedger, RuntimeError> {
        let path = self.paths.data_dir.join(OUTBOUND_BLOB_REPAIR_LEDGER_FILE);
        let Some(bytes) = read_local_metadata_file_with_limit(
            &path,
            OUTBOUND_BLOB_REPAIR_LEDGER_MAX_BYTES,
            "outbound blob repair ledger",
        )?
        else {
            return Ok(OutboundBlobRepairLedger::default());
        };
        let Ok(mut ledger) = serde_json::from_slice::<OutboundBlobRepairLedger>(&bytes) else {
            return Ok(OutboundBlobRepairLedger::default());
        };
        if ledger.schema_version != OUTBOUND_BLOB_REPAIR_LEDGER_SCHEMA_VERSION {
            return Ok(OutboundBlobRepairLedger::default());
        }
        if ledger.peers.len() > OUTBOUND_BLOB_REPAIR_MAX_PEERS {
            let remove_count = ledger.peers.len() - OUTBOUND_BLOB_REPAIR_MAX_PEERS;
            ledger.peers.drain(0..remove_count);
        }
        Ok(ledger)
    }

    fn write_outbound_blob_repair_ledger(
        &self,
        ledger: &OutboundBlobRepairLedger,
    ) -> Result<(), RuntimeError> {
        let path = self.paths.data_dir.join(OUTBOUND_BLOB_REPAIR_LEDGER_FILE);
        let bytes = serde_json::to_vec(ledger)?;
        write_derived_cache_file(&path, &bytes)
    }

    fn merge_blob_retry_into_published(
        published: &mut PublishedWorkspace,
        retry: BlobTransferRetryReport,
    ) {
        let mut published_hashes = published
            .published_blob_hashes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for blob_hash in retry.retried_blob_hashes {
            if published_hashes.insert(blob_hash.clone()) {
                published.published_blob_hashes.push(blob_hash);
            }
        }
        let mut missing_hashes = published
            .missing_blob_hashes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        for blob_hash in retry.missing_blob_hashes {
            if missing_hashes.insert(blob_hash.clone()) {
                published.missing_blob_hashes.push(blob_hash);
            }
        }
        published
            .blob_transfer_attempts
            .extend(retry.blob_transfer_attempts);
        published.refresh_counts();
    }

    fn pulled_workspace_from_report(
        workspace_id: WorkspaceId,
        report: PullSyncReport,
    ) -> PulledWorkspace {
        let mut pulled = PulledWorkspace {
            workspace_id: workspace_id.0,
            requested_event_count: 0,
            requested_event_ids: report
                .requested_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            fetched_event_count: 0,
            fetched_event_ids: report
                .fetched_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            fetched_blob_count: 0,
            fetched_blob_hashes: Vec::new(),
            missing_blob_count: 0,
            missing_blob_hashes: Vec::new(),
            ignored_event_count: 0,
            ignored_event_ids: report
                .ignored_event_ids
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            applied_event_count: 0,
            applied_event_ids: report
                .materialization
                .applied_events
                .into_iter()
                .map(|event_id| event_id.0)
                .collect(),
            invite_profile_event_count: 0,
            invite_profile_event_ids: Vec::new(),
            openmls_catchup: PulledOpenMlsCatchup::default(),
            compromise_response: None,
            gap_count: 0,
            gaps: report
                .materialization
                .gaps
                .into_iter()
                .map(|gap| PulledWorkspaceGap {
                    event_id: gap.event_id.0,
                    missing_parent_ids: gap
                        .missing_parent_ids
                        .into_iter()
                        .map(|event_id| event_id.0)
                        .collect(),
                })
                .collect(),
        };
        pulled.refresh_counts();
        pulled
    }

    pub fn reconcile_openmls_access(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<PulledOpenMlsCatchup, RuntimeError> {
        self.reconcile_openmls_access_with_membership_hint(workspace_id, None)
    }

    fn reconcile_openmls_access_with_membership_hint(
        &self,
        workspace_id: WorkspaceId,
        local_membership_hint: Option<bool>,
    ) -> Result<PulledOpenMlsCatchup, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let start_snapshot = self.openmls_reconcile_snapshot(&workspace_id);
        if start_snapshot.as_ref().is_some_and(|snapshot| {
            self.openmls_reconcile_checkpoint_matches(&workspace_id, snapshot)
        }) {
            return Ok(PulledOpenMlsCatchup::default());
        }

        let catchup =
            self.reconcile_openmls_access_uncached(workspace_id.clone(), local_membership_hint)?;
        if catchup.provisioning_errors.is_empty()
            && self.openmls_workspace_auto_provisioning_is_stable(&workspace_id)
            && let (Some(start_snapshot), Some(end_snapshot)) = (
                start_snapshot,
                self.openmls_reconcile_snapshot(&workspace_id),
            )
        {
            let generated_event_ids = openmls_reconcile_generated_event_ids(&catchup);
            // Reconciliation legitimately mutates MLS secret state while
            // joining, applying, or provisioning. Cache only a stable
            // secret snapshot; the next pass after such work can record
            // it. This also prevents a concurrent, unprocessed secret
            // change from being accepted as reconciled.
            if start_snapshot.local_secret_fingerprint == end_snapshot.local_secret_fingerprint
                && openmls_reconcile_end_inventory_is_expected(
                    &start_snapshot.event_ids,
                    &end_snapshot.event_ids,
                    &generated_event_ids,
                )
            {
                self.record_openmls_reconcile_checkpoint(&workspace_id, &end_snapshot);
            }
        }
        Ok(catchup)
    }

    fn openmls_workspace_auto_provisioning_is_stable(&self, workspace_id: &WorkspaceId) -> bool {
        if !self.openmls_workspace_group_path(workspace_id).exists() {
            return true;
        }
        let Ok(events) = self.materialized_workspace_events(workspace_id) else {
            return false;
        };
        let mut state = WorkspaceState::new(workspace_id.clone());
        if state.apply_batch(&events).is_err() {
            return false;
        }
        if !state.members.contains_key(self.identity.device_id())
            || workspace_creator_device_id_from_events(&events).as_ref()
                != Some(self.identity.device_id())
        {
            return true;
        }
        let Ok(Some(local_group_id)) = self.local_openmls_workspace_group_id(workspace_id) else {
            return false;
        };
        let index = OpenMlsAutoProvisionIndex::from_events(&events);
        !state.members.keys().any(|device_id| {
            device_id != self.identity.device_id()
                && !index.workspace_device_is_revoked(device_id)
                && !index.workspace_group_has_device_in_group(&local_group_id, device_id)
                && index
                    .latest_unused_key_package_id_for_device(device_id)
                    .is_some()
        })
    }

    fn reconcile_openmls_access_uncached(
        &self,
        workspace_id: WorkspaceId,
        local_membership_hint: Option<bool>,
    ) -> Result<PulledOpenMlsCatchup, RuntimeError> {
        let has_local_group_state = self.openmls_workspace_group_path(&workspace_id).exists()
            || !self
                .local_openmls_channel_group_ids(&workspace_id)?
                .is_empty();
        if local_membership_hint == Some(false) && !has_local_group_state {
            return Ok(PulledOpenMlsCatchup::default());
        }

        // A replica or pre-authorization runtime cannot publish packages,
        // join groups, or provision members. Detect that once up front so a
        // pull does not repeatedly materialize the same history through each
        // reconciliation helper.
        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        if !state.members.contains_key(self.identity.device_id()) && !has_local_group_state {
            return Ok(PulledOpenMlsCatchup::default());
        }

        let published_key_package_event_ids = self
            .ensure_openmls_device_key_packages(workspace_id.clone())?
            .into_iter()
            .map(|published| published.event_id)
            .collect();
        let mut catchup = PulledOpenMlsCatchup {
            published_key_package_event_ids,
            ..PulledOpenMlsCatchup::default()
        };

        if !self.openmls_workspace_group_path(&workspace_id).exists() {
            match self.join_openmls_workspace_group(workspace_id.clone(), None) {
                Ok(joined) => catchup.workspace_joined_event_id = Some(joined.source_event_id),
                Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { .. })
                | Err(RuntimeError::OpenMlsWorkspaceGroupInviteNotFound { .. })
                | Err(RuntimeError::OpenMlsPrivateKeyPackageMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        if self.openmls_workspace_group_path(&workspace_id).exists() {
            match self.apply_openmls_workspace_group_commits(workspace_id.clone(), None) {
                Ok(applied) => {
                    catchup.workspace_applied_event_ids = applied.applied_event_ids;
                    catchup.workspace_self_removed = applied.self_removed;
                }
                Err(RuntimeError::OpenMlsWorkspaceGroupMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        for channel_id in self.joinable_openmls_channel_group_ids(&workspace_id)? {
            match self.join_openmls_channel_group(workspace_id.clone(), channel_id.clone(), None) {
                Ok(joined) => {
                    catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                        channel_id: channel_id.0,
                        event_count: 0,
                        joined_event_id: Some(joined.source_event_id),
                        applied_event_ids: Vec::new(),
                        provisioned_event_ids: Vec::new(),
                        self_removed: false,
                    });
                }
                Err(RuntimeError::OpenMlsChannelGroupAlreadyExists { .. })
                | Err(RuntimeError::OpenMlsChannelGroupInviteNotFound { .. })
                | Err(RuntimeError::OpenMlsPrivateKeyPackageMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        for channel_id in self.local_openmls_channel_group_ids(&workspace_id)? {
            match self.apply_openmls_channel_group_commits(
                workspace_id.clone(),
                channel_id.clone(),
                None,
            ) {
                Ok(applied) => {
                    let channel_id_string = channel_id.0;
                    let Some(existing) = catchup
                        .channel_groups
                        .iter_mut()
                        .find(|group| group.channel_id == channel_id_string)
                    else {
                        if applied.applied_event_ids.is_empty() && !applied.self_removed {
                            continue;
                        }
                        catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                            channel_id: channel_id_string,
                            event_count: 0,
                            joined_event_id: None,
                            applied_event_ids: applied.applied_event_ids,
                            provisioned_event_ids: Vec::new(),
                            self_removed: applied.self_removed,
                        });
                        continue;
                    };
                    existing.applied_event_ids = applied.applied_event_ids;
                    existing.self_removed |= applied.self_removed;
                }
                Err(RuntimeError::OpenMlsChannelGroupMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        catchup.workspace_provisioned_event_ids =
            self.auto_provision_openmls_workspace_members(&workspace_id);
        let (created_channel_group_ids, provisioned_channel_members) =
            self.auto_provision_openmls_channel_members(&workspace_id, &events)?;
        catchup.created_channel_group_ids = created_channel_group_ids;
        for provisioned in provisioned_channel_members {
            catchup
                .channel_provisioning_outcomes
                .extend(provisioned.outcomes.clone());
            catchup.provisioning_errors.extend(
                provisioned
                    .outcomes
                    .iter()
                    .filter_map(|outcome| outcome.provisioning_error.clone()),
            );
            let Some(existing) = catchup
                .channel_groups
                .iter_mut()
                .find(|group| group.channel_id == provisioned.channel_id)
            else {
                catchup.channel_groups.push(PulledOpenMlsChannelCatchup {
                    channel_id: provisioned.channel_id,
                    event_count: 0,
                    joined_event_id: None,
                    applied_event_ids: Vec::new(),
                    provisioned_event_ids: provisioned.event_ids,
                    self_removed: false,
                });
                continue;
            };
            existing.provisioned_event_ids.extend(provisioned.event_ids);
        }

        catchup.refresh_counts();
        Ok(catchup)
    }

    fn openmls_reconcile_snapshot(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Option<OpenMlsReconcileSnapshot> {
        let event_ids = self
            .store
            .list_servable_event_ids_for_workspace(&workspace_id.0)
            .ok()?;
        let event_inventory_fingerprint = event_inventory_fingerprint(&event_ids);
        let workspace_keys_dir = self.paths.workspace_keys_dir.join(&workspace_id.0);
        let local_secret_fingerprint = openmls_reconcile_secret_fingerprint(&workspace_keys_dir)?;
        Some(OpenMlsReconcileSnapshot {
            event_ids,
            event_inventory_fingerprint,
            local_secret_fingerprint,
        })
    }

    fn openmls_reconcile_checkpoint_matches(
        &self,
        workspace_id: &WorkspaceId,
        snapshot: &OpenMlsReconcileSnapshot,
    ) -> bool {
        let _guard = OPENMLS_RECONCILE_CHECKPOINT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let ledger = self
            .read_openmls_reconcile_checkpoint_ledger()
            .unwrap_or_default();
        ledger.entries.iter().any(|entry| {
            entry.workspace_id == workspace_id.0
                && entry.device_id == self.identity.device_id().0
                && entry.event_inventory_fingerprint == snapshot.event_inventory_fingerprint
                && entry.local_secret_fingerprint == snapshot.local_secret_fingerprint
        })
    }

    fn record_openmls_reconcile_checkpoint(
        &self,
        workspace_id: &WorkspaceId,
        snapshot: &OpenMlsReconcileSnapshot,
    ) {
        let _guard = OPENMLS_RECONCILE_CHECKPOINT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut ledger = self
            .read_openmls_reconcile_checkpoint_ledger()
            .unwrap_or_default();
        ledger.entries.retain(|entry| {
            entry.workspace_id != workspace_id.0 || entry.device_id != self.identity.device_id().0
        });
        if ledger.entries.len() >= OPENMLS_RECONCILE_CHECKPOINT_MAX_ENTRIES {
            let remove_count = ledger.entries.len() + 1 - OPENMLS_RECONCILE_CHECKPOINT_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        ledger.entries.push(OpenMlsReconcileCheckpointEntry {
            workspace_id: workspace_id.0.clone(),
            device_id: self.identity.device_id().0.clone(),
            event_inventory_fingerprint: snapshot.event_inventory_fingerprint.clone(),
            local_secret_fingerprint: snapshot.local_secret_fingerprint.clone(),
        });
        // This checkpoint only suppresses repeatable derived work. A corrupt,
        // oversized, or unwritable cache must leave reconciliation enabled.
        let _ = self.write_openmls_reconcile_checkpoint_ledger(&ledger);
    }

    fn read_openmls_reconcile_checkpoint_ledger(
        &self,
    ) -> Result<OpenMlsReconcileCheckpointLedger, RuntimeError> {
        let path = self.paths.data_dir.join(OPENMLS_RECONCILE_CHECKPOINT_FILE);
        let Some(bytes) = read_local_metadata_file_with_limit(
            &path,
            OPENMLS_RECONCILE_CHECKPOINT_MAX_BYTES,
            "OpenMLS reconcile checkpoint",
        )?
        else {
            return Ok(OpenMlsReconcileCheckpointLedger::default());
        };
        let Ok(mut ledger) = serde_json::from_slice::<OpenMlsReconcileCheckpointLedger>(&bytes)
        else {
            return Ok(OpenMlsReconcileCheckpointLedger::default());
        };
        if ledger.schema_version != OPENMLS_RECONCILE_CHECKPOINT_SCHEMA_VERSION {
            return Ok(OpenMlsReconcileCheckpointLedger::default());
        }
        if ledger.entries.len() > OPENMLS_RECONCILE_CHECKPOINT_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - OPENMLS_RECONCILE_CHECKPOINT_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        Ok(ledger)
    }

    fn write_openmls_reconcile_checkpoint_ledger(
        &self,
        ledger: &OpenMlsReconcileCheckpointLedger,
    ) -> Result<(), RuntimeError> {
        let path = self.paths.data_dir.join(OPENMLS_RECONCILE_CHECKPOINT_FILE);
        let bytes = serde_json::to_vec(ledger)?;
        write_derived_cache_file(&path, &bytes)
    }

    async fn retry_blob_transfer_to_peer<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        workspace_id: &str,
        blob_hash: &str,
        bytes: Vec<u8>,
        remote_availability: Option<&BlobAvailability>,
    ) -> Result<(BlobTransferAttempt, bool), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        if bytes.len() > DIRECT_WHOLE_BLOB_SYNC_LIMIT {
            let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
                planned_chunk_upload(&bytes, remote_availability);
            let attempt = self.record_blob_transfer_started(
                workspace_id,
                peer,
                blob_hash,
                BlobTransferMode::ChunkedBlob,
                bytes.len() as u64,
                Some(chunk_size),
                chunk_hashes,
                planned_chunk_hashes,
                remote_available_chunk_hashes,
            )?;
            return match transport
                .put_blob_chunked(peer, bytes, DIRECT_BLOB_CHUNK_SIZE)
                .await
            {
                Ok(_) => self
                    .record_blob_transfer_finished(&attempt, BlobTransferStatus::Succeeded, None)
                    .map(|attempt| (attempt, false)),
                Err(error) => {
                    let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                    self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Failed,
                        Some(error.to_string()),
                    )
                    .map(|attempt| (attempt, suspect_protocol_error))
                }
            };
        }

        let attempt = self.record_blob_transfer_started(
            workspace_id,
            peer,
            blob_hash,
            BlobTransferMode::WholeBlob,
            bytes.len() as u64,
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )?;
        match transport.put_blobs(peer, vec![bytes]).await {
            Ok(_) => self
                .record_blob_transfer_finished(&attempt, BlobTransferStatus::Succeeded, None)
                .map(|attempt| (attempt, false)),
            Err(error) => {
                let suspect_protocol_error = matches!(error, NetError::Protocol(_));
                self.record_blob_transfer_finished(
                    &attempt,
                    BlobTransferStatus::Failed,
                    Some(error.to_string()),
                )
                .map(|attempt| (attempt, suspect_protocol_error))
            }
        }
    }

    async fn publish_materialized_event_blobs_direct<T>(
        &self,
        transport: &T,
        peer: &PeerAddress,
        events: &[SignedEvent],
        published: &mut PublishedWorkspace,
    ) -> Result<(), RuntimeError>
    where
        T: BlobSyncTransport,
    {
        let blob_hashes = attachment_blob_hashes(events);
        if blob_hashes.is_empty() {
            return Ok(());
        }

        let blob_store = self.open_blob_store()?;
        let remote_blob_availability = transport
            .fetch_blob_availabilities(peer, blob_hashes.clone())
            .await?;
        let mut blobs_to_publish = Vec::new();
        let mut chunked_blobs_to_publish = Vec::new();

        for blob_hash in blob_hashes {
            if remote_blob_availability
                .get(&blob_hash)
                .is_some_and(|availability| availability.is_complete())
            {
                published.blob_transfer_attempts.extend(
                    self.reconcile_completed_blob_transfer_attempts(
                        &published.workspace_id,
                        peer,
                        &blob_hash,
                    )?,
                );
                continue;
            }
            match blob_store.get_complete_bytes(&blob_hash)? {
                Some(bytes) if bytes.len() > DIRECT_WHOLE_BLOB_SYNC_LIMIT => {
                    chunked_blobs_to_publish.push((blob_hash, bytes));
                }
                Some(bytes) => blobs_to_publish.push((blob_hash, bytes)),
                None => published.missing_blob_hashes.push(blob_hash),
            }
        }
        if !blobs_to_publish.is_empty() {
            let attempts = blobs_to_publish
                .iter()
                .map(|(blob_hash, bytes)| {
                    self.record_blob_transfer_started(
                        &published.workspace_id,
                        peer,
                        blob_hash,
                        BlobTransferMode::WholeBlob,
                        bytes.len() as u64,
                        None,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            match transport
                .put_blobs(
                    peer,
                    blobs_to_publish
                        .iter()
                        .map(|(_, bytes)| bytes.clone())
                        .collect(),
                )
                .await
            {
                Ok(_) => {
                    for attempt in &attempts {
                        let finished = self.record_blob_transfer_finished(
                            attempt,
                            BlobTransferStatus::Succeeded,
                            None,
                        )?;
                        published.blob_transfer_attempts.push(finished);
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    for attempt in &attempts {
                        let finished = self.record_blob_transfer_finished(
                            attempt,
                            BlobTransferStatus::Failed,
                            Some(message.clone()),
                        )?;
                        published.blob_transfer_attempts.push(finished);
                    }
                    return Err(error.into());
                }
            }
            published
                .published_blob_hashes
                .extend(blobs_to_publish.into_iter().map(|(hash, _)| hash));
        }
        for (blob_hash, bytes) in chunked_blobs_to_publish {
            let (chunk_size, chunk_hashes, planned_chunk_hashes, remote_available_chunk_hashes) =
                planned_chunk_upload(&bytes, remote_blob_availability.get(&blob_hash));
            let attempt = self.record_blob_transfer_started(
                &published.workspace_id,
                peer,
                &blob_hash,
                BlobTransferMode::ChunkedBlob,
                bytes.len() as u64,
                Some(chunk_size),
                chunk_hashes,
                planned_chunk_hashes,
                remote_available_chunk_hashes,
            )?;
            match transport
                .put_blob_chunked(peer, bytes, DIRECT_BLOB_CHUNK_SIZE)
                .await
            {
                Ok(_) => {
                    let finished = self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Succeeded,
                        None,
                    )?;
                    published.blob_transfer_attempts.push(finished);
                }
                Err(error) => {
                    let message = error.to_string();
                    let finished = self.record_blob_transfer_finished(
                        &attempt,
                        BlobTransferStatus::Failed,
                        Some(message),
                    )?;
                    published.blob_transfer_attempts.push(finished);
                    return Err(error.into());
                }
            }
            published.published_blob_hashes.push(blob_hash);
        }

        Ok(())
    }
}

fn valid_blob_hash_for_repair_ledger(hash: &str) -> bool {
    hash.len() == blake3::OUT_LEN * 2
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn event_inventory_fingerprint(event_ids: &[EventId]) -> String {
    let mut hasher = blake3::Hasher::new();
    for event_id in event_ids {
        hasher.update(&(event_id.0.len() as u64).to_le_bytes());
        hasher.update(event_id.0.as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

fn openmls_reconcile_generated_event_ids(catchup: &PulledOpenMlsCatchup) -> Vec<EventId> {
    catchup
        .published_key_package_event_ids
        .iter()
        .chain(catchup.workspace_provisioned_event_ids.iter())
        .chain(
            catchup
                .channel_groups
                .iter()
                .flat_map(|group| group.provisioned_event_ids.iter()),
        )
        .cloned()
        .map(EventId)
        .collect()
}

fn openmls_reconcile_end_inventory_is_expected(
    start_event_ids: &[EventId],
    end_event_ids: &[EventId],
    generated_event_ids: &[EventId],
) -> bool {
    if !end_event_ids.starts_with(start_event_ids) {
        return false;
    }

    let start_ids = start_event_ids.iter().collect::<BTreeSet<_>>();
    if start_ids.len() != start_event_ids.len() {
        return false;
    }
    let generated_ids = generated_event_ids.iter().collect::<BTreeSet<_>>();
    if generated_ids.len() != generated_event_ids.len()
        || generated_ids
            .iter()
            .any(|event_id| start_ids.contains(event_id))
        || end_event_ids.len() != start_event_ids.len() + generated_event_ids.len()
    {
        return false;
    }

    let end_ids = end_event_ids.iter().collect::<BTreeSet<_>>();
    end_ids.len() == end_event_ids.len()
        && end_ids == start_ids.union(&generated_ids).copied().collect()
}

fn openmls_reconcile_secret_fingerprint(workspace_keys_dir: &Path) -> Option<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"chaft-openmls-reconcile-secret-fingerprint-v1");
    let mut file_count = 0usize;
    let mut total_bytes = 0usize;

    hash_openmls_secret_directory(
        &mut hasher,
        b"key-packages",
        &workspace_keys_dir.join("mls-key-packages"),
        &mut file_count,
        &mut total_bytes,
    )?;
    hash_openmls_secret_file_slot(
        &mut hasher,
        b"workspace-group",
        &workspace_keys_dir.join("mls-groups").join("workspace.json"),
        &mut file_count,
        &mut total_bytes,
    )?;
    hash_openmls_secret_directory(
        &mut hasher,
        b"channel-groups",
        &workspace_keys_dir.join("mls-groups").join("channels"),
        &mut file_count,
        &mut total_bytes,
    )?;

    Some(hasher.finalize().to_hex().to_string())
}

fn hash_openmls_secret_directory(
    hasher: &mut blake3::Hasher,
    label: &[u8],
    directory: &Path,
    file_count: &mut usize,
    total_bytes: &mut usize,
) -> Option<()> {
    hash_framed_bytes(hasher, label);
    let before = match fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hasher.update(&[0]);
            return Some(());
        }
        Err(_) => return None,
    };
    if !before.file_type().is_dir() {
        return None;
    }
    let before_modified = metadata_modified_stamp(&before)?;
    hasher.update(&[1]);
    hash_metadata_stamp(hasher, &before, before_modified);

    let mut paths = fs::read_dir(directory)
        .ok()?
        .map(|entry| entry.map(|entry| entry.path()).ok())
        .collect::<Option<Vec<_>>>()?;
    paths.sort();
    if file_count.saturating_add(paths.len()) > OPENMLS_RECONCILE_SECRET_MAX_FILES {
        return None;
    }
    for path in paths {
        let name = path.file_name()?.as_encoded_bytes();
        hash_framed_bytes(hasher, name);
        hash_openmls_secret_file(hasher, &path, file_count, total_bytes)?;
    }

    let after = fs::symlink_metadata(directory).ok()?;
    if !after.file_type().is_dir()
        || metadata_modified_stamp(&after)? != before_modified
        || after.len() != before.len()
    {
        return None;
    }
    Some(())
}

fn hash_openmls_secret_file_slot(
    hasher: &mut blake3::Hasher,
    label: &[u8],
    path: &Path,
    file_count: &mut usize,
    total_bytes: &mut usize,
) -> Option<()> {
    hash_framed_bytes(hasher, label);
    match fs::symlink_metadata(path) {
        Ok(_) => {
            hasher.update(&[1]);
            hash_openmls_secret_file(hasher, path, file_count, total_bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hasher.update(&[0]);
            Some(())
        }
        Err(_) => None,
    }
}

fn hash_openmls_secret_file(
    hasher: &mut blake3::Hasher,
    path: &Path,
    file_count: &mut usize,
    total_bytes: &mut usize,
) -> Option<()> {
    let before = fs::symlink_metadata(path).ok()?;
    if !before.file_type().is_file() {
        return None;
    }
    let before_modified = metadata_modified_stamp(&before)?;
    let expected_len = usize::try_from(before.len()).ok()?;
    if file_count.saturating_add(1) > OPENMLS_RECONCILE_SECRET_MAX_FILES
        || total_bytes.saturating_add(expected_len) > OPENMLS_RECONCILE_SECRET_MAX_TOTAL_BYTES
    {
        return None;
    }

    hash_metadata_stamp(hasher, &before, before_modified);
    let mut file = File::open(path).ok()?;
    let mut read_len = 0usize;
    let mut buffer = [0u8; 32 * 1024];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        read_len = read_len.checked_add(count)?;
        if read_len > expected_len {
            return None;
        }
        hasher.update(&buffer[..count]);
    }
    if read_len != expected_len {
        return None;
    }

    let after = fs::symlink_metadata(path).ok()?;
    if !after.file_type().is_file()
        || after.len() != before.len()
        || metadata_modified_stamp(&after)? != before_modified
    {
        return None;
    }
    *file_count += 1;
    *total_bytes += read_len;
    Some(())
}

fn metadata_modified_stamp(metadata: &Metadata) -> Option<(u64, u32)> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((duration.as_secs(), duration.subsec_nanos()))
}

fn hash_metadata_stamp(hasher: &mut blake3::Hasher, metadata: &Metadata, modified: (u64, u32)) {
    hasher.update(&metadata.len().to_le_bytes());
    hasher.update(&modified.0.to_le_bytes());
    hasher.update(&modified.1.to_le_bytes());
}

fn hash_framed_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;

    #[test]
    fn ordered_event_inventory_fingerprint_detects_same_count_replacement_and_reorder() {
        let original = vec![EventId("evt_a".to_owned()), EventId("evt_b".to_owned())];
        let replaced = vec![EventId("evt_a".to_owned()), EventId("evt_c".to_owned())];
        let reordered = vec![EventId("evt_b".to_owned()), EventId("evt_a".to_owned())];

        assert_ne!(
            event_inventory_fingerprint(&original),
            event_inventory_fingerprint(&replaced)
        );
        assert_ne!(
            event_inventory_fingerprint(&original),
            event_inventory_fingerprint(&reordered)
        );
    }

    #[test]
    fn checkpoint_toctou_predicate_accepts_only_reported_local_appends() {
        let start = vec![EventId("evt_a".to_owned()), EventId("evt_b".to_owned())];
        let generated = vec![EventId("evt_generated".to_owned())];
        let expected_end = vec![
            EventId("evt_a".to_owned()),
            EventId("evt_b".to_owned()),
            EventId("evt_generated".to_owned()),
        ];
        assert!(openmls_reconcile_end_inventory_is_expected(
            &start,
            &expected_end,
            &generated
        ));

        let concurrent_append = vec![
            EventId("evt_a".to_owned()),
            EventId("evt_b".to_owned()),
            EventId("evt_concurrent".to_owned()),
            EventId("evt_generated".to_owned()),
        ];
        assert!(!openmls_reconcile_end_inventory_is_expected(
            &start,
            &concurrent_append,
            &generated
        ));

        let replaced_during_reconcile = vec![
            EventId("evt_a".to_owned()),
            EventId("evt_replaced".to_owned()),
            EventId("evt_generated".to_owned()),
        ];
        assert!(!openmls_reconcile_end_inventory_is_expected(
            &start,
            &replaced_during_reconcile,
            &generated
        ));
    }

    #[test]
    fn openmls_secret_fingerprint_detects_corruption_and_deletion() {
        let tempdir = tempfile::tempdir().unwrap();
        let workspace_keys_dir = tempdir.path().join("workspace");
        let package_dir = workspace_keys_dir.join("mls-key-packages");
        let channel_dir = workspace_keys_dir.join("mls-groups").join("channels");
        fs::create_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&channel_dir).unwrap();
        let package_path = package_dir.join("package.json");
        let group_path = channel_dir.join("channel.json");
        fs::write(&package_path, b"package-state").unwrap();
        fs::write(&group_path, b"group-state").unwrap();
        let original = openmls_reconcile_secret_fingerprint(&workspace_keys_dir).unwrap();

        fs::write(&package_path, b"package-corrupt").unwrap();
        let corrupted = openmls_reconcile_secret_fingerprint(&workspace_keys_dir).unwrap();
        assert_ne!(original, corrupted);

        fs::remove_file(&group_path).unwrap();
        let deleted = openmls_reconcile_secret_fingerprint(&workspace_keys_dir).unwrap();
        assert_ne!(corrupted, deleted);
    }

    #[test]
    fn stable_reconcile_records_checkpoint_and_secret_damage_invalidates_it() {
        let tempdir = tempfile::tempdir().unwrap();
        let runtime = LocalRuntime::open(tempdir.path(), None).unwrap();
        let created = runtime
            .create_workspace("OpenMLS checkpoint", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        let first = runtime
            .reconcile_openmls_access(workspace_id.clone())
            .unwrap();
        assert_eq!(first.published_key_package_event_ids.len(), 4);
        let second = runtime
            .reconcile_openmls_access(workspace_id.clone())
            .unwrap();
        assert_eq!(second.event_count, 0);
        let stable_snapshot = runtime.openmls_reconcile_snapshot(&workspace_id).unwrap();
        assert!(runtime.openmls_reconcile_checkpoint_matches(&workspace_id, &stable_snapshot));
        let third = runtime
            .reconcile_openmls_access(workspace_id.clone())
            .unwrap();
        assert_eq!(third, PulledOpenMlsCatchup::default());

        let package_dir = runtime
            .paths
            .workspace_keys_dir
            .join(&workspace_id.0)
            .join("mls-key-packages");
        let mut package_paths = fs::read_dir(&package_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        package_paths.sort();
        fs::write(&package_paths[0], b"corrupt package").unwrap();
        let after_corruption = runtime
            .reconcile_openmls_access(workspace_id.clone())
            .unwrap();
        assert_eq!(after_corruption.published_key_package_event_ids.len(), 1);

        let settled_again = runtime
            .reconcile_openmls_access(workspace_id.clone())
            .unwrap();
        assert_eq!(settled_again.event_count, 0);
        let stable_again = runtime.openmls_reconcile_snapshot(&workspace_id).unwrap();
        assert!(runtime.openmls_reconcile_checkpoint_matches(&workspace_id, &stable_again));

        package_paths = fs::read_dir(&package_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| fs::read(path).unwrap() != b"corrupt package")
            .collect();
        package_paths.sort();
        fs::remove_file(&package_paths[0]).unwrap();
        let after_deletion = runtime.reconcile_openmls_access(workspace_id).unwrap();
        assert_eq!(after_deletion.published_key_package_event_ids.len(), 1);
    }
}
