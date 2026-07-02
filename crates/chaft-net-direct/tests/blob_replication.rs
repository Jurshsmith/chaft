use std::sync::{Arc, Mutex};

use chaft_crypto::{
    ContentKey, encrypted_blob_ref_from_payload, open_attachment_blob, seal_attachment_blob,
    sealed_payload_from_encrypted_blob_ref,
};
use chaft_media::{BlobDescriptor, BlobStore, blob_hash, describe_blob, plan_missing_chunks};
use chaft_net::{PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_store::EventStore;
use chaft_types::{ChannelId, MessageId, WorkspaceId};
use chaft_wire::{
    WireBlobAvailability, WireBlobDescriptor, WireBlobEnvelope, WireSyncRequest,
    WireSyncRequestKind, WireSyncResponse, decode_sync_request, decode_sync_response,
    encode_sync_request, encode_sync_response,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

#[tokio::test]
async fn direct_peer_replicates_content_addressed_blob() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("blob-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let bytes = b"replicated attachment bytes".to_vec();
    let transport = DirectTransport;
    let hash = transport.put_blob(&peer, bytes.clone()).await.unwrap();
    let fetched = transport.fetch_blob(&peer, &hash).await.unwrap();

    assert_eq!(hash, blob_hash(&bytes));
    assert_eq!(fetched, Some(bytes));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_blob_availability_response_omits_blob_bytes() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let bytes = b"available without transfer".to_vec();
    let descriptor = blob_store.put_bytes(&bytes).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap().to_string();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let request = WireSyncRequest {
        kind: WireSyncRequestKind::FetchBlobAvailability as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: vec![descriptor.hash.clone()],
        blobs: Vec::new(),
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    };
    let response = raw_request(&endpoint, encode_sync_request(&request)).await;

    assert!(response.error.is_none());
    assert!(response.blobs.is_empty());
    assert!(response.blob_descriptors.is_empty());
    assert_eq!(response.blob_availability.len(), 1);
    assert_eq!(response.blob_availability[0].hash, descriptor.hash);
    assert!(response.blob_availability[0].has_whole_blob);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_non_empty_blob_upload_ack_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
        assert_eq!(request.blobs.len(), 1);

        let response = WireSyncResponse {
            event_ids: Vec::new(),
            events: Vec::new(),
            error: None,
            blobs: vec![WireBlobEnvelope {
                hash: blob_hash(b"unsolicited ack blob"),
                bytes: b"unsolicited ack blob".to_vec(),
            }],
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
        peer_id: PeerId("non-empty-blob-upload-ack".to_owned()),
        endpoint,
    };

    let error = transport
        .put_blob(&peer, b"blob upload".to_vec())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("non-empty ack response"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_replicates_multiple_blobs_per_request() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("batch-blob-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let first = b"first replicated blob".to_vec();
    let second = b"second replicated blob".to_vec();
    let missing_hash = blob_hash(b"not uploaded");
    let transport = DirectTransport;
    let hashes = transport
        .put_blobs(&peer, vec![first.clone(), second.clone()])
        .await
        .unwrap();
    let fetched = transport
        .fetch_blobs(
            &peer,
            vec![hashes[0].clone(), missing_hash.clone(), hashes[1].clone()],
        )
        .await
        .unwrap();

    assert_eq!(hashes, vec![blob_hash(&first), blob_hash(&second)]);
    assert_eq!(fetched.get(&hashes[0]), Some(&first));
    assert_eq!(fetched.get(&hashes[1]), Some(&second));
    assert!(!fetched.contains_key(&missing_hash));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_unrequested_blob_fetch_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let requested_hash = blob_hash(b"requested blob");
    let unrequested_bytes = b"unrequested blob".to_vec();
    let unrequested_hash = blob_hash(&unrequested_bytes);
    let server_requested_hash = requested_hash.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::FetchBlobs as i32);
        assert_eq!(request.blob_hashes, vec![server_requested_hash]);

        let response = WireSyncResponse {
            blobs: vec![WireBlobEnvelope {
                hash: unrequested_hash,
                bytes: unrequested_bytes,
            }],
            ..empty_response()
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("unrequested-blob-response".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_blobs(&peer, vec![requested_hash])
        .await
        .unwrap_err();

    assert!(error.to_string().contains("unrequested blob"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_mismatched_fetch_blob_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let requested_hash = blob_hash(b"requested blob");
    let server_requested_hash = requested_hash.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::FetchBlobs as i32);
        assert_eq!(request.blob_hashes, vec![server_requested_hash.clone()]);

        let response = WireSyncResponse {
            blobs: vec![WireBlobEnvelope {
                hash: server_requested_hash,
                bytes: b"different blob".to_vec(),
            }],
            ..empty_response()
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("mismatched-blob-response".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_blobs(&peer, vec![requested_hash])
        .await
        .unwrap_err();

    assert!(error.to_string().contains("fetched blob hash mismatch"));
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_unexpected_fetch_blob_response_fields() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let bytes = b"requested blob".to_vec();
    let requested_hash = blob_hash(&bytes);
    let server_requested_hash = requested_hash.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(request.kind, WireSyncRequestKind::FetchBlobs as i32);
        assert_eq!(request.blob_hashes, vec![server_requested_hash.clone()]);

        let response = WireSyncResponse {
            event_ids: vec![
                "evt_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            ],
            blobs: vec![WireBlobEnvelope {
                hash: server_requested_hash,
                bytes,
            }],
            ..empty_response()
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("unexpected-fetch-blob-fields".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_blobs(&peer, vec![requested_hash])
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unexpected fetch-blobs response fields")
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_blob_bytes_in_availability_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let bytes = b"requested blob".to_vec();
    let requested_hash = blob_hash(&bytes);
    let server_requested_hash = requested_hash.clone();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request_len = stream.read_u32().await.unwrap() as usize;
        let mut request_bytes = vec![0; request_len];
        stream.read_exact(&mut request_bytes).await.unwrap();
        let request = decode_sync_request(&request_bytes).unwrap();
        assert_eq!(
            request.kind,
            WireSyncRequestKind::FetchBlobAvailability as i32
        );
        assert_eq!(request.blob_hashes, vec![server_requested_hash.clone()]);

        let response = WireSyncResponse {
            blobs: vec![WireBlobEnvelope {
                hash: server_requested_hash.clone(),
                bytes,
            }],
            blob_availability: vec![WireBlobAvailability {
                hash: server_requested_hash,
                has_whole_blob: true,
                descriptor: None,
                available_chunk_hashes: Vec::new(),
                missing_chunk_hashes: Vec::new(),
            }],
            ..empty_response()
        };
        let response = encode_sync_response(&response);
        stream.write_u32(response.len() as u32).await.unwrap();
        stream.write_all(&response).await.unwrap();
    });
    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("unexpected-availability-bytes".to_owned()),
        endpoint,
    };

    let error = transport
        .fetch_blob_availability(&peer, &requested_hash)
        .await
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("unexpected fetch-blob-availability response fields")
    );
    server_task.await.unwrap();
}

#[tokio::test]
async fn direct_peer_replicates_encrypted_attachment_blob_without_plaintext() {
    const PRIVATE_ATTACHMENT: &[u8] = b"raw attachment bytes should not be on the replica";

    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("encrypted-blob-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let key = ContentKey::from_bytes([9; 32]);
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let sealed = seal_attachment_blob(
        "workspace-key-1",
        &key,
        &workspace_id,
        &channel_id,
        &message_id,
        0,
        PRIVATE_ATTACHMENT,
    )
    .unwrap();
    let encrypted_ref =
        encrypted_blob_ref_from_payload(&sealed, PRIVATE_ATTACHMENT.len() as u64).unwrap();
    let ciphertext = sealed.bytes.clone();
    let transport = DirectTransport;
    let hash = transport.put_blob(&peer, ciphertext.clone()).await.unwrap();
    let fetched_ciphertext = transport.fetch_blob(&peer, &hash).await.unwrap().unwrap();
    let reconstructed = sealed_payload_from_encrypted_blob_ref(&encrypted_ref, fetched_ciphertext);
    let opened = open_attachment_blob(
        &key,
        &reconstructed,
        &workspace_id,
        &channel_id,
        &message_id,
        0,
    )
    .unwrap();

    assert_eq!(hash, blob_hash(&ciphertext));
    assert_ne!(ciphertext, PRIVATE_ATTACHMENT);
    assert!(!String::from_utf8_lossy(&ciphertext).contains("attachment bytes"));
    assert_eq!(opened, PRIVATE_ATTACHMENT);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_blob_with_mismatched_hash() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap().to_string();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let request = WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: vec![WireBlobEnvelope {
            hash: blob_hash(b"expected"),
            bytes: b"actual".to_vec(),
        }],
        blob_descriptors: Vec::new(),
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    };
    let response = raw_request(&endpoint, encode_sync_request(&request)).await;

    assert!(response.error.unwrap().contains("blob hash mismatch"));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_chunked_manifest_with_inconsistent_chunk_count() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap().to_string();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let mut descriptor = describe_blob(b"abcdef", 2);
    descriptor.chunk_hashes.pop();
    let request = WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: Vec::new(),
        blob_descriptors: vec![WireBlobDescriptor {
            hash: descriptor.hash,
            byte_len: descriptor.byte_len,
            chunk_size: descriptor.chunk_size as u64,
            chunk_hashes: descriptor.chunk_hashes,
        }],
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    };
    let response = raw_request(&endpoint, encode_sync_request(&request)).await;

    assert!(response.error.unwrap().contains("invalid blob descriptor"));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_replicates_chunked_blob() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("chunked-blob-replica".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let bytes = b"abcdef".to_vec();
    let transport = DirectTransport;
    let descriptor = transport
        .put_blob_chunked(&peer, bytes.clone(), 2)
        .await
        .unwrap();
    let replica_blobs = BlobStore::open(tempdir.path()).unwrap();
    let availability = replica_blobs
        .availability(&descriptor.hash)
        .unwrap()
        .unwrap();
    let fetched = transport
        .fetch_blob_chunked(&peer, &descriptor.hash)
        .await
        .unwrap();

    assert_eq!(descriptor.chunk_hashes.len(), 3);
    assert!(!replica_blobs.has_blob(&descriptor.hash).unwrap());
    assert!(availability.is_complete());
    assert_eq!(fetched, Some(bytes));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_chunked_upload_skips_remotely_available_chunks() {
    let bytes = b"abcdef".to_vec();
    let descriptor = describe_blob(&bytes, 2);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap().to_string();
    let uploaded_chunk_hashes = Arc::new(Mutex::new(Vec::new()));
    let server_uploaded_chunk_hashes = Arc::clone(&uploaded_chunk_hashes);
    let server_descriptor = descriptor.clone();
    let server_task = tokio::spawn(async move {
        for request_index in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request_len = stream.read_u32().await.unwrap() as usize;
            let mut request_bytes = vec![0; request_len];
            stream.read_exact(&mut request_bytes).await.unwrap();
            let request = decode_sync_request(&request_bytes).unwrap();
            let mut response = empty_response();

            match request_index {
                0 => {
                    assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                    assert!(request.blobs.is_empty());
                    assert_eq!(request.blob_descriptors.len(), 1);
                    assert_eq!(request.blob_descriptors[0].hash, server_descriptor.hash);
                }
                1 => {
                    assert_eq!(
                        request.kind,
                        WireSyncRequestKind::FetchBlobAvailability as i32
                    );
                    assert_eq!(request.blob_hashes, vec![server_descriptor.hash.clone()]);
                    response.blob_availability.push(WireBlobAvailability {
                        hash: server_descriptor.hash.clone(),
                        has_whole_blob: false,
                        descriptor: Some(WireBlobDescriptor {
                            hash: server_descriptor.hash.clone(),
                            byte_len: server_descriptor.byte_len,
                            chunk_size: server_descriptor.chunk_size as u64,
                            chunk_hashes: server_descriptor.chunk_hashes.clone(),
                        }),
                        available_chunk_hashes: vec![server_descriptor.chunk_hashes[0].clone()],
                        missing_chunk_hashes: server_descriptor.chunk_hashes[1..].to_vec(),
                    });
                }
                _ => {
                    assert_eq!(request.kind, WireSyncRequestKind::PutBlobs as i32);
                    assert_eq!(request.blobs.len(), 2);
                    server_uploaded_chunk_hashes
                        .lock()
                        .unwrap()
                        .extend(request.blobs.iter().map(|blob| blob.hash.clone()));
                }
            }

            let response = encode_sync_response(&response);
            stream.write_u32(response.len() as u32).await.unwrap();
            stream.write_all(&response).await.unwrap();
        }
    });

    let transport = DirectTransport;
    let peer = PeerAddress {
        peer_id: PeerId("resumable-chunk-upload".to_owned()),
        endpoint,
    };
    let uploaded = transport.put_blob_chunked(&peer, bytes, 2).await.unwrap();

    server_task.await.unwrap();
    assert_eq!(uploaded, descriptor);
    assert_eq!(
        *uploaded_chunk_hashes.lock().unwrap(),
        descriptor.chunk_hashes[1..].to_vec()
    );
}

#[tokio::test]
async fn direct_peer_returns_none_for_incomplete_chunked_blob() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap().to_string();
    let peer = PeerAddress {
        peer_id: PeerId("partial-chunked-blob-replica".to_owned()),
        endpoint: endpoint.clone(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let descriptor = describe_blob(b"abcdef", 2);
    let request = WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: vec![WireBlobEnvelope {
            hash: descriptor.chunk_hashes[0].clone(),
            bytes: b"ab".to_vec(),
        }],
        blob_descriptors: vec![WireBlobDescriptor {
            hash: descriptor.hash.clone(),
            byte_len: descriptor.byte_len,
            chunk_size: descriptor.chunk_size as u64,
            chunk_hashes: descriptor.chunk_hashes.clone(),
        }],
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    };
    let response = raw_request(&endpoint, encode_sync_request(&request)).await;
    assert!(response.error.is_none());

    let transport = DirectTransport;
    let fetched = transport
        .fetch_blob_chunked(&peer, &descriptor.hash)
        .await
        .unwrap();

    assert_eq!(fetched, None);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_rejects_chunked_fetch_when_manifest_lies_about_chunk_lengths() {
    let tempdir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
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
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let peer = PeerAddress {
        peer_id: PeerId("lying-manifest".to_owned()),
        endpoint: server.local_addr().unwrap().to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let transport = DirectTransport;

    let availability = transport
        .fetch_blob_availability(&peer, &descriptor.hash)
        .await
        .unwrap()
        .unwrap();
    let error = transport
        .fetch_blob_chunked(&peer, &descriptor.hash)
        .await
        .unwrap_err();

    assert!(!availability.is_complete());
    assert!(error.to_string().contains("invalid blob descriptor"));

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn direct_peer_reports_chunk_availability_for_partial_blob() {
    let tempdir = tempfile::tempdir().unwrap();
    let local_dir = tempfile::tempdir().unwrap();
    let store = EventStore::open_in_memory().unwrap();
    let blob_store = BlobStore::open(tempdir.path()).unwrap();
    let server = DirectPeerServer::bind_with_blobs("127.0.0.1:0", store, blob_store)
        .await
        .unwrap();
    let endpoint = server.local_addr().unwrap().to_string();
    let peer = PeerAddress {
        peer_id: PeerId("availability-replica".to_owned()),
        endpoint: endpoint.clone(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });

    let descriptor = describe_blob(b"abcdef", 2);
    let request = WireSyncRequest {
        kind: WireSyncRequestKind::PutBlobs as i32,
        event_ids: Vec::new(),
        events: Vec::new(),
        authorization_events: Vec::new(),
        authorization_snapshots: Vec::new(),
        blob_hashes: Vec::new(),
        blobs: vec![WireBlobEnvelope {
            hash: descriptor.chunk_hashes[0].clone(),
            bytes: b"ab".to_vec(),
        }],
        blob_descriptors: vec![WireBlobDescriptor {
            hash: descriptor.hash.clone(),
            byte_len: descriptor.byte_len,
            chunk_size: descriptor.chunk_size as u64,
            chunk_hashes: descriptor.chunk_hashes.clone(),
        }],
        workspace_id: None,
        event_envelopes: Vec::new(),
        authorization_event_envelopes: Vec::new(),
        authorization_snapshot_envelopes: Vec::new(),
        inventory_start_index: None,
        inventory_limit: None,
    };
    let response = raw_request(&endpoint, encode_sync_request(&request)).await;
    assert!(response.error.is_none());

    let transport = DirectTransport;
    let availability = transport
        .fetch_blob_availability(&peer, &descriptor.hash)
        .await
        .unwrap()
        .unwrap();
    let availability_by_hash = transport
        .fetch_blob_availabilities(&peer, vec![descriptor.hash.clone(), blob_hash(b"missing")])
        .await
        .unwrap();
    let local = BlobStore::open(local_dir.path()).unwrap();
    local.put_manifest(&descriptor).unwrap();
    let planned = plan_missing_chunks(&local, &availability).unwrap();

    assert!(availability_by_hash.contains_key(&descriptor.hash));
    assert!(!availability_by_hash.contains_key(&blob_hash(b"missing")));
    assert!(!availability.is_complete());
    assert_eq!(
        availability.available_chunk_hashes,
        vec![descriptor.chunk_hashes[0].clone()]
    );
    assert_eq!(
        availability.missing_chunk_hashes,
        descriptor.chunk_hashes[1..].to_vec()
    );
    assert_eq!(planned, vec![descriptor.chunk_hashes[0].clone()]);

    shutdown_tx.send(()).unwrap();
    server_task.await.unwrap().unwrap();
}

async fn raw_request(endpoint: &str, request: Vec<u8>) -> chaft_wire::WireSyncResponse {
    let mut stream = TcpStream::connect(endpoint).await.unwrap();
    stream.write_u32(request.len() as u32).await.unwrap();
    stream.write_all(&request).await.unwrap();
    let len = stream.read_u32().await.unwrap() as usize;
    let mut response = vec![0; len];
    stream.read_exact(&mut response).await.unwrap();
    decode_sync_response(&response).unwrap()
}

fn empty_response() -> WireSyncResponse {
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
