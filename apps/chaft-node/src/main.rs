use std::{
    collections::{BTreeSet, HashMap},
    error::Error as StdError,
    fmt, fs,
    future::Future,
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, bail};
use chaft_core::WorkspaceState;
use chaft_identity::verify_self_contained_event;
use chaft_media::BlobStore;
use chaft_net::{ChaftTransport, PeerAddress, PeerId};
#[cfg(test)]
use chaft_net_direct::DirectTransport;
use chaft_net_direct::{
    BlobSyncTransport, DirectPeerServer, JoinRequestInbox, JoinResponseInbox,
    MAX_ACTIVE_DIRECT_CONNECTIONS, SyncPeerStore,
};
use chaft_net_iroh::{IrohSyncPeer, IrohTransport, IrohTransportConfig};
use chaft_store::{EventStore, WorkspaceEventStorageHealth, WorkspaceEventStorageRepair};
use chaft_sync::pull_workspace_from_peer_with_inventory;
use chaft_types::{
    EventBody, PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES,
    PEER_ENDPOINT_TRANSPORT_MAX_BYTES, ReplicaStorageClass, SignedEvent, WorkspaceId,
    direct_tcp_peer_listen_address_is_valid, peer_endpoint_hint_is_supported,
    peer_endpoint_hint_transport_is_consistent, validate_workspace_id_str,
};
use clap::{Parser, Subcommand};
use serde_json::json;
use tokio::{
    sync::oneshot,
    time::{Duration, sleep},
};

const MAX_DISCOVERED_MIRROR_PEERS: usize = 32;
const MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS: usize = 64;
const MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS: usize = 64;
const MAX_MIRROR_STATUS_ERROR_BYTES: usize = 2048;
const MIRROR_STATUS_FILE_MAX_BYTES: usize = 1024 * 1024;
const NODE_PATH_MAX_BYTES: usize = 64 * 1024;
const STATUS_TRUNCATED_SUFFIX: &str = "...";
const ACCESS_ENVELOPE_ENTRY_MAX_BYTES: usize = 512 * 1024;
const JOIN_REQUEST_INBOX_DIR: &str = "join-request-inbox";
const JOIN_RESPONSE_INBOX_DIR: &str = "join-response-inbox";

static MIRROR_STATUS_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
static ACCESS_ENVELOPE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Parser)]
#[command(name = "chaft-node")]
#[command(about = "Headless encrypted replica node for Chaft")]
struct Args {
    #[arg(long, default_value = "./data/chaft-node")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Serve this node's encrypted event/blob store over direct TCP")]
    Serve {
        #[arg(long, default_value = "127.0.0.1:0", help = "Direct TCP bind address")]
        listen: String,

        #[arg(long, help = "Serve a single direct TCP connection, then exit")]
        once: bool,

        #[arg(long, default_value_t = MAX_ACTIVE_DIRECT_CONNECTIONS)]
        max_active_connections: usize,
    },
    #[command(about = "Serve this node's encrypted event/blob store over native Iroh QUIC")]
    ServeIroh,
    #[command(about = "Mirror one workspace from one or more peers into this encrypted replica")]
    MirrorWorkspace {
        #[arg(long)]
        workspace_id: String,

        #[arg(
            long = "peer",
            required = true,
            value_name = "ENDPOINT",
            help = "Upstream peer endpoint; repeat to merge from multiple peers"
        )]
        peers: Vec<String>,

        #[arg(long, help = "Also serve the mirrored store over direct TCP")]
        listen: Option<String>,

        #[arg(long, help = "Also serve the mirrored store over native Iroh QUIC")]
        listen_iroh: bool,

        #[arg(long, default_value_t = MAX_ACTIVE_DIRECT_CONNECTIONS)]
        max_active_connections: usize,

        #[arg(long, default_value_t = 60)]
        interval_seconds: u64,

        #[arg(
            long,
            value_name = "PATH",
            help = "Write mirror health/status JSON; defaults to <data-dir>/mirror-status.json"
        )]
        status_file: Option<PathBuf>,

        #[arg(long)]
        once: bool,

        #[arg(
            long,
            help = "Disable learning additional upstream peers from signed workspace endpoint hints"
        )]
        no_discover_peers: bool,
    },
    #[command(about = "Print this node's mirror health/status summary")]
    Status {
        #[arg(
            long,
            value_name = "PATH",
            help = "Read mirror health/status JSON; defaults to <data-dir>/mirror-status.json"
        )]
        status_file: Option<PathBuf>,

        #[arg(long, help = "Print the raw status JSON instead of a one-line summary")]
        json: bool,

        #[arg(long, help = "Exit with an error unless mirror health is healthy")]
        require_healthy: bool,

        #[arg(
            long,
            value_name = "SECONDS",
            help = "Exit with an error if checkedAtUnixMs is older than this many seconds"
        )]
        max_age_seconds: Option<u64>,
    },
    #[command(about = "Repair this node's local workspace storage metadata")]
    RepairStorageMetadata {
        #[arg(long)]
        workspace_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let data_dir = checked_node_path_arg(args.data_dir, "data directory")?;
    let store_path = checked_node_child_path(&data_dir, "events.db", "event store path")?;
    let blob_path = checked_node_child_path(&data_dir, "blobs", "blob store path")?;

    match args.command {
        Some(Command::Serve {
            listen,
            once,
            max_active_connections,
        }) => {
            let listen = normalize_listen_endpoint(listen, "listen endpoint")?;
            let max_active_connections =
                max_active_connections_arg(max_active_connections, "max active connections")?;
            let (store, blob_store) =
                open_node_store_with_blobs(&data_dir, &store_path, &blob_path)?;
            let server = DirectPeerServer::bind_with_blobs_and_access_envelope_inboxes(
                &listen,
                store,
                blob_store,
                Arc::new(NodeJoinRequestInbox::new(data_dir.clone())),
                Arc::new(NodeJoinResponseInbox::new(data_dir.clone())),
            )
            .await?;
            println!(
                "chaft-node serving {} from {}",
                server.local_addr()?,
                store_path.display()
            );

            if once {
                server.serve_one().await?;
            } else {
                let (shutdown_tx, shutdown_rx) = oneshot::channel();
                tokio::spawn(async move {
                    let _ = tokio::signal::ctrl_c().await;
                    let _ = shutdown_tx.send(());
                });
                server
                    .serve_until_shutdown_with_max_connections(shutdown_rx, max_active_connections)
                    .await?;
            }
        }
        Some(Command::ServeIroh) => {
            let (store, blob_store) =
                open_node_store_with_blobs(&data_dir, &store_path, &blob_path)?;
            let sync_store = SyncPeerStore::with_blobs(store, blob_store);
            let server =
                IrohSyncPeer::bind(sync_store, IrohTransportConfig::from_environment()).await?;
            println!(
                "chaft-node serving {} from {}",
                server.endpoint_url(),
                store_path.display()
            );
            tokio::signal::ctrl_c().await?;
            server.close().await?;
        }
        Some(Command::MirrorWorkspace {
            workspace_id,
            peers,
            listen,
            listen_iroh,
            max_active_connections,
            interval_seconds,
            status_file,
            once,
            no_discover_peers,
        }) => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let configured_peers = mirror_peer_addresses(peers)?;
            let listen = normalize_mirror_listen_options(listen, listen_iroh)?;
            let max_active_connections = max_active_connections_arg(
                max_active_connections,
                "mirror max active connections",
            )?;
            let status_file = mirror_status_file_path(&data_dir, status_file)?;
            let (store, blob_store) =
                open_node_store_with_blobs(&data_dir, &store_path, &blob_path)?;
            let hosted_mirror = start_mirror_server(
                listen,
                listen_iroh,
                store_path.as_path(),
                blob_path.as_path(),
                max_active_connections,
            )
            .await?;
            let mirror_options =
                MirrorWorkspaceRunOptions::new(interval_seconds, once, Some(status_file))
                    .with_hosted_endpoint(
                        hosted_mirror
                            .as_ref()
                            .map(HostedMirrorServer::status_endpoint),
                    )
                    .with_peer_discovery(!no_discover_peers);
            let mirror_result = if once {
                mirror_workspace_with_configured_peers(
                    store,
                    blob_store,
                    workspace_id,
                    configured_peers,
                    mirror_options,
                )
                .await
            } else {
                mirror_workspace_until_shutdown_with_configured_peers(
                    store,
                    blob_store,
                    workspace_id,
                    configured_peers,
                    mirror_options,
                    async {
                        if let Err(error) = tokio::signal::ctrl_c().await {
                            eprintln!("failed to listen for Ctrl+C: {error}");
                        }
                    },
                )
                .await
            };
            let stop_result = match hosted_mirror {
                Some(hosted_mirror) => hosted_mirror.stop().await,
                None => Ok(()),
            };
            if let Err(error) = mirror_result {
                if let Err(stop_error) = stop_result {
                    eprintln!("failed to stop hosted mirror after mirror error: {stop_error}");
                }
                return Err(error);
            }
            stop_result?;
        }
        Some(Command::Status {
            status_file,
            json: emit_json,
            require_healthy,
            max_age_seconds,
        }) => {
            let status_file = mirror_status_file_path(&data_dir, status_file)?;
            let status = read_mirror_status_file(&status_file)?;
            if emit_json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("{}", mirror_status_summary_text(&status));
            }
            if require_healthy {
                ensure_mirror_status_healthy(&status)?;
            }
            if let Some(max_age_seconds) = max_age_seconds {
                ensure_mirror_status_fresh(&status, max_age_seconds, current_unix_millis())?;
            }
        }
        Some(Command::RepairStorageMetadata { workspace_id }) => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let store = open_node_store(&data_dir, &store_path)?;
            let report = repair_storage_metadata_report(&store, &workspace_id)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        None => {
            let (_store, _blob_store) =
                open_node_store_with_blobs(&data_dir, &store_path, &blob_path)?;
            println!(
                "chaft-node initialized at {} with blobs at {}",
                store_path.display(),
                blob_path.display()
            );
        }
    }

    Ok(())
}

fn open_node_store(data_dir: &Path, store_path: &Path) -> Result<EventStore> {
    validate_node_path(data_dir, "data directory")?;
    validate_node_path(store_path, "event store path")?;
    fs::create_dir_all(data_dir)?;
    EventStore::open(store_path).map_err(Into::into)
}

fn open_node_store_with_blobs(
    data_dir: &Path,
    store_path: &Path,
    blob_path: &Path,
) -> Result<(EventStore, BlobStore)> {
    validate_node_path(data_dir, "data directory")?;
    validate_node_path(store_path, "event store path")?;
    validate_node_path(blob_path, "blob store path")?;
    fs::create_dir_all(data_dir)?;
    let store = EventStore::open(store_path)?;
    let blob_store = BlobStore::open(blob_path)?;
    Ok((store, blob_store))
}

fn checked_node_path_arg(path: PathBuf, label: &str) -> Result<PathBuf> {
    validate_node_path(&path, label)?;
    Ok(path)
}

fn checked_node_child_path(parent: &Path, child: &str, label: &str) -> Result<PathBuf> {
    let path = parent.join(child);
    validate_node_path(&path, label)?;
    Ok(path)
}

fn mirror_status_file_path(data_dir: &Path, status_file: Option<PathBuf>) -> Result<PathBuf> {
    match status_file {
        Some(path) => checked_node_path_arg(path, "mirror status file"),
        None => checked_node_child_path(data_dir, "mirror-status.json", "mirror status file"),
    }
}

fn validate_node_path(path: &Path, label: &str) -> Result<()> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.is_empty() {
        bail!("{label} cannot be empty");
    }
    if bytes.len() > NODE_PATH_MAX_BYTES {
        bail!(
            "{label} is too large ({} bytes, max {})",
            bytes.len(),
            NODE_PATH_MAX_BYTES
        );
    }
    Ok(())
}

struct HostedMirrorServer {
    endpoint: String,
    transport: &'static str,
    inner: HostedMirrorServerInner,
}

enum HostedMirrorServerInner {
    Direct {
        shutdown_tx: oneshot::Sender<()>,
        task: tokio::task::JoinHandle<Result<(), chaft_net::NetError>>,
    },
    Iroh {
        server: IrohSyncPeer,
    },
}

impl HostedMirrorServer {
    fn status_endpoint(&self) -> MirrorHostedEndpoint {
        MirrorHostedEndpoint {
            endpoint: self.endpoint.clone(),
            transport: self.transport.to_owned(),
        }
    }

    async fn stop(self) -> Result<()> {
        match self.inner {
            HostedMirrorServerInner::Direct { shutdown_tx, task } => {
                let _ = shutdown_tx.send(());
                task.await??;
            }
            HostedMirrorServerInner::Iroh { server } => {
                server.close().await?;
            }
        }
        Ok(())
    }
}

fn normalize_mirror_listen_options(
    listen: Option<String>,
    listen_iroh: bool,
) -> Result<Option<String>> {
    if listen.is_some() && listen_iroh {
        bail!("use either --listen for direct TCP or --listen-iroh for native Iroh, not both");
    }
    listen
        .map(|listen| normalize_listen_endpoint(listen, "mirror listen endpoint"))
        .transpose()
}

fn max_active_connections_arg(value: usize, label: &str) -> Result<usize> {
    if value == 0 {
        bail!("{label} must be greater than zero");
    }
    Ok(value)
}

async fn start_mirror_server(
    listen: Option<String>,
    listen_iroh: bool,
    store_path: &Path,
    blob_path: &Path,
    max_active_connections: usize,
) -> Result<Option<HostedMirrorServer>> {
    if listen.is_some() && listen_iroh {
        bail!("use either --listen for direct TCP or --listen-iroh for native Iroh, not both");
    }
    if listen_iroh {
        validate_node_path(store_path, "event store path")?;
        validate_node_path(blob_path, "blob store path")?;
        let server_store = EventStore::open(store_path)?;
        let server_blob_store = BlobStore::open(blob_path)?;
        let sync_store = SyncPeerStore::with_blobs(server_store, server_blob_store);
        let server =
            IrohSyncPeer::bind(sync_store, IrohTransportConfig::from_environment()).await?;
        let endpoint = server.endpoint_url();
        println!(
            "chaft-node mirror serving {endpoint} from {}",
            store_path.display()
        );
        return Ok(Some(HostedMirrorServer {
            endpoint,
            transport: "iroh",
            inner: HostedMirrorServerInner::Iroh { server },
        }));
    }

    let Some(listen) = listen else {
        return Ok(None);
    };
    let listen = normalize_listen_endpoint(listen, "mirror listen endpoint")?;
    let max_active_connections =
        max_active_connections_arg(max_active_connections, "mirror max active connections")?;
    validate_node_path(store_path, "event store path")?;
    validate_node_path(blob_path, "blob store path")?;

    let server_store = EventStore::open(store_path)?;
    let server_blob_store = BlobStore::open(blob_path)?;
    let server =
        DirectPeerServer::bind_with_blobs(&listen, server_store, server_blob_store).await?;
    let local_addr = server.local_addr()?.to_string();
    println!(
        "chaft-node mirror serving {local_addr} from {}",
        store_path.display()
    );
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        server
            .serve_until_shutdown_with_max_connections(shutdown_rx, max_active_connections)
            .await
    });

    Ok(Some(HostedMirrorServer {
        endpoint: local_addr,
        transport: "direct-tcp",
        inner: HostedMirrorServerInner::Direct { shutdown_tx, task },
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorHostedEndpoint {
    endpoint: String,
    transport: String,
}

#[derive(Debug, Clone)]
struct MirrorWorkspaceRunOptions {
    interval_seconds: u64,
    once: bool,
    status_file: Option<PathBuf>,
    hosted_endpoint: Option<MirrorHostedEndpoint>,
    discover_peers: bool,
}

impl MirrorWorkspaceRunOptions {
    fn new(interval_seconds: u64, once: bool, status_file: Option<PathBuf>) -> Self {
        Self {
            interval_seconds,
            once,
            status_file,
            hosted_endpoint: None,
            discover_peers: true,
        }
    }

    fn with_hosted_endpoint(mut self, hosted_endpoint: Option<MirrorHostedEndpoint>) -> Self {
        self.hosted_endpoint = hosted_endpoint;
        self
    }

    fn with_peer_discovery(mut self, discover_peers: bool) -> Self {
        self.discover_peers = discover_peers;
        self
    }
}

#[cfg(test)]
async fn mirror_workspace(
    store: EventStore,
    blob_store: BlobStore,
    workspace_id: WorkspaceId,
    peer_endpoints: Vec<String>,
    options: MirrorWorkspaceRunOptions,
) -> Result<()> {
    let configured_peers = mirror_peer_addresses(peer_endpoints)?;
    mirror_workspace_with_configured_peers(
        store,
        blob_store,
        workspace_id,
        configured_peers,
        options,
    )
    .await
}

async fn mirror_workspace_with_configured_peers(
    store: EventStore,
    blob_store: BlobStore,
    workspace_id: WorkspaceId,
    configured_peers: Vec<PeerAddress>,
    options: MirrorWorkspaceRunOptions,
) -> Result<()> {
    mirror_workspace_until_shutdown_with_configured_peers(
        store,
        blob_store,
        workspace_id,
        configured_peers,
        options,
        std::future::pending(),
    )
    .await
}

#[cfg(test)]
async fn mirror_workspace_until_shutdown<F>(
    store: EventStore,
    blob_store: BlobStore,
    workspace_id: WorkspaceId,
    peer_endpoints: Vec<String>,
    options: MirrorWorkspaceRunOptions,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()>,
{
    let configured_peers = mirror_peer_addresses(peer_endpoints)?;
    mirror_workspace_until_shutdown_with_configured_peers(
        store,
        blob_store,
        workspace_id,
        configured_peers,
        options,
        shutdown,
    )
    .await
}

async fn mirror_workspace_until_shutdown_with_configured_peers<F>(
    store: EventStore,
    blob_store: BlobStore,
    workspace_id: WorkspaceId,
    configured_peers: Vec<PeerAddress>,
    options: MirrorWorkspaceRunOptions,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()>,
{
    if configured_peers.is_empty() {
        bail!("mirror-workspace requires at least one --peer endpoint");
    }
    let transport = IrohTransport::from_environment();
    let interval = Duration::from_secs(options.interval_seconds.max(1));
    tokio::pin!(shutdown);

    loop {
        let peer_set = mirror_peer_set(
            &configured_peers,
            &store,
            &workspace_id,
            options.discover_peers,
            options.hosted_endpoint.as_ref(),
            current_unix_millis(),
        )?;
        let mirror_result = tokio::select! {
            _ = &mut shutdown => return Ok(()),
            result = mirror_workspace_from_peer_set_once(&transport, &peer_set, &store, &blob_store, &workspace_id, &options) => result,
        };
        let checked_at_unix_ms = current_unix_millis();
        let status_peer_set = mirror_peer_set(
            &configured_peers,
            &store,
            &workspace_id,
            options.discover_peers,
            options.hosted_endpoint.as_ref(),
            checked_at_unix_ms,
        )
        .unwrap_or_else(|_| peer_set.clone());
        let storage_health = mirror_status_storage_health(&store, &workspace_id);
        match mirror_result {
            Ok(attempt) => {
                write_mirror_success_status(
                    options.status_file.as_deref(),
                    &workspace_id,
                    &status_peer_set,
                    options.hosted_endpoint.as_ref(),
                    checked_at_unix_ms,
                    &storage_health,
                    &attempt,
                );
                println!(
                    "mirrored workspace={} peer={} configured_peers={} discovered_peers={} active_peers={} successful_peers={} requested={} fetched={} blobs={} missing_blobs={} ignored={} gaps={}",
                    workspace_id.0,
                    attempt.peer_endpoint,
                    status_peer_set.configured.len(),
                    status_peer_set.discovered.len(),
                    status_peer_set.active.len(),
                    attempt.successful_peer_count,
                    attempt.report.requested_event_count,
                    attempt.report.fetched_event_count,
                    attempt.report.fetched_blob_count,
                    attempt.report.missing_blob_count,
                    attempt.report.ignored_event_count,
                    attempt.report.gap_count
                );
                if options.once {
                    return Ok(());
                }
            }
            Err(error) if options.once => {
                write_mirror_failure_status(
                    options.status_file.as_deref(),
                    &workspace_id,
                    &status_peer_set,
                    options.hosted_endpoint.as_ref(),
                    checked_at_unix_ms,
                    &storage_health,
                    &error,
                );
                return Err(error.into());
            }
            Err(error) => {
                write_mirror_failure_status(
                    options.status_file.as_deref(),
                    &workspace_id,
                    &status_peer_set,
                    options.hosted_endpoint.as_ref(),
                    checked_at_unix_ms,
                    &storage_health,
                    &error,
                );
                eprintln!(
                    "mirror workspace={} failed: {error}; retrying in {}s",
                    workspace_id.0,
                    interval.as_secs()
                );
            }
        }
        tokio::select! {
            _ = &mut shutdown => return Ok(()),
            _ = sleep(interval) => {}
        }
    }
}

fn mirror_peer_addresses(peer_endpoints: Vec<String>) -> Result<Vec<PeerAddress>> {
    let peer_endpoints = deduplicate_normalized_peer_endpoints(peer_endpoints)?;
    if peer_endpoints.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        bail!(
            "mirror peer endpoint list is too large ({} endpoints, max {})",
            peer_endpoints.len(),
            PEER_ENDPOINT_LIST_MAX_ITEMS
        );
    }
    let peers = peer_endpoints
        .into_iter()
        .map(|peer_endpoint| PeerAddress {
            peer_id: PeerId(peer_endpoint.clone()),
            endpoint: peer_endpoint,
        })
        .collect::<Vec<_>>();
    if peers.is_empty() {
        bail!("mirror-workspace requires at least one --peer endpoint");
    }
    Ok(peers)
}

fn deduplicate_normalized_peer_endpoints(peer_endpoints: Vec<String>) -> Result<Vec<String>> {
    let mut endpoints = Vec::new();
    let mut seen = BTreeSet::new();
    for peer_endpoint in peer_endpoints {
        let peer_endpoint = normalize_mirror_peer_endpoint(peer_endpoint)?;
        if seen.insert(peer_endpoint.clone()) {
            endpoints.push(peer_endpoint);
        }
    }
    Ok(endpoints)
}

fn normalize_mirror_peer_endpoint(endpoint: String) -> Result<String> {
    let endpoint = normalize_peer_endpoint(endpoint, "mirror peer endpoint")?;
    if !peer_endpoint_hint_is_supported(&endpoint) {
        bail!("mirror peer endpoint must be a direct TCP or native Iroh direct route");
    }
    Ok(endpoint)
}

fn normalize_listen_endpoint(endpoint: String, label: &str) -> Result<String> {
    let endpoint = normalize_peer_endpoint(endpoint, label)?;
    if !direct_tcp_peer_listen_address_is_valid(&endpoint) {
        bail!("{label} must be host:port with numeric port");
    }
    Ok(endpoint)
}

fn normalize_peer_endpoint(endpoint: String, label: &str) -> Result<String> {
    let endpoint = endpoint.trim().to_owned();
    if endpoint.is_empty() {
        bail!("{label} cannot be empty");
    }
    if endpoint.len() > PEER_ENDPOINT_MAX_BYTES {
        bail!(
            "{label} is too large ({} bytes, max {})",
            endpoint.len(),
            PEER_ENDPOINT_MAX_BYTES
        );
    }
    Ok(endpoint)
}

fn workspace_id_arg(value: String) -> Result<WorkspaceId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("workspace ID cannot be empty");
    }
    validate_workspace_id_str(&value).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(WorkspaceId(value))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorPeerSet {
    configured: Vec<PeerAddress>,
    discovered: Vec<PeerAddress>,
    active: Vec<PeerAddress>,
}

fn mirror_peer_set(
    configured_peers: &[PeerAddress],
    store: &EventStore,
    workspace_id: &WorkspaceId,
    discover_peers: bool,
    hosted_endpoint: Option<&MirrorHostedEndpoint>,
    now_unix_ms: u64,
) -> Result<MirrorPeerSet> {
    let mut active = Vec::new();
    let mut discovered = Vec::new();
    let mut seen = BTreeSet::new();
    let hosted_endpoint = hosted_endpoint.map(|endpoint| endpoint.endpoint.as_str());

    for peer in configured_peers {
        if hosted_endpoint == Some(peer.endpoint.as_str()) {
            continue;
        }
        if seen.insert(peer.endpoint.clone()) {
            active.push(peer.clone());
        }
    }

    if discover_peers {
        for peer in discovered_peer_addresses(store, workspace_id, now_unix_ms)? {
            if hosted_endpoint == Some(peer.endpoint.as_str()) {
                continue;
            }
            if seen.insert(peer.endpoint.clone()) {
                active.push(peer.clone());
                discovered.push(peer);
            }
        }
    }

    if active.is_empty() {
        bail!("mirror-workspace has no usable peer endpoints");
    }

    Ok(MirrorPeerSet {
        configured: configured_peers.to_vec(),
        discovered,
        active,
    })
}

fn discovered_peer_addresses(
    store: &EventStore,
    workspace_id: &WorkspaceId,
    now_unix_ms: u64,
) -> Result<Vec<PeerAddress>> {
    let events = verified_node_events(store.list_parseable_events_for_workspace(&workspace_id.0)?)
        .into_iter()
        .filter(peer_endpoint_hint_metadata_is_bounded)
        .collect::<Vec<_>>();
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let mut state = WorkspaceState::new(workspace_id.clone());
    state.apply_batch(&events)?;
    let mut endpoints = state
        .peer_endpoints
        .values()
        .filter(|endpoint| {
            endpoint
                .expires_at_ms
                .is_none_or(|expires_at_ms| expires_at_ms > now_unix_ms as i64)
        })
        .filter(|endpoint| !endpoint.endpoint.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    endpoints.sort_by(|left, right| {
        discovered_peer_rank(left.is_backup_peer, left.replica_storage_class)
            .cmp(&discovered_peer_rank(
                right.is_backup_peer,
                right.replica_storage_class,
            ))
            .then_with(|| right.physical_ms.cmp(&left.physical_ms))
            .then_with(|| left.endpoint.cmp(&right.endpoint))
    });

    let mut peers = Vec::new();
    let mut seen = BTreeSet::new();
    for endpoint in endpoints {
        let peer_endpoint = endpoint.endpoint.trim();
        if peer_endpoint.is_empty() || peer_endpoint.len() > PEER_ENDPOINT_MAX_BYTES {
            continue;
        }
        if !peer_endpoint_hint_is_supported(peer_endpoint) {
            continue;
        }
        let peer_endpoint = peer_endpoint.to_owned();
        if seen.insert(peer_endpoint.clone()) {
            peers.push(PeerAddress {
                peer_id: PeerId(peer_endpoint.clone()),
                endpoint: peer_endpoint,
            });
            if peers.len() >= MAX_DISCOVERED_MIRROR_PEERS {
                break;
            }
        }
    }
    Ok(peers)
}

fn discovered_peer_rank(
    is_backup_peer: bool,
    replica_storage_class: Option<ReplicaStorageClass>,
) -> u8 {
    if !is_backup_peer {
        return 6;
    }
    match replica_storage_class {
        Some(ReplicaStorageClass::FullHistoryWithBlobs) => 0,
        Some(ReplicaStorageClass::FullHistory) => 1,
        None => 2,
        Some(ReplicaStorageClass::PartialHistory) => 3,
        Some(ReplicaStorageClass::MetadataIndex) => 4,
        Some(ReplicaStorageClass::EphemeralPeer) => 5,
    }
}

fn peer_endpoint_hint_metadata_is_bounded(event: &SignedEvent) -> bool {
    match &event.event.body {
        EventBody::PeerEndpointPublished {
            endpoint_id,
            endpoint,
            transport,
            ..
        } => {
            !endpoint_id.trim().is_empty()
                && !endpoint.trim().is_empty()
                && !transport.trim().is_empty()
                && endpoint_id.len() <= PEER_ENDPOINT_ID_MAX_BYTES
                && endpoint.len() <= PEER_ENDPOINT_MAX_BYTES
                && transport.len() <= PEER_ENDPOINT_TRANSPORT_MAX_BYTES
                && peer_endpoint_hint_is_supported(endpoint)
                && peer_endpoint_hint_transport_is_consistent(endpoint, transport)
        }
        _ => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorWorkspaceAttemptReport {
    peer_endpoint: String,
    report: MirrorWorkspaceReport,
    peer_failures: Vec<MirrorPeerFailure>,
    successful_peer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorPeerFailure {
    peer_endpoint: String,
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorWorkspaceAttemptError {
    peer_failures: Vec<MirrorPeerFailure>,
}

impl fmt::Display for MirrorWorkspaceAttemptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "all mirror peers failed: ")?;
        for (index, failure) in self.peer_failures.iter().enumerate() {
            if index > 0 {
                write!(formatter, "; ")?;
            }
            write!(formatter, "{}: {}", failure.peer_endpoint, failure.error)?;
        }
        Ok(())
    }
}

impl StdError for MirrorWorkspaceAttemptError {}

fn current_unix_millis() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    elapsed.as_millis().min(u128::from(u64::MAX)) as u64
}

#[derive(Debug, Clone)]
struct NodeJoinRequestInbox {
    data_dir: PathBuf,
}

impl NodeJoinRequestInbox {
    fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl JoinRequestInbox for NodeJoinRequestInbox {
    fn submit_join_request(
        &self,
        workspace_id: Option<&str>,
        request: Vec<u8>,
    ) -> Result<(), chaft_net::NetError> {
        let request_text = String::from_utf8(request).map_err(|_| {
            chaft_net::NetError::Protocol("join request payload must be UTF-8 JSON".to_owned())
        })?;
        write_access_envelope_entry(
            &self.data_dir,
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            workspace_id,
            &request_text,
        )
        .map(|_| ())
        .map_err(|error| chaft_net::NetError::Protocol(error.to_string()))
    }

    fn list_join_requests(
        &self,
        workspace_id: &str,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, chaft_net::NetError> {
        list_access_envelope_entries(
            &self.data_dir,
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            workspace_id,
            max_entries,
        )
        .map_err(|error| chaft_net::NetError::Protocol(error.to_string()))
    }
}

#[derive(Debug, Clone)]
struct NodeJoinResponseInbox {
    data_dir: PathBuf,
}

impl NodeJoinResponseInbox {
    fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
}

impl JoinResponseInbox for NodeJoinResponseInbox {
    fn submit_join_response(
        &self,
        workspace_id: Option<&str>,
        response: Vec<u8>,
    ) -> Result<(), chaft_net::NetError> {
        let response_text = String::from_utf8(response).map_err(|_| {
            chaft_net::NetError::Protocol("join response payload must be UTF-8 JSON".to_owned())
        })?;
        write_access_envelope_entry(
            &self.data_dir,
            JOIN_RESPONSE_INBOX_DIR,
            "responseText",
            workspace_id,
            &response_text,
        )
        .map(|_| ())
        .map_err(|error| chaft_net::NetError::Protocol(error.to_string()))
    }

    fn list_join_responses(
        &self,
        workspace_id: &str,
        max_entries: usize,
    ) -> Result<Vec<Vec<u8>>, chaft_net::NetError> {
        list_access_envelope_entries(
            &self.data_dir,
            JOIN_RESPONSE_INBOX_DIR,
            "responseText",
            workspace_id,
            max_entries,
        )
        .map_err(|error| chaft_net::NetError::Protocol(error.to_string()))
    }
}

fn write_access_envelope_entry(
    data_dir: &Path,
    inbox_dir_name: &str,
    text_key: &str,
    workspace_id: Option<&str>,
    envelope_text: &str,
) -> Result<()> {
    if envelope_text.trim().is_empty() {
        bail!("access envelope payload is empty");
    }
    let parsed: serde_json::Value = serde_json::from_str(envelope_text)?;
    let workspace_id = access_envelope_workspace_id(workspace_id, &parsed)?;
    let inbox_dir = data_dir.join(inbox_dir_name);
    fs::create_dir_all(&inbox_dir)?;
    let entry_id = access_envelope_entry_id(&workspace_id, &parsed).unwrap_or_else(|| {
        format!(
            "access_{}_{}",
            current_unix_millis(),
            ACCESS_ENVELOPE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    });
    let final_path = inbox_dir.join(format!("{entry_id}.json"));
    if final_path.exists() {
        return Ok(());
    }
    let entry = json!({
        "schemaVersion": 1,
        "entryId": entry_id,
        "workspaceId": workspace_id,
        "receivedAtUnixMs": current_unix_millis(),
        text_key: envelope_text,
    });
    let bytes = serde_json::to_vec_pretty(&entry)?;
    if bytes.len() > ACCESS_ENVELOPE_ENTRY_MAX_BYTES {
        bail!(
            "access envelope entry is too large ({} bytes, max {})",
            bytes.len(),
            ACCESS_ENVELOPE_ENTRY_MAX_BYTES
        );
    }
    let temp_path = inbox_dir.join(format!(".{entry_id}.tmp"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    let result = (|| -> Result<()> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, &final_path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn access_envelope_entry_id(workspace_id: &str, parsed: &serde_json::Value) -> Option<String> {
    let request_id = parsed.get("requestId")?.as_str()?.trim();
    if request_id.is_empty() || request_id.len() > 128 {
        return None;
    }
    if !request_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return None;
    }
    let hash = blake3::hash(format!("{workspace_id}\0{request_id}").as_bytes());
    let hash_hex = hash.to_hex();
    Some(format!("access_{}_{}", &hash_hex[..16], request_id))
}

fn access_envelope_workspace_id(
    explicit_workspace_id: Option<&str>,
    parsed: &serde_json::Value,
) -> Result<String> {
    let explicit_workspace_id = explicit_workspace_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let payload_workspace_id = parsed
        .get("workspaceId")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if let (Some(explicit), Some(payload)) = (&explicit_workspace_id, &payload_workspace_id)
        && explicit != payload
    {
        bail!(
            "access envelope workspace ID mismatch: explicit {explicit} does not match payload {payload}"
        );
    }
    let workspace_id = explicit_workspace_id
        .or(payload_workspace_id)
        .ok_or_else(|| anyhow::anyhow!("access envelope workspace ID is required"))?;
    validate_workspace_id_str(&workspace_id).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(workspace_id)
}

fn list_access_envelope_entries(
    data_dir: &Path,
    inbox_dir_name: &str,
    text_key: &str,
    workspace_id: &str,
    max_entries: usize,
) -> Result<Vec<Vec<u8>>> {
    validate_workspace_id_str(workspace_id).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let inbox_dir = data_dir.join(inbox_dir_name);
    let mut paths = match fs::read_dir(&inbox_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    paths.sort();

    let mut envelopes = Vec::new();
    for path in paths {
        if envelopes.len() >= max_entries {
            break;
        }
        let metadata = fs::metadata(&path)?;
        if metadata.len() > ACCESS_ENVELOPE_ENTRY_MAX_BYTES as u64 {
            bail!("access envelope entry is too large: {}", path.display());
        }
        let text = fs::read_to_string(&path)?;
        let entry: serde_json::Value = serde_json::from_str(&text)?;
        if entry.get("workspaceId").and_then(serde_json::Value::as_str) != Some(workspace_id) {
            continue;
        }
        let Some(envelope) = entry.get(text_key).and_then(serde_json::Value::as_str) else {
            continue;
        };
        envelopes.push(envelope.as_bytes().to_vec());
    }
    Ok(envelopes)
}

fn write_mirror_success_status(
    status_file: Option<&Path>,
    workspace_id: &WorkspaceId,
    peer_set: &MirrorPeerSet,
    hosted_endpoint: Option<&MirrorHostedEndpoint>,
    checked_at_unix_ms: u64,
    storage_health: &serde_json::Value,
    attempt: &MirrorWorkspaceAttemptReport,
) {
    let report = &attempt.report;
    let partial = report.missing_blob_count > 0
        || report.gap_count > 0
        || mirror_storage_health_has_issue(storage_health);
    let status = json!({
        "schemaVersion": 1,
        "workspaceId": workspace_id.0,
        "configuredPeers": mirror_status_peer_endpoints(&peer_set.configured),
        "discoveredPeers": mirror_status_peer_endpoints(&peer_set.discovered),
        "activePeers": mirror_status_peer_endpoints(&peer_set.active),
        "hostedEndpoint": mirror_status_hosted_endpoint(hosted_endpoint),
        "checkedAtUnixMs": checked_at_unix_ms,
        "lastResult": "success",
        "health": if partial { "partial" } else { "healthy" },
        "partial": partial,
        "lastSuccessfulPeer": attempt.peer_endpoint,
        "lastError": null,
        "peerFailures": mirror_status_peer_failures(&attempt.peer_failures),
        "storageHealth": storage_health,
        "lastReport": {
            "successfulPeerCount": attempt.successful_peer_count,
            "requestedEventCount": report.requested_event_count,
            "fetchedEventCount": report.fetched_event_count,
            "fetchedBlobCount": report.fetched_blob_count,
            "missingBlobCount": report.missing_blob_count,
            "missingBlobHashes": mirror_status_missing_blob_hashes(&report.missing_blob_hashes),
            "ignoredEventCount": report.ignored_event_count,
            "gapCount": report.gap_count,
            "gaps": mirror_status_gaps(&report.gaps),
        },
    });
    write_mirror_status_file(status_file, &status);
}

fn write_mirror_failure_status(
    status_file: Option<&Path>,
    workspace_id: &WorkspaceId,
    peer_set: &MirrorPeerSet,
    hosted_endpoint: Option<&MirrorHostedEndpoint>,
    checked_at_unix_ms: u64,
    storage_health: &serde_json::Value,
    error: &MirrorWorkspaceAttemptError,
) {
    let status = json!({
        "schemaVersion": 1,
        "workspaceId": workspace_id.0,
        "configuredPeers": mirror_status_peer_endpoints(&peer_set.configured),
        "discoveredPeers": mirror_status_peer_endpoints(&peer_set.discovered),
        "activePeers": mirror_status_peer_endpoints(&peer_set.active),
        "hostedEndpoint": mirror_status_hosted_endpoint(hosted_endpoint),
        "checkedAtUnixMs": checked_at_unix_ms,
        "lastResult": "failed",
        "health": "unreachable",
        "partial": true,
        "lastSuccessfulPeer": null,
        "lastError": mirror_status_attempt_error(error),
        "peerFailures": mirror_status_peer_failures(&error.peer_failures),
        "storageHealth": storage_health,
        "lastReport": null,
    });
    write_mirror_status_file(status_file, &status);
}

fn mirror_status_storage_health(
    store: &EventStore,
    workspace_id: &WorkspaceId,
) -> serde_json::Value {
    match store.workspace_event_storage_health(&workspace_id.0) {
        Ok(health) => mirror_status_storage_health_value(&health),
        Err(error) => json!({
            "workspaceId": workspace_id.0,
            "error": error.to_string(),
        }),
    }
}

fn mirror_status_storage_health_value(health: &WorkspaceEventStorageHealth) -> serde_json::Value {
    json!({
        "workspaceId": health.workspace_id.as_str(),
        "totalEventCount": health.total_event_count,
        "parseableEventCount": health.parseable_event_count,
        "corruptEventCount": health.corrupt_event_count,
        "signatureValidMetadataCount": health.signature_valid_metadata_count,
        "servableEventCount": health.servable_event_count,
        "poisonedServableMetadataCount": health.poisoned_servable_metadata_count,
        "promotableServableMetadataCount": health.promotable_servable_metadata_count,
        "nonServableParseableEventCount": health.non_servable_parseable_event_count,
    })
}

fn repair_storage_metadata_report(
    store: &EventStore,
    workspace_id: &WorkspaceId,
) -> Result<serde_json::Value> {
    let repair = store.repair_workspace_event_storage_metadata(&workspace_id.0)?;
    let health = store.workspace_event_storage_health(&workspace_id.0)?;
    Ok(json!({
        "workspaceId": workspace_id.0.as_str(),
        "repair": mirror_status_storage_repair_value(&repair),
        "storageHealth": mirror_status_storage_health_value(&health),
    }))
}

fn mirror_status_storage_repair_value(repair: &WorkspaceEventStorageRepair) -> serde_json::Value {
    json!({
        "workspaceId": repair.workspace_id.as_str(),
        "totalEventCount": repair.total_event_count,
        "parseableEventCount": repair.parseable_event_count,
        "corruptEventCount": repair.corrupt_event_count,
        "signatureValidMetadataBeforeCount": repair.signature_valid_metadata_before_count,
        "signatureValidMetadataAfterCount": repair.signature_valid_metadata_after_count,
        "repairedMetadataCount": repair.repaired_metadata_count,
        "promotedServableMetadataCount": repair.promoted_servable_metadata_count,
        "clearedUnservableMetadataCount": repair.cleared_unservable_metadata_count,
    })
}

fn mirror_storage_health_has_issue(storage_health: &serde_json::Value) -> bool {
    storage_health
        .get("error")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|error| !error.is_empty())
        || status_u64(storage_health, "corruptEventCount").unwrap_or(0) > 0
        || status_u64(storage_health, "poisonedServableMetadataCount").unwrap_or(0) > 0
        || status_u64(storage_health, "promotableServableMetadataCount").unwrap_or(0) > 0
        || status_u64(storage_health, "nonServableParseableEventCount").unwrap_or(0) > 0
}

fn mirror_status_peer_endpoints(peers: &[PeerAddress]) -> Vec<String> {
    peers.iter().map(|peer| peer.endpoint.clone()).collect()
}

fn mirror_status_hosted_endpoint(
    hosted_endpoint: Option<&MirrorHostedEndpoint>,
) -> serde_json::Value {
    match hosted_endpoint {
        Some(hosted_endpoint) => json!({
            "endpoint": hosted_endpoint.endpoint,
            "transport": hosted_endpoint.transport,
        }),
        None => serde_json::Value::Null,
    }
}

fn mirror_status_peer_failures(peer_failures: &[MirrorPeerFailure]) -> Vec<serde_json::Value> {
    peer_failures
        .iter()
        .map(|failure| {
            json!({
                "peerEndpoint": failure.peer_endpoint,
                "error": bounded_mirror_status_text(&failure.error),
            })
        })
        .collect()
}

fn mirror_status_attempt_error(error: &MirrorWorkspaceAttemptError) -> String {
    let mut message = String::from("all mirror peers failed");
    for (index, failure) in error.peer_failures.iter().enumerate() {
        let separator = if index == 0 { ": " } else { "; " };
        let next = format!("{separator}{}: {}", failure.peer_endpoint, failure.error);
        if message.len().saturating_add(next.len()) > MAX_MIRROR_STATUS_ERROR_BYTES {
            message.push_str(STATUS_TRUNCATED_SUFFIX);
            return bounded_mirror_status_text(&message);
        }
        message.push_str(&next);
    }
    bounded_mirror_status_text(&message)
}

fn bounded_mirror_status_text(value: &str) -> String {
    bounded_utf8_text(value, MAX_MIRROR_STATUS_ERROR_BYTES)
}

fn bounded_utf8_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    if max_bytes <= STATUS_TRUNCATED_SUFFIX.len() {
        return STATUS_TRUNCATED_SUFFIX[..max_bytes].to_owned();
    }

    let mut end = max_bytes - STATUS_TRUNCATED_SUFFIX.len();
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = value[..end].to_owned();
    bounded.push_str(STATUS_TRUNCATED_SUFFIX);
    bounded
}

fn mirror_status_missing_blob_hashes(missing_blob_hashes: &[String]) -> Vec<String> {
    missing_blob_hashes
        .iter()
        .take(MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS)
        .cloned()
        .collect()
}

fn mirror_status_gaps(gaps: &[MirrorWorkspaceGap]) -> Vec<serde_json::Value> {
    gaps.iter()
        .take(MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS)
        .map(|gap| {
            json!({
                "eventId": gap.event_id,
                "missingParentIds": gap.missing_parent_ids,
            })
        })
        .collect()
}

fn write_mirror_status_file(status_file: Option<&Path>, status: &serde_json::Value) {
    let Some(status_file) = status_file else {
        return;
    };
    if let Err(error) = write_mirror_status_file_result(status_file, status) {
        eprintln!(
            "failed to write mirror status {}: {error}",
            status_file.display()
        );
    }
}

fn write_mirror_status_file_result(status_file: &Path, status: &serde_json::Value) -> Result<()> {
    validate_node_path(status_file, "mirror status file")?;
    let parent = status_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }

    remove_legacy_mirror_status_temp_file(status_file)?;
    let (temp_file, mut file) = create_unique_mirror_status_temp_file(status_file)?;
    let result = (|| -> Result<()> {
        let status_bytes = serde_json::to_vec_pretty(status)?;
        if status_bytes.len() > MIRROR_STATUS_FILE_MAX_BYTES {
            bail!(
                "mirror status JSON is too large ({} bytes, max {})",
                status_bytes.len(),
                MIRROR_STATUS_FILE_MAX_BYTES
            );
        }
        file.write_all(&status_bytes)?;
        file.sync_all()?;
        drop(file);
        replace_mirror_status_file(&temp_file, status_file)?;
        if let Some(parent) = parent {
            sync_mirror_status_parent_directory(parent)?;
        }
        Ok(())
    })();

    if result.is_err() {
        match fs::remove_file(&temp_file) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
    result
}

fn create_unique_mirror_status_temp_file(status_file: &Path) -> Result<(PathBuf, fs::File)> {
    let file_name = status_file
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("mirror status path has no file name"))?;

    for _ in 0..32 {
        let counter = MIRROR_STATUS_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_file =
            status_file.with_file_name(format!(".{file_name}.tmp.{}.{}", process::id(), counter));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_file)
        {
            Ok(file) => return Ok((temp_file, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(anyhow::anyhow!(
        "could not create unique mirror status temp file"
    ))
}

fn legacy_mirror_status_temp_file(status_file: &Path) -> Option<PathBuf> {
    let mut temp_file_name = status_file.file_name()?.to_os_string();
    temp_file_name.push(".tmp");
    Some(status_file.with_file_name(temp_file_name))
}

fn remove_legacy_mirror_status_temp_file(status_file: &Path) -> Result<()> {
    let Some(temp_file) = legacy_mirror_status_temp_file(status_file) else {
        return Ok(());
    };
    match fs::remove_file(temp_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn sync_mirror_status_parent_directory(parent: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
    }
    Ok(())
}

fn replace_mirror_status_file(temp_file: &Path, status_file: &Path) -> Result<()> {
    for _ in 0..32 {
        match fs::rename(temp_file, status_file) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                match fs::remove_file(status_file) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => {
                        let _ = fs::remove_file(temp_file);
                        return Err(error.into());
                    }
                }
            }
            Err(error) => {
                let _ = fs::remove_file(temp_file);
                return Err(error.into());
            }
        }
    }

    let _ = fs::remove_file(temp_file);
    Err(anyhow::anyhow!("could not replace mirror status file"))
}

fn read_mirror_status_file(status_file: &Path) -> Result<serde_json::Value> {
    let bytes = read_mirror_status_file_bytes(status_file)?;
    serde_json::from_slice(&bytes).map_err(|error| {
        anyhow::anyhow!(
            "failed to parse mirror status {}: {error}",
            status_file.display()
        )
    })
}

fn read_mirror_status_file_bytes(status_file: &Path) -> Result<Vec<u8>> {
    validate_node_path(status_file, "mirror status file")?;
    let metadata = fs::metadata(status_file).map_err(|error| {
        anyhow::anyhow!(
            "failed to stat mirror status {}: {error}",
            status_file.display()
        )
    })?;
    if metadata.len() > MIRROR_STATUS_FILE_MAX_BYTES as u64 {
        bail!(
            "mirror status file {} is too large ({} bytes, max {})",
            status_file.display(),
            metadata.len(),
            MIRROR_STATUS_FILE_MAX_BYTES
        );
    }

    let file = fs::File::open(status_file).map_err(|error| {
        anyhow::anyhow!(
            "failed to read mirror status {}: {error}",
            status_file.display()
        )
    })?;
    let mut limited_file = file.take(MIRROR_STATUS_FILE_MAX_BYTES as u64 + 1);
    let mut bytes =
        Vec::with_capacity(metadata.len().min(MIRROR_STATUS_FILE_MAX_BYTES as u64) as usize);
    limited_file.read_to_end(&mut bytes).map_err(|error| {
        anyhow::anyhow!(
            "failed to read mirror status {}: {error}",
            status_file.display()
        )
    })?;
    if bytes.len() > MIRROR_STATUS_FILE_MAX_BYTES {
        bail!(
            "mirror status file {} is too large ({} bytes, max {})",
            status_file.display(),
            bytes.len(),
            MIRROR_STATUS_FILE_MAX_BYTES
        );
    }
    Ok(bytes)
}

fn mirror_status_summary_text(status: &serde_json::Value) -> String {
    mirror_status_summary_text_at(status, current_unix_millis())
}

fn mirror_status_summary_text_at(status: &serde_json::Value, now_unix_ms: u64) -> String {
    let workspace_id = status_string(status, "workspaceId").unwrap_or("unknown");
    let health = mirror_status_health(status);
    let last_result = status_string(status, "lastResult").unwrap_or("unknown");
    let checked_at_ms = status_u64(status, "checkedAtUnixMs");
    let checked_at = checked_at_ms
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let age_ms = checked_at_ms
        .map(|checked_at| now_unix_ms.saturating_sub(checked_at).to_string())
        .unwrap_or_else(|| "unknown".to_owned());
    let configured_peers = status_array_len(status, "configuredPeers");
    let discovered_peers = status_array_len(status, "discoveredPeers");
    let active_peers = status_array_len(status, "activePeers");
    let peer_failures = status_array_len(status, "peerFailures");
    let hosted_endpoint = status
        .get("hostedEndpoint")
        .and_then(|value| value.get("endpoint"))
        .and_then(serde_json::Value::as_str)
        .filter(|endpoint| !endpoint.is_empty())
        .unwrap_or("none");
    let hosted_transport = status
        .get("hostedEndpoint")
        .and_then(|value| value.get("transport"))
        .and_then(serde_json::Value::as_str)
        .filter(|transport| !transport.is_empty())
        .unwrap_or("none");
    let last_successful_peer = status_string(status, "lastSuccessfulPeer").unwrap_or("none");
    let last_report = status.get("lastReport").unwrap_or(&serde_json::Value::Null);
    let successful_peer_count = status_u64(last_report, "successfulPeerCount").unwrap_or(0);
    let requested_event_count = status_u64(last_report, "requestedEventCount").unwrap_or(0);
    let fetched_event_count = status_u64(last_report, "fetchedEventCount").unwrap_or(0);
    let fetched_blob_count = status_u64(last_report, "fetchedBlobCount").unwrap_or(0);
    let missing_blob_count = status_u64(last_report, "missingBlobCount").unwrap_or(0);
    let gap_count = status_u64(last_report, "gapCount").unwrap_or(0);
    let storage_health = status
        .get("storageHealth")
        .unwrap_or(&serde_json::Value::Null);
    let storage_total_count = status_u64(storage_health, "totalEventCount").unwrap_or(0);
    let storage_corrupt_count = status_u64(storage_health, "corruptEventCount").unwrap_or(0);
    let storage_poisoned_count =
        status_u64(storage_health, "poisonedServableMetadataCount").unwrap_or(0);
    let storage_promotable_count =
        status_u64(storage_health, "promotableServableMetadataCount").unwrap_or(0);
    let storage_non_servable_count =
        status_u64(storage_health, "nonServableParseableEventCount").unwrap_or(0);

    format!(
        "workspace={workspace_id} health={health} result={last_result} checkedAtUnixMs={checked_at} ageMs={age_ms} hostedEndpoint={hosted_endpoint} hostedTransport={hosted_transport} lastSuccessfulPeer={last_successful_peer} configuredPeers={configured_peers} discoveredPeers={discovered_peers} activePeers={active_peers} successfulPeers={successful_peer_count} requested={requested_event_count} fetched={fetched_event_count} blobs={fetched_blob_count} missingBlobs={missing_blob_count} gaps={gap_count} failures={peer_failures} storageRows={storage_total_count} storageCorrupt={storage_corrupt_count} storagePoisoned={storage_poisoned_count} storagePromotable={storage_promotable_count} storageNonServable={storage_non_servable_count}"
    )
}

fn ensure_mirror_status_healthy(status: &serde_json::Value) -> Result<()> {
    let health = mirror_status_health(status);
    if health == "healthy" {
        return Ok(());
    }
    bail!("mirror health is {health}");
}

fn ensure_mirror_status_fresh(
    status: &serde_json::Value,
    max_age_seconds: u64,
    now_unix_ms: u64,
) -> Result<()> {
    let checked_at_ms = status_u64(status, "checkedAtUnixMs")
        .ok_or_else(|| anyhow::anyhow!("mirror status is missing checkedAtUnixMs"))?;
    let age_ms = now_unix_ms.saturating_sub(checked_at_ms);
    let max_age_ms = max_age_seconds.saturating_mul(1_000);
    if age_ms <= max_age_ms {
        return Ok(());
    }
    bail!("mirror status is stale: ageMs={age_ms}, maxAgeMs={max_age_ms}");
}

fn mirror_status_health(status: &serde_json::Value) -> &str {
    status_string(status, "health").unwrap_or("unknown")
}

fn status_string<'a>(status: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    status
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
}

fn status_u64(status: &serde_json::Value, field: &str) -> Option<u64> {
    status.get(field).and_then(serde_json::Value::as_u64)
}

fn status_array_len(status: &serde_json::Value, field: &str) -> usize {
    status
        .get(field)
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

async fn mirror_workspace_from_peers_once(
    transport: &IrohTransport,
    peers: &[PeerAddress],
    store: &EventStore,
    blob_store: &BlobStore,
    workspace_id: &WorkspaceId,
) -> std::result::Result<MirrorWorkspaceAttemptReport, MirrorWorkspaceAttemptError> {
    let mut peer_failures = Vec::new();
    let mut aggregate_report: Option<MirrorWorkspaceReport> = None;
    let mut last_successful_peer = None;
    let mut successful_peer_count = 0;
    for peer in peers {
        match mirror_workspace_once(transport, peer, store, blob_store, workspace_id).await {
            Ok(report) => {
                successful_peer_count += 1;
                last_successful_peer = Some(peer.endpoint.clone());
                match &mut aggregate_report {
                    Some(aggregate_report) => aggregate_report.merge_peer_pass(report),
                    None => aggregate_report = Some(report),
                }
            }
            Err(error) => peer_failures.push(MirrorPeerFailure {
                peer_endpoint: peer.endpoint.clone(),
                error: bounded_mirror_status_text(&error.to_string()),
            }),
        }
    }

    match (last_successful_peer, aggregate_report) {
        (Some(peer_endpoint), Some(report)) => Ok(MirrorWorkspaceAttemptReport {
            peer_endpoint,
            report,
            peer_failures,
            successful_peer_count,
        }),
        _ => Err(MirrorWorkspaceAttemptError { peer_failures }),
    }
}

async fn mirror_workspace_from_peer_set_once(
    transport: &IrohTransport,
    peer_set: &MirrorPeerSet,
    store: &EventStore,
    blob_store: &BlobStore,
    workspace_id: &WorkspaceId,
    options: &MirrorWorkspaceRunOptions,
) -> std::result::Result<MirrorWorkspaceAttemptReport, MirrorWorkspaceAttemptError> {
    let mut attempt = mirror_workspace_from_peers_once(
        transport,
        &peer_set.active,
        store,
        blob_store,
        workspace_id,
    )
    .await?;

    if !options.discover_peers {
        return Ok(attempt);
    }

    let refreshed_peer_set = mirror_peer_set(
        &peer_set.configured,
        store,
        workspace_id,
        true,
        options.hosted_endpoint.as_ref(),
        current_unix_millis(),
    )
    .map_err(|error| MirrorWorkspaceAttemptError {
        peer_failures: vec![MirrorPeerFailure {
            peer_endpoint: "discovered-peers".to_owned(),
            error: bounded_mirror_status_text(&error.to_string()),
        }],
    })?;

    let active = peer_set
        .active
        .iter()
        .map(|peer| peer.endpoint.as_str())
        .collect::<BTreeSet<_>>();
    let newly_discovered = refreshed_peer_set
        .active
        .into_iter()
        .filter(|peer| !active.contains(peer.endpoint.as_str()))
        .collect::<Vec<_>>();
    if newly_discovered.is_empty() {
        return Ok(attempt);
    }

    match mirror_workspace_from_peers_once(
        transport,
        &newly_discovered,
        store,
        blob_store,
        workspace_id,
    )
    .await
    {
        Ok(discovered_attempt) => {
            attempt.peer_endpoint = discovered_attempt.peer_endpoint;
            attempt.successful_peer_count += discovered_attempt.successful_peer_count;
            attempt
                .peer_failures
                .extend(discovered_attempt.peer_failures);
            attempt.report.merge_peer_pass(discovered_attempt.report);
            Ok(attempt)
        }
        Err(discovered_error) => {
            attempt.peer_failures.extend(discovered_error.peer_failures);
            Ok(attempt)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorWorkspaceReport {
    requested_event_count: usize,
    fetched_event_count: usize,
    fetched_blob_count: usize,
    missing_blob_count: usize,
    missing_blob_hashes: Vec<String>,
    ignored_event_count: usize,
    gap_count: usize,
    gaps: Vec<MirrorWorkspaceGap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MirrorWorkspaceGap {
    event_id: String,
    missing_parent_ids: Vec<String>,
}

impl MirrorWorkspaceReport {
    fn merge_peer_pass(&mut self, peer_pass: Self) {
        self.requested_event_count += peer_pass.requested_event_count;
        self.fetched_event_count += peer_pass.fetched_event_count;
        self.fetched_blob_count += peer_pass.fetched_blob_count;
        self.ignored_event_count += peer_pass.ignored_event_count;
        self.missing_blob_count = peer_pass.missing_blob_count;
        self.missing_blob_hashes = peer_pass.missing_blob_hashes;
        self.gap_count = peer_pass.gap_count;
        self.gaps = peer_pass.gaps;
    }
}

async fn mirror_workspace_once(
    transport: &IrohTransport,
    peer: &PeerAddress,
    store: &EventStore,
    blob_store: &BlobStore,
    workspace_id: &WorkspaceId,
) -> Result<MirrorWorkspaceReport> {
    let remote_event_ids = transport
        .fetch_workspace_inventory(peer, workspace_id)
        .await?;
    let report = pull_workspace_from_peer_with_inventory(
        transport,
        peer,
        store,
        workspace_id.clone(),
        remote_event_ids,
    )
    .await?;
    let mut fetched_blob_count = 0;
    let mut missing_blob_hashes = Vec::new();
    let mut missing_local_blob_hashes = Vec::new();
    let materialized_events = materialized_workspace_events(store, workspace_id)?;
    for blob_hash in attachment_blob_hashes(&materialized_events) {
        if blob_store.has_complete_blob(&blob_hash)? {
            continue;
        }
        missing_local_blob_hashes.push(blob_hash);
    }
    let fetched_blobs = transport
        .fetch_blobs(peer, missing_local_blob_hashes.clone())
        .await?;
    for blob_hash in missing_local_blob_hashes {
        if let Some(bytes) = fetched_blobs.get(&blob_hash) {
            blob_store.put_bytes_with_hash(&blob_hash, bytes)?;
            fetched_blob_count += 1;
        } else {
            match transport.fetch_blob_chunked(peer, &blob_hash).await? {
                Some(bytes) => {
                    blob_store.put_bytes_with_hash(&blob_hash, &bytes)?;
                    fetched_blob_count += 1;
                }
                None => missing_blob_hashes.push(blob_hash),
            }
        }
    }
    let gaps = report
        .materialization
        .gaps
        .iter()
        .map(|gap| MirrorWorkspaceGap {
            event_id: gap.event_id.0.clone(),
            missing_parent_ids: gap
                .missing_parent_ids
                .iter()
                .map(|parent_id| parent_id.0.clone())
                .collect(),
        })
        .collect::<Vec<_>>();

    Ok(MirrorWorkspaceReport {
        requested_event_count: report.requested_event_ids.len(),
        fetched_event_count: report.fetched_event_ids.len(),
        fetched_blob_count,
        missing_blob_count: missing_blob_hashes.len(),
        missing_blob_hashes,
        ignored_event_count: report.ignored_event_ids.len(),
        gap_count: gaps.len(),
        gaps,
    })
}

fn attachment_blob_hashes(events: &[SignedEvent]) -> Vec<String> {
    let mut hashes = BTreeSet::new();
    for event in events {
        let attachments = match &event.event.body {
            EventBody::MessageCreated { attachments, .. }
            | EventBody::MessageCreatedEncrypted { attachments, .. }
            | EventBody::MessageReplyCreated { attachments, .. }
            | EventBody::MessageReplyCreatedEncrypted { attachments, .. } => attachments,
            _ => continue,
        };
        for attachment in attachments {
            hashes.insert(attachment.blob_hash.clone());
        }
    }
    hashes.into_iter().collect()
}

fn materialized_workspace_events(
    store: &EventStore,
    workspace_id: &WorkspaceId,
) -> Result<Vec<SignedEvent>> {
    let events = verified_node_events(store.list_parseable_events_for_workspace(&workspace_id.0)?);
    let mut state = WorkspaceState::new(workspace_id.clone());
    let report = state.apply_batch(&events)?;
    let mut events_by_id = events
        .into_iter()
        .map(|event| (event.event_id.clone(), event))
        .collect::<HashMap<_, _>>();

    Ok(report
        .applied_events
        .into_iter()
        .filter_map(|event_id| events_by_id.remove(&event_id))
        .collect())
}

fn verified_node_events(events: Vec<SignedEvent>) -> Vec<SignedEvent> {
    events
        .into_iter()
        .filter(|event| {
            event.author_public_key.is_empty() || verify_self_contained_event(event).is_ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::{Arc, Barrier},
        thread,
    };

    use chaft_crypto::{
        ContentKey, encrypted_blob_ref_from_payload, seal_attachment_blob, seal_message_markdown,
    };
    use chaft_identity::DeviceIdentity;
    use chaft_types::{
        AttachmentRef, ChannelId, DeviceId, EncryptedBlobRef, EventId, MessageId, SignableEvent,
        WORKSPACE_ID_MAX_BYTES,
    };
    use tokio::sync::oneshot;

    use super::*;

    fn signed(event: SignableEvent) -> SignedEvent {
        SignedEvent::from_signed_bytes(event, vec![1, 2, 3])
    }

    fn unused_direct_endpoint() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().to_string()
    }

    fn workspace_root(identity: &DeviceIdentity, workspace_id: WorkspaceId) -> SignedEvent {
        identity.sign_event(SignableEvent::new(
            workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Mirror Source".to_owned(),
            },
        ))
    }

    fn test_content_key() -> ContentKey {
        ContentKey::from_bytes([31; 32])
    }

    #[test]
    fn node_access_envelope_dedupe_is_workspace_scoped() {
        let tempdir = tempfile::tempdir().unwrap();
        let one = json!({
            "kind": "chaft.workspace-join-request.v1",
            "workspaceId": "wrk_access_one",
            "requestId": "req_same",
            "note": "first workspace"
        })
        .to_string();
        let two = json!({
            "kind": "chaft.workspace-join-request.v1",
            "workspaceId": "wrk_access_two",
            "requestId": "req_same",
            "note": "second workspace"
        })
        .to_string();

        write_access_envelope_entry(
            tempdir.path(),
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            None,
            &one,
        )
        .unwrap();
        write_access_envelope_entry(
            tempdir.path(),
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            None,
            &one,
        )
        .unwrap();
        write_access_envelope_entry(
            tempdir.path(),
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            None,
            &two,
        )
        .unwrap();

        let workspace_one = list_access_envelope_entries(
            tempdir.path(),
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            "wrk_access_one",
            20,
        )
        .unwrap();
        let workspace_two = list_access_envelope_entries(
            tempdir.path(),
            JOIN_REQUEST_INBOX_DIR,
            "requestText",
            "wrk_access_two",
            20,
        )
        .unwrap();

        assert_eq!(workspace_one.len(), 1);
        assert_eq!(workspace_two.len(), 1);
        assert!(
            String::from_utf8(workspace_one[0].clone())
                .unwrap()
                .contains("first workspace")
        );
        assert!(
            String::from_utf8(workspace_two[0].clone())
                .unwrap()
                .contains("second workspace")
        );
    }

    #[test]
    fn node_access_envelope_rejects_workspace_mismatch() {
        let tempdir = tempfile::tempdir().unwrap();
        let envelope = json!({
            "kind": "chaft.workspace-join-response.v1",
            "workspaceId": "wrk_payload",
            "requestId": "req_mismatch",
            "resolution": "declined"
        })
        .to_string();

        let error = write_access_envelope_entry(
            tempdir.path(),
            JOIN_RESPONSE_INBOX_DIR,
            "responseText",
            Some("wrk_explicit"),
            &envelope,
        )
        .unwrap_err();

        assert!(error.to_string().contains("workspace ID mismatch"));
        assert!(
            list_access_envelope_entries(
                tempdir.path(),
                JOIN_RESPONSE_INBOX_DIR,
                "responseText",
                "wrk_payload",
                20
            )
            .unwrap()
            .is_empty()
        );
    }

    fn sealed_attachment_fixture(
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        plaintext: &[u8],
    ) -> (Vec<u8>, EncryptedBlobRef) {
        let sealed = seal_attachment_blob(
            "test-key",
            &test_content_key(),
            workspace_id,
            channel_id,
            message_id,
            0,
            plaintext,
        )
        .unwrap();
        let encrypted = encrypted_blob_ref_from_payload(&sealed, plaintext.len() as u64).unwrap();
        (sealed.bytes, encrypted)
    }

    fn encrypted_attachment_ref(
        blob_hash: &str,
        ciphertext_byte_len: u64,
        encryption: EncryptedBlobRef,
    ) -> AttachmentRef {
        AttachmentRef {
            blob_hash: blob_hash.to_owned(),
            media_type: "text/plain".to_owned(),
            byte_len: ciphertext_byte_len,
            display_name: "note.txt".to_owned(),
            attachment_id: String::new(),
            encryption: Some(encryption),
        }
    }

    fn encrypted_message_body(
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: MessageId,
        attachment: Option<AttachmentRef>,
    ) -> EventBody {
        let sealed_markdown = seal_message_markdown(
            "test-key",
            &test_content_key(),
            workspace_id,
            channel_id,
            &message_id,
            "node mirror message",
        )
        .unwrap();
        EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            attachments: attachment.into_iter().collect(),
        }
    }

    fn insert_corrupt_event_json(store_path: &Path, workspace_id: &WorkspaceId, event_id: &str) {
        let connection = rusqlite::Connection::open(store_path).unwrap();
        connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)
                ",
                rusqlite::params![
                    event_id,
                    workspace_id.0.as_str(),
                    "dev_corrupt",
                    1_i64,
                    0_i64,
                    1_i64,
                    b"not valid signed event json".as_slice()
                ],
            )
            .unwrap();
    }

    fn set_signature_valid_metadata(store_path: &Path, event_id: &EventId, signature_valid: bool) {
        let connection = rusqlite::Connection::open(store_path).unwrap();
        connection
            .execute(
                "
                UPDATE events
                SET self_contained_signature_valid = ?2
                WHERE event_id = ?1
                ",
                rusqlite::params![event_id.0.as_str(), if signature_valid { 1_i64 } else { 0 }],
            )
            .unwrap();
    }

    #[test]
    fn repair_storage_metadata_report_repairs_node_store_metadata() {
        let node_dir = tempfile::tempdir().unwrap();
        let store_path = node_dir.path().join("events.db");
        let store = EventStore::open(&store_path).unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        store.append_event(&root).unwrap();
        set_signature_valid_metadata(&store_path, &root.event_id, false);
        insert_corrupt_event_json(
            &store_path,
            &workspace_id,
            "evt_corrupt_node_repair_tripwire",
        );

        let before = mirror_status_storage_health(&store, &workspace_id);
        assert_eq!(before["promotableServableMetadataCount"].as_u64(), Some(1));
        assert_eq!(before["poisonedServableMetadataCount"].as_u64(), Some(1));

        let report = repair_storage_metadata_report(&store, &workspace_id).unwrap();

        assert_eq!(
            report["workspaceId"].as_str(),
            Some(workspace_id.0.as_str())
        );
        assert_eq!(
            report["repair"]["signatureValidMetadataBeforeCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            report["repair"]["signatureValidMetadataAfterCount"].as_u64(),
            Some(1)
        );
        assert_eq!(report["repair"]["repairedMetadataCount"].as_u64(), Some(2));
        assert_eq!(
            report["repair"]["promotedServableMetadataCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            report["repair"]["clearedUnservableMetadataCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            report["storageHealth"]["corruptEventCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            report["storageHealth"]["poisonedServableMetadataCount"].as_u64(),
            Some(0)
        );
        assert_eq!(
            report["storageHealth"]["promotableServableMetadataCount"].as_u64(),
            Some(0)
        );
        assert_eq!(
            report["storageHealth"]["servableEventCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            store
                .list_servable_events_for_workspace(&workspace_id.0)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn mirror_status_file_replaces_previous_json() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let temp_file = node_dir.path().join("mirror-status.json.tmp");

        write_mirror_status_file_result(&status_file, &json!({ "health": "old" })).unwrap();
        fs::write(&temp_file, b"stale temp status").unwrap();
        write_mirror_status_file_result(
            &status_file,
            &json!({
                "health": "healthy",
                "lastResult": "success",
            }),
        )
        .unwrap();

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert!(!temp_file.exists());
    }

    #[test]
    fn mirror_status_write_rejects_oversized_json_without_replacing_previous_status() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        write_mirror_status_file_result(
            &status_file,
            &json!({
                "health": "healthy",
                "lastResult": "success",
            }),
        )
        .unwrap();

        let oversized_status = json!({
            "health": "partial",
            "lastError": "x".repeat(MIRROR_STATUS_FILE_MAX_BYTES + 1),
        });
        let error = write_mirror_status_file_result(&status_file, &oversized_status).unwrap_err();
        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert!(
            error
                .to_string()
                .contains("mirror status JSON is too large")
        );
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert!(mirror_status_temp_artifacts_under(node_dir.path()).is_empty());
    }

    #[test]
    fn mirror_status_file_concurrent_writes_use_unique_temp_files() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = Arc::new(node_dir.path().join("mirror-status.json"));
        let writer_count = 8usize;
        let barrier = Arc::new(Barrier::new(writer_count));
        let mut handles = Vec::new();

        for writer in 0..writer_count {
            let status_file = Arc::clone(&status_file);
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                barrier.wait();
                write_mirror_status_file_result(
                    &status_file,
                    &json!({
                        "health": "healthy",
                        "writer": writer,
                    }),
                )
                .unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file.as_ref()).unwrap()).unwrap();
        let writer = status["writer"].as_u64().unwrap();

        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert!(writer < writer_count as u64);
        assert!(mirror_status_temp_artifacts_under(node_dir.path()).is_empty());
    }

    #[test]
    fn mirror_status_read_rejects_oversized_file_before_parse() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let file = fs::File::create(&status_file).unwrap();
        file.set_len(MIRROR_STATUS_FILE_MAX_BYTES as u64 + 1)
            .unwrap();

        let error = read_mirror_status_file(&status_file).unwrap_err();
        assert!(error.to_string().contains("mirror status file"));
        assert!(error.to_string().contains("is too large"));
    }

    #[test]
    fn mirror_status_read_write_reject_invalid_paths_before_filesystem_work() {
        let oversized_status_file = PathBuf::from("s".repeat(NODE_PATH_MAX_BYTES + 1));

        let blank_write =
            write_mirror_status_file_result(Path::new(""), &json!({ "health": "healthy" }))
                .unwrap_err();
        let oversized_write = write_mirror_status_file_result(
            &oversized_status_file,
            &json!({ "health": "healthy" }),
        )
        .unwrap_err();
        let blank_read = read_mirror_status_file(Path::new("")).unwrap_err();
        let oversized_read = read_mirror_status_file(&oversized_status_file).unwrap_err();

        assert!(
            blank_write
                .to_string()
                .contains("mirror status file cannot be empty")
        );
        assert!(
            oversized_write
                .to_string()
                .contains("mirror status file is too large")
        );
        assert!(
            blank_read
                .to_string()
                .contains("mirror status file cannot be empty")
        );
        assert!(
            oversized_read
                .to_string()
                .contains("mirror status file is too large")
        );
    }

    fn mirror_status_temp_artifacts_under(root: &Path) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                let is_temp_artifact = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(|name| name.contains("mirror-status.json.tmp"))
                    .unwrap_or(false);
                if is_temp_artifact {
                    artifacts.push(path);
                }
            }
        }
        artifacts.sort();
        artifacts
    }

    #[test]
    fn mirror_status_caps_partial_samples_and_preserves_counts() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let workspace_id = WorkspaceId::new();
        let peer_endpoint = "direct+tcp://127.0.0.1:7001".to_owned();
        let peer = PeerAddress {
            peer_id: PeerId(peer_endpoint.clone()),
            endpoint: peer_endpoint.clone(),
        };
        let peer_set = MirrorPeerSet {
            configured: vec![peer.clone()],
            discovered: Vec::new(),
            active: vec![peer],
        };
        let missing_blob_hashes = (0..MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS + 3)
            .map(|index| format!("missing-blob-{index:03}"))
            .collect::<Vec<_>>();
        let gaps = (0..MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS + 3)
            .map(|index| MirrorWorkspaceGap {
                event_id: format!("event-{index:03}"),
                missing_parent_ids: vec![format!("parent-{index:03}")],
            })
            .collect::<Vec<_>>();
        let attempt = MirrorWorkspaceAttemptReport {
            peer_endpoint,
            successful_peer_count: 1,
            peer_failures: Vec::new(),
            report: MirrorWorkspaceReport {
                requested_event_count: 100,
                fetched_event_count: 90,
                fetched_blob_count: 12,
                missing_blob_count: missing_blob_hashes.len(),
                missing_blob_hashes,
                ignored_event_count: 4,
                gap_count: gaps.len(),
                gaps,
            },
        };
        let storage_health = json!({
            "workspaceId": workspace_id.0,
            "totalEventCount": 100,
            "parseableEventCount": 100,
            "corruptEventCount": 0,
            "signatureValidMetadataCount": 100,
            "servableEventCount": 100,
            "poisonedServableMetadataCount": 0,
            "promotableServableMetadataCount": 0,
            "nonServableParseableEventCount": 0,
        });

        write_mirror_success_status(
            Some(&status_file),
            &workspace_id,
            &peer_set,
            None,
            1_700_000_000_000,
            &storage_health,
            &attempt,
        );

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();
        let last_report = &status["lastReport"];

        assert_eq!(status["health"].as_str(), Some("partial"));
        assert_eq!(status["partial"].as_bool(), Some(true));
        assert_eq!(
            status["storageHealth"]["totalEventCount"].as_u64(),
            Some(100)
        );
        assert_eq!(
            status["storageHealth"]["poisonedServableMetadataCount"].as_u64(),
            Some(0)
        );
        assert_eq!(
            last_report["missingBlobCount"].as_u64(),
            Some((MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS + 3) as u64)
        );
        assert_eq!(
            last_report["missingBlobHashes"].as_array().unwrap().len(),
            MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS
        );
        assert_eq!(
            last_report["missingBlobHashes"][MAX_MIRROR_STATUS_MISSING_BLOB_HASH_SAMPLE_ROWS - 1]
                .as_str(),
            Some("missing-blob-063")
        );
        assert_eq!(
            last_report["gapCount"].as_u64(),
            Some((MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS + 3) as u64)
        );
        assert_eq!(
            last_report["gaps"].as_array().unwrap().len(),
            MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS
        );
        assert_eq!(
            last_report["gaps"][MAX_MIRROR_STATUS_GAP_SAMPLE_ROWS - 1]["eventId"].as_str(),
            Some("event-063")
        );
    }

    #[test]
    fn mirror_status_caps_peer_failure_error_text() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let workspace_id = WorkspaceId::new();
        let peer_endpoint = "direct+tcp://127.0.0.1:7001".to_owned();
        let peer = PeerAddress {
            peer_id: PeerId(peer_endpoint.clone()),
            endpoint: peer_endpoint.clone(),
        };
        let peer_set = MirrorPeerSet {
            configured: vec![peer.clone()],
            discovered: Vec::new(),
            active: vec![peer],
        };
        let long_error = "é".repeat(MAX_MIRROR_STATUS_ERROR_BYTES);
        let attempt_error = MirrorWorkspaceAttemptError {
            peer_failures: vec![MirrorPeerFailure {
                peer_endpoint,
                error: long_error,
            }],
        };
        let storage_health = json!({
            "workspaceId": workspace_id.0,
            "totalEventCount": 0,
            "parseableEventCount": 0,
            "corruptEventCount": 0,
            "signatureValidMetadataCount": 0,
            "servableEventCount": 0,
            "poisonedServableMetadataCount": 0,
            "promotableServableMetadataCount": 0,
            "nonServableParseableEventCount": 0,
        });

        write_mirror_failure_status(
            Some(&status_file),
            &workspace_id,
            &peer_set,
            None,
            1_700_000_000_000,
            &storage_health,
            &attempt_error,
        );

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();
        let last_error = status["lastError"].as_str().unwrap();
        let peer_error = status["peerFailures"][0]["error"].as_str().unwrap();

        assert_eq!(status["health"].as_str(), Some("unreachable"));
        assert!(last_error.len() <= MAX_MIRROR_STATUS_ERROR_BYTES);
        assert!(peer_error.len() <= MAX_MIRROR_STATUS_ERROR_BYTES);
        assert!(last_error.ends_with(STATUS_TRUNCATED_SUFFIX));
        assert!(peer_error.ends_with(STATUS_TRUNCATED_SUFFIX));
    }

    #[test]
    fn mirror_status_local_storage_health_can_make_success_partial() {
        let node_dir = tempfile::tempdir().unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let workspace_id = WorkspaceId::new();
        let peer_endpoint = "direct+tcp://127.0.0.1:7001".to_owned();
        let peer = PeerAddress {
            peer_id: PeerId(peer_endpoint.clone()),
            endpoint: peer_endpoint.clone(),
        };
        let peer_set = MirrorPeerSet {
            configured: vec![peer.clone()],
            discovered: Vec::new(),
            active: vec![peer],
        };
        let attempt = MirrorWorkspaceAttemptReport {
            peer_endpoint,
            successful_peer_count: 1,
            peer_failures: Vec::new(),
            report: MirrorWorkspaceReport {
                requested_event_count: 1,
                fetched_event_count: 1,
                fetched_blob_count: 0,
                missing_blob_count: 0,
                missing_blob_hashes: Vec::new(),
                ignored_event_count: 0,
                gap_count: 0,
                gaps: Vec::new(),
            },
        };
        let storage_health = json!({
            "workspaceId": workspace_id.0,
            "totalEventCount": 3,
            "parseableEventCount": 2,
            "corruptEventCount": 1,
            "signatureValidMetadataCount": 2,
            "servableEventCount": 1,
            "poisonedServableMetadataCount": 1,
            "promotableServableMetadataCount": 0,
            "nonServableParseableEventCount": 1,
        });

        write_mirror_success_status(
            Some(&status_file),
            &workspace_id,
            &peer_set,
            None,
            1_700_000_000_000,
            &storage_health,
            &attempt,
        );

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("partial"));
        assert_eq!(status["partial"].as_bool(), Some(true));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(0));
        assert_eq!(
            status["storageHealth"]["corruptEventCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            status["storageHealth"]["poisonedServableMetadataCount"].as_u64(),
            Some(1)
        );
    }

    #[test]
    fn mirror_status_summary_includes_health_counters_and_hosted_endpoint() {
        let status = json!({
            "schemaVersion": 1,
            "workspaceId": "wrk_status",
            "configuredPeers": ["127.0.0.1:7001", "127.0.0.1:7002"],
            "discoveredPeers": ["127.0.0.1:7003"],
            "activePeers": ["127.0.0.1:7001", "127.0.0.1:7002", "127.0.0.1:7003"],
            "hostedEndpoint": {
                "endpoint": "iroh://node?addr=127.0.0.1:7003",
                "transport": "iroh",
            },
            "checkedAtUnixMs": 1_700_000_000_000u64,
            "lastResult": "success",
            "health": "partial",
            "lastSuccessfulPeer": "127.0.0.1:7002",
            "peerFailures": [
                {
                    "peerEndpoint": "127.0.0.1:7001",
                    "error": "offline",
                },
            ],
            "lastReport": {
                "successfulPeerCount": 1,
                "requestedEventCount": 5,
                "fetchedEventCount": 3,
                "fetchedBlobCount": 2,
                "missingBlobCount": 1,
                "gapCount": 1,
            },
            "storageHealth": {
                "totalEventCount": 9,
                "corruptEventCount": 1,
                "poisonedServableMetadataCount": 2,
                "promotableServableMetadataCount": 3,
                "nonServableParseableEventCount": 4,
            },
        });

        let summary = mirror_status_summary_text_at(&status, 1_700_000_005_000u64);

        assert!(summary.contains("workspace=wrk_status"));
        assert!(summary.contains("health=partial"));
        assert!(summary.contains("checkedAtUnixMs=1700000000000"));
        assert!(summary.contains("ageMs=5000"));
        assert!(summary.contains("hostedEndpoint=iroh://node?addr=127.0.0.1:7003"));
        assert!(summary.contains("hostedTransport=iroh"));
        assert!(summary.contains("configuredPeers=2"));
        assert!(summary.contains("discoveredPeers=1"));
        assert!(summary.contains("activePeers=3"));
        assert!(summary.contains("successfulPeers=1"));
        assert!(summary.contains("missingBlobs=1"));
        assert!(summary.contains("gaps=1"));
        assert!(summary.contains("failures=1"));
        assert!(summary.contains("storageRows=9"));
        assert!(summary.contains("storageCorrupt=1"));
        assert!(summary.contains("storagePoisoned=2"));
        assert!(summary.contains("storagePromotable=3"));
        assert!(summary.contains("storageNonServable=4"));
        assert!(ensure_mirror_status_healthy(&status).is_err());
    }

    #[test]
    fn mirror_status_healthy_gate_accepts_only_healthy_status() {
        assert!(ensure_mirror_status_healthy(&json!({ "health": "healthy" })).is_ok());
        assert!(ensure_mirror_status_healthy(&json!({ "health": "partial" })).is_err());
        assert!(ensure_mirror_status_healthy(&json!({})).is_err());
    }

    #[test]
    fn mirror_status_freshness_gate_uses_checked_at_timestamp() {
        let status = json!({ "checkedAtUnixMs": 10_000u64 });

        assert!(ensure_mirror_status_fresh(&status, 5, 15_000).is_ok());
        assert!(ensure_mirror_status_fresh(&status, 5, 15_001).is_err());
        assert!(ensure_mirror_status_fresh(&json!({}), 5, 15_000).is_err());
        assert!(ensure_mirror_status_fresh(&status, 0, 10_000).is_ok());
        assert!(ensure_mirror_status_fresh(&status, 0, 10_001).is_err());
    }

    #[test]
    fn mirror_blob_hashes_ignore_unmaterialized_gap_events() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Node Mirror".to_owned(),
            },
        ));
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = signed(channel);
        let complete_message_id = MessageId::new();
        let (complete_blob_bytes, complete_blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &complete_message_id,
            b"materialized mirror attachment",
        );
        let mut complete_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                complete_message_id,
                Some(encrypted_attachment_ref(
                    "materialized-blob",
                    complete_blob_bytes.len() as u64,
                    complete_blob_encryption,
                )),
            ),
        );
        complete_message.parents = vec![channel.event_id.clone()];
        let complete_message = signed(complete_message);
        let gap_message_id = MessageId::new();
        let (gap_blob_bytes, gap_blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &gap_message_id,
            b"gap mirror attachment",
        );
        let mut gap_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id,
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                gap_message_id,
                Some(encrypted_attachment_ref(
                    "gap-blob",
                    gap_blob_bytes.len() as u64,
                    gap_blob_encryption,
                )),
            ),
        );
        gap_message.parents = vec![EventId("evt_missing_gap_parent".to_owned())];
        let gap_message = signed(gap_message);

        for event in [&root, &channel, &complete_message, &gap_message] {
            store.append_event(event).unwrap();
        }

        let materialized = materialized_workspace_events(&store, &workspace_id).unwrap();
        let blob_hashes = attachment_blob_hashes(&materialized);

        assert_eq!(blob_hashes, vec!["materialized-blob".to_owned()]);
        assert_eq!(
            store
                .list_events_for_workspace(&workspace_id.0)
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn mirror_blob_hashes_ignore_corrupt_local_event_json() {
        let node_dir = tempfile::tempdir().unwrap();
        let store_path = node_dir.path().join("events.db");
        let store = EventStore::open(&store_path).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"materialized mirror attachment",
        );
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    "materialized-blob",
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);

        for event in [&root, &channel, &message] {
            store.append_event(event).unwrap();
        }
        insert_corrupt_event_json(
            &store_path,
            &workspace_id,
            "evt_corrupt_node_materialization_tripwire",
        );
        assert!(store.list_events_for_workspace(&workspace_id.0).is_err());

        let materialized = materialized_workspace_events(&store, &workspace_id).unwrap();
        let blob_hashes = attachment_blob_hashes(&materialized);

        assert_eq!(materialized.len(), 3);
        assert_eq!(blob_hashes, vec!["materialized-blob".to_owned()]);
    }

    #[test]
    fn mirror_blob_hashes_ignore_invalid_signature_event_blobs() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let valid_message_id = MessageId::new();
        let (valid_blob_bytes, valid_blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &valid_message_id,
            b"valid mirror attachment",
        );
        let mut valid_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                valid_message_id,
                Some(encrypted_attachment_ref(
                    "valid-blob",
                    valid_blob_bytes.len() as u64,
                    valid_blob_encryption,
                )),
            ),
        );
        valid_message.parents = vec![channel.event_id.clone()];
        let valid_message = owner.sign_event(valid_message);
        let forged_message_id = MessageId::new();
        let (forged_blob_bytes, forged_blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &forged_message_id,
            b"forged mirror attachment",
        );
        let mut forged_message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                forged_message_id,
                Some(encrypted_attachment_ref(
                    "forged-blob",
                    forged_blob_bytes.len() as u64,
                    forged_blob_encryption,
                )),
            ),
        );
        forged_message.parents = vec![valid_message.event_id.clone()];
        let mut forged_message = owner.sign_event(forged_message);
        forged_message.signature[0] ^= 1;

        for event in [&root, &channel, &valid_message, &forged_message] {
            store.append_event(event).unwrap();
        }

        let materialized = materialized_workspace_events(&store, &workspace_id).unwrap();
        let blob_hashes = attachment_blob_hashes(&materialized);

        assert_eq!(blob_hashes, vec!["valid-blob".to_owned()]);
        assert_eq!(
            store
                .list_events_for_workspace(&workspace_id.0)
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn discovered_peer_addresses_ignore_invalid_signature_endpoint_hints() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut forged_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "forged-backup".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        forged_endpoint.parents = vec![root.event_id.clone()];
        let mut forged_endpoint = owner.sign_event(forged_endpoint);
        forged_endpoint.signature[0] ^= 1;
        for event in [&root, &forged_endpoint] {
            store.append_event(event).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert!(peers.is_empty());
    }

    #[test]
    fn discovered_peer_addresses_ignore_oversized_endpoint_hints() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "oversized-backup".to_owned(),
                endpoint: "e".repeat(PEER_ENDPOINT_MAX_BYTES + 1),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        endpoint.parents = vec![root.event_id.clone()];
        let endpoint = owner.sign_event(endpoint);
        for event in [&root, &endpoint] {
            store.append_event(event).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert!(peers.is_empty());
    }

    #[test]
    fn discovered_peer_addresses_ignore_blank_endpoint_hints() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        store.append_event(&root).unwrap();

        for (endpoint_id, endpoint, transport) in [
            (" ", "direct+tcp://127.0.0.1:7997", "direct-tcp"),
            ("blank-endpoint", " ", "direct-tcp"),
            ("blank-transport", "direct+tcp://127.0.0.1:7998", " "),
        ] {
            let mut blank_hint = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.device_id().clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: endpoint_id.to_owned(),
                    endpoint: endpoint.to_owned(),
                    transport: transport.to_owned(),
                    is_backup_peer: true,
                    expires_at_ms: None,
                    replica_storage_class: None,
                    replica_retention_hint: None,
                },
            );
            blank_hint.parents = vec![root.event_id.clone()];
            store.append_event(&owner.sign_event(blank_hint)).unwrap();
        }

        let mut supported_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "direct-node".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        supported_endpoint.parents = vec![root.event_id.clone()];
        store
            .append_event(&owner.sign_event(supported_endpoint))
            .unwrap();

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7999");
    }

    #[test]
    fn discovered_peer_addresses_ignore_unsupported_endpoint_schemes() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut unsupported_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "centralized-ws".to_owned(),
                endpoint: "wss://central.example.invalid/sync".to_owned(),
                transport: "wss".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        unsupported_endpoint.parents = vec![root.event_id.clone()];
        let unsupported_endpoint = owner.sign_event(unsupported_endpoint);
        let mut supported_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "direct-node".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        supported_endpoint.parents = vec![root.event_id.clone()];
        let supported_endpoint = owner.sign_event(supported_endpoint);
        for event in [&root, &unsupported_endpoint, &supported_endpoint] {
            store.append_event(event).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7999");
    }

    #[test]
    fn discovered_peer_addresses_ignore_mismatched_endpoint_transport_labels() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut mismatched_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "bad-label".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7998".to_owned(),
                transport: "iroh".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        mismatched_endpoint.parents = vec![root.event_id.clone()];
        let mismatched_endpoint = owner.sign_event(mismatched_endpoint);
        let mut supported_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "direct-node".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        supported_endpoint.parents = vec![root.event_id.clone()];
        let supported_endpoint = owner.sign_event(supported_endpoint);
        for event in [&root, &mismatched_endpoint, &supported_endpoint] {
            store.append_event(event).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7999");
    }

    #[test]
    fn workspace_id_arg_trims_cli_input() {
        let workspace_id = workspace_id_arg("  wrk_node_mirror  ".to_owned()).unwrap();

        assert_eq!(workspace_id.0, "wrk_node_mirror");
    }

    #[test]
    fn workspace_id_arg_rejects_blank_cli_input() {
        let error = workspace_id_arg(" \t\n ".to_owned()).unwrap_err();

        assert!(error.to_string().contains("workspace ID cannot be empty"));
    }

    #[test]
    fn node_path_args_reject_blank_paths() {
        let error = checked_node_path_arg(PathBuf::new(), "data directory").unwrap_err();

        assert!(error.to_string().contains("data directory cannot be empty"));
    }

    #[test]
    fn node_path_args_reject_oversized_paths() {
        let error = checked_node_path_arg(
            PathBuf::from("d".repeat(NODE_PATH_MAX_BYTES + 1)),
            "data directory",
        )
        .unwrap_err();

        assert!(error.to_string().contains("data directory is too large"));
    }

    #[test]
    fn node_derived_paths_reject_oversized_paths() {
        let data_dir = PathBuf::from("d".repeat(NODE_PATH_MAX_BYTES));
        let error =
            checked_node_child_path(&data_dir, "events.db", "event store path").unwrap_err();

        assert!(error.to_string().contains("event store path is too large"));
    }

    #[test]
    fn node_store_helpers_reject_invalid_paths_before_filesystem_work() {
        let tempdir = tempfile::tempdir().unwrap();
        let data_dir = tempdir.path().join("node-data");
        let oversized_store = PathBuf::from("e".repeat(NODE_PATH_MAX_BYTES + 1));
        let oversized_blob = PathBuf::from("b".repeat(NODE_PATH_MAX_BYTES + 1));

        let store_error = match open_node_store(&data_dir, &oversized_store) {
            Ok(_) => panic!("expected oversized store path to fail"),
            Err(error) => error,
        };
        let blob_error = match open_node_store_with_blobs(
            &data_dir,
            &data_dir.join("events.db"),
            &oversized_blob,
        ) {
            Ok(_) => panic!("expected oversized blob path to fail"),
            Err(error) => error,
        };

        assert!(
            store_error
                .to_string()
                .contains("event store path is too large")
        );
        assert!(
            blob_error
                .to_string()
                .contains("blob store path is too large")
        );
        assert!(!data_dir.exists());
    }

    #[test]
    fn mirror_status_file_path_rejects_oversized_explicit_path() {
        let error = mirror_status_file_path(
            Path::new("data"),
            Some(PathBuf::from("s".repeat(NODE_PATH_MAX_BYTES + 1))),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("mirror status file is too large")
        );
    }

    #[test]
    fn mirror_status_file_path_rejects_oversized_default_path() {
        let data_dir = PathBuf::from("d".repeat(NODE_PATH_MAX_BYTES));
        let error = mirror_status_file_path(&data_dir, None).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("mirror status file is too large")
        );
    }

    #[test]
    fn workspace_id_arg_rejects_oversized_cli_input() {
        let error = workspace_id_arg("w".repeat(WORKSPACE_ID_MAX_BYTES + 1)).unwrap_err();

        assert!(error.to_string().contains("workspace ID is too large"));
    }

    #[test]
    fn mirror_peer_addresses_reject_oversized_configured_endpoints() {
        let error =
            mirror_peer_addresses(vec!["e".repeat(PEER_ENDPOINT_MAX_BYTES + 1)]).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("mirror peer endpoint is too large")
        );
    }

    #[test]
    fn mirror_peer_addresses_reject_unsupported_configured_endpoint_schemes() {
        for endpoint in [
            "https://central.example.invalid/sync",
            "wss://central.example.invalid/sync",
            "relay://relay.example.invalid/device",
            "discovery://workspace",
        ] {
            let error = mirror_peer_addresses(vec![endpoint.to_owned()]).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("direct TCP or native Iroh direct route"),
                "unexpected error for {endpoint}: {error}"
            );
        }
    }

    #[test]
    fn mirror_peer_addresses_reject_malformed_configured_direct_endpoints() {
        for endpoint in [
            "direct+tcp://127.0.0.1:0",
            "tcp://127.0.0.1:0",
            "127.0.0.1:0",
            "127.0.0.1:not-a-port",
            "direct+tcp://127.0.0.1",
        ] {
            let error = mirror_peer_addresses(vec![endpoint.to_owned()]).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("direct TCP or native Iroh direct route"),
                "unexpected error for {endpoint}: {error}"
            );
        }
    }

    #[test]
    fn mirror_peer_addresses_deduplicate_configured_peers_before_limit() {
        let peers = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| {
                if index % 2 == 0 {
                    " direct+tcp://127.0.0.1:7001 ".to_owned()
                } else {
                    "direct+tcp://127.0.0.1:7002".to_owned()
                }
            })
            .collect::<Vec<_>>();

        let peers = mirror_peer_addresses(peers).unwrap();

        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7001");
        assert_eq!(peers[1].endpoint, "direct+tcp://127.0.0.1:7002");
    }

    #[test]
    fn mirror_peer_addresses_reject_oversized_configured_peer_lists() {
        let peers = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| format!("direct+tcp://127.0.0.1:{}", 10_000 + index))
            .collect::<Vec<_>>();
        let error = mirror_peer_addresses(peers).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("mirror peer endpoint list is too large")
        );
    }

    #[test]
    fn discovered_peer_addresses_ignore_corrupt_local_event_json() {
        let node_dir = tempfile::tempdir().unwrap();
        let store_path = node_dir.path().join("events.db");
        let store = EventStore::open(&store_path).unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "node-primary".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: Some(current_unix_millis() as i64 + 60_000),
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        endpoint.parents = vec![root.event_id.clone()];
        let endpoint = owner.sign_event(endpoint);
        for event in [&root, &endpoint] {
            store.append_event(event).unwrap();
        }
        insert_corrupt_event_json(
            &store_path,
            &workspace_id,
            "evt_corrupt_node_discovery_tripwire",
        );
        assert!(store.list_events_for_workspace(&workspace_id.0).is_err());

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7999");
    }

    #[test]
    fn discovered_peer_addresses_use_authorized_non_expired_endpoint_hints() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut fresh_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "fresh-backup".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7001".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        fresh_endpoint.parents = vec![root.event_id.clone()];
        let fresh_endpoint = owner.sign_event(fresh_endpoint);
        let mut expired_endpoint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "expired-backup".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7002".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: Some(1),
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        expired_endpoint.parents = vec![fresh_endpoint.event_id.clone()];
        let expired_endpoint = owner.sign_event(expired_endpoint);
        for event in [&root, &fresh_endpoint, &expired_endpoint] {
            store.append_event(event).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].endpoint, "direct+tcp://127.0.0.1:7001");
    }

    #[test]
    fn discovered_peer_addresses_prioritize_replica_capabilities() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        store.append_event(&root).unwrap();
        let mut parent_id = root.event_id.clone();

        let endpoints = [
            ("member-newest", 7205, false, None, 5_i64),
            (
                "partial-backup",
                7203,
                true,
                Some(ReplicaStorageClass::PartialHistory),
                4_i64,
            ),
            ("legacy-backup", 7202, true, None, 3_i64),
            (
                "full-history",
                7201,
                true,
                Some(ReplicaStorageClass::FullHistory),
                2_i64,
            ),
            (
                "full-history-with-blobs",
                7200,
                true,
                Some(ReplicaStorageClass::FullHistoryWithBlobs),
                1_i64,
            ),
        ];

        for (endpoint_id, port, is_backup_peer, replica_storage_class, physical_ms) in endpoints {
            let mut endpoint = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.device_id().clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: endpoint_id.to_owned(),
                    endpoint: format!("direct+tcp://127.0.0.1:{port}"),
                    transport: "direct-tcp".to_owned(),
                    is_backup_peer,
                    expires_at_ms: None,
                    replica_storage_class,
                    replica_retention_hint: None,
                },
            );
            endpoint.timestamp = chaft_types::HybridTimestamp {
                physical_ms,
                logical: 0,
            };
            endpoint.parents = vec![parent_id];
            let endpoint = owner.sign_event(endpoint);
            parent_id = endpoint.event_id.clone();
            store.append_event(&endpoint).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        let endpoints = peers
            .iter()
            .map(|peer| peer.endpoint.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            endpoints,
            vec![
                "direct+tcp://127.0.0.1:7200",
                "direct+tcp://127.0.0.1:7201",
                "direct+tcp://127.0.0.1:7202",
                "direct+tcp://127.0.0.1:7203",
                "direct+tcp://127.0.0.1:7205",
            ]
        );
    }

    #[test]
    fn discovered_peer_addresses_are_capped_after_priority_sorting() {
        let store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        store.append_event(&root).unwrap();
        let total_endpoint_count = MAX_DISCOVERED_MIRROR_PEERS + 5;
        let mut parent_id = root.event_id.clone();

        for index in 0..total_endpoint_count {
            let mut endpoint = SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.device_id().clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: format!("peer-{index:03}"),
                    endpoint: format!("direct+tcp://127.0.0.1:{}", 7000 + index),
                    transport: "direct-tcp".to_owned(),
                    is_backup_peer: false,
                    expires_at_ms: None,
                    replica_storage_class: None,
                    replica_retention_hint: None,
                },
            );
            endpoint.timestamp = chaft_types::HybridTimestamp {
                physical_ms: index as i64,
                logical: 0,
            };
            endpoint.parents = vec![parent_id];
            let endpoint = owner.sign_event(endpoint);
            parent_id = endpoint.event_id.clone();
            store.append_event(&endpoint).unwrap();
        }

        let peers =
            discovered_peer_addresses(&store, &workspace_id, current_unix_millis()).unwrap();

        let expected_first = format!("direct+tcp://127.0.0.1:{}", 7000 + total_endpoint_count - 1);
        let expected_last = format!(
            "direct+tcp://127.0.0.1:{}",
            7000 + total_endpoint_count - MAX_DISCOVERED_MIRROR_PEERS
        );
        assert_eq!(peers.len(), MAX_DISCOVERED_MIRROR_PEERS);
        assert_eq!(
            peers.first().map(|peer| peer.endpoint.as_str()),
            Some(expected_first.as_str())
        );
        assert_eq!(
            peers.last().map(|peer| peer.endpoint.as_str()),
            Some(expected_last.as_str())
        );
    }

    #[tokio::test]
    async fn mirror_workspace_uses_workspace_scoped_peer_inventory() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let primary_workspace_id = WorkspaceId::new();
        let other_workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let primary_root = workspace_root(&owner, primary_workspace_id.clone());
        let invalid_other_root = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                other_workspace_id.clone(),
                None,
                DeviceId("dev_invalid".to_owned()),
                EventBody::WorkspaceCreated {
                    name: "Invalid Other".to_owned(),
                },
            ),
            vec![9, 9, 9],
        );
        remote_store.append_event(&primary_root).unwrap();
        remote_store.append_event(&invalid_other_root).unwrap();

        let server = DirectPeerServer::bind("127.0.0.1:0", remote_store)
            .await
            .unwrap();
        let peer_endpoint = format!("direct+tcp://{}", server.local_addr().unwrap());
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server_task =
            tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();

        mirror_workspace(
            node_store,
            node_blob_store,
            primary_workspace_id.clone(),
            vec![peer_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, None),
        )
        .await
        .unwrap();

        let reopened = EventStore::open(&node_store_path).unwrap();
        let primary_events = reopened
            .list_events_for_workspace(&primary_workspace_id.0)
            .unwrap();
        let other_events = reopened
            .list_events_for_workspace(&other_workspace_id.0)
            .unwrap();

        assert_eq!(primary_events, vec![primary_root]);
        assert!(other_events.is_empty());

        shutdown_tx.send(()).unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_can_serve_mirrored_data() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        remote_store.append_event(&root).unwrap();
        let remote_server = DirectPeerServer::bind("127.0.0.1:0", remote_store)
            .await
            .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let hosted = start_mirror_server(
            Some("127.0.0.1:0".to_owned()),
            false,
            &node_store_path,
            &node_blob_path,
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        .unwrap()
        .unwrap();
        let served_peer = PeerAddress {
            peer_id: PeerId("mirror".to_owned()),
            endpoint: hosted.endpoint.clone(),
        };

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, None),
        )
        .await
        .unwrap();

        let transport = DirectTransport;
        let inventory = transport
            .fetch_workspace_inventory(&served_peer, &workspace_id)
            .await
            .unwrap();

        assert_eq!(inventory, vec![root.event_id]);

        hosted.stop().await.unwrap();
        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_can_serve_mirrored_data_over_native_iroh() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        remote_store.append_event(&root).unwrap();
        let remote_server = DirectPeerServer::bind("127.0.0.1:0", remote_store)
            .await
            .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let hosted = start_mirror_server(
            None,
            true,
            &node_store_path,
            &node_blob_path,
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        .unwrap()
        .unwrap();

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, None),
        )
        .await
        .unwrap();

        let served_peer = PeerAddress {
            peer_id: PeerId("mirror-iroh".to_owned()),
            endpoint: hosted.endpoint.clone(),
        };
        let transport = IrohTransport::default();
        let inventory = transport
            .fetch_workspace_inventory(&served_peer, &workspace_id)
            .await
            .unwrap();

        assert_eq!(inventory, vec![root.event_id]);

        hosted.stop().await.unwrap();
        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_server_rejects_direct_and_native_iroh_listen_together() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let error = match start_mirror_server(
            Some("127.0.0.1:0".to_owned()),
            true,
            &node_store_path,
            &node_blob_path,
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        {
            Ok(_) => panic!("expected mixed listen modes to fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("use either --listen for direct TCP or --listen-iroh")
        );
    }

    #[test]
    fn mirror_listen_options_validate_before_storage_open() {
        assert_eq!(
            normalize_mirror_listen_options(Some(" 127.0.0.1:0 ".to_owned()), false).unwrap(),
            Some("127.0.0.1:0".to_owned())
        );
        assert_eq!(normalize_mirror_listen_options(None, true).unwrap(), None);

        let mixed =
            normalize_mirror_listen_options(Some("127.0.0.1:0".to_owned()), true).unwrap_err();
        assert!(
            mixed
                .to_string()
                .contains("use either --listen for direct TCP or --listen-iroh")
        );

        let unsupported = normalize_mirror_listen_options(
            Some("https://central.example.invalid/sync".to_owned()),
            false,
        )
        .unwrap_err();
        assert!(
            unsupported
                .to_string()
                .contains("mirror listen endpoint must be host:port")
        );
    }

    #[test]
    fn max_active_connections_arg_rejects_zero() {
        assert_eq!(
            max_active_connections_arg(1, "max active connections").unwrap(),
            1
        );
        let error = max_active_connections_arg(0, "max active connections").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("max active connections must be greater than zero")
        );
    }

    #[tokio::test]
    async fn mirror_server_rejects_zero_direct_connection_limit_before_storage_open() {
        let node_dir = tempfile::tempdir().unwrap();
        let oversized_store_path = PathBuf::from("e".repeat(NODE_PATH_MAX_BYTES + 1));
        let error = match start_mirror_server(
            Some("127.0.0.1:0".to_owned()),
            false,
            &oversized_store_path,
            &node_dir.path().join("blobs"),
            0,
        )
        .await
        {
            Ok(_) => panic!("expected zero connection limit to fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("mirror max active connections must be greater than zero")
        );
        assert!(!node_dir.path().join("blobs").exists());
    }

    #[tokio::test]
    async fn mirror_server_rejects_oversized_direct_listen_endpoint() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        let error = match start_mirror_server(
            Some("l".repeat(PEER_ENDPOINT_MAX_BYTES + 1)),
            false,
            &node_store_path,
            &node_blob_path,
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        {
            Ok(_) => panic!("expected oversized listen endpoint to fail"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("mirror listen endpoint is too large")
        );
    }

    #[tokio::test]
    async fn mirror_server_rejects_oversized_store_paths_before_storage_open() {
        let node_dir = tempfile::tempdir().unwrap();
        let oversized_store_path = PathBuf::from("e".repeat(NODE_PATH_MAX_BYTES + 1));
        let oversized_blob_path = PathBuf::from("b".repeat(NODE_PATH_MAX_BYTES + 1));
        let direct_error = match start_mirror_server(
            Some("127.0.0.1:0".to_owned()),
            false,
            &oversized_store_path,
            &node_dir.path().join("blobs"),
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        {
            Ok(_) => panic!("expected oversized store path to fail"),
            Err(error) => error,
        };
        let iroh_error = match start_mirror_server(
            None,
            true,
            &node_dir.path().join("events.db"),
            &oversized_blob_path,
            MAX_ACTIVE_DIRECT_CONNECTIONS,
        )
        .await
        {
            Ok(_) => panic!("expected oversized blob path to fail"),
            Err(error) => error,
        };

        assert!(
            direct_error
                .to_string()
                .contains("event store path is too large")
        );
        assert!(
            iroh_error
                .to_string()
                .contains("blob store path is too large")
        );
        assert!(!node_dir.path().join("events.db").exists());
    }

    #[tokio::test]
    async fn mirror_server_rejects_unsupported_direct_listen_endpoint() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_blob_path = node_dir.path().join("blobs");
        for listen in [
            "https://central.example.invalid/sync",
            "relay://relay.example.invalid/device",
            "direct+tcp://127.0.0.1:0",
            "127.0.0.1:not-a-port",
        ] {
            let error = match start_mirror_server(
                Some(listen.to_owned()),
                false,
                &node_store_path,
                &node_blob_path,
                MAX_ACTIVE_DIRECT_CONNECTIONS,
            )
            .await
            {
                Ok(_) => panic!("expected unsupported listen endpoint to fail"),
                Err(error) => error,
            };

            assert!(
                error
                    .to_string()
                    .contains("mirror listen endpoint must be host:port"),
                "unexpected error for {listen}: {error}"
            );
        }
    }

    #[tokio::test]
    async fn mirror_workspace_once_returns_upstream_errors() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let unreachable_peer = unused_direct_endpoint();
        let error = mirror_workspace(
            node_store,
            node_blob_store,
            WorkspaceId::new(),
            vec![unreachable_peer.clone()],
            MirrorWorkspaceRunOptions::new(1, true, None),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains(&unreachable_peer));
    }

    #[tokio::test]
    async fn mirror_workspace_once_writes_failure_status_file() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let status_file = node_dir.path().join("status").join("mirror.json");
        let workspace_id = WorkspaceId::new();
        let unreachable_peer = unused_direct_endpoint();
        let error = mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![unreachable_peer.clone()],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap_err();

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert!(error.to_string().contains(&unreachable_peer));
        assert_eq!(status["schemaVersion"].as_i64(), Some(1));
        assert_eq!(
            status["workspaceId"].as_str(),
            Some(workspace_id.0.as_str())
        );
        assert_eq!(status["lastResult"].as_str(), Some("failed"));
        assert_eq!(status["health"].as_str(), Some("unreachable"));
        assert_eq!(status["partial"].as_bool(), Some(true));
        assert!(status["checkedAtUnixMs"].as_u64().unwrap() > 0);
        assert!(
            status["lastError"]
                .as_str()
                .unwrap()
                .contains(&unreachable_peer)
        );
        assert_eq!(
            status["configuredPeers"][0].as_str(),
            Some(unreachable_peer.as_str())
        );
        assert_eq!(
            status["peerFailures"][0]["peerEndpoint"].as_str(),
            Some(unreachable_peer.as_str())
        );
        assert_eq!(status["storageHealth"]["totalEventCount"].as_u64(), Some(0));
        assert_eq!(
            status["storageHealth"]["corruptEventCount"].as_u64(),
            Some(0)
        );
        assert!(status["lastReport"].is_null());
    }

    #[tokio::test]
    async fn mirror_workspace_once_uses_fallback_peer_after_upstream_error() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        remote_store.append_event(&root).unwrap();
        let remote_server = DirectPeerServer::bind("127.0.0.1:0", remote_store)
            .await
            .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");
        let unreachable_peer = unused_direct_endpoint();

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![unreachable_peer.clone(), remote_endpoint.clone()],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let mirrored = EventStore::open(&node_store_path)
            .unwrap()
            .list_events_for_workspace(&workspace_id.0)
            .unwrap();

        assert_eq!(mirrored, vec![root]);

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["schemaVersion"].as_i64(), Some(1));
        assert_eq!(
            status["workspaceId"].as_str(),
            Some(workspace_id.0.as_str())
        );
        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["partial"].as_bool(), Some(false));
        assert!(status["checkedAtUnixMs"].as_u64().unwrap() > 0);
        assert_eq!(
            status["lastSuccessfulPeer"].as_str(),
            Some(remote_endpoint.as_str())
        );
        assert_eq!(
            status["configuredPeers"][0].as_str(),
            Some(unreachable_peer.as_str())
        );
        assert_eq!(
            status["configuredPeers"][1].as_str(),
            Some(remote_endpoint.as_str())
        );
        assert_eq!(
            status["peerFailures"][0]["peerEndpoint"].as_str(),
            Some(unreachable_peer.as_str())
        );
        assert_eq!(
            status["lastReport"]["requestedEventCount"].as_u64(),
            Some(1)
        );
        assert_eq!(
            status["lastReport"]["successfulPeerCount"].as_u64(),
            Some(1)
        );
        assert_eq!(status["lastReport"]["fetchedEventCount"].as_u64(), Some(1));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(0));
        assert_eq!(status["storageHealth"]["totalEventCount"].as_u64(), Some(1));
        assert_eq!(
            status["storageHealth"]["corruptEventCount"].as_u64(),
            Some(0)
        );
        assert_eq!(
            status["storageHealth"]["poisonedServableMetadataCount"].as_u64(),
            Some(0)
        );

        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_merges_reachable_peers_to_fill_missing_blobs() {
        let event_peer_store = EventStore::open_in_memory().unwrap();
        let event_peer_dir = tempfile::tempdir().unwrap();
        let event_peer_blobs = BlobStore::open(event_peer_dir.path().join("blobs")).unwrap();
        let blob_peer_store = EventStore::open_in_memory().unwrap();
        let blob_peer_dir = tempfile::tempdir().unwrap();
        let blob_peer_blobs = BlobStore::open(blob_peer_dir.path().join("blobs")).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"blob from a different backup peer",
        );
        let descriptor = blob_peer_blobs.put_bytes(&blob_bytes).unwrap();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    &descriptor.hash,
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        for event in [&root, &channel, &message] {
            event_peer_store.append_event(event).unwrap();
        }

        let event_peer_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", event_peer_store, event_peer_blobs)
                .await
                .unwrap();
        let event_peer_endpoint = event_peer_server.local_addr().unwrap().to_string();
        let (event_peer_shutdown_tx, event_peer_shutdown_rx) = oneshot::channel();
        let event_peer_task = tokio::spawn(async move {
            event_peer_server
                .serve_until_shutdown(event_peer_shutdown_rx)
                .await
        });
        let blob_peer_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", blob_peer_store, blob_peer_blobs)
                .await
                .unwrap();
        let blob_peer_endpoint = blob_peer_server.local_addr().unwrap().to_string();
        let (blob_peer_shutdown_tx, blob_peer_shutdown_rx) = oneshot::channel();
        let blob_peer_task = tokio::spawn(async move {
            blob_peer_server
                .serve_until_shutdown(blob_peer_shutdown_rx)
                .await
        });

        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_path = node_dir.path().join("blobs");
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![event_peer_endpoint.clone(), blob_peer_endpoint.clone()],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let mirrored_blobs = BlobStore::open(&node_blob_path).unwrap();
        assert_eq!(
            mirrored_blobs.get_bytes(&descriptor.hash).unwrap(),
            Some(blob_bytes)
        );

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["partial"].as_bool(), Some(false));
        assert_eq!(
            status["lastSuccessfulPeer"].as_str(),
            Some(blob_peer_endpoint.as_str())
        );
        assert_eq!(
            status["lastReport"]["successfulPeerCount"].as_u64(),
            Some(2)
        );
        assert_eq!(
            status["lastReport"]["requestedEventCount"].as_u64(),
            Some(3)
        );
        assert_eq!(status["lastReport"]["fetchedEventCount"].as_u64(), Some(3));
        assert_eq!(status["lastReport"]["fetchedBlobCount"].as_u64(), Some(1));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(0));
        assert_eq!(status["peerFailures"].as_array().unwrap().len(), 0);

        event_peer_shutdown_tx.send(()).unwrap();
        blob_peer_shutdown_tx.send(()).unwrap();
        event_peer_task.await.unwrap().unwrap();
        blob_peer_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_discovers_backup_peer_hint_to_fill_missing_blobs() {
        let event_peer_store = EventStore::open_in_memory().unwrap();
        let event_peer_dir = tempfile::tempdir().unwrap();
        let event_peer_blobs = BlobStore::open(event_peer_dir.path().join("blobs")).unwrap();
        let blob_peer_store = EventStore::open_in_memory().unwrap();
        let blob_peer_dir = tempfile::tempdir().unwrap();
        let blob_peer_blobs = BlobStore::open(blob_peer_dir.path().join("blobs")).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"blob from a signed discovered backup peer",
        );
        let descriptor = blob_peer_blobs.put_bytes(&blob_bytes).unwrap();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    &descriptor.hash,
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);

        let blob_peer_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", blob_peer_store, blob_peer_blobs)
                .await
                .unwrap();
        let blob_peer_endpoint = blob_peer_server.local_addr().unwrap().to_string();
        let (blob_peer_shutdown_tx, blob_peer_shutdown_rx) = oneshot::channel();
        let blob_peer_task = tokio::spawn(async move {
            blob_peer_server
                .serve_until_shutdown(blob_peer_shutdown_rx)
                .await
        });
        let mut endpoint_hint = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "backup-node".to_owned(),
                endpoint: blob_peer_endpoint.clone(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        );
        endpoint_hint.parents = vec![root.event_id.clone()];
        let endpoint_hint = owner.sign_event(endpoint_hint);
        for event in [&root, &channel, &message, &endpoint_hint] {
            event_peer_store.append_event(event).unwrap();
        }
        let event_peer_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", event_peer_store, event_peer_blobs)
                .await
                .unwrap();
        let event_peer_endpoint = event_peer_server.local_addr().unwrap().to_string();
        let (event_peer_shutdown_tx, event_peer_shutdown_rx) = oneshot::channel();
        let event_peer_task = tokio::spawn(async move {
            event_peer_server
                .serve_until_shutdown(event_peer_shutdown_rx)
                .await
        });

        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_path = node_dir.path().join("blobs");
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![event_peer_endpoint.clone()],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let mirrored_blobs = BlobStore::open(&node_blob_path).unwrap();
        assert_eq!(
            mirrored_blobs.get_bytes(&descriptor.hash).unwrap(),
            Some(blob_bytes)
        );

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(
            status["configuredPeers"][0].as_str(),
            Some(event_peer_endpoint.as_str())
        );
        assert_eq!(
            status["discoveredPeers"][0].as_str(),
            Some(blob_peer_endpoint.as_str())
        );
        assert_eq!(status["activePeers"].as_array().unwrap().len(), 2);
        assert_eq!(
            status["lastReport"]["successfulPeerCount"].as_u64(),
            Some(2)
        );
        assert_eq!(status["lastReport"]["fetchedBlobCount"].as_u64(), Some(1));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));

        event_peer_shutdown_tx.send(()).unwrap();
        blob_peer_shutdown_tx.send(()).unwrap();
        event_peer_task.await.unwrap().unwrap();
        blob_peer_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_status_names_missing_blobs() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let remote_dir = tempfile::tempdir().unwrap();
        let remote_blob_store = BlobStore::open(remote_dir.path().join("blobs")).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"attachment referenced by events but absent from the peer",
        );
        let descriptor = chaft_media::describe_blob(&blob_bytes, 7);
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    &descriptor.hash,
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        for event in [&root, &channel, &message] {
            remote_store.append_event(event).unwrap();
        }
        let remote_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", remote_store, remote_blob_store)
                .await
                .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id,
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("partial"));
        assert_eq!(status["partial"].as_bool(), Some(true));
        assert_eq!(status["lastReport"]["fetchedBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(1));
        assert_eq!(
            status["lastReport"]["missingBlobHashes"][0].as_str(),
            Some(descriptor.hash.as_str())
        );
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gaps"].as_array().unwrap().len(), 0);

        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_status_names_materialization_gaps() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let remote_dir = tempfile::tempdir().unwrap();
        let remote_blob_store = BlobStore::open(remote_dir.path().join("blobs")).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(&workspace_id, &channel_id, message_id, None),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        remote_store.append_event(&message).unwrap();
        let remote_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", remote_store, remote_blob_store)
                .await
                .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id,
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("partial"));
        assert_eq!(status["partial"].as_bool(), Some(true));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(1));
        assert_eq!(
            status["lastReport"]["gaps"][0]["eventId"].as_str(),
            Some(message.event_id.0.as_str())
        );
        assert_eq!(
            status["lastReport"]["gaps"][0]["missingParentIds"][0].as_str(),
            Some(channel.event_id.0.as_str())
        );

        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_merges_reachable_peers_to_fill_materialization_gaps() {
        let gap_peer_store = EventStore::open_in_memory().unwrap();
        let parent_peer_store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(&workspace_id, &channel_id, message_id, None),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        gap_peer_store.append_event(&message).unwrap();
        for event in [&root, &channel] {
            parent_peer_store.append_event(event).unwrap();
        }

        let gap_peer_server = DirectPeerServer::bind("127.0.0.1:0", gap_peer_store)
            .await
            .unwrap();
        let gap_peer_endpoint = gap_peer_server.local_addr().unwrap().to_string();
        let (gap_peer_shutdown_tx, gap_peer_shutdown_rx) = oneshot::channel();
        let gap_peer_task = tokio::spawn(async move {
            gap_peer_server
                .serve_until_shutdown(gap_peer_shutdown_rx)
                .await
        });
        let parent_peer_server = DirectPeerServer::bind("127.0.0.1:0", parent_peer_store)
            .await
            .unwrap();
        let parent_peer_endpoint = parent_peer_server.local_addr().unwrap().to_string();
        let (parent_peer_shutdown_tx, parent_peer_shutdown_rx) = oneshot::channel();
        let parent_peer_task = tokio::spawn(async move {
            parent_peer_server
                .serve_until_shutdown(parent_peer_shutdown_rx)
                .await
        });

        let node_dir = tempfile::tempdir().unwrap();
        let node_store_path = node_dir.path().join("events.db");
        let node_store = EventStore::open(&node_store_path).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id.clone(),
            vec![gap_peer_endpoint, parent_peer_endpoint.clone()],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let mirrored = EventStore::open(&node_store_path)
            .unwrap()
            .list_events_for_workspace(&workspace_id.0)
            .unwrap();
        assert_eq!(mirrored.len(), 3);
        assert!(mirrored.contains(&root));
        assert!(mirrored.contains(&channel));
        assert!(mirrored.contains(&message));

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["partial"].as_bool(), Some(false));
        assert_eq!(
            status["lastSuccessfulPeer"].as_str(),
            Some(parent_peer_endpoint.as_str())
        );
        assert_eq!(
            status["lastReport"]["successfulPeerCount"].as_u64(),
            Some(2)
        );
        assert_eq!(
            status["lastReport"]["requestedEventCount"].as_u64(),
            Some(3)
        );
        assert_eq!(status["lastReport"]["fetchedEventCount"].as_u64(), Some(3));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["gapCount"].as_u64(), Some(0));

        gap_peer_shutdown_tx.send(()).unwrap();
        parent_peer_shutdown_tx.send(()).unwrap();
        gap_peer_task.await.unwrap().unwrap();
        parent_peer_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_periodic_retries_after_upstream_error() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let unreachable_peer = unused_direct_endpoint();
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            mirror_workspace(
                node_store,
                node_blob_store,
                WorkspaceId::new(),
                vec![unreachable_peer],
                MirrorWorkspaceRunOptions::new(1, false, None),
            ),
        )
        .await;

        assert!(
            result.is_err(),
            "periodic mirror exited after a transient error"
        );
    }

    #[tokio::test]
    async fn mirror_workspace_periodic_exits_on_shutdown() {
        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_store = BlobStore::open(node_dir.path().join("blobs")).unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        shutdown_tx.send(()).unwrap();
        let unreachable_peer = unused_direct_endpoint();

        let result = tokio::time::timeout(
            Duration::from_millis(100),
            mirror_workspace_until_shutdown(
                node_store,
                node_blob_store,
                WorkspaceId::new(),
                vec![unreachable_peer],
                MirrorWorkspaceRunOptions::new(60, false, None),
                async {
                    let _ = shutdown_rx.await;
                },
            ),
        )
        .await
        .unwrap();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mirror_workspace_fetches_chunked_attachment_blobs() {
        let remote_dir = tempfile::tempdir().unwrap();
        let remote_store = EventStore::open_in_memory().unwrap();
        let remote_blob_store = BlobStore::open(remote_dir.path().join("blobs")).unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"chunked backup attachment",
        );
        let descriptor = remote_blob_store.put_bytes_chunked(&blob_bytes, 7).unwrap();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    &descriptor.hash,
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        for event in [&root, &channel, &message] {
            remote_store.append_event(event).unwrap();
        }
        let remote_server =
            DirectPeerServer::bind_with_blobs("127.0.0.1:0", remote_store, remote_blob_store)
                .await
                .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );
        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_path = node_dir.path().join("blobs");
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id,
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, None),
        )
        .await
        .unwrap();

        let mirrored_blobs = BlobStore::open(&node_blob_path).unwrap();

        assert_eq!(
            mirrored_blobs.get_bytes(&descriptor.hash).unwrap(),
            Some(blob_bytes)
        );

        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn mirror_workspace_treats_complete_local_chunks_as_available() {
        let remote_store = EventStore::open_in_memory().unwrap();
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceIdentity::generate();
        let root = workspace_root(&owner, workspace_id.clone());
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        );
        channel.parents = vec![root.event_id.clone()];
        let channel = owner.sign_event(channel);
        let message_id = MessageId::new();
        let (blob_bytes, blob_encryption) = sealed_attachment_fixture(
            &workspace_id,
            &channel_id,
            &message_id,
            b"already mirrored as chunks",
        );
        let descriptor = chaft_media::describe_blob(&blob_bytes, 7);
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.device_id().clone(),
            encrypted_message_body(
                &workspace_id,
                &channel_id,
                message_id,
                Some(encrypted_attachment_ref(
                    &descriptor.hash,
                    blob_bytes.len() as u64,
                    blob_encryption,
                )),
            ),
        );
        message.parents = vec![channel.event_id.clone()];
        let message = owner.sign_event(message);
        for event in [&root, &channel, &message] {
            remote_store.append_event(event).unwrap();
        }
        let remote_server = DirectPeerServer::bind("127.0.0.1:0", remote_store)
            .await
            .unwrap();
        let remote_endpoint = remote_server.local_addr().unwrap().to_string();
        let (remote_shutdown_tx, remote_shutdown_rx) = oneshot::channel();
        let remote_task =
            tokio::spawn(
                async move { remote_server.serve_until_shutdown(remote_shutdown_rx).await },
            );

        let node_dir = tempfile::tempdir().unwrap();
        let node_store = EventStore::open(node_dir.path().join("events.db")).unwrap();
        let node_blob_path = node_dir.path().join("blobs");
        let node_blob_store = BlobStore::open(&node_blob_path).unwrap();
        node_blob_store.put_bytes_chunked(&blob_bytes, 7).unwrap();
        assert!(!node_blob_store.has_blob(&descriptor.hash).unwrap());
        assert!(node_blob_store.has_complete_blob(&descriptor.hash).unwrap());
        let status_file = node_dir.path().join("mirror-status.json");

        mirror_workspace(
            node_store,
            node_blob_store,
            workspace_id,
            vec![remote_endpoint],
            MirrorWorkspaceRunOptions::new(1, true, Some(status_file.clone())),
        )
        .await
        .unwrap();

        let status: serde_json::Value =
            serde_json::from_slice(&fs::read(status_file).unwrap()).unwrap();
        let mirrored_blobs = BlobStore::open(&node_blob_path).unwrap();

        assert_eq!(status["lastResult"].as_str(), Some("success"));
        assert_eq!(status["health"].as_str(), Some("healthy"));
        assert_eq!(status["lastReport"]["fetchedBlobCount"].as_u64(), Some(0));
        assert_eq!(status["lastReport"]["missingBlobCount"].as_u64(), Some(0));
        assert!(!mirrored_blobs.has_blob(&descriptor.hash).unwrap());
        assert_eq!(
            mirrored_blobs.get_bytes_chunked(&descriptor.hash).unwrap(),
            Some(blob_bytes)
        );

        remote_shutdown_tx.send(()).unwrap();
        remote_task.await.unwrap().unwrap();
    }
}
