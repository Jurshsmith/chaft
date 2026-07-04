use std::{
    ffi::c_char,
    io::Read,
    path::{Path, PathBuf},
};

use chaft_runtime::{
    AddedChannelMember, AddedOpenMlsChannelGroupMember, AddedOpenMlsWorkspaceGroupMember,
    AddedReaction, AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits,
    CreatedChannel, CreatedMessage, CreatedOpenMlsChannelGroup, CreatedOpenMlsWorkspaceGroup,
    CreatedWorkspace, DeletedMessage, EditedMessage, InvitedMember, JoinedOpenMlsChannelGroup,
    JoinedOpenMlsWorkspaceGroup, MarkedChannelRead, PrunedBlobCache, PublishPeerEndpointRequest,
    PublishedDeviceKeyPackage, PublishedOpenMlsKeyPackage, PublishedPeerEndpoint,
    RemovedChannelMember, RemovedChannelMemberWithKeyRotation, RemovedChannelMemberWithOpenMls,
    RemovedMember, RemovedMemberWithKeyRotation, RemovedMemberWithOpenMls,
    RemovedOpenMlsChannelGroupMember, RemovedOpenMlsWorkspaceGroupMember, RemovedReaction,
    SavedAttachment, UpdatedDeviceProfile, UpdatedOpenMlsChannelGroup,
    UpdatedOpenMlsWorkspaceGroup, UpdatedWorkspaceOpenMlsGroups,
};
use chaft_types::{
    ChannelId, DeviceId, DeviceKeyPackageId, MessageId, REPLICA_RETENTION_HINT_MAX_BYTES,
    ReplicaStorageClass, WorkspaceId,
};

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::{
        ffi_channel_id_arg, ffi_device_id_arg, ffi_device_key_package_id_arg, ffi_message_id_arg,
        ffi_optional_event_id_arg, ffi_optional_message_id_arg, ffi_workspace_id_arg,
    },
    input::{optional_c_string, parse_workspace_role, read_c_string},
    open_runtime_from_ffi,
    peer_endpoint::validate_peer_endpoint_hint_inputs,
    result_sampling::{
        sample_applied_openmls_channel_commits_report,
        sample_applied_openmls_workspace_commits_report, sample_pruned_blob_cache_report,
        sample_removed_member_with_key_rotation_report,
        sample_updated_workspace_openmls_groups_report,
    },
};

pub(crate) const DEVICE_KEY_PACKAGE_FILE_MAX_BYTES: u64 = 64 * 1024;

pub(crate) fn runtime_create_workspace_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    name: *const c_char,
    default_channel_name: *const c_char,
) -> FfiResult<CreatedWorkspace> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let name = read_c_string(name, "name")?;
        let default_channel_name = read_c_string(default_channel_name, "default_channel_name")?;
        runtime
            .create_workspace(name, default_channel_name)
            .map_err(|error| ffi_error("runtime_create_workspace_failed", error.to_string()))
    })
}

pub(crate) fn runtime_create_channel_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    name: *const c_char,
    is_private: bool,
) -> FfiResult<CreatedChannel> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let name = read_c_string(name, "name")?;
        runtime
            .create_channel(WorkspaceId(workspace_id), name, is_private)
            .map_err(|error| ffi_error("runtime_create_channel_failed", error.to_string()))
    })
}

pub(crate) fn runtime_update_device_profile_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
) -> FfiResult<UpdatedDeviceProfile> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        runtime
            .update_device_profile(WorkspaceId(workspace_id), display_name)
            .map_err(|error| ffi_error("runtime_update_device_profile_failed", error.to_string()))
    })
}

pub(crate) fn runtime_publish_device_key_package_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    protocol: *const c_char,
    key_package_file: *const c_char,
) -> FfiResult<PublishedDeviceKeyPackage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let protocol = read_c_string(protocol, "protocol")?;
        let key_package_file = read_c_string(key_package_file, "key_package_file")?;
        let key_package = read_device_key_package_file(Path::new(&key_package_file))?;
        runtime
            .publish_device_key_package(WorkspaceId(workspace_id), protocol, key_package)
            .map_err(|error| {
                ffi_error(
                    "runtime_publish_device_key_package_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) struct PeerEndpointFfiArgs {
    pub(crate) data_dir: *const c_char,
    pub(crate) identity_file: *const c_char,
    pub(crate) workspace_id: *const c_char,
    pub(crate) endpoint_id: *const c_char,
    pub(crate) endpoint: *const c_char,
    pub(crate) transport: *const c_char,
    pub(crate) is_backup_peer: bool,
    pub(crate) has_expires_at_ms: bool,
    pub(crate) expires_at_ms: i64,
    pub(crate) replica_storage_class: *const c_char,
    pub(crate) replica_retention_hint: *const c_char,
}

pub(crate) fn runtime_publish_peer_endpoint_result(
    args: PeerEndpointFfiArgs,
) -> FfiResult<PublishedPeerEndpoint> {
    result_envelope(|| {
        let workspace_id = ffi_workspace_id_arg(read_c_string(args.workspace_id, "workspace_id")?)?;
        let endpoint_id = read_c_string(args.endpoint_id, "endpoint_id")?;
        let endpoint = read_c_string(args.endpoint, "endpoint")?;
        let transport = read_c_string(args.transport, "transport")?;
        let (endpoint_id, endpoint, transport) =
            validate_peer_endpoint_hint_inputs(endpoint_id, endpoint, transport)?;
        let replica_storage_class =
            parse_optional_replica_storage_class(args.replica_storage_class)?;
        let replica_retention_hint =
            normalize_optional_replica_retention_hint(args.replica_retention_hint)?;
        if !args.is_backup_peer
            && (replica_storage_class.is_some() || replica_retention_hint.is_some())
        {
            return Err(ffi_error(
                "replica_capability_requires_backup_peer",
                "replica capability metadata requires a backup peer endpoint",
            ));
        }
        let runtime = open_runtime_from_ffi(args.data_dir, args.identity_file)?;
        runtime
            .publish_peer_endpoint_with_replica_capability(PublishPeerEndpointRequest {
                workspace_id: WorkspaceId(workspace_id),
                endpoint_id,
                endpoint,
                transport,
                is_backup_peer: args.is_backup_peer,
                expires_at_ms: args.has_expires_at_ms.then_some(args.expires_at_ms),
                replica_storage_class,
                replica_retention_hint,
            })
            .map_err(|error| ffi_error("runtime_publish_peer_endpoint_failed", error.to_string()))
    })
}

fn parse_optional_replica_storage_class(
    value: *const c_char,
) -> Result<Option<ReplicaStorageClass>, FfiError> {
    let Some(value) = optional_c_string(value, "replica_storage_class")? else {
        return Ok(None);
    };
    let normalized = value.trim().replace('-', "_");
    if normalized.is_empty() {
        return Err(ffi_error(
            "replica_storage_class_required",
            "replica storage class is required when provided",
        ));
    }
    ReplicaStorageClass::from_wire(&normalized)
        .map(Some)
        .ok_or_else(|| {
            ffi_error(
                "replica_storage_class_unsupported",
                format!(
                    "replica storage class must be one of: {}",
                    ReplicaStorageClass::supported_wire_values().join(", ")
                ),
            )
        })
}

fn normalize_optional_replica_retention_hint(
    value: *const c_char,
) -> Result<Option<String>, FfiError> {
    let Some(value) = optional_c_string(value, "replica_retention_hint")? else {
        return Ok(None);
    };
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "replica_retention_hint_required",
            "replica retention hint is required when provided",
        ));
    }
    if value.len() > REPLICA_RETENTION_HINT_MAX_BYTES {
        return Err(ffi_error(
            "replica_retention_hint_too_large",
            format!(
                "replica retention hint is too large ({} bytes, max {})",
                value.len(),
                REPLICA_RETENTION_HINT_MAX_BYTES
            ),
        ));
    }
    Ok(Some(value))
}

pub(crate) fn runtime_publish_openmls_device_key_package_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<PublishedOpenMlsKeyPackage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .publish_openmls_device_key_package(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_publish_openmls_device_key_package_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_create_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<CreatedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .create_openmls_workspace_group(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_create_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_add_openmls_workspace_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    key_package_id: *const c_char,
) -> FfiResult<AddedOpenMlsWorkspaceGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let key_package_id =
            ffi_device_key_package_id_arg(read_c_string(key_package_id, "key_package_id")?)?;
        runtime
            .add_openmls_workspace_group_member(
                WorkspaceId(workspace_id),
                DeviceKeyPackageId(key_package_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_add_openmls_workspace_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_remove_openmls_workspace_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedOpenMlsWorkspaceGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_openmls_workspace_group_member(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_openmls_workspace_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_join_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<JoinedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .join_openmls_workspace_group(WorkspaceId(workspace_id), source_event_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_join_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_update_openmls_workspace_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<UpdatedOpenMlsWorkspaceGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .update_openmls_workspace_group(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_openmls_workspace_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_update_workspace_openmls_groups_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<UpdatedWorkspaceOpenMlsGroups> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .update_workspace_openmls_groups(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_workspace_openmls_groups_failed",
                    error.to_string(),
                )
            })
            .map(sample_updated_workspace_openmls_groups_report)
    })
}

pub(crate) fn runtime_apply_openmls_workspace_group_commits_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<AppliedOpenMlsWorkspaceGroupCommits> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .apply_openmls_workspace_group_commits(WorkspaceId(workspace_id), source_event_id)
            .map_err(|error| {
                ffi_error(
                    "runtime_apply_openmls_workspace_group_commits_failed",
                    error.to_string(),
                )
            })
            .map(sample_applied_openmls_workspace_commits_report)
    })
}

pub(crate) fn runtime_create_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<CreatedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .create_openmls_channel_group(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_create_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_add_openmls_channel_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    key_package_id: *const c_char,
) -> FfiResult<AddedOpenMlsChannelGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let key_package_id =
            ffi_device_key_package_id_arg(read_c_string(key_package_id, "key_package_id")?)?;
        runtime
            .add_openmls_channel_group_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceKeyPackageId(key_package_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_add_openmls_channel_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_remove_openmls_channel_group_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedOpenMlsChannelGroupMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_openmls_channel_group_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_openmls_channel_group_member_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_join_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<JoinedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .join_openmls_channel_group(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                source_event_id,
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_join_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_update_openmls_channel_group_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<UpdatedOpenMlsChannelGroup> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .update_openmls_channel_group(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_update_openmls_channel_group_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_apply_openmls_channel_group_commits_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    source_event_id: *const c_char,
) -> FfiResult<AppliedOpenMlsChannelGroupCommits> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let source_event_id =
            ffi_optional_event_id_arg(optional_c_string(source_event_id, "source_event_id")?)?;
        runtime
            .apply_openmls_channel_group_commits(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                source_event_id,
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_apply_openmls_channel_group_commits_failed",
                    error.to_string(),
                )
            })
            .map(sample_applied_openmls_channel_commits_report)
    })
}

pub(crate) fn runtime_send_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .send_message(WorkspaceId(workspace_id), ChannelId(channel_id), text)
            .map_err(|error| ffi_error("runtime_send_message_failed", error.to_string()))
    })
}

pub(crate) fn runtime_send_message_reply_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let reply_to_message_id =
            ffi_message_id_arg(read_c_string(reply_to_message_id, "reply_to_message_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .send_message_reply(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                MessageId(reply_to_message_id),
                text,
            )
            .map_err(|error| ffi_error("runtime_send_message_failed", error.to_string()))
    })
}

pub(crate) fn runtime_send_attachment_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let text = read_c_string(text, "text")?;
        let file_path = read_c_string(file_path, "file_path")?;
        let media_type = read_c_string(media_type, "media_type")?;
        runtime
            .send_message_with_attachment_file(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                text,
                PathBuf::from(file_path),
                media_type,
            )
            .map_err(|error| ffi_error("runtime_send_attachment_failed", error.to_string()))
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_send_attachment_reply_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    reply_to_message_id: *const c_char,
    text: *const c_char,
    file_path: *const c_char,
    media_type: *const c_char,
) -> FfiResult<CreatedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let reply_to_message_id = ffi_optional_message_id_arg(optional_c_string(
            reply_to_message_id,
            "reply_to_message_id",
        )?)?;
        let text = read_c_string(text, "text")?;
        let file_path = read_c_string(file_path, "file_path")?;
        let media_type = read_c_string(media_type, "media_type")?;
        runtime
            .send_message_with_attachment_file_reply(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                reply_to_message_id,
                text,
                PathBuf::from(file_path),
                media_type,
            )
            .map_err(|error| ffi_error("runtime_send_attachment_failed", error.to_string()))
    })
}

pub(crate) fn runtime_save_attachment_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    blob_hash: *const c_char,
    output_path: *const c_char,
) -> FfiResult<SavedAttachment> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let attachment_selector = read_c_string(blob_hash, "blob_hash")?;
        let output_path = read_c_string(output_path, "output_path")?;
        runtime
            .save_attachment_to_file(
                WorkspaceId(workspace_id),
                MessageId(message_id),
                attachment_selector,
                PathBuf::from(output_path),
            )
            .map_err(|error| ffi_error("runtime_save_attachment_failed", error.to_string()))
    })
}

pub(crate) fn runtime_prune_blobs_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> FfiResult<PrunedBlobCache> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .prune_unreferenced_blobs()
            .map(sample_pruned_blob_cache_report)
            .map_err(|error| ffi_error("runtime_prune_blobs_failed", error.to_string()))
    })
}

pub(crate) fn runtime_edit_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    text: *const c_char,
) -> FfiResult<EditedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let text = read_c_string(text, "text")?;
        runtime
            .edit_message(WorkspaceId(workspace_id), MessageId(message_id), text)
            .map_err(|error| ffi_error("runtime_edit_message_failed", error.to_string()))
    })
}

pub(crate) fn runtime_delete_message_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
) -> FfiResult<DeletedMessage> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        runtime
            .delete_message(WorkspaceId(workspace_id), MessageId(message_id))
            .map_err(|error| ffi_error("runtime_delete_message_failed", error.to_string()))
    })
}

pub(crate) fn runtime_add_reaction_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> FfiResult<AddedReaction> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let reaction = read_c_string(reaction, "reaction")?;
        runtime
            .add_reaction(WorkspaceId(workspace_id), MessageId(message_id), reaction)
            .map_err(|error| ffi_error("runtime_add_reaction_failed", error.to_string()))
    })
}

pub(crate) fn runtime_remove_reaction_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    message_id: *const c_char,
    reaction: *const c_char,
) -> FfiResult<RemovedReaction> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let message_id = ffi_message_id_arg(read_c_string(message_id, "message_id")?)?;
        let reaction = read_c_string(reaction, "reaction")?;
        runtime
            .remove_reaction(WorkspaceId(workspace_id), MessageId(message_id), reaction)
            .map_err(|error| ffi_error("runtime_remove_reaction_failed", error.to_string()))
    })
}

pub(crate) fn runtime_mark_channel_read_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<MarkedChannelRead> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .mark_channel_read(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_mark_channel_read_failed", error.to_string()))
    })
}

pub(crate) fn runtime_invite_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
    role: *const c_char,
) -> FfiResult<InvitedMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        let role = parse_workspace_role(&read_c_string(role, "role")?)?;
        runtime
            .invite_member(WorkspaceId(workspace_id), DeviceId(device_id), role)
            .map_err(|error| ffi_error("runtime_invite_member_failed", error.to_string()))
    })
}

pub(crate) fn runtime_remove_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| ffi_error("runtime_remove_member_failed", error.to_string()))
    })
}

pub(crate) fn runtime_remove_member_with_openmls_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMemberWithOpenMls> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member_with_openmls(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_member_with_openmls_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_remove_member_with_key_rotation_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedMemberWithKeyRotation> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_member_with_key_rotation(WorkspaceId(workspace_id), DeviceId(device_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_member_with_key_rotation_failed",
                    error.to_string(),
                )
            })
            .map(sample_removed_member_with_key_rotation_report)
    })
}

pub(crate) fn runtime_add_channel_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<AddedChannelMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .add_channel_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| ffi_error("runtime_add_channel_member_failed", error.to_string()))
    })
}

pub(crate) fn runtime_remove_channel_member_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMember> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| ffi_error("runtime_remove_channel_member_failed", error.to_string()))
    })
}

pub(crate) fn runtime_remove_channel_member_with_openmls_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMemberWithOpenMls> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member_with_openmls(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_channel_member_with_openmls_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_remove_channel_member_with_key_rotation_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    device_id: *const c_char,
) -> FfiResult<RemovedChannelMemberWithKeyRotation> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        runtime
            .remove_channel_member_with_key_rotation(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                DeviceId(device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_remove_channel_member_with_key_rotation_failed",
                    error.to_string(),
                )
            })
    })
}

fn read_device_key_package_file(file_path: &Path) -> Result<Vec<u8>, FfiError> {
    let metadata = std::fs::metadata(file_path)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    if metadata.len() > DEVICE_KEY_PACKAGE_FILE_MAX_BYTES {
        return Err(ffi_error(
            "runtime_publish_device_key_package_failed",
            format!(
                "device key package is too large ({} bytes, max {})",
                metadata.len(),
                DEVICE_KEY_PACKAGE_FILE_MAX_BYTES
            ),
        ));
    }

    let file = std::fs::File::open(file_path)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    let mut limited_file = file.take(DEVICE_KEY_PACKAGE_FILE_MAX_BYTES + 1);
    let mut bytes = Vec::new();
    limited_file
        .read_to_end(&mut bytes)
        .map_err(|error| ffi_error("device_key_package_read_failed", error.to_string()))?;
    if bytes.len() as u64 > DEVICE_KEY_PACKAGE_FILE_MAX_BYTES {
        return Err(ffi_error(
            "runtime_publish_device_key_package_failed",
            format!(
                "device key package is too large ({} bytes, max {})",
                bytes.len(),
                DEVICE_KEY_PACKAGE_FILE_MAX_BYTES
            ),
        ));
    }
    Ok(bytes)
}
