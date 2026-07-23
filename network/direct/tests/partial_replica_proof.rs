use chaft_core::WorkspaceState;
use chaft_crypto::{ContentKey, seal_message_markdown};
use chaft_identity::DeviceIdentity;
use chaft_net::{ChaftTransport, PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_store::EventStore;
use chaft_types::{
    ChannelId, EventBody, EventId, MessageId, SignableEvent, TrustSnapshot, TrustSnapshotChannel,
    TrustSnapshotRole, WorkspaceId, WorkspaceRole,
};
use tokio::sync::oneshot;

#[tokio::test]
async fn partial_replica_accepts_message_with_authorization_proof_without_storing_proof() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([6; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let channel = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let message = member.sign_event(SignableEvent::new(
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
                "authorized by proof",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;
    transport
        .publish_event_with_proof(&peer, message.clone(), vec![channel, invite, root])
        .await
        .unwrap();

    let inventory = transport.fetch_inventory(&peer).await.unwrap();

    assert_eq!(inventory, vec![message.event_id]);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn partial_replica_rejects_plaintext_message_in_authorization_proof() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let plaintext_message_id = MessageId::new();
    let encrypted_message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([15; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let channel = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let plaintext_proof = member.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        member.device_id().clone(),
        EventBody::MessageCreated {
            message_id: plaintext_message_id,
            markdown: "proof slices must not leak plaintext".to_owned(),
            attachments: Vec::new(),
        },
    ));
    let message = member.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        member.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: encrypted_message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &encrypted_message_id,
                "authorized without plaintext proof",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;
    let error = transport
        .publish_event_with_proof(&peer, message, vec![root, invite, channel, plaintext_proof])
        .await
        .unwrap_err();

    assert!(error.to_string().contains("encrypted message payloads"));
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn partial_replica_accepts_message_with_compact_trust_snapshot() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([13; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let trust_snapshot = owner
        .sign_trust_snapshot(
            TrustSnapshot {
                schema_version: 1,
                workspace_id: workspace_id.clone(),
                root_event_id: root.event_id.clone(),
                root_author_device_id: owner.device_id().clone(),
                roles: vec![TrustSnapshotRole {
                    device_id: member.device_id().clone(),
                    role: WorkspaceRole::Member,
                }],
                channels: vec![TrustSnapshotChannel {
                    channel_id: channel_id.clone(),
                    is_private: false,
                    creator_device_id: owner.device_id().clone(),
                    member_device_ids: Vec::new(),
                }],
                messages: Vec::new(),
                event_channels: Vec::new(),
                person_device_links: Vec::new(),
            },
            root,
        )
        .unwrap();
    let message = member.sign_event(SignableEvent::new(
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
                "authorized by compact trust snapshot",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;
    transport
        .publish_event_with_trust_snapshot(&peer, message.clone(), trust_snapshot)
        .await
        .unwrap();

    let inventory = transport.fetch_inventory(&peer).await.unwrap();
    let fetched = transport
        .fetch_events(&peer, inventory.clone())
        .await
        .unwrap();

    assert_eq!(inventory, vec![message.event_id.clone()]);
    assert_eq!(fetched, vec![message]);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn partial_replica_rejects_forged_compact_trust_snapshot() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([14; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let mut trust_snapshot = owner
        .sign_trust_snapshot(
            TrustSnapshot {
                schema_version: 1,
                workspace_id: workspace_id.clone(),
                root_event_id: root.event_id.clone(),
                root_author_device_id: owner.device_id().clone(),
                roles: vec![TrustSnapshotRole {
                    device_id: member.device_id().clone(),
                    role: WorkspaceRole::Member,
                }],
                channels: vec![TrustSnapshotChannel {
                    channel_id: channel_id.clone(),
                    is_private: false,
                    creator_device_id: owner.device_id().clone(),
                    member_device_ids: Vec::new(),
                }],
                messages: Vec::new(),
                event_channels: Vec::new(),
                person_device_links: Vec::new(),
            },
            root,
        )
        .unwrap();
    trust_snapshot.snapshot.roles.clear();
    let message = member.sign_event(SignableEvent::new(
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
                "forged snapshot should fail",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;

    assert!(
        transport
            .publish_event_with_trust_snapshot(&peer, message, trust_snapshot)
            .await
            .is_err()
    );
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn partial_replica_accepts_private_channel_message_with_member_grant_proof() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([12; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let channel = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "strategy".to_owned(),
            is_private: true,
        },
    ));
    let grant = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelMemberAdded {
            channel_id: channel_id.clone(),
            member_device_id: member.device_id().clone(),
        },
    ));
    let message = member.sign_event(SignableEvent::new(
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
                "authorized by private channel proof",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;
    transport
        .publish_event_with_proof(&peer, message.clone(), vec![grant, channel, invite, root])
        .await
        .unwrap();

    let inventory = transport.fetch_inventory(&peer).await.unwrap();

    assert_eq!(inventory, vec![message.event_id]);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn client_reports_missing_history_for_partial_replica_event() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let missing_parent_id = EventId("evt_missing_history_slice".to_owned());
    let content_key = ContentKey::from_bytes([10; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let channel = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let mut message_event = SignableEvent::new(
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
                "later encrypted slice",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    );
    message_event.parents = vec![missing_parent_id.clone()];
    let message = member.sign_event(message_event);

    let transport = DirectTransport;
    transport
        .publish_event_with_proof(&peer, message.clone(), vec![root, invite, channel])
        .await
        .unwrap();

    let inventory = transport.fetch_inventory(&peer).await.unwrap();
    let fetched = transport.fetch_events(&peer, inventory).await.unwrap();
    let mut state = WorkspaceState::new(workspace_id);
    let report = state.apply_batch(&fetched).unwrap();

    assert!(report.applied_events.is_empty());
    assert_eq!(report.gaps.len(), 1);
    assert_eq!(report.gaps[0].event_id, message.event_id);
    assert_eq!(report.gaps[0].missing_parent_ids, vec![missing_parent_id]);
    assert!(!state.messages.contains_key(&message_id));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn partial_replica_rejects_message_when_proof_does_not_invite_author() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("partial-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let outsider = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([7; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let invite_someone_else = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: outsider.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let channel = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    ));
    let message = member.sign_event(SignableEvent::new(
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
                "not authorized by proof",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;

    assert!(
        transport
            .publish_event_with_proof(&peer, message, vec![root, invite_someone_else, channel])
            .await
            .is_err()
    );
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
