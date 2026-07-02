use chaft_core::WorkspaceState;
use chaft_identity::DeviceIdentity;
use chaft_net::{ChaftTransport, PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport, MAX_INVENTORY_EVENT_IDS_PER_RESPONSE};
use chaft_store::EventStore;
use chaft_sync::EventInventory;
use chaft_types::{
    ChannelId, EventBody, EventId, MessageId, SignableEvent, SignedEvent, WorkspaceId,
};
use chaft_wire::{
    WireSyncRequestKind, WireSyncResponse, decode_sync_request, encode_event_envelope,
    encode_sync_response,
};
use rusqlite::params;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    time::{Duration, sleep, timeout},
};

#[tokio::test]
async fn direct_peers_fetch_missing_events_over_tcp_without_central_server() {
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
        Some(channel_id),
        alice.device_id().clone(),
        EventBody::MessageCreated {
            message_id: message_id.clone(),
            markdown: "direct transport sync".to_owned(),
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
    transport.connect(peer.clone()).await.unwrap();
    let remote_inventory =
        EventInventory::from_event_ids(transport.fetch_inventory(&peer).await.unwrap().into_iter());
    let local_inventory = EventInventory::default();
    let missing_ids = local_inventory.missing_from(&remote_inventory);
    let fetched = transport.fetch_events(&peer, missing_ids).await.unwrap();

    assert_eq!(fetched.len(), 3);

    for event in &fetched {
        bob_store.append_event(event).unwrap();
    }
    let mut bob_state = WorkspaceState::new(workspace_id);
    let report = bob_state
        .apply_batch(&bob_store.list_events().unwrap())
        .unwrap();

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();

    assert!(report.gaps.is_empty());
    assert_eq!(
        bob_state.messages[&message_id].markdown,
        "direct transport sync"
    );
}

#[tokio::test]
async fn direct_peer_can_return_workspace_scoped_inventory() {
    let store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let first_workspace_id = WorkspaceId::new();
    let second_workspace_id = WorkspaceId::new();
    let first = identity.sign_event(SignableEvent::new(
        first_workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "First".to_owned(),
        },
    ));
    let second = identity.sign_event(SignableEvent::new(
        second_workspace_id,
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Second".to_owned(),
        },
    ));
    store.append_event(&first).unwrap();
    store.append_event(&second).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let transport = DirectTransport;
    let full_inventory = transport.fetch_inventory(&peer).await.unwrap();
    let scoped_inventory = transport
        .fetch_workspace_inventory(&peer, &first_workspace_id)
        .await
        .unwrap();

    assert_eq!(full_inventory.len(), 2);
    assert_eq!(scoped_inventory, vec![first.event_id]);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_workspace_inventory_fetches_paged_responses() {
    let workspace_id = WorkspaceId::new();
    let workspace_id_for_server = workspace_id.0.clone();
    let first_event_id =
        EventId("evt_0000000000000000000000000000000000000000000000000000000000000001".to_owned());
    let second_event_id =
        EventId("evt_0000000000000000000000000000000000000000000000000000000000000002".to_owned());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let first_event_id_for_server = first_event_id.0.clone();
    let second_event_id_for_server = second_event_id.0.clone();
    let server_task = tokio::spawn(async move {
        for (expected_start_index, event_id) in [
            (0u64, first_event_id_for_server),
            (1u64, second_event_id_for_server),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request_len = stream.read_u32().await.unwrap() as usize;
            let mut request_bytes = vec![0; request_len];
            stream.read_exact(&mut request_bytes).await.unwrap();
            let request = decode_sync_request(&request_bytes).unwrap();
            assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
            assert_eq!(
                request.workspace_id.as_deref(),
                Some(workspace_id_for_server.as_str())
            );
            assert_eq!(request.inventory_start_index, Some(expected_start_index));
            assert_eq!(
                request.inventory_limit,
                Some(MAX_INVENTORY_EVENT_IDS_PER_RESPONSE as u64)
            );

            let response = WireSyncResponse {
                event_ids: vec![event_id],
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: Some(2),
            };
            let response = encode_sync_response(&response);
            stream.write_u32(response.len() as u32).await.unwrap();
            stream.write_all(&response).await.unwrap();
        }
    });

    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("paged-inventory-response".to_owned()),
        endpoint,
    };
    let event_ids = transport
        .fetch_workspace_inventory(&peer, &workspace_id)
        .await
        .unwrap();

    assert_eq!(event_ids, vec![first_event_id, second_event_id]);
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_full_inventory_fetches_paged_responses() {
    let first_event_id =
        EventId("evt_0000000000000000000000000000000000000000000000000000000000000011".to_owned());
    let second_event_id =
        EventId("evt_0000000000000000000000000000000000000000000000000000000000000012".to_owned());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let first_event_id_for_server = first_event_id.0.clone();
    let second_event_id_for_server = second_event_id.0.clone();
    let server_task = tokio::spawn(async move {
        for (expected_start_index, event_id) in [
            (0u64, first_event_id_for_server),
            (1u64, second_event_id_for_server),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request_len = stream.read_u32().await.unwrap() as usize;
            let mut request_bytes = vec![0; request_len];
            stream.read_exact(&mut request_bytes).await.unwrap();
            let request = decode_sync_request(&request_bytes).unwrap();
            assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
            assert!(request.workspace_id.is_none());
            assert_eq!(request.inventory_start_index, Some(expected_start_index));
            assert_eq!(
                request.inventory_limit,
                Some(MAX_INVENTORY_EVENT_IDS_PER_RESPONSE as u64)
            );

            let response = WireSyncResponse {
                event_ids: vec![event_id],
                events: Vec::new(),
                error: None,
                blobs: Vec::new(),
                blob_descriptors: Vec::new(),
                blob_availability: Vec::new(),
                event_envelopes: Vec::new(),
                inventory_total_count: Some(2),
            };
            let response = encode_sync_response(&response);
            stream.write_u32(response.len() as u32).await.unwrap();
            stream.write_all(&response).await.unwrap();
        }
    });

    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("paged-full-inventory-response".to_owned()),
        endpoint,
    };
    let event_ids = transport.fetch_inventory(&peer).await.unwrap();

    assert_eq!(event_ids, vec![first_event_id, second_event_id]);
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_noncanonical_inventory_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
        assert!(request.workspace_id.is_none());
        assert_eq!(request.inventory_start_index, Some(0));
        assert_eq!(
            request.inventory_limit,
            Some(MAX_INVENTORY_EVENT_IDS_PER_RESPONSE as u64)
        );

        let response = WireSyncResponse {
            event_ids: vec!["evt_not_the_canonical_hash".to_owned()],
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
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("invalid-inventory-response".to_owned()),
        endpoint,
    };

    let error = transport.fetch_inventory(&peer).await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("non-canonical inventory event id")
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_connect_rejects_inventory_error_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
        assert!(request.workspace_id.is_none());
        assert_eq!(request.inventory_start_index, Some(0));
        assert_eq!(request.inventory_limit, Some(0));

        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: Some("inventory unavailable".to_owned()),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: None,
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("connect-error-response".to_owned()),
        endpoint,
    };

    let error = transport.connect(peer).await.unwrap_err();

    assert!(error.to_string().contains("inventory unavailable"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_connect_requests_empty_inventory_page() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
        assert!(request.workspace_id.is_none());
        assert_eq!(request.inventory_start_index, Some(0));
        assert_eq!(request.inventory_limit, Some(0));

        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: Some(5000),
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("connect-empty-page-response".to_owned()),
        endpoint,
    };

    transport.connect(peer).await.unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_connect_rejects_inventory_ids_in_empty_page_response() {
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let event = identity.sign_event(SignableEvent::new(
        workspace_id,
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "oversized connect inventory".to_owned(),
        },
    ));
    let event_id = event.event_id.0;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::Inventory as i32);
        assert!(request.workspace_id.is_none());
        assert_eq!(request.inventory_start_index, Some(0));
        assert_eq!(request.inventory_limit, Some(0));

        let response = WireSyncResponse {
            event_ids: vec![event_id],
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: Vec::new(),
            inventory_total_count: Some(1),
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("connect-nonempty-page-response".to_owned()),
        endpoint,
    };

    let error = transport.connect(peer).await.unwrap_err();

    assert!(error.to_string().contains("exceeds requested limit 0"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_non_empty_publish_ack_response() {
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let event = identity.sign_event(SignableEvent::new(
        workspace_id,
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Publish Ack".to_owned(),
        },
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::PublishEvents as i32);

        let response = WireSyncResponse {
            event_ids: vec![
                "evt_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ],
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
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("non-empty-publish-ack".to_owned()),
        endpoint,
    };

    let error = transport.publish_event(&peer, event).await.unwrap_err();

    assert!(error.to_string().contains("non-empty ack response"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_fetch_omits_corrupt_nonservable_local_event() {
    let tempdir = tempfile::tempdir().unwrap();
    let store_path = tempdir.path().join("events.db");
    let store = EventStore::open(&store_path).unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let root = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Corrupt local row".to_owned(),
        },
    ));
    store.append_event(&root).unwrap();
    let corrupt_event_id = format!("evt_{}", "c".repeat(64));
    let connection = rusqlite::Connection::open(&store_path).unwrap();
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
            ) VALUES (?1, ?2, NULL, ?3, 2, 0, 0, ?4)
            ",
            params![
                corrupt_event_id,
                workspace_id.0,
                "dev_corrupt",
                b"{not valid json}".as_slice()
            ],
        )
        .unwrap();
    drop(connection);

    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let transport = DirectTransport;
    let fetched = transport
        .fetch_events(
            &peer,
            vec![root.event_id.clone(), EventId(corrupt_event_id.clone())],
        )
        .await
        .unwrap();

    assert_eq!(fetched, vec![root]);
    assert!(
        !transport
            .fetch_inventory(&peer)
            .await
            .unwrap()
            .contains(&EventId(corrupt_event_id))
    );

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_unrequested_fetch_event_response() {
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
        workspace_id,
        None,
        identity.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: ChannelId::new(),
            name: "unrequested".to_owned(),
            is_private: false,
        },
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let requested_event_id = requested.event_id.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::FetchEvents as i32);
        assert_eq!(request.event_ids, vec![requested_event_id.0]);

        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: vec![encode_event_envelope(&unrequested)],
            inventory_total_count: None,
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("unrequested-event-response".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_events(&peer, vec![requested.event_id])
        .await
        .unwrap_err();

    assert!(error.to_string().contains("unrequested event"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_unexpected_fetch_event_response_fields() {
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let requested = identity.sign_event(SignableEvent::new(
        workspace_id,
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Requested".to_owned(),
        },
    ));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let requested_event_id = requested.event_id.clone();
    let server_event = requested.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::FetchEvents as i32);
        assert_eq!(request.event_ids, vec![requested_event_id.0]);

        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: vec![chaft_wire::WireBlobEnvelope {
                hash: "f".repeat(64),
                bytes: b"unexpected fetch event blob".to_vec(),
            }],
            blob_descriptors: Vec::new(),
            blob_availability: Vec::new(),
            event_envelopes: vec![encode_event_envelope(&server_event)],
            inventory_total_count: None,
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("unexpected-fetch-event-fields".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_events(&peer, vec![requested.event_id])
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unexpected fetch-events response fields")
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_inventory_and_fetch_skip_invalid_local_events() {
    let store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let root = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Filtered".to_owned(),
        },
    ));
    let mut forged = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: ChannelId::new(),
            name: "forged".to_owned(),
            is_private: false,
        },
    ));
    forged.signature[0] ^= 1;
    let forged = SignedEvent::from_author_signature(
        forged.event,
        forged.author_public_key,
        forged.signature,
    );
    let forged_event_id = forged.event_id.clone();
    store.append_event(&root).unwrap();
    store.append_event(&forged).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let transport = DirectTransport;
    let full_inventory = transport.fetch_inventory(&peer).await.unwrap();
    let scoped_inventory = transport
        .fetch_workspace_inventory(&peer, &workspace_id)
        .await
        .unwrap();
    let fetched = transport
        .fetch_events(&peer, vec![root.event_id.clone(), forged_event_id.clone()])
        .await
        .unwrap();

    assert_eq!(full_inventory, vec![root.event_id.clone()]);
    assert_eq!(scoped_inventory, vec![root.event_id.clone()]);
    assert_eq!(fetched, vec![root]);
    assert!(!full_inventory.contains(&forged_event_id));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_serves_new_connections_while_one_client_is_idle() {
    let store = EventStore::open_in_memory().unwrap();
    let identity = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let root = identity.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    store.append_event(&root).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let addr = server.local_addr().unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: addr.to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let idle_stream = TcpStream::connect(addr).await.unwrap();
    sleep(Duration::from_millis(20)).await;

    let transport = DirectTransport;
    let scoped_inventory = timeout(
        Duration::from_secs(2),
        transport.fetch_workspace_inventory(&peer, &workspace_id),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(scoped_inventory, vec![root.event_id]);

    drop(idle_stream);
    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
