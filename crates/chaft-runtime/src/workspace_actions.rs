use chaft_core::{WorkspaceState, authorize_event_with_history};
use chaft_crypto::seal_message_markdown;
use chaft_types::{
    CHANNEL_NAME_MAX_BYTES, CHANNEL_TOPIC_MAX_BYTES, ChannelId, DEVICE_DISPLAY_NAME_MAX_BYTES,
    DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES, DeviceId, DeviceKeyPackageId, EventBody, MessageId,
    PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_MAX_BYTES, PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
    PERSON_ID_MAX_BYTES, PersonId, REACTION_TEXT_MAX_BYTES, REPLICA_RETENTION_HINT_MAX_BYTES,
    ReplicaStorageClass, SignableEvent, WORKSPACE_ACCESS_POLICY_MAX_BYTES,
    WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES, WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
    WORKSPACE_INVITE_ID_MAX_BYTES, WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES,
    WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES, WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
    WORKSPACE_NAME_MAX_BYTES, WorkspaceAccessPolicy, WorkspaceId, WorkspaceInviteResolution,
    WorkspaceJoinRequestResolution, WorkspaceRole, peer_endpoint_hint_is_supported,
    peer_endpoint_hint_transport_is_consistent,
};
use serde::{Deserialize, Serialize};

use crate::{
    DEVICE_KEY_PACKAGE_MAX_LEN, LocalRuntime, PendingAttachment, RuntimeError,
    content_keys::{ChannelKey, RotatedChannelKey, RotatedWorkspaceKey, WorkspaceKey},
    runtime_validation::{validate_device_id_reference, validate_metadata_field_size},
    validate_channel_id_reference, validate_message_id_reference, validate_message_markdown_size,
    validate_workspace_id_reference,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWorkspace {
    pub workspace_id: String,
    pub channel_id: String,
    pub owner_device_id: String,
    pub access_policy: WorkspaceAccessPolicy,
    pub workspace_event_id: String,
    pub access_policy_event_id: Option<String>,
    pub channel_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedChannel {
    pub workspace_id: String,
    pub channel_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedDeviceProfile {
    pub workspace_id: String,
    pub device_id: String,
    pub display_name: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedPersonProfile {
    pub workspace_id: String,
    pub person_id: String,
    pub device_id: String,
    pub display_name: String,
    pub link_event_id: Option<String>,
    pub profile_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedDeviceKeyPackage {
    pub workspace_id: String,
    pub device_id: String,
    pub key_package_id: String,
    pub protocol: String,
    pub byte_len: usize,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPeerEndpoint {
    pub workspace_id: String,
    pub device_id: String,
    pub endpoint_id: String,
    pub endpoint: String,
    pub transport: String,
    pub is_backup_peer: bool,
    pub expires_at_ms: Option<i64>,
    pub replica_storage_class: Option<ReplicaStorageClass>,
    pub replica_retention_hint: Option<String>,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPeerEndpointRequest {
    pub workspace_id: WorkspaceId,
    pub endpoint_id: String,
    pub endpoint: String,
    pub transport: String,
    pub is_backup_peer: bool,
    pub expires_at_ms: Option<i64>,
    pub replica_storage_class: Option<ReplicaStorageClass>,
    pub replica_retention_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<String>,
    pub event_id: String,
    pub encrypted: bool,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAttachment {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub blob_hash: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub attachment_id: String,
    pub display_name: String,
    pub media_type: String,
    pub byte_len: u64,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrunedBlobCache {
    #[serde(default)]
    pub workspace_count: usize,
    pub workspace_ids: Vec<String>,
    #[serde(default)]
    pub referenced_blob_count: usize,
    pub referenced_blob_hashes: Vec<String>,
    #[serde(default)]
    pub removed_blob_count: usize,
    pub removed_blob_hashes: Vec<String>,
    #[serde(default)]
    pub removed_manifest_count: usize,
    pub removed_manifest_hashes: Vec<String>,
    #[serde(default)]
    pub removed_chunk_count: usize,
    pub removed_chunk_hashes: Vec<String>,
    #[serde(default)]
    pub removed_temp_file_count: usize,
    #[serde(default)]
    pub removed_temp_file_paths: Vec<String>,
}

impl PrunedBlobCache {
    pub(crate) fn from_parts(
        workspace_ids: Vec<String>,
        referenced_blob_hashes: Vec<String>,
        removed_blob_hashes: Vec<String>,
        removed_manifest_hashes: Vec<String>,
        removed_chunk_hashes: Vec<String>,
        removed_temp_file_paths: Vec<String>,
    ) -> Self {
        Self {
            workspace_count: workspace_ids.len(),
            workspace_ids,
            referenced_blob_count: referenced_blob_hashes.len(),
            referenced_blob_hashes,
            removed_blob_count: removed_blob_hashes.len(),
            removed_blob_hashes,
            removed_manifest_count: removed_manifest_hashes.len(),
            removed_manifest_hashes,
            removed_chunk_count: removed_chunk_hashes.len(),
            removed_chunk_hashes,
            removed_temp_file_count: removed_temp_file_paths.len(),
            removed_temp_file_paths,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub event_id: String,
    pub encrypted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedChannelDetails {
    pub workspace_id: String,
    pub channel_id: String,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub archived: Option<bool>,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedMessage {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedReaction {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub reaction: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedReaction {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub reaction: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkedChannelRead {
    pub workspace_id: String,
    pub channel_id: String,
    pub read_through_event_id: String,
    pub marker_event_id: Option<String>,
    pub already_read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitedMember {
    pub workspace_id: String,
    pub invitee_device_id: String,
    pub role: WorkspaceRole,
    pub event_id: String,
    pub openmls_member_add_event_id: Option<String>,
    pub openmls_epoch: Option<u64>,
    pub openmls_member_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedWorkspaceInvite {
    pub workspace_id: String,
    pub invite_id: String,
    pub invitee_device_id: String,
    pub display_name: String,
    pub role: WorkspaceRole,
    pub request_id: Option<String>,
    pub expires_at: String,
    pub approval_policy: String,
    pub sync_expectation: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkspaceInvite {
    pub workspace_id: String,
    pub invite_id: String,
    pub resolution: WorkspaceInviteResolution,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedWorkspaceJoinRequest {
    pub workspace_id: String,
    pub request_id: String,
    pub requester_device_id: String,
    pub display_name: String,
    pub note: String,
    pub source_type: String,
    pub source_invite_id: String,
    pub source_display_name: String,
    pub source_approval_policy: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedWorkspaceJoinRequest {
    pub workspace_id: String,
    pub request_id: String,
    pub resolution: WorkspaceJoinRequestResolution,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedMemberRole {
    pub workspace_id: String,
    pub member_device_id: String,
    pub role: WorkspaceRole,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedWorkspaceAccessPolicy {
    pub workspace_id: String,
    pub access_policy: WorkspaceAccessPolicy,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMember {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMemberWithOpenMls {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub openmls_event_id: String,
    pub removal_event_id: String,
    pub openmls_epoch: u64,
    pub openmls_member_count: usize,
    pub openmls_private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedMemberWithKeyRotation {
    pub workspace_id: String,
    pub removed_device_id: String,
    pub removal_event_id: String,
    pub workspace_key_rotation: RotatedWorkspaceKey,
    #[serde(default)]
    pub channel_key_rotation_count: usize,
    pub channel_key_rotations: Vec<RotatedChannelKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedChannelMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub event_id: String,
    pub openmls_member_add_event_id: Option<String>,
    pub openmls_epoch: Option<u64>,
    pub openmls_member_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMemberWithOpenMls {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub openmls_event_id: String,
    pub removal_event_id: String,
    pub openmls_epoch: u64,
    pub openmls_member_count: usize,
    pub openmls_private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedChannelMemberWithKeyRotation {
    pub workspace_id: String,
    pub channel_id: String,
    pub member_device_id: String,
    pub removal_event_id: String,
    pub channel_key_rotation: RotatedChannelKey,
}

impl LocalRuntime {
    pub fn create_workspace(
        &self,
        name: impl Into<String>,
        default_channel_name: impl Into<String>,
    ) -> Result<CreatedWorkspace, RuntimeError> {
        self.create_workspace_with_access_policy(
            name,
            default_channel_name,
            WorkspaceAccessPolicy::InviteOnly,
        )
    }

    pub fn create_workspace_with_access_policy(
        &self,
        name: impl Into<String>,
        default_channel_name: impl Into<String>,
        access_policy: WorkspaceAccessPolicy,
    ) -> Result<CreatedWorkspace, RuntimeError> {
        let name = name.into();
        let default_channel_name = default_channel_name.into();
        validate_metadata_field_size("workspace name", &name, WORKSPACE_NAME_MAX_BYTES)?;
        validate_metadata_field_size(
            "default channel name",
            &default_channel_name,
            CHANNEL_NAME_MAX_BYTES,
        )?;

        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let workspace_key = WorkspaceKey::generate(workspace_id.clone());
        self.save_workspace_key(&workspace_key)?;

        let workspace = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceCreated { name },
        );
        let workspace = self.identity.sign_event(workspace);
        self.store.append_event(&workspace)?;

        let access_policy_event_id = if access_policy == WorkspaceAccessPolicy::InviteOnly {
            None
        } else {
            let mut event = SignableEvent::new(
                workspace_id.clone(),
                None,
                self.identity.device_id().clone(),
                EventBody::WorkspaceAccessPolicyUpdated {
                    policy: access_policy,
                },
            );
            event.parents = vec![workspace.event_id.clone()];
            let event = self.sign_authorize_and_append(event)?;
            Some(event.event_id)
        };

        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: default_channel_name,
                is_private: false,
            },
        );
        channel.parents = vec![
            access_policy_event_id
                .clone()
                .unwrap_or_else(|| workspace.event_id.clone()),
        ];
        let channel = self.sign_authorize_and_append(channel)?;

        Ok(CreatedWorkspace {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            owner_device_id: self.identity.device_id().0.clone(),
            access_policy,
            workspace_event_id: workspace.event_id.0,
            access_policy_event_id: access_policy_event_id.map(|event_id| event_id.0),
            channel_event_id: channel.event_id.0,
        })
    }

    pub fn create_channel(
        &self,
        workspace_id: WorkspaceId,
        name: impl Into<String>,
        is_private: bool,
    ) -> Result<CreatedChannel, RuntimeError> {
        let name = name.into();
        validate_metadata_field_size("channel name", &name, CHANNEL_NAME_MAX_BYTES)?;

        let channel_id = ChannelId::new();
        let context = self.workspace_write_context(&workspace_id)?;
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name,
                is_private,
            },
        );
        channel.parents = context.head_event_ids.clone();
        let channel_key =
            is_private.then(|| ChannelKey::generate(workspace_id.clone(), channel_id.clone()));
        let channel = self.identity.sign_event(channel);
        authorize_event_with_history(&context.events, &channel)?;
        if let Some(channel_key) = channel_key.as_ref() {
            self.save_channel_key(channel_key)?;
        }
        self.store.append_event(&channel)?;

        Ok(CreatedChannel {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            event_id: channel.event_id.0,
        })
    }

    pub fn create_direct_message_channel(
        &self,
        workspace_id: WorkspaceId,
        name: impl Into<String>,
        participant_device_id: DeviceId,
    ) -> Result<CreatedChannel, RuntimeError> {
        let name = name.into();
        validate_metadata_field_size("direct message name", &name, CHANNEL_NAME_MAX_BYTES)?;
        validate_device_id_reference(&participant_device_id)?;

        let channel_id = ChannelId::new();
        let context = self.workspace_write_context(&workspace_id)?;
        let mut participant_device_ids =
            vec![self.identity.device_id().clone(), participant_device_id];
        participant_device_ids.sort_by(|left, right| left.0.cmp(&right.0));
        participant_device_ids.dedup();
        let mut channel = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DirectMessageChannelCreated {
                channel_id: channel_id.clone(),
                name,
                participant_device_ids,
            },
        );
        channel.parents = context.head_event_ids.clone();
        let channel_key = ChannelKey::generate(workspace_id.clone(), channel_id.clone());
        let channel = self.identity.sign_event(channel);
        authorize_event_with_history(&context.events, &channel)?;
        self.save_channel_key(&channel_key)?;
        self.store.append_event(&channel)?;

        Ok(CreatedChannel {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            event_id: channel.event_id.0,
        })
    }

    pub fn update_channel_details(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        name: Option<String>,
        topic: Option<String>,
    ) -> Result<UpdatedChannelDetails, RuntimeError> {
        validate_channel_id_reference(&channel_id)?;
        if name.is_none() && topic.is_none() {
            return Err(RuntimeError::MetadataFieldRequired {
                field: "channel details",
            });
        }

        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty());
        if name.is_none() && topic.is_none() {
            return Err(RuntimeError::MetadataFieldRequired {
                field: "channel name",
            });
        }
        if let Some(name) = name.as_ref() {
            validate_metadata_field_size("channel name", name, CHANNEL_NAME_MAX_BYTES)?;
        }
        let topic = topic.map(|topic| topic.trim().to_owned());
        if let Some(topic) = topic.as_ref() {
            validate_metadata_field_size("channel topic", topic, CHANNEL_TOPIC_MAX_BYTES)?;
        }

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelDetailsUpdated {
                channel_id: channel_id.clone(),
                name: name.clone(),
                topic: topic.clone(),
                archived: None,
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(UpdatedChannelDetails {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            name,
            topic,
            archived: None,
            event_id: event.event_id.0,
        })
    }

    pub fn update_channel_archive(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        archived: bool,
    ) -> Result<UpdatedChannelDetails, RuntimeError> {
        validate_channel_id_reference(&channel_id)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelDetailsUpdated {
                channel_id: channel_id.clone(),
                name: None,
                topic: None,
                archived: Some(archived),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(UpdatedChannelDetails {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            name: None,
            topic: None,
            archived: Some(archived),
            event_id: event.event_id.0,
        })
    }

    pub fn update_device_profile(
        &self,
        workspace_id: WorkspaceId,
        display_name: impl AsRef<str>,
    ) -> Result<UpdatedDeviceProfile, RuntimeError> {
        let display_name = display_name.as_ref().trim().to_owned();
        if display_name.is_empty() {
            return Err(RuntimeError::DisplayNameRequired);
        }
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: display_name.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(UpdatedDeviceProfile {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            display_name,
            event_id: event.event_id.0,
        })
    }

    pub fn update_local_person_profile(
        &self,
        workspace_id: WorkspaceId,
        display_name: impl AsRef<str>,
    ) -> Result<UpdatedPersonProfile, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let display_name = display_name.as_ref().trim().to_owned();
        if display_name.is_empty() {
            return Err(RuntimeError::DisplayNameRequired);
        }
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&context.events)?;
        let device_id = self.identity.device_id();
        let person_id = state
            .person_device_links
            .get(device_id)
            .map(|link| link.person_id.clone())
            .unwrap_or_else(PersonId::new);

        self.update_person_profile(workspace_id, person_id, display_name)
    }

    pub fn update_person_profile(
        &self,
        workspace_id: WorkspaceId,
        person_id: PersonId,
        display_name: impl AsRef<str>,
    ) -> Result<UpdatedPersonProfile, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_metadata_field_size("person ID", &person_id.0, PERSON_ID_MAX_BYTES)?;
        let display_name = display_name.as_ref().trim().to_owned();
        if display_name.is_empty() {
            return Err(RuntimeError::DisplayNameRequired);
        }
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&context.events)?;

        let device_id = self.identity.device_id().clone();
        let already_linked = state
            .person_device_links
            .get(&device_id)
            .is_some_and(|link| link.person_id == person_id);
        let mut history = context.events;
        let mut parents = context.head_event_ids;
        let link_event_id = if already_linked {
            None
        } else {
            let mut link = SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::PersonDeviceLinked {
                    person_id: person_id.clone(),
                    device_id: device_id.clone(),
                },
            );
            link.parents = parents;
            let link = self.sign_authorize_and_append_with_history(link, &history)?;
            parents = vec![link.event_id.clone()];
            let link_event_id = link.event_id.0.clone();
            history.push(link);
            Some(link_event_id)
        };

        let mut profile = SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::PersonProfileUpdated {
                person_id: person_id.clone(),
                display_name: display_name.clone(),
            },
        );
        profile.parents = parents;
        let profile = self.sign_authorize_and_append_with_history(profile, &history)?;

        Ok(UpdatedPersonProfile {
            workspace_id: workspace_id.0,
            person_id: person_id.0,
            device_id: device_id.0,
            display_name,
            link_event_id,
            profile_event_id: profile.event_id.0,
        })
    }

    pub fn publish_device_key_package(
        &self,
        workspace_id: WorkspaceId,
        protocol: impl AsRef<str>,
        key_package: Vec<u8>,
    ) -> Result<PublishedDeviceKeyPackage, RuntimeError> {
        let protocol = protocol.as_ref().trim().to_owned();
        if protocol.is_empty() {
            return Err(RuntimeError::DeviceKeyPackageProtocolRequired);
        }
        validate_metadata_field_size(
            "device key package protocol",
            &protocol,
            DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
        )?;
        if key_package.is_empty() {
            return Err(RuntimeError::DeviceKeyPackageRequired);
        }
        if key_package.len() > DEVICE_KEY_PACKAGE_MAX_LEN {
            return Err(RuntimeError::DeviceKeyPackageTooLarge);
        }

        let key_package_id = DeviceKeyPackageId::new();
        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: protocol.clone(),
                key_package,
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        let byte_len = match &event.event.body {
            EventBody::DeviceKeyPackagePublished { key_package, .. } => key_package.len(),
            _ => 0,
        };

        Ok(PublishedDeviceKeyPackage {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            key_package_id: key_package_id.0,
            protocol,
            byte_len,
            event_id: event.event_id.0,
        })
    }

    pub fn publish_peer_endpoint(
        &self,
        workspace_id: WorkspaceId,
        endpoint_id: impl AsRef<str>,
        endpoint: impl AsRef<str>,
        transport: impl AsRef<str>,
        is_backup_peer: bool,
        expires_at_ms: Option<i64>,
    ) -> Result<PublishedPeerEndpoint, RuntimeError> {
        self.publish_peer_endpoint_with_replica_capability(PublishPeerEndpointRequest {
            workspace_id,
            endpoint_id: endpoint_id.as_ref().to_owned(),
            endpoint: endpoint.as_ref().to_owned(),
            transport: transport.as_ref().to_owned(),
            is_backup_peer,
            expires_at_ms,
            replica_storage_class: None,
            replica_retention_hint: None,
        })
    }

    pub fn publish_peer_endpoint_with_replica_capability(
        &self,
        request: PublishPeerEndpointRequest,
    ) -> Result<PublishedPeerEndpoint, RuntimeError> {
        let PublishPeerEndpointRequest {
            workspace_id,
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            expires_at_ms,
            replica_storage_class,
            replica_retention_hint,
        } = request;

        let endpoint_id = endpoint_id.trim().to_owned();
        if endpoint_id.is_empty() {
            return Err(RuntimeError::PeerEndpointIdRequired);
        }
        validate_metadata_field_size("peer endpoint ID", &endpoint_id, PEER_ENDPOINT_ID_MAX_BYTES)?;
        let endpoint = endpoint.trim().to_owned();
        if endpoint.is_empty() {
            return Err(RuntimeError::PeerEndpointRequired);
        }
        validate_metadata_field_size("peer endpoint", &endpoint, PEER_ENDPOINT_MAX_BYTES)?;
        if !peer_endpoint_hint_is_supported(&endpoint) {
            return Err(RuntimeError::UnsupportedPeerEndpoint);
        }
        let transport = transport.trim().to_owned();
        if transport.is_empty() {
            return Err(RuntimeError::PeerEndpointTransportRequired);
        }
        validate_metadata_field_size(
            "peer endpoint transport",
            &transport,
            PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
        )?;
        if !peer_endpoint_hint_transport_is_consistent(&endpoint, &transport) {
            return Err(RuntimeError::PeerEndpointTransportMismatch);
        }
        let replica_retention_hint = match replica_retention_hint {
            Some(hint) => {
                let hint = hint.trim().to_owned();
                if hint.is_empty() {
                    return Err(RuntimeError::MetadataFieldRequired {
                        field: "replica retention hint",
                    });
                }
                validate_metadata_field_size(
                    "replica retention hint",
                    &hint,
                    REPLICA_RETENTION_HINT_MAX_BYTES,
                )?;
                Some(hint)
            }
            None => None,
        };
        if !is_backup_peer && (replica_storage_class.is_some() || replica_retention_hint.is_some())
        {
            return Err(RuntimeError::ReplicaCapabilityRequiresBackupPeer);
        }

        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: endpoint_id.clone(),
                endpoint: endpoint.clone(),
                transport: transport.clone(),
                is_backup_peer,
                expires_at_ms,
                replica_storage_class,
                replica_retention_hint: replica_retention_hint.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(PublishedPeerEndpoint {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            expires_at_ms,
            replica_storage_class,
            replica_retention_hint,
            event_id: event.event_id.0,
        })
    }

    pub fn invite_member(
        &self,
        workspace_id: WorkspaceId,
        invitee_device_id: DeviceId,
        role: WorkspaceRole,
    ) -> Result<InvitedMember, RuntimeError> {
        validate_device_id_reference(&invitee_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut invite = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberInvited {
                invitee_device_id: invitee_device_id.clone(),
                role,
            },
        );
        invite.parents = context.head_event_ids.clone();
        let invite = self.sign_authorize_and_append_with_history(invite, &context.events)?;
        let openmls =
            self.auto_add_openmls_workspace_member_if_ready(&workspace_id, &invitee_device_id);

        Ok(InvitedMember {
            workspace_id: workspace_id.0,
            invitee_device_id: invitee_device_id.0,
            role,
            event_id: invite.event_id.0,
            openmls_member_add_event_id: openmls.as_ref().map(|added| added.event_id.clone()),
            openmls_epoch: openmls.as_ref().map(|added| added.epoch),
            openmls_member_count: openmls.as_ref().map(|added| added.member_count),
        })
    }

    pub fn record_workspace_join_request(
        &self,
        workspace_id: WorkspaceId,
        request_id: String,
        requester_device_id: DeviceId,
        display_name: String,
        note: String,
        source_type: String,
        source_invite_id: String,
        source_display_name: String,
        source_approval_policy: String,
    ) -> Result<RecordedWorkspaceJoinRequest, RuntimeError> {
        validate_device_id_reference(&requester_device_id)?;
        let request_id = request_id.trim().to_owned();
        let display_name = display_name.trim().to_owned();
        let note = note.trim().to_owned();
        let source_type = source_type.trim().to_owned();
        let source_invite_id = source_invite_id.trim().to_owned();
        let source_display_name = source_display_name.trim().to_owned();
        let source_approval_policy = source_approval_policy.trim().to_owned();
        if request_id.is_empty() {
            return Err(RuntimeError::MetadataFieldRequired {
                field: "join request ID",
            });
        }
        validate_metadata_field_size(
            "join request ID",
            &request_id,
            WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
        )?;
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;
        validate_metadata_field_size(
            "join request note",
            &note,
            WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "join request source",
            &source_type,
            WORKSPACE_ACCESS_POLICY_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "source invite ID",
            &source_invite_id,
            WORKSPACE_INVITE_ID_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "source display name",
            &source_display_name,
            DEVICE_DISPLAY_NAME_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "source approval policy",
            &source_approval_policy,
            WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES,
        )?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut request = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceJoinRequestRecorded {
                request_id: request_id.clone(),
                requester_device_id: requester_device_id.clone(),
                display_name: display_name.clone(),
                note: note.clone(),
                source_type: source_type.clone(),
                source_invite_id: source_invite_id.clone(),
                source_display_name: source_display_name.clone(),
                source_approval_policy: source_approval_policy.clone(),
            },
        );
        request.parents = context.head_event_ids.clone();
        let request = self.sign_authorize_and_append_with_history(request, &context.events)?;

        Ok(RecordedWorkspaceJoinRequest {
            workspace_id: workspace_id.0,
            request_id,
            requester_device_id: requester_device_id.0,
            display_name,
            note,
            source_type,
            source_invite_id,
            source_display_name,
            source_approval_policy,
            event_id: request.event_id.0,
        })
    }

    pub fn record_workspace_invite(
        &self,
        workspace_id: WorkspaceId,
        invite_id: String,
        invitee_device_id: DeviceId,
        display_name: String,
        role: WorkspaceRole,
        request_id: Option<String>,
        expires_at: String,
        approval_policy: String,
        sync_expectation: String,
    ) -> Result<RecordedWorkspaceInvite, RuntimeError> {
        validate_device_id_reference(&invitee_device_id)?;
        let invite_id = invite_id.trim().to_owned();
        let display_name = display_name.trim().to_owned();
        let request_id = request_id
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let expires_at = expires_at.trim().to_owned();
        let approval_policy = approval_policy.trim().to_owned();
        let sync_expectation = sync_expectation.trim().to_owned();
        if invite_id.is_empty() {
            return Err(RuntimeError::MetadataFieldRequired { field: "invite ID" });
        }
        validate_metadata_field_size("invite ID", &invite_id, WORKSPACE_INVITE_ID_MAX_BYTES)?;
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;
        if let Some(request_id) = request_id.as_ref() {
            validate_metadata_field_size(
                "join request ID",
                request_id,
                WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
            )?;
        }
        validate_metadata_field_size(
            "invite expiry",
            &expires_at,
            WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "invite approval policy",
            &approval_policy,
            WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "invite sync expectation",
            &sync_expectation,
            WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES,
        )?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut invite = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceInviteRecorded {
                invite_id: invite_id.clone(),
                invitee_device_id: invitee_device_id.clone(),
                display_name: display_name.clone(),
                role,
                request_id: request_id.clone(),
                expires_at: expires_at.clone(),
                approval_policy: approval_policy.clone(),
                sync_expectation: sync_expectation.clone(),
            },
        );
        invite.parents = context.head_event_ids.clone();
        let invite = self.sign_authorize_and_append_with_history(invite, &context.events)?;

        Ok(RecordedWorkspaceInvite {
            workspace_id: workspace_id.0,
            invite_id,
            invitee_device_id: invitee_device_id.0,
            display_name,
            role,
            request_id,
            expires_at,
            approval_policy,
            sync_expectation,
            event_id: invite.event_id.0,
        })
    }

    pub fn resolve_workspace_invite(
        &self,
        workspace_id: WorkspaceId,
        invite_id: String,
        resolution: WorkspaceInviteResolution,
    ) -> Result<ResolvedWorkspaceInvite, RuntimeError> {
        let invite_id = invite_id.trim().to_owned();
        if invite_id.is_empty() {
            return Err(RuntimeError::MetadataFieldRequired { field: "invite ID" });
        }
        validate_metadata_field_size("invite ID", &invite_id, WORKSPACE_INVITE_ID_MAX_BYTES)?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut invite = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceInviteResolved {
                invite_id: invite_id.clone(),
                resolution,
            },
        );
        invite.parents = context.head_event_ids.clone();
        let invite = self.sign_authorize_and_append_with_history(invite, &context.events)?;

        Ok(ResolvedWorkspaceInvite {
            workspace_id: workspace_id.0,
            invite_id,
            resolution,
            event_id: invite.event_id.0,
        })
    }

    pub fn resolve_workspace_join_request(
        &self,
        workspace_id: WorkspaceId,
        request_id: String,
        resolution: WorkspaceJoinRequestResolution,
    ) -> Result<ResolvedWorkspaceJoinRequest, RuntimeError> {
        let request_id = request_id.trim().to_owned();
        if request_id.is_empty() {
            return Err(RuntimeError::MetadataFieldRequired {
                field: "join request ID",
            });
        }
        validate_metadata_field_size(
            "join request ID",
            &request_id,
            WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
        )?;

        let context = self.workspace_write_context(&workspace_id)?;
        let mut request = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceJoinRequestResolved {
                request_id: request_id.clone(),
                resolution,
            },
        );
        request.parents = context.head_event_ids.clone();
        let request = self.sign_authorize_and_append_with_history(request, &context.events)?;

        Ok(ResolvedWorkspaceJoinRequest {
            workspace_id: workspace_id.0,
            request_id,
            resolution,
            event_id: request.event_id.0,
        })
    }

    pub fn update_member_role(
        &self,
        workspace_id: WorkspaceId,
        member_device_id: DeviceId,
        role: WorkspaceRole,
    ) -> Result<UpdatedMemberRole, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut update = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberRoleUpdated {
                member_device_id: member_device_id.clone(),
                role,
            },
        );
        update.parents = context.head_event_ids.clone();
        let update = self.sign_authorize_and_append_with_history(update, &context.events)?;

        Ok(UpdatedMemberRole {
            workspace_id: workspace_id.0,
            member_device_id: member_device_id.0,
            role,
            event_id: update.event_id.0,
        })
    }

    pub fn update_workspace_access_policy(
        &self,
        workspace_id: WorkspaceId,
        access_policy: WorkspaceAccessPolicy,
    ) -> Result<UpdatedWorkspaceAccessPolicy, RuntimeError> {
        let context = self.workspace_write_context(&workspace_id)?;
        let mut update = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceAccessPolicyUpdated {
                policy: access_policy,
            },
        );
        update.parents = context.head_event_ids.clone();
        let update = self.sign_authorize_and_append_with_history(update, &context.events)?;

        Ok(UpdatedWorkspaceAccessPolicy {
            workspace_id: workspace_id.0,
            access_policy,
            event_id: update.event_id.0,
        })
    }

    pub fn remove_member(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMember, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut removal = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberRemoved {
                removed_device_id: removed_device_id.clone(),
            },
        );
        removal.parents = context.head_event_ids.clone();
        let removal = self.sign_authorize_and_append_with_history(removal, &context.events)?;

        Ok(RemovedMember {
            workspace_id: workspace_id.0,
            removed_device_id: removed_device_id.0,
            event_id: removal.event_id.0,
        })
    }

    pub fn remove_member_with_openmls(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMemberWithOpenMls, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let openmls = self.remove_openmls_workspace_group_member(
            workspace_id.clone(),
            removed_device_id.clone(),
        )?;
        let removal = self.remove_member(workspace_id, removed_device_id)?;

        Ok(RemovedMemberWithOpenMls {
            workspace_id: removal.workspace_id,
            removed_device_id: removal.removed_device_id,
            openmls_event_id: openmls.event_id,
            removal_event_id: removal.event_id,
            openmls_epoch: openmls.epoch,
            openmls_member_count: openmls.member_count,
            openmls_private_group_state_path: openmls.private_group_state_path,
        })
    }

    pub fn remove_member_with_key_rotation(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedMemberWithKeyRotation, RuntimeError> {
        validate_device_id_reference(&removed_device_id)?;
        let removal = self.remove_member(workspace_id.clone(), removed_device_id)?;
        let key_rotations = self.rotate_workspace_manual_keys(workspace_id)?;

        Ok(RemovedMemberWithKeyRotation {
            workspace_id: removal.workspace_id,
            removed_device_id: removal.removed_device_id,
            removal_event_id: removal.event_id,
            workspace_key_rotation: key_rotations.workspace_key_rotation,
            channel_key_rotation_count: key_rotations.channel_key_rotation_count,
            channel_key_rotations: key_rotations.channel_key_rotations,
        })
    }

    pub fn add_channel_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<AddedChannelMember, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut grant = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelMemberAdded {
                channel_id: channel_id.clone(),
                member_device_id: member_device_id.clone(),
            },
        );
        grant.parents = context.head_event_ids.clone();
        let grant = self.sign_authorize_and_append_with_history(grant, &context.events)?;
        let openmls = self.auto_add_openmls_channel_member_if_ready(
            &workspace_id,
            &channel_id,
            &member_device_id,
        );

        Ok(AddedChannelMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            member_device_id: member_device_id.0,
            event_id: grant.event_id.0,
            openmls_member_add_event_id: openmls.as_ref().map(|added| added.event_id.clone()),
            openmls_epoch: openmls.as_ref().map(|added| added.epoch),
            openmls_member_count: openmls.as_ref().map(|added| added.member_count),
        })
    }

    pub fn remove_channel_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMember, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let mut removal = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ChannelMemberRemoved {
                channel_id: channel_id.clone(),
                member_device_id: member_device_id.clone(),
            },
        );
        removal.parents = context.head_event_ids.clone();
        let removal = self.sign_authorize_and_append_with_history(removal, &context.events)?;

        Ok(RemovedChannelMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            member_device_id: member_device_id.0,
            event_id: removal.event_id.0,
        })
    }

    pub fn remove_channel_member_with_openmls(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMemberWithOpenMls, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let openmls = self.remove_openmls_channel_group_member(
            workspace_id.clone(),
            channel_id.clone(),
            member_device_id.clone(),
        )?;
        let removal = self.remove_channel_member(workspace_id, channel_id, member_device_id)?;

        Ok(RemovedChannelMemberWithOpenMls {
            workspace_id: removal.workspace_id,
            channel_id: removal.channel_id,
            member_device_id: removal.member_device_id,
            openmls_event_id: openmls.event_id,
            removal_event_id: removal.event_id,
            openmls_epoch: openmls.epoch,
            openmls_member_count: openmls.member_count,
            openmls_private_group_state_path: openmls.private_group_state_path,
        })
    }

    pub fn remove_channel_member_with_key_rotation(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        member_device_id: DeviceId,
    ) -> Result<RemovedChannelMemberWithKeyRotation, RuntimeError> {
        validate_device_id_reference(&member_device_id)?;
        let removal =
            self.remove_channel_member(workspace_id.clone(), channel_id.clone(), member_device_id)?;
        let channel_key_rotation = self.rotate_channel_key(workspace_id, channel_id)?;

        Ok(RemovedChannelMemberWithKeyRotation {
            workspace_id: removal.workspace_id,
            channel_id: removal.channel_id,
            member_device_id: removal.member_device_id,
            removal_event_id: removal.event_id,
            channel_key_rotation,
        })
    }

    pub fn send_message(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        self.send_message_with_attachments(workspace_id, channel_id, markdown, None, Vec::new())
    }

    pub fn send_message_reply(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        reply_to_message_id: MessageId,
        markdown: impl AsRef<str>,
    ) -> Result<CreatedMessage, RuntimeError> {
        self.send_message_with_attachments(
            workspace_id,
            channel_id,
            markdown,
            Some(reply_to_message_id),
            Vec::new(),
        )
    }

    pub(crate) fn send_message_with_attachments(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
        reply_to_message_id: Option<MessageId>,
        pending_attachments: Vec<PendingAttachment>,
    ) -> Result<CreatedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        if let Some(reply_to_message_id) = reply_to_message_id.as_ref() {
            validate_message_id_reference(reply_to_message_id)?;
        }
        let markdown = markdown.as_ref().to_owned();
        validate_message_markdown_size(&markdown)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let content_key =
            self.content_key_for_local_write_in_state(&workspace_id, &channel_id, &context.state)?;
        let message_id = MessageId::new();
        let attachments = self.seal_and_store_attachments(
            &workspace_id,
            &channel_id,
            &message_id,
            &content_key,
            pending_attachments,
        )?;
        let sealed_markdown = seal_message_markdown(
            content_key.key_id(),
            content_key.content_key(),
            &workspace_id,
            &channel_id,
            &message_id,
            &markdown,
        )?;
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            match reply_to_message_id.clone() {
                Some(reply_to_message_id) => EventBody::MessageReplyCreatedEncrypted {
                    message_id: message_id.clone(),
                    reply_to_message_id,
                    sealed_markdown,
                    attachments,
                },
                None => EventBody::MessageCreatedEncrypted {
                    message_id: message_id.clone(),
                    sealed_markdown,
                    attachments,
                },
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.index_message_plaintext(
            &workspace_id,
            &channel_id,
            &message_id,
            &message.event_id,
            message.event.timestamp.physical_ms,
            &markdown,
        );

        Ok(CreatedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reply_to_message_id: reply_to_message_id.map(|message_id| message_id.0),
            event_id: message.event_id.0,
            encrypted: true,
            attachment_count: match &message.event.body {
                EventBody::MessageCreatedEncrypted { attachments, .. }
                | EventBody::MessageReplyCreatedEncrypted { attachments, .. } => attachments.len(),
                _ => 0,
            },
        })
    }

    pub fn edit_message(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        markdown: impl AsRef<str>,
    ) -> Result<EditedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let markdown = markdown.as_ref().to_owned();
        validate_message_markdown_size(&markdown)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let message_view =
            Self::message_view_from_state(&context.state, &workspace_id, &message_id)?;
        let channel_id = message_view.channel_id.clone();
        let indexed_event_id = message_view.author_event_id.clone();
        let indexed_physical_ms = context
            .events
            .iter()
            .find(|event| event.event_id == indexed_event_id)
            .map(|event| event.event.timestamp.physical_ms)
            .unwrap_or_default();
        let content_key =
            self.content_key_for_local_write_in_state(&workspace_id, &channel_id, &context.state)?;
        let sealed_markdown = seal_message_markdown(
            content_key.key_id(),
            content_key.content_key(),
            &workspace_id,
            &channel_id,
            &message_id,
            &markdown,
        )?;
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::MessageEditedEncrypted {
                message_id: message_id.clone(),
                sealed_markdown,
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.index_message_plaintext(
            &workspace_id,
            &channel_id,
            &message_id,
            &indexed_event_id,
            indexed_physical_ms,
            &markdown,
        );

        Ok(EditedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            event_id: message.event_id.0,
            encrypted: true,
        })
    }

    pub fn delete_message(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
    ) -> Result<DeletedMessage, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let message_view =
            Self::message_view_from_state(&context.state, &workspace_id, &message_id)?;
        let channel_id = message_view.channel_id.clone();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::MessageDeleted {
                message_id: message_id.clone(),
            },
        );
        message.parents = context.head_event_ids.clone();
        let message = self.sign_authorize_and_append_with_history(message, &context.events)?;
        let _ = self.remove_message_from_search(&workspace_id, &message_id);

        Ok(DeletedMessage {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            event_id: message.event_id.0,
        })
    }

    pub fn add_reaction(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        reaction: impl AsRef<str>,
    ) -> Result<AddedReaction, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let reaction = reaction.as_ref().trim().to_owned();
        if reaction.is_empty() {
            return Err(RuntimeError::ReactionRequired);
        }
        validate_metadata_field_size("reaction", &reaction, REACTION_TEXT_MAX_BYTES)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let channel_id =
            Self::message_channel_id_from_state(&context.state, &workspace_id, &message_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: reaction.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(AddedReaction {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reaction,
            event_id: event.event_id.0,
        })
    }

    pub fn remove_reaction(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        reaction: impl AsRef<str>,
    ) -> Result<RemovedReaction, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let reaction = reaction.as_ref().trim().to_owned();
        if reaction.is_empty() {
            return Err(RuntimeError::ReactionRequired);
        }
        validate_metadata_field_size("reaction", &reaction, REACTION_TEXT_MAX_BYTES)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let channel_id =
            Self::message_channel_id_from_state(&context.state, &workspace_id, &message_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReactionRemoved {
                message_id: message_id.clone(),
                reaction: reaction.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(RemovedReaction {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            reaction,
            event_id: event.event_id.0,
        })
    }

    pub fn mark_channel_read(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<MarkedChannelRead, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        let context = self.workspace_write_context(&workspace_id)?;
        let read_through_event_id =
            Self::latest_channel_read_event_id_from_context(&context, &workspace_id, &channel_id)?;
        if Self::channel_is_read_through_in_state(
            &context.state,
            self.identity.device_id(),
            &channel_id,
            &read_through_event_id,
        ) {
            return Ok(MarkedChannelRead {
                workspace_id: workspace_id.0,
                channel_id: channel_id.0,
                read_through_event_id: read_through_event_id.0,
                marker_event_id: None,
                already_read: true,
            });
        }

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: read_through_event_id.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;

        Ok(MarkedChannelRead {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            read_through_event_id: read_through_event_id.0,
            marker_event_id: Some(event.event_id.0),
            already_read: false,
        })
    }
}
