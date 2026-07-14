use std::{
    ffi::c_char,
    io::Read,
    path::{Path, PathBuf},
};

use chaft_runtime::{
    AddedChannelMember, AddedOpenMlsChannelGroupMember, AddedOpenMlsWorkspaceGroupMember,
    AddedReaction, AppliedOpenMlsChannelGroupCommits, AppliedOpenMlsWorkspaceGroupCommits,
    ClaimedWorkspaceInvite, CreatedChannel, CreatedMessage, CreatedOpenMlsChannelGroup,
    CreatedOpenMlsWorkspaceGroup, CreatedWorkspace, CreatedWorkspaceInvite, DeletedMessage,
    EditedMessage, ImportedWorkspaceInviteResponse, InvitedMember, JoinedOpenMlsChannelGroup,
    JoinedOpenMlsWorkspaceGroup, MarkedChannelRead, PrunedBlobCache, PublishPeerEndpointRequest,
    PublishedDeviceKeyPackage, PublishedOpenMlsKeyPackage, PublishedPeerEndpoint,
    RecordedWorkspaceInvite, RecordedWorkspaceJoinRequest, RemovedChannelMember,
    RemovedChannelMemberWithKeyRotation, RemovedChannelMemberWithOpenMls, RemovedMember,
    RemovedMemberWithKeyRotation, RemovedMemberWithOpenMls, RemovedOpenMlsChannelGroupMember,
    RemovedOpenMlsWorkspaceGroupMember, RemovedReaction, ResolvedWorkspaceInvite,
    ResolvedWorkspaceJoinRequest, SavedAttachment, UpdatedChannelDetails, UpdatedDeviceProfile,
    UpdatedMemberRole, UpdatedOpenMlsChannelGroup, UpdatedOpenMlsWorkspaceGroup,
    UpdatedPersonProfile, UpdatedWorkspaceAccessPolicy, UpdatedWorkspaceOpenMlsGroups,
    WorkspaceInviteArtifact, WorkspaceInviteClaim, WorkspaceInviteResponse,
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
    input::{
        optional_c_string, parse_workspace_access_policy, parse_workspace_invite_resolution,
        parse_workspace_join_request_resolution, parse_workspace_role, read_c_string,
    },
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

pub(crate) fn runtime_create_workspace_with_access_policy_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    name: *const c_char,
    default_channel_name: *const c_char,
    access_policy: *const c_char,
) -> FfiResult<CreatedWorkspace> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let name = read_c_string(name, "name")?;
        let default_channel_name = read_c_string(default_channel_name, "default_channel_name")?;
        let access_policy =
            parse_workspace_access_policy(&read_c_string(access_policy, "access_policy")?)?;
        runtime
            .create_workspace_with_access_policy(name, default_channel_name, access_policy)
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

pub(crate) fn runtime_create_direct_message_channel_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    name: *const c_char,
    participant_device_id: *const c_char,
) -> FfiResult<CreatedChannel> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let name = read_c_string(name, "name")?;
        let participant_device_id = ffi_device_id_arg(read_c_string(
            participant_device_id,
            "participant_device_id",
        )?)?;
        runtime
            .create_direct_message_channel(
                WorkspaceId(workspace_id),
                name,
                DeviceId(participant_device_id),
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_create_direct_message_channel_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_update_channel_details_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    name: *const c_char,
    topic: *const c_char,
) -> FfiResult<UpdatedChannelDetails> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        let name = optional_c_string(name, "name")?;
        let topic = optional_c_string(topic, "topic")?;
        runtime
            .update_channel_details(
                WorkspaceId(workspace_id),
                ChannelId(channel_id),
                name,
                topic,
            )
            .map_err(|error| ffi_error("runtime_update_channel_details_failed", error.to_string()))
    })
}

pub(crate) fn runtime_update_channel_archive_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
    archived: bool,
) -> FfiResult<UpdatedChannelDetails> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .update_channel_archive(WorkspaceId(workspace_id), ChannelId(channel_id), archived)
            .map_err(|error| ffi_error("runtime_update_channel_archive_failed", error.to_string()))
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

pub(crate) fn runtime_update_local_person_profile_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
) -> FfiResult<UpdatedPersonProfile> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        runtime
            .update_local_person_profile(WorkspaceId(workspace_id), display_name)
            .map_err(|error| {
                ffi_error(
                    "runtime_update_local_person_profile_failed",
                    error.to_string(),
                )
            })
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_create_workspace_invite_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    expires_at: *const c_char,
    peer_endpoint: *const c_char,
    sync_expectation: *const c_char,
) -> FfiResult<CreatedWorkspaceInvite> {
    runtime_create_workspace_invite_with_max_claims_result(
        data_dir,
        identity_file,
        workspace_id,
        display_name,
        role,
        1,
        expires_at,
        peer_endpoint,
        sync_expectation,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_create_workspace_invite_with_max_claims_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    max_claims: u32,
    expires_at: *const c_char,
    peer_endpoint: *const c_char,
    sync_expectation: *const c_char,
) -> FfiResult<CreatedWorkspaceInvite> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        let role = parse_workspace_role(&read_c_string(role, "role")?)?;
        let expires_at = read_c_string(expires_at, "expires_at")?;
        let peer_endpoint = read_c_string(peer_endpoint, "peer_endpoint")?;
        let sync_expectation = read_c_string(sync_expectation, "sync_expectation")?;
        runtime
            .create_workspace_invite_with_max_claims(
                WorkspaceId(workspace_id),
                display_name,
                role,
                max_claims,
                expires_at,
                peer_endpoint,
                sync_expectation,
            )
            .map_err(|error| ffi_error("runtime_create_workspace_invite_failed", error.to_string()))
    })
}

pub(crate) fn runtime_prepare_workspace_invite_claim_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    artifact_json: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    response_peer_endpoint: *const c_char,
) -> FfiResult<WorkspaceInviteClaim> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let artifact_json = read_c_string(artifact_json, "artifact_json")?;
        let artifact = serde_json::from_str::<WorkspaceInviteArtifact>(&artifact_json)
            .map_err(|error| ffi_error("workspace_invite_invalid", error.to_string()))?;
        let display_name = read_c_string(display_name, "display_name")?;
        let note = read_c_string(note, "note")?;
        let response_peer_endpoint =
            read_c_string(response_peer_endpoint, "response_peer_endpoint")?;
        runtime
            .prepare_workspace_invite_claim(artifact, display_name, note, response_peer_endpoint)
            .map_err(|error| {
                ffi_error(
                    "runtime_prepare_workspace_invite_claim_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_claim_workspace_invite_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    claim_json: *const c_char,
) -> FfiResult<ClaimedWorkspaceInvite> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let claim_json = read_c_string(claim_json, "claim_json")?;
        let claim = serde_json::from_str::<WorkspaceInviteClaim>(&claim_json)
            .map_err(|error| ffi_error("workspace_invite_claim_invalid", error.to_string()))?;
        runtime
            .claim_workspace_invite(claim)
            .map_err(|error| ffi_error("runtime_claim_workspace_invite_failed", error.to_string()))
    })
}

pub(crate) fn runtime_import_workspace_invite_response_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    response_json: *const c_char,
) -> FfiResult<ImportedWorkspaceInviteResponse> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let response_json = read_c_string(response_json, "response_json")?;
        let response = serde_json::from_str::<WorkspaceInviteResponse>(&response_json)
            .map_err(|error| ffi_error("workspace_invite_response_invalid", error.to_string()))?;
        runtime
            .import_workspace_invite_response(response)
            .map_err(|error| {
                ffi_error(
                    "runtime_import_workspace_invite_response_failed",
                    error.to_string(),
                )
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_record_workspace_join_request_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    source_type: *const c_char,
    source_invite_id: *const c_char,
    source_display_name: *const c_char,
    source_approval_policy: *const c_char,
) -> FfiResult<RecordedWorkspaceJoinRequest> {
    runtime_record_workspace_join_request_with_response_route_result(
        data_dir,
        identity_file,
        workspace_id,
        request_id,
        device_id,
        display_name,
        note,
        source_type,
        source_invite_id,
        source_display_name,
        source_approval_policy,
        std::ptr::null(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_record_workspace_join_request_with_response_route_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    note: *const c_char,
    source_type: *const c_char,
    source_invite_id: *const c_char,
    source_display_name: *const c_char,
    source_approval_policy: *const c_char,
    response_peer_endpoint: *const c_char,
) -> FfiResult<RecordedWorkspaceJoinRequest> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let request_id = read_c_string(request_id, "request_id")?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        let note = read_c_string(note, "join_request_note")?;
        let source_type = read_c_string(source_type, "join_request_source")?;
        let source_invite_id = read_c_string(source_invite_id, "source_invite_id")?;
        let source_display_name = read_c_string(source_display_name, "source_display_name")?;
        let source_approval_policy =
            read_c_string(source_approval_policy, "source_approval_policy")?;
        let response_peer_endpoint =
            optional_c_string(response_peer_endpoint, "response_peer_endpoint")?
                .unwrap_or_default();
        runtime
            .record_workspace_join_request_with_response_route(
                WorkspaceId(workspace_id),
                request_id,
                DeviceId(device_id),
                display_name,
                note,
                source_type,
                source_invite_id,
                source_display_name,
                source_approval_policy,
                response_peer_endpoint,
            )
            .map_err(|error| {
                ffi_error(
                    "runtime_record_workspace_join_request_failed",
                    error.to_string(),
                )
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn runtime_record_workspace_invite_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    invite_id: *const c_char,
    device_id: *const c_char,
    display_name: *const c_char,
    role: *const c_char,
    request_id: *const c_char,
    expires_at: *const c_char,
    approval_policy: *const c_char,
    sync_expectation: *const c_char,
) -> FfiResult<RecordedWorkspaceInvite> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let invite_id = read_c_string(invite_id, "invite_id")?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        let display_name = read_c_string(display_name, "display_name")?;
        let role = parse_workspace_role(&read_c_string(role, "role")?)?;
        let request_id = optional_c_string(request_id, "request_id")?;
        let expires_at = read_c_string(expires_at, "timestamp")?;
        let approval_policy = read_c_string(approval_policy, "invite_approval_policy")?;
        let sync_expectation = read_c_string(sync_expectation, "invite_sync_expectation")?;
        runtime
            .record_workspace_invite(
                WorkspaceId(workspace_id),
                invite_id,
                DeviceId(device_id),
                display_name,
                role,
                request_id,
                expires_at,
                approval_policy,
                sync_expectation,
            )
            .map_err(|error| ffi_error("runtime_record_workspace_invite_failed", error.to_string()))
    })
}

pub(crate) fn runtime_resolve_workspace_invite_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    invite_id: *const c_char,
    resolution: *const c_char,
) -> FfiResult<ResolvedWorkspaceInvite> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let invite_id = read_c_string(invite_id, "invite_id")?;
        let resolution =
            parse_workspace_invite_resolution(&read_c_string(resolution, "invite_resolution")?)?;
        runtime
            .resolve_workspace_invite(WorkspaceId(workspace_id), invite_id, resolution)
            .map_err(|error| {
                ffi_error("runtime_resolve_workspace_invite_failed", error.to_string())
            })
    })
}

pub(crate) fn runtime_resolve_workspace_join_request_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    request_id: *const c_char,
    resolution: *const c_char,
) -> FfiResult<ResolvedWorkspaceJoinRequest> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let request_id = read_c_string(request_id, "request_id")?;
        let resolution = parse_workspace_join_request_resolution(&read_c_string(
            resolution,
            "join_request_resolution",
        )?)?;
        runtime
            .resolve_workspace_join_request(WorkspaceId(workspace_id), request_id, resolution)
            .map_err(|error| {
                ffi_error(
                    "runtime_resolve_workspace_join_request_failed",
                    error.to_string(),
                )
            })
    })
}

pub(crate) fn runtime_update_member_role_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    device_id: *const c_char,
    role: *const c_char,
) -> FfiResult<UpdatedMemberRole> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let device_id = ffi_device_id_arg(read_c_string(device_id, "device_id")?)?;
        let role = parse_workspace_role(&read_c_string(role, "role")?)?;
        runtime
            .update_member_role(WorkspaceId(workspace_id), DeviceId(device_id), role)
            .map_err(|error| ffi_error("runtime_update_member_role_failed", error.to_string()))
    })
}

pub(crate) fn runtime_update_workspace_access_policy_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    access_policy: *const c_char,
) -> FfiResult<UpdatedWorkspaceAccessPolicy> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let access_policy =
            parse_workspace_access_policy(&read_c_string(access_policy, "access_policy")?)?;
        runtime
            .update_workspace_access_policy(WorkspaceId(workspace_id), access_policy)
            .map_err(|error| {
                ffi_error(
                    "runtime_update_workspace_access_policy_failed",
                    error.to_string(),
                )
            })
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
