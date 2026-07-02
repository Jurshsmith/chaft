use async_trait::async_trait;
use chaft_core::WorkspaceState;
use chaft_crypto::{ContentKey, seal_message_markdown};
use chaft_identity::DeviceIdentity;
use chaft_net::{ChaftTransport, NetError, PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_store::EventStore;
use chaft_sync::{
    SyncError, events_missing_from_local, inventory_from_events, pull_workspace_from_peer,
};
use chaft_types::{
    ChannelId, EventBody, EventId, MessageId, SignableEvent, SignedEvent, WorkspaceId,
};
use std::path::Path;
use tokio::sync::oneshot;

struct ReversedFetchTransport {
    events: Vec<SignedEvent>,
}

#[async_trait]
impl ChaftTransport for ReversedFetchTransport {
    async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        Ok(self
            .events
            .iter()
            .map(|event| event.event_id.clone())
            .collect())
    }

    async fn publish_event(
        &self,
        _peer: &PeerAddress,
        _event: SignedEvent,
    ) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_events(
        &self,
        _peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        Ok(self
            .events
            .iter()
            .rev()
            .filter(|event| event_ids.contains(&event.event_id))
            .cloned()
            .collect())
    }
}

struct WorkspaceOnlyInventoryTransport {
    events: Vec<SignedEvent>,
}

#[async_trait]
impl ChaftTransport for WorkspaceOnlyInventoryTransport {
    async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        Err(NetError::Protocol(
            "full-peer inventory should not be used for workspace pull".to_owned(),
        ))
    }

    async fn fetch_workspace_inventory(
        &self,
        _peer: &PeerAddress,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        Ok(self
            .events
            .iter()
            .filter(|event| event.event.workspace_id == *workspace_id)
            .map(|event| event.event_id.clone())
            .collect())
    }

    async fn publish_event(
        &self,
        _peer: &PeerAddress,
        _event: SignedEvent,
    ) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_events(
        &self,
        _peer: &PeerAddress,
        event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        Ok(self
            .events
            .iter()
            .filter(|event| event_ids.contains(&event.event_id))
            .cloned()
            .collect())
    }
}

struct ScriptedPullTransport {
    inventory: Vec<EventId>,
    fetched_events: Vec<SignedEvent>,
}

#[async_trait]
impl ChaftTransport for ScriptedPullTransport {
    async fn connect(&self, _peer: PeerAddress) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_inventory(&self, _peer: &PeerAddress) -> Result<Vec<EventId>, NetError> {
        Ok(self.inventory.clone())
    }

    async fn fetch_workspace_inventory(
        &self,
        _peer: &PeerAddress,
        _workspace_id: &WorkspaceId,
    ) -> Result<Vec<EventId>, NetError> {
        Ok(self.inventory.clone())
    }

    async fn publish_event(
        &self,
        _peer: &PeerAddress,
        _event: SignedEvent,
    ) -> Result<(), NetError> {
        Ok(())
    }

    async fn fetch_events(
        &self,
        _peer: &PeerAddress,
        _event_ids: Vec<EventId>,
    ) -> Result<Vec<SignedEvent>, NetError> {
        Ok(self.fetched_events.clone())
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

fn sync_protocol_error_message(error: SyncError) -> String {
    match error {
        SyncError::Net(NetError::Protocol(message)) => message,
        other => panic!("expected protocol error, got {other:?}"),
    }
}

#[test]
fn two_peers_exchange_missing_events_without_central_server() {
    let alice_store = EventStore::open_in_memory().unwrap();
    let bob_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();

    let root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let channel = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let message = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        alice.device_id().clone(),
        EventBody::MessageCreated {
            message_id: message_id.clone(),
            markdown: "sent over a direct peer sync".to_owned(),
            attachments: Vec::new(),
        },
    ));

    alice_store.append_event(&root).unwrap();
    alice_store.append_event(&channel).unwrap();
    alice_store.append_event(&message).unwrap();

    let bob_inventory = inventory_from_events(&bob_store.list_events().unwrap());
    let alice_events = alice_store.list_events().unwrap();
    let missing_for_bob = events_missing_from_local(&bob_inventory, &alice_events);

    assert_eq!(missing_for_bob.len(), 3);

    for event in &missing_for_bob {
        bob_store.append_event(event).unwrap();
    }

    let mut bob_state = WorkspaceState::new(workspace_id);
    for event in bob_store.list_events().unwrap() {
        bob_state.apply(&event).unwrap();
    }

    assert_eq!(bob_state.channels.len(), 1);
    assert_eq!(
        bob_state.messages[&message_id].markdown,
        "sent over a direct peer sync"
    );
}

#[tokio::test]
async fn pull_workspace_uses_workspace_scoped_inventory_when_available() {
    let local_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let other_workspace_id = WorkspaceId::new();
    let target_event = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Target".to_owned(),
        },
    ));
    let other_event = alice.sign_event(SignableEvent::new(
        other_workspace_id,
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Other".to_owned(),
        },
    ));
    let transport = WorkspaceOnlyInventoryTransport {
        events: vec![target_event.clone(), other_event],
    };
    let peer = PeerAddress {
        peer_id: PeerId("scoped-inventory".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();
    let stored_events = local_store.list_events().unwrap();

    assert_eq!(
        report.requested_event_ids,
        vec![target_event.event_id.clone()]
    );
    assert_eq!(
        report.fetched_event_ids,
        vec![target_event.event_id.clone()]
    );
    assert!(report.ignored_event_ids.is_empty());
    assert_eq!(stored_events.len(), 1);
    assert_eq!(stored_events[0].event_id, target_event.event_id);
}

#[tokio::test]
async fn pull_workspace_rejects_non_canonical_remote_inventory_id() {
    let local_store = EventStore::open_in_memory().unwrap();
    let workspace_id = WorkspaceId::new();
    let transport = ScriptedPullTransport {
        inventory: vec![EventId("evt_NOT_CANONICAL".to_owned())],
        fetched_events: Vec::new(),
    };
    let peer = PeerAddress {
        peer_id: PeerId("bad-inventory-id".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let error = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap_err();

    assert!(sync_protocol_error_message(error).contains("non-canonical inventory event id"));
}

#[tokio::test]
async fn pull_workspace_rejects_duplicate_remote_inventory_id() {
    let local_store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let event = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Duplicate inventory".to_owned(),
        },
    ));
    let transport = ScriptedPullTransport {
        inventory: vec![event.event_id.clone(), event.event_id],
        fetched_events: Vec::new(),
    };
    let peer = PeerAddress {
        peer_id: PeerId("duplicate-inventory-id".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let error = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap_err();

    assert!(sync_protocol_error_message(error).contains("duplicate inventory event id"));
}

#[tokio::test]
async fn pull_workspace_rejects_duplicate_fetched_events() {
    let local_store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let event = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Duplicate fetch".to_owned(),
        },
    ));
    let transport = ScriptedPullTransport {
        inventory: vec![event.event_id.clone()],
        fetched_events: vec![event.clone(), event],
    };
    let peer = PeerAddress {
        peer_id: PeerId("duplicate-fetched-event".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let error = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap_err();

    assert!(sync_protocol_error_message(error).contains("duplicate event"));
}

#[tokio::test]
async fn pull_workspace_rejects_unrequested_fetched_event() {
    let local_store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let requested = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Requested".to_owned(),
        },
    ));
    let unrequested = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: ChannelId::new(),
            name: "unrequested".to_owned(),
            is_private: false,
        },
    ));
    let transport = ScriptedPullTransport {
        inventory: vec![requested.event_id],
        fetched_events: vec![unrequested],
    };
    let peer = PeerAddress {
        peer_id: PeerId("unrequested-fetched-event".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let error = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap_err();

    assert!(sync_protocol_error_message(error).contains("unrequested event"));
}

#[tokio::test]
async fn pull_workspace_imports_out_of_order_fetches_in_materialized_order() {
    let local_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();

    let root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let mut channel_event = SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    );
    channel_event.parents = vec![root.event_id.clone()];
    let channel = alice.sign_event(channel_event);
    let mut message_event = SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        alice.device_id().clone(),
        EventBody::MessageCreated {
            message_id: message_id.clone(),
            markdown: "fetched child before parents".to_owned(),
            attachments: Vec::new(),
        },
    );
    message_event.parents = vec![channel.event_id.clone()];
    let message = alice.sign_event(message_event);
    let expected_event_ids = vec![
        root.event_id.clone(),
        channel.event_id.clone(),
        message.event_id.clone(),
    ];
    let transport = ReversedFetchTransport {
        events: vec![root, channel, message],
    };
    let peer = PeerAddress {
        peer_id: PeerId("reversed".to_owned()),
        endpoint: "memory".to_owned(),
    };

    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();
    let stored_event_ids = local_store
        .list_events()
        .unwrap()
        .into_iter()
        .map(|event| event.event_id)
        .collect::<Vec<_>>();

    assert_eq!(report.fetched_event_ids, expected_event_ids);
    assert_eq!(stored_event_ids, report.fetched_event_ids);
    assert_eq!(
        report.materialization.applied_events,
        report.fetched_event_ids
    );
    assert!(report.materialization.gaps.is_empty());
}

#[tokio::test]
async fn pull_workspace_materialization_ignores_invalid_local_signature_events() {
    let local_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();

    let root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let mut channel_event = SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    );
    channel_event.parents = vec![root.event_id.clone()];
    let channel = alice.sign_event(channel_event);
    let mut forged_message_event = SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id),
        alice.device_id().clone(),
        EventBody::MessageCreated {
            message_id,
            markdown: "forged local sync state".to_owned(),
            attachments: Vec::new(),
        },
    );
    forged_message_event.parents = vec![channel.event_id.clone()];
    let mut forged_message = alice.sign_event(forged_message_event);
    forged_message.signature[0] ^= 1;

    for event in [&root, &channel, &forged_message] {
        local_store.append_event(event).unwrap();
    }

    let transport = ReversedFetchTransport { events: Vec::new() };
    let peer = PeerAddress {
        peer_id: PeerId("empty".to_owned()),
        endpoint: "memory".to_owned(),
    };
    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();

    assert!(report.requested_event_ids.is_empty());
    assert!(report.fetched_event_ids.is_empty());
    assert_eq!(
        report.materialization.applied_events,
        vec![root.event_id, channel.event_id]
    );
    assert!(
        !report
            .materialization
            .applied_events
            .contains(&forged_message.event_id)
    );
    assert!(report.materialization.gaps.is_empty());
}

#[tokio::test]
async fn pull_workspace_materialization_ignores_corrupt_local_event_json() {
    let tempdir = tempfile::tempdir().unwrap();
    let store_path = tempdir.path().join("events.db");
    let local_store = EventStore::open(&store_path).unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();

    let root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let mut channel_event = SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id,
            name: "general".to_owned(),
            is_private: false,
        },
    );
    channel_event.parents = vec![root.event_id.clone()];
    let channel = alice.sign_event(channel_event);

    local_store.append_event(&root).unwrap();
    local_store.append_event(&channel).unwrap();
    insert_corrupt_event_json(
        &store_path,
        &workspace_id,
        "evt_corrupt_sync_materialization_tripwire",
    );
    assert!(
        local_store
            .list_events_for_workspace(&workspace_id.0)
            .is_err()
    );

    let transport = ReversedFetchTransport { events: Vec::new() };
    let peer = PeerAddress {
        peer_id: PeerId("empty".to_owned()),
        endpoint: "memory".to_owned(),
    };
    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();

    assert!(report.requested_event_ids.is_empty());
    assert!(report.fetched_event_ids.is_empty());
    assert_eq!(
        report.materialization.applied_events,
        vec![root.event_id, channel.event_id]
    );
    assert!(report.materialization.gaps.is_empty());
}

#[tokio::test]
async fn pull_workspace_repairs_invalid_local_row_with_valid_remote_event() {
    let local_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let valid_root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Repaired".to_owned(),
        },
    ));
    let mut poisoned_root = valid_root.clone();
    poisoned_root.signature[0] ^= 1;
    local_store.append_event(&poisoned_root).unwrap();

    let transport = ReversedFetchTransport {
        events: vec![valid_root.clone()],
    };
    let peer = PeerAddress {
        peer_id: PeerId("repair".to_owned()),
        endpoint: "memory".to_owned(),
    };
    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();

    assert_eq!(
        report.requested_event_ids,
        vec![valid_root.event_id.clone()]
    );
    assert_eq!(report.fetched_event_ids, vec![valid_root.event_id.clone()]);
    assert_eq!(
        report.materialization.applied_events,
        vec![valid_root.event_id.clone()]
    );
    assert_eq!(
        local_store.get_event(&valid_root.event_id).unwrap(),
        Some(valid_root)
    );
}

#[tokio::test]
async fn pull_workspace_from_direct_peer_fetches_stores_and_materializes_events() {
    let alice_store = EventStore::open_in_memory().unwrap();
    let bob_store = EventStore::open_in_memory().unwrap();
    let alice = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([11; 32]);

    let root = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let channel = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        alice.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let message = alice.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        alice.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &message_id,
                "pulled encrypted message",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    alice_store.append_event(&root).unwrap();
    alice_store.append_event(&channel).unwrap();
    alice_store.append_event(&message).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", alice_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("alice".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = DirectTransport;

    let report = pull_workspace_from_peer(&transport, &peer, &bob_store, workspace_id)
        .await
        .unwrap();

    assert_eq!(report.requested_event_ids.len(), 3);
    assert_eq!(report.fetched_event_ids.len(), 3);
    assert!(report.ignored_event_ids.is_empty());
    assert_eq!(report.materialization.applied_events.len(), 3);
    assert!(report.materialization.gaps.is_empty());
    assert_eq!(bob_store.list_events().unwrap().len(), 3);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn pull_workspace_from_partial_peer_reports_materialization_gap() {
    let replica_store = EventStore::open_in_memory().unwrap();
    let local_store = EventStore::open_in_memory().unwrap();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let missing_parent_id = EventId("evt_missing_parent".to_owned());
    let content_key = ContentKey::from_bytes([12; 32]);
    let mut event = SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        member.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &message_id,
                "partial encrypted message",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    );
    event.parents = vec![missing_parent_id.clone()];
    let signed = member.sign_event(event);

    replica_store.append_event(&signed).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", replica_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = DirectTransport;

    let report = pull_workspace_from_peer(&transport, &peer, &local_store, workspace_id)
        .await
        .unwrap();

    assert_eq!(report.fetched_event_ids, vec![signed.event_id.clone()]);
    assert!(report.ignored_event_ids.is_empty());
    assert!(report.materialization.applied_events.is_empty());
    assert_eq!(report.materialization.gaps.len(), 1);
    assert_eq!(report.materialization.gaps[0].event_id, signed.event_id);
    assert_eq!(
        report.materialization.gaps[0].missing_parent_ids,
        vec![missing_parent_id]
    );

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
