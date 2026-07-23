use std::collections::BTreeSet;

use chaft_net::{PeerAddress, PeerId};
use chaft_runtime::{PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES};
use chaft_types::{
    direct_tcp_peer_listen_address_is_valid, peer_endpoint_hint_is_supported,
    peer_endpoint_hint_transport_is_consistent,
};

use crate::envelope::{FfiError, ffi_error};

pub(crate) fn validate_peer_endpoint_text(
    endpoint: &str,
    field_name: &'static str,
) -> Result<(), FfiError> {
    if endpoint.trim().is_empty() {
        return Err(ffi_error(field_name, "peer endpoint is required"));
    }
    if endpoint.len() > PEER_ENDPOINT_MAX_BYTES {
        return Err(ffi_error(
            "peer_endpoint_too_large",
            format!(
                "peer endpoint is too large ({} bytes, max {})",
                endpoint.len(),
                PEER_ENDPOINT_MAX_BYTES
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_direct_listen_endpoint_text(endpoint: &str) -> Result<(), FfiError> {
    if direct_tcp_peer_listen_address_is_valid(endpoint) {
        return Ok(());
    }
    Err(ffi_error(
        "peer_endpoint_unsupported",
        "direct listen endpoint must be host:port with numeric port",
    ))
}

pub(crate) fn direct_peer_address(endpoint: String) -> Result<PeerAddress, FfiError> {
    let endpoint = endpoint.trim().to_owned();
    validate_peer_endpoint_text(&endpoint, "peer_endpoint")?;
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(ffi_error(
            "peer_endpoint_unsupported",
            "peer endpoint must be a direct TCP or native Iroh direct route",
        ));
    }
    Ok(PeerAddress {
        peer_id: PeerId(endpoint.clone()),
        endpoint,
    })
}

pub(crate) fn direct_peer_addresses(endpoints: &str) -> Result<Vec<PeerAddress>, FfiError> {
    let endpoints = deduplicate_peer_endpoints(split_peer_endpoints(endpoints));
    if endpoints.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        return Err(ffi_error(
            "peer_endpoint_list_too_large",
            format!(
                "peer endpoint list is too large ({} endpoints, max {})",
                endpoints.len(),
                PEER_ENDPOINT_LIST_MAX_ITEMS
            ),
        ));
    }
    endpoints.into_iter().map(direct_peer_address).collect()
}

pub(crate) fn validate_peer_endpoint_hint_inputs(
    endpoint_id: String,
    endpoint: String,
    transport: String,
) -> Result<(String, String, String), FfiError> {
    let endpoint_id = endpoint_id.trim().to_owned();
    if endpoint_id.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_id_required",
            "peer endpoint ID is required",
        ));
    }

    let endpoint = endpoint.trim().to_owned();
    if endpoint.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_required",
            "peer endpoint is required",
        ));
    }
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(ffi_error(
            "peer_endpoint_unsupported",
            "peer endpoint must be a direct TCP or native Iroh direct route",
        ));
    }

    let transport = transport.trim().to_owned();
    if transport.is_empty() {
        return Err(ffi_error(
            "peer_endpoint_transport_required",
            "peer endpoint transport is required",
        ));
    }
    if !peer_endpoint_hint_transport_is_consistent(&endpoint, &transport) {
        return Err(ffi_error(
            "peer_endpoint_transport_mismatch",
            "peer endpoint transport does not match the endpoint route",
        ));
    }

    Ok((endpoint_id, endpoint, transport))
}

fn split_peer_endpoints(endpoints: &str) -> Vec<String> {
    endpoints
        .split([',', ';'])
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .map(str::to_owned)
        .collect()
}

fn deduplicate_peer_endpoints(endpoints: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    endpoints
        .into_iter()
        .filter(|endpoint| seen.insert(endpoint.clone()))
        .collect()
}
