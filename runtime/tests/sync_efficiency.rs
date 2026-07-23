use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use chaft_identity::DeviceIdentity;
use chaft_media::{BlobAvailability, BlobDescriptor, BlobStore};
use chaft_net::{ChaftTransport, NetError, PeerAddress, PeerId};
use chaft_net_direct::{
    AuthorizedPublishTransport, BlobSyncTransport, DirectPeerServer, DirectTransport,
};
use chaft_runtime::LocalRuntime;
use chaft_store::EventStore;
use chaft_types::{
    ChannelId, EventBody, EventId, MessageId, SignableEvent, SignedEvent, SignedTrustSnapshot,
    WorkspaceId, WorkspaceRole,
};
use tokio::sync::oneshot;

#[derive(Clone, Default)]
struct CountingDirectTransport {
    inventory_fetch_count: Arc<AtomicUsize>,
    direct: DirectTransport,
}

impl CountingDirectTransport {
    fn inventory_fetch_count(&self) -> usize {
        self.inventory_fetch_count.load(Ordering::SeqCst)
    }

    fn reset_inventory_fetch_count(&self) {
        self.inventory_fetch_count.store(0, Ordering::SeqCst);
    }
}

#[async_trait]
impl ChaftTransport for CountingDirectTransport {
    async fn connect(&self, peer: PeerAddress) -> Result<(), NetError> {
        self.direct.connect(peer).await
    }

    async fn fetch_inventory(&self, peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        self.inventory_fetch_count.fetch_add(1, Ordering::SeqCst);
        self.direct.fetch_inventory(peer).await
    }

    async fn fetch_workspace_inventory(
        &self,
        peer: &PeerAddress,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        self.inventory_fetch_count.fetch_add(1, Ordering::SeqCst);
        self.direct
            .fetch_workspace_inventory(peer, workspace_id)
            .await
    }

    async fn publish_event(&self, peer: &PeerAddress, event: SignedEvent) -> Result<(), NetError> {
        self.direct.publish_event(peer, event).await
    }

    async fn fetch_events(
        &self,
        peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        self.direct.fetch_events(peer, event_ids).await
    }
}

#[async_trait]
impl AuthorizedPublishTransport for CountingDirectTransport {
    async fn publish_events_with_authorization(
        &self,
        peer: &PeerAddress,
        events: Vec<SignedEvent>,
        authorization_events: Vec<SignedEvent>,
        authorization_snapshots: Vec<SignedTrustSnapshot>,
    ) -> Result<(), NetError> {
        self.direct
            .publish_events_with_authorization(
                peer,
                events,
                authorization_events,
                authorization_snapshots,
            )
            .await
    }
}

#[async_trait]
impl BlobSyncTransport for CountingDirectTransport {
    async fn put_blobs(
        &self,
        peer: &PeerAddress,
        blobs: Vec<Vec<u8>>,
    ) -> Result<Vec<String>, NetError> {
        self.direct.put_blobs(peer, blobs).await
    }

    async fn fetch_blobs(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, Vec<u8>>, NetError> {
        self.direct.fetch_blobs(peer, hashes).await
    }

    async fn fetch_blob_availabilities(
        &self,
        peer: &PeerAddress,
        hashes: Vec<String>,
    ) -> Result<HashMap<String, BlobAvailability>, NetError> {
        self.direct.fetch_blob_availabilities(peer, hashes).await
    }

    async fn put_blob_chunked(
        &self,
        peer: &PeerAddress,
        bytes: Vec<u8>,
        chunk_size: usize,
    ) -> Result<BlobDescriptor, NetError> {
        self.direct.put_blob_chunked(peer, bytes, chunk_size).await
    }

    async fn fetch_blob_chunked(
        &self,
        peer: &PeerAddress,
        hash: &str,
    ) -> Result<Option<Vec<u8>>, NetError> {
        self.direct.fetch_blob_chunked(peer, hash).await
    }
}

async fn direct_peer_for_store(
    store: EventStore,
    peer_id: &str,
) -> (
    PeerAddress,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<Result<(), NetError>>,
) {
    let server = DirectPeerServer::bind("127.0.0.1:0", store)
        .await
        .expect("bind direct peer");
    let peer = PeerAddress {
        peer_id: PeerId(peer_id.to_owned()),
        endpoint: server.local_addr().expect("peer address").to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    (peer, shutdown_tx, server_task)
}

#[tokio::test]
async fn direct_sync_fetches_remote_inventory_once_and_self_heals_corrupt_repair_cache() {
    let source_dir = tempfile::tempdir().expect("source temp dir");
    let peer_dir = tempfile::tempdir().expect("peer temp dir");
    let source = LocalRuntime::open(source_dir.path(), None).expect("open source runtime");
    let created = source
        .create_workspace("Inventory Count", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let (peer, shutdown_tx, server_task) = direct_peer_for_store(
        EventStore::open(peer_dir.path().join("events.db")).expect("open peer store"),
        "counted-peer",
    )
    .await;
    let transport = CountingDirectTransport::default();

    let delta = source
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("delta sync");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(!delta.published.published_event_ids.is_empty());

    let repair_cache_path = source
        .paths()
        .data_dir
        .join("inbound-blob-repair-ledger.json");
    std::fs::write(&repair_cache_path, br#"{"schemaVersion":1,"workspaces":["#)
        .expect("corrupt derived repair cache");

    transport.reset_inventory_fetch_count();
    let no_change = source
        .sync_workspace_direct(&transport, &peer, workspace_id)
        .await
        .expect("no-change sync");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(no_change.published.published_event_ids.is_empty());
    assert!(no_change.pulled.requested_event_ids.is_empty());
    assert!(no_change.pulled.fetched_event_ids.is_empty());
    assert!(no_change.pulled.applied_event_ids.is_empty());
    let repaired_cache: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repair_cache_path).expect("read self-healed repair cache"),
    )
    .expect("repair cache was rewritten as valid JSON");
    assert_eq!(repaired_cache["schemaVersion"], 1);

    shutdown_tx.send(()).expect("stop direct peer");
    server_task
        .await
        .expect("join direct peer")
        .expect("serve direct peer");
}

#[tokio::test]
async fn no_change_sync_keeps_persistent_gap_health_after_cache_corruption_and_restart() {
    let target_dir = tempfile::tempdir().expect("target temp dir");
    let peer_dir = tempfile::tempdir().expect("peer temp dir");
    let author = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let missing_parent_id = EventId("evt_persistent_missing_parent".to_owned());
    let mut gap_event = SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id),
        author.device_id().clone(),
        EventBody::MessageCreated {
            message_id: MessageId::new(),
            markdown: "stored with incomplete history".to_owned(),
            attachments: Vec::new(),
        },
    );
    gap_event.parents = vec![missing_parent_id.clone()];
    let gap_event = author.sign_event(gap_event);
    let peer_store = EventStore::open(peer_dir.path().join("events.db")).expect("open peer store");
    peer_store
        .append_event(&gap_event)
        .expect("append peer gap event");
    let (peer, shutdown_tx, server_task) =
        direct_peer_for_store(peer_store, "persistent-gap-peer").await;
    let transport = CountingDirectTransport::default();
    let target = LocalRuntime::open(target_dir.path(), None).expect("open target runtime");

    let initial = target
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("initial gap sync");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert_eq!(initial.pulled.gap_count, 1);
    assert_eq!(initial.pulled.gaps[0].event_id, gap_event.event_id.0);
    assert_eq!(
        initial.pulled.gaps[0].missing_parent_ids,
        vec![missing_parent_id.0.clone()]
    );

    transport.reset_inventory_fetch_count();
    let cached = target
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("cached no-change gap sync");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(cached.pulled.requested_event_ids.is_empty());
    assert!(cached.pulled.fetched_event_ids.is_empty());
    assert!(cached.pulled.applied_event_ids.is_empty());
    assert_eq!(cached.pulled.gaps, initial.pulled.gaps);

    let health_cache_path = target_dir.path().join("materialization-health-cache.json");
    assert!(health_cache_path.is_file());
    std::fs::write(&health_cache_path, br#"{"schemaVersion":1,"workspaces":["#)
        .expect("corrupt derived materialization health cache");
    drop(target);

    let restarted = LocalRuntime::open(target_dir.path(), None).expect("restart target runtime");
    transport.reset_inventory_fetch_count();
    let recovered = restarted
        .sync_workspace_direct(&transport, &peer, workspace_id)
        .await
        .expect("recompute gap health after restart");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(recovered.pulled.requested_event_ids.is_empty());
    assert!(recovered.pulled.fetched_event_ids.is_empty());
    assert!(recovered.pulled.applied_event_ids.is_empty());
    assert_eq!(recovered.pulled.gaps, initial.pulled.gaps);
    let repaired_cache: serde_json::Value = serde_json::from_slice(
        &std::fs::read(health_cache_path).expect("read repaired health cache"),
    )
    .expect("health cache was rewritten as valid JSON");
    assert_eq!(repaired_cache["schemaVersion"], 1);

    shutdown_tx.send(()).expect("stop persistent-gap peer");
    server_task
        .await
        .expect("join persistent-gap peer")
        .expect("serve persistent-gap peer");
}

#[tokio::test]
async fn direct_sync_publishes_events_generated_by_pull_catchup_without_refetching_inventory() {
    let alice_dir = tempfile::tempdir().expect("alice temp dir");
    let bob_dir = tempfile::tempdir().expect("bob temp dir");
    let alice = LocalRuntime::open(alice_dir.path(), None).expect("open alice runtime");
    let bob = LocalRuntime::open(bob_dir.path(), None).expect("open bob runtime");
    let created = alice
        .create_workspace("Catch-up Follow-up", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);

    alice
        .create_openmls_workspace_group(workspace_id.clone())
        .expect("create workspace group");
    alice
        .invite_member(
            workspace_id.clone(),
            bob.device_id().clone(),
            WorkspaceRole::Member,
        )
        .expect("invite bob");
    let bob_store = EventStore::open(bob.paths().event_store.clone()).expect("open bob store");
    for event in alice
        .workspace_events(&workspace_id)
        .expect("read alice events")
    {
        bob_store.append_event(&event).expect("copy event to bob");
    }
    bob.publish_openmls_device_key_package(workspace_id.clone())
        .expect("publish bob key package");

    let (peer, shutdown_tx, server_task) = direct_peer_for_store(
        EventStore::open(bob.paths().event_store.clone()).expect("open bob peer store"),
        "bob",
    )
    .await;
    let transport = CountingDirectTransport::default();
    let synced = alice
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("sync catch-up");

    let provisioned_event_id = synced
        .pulled
        .openmls_catchup
        .workspace_provisioned_event_ids
        .first()
        .expect("catch-up provisioned event");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(
        synced
            .published
            .published_event_ids
            .contains(provisioned_event_id)
    );

    shutdown_tx.send(()).expect("stop bob peer");
    server_task
        .await
        .expect("join bob peer")
        .expect("serve bob peer");
}

#[tokio::test]
async fn no_change_sync_retries_a_persisted_missing_inbound_blob() {
    let source_dir = tempfile::tempdir().expect("source temp dir");
    let target_dir = tempfile::tempdir().expect("target temp dir");
    let peer_dir = tempfile::tempdir().expect("peer temp dir");
    let attachment_path = source_dir.path().join("attachment.txt");
    std::fs::write(&attachment_path, b"eventual attachment repair")
        .expect("write attachment fixture");
    let source =
        LocalRuntime::open(source_dir.path().join("runtime"), None).expect("open source runtime");
    let target = LocalRuntime::open(target_dir.path(), None).expect("open target runtime");
    let created = source
        .create_workspace("Inbound Repair", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let sent = source
        .send_message_with_attachment_file(
            workspace_id.clone(),
            ChannelId(created.channel_id),
            "attachment repair",
            &attachment_path,
            "text/plain",
        )
        .expect("send attachment message");
    let attachment = source
        .workspace_events(&workspace_id)
        .expect("read source events")
        .into_iter()
        .find_map(|event| {
            if event.event_id.0 != sent.event_id {
                return None;
            }
            match event.event.body {
                EventBody::MessageCreatedEncrypted { attachments, .. } => {
                    attachments.into_iter().next()
                }
                _ => None,
            }
        })
        .expect("attachment event");
    let source_blobs =
        BlobStore::open(source.paths().blob_store.clone()).expect("open source blob store");
    let attachment_bytes = source_blobs
        .get_complete_bytes(&attachment.blob_hash)
        .expect("read source blob")
        .expect("source blob exists");

    let peer_store = EventStore::open(peer_dir.path().join("events.db")).expect("open peer store");
    for event in source
        .workspace_events(&workspace_id)
        .expect("read source events")
    {
        peer_store.append_event(&event).expect("copy peer event");
    }
    let peer_blobs = BlobStore::open(peer_dir.path().join("blobs")).expect("open peer blobs");
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", peer_store, peer_blobs.clone())
        .await
        .expect("bind blob peer");
    let peer = PeerAddress {
        peer_id: PeerId("blob-peer".to_owned()),
        endpoint: server.local_addr().expect("peer address").to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = CountingDirectTransport::default();

    let interrupted = target
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("initial event sync");
    assert_eq!(
        interrupted.pulled.missing_blob_hashes,
        vec![attachment.blob_hash.clone()]
    );

    peer_blobs
        .put_bytes_with_hash(&attachment.blob_hash, &attachment_bytes)
        .expect("make peer blob available");
    transport.reset_inventory_fetch_count();
    let repaired = target
        .sync_workspace_direct(&transport, &peer, workspace_id)
        .await
        .expect("no-change repair sync");

    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(repaired.pulled.fetched_event_ids.is_empty());
    assert_eq!(
        repaired.pulled.fetched_blob_hashes,
        vec![attachment.blob_hash.clone()]
    );
    assert!(
        BlobStore::open(target.paths().blob_store.clone())
            .expect("open target blobs")
            .has_complete_blob(&attachment.blob_hash)
            .expect("check target blob")
    );

    shutdown_tx.send(()).expect("stop blob peer");
    server_task
        .await
        .expect("join blob peer")
        .expect("serve blob peer");
}

#[tokio::test]
async fn no_change_sync_repairs_a_peer_blob_after_outbound_ledger_corruption() {
    let source_dir = tempfile::tempdir().expect("source temp dir");
    let peer_dir = tempfile::tempdir().expect("peer temp dir");
    let attachment_bytes = b"persistent outbound attachment repair";
    let attachment_path = source_dir.path().join("outbound-repair.txt");
    std::fs::write(&attachment_path, attachment_bytes).expect("write attachment fixture");

    let source =
        LocalRuntime::open(source_dir.path().join("runtime"), None).expect("open source runtime");
    let created = source
        .create_workspace("Outbound Repair", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let sent = source
        .send_message_with_attachment_file(
            workspace_id.clone(),
            ChannelId(created.channel_id),
            "outbound repair",
            &attachment_path,
            "text/plain",
        )
        .expect("send attachment message");
    let attachment = source
        .workspace_events(&workspace_id)
        .expect("read source events")
        .into_iter()
        .find_map(|event| {
            if event.event_id.0 != sent.event_id {
                return None;
            }
            match event.event.body {
                EventBody::MessageCreatedEncrypted { attachments, .. } => {
                    attachments.into_iter().next()
                }
                _ => None,
            }
        })
        .expect("attachment event");
    let stored_attachment_bytes = BlobStore::open(source.paths().blob_store.clone())
        .expect("open source blobs")
        .get_complete_bytes(&attachment.blob_hash)
        .expect("read source attachment blob")
        .expect("source attachment blob exists");

    let peer_store_path = peer_dir.path().join("events.db");
    let peer_blob_root = peer_dir.path().join("blobs");
    let peer_blobs = BlobStore::open(&peer_blob_root).expect("open peer blobs");
    let server = DirectPeerServer::bind_with_blobs(
        "127.0.0.1:0",
        EventStore::open(&peer_store_path).expect("open peer store"),
        peer_blobs.clone(),
    )
    .await
    .expect("bind blob peer");
    let peer = PeerAddress {
        peer_id: PeerId("outbound-repair-peer".to_owned()),
        endpoint: server.local_addr().expect("peer address").to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = CountingDirectTransport::default();

    let initial = source
        .sync_workspace_direct(&transport, &peer, workspace_id.clone())
        .await
        .expect("initial attachment sync");
    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(
        initial
            .published
            .published_event_ids
            .contains(&sent.event_id)
    );
    assert_eq!(
        initial.published.published_blob_hashes,
        vec![attachment.blob_hash.clone()]
    );
    assert!(
        peer_blobs
            .has_complete_blob(&attachment.blob_hash)
            .expect("check initial peer blob")
    );

    let peer_blob_path = peer_blob_root
        .join(&attachment.blob_hash[..2])
        .join(&attachment.blob_hash);
    std::fs::remove_file(&peer_blob_path).expect("remove peer blob only");
    assert!(
        !peer_blobs
            .has_complete_blob(&attachment.blob_hash)
            .expect("check removed peer blob")
    );
    assert!(
        EventStore::open(&peer_store_path)
            .expect("reopen peer store")
            .get_event(&EventId(sent.event_id.clone()))
            .expect("read peer event")
            .is_some(),
        "attachment event must remain while its peer blob is missing"
    );

    let repair_ledger_path = source
        .paths()
        .data_dir
        .join("outbound-blob-repair-ledger.json");
    assert!(repair_ledger_path.is_file());
    std::fs::write(&repair_ledger_path, br#"{"schemaVersion":1,"peers":["#)
        .expect("corrupt derived outbound repair ledger");

    transport.reset_inventory_fetch_count();
    let repaired = source
        .sync_workspace_direct(&transport, &peer, workspace_id)
        .await
        .expect("no-change outbound repair sync");

    assert_eq!(transport.inventory_fetch_count(), 1);
    assert!(repaired.published.published_event_ids.is_empty());
    assert!(repaired.pulled.fetched_event_ids.is_empty());
    assert_eq!(
        repaired.published.published_blob_hashes,
        vec![attachment.blob_hash.clone()]
    );
    assert_eq!(
        peer_blobs
            .get_complete_bytes(&attachment.blob_hash)
            .expect("read repaired peer blob")
            .expect("repaired peer blob exists"),
        stored_attachment_bytes
    );
    let repaired_ledger: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repair_ledger_path).expect("read repaired outbound ledger"),
    )
    .expect("outbound repair ledger was rewritten as valid JSON");
    assert_eq!(repaired_ledger["schemaVersion"], 1);

    shutdown_tx.send(()).expect("stop blob peer");
    server_task
        .await
        .expect("join blob peer")
        .expect("serve blob peer");
}
