use std::collections::BTreeSet;

use chaft_media::{BlobAvailability, describe_blob, validate_blob_availability};
use chaft_net::PeerAddress;

use crate::{BlobTransferAttempt, BlobTransferStatus, DIRECT_BLOB_CHUNK_SIZE};

pub(crate) fn planned_chunk_upload(
    bytes: &[u8],
    remote_availability: Option<&BlobAvailability>,
) -> (u64, Vec<String>, Vec<String>, Vec<String>) {
    let descriptor = describe_blob(bytes, DIRECT_BLOB_CHUNK_SIZE);
    let remote_available = remote_availability
        .filter(|availability| {
            availability.descriptor.as_ref() == Some(&descriptor)
                && validate_blob_availability(availability).is_ok()
        })
        .map(|availability| {
            availability
                .available_chunk_hashes
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mut remote_available_chunk_hashes = Vec::new();
    let mut planned_chunk_hashes = Vec::new();
    let mut seen_remote = BTreeSet::new();
    let mut seen_planned = BTreeSet::new();
    for chunk_hash in &descriptor.chunk_hashes {
        if remote_available.contains(chunk_hash) {
            if seen_remote.insert(chunk_hash.clone()) {
                remote_available_chunk_hashes.push(chunk_hash.clone());
            }
        } else if seen_planned.insert(chunk_hash.clone()) {
            planned_chunk_hashes.push(chunk_hash.clone());
        }
    }

    (
        descriptor.chunk_size as u64,
        descriptor.chunk_hashes,
        planned_chunk_hashes,
        remote_available_chunk_hashes,
    )
}

pub(crate) fn planned_retry_peers<'a>(
    peers: &'a [PeerAddress],
    attempts: &[BlobTransferAttempt],
    workspace_id: &str,
    blob_hash: &str,
) -> Vec<&'a PeerAddress> {
    let mut ranked = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, peer) in peers.iter().enumerate() {
        if seen.insert(peer.endpoint.clone()) {
            ranked.push((
                retry_peer_rank(peer, attempts, workspace_id, blob_hash),
                index,
                peer,
            ));
        }
    }
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked.into_iter().map(|(_, _, peer)| peer).collect()
}

fn retry_peer_rank(
    peer: &PeerAddress,
    attempts: &[BlobTransferAttempt],
    workspace_id: &str,
    blob_hash: &str,
) -> u8 {
    let mut rank = 1;
    for attempt in attempts {
        if attempt.workspace_id != workspace_id
            || attempt.peer_endpoint != peer.endpoint
            || attempt.blob_hash != blob_hash
        {
            continue;
        }
        match attempt.status {
            BlobTransferStatus::Succeeded => return 0,
            BlobTransferStatus::InProgress | BlobTransferStatus::Failed => rank = 2,
        }
    }
    rank
}
