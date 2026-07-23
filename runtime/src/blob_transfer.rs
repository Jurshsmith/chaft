use serde::{Deserialize, Serialize};

use chaft_media::BLOB_DESCRIPTOR_MAX_CHUNKS;
use chaft_types::{
    ATTACHMENT_BLOB_HASH_MAX_BYTES, PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_MAX_BYTES,
    WORKSPACE_ID_MAX_BYTES,
};

use crate::{truncate_string_bytes, truncate_string_list_bytes, truncate_string_option_bytes};

pub(crate) const BLOB_TRANSFER_LEDGER_SCHEMA_VERSION: u32 = 1;
pub(crate) const BLOB_TRANSFER_LEDGER_MAX_ENTRIES: usize = 512;
pub(crate) const BLOB_TRANSFER_LEDGER_MAX_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES: usize =
    20 + 1 + 10 + 1 + PEER_ENDPOINT_ID_MAX_BYTES + 1 + ATTACHMENT_BLOB_HASH_MAX_BYTES;
pub(crate) const BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferLedger {
    pub schema_version: u32,
    pub entries: Vec<BlobTransferAttempt>,
}

impl Default for BlobTransferLedger {
    fn default() -> Self {
        Self {
            schema_version: BLOB_TRANSFER_LEDGER_SCHEMA_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferAttempt {
    pub attempt_id: String,
    pub workspace_id: String,
    pub peer_id: String,
    pub peer_endpoint: String,
    pub blob_hash: String,
    pub mode: BlobTransferMode,
    pub status: BlobTransferStatus,
    pub attempt_count: u32,
    pub total_byte_len: u64,
    pub chunk_size: Option<u64>,
    #[serde(default)]
    pub chunk_count: usize,
    pub chunk_hashes: Vec<String>,
    #[serde(default)]
    pub planned_chunk_count: usize,
    pub planned_chunk_hashes: Vec<String>,
    #[serde(default)]
    pub remote_available_chunk_count: usize,
    pub remote_available_chunk_hashes: Vec<String>,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: Option<u64>,
    pub error: Option<String>,
}

impl BlobTransferAttempt {
    fn refresh_counts(&mut self) {
        self.chunk_count = self.chunk_hashes.len();
        self.planned_chunk_count = self.planned_chunk_hashes.len();
        self.remote_available_chunk_count = self.remote_available_chunk_hashes.len();
    }

    pub(crate) fn normalize_after_read(&mut self) {
        truncate_string_bytes(&mut self.attempt_id, BLOB_TRANSFER_ATTEMPT_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.workspace_id, WORKSPACE_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.peer_id, PEER_ENDPOINT_ID_MAX_BYTES);
        truncate_string_bytes(&mut self.peer_endpoint, PEER_ENDPOINT_MAX_BYTES);
        truncate_string_bytes(&mut self.blob_hash, ATTACHMENT_BLOB_HASH_MAX_BYTES);
        self.chunk_hashes.truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        self.planned_chunk_hashes
            .truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        self.remote_available_chunk_hashes
            .truncate(BLOB_DESCRIPTOR_MAX_CHUNKS);
        truncate_string_list_bytes(&mut self.chunk_hashes, ATTACHMENT_BLOB_HASH_MAX_BYTES);
        truncate_string_list_bytes(
            &mut self.planned_chunk_hashes,
            ATTACHMENT_BLOB_HASH_MAX_BYTES,
        );
        truncate_string_list_bytes(
            &mut self.remote_available_chunk_hashes,
            ATTACHMENT_BLOB_HASH_MAX_BYTES,
        );
        if self.mode == BlobTransferMode::WholeBlob {
            self.chunk_size = None;
            self.chunk_hashes.clear();
            self.planned_chunk_hashes.clear();
            self.remote_available_chunk_hashes.clear();
        }
        truncate_string_option_bytes(&mut self.error, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
        self.refresh_counts();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferRetryReport {
    pub workspace_id: String,
    #[serde(default)]
    pub pending_attempt_count: usize,
    pub pending_attempt_ids: Vec<String>,
    #[serde(default)]
    pub retried_blob_count: usize,
    pub retried_blob_hashes: Vec<String>,
    #[serde(default)]
    pub reconciled_blob_count: usize,
    pub reconciled_blob_hashes: Vec<String>,
    #[serde(default)]
    pub missing_blob_count: usize,
    pub missing_blob_hashes: Vec<String>,
    #[serde(default)]
    pub skipped_blob_count: usize,
    pub skipped_blob_hashes: Vec<String>,
    #[serde(default)]
    pub peer_error_count: usize,
    pub peer_errors: Vec<BlobTransferPeerError>,
    #[serde(default)]
    pub blob_transfer_attempt_count: usize,
    pub blob_transfer_attempts: Vec<BlobTransferAttempt>,
}

impl BlobTransferRetryReport {
    pub(crate) fn refresh_counts(&mut self) {
        self.pending_attempt_count = self.pending_attempt_ids.len();
        self.retried_blob_count = self.retried_blob_hashes.len();
        self.reconciled_blob_count = self.reconciled_blob_hashes.len();
        self.missing_blob_count = self.missing_blob_hashes.len();
        self.skipped_blob_count = self.skipped_blob_hashes.len();
        self.peer_error_count = self.peer_errors.len();
        self.blob_transfer_attempt_count = self.blob_transfer_attempts.len();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobTransferPeerError {
    pub peer_id: String,
    pub peer_endpoint: String,
    pub blob_hash: String,
    pub message: String,
    pub suspect_protocol_error: bool,
}

pub(crate) fn blob_transfer_peer_error(
    peer_id: &str,
    peer_endpoint: &str,
    blob_hash: &str,
    mut message: String,
    suspect_protocol_error: bool,
) -> BlobTransferPeerError {
    let mut peer_id = peer_id.to_owned();
    let mut peer_endpoint = peer_endpoint.to_owned();
    let mut blob_hash = blob_hash.to_owned();
    truncate_string_bytes(&mut peer_id, PEER_ENDPOINT_ID_MAX_BYTES);
    truncate_string_bytes(&mut peer_endpoint, PEER_ENDPOINT_MAX_BYTES);
    truncate_string_bytes(&mut blob_hash, ATTACHMENT_BLOB_HASH_MAX_BYTES);
    truncate_string_bytes(&mut message, BLOB_TRANSFER_ATTEMPT_ERROR_MAX_BYTES);
    BlobTransferPeerError {
        peer_id,
        peer_endpoint,
        blob_hash,
        message,
        suspect_protocol_error,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobTransferMode {
    WholeBlob,
    ChunkedBlob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobTransferStatus {
    InProgress,
    Succeeded,
    Failed,
}
