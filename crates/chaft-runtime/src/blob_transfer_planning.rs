use std::collections::BTreeSet;

use chaft_media::{BlobAvailability, describe_blob, validate_blob_availability};
use chaft_net::PeerAddress;

use crate::DIRECT_BLOB_CHUNK_SIZE;

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

pub(crate) fn ordered_retry_peers(peers: &[PeerAddress]) -> Vec<&PeerAddress> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    for peer in peers {
        if seen.insert(peer.endpoint.clone()) {
            ordered.push(peer);
        }
    }
    ordered
}
