use std::ffi::c_char;

use chaft_app::{WorkspaceChannelPage, WorkspaceChannelSearch, WorkspaceMemberPage};
use chaft_runtime::{
    IndexedWorkspaceSearch, LocalWorkspaceSummary, LocalWorkspaceSummaryPage, SearchedWorkspace,
    WorkspacePublishQueue, WorkspaceStorageHealth, WorkspaceStorageRepair,
};
use chaft_types::{ChannelId, WorkspaceId};
use serde::Serialize;

use crate::{
    envelope::{FfiResult, ffi_error, result_envelope},
    id_args::{ffi_channel_id_arg, ffi_workspace_id_arg},
    input::read_c_string,
    result_sampling::MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDevice {
    device_id: String,
}

pub(crate) fn runtime_device_id_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<RuntimeDevice> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        Ok(RuntimeDevice {
            device_id: runtime.device_id().0.clone(),
        })
    })
}

pub(crate) fn runtime_list_workspaces_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<Vec<LocalWorkspaceSummary>> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .list_workspace_page(0, MAX_RESULT_WORKSPACE_SUMMARY_SAMPLE_ROWS)
            .map(|page| page.workspaces)
            .map_err(|error| ffi_error("runtime_list_workspaces_failed", error.to_string()))
    })
}

pub(crate) fn runtime_list_workspace_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<LocalWorkspaceSummaryPage> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .list_workspace_page(start_index, limit)
            .map_err(|error| ffi_error("runtime_list_workspace_page_failed", error.to_string()))
    })
}

pub(crate) fn runtime_workspace_storage_health_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceStorageHealth> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .workspace_storage_health(workspace_id)
            .map_err(|error| {
                ffi_error("runtime_workspace_storage_health_failed", error.to_string())
            })
    })
}

pub(crate) fn runtime_repair_workspace_storage_metadata_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceStorageRepair> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .repair_workspace_storage_metadata(workspace_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_repair_workspace_storage_metadata_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_list_workspace_member_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<WorkspaceMemberPage> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .list_workspace_member_page(workspace_id, start_index, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_member_page_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_list_workspace_channel_page_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    start_index: usize,
    limit: usize,
) -> FfiResult<WorkspaceChannelPage> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        runtime
            .list_workspace_channel_page(workspace_id, start_index, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_channel_page_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_list_workspace_channel_page_containing_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    limit: usize,
) -> FfiResult<WorkspaceChannelPage> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        let channel_id = ChannelId(ffi_channel_id_arg(read_c_string(
            channel_id,
            "channel_id",
        )?)?);
        runtime
            .list_workspace_channel_page_containing(workspace_id, channel_id, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_list_workspace_channel_page_containing_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_search_workspace_channels_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
    limit: usize,
) -> FfiResult<WorkspaceChannelSearch> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        let query = read_c_string(query, "query")?;
        runtime
            .search_workspace_channels(workspace_id, query, limit)
            .map_err(|error| {
                ffi_error(
                    "runtime_search_workspace_channels_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_reindex_workspace_search_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<IndexedWorkspaceSearch> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .reindex_workspace_search(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error("runtime_reindex_workspace_search_failed", error.to_string())
            })
    })
}

pub(crate) fn runtime_search_workspace_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    query: *const c_char,
) -> FfiResult<SearchedWorkspace> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let query = read_c_string(query, "query")?;
        runtime
            .search_workspace_messages(WorkspaceId(workspace_id), query)
            .map_err(|error| ffi_error("runtime_search_workspace_failed", error.to_string()))
    })
}

pub(crate) fn runtime_workspace_publish_queue_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspacePublishQueue> {
    result_envelope(|| {
        let runtime = crate::open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .workspace_publish_queue(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_publish_queue_failed", error.to_string()))
    })
}
