use std::collections::BTreeSet;

use chaft_media::BlobAvailability;
use chaft_net::{ChaftTransport, NetError, PeerAddress};
use chaft_net_direct::{
    AuthorizedPublishTransport, BlobSyncTransport, MAX_PUBLISH_EVENTS_PER_REQUEST,
};
use chaft_sync::{
    PullSyncReport, pull_workspace_from_peer, pull_workspace_from_peer_with_inventory,
    validate_remote_inventory_event_ids,
};
use chaft_types::{EventId, SignedEvent, WorkspaceId};

use crate::{
    BlobTransferAttempt, BlobTransferMode, BlobTransferRetryReport, BlobTransferStatus,
    DIRECT_BLOB_CHUNK_SIZE, DIRECT_WHOLE_BLOB_SYNC_LIMIT, LocalRuntime,
    MAX_PUBLISH_QUEUE_BLOB_HASH_SAMPLE_ROWS, MAX_PUBLISH_QUEUE_EVENT_ID_SAMPLE_ROWS,
    MAX_PUBLISH_QUEUE_SKIPPED_GAP_SAMPLE_ROWS, PublishedWorkspace, PulledOpenMlsCatchup,
    PulledOpenMlsChannelCatchup, PulledWorkspace, PulledWorkspaceGap, RuntimeError,
    SyncedWorkspace, WorkspacePublishQueue, attachment_blob_hashes, blob_transfer_peer_error,
    is_backup_slice_event, merge_published_workspace, ordered_retry_peers, planned_chunk_upload,
    validate_event_id_reference, validate_peer_address, validate_peer_addresses,
    validate_workspace_id_reference, workspace_publish_queue_summary,
};

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
        let (events, skipped_gaps) = self.materialized_workspace_events_with_gaps(&workspace_id)?;
        let remote_event_ids = transport
            .fetch_workspace_inventory(peer, &workspace_id)
            .await?;
        validate_remote_inventory_event_ids(&remote_event_ids)?;
        let remote_event_ids = remote_event_ids.into_iter().collect::<BTreeSet<_>>();
        let events_to_publish = events
            .iter()
            .filter(|event| !remote_event_ids.contains(&event.event_id))
            .cloned()
            .collect::<Vec<_>>();
        let published_event_ids = events_to_publish
            .iter()
            .map(|event| event.event_id.0.clone())
            .collect::<Vec<_>>();
        transport
            .publish_events_with_authorization(peer, events_to_publish, Vec::new(), Vec::new())
            .await?;

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
        let ledger = self.read_blob_transfer_ledger()?;
        let pending_entries = ledger
            .entries
            .into_iter()
            .filter(|entry| {
                entry.workspace_id == workspace_id.0
                    && entry.status != BlobTransferStatus::Succeeded
            })
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
        let retry_peers = ordered_retry_peers(peers);

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

            for &peer in &retry_peers {
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
        let openmls_catchup = self.apply_local_openmls_catchup(&workspace_id)?;
        let compromise_response = self.automatic_compromise_response_if_needed(&workspace_id)?;
        let _ = self.reindex_workspace_search_if_key_available(&workspace_id);
        let mut pulled = Self::pulled_workspace_from_report(workspace_id, report);
        pulled.openmls_catchup = openmls_catchup;
        pulled.compromise_response = compromise_response;
        Ok(pulled)
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
        let report = pull_workspace_from_peer_with_inventory(
            transport,
            peer,
            &self.store,
            workspace_id.clone(),
            remote_event_ids,
        )
        .await?;
        let openmls_catchup = self.apply_local_openmls_catchup(&workspace_id)?;
        let compromise_response = self.automatic_compromise_response_if_needed(&workspace_id)?;
        let _ = self.reindex_workspace_search_if_key_available(&workspace_id);
        let mut pulled = Self::pulled_workspace_from_report(workspace_id.clone(), report);
        pulled.openmls_catchup = openmls_catchup;
        pulled.compromise_response = compromise_response;
        let events = self.materialized_workspace_events(&workspace_id)?;
        let blob_hashes = attachment_blob_hashes(&events);
        let blob_store = self.open_blob_store()?;
        let mut missing_local_blob_hashes = Vec::new();

        for blob_hash in blob_hashes {
            if blob_store.has_complete_blob(&blob_hash)? {
                continue;
            }
            missing_local_blob_hashes.push(blob_hash);
        }

        let fetched_blobs = transport
            .fetch_blobs(peer, missing_local_blob_hashes.clone())
            .await?;
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
                    None => pulled.missing_blob_hashes.push(blob_hash),
                },
            }
        }
        pulled.refresh_counts();

        Ok(pulled)
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
        validate_peer_address(peer)?;
        let mut published = self
            .publish_workspace_to_peer(transport, peer, workspace_id.clone())
            .await?;
        let pulled = self
            .pull_workspace_from_peer(transport, peer, workspace_id.clone())
            .await?;
        if pulled.has_local_generated_events() {
            let followup = self
                .publish_workspace_to_peer(transport, peer, workspace_id.clone())
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
        validate_peer_address(peer)?;
        let mut published = self
            .publish_workspace_direct(transport, peer, workspace_id.clone())
            .await?;
        let pulled = self
            .pull_workspace_direct(transport, peer, workspace_id.clone())
            .await?;
        if pulled.has_local_generated_events() {
            let followup = self
                .publish_workspace_direct(transport, peer, workspace_id.clone())
                .await?;
            merge_published_workspace(&mut published, followup);
        }

        Ok(SyncedWorkspace {
            workspace_id: workspace_id.0,
            published,
            pulled,
        })
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

    fn apply_local_openmls_catchup(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<PulledOpenMlsCatchup, RuntimeError> {
        let mut catchup = PulledOpenMlsCatchup::default();

        if !self.openmls_workspace_group_path(workspace_id).exists() {
            match self.join_openmls_workspace_group(workspace_id.clone(), None) {
                Ok(joined) => catchup.workspace_joined_event_id = Some(joined.source_event_id),
                Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { .. })
                | Err(RuntimeError::OpenMlsWorkspaceGroupInviteNotFound { .. })
                | Err(RuntimeError::OpenMlsPrivateKeyPackageMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        if self.openmls_workspace_group_path(workspace_id).exists() {
            match self.apply_openmls_workspace_group_commits(workspace_id.clone(), None) {
                Ok(applied) => {
                    catchup.workspace_applied_event_ids = applied.applied_event_ids;
                    catchup.workspace_self_removed = applied.self_removed;
                }
                Err(RuntimeError::OpenMlsWorkspaceGroupMissing { .. }) => {}
                Err(error) => return Err(error),
            }
        }

        for channel_id in self.joinable_openmls_channel_group_ids(workspace_id)? {
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

        for channel_id in self.local_openmls_channel_group_ids(workspace_id)? {
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
            self.auto_provision_openmls_workspace_members(workspace_id);
        for provisioned in self.auto_provision_openmls_channel_members(workspace_id) {
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
