use std::{
    collections::{BTreeSet, HashMap},
    net::{SocketAddr, ToSocketAddrs},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use chaft_core::{authorize_event_with_history, authorize_event_with_trust_snapshot};
use chaft_identity::{verify_self_contained_event, verify_self_contained_trust_snapshot};
use chaft_media::{
    BlobAvailability, BlobDescriptor, BlobStore, blob_hash, describe_blob,
    validate_blob_availability, validate_blob_descriptor, validate_chunk_payload,
    validate_reassembled_blob,
};
use chaft_net::{ChaftTransport, NetError, PeerAddress};
use chaft_store::{EventStore, validate_signed_event_json_size};
use chaft_types::{
    AttachmentRef, EventBody, EventId, PayloadEncryption, SealedPayload, SignedEvent,
    SignedTrustSnapshot, WorkspaceId, direct_tcp_peer_endpoint_address_is_valid,
    is_canonical_event_id_str, validate_workspace_id_str,
};
use chaft_wire::{
    WireBlobAvailability, WireBlobDescriptor, WireBlobEnvelope, WireEventEnvelope,
    WireSignedTrustSnapshot, WireSyncRequest, WireSyncRequestKind, WireSyncResponse, decode_event,
    decode_event_envelope, decode_sync_request, decode_sync_response, decode_trust_snapshot,
    decode_trust_snapshot_envelope, encode_event_envelope, encode_sync_request,
    encode_sync_response, encode_trust_snapshot_envelope,
};
use prost::Message as _;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinSet,
    time::timeout,
};

pub const MAX_FRAME_LEN: usize = chaft_wire::SYNC_FRAME_MAX_BYTES;
pub const MAX_EVENT_UPLOAD_BATCH_BYTES: usize = MAX_FRAME_LEN / 2;
pub const MAX_WHOLE_BLOB_UPLOAD_BATCH_BYTES: usize = MAX_FRAME_LEN / 2;
pub const MAX_CHUNK_UPLOAD_BATCH_BYTES: usize = MAX_FRAME_LEN / 2;
pub const MAX_PUBLISH_EVENTS_PER_REQUEST: usize = 128;
pub const MAX_AUTHORIZATION_EVENTS_PER_REQUEST: usize = 128;
pub const MAX_AUTHORIZATION_SNAPSHOTS_PER_REQUEST: usize = 32;
pub const MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST: usize = 128;
pub const MAX_BLOB_UPLOAD_DESCRIPTORS_PER_REQUEST: usize = 128;
pub const MAX_FETCH_EVENT_IDS_PER_REQUEST: usize = 128;
pub const MAX_FETCH_BLOB_HASHES_PER_REQUEST: usize = 128;
pub const MAX_INVENTORY_EVENT_IDS_PER_RESPONSE: usize = 1024;
pub const MAX_INVENTORY_EVENT_IDS_PER_PULL: usize = MAX_INVENTORY_EVENT_IDS_PER_RESPONSE * 1024;
pub const MAX_ACTIVE_DIRECT_CONNECTIONS: usize = 256;
pub const MAX_SYNC_RESPONSE_ERROR_BYTES: usize = 2 * 1024;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const FRAME_IO_TIMEOUT: Duration = Duration::from_secs(30);
const SYNC_RESPONSE_ERROR_TRUNCATED_SUFFIX: &str = "...";

#[derive(Debug, Clone, Default)]
pub struct DirectTransport;

#[async_trait]
pub trait AuthorizedPublishTransport: ChaftTransport {
    async fn publish_events_with_authorization(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
        authorization_snapshots: Vec<SignedTrustSnapshot>,
    ) -> Result<(), NetError>;
}

#[async_trait]
pub trait BlobSyncTransport: ChaftTransport {
    async fn put_blobs(
        &self,
        peer: &PeerAddress,
        blobs: Vec<Vec<u8>>,
    ) -> Result<Vec<String>, NetError>;

    async fn fetch_blobs(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, NetError>;

    async fn fetch_blob_availabilities(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, BlobAvailability>, NetError>;

    async fn put_blob_chunked(
        &self,
        peer: &PeerAddress,
        bytes: Vec<u8>,
        chunk_size: usize,
    ) -> Result<BlobDescriptor, NetError>;

    async fn fetch_blob_chunked(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError>;
}

pub struct DirectPeerServer {
    listener: TcpListener,
    sync_store: SyncPeerStore,
}

#[derive(Clone)]
pub struct SyncPeerStore {
    store: Arc<Mutex<EventStore>>,
    blob_store: Option<Arc<BlobStore>>,
}

impl SyncPeerStore {
    pub fn new(store: EventStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            blob_store: None,
        }
    }

    pub fn with_blobs(store: EventStore, blob_store: BlobStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            blob_store: Some(Arc::new(blob_store)),
        }
    }

    pub async fn serve_stream<S>(&self, stream: &mut S) -> Result<(), NetError>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        handle_sync_stream(stream, Arc::clone(&self.store), self.blob_store.clone()).await
    }
}

impl DirectPeerServer {
    pub async fn bind(addr: impl ToSocketAddrs, store: EventStore) -> Result<Self, NetError> {
        let addr = addr
            .to_socket_addrs()
            .map_err(NetError::from)?
            .next()
            .ok_or_else(|| NetError::Protocol("no socket address resolved".to_owned()))?;
        let listener = TcpListener::bind(addr).await?;

        Ok(Self {
            listener,
            sync_store: SyncPeerStore::new(store),
        })
    }

    pub async fn bind_with_blobs(
        addr: impl ToSocketAddrs,
        store: EventStore,
        blob_store: BlobStore,
    ) -> Result<Self, NetError> {
        let addr = addr
            .to_socket_addrs()
            .map_err(NetError::from)?
            .next()
            .ok_or_else(|| NetError::Protocol("no socket address resolved".to_owned()))?;
        let listener = TcpListener::bind(addr).await?;
        let server = Self {
            listener,
            sync_store: SyncPeerStore::with_blobs(store, blob_store),
        };
        Ok(server)
    }

    pub fn local_addr(&self) -> Result<SocketAddr, NetError> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn serve_one(&self) -> Result<(), NetError> {
        let (stream, _) = self.listener.accept().await?;
        handle_connection(stream, self.sync_store.clone()).await
    }

    pub async fn serve_until_shutdown(
        &self,
        shutdown: oneshot::Receiver<()>,
    ) -> Result<(), NetError> {
        self.serve_until_shutdown_with_max_connections(shutdown, MAX_ACTIVE_DIRECT_CONNECTIONS)
            .await
    }

    pub async fn serve_until_shutdown_with_max_connections(
        &self,
        shutdown: oneshot::Receiver<()>,
        max_active_connections: usize,
    ) -> Result<(), NetError> {
        self.serve_until_shutdown_with_connection_limit(shutdown, max_active_connections)
            .await
    }

    async fn serve_until_shutdown_with_connection_limit(
        &self,
        mut shutdown: oneshot::Receiver<()>,
        max_active_connections: usize,
    ) -> Result<(), NetError> {
        if max_active_connections == 0 {
            return Err(NetError::Protocol(
                "direct peer connection limit must be greater than zero".to_owned(),
            ));
        }

        let mut connections = JoinSet::new();
        let mut active_connections = 0usize;

        loop {
            tokio::select! {
                accept = self.listener.accept(), if active_connections < max_active_connections => {
                    let (stream, _) = accept?;
                    let sync_store = self.sync_store.clone();
                    connections.spawn(async move {
                        let _ = handle_connection(stream, sync_store).await;
                    });
                    active_connections += 1;
                }
                result = connections.join_next(), if active_connections > 0 => {
                    if result.is_some() {
                        active_connections -= 1;
                    } else {
                        active_connections = 0;
                    }
                }
                _ = &mut shutdown => return Ok(()),
            }
        }
    }
}

#[async_trait]
impl ChaftTransport for DirectTransport {
    async fn connect(&self, peer: PeerAddress) -> Result<(), NetError> {
        let response = DirectTransport::fetch_inventory_page(&peer, None, 0, 0).await?;
        validate_inventory_page_response(&response, 0)
    }

    async fn fetch_inventory(&self, peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        fetch_inventory_paged(peer, None).await
    }

    async fn fetch_workspace_inventory(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        DirectTransport::fetch_workspace_inventory(self, peer, workspace_id).await
    }

    async fn publish_event(&self, peer: &PeerAddress, event: SignedEvent) -> Result<(), NetError> {
        let request = WireSyncRequest {
            kind: WireSyncRequestKind::PublishEvents as i32,
            event_ids: Vec::new(),
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            workspace_id: None,
            event_envelopes: vec![encode_event_envelope(&event)],
            authorization_event_envelopes: Vec::new(),
            authorization_snapshot_envelopes: Vec::new(),
            inventory_start_index: None,
            inventory_limit: None,
        };
        let mut response = request_peer(peer, request).await?;
        response_error(response.error.take())?;
        validate_empty_ack_response(&response)
    }

    async fn fetch_events(
        &self,
        peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        fetch_events_batched(peer, event_ids).await
    }
}

impl DirectTransport {
    pub async fn fetch_workspace_inventory(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        fetch_inventory_paged(peer, Some(workspace_id)).await
    }

    async fn fetch_inventory_page(
        peer: &PeerAddress,
        workspace_id: Option<&WorkspaceId>,
        start_index: usize,
        limit: usize,
    ) -> Result<WireSyncResponse, NetError> {
        let request = WireSyncRequest {
            kind: WireSyncRequestKind::Inventory as i32,
            event_ids: Vec::new(),
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            workspace_id: workspace_id.map(|id| id.0.clone()),
            event_envelopes: Vec::new(),
            authorization_event_envelopes: Vec::new(),
            authorization_snapshot_envelopes: Vec::new(),
            inventory_start_index: Some(start_index as u64),
            inventory_limit: Some(limit as u64),
        };
        let mut response = request_peer(peer, request).await?;
        response_error(response.error.take())?;
        validate_inventory_response(&response)?;
        Ok(response)
    }

    pub async fn publish_events_with_proof(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
    ) -> Result<(), NetError> {
        self.publish_events_with_authorization(peer, events, authorization_events, Vec::new())
            .await
    }

    pub async fn publish_events_with_authorization(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
        authorization_snapshots: Vec<SignedTrustSnapshot>,
    ) -> Result<(), NetError> {
        if events.is_empty() {
            return Ok(());
        }

        for request in
            build_publish_events_requests(events, authorization_events, authorization_snapshots)?
        {
            let mut response = request_peer(peer, request).await?;
            response_error(response.error.take())?;
            validate_empty_ack_response(&response)?;
        }
        Ok(())
    }

    pub async fn publish_event_with_proof(
        &self,
        peer: &PeerAddress,
        event: SignedEvent,
        authorization_events: Vec<SignedEvent>,
    ) -> Result<(), NetError> {
        self.publish_events_with_proof(peer, vec![event], authorization_events)
            .await
    }

    pub async fn publish_event_with_trust_snapshot(
        &self,
        peer: &PeerAddress,
        event: SignedEvent,
        authorization_snapshot: SignedTrustSnapshot,
    ) -> Result<(), NetError> {
        self.publish_events_with_authorization(
            peer,
            vec![event],
            Vec::new(),
            vec![authorization_snapshot],
        )
        .await
    }

    pub async fn put_blob(&self, peer: &PeerAddress, bytes: Vec<u8>) -> Result<String, NetError> {
        let mut hashes = self.put_blobs(peer, vec![bytes]).await?;
        hashes
            .pop()
            .ok_or_else(|| NetError::Protocol("blob upload returned no hash".to_owned()))
    }

    pub async fn put_blobs(
        &self,
        peer: &PeerAddress,
        blobs: Vec<Vec<u8>>,
    ) -> Result<Vec<String>, NetError> {
        if blobs.is_empty() {
            return Ok(Vec::new());
        }

        let (envelopes, hashes) = whole_blob_upload_envelopes(blobs);
        put_blob_envelopes_batched(peer, envelopes).await?;
        Ok(hashes)
    }

    pub async fn fetch_blob(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError> {
        let mut blobs = self.fetch_blobs(peer, vec![hash.to_owned()]).await?;
        Ok(blobs.remove(hash))
    }

    pub async fn fetch_blobs(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, NetError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let mut blobs = HashMap::new();
        for response in fetch_blobs_responses_batched(peer, hashes).await? {
            blobs.extend(
                response
                    .blobs
                    .into_iter()
                    .map(|blob| (blob.hash, blob.bytes)),
            );
        }
        Ok(blobs)
    }

    pub async fn fetch_blob_availability(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<BlobAvailability>, NetError> {
        let mut availability = self
            .fetch_blob_availabilities(peer, vec![hash.to_owned()])
            .await?;
        Ok(availability.remove(hash))
    }

    pub async fn fetch_blob_availabilities(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, BlobAvailability>, NetError> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }

        let mut availabilities = HashMap::new();
        for response in fetch_blob_availability_responses_batched(peer, hashes).await? {
            let response_availabilities = response
                .blob_availability
                .into_iter()
                .map(wire_to_availability)
                .map(|availability| {
                    availability.map(|availability| (availability.hash.clone(), availability))
                })
                .collect::<Result<HashMap<_, _>, _>>()?;
            availabilities.extend(response_availabilities);
        }
        Ok(availabilities)
    }

    pub async fn put_blob_chunked(
        &self,
        peer: &PeerAddress,
        bytes: Vec<u8>,
        chunk_size: usize,
    ) -> Result<BlobDescriptor, NetError> {
        let descriptor = describe_blob(&bytes, chunk_size);
        validate_blob_descriptor(&descriptor)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        validate_chunk_upload_single_frame_lengths(&descriptor, &bytes)?;
        let manifest_request = WireSyncRequest {
            kind: WireSyncRequestKind::PutBlobs as i32,
            event_ids: Vec::new(),
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: vec![descriptor_to_wire(&descriptor)],
            workspace_id: None,
            event_envelopes: Vec::new(),
            authorization_event_envelopes: Vec::new(),
            authorization_snapshot_envelopes: Vec::new(),
            inventory_start_index: None,
            inventory_limit: None,
        };
        let mut manifest_response = request_peer(peer, manifest_request).await?;
        response_error(manifest_response.error.take())?;
        validate_empty_ack_response(&manifest_response)?;

        let availability = self.fetch_blob_availability(peer, &descriptor.hash).await?;
        if availability
            .as_ref()
            .is_some_and(|availability| availability.has_whole_blob)
        {
            return Ok(descriptor);
        }
        let available_chunks = availability
            .filter(|availability| availability.descriptor.as_ref() == Some(&descriptor))
            .map(|availability| {
                availability
                    .available_chunk_hashes
                    .into_iter()
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();

        let chunk_frame_base_bytes =
            encode_sync_request(&put_blob_chunks_request(&descriptor, Vec::new())).len();
        let mut batch = Vec::new();
        let mut batch_bytes = chunk_frame_base_bytes;
        let mut planned_chunk_hashes = BTreeSet::new();

        for (chunk_hash, chunk) in descriptor
            .chunk_hashes
            .iter()
            .zip(bytes.chunks(descriptor.chunk_size))
        {
            if available_chunks.contains(chunk_hash) {
                continue;
            }
            if !planned_chunk_hashes.insert(chunk_hash.clone()) {
                continue;
            }
            if blob_hash(chunk) != *chunk_hash {
                return Err(NetError::Protocol(
                    "chunked blob descriptor hash mismatch".to_owned(),
                ));
            }
            let envelope = WireBlobEnvelope {
                hash: chunk_hash.clone(),
                bytes: chunk.to_vec(),
            };
            let envelope_bytes = message_field_encoded_len(envelope.encoded_len());
            if !batch.is_empty()
                && (batch.len() >= MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST
                    || batch_bytes.saturating_add(envelope_bytes) > MAX_CHUNK_UPLOAD_BATCH_BYTES)
            {
                let mut chunk_response = request_peer(
                    peer,
                    put_blob_chunks_request(&descriptor, std::mem::take(&mut batch)),
                )
                .await?;
                response_error(chunk_response.error.take())?;
                validate_empty_ack_response(&chunk_response)?;
                batch_bytes = chunk_frame_base_bytes;
            }
            batch_bytes = batch_bytes.saturating_add(envelope_bytes);
            batch.push(envelope);
        }

        if !batch.is_empty() {
            let mut chunk_response =
                request_peer(peer, put_blob_chunks_request(&descriptor, batch)).await?;
            response_error(chunk_response.error.take())?;
            validate_empty_ack_response(&chunk_response)?;
        }
        Ok(descriptor)
    }

    pub async fn fetch_blob_chunked(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError> {
        let requested_hashes = vec![hash.to_owned()];
        validate_request_blob_hashes(&requested_hashes)?;
        let mut manifest_response = request_peer(
            peer,
            WireSyncRequest {
                kind: WireSyncRequestKind::FetchBlobs as i32,
                event_ids: Vec::new(),
                events: Vec::new(),
                authorization_events: Vec::new(),
                authorization_snapshots: Vec::new(),
                blob_hashes: requested_hashes.clone(),
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                workspace_id: None,
                event_envelopes: Vec::new(),
                authorization_event_envelopes: Vec::new(),
                authorization_snapshot_envelopes: Vec::new(),
                inventory_start_index: None,
                inventory_limit: None,
            },
        )
        .await?;
        response_error(manifest_response.error.take())?;
        let requested = requested_hashes.into_iter().collect::<BTreeSet<_>>();
        validate_fetch_blobs_response(&manifest_response, &requested)?;

        if let Some(blob) = manifest_response
            .blobs
            .into_iter()
            .find(|blob| blob.hash == hash)
        {
            return Ok(Some(blob.bytes));
        }

        let Some(descriptor) = manifest_response
            .blob_descriptors
            .into_iter()
            .find(|descriptor| descriptor.hash == hash)
            .map(wire_to_descriptor)
            .transpose()?
        else {
            return Ok(None);
        };

        let chunk_hashes = descriptor
            .chunk_hashes
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let chunks = self.fetch_blobs(peer, chunk_hashes).await?;
        let mut bytes = Vec::new();
        for (chunk_index, chunk_hash) in descriptor.chunk_hashes.iter().enumerate() {
            let Some(chunk) = chunks.get(chunk_hash) else {
                return Ok(None);
            };
            validate_chunk_payload(&descriptor, chunk_index, chunk)
                .map_err(|error| NetError::Protocol(error.to_string()))?;
            bytes.extend_from_slice(chunk);
        }

        validate_reassembled_blob(&descriptor, &bytes)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        Ok(Some(bytes))
    }
}

#[async_trait]
impl AuthorizedPublishTransport for DirectTransport {
    async fn publish_events_with_authorization(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
        authorization_snapshots: Vec<SignedTrustSnapshot>,
    ) -> Result<(), NetError> {
        DirectTransport::publish_events_with_authorization(
            self,
            peer,
            events,
            authorization_events,
            authorization_snapshots,
        )
        .await
    }
}

#[async_trait]
impl BlobSyncTransport for DirectTransport {
    async fn put_blobs(
        &self,
        peer: &PeerAddress,
        blobs: Vec<Vec<u8>>,
    ) -> Result<Vec<String>, NetError> {
        DirectTransport::put_blobs(self, peer, blobs).await
    }

    async fn fetch_blobs(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, NetError> {
        DirectTransport::fetch_blobs(self, peer, hashes).await
    }

    async fn fetch_blob_availabilities(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, BlobAvailability>, NetError> {
        DirectTransport::fetch_blob_availabilities(self, peer, hashes).await
    }

    async fn put_blob_chunked(
        &self,
        peer: &PeerAddress,
        bytes: Vec<u8>,
        chunk_size: usize,
    ) -> Result<BlobDescriptor, NetError> {
        DirectTransport::put_blob_chunked(self, peer, bytes, chunk_size).await
    }

    async fn fetch_blob_chunked(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError> {
        DirectTransport::fetch_blob_chunked(self, peer, hash).await
    }
}

async fn request_peer(
    peer: &PeerAddress,
    request: WireSyncRequest,
) -> Result<WireSyncResponse, NetError> {
    let endpoint = validate_direct_peer_endpoint(peer)?;
    let mut stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(endpoint))
        .await
        .map_err(|_| {
            NetError::Protocol(format!(
                "direct TCP connect to {} timed out after {} ms",
                endpoint,
                CONNECT_TIMEOUT.as_millis()
            ))
        })??;
    request_sync_stream(&mut stream, request).await
}

fn validate_direct_peer_endpoint(peer: &PeerAddress) -> Result<&str, NetError> {
    let endpoint = peer.endpoint.trim();
    if direct_tcp_peer_endpoint_address_is_valid(endpoint) {
        return Ok(endpoint);
    }
    Err(NetError::Protocol(format!(
        "direct TCP endpoint must be host:port with nonzero numeric port: {}",
        peer.endpoint
    )))
}

async fn fetch_inventory_paged(
    peer: &PeerAddress,
    workspace_id: Option<&WorkspaceId>,
) -> Result<Vec<EventId>, NetError> {
    if let Some(workspace_id) = workspace_id {
        validate_wire_workspace_id("inventory", &workspace_id.0)?;
    }

    let mut start_index = 0usize;
    let mut event_ids = Vec::new();
    let mut seen = BTreeSet::new();

    loop {
        let response = DirectTransport::fetch_inventory_page(
            peer,
            workspace_id,
            start_index,
            MAX_INVENTORY_EVENT_IDS_PER_RESPONSE,
        )
        .await?;
        let Some(total_count) = response.inventory_total_count else {
            return Ok(response.event_ids.into_iter().map(EventId).collect());
        };
        let total_count = validate_inventory_total_count(total_count)?;
        validate_inventory_page_response(&response, MAX_INVENTORY_EVENT_IDS_PER_RESPONSE)?;
        if start_index.saturating_add(response.event_ids.len()) > total_count {
            return Err(NetError::Protocol(
                "peer returned inventory page past total count".to_owned(),
            ));
        }

        let page_len = response.event_ids.len();
        for event_id in response.event_ids {
            if !seen.insert(event_id.clone()) {
                return Err(NetError::Protocol(format!(
                    "peer returned duplicate inventory event id {event_id}"
                )));
            }
            event_ids.push(EventId(event_id));
        }

        if event_ids.len() >= total_count {
            return Ok(event_ids);
        }
        if page_len == 0 {
            return Err(NetError::Protocol(
                "peer returned empty inventory page before total count".to_owned(),
            ));
        }
        start_index = start_index.saturating_add(page_len);
    }
}

async fn fetch_events_batched(
    peer: &PeerAddress,
    event_ids: Vec<EventId>,
) -> Result<Vec<SignedEvent>, NetError> {
    let event_ids = deduplicate_event_ids(event_ids);
    let mut pending = event_ids
        .chunks(MAX_FETCH_EVENT_IDS_PER_REQUEST)
        .rev()
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    let mut events = Vec::new();

    while let Some(batch) = pending.pop() {
        match fetch_events_once(peer, batch.clone()).await {
            Ok(mut fetched) => events.append(&mut fetched),
            Err(error)
                if batch.len() > 1 && fetch_events_error_may_be_oversized_response(&error) =>
            {
                let mid = batch.len() / 2;
                pending.push(batch[mid..].to_vec());
                pending.push(batch[..mid].to_vec());
            }
            Err(error) => return Err(error),
        }
    }

    Ok(events)
}

async fn fetch_events_once(
    peer: &PeerAddress,
    event_ids: Vec<EventId>,
) -> Result<Vec<SignedEvent>, NetError> {
    if event_ids.is_empty() {
        return Ok(Vec::new());
    }

    validate_request_event_ids(event_ids.iter().map(|event_id| event_id.0.as_str()))?;
    let requested = event_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut response = request_peer(peer, fetch_events_request(event_ids)).await?;
    response_error(response.error.take())?;
    validate_fetch_events_wire_response(&response, requested.len())?;
    let events = decode_events(response.event_envelopes, response.events)?;
    validate_fetch_events_response(&events, &requested)?;
    Ok(events)
}

fn deduplicate_event_ids(event_ids: Vec<EventId>) -> Vec<EventId> {
    let mut seen = BTreeSet::new();
    event_ids
        .into_iter()
        .filter(|event_id| seen.insert(event_id.clone()))
        .collect()
}

pub fn validate_fetch_events_wire_response(
    response: &WireSyncResponse,
    requested_event_count: usize,
) -> Result<(), NetError> {
    validate_response_shape(
        response,
        "unexpected fetch-events",
        AllowedResponseFields {
            events: true,
            event_envelopes: true,
            ..AllowedResponseFields::empty()
        },
    )?;
    validate_response_item_count(
        "fetch-events event",
        response
            .event_envelopes
            .len()
            .saturating_add(response.events.len()),
        requested_event_count,
    )
}

pub fn validate_fetch_events_response(
    events: &[SignedEvent],
    requested_event_ids: &BTreeSet<EventId>,
) -> Result<(), NetError> {
    let mut seen_event_ids = BTreeSet::new();
    for event in events {
        if !requested_event_ids.contains(&event.event_id) {
            return Err(NetError::Protocol(format!(
                "peer returned unrequested event {}",
                event.event_id
            )));
        }
        if !seen_event_ids.insert(event.event_id.clone()) {
            return Err(NetError::Protocol(format!(
                "peer returned duplicate event {}",
                event.event_id
            )));
        }
    }
    Ok(())
}

pub fn validate_inventory_event_ids(event_ids: &[String]) -> Result<(), NetError> {
    let mut seen = BTreeSet::new();
    for event_id in event_ids {
        if !is_canonical_wire_event_id(event_id) {
            return Err(NetError::Protocol(
                "peer returned non-canonical inventory event id".to_owned(),
            ));
        }
        if !seen.insert(event_id.as_str()) {
            return Err(NetError::Protocol(format!(
                "peer returned duplicate inventory event id {event_id}"
            )));
        }
    }
    Ok(())
}

pub fn validate_inventory_response(response: &WireSyncResponse) -> Result<(), NetError> {
    validate_response_shape(
        response,
        "unexpected inventory",
        AllowedResponseFields {
            event_ids: true,
            inventory_total_count: true,
            ..AllowedResponseFields::empty()
        },
    )?;
    validate_inventory_event_ids(&response.event_ids)
}

pub fn validate_inventory_page_response(
    response: &WireSyncResponse,
    requested_limit: usize,
) -> Result<(), NetError> {
    if response.event_ids.len() > requested_limit {
        return Err(NetError::Protocol(format!(
            "peer returned inventory page count {} exceeds requested limit {}",
            response.event_ids.len(),
            requested_limit
        )));
    }
    Ok(())
}

pub fn validate_inventory_total_count(total_count: u64) -> Result<usize, NetError> {
    let total_count = usize::try_from(total_count).map_err(|_| {
        NetError::Protocol("peer returned inventory total count too large".to_owned())
    })?;
    if total_count <= MAX_INVENTORY_EVENT_IDS_PER_PULL {
        return Ok(total_count);
    }

    Err(NetError::Protocol(format!(
        "peer returned inventory total count {total_count} exceeds max {MAX_INVENTORY_EVENT_IDS_PER_PULL}"
    )))
}

fn validate_response_item_count(context: &str, count: usize, max: usize) -> Result<(), NetError> {
    if count <= max {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "peer returned {context} count {count} exceeds requested limit {max}"
    )))
}

pub fn validate_empty_ack_response(response: &WireSyncResponse) -> Result<(), NetError> {
    validate_response_shape(response, "non-empty ack", AllowedResponseFields::empty())
}

#[derive(Clone, Copy)]
struct AllowedResponseFields {
    event_ids: bool,
    events: bool,
    blobs: bool,
    blob_descriptors: bool,
    blob_availability: bool,
    event_envelopes: bool,
    inventory_total_count: bool,
}

impl AllowedResponseFields {
    const fn empty() -> Self {
        Self {
            event_ids: false,
            events: false,
            blobs: false,
            blob_descriptors: false,
            blob_availability: false,
            event_envelopes: false,
            inventory_total_count: false,
        }
    }
}

fn validate_response_shape(
    response: &WireSyncResponse,
    context: &str,
    allowed: AllowedResponseFields,
) -> Result<(), NetError> {
    let mut fields = Vec::new();
    if !allowed.event_ids && !response.event_ids.is_empty() {
        fields.push("event_ids");
    }
    if !allowed.events && !response.events.is_empty() {
        fields.push("events");
    }
    if !allowed.blobs && !response.blobs.is_empty() {
        fields.push("blobs");
    }
    if !allowed.blob_descriptors && !response.blob_descriptors.is_empty() {
        fields.push("blob_descriptors");
    }
    if !allowed.blob_availability && !response.blob_availability.is_empty() {
        fields.push("blob_availability");
    }
    if !allowed.event_envelopes && !response.event_envelopes.is_empty() {
        fields.push("event_envelopes");
    }
    if !allowed.inventory_total_count && response.inventory_total_count.is_some() {
        fields.push("inventory_total_count");
    }

    if fields.is_empty() {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "peer returned {context} response fields: {}",
        fields.join(", ")
    )))
}

fn is_canonical_wire_event_id(event_id: &str) -> bool {
    is_canonical_event_id_str(event_id)
}

pub fn validate_request_event_ids<'a>(
    event_ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), NetError> {
    for event_id in event_ids {
        if !is_canonical_wire_event_id(event_id) {
            return Err(NetError::Protocol(
                "peer requested non-canonical event id".to_owned(),
            ));
        }
    }
    Ok(())
}

pub fn validate_request_blob_hashes(hashes: &[String]) -> Result<(), NetError> {
    for hash in hashes {
        if !is_canonical_wire_blob_hash(hash) {
            return Err(NetError::Protocol(
                "peer requested non-canonical blob hash".to_owned(),
            ));
        }
    }
    Ok(())
}

fn is_canonical_wire_blob_hash(hash: &str) -> bool {
    const BLAKE3_HEX_LEN: usize = 64;

    hash.len() == BLAKE3_HEX_LEN
        && hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

async fn put_blob_envelopes_batched(
    peer: &PeerAddress,
    envelopes: Vec<WireBlobEnvelope>,
) -> Result<(), NetError> {
    let base_bytes = encode_sync_request(&put_blob_envelopes_request(Vec::new())).len();
    let mut batch = Vec::new();
    let mut batch_bytes = base_bytes;

    for envelope in envelopes {
        let envelope_bytes = message_field_encoded_len(envelope.encoded_len());
        let single_frame_bytes = base_bytes.saturating_add(envelope_bytes);
        if single_frame_bytes > MAX_FRAME_LEN {
            return Err(NetError::Protocol(format!(
                "blob upload frame length {single_frame_bytes} exceeds max {MAX_FRAME_LEN}"
            )));
        }

        if !batch.is_empty()
            && (batch.len() >= MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST
                || batch_bytes.saturating_add(envelope_bytes) > MAX_WHOLE_BLOB_UPLOAD_BATCH_BYTES)
        {
            let mut response =
                request_peer(peer, put_blob_envelopes_request(std::mem::take(&mut batch))).await?;
            response_error(response.error.take())?;
            validate_empty_ack_response(&response)?;
            batch_bytes = base_bytes;
        }

        batch_bytes = batch_bytes.saturating_add(envelope_bytes);
        batch.push(envelope);
    }

    if !batch.is_empty() {
        let mut response = request_peer(peer, put_blob_envelopes_request(batch)).await?;
        response_error(response.error.take())?;
        validate_empty_ack_response(&response)?;
    }

    Ok(())
}

fn put_blob_envelopes_request(blobs: Vec<WireBlobEnvelope>) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs,
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
}

async fn fetch_blobs_responses_batched(
    peer: &PeerAddress,
    hashes: Vec<String>,
) -> Result<Vec<WireSyncResponse>, NetError> {
    let hashes = deduplicate_strings(hashes);
    let mut pending = hashes
        .chunks(MAX_FETCH_BLOB_HASHES_PER_REQUEST)
        .rev()
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    let mut responses = Vec::new();

    while let Some(batch) = pending.pop() {
        match fetch_blobs_response_once(peer, batch.clone()).await {
            Ok(response) => responses.push(response),
            Err(error) if batch.len() > 1 && response_error_may_be_oversized_response(&error) => {
                let mid = batch.len() / 2;
                pending.push(batch[mid..].to_vec());
                pending.push(batch[..mid].to_vec());
            }
            Err(error) => return Err(error),
        }
    }

    Ok(responses)
}

async fn fetch_blob_availability_responses_batched(
    peer: &PeerAddress,
    hashes: Vec<String>,
) -> Result<Vec<WireSyncResponse>, NetError> {
    let hashes = deduplicate_strings(hashes);
    let mut pending = hashes
        .chunks(MAX_FETCH_BLOB_HASHES_PER_REQUEST)
        .rev()
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<_>>();
    let mut responses = Vec::new();

    while let Some(batch) = pending.pop() {
        match fetch_blob_availability_response_once(peer, batch.clone()).await {
            Ok(response) => responses.push(response),
            Err(error) if batch.len() > 1 && response_error_may_be_oversized_response(&error) => {
                let mid = batch.len() / 2;
                pending.push(batch[mid..].to_vec());
                pending.push(batch[..mid].to_vec());
            }
            Err(error) => return Err(error),
        }
    }

    Ok(responses)
}

fn deduplicate_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

fn whole_blob_upload_envelopes(blobs: Vec<Vec<u8>>) -> (Vec<WireBlobEnvelope>, Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut envelopes = Vec::new();
    let mut hashes = Vec::new();
    for bytes in blobs {
        let hash = blob_hash(&bytes);
        if seen.insert(hash.clone()) {
            hashes.push(hash.clone());
            envelopes.push(WireBlobEnvelope { hash, bytes });
        }
    }
    (envelopes, hashes)
}

fn validate_chunk_upload_single_frame_lengths(
    descriptor: &BlobDescriptor,
    bytes: &[u8],
) -> Result<(), NetError> {
    let base_bytes = encode_sync_request(&put_blob_chunks_request(descriptor, Vec::new())).len();
    if base_bytes > MAX_FRAME_LEN {
        return Err(NetError::Protocol(format!(
            "chunk upload frame length {base_bytes} exceeds max {MAX_FRAME_LEN}"
        )));
    }

    for (chunk_hash, chunk) in descriptor
        .chunk_hashes
        .iter()
        .zip(bytes.chunks(descriptor.chunk_size))
    {
        let envelope = WireBlobEnvelope {
            hash: chunk_hash.clone(),
            bytes: chunk.to_vec(),
        };
        let frame_len =
            base_bytes.saturating_add(message_field_encoded_len(envelope.encoded_len()));
        if frame_len > MAX_FRAME_LEN {
            return Err(NetError::Protocol(format!(
                "chunk upload frame length {frame_len} exceeds max {MAX_FRAME_LEN}"
            )));
        }
    }

    Ok(())
}

async fn fetch_blobs_response_once(
    peer: &PeerAddress,
    hashes: Vec<String>,
) -> Result<WireSyncResponse, NetError> {
    if hashes.is_empty() {
        return Ok(WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        });
    }

    validate_request_blob_hashes(&hashes)?;
    let requested = hashes.iter().cloned().collect::<BTreeSet<_>>();
    let mut response = request_peer(peer, fetch_blobs_request(hashes)).await?;
    response_error(response.error.take())?;
    validate_fetch_blobs_response(&response, &requested)?;
    Ok(response)
}

async fn fetch_blob_availability_response_once(
    peer: &PeerAddress,
    hashes: Vec<String>,
) -> Result<WireSyncResponse, NetError> {
    if hashes.is_empty() {
        return Ok(WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        });
    }

    validate_request_blob_hashes(&hashes)?;
    let requested = hashes.iter().cloned().collect::<BTreeSet<_>>();
    let mut response = request_peer(peer, fetch_blob_availability_request(hashes)).await?;
    response_error(response.error.take())?;
    validate_fetch_blob_availability_response(&response, &requested)?;
    Ok(response)
}

pub fn validate_fetch_blobs_response(
    response: &WireSyncResponse,
    requested_hashes: &BTreeSet<String>,
) -> Result<(), NetError> {
    validate_response_shape(
        response,
        "unexpected fetch-blobs",
        AllowedResponseFields {
            blobs: true,
            blob_descriptors: true,
            blob_availability: true,
            ..AllowedResponseFields::empty()
        },
    )?;
    validate_response_item_count(
        "fetch-blobs blob",
        response.blobs.len(),
        requested_hashes.len(),
    )?;
    validate_response_item_count(
        "fetch-blobs descriptor",
        response.blob_descriptors.len(),
        requested_hashes.len(),
    )?;
    validate_response_item_count(
        "fetch-blobs availability",
        response.blob_availability.len(),
        requested_hashes.len(),
    )?;

    let mut blob_hashes = BTreeSet::new();
    for blob in &response.blobs {
        validate_requested_blob_hash("blob", &blob.hash, requested_hashes)?;
        validate_unique_response_blob_hash("blob", &blob.hash, &mut blob_hashes)?;
        let actual = blob_hash(&blob.bytes);
        if actual != blob.hash {
            return Err(NetError::Protocol(format!(
                "fetched blob hash mismatch: expected {}, actual {}",
                blob.hash, actual
            )));
        }
    }

    let mut descriptor_hashes = BTreeSet::new();
    for descriptor in &response.blob_descriptors {
        validate_requested_blob_hash("blob descriptor", &descriptor.hash, requested_hashes)?;
        validate_unique_response_blob_hash(
            "blob descriptor",
            &descriptor.hash,
            &mut descriptor_hashes,
        )?;
        let descriptor = wire_to_descriptor(descriptor.clone())?;
        validate_blob_descriptor(&descriptor)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
    }

    let mut availability_hashes = BTreeSet::new();
    for availability in &response.blob_availability {
        validate_requested_blob_hash("blob availability", &availability.hash, requested_hashes)?;
        validate_unique_response_blob_hash(
            "blob availability",
            &availability.hash,
            &mut availability_hashes,
        )?;
        let availability = wire_to_availability(availability.clone())?;
        validate_blob_availability(&availability)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
    }

    Ok(())
}

pub fn validate_fetch_blob_availability_response(
    response: &WireSyncResponse,
    requested_hashes: &BTreeSet<String>,
) -> Result<(), NetError> {
    validate_response_shape(
        response,
        "unexpected fetch-blob-availability",
        AllowedResponseFields {
            blob_availability: true,
            ..AllowedResponseFields::empty()
        },
    )?;
    validate_response_item_count(
        "fetch-blob-availability availability",
        response.blob_availability.len(),
        requested_hashes.len(),
    )?;

    let mut availability_hashes = BTreeSet::new();
    for availability in &response.blob_availability {
        validate_requested_blob_hash("blob availability", &availability.hash, requested_hashes)?;
        validate_unique_response_blob_hash(
            "blob availability",
            &availability.hash,
            &mut availability_hashes,
        )?;
        let availability = wire_to_availability(availability.clone())?;
        validate_blob_availability(&availability)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
    }

    Ok(())
}

fn validate_unique_response_blob_hash(
    kind: &str,
    hash: &str,
    seen_hashes: &mut BTreeSet<String>,
) -> Result<(), NetError> {
    if seen_hashes.insert(hash.to_owned()) {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "peer returned duplicate {kind} {hash}"
    )))
}

fn validate_requested_blob_hash(
    kind: &str,
    hash: &str,
    requested_hashes: &BTreeSet<String>,
) -> Result<(), NetError> {
    if !is_canonical_wire_blob_hash(hash) {
        return Err(NetError::Protocol(format!(
            "peer returned non-canonical {kind} hash"
        )));
    }
    if requested_hashes.contains(hash) {
        return Ok(());
    }
    Err(NetError::Protocol(format!(
        "peer returned unrequested {kind} {hash}"
    )))
}

fn fetch_blobs_request(hashes: Vec<String>) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::FetchBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: hashes,
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
}

fn fetch_blob_availability_request(hashes: Vec<String>) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::FetchBlobAvailability as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: hashes,
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
}

fn fetch_events_request(event_ids: Vec<EventId>) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::FetchEvents as i32,
        event_ids: event_ids.into_iter().map(|id| id.0).collect(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
}

pub fn fetch_events_error_may_be_oversized_response(error: &NetError) -> bool {
    response_error_may_be_oversized_response(error)
}

pub fn response_error_may_be_oversized_response(error: &NetError) -> bool {
    match error {
        NetError::Protocol(message) => message.contains("frame length"),
        NetError::Io(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("early eof")
                || message.contains("failed to fill whole buffer")
                || message.contains("unexpected end of file")
                || message.contains("connection reset")
                || message.contains("broken pipe")
        }
        NetError::Unavailable(_) => false,
    }
}

pub async fn request_sync_stream<S>(
    stream: &mut S,
    request: WireSyncRequest,
) -> Result<WireSyncResponse, NetError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    write_frame(stream, &encode_sync_request(&request)).await?;
    let response_bytes = read_frame(stream).await?;
    decode_sync_response(&response_bytes).map_err(|error| NetError::Protocol(error.to_string()))
}

async fn handle_connection(
    mut stream: TcpStream,
    sync_store: SyncPeerStore,
) -> Result<(), NetError> {
    sync_store.serve_stream(&mut stream).await
}

async fn handle_sync_stream<S>(
    stream: &mut S,
    store: Arc<Mutex<EventStore>>,
    blob_store: Option<Arc<BlobStore>>,
) -> Result<(), NetError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request_bytes = read_frame(stream).await?;
    let response = match decode_sync_request(&request_bytes)
        .map_err(|error| NetError::Protocol(error.to_string()))
        .and_then(|request| handle_request(request, store, blob_store))
    {
        Ok(response) => response,
        Err(error) => sync_error_response(error),
    };
    write_frame(stream, &encode_sync_response_with_frame_limit(response)).await?;
    stream.shutdown().await?;
    Ok(())
}

fn sync_error_response(error: NetError) -> WireSyncResponse {
    WireSyncResponse {
        event_ids: Vec::new(),
        events: Vec::new(),
        error: Some(bounded_sync_response_error(&error.to_string())),
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        blob_availability: Vec::new(),
        event_envelopes: Vec::new(),
        inventory_total_count: None,
    }
}

fn encode_sync_response_with_frame_limit(response: WireSyncResponse) -> Vec<u8> {
    let response_bytes = encode_sync_response(&response);
    if response_bytes.len() <= MAX_FRAME_LEN {
        return response_bytes;
    }

    encode_sync_response(&sync_error_response(oversized_sync_response_error(
        "sync",
        response_bytes.len(),
    )))
}

fn validate_sync_response_frame_len(
    context: &str,
    response: &WireSyncResponse,
) -> Result<(), NetError> {
    let len = response.encoded_len();
    if len <= MAX_FRAME_LEN {
        return Ok(());
    }

    Err(oversized_sync_response_error(context, len))
}

fn oversized_sync_response_error(context: &str, len: usize) -> NetError {
    NetError::Protocol(format!(
        "{context} response frame length {len} exceeds max {MAX_FRAME_LEN}"
    ))
}

fn bounded_sync_response_error(error: &str) -> String {
    if error.len() <= MAX_SYNC_RESPONSE_ERROR_BYTES {
        return error.to_owned();
    }

    let mut end =
        MAX_SYNC_RESPONSE_ERROR_BYTES.saturating_sub(SYNC_RESPONSE_ERROR_TRUNCATED_SUFFIX.len());
    while end > 0 && !error.is_char_boundary(end) {
        end -= 1;
    }

    let mut bounded = error[..end].to_owned();
    bounded.push_str(SYNC_RESPONSE_ERROR_TRUNCATED_SUFFIX);
    bounded
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryPageRequest {
    start_index: usize,
    limit: usize,
}

fn inventory_page_request(
    request: &WireSyncRequest,
) -> Result<Option<InventoryPageRequest>, NetError> {
    match (request.inventory_start_index, request.inventory_limit) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(NetError::Protocol(
            "inventory_start_index requires inventory_limit".to_owned(),
        )),
        (start_index, Some(limit)) => {
            let start_index =
                inventory_page_value("inventory_start_index", start_index.unwrap_or_default())?;
            let limit = inventory_page_value("inventory_limit", limit)?;
            validate_request_item_count(
                "inventory page",
                limit,
                MAX_INVENTORY_EVENT_IDS_PER_RESPONSE,
            )?;
            Ok(Some(InventoryPageRequest { start_index, limit }))
        }
    }
}

fn inventory_page_value(context: &str, value: u64) -> Result<usize, NetError> {
    usize::try_from(value).map_err(|_| NetError::Protocol(format!("{context} is too large")))
}

fn handle_request(
    request: WireSyncRequest,
    store: Arc<Mutex<EventStore>>,
    blob_store: Option<Arc<BlobStore>>,
) -> Result<WireSyncResponse, NetError> {
    let kind = WireSyncRequestKind::try_from(request.kind)
        .map_err(|_| NetError::Protocol(format!("unknown sync request kind {}", request.kind)))?;

    match kind {
        WireSyncRequestKind::Inventory => {
            validate_request_shape(
                &request,
                "inventory",
                AllowedRequestFields {
                    workspace_id: true,
                    inventory_start_index: true,
                    inventory_limit: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_workspace_id_option("inventory", request.workspace_id.as_deref())?;
            let inventory_page =
                inventory_page_request(&request)?.unwrap_or(InventoryPageRequest {
                    start_index: 0,
                    limit: MAX_INVENTORY_EVENT_IDS_PER_RESPONSE,
                });
            let (event_ids, inventory_total_count) = {
                let store = lock_event_store(&store)?;
                match request.workspace_id {
                    Some(workspace_id) => {
                        let total_count = store
                            .count_servable_events_for_workspace(&workspace_id)
                            .map_err(|error| NetError::Protocol(error.to_string()))?;
                        let event_ids = store
                            .list_servable_event_ids_for_workspace_page(
                                &workspace_id,
                                inventory_page.start_index,
                                inventory_page.limit,
                            )
                            .map_err(|error| NetError::Protocol(error.to_string()))?;
                        (event_ids, Some(total_count as u64))
                    }
                    None => {
                        let total_count = store
                            .count_servable_events()
                            .map_err(|error| NetError::Protocol(error.to_string()))?;
                        let event_ids = store
                            .list_servable_event_ids_page(
                                inventory_page.start_index,
                                inventory_page.limit,
                            )
                            .map_err(|error| NetError::Protocol(error.to_string()))?;
                        (event_ids, Some(total_count as u64))
                    }
                }
            };
            Ok(WireSyncResponse {
                event_ids: event_ids.into_iter().map(|event_id| event_id.0).collect(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count,
            })
        }
        WireSyncRequestKind::FetchEvents => {
            validate_request_shape(
                &request,
                "fetch-events",
                AllowedRequestFields {
                    event_ids: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_item_count(
                "fetch-events event id",
                request.event_ids.len(),
                MAX_FETCH_EVENT_IDS_PER_REQUEST,
            )?;
            validate_request_items_unique("fetch-events event id", &request.event_ids)?;
            validate_request_event_ids(request.event_ids.iter().map(String::as_str))?;
            let events = {
                let store = lock_event_store(&store)?;
                let mut events = Vec::new();
                for event_id in request.event_ids {
                    if let Some(event) = store
                        .get_servable_event(&EventId(event_id))
                        .map_err(|error| NetError::Protocol(error.to_string()))?
                    {
                        events.push(event);
                    }
                }
                events
            };
            let mut response = WireSyncResponse {
                event_ids: Vec::new(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: None,
            };
            for event in &events {
                response.event_envelopes.push(encode_event_envelope(event));
                validate_sync_response_frame_len("fetch-events", &response)?;
            }
            Ok(response)
        }
        WireSyncRequestKind::PublishEvents => {
            validate_request_shape(
                &request,
                "publish-events",
                AllowedRequestFields {
                    events: true,
                    authorization_events: true,
                    authorization_snapshots: true,
                    workspace_id: true,
                    event_envelopes: true,
                    authorization_event_envelopes: true,
                    authorization_snapshot_envelopes: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_workspace_id_option(
                "publish-events",
                request.workspace_id.as_deref(),
            )?;
            validate_request_item_count(
                "publish-events event",
                request
                    .event_envelopes
                    .len()
                    .saturating_add(request.events.len()),
                MAX_PUBLISH_EVENTS_PER_REQUEST,
            )?;
            validate_request_item_count(
                "publish-events authorization event",
                request
                    .authorization_event_envelopes
                    .len()
                    .saturating_add(request.authorization_events.len()),
                MAX_AUTHORIZATION_EVENTS_PER_REQUEST,
            )?;
            validate_request_item_count(
                "publish-events authorization snapshot",
                request
                    .authorization_snapshot_envelopes
                    .len()
                    .saturating_add(request.authorization_snapshots.len()),
                MAX_AUTHORIZATION_SNAPSHOTS_PER_REQUEST,
            )?;
            let proof_events = decode_and_verify_events(
                request.authorization_event_envelopes,
                request.authorization_events,
            )?;
            let trust_snapshots = decode_and_verify_trust_snapshots(
                request.authorization_snapshot_envelopes,
                request.authorization_snapshots,
            )?;
            let publish_events = decode_and_verify_events(request.event_envelopes, request.events)?;
            validate_publish_event_sets_unique(&proof_events, &publish_events)?;
            validate_trust_snapshots_unique(&trust_snapshots)?;
            let workspace_id = publish_request_workspace_id(
                request.workspace_id.as_deref(),
                &proof_events,
                &trust_snapshots,
                &publish_events,
            )?;

            let store = lock_event_store(&store)?;
            let mut history = match workspace_id {
                Some(workspace_id) => store
                    .list_servable_events_for_workspace(&workspace_id.0)
                    .map_err(|error| NetError::Protocol(error.to_string()))?,
                None => Vec::new(),
            };

            authorize_proof_events(&mut history, proof_events)?;
            authorize_and_store_publish_events(
                &store,
                &mut history,
                &trust_snapshots,
                publish_events,
            )?;

            Ok(WireSyncResponse {
                event_ids: Vec::new(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: None,
            })
        }
        WireSyncRequestKind::PutBlobs => {
            validate_request_shape(
                &request,
                "put-blobs",
                AllowedRequestFields {
                    blobs: true,
                    blob_descriptors: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_item_count(
                "put-blobs blob",
                request.blobs.len(),
                MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST,
            )?;
            validate_request_item_count(
                "put-blobs descriptor",
                request.blob_descriptors.len(),
                MAX_BLOB_UPLOAD_DESCRIPTORS_PER_REQUEST,
            )?;
            let descriptors = validate_put_blobs_request_uploads(&request)?;
            let blob_store = blob_store
                .as_ref()
                .ok_or_else(|| NetError::Protocol("blob store unavailable".to_owned()))?;
            if descriptors.is_empty() {
                for blob in request.blobs {
                    blob_store
                        .put_bytes_with_hash(&blob.hash, &blob.bytes)
                        .map_err(|error| NetError::Protocol(error.to_string()))?;
                }
            } else {
                for descriptor in &descriptors {
                    blob_store
                        .put_manifest(descriptor)
                        .map_err(|error| NetError::Protocol(error.to_string()))?;
                }
                for blob in request.blobs {
                    blob_store
                        .put_chunk_with_hash(&blob.hash, &blob.bytes)
                        .map_err(|error| NetError::Protocol(error.to_string()))?;
                }
            }
            Ok(WireSyncResponse {
                event_ids: Vec::new(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: None,
            })
        }
        WireSyncRequestKind::FetchBlobs => {
            validate_request_shape(
                &request,
                "fetch-blobs",
                AllowedRequestFields {
                    blob_hashes: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_item_count(
                "fetch-blobs blob hash",
                request.blob_hashes.len(),
                MAX_FETCH_BLOB_HASHES_PER_REQUEST,
            )?;
            validate_request_items_unique("fetch-blobs blob hash", &request.blob_hashes)?;
            validate_request_blob_hashes(&request.blob_hashes)?;
            let blob_store = blob_store
                .as_ref()
                .ok_or_else(|| NetError::Protocol("blob store unavailable".to_owned()))?;
            let mut response = WireSyncResponse {
                event_ids: Vec::new(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: None,
            };
            for hash in request.blob_hashes {
                if let Some(availability) = blob_store
                    .availability(&hash)
                    .map_err(|error| NetError::Protocol(error.to_string()))?
                {
                    response
                        .blob_availability
                        .push(availability_to_wire(&availability));
                    validate_sync_response_frame_len("fetch-blobs", &response)?;
                }
                if let Some(bytes) = blob_store
                    .get_bytes(&hash)
                    .map_err(|error| NetError::Protocol(error.to_string()))?
                {
                    response.blobs.push(WireBlobEnvelope {
                        hash: hash.clone(),
                        bytes,
                    });
                    validate_sync_response_frame_len("fetch-blobs", &response)?;
                } else if let Some(bytes) = blob_store
                    .get_chunk(&hash)
                    .map_err(|error| NetError::Protocol(error.to_string()))?
                {
                    response.blobs.push(WireBlobEnvelope {
                        hash: hash.clone(),
                        bytes,
                    });
                    validate_sync_response_frame_len("fetch-blobs", &response)?;
                }
                if let Some(descriptor) = blob_store
                    .get_manifest(&hash)
                    .map_err(|error| NetError::Protocol(error.to_string()))?
                {
                    response
                        .blob_descriptors
                        .push(descriptor_to_wire(&descriptor));
                    validate_sync_response_frame_len("fetch-blobs", &response)?;
                }
            }
            Ok(response)
        }
        WireSyncRequestKind::FetchBlobAvailability => {
            validate_request_shape(
                &request,
                "fetch-blob-availability",
                AllowedRequestFields {
                    blob_hashes: true,
                    ..AllowedRequestFields::empty()
                },
            )?;
            validate_request_item_count(
                "fetch-blob-availability blob hash",
                request.blob_hashes.len(),
                MAX_FETCH_BLOB_HASHES_PER_REQUEST,
            )?;
            validate_request_items_unique(
                "fetch-blob-availability blob hash",
                &request.blob_hashes,
            )?;
            validate_request_blob_hashes(&request.blob_hashes)?;
            let blob_store = blob_store
                .as_ref()
                .ok_or_else(|| NetError::Protocol("blob store unavailable".to_owned()))?;
            let mut response = WireSyncResponse {
                event_ids: Vec::new(),
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: None,
            };
            for hash in request.blob_hashes {
                if let Some(availability) = blob_store
                    .availability(&hash)
                    .map_err(|error| NetError::Protocol(error.to_string()))?
                {
                    response
                        .blob_availability
                        .push(availability_to_wire(&availability));
                    validate_sync_response_frame_len("fetch-blob-availability", &response)?;
                }
            }
            Ok(response)
        }
        WireSyncRequestKind::Unspecified => Ok(WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: Some("unspecified sync request kind".to_owned()),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        }),
    }
}

fn validate_request_item_count(context: &str, count: usize, max: usize) -> Result<(), NetError> {
    if count <= max {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "{context} count {count} exceeds max {max}"
    )))
}

fn validate_request_items_unique(context: &str, values: &[String]) -> Result<(), NetError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(NetError::Protocol(format!("{context} duplicate value")));
        }
    }
    Ok(())
}

fn validate_request_workspace_id_option(
    context: &str,
    workspace_id: Option<&str>,
) -> Result<(), NetError> {
    if let Some(workspace_id) = workspace_id {
        validate_wire_workspace_id(context, workspace_id)?;
    }
    Ok(())
}

pub fn validate_wire_workspace_id(context: &str, workspace_id: &str) -> Result<(), NetError> {
    if workspace_id.trim().is_empty() {
        return Err(NetError::Protocol(format!(
            "{context} workspace ID is blank"
        )));
    }
    if workspace_id.trim() != workspace_id {
        return Err(NetError::Protocol(format!(
            "{context} workspace ID must be trimmed"
        )));
    }
    validate_workspace_id_str(workspace_id).map_err(|error| NetError::Protocol(error.to_string()))
}

fn validate_put_blobs_request_uploads(
    request: &WireSyncRequest,
) -> Result<Vec<BlobDescriptor>, NetError> {
    validate_put_blobs_request_uniqueness(request)?;
    if request.blob_descriptors.is_empty() {
        validate_whole_blob_upload_envelopes(&request.blobs)?;
        return Ok(Vec::new());
    }

    let descriptors = request
        .blob_descriptors
        .iter()
        .cloned()
        .map(wire_to_descriptor)
        .collect::<Result<Vec<_>, _>>()?;
    validate_chunk_uploads_declared_by_descriptors(&request.blobs, &descriptors)?;
    validate_chunk_upload_payloads(&request.blobs, &descriptors)?;
    Ok(descriptors)
}

fn validate_put_blobs_request_uniqueness(request: &WireSyncRequest) -> Result<(), NetError> {
    if request.blob_descriptors.is_empty() {
        validate_blob_envelope_hashes_unique("put-blobs blob", &request.blobs)?;
    } else {
        validate_blob_envelope_hashes_unique("put-blobs chunk", &request.blobs)?;
    }
    validate_blob_descriptor_hashes_unique("put-blobs descriptor", &request.blob_descriptors)
}

fn validate_whole_blob_upload_envelopes(blobs: &[WireBlobEnvelope]) -> Result<(), NetError> {
    for blob in blobs {
        validate_canonical_wire_blob_hash("put-blobs blob", &blob.hash)?;
        validate_blob_envelope_content_hash("put-blobs blob", blob)?;
    }
    Ok(())
}

fn validate_chunk_upload_payloads(
    blobs: &[WireBlobEnvelope],
    descriptors: &[BlobDescriptor],
) -> Result<(), NetError> {
    let expected_lengths = descriptor_chunk_lengths_by_hash(descriptors)?;
    for blob in blobs {
        validate_canonical_wire_blob_hash("put-blobs chunk", &blob.hash)?;
        validate_blob_envelope_content_hash("put-blobs chunk", blob)?;
        let Some(lengths) = expected_lengths.get(&blob.hash) else {
            return Err(NetError::Protocol(format!(
                "put-blobs chunk {} not declared by descriptor",
                blob.hash
            )));
        };
        if !lengths.contains(&blob.bytes.len()) {
            return Err(NetError::Protocol(format!(
                "put-blobs chunk {} byte length {} does not match descriptor",
                blob.hash,
                blob.bytes.len()
            )));
        }
    }
    Ok(())
}

fn descriptor_chunk_lengths_by_hash(
    descriptors: &[BlobDescriptor],
) -> Result<HashMap<String, BTreeSet<usize>>, NetError> {
    let mut lengths_by_hash: HashMap<String, BTreeSet<usize>> = HashMap::new();
    for descriptor in descriptors {
        for (chunk_index, chunk_hash) in descriptor.chunk_hashes.iter().enumerate() {
            lengths_by_hash
                .entry(chunk_hash.clone())
                .or_default()
                .insert(descriptor_chunk_byte_len(descriptor, chunk_index)?);
        }
    }
    Ok(lengths_by_hash)
}

fn descriptor_chunk_byte_len(
    descriptor: &BlobDescriptor,
    chunk_index: usize,
) -> Result<usize, NetError> {
    if chunk_index + 1 < descriptor.chunk_hashes.len() {
        return Ok(descriptor.chunk_size);
    }

    let chunk_size = u64::try_from(descriptor.chunk_size)
        .map_err(|_| NetError::Protocol("blob descriptor chunk size overflows u64".to_owned()))?;
    let chunks_before = u64::try_from(chunk_index)
        .map_err(|_| NetError::Protocol("blob descriptor chunk index overflows u64".to_owned()))?;
    let consumed_before = chunks_before
        .checked_mul(chunk_size)
        .ok_or_else(|| NetError::Protocol("blob descriptor byte length overflows".to_owned()))?;
    let remaining = descriptor
        .byte_len
        .checked_sub(consumed_before)
        .ok_or_else(|| NetError::Protocol("invalid blob descriptor".to_owned()))?;
    usize::try_from(remaining).map_err(|_| {
        NetError::Protocol("blob descriptor chunk byte length overflows usize".to_owned())
    })
}

fn validate_blob_envelope_content_hash(
    context: &str,
    blob: &WireBlobEnvelope,
) -> Result<(), NetError> {
    let actual = blob_hash(&blob.bytes);
    if actual == blob.hash {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "{context} hash mismatch: declared {}, actual {}",
        blob.hash, actual
    )))
}

fn validate_blob_envelope_hashes_unique(
    context: &str,
    blobs: &[WireBlobEnvelope],
) -> Result<(), NetError> {
    let mut seen = BTreeSet::new();
    for blob in blobs {
        if !seen.insert(blob.hash.as_str()) {
            return Err(NetError::Protocol(format!("{context} duplicate value")));
        }
    }
    Ok(())
}

fn validate_blob_descriptor_hashes_unique(
    context: &str,
    descriptors: &[WireBlobDescriptor],
) -> Result<(), NetError> {
    let mut seen = BTreeSet::new();
    for descriptor in descriptors {
        if !seen.insert(descriptor.hash.as_str()) {
            return Err(NetError::Protocol(format!("{context} duplicate value")));
        }
    }
    Ok(())
}

fn validate_canonical_wire_blob_hash(context: &str, hash: &str) -> Result<(), NetError> {
    if is_canonical_wire_blob_hash(hash) {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "{context} non-canonical blob hash"
    )))
}

pub fn validate_wire_blob_descriptor_hashes(
    descriptor: &WireBlobDescriptor,
) -> Result<(), NetError> {
    validate_canonical_wire_blob_hash("blob descriptor", &descriptor.hash)?;
    for chunk_hash in &descriptor.chunk_hashes {
        validate_canonical_wire_blob_hash("blob descriptor chunk", chunk_hash)?;
    }
    Ok(())
}

pub fn validate_wire_blob_availability_hashes(
    availability: &WireBlobAvailability,
) -> Result<(), NetError> {
    validate_canonical_wire_blob_hash("blob availability", &availability.hash)?;
    if let Some(descriptor) = availability.descriptor.as_ref() {
        validate_wire_blob_descriptor_hashes(descriptor)?;
    }
    for chunk_hash in &availability.available_chunk_hashes {
        validate_canonical_wire_blob_hash("blob availability available chunk", chunk_hash)?;
    }
    for chunk_hash in &availability.missing_chunk_hashes {
        validate_canonical_wire_blob_hash("blob availability missing chunk", chunk_hash)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AllowedRequestFields {
    event_ids: bool,
    events: bool,
    authorization_events: bool,
    authorization_snapshots: bool,
    blob_hashes: bool,
    blobs: bool,
    blob_descriptors: bool,
    workspace_id: bool,
    event_envelopes: bool,
    authorization_event_envelopes: bool,
    authorization_snapshot_envelopes: bool,
    inventory_start_index: bool,
    inventory_limit: bool,
}

impl AllowedRequestFields {
    const fn empty() -> Self {
        Self {
            event_ids: false,
            events: false,
            authorization_events: false,
            authorization_snapshots: false,
            blob_hashes: false,
            blobs: false,
            blob_descriptors: false,
            workspace_id: false,
            event_envelopes: false,
            authorization_event_envelopes: false,
            authorization_snapshot_envelopes: false,
            inventory_start_index: false,
            inventory_limit: false,
        }
    }
}

fn validate_request_shape(
    request: &WireSyncRequest,
    context: &str,
    allowed: AllowedRequestFields,
) -> Result<(), NetError> {
    let mut fields = Vec::new();
    if !allowed.event_ids && !request.event_ids.is_empty() {
        fields.push("event_ids");
    }
    if !allowed.events && !request.events.is_empty() {
        fields.push("events");
    }
    if !allowed.authorization_events && !request.authorization_events.is_empty() {
        fields.push("authorization_events");
    }
    if !allowed.authorization_snapshots && !request.authorization_snapshots.is_empty() {
        fields.push("authorization_snapshots");
    }
    if !allowed.blob_hashes && !request.blob_hashes.is_empty() {
        fields.push("blob_hashes");
    }
    if !allowed.blobs && !request.blobs.is_empty() {
        fields.push("blobs");
    }
    if !allowed.blob_descriptors && !request.blob_descriptors.is_empty() {
        fields.push("blob_descriptors");
    }
    if !allowed.workspace_id && request.workspace_id.is_some() {
        fields.push("workspace_id");
    }
    if !allowed.event_envelopes && !request.event_envelopes.is_empty() {
        fields.push("event_envelopes");
    }
    if !allowed.authorization_event_envelopes && !request.authorization_event_envelopes.is_empty() {
        fields.push("authorization_event_envelopes");
    }
    if !allowed.authorization_snapshot_envelopes
        && !request.authorization_snapshot_envelopes.is_empty()
    {
        fields.push("authorization_snapshot_envelopes");
    }
    if !allowed.inventory_start_index && request.inventory_start_index.is_some() {
        fields.push("inventory_start_index");
    }
    if !allowed.inventory_limit && request.inventory_limit.is_some() {
        fields.push("inventory_limit");
    }

    if fields.is_empty() {
        return Ok(());
    }

    Err(NetError::Protocol(format!(
        "peer sent unexpected {context} request fields: {}",
        fields.join(", ")
    )))
}

fn lock_event_store(
    store: &Arc<Mutex<EventStore>>,
) -> Result<MutexGuard<'_, EventStore>, NetError> {
    store
        .lock()
        .map_err(|_| NetError::Protocol("event store lock poisoned".to_owned()))
}

fn decode_events(
    event_envelopes: Vec<WireEventEnvelope>,
    event_bytes: Vec<Vec<u8>>,
) -> Result<Vec<SignedEvent>, NetError> {
    let mut events = Vec::with_capacity(event_envelopes.len() + event_bytes.len());
    for envelope in event_envelopes {
        let event = decode_event_envelope(envelope)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        validate_decoded_event_size(&event)?;
        events.push(event);
    }
    for bytes in event_bytes {
        let event = decode_event(&bytes).map_err(|error| NetError::Protocol(error.to_string()))?;
        validate_decoded_event_size(&event)?;
        events.push(event);
    }
    Ok(events)
}

pub fn validate_decoded_event_size(event: &SignedEvent) -> Result<(), NetError> {
    validate_signed_event_json_size(event).map_err(|error| NetError::Protocol(error.to_string()))
}

fn decode_and_verify_events(
    event_envelopes: Vec<WireEventEnvelope>,
    event_bytes: Vec<Vec<u8>>,
) -> Result<Vec<SignedEvent>, NetError> {
    let events = decode_events(event_envelopes, event_bytes)?;
    for event in &events {
        verify_self_contained_event(event)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
    }
    Ok(events)
}

fn validate_publish_event_sets_unique(
    proof_events: &[SignedEvent],
    publish_events: &[SignedEvent],
) -> Result<(), NetError> {
    let mut proof_event_ids = BTreeSet::new();
    for event in proof_events {
        if !proof_event_ids.insert(&event.event_id) {
            return Err(NetError::Protocol(format!(
                "publish-events proof event duplicate value {}",
                event.event_id
            )));
        }
    }

    let mut publish_event_ids = BTreeSet::new();
    for event in publish_events {
        if !publish_event_ids.insert(&event.event_id) {
            return Err(NetError::Protocol(format!(
                "publish-events event duplicate value {}",
                event.event_id
            )));
        }
        if proof_event_ids.contains(&event.event_id) {
            return Err(NetError::Protocol(format!(
                "publish-events event also appears as proof event {}",
                event.event_id
            )));
        }
    }

    Ok(())
}

fn validate_trust_snapshots_unique(snapshots: &[SignedTrustSnapshot]) -> Result<(), NetError> {
    let mut seen_snapshots = BTreeSet::new();
    for snapshot in snapshots {
        let key = (
            snapshot.snapshot.signing_bytes(),
            snapshot.root_event.event_id.0.clone(),
            snapshot.author_public_key.clone(),
            snapshot.signature.clone(),
        );
        if !seen_snapshots.insert(key) {
            return Err(NetError::Protocol(format!(
                "publish-events trust snapshot duplicate value {}",
                snapshot.snapshot.root_event_id
            )));
        }
    }
    Ok(())
}

fn decode_and_verify_trust_snapshots(
    snapshot_envelopes: Vec<WireSignedTrustSnapshot>,
    snapshot_bytes: Vec<Vec<u8>>,
) -> Result<Vec<SignedTrustSnapshot>, NetError> {
    let mut snapshots = Vec::with_capacity(snapshot_envelopes.len() + snapshot_bytes.len());
    for envelope in snapshot_envelopes {
        let snapshot = decode_trust_snapshot_envelope(envelope)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        verify_self_contained_trust_snapshot(&snapshot)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        snapshots.push(snapshot);
    }
    for bytes in snapshot_bytes {
        let snapshot =
            decode_trust_snapshot(&bytes).map_err(|error| NetError::Protocol(error.to_string()))?;
        verify_self_contained_trust_snapshot(&snapshot)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

fn publish_request_workspace_id(
    wire_workspace_id: Option<&str>,
    proof_events: &[SignedEvent],
    trust_snapshots: &[SignedTrustSnapshot],
    publish_events: &[SignedEvent],
) -> Result<Option<WorkspaceId>, NetError> {
    validate_request_workspace_id_option("publish-events", wire_workspace_id)?;
    let mut workspace_id =
        wire_workspace_id.map(|workspace_id| WorkspaceId(workspace_id.to_owned()));
    for event in publish_events.iter().chain(proof_events) {
        observe_publish_workspace(&mut workspace_id, &event.event.workspace_id)?;
    }
    for snapshot in trust_snapshots {
        observe_publish_workspace(&mut workspace_id, &snapshot.snapshot.workspace_id)?;
        observe_publish_workspace(&mut workspace_id, &snapshot.root_event.event.workspace_id)?;
    }
    Ok(workspace_id)
}

fn observe_publish_workspace(
    workspace_id: &mut Option<WorkspaceId>,
    candidate: &WorkspaceId,
) -> Result<(), NetError> {
    let Some(existing) = workspace_id else {
        *workspace_id = Some(candidate.clone());
        return Ok(());
    };
    if existing == candidate {
        return Ok(());
    }
    Err(NetError::Protocol(
        "publish request spans multiple workspaces".to_owned(),
    ))
}

fn authorize_proof_events(
    history: &mut Vec<SignedEvent>,
    proof_events: Vec<SignedEvent>,
) -> Result<(), NetError> {
    authorize_pending_events(history, &[], proof_events, |history, event| {
        validate_replica_event_privacy_policy(&event)?;
        history.push(event);
        Ok(())
    })
}

fn authorize_and_store_publish_events(
    store: &EventStore,
    history: &mut Vec<SignedEvent>,
    trust_snapshots: &[SignedTrustSnapshot],
    events: Vec<SignedEvent>,
) -> Result<(), NetError> {
    authorize_pending_events(history, trust_snapshots, events, |history, event| {
        validate_replica_event_privacy_policy(&event)?;
        store
            .append_event(&event)
            .map_err(|error| NetError::Protocol(error.to_string()))?;
        history.push(event);
        Ok(())
    })
}

fn authorize_pending_events<F>(
    history: &mut Vec<SignedEvent>,
    trust_snapshots: &[SignedTrustSnapshot],
    events: Vec<SignedEvent>,
    mut on_authorized: F,
) -> Result<(), NetError>
where
    F: FnMut(&mut Vec<SignedEvent>, SignedEvent) -> Result<(), NetError>,
{
    let mut pending = events;

    loop {
        let mut progressed = false;
        let mut index = 0;

        while index < pending.len() {
            if history_contains(history, &pending[index].event_id) {
                pending.remove(index);
                progressed = true;
                continue;
            }

            match authorize_with_history_or_snapshot(history, trust_snapshots, &pending[index]) {
                Ok(()) => {
                    let event = pending.remove(index);
                    on_authorized(history, event)?;
                    progressed = true;
                }
                Err(_) => {
                    index += 1;
                }
            }
        }

        if pending.is_empty() {
            return Ok(());
        }
        if !progressed {
            let event = &pending[0];
            let error = authorize_with_history_or_snapshot(history, trust_snapshots, event)
                .expect_err("pending event should still fail authorization");
            return Err(NetError::Protocol(format!(
                "event {} is not authorized by workspace history: {}",
                event.event_id, error
            )));
        }
    }
}

fn validate_chunk_uploads_declared_by_descriptors(
    blobs: &[WireBlobEnvelope],
    descriptors: &[BlobDescriptor],
) -> Result<(), NetError> {
    let declared_chunk_hashes = descriptors
        .iter()
        .flat_map(|descriptor| descriptor.chunk_hashes.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();

    for blob in blobs {
        if !declared_chunk_hashes.contains(blob.hash.as_str()) {
            return Err(NetError::Protocol(format!(
                "put-blobs chunk {} not declared by descriptor",
                blob.hash
            )));
        }
    }

    Ok(())
}

fn authorize_with_history_or_snapshot(
    history: &[SignedEvent],
    trust_snapshots: &[SignedTrustSnapshot],
    event: &SignedEvent,
) -> Result<(), chaft_core::AuthorizationError> {
    match authorize_event_with_history(history, event) {
        Ok(()) => Ok(()),
        Err(history_error) => {
            for snapshot in trust_snapshots {
                if authorize_event_with_trust_snapshot(&snapshot.snapshot, event).is_ok() {
                    return Ok(());
                }
            }
            Err(history_error)
        }
    }
}

fn descriptor_to_wire(descriptor: &BlobDescriptor) -> WireBlobDescriptor {
    WireBlobDescriptor {
        hash: descriptor.hash.clone(),
        byte_len: descriptor.byte_len,
        chunk_size: descriptor.chunk_size as u64,
        chunk_hashes: descriptor.chunk_hashes.clone(),
    }
}

pub fn build_publish_events_requests(
    events: Vec<SignedEvent>,
    authorization_events: Vec<SignedEvent>,
    authorization_snapshots: Vec<SignedTrustSnapshot>,
) -> Result<Vec<WireSyncRequest>, NetError> {
    if events.is_empty() {
        return Ok(Vec::new());
    }
    validate_publish_event_sets_unique(&authorization_events, &events)?;
    validate_trust_snapshots_unique(&authorization_snapshots)?;

    let event_envelopes = events.iter().map(encode_event_envelope).collect::<Vec<_>>();
    let authorization_event_envelopes = authorization_events
        .iter()
        .map(encode_event_envelope)
        .collect::<Vec<_>>();
    let authorization_snapshot_envelopes = authorization_snapshots
        .iter()
        .map(encode_trust_snapshot_envelope)
        .collect::<Vec<_>>();

    build_publish_event_envelope_requests(
        event_envelopes,
        authorization_event_envelopes,
        authorization_snapshot_envelopes,
    )
}

fn build_publish_event_envelope_requests(
    event_envelopes: Vec<WireEventEnvelope>,
    authorization_event_envelopes: Vec<WireEventEnvelope>,
    authorization_snapshot_envelopes: Vec<WireSignedTrustSnapshot>,
) -> Result<Vec<WireSyncRequest>, NetError> {
    validate_request_item_count(
        "publish-events authorization event",
        authorization_event_envelopes.len(),
        MAX_AUTHORIZATION_EVENTS_PER_REQUEST,
    )?;
    validate_request_item_count(
        "publish-events authorization snapshot",
        authorization_snapshot_envelopes.len(),
        MAX_AUTHORIZATION_SNAPSHOTS_PER_REQUEST,
    )?;

    let proof_bytes = encode_sync_request(&publish_events_request_from_envelopes(
        Vec::new(),
        authorization_event_envelopes.clone(),
        authorization_snapshot_envelopes.clone(),
    ))
    .len();
    if proof_bytes > MAX_FRAME_LEN {
        return Err(NetError::Protocol(format!(
            "publish proof frame length {proof_bytes} exceeds max {MAX_FRAME_LEN}"
        )));
    }

    let mut requests = Vec::new();
    let mut batch = Vec::new();
    let mut batch_bytes = proof_bytes;

    for envelope in event_envelopes {
        let event_bytes = message_field_encoded_len(envelope.encoded_len());
        let single_frame_bytes = proof_bytes.saturating_add(event_bytes);
        if single_frame_bytes > MAX_FRAME_LEN {
            return Err(NetError::Protocol(format!(
                "publish event frame length {single_frame_bytes} exceeds max {MAX_FRAME_LEN}"
            )));
        }

        if !batch.is_empty()
            && (batch.len() >= MAX_PUBLISH_EVENTS_PER_REQUEST
                || batch_bytes.saturating_add(event_bytes) > MAX_EVENT_UPLOAD_BATCH_BYTES)
        {
            requests.push(publish_events_request_from_envelopes(
                std::mem::take(&mut batch),
                authorization_event_envelopes.clone(),
                authorization_snapshot_envelopes.clone(),
            ));
            batch_bytes = proof_bytes;
        }

        batch_bytes = batch_bytes.saturating_add(event_bytes);
        batch.push(envelope);
    }

    if !batch.is_empty() {
        requests.push(publish_events_request_from_envelopes(
            batch,
            authorization_event_envelopes,
            authorization_snapshot_envelopes,
        ));
    }

    Ok(requests)
}

fn publish_events_request_from_envelopes(
    event_envelopes: Vec<WireEventEnvelope>,
    authorization_event_envelopes: Vec<WireEventEnvelope>,
    authorization_snapshot_envelopes: Vec<WireSignedTrustSnapshot>,
) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::PublishEvents as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes,
        authorization_event_envelopes,
        authorization_snapshot_envelopes,
        inventory_start_index: None,
        inventory_limit: None,
    }
}

fn message_field_encoded_len(message_len: usize) -> usize {
    1 + varint_encoded_len(message_len as u64) + message_len
}

fn varint_encoded_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

fn put_blob_chunks_request(
    descriptor: &BlobDescriptor,
    blobs: Vec<WireBlobEnvelope>,
) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs,
        blob_descriptors: vec![descriptor_to_wire(descriptor)],
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
}

fn wire_to_descriptor(descriptor: WireBlobDescriptor) -> Result<BlobDescriptor, NetError> {
    validate_wire_blob_descriptor_hashes(&descriptor)?;
    let chunk_size = usize::try_from(descriptor.chunk_size)
        .map_err(|_| NetError::Protocol("blob descriptor chunk size overflows usize".to_owned()))?;
    let descriptor = BlobDescriptor {
        hash: descriptor.hash,
        byte_len: descriptor.byte_len,
        chunk_size,
        chunk_hashes: descriptor.chunk_hashes,
    };
    validate_blob_descriptor(&descriptor).map_err(|error| NetError::Protocol(error.to_string()))?;
    Ok(descriptor)
}

fn availability_to_wire(availability: &BlobAvailability) -> WireBlobAvailability {
    WireBlobAvailability {
        hash: availability.hash.clone(),
        has_whole_blob: availability.has_whole_blob,
        descriptor: availability.descriptor.as_ref().map(descriptor_to_wire),
        available_chunk_hashes: availability.available_chunk_hashes.clone(),
        missing_chunk_hashes: availability.missing_chunk_hashes.clone(),
    }
}

fn wire_to_availability(availability: WireBlobAvailability) -> Result<BlobAvailability, NetError> {
    validate_wire_blob_availability_hashes(&availability)?;
    Ok(BlobAvailability {
        hash: availability.hash,
        has_whole_blob: availability.has_whole_blob,
        descriptor: availability
            .descriptor
            .map(wire_to_descriptor)
            .transpose()?,
        available_chunk_hashes: availability.available_chunk_hashes,
        missing_chunk_hashes: availability.missing_chunk_hashes,
    })
}

fn history_contains(history: &[SignedEvent], event_id: &EventId) -> bool {
    history.iter().any(|event| &event.event_id == event_id)
}

fn validate_replica_event_privacy_policy(event: &SignedEvent) -> Result<(), NetError> {
    match &event.event.body {
        EventBody::MessageCreated { .. }
        | EventBody::MessageReplyCreated { .. }
        | EventBody::MessageEdited { .. } => Err(NetError::Protocol(
            "replica sync requires encrypted message payloads".to_owned(),
        )),
        EventBody::MessageCreatedEncrypted {
            sealed_markdown,
            attachments,
            ..
        }
        | EventBody::MessageReplyCreatedEncrypted {
            sealed_markdown,
            attachments,
            ..
        } => {
            validate_sealed_message_payload(sealed_markdown)?;
            validate_encrypted_attachment_refs(attachments)
        }
        EventBody::MessageEditedEncrypted {
            sealed_markdown, ..
        } => validate_sealed_message_payload(sealed_markdown),
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::ChannelCreated { .. }
        | EventBody::ChannelMemberAdded { .. }
        | EventBody::ChannelMemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
        | EventBody::ContentKeyEpochPublished { .. }
        | EventBody::MessageDeleted { .. }
        | EventBody::ReactionAdded { .. }
        | EventBody::ReactionRemoved { .. }
        | EventBody::ReadMarkerUpdated { .. } => Ok(()),
    }
}

fn validate_sealed_message_payload(sealed: &SealedPayload) -> Result<(), NetError> {
    if sealed.mode != PayloadEncryption::Aes256GcmSiv {
        return Err(NetError::Protocol(
            "replica sync requires AES-256-GCM-SIV encrypted message payloads".to_owned(),
        ));
    }
    Ok(())
}

fn validate_encrypted_attachment_refs(attachments: &[AttachmentRef]) -> Result<(), NetError> {
    for attachment in attachments {
        let Some(encryption) = &attachment.encryption else {
            return Err(NetError::Protocol(
                "replica sync requires encrypted attachment metadata".to_owned(),
            ));
        };
        if encryption.mode != PayloadEncryption::Aes256GcmSiv {
            return Err(NetError::Protocol(
                "replica sync requires AES-256-GCM-SIV encrypted attachment metadata".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn read_frame<S>(stream: &mut S) -> Result<Vec<u8>, NetError>
where
    S: AsyncRead + Unpin,
{
    read_frame_with_timeout(stream, FRAME_IO_TIMEOUT).await
}

async fn read_frame_with_timeout<S>(
    stream: &mut S,
    timeout_duration: Duration,
) -> Result<Vec<u8>, NetError>
where
    S: AsyncRead + Unpin,
{
    let len = timeout(timeout_duration, stream.read_u32())
        .await
        .map_err(|_| frame_timeout_error("length read", timeout_duration))?? as usize;
    if len > MAX_FRAME_LEN {
        return Err(NetError::Protocol(format!(
            "frame length {len} exceeds max {MAX_FRAME_LEN}"
        )));
    }
    let mut bytes = vec![0; len];
    timeout(timeout_duration, stream.read_exact(&mut bytes))
        .await
        .map_err(|_| frame_timeout_error("body read", timeout_duration))??;
    Ok(bytes)
}

async fn write_frame<S>(stream: &mut S, bytes: &[u8]) -> Result<(), NetError>
where
    S: AsyncWrite + Unpin,
{
    write_frame_with_timeout(stream, bytes, FRAME_IO_TIMEOUT).await
}

async fn write_frame_with_timeout<S>(
    stream: &mut S,
    bytes: &[u8],
    timeout_duration: Duration,
) -> Result<(), NetError>
where
    S: AsyncWrite + Unpin,
{
    if bytes.len() > MAX_FRAME_LEN {
        return Err(NetError::Protocol(format!(
            "frame length {} exceeds max {}",
            bytes.len(),
            MAX_FRAME_LEN
        )));
    }
    timeout(timeout_duration, stream.write_u32(bytes.len() as u32))
        .await
        .map_err(|_| frame_timeout_error("length write", timeout_duration))??;
    timeout(timeout_duration, stream.write_all(bytes))
        .await
        .map_err(|_| frame_timeout_error("body write", timeout_duration))??;
    Ok(())
}

fn frame_timeout_error(operation: &str, timeout_duration: Duration) -> NetError {
    NetError::Protocol(format!(
        "sync frame {operation} timed out after {} ms",
        timeout_duration.as_millis()
    ))
}

fn response_error(error: Option<String>) -> Result<(), NetError> {
    match error {
        Some(error) if error.len() > MAX_SYNC_RESPONSE_ERROR_BYTES => {
            Err(NetError::Protocol(format!(
                "peer error message length {} exceeds max {}",
                error.len(),
                MAX_SYNC_RESPONSE_ERROR_BYTES
            )))
        }
        Some(error) => Err(NetError::Protocol(error)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chaft_identity::DeviceIdentity;
    use chaft_media::BLOB_CHUNK_FILE_MAX_BYTES;
    use chaft_net::PeerId;
    use chaft_store::EVENT_JSON_MAX_BYTES;
    use chaft_types::{
        ChannelId, DeviceId, DeviceKeyPackageId, EventBody, SignableEvent, SignedEvent,
        TrustSnapshot, WORKSPACE_ID_MAX_BYTES, WorkspaceId,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt, duplex},
        net::{TcpListener, TcpStream},
        sync::oneshot,
        time::{sleep, timeout},
    };

    use super::*;

    fn empty_sync_request(kind: WireSyncRequestKind) -> WireSyncRequest {
        WireSyncRequest {
            kind: kind as i32,
            event_ids: Vec::new(),
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            workspace_id: None,
            event_envelopes: Vec::new(),
            authorization_event_envelopes: Vec::new(),
            authorization_snapshot_envelopes: Vec::new(),
            inventory_start_index: None,
            inventory_limit: None,
        }
    }

    fn poisoned_event_store() -> Arc<Mutex<EventStore>> {
        let store = Arc::new(Mutex::new(EventStore::open_in_memory().unwrap()));
        let poisoned_store = Arc::clone(&store);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned_store.lock().unwrap();
            panic!("poison event store lock");
        })
        .join();
        store
    }

    #[test]
    fn response_error_accepts_bounded_peer_error_message() {
        let message = "x".repeat(MAX_SYNC_RESPONSE_ERROR_BYTES);

        let error = response_error(Some(message.clone())).unwrap_err();

        assert_eq!(error.to_string(), format!("protocol error: {message}"));
    }

    #[test]
    fn response_error_rejects_oversized_peer_error_without_echoing_body() {
        let message = "x".repeat(MAX_SYNC_RESPONSE_ERROR_BYTES + 1);

        let error = response_error(Some(message)).unwrap_err().to_string();

        assert!(error.contains("peer error message length"));
        assert!(error.contains(&MAX_SYNC_RESPONSE_ERROR_BYTES.to_string()));
        assert!(!error.contains(&"x".repeat(128)));
    }

    #[test]
    fn sync_error_response_bounds_outbound_error_message() {
        let message = "x".repeat(MAX_SYNC_RESPONSE_ERROR_BYTES + 128);

        let response = sync_error_response(NetError::Protocol(message));
        let error = response.error.unwrap();

        assert_eq!(error.len(), MAX_SYNC_RESPONSE_ERROR_BYTES);
        assert!(error.ends_with(SYNC_RESPONSE_ERROR_TRUNCATED_SUFFIX));
        assert!(response.event_ids.is_empty());
        assert!(response.event_envelopes.is_empty());
        assert!(response.blobs.is_empty());
        assert!(response.inventory_total_count.is_none());
    }

    #[test]
    fn oversized_sync_response_is_encoded_as_bounded_error_frame() {
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: vec![WireBlobEnvelope {
                hash: "0".repeat(64),
                bytes: vec![7; MAX_FRAME_LEN],
            }],
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let encoded = encode_sync_response_with_frame_limit(response);
        let decoded = decode_sync_response(&encoded).unwrap();
        let error = decoded.error.unwrap();

        assert!(encoded.len() <= MAX_FRAME_LEN);
        assert!(decoded.blobs.is_empty());
        assert!(error.contains("sync response frame length"));
        assert!(error.contains("exceeds max"));
        assert!(error.len() <= MAX_SYNC_RESPONSE_ERROR_BYTES);
    }

    #[test]
    fn bounded_sync_response_error_preserves_utf8_boundaries() {
        let message =
            "protocol error: ".to_owned() + &"é".repeat(MAX_SYNC_RESPONSE_ERROR_BYTES) + "tail";

        let error = bounded_sync_response_error(&message);

        assert!(error.is_char_boundary(error.len()));
        assert!(error.len() <= MAX_SYNC_RESPONSE_ERROR_BYTES);
        assert!(error.ends_with(SYNC_RESPONSE_ERROR_TRUNCATED_SUFFIX));
    }

    #[test]
    fn bounded_sync_response_error_leaves_bounded_message_unchanged() {
        let message = "bounded peer request failure";

        assert_eq!(bounded_sync_response_error(message), message);
    }

    #[tokio::test]
    async fn sync_peer_store_serves_protocol_over_generic_async_stream() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let root = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Generic Stream".to_owned(),
            },
        ));
        let store = EventStore::open_in_memory().unwrap();
        store.append_event(&root).unwrap();
        let sync_store = SyncPeerStore::new(store);
        let (mut client, mut server) = duplex(4096);
        let server_task = tokio::spawn(async move { sync_store.serve_stream(&mut server).await });

        let response = request_sync_stream(
            &mut client,
            WireSyncRequest {
                kind: WireSyncRequestKind::Inventory as i32,
                event_ids: Vec::new(),
                events: Vec::new(),
                authorization_events: Vec::new(),
                authorization_snapshots: Vec::new(),
                blob_hashes: Vec::new(),
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                workspace_id: Some(workspace_id.0),
                event_envelopes: Vec::new(),
                authorization_event_envelopes: Vec::new(),
                authorization_snapshot_envelopes: Vec::new(),
                inventory_start_index: None,
                inventory_limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(response.event_ids, vec![root.event_id.0]);
        assert!(response.error.is_none());

        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn inventory_request_returns_paged_ids_with_total_count() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let first = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Paged".to_owned(),
            },
        ));
        let second = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Second".to_owned(),
            },
        ));
        let third = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Third".to_owned(),
            },
        ));
        let store = EventStore::open_in_memory().unwrap();
        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();
        store.append_event(&third).unwrap();

        let mut request = empty_sync_request(WireSyncRequestKind::Inventory);
        request.workspace_id = Some(workspace_id.0);
        request.inventory_start_index = Some(1);
        request.inventory_limit = Some(1);

        let response = handle_request(request, Arc::new(Mutex::new(store)), None).unwrap();

        assert_eq!(response.event_ids, vec![second.event_id.0]);
        assert_eq!(response.inventory_total_count, Some(3));
    }

    #[test]
    fn inventory_request_without_explicit_page_is_bounded_and_reports_total_count() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let store = EventStore::open_in_memory().unwrap();
        let mut event_ids = Vec::new();
        for index in 0..=MAX_INVENTORY_EVENT_IDS_PER_RESPONSE {
            let event = identity.sign_event(SignableEvent::new(
                workspace_id.clone(),
                None,
                identity.device_id().clone(),
                EventBody::DeviceProfileUpdated {
                    display_name: format!("Inventory {index:04}"),
                },
            ));
            event_ids.push(event.event_id.0.clone());
            store.append_event(&event).unwrap();
        }

        let mut request = empty_sync_request(WireSyncRequestKind::Inventory);
        request.workspace_id = Some(workspace_id.0);

        let response = handle_request(request, Arc::new(Mutex::new(store)), None).unwrap();

        assert_eq!(
            response.event_ids.len(),
            MAX_INVENTORY_EVENT_IDS_PER_RESPONSE
        );
        assert_eq!(
            response.event_ids,
            event_ids[..MAX_INVENTORY_EVENT_IDS_PER_RESPONSE]
        );
        assert_eq!(
            response.inventory_total_count,
            Some((MAX_INVENTORY_EVENT_IDS_PER_RESPONSE + 1) as u64)
        );
    }

    #[test]
    fn inventory_total_count_accepts_pull_budget_boundary() {
        let total_count =
            validate_inventory_total_count(MAX_INVENTORY_EVENT_IDS_PER_PULL as u64).unwrap();

        assert_eq!(total_count, MAX_INVENTORY_EVENT_IDS_PER_PULL);
    }

    #[test]
    fn inventory_total_count_rejects_over_pull_budget() {
        let error = validate_inventory_total_count(MAX_INVENTORY_EVENT_IDS_PER_PULL as u64 + 1)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned inventory total count"));
        assert!(error.contains(&MAX_INVENTORY_EVENT_IDS_PER_PULL.to_string()));
    }

    #[tokio::test]
    async fn sync_peer_store_fetches_events_as_typed_envelopes() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let root = identity.sign_event(SignableEvent::new(
            workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Typed Fetch".to_owned(),
            },
        ));
        let store = EventStore::open_in_memory().unwrap();
        store.append_event(&root).unwrap();
        let sync_store = SyncPeerStore::new(store);
        let (mut client, mut server) = duplex(4096);
        let server_task = tokio::spawn(async move { sync_store.serve_stream(&mut server).await });

        let response = request_sync_stream(
            &mut client,
            WireSyncRequest {
                kind: WireSyncRequestKind::FetchEvents as i32,
                event_ids: vec![root.event_id.0.clone()],
                events: Vec::new(),
                authorization_events: Vec::new(),
                authorization_snapshots: Vec::new(),
                blob_hashes: Vec::new(),
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                workspace_id: None,
                event_envelopes: Vec::new(),
                authorization_event_envelopes: Vec::new(),
                authorization_snapshot_envelopes: Vec::new(),
                inventory_start_index: None,
                inventory_limit: None,
            },
        )
        .await
        .unwrap();

        assert!(response.events.is_empty());
        assert_eq!(response.event_envelopes.len(), 1);
        let decoded =
            decode_event_envelope(response.event_envelopes.into_iter().next().unwrap()).unwrap();
        assert_eq!(decoded, root);

        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn publish_event_requests_split_large_event_sets() {
        let workspace_id = WorkspaceId::new();
        let events = (0..3)
            .map(|index| large_device_key_package_event(&workspace_id, index))
            .collect::<Vec<_>>();

        let requests = build_publish_events_requests(events, Vec::new(), Vec::new()).unwrap();

        assert!(requests.len() > 1);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.event_envelopes.len())
                .sum::<usize>(),
            3
        );
        for request in requests {
            assert!(encode_sync_request(&request).len() <= MAX_FRAME_LEN);
        }
    }

    #[test]
    fn publish_event_requests_split_large_event_counts() {
        let workspace_id = WorkspaceId::new();
        let events = (0..=MAX_PUBLISH_EVENTS_PER_REQUEST)
            .map(|index| small_device_key_package_event(&workspace_id, index))
            .collect::<Vec<_>>();

        let requests = build_publish_events_requests(events, Vec::new(), Vec::new()).unwrap();

        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests
                .iter()
                .map(|request| request.event_envelopes.len())
                .collect::<Vec<_>>(),
            vec![MAX_PUBLISH_EVENTS_PER_REQUEST, 1]
        );
    }

    #[test]
    fn publish_event_batches_repeat_authorization_context() {
        let workspace_id = WorkspaceId::new();
        let events = (0..3)
            .map(|index| large_device_key_package_event(&workspace_id, index))
            .collect::<Vec<_>>();
        let proof_event = small_device_key_package_event(&workspace_id, 99);
        let proof_event_id = proof_event.event_id.0.clone();

        let requests =
            build_publish_events_requests(events, vec![proof_event], Vec::new()).unwrap();

        assert!(requests.len() > 1);
        for request in requests {
            assert_eq!(request.authorization_event_envelopes.len(), 1);
            assert_eq!(
                request.authorization_event_envelopes[0].event_id,
                proof_event_id
            );
            assert!(encode_sync_request(&request).len() <= MAX_FRAME_LEN);
        }
    }

    #[test]
    fn publish_event_requests_reject_too_many_authorization_events() {
        let workspace_id = WorkspaceId::new();
        let event = small_device_key_package_event(&workspace_id, 0);
        let authorization_events = (0..=MAX_AUTHORIZATION_EVENTS_PER_REQUEST)
            .map(|index| small_device_key_package_event(&workspace_id, index + 1))
            .collect::<Vec<_>>();

        let error = build_publish_events_requests(vec![event], authorization_events, Vec::new())
            .unwrap_err()
            .to_string();

        assert!(error.contains("publish-events authorization event count 129 exceeds max 128"));
    }

    #[test]
    fn publish_event_requests_reject_duplicate_target_events_before_encoding() {
        let workspace_id = WorkspaceId::new();
        let event = small_device_key_package_event(&workspace_id, 0);

        let error =
            build_publish_events_requests(vec![event.clone(), event], Vec::new(), Vec::new())
                .unwrap_err()
                .to_string();

        assert!(error.contains("publish-events event duplicate value"));
    }

    #[test]
    fn publish_event_requests_reject_target_event_repeated_as_proof_before_encoding() {
        let workspace_id = WorkspaceId::new();
        let event = small_device_key_package_event(&workspace_id, 0);

        let error = build_publish_events_requests(vec![event.clone()], vec![event], Vec::new())
            .unwrap_err()
            .to_string();

        assert!(error.contains("publish-events event also appears as proof event"));
    }

    #[test]
    fn publish_event_requests_reject_duplicate_trust_snapshots_before_encoding() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let event = small_device_key_package_event(&workspace_id, 0);
        let snapshot = small_signed_trust_snapshot(&identity, &workspace_id);

        let error = build_publish_events_requests(
            vec![event],
            Vec::new(),
            vec![snapshot.clone(), snapshot],
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("publish-events trust snapshot duplicate value"));
    }

    #[test]
    fn fetch_batch_inputs_deduplicate_while_preserving_order() {
        let first = EventId(format!("evt_{}", "1".repeat(64)));
        let second = EventId(format!("evt_{}", "2".repeat(64)));
        let first_blob = b"first whole blob".to_vec();
        let second_blob = b"second whole blob".to_vec();
        let first_hash = blob_hash(&first_blob);
        let second_hash = blob_hash(&second_blob);

        assert_eq!(
            deduplicate_event_ids(vec![first.clone(), second.clone(), first.clone()]),
            vec![first, second]
        );
        assert_eq!(
            deduplicate_strings(vec![
                "first".to_owned(),
                "second".to_owned(),
                "first".to_owned()
            ]),
            vec!["first".to_owned(), "second".to_owned()]
        );

        let (envelopes, hashes) = whole_blob_upload_envelopes(vec![
            first_blob.clone(),
            second_blob.clone(),
            first_blob.clone(),
        ]);
        assert_eq!(hashes, vec![first_hash.clone(), second_hash.clone()]);
        assert_eq!(envelopes.len(), 2);
        assert_eq!(envelopes[0].hash, first_hash);
        assert_eq!(envelopes[0].bytes, first_blob);
        assert_eq!(envelopes[1].hash, second_hash);
        assert_eq!(envelopes[1].bytes, second_blob);
    }

    #[test]
    fn direct_peer_endpoint_validation_rejects_zero_port_before_dial() {
        let peer = PeerAddress {
            peer_id: PeerId("zero-port-direct-peer".to_owned()),
            endpoint: "127.0.0.1:0".to_owned(),
        };

        let error = validate_direct_peer_endpoint(&peer)
            .unwrap_err()
            .to_string();

        assert!(error.contains("direct TCP endpoint must be host:port"));
        assert!(error.contains("127.0.0.1:0"));
    }

    #[test]
    fn fetch_events_wire_response_rejects_more_events_than_requested() {
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: vec![vec![0xff], vec![0xff]],
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_events_wire_response(&response, 1)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-events event count 2 exceeds requested limit 1"));
    }

    #[tokio::test]
    async fn fetch_events_rejects_non_canonical_event_id_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("non-canonical-fetch-event-id".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_events(&peer, vec![EventId("evt_NOT_CANONICAL".to_owned())])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer requested non-canonical event id"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn fetch_workspace_inventory_rejects_blank_workspace_id_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("blank-workspace-inventory".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_workspace_inventory(&peer, &WorkspaceId(" ".to_owned()))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("inventory workspace ID is blank"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn fetch_workspace_inventory_rejects_oversized_workspace_id_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("oversized-workspace-inventory".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_workspace_inventory(&peer, &WorkspaceId("w".repeat(WORKSPACE_ID_MAX_BYTES + 1)))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("workspace ID is too large"));
        assert!(!error.contains("Connection refused"));
    }

    #[test]
    fn fetch_events_response_rejects_duplicate_returned_events() {
        let workspace_id = WorkspaceId::new();
        let event = small_device_key_package_event(&workspace_id, 1);
        let requested = BTreeSet::from([event.event_id.clone()]);

        let error = validate_fetch_events_response(&[event.clone(), event], &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned duplicate event"));
    }

    #[tokio::test]
    async fn put_blob_envelopes_split_large_blob_counts() {
        let envelopes = (0..=MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST)
            .map(|index| {
                let bytes = vec![index as u8];
                WireBlobEnvelope {
                    hash: blob_hash(&bytes),
                    bytes,
                }
            })
            .collect::<Vec<_>>();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("split-blob-upload-count".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let request_sizes = Arc::new(Mutex::new(Vec::new()));
        let server_request_sizes = Arc::clone(&request_sizes);
        let server_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();
                assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                server_request_sizes
                    .lock()
                    .unwrap()
                    .push(request.blobs.len());

                let response = WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs: Vec::new(),
                    blob_descriptors: Vec::new(),
                    blob_availability: Vec::new(),
                    event_envelopes: Vec::new(),
                    inventory_total_count: None,
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        put_blob_envelopes_batched(&peer, envelopes).await.unwrap();

        server_task.await.unwrap();
        assert_eq!(
            *request_sizes.lock().unwrap(),
            vec![MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST, 1]
        );
    }

    #[tokio::test]
    async fn put_blob_envelopes_flushes_before_large_valid_next_frame() {
        let envelopes = [1_u8, 2_u8]
            .into_iter()
            .map(|value| {
                let bytes = vec![value; MAX_WHOLE_BLOB_UPLOAD_BATCH_BYTES];
                WireBlobEnvelope {
                    hash: blob_hash(&bytes),
                    bytes,
                }
            })
            .collect::<Vec<_>>();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("split-large-blob-upload-frame".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let request_blob_counts = Arc::new(Mutex::new(Vec::new()));
        let request_frame_lengths = Arc::new(Mutex::new(Vec::new()));
        let server_request_blob_counts = Arc::clone(&request_blob_counts);
        let server_request_frame_lengths = Arc::clone(&request_frame_lengths);
        let server_task = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                assert!(request_len <= MAX_FRAME_LEN);
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();
                assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                server_request_blob_counts
                    .lock()
                    .unwrap()
                    .push(request.blobs.len());
                server_request_frame_lengths
                    .lock()
                    .unwrap()
                    .push(request_len);

                let response = WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs: Vec::new(),
                    blob_descriptors: Vec::new(),
                    blob_availability: Vec::new(),
                    event_envelopes: Vec::new(),
                    inventory_total_count: None,
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        put_blob_envelopes_batched(&peer, envelopes).await.unwrap();

        server_task.await.unwrap();
        assert_eq!(*request_blob_counts.lock().unwrap(), vec![1, 1]);
        assert_eq!(request_frame_lengths.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn put_blob_chunked_rejects_oversized_chunk_frame_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("oversized-chunk-preflight".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .put_blob_chunked(&peer, vec![7; MAX_FRAME_LEN], MAX_FRAME_LEN)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("chunk upload frame length"));
        assert!(error.contains("exceeds max"));
    }

    #[tokio::test]
    async fn put_blob_chunked_rejects_invalid_descriptor_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("invalid-chunk-descriptor-preflight".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .put_blob_chunked(
                &peer,
                b"small chunk upload".to_vec(),
                BLOB_CHUNK_FILE_MAX_BYTES + 1,
            )
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("invalid blob descriptor"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn put_blob_chunked_uploads_repeated_chunk_hash_once() {
        let bytes = b"abcabc".to_vec();
        let descriptor = describe_blob(&bytes, 3);
        assert_eq!(descriptor.chunk_hashes.len(), 2);
        assert_eq!(descriptor.chunk_hashes[0], descriptor.chunk_hashes[1]);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("dedupe-repeated-chunk-upload".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let server_descriptor = descriptor.clone();
        let server_task = tokio::spawn(async move {
            for request_index in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();

                let response = match request_index {
                    0 => {
                        assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                        assert!(request.blobs.is_empty());
                        assert_eq!(request.blob_descriptors.len(), 1);
                        assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: Vec::new(),
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                    1 => {
                        assert_eq!(
                            request.kind,
                            WireSyncRequestKind::FetchBlobAvailability as i32
                        );
                        assert_eq!(request.blob_hashes, vec![server_descriptor.hash.clone()]);
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: vec![WireBlobAvailability {
                                hash: server_descriptor.hash.clone(),
                                has_whole_blob: false,
                                descriptor: None,
                                available_chunk_hashes: Vec::new(),
                                missing_chunk_hashes: Vec::new(),
                            }],
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                    _ => {
                        assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                        assert_eq!(request.blob_descriptors.len(), 1);
                        assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
                        assert_eq!(request.blobs.len(), 1);
                        assert_eq!(request.blobs[0].hash, server_descriptor.chunk_hashes[0]);
                        assert_eq!(request.blobs[0].bytes, b"abc");
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: Vec::new(),
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        let uploaded = DirectTransport
            .put_blob_chunked(&peer, bytes, 3)
            .await
            .unwrap();

        server_task.await.unwrap();
        assert_eq!(uploaded, descriptor);
    }

    #[tokio::test]
    async fn put_blob_chunked_ignores_available_chunks_from_mismatched_descriptor() {
        let bytes = b"abcdef".to_vec();
        let descriptor = describe_blob(&bytes, 2);
        let mismatched_descriptor = BlobDescriptor {
            hash: descriptor.hash.clone(),
            byte_len: descriptor.byte_len,
            chunk_size: descriptor.chunk_size,
            chunk_hashes: vec![
                descriptor.chunk_hashes[0].clone(),
                blob_hash(b"wrong"),
                descriptor.chunk_hashes[2].clone(),
            ],
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("mismatched-descriptor-chunk-upload".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let server_descriptor = descriptor.clone();
        let server_mismatched_descriptor = mismatched_descriptor.clone();
        let server_task = tokio::spawn(async move {
            for request_index in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();

                let response = match request_index {
                    0 => {
                        assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                        assert!(request.blobs.is_empty());
                        assert_eq!(request.blob_descriptors.len(), 1);
                        assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: Vec::new(),
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                    1 => {
                        assert_eq!(
                            request.kind,
                            WireSyncRequestKind::FetchBlobAvailability as i32
                        );
                        assert_eq!(request.blob_hashes, vec![server_descriptor.hash.clone()]);
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: vec![WireBlobAvailability {
                                hash: server_descriptor.hash.clone(),
                                has_whole_blob: false,
                                descriptor: Some(descriptor_to_wire(&server_mismatched_descriptor)),
                                available_chunk_hashes: vec![
                                    server_mismatched_descriptor.chunk_hashes[0].clone(),
                                ],
                                missing_chunk_hashes: server_mismatched_descriptor.chunk_hashes
                                    [1..]
                                    .to_vec(),
                            }],
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                    _ => {
                        assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                        assert_eq!(request.blob_descriptors.len(), 1);
                        assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
                        assert_eq!(request.blobs.len(), server_descriptor.chunk_hashes.len());
                        assert_eq!(
                            request
                                .blobs
                                .iter()
                                .map(|blob| blob.hash.clone())
                                .collect::<Vec<_>>(),
                            server_descriptor.chunk_hashes
                        );
                        WireSyncResponse {
                            event_ids: Vec::new(),
                            events: Vec::new(),
                            error: None,
                            blobs: Vec::new(),
                            blob_descriptors: Vec::new(),
                            blob_availability: Vec::new(),
                            event_envelopes: Vec::new(),
                            inventory_total_count: None,
                        }
                    }
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        let uploaded = DirectTransport
            .put_blob_chunked(&peer, bytes, 2)
            .await
            .unwrap();

        server_task.await.unwrap();
        assert_eq!(uploaded, descriptor);
    }

    #[tokio::test]
    async fn fetch_events_splits_batch_after_closed_oversized_response() {
        let workspace_id = WorkspaceId::new();
        let first = small_device_key_package_event(&workspace_id, 1);
        let second = small_device_key_package_event(&workspace_id, 2);
        let event_by_id = HashMap::from([
            (first.event_id.0.clone(), first.clone()),
            (second.event_id.0.clone(), second.clone()),
        ]);
        let request_sizes = Arc::new(Mutex::new(Vec::new()));
        let server_request_sizes = Arc::clone(&request_sizes);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("split-fetch".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let server_task = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();
                server_request_sizes
                    .lock()
                    .unwrap()
                    .push(request.event_ids.len());

                if request.event_ids.len() > 1 {
                    continue;
                }

                let response_events = request
                    .event_ids
                    .iter()
                    .filter_map(|event_id| event_by_id.get(event_id))
                    .map(encode_event_envelope)
                    .collect::<Vec<_>>();
                let response = WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs: Vec::new(),
                    blob_descriptors: Vec::new(),
                    blob_availability: Vec::new(),
                    event_envelopes: response_events,
                    inventory_total_count: None,
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        let fetched = DirectTransport
            .fetch_events(&peer, vec![first.event_id.clone(), second.event_id.clone()])
            .await
            .unwrap();

        server_task.await.unwrap();
        assert_eq!(fetched, vec![first, second]);
        assert_eq!(*request_sizes.lock().unwrap(), vec![2, 1, 1]);
    }

    #[tokio::test]
    async fn fetch_blobs_splits_batch_after_closed_oversized_response() {
        let first = b"first blob".to_vec();
        let second = b"second blob".to_vec();
        let first_hash = blob_hash(&first);
        let second_hash = blob_hash(&second);
        let blob_by_hash = HashMap::from([
            (first_hash.clone(), first.clone()),
            (second_hash.clone(), second.clone()),
        ]);
        let request_sizes = Arc::new(Mutex::new(Vec::new()));
        let server_request_sizes = Arc::clone(&request_sizes);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("split-blob-fetch".to_owned()),
            endpoint: listener.local_addr().unwrap().to_string(),
        };
        let server_task = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request_len = stream.read_u32().await.unwrap() as usize;
                let mut request_bytes = vec![0; request_len];
                stream.read_exact(&mut request_bytes).await.unwrap();
                let request = decode_sync_request(&request_bytes).unwrap();
                server_request_sizes
                    .lock()
                    .unwrap()
                    .push(request.blob_hashes.len());

                if request.blob_hashes.len() > 1 {
                    continue;
                }

                let blobs = request
                    .blob_hashes
                    .iter()
                    .filter_map(|hash| {
                        blob_by_hash.get(hash).map(|bytes| WireBlobEnvelope {
                            hash: hash.clone(),
                            bytes: bytes.clone(),
                        })
                    })
                    .collect::<Vec<_>>();
                let response = WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs,
                    blob_descriptors: Vec::new(),
                    blob_availability: Vec::new(),
                    event_envelopes: Vec::new(),
                    inventory_total_count: None,
                };
                let response = encode_sync_response(&response);
                stream.write_u32(response.len() as u32).await.unwrap();
                stream.write_all(&response).await.unwrap();
            }
        });

        let fetched = DirectTransport
            .fetch_blobs(&peer, vec![first_hash.clone(), second_hash.clone()])
            .await
            .unwrap();

        server_task.await.unwrap();
        assert_eq!(fetched.get(&first_hash), Some(&first));
        assert_eq!(fetched.get(&second_hash), Some(&second));
        assert_eq!(*request_sizes.lock().unwrap(), vec![2, 1, 1]);
    }

    #[tokio::test]
    async fn fetch_blobs_rejects_non_canonical_hash_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("non-canonical-blob-fetch".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_blobs(&peer, vec!["A".repeat(64)])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn fetch_blob_availability_rejects_non_canonical_hash_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("non-canonical-blob-availability".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_blob_availabilities(&peer, vec!["A".repeat(64)])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn fetch_blob_chunked_rejects_non_canonical_hash_before_network() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        drop(listener);
        let peer = PeerAddress {
            peer_id: PeerId("non-canonical-chunked-blob-fetch".to_owned()),
            endpoint,
        };

        let error = DirectTransport
            .fetch_blob_chunked(&peer, &"A".repeat(64))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("Connection refused"));
    }

    #[tokio::test]
    async fn read_frame_times_out_when_peer_stalls_before_length() {
        let (_client, mut server) = duplex(8);

        let error = read_frame_with_timeout(&mut server, Duration::from_millis(10))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("length read timed out"));
    }

    #[tokio::test]
    async fn read_frame_times_out_when_peer_stalls_during_body() {
        let (mut client, mut server) = duplex(8);
        client.write_u32(4).await.unwrap();

        let error = read_frame_with_timeout(&mut server, Duration::from_millis(10))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("body read timed out"));
    }

    #[tokio::test]
    async fn read_frame_rejects_oversized_length_before_body_read() {
        let (mut client, mut server) = duplex(8);
        client.write_u32((MAX_FRAME_LEN + 1) as u32).await.unwrap();

        let error = read_frame_with_timeout(&mut server, Duration::from_millis(10))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("frame length"));
        assert!(error.contains("exceeds max"));
    }

    #[tokio::test]
    async fn direct_server_enforces_active_connection_limit() {
        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let root = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Limited".to_owned(),
            },
        ));
        store.append_event(&root).unwrap();

        let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
        let addr = server.local_addr().unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("limited".to_owned()),
            endpoint: addr.to_string(),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            server
                .serve_until_shutdown_with_max_connections(shutdown_rx, 1)
                .await
        });

        let idle_stream = TcpStream::connect(addr).await.unwrap();
        sleep(Duration::from_millis(20)).await;

        let transport = DirectTransport;
        let blocked = timeout(
            Duration::from_millis(100),
            transport.fetch_workspace_inventory(&peer, &workspace_id),
        )
        .await;
        assert!(blocked.is_err());

        drop(idle_stream);
        let scoped_inventory = timeout(
            Duration::from_secs(2),
            transport.fetch_workspace_inventory(&peer, &workspace_id),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(scoped_inventory, vec![root.event_id]);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_server_rejects_zero_active_connection_limit() {
        let store = EventStore::open_in_memory().unwrap();
        let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
        let (_shutdown_tx, shutdown_rx) = oneshot::channel();

        let error = server
            .serve_until_shutdown_with_max_connections(shutdown_rx, 0)
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("connection limit must be greater than zero")
        );
    }

    #[test]
    fn inventory_request_rejects_unexpected_payload_fields_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::Inventory);
        request.event_ids = vec!["evt_unexpected".to_owned()];
        request.workspace_id = Some("wrk_allowed".to_owned());

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("unexpected inventory request fields: event_ids"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn inventory_request_rejects_blank_workspace_id_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::Inventory);
        request.workspace_id = Some("   ".to_owned());

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("inventory workspace ID is blank"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn inventory_request_rejects_oversized_workspace_id_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::Inventory);
        request.workspace_id = Some("w".repeat(WORKSPACE_ID_MAX_BYTES + 1));

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("workspace ID is too large"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn fetch_events_request_rejects_unexpected_blob_fields_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchEvents);
        request.event_ids = vec!["evt_allowed".to_owned()];
        request.blobs = vec![WireBlobEnvelope {
            hash: blob_hash(b"unexpected fetch-events blob"),
            bytes: b"unexpected fetch-events blob".to_vec(),
        }];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("unexpected fetch-events request fields: blobs"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn fetch_events_request_rejects_too_many_ids_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchEvents);
        request.event_ids = (0..=MAX_FETCH_EVENT_IDS_PER_REQUEST)
            .map(|index| format!("evt_{index:064x}"))
            .collect();

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-events event id count 129 exceeds max 128"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn fetch_events_request_rejects_duplicate_ids_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchEvents);
        let event_id = format!("evt_{}", "1".repeat(64));
        request.event_ids = vec![event_id.clone(), event_id];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-events event id duplicate value"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn fetch_events_request_duplicate_error_does_not_echo_unvalidated_id() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchEvents);
        let event_id = "UNTRUSTED_DUPLICATE_EVENT_ID".repeat(256);
        request.event_ids = vec![event_id.clone(), event_id];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-events event id duplicate value"));
        assert!(!error.contains("UNTRUSTED_DUPLICATE_EVENT_ID"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn fetch_events_request_rejects_non_canonical_ids_before_event_store_lock() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchEvents);
        request.event_ids = vec!["evt_NOT_CANONICAL".to_owned()];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer requested non-canonical event id"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn publish_request_rejects_unexpected_blob_fields_before_event_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.events = vec![vec![0xff]];
        request.blobs = vec![WireBlobEnvelope {
            hash: blob_hash(b"unexpected publish blob"),
            bytes: b"unexpected publish blob".to_vec(),
        }];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unexpected publish-events request fields: blobs"));
        assert!(!error.contains("protobuf decode failed"));
    }

    #[test]
    fn publish_request_rejects_too_many_events_before_event_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.events = vec![vec![0xff]; MAX_PUBLISH_EVENTS_PER_REQUEST + 1];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("publish-events event count 129 exceeds max 128"));
        assert!(!error.contains("protobuf decode failed"));
    }

    #[test]
    fn publish_request_rejects_too_many_authorization_events_before_event_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.authorization_events = vec![vec![0xff]; MAX_AUTHORIZATION_EVENTS_PER_REQUEST + 1];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("publish-events authorization event count 129 exceeds max 128"));
        assert!(!error.contains("protobuf decode failed"));
    }

    #[test]
    fn publish_request_rejects_too_many_authorization_snapshots_before_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.authorization_snapshots =
            vec![vec![0xff]; MAX_AUTHORIZATION_SNAPSHOTS_PER_REQUEST + 1];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("publish-events authorization snapshot count 33 exceeds max 32"));
        assert!(!error.contains("trust snapshot"));
    }

    #[test]
    fn publish_request_rejects_blank_workspace_id_before_event_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.workspace_id = Some(" ".to_owned());
        request.events = vec![vec![0xff]];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("publish-events workspace ID is blank"));
        assert!(!error.contains("protobuf decode failed"));
    }

    #[test]
    fn publish_request_rejects_oversized_workspace_id_before_event_decode() {
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.workspace_id = Some("w".repeat(WORKSPACE_ID_MAX_BYTES + 1));
        request.events = vec![vec![0xff]];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("workspace ID is too large"));
        assert!(!error.contains("protobuf decode failed"));
    }

    #[test]
    fn publish_request_rejects_duplicate_events_before_event_store_lock() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let event = identity.sign_event(SignableEvent::new(
            workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Duplicate Target".to_owned(),
            },
        ));
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.event_envelopes =
            vec![encode_event_envelope(&event), encode_event_envelope(&event)];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("publish-events event duplicate value"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn publish_request_rejects_duplicate_proof_events_before_event_store_lock() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let event = identity.sign_event(SignableEvent::new(
            workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "Duplicate Proof".to_owned(),
            },
        ));
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.authorization_event_envelopes =
            vec![encode_event_envelope(&event), encode_event_envelope(&event)];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("publish-events proof event duplicate value"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn publish_request_rejects_duplicate_trust_snapshots_before_event_store_lock() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let snapshot = small_signed_trust_snapshot(&identity, &workspace_id);
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.authorization_snapshot_envelopes = vec![
            encode_trust_snapshot_envelope(&snapshot),
            encode_trust_snapshot_envelope(&snapshot),
        ];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("publish-events trust snapshot duplicate value"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn publish_request_rejects_oversized_typed_event_before_event_store_lock() {
        let oversized = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                WorkspaceId::new(),
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: chaft_types::MessageId::new(),
                    markdown: "x".repeat(EVENT_JSON_MAX_BYTES),
                    attachments: Vec::new(),
                },
            ),
            vec![1, 2, 3],
        );
        let mut request = empty_sync_request(WireSyncRequestKind::PublishEvents);
        request.event_envelopes = vec![encode_event_envelope(&oversized)];

        let error = handle_request(request, poisoned_event_store(), None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("event JSON is too large"));
        assert!(!error.contains("event store lock poisoned"));
    }

    #[test]
    fn put_blobs_request_rejects_unexpected_event_fields_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.event_ids = vec!["evt_unexpected".to_owned()];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unexpected put-blobs request fields: event_ids"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_too_many_blobs_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blobs = (0..=MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST)
            .map(|index| WireBlobEnvelope {
                hash: format!("{index:064x}"),
                bytes: Vec::new(),
            })
            .collect();

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs blob count 129 exceeds max 128"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_too_many_descriptors_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = (0..=MAX_BLOB_UPLOAD_DESCRIPTORS_PER_REQUEST)
            .map(|index| WireBlobDescriptor {
                hash: format!("{index:064x}"),
                byte_len: 0,
                chunk_size: 0,
                chunk_hashes: Vec::new(),
            })
            .collect();

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs descriptor count 129 exceeds max 128"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_duplicate_whole_blob_hashes_before_blob_store_lookup() {
        let bytes = b"duplicate whole blob upload".to_vec();
        let hash = blob_hash(&bytes);
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blobs = vec![
            WireBlobEnvelope {
                hash: hash.clone(),
                bytes: bytes.clone(),
            },
            WireBlobEnvelope { hash, bytes },
        ];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs blob duplicate value"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_duplicate_error_does_not_echo_unvalidated_hash() {
        let hash = "UNTRUSTED_DUPLICATE_BLOB_HASH".repeat(256);
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blobs = vec![
            WireBlobEnvelope {
                hash: hash.clone(),
                bytes: Vec::new(),
            },
            WireBlobEnvelope {
                hash,
                bytes: Vec::new(),
            },
        ];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs blob duplicate value"));
        assert!(!error.contains("UNTRUSTED_DUPLICATE_BLOB_HASH"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_duplicate_descriptors_before_blob_store_lookup() {
        let descriptor = describe_blob(b"duplicate descriptor upload", 8);
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = vec![
            descriptor_to_wire(&descriptor),
            descriptor_to_wire(&descriptor),
        ];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs descriptor duplicate value"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_duplicate_chunk_hashes_before_blob_store_lookup() {
        let bytes = b"duplicate chunk upload".to_vec();
        let descriptor = describe_blob(&bytes, 8);
        let first_chunk = bytes[..descriptor.chunk_size].to_vec();
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = vec![descriptor_to_wire(&descriptor)];
        request.blobs = vec![
            WireBlobEnvelope {
                hash: descriptor.chunk_hashes[0].clone(),
                bytes: first_chunk.clone(),
            },
            WireBlobEnvelope {
                hash: descriptor.chunk_hashes[0].clone(),
                bytes: first_chunk,
            },
        ];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs chunk duplicate value"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_noncanonical_whole_blob_hash_before_blob_store_lookup() {
        let bytes = b"noncanonical whole blob upload".to_vec();
        let mut hash = blob_hash(&bytes);
        hash.make_ascii_uppercase();
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blobs = vec![WireBlobEnvelope { hash, bytes }];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs blob non-canonical blob hash"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_whole_blob_hash_mismatch_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blobs = vec![WireBlobEnvelope {
            hash: blob_hash(b"declared whole blob"),
            bytes: b"different whole blob".to_vec(),
        }];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs blob hash mismatch"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_noncanonical_descriptor_hash_before_blob_store_lookup() {
        let descriptor = describe_blob(b"noncanonical descriptor upload", 8);
        let mut wire_descriptor = descriptor_to_wire(&descriptor);
        wire_descriptor.chunk_hashes[0].make_ascii_uppercase();
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = vec![wire_descriptor];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("blob descriptor chunk non-canonical blob hash"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn put_blobs_request_rejects_chunk_hash_mismatch_before_storage() {
        let blob_dir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(blob_dir.path().join("blobs")).unwrap();
        let descriptor = describe_blob(b"declared chunks", 4);
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = vec![descriptor_to_wire(&descriptor)];
        request.blobs = vec![WireBlobEnvelope {
            hash: descriptor.chunk_hashes[0].clone(),
            bytes: b"nope".to_vec(),
        }];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            Some(Arc::new(blob_store.clone())),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("put-blobs chunk hash mismatch"));
        assert!(blob_store.get_manifest(&descriptor.hash).unwrap().is_none());
        assert!(
            blob_store
                .get_chunk(&descriptor.chunk_hashes[0])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn put_blobs_request_rejects_chunk_not_declared_by_descriptor_before_storage() {
        let blob_dir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(blob_dir.path().join("blobs")).unwrap();
        let descriptor = describe_blob(b"declared chunks", 4);
        let stray_chunk = b"stray chunk".to_vec();
        let stray_hash = blob_hash(&stray_chunk);
        let mut request = empty_sync_request(WireSyncRequestKind::PutBlobs);
        request.blob_descriptors = vec![descriptor_to_wire(&descriptor)];
        request.blobs = vec![WireBlobEnvelope {
            hash: stray_hash.clone(),
            bytes: stray_chunk,
        }];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            Some(Arc::new(blob_store.clone())),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("not declared by descriptor"));
        assert!(blob_store.get_manifest(&descriptor.hash).unwrap().is_none());
        assert!(blob_store.get_chunk(&stray_hash).unwrap().is_none());
    }

    #[test]
    fn fetch_blobs_request_rejects_unexpected_event_fields_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        request.blob_hashes = vec![blob_hash(b"allowed fetch hash")];
        request.event_ids = vec!["evt_unexpected".to_owned()];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unexpected fetch-blobs request fields: event_ids"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn fetch_blobs_request_rejects_too_many_hashes_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        request.blob_hashes = (0..=MAX_FETCH_BLOB_HASHES_PER_REQUEST)
            .map(|index| format!("{index:064x}"))
            .collect();

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("fetch-blobs blob hash count 129 exceeds max 128"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn fetch_blobs_request_rejects_duplicate_hashes_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        let hash = blob_hash(b"duplicate fetch hash");
        request.blob_hashes = vec![hash.clone(), hash];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("fetch-blobs blob hash duplicate value"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn fetch_blobs_duplicate_error_does_not_echo_unvalidated_hash() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        let hash = "UNTRUSTED_DUPLICATE_FETCH_HASH".repeat(256);
        request.blob_hashes = vec![hash.clone(), hash];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("fetch-blobs blob hash duplicate value"));
        assert!(!error.contains("UNTRUSTED_DUPLICATE_FETCH_HASH"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn fetch_blobs_request_rejects_noncanonical_hash_before_blob_store_lookup() {
        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        request.blob_hashes = vec!["A".repeat(64)];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("blob store unavailable"));
    }

    #[test]
    fn fetch_blobs_request_rejects_oversized_response_before_write() {
        let blob_dir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(blob_dir.path().join("blobs")).unwrap();
        let bytes = vec![9; MAX_FRAME_LEN];
        let hash = blob_hash(&bytes);
        blob_store.put_bytes_with_hash(&hash, &bytes).unwrap();

        let mut request = empty_sync_request(WireSyncRequestKind::FetchBlobs);
        request.blob_hashes = vec![hash];

        let error = handle_request(
            request,
            Arc::new(Mutex::new(EventStore::open_in_memory().unwrap())),
            Some(Arc::new(blob_store)),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("fetch-blobs response frame length"));
        assert!(error.contains("exceeds max"));
    }

    #[test]
    fn fetch_blobs_response_rejects_noncanonical_descriptor_hash() {
        let hash = "A".repeat(64);
        let requested = BTreeSet::from([hash.clone()]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: vec![WireBlobDescriptor {
                hash,
                byte_len: 0,
                chunk_size: 1,
                chunk_hashes: Vec::new(),
            }],
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned non-canonical blob descriptor hash"));
    }

    #[test]
    fn fetch_blobs_response_rejects_duplicate_blob_entries() {
        let bytes = b"duplicate returned blob".to_vec();
        let hash = blob_hash(&bytes);
        let requested = BTreeSet::from([hash.clone(), blob_hash(b"other requested blob")]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: vec![
                WireBlobEnvelope {
                    hash: hash.clone(),
                    bytes: bytes.clone(),
                },
                WireBlobEnvelope {
                    hash: hash.clone(),
                    bytes,
                },
            ],
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned duplicate blob"));
    }

    #[test]
    fn fetch_blobs_response_rejects_more_blobs_than_requested_before_hashing() {
        let requested = BTreeSet::from(["0".repeat(64)]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: vec![
                WireBlobEnvelope {
                    hash: "not-a-hash".to_owned(),
                    bytes: Vec::new(),
                },
                WireBlobEnvelope {
                    hash: "also-not-a-hash".to_owned(),
                    bytes: Vec::new(),
                },
            ],
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-blobs blob count 2 exceeds requested limit 1"));
    }

    #[test]
    fn fetch_blobs_response_rejects_duplicate_descriptors() {
        let descriptor = describe_blob(b"duplicate returned descriptor", 4);
        let requested = BTreeSet::from([
            descriptor.hash.clone(),
            blob_hash(b"other requested descriptor"),
        ]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: vec![
                descriptor_to_wire(&descriptor),
                descriptor_to_wire(&descriptor),
            ],
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned duplicate blob descriptor"));
    }

    #[test]
    fn fetch_blobs_response_rejects_more_descriptors_than_requested_before_conversion() {
        let requested = BTreeSet::from(["0".repeat(64)]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: vec![
                WireBlobDescriptor {
                    hash: "not-a-hash".to_owned(),
                    byte_len: 0,
                    chunk_size: 0,
                    chunk_hashes: Vec::new(),
                },
                WireBlobDescriptor {
                    hash: "also-not-a-hash".to_owned(),
                    byte_len: 0,
                    chunk_size: 0,
                    chunk_hashes: Vec::new(),
                },
            ],
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-blobs descriptor count 2 exceeds requested limit 1"));
    }

    #[test]
    fn fetch_blob_availability_response_rejects_duplicate_entries() {
        let hash = blob_hash(b"duplicate returned availability");
        let requested = BTreeSet::from([hash.clone(), blob_hash(b"other requested availability")]);
        let availability = BlobAvailability {
            hash,
            has_whole_blob: true,
            descriptor: None,
            available_chunk_hashes: Vec::new(),
            missing_chunk_hashes: Vec::new(),
        };
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: vec![
                availability_to_wire(&availability),
                availability_to_wire(&availability),
            ],
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blob_availability_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("peer returned duplicate blob availability"));
    }

    #[test]
    fn fetch_blob_availability_response_rejects_more_entries_than_requested_before_conversion() {
        let requested = BTreeSet::from(["0".repeat(64)]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: vec![
                WireBlobAvailability {
                    hash: "not-a-hash".to_owned(),
                    has_whole_blob: false,
                    descriptor: None,
                    available_chunk_hashes: Vec::new(),
                    missing_chunk_hashes: Vec::new(),
                },
                WireBlobAvailability {
                    hash: "also-not-a-hash".to_owned(),
                    has_whole_blob: false,
                    descriptor: None,
                    available_chunk_hashes: Vec::new(),
                    missing_chunk_hashes: Vec::new(),
                },
            ],
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blob_availability_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(
            error
                .contains("fetch-blob-availability availability count 2 exceeds requested limit 1")
        );
    }

    #[test]
    fn fetch_blobs_response_rejects_more_availability_than_requested_before_conversion() {
        let requested = BTreeSet::from(["0".repeat(64)]);
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: vec![
                WireBlobAvailability {
                    hash: "not-a-hash".to_owned(),
                    has_whole_blob: false,
                    descriptor: None,
                    available_chunk_hashes: Vec::new(),
                    missing_chunk_hashes: Vec::new(),
                },
                WireBlobAvailability {
                    hash: "also-not-a-hash".to_owned(),
                    has_whole_blob: false,
                    descriptor: None,
                    available_chunk_hashes: Vec::new(),
                    missing_chunk_hashes: Vec::new(),
                },
            ],
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("fetch-blobs availability count 2 exceeds requested limit 1"));
    }

    #[test]
    fn fetch_blobs_response_rejects_noncanonical_availability_available_chunk_hash() {
        let descriptor = describe_blob(b"availability available chunk case", 8);
        let requested = BTreeSet::from([descriptor.hash.clone()]);
        let mut available_chunk_hash = descriptor.chunk_hashes[0].clone();
        available_chunk_hash.make_ascii_uppercase();
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: vec![WireBlobAvailability {
                hash: descriptor.hash.clone(),
                has_whole_blob: false,
                descriptor: Some(descriptor_to_wire(&descriptor)),
                available_chunk_hashes: vec![available_chunk_hash],
                missing_chunk_hashes: descriptor.chunk_hashes[1..].to_vec(),
            }],
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blobs_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("blob availability available chunk non-canonical blob hash"));
    }

    #[test]
    fn fetch_blob_availability_response_rejects_noncanonical_missing_chunk_hash() {
        let descriptor = describe_blob(b"availability missing chunk case", 8);
        let requested = BTreeSet::from([descriptor.hash.clone()]);
        let mut missing_chunk_hash = descriptor.chunk_hashes[1].clone();
        missing_chunk_hash.make_ascii_uppercase();
        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: vec![WireBlobAvailability {
                hash: descriptor.hash.clone(),
                has_whole_blob: false,
                descriptor: Some(descriptor_to_wire(&descriptor)),
                available_chunk_hashes: vec![descriptor.chunk_hashes[0].clone()],
                missing_chunk_hashes: vec![missing_chunk_hash],
            }],
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };

        let error = validate_fetch_blob_availability_response(&response, &requested)
            .unwrap_err()
            .to_string();

        assert!(error.contains("blob availability missing chunk non-canonical blob hash"));
    }

    #[test]
    fn blob_put_request_does_not_require_event_store_lock() {
        let store = Arc::new(Mutex::new(EventStore::open_in_memory().unwrap()));
        let poisoned_store = Arc::clone(&store);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned_store.lock().unwrap();
            panic!("poison event store lock");
        })
        .join();

        let blob_dir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(blob_dir.path().join("blobs")).unwrap();
        let bytes = b"blob-only request should not touch the event store".to_vec();
        let hash = blob_hash(&bytes);

        let response = handle_request(
            WireSyncRequest {
                kind: WireSyncRequestKind::PutBlobs as i32,
                event_ids: Vec::new(),
                events: Vec::new(),
                authorization_events: Vec::new(),
                authorization_snapshots: Vec::new(),
                blob_hashes: Vec::new(),
                blobs: vec![WireBlobEnvelope {
                    hash: hash.clone(),
                    bytes: bytes.clone(),
                }],
                blob_descriptors: Vec::new(),
                workspace_id: None,
                event_envelopes: Vec::new(),
                authorization_event_envelopes: Vec::new(),
                authorization_snapshot_envelopes: Vec::new(),
                inventory_start_index: None,
                inventory_limit: None,
            },
            store,
            Some(Arc::new(blob_store.clone())),
        )
        .unwrap();

        assert!(response.error.is_none());
        assert_eq!(blob_store.get_bytes(&hash).unwrap(), Some(bytes));
    }

    #[test]
    fn malformed_publish_is_rejected_before_event_store_lock() {
        let store = Arc::new(Mutex::new(EventStore::open_in_memory().unwrap()));
        let poisoned_store = Arc::clone(&store);
        let _ = std::thread::spawn(move || {
            let _guard = poisoned_store.lock().unwrap();
            panic!("poison event store lock");
        })
        .join();

        let error = handle_request(
            WireSyncRequest {
                kind: WireSyncRequestKind::PublishEvents as i32,
                event_ids: Vec::new(),
                events: vec![vec![0xff]],
                authorization_events: Vec::new(),
                authorization_snapshots: Vec::new(),
                blob_hashes: Vec::new(),
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                workspace_id: None,
                event_envelopes: Vec::new(),
                authorization_event_envelopes: Vec::new(),
                authorization_snapshot_envelopes: Vec::new(),
                inventory_start_index: None,
                inventory_limit: None,
            },
            store,
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("protocol error"));
        assert!(!error.contains("event store lock poisoned"));
    }

    fn large_device_key_package_event(workspace_id: &WorkspaceId, index: usize) -> SignedEvent {
        device_key_package_event(workspace_id, index, MAX_EVENT_UPLOAD_BATCH_BYTES / 2)
    }

    fn small_device_key_package_event(workspace_id: &WorkspaceId, index: usize) -> SignedEvent {
        device_key_package_event(workspace_id, index, 128)
    }

    fn small_signed_trust_snapshot(
        identity: &DeviceIdentity,
        workspace_id: &WorkspaceId,
    ) -> SignedTrustSnapshot {
        let root_event = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Trust Snapshot".to_owned(),
            },
        ));
        let snapshot = TrustSnapshot {
            schema_version: 1,
            workspace_id: workspace_id.clone(),
            root_event_id: root_event.event_id.clone(),
            root_author_device_id: identity.device_id().clone(),
            roles: Vec::new(),
            channels: Vec::new(),
            messages: Vec::new(),
            event_channels: Vec::new(),
        };
        identity.sign_trust_snapshot(snapshot, root_event).unwrap()
    }

    fn device_key_package_event(
        workspace_id: &WorkspaceId,
        index: usize,
        key_package_len: usize,
    ) -> SignedEvent {
        let event = SignableEvent::new(
            workspace_id.clone(),
            None,
            DeviceId(format!("dev_batch_{index}")),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: DeviceKeyPackageId(format!("dkp_batch_{index}")),
                protocol: "test".to_owned(),
                key_package: vec![index as u8; key_package_len],
            },
        );
        SignedEvent::from_author_signature(event, vec![index as u8; 32], vec![index as u8; 64])
    }
}
