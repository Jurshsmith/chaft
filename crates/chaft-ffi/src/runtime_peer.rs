use std::{ffi::c_char, path::PathBuf, sync::mpsc, thread};

use chaft_media::BlobStore;
use chaft_net_direct::{DirectPeerServer, SyncPeerStore};
use chaft_net_iroh::{IrohSyncPeer, IrohTransportConfig};
use chaft_runtime::PEER_ENDPOINT_MAX_BYTES;
use chaft_store::EventStore;
use tokio::sync::oneshot;

use crate::{
    envelope::{FfiResult, ffi_error, result_envelope},
    input::{optional_c_string, optional_c_string_with_max_bytes, read_c_string},
    open_runtime_from_ffi, open_runtime_from_paths,
    peer_endpoint::{validate_direct_listen_endpoint_text, validate_peer_endpoint_text},
    peer_host::{
        HostedPeer, StoppedPeer, next_hosted_peer_id, register_hosted_peer, stop_hosted_peer,
    },
};

pub(crate) fn runtime_start_direct_peer_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    listen: *const c_char,
) -> FfiResult<HostedPeer> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let listen = optional_c_string_with_max_bytes(
            listen,
            "listen",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?
        .map(|listen| listen.trim().to_owned())
        .filter(|listen| !listen.is_empty())
        .unwrap_or_else(|| "127.0.0.1:0".to_owned());
        validate_peer_endpoint_text(&listen, "listen")?;
        validate_direct_listen_endpoint_text(&listen)?;
        let runtime = open_runtime_from_paths(&data_dir, identity_file)?;
        let paths = runtime.paths().clone();
        let peer_id = next_hosted_peer_id("direct-peer");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        let thread = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = ready_tx.send(Err("tokio runtime creation failed".to_owned()));
                return;
            };

            runtime.block_on(async move {
                let store = match EventStore::open(&paths.event_store) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let blob_store = match BlobStore::open(&paths.blob_store) {
                    Ok(blob_store) => blob_store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let server =
                    match DirectPeerServer::bind_with_blobs(&listen, store, blob_store).await {
                        Ok(server) => server,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return;
                        }
                    };
                let endpoint = match server.local_addr() {
                    Ok(endpoint) => endpoint.to_string(),
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(endpoint));
                let _ = server.serve_until_shutdown(shutdown_rx).await;
            });
        });

        let endpoint = match ready_rx
            .recv()
            .map_err(|_| ffi_error("runtime_direct_peer_start_failed", "peer thread exited"))?
        {
            Ok(endpoint) => endpoint,
            Err(message) => {
                let _ = thread.join();
                return Err(ffi_error("runtime_direct_peer_start_failed", message));
            }
        };

        register_hosted_peer(
            peer_id,
            endpoint,
            shutdown_tx,
            thread,
            "runtime_direct_peer_registry_failed",
        )
    })
}

pub(crate) fn runtime_start_iroh_peer_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<HostedPeer> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let paths = runtime.paths().clone();
        let peer_id = next_hosted_peer_id("iroh-peer");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        let thread = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = ready_tx.send(Err("tokio runtime creation failed".to_owned()));
                return;
            };

            runtime.block_on(async move {
                let store = match EventStore::open(&paths.event_store) {
                    Ok(store) => store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let blob_store = match BlobStore::open(&paths.blob_store) {
                    Ok(blob_store) => blob_store,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                let sync_store = SyncPeerStore::with_blobs(store, blob_store);
                let server =
                    match IrohSyncPeer::bind(sync_store, IrohTransportConfig::from_environment())
                        .await
                    {
                        Ok(server) => server,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.to_string()));
                            return;
                        }
                    };
                let endpoint = server.endpoint_url();
                let _ = ready_tx.send(Ok(endpoint));
                let _ = shutdown_rx.await;
                let _ = server.close().await;
            });
        });

        let endpoint = match ready_rx
            .recv()
            .map_err(|_| ffi_error("runtime_iroh_peer_start_failed", "peer thread exited"))?
        {
            Ok(endpoint) => endpoint,
            Err(message) => {
                let _ = thread.join();
                return Err(ffi_error("runtime_iroh_peer_start_failed", message));
            }
        };

        register_hosted_peer(
            peer_id,
            endpoint,
            shutdown_tx,
            thread,
            "runtime_iroh_peer_registry_failed",
        )
    })
}

pub(crate) fn runtime_stop_direct_peer_result(peer_id: *const c_char) -> FfiResult<StoppedPeer> {
    result_envelope(|| {
        let peer_id = read_c_string(peer_id, "peer_id")?;
        stop_hosted_peer(peer_id)
    })
}
