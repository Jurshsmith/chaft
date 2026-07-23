use chaft_net::PeerAddress;
use chaft_types::ChannelId;
use chaft_types::{
    DEVICE_ID_MAX_BYTES, DeviceId, DeviceKeyPackageId, EventId, IdValidationError,
    MESSAGE_MARKDOWN_MAX_BYTES, MessageId, PEER_ENDPOINT_ID_MAX_BYTES,
    PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES, WorkspaceId,
    peer_endpoint_hint_is_supported, validate_channel_id as validate_type_channel_id,
    validate_device_key_package_id as validate_type_device_key_package_id,
    validate_event_id as validate_type_event_id, validate_message_id as validate_type_message_id,
    validate_workspace_id as validate_type_workspace_id,
};

use crate::{RuntimeError, SEARCH_QUERY_MAX_BYTES};

pub(crate) const DEVICE_ID_REFERENCE_MAX_BYTES: usize = DEVICE_ID_MAX_BYTES;

pub(crate) fn validate_message_markdown_size(markdown: &str) -> Result<(), RuntimeError> {
    let actual_bytes = markdown.len();
    if actual_bytes > MESSAGE_MARKDOWN_MAX_BYTES {
        return Err(RuntimeError::MessageMarkdownTooLarge {
            actual_bytes,
            max_bytes: MESSAGE_MARKDOWN_MAX_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn validate_metadata_field_size(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), RuntimeError> {
    let actual_bytes = value.len();
    if actual_bytes > max_bytes {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}

fn validate_identifier_size(result: Result<(), IdValidationError>) -> Result<(), RuntimeError> {
    result.map_err(|error| RuntimeError::MetadataFieldTooLarge {
        field: error.field,
        actual_bytes: error.actual_bytes,
        max_bytes: error.max_bytes,
    })
}

pub(crate) fn validate_workspace_id_reference(
    workspace_id: &WorkspaceId,
) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_workspace_id(workspace_id))
}

pub(crate) fn validate_channel_id_reference(channel_id: &ChannelId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_channel_id(channel_id))
}

pub(crate) fn validate_message_id_reference(message_id: &MessageId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_message_id(message_id))
}

pub(crate) fn validate_device_key_package_id_reference(
    key_package_id: &DeviceKeyPackageId,
) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_device_key_package_id(key_package_id))
}

pub(crate) fn validate_event_id_reference(event_id: &EventId) -> Result<(), RuntimeError> {
    validate_identifier_size(validate_type_event_id(event_id))
}

pub(crate) fn validate_search_query_size(query: &str) -> Result<(), RuntimeError> {
    let actual_bytes = query.len();
    if actual_bytes > SEARCH_QUERY_MAX_BYTES {
        return Err(RuntimeError::SearchQueryTooLarge {
            actual_bytes,
            max_bytes: SEARCH_QUERY_MAX_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn validate_device_id_reference(device_id: &DeviceId) -> Result<(), RuntimeError> {
    validate_metadata_field_size("device ID", &device_id.0, DEVICE_ID_REFERENCE_MAX_BYTES)
}

pub(crate) fn validate_peer_endpoint_input(endpoint: &str) -> Result<(), RuntimeError> {
    if endpoint.trim().is_empty() {
        return Err(RuntimeError::PeerEndpointRequired);
    }
    validate_metadata_field_size("peer endpoint", endpoint, PEER_ENDPOINT_MAX_BYTES)?;
    if !peer_endpoint_hint_is_supported(endpoint) {
        return Err(RuntimeError::UnsupportedPeerEndpoint);
    }
    Ok(())
}

pub(crate) fn validate_peer_address(peer: &PeerAddress) -> Result<(), RuntimeError> {
    validate_peer_endpoint_input(&peer.endpoint)?;
    validate_metadata_field_size("peer ID", &peer.peer_id.0, PEER_ENDPOINT_ID_MAX_BYTES)
}

pub(crate) fn validate_peer_addresses(peers: &[PeerAddress]) -> Result<(), RuntimeError> {
    if peers.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        return Err(RuntimeError::PeerEndpointListTooLarge {
            actual_count: peers.len(),
            max_count: PEER_ENDPOINT_LIST_MAX_ITEMS,
        });
    }
    for peer in peers {
        validate_peer_address(peer)?;
    }
    Ok(())
}
