use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
};

use chaft_ffi::{
    chaft_decrypted_workspace_snapshot_from_runtime_latest_result_json, chaft_string_free,
};
use chaft_media::BlobStore;
use chaft_net::{PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_runtime::LocalRuntime;
use chaft_store::EventStore;
use chaft_types::{ChannelId, WorkspaceId};
use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use tempfile::TempDir;
use tokio::{runtime::Runtime, sync::oneshot, task::JoinHandle};

const BASE_MESSAGE_COUNT: usize = 128;
const SNAPSHOT_MESSAGE_COUNT: usize = 256;
const SYNC_MESSAGE_COUNT: usize = 96;
const BLOB_BYTES: usize = 256 * 1024;

struct RuntimeFixture {
    _temp_dir: TempDir,
    runtime: LocalRuntime,
    workspace_id: WorkspaceId,
    channel_id: ChannelId,
}

struct DirectSyncFixture {
    _temp_dir: TempDir,
    target: LocalRuntime,
    workspace_id: WorkspaceId,
    peer: PeerAddress,
    shutdown: Option<oneshot::Sender<()>>,
    server_task: JoinHandle<()>,
}

impl DirectSyncFixture {
    async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let _ = self.server_task.await;
    }
}

fn runtime_fixture(messages: usize) -> RuntimeFixture {
    let temp_dir = TempDir::new().expect("create benchmark temp dir");
    let runtime =
        LocalRuntime::open(temp_dir.path().join("runtime"), None).expect("open benchmark runtime");
    let created = runtime
        .create_workspace("Bench Workspace", "general")
        .expect("create benchmark workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let channel_id = ChannelId(created.channel_id);
    for index in 0..messages {
        runtime
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                format!("benchmark message {index:04} search-token"),
            )
            .expect("append benchmark message");
    }

    RuntimeFixture {
        _temp_dir: temp_dir,
        runtime,
        workspace_id,
        channel_id,
    }
}

fn attachment_file(temp_dir: &TempDir, bytes: usize) -> std::path::PathBuf {
    let path = temp_dir.path().join("attachment.bin");
    let mut body = Vec::with_capacity(bytes);
    while body.len() < bytes {
        body.extend_from_slice(b"chaft benchmark attachment payload\n");
    }
    body.truncate(bytes);
    std::fs::write(&path, body).expect("write benchmark attachment");
    path
}

fn direct_sync_fixture(rt: &Runtime, messages: usize, include_blob: bool) -> DirectSyncFixture {
    let temp_dir = TempDir::new().expect("create direct-sync benchmark temp dir");
    let source = LocalRuntime::open(temp_dir.path().join("source"), None)
        .expect("open source benchmark runtime");
    let target = LocalRuntime::open(temp_dir.path().join("target"), None)
        .expect("open target benchmark runtime");
    let created = source
        .create_workspace("Bench Sync", "general")
        .expect("create source workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let channel_id = ChannelId(created.channel_id);
    for index in 0..messages {
        source
            .send_message(
                workspace_id.clone(),
                channel_id.clone(),
                format!("sync message {index:04}"),
            )
            .expect("append source sync message");
    }
    if include_blob {
        let path = attachment_file(&temp_dir, BLOB_BYTES);
        source
            .send_message_with_attachment_file(
                workspace_id.clone(),
                channel_id,
                "blob transfer benchmark",
                path,
                "application/octet-stream",
            )
            .expect("append source attachment message");
    }

    let store = EventStore::open(source.paths().event_store.clone()).expect("open source store");
    let blob_store = BlobStore::open(source.paths().blob_store.clone()).expect("open blob store");
    let server = rt
        .block_on(DirectPeerServer::bind_with_blobs(
            "127.0.0.1:0",
            store,
            blob_store,
        ))
        .expect("bind direct benchmark peer");
    let endpoint = server
        .local_addr()
        .expect("read local benchmark peer address")
        .to_string();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = rt.spawn(async move {
        let _ = server.serve_until_shutdown(shutdown_rx).await;
    });

    DirectSyncFixture {
        _temp_dir: temp_dir,
        target,
        workspace_id,
        peer: PeerAddress {
            peer_id: PeerId("benchmark-source".to_owned()),
            endpoint,
        },
        shutdown: Some(shutdown_tx),
        server_task,
    }
}

fn take_ffi_string(value: *mut c_char) -> String {
    assert!(!value.is_null(), "FFI returned null string");
    let text = unsafe { CStr::from_ptr(value).to_string_lossy().into_owned() };
    unsafe { chaft_string_free(value) };
    text
}

fn bench_runtime_append(c: &mut Criterion) {
    c.bench_function("runtime_append/encrypted_message_after_128_events", |b| {
        b.iter_batched(
            || runtime_fixture(BASE_MESSAGE_COUNT),
            |fixture| {
                let created = fixture
                    .runtime
                    .send_message(
                        fixture.workspace_id,
                        fixture.channel_id,
                        "new benchmark message",
                    )
                    .expect("append benchmark message");
                black_box(created.event_id);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_snapshot_hydration(c: &mut Criterion) {
    let fixture = runtime_fixture(SNAPSHOT_MESSAGE_COUNT);
    c.bench_function("snapshot_hydration/decrypted_latest_256_events", |b| {
        b.iter(|| {
            let snapshot = fixture
                .runtime
                .decrypted_workspace_snapshot(black_box(fixture.workspace_id.clone()))
                .expect("hydrate decrypted snapshot");
            black_box(snapshot.timeline.len());
        });
    });
}

fn bench_search(c: &mut Criterion) {
    let fixture = runtime_fixture(SNAPSHOT_MESSAGE_COUNT);
    c.bench_function("search/message_query_256_events", |b| {
        b.iter(|| {
            let hits = fixture
                .runtime
                .search_workspace_messages(
                    black_box(fixture.workspace_id.clone()),
                    black_box("search-token"),
                )
                .expect("search benchmark workspace");
            black_box(hits.hit_count);
        });
    });
}

fn bench_direct_sync(c: &mut Criterion) {
    let rt = Runtime::new().expect("create benchmark tokio runtime");
    c.bench_function("sync_pull/direct_tcp_96_events", |b| {
        b.iter_batched(
            || direct_sync_fixture(&rt, SYNC_MESSAGE_COUNT, false),
            |fixture| {
                rt.block_on(async move {
                    let report = fixture
                        .target
                        .pull_workspace_direct(
                            &DirectTransport,
                            &fixture.peer,
                            fixture.workspace_id.clone(),
                        )
                        .await
                        .expect("pull benchmark workspace");
                    black_box(report.fetched_event_count);
                    fixture.shutdown().await;
                });
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_blob_transfer(c: &mut Criterion) {
    let rt = Runtime::new().expect("create benchmark tokio runtime");
    c.bench_function("blob_transfer/direct_pull_256kb_attachment", |b| {
        b.iter_batched(
            || direct_sync_fixture(&rt, 24, true),
            |fixture| {
                rt.block_on(async move {
                    let report = fixture
                        .target
                        .pull_workspace_direct(
                            &DirectTransport,
                            &fixture.peer,
                            fixture.workspace_id.clone(),
                        )
                        .await
                        .expect("pull benchmark attachment");
                    black_box(report.fetched_blob_count);
                    fixture.shutdown().await;
                });
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_ffi_json(c: &mut Criterion) {
    let fixture = runtime_fixture(SNAPSHOT_MESSAGE_COUNT);
    let data_dir = CString::new(
        fixture
            .runtime
            .paths()
            .data_dir
            .to_string_lossy()
            .into_owned(),
    )
    .expect("runtime path has no interior nul");
    let workspace_id =
        CString::new(fixture.workspace_id.0.clone()).expect("workspace ID has no interior nul");
    c.bench_function("ffi_json/decrypted_latest_snapshot_256_events", |b| {
        b.iter(|| {
            let json = unsafe {
                chaft_decrypted_workspace_snapshot_from_runtime_latest_result_json(
                    data_dir.as_ptr(),
                    std::ptr::null(),
                    workspace_id.as_ptr(),
                    128,
                )
            };
            black_box(take_ffi_string(json).len());
        });
    });
}

criterion_group!(
    hot_paths,
    bench_runtime_append,
    bench_snapshot_hydration,
    bench_search,
    bench_direct_sync,
    bench_blob_transfer,
    bench_ffi_json
);
criterion_main!(hot_paths);
