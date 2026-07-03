use chaft_net::PeerAddress;

use crate::{
    BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES,
    BLOB_TRANSFER_LEDGER_MAX_BYTES, BLOB_TRANSFER_LEDGER_MAX_ENTRIES,
    BLOB_TRANSFER_LEDGER_SCHEMA_VERSION, BlobTransferAttempt, BlobTransferLedger, BlobTransferMode,
    BlobTransferStatus, LocalRuntime, RuntimeError, now_unix_ms,
    read_local_metadata_file_with_limit, truncate_string_bytes, truncate_string_option_bytes,
    validate_peer_address, write_secret_file,
};

impl LocalRuntime {
    pub fn blob_transfer_ledger(&self) -> Result<BlobTransferLedger, RuntimeError> {
        self.read_blob_transfer_ledger()
    }

    pub(crate) fn read_blob_transfer_ledger(&self) -> Result<BlobTransferLedger, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            &self.paths.blob_transfer_ledger,
            BLOB_TRANSFER_LEDGER_MAX_BYTES,
            "blob transfer ledger",
        )?
        else {
            return Ok(BlobTransferLedger::default());
        };
        let mut ledger = serde_json::from_slice::<BlobTransferLedger>(&bytes)?;
        if ledger.schema_version != BLOB_TRANSFER_LEDGER_SCHEMA_VERSION {
            ledger = BlobTransferLedger::default();
        } else {
            if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
                let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
                ledger.entries.drain(0..remove_count);
            }
            for entry in &mut ledger.entries {
                entry.normalize_after_read();
            }
        }
        Ok(ledger)
    }

    pub(crate) fn write_blob_transfer_ledger(
        &self,
        ledger: &BlobTransferLedger,
    ) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec_pretty(ledger)?;
        write_secret_file(&self.paths.blob_transfer_ledger, &bytes)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_blob_transfer_started(
        &self,
        workspace_id: &str,
        peer: &PeerAddress,
        blob_hash: &str,
        mode: BlobTransferMode,
        total_byte_len: u64,
        chunk_size: Option<u64>,
        chunk_hashes: Vec<String>,
        planned_chunk_hashes: Vec<String>,
        remote_available_chunk_hashes: Vec<String>,
    ) -> Result<BlobTransferAttempt, RuntimeError> {
        validate_peer_address(peer)?;
        let mut ledger = self.read_blob_transfer_ledger()?;
        let attempt_count = ledger
            .entries
            .iter()
            .filter(|entry| {
                entry.workspace_id == workspace_id
                    && entry.peer_id == peer.peer_id.0
                    && entry.peer_endpoint == peer.endpoint
                    && entry.blob_hash == blob_hash
            })
            .count() as u32
            + 1;
        let started_at_unix_ms = now_unix_ms();
        let mut attempt_id = format!(
            "{}:{}:{}:{}",
            started_at_unix_ms, attempt_count, peer.peer_id.0, blob_hash
        );
        truncate_string_bytes(&mut attempt_id, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES);
        let attempt = BlobTransferAttempt {
            attempt_id,
            workspace_id: workspace_id.to_owned(),
            peer_id: peer.peer_id.0.clone(),
            peer_endpoint: peer.endpoint.clone(),
            blob_hash: blob_hash.to_owned(),
            mode,
            status: BlobTransferStatus::InProgress,
            attempt_count,
            total_byte_len,
            chunk_size,
            chunk_count: chunk_hashes.len(),
            chunk_hashes,
            planned_chunk_count: planned_chunk_hashes.len(),
            planned_chunk_hashes,
            remote_available_chunk_count: remote_available_chunk_hashes.len(),
            remote_available_chunk_hashes,
            started_at_unix_ms,
            finished_at_unix_ms: None,
            error: None,
        };
        ledger.entries.push(attempt.clone());
        if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        self.write_blob_transfer_ledger(&ledger)?;
        Ok(attempt)
    }

    pub(crate) fn record_blob_transfer_finished(
        &self,
        started: &BlobTransferAttempt,
        status: BlobTransferStatus,
        error: Option<String>,
    ) -> Result<BlobTransferAttempt, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let mut finished = started.clone();
        finished.status = status;
        finished.finished_at_unix_ms = Some(now_unix_ms());
        finished.error = error;
        truncate_string_option_bytes(&mut finished.error, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);

        if let Some(entry) = ledger
            .entries
            .iter_mut()
            .find(|entry| entry.attempt_id == started.attempt_id)
        {
            *entry = finished.clone();
        } else {
            ledger.entries.push(finished.clone());
        }
        if ledger.entries.len() > BLOB_TRANSFER_LEDGER_MAX_ENTRIES {
            let remove_count = ledger.entries.len() - BLOB_TRANSFER_LEDGER_MAX_ENTRIES;
            ledger.entries.drain(0..remove_count);
        }
        self.write_blob_transfer_ledger(&ledger)?;
        Ok(finished)
    }

    pub(crate) fn reconcile_completed_blob_transfer_attempts(
        &self,
        workspace_id: &str,
        peer: &PeerAddress,
        blob_hash: &str,
    ) -> Result<Vec<BlobTransferAttempt>, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let finished_at_unix_ms = now_unix_ms();
        let mut reconciled = Vec::new();

        for entry in &mut ledger.entries {
            if entry.workspace_id == workspace_id
                && entry.peer_id == peer.peer_id.0
                && entry.peer_endpoint == peer.endpoint
                && entry.blob_hash == blob_hash
                && entry.status != BlobTransferStatus::Succeeded
            {
                entry.status = BlobTransferStatus::Succeeded;
                entry.finished_at_unix_ms = Some(finished_at_unix_ms);
                entry.error = None;
                reconciled.push(entry.clone());
            }
        }

        if !reconciled.is_empty() {
            self.write_blob_transfer_ledger(&ledger)?;
        }
        Ok(reconciled)
    }

    pub(crate) fn reconcile_satisfied_blob_transfer_attempts(
        &self,
        workspace_id: &str,
        blob_hash: &str,
    ) -> Result<Vec<BlobTransferAttempt>, RuntimeError> {
        let mut ledger = self.read_blob_transfer_ledger()?;
        let finished_at_unix_ms = now_unix_ms();
        let mut reconciled = Vec::new();

        for entry in &mut ledger.entries {
            if entry.workspace_id == workspace_id
                && entry.blob_hash == blob_hash
                && entry.status != BlobTransferStatus::Succeeded
            {
                entry.status = BlobTransferStatus::Succeeded;
                entry.finished_at_unix_ms = Some(finished_at_unix_ms);
                entry.error = None;
                reconciled.push(entry.clone());
            }
        }

        if !reconciled.is_empty() {
            self.write_blob_transfer_ledger(&ledger)?;
        }
        Ok(reconciled)
    }
}
