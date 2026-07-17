use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    env, fmt,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex as StdMutex, OnceLock},
    task::{Context, Poll},
    time::Duration,
};

use async_trait::async_trait;
use chaft_media::{
    BlobAvailability, BlobDescriptor, blob_hash, describe_blob, validate_blob_descriptor,
    validate_chunk_payload, validate_reassembled_blob,
};
use chaft_net::{ChaftTransport, NetError, PeerAddress, PeerId};
use chaft_net_direct::{
    AccessEnvelopeTransport, AuthorizedPublishTransport, BlobSyncTransport, DirectTransport,
    MAX_ACTIVE_DIRECT_CONNECTIONS, MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST,
    MAX_CHUNK_UPLOAD_BATCH_BYTES, MAX_FETCH_BLOB_HASHES_PER_REQUEST,
    MAX_FETCH_EVENT_IDS_PER_REQUEST, MAX_FRAME_LEN, MAX_INVENTORY_EVENT_IDS_PER_RESPONSE,
    MAX_SYNC_RESPONSE_ERROR_BYTES, MAX_WHOLE_BLOB_UPLOAD_BATCH_BYTES,
    PreparedAccessEnvelopeExchange, SyncPeerStore, build_publish_events_requests,
    prepare_join_request_fetch, prepare_join_request_submission, prepare_join_response_fetch,
    prepare_join_response_submission, prepare_scoped_join_response_fetch, request_sync_stream,
    response_error_may_be_oversized_response, validate_decoded_event_size,
    validate_empty_ack_response, validate_fetch_blob_availability_response,
    validate_fetch_blobs_response, validate_fetch_events_response,
    validate_fetch_events_wire_response, validate_inventory_page_response,
    validate_inventory_response, validate_inventory_total_count, validate_request_blob_hashes,
    validate_request_event_ids, validate_wire_blob_availability_hashes,
    validate_wire_blob_descriptor_hashes, validate_wire_workspace_id,
};
use chaft_types::{
    EventId, SignedEvent, SignedTrustSnapshot, WorkspaceId,
    direct_tcp_peer_endpoint_address_is_valid,
};
use chaft_wire::{
    WireBlobAvailability, WireBlobDescriptor, WireBlobEnvelope, WireEventEnvelope, WireSyncRequest,
    WireSyncRequestKind, WireSyncResponse, decode_event, decode_event_envelope,
    encode_sync_request,
};
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, SecretKey, TransportAddr,
    endpoint::{Connection, RecvStream, SendStream, presets},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::Mutex,
    task::{JoinHandle, JoinSet},
    time::timeout,
};

pub const CHAFT_SYNC_ALPN: &[u8] = b"dev.chaft.sync.v1";
pub const MAX_ACTIVE_IROH_CONNECTIONS: usize = MAX_ACTIVE_DIRECT_CONNECTIONS;
pub const MAX_ACTIVE_IROH_STREAMS_PER_CONNECTION: usize = 128;
pub const MAX_CACHED_IROH_PEER_CONNECTIONS: usize = 64;
pub const NATIVE_IROH_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub const NATIVE_IROH_RELAY_READY_TIMEOUT: Duration = Duration::from_secs(12);
pub const CHAFT_IROH_ALLOW_PUBLIC_RELAYS_ENV: &str = "CHAFT_IROH_ALLOW_PUBLIC_RELAYS";
pub const CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY_ENV: &str = "CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY";
pub const CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE_ENV: &str = "CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE";
pub const IROH_POLICY_ENV_FLAG_MAX_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IrohTransportConfig {
    pub allow_public_relays: bool,
    pub allow_public_discovery: bool,
    pub allow_direct_tcp_bridge: bool,
}

#[derive(Clone)]
pub struct IrohTransport {
    pub config: IrohTransportConfig,
    direct: DirectTransport,
    native_client_endpoint: Arc<Mutex<Option<Endpoint>>>,
    native_connections: Arc<Mutex<NativeConnectionCache>>,
    native_connection_gates: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrohEndpointRoute {
    DirectTcp { address: String },
    NativeIroh { endpoint: String },
    PublicRelay { endpoint: String },
    PublicDiscovery { endpoint: String },
}

#[derive(Default)]
struct NativeConnectionCache {
    connections: HashMap<String, Connection>,
    order: VecDeque<String>,
}

impl NativeConnectionCache {
    fn live_connection(&mut self, endpoint: &str) -> (Option<Connection>, bool) {
        let Some(connection) = self.connections.get(endpoint) else {
            return (None, false);
        };
        if connection.close_reason().is_none() {
            let connection = connection.clone();
            self.touch(endpoint);
            return (Some(connection), false);
        }

        self.remove(endpoint);
        (None, true)
    }

    fn insert(
        &mut self,
        endpoint: String,
        connection: Connection,
        max_cached_connections: usize,
    ) -> Vec<String> {
        self.remove(&endpoint);
        self.connections.insert(endpoint.clone(), connection);
        self.order.push_back(endpoint);

        let mut evicted = Vec::new();
        while self.connections.len() > max_cached_connections {
            let Some(candidate) = self.order.pop_front() else {
                break;
            };
            if let Some(connection) = self.connections.remove(&candidate) {
                connection.close(0u8.into(), b"cache evicted");
                evicted.push(candidate);
            }
        }
        evicted
    }

    fn remove(&mut self, endpoint: &str) {
        if let Some(connection) = self.connections.remove(endpoint) {
            connection.close(0u8.into(), b"connection removed");
        }
        self.order.retain(|candidate| candidate != endpoint);
    }

    fn touch(&mut self, endpoint: &str) {
        self.order.retain(|candidate| candidate != endpoint);
        self.order.push_back(endpoint.to_owned());
    }
}

impl IrohTransport {
    pub fn new(config: IrohTransportConfig) -> Self {
        Self {
            config,
            direct: DirectTransport,
            native_client_endpoint: Arc::new(Mutex::new(None)),
            native_connections: Arc::new(Mutex::new(NativeConnectionCache::default())),
            native_connection_gates: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn from_environment() -> Self {
        Self::new(IrohTransportConfig::from_environment())
    }

    /// Returns a process-shared transport for the current environment policy.
    /// Clones share the native endpoint and live-connection cache.
    pub fn shared_from_environment() -> Self {
        Self::shared_for_config(IrohTransportConfig::from_environment())
    }

    fn shared_for_config(config: IrohTransportConfig) -> Self {
        static TRANSPORTS: OnceLock<StdMutex<HashMap<IrohTransportConfig, IrohTransport>>> =
            OnceLock::new();

        let transports = TRANSPORTS.get_or_init(|| StdMutex::new(HashMap::new()));
        let mut transports = transports
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        transports
            .entry(config.clone())
            .or_insert_with(|| Self::new(config))
            .clone()
    }

    pub fn classify_endpoint(endpoint: &str) -> Result<IrohEndpointRoute, NetError> {
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            return Err(NetError::Protocol("peer endpoint is empty".to_owned()));
        }

        if let Some(address) = endpoint.strip_prefix("direct+tcp://") {
            return Self::direct_tcp_route(address);
        }
        if let Some(address) = endpoint.strip_prefix("tcp://") {
            return Self::direct_tcp_route(address);
        }
        if endpoint.contains("://") {
            if endpoint.starts_with("iroh+relay://") || endpoint.starts_with("relay://") {
                return Ok(IrohEndpointRoute::PublicRelay {
                    endpoint: endpoint.to_owned(),
                });
            }
            if endpoint.starts_with("iroh+discovery://") || endpoint.starts_with("discovery://") {
                return Ok(IrohEndpointRoute::PublicDiscovery {
                    endpoint: endpoint.to_owned(),
                });
            }
            if endpoint.starts_with("iroh://") {
                return Ok(IrohEndpointRoute::NativeIroh {
                    endpoint: endpoint.to_owned(),
                });
            }
            let scheme = endpoint
                .split_once("://")
                .map(|(scheme, _)| scheme)
                .unwrap_or(endpoint);
            return Err(NetError::Protocol(format!(
                "unsupported peer endpoint scheme: {scheme}"
            )));
        }

        Self::direct_tcp_route(endpoint)
    }

    fn direct_tcp_route(address: &str) -> Result<IrohEndpointRoute, NetError> {
        let address = address.trim();
        if !direct_tcp_peer_endpoint_address_is_valid(address) {
            return Err(NetError::Protocol(
                "direct TCP endpoint must be host:port with nonzero numeric port".to_owned(),
            ));
        }
        Ok(IrohEndpointRoute::DirectTcp {
            address: address.to_owned(),
        })
    }

    fn resolve_peer(&self, peer: &PeerAddress) -> Result<ResolvedPeer, NetError> {
        match Self::classify_endpoint(&peer.endpoint)? {
            IrohEndpointRoute::DirectTcp { address } => self.direct_tcp_peer(peer, &address),
            IrohEndpointRoute::NativeIroh { endpoint } => {
                let native_peer = parse_native_endpoint(&endpoint, &self.config)?;
                Ok(ResolvedPeer::NativeIroh {
                    endpoint: native_peer.endpoint,
                })
            }
            IrohEndpointRoute::PublicRelay { .. } => {
                if !self.config.allow_public_relays {
                    return Err(NetError::Unavailable(
                        "iroh public relay endpoints are disabled",
                    ));
                }
                Err(NetError::Unavailable(
                    "native iroh relay backend is not linked yet",
                ))
            }
            IrohEndpointRoute::PublicDiscovery { .. } => {
                if !self.config.allow_public_discovery {
                    return Err(NetError::Unavailable(
                        "iroh public discovery endpoints are disabled",
                    ));
                }
                Err(NetError::Unavailable(
                    "native iroh discovery backend is not linked yet",
                ))
            }
        }
    }

    fn direct_tcp_peer(&self, peer: &PeerAddress, address: &str) -> Result<ResolvedPeer, NetError> {
        if !self.config.allow_direct_tcp_bridge {
            return Err(NetError::Unavailable("direct TCP bridge is disabled"));
        }
        let address = address.trim();
        if address.is_empty() {
            return Err(NetError::Protocol(
                "direct TCP endpoint is empty".to_owned(),
            ));
        }

        Ok(ResolvedPeer::DirectTcp(PeerAddress {
            peer_id: PeerId(peer.peer_id.0.clone()),
            endpoint: address.to_owned(),
        }))
    }

    async fn native_request(
        &self,
        endpoint: &str,
        request: WireSyncRequest,
    ) -> Result<WireSyncResponse, NetError> {
        self.request_native_endpoint(endpoint, request).await
    }

    async fn native_client_endpoint(&self) -> Result<Endpoint, NetError> {
        let mut endpoint = self.native_client_endpoint.lock().await;
        if let Some(endpoint) = endpoint.as_ref() {
            return Ok(endpoint.clone());
        }

        let bound = bind_native_endpoint(&self.config).await?;
        *endpoint = Some(bound.clone());
        Ok(bound)
    }

    async fn request_native_endpoint(
        &self,
        endpoint: &str,
        request: WireSyncRequest,
    ) -> Result<WireSyncResponse, NetError> {
        let parsed = parse_native_endpoint(endpoint, &self.config)?;
        let endpoint_key = parsed.endpoint.clone();
        let connection = self.native_connection(parsed).await?;
        let response = self
            .request_native_connection_stream(&connection, request)
            .await;
        if response.is_err() {
            self.remove_native_connection(&endpoint_key).await;
        }
        response
    }

    async fn native_connection(
        &self,
        parsed: ParsedNativeEndpoint,
    ) -> Result<Connection, NetError> {
        self.native_connection_with_cache_limit(parsed, MAX_CACHED_IROH_PEER_CONNECTIONS)
            .await
    }

    async fn native_connection_with_cache_limit(
        &self,
        parsed: ParsedNativeEndpoint,
        max_cached_connections: usize,
    ) -> Result<Connection, NetError> {
        if max_cached_connections == 0 {
            return Err(NetError::Protocol(
                "native Iroh connection cache limit must be greater than zero".to_owned(),
            ));
        }

        if let Some(connection) = self.cached_live_native_connection(&parsed.endpoint).await {
            return Ok(connection);
        }

        let endpoint_key = parsed.endpoint.clone();
        let connection_gate = self.native_connection_gate(&endpoint_key).await;
        let _connection_guard = connection_gate.lock().await;
        if let Some(connection) = self.cached_live_native_connection(&endpoint_key).await {
            return Ok(connection);
        }

        let local = self.native_client_endpoint().await?;
        let connection = match timeout(
            NATIVE_IROH_HANDSHAKE_TIMEOUT,
            local.connect(parsed.addr, CHAFT_SYNC_ALPN),
        )
        .await
        {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => {
                self.remove_native_connection_gate(&endpoint_key).await;
                return Err(NetError::Io(error.to_string()));
            }
            Err(_) => {
                self.remove_native_connection_gate(&endpoint_key).await;
                return Err(native_iroh_timeout_error("connect"));
            }
        };

        let evicted_endpoints = {
            let mut connections = self.native_connections.lock().await;
            connections.insert(endpoint_key, connection.clone(), max_cached_connections)
        };
        for endpoint in evicted_endpoints {
            self.remove_native_connection_gate(&endpoint).await;
        }
        Ok(connection)
    }

    async fn cached_live_native_connection(&self, endpoint: &str) -> Option<Connection> {
        let (connection, remove_gate) = {
            let mut connections = self.native_connections.lock().await;
            connections.live_connection(endpoint)
        };
        if remove_gate {
            self.remove_native_connection_gate(endpoint).await;
        }
        connection
    }

    async fn native_connection_gate(&self, endpoint: &str) -> Arc<Mutex<()>> {
        let mut gates = self.native_connection_gates.lock().await;
        gates
            .entry(endpoint.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn remove_native_connection(&self, endpoint: &str) {
        {
            let mut connections = self.native_connections.lock().await;
            connections.remove(endpoint);
        }
        self.remove_native_connection_gate(endpoint).await;
    }

    async fn remove_native_connection_gate(&self, endpoint: &str) {
        self.native_connection_gates.lock().await.remove(endpoint);
    }

    async fn request_native_connection_stream(
        &self,
        connection: &Connection,
        request: WireSyncRequest,
    ) -> Result<WireSyncResponse, NetError> {
        let (send, recv) = timeout(NATIVE_IROH_HANDSHAKE_TIMEOUT, connection.open_bi())
            .await
            .map_err(|_| native_iroh_timeout_error("open bidirectional stream"))?
            .map_err(|error| NetError::Io(error.to_string()))?;
        let mut stream = IrohBiStream::new(send, recv);
        let response = request_sync_stream(&mut stream, request).await;
        if response.is_ok() {
            stream
                .shutdown()
                .await
                .map_err(|error| NetError::Io(error.to_string()))?;
        }
        response
    }

    async fn native_fetch_inventory_paged(
        &self,
        endpoint: &str,
        workspace_id: Option<&WorkspaceId>,
    ) -> Result<Vec<EventId>, NetError> {
        if let Some(workspace_id) = workspace_id {
            validate_wire_workspace_id("inventory", &workspace_id.0)?;
        }

        let mut start_index = 0usize;
        let mut event_ids = Vec::new();
        let mut seen = BTreeSet::new();

        loop {
            let mut response = self
                .native_request(
                    endpoint,
                    inventory_request(
                        workspace_id,
                        Some(start_index),
                        Some(MAX_INVENTORY_EVENT_IDS_PER_RESPONSE),
                    ),
                )
                .await?;
            response_error(response.error.take())?;
            validate_inventory_response(&response)?;
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

    async fn native_fetch_events_batched(
        &self,
        endpoint: &str,
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
            match self.native_fetch_events_once(endpoint, batch.clone()).await {
                Ok(mut fetched) => events.append(&mut fetched),
                Err(error)
                    if batch.len() > 1 && response_error_may_be_oversized_response(&error) =>
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

    async fn native_fetch_events_once(
        &self,
        endpoint: &str,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }

        validate_request_event_ids(event_ids.iter().map(|event_id| event_id.0.as_str()))?;
        let requested = event_ids.iter().cloned().collect::<BTreeSet<_>>();
        let mut response = self
            .native_request(endpoint, fetch_events_request(event_ids))
            .await?;
        response_error(response.error.take())?;
        validate_fetch_events_wire_response(&response, requested.len())?;
        let events = decode_events(response.event_envelopes, response.events)?;
        validate_fetch_events_response(&events, &requested)?;
        Ok(events)
    }

    async fn native_put_blob_envelopes_batched(
        &self,
        endpoint: &str,
        envelopes: Vec<WireBlobEnvelope>,
    ) -> Result<(), NetError> {
        let base_bytes = encode_sync_request(&put_blob_envelopes_request(Vec::new())).len();
        let mut batch = Vec::new();
        let mut batch_bytes = base_bytes;

        for envelope in envelopes {
            let envelope_bytes =
                encode_sync_request(&put_blob_envelopes_request(vec![envelope.clone()]))
                    .len()
                    .saturating_sub(base_bytes);
            let single_frame_bytes = base_bytes.saturating_add(envelope_bytes);
            if single_frame_bytes > MAX_FRAME_LEN {
                return Err(NetError::Protocol(format!(
                    "blob upload frame length {single_frame_bytes} exceeds max {MAX_FRAME_LEN}"
                )));
            }

            if !batch.is_empty()
                && (batch.len() >= MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST
                    || batch_bytes.saturating_add(envelope_bytes)
                        > MAX_WHOLE_BLOB_UPLOAD_BATCH_BYTES)
            {
                let mut response = self
                    .native_request(
                        endpoint,
                        put_blob_envelopes_request(std::mem::take(&mut batch)),
                    )
                    .await?;
                response_error(response.error.take())?;
                validate_empty_ack_response(&response)?;
                batch_bytes = base_bytes;
            }

            batch_bytes = batch_bytes.saturating_add(envelope_bytes);
            batch.push(envelope);
        }

        if !batch.is_empty() {
            let mut response = self
                .native_request(endpoint, put_blob_envelopes_request(batch))
                .await?;
            response_error(response.error.take())?;
            validate_empty_ack_response(&response)?;
        }

        Ok(())
    }

    async fn native_fetch_blobs_responses_batched(
        &self,
        endpoint: &str,
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
            match self
                .native_fetch_blobs_response_once(endpoint, batch.clone())
                .await
            {
                Ok(response) => responses.push(response),
                Err(error)
                    if batch.len() > 1 && response_error_may_be_oversized_response(&error) =>
                {
                    let mid = batch.len() / 2;
                    pending.push(batch[mid..].to_vec());
                    pending.push(batch[..mid].to_vec());
                }
                Err(error) => return Err(error),
            }
        }

        Ok(responses)
    }

    async fn native_fetch_blobs_response_once(
        &self,
        endpoint: &str,
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
        let mut response = self
            .native_request(endpoint, fetch_blobs_request(hashes))
            .await?;
        response_error(response.error.take())?;
        validate_fetch_blobs_response(&response, &requested)?;
        Ok(response)
    }

    async fn native_fetch_blob_availability_responses_batched(
        &self,
        endpoint: &str,
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
            match self
                .native_fetch_blob_availability_response_once(endpoint, batch.clone())
                .await
            {
                Ok(response) => responses.push(response),
                Err(error)
                    if batch.len() > 1 && response_error_may_be_oversized_response(&error) =>
                {
                    let mid = batch.len() / 2;
                    pending.push(batch[mid..].to_vec());
                    pending.push(batch[..mid].to_vec());
                }
                Err(error) => return Err(error),
            }
        }

        Ok(responses)
    }

    async fn native_fetch_blob_availability_response_once(
        &self,
        endpoint: &str,
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
        let mut response = self
            .native_request(endpoint, fetch_blob_availability_request(hashes))
            .await?;
        response_error(response.error.take())?;
        validate_fetch_blob_availability_response(&response, &requested)?;
        Ok(response)
    }

    pub async fn submit_join_request(
        &self,
        peer: &PeerAddress,
        workspace_id: Option<&WorkspaceId>,
        request: Vec<u8>,
    ) -> Result<(), NetError> {
        let exchange = prepare_join_request_submission(workspace_id, &request)?;
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .submit_join_request(&peer, workspace_id, request)
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => self
                .execute_native_access_envelope_exchange(&endpoint, exchange)
                .await
                .map(|_| ()),
        }
    }

    pub async fn submit_join_response(
        &self,
        peer: &PeerAddress,
        workspace_id: Option<&WorkspaceId>,
        response_package: Vec<u8>,
    ) -> Result<(), NetError> {
        let exchange = prepare_join_response_submission(workspace_id, &response_package)?;
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .submit_join_response(&peer, workspace_id, response_package)
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => self
                .execute_native_access_envelope_exchange(&endpoint, exchange)
                .await
                .map(|_| ()),
        }
    }

    pub async fn fetch_join_requests(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let exchange = prepare_join_request_fetch(workspace_id, max_entries)?;
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .fetch_join_requests(&peer, workspace_id, max_entries)
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                self.execute_native_access_envelope_exchange(&endpoint, exchange)
                    .await
            }
        }
    }

    pub async fn fetch_join_responses(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let exchange = prepare_join_response_fetch(workspace_id, max_entries)?;
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .fetch_join_responses(&peer, workspace_id, max_entries)
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                self.execute_native_access_envelope_exchange(&endpoint, exchange)
                    .await
            }
        }
    }

    pub async fn fetch_join_responses_for_requests(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        request_ids: Vec<String>,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let Some(exchange) =
            prepare_scoped_join_response_fetch(workspace_id, &request_ids, max_entries)?
        else {
            return Ok(Vec::new());
        };
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .fetch_join_responses_for_requests(
                        &peer,
                        workspace_id,
                        request_ids,
                        max_entries,
                    )
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                self.execute_native_access_envelope_exchange(&endpoint, exchange)
                    .await
            }
        }
    }

    async fn execute_native_access_envelope_exchange(
        &self,
        endpoint: &str,
        exchange: PreparedAccessEnvelopeExchange,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        let response = self
            .native_request(endpoint, exchange.wire_request())
            .await?;
        exchange.validate_response(response)
    }
}

fn deduplicate_event_ids(event_ids: Vec<EventId>) -> Vec<EventId> {
    let mut seen = BTreeSet::new();
    event_ids
        .into_iter()
        .filter(|event_id| seen.insert(event_id.clone()))
        .collect()
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

fn validate_native_chunk_upload_single_frame_lengths(
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
            encode_sync_request(&put_blob_chunks_request(descriptor, vec![envelope])).len();
        if frame_len > MAX_FRAME_LEN {
            return Err(NetError::Protocol(format!(
                "chunk upload frame length {frame_len} exceeds max {MAX_FRAME_LEN}"
            )));
        }
    }

    Ok(())
}

impl fmt::Debug for IrohTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IrohTransport")
            .field("config", &self.config)
            .field("direct", &self.direct)
            .finish_non_exhaustive()
    }
}

impl Default for IrohTransport {
    fn default() -> Self {
        Self::new(IrohTransportConfig::default())
    }
}

impl Default for IrohTransportConfig {
    fn default() -> Self {
        Self {
            allow_public_relays: false,
            allow_public_discovery: false,
            allow_direct_tcp_bridge: true,
        }
    }
}

impl IrohTransportConfig {
    pub fn from_environment() -> Self {
        Self::from_env_lookup(|key| env::var(key).ok())
    }

    fn from_env_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            allow_public_relays: lookup(CHAFT_IROH_ALLOW_PUBLIC_RELAYS_ENV)
                .is_some_and(|value| parse_env_enabled_flag(&value)),
            allow_public_discovery: lookup(CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY_ENV)
                .is_some_and(|value| parse_env_enabled_flag(&value)),
            allow_direct_tcp_bridge: !lookup(CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE_ENV)
                .is_some_and(|value| parse_env_enabled_flag(&value)),
        }
    }
}

fn parse_env_enabled_flag(value: &str) -> bool {
    if value.len() > IROH_POLICY_ENV_FLAG_MAX_BYTES {
        return false;
    }

    let value = value.trim();
    value == "1"
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
}

#[derive(Debug, Clone)]
enum ResolvedPeer {
    DirectTcp(PeerAddress),
    NativeIroh { endpoint: String },
}

pub struct IrohSyncPeer {
    endpoint: Endpoint,
    advertise_by_discovery: bool,
    task: JoinHandle<Result<(), NetError>>,
}

impl IrohSyncPeer {
    pub async fn bind(
        sync_store: SyncPeerStore,
        config: IrohTransportConfig,
    ) -> Result<Self, NetError> {
        Self::bind_inner(sync_store, config, None).await
    }

    pub async fn bind_with_secret_key_bytes(
        sync_store: SyncPeerStore,
        config: IrohTransportConfig,
        secret_key_bytes: [u8; 32],
    ) -> Result<Self, NetError> {
        Self::bind_inner(sync_store, config, Some(secret_key_bytes)).await
    }

    async fn bind_inner(
        sync_store: SyncPeerStore,
        config: IrohTransportConfig,
        secret_key_bytes: Option<[u8; 32]>,
    ) -> Result<Self, NetError> {
        let endpoint = bind_native_endpoint_with_secret(&config, secret_key_bytes).await?;
        if config.allow_public_relays
            && timeout(NATIVE_IROH_RELAY_READY_TIMEOUT, endpoint.online())
                .await
                .is_err()
        {
            endpoint.close().await;
            return Err(NetError::Unavailable(
                "Iroh relay did not become reachable before timeout",
            ));
        }
        let accept_endpoint = endpoint.clone();
        let task =
            tokio::spawn(async move { serve_native_endpoint(accept_endpoint, sync_store).await });
        Ok(Self {
            endpoint,
            advertise_by_discovery: config.allow_public_discovery,
            task,
        })
    }

    pub fn endpoint_url(&self) -> String {
        if self.advertise_by_discovery {
            format!("iroh://{}", self.endpoint.id())
        } else {
            native_endpoint_url(&self.endpoint)
        }
    }

    pub async fn close(self) -> Result<(), NetError> {
        self.endpoint.close().await;
        match self.task.await {
            Ok(result) => result,
            Err(error) if error.is_cancelled() => Ok(()),
            Err(error) => Err(NetError::Io(error.to_string())),
        }
    }
}

struct IrohBiStream {
    send: SendStream,
    recv: RecvStream,
}

impl IrohBiStream {
    fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }
}

impl AsyncRead for IrohBiStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for IrohBiStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.send).poll_write(cx, buf) {
            Poll::Ready(Ok(written)) => Poll::Ready(Ok(written)),
            Poll::Ready(Err(error)) => Poll::Ready(Err(std::io::Error::other(error.to_string()))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.send).poll_flush(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(std::io::Error::other(error.to_string()))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        match Pin::new(&mut this.send).poll_shutdown(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(std::io::Error::other(error.to_string()))),
            Poll::Pending => Poll::Pending,
        }
    }
}

struct ParsedNativeEndpoint {
    endpoint: String,
    addr: EndpointAddr,
}

fn inventory_request(
    workspace_id: Option<&WorkspaceId>,
    inventory_start_index: Option<usize>,
    inventory_limit: Option<usize>,
) -> WireSyncRequest {
    WireSyncRequest {
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
        inventory_start_index: inventory_start_index.map(|value| value as u64),
        inventory_limit: inventory_limit.map(|value| value as u64),
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

fn put_blob_manifest_request(descriptor: &BlobDescriptor) -> WireSyncRequest {
    WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: Vec::new(),
        blob_descriptors: vec![descriptor_to_wire(descriptor)],
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    }
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

fn descriptor_to_wire(descriptor: &BlobDescriptor) -> WireBlobDescriptor {
    WireBlobDescriptor {
        hash: descriptor.hash.clone(),
        byte_len: descriptor.byte_len,
        chunk_size: descriptor.chunk_size as u64,
        chunk_hashes: descriptor.chunk_hashes.clone(),
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

fn parse_native_endpoint(
    endpoint: &str,
    config: &IrohTransportConfig,
) -> Result<ParsedNativeEndpoint, NetError> {
    let endpoint = endpoint.trim();
    let endpoint = endpoint.strip_prefix("iroh://").ok_or_else(|| {
        NetError::Protocol("native iroh endpoint must start with iroh://".to_owned())
    })?;
    let (endpoint_id, query) = endpoint.split_once('?').unwrap_or((endpoint, ""));
    if endpoint_id.is_empty() {
        return Err(NetError::Protocol(
            "native iroh endpoint ID is required".to_owned(),
        ));
    }
    let endpoint_id = endpoint_id
        .parse::<EndpointId>()
        .map_err(|error| NetError::Protocol(format!("invalid native iroh endpoint ID: {error}")))?;

    let mut addrs = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').ok_or_else(|| {
            NetError::Protocol(format!("invalid native iroh endpoint query: {pair}"))
        })?;
        match key {
            "addr" => {
                let addr = value.parse::<SocketAddr>().map_err(|error| {
                    NetError::Protocol(format!("invalid native iroh direct address: {error}"))
                })?;
                if addr.port() == 0 {
                    return Err(NetError::Protocol(
                        "invalid native iroh direct address: port must be greater than zero"
                            .to_owned(),
                    ));
                }
                addrs.push(TransportAddr::Ip(addr));
            }
            "relay" => {
                if !config.allow_public_relays {
                    return Err(NetError::Unavailable(
                        "iroh public relay endpoints are disabled",
                    ));
                }
                let relay = value.parse::<RelayUrl>().map_err(|error| {
                    NetError::Protocol(format!("invalid native iroh relay URL: {error}"))
                })?;
                addrs.push(TransportAddr::Relay(relay));
            }
            unknown => {
                return Err(NetError::Protocol(format!(
                    "unsupported native iroh endpoint query key: {unknown}"
                )));
            }
        }
    }
    if addrs.is_empty() && !config.allow_public_discovery {
        return Err(NetError::Unavailable(
            "iroh public discovery endpoints are disabled",
        ));
    }

    Ok(ParsedNativeEndpoint {
        endpoint: format_native_endpoint_addr(endpoint_id, addrs.iter()),
        addr: EndpointAddr::from_parts(endpoint_id, addrs),
    })
}

fn format_native_endpoint_addr<'a>(
    endpoint_id: EndpointId,
    addrs: impl IntoIterator<Item = &'a TransportAddr>,
) -> String {
    let mut endpoint = format!("iroh://{endpoint_id}");
    let mut first = true;
    for addr in addrs {
        endpoint.push(if first { '?' } else { '&' });
        first = false;
        match addr {
            TransportAddr::Ip(addr) => {
                endpoint.push_str("addr=");
                endpoint.push_str(&addr.to_string());
            }
            TransportAddr::Relay(url) => {
                endpoint.push_str("relay=");
                endpoint.push_str(url.as_str());
            }
            TransportAddr::Custom(addr) => {
                endpoint.push_str("custom=");
                endpoint.push_str(&addr.to_string());
            }
            _ => {}
        }
    }
    endpoint
}

fn native_endpoint_url(endpoint: &Endpoint) -> String {
    let addr = endpoint.addr();
    let addrs = addr
        .ip_addrs()
        .copied()
        .map(TransportAddr::Ip)
        .chain(addr.relay_urls().cloned().map(TransportAddr::Relay))
        .collect::<Vec<_>>();
    format_native_endpoint_addr(endpoint.id(), addrs.iter())
}

async fn bind_native_endpoint(config: &IrohTransportConfig) -> Result<Endpoint, NetError> {
    bind_native_endpoint_with_secret(config, None).await
}

async fn bind_native_endpoint_with_secret(
    config: &IrohTransportConfig,
    secret_key_bytes: Option<[u8; 32]>,
) -> Result<Endpoint, NetError> {
    let builder = if config.allow_public_discovery {
        Endpoint::builder(presets::N0)
    } else {
        Endpoint::builder(presets::Minimal)
    };
    builder
        .secret_key(
            secret_key_bytes
                .map(|bytes| SecretKey::from_bytes(&bytes))
                .unwrap_or_else(SecretKey::generate),
        )
        .alpns(vec![CHAFT_SYNC_ALPN.to_vec()])
        .relay_mode(if config.allow_public_relays {
            RelayMode::Default
        } else {
            RelayMode::Disabled
        })
        .bind()
        .await
        .map_err(|error| NetError::Io(error.to_string()))
}

async fn serve_native_endpoint(
    endpoint: Endpoint,
    sync_store: SyncPeerStore,
) -> Result<(), NetError> {
    serve_native_endpoint_with_connection_limit(endpoint, sync_store, MAX_ACTIVE_IROH_CONNECTIONS)
        .await
}

async fn serve_native_endpoint_with_connection_limit(
    endpoint: Endpoint,
    sync_store: SyncPeerStore,
    max_active_connections: usize,
) -> Result<(), NetError> {
    serve_native_endpoint_with_limits(
        endpoint,
        sync_store,
        max_active_connections,
        MAX_ACTIVE_IROH_STREAMS_PER_CONNECTION,
    )
    .await
}

async fn serve_native_endpoint_with_limits(
    endpoint: Endpoint,
    sync_store: SyncPeerStore,
    max_active_connections: usize,
    max_active_streams_per_connection: usize,
) -> Result<(), NetError> {
    if max_active_connections == 0 {
        return Err(NetError::Protocol(
            "native Iroh connection limit must be greater than zero".to_owned(),
        ));
    }
    if max_active_streams_per_connection == 0 {
        return Err(NetError::Protocol(
            "native Iroh stream limit must be greater than zero".to_owned(),
        ));
    }

    let mut connections = JoinSet::new();
    let mut active_connections = 0usize;

    loop {
        tokio::select! {
            incoming = endpoint.accept(), if active_connections < max_active_connections => {
                let Some(incoming) = incoming else {
                    break;
                };
                let sync_store = sync_store.clone();
                connections.spawn(async move {
                    let result = async {
                        let mut accepting = incoming
                            .accept()
                            .map_err(|error| NetError::Io(error.to_string()))?;
                        let alpn = accepting
                            .alpn();
                        let alpn = timeout(NATIVE_IROH_HANDSHAKE_TIMEOUT, alpn)
                            .await
                            .map_err(|_| native_iroh_timeout_error("accept ALPN"))?
                            .map_err(|error| NetError::Io(error.to_string()))?;
                        if alpn != CHAFT_SYNC_ALPN {
                            return Err(NetError::Protocol(format!(
                                "unsupported iroh ALPN: {}",
                                String::from_utf8_lossy(&alpn)
                            )));
                        }
                        let connection = timeout(NATIVE_IROH_HANDSHAKE_TIMEOUT, accepting)
                            .await
                            .map_err(|_| native_iroh_timeout_error("accept connection"))?
                            .map_err(|error| NetError::Io(error.to_string()))?;
                        serve_native_connection(
                            connection,
                            sync_store,
                            max_active_streams_per_connection,
                        )
                        .await
                    }
                    .await;
                    let _ = result;
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
        }
    }
    Ok(())
}

async fn serve_native_connection(
    connection: Connection,
    sync_store: SyncPeerStore,
    max_active_streams: usize,
) -> Result<(), NetError> {
    if max_active_streams == 0 {
        return Err(NetError::Protocol(
            "native Iroh stream limit must be greater than zero".to_owned(),
        ));
    }

    let mut streams = JoinSet::new();

    loop {
        tokio::select! {
            accepted = connection.accept_bi(), if streams.len() < max_active_streams => {
                match accepted {
                    Ok((send, recv)) => {
                        let sync_store = sync_store.clone();
                        streams.spawn(async move {
                            let mut stream = IrohBiStream::new(send, recv);
                            sync_store.serve_stream(&mut stream).await
                        });
                    }
                    Err(_) => break,
                }
            }
            result = streams.join_next(), if !streams.is_empty() => {
                let _ = result;
            }
        }
    }

    while streams.join_next().await.is_some() {}
    Ok(())
}

fn native_iroh_timeout_error(operation: &str) -> NetError {
    NetError::Protocol(format!(
        "native Iroh {operation} timed out after {} ms",
        NATIVE_IROH_HANDSHAKE_TIMEOUT.as_millis()
    ))
}

#[async_trait]
impl ChaftTransport for IrohTransport {
    async fn connect(&self, peer: PeerAddress) -> Result<(), NetError> {
        match self.resolve_peer(&peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.connect(peer).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                let mut response = self
                    .native_request(&endpoint, inventory_request(None, Some(0), Some(0)))
                    .await?;
                response_error(response.error.take())?;
                validate_inventory_response(&response)?;
                validate_inventory_page_response(&response, 0)
            }
        }
    }

    async fn fetch_inventory(&self, peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.fetch_inventory(&peer).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                self.native_fetch_inventory_paged(&endpoint, None).await
            }
        }
    }

    async fn fetch_workspace_inventory(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .fetch_workspace_inventory(&peer, workspace_id)
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                self.native_fetch_inventory_paged(&endpoint, Some(workspace_id))
                    .await
            }
        }
    }

    async fn publish_event(&self, peer: &PeerAddress, event: SignedEvent) -> Result<(), NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.publish_event(&peer, event).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                for request in build_publish_events_requests(vec![event], Vec::new(), Vec::new())? {
                    let mut response = self.native_request(&endpoint, request).await?;
                    response_error(response.error.take())?;
                    validate_empty_ack_response(&response)?;
                }
                Ok(())
            }
        }
    }

    async fn fetch_events(
        &self,
        peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.fetch_events(&peer, event_ids).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                self.native_fetch_events_batched(&endpoint, event_ids).await
            }
        }
    }
}

#[async_trait]
impl AccessEnvelopeTransport for IrohTransport {
    async fn submit_join_request(
        &self,
        peer: &PeerAddress,
        workspace_id: Option<&WorkspaceId>,
        request: Vec<u8>,
    ) -> Result<(), NetError> {
        IrohTransport::submit_join_request(self, peer, workspace_id, request).await
    }

    async fn submit_join_response(
        &self,
        peer: &PeerAddress,
        workspace_id: Option<&WorkspaceId>,
        response_package: Vec<u8>,
    ) -> Result<(), NetError> {
        IrohTransport::submit_join_response(self, peer, workspace_id, response_package).await
    }

    async fn fetch_join_requests(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        IrohTransport::fetch_join_requests(self, peer, workspace_id, max_entries).await
    }

    async fn fetch_join_responses(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        IrohTransport::fetch_join_responses(self, peer, workspace_id, max_entries).await
    }

    async fn fetch_join_responses_for_requests(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
        request_ids: Vec<String>,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, NetError> {
        IrohTransport::fetch_join_responses_for_requests(
            self,
            peer,
            workspace_id,
            request_ids,
            max_entries,
        )
        .await
    }
}

#[async_trait]
impl AuthorizedPublishTransport for IrohTransport {
    async fn publish_events_with_authorization(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
        authorization_snapshots: Vec<SignedTrustSnapshot>,
    ) -> Result<(), NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct
                    .publish_events_with_authorization(
                        &peer,
                        events,
                        authorization_events,
                        authorization_snapshots,
                    )
                    .await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                for request in build_publish_events_requests(
                    events,
                    authorization_events,
                    authorization_snapshots,
                )? {
                    let mut response = self.native_request(&endpoint, request).await?;
                    response_error(response.error.take())?;
                    validate_empty_ack_response(&response)?;
                }
                Ok(())
            }
        }
    }
}

#[async_trait]
impl BlobSyncTransport for IrohTransport {
    async fn put_blobs(
        &self,
        peer: &PeerAddress,
        blobs: Vec<Vec<u8>>,
    ) -> Result<Vec<String>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.put_blobs(&peer, blobs).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                let (envelopes, blob_hashes) = whole_blob_upload_envelopes(blobs);
                self.native_put_blob_envelopes_batched(&endpoint, envelopes)
                    .await?;
                Ok(blob_hashes)
            }
        }
    }

    async fn fetch_blobs(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.fetch_blobs(&peer, hashes).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                let mut blobs = HashMap::new();
                for response in self
                    .native_fetch_blobs_responses_batched(&endpoint, hashes)
                    .await?
                {
                    blobs.extend(
                        response
                            .blobs
                            .into_iter()
                            .map(|blob| (blob.hash, blob.bytes)),
                    );
                }
                Ok(blobs)
            }
        }
    }

    async fn fetch_blob_availabilities(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, BlobAvailability>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct.fetch_blob_availabilities(&peer, hashes).await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                let mut availabilities = HashMap::new();
                for response in self
                    .native_fetch_blob_availability_responses_batched(&endpoint, hashes)
                    .await?
                {
                    let response_availabilities = response
                        .blob_availability
                        .into_iter()
                        .map(wire_to_availability)
                        .map(|availability| availability.map(|value| (value.hash.clone(), value)))
                        .collect::<Result<HashMap<_, _>, _>>()?;
                    availabilities.extend(response_availabilities);
                }
                Ok(availabilities)
            }
        }
    }

    async fn put_blob_chunked(
        &self,
        peer: &PeerAddress,
        bytes: Vec<u8>,
        chunk_size: usize,
    ) -> Result<BlobDescriptor, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => {
                self.direct.put_blob_chunked(&peer, bytes, chunk_size).await
            }
            ResolvedPeer::NativeIroh { endpoint } => {
                let descriptor = describe_blob(&bytes, chunk_size);
                validate_blob_descriptor(&descriptor)
                    .map_err(|error| NetError::Protocol(error.to_string()))?;
                validate_native_chunk_upload_single_frame_lengths(&descriptor, &bytes)?;
                let mut response = self
                    .native_request(&endpoint, put_blob_manifest_request(&descriptor))
                    .await?;
                response_error(response.error.take())?;
                validate_empty_ack_response(&response)?;

                let availability_response = self
                    .native_fetch_blob_availability_response_once(
                        &endpoint,
                        vec![descriptor.hash.clone()],
                    )
                    .await?;
                let availability = availability_response
                    .blob_availability
                    .into_iter()
                    .find(|availability| availability.hash == descriptor.hash)
                    .map(wire_to_availability)
                    .transpose()?;
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
                    let envelope_bytes = encode_sync_request(&put_blob_chunks_request(
                        &descriptor,
                        vec![envelope.clone()],
                    ))
                    .len()
                    .saturating_sub(chunk_frame_base_bytes);
                    if !batch.is_empty()
                        && (batch.len() >= MAX_BLOB_UPLOAD_ENVELOPES_PER_REQUEST
                            || batch_bytes.saturating_add(envelope_bytes)
                                > MAX_CHUNK_UPLOAD_BATCH_BYTES)
                    {
                        let mut chunk_response = self
                            .native_request(
                                &endpoint,
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
                    let mut chunk_response = self
                        .native_request(&endpoint, put_blob_chunks_request(&descriptor, batch))
                        .await?;
                    response_error(chunk_response.error.take())?;
                    validate_empty_ack_response(&chunk_response)?;
                }
                Ok(descriptor)
            }
        }
    }

    async fn fetch_blob_chunked(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError> {
        match self.resolve_peer(peer)? {
            ResolvedPeer::DirectTcp(peer) => self.direct.fetch_blob_chunked(&peer, hash).await,
            ResolvedPeer::NativeIroh { endpoint } => {
                let manifest_response = self
                    .native_fetch_blobs_response_once(&endpoint, vec![hash.to_owned()])
                    .await?;
                if let Some(blob) = manifest_response
                    .blobs
                    .into_iter()
                    .find(|blob| blob.hash == hash)
                {
                    return Ok(Some(blob.bytes));
                }

                let descriptor_from_list = manifest_response
                    .blob_descriptors
                    .into_iter()
                    .find(|descriptor| descriptor.hash == hash)
                    .map(wire_to_descriptor)
                    .transpose()?;
                let descriptor_from_availability = manifest_response
                    .blob_availability
                    .into_iter()
                    .find(|availability| availability.hash == hash)
                    .and_then(|availability| availability.descriptor)
                    .map(wire_to_descriptor)
                    .transpose()?;
                let descriptor = descriptor_from_list.or(descriptor_from_availability);
                let Some(descriptor) = descriptor else {
                    return Ok(None);
                };

                let chunk_hashes = descriptor
                    .chunk_hashes
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut chunks = HashMap::new();
                for response in self
                    .native_fetch_blobs_responses_batched(&endpoint, chunk_hashes)
                    .await?
                {
                    chunks.extend(
                        response
                            .blobs
                            .into_iter()
                            .map(|blob| (blob.hash, blob.bytes)),
                    );
                }
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
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use chaft_identity::DeviceIdentity;
    use chaft_media::{BLOB_CHUNK_FILE_MAX_BYTES, BlobStore};
    use chaft_net::PeerId;
    use chaft_net_direct::{DirectPeerServer, JoinRequestInbox, JoinResponseInbox, SyncPeerStore};
    use chaft_store::{EVENT_JSON_MAX_BYTES, EventStore};
    use chaft_types::{
        ChannelId, DeviceId, EventBody, MessageId, SignableEvent, SignedEvent,
        WORKSPACE_ID_MAX_BYTES, WorkspaceId,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::oneshot,
    };

    use super::*;

    #[test]
    fn shared_transports_reuse_state_per_policy_and_isolate_other_policies() {
        let default_config = IrohTransportConfig::default();
        let first = IrohTransport::shared_for_config(default_config.clone());
        let second = IrohTransport::shared_for_config(default_config);
        assert!(Arc::ptr_eq(
            &first.native_client_endpoint,
            &second.native_client_endpoint
        ));
        assert!(Arc::ptr_eq(
            &first.native_connections,
            &second.native_connections
        ));
        assert!(Arc::ptr_eq(
            &first.native_connection_gates,
            &second.native_connection_gates
        ));

        let relay_config = IrohTransportConfig {
            allow_public_relays: true,
            ..IrohTransportConfig::default()
        };
        let isolated = IrohTransport::shared_for_config(relay_config);
        assert!(!Arc::ptr_eq(
            &first.native_client_endpoint,
            &isolated.native_client_endpoint
        ));
        assert!(!Arc::ptr_eq(
            &first.native_connections,
            &isolated.native_connections
        ));
    }

    #[derive(Default)]
    struct MemoryAccessInbox {
        join_requests: StdMutex<Vec<(String, Vec<u8>)>>,
        join_responses: StdMutex<Vec<(String, Vec<u8>)>>,
    }

    impl JoinRequestInbox for MemoryAccessInbox {
        fn submit_join_request(
            &self,
            workspace_id: Option<&str>,
            request: Vec<u8>,
        ) -> Result<(), NetError> {
            self.join_requests
                .lock()
                .unwrap()
                .push((workspace_id.unwrap_or_default().to_owned(), request));
            Ok(())
        }
    }

    impl JoinResponseInbox for MemoryAccessInbox {
        fn submit_join_response(
            &self,
            workspace_id: Option<&str>,
            response: Vec<u8>,
        ) -> Result<(), NetError> {
            self.join_responses
                .lock()
                .unwrap()
                .push((workspace_id.unwrap_or_default().to_owned(), response));
            Ok(())
        }

        fn list_join_responses(
            &self,
            workspace_id: &str,
            max_entries: usize,
        ) -> Result<Vec<Vec<u8>>, NetError> {
            Ok(self
                .join_responses
                .lock()
                .unwrap()
                .iter()
                .filter(|(entry_workspace_id, _)| entry_workspace_id == workspace_id)
                .take(max_entries)
                .map(|(_, response)| response.clone())
                .collect())
        }
    }

    async fn assert_access_envelope_transport_round_trip(
        transport: &IrohTransport,
        peer: &PeerAddress,
        inbox: &MemoryAccessInbox,
    ) {
        let workspace_id = WorkspaceId("wrk_access_transport".to_owned());
        let request =
            br#"{"kind":"chaft.workspace-invite-claim.v1","requestId":"req_route_1"}"#.to_vec();
        let response = br#"{"kind":"chaft.workspace-invite-response.v1","requestId":"req_route_1","workspaceId":"wrk_access_transport"}"#
            .to_vec();

        transport
            .submit_join_request(peer, Some(&workspace_id), request.clone())
            .await
            .unwrap();
        assert_eq!(
            inbox.join_requests.lock().unwrap().as_slice(),
            [(workspace_id.0.clone(), request)].as_slice()
        );

        transport
            .submit_join_response(peer, Some(&workspace_id), response.clone())
            .await
            .unwrap();
        assert_eq!(
            inbox.join_responses.lock().unwrap().as_slice(),
            [(workspace_id.0.clone(), response.clone())].as_slice()
        );

        let request_listing_error = transport
            .fetch_join_requests(peer, &workspace_id, 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(request_listing_error.contains("remote join request listing is disabled"));

        let unscoped_response_error = transport
            .fetch_join_responses(peer, &workspace_id, 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(unscoped_response_error.contains("requires at least one request id"));

        let responses = transport
            .fetch_join_responses_for_requests(
                peer,
                &workspace_id,
                vec!["req_route_1".to_owned(), "req_route_1".to_owned()],
                1,
            )
            .await
            .unwrap();
        assert_eq!(responses, vec![response]);
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

    async fn read_native_test_request(stream: &mut IrohBiStream) -> WireSyncRequest {
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        chaft_wire::decode_sync_request(&request_bytes).unwrap()
    }

    async fn write_native_test_response(stream: &mut IrohBiStream, response: WireSyncResponse) {
        let response = chaft_wire::encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
        stream.shutdown().await.unwrap();
    }

    #[test]
    fn endpoint_classifier_normalizes_direct_tcp_forms() {
        assert_eq!(
            IrohTransport::classify_endpoint(" direct+tcp://127.0.0.1:7777 ").unwrap(),
            IrohEndpointRoute::DirectTcp {
                address: "127.0.0.1:7777".to_owned()
            }
        );
        assert_eq!(
            IrohTransport::classify_endpoint("tcp://127.0.0.1:7777").unwrap(),
            IrohEndpointRoute::DirectTcp {
                address: "127.0.0.1:7777".to_owned()
            }
        );
        assert_eq!(
            IrohTransport::classify_endpoint("127.0.0.1:7777").unwrap(),
            IrohEndpointRoute::DirectTcp {
                address: "127.0.0.1:7777".to_owned()
            }
        );
    }

    #[test]
    fn native_decode_events_rejects_oversized_typed_event() {
        let oversized = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                WorkspaceId::new(),
                Some(ChannelId::new()),
                DeviceId("dev_test".to_owned()),
                EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "x".repeat(EVENT_JSON_MAX_BYTES),
                    attachments: Vec::new(),
                },
            ),
            vec![1, 2, 3],
        );

        let error = decode_events(
            vec![chaft_wire::encode_event_envelope(&oversized)],
            Vec::new(),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("event JSON is too large"));
    }

    #[test]
    fn endpoint_classifier_rejects_empty_direct_tcp_address() {
        let error = IrohTransport::classify_endpoint("direct+tcp://").unwrap_err();

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "direct TCP endpoint must be host:port with nonzero numeric port"
        ));
    }

    #[test]
    fn endpoint_classifier_rejects_zero_port_direct_tcp_address() {
        let error = IrohTransport::classify_endpoint("direct+tcp://127.0.0.1:0").unwrap_err();

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "direct TCP endpoint must be host:port with nonzero numeric port"
        ));
    }

    #[test]
    fn endpoint_classifier_reports_unsupported_scheme() {
        let error = IrohTransport::classify_endpoint("wss://peer.example.invalid").unwrap_err();

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "unsupported peer endpoint scheme: wss"
        ));
    }

    #[test]
    fn native_iroh_endpoint_without_route_requires_discovery_policy() {
        let endpoint_id = SecretKey::generate().public();
        let endpoint = format!("iroh://{endpoint_id}");

        let error = match parse_native_endpoint(&endpoint, &IrohTransportConfig::default()) {
            Ok(_) => panic!("expected route-less native endpoint to require discovery policy"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            NetError::Unavailable(message)
                if message == "iroh public discovery endpoints are disabled"
        ));

        let discovery_config = IrohTransportConfig {
            allow_public_discovery: true,
            ..IrohTransportConfig::default()
        };
        let parsed = parse_native_endpoint(&endpoint, &discovery_config).unwrap();

        assert_eq!(parsed.endpoint, endpoint);
    }

    #[test]
    fn native_iroh_endpoint_rejects_zero_port_direct_address() {
        let endpoint_id = SecretKey::generate().public();
        let endpoint = format!("iroh://{endpoint_id}?addr=127.0.0.1:0");

        let error = match parse_native_endpoint(&endpoint, &IrohTransportConfig::default()) {
            Ok(_) => panic!("expected zero-port native endpoint to be rejected"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "invalid native iroh direct address: port must be greater than zero"
        ));
    }

    #[test]
    fn iroh_transport_config_from_env_lookup_keeps_public_network_off_by_default() {
        let config = IrohTransportConfig::from_env_lookup(|_| None);

        assert!(!config.allow_public_relays);
        assert!(!config.allow_public_discovery);
        assert!(config.allow_direct_tcp_bridge);
    }

    #[test]
    fn iroh_transport_config_from_env_lookup_honors_explicit_policy_flags() {
        let config = IrohTransportConfig::from_env_lookup(|key| match key {
            CHAFT_IROH_ALLOW_PUBLIC_RELAYS_ENV => Some("yes".to_owned()),
            CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY_ENV => Some("true".to_owned()),
            CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE_ENV => Some("1".to_owned()),
            _ => None,
        });

        assert!(config.allow_public_relays);
        assert!(config.allow_public_discovery);
        assert!(!config.allow_direct_tcp_bridge);
    }

    #[test]
    fn iroh_transport_config_from_env_lookup_ignores_oversized_policy_flags() {
        let config = IrohTransportConfig::from_env_lookup(|key| match key {
            CHAFT_IROH_ALLOW_PUBLIC_RELAYS_ENV => {
                Some(format!("{}1", " ".repeat(IROH_POLICY_ENV_FLAG_MAX_BYTES)))
            }
            CHAFT_IROH_ALLOW_PUBLIC_DISCOVERY_ENV => {
                Some("true".repeat(IROH_POLICY_ENV_FLAG_MAX_BYTES))
            }
            CHAFT_IROH_DISABLE_DIRECT_TCP_BRIDGE_ENV => {
                Some("yes".repeat(IROH_POLICY_ENV_FLAG_MAX_BYTES))
            }
            _ => None,
        });

        assert!(!config.allow_public_relays);
        assert!(!config.allow_public_discovery);
        assert!(config.allow_direct_tcp_bridge);
    }

    #[test]
    fn native_fetch_batch_inputs_deduplicate_while_preserving_order() {
        let first = EventId(format!("evt_{}", "1".repeat(64)));
        let second = EventId(format!("evt_{}", "2".repeat(64)));
        let first_blob = b"first native whole blob".to_vec();
        let second_blob = b"second native whole blob".to_vec();
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

    #[tokio::test]
    async fn native_iroh_transport_reuses_client_endpoint_across_clones() {
        let transport = IrohTransport::default();

        let first = transport.native_client_endpoint().await.unwrap().id();
        let second = transport.native_client_endpoint().await.unwrap().id();
        let clone = transport.clone();
        let cloned = clone.native_client_endpoint().await.unwrap().id();

        assert_eq!(first, second);
        assert_eq!(first, cloned);
    }

    #[tokio::test]
    async fn native_iroh_transport_reuses_connection_for_multiple_streams() {
        let server = IrohSyncPeer::bind(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-connection-reuse".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        transport.fetch_inventory(&peer).await.unwrap();
        transport.fetch_inventory(&peer).await.unwrap();

        assert_eq!(
            transport.native_connections.lock().await.connections.len(),
            1
        );
        server.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires the public Iroh relay and discovery services"]
    async fn public_discovery_endpoint_routes_between_independent_peers() {
        let config = IrohTransportConfig {
            allow_public_relays: true,
            allow_public_discovery: true,
            allow_direct_tcp_bridge: true,
        };
        let server = IrohSyncPeer::bind_with_secret_key_bytes(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            config.clone(),
            [42; 32],
        )
        .await
        .unwrap();
        let endpoint = server.endpoint_url();
        assert!(endpoint.starts_with("iroh://"));
        assert!(!endpoint.contains('?'));

        let transport = IrohTransport::new(config);
        let peer = PeerAddress {
            peer_id: PeerId("public-discovery-peer".to_owned()),
            endpoint,
        };
        transport.fetch_inventory(&peer).await.unwrap();

        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_transport_coalesces_concurrent_first_connection_to_peer() {
        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let server_task = tokio::spawn(async move {
            serve_native_endpoint_with_limits(
                accept_endpoint,
                SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
                1,
                MAX_ACTIVE_IROH_STREAMS_PER_CONNECTION,
            )
            .await
        });
        let peer = PeerAddress {
            peer_id: PeerId("native-concurrent-first-request".to_owned()),
            endpoint: peer_endpoint,
        };
        let transport = IrohTransport::default();

        let (first, second) = tokio::join!(
            transport.fetch_inventory(&peer),
            transport.fetch_inventory(&peer)
        );

        assert_eq!(first.unwrap(), Vec::<EventId>::new());
        assert_eq!(second.unwrap(), Vec::<EventId>::new());
        assert_eq!(
            transport.native_connections.lock().await.connections.len(),
            1
        );

        server_endpoint.close().await;
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn native_iroh_transport_evicts_oldest_cached_connection_at_limit() {
        let first_server = IrohSyncPeer::bind(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let second_server = IrohSyncPeer::bind(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let third_server = IrohSyncPeer::bind(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let config = IrohTransportConfig::default();
        let first = parse_native_endpoint(&first_server.endpoint_url(), &config).unwrap();
        let second = parse_native_endpoint(&second_server.endpoint_url(), &config).unwrap();
        let third = parse_native_endpoint(&third_server.endpoint_url(), &config).unwrap();
        let first_key = first.endpoint.clone();
        let second_key = second.endpoint.clone();
        let third_key = third.endpoint.clone();
        let transport = IrohTransport::default();

        transport
            .native_connection_with_cache_limit(first, 2)
            .await
            .unwrap();
        transport
            .native_connection_with_cache_limit(second, 2)
            .await
            .unwrap();
        transport
            .native_connection_with_cache_limit(third, 2)
            .await
            .unwrap();

        let cache = transport.native_connections.lock().await;
        assert_eq!(cache.connections.len(), 2);
        assert!(!cache.connections.contains_key(&first_key));
        assert!(cache.connections.contains_key(&second_key));
        assert!(cache.connections.contains_key(&third_key));
        drop(cache);
        let gates = transport.native_connection_gates.lock().await;
        assert_eq!(gates.len(), 2);
        assert!(!gates.contains_key(&first_key));
        assert!(gates.contains_key(&second_key));
        assert!(gates.contains_key(&third_key));
        drop(gates);

        first_server.close().await.unwrap();
        second_server.close().await.unwrap();
        third_server.close().await.unwrap();
    }

    #[tokio::test]
    async fn direct_tcp_bridge_carries_chaft_sync_protocol() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let workspace = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Iroh Bridge".to_owned(),
            },
        ));
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id,
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![workspace.event_id.clone()];
        let channel = identity.sign_event(channel);

        let server = DirectPeerServer::bind("127.0.0.1:0", EventStore::open_in_memory().unwrap())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("bootstrap-peer".to_owned()),
            endpoint: format!("direct+tcp://{}", server.local_addr().unwrap()),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let transport = IrohTransport::default();

        transport
            .publish_event(&peer, workspace.clone())
            .await
            .unwrap();
        transport
            .publish_event(&peer, channel.clone())
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();
        let fetched = transport
            .fetch_events(&peer, inventory.clone())
            .await
            .unwrap();

        assert_eq!(
            inventory,
            vec![workspace.event_id.clone(), channel.event_id.clone()]
        );
        assert_eq!(fetched, vec![workspace, channel]);

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn direct_tcp_bridge_carries_access_envelope_protocol() {
        let inbox = Arc::new(MemoryAccessInbox::default());
        let blob_dir = tempfile::tempdir().unwrap();
        let server = DirectPeerServer::bind_with_blobs_and_access_envelope_inboxes(
            "127.0.0.1:0",
            EventStore::open_in_memory().unwrap(),
            BlobStore::open(blob_dir.path()).unwrap(),
            inbox.clone(),
            inbox.clone(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("direct-access-envelope-peer".to_owned()),
            endpoint: format!("direct+tcp://{}", server.local_addr().unwrap()),
        };
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

        assert_access_envelope_transport_round_trip(&IrohTransport::default(), &peer, &inbox).await;

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn native_iroh_carries_access_envelope_protocol() {
        let inbox = Arc::new(MemoryAccessInbox::default());
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_access_envelope_inboxes(
                EventStore::open_in_memory().unwrap(),
                inbox.clone(),
                inbox.clone(),
            ),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-access-envelope-peer".to_owned()),
            endpoint: server.endpoint_url(),
        };

        assert_access_envelope_transport_round_trip(&IrohTransport::default(), &peer, &inbox).await;

        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_server_rejects_zero_connection_limit() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let error = serve_native_endpoint_with_connection_limit(
            endpoint,
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            0,
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "native Iroh connection limit must be greater than zero"
        ));
    }

    #[tokio::test]
    async fn native_iroh_server_rejects_zero_stream_limit() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let error = serve_native_endpoint_with_limits(
            endpoint,
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            1,
            0,
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            NetError::Protocol(message)
                if message == "native Iroh stream limit must be greater than zero"
        ));
    }

    #[tokio::test]
    async fn native_iroh_server_limits_active_streams_per_connection() {
        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let server_task = tokio::spawn(async move {
            serve_native_endpoint_with_limits(
                accept_endpoint,
                SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
                1,
                1,
            )
            .await
        });
        let parsed =
            parse_native_endpoint(&peer_endpoint, &IrohTransportConfig::default()).unwrap();
        let client_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let connection = client_endpoint
            .connect(parsed.addr, CHAFT_SYNC_ALPN)
            .await
            .unwrap();
        let (send, recv) = connection.open_bi().await.unwrap();
        let mut stalled_stream = IrohBiStream::new(send, recv);
        stalled_stream.write_all(&[0]).await.unwrap();
        stalled_stream.flush().await.unwrap();

        let (send, recv) = connection.open_bi().await.unwrap();
        let mut queued_stream = IrohBiStream::new(send, recv);
        let mut queued_request = Box::pin(request_sync_stream(
            &mut queued_stream,
            inventory_request(None, None, None),
        ));

        assert!(
            timeout(Duration::from_millis(100), &mut queued_request)
                .await
                .is_err()
        );

        stalled_stream.shutdown().await.unwrap();
        let response = timeout(Duration::from_secs(2), &mut queued_request)
            .await
            .unwrap()
            .unwrap();

        assert!(response.error.is_none());
        assert!(response.event_ids.is_empty());

        connection.close(0u8.into(), b"done");
        client_endpoint.close().await;
        server_endpoint.close().await;
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn public_relay_endpoint_is_rejected_by_default() {
        let transport = IrohTransport::default();
        let peer = PeerAddress {
            peer_id: PeerId("relay-peer".to_owned()),
            endpoint: "iroh+relay://relay.example.invalid/dev_test".to_owned(),
        };
        let error = transport.fetch_inventory(&peer).await.unwrap_err();

        assert!(matches!(error, NetError::Unavailable(message) if message.contains("disabled")));
    }

    #[tokio::test]
    async fn allowed_public_relay_still_requires_native_backend() {
        let transport = IrohTransport::new(IrohTransportConfig {
            allow_public_relays: true,
            ..IrohTransportConfig::default()
        });
        let peer = PeerAddress {
            peer_id: PeerId("relay-peer".to_owned()),
            endpoint: "iroh+relay://relay.example.invalid/dev_test".to_owned(),
        };
        let error = transport.fetch_inventory(&peer).await.unwrap_err();

        assert!(matches!(
            error,
            NetError::Unavailable("native iroh relay backend is not linked yet")
        ));
    }

    #[tokio::test]
    async fn native_iroh_endpoint_carries_chaft_sync_protocol() {
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let workspace = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Native Iroh".to_owned(),
            },
        ));
        let mut channel = SignableEvent::new(
            workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id,
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![workspace.event_id.clone()];
        let channel = identity.sign_event(channel);
        let server = IrohSyncPeer::bind(
            SyncPeerStore::new(EventStore::open_in_memory().unwrap()),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-peer".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        transport
            .publish_event(&peer, workspace.clone())
            .await
            .unwrap();
        transport
            .publish_event(&peer, channel.clone())
            .await
            .unwrap();
        let inventory = transport.fetch_inventory(&peer).await.unwrap();
        let fetched = transport
            .fetch_events(&peer, inventory.clone())
            .await
            .unwrap();

        assert_eq!(
            inventory,
            vec![workspace.event_id.clone(), channel.event_id.clone()]
        );
        assert_eq!(fetched, vec![workspace, channel]);

        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_fetch_events_deduplicates_duplicate_selectors() {
        let identity = DeviceIdentity::generate();
        let workspace = identity.sign_event(SignableEvent::new(
            WorkspaceId::new(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Native Duplicate Fetch".to_owned(),
            },
        ));
        let store = EventStore::open_in_memory().unwrap();
        store.append_event(&workspace).unwrap();
        let server = IrohSyncPeer::bind(SyncPeerStore::new(store), IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-event-dedupe".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let fetched = transport
            .fetch_events(
                &peer,
                vec![workspace.event_id.clone(), workspace.event_id.clone()],
            )
            .await
            .unwrap();

        assert_eq!(fetched, vec![workspace]);
        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_fetch_events_rejects_non_canonical_event_id_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-non-canonical-event-id".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_events(&peer, vec![EventId("evt_NOT_CANONICAL".to_owned())])
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("peer requested non-canonical event id"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_fetch_workspace_inventory_rejects_blank_workspace_id_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-blank-workspace-inventory".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_workspace_inventory(&peer, &WorkspaceId(" ".to_owned()))
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("inventory workspace ID is blank"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_fetch_workspace_inventory_rejects_oversized_workspace_id_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-oversized-workspace-inventory".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_workspace_inventory(&peer, &WorkspaceId("w".repeat(WORKSPACE_ID_MAX_BYTES + 1)))
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("workspace ID is too large"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_fetch_blobs_deduplicates_duplicate_selectors() {
        let tempdir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = b"native duplicate blob fetch".to_vec();
        let hash = blob_store.put_bytes(&bytes).unwrap().hash;
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_blobs(EventStore::open_in_memory().unwrap(), blob_store),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-blob-dedupe".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let fetched = transport
            .fetch_blobs(&peer, vec![hash.clone(), hash.clone()])
            .await
            .unwrap();

        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched.get(&hash), Some(&bytes));
        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_fetch_blobs_rejects_non_canonical_hash_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-non-canonical-blob-hash".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_blobs(&peer, vec!["A".repeat(64)])
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_fetch_blob_availability_rejects_non_canonical_hash_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-non-canonical-availability-hash".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_blob_availabilities(&peer, vec!["A".repeat(64)])
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("peer requested non-canonical blob hash"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_fetches_chunked_blob_from_manifest_and_chunks() {
        let tempdir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = b"native iroh chunked blob payload".repeat(4096);
        let descriptor = blob_store.put_bytes_chunked(&bytes, 1024).unwrap();
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_blobs(EventStore::open_in_memory().unwrap(), blob_store),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-blob-peer".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let fetched = transport
            .fetch_blob_chunked(&peer, &descriptor.hash)
            .await
            .unwrap();

        let fetched = fetched.unwrap();
        assert_eq!(fetched.len(), bytes.len());
        assert_eq!(blob_hash(&fetched), descriptor.hash);
        assert_eq!(fetched, bytes);

        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_fetches_chunked_blob_in_bounded_hash_batches() {
        let tempdir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = (0..=MAX_FETCH_BLOB_HASHES_PER_REQUEST)
            .map(|index| index as u8)
            .collect::<Vec<_>>();
        let descriptor = blob_store.put_bytes_chunked(&bytes, 1).unwrap();
        assert!(descriptor.chunk_hashes.len() > MAX_FETCH_BLOB_HASHES_PER_REQUEST);
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_blobs(EventStore::open_in_memory().unwrap(), blob_store),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-batched-chunk-fetch".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let fetched = transport
            .fetch_blob_chunked(&peer, &descriptor.hash)
            .await
            .unwrap();

        assert_eq!(fetched, Some(bytes));
        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn native_iroh_fetch_blobs_splits_after_bounded_oversized_response_error() {
        let first = b"first native bounded blob".to_vec();
        let second = b"second native bounded blob".to_vec();
        let first_hash = blob_hash(&first);
        let second_hash = blob_hash(&second);
        let blob_by_hash = HashMap::from([
            (first_hash.clone(), first.clone()),
            (second_hash.clone(), second.clone()),
        ]);
        let request_sizes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let server_request_sizes = Arc::clone(&request_sizes);
        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let (done_tx, done_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let incoming = accept_endpoint.accept().await.unwrap();
            let mut accepting = incoming.accept().unwrap();
            let alpn = accepting.alpn().await.unwrap();
            assert_eq!(alpn, CHAFT_SYNC_ALPN);
            let connection = accepting.await.unwrap();

            for _ in 0..3 {
                let (send, recv) = connection.accept_bi().await.unwrap();
                let mut stream = IrohBiStream::new(send, recv);
                let request = read_native_test_request(&mut stream).await;
                assert_eq!(request.kind, WireSyncRequestKind::FetchBlobs as i32);
                server_request_sizes
                    .lock()
                    .unwrap()
                    .push(request.blob_hashes.len());

                let response = if request.blob_hashes.len() > 1 {
                    WireSyncResponse {
                        event_ids: Vec::new(),
                        events: Vec::new(),
                        error: Some(format!(
                            "fetch-blobs response frame length {} exceeds max {}",
                            MAX_FRAME_LEN + 1,
                            MAX_FRAME_LEN
                        )),
                        blobs: Vec::new(),
                        blob_descriptors: Vec::new(),
                        blob_availability: Vec::new(),
                        event_envelopes: Vec::new(),
                        inventory_total_count: None,
                    }
                } else {
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
                    WireSyncResponse {
                        event_ids: Vec::new(),
                        events: Vec::new(),
                        error: None,
                        blobs,
                        blob_descriptors: Vec::new(),
                        blob_availability: Vec::new(),
                        event_envelopes: Vec::new(),
                        inventory_total_count: None,
                    }
                };
                write_native_test_response(&mut stream, response).await;
            }
            let _ = done_rx.await;
        });
        let peer = PeerAddress {
            peer_id: PeerId("native-split-bounded-blob-fetch".to_owned()),
            endpoint: peer_endpoint,
        };
        let transport = IrohTransport::default();

        let fetched = transport
            .fetch_blobs(&peer, vec![first_hash.clone(), second_hash.clone()])
            .await
            .unwrap();

        let _ = done_tx.send(());
        server_task.await.unwrap();
        server_endpoint.close().await;
        assert_eq!(fetched.get(&first_hash), Some(&first));
        assert_eq!(fetched.get(&second_hash), Some(&second));
        assert_eq!(*request_sizes.lock().unwrap(), vec![2, 1, 1]);
    }

    #[tokio::test]
    async fn native_iroh_rejects_chunked_fetch_when_manifest_lies_about_chunk_lengths() {
        let tempdir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(tempdir.path()).unwrap();
        let chunks = [b"abc".as_slice(), b"def".as_slice()];
        let descriptor = BlobDescriptor {
            hash: blob_hash(b"abcdef"),
            byte_len: 1000,
            chunk_size: 500,
            chunk_hashes: chunks.iter().map(|chunk| blob_hash(chunk)).collect(),
        };
        blob_store.put_manifest(&descriptor).unwrap();
        for (chunk_hash, chunk) in descriptor.chunk_hashes.iter().zip(chunks) {
            blob_store.put_chunk_with_hash(chunk_hash, chunk).unwrap();
        }
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_blobs(EventStore::open_in_memory().unwrap(), blob_store),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-lying-manifest".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let error = transport
            .fetch_blob_chunked(&peer, &descriptor.hash)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("invalid blob descriptor"));
        server.close().await.unwrap();
    }

    #[test]
    fn native_chunk_upload_preflight_rejects_oversized_chunk_frame() {
        let bytes = vec![7; MAX_FRAME_LEN];
        let descriptor = describe_blob(&bytes, MAX_FRAME_LEN);

        let error = validate_native_chunk_upload_single_frame_lengths(&descriptor, &bytes)
            .unwrap_err()
            .to_string();

        assert!(error.contains("chunk upload frame length"));
        assert!(error.contains("exceeds max"));
    }

    #[tokio::test]
    async fn native_iroh_put_blob_chunked_rejects_invalid_descriptor_before_stream() {
        let endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-invalid-chunk-descriptor-preflight".to_owned()),
            endpoint: native_endpoint_url(&endpoint),
        };
        let transport = IrohTransport::default();

        let error = transport
            .put_blob_chunked(
                &peer,
                b"small native chunk upload".to_vec(),
                BLOB_CHUNK_FILE_MAX_BYTES + 1,
            )
            .await
            .unwrap_err()
            .to_string();

        endpoint.close().await;
        assert!(error.contains("invalid blob descriptor"));
        assert!(!error.contains("open bidirectional stream"));
    }

    #[tokio::test]
    async fn native_iroh_put_blob_chunked_uploads_repeated_chunk_hash_once() {
        let bytes = b"abcabc".to_vec();
        let descriptor = describe_blob(&bytes, 3);
        assert_eq!(descriptor.chunk_hashes.len(), 2);
        assert_eq!(descriptor.chunk_hashes[0], descriptor.chunk_hashes[1]);

        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let server_descriptor = descriptor.clone();
        let (done_tx, done_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let incoming = accept_endpoint.accept().await.unwrap();
            let mut accepting = incoming.accept().unwrap();
            let alpn = accepting.alpn().await.unwrap();
            assert_eq!(alpn, CHAFT_SYNC_ALPN);
            let connection = accepting.await.unwrap();

            for request_index in 0..3 {
                let (send, recv) = connection.accept_bi().await.unwrap();
                let mut stream = IrohBiStream::new(send, recv);
                let request = read_native_test_request(&mut stream).await;

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
                write_native_test_response(&mut stream, response).await;
            }
            let _ = done_rx.await;
        });
        let peer = PeerAddress {
            peer_id: PeerId("native-dedupe-repeated-chunk-upload".to_owned()),
            endpoint: peer_endpoint,
        };
        let transport = IrohTransport::default();

        let uploaded = transport.put_blob_chunked(&peer, bytes, 3).await.unwrap();

        let _ = done_tx.send(());
        server_task.await.unwrap();
        server_endpoint.close().await;
        assert_eq!(uploaded, descriptor);
    }

    #[tokio::test]
    async fn native_iroh_put_blob_chunked_ignores_available_chunks_from_mismatched_descriptor() {
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
        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let server_descriptor = descriptor.clone();
        let server_mismatched_descriptor = mismatched_descriptor.clone();
        let (done_tx, done_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let incoming = accept_endpoint.accept().await.unwrap();
            let mut accepting = incoming.accept().unwrap();
            let alpn = accepting.alpn().await.unwrap();
            assert_eq!(alpn, CHAFT_SYNC_ALPN);
            let connection = accepting.await.unwrap();

            for request_index in 0..3 {
                let (send, recv) = connection.accept_bi().await.unwrap();
                let mut stream = IrohBiStream::new(send, recv);
                let request = read_native_test_request(&mut stream).await;

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
                write_native_test_response(&mut stream, response).await;
            }
            let _ = done_rx.await;
        });
        let peer = PeerAddress {
            peer_id: PeerId("native-mismatched-descriptor-chunk-upload".to_owned()),
            endpoint: peer_endpoint,
        };
        let transport = IrohTransport::default();

        let uploaded = transport.put_blob_chunked(&peer, bytes, 2).await.unwrap();

        let _ = done_tx.send(());
        server_task.await.unwrap();
        server_endpoint.close().await;
        assert_eq!(uploaded, descriptor);
    }

    #[tokio::test]
    async fn native_iroh_chunked_upload_uses_availability_probe_after_manifest() {
        let bytes = b"native iroh already backed up chunked blob".repeat(4096);
        let descriptor = describe_blob(&bytes, 1024);
        let server_endpoint = bind_native_endpoint(&IrohTransportConfig::default())
            .await
            .unwrap();
        let peer_endpoint = native_endpoint_url(&server_endpoint);
        let accept_endpoint = server_endpoint.clone();
        let server_descriptor = descriptor.clone();
        let (done_tx, done_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let incoming = accept_endpoint.accept().await.unwrap();
            let mut accepting = incoming.accept().unwrap();
            let alpn = accepting.alpn().await.unwrap();
            assert_eq!(alpn, CHAFT_SYNC_ALPN);
            let connection = accepting.await.unwrap();

            let (send, recv) = connection.accept_bi().await.unwrap();
            let mut stream = IrohBiStream::new(send, recv);
            let request = read_native_test_request(&mut stream).await;
            assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
            assert!(request.blobs.is_empty());
            assert_eq!(request.blob_descriptors.len(), 1);
            assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
            write_native_test_response(
                &mut stream,
                WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs: Vec::new(),
                    blob_descriptors: Vec::new(),
                    blob_availability: Vec::new(),
                    event_envelopes: Vec::new(),
                    inventory_total_count: None,
                },
            )
            .await;

            let (send, recv) = connection.accept_bi().await.unwrap();
            let mut stream = IrohBiStream::new(send, recv);
            let request = read_native_test_request(&mut stream).await;
            assert_eq!(
                request.kind,
                WireSyncRequestKind::FetchBlobAvailability as i32
            );
            assert_eq!(request.blob_hashes, vec![server_descriptor.hash.clone()]);
            write_native_test_response(
                &mut stream,
                WireSyncResponse {
                    event_ids: Vec::new(),
                    events: Vec::new(),
                    error: None,
                    blobs: Vec::new(),
                    blob_descriptors: Vec::new(),
                    blob_availability: vec![WireBlobAvailability {
                        hash: server_descriptor.hash,
                        has_whole_blob: true,
                        descriptor: None,
                        available_chunk_hashes: Vec::new(),
                        missing_chunk_hashes: Vec::new(),
                    }],
                    event_envelopes: Vec::new(),
                    inventory_total_count: None,
                },
            )
            .await;
            let _ = done_rx.await;
        });
        let peer = PeerAddress {
            peer_id: PeerId("native-upload-availability-probe".to_owned()),
            endpoint: peer_endpoint,
        };
        let transport = IrohTransport::default();

        let upload_result = transport.put_blob_chunked(&peer, bytes, 1024).await;
        let _ = done_tx.send(());
        let server_result = server_task.await;
        server_endpoint.close().await;

        server_result.unwrap();
        let uploaded = upload_result.unwrap();
        assert_eq!(uploaded, descriptor);
    }

    #[tokio::test]
    async fn native_iroh_resumes_partial_chunked_blob_upload() {
        let tempdir = tempfile::tempdir().unwrap();
        let blob_store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = b"native iroh resumable chunked blob".repeat(4096);
        let descriptor = describe_blob(&bytes, 1024);
        blob_store.put_manifest(&descriptor).unwrap();
        blob_store
            .put_chunk_with_hash(&descriptor.chunk_hashes[0], &bytes[..descriptor.chunk_size])
            .unwrap();
        let server = IrohSyncPeer::bind(
            SyncPeerStore::with_blobs(EventStore::open_in_memory().unwrap(), blob_store),
            IrohTransportConfig::default(),
        )
        .await
        .unwrap();
        let peer = PeerAddress {
            peer_id: PeerId("native-resumable-blob-peer".to_owned()),
            endpoint: server.endpoint_url(),
        };
        let transport = IrohTransport::default();

        let uploaded = transport
            .put_blob_chunked(&peer, bytes.clone(), 1024)
            .await
            .unwrap();
        let replica_store = BlobStore::open(tempdir.path()).unwrap();
        let availability = replica_store
            .availability(&descriptor.hash)
            .unwrap()
            .unwrap();

        assert_eq!(uploaded, descriptor);
        assert!(availability.is_complete());
        assert_eq!(
            replica_store.get_bytes_chunked(&descriptor.hash).unwrap(),
            Some(bytes)
        );

        server.close().await.unwrap();
    }

    #[tokio::test]
    async fn direct_tcp_bridge_can_be_disabled() {
        let transport = IrohTransport::new(IrohTransportConfig {
            allow_direct_tcp_bridge: false,
            ..IrohTransportConfig::default()
        });
        let peer = PeerAddress {
            peer_id: PeerId("direct-peer".to_owned()),
            endpoint: "direct+tcp://127.0.0.1:9".to_owned(),
        };
        let error = transport.connect(peer).await.unwrap_err();

        assert!(matches!(
            error,
            NetError::Unavailable("direct TCP bridge is disabled")
        ));
    }
}
