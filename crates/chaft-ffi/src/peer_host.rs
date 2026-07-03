use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
};

use serde::Serialize;
use tokio::sync::oneshot;

use crate::envelope::{FfiError, ffi_error};

static HOSTED_PEERS: OnceLock<Mutex<HashMap<String, RunningPeer>>> = OnceLock::new();
static HOSTED_PEER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostedPeer {
    peer_id: String,
    endpoint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoppedPeer {
    peer_id: String,
    endpoint: String,
}

struct RunningPeer {
    endpoint: String,
    shutdown: oneshot::Sender<()>,
    thread: thread::JoinHandle<()>,
}

pub(crate) fn next_hosted_peer_id(prefix: &'static str) -> String {
    format!(
        "{}-{}",
        prefix,
        HOSTED_PEER_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub(crate) fn register_hosted_peer(
    peer_id: String,
    endpoint: String,
    shutdown: oneshot::Sender<()>,
    thread: thread::JoinHandle<()>,
    registry_error_code: &'static str,
) -> Result<HostedPeer, FfiError> {
    hosted_peer_registry()
        .lock()
        .map_err(|_| ffi_error(registry_error_code, "registry poisoned"))?
        .insert(
            peer_id.clone(),
            RunningPeer {
                endpoint: endpoint.clone(),
                shutdown,
                thread,
            },
        );

    Ok(HostedPeer { peer_id, endpoint })
}

pub(crate) fn stop_hosted_peer(peer_id: String) -> Result<StoppedPeer, FfiError> {
    let running = hosted_peer_registry()
        .lock()
        .map_err(|_| ffi_error("runtime_direct_peer_registry_failed", "registry poisoned"))?
        .remove(&peer_id)
        .ok_or_else(|| ffi_error("runtime_direct_peer_not_found", "peer is not running"))?;
    let endpoint = running.endpoint;
    let _ = running.shutdown.send(());
    running
        .thread
        .join()
        .map_err(|_| ffi_error("runtime_direct_peer_stop_failed", "peer thread panicked"))?;

    Ok(StoppedPeer { peer_id, endpoint })
}

fn hosted_peer_registry() -> &'static Mutex<HashMap<String, RunningPeer>> {
    HOSTED_PEERS.get_or_init(|| Mutex::new(HashMap::new()))
}
