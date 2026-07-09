use std::{ffi::c_char, path::PathBuf};

use chaft_net_direct::{DirectTransport, MAX_JOIN_REQUEST_SUBMISSION_BYTES};
use chaft_runtime::{
    BlobTransferRetryReport, PEER_ENDPOINT_MAX_BYTES, PublishedWorkspace, PulledWorkspace,
    SyncedWorkspace,
};
use chaft_types::{EventId, WorkspaceId};
use serde::Serialize;

use crate::{
    direct_network::run_direct_runtime_command,
    envelope::{FfiResult, ffi_error, result_envelope},
    id_args::{direct_event_id_arg, direct_workspace_id_arg},
    input::{
        PEER_ENDPOINT_LIST_TEXT_MAX_BYTES, optional_c_string, read_c_string,
        read_c_string_with_max_bytes,
    },
    peer_endpoint::{direct_peer_address, direct_peer_addresses},
    result_sampling::{
        sample_blob_transfer_retry_report, sample_published_workspace_report,
        sample_pulled_workspace_report, sample_synced_workspace_report,
    },
    worker::{run_on_worker_thread, run_runtime_future},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmittedJoinRequestDirect {
    peer_endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_id: Option<String>,
    request_byte_len: usize,
}

pub(crate) fn runtime_publish_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.publish_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_publish_workspace_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

pub(crate) fn runtime_backup_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.backup_workspace_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    WorkspaceId(workspace_id),
                ),
                "runtime_backup_workspace_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

pub(crate) fn runtime_publish_event_with_trust_snapshot_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    event_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PublishedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let event_id = direct_event_id_arg(read_c_string(event_id, "event_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.publish_event_direct_with_trust_snapshot(
                    &transport,
                    &peer,
                    WorkspaceId(workspace_id),
                    EventId(event_id),
                ),
                "runtime_publish_event_with_trust_snapshot_failed",
            )
            .map(sample_published_workspace_report)
        })
    })
}

pub(crate) fn runtime_pull_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<PulledWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.pull_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_pull_workspace_failed",
            )
            .map(sample_pulled_workspace_report)
        })
    })
}

pub(crate) fn runtime_sync_workspace_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoint: *const c_char,
) -> FfiResult<SyncedWorkspace> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.sync_workspace_direct(&transport, &peer, WorkspaceId(workspace_id)),
                "runtime_sync_workspace_failed",
            )
            .map(sample_synced_workspace_report)
        })
    })
}

pub(crate) fn runtime_retry_blob_transfers_direct_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    peer_endpoints: *const c_char,
) -> FfiResult<BlobTransferRetryReport> {
    result_envelope(|| {
        let data_dir = read_c_string(data_dir, "data_dir")?;
        let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
        let workspace_id = direct_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let peer_endpoints = read_c_string_with_max_bytes(
            peer_endpoints,
            "peer_endpoints",
            PEER_ENDPOINT_LIST_TEXT_MAX_BYTES,
            "peer_endpoint_list_too_large",
            "peer endpoint list",
        )?;
        let peers = direct_peer_addresses(&peer_endpoints)?;

        run_direct_runtime_command(data_dir, identity_file, move |runtime, transport| {
            run_runtime_future(
                runtime.retry_pending_blob_transfers_direct(
                    &transport,
                    WorkspaceId(workspace_id),
                    &peers,
                ),
                "runtime_retry_blob_transfers_failed",
            )
            .map(sample_blob_transfer_retry_report)
        })
    })
}

pub(crate) fn runtime_submit_join_request_direct_result(
    peer_endpoint: *const c_char,
    workspace_id: *const c_char,
    request_json: *const c_char,
) -> FfiResult<SubmittedJoinRequestDirect> {
    result_envelope(|| {
        let peer_endpoint = read_c_string_with_max_bytes(
            peer_endpoint,
            "peer_endpoint",
            PEER_ENDPOINT_MAX_BYTES,
            "peer_endpoint_too_large",
            "peer endpoint",
        )?;
        let peer = direct_peer_address(peer_endpoint.clone())?;
        let workspace_id = optional_c_string(workspace_id, "workspace_id")?
            .map(|workspace_id| workspace_id.trim().to_owned())
            .filter(|workspace_id| !workspace_id.is_empty())
            .map(direct_workspace_id_arg)
            .transpose()?
            .map(WorkspaceId);
        let request_json = read_c_string_with_max_bytes(
            request_json,
            "request_json",
            MAX_JOIN_REQUEST_SUBMISSION_BYTES,
            "join_request_too_large",
            "join request",
        )?;
        let request_bytes = request_json.into_bytes();
        let request_byte_len = request_bytes.len();
        let result_workspace_id = workspace_id.as_ref().map(|id| id.0.clone());
        let result_peer_endpoint = peer_endpoint.trim().to_owned();

        run_on_worker_thread(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ffi_error("tokio_runtime_failed", error.to_string()))?;
            runtime
                .block_on(DirectTransport.submit_join_request(
                    &peer,
                    workspace_id.as_ref(),
                    request_bytes,
                ))
                .map_err(|error| {
                    ffi_error("runtime_submit_join_request_failed", error.to_string())
                })?;
            Ok(SubmittedJoinRequestDirect {
                peer_endpoint: result_peer_endpoint,
                workspace_id: result_workspace_id,
                request_byte_len,
            })
        })
    })
}
