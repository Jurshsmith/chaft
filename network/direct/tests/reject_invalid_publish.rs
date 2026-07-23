use chaft_net::{ChaftTransport, PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_store::EventStore;
use chaft_types::{
    DeviceId, EventBody, EventId, MessageId, SignableEvent, SignedEvent, WorkspaceId,
};
use tokio::sync::oneshot;

#[tokio::test]
async fn direct_peer_rejects_events_without_self_contained_author_signature() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let forged_event = SignedEvent::from_signed_bytes(
        SignableEvent::new(
            WorkspaceId::new(),
            None,
            DeviceId("dev_forged".to_owned()),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "forged".to_owned(),
                attachments: Vec::new(),
            },
        ),
        vec![1, 2, 3],
    );
    let transport = DirectTransport;

    assert!(transport.publish_event(&peer, forged_event).await.is_err());
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_events_with_noncanonical_event_id() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let identity = chaft_identity::DeviceIdentity::generate();
    let mut event = identity.sign_event(SignableEvent::new(
        WorkspaceId::new(),
        None,
        identity.device_id().clone(),
        EventBody::MessageCreated {
            message_id: MessageId::new(),
            markdown: "wrong event id".to_owned(),
            attachments: Vec::new(),
        },
    ));
    event.event_id = EventId("evt_not_the_canonical_hash".to_owned());
    let transport = DirectTransport;

    assert!(transport.publish_event(&peer, event).await.is_err());
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
