use chaft_crypto::{ContentKey, seal_message_markdown};
use chaft_identity::DeviceIdentity;
use chaft_net::{ChaftTransport, PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_store::EventStore;
use chaft_types::{
    AttachmentRef, ChannelId, EventBody, MessageId, PayloadEncryption, SealedPayload,
    SignableEvent, SignedEvent, WorkspaceId, WorkspaceRole,
};
use tokio::sync::oneshot;

#[tokio::test]
async fn direct_peer_accepts_invited_member_and_rejects_uninvited_device() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let outsider = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let member_message_id = MessageId::new();
    let outsider_message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([5; 32]);
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    ));
    let member_message = member.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        member.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: member_message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &member_message_id,
                "authorized by invite",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));
    let outsider_message = outsider.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        outsider.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: outsider_message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &outsider_message_id,
                "not invited",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    transport.publish_event(&peer, invite).await.unwrap();
    transport
        .publish_event(&peer, member_message)
        .await
        .unwrap();
    assert!(
        transport
            .publish_event(&peer, outsider_message)
            .await
            .is_err()
    );

    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 4);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_mixed_workspace_publish_batch() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let first_root = owner.sign_event(SignableEvent::new(
        WorkspaceId::new(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "First".to_owned(),
        },
    ));
    let second_root = owner.sign_event(SignableEvent::new(
        WorkspaceId::new(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Second".to_owned(),
        },
    ));
    let transport = DirectTransport;
    let error = transport
        .publish_events_with_authorization(
            &peer,
            vec![first_root, second_root],
            Vec::new(),
            Vec::new(),
        )
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("publish request spans multiple workspaces")
    );
    assert!(transport.fetch_inventory(&peer).await.unwrap().is_empty());

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_publish_authorization_ignores_invalid_local_history_events() {
    let store = EventStore::open_in_memory().unwrap();
    let owner = DeviceIdentity::generate();
    let outsider = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([17; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Poisoned".to_owned(),
        },
    ));
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
    let mut forged_invite = SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: outsider.device_id().clone(),
            role: WorkspaceRole::Member,
        },
    );
    forged_invite.parents = vec![channel.event_id.clone()];
    let mut forged_invite = owner.sign_event(forged_invite);
    forged_invite.signature[0] ^= 1;
    let forged_invite = SignedEvent::from_author_signature(
        forged_invite.event,
        forged_invite.author_public_key,
        forged_invite.signature,
    );
    let forged_invite_event_id = forged_invite.event_id.clone();
    let mut outsider_message = SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        outsider.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-poisoned-history",
                &content_key,
                &workspace_id,
                &channel_id,
                &message_id,
                "should not be authorized by forged local invite",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    );
    outsider_message.parents = vec![forged_invite_event_id.clone()];
    let outsider_message = outsider.sign_event(outsider_message);

    store.append_event(&root).unwrap();
    store.append_event(&channel).unwrap();
    store.append_event(&forged_invite).unwrap();

    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = DirectTransport;
    let error = transport
        .publish_event(&peer, outsider_message)
        .await
        .unwrap_err();
    let inventory = transport.fetch_inventory(&peer).await.unwrap();

    assert!(
        error
            .to_string()
            .contains("not authorized by workspace history")
    );
    assert_eq!(inventory, vec![root.event_id, channel.event_id]);
    assert!(!inventory.contains(&forged_invite_event_id));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_enforces_private_channel_member_grants() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let member = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([7; 32]);
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let invite = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::MemberInvited {
            invitee_device_id: member.device_id().clone(),
            role: WorkspaceRole::Member,
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
                "authorized by private channel grant",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    transport.publish_event(&peer, invite).await.unwrap();
    assert!(
        transport
            .publish_event(&peer, message.clone())
            .await
            .is_err()
    );
    transport.publish_event(&peer, grant).await.unwrap();
    transport.publish_event(&peer, message).await.unwrap();

    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 5);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_accepts_out_of_order_batch_publish() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([9; 32]);
    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let message = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(channel_id.clone()),
        owner.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &channel_id,
                &message_id,
                "batch authorized",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    let transport = DirectTransport;
    transport
        .publish_events_with_proof(
            &peer,
            vec![message.clone(), channel.clone(), root.clone()],
            Vec::new(),
        )
        .await
        .unwrap();

    let inventory = transport.fetch_inventory(&peer).await.unwrap();

    assert_eq!(
        inventory,
        vec![root.event_id, channel.event_id, message.event_id]
    );

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_message_for_unknown_channel() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let missing_channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([6; 32]);
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
        },
    ));
    let message = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        Some(missing_channel_id.clone()),
        owner.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: message_id.clone(),
            sealed_markdown: seal_message_markdown(
                "workspace-key-1",
                &content_key,
                &workspace_id,
                &missing_channel_id,
                &message_id,
                "unknown channel",
            )
            .unwrap(),
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    let error = transport.publish_event(&peer, message).await.unwrap_err();

    assert!(
        error
            .to_string()
            .contains("not authorized by workspace history")
    );
    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 1);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_plaintext_message_storage() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let plaintext_message = owner.sign_event(SignableEvent::new(
        workspace_id,
        Some(channel_id),
        owner.device_id().clone(),
        EventBody::MessageCreated {
            message_id: MessageId::new(),
            markdown: "replica should refuse this plaintext".to_owned(),
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    let error = transport
        .publish_event(&peer, plaintext_message)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("encrypted message payloads"));
    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 2);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_development_plaintext_encrypted_message_storage() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let dev_plaintext_message = owner.sign_event(SignableEvent::new(
        workspace_id,
        Some(channel_id),
        owner.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id: MessageId::new(),
            sealed_markdown: SealedPayload {
                mode: PayloadEncryption::DevelopmentPlaintext,
                key_id: "dev-only".to_owned(),
                nonce: Vec::new(),
                aad: Vec::new(),
                bytes: b"replica should refuse development plaintext".to_vec(),
            },
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    let error = transport
        .publish_event(&peer, dev_plaintext_message)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("AES-256-GCM-SIV"));
    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 2);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_unencrypted_attachment_metadata_storage() {
    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([13; 32]);
    let sealed_markdown = seal_message_markdown(
        "workspace-key-attachment-policy",
        &content_key,
        &workspace_id,
        &channel_id,
        &message_id,
        "encrypted markdown with unsafe attachment metadata",
    )
    .unwrap();
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let message = owner.sign_event(SignableEvent::new(
        workspace_id,
        Some(channel_id),
        owner.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            attachments: vec![AttachmentRef {
                blob_hash: "unencrypted-attachment-blob".to_owned(),
                media_type: "text/plain".to_owned(),
                byte_len: 32,
                display_name: "plain.txt".to_owned(),
                attachment_id: String::new(),
                encryption: None,
            }],
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    let error = transport.publish_event(&peer, message).await.unwrap_err();

    assert!(error.to_string().contains("encrypted attachment metadata"));
    assert_eq!(transport.fetch_inventory(&peer).await.unwrap().len(), 2);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_stores_encrypted_message_without_plaintext() {
    const PRIVATE_MARKDOWN: &str = "launch plan should never be replica plaintext";

    let store = EventStore::open_in_memory().unwrap();
    let server = DirectPeerServer::bind("127.0.0.1:0", store).await.unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let owner = DeviceIdentity::generate();
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::from_bytes([8; 32]);
    let sealed_markdown = seal_message_markdown(
        "workspace-key-1",
        &content_key,
        &workspace_id,
        &channel_id,
        &message_id,
        PRIVATE_MARKDOWN,
    )
    .unwrap();
    let transport = DirectTransport;

    let root = owner.sign_event(SignableEvent::new(
        workspace_id.clone(),
        None,
        owner.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft".to_owned(),
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
    let message = owner.sign_event(SignableEvent::new(
        workspace_id,
        Some(channel_id),
        owner.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            attachments: Vec::new(),
        },
    ));

    transport.publish_event(&peer, root).await.unwrap();
    transport.publish_event(&peer, channel).await.unwrap();
    transport.publish_event(&peer, message).await.unwrap();

    let event_ids = transport.fetch_inventory(&peer).await.unwrap();
    let fetched = transport.fetch_events(&peer, event_ids).await.unwrap();
    let replica_visible_json = serde_json::to_string(&fetched).unwrap();

    assert_eq!(fetched.len(), 3);
    assert!(!replica_visible_json.contains(PRIVATE_MARKDOWN));
    assert!(replica_visible_json.contains("message_created_encrypted"));
    assert!(replica_visible_json.contains("aes256_gcm_siv"));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}
