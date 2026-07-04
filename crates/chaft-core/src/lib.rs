use std::collections::{BTreeMap, HashMap, HashSet};

use chaft_types::{
    ATTACHMENT_BLOB_HASH_MAX_BYTES, ATTACHMENT_CIPHERTEXT_MAX_BYTES,
    ATTACHMENT_DISPLAY_NAME_MAX_BYTES, ATTACHMENT_ID_MAX_BYTES, ATTACHMENT_KEY_ID_MAX_BYTES,
    ATTACHMENT_MEDIA_TYPE_MAX_BYTES, ATTACHMENT_PLAINTEXT_MAX_BYTES, AttachmentRef,
    CHANNEL_ID_MAX_BYTES, CHANNEL_NAME_MAX_BYTES, CONTENT_KEY_ALGORITHM_MAX_BYTES,
    CONTENT_KEY_ID_MAX_BYTES, ChannelId, ContentKeyScope, DEVICE_DISPLAY_NAME_MAX_BYTES,
    DEVICE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
    DeviceId, DeviceKeyPackageId, EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES, EVENT_ID_MAX_BYTES,
    EVENT_SIGNATURE_MAX_BYTES, EventBody, EventId, MESSAGE_ATTACHMENT_MAX_COUNT,
    MESSAGE_ID_MAX_BYTES, MESSAGE_MARKDOWN_MAX_BYTES, MessageId, OPENMLS_CIPHERSUITE_MAX_BYTES,
    OPENMLS_COMMIT_MAX_BYTES, OPENMLS_GROUP_ID_MAX_BYTES, OPENMLS_KEY_PACKAGE_MAX_BYTES,
    OPENMLS_KEY_PACKAGE_REF_MAX_BYTES, OPENMLS_PROTOCOL_MAX_BYTES, OPENMLS_RATCHET_TREE_MAX_BYTES,
    OPENMLS_WELCOME_MAX_BYTES, PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_MAX_BYTES,
    PEER_ENDPOINT_TRANSPORT_MAX_BYTES, REACTION_TEXT_MAX_BYTES, REPLICA_RETENTION_HINT_MAX_BYTES,
    ReplicaStorageClass, SEALED_MESSAGE_MARKDOWN_MAX_BYTES, SEALED_PAYLOAD_AAD_MAX_BYTES,
    SEALED_PAYLOAD_KEY_ID_MAX_BYTES, SEALED_PAYLOAD_NONCE_MAX_BYTES, SealedPayload, SignedEvent,
    TrustSnapshot, TrustSnapshotChannel, TrustSnapshotEventChannel, TrustSnapshotMessage,
    TrustSnapshotRole, WORKSPACE_ID_MAX_BYTES, WORKSPACE_NAME_MAX_BYTES, WorkspaceId,
    WorkspaceRole, peer_endpoint_hint_is_supported, peer_endpoint_hint_transport_is_consistent,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("event belongs to another workspace")]
    WrongWorkspace,
    #[error("channel is required for this event")]
    MissingChannel,
    #[error("workspace authorization error: {0}")]
    Authorization(#[from] AuthorizationError),
    #[error("event {event_id} is missing causal parents")]
    MissingParents {
        event_id: EventId,
        missing_parent_ids: Vec<EventId>,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorizationError {
    #[error("event belongs to another workspace")]
    WrongWorkspace,
    #[error("workspace has no trusted root event")]
    MissingWorkspaceRoot,
    #[error("workspace root already exists")]
    WorkspaceAlreadyCreated,
    #[error("event is missing a channel context")]
    MissingChannelContext,
    #[error("channel {channel_id:?} is not authorized by workspace history")]
    ChannelNotFound { channel_id: ChannelId },
    #[error("device {device_id:?} is not authorized for private channel {channel_id:?}")]
    PrivateChannelAccessDenied {
        channel_id: ChannelId,
        device_id: DeviceId,
    },
    #[error("device {device_id:?} cannot add members to channel {channel_id:?}")]
    ChannelMemberGrantDenied {
        channel_id: ChannelId,
        device_id: DeviceId,
    },
    #[error("message {message_id:?} is not authorized by workspace history")]
    MessageNotFound { message_id: MessageId },
    #[error("event channel {actual:?} does not match expected channel {expected:?}")]
    ChannelMismatch {
        expected: ChannelId,
        actual: ChannelId,
    },
    #[error("read marker target event {event_id:?} is not authorized by workspace history")]
    ReadMarkerTargetNotFound { event_id: EventId },
    #[error("device {device_id:?} is not a workspace member")]
    NotAMember { device_id: DeviceId },
    #[error("workspace root device {device_id:?} cannot be removed")]
    WorkspaceRootCannotBeRemoved { device_id: DeviceId },
    #[error("role {role:?} cannot perform {action}")]
    InsufficientRole {
        role: WorkspaceRole,
        action: &'static str,
    },
    #[error("trust snapshot is invalid for this authorization check")]
    InvalidTrustSnapshot,
    #[error("event {label} is too large ({actual_bytes} bytes, max {max_bytes})")]
    EventPayloadTooLarge {
        label: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("event {label} is required")]
    EventPayloadRequired { label: &'static str },
    #[error("event peer endpoint uses an unsupported P2P route")]
    UnsupportedPeerEndpoint,
    #[error("event peer endpoint transport does not match its route")]
    PeerEndpointTransportMismatch,
    #[error("event replica capability metadata requires a backup peer endpoint")]
    ReplicaCapabilityRequiresBackupPeer,
    #[error("event {label} has too many items ({actual_count}, max {max_count})")]
    EventItemCountTooLarge {
        label: &'static str,
        actual_count: usize,
        max_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelView {
    pub channel_id: ChannelId,
    pub name: String,
    pub is_private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageView {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub author_event_id: EventId,
    pub reply_to_message_id: Option<MessageId>,
    pub markdown: String,
    pub sealed_markdown: Option<SealedPayload>,
    pub attachments: Vec<AttachmentRef>,
    pub reactions: BTreeMap<String, u32>,
    reaction_authors: HashMap<String, HashSet<DeviceId>>,
    pub deleted: bool,
}

impl MessageView {
    fn new(
        message_id: MessageId,
        channel_id: ChannelId,
        author_event_id: EventId,
        reply_to_message_id: Option<MessageId>,
        markdown: String,
        sealed_markdown: Option<SealedPayload>,
        attachments: Vec<AttachmentRef>,
    ) -> Self {
        Self {
            message_id,
            channel_id,
            author_event_id,
            reply_to_message_id,
            markdown,
            sealed_markdown,
            attachments,
            reactions: BTreeMap::new(),
            reaction_authors: HashMap::new(),
            deleted: false,
        }
    }

    fn add_reaction(&mut self, reaction: &str, author_device_id: &DeviceId) {
        let authors = self
            .reaction_authors
            .entry(reaction.to_owned())
            .or_default();
        if authors.insert(author_device_id.clone()) {
            Self::set_reaction_count(&mut self.reactions, reaction, authors.len());
        }
    }

    fn remove_reaction(&mut self, reaction: &str, author_device_id: &DeviceId) {
        let Some(authors) = self.reaction_authors.get_mut(reaction) else {
            return;
        };
        if !authors.remove(author_device_id) {
            return;
        }
        if authors.is_empty() {
            self.reaction_authors.remove(reaction);
            self.reactions.remove(reaction);
        } else {
            Self::set_reaction_count(&mut self.reactions, reaction, authors.len());
        }
    }

    fn set_reaction_count(reactions: &mut BTreeMap<String, u32>, reaction: &str, count: usize) {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        reactions.insert(reaction.to_owned(), count);
    }

    pub fn reactions_for_device(&self, device_id: &DeviceId) -> Vec<String> {
        self.reactions
            .keys()
            .filter(|reaction| {
                self.reaction_authors
                    .get(*reaction)
                    .is_some_and(|authors| authors.contains(device_id))
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProfileView {
    pub device_id: DeviceId,
    pub display_name: String,
    pub updated_event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyPackageView {
    pub device_id: DeviceId,
    pub key_package_id: DeviceKeyPackageId,
    pub protocol: String,
    pub key_package: Vec<u8>,
    pub published_event_id: EventId,
    pub physical_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerEndpointView {
    pub device_id: DeviceId,
    pub endpoint_id: String,
    pub endpoint: String,
    pub transport: String,
    pub is_backup_peer: bool,
    pub expires_at_ms: Option<i64>,
    pub replica_storage_class: Option<ReplicaStorageClass>,
    pub replica_retention_hint: Option<String>,
    pub published_event_id: EventId,
    pub physical_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMemberView {
    pub device_id: DeviceId,
    pub role: WorkspaceRole,
    pub membership_event_id: EventId,
}

#[derive(Debug, Clone)]
pub struct WorkspaceState {
    pub workspace_id: WorkspaceId,
    pub name: Option<String>,
    pub channels: HashMap<ChannelId, ChannelView>,
    pub messages: HashMap<MessageId, MessageView>,
    pub members: HashMap<DeviceId, WorkspaceMemberView>,
    pub profiles: HashMap<DeviceId, DeviceProfileView>,
    pub key_packages: HashMap<DeviceKeyPackageId, DeviceKeyPackageView>,
    pub peer_endpoints: HashMap<(DeviceId, String), PeerEndpointView>,
    pub read_markers: HashMap<DeviceId, HashMap<ChannelId, EventId>>,
    pub applied_events: Vec<EventId>,
    access_index: WorkspaceAccessIndex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingHistoryGap {
    pub event_id: EventId,
    pub missing_parent_ids: Vec<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaterializationReport {
    pub applied_events: Vec<EventId>,
    pub gaps: Vec<MissingHistoryGap>,
}

impl WorkspaceState {
    pub fn new(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id: workspace_id.clone(),
            name: None,
            channels: HashMap::new(),
            messages: HashMap::new(),
            members: HashMap::new(),
            profiles: HashMap::new(),
            key_packages: HashMap::new(),
            peer_endpoints: HashMap::new(),
            read_markers: HashMap::new(),
            applied_events: Vec::new(),
            access_index: WorkspaceAccessIndex::new(workspace_id),
        }
    }

    pub fn apply(&mut self, signed: &SignedEvent) -> Result<(), CoreError> {
        validate_signed_event_ids(signed)?;
        if signed.event.workspace_id != self.workspace_id {
            return Err(CoreError::WrongWorkspace);
        }
        let missing_parent_ids = self.missing_parent_ids(signed)?;
        if !missing_parent_ids.is_empty() {
            return Err(CoreError::MissingParents {
                event_id: signed.event_id.clone(),
                missing_parent_ids,
            });
        }

        self.apply_ready_event(signed)
    }

    pub fn apply_batch(
        &mut self,
        events: &[SignedEvent],
    ) -> Result<MaterializationReport, CoreError> {
        for event in events {
            validate_signed_event_ids(event)?;
            if event.event.workspace_id != self.workspace_id {
                return Err(CoreError::WrongWorkspace);
            }
        }

        let mut report = MaterializationReport::default();
        let mut pending = events.iter().collect::<Vec<_>>();

        loop {
            let mut progressed = false;
            let mut index = 0;

            while index < pending.len() {
                if self.missing_parent_ids(pending[index])?.is_empty() {
                    match self.apply_ready_event(pending[index]) {
                        Ok(()) => {
                            let event = pending.remove(index);
                            report.applied_events.push(event.event_id.clone());
                            progressed = true;
                        }
                        Err(CoreError::Authorization(
                            error @ (AuthorizationError::EventPayloadTooLarge { .. }
                            | AuthorizationError::EventPayloadRequired { .. }
                            | AuthorizationError::EventItemCountTooLarge { .. }
                            | AuthorizationError::UnsupportedPeerEndpoint
                            | AuthorizationError::PeerEndpointTransportMismatch
                            | AuthorizationError::ReplicaCapabilityRequiresBackupPeer),
                        )) => return Err(CoreError::Authorization(error)),
                        Err(CoreError::Authorization(_)) => index += 1,
                        Err(error) => return Err(error),
                    }
                } else {
                    index += 1;
                }
            }

            if !progressed {
                break;
            }
        }

        for event in pending {
            report.gaps.push(MissingHistoryGap {
                event_id: event.event_id.clone(),
                missing_parent_ids: self.missing_parent_ids(event)?,
            });
        }

        Ok(report)
    }

    pub fn missing_parent_ids(&self, signed: &SignedEvent) -> Result<Vec<EventId>, CoreError> {
        validate_signed_event_ids(signed)?;
        if signed.event.workspace_id != self.workspace_id {
            return Err(CoreError::WrongWorkspace);
        }
        Ok(signed
            .event
            .parents
            .iter()
            .filter(|parent_id| !self.has_applied_event(parent_id))
            .cloned()
            .collect())
    }

    fn has_applied_event(&self, event_id: &EventId) -> bool {
        self.applied_events
            .iter()
            .any(|applied_id| applied_id == event_id)
    }

    pub fn channel_accessible_to(&self, channel_id: &ChannelId, device_id: &DeviceId) -> bool {
        self.access_index
            .channel_accessible_to(channel_id, device_id)
    }

    fn apply_ready_event(&mut self, signed: &SignedEvent) -> Result<(), CoreError> {
        if signed.event.workspace_id != self.workspace_id {
            return Err(CoreError::WrongWorkspace);
        }
        self.access_index.authorize_and_apply(signed)?;

        match &signed.event.body {
            EventBody::WorkspaceCreated { name } => {
                self.name = Some(name.clone());
                let device_id = signed.event.author_device_id.clone();
                self.members.insert(
                    device_id.clone(),
                    WorkspaceMemberView {
                        device_id,
                        role: WorkspaceRole::Owner,
                        membership_event_id: signed.event_id.clone(),
                    },
                );
            }
            EventBody::MemberInvited {
                invitee_device_id,
                role,
            } => {
                self.members.insert(
                    invitee_device_id.clone(),
                    WorkspaceMemberView {
                        device_id: invitee_device_id.clone(),
                        role: *role,
                        membership_event_id: signed.event_id.clone(),
                    },
                );
            }
            EventBody::MemberRemoved { removed_device_id } => {
                self.members.remove(removed_device_id);
            }
            EventBody::ChannelCreated {
                channel_id,
                name,
                is_private,
            } => {
                self.channels.insert(
                    channel_id.clone(),
                    ChannelView {
                        channel_id: channel_id.clone(),
                        name: name.clone(),
                        is_private: *is_private,
                    },
                );
            }
            EventBody::DeviceProfileUpdated { display_name } => {
                let device_id = signed.event.author_device_id.clone();
                self.profiles.insert(
                    device_id.clone(),
                    DeviceProfileView {
                        device_id,
                        display_name: display_name.clone(),
                        updated_event_id: signed.event_id.clone(),
                    },
                );
            }
            EventBody::DeviceKeyPackagePublished {
                key_package_id,
                protocol,
                key_package,
            } => {
                self.key_packages.insert(
                    key_package_id.clone(),
                    DeviceKeyPackageView {
                        device_id: signed.event.author_device_id.clone(),
                        key_package_id: key_package_id.clone(),
                        protocol: protocol.clone(),
                        key_package: key_package.clone(),
                        published_event_id: signed.event_id.clone(),
                        physical_ms: signed.event.timestamp.physical_ms,
                    },
                );
            }
            EventBody::PeerEndpointPublished {
                endpoint_id,
                endpoint,
                transport,
                is_backup_peer,
                expires_at_ms,
                replica_storage_class,
                replica_retention_hint,
            } => {
                let device_id = signed.event.author_device_id.clone();
                self.peer_endpoints.insert(
                    (device_id.clone(), endpoint_id.clone()),
                    PeerEndpointView {
                        device_id,
                        endpoint_id: endpoint_id.clone(),
                        endpoint: endpoint.clone(),
                        transport: transport.clone(),
                        is_backup_peer: *is_backup_peer,
                        expires_at_ms: *expires_at_ms,
                        replica_storage_class: *replica_storage_class,
                        replica_retention_hint: replica_retention_hint.clone(),
                        published_event_id: signed.event_id.clone(),
                        physical_ms: signed.event.timestamp.physical_ms,
                    },
                );
            }
            EventBody::OpenMlsWorkspaceGroupMemberAdded { .. } => {}
            EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. } => {}
            EventBody::OpenMlsChannelGroupMemberAdded { .. } => {}
            EventBody::OpenMlsChannelGroupMemberRemoved { .. } => {}
            EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => {}
            EventBody::OpenMlsChannelGroupSelfUpdated { .. } => {}
            EventBody::ContentKeyEpochPublished { .. } => {}
            EventBody::MessageCreated {
                message_id,
                markdown,
                attachments,
            } => {
                let channel_id = signed
                    .event
                    .channel_id
                    .clone()
                    .ok_or(CoreError::MissingChannel)?;
                self.messages.insert(
                    message_id.clone(),
                    MessageView::new(
                        message_id.clone(),
                        channel_id,
                        signed.event_id.clone(),
                        None,
                        markdown.clone(),
                        None,
                        attachments.clone(),
                    ),
                );
            }
            EventBody::MessageReplyCreated {
                message_id,
                reply_to_message_id,
                markdown,
                attachments,
            } => {
                let channel_id = signed
                    .event
                    .channel_id
                    .clone()
                    .ok_or(CoreError::MissingChannel)?;
                self.messages.insert(
                    message_id.clone(),
                    MessageView::new(
                        message_id.clone(),
                        channel_id,
                        signed.event_id.clone(),
                        Some(reply_to_message_id.clone()),
                        markdown.clone(),
                        None,
                        attachments.clone(),
                    ),
                );
            }
            EventBody::MessageCreatedEncrypted {
                message_id,
                sealed_markdown,
                attachments,
            } => {
                let channel_id = signed
                    .event
                    .channel_id
                    .clone()
                    .ok_or(CoreError::MissingChannel)?;
                self.messages.insert(
                    message_id.clone(),
                    MessageView::new(
                        message_id.clone(),
                        channel_id,
                        signed.event_id.clone(),
                        None,
                        String::new(),
                        Some(sealed_markdown.clone()),
                        attachments.clone(),
                    ),
                );
            }
            EventBody::MessageReplyCreatedEncrypted {
                message_id,
                reply_to_message_id,
                sealed_markdown,
                attachments,
            } => {
                let channel_id = signed
                    .event
                    .channel_id
                    .clone()
                    .ok_or(CoreError::MissingChannel)?;
                self.messages.insert(
                    message_id.clone(),
                    MessageView::new(
                        message_id.clone(),
                        channel_id,
                        signed.event_id.clone(),
                        Some(reply_to_message_id.clone()),
                        String::new(),
                        Some(sealed_markdown.clone()),
                        attachments.clone(),
                    ),
                );
            }
            EventBody::MessageEdited {
                message_id,
                markdown,
            } => {
                if let Some(message) = self.messages.get_mut(message_id) {
                    message.markdown = markdown.clone();
                    message.sealed_markdown = None;
                }
            }
            EventBody::MessageEditedEncrypted {
                message_id,
                sealed_markdown,
            } => {
                if let Some(message) = self.messages.get_mut(message_id) {
                    message.markdown.clear();
                    message.sealed_markdown = Some(sealed_markdown.clone());
                }
            }
            EventBody::MessageDeleted { message_id } => {
                if let Some(message) = self.messages.get_mut(message_id) {
                    message.deleted = true;
                }
            }
            EventBody::ReactionAdded {
                message_id,
                reaction,
            } => {
                if let Some(message) = self.messages.get_mut(message_id) {
                    message.add_reaction(reaction, &signed.event.author_device_id);
                }
            }
            EventBody::ReactionRemoved {
                message_id,
                reaction,
            } => {
                if let Some(message) = self.messages.get_mut(message_id) {
                    message.remove_reaction(reaction, &signed.event.author_device_id);
                }
            }
            EventBody::ReadMarkerUpdated {
                channel_id,
                event_id,
            } => {
                self.read_markers
                    .entry(signed.event.author_device_id.clone())
                    .or_default()
                    .insert(channel_id.clone(), event_id.clone());
            }
            EventBody::ChannelMemberAdded { .. } => {}
            EventBody::ChannelMemberRemoved { .. } => {}
        }

        self.applied_events.push(signed.event_id.clone());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceAccessIndex {
    workspace_id: WorkspaceId,
    roles: HashMap<DeviceId, WorkspaceRole>,
    channels: HashMap<ChannelId, ChannelAccess>,
    messages: HashMap<MessageId, ChannelId>,
    event_channels: HashMap<EventId, ChannelId>,
    root_device_id: Option<DeviceId>,
    root_seen: bool,
}

#[derive(Debug, Clone)]
struct ChannelAccess {
    is_private: bool,
    creator_device_id: DeviceId,
    members: HashSet<DeviceId>,
}

impl WorkspaceAccessIndex {
    pub fn new(workspace_id: WorkspaceId) -> Self {
        Self {
            workspace_id,
            roles: HashMap::new(),
            channels: HashMap::new(),
            messages: HashMap::new(),
            event_channels: HashMap::new(),
            root_device_id: None,
            root_seen: false,
        }
    }

    pub fn from_trust_snapshot(snapshot: &TrustSnapshot) -> Result<Self, AuthorizationError> {
        if snapshot.schema_version != 1 {
            return Err(AuthorizationError::InvalidTrustSnapshot);
        }
        validate_trust_snapshot_ids(snapshot)?;

        let mut index = Self::new(snapshot.workspace_id.clone());
        index.root_seen = true;
        index.root_device_id = Some(snapshot.root_author_device_id.clone());
        index
            .roles
            .insert(snapshot.root_author_device_id.clone(), WorkspaceRole::Owner);
        for role in &snapshot.roles {
            if index.roles.contains_key(&role.device_id) {
                return Err(AuthorizationError::InvalidTrustSnapshot);
            }
            index.roles.insert(role.device_id.clone(), role.role);
        }
        for channel in &snapshot.channels {
            if index.channels.contains_key(&channel.channel_id) {
                return Err(AuthorizationError::InvalidTrustSnapshot);
            }
            let mut members = HashSet::new();
            for member_device_id in &channel.member_device_ids {
                if !index.roles.contains_key(member_device_id)
                    || !members.insert(member_device_id.clone())
                {
                    return Err(AuthorizationError::InvalidTrustSnapshot);
                }
            }
            index.channels.insert(
                channel.channel_id.clone(),
                ChannelAccess {
                    is_private: channel.is_private,
                    creator_device_id: channel.creator_device_id.clone(),
                    members,
                },
            );
        }
        for message in &snapshot.messages {
            if !index.channels.contains_key(&message.channel_id)
                || index.messages.contains_key(&message.message_id)
            {
                return Err(AuthorizationError::InvalidTrustSnapshot);
            }
            index
                .messages
                .insert(message.message_id.clone(), message.channel_id.clone());
        }
        for event_channel in &snapshot.event_channels {
            if !index.channels.contains_key(&event_channel.channel_id)
                || index.event_channels.contains_key(&event_channel.event_id)
            {
                return Err(AuthorizationError::InvalidTrustSnapshot);
            }
            index.event_channels.insert(
                event_channel.event_id.clone(),
                event_channel.channel_id.clone(),
            );
        }
        Ok(index)
    }

    pub fn role_for(&self, device_id: &DeviceId) -> Option<WorkspaceRole> {
        self.roles.get(device_id).copied()
    }

    pub fn channel_accessible_to(&self, channel_id: &ChannelId, device_id: &DeviceId) -> bool {
        if !self.root_seen || !self.roles.contains_key(device_id) {
            return false;
        }

        self.channels
            .get(channel_id)
            .is_some_and(|channel| !channel.is_private || channel.members.contains(device_id))
    }

    pub fn authorize_and_apply(&mut self, event: &SignedEvent) -> Result<(), AuthorizationError> {
        self.authorize(event)?;
        self.apply_authorized(event);
        Ok(())
    }

    pub fn authorize(&self, event: &SignedEvent) -> Result<(), AuthorizationError> {
        validate_signed_event_ids(event)?;
        if event.event.workspace_id != self.workspace_id {
            return Err(AuthorizationError::WrongWorkspace);
        }
        validate_event_body_payload_sizes(&event.event.body)?;

        match &event.event.body {
            EventBody::WorkspaceCreated { .. } => {
                if self.root_seen {
                    return Err(AuthorizationError::WorkspaceAlreadyCreated);
                }
                Ok(())
            }
            EventBody::MemberInvited { role, .. } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                if *role == WorkspaceRole::Owner {
                    require_role(author_role, Action::GrantOwner)
                } else {
                    require_role(author_role, Action::InviteMember)
                }
            }
            EventBody::MemberRemoved { removed_device_id } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                let removed_role = self.require_rooted_member(removed_device_id)?;
                if self.root_device_id.as_ref() == Some(removed_device_id) {
                    return Err(AuthorizationError::WorkspaceRootCannotBeRemoved {
                        device_id: removed_device_id.clone(),
                    });
                }
                if removed_role == WorkspaceRole::Owner {
                    require_role(author_role, Action::GrantOwner)
                } else {
                    require_role(author_role, Action::RemoveMember)
                }
            }
            EventBody::ChannelCreated { .. } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                require_role(author_role, Action::CreateChannel)
            }
            EventBody::DeviceProfileUpdated { .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                Ok(())
            }
            EventBody::DeviceKeyPackagePublished { .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                Ok(())
            }
            EventBody::PeerEndpointPublished { .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                Ok(())
            }
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id, ..
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                self.require_rooted_member(invitee_device_id)?;
                require_role(author_role, Action::ManageOpenMlsGroup)
            }
            EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                removed_device_id, ..
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                let removed_role = self.require_rooted_member(removed_device_id)?;
                if self.root_device_id.as_ref() == Some(removed_device_id) {
                    return Err(AuthorizationError::WorkspaceRootCannotBeRemoved {
                        device_id: removed_device_id.clone(),
                    });
                }
                if removed_role == WorkspaceRole::Owner {
                    require_role(author_role, Action::GrantOwner)
                } else {
                    require_role(author_role, Action::ManageOpenMlsGroup)
                }
            }
            EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id,
                invitee_device_id,
                ..
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                self.require_rooted_member(invitee_device_id)?;
                let actual_channel_id = require_event_channel(event)?;
                if actual_channel_id != channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: channel_id.clone(),
                        actual: actual_channel_id.clone(),
                    });
                }
                self.require_channel_member_grant(
                    channel_id,
                    &event.event.author_device_id,
                    author_role,
                )?;
                self.require_channel_access(channel_id, invitee_device_id)?;
                Ok(())
            }
            EventBody::OpenMlsChannelGroupMemberRemoved {
                channel_id,
                removed_device_id,
                ..
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                self.require_rooted_member(removed_device_id)?;
                let actual_channel_id = require_event_channel(event)?;
                if actual_channel_id != channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: channel_id.clone(),
                        actual: actual_channel_id.clone(),
                    });
                }
                self.require_channel_member_grant(
                    channel_id,
                    &event.event.author_device_id,
                    author_role,
                )?;
                self.require_channel_access(channel_id, removed_device_id)?;
                Ok(())
            }
            EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                Ok(())
            }
            EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                let actual_channel_id = require_event_channel(event)?;
                if actual_channel_id != channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: channel_id.clone(),
                        actual: actual_channel_id.clone(),
                    });
                }
                self.require_channel_access(channel_id, &event.event.author_device_id)?;
                Ok(())
            }
            EventBody::ContentKeyEpochPublished { scope, .. } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                match scope {
                    ContentKeyScope::Workspace => {
                        require_role(author_role, Action::RotateContentKey)
                    }
                    ContentKeyScope::Channel { channel_id } => {
                        let actual_channel_id = require_event_channel(event)?;
                        if actual_channel_id != channel_id {
                            return Err(AuthorizationError::ChannelMismatch {
                                expected: channel_id.clone(),
                                actual: actual_channel_id.clone(),
                            });
                        }
                        self.require_channel_member_grant(
                            channel_id,
                            &event.event.author_device_id,
                            author_role,
                        )
                    }
                }
            }
            EventBody::ChannelMemberAdded {
                channel_id,
                member_device_id,
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                self.require_rooted_member(member_device_id)?;
                self.require_channel_member_grant(
                    channel_id,
                    &event.event.author_device_id,
                    author_role,
                )
            }
            EventBody::ChannelMemberRemoved {
                channel_id,
                member_device_id,
            } => {
                let author_role = self.require_rooted_member(&event.event.author_device_id)?;
                self.require_rooted_member(member_device_id)?;
                self.require_channel_member_grant(
                    channel_id,
                    &event.event.author_device_id,
                    author_role,
                )?;
                self.require_channel_access(channel_id, member_device_id)
            }
            EventBody::MessageCreated { .. } | EventBody::MessageCreatedEncrypted { .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                let channel_id = require_event_channel(event)?;
                self.require_channel_access(channel_id, &event.event.author_device_id)?;
                Ok(())
            }
            EventBody::MessageReplyCreated {
                reply_to_message_id,
                ..
            }
            | EventBody::MessageReplyCreatedEncrypted {
                reply_to_message_id,
                ..
            } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                let channel_id = require_event_channel(event)?;
                self.require_channel_access(channel_id, &event.event.author_device_id)?;
                let reply_channel_id = self.require_message(reply_to_message_id)?;
                if channel_id != reply_channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: reply_channel_id.clone(),
                        actual: channel_id.clone(),
                    });
                }
                Ok(())
            }
            EventBody::MessageEdited { message_id, .. }
            | EventBody::MessageEditedEncrypted { message_id, .. }
            | EventBody::MessageDeleted { message_id }
            | EventBody::ReactionAdded { message_id, .. }
            | EventBody::ReactionRemoved { message_id, .. } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                let expected_channel_id = self.require_message(message_id)?;
                let actual_channel_id = require_event_channel(event)?;
                if actual_channel_id != expected_channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: expected_channel_id.clone(),
                        actual: actual_channel_id.clone(),
                    });
                }
                self.require_channel_access(expected_channel_id, &event.event.author_device_id)?;
                Ok(())
            }
            EventBody::ReadMarkerUpdated {
                channel_id,
                event_id,
            } => {
                self.require_rooted_member(&event.event.author_device_id)?;
                let actual_channel_id = require_event_channel(event)?;
                if actual_channel_id != channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: channel_id.clone(),
                        actual: actual_channel_id.clone(),
                    });
                }
                self.require_channel_access(channel_id, &event.event.author_device_id)?;
                let target_channel_id = self.event_channels.get(event_id).ok_or_else(|| {
                    AuthorizationError::ReadMarkerTargetNotFound {
                        event_id: event_id.clone(),
                    }
                })?;
                if target_channel_id != channel_id {
                    return Err(AuthorizationError::ChannelMismatch {
                        expected: channel_id.clone(),
                        actual: target_channel_id.clone(),
                    });
                }
                Ok(())
            }
        }
    }

    fn apply_authorized(&mut self, event: &SignedEvent) {
        match &event.event.body {
            EventBody::WorkspaceCreated { .. } => {
                self.root_seen = true;
                self.root_device_id = Some(event.event.author_device_id.clone());
                self.roles
                    .insert(event.event.author_device_id.clone(), WorkspaceRole::Owner);
            }
            EventBody::MemberInvited {
                invitee_device_id,
                role,
            } => {
                self.roles.insert(invitee_device_id.clone(), *role);
            }
            EventBody::MemberRemoved { removed_device_id } => {
                self.roles.remove(removed_device_id);
                for channel in self.channels.values_mut() {
                    channel.members.remove(removed_device_id);
                }
            }
            EventBody::ChannelCreated {
                channel_id,
                is_private,
                ..
            } => {
                let mut members = HashSet::new();
                if *is_private {
                    members.insert(event.event.author_device_id.clone());
                }
                self.channels.insert(
                    channel_id.clone(),
                    ChannelAccess {
                        is_private: *is_private,
                        creator_device_id: event.event.author_device_id.clone(),
                        members,
                    },
                );
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ChannelMemberAdded {
                channel_id,
                member_device_id,
            } => {
                if let Some(channel) = self.channels.get_mut(channel_id) {
                    channel.members.insert(member_device_id.clone());
                }
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ChannelMemberRemoved {
                channel_id,
                member_device_id,
            } => {
                if let Some(channel) = self.channels.get_mut(channel_id) {
                    channel.members.remove(member_device_id);
                }
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::DeviceProfileUpdated { .. }
            | EventBody::DeviceKeyPackagePublished { .. }
            | EventBody::PeerEndpointPublished { .. }
            | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
            | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
            | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => {}
            EventBody::OpenMlsChannelGroupMemberAdded { channel_id, .. } => {
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::OpenMlsChannelGroupMemberRemoved { channel_id, .. } => {
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. } => {
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ContentKeyEpochPublished { scope, .. } => {
                if let ContentKeyScope::Channel { channel_id } = scope {
                    self.event_channels
                        .insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::MessageCreated { message_id, .. }
            | EventBody::MessageCreatedEncrypted { message_id, .. }
            | EventBody::MessageReplyCreated { message_id, .. }
            | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => {
                if let Some(channel_id) = event.event.channel_id.as_ref() {
                    self.messages.insert(message_id.clone(), channel_id.clone());
                    self.event_channels
                        .insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::MessageEdited { message_id, .. }
            | EventBody::MessageEditedEncrypted { message_id, .. }
            | EventBody::MessageDeleted { message_id }
            | EventBody::ReactionAdded { message_id, .. }
            | EventBody::ReactionRemoved { message_id, .. } => {
                if let Some(channel_id) = self.messages.get(message_id) {
                    self.event_channels
                        .insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::ReadMarkerUpdated { channel_id, .. } => {
                self.event_channels
                    .insert(event.event_id.clone(), channel_id.clone());
            }
        }
    }

    fn require_channel_access(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> Result<(), AuthorizationError> {
        let channel =
            self.channels
                .get(channel_id)
                .ok_or_else(|| AuthorizationError::ChannelNotFound {
                    channel_id: channel_id.clone(),
                })?;

        if !channel.is_private || channel.members.contains(device_id) {
            Ok(())
        } else {
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: channel_id.clone(),
                device_id: device_id.clone(),
            })
        }
    }

    fn require_channel_member_grant(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
        role: WorkspaceRole,
    ) -> Result<(), AuthorizationError> {
        let channel =
            self.channels
                .get(channel_id)
                .ok_or_else(|| AuthorizationError::ChannelNotFound {
                    channel_id: channel_id.clone(),
                })?;

        if !channel.is_private
            || channel.creator_device_id == *device_id
            || matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
        {
            Ok(())
        } else {
            Err(AuthorizationError::ChannelMemberGrantDenied {
                channel_id: channel_id.clone(),
                device_id: device_id.clone(),
            })
        }
    }

    fn require_message(&self, message_id: &MessageId) -> Result<&ChannelId, AuthorizationError> {
        self.messages
            .get(message_id)
            .ok_or_else(|| AuthorizationError::MessageNotFound {
                message_id: message_id.clone(),
            })
    }

    fn require_rooted_member(
        &self,
        device_id: &DeviceId,
    ) -> Result<WorkspaceRole, AuthorizationError> {
        if !self.root_seen {
            return Err(AuthorizationError::MissingWorkspaceRoot);
        }

        self.role_for(device_id)
            .ok_or_else(|| AuthorizationError::NotAMember {
                device_id: device_id.clone(),
            })
    }
}

pub fn authorize_event_with_history(
    history: &[SignedEvent],
    event: &SignedEvent,
) -> Result<(), AuthorizationError> {
    validate_signed_event_ids(event)?;
    let mut index = WorkspaceAccessIndex::new(event.event.workspace_id.clone());
    let mut pending = history
        .iter()
        .filter(|historical| historical.event.workspace_id == event.event.workspace_id)
        .collect::<Vec<_>>();

    loop {
        let mut progressed = false;
        let mut index_in_pending = 0;

        while index_in_pending < pending.len() {
            match index.authorize_and_apply(pending[index_in_pending]) {
                Ok(()) => {
                    pending.remove(index_in_pending);
                    progressed = true;
                }
                Err(AuthorizationError::WorkspaceAlreadyCreated) => {
                    pending.remove(index_in_pending);
                }
                Err(
                    AuthorizationError::MissingWorkspaceRoot
                    | AuthorizationError::ChannelNotFound { .. }
                    | AuthorizationError::MessageNotFound { .. }
                    | AuthorizationError::ReadMarkerTargetNotFound { .. }
                    | AuthorizationError::NotAMember { .. }
                    | AuthorizationError::WorkspaceRootCannotBeRemoved { .. }
                    | AuthorizationError::PrivateChannelAccessDenied { .. }
                    | AuthorizationError::ChannelMemberGrantDenied { .. }
                    | AuthorizationError::InsufficientRole { .. },
                ) => {
                    index_in_pending += 1;
                }
                Err(
                    AuthorizationError::WrongWorkspace
                    | AuthorizationError::MissingChannelContext
                    | AuthorizationError::ChannelMismatch { .. }
                    | AuthorizationError::InvalidTrustSnapshot,
                ) => {
                    index_in_pending += 1;
                }
                Err(
                    error @ (AuthorizationError::EventPayloadTooLarge { .. }
                    | AuthorizationError::EventPayloadRequired { .. }
                    | AuthorizationError::EventItemCountTooLarge { .. }
                    | AuthorizationError::UnsupportedPeerEndpoint
                    | AuthorizationError::PeerEndpointTransportMismatch
                    | AuthorizationError::ReplicaCapabilityRequiresBackupPeer),
                ) => return Err(error),
            }
        }

        if !progressed {
            break;
        }
    }

    index.authorize(event)
}

pub fn authorize_event_with_trust_snapshot(
    snapshot: &TrustSnapshot,
    event: &SignedEvent,
) -> Result<(), AuthorizationError> {
    let index = WorkspaceAccessIndex::from_trust_snapshot(snapshot)?;
    index.authorize(event)
}

pub fn trust_snapshot_from_events(
    workspace_id: WorkspaceId,
    events: &[SignedEvent],
) -> Result<(TrustSnapshot, SignedEvent), CoreError> {
    let mut state = WorkspaceState::new(workspace_id.clone());
    let report = state.apply_batch(events)?;
    let applied_event_ids = report.applied_events.into_iter().collect::<HashSet<_>>();
    let applied_events = events
        .iter()
        .filter(|event| applied_event_ids.contains(&event.event_id))
        .collect::<Vec<_>>();
    let root_event = applied_events
        .iter()
        .find(|event| matches!(event.event.body, EventBody::WorkspaceCreated { .. }))
        .map(|event| (*event).clone())
        .ok_or(AuthorizationError::MissingWorkspaceRoot)?;

    let mut roles = HashMap::<DeviceId, WorkspaceRole>::new();
    let mut channels = HashMap::<ChannelId, TrustSnapshotChannel>::new();
    let mut messages = HashMap::<MessageId, ChannelId>::new();
    let mut event_channels = HashMap::<EventId, ChannelId>::new();

    for event in applied_events {
        match &event.event.body {
            EventBody::WorkspaceCreated { .. } => {}
            EventBody::DeviceProfileUpdated { .. } => {}
            EventBody::DeviceKeyPackagePublished { .. } => {}
            EventBody::PeerEndpointPublished { .. } => {}
            EventBody::OpenMlsWorkspaceGroupMemberAdded { .. } => {}
            EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. } => {}
            EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => {}
            EventBody::OpenMlsChannelGroupMemberAdded { channel_id, .. } => {
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::OpenMlsChannelGroupMemberRemoved { channel_id, .. } => {
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. } => {
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ContentKeyEpochPublished { scope, .. } => {
                if let ContentKeyScope::Channel { channel_id } = scope {
                    event_channels.insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::MemberInvited {
                invitee_device_id,
                role,
            } => {
                roles.insert(invitee_device_id.clone(), *role);
            }
            EventBody::MemberRemoved { removed_device_id } => {
                roles.remove(removed_device_id);
                for channel in channels.values_mut() {
                    channel
                        .member_device_ids
                        .retain(|device_id| device_id != removed_device_id);
                }
            }
            EventBody::ChannelCreated {
                channel_id,
                is_private,
                ..
            } => {
                let mut member_device_ids = Vec::new();
                if *is_private {
                    member_device_ids.push(event.event.author_device_id.clone());
                }
                channels.insert(
                    channel_id.clone(),
                    TrustSnapshotChannel {
                        channel_id: channel_id.clone(),
                        is_private: *is_private,
                        creator_device_id: event.event.author_device_id.clone(),
                        member_device_ids,
                    },
                );
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ChannelMemberAdded {
                channel_id,
                member_device_id,
            } => {
                if let Some(channel) = channels.get_mut(channel_id)
                    && !channel.member_device_ids.contains(member_device_id)
                {
                    channel.member_device_ids.push(member_device_id.clone());
                }
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::ChannelMemberRemoved {
                channel_id,
                member_device_id,
            } => {
                if let Some(channel) = channels.get_mut(channel_id) {
                    channel
                        .member_device_ids
                        .retain(|device_id| device_id != member_device_id);
                }
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
            EventBody::MessageCreated { message_id, .. }
            | EventBody::MessageCreatedEncrypted { message_id, .. }
            | EventBody::MessageReplyCreated { message_id, .. }
            | EventBody::MessageReplyCreatedEncrypted { message_id, .. } => {
                if let Some(channel_id) = event.event.channel_id.as_ref() {
                    messages.insert(message_id.clone(), channel_id.clone());
                    event_channels.insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::MessageEdited { message_id, .. }
            | EventBody::MessageEditedEncrypted { message_id, .. }
            | EventBody::MessageDeleted { message_id }
            | EventBody::ReactionAdded { message_id, .. }
            | EventBody::ReactionRemoved { message_id, .. } => {
                if let Some(channel_id) = messages.get(message_id) {
                    event_channels.insert(event.event_id.clone(), channel_id.clone());
                }
            }
            EventBody::ReadMarkerUpdated { channel_id, .. } => {
                event_channels.insert(event.event_id.clone(), channel_id.clone());
            }
        }
    }

    let root_author_device_id = root_event.event.author_device_id.clone();
    roles.remove(&root_author_device_id);
    let mut roles = roles
        .into_iter()
        .map(|(device_id, role)| TrustSnapshotRole { device_id, role })
        .collect::<Vec<_>>();
    roles.sort_by(|left, right| left.device_id.0.cmp(&right.device_id.0));

    let mut channels = channels.into_values().collect::<Vec<_>>();
    for channel in &mut channels {
        channel
            .member_device_ids
            .sort_by(|left, right| left.0.cmp(&right.0));
        channel.member_device_ids.dedup();
    }
    channels.sort_by(|left, right| left.channel_id.0.cmp(&right.channel_id.0));

    let mut messages = messages
        .into_iter()
        .map(|(message_id, channel_id)| TrustSnapshotMessage {
            message_id,
            channel_id,
        })
        .collect::<Vec<_>>();
    messages.sort_by(|left, right| left.message_id.0.cmp(&right.message_id.0));

    let mut event_channels = event_channels
        .into_iter()
        .map(|(event_id, channel_id)| TrustSnapshotEventChannel {
            event_id,
            channel_id,
        })
        .collect::<Vec<_>>();
    event_channels.sort_by(|left, right| left.event_id.0.cmp(&right.event_id.0));

    Ok((
        TrustSnapshot {
            schema_version: 1,
            workspace_id,
            root_event_id: root_event.event_id.clone(),
            root_author_device_id,
            roles,
            channels,
            messages,
            event_channels,
        },
        root_event.clone(),
    ))
}

pub fn trust_snapshot_for_event_from_events(
    workspace_id: WorkspaceId,
    events: &[SignedEvent],
    event: &SignedEvent,
) -> Result<(TrustSnapshot, SignedEvent), CoreError> {
    trust_snapshot_for_events_from_events(workspace_id, events, std::slice::from_ref(event))
}

pub fn trust_snapshot_for_events_from_events(
    workspace_id: WorkspaceId,
    events: &[SignedEvent],
    target_events: &[SignedEvent],
) -> Result<(TrustSnapshot, SignedEvent), CoreError> {
    let (snapshot, root_event) = trust_snapshot_from_events(workspace_id, events)?;
    Ok((
        trust_snapshot_for_events(snapshot, target_events),
        root_event,
    ))
}

fn trust_snapshot_for_events(
    mut snapshot: TrustSnapshot,
    target_events: &[SignedEvent],
) -> TrustSnapshot {
    let mut needed_devices = HashSet::<DeviceId>::new();
    let mut needed_channels = HashSet::<ChannelId>::new();
    let mut needed_messages = HashSet::<MessageId>::new();
    let mut needed_event_channels = HashSet::<EventId>::new();

    needed_devices.insert(snapshot.root_author_device_id.clone());

    for event in target_events {
        collect_trust_snapshot_dependencies(
            event,
            &mut needed_devices,
            &mut needed_channels,
            &mut needed_messages,
            &mut needed_event_channels,
        );
    }

    for message in &snapshot.messages {
        if needed_messages.contains(&message.message_id) {
            needed_channels.insert(message.channel_id.clone());
        }
    }
    for event_channel in &snapshot.event_channels {
        if needed_event_channels.contains(&event_channel.event_id) {
            needed_channels.insert(event_channel.channel_id.clone());
        }
    }

    snapshot
        .roles
        .retain(|role| needed_devices.contains(&role.device_id));
    snapshot
        .channels
        .retain(|channel| needed_channels.contains(&channel.channel_id));
    for channel in &mut snapshot.channels {
        channel
            .member_device_ids
            .retain(|device_id| needed_devices.contains(device_id));
    }
    snapshot
        .messages
        .retain(|message| needed_messages.contains(&message.message_id));
    snapshot
        .event_channels
        .retain(|event_channel| needed_event_channels.contains(&event_channel.event_id));

    snapshot
}

fn collect_trust_snapshot_dependencies(
    event: &SignedEvent,
    needed_devices: &mut HashSet<DeviceId>,
    needed_channels: &mut HashSet<ChannelId>,
    needed_messages: &mut HashSet<MessageId>,
    needed_event_channels: &mut HashSet<EventId>,
) {
    needed_devices.insert(event.event.author_device_id.clone());
    if let Some(channel_id) = event.event.channel_id.as_ref() {
        needed_channels.insert(channel_id.clone());
    }

    match &event.event.body {
        EventBody::WorkspaceCreated { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => {}
        EventBody::MemberInvited { .. } => {}
        EventBody::MemberRemoved { removed_device_id } => {
            needed_devices.insert(removed_device_id.clone());
        }
        EventBody::ChannelCreated { .. } => {}
        EventBody::ChannelMemberAdded {
            channel_id,
            member_device_id,
        }
        | EventBody::ChannelMemberRemoved {
            channel_id,
            member_device_id,
        } => {
            needed_channels.insert(channel_id.clone());
            needed_devices.insert(member_device_id.clone());
        }
        EventBody::OpenMlsWorkspaceGroupMemberAdded {
            invitee_device_id, ..
        } => {
            needed_devices.insert(invitee_device_id.clone());
        }
        EventBody::OpenMlsWorkspaceGroupMemberRemoved {
            removed_device_id, ..
        } => {
            needed_devices.insert(removed_device_id.clone());
        }
        EventBody::OpenMlsChannelGroupMemberAdded {
            channel_id,
            invitee_device_id,
            ..
        } => {
            needed_channels.insert(channel_id.clone());
            needed_devices.insert(invitee_device_id.clone());
        }
        EventBody::OpenMlsChannelGroupMemberRemoved {
            channel_id,
            removed_device_id,
            ..
        } => {
            needed_channels.insert(channel_id.clone());
            needed_devices.insert(removed_device_id.clone());
        }
        EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. } => {
            needed_channels.insert(channel_id.clone());
        }
        EventBody::ContentKeyEpochPublished { scope, .. } => {
            if let ContentKeyScope::Channel { channel_id } = scope {
                needed_channels.insert(channel_id.clone());
            }
        }
        EventBody::MessageCreated { .. } | EventBody::MessageCreatedEncrypted { .. } => {}
        EventBody::MessageReplyCreated {
            reply_to_message_id,
            ..
        }
        | EventBody::MessageReplyCreatedEncrypted {
            reply_to_message_id,
            ..
        } => {
            needed_messages.insert(reply_to_message_id.clone());
        }
        EventBody::MessageEdited { message_id, .. }
        | EventBody::MessageEditedEncrypted { message_id, .. }
        | EventBody::MessageDeleted { message_id }
        | EventBody::ReactionAdded { message_id, .. }
        | EventBody::ReactionRemoved { message_id, .. } => {
            needed_messages.insert(message_id.clone());
        }
        EventBody::ReadMarkerUpdated {
            channel_id,
            event_id,
        } => {
            needed_channels.insert(channel_id.clone());
            needed_event_channels.insert(event_id.clone());
        }
    }
}

fn require_event_channel(event: &SignedEvent) -> Result<&ChannelId, AuthorizationError> {
    event
        .event
        .channel_id
        .as_ref()
        .ok_or(AuthorizationError::MissingChannelContext)
}

fn validate_signed_event_ids(event: &SignedEvent) -> Result<(), AuthorizationError> {
    validate_workspace_id_size("workspace ID", &event.event.workspace_id)?;
    if let Some(channel_id) = &event.event.channel_id {
        validate_channel_id_size("event channel ID", channel_id)?;
    }
    validate_device_id_size("author device ID", &event.event.author_device_id)?;
    validate_event_id_size("event ID", &event.event_id)?;
    validate_event_payload_size(
        "author public key",
        &event.author_public_key,
        EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES,
    )?;
    validate_event_payload_size(
        "event signature",
        &event.signature,
        EVENT_SIGNATURE_MAX_BYTES,
    )?;
    for parent_id in &event.event.parents {
        validate_event_id_size("parent event ID", parent_id)?;
    }
    validate_event_body_ids(&event.event.body)
}

fn validate_event_body_ids(body: &EventBody) -> Result<(), AuthorizationError> {
    match body {
        EventBody::WorkspaceCreated { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::PeerEndpointPublished { .. } => Ok(()),
        EventBody::MemberInvited {
            invitee_device_id, ..
        } => validate_device_id_size("invitee device ID", invitee_device_id),
        EventBody::MemberRemoved { removed_device_id } => {
            validate_device_id_size("removed device ID", removed_device_id)
        }
        EventBody::ChannelCreated { channel_id, .. } => {
            validate_channel_id_size("channel ID", channel_id)
        }
        EventBody::ChannelMemberAdded {
            channel_id,
            member_device_id,
        }
        | EventBody::ChannelMemberRemoved {
            channel_id,
            member_device_id,
        } => {
            validate_channel_id_size("channel ID", channel_id)?;
            validate_device_id_size("member device ID", member_device_id)
        }
        EventBody::DeviceKeyPackagePublished { key_package_id, .. } => {
            validate_device_key_package_id_size("device key package ID", key_package_id)
        }
        EventBody::OpenMlsWorkspaceGroupMemberAdded {
            invitee_device_id,
            invitee_key_package_id,
            ..
        } => {
            validate_device_id_size("invitee device ID", invitee_device_id)?;
            validate_device_key_package_id_size(
                "invitee device key package ID",
                invitee_key_package_id,
            )
        }
        EventBody::OpenMlsWorkspaceGroupMemberRemoved {
            removed_device_id, ..
        } => validate_device_id_size("removed device ID", removed_device_id),
        EventBody::OpenMlsChannelGroupMemberAdded {
            channel_id,
            invitee_device_id,
            invitee_key_package_id,
            ..
        } => {
            validate_channel_id_size("channel ID", channel_id)?;
            validate_device_id_size("invitee device ID", invitee_device_id)?;
            validate_device_key_package_id_size(
                "invitee device key package ID",
                invitee_key_package_id,
            )
        }
        EventBody::OpenMlsChannelGroupMemberRemoved {
            channel_id,
            removed_device_id,
            ..
        } => {
            validate_channel_id_size("channel ID", channel_id)?;
            validate_device_id_size("removed device ID", removed_device_id)
        }
        EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => Ok(()),
        EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. } => {
            validate_channel_id_size("channel ID", channel_id)
        }
        EventBody::ContentKeyEpochPublished { scope, .. } => validate_content_key_scope_ids(scope),
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. } => {
            validate_message_id_size("message ID", message_id)
        }
        EventBody::MessageReplyCreated {
            message_id,
            reply_to_message_id,
            ..
        }
        | EventBody::MessageReplyCreatedEncrypted {
            message_id,
            reply_to_message_id,
            ..
        } => {
            validate_message_id_size("message ID", message_id)?;
            validate_message_id_size("reply target message ID", reply_to_message_id)
        }
        EventBody::MessageEdited { message_id, .. }
        | EventBody::MessageEditedEncrypted { message_id, .. }
        | EventBody::MessageDeleted { message_id }
        | EventBody::ReactionAdded { message_id, .. }
        | EventBody::ReactionRemoved { message_id, .. } => {
            validate_message_id_size("message ID", message_id)
        }
        EventBody::ReadMarkerUpdated {
            channel_id,
            event_id,
        } => {
            validate_channel_id_size("channel ID", channel_id)?;
            validate_event_id_size("read marker target event ID", event_id)
        }
    }
}

fn validate_content_key_scope_ids(scope: &ContentKeyScope) -> Result<(), AuthorizationError> {
    match scope {
        ContentKeyScope::Workspace => Ok(()),
        ContentKeyScope::Channel { channel_id } => {
            validate_channel_id_size("channel ID", channel_id)
        }
    }
}

fn validate_trust_snapshot_ids(snapshot: &TrustSnapshot) -> Result<(), AuthorizationError> {
    validate_workspace_id_size("trust snapshot workspace ID", &snapshot.workspace_id)?;
    validate_event_id_size("trust snapshot root event ID", &snapshot.root_event_id)?;
    validate_device_id_size(
        "trust snapshot root author device ID",
        &snapshot.root_author_device_id,
    )?;
    for role in &snapshot.roles {
        validate_device_id_size("trust snapshot role device ID", &role.device_id)?;
    }
    for channel in &snapshot.channels {
        validate_channel_id_size("trust snapshot channel ID", &channel.channel_id)?;
        validate_device_id_size(
            "trust snapshot channel creator device ID",
            &channel.creator_device_id,
        )?;
        for member_device_id in &channel.member_device_ids {
            validate_device_id_size("trust snapshot channel member device ID", member_device_id)?;
        }
    }
    for message in &snapshot.messages {
        validate_message_id_size("trust snapshot message ID", &message.message_id)?;
        validate_channel_id_size("trust snapshot message channel ID", &message.channel_id)?;
    }
    for event_channel in &snapshot.event_channels {
        validate_event_id_size("trust snapshot event ID", &event_channel.event_id)?;
        validate_channel_id_size("trust snapshot event channel ID", &event_channel.channel_id)?;
    }
    Ok(())
}

fn validate_workspace_id_size(
    label: &'static str,
    value: &WorkspaceId,
) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, WORKSPACE_ID_MAX_BYTES)
}

fn validate_channel_id_size(
    label: &'static str,
    value: &ChannelId,
) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, CHANNEL_ID_MAX_BYTES)
}

fn validate_message_id_size(
    label: &'static str,
    value: &MessageId,
) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, MESSAGE_ID_MAX_BYTES)
}

fn validate_device_key_package_id_size(
    label: &'static str,
    value: &DeviceKeyPackageId,
) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, DEVICE_KEY_PACKAGE_ID_MAX_BYTES)
}

fn validate_event_id_size(label: &'static str, value: &EventId) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, EVENT_ID_MAX_BYTES)
}

fn validate_device_id_size(
    label: &'static str,
    value: &DeviceId,
) -> Result<(), AuthorizationError> {
    validate_event_text_size(label, &value.0, DEVICE_ID_MAX_BYTES)
}

fn validate_event_body_payload_sizes(body: &EventBody) -> Result<(), AuthorizationError> {
    match body {
        EventBody::WorkspaceCreated { name } => {
            validate_event_text_size("workspace name", name, WORKSPACE_NAME_MAX_BYTES)
        }
        EventBody::ChannelCreated { name, .. } => {
            validate_event_text_size("channel name", name, CHANNEL_NAME_MAX_BYTES)
        }
        EventBody::DeviceProfileUpdated { display_name } => {
            validate_event_text_size("display name", display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)
        }
        EventBody::PeerEndpointPublished {
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            replica_storage_class,
            replica_retention_hint,
            ..
        } => {
            validate_event_text_required("peer endpoint ID", endpoint_id)?;
            validate_event_text_required("peer endpoint", endpoint)?;
            validate_event_text_required("peer endpoint transport", transport)?;
            validate_event_text_size("peer endpoint ID", endpoint_id, PEER_ENDPOINT_ID_MAX_BYTES)?;
            validate_event_text_size("peer endpoint", endpoint, PEER_ENDPOINT_MAX_BYTES)?;
            validate_event_text_size(
                "peer endpoint transport",
                transport,
                PEER_ENDPOINT_TRANSPORT_MAX_BYTES,
            )?;
            if let Some(replica_retention_hint) = replica_retention_hint {
                validate_event_text_required("replica retention hint", replica_retention_hint)?;
                validate_event_text_size(
                    "replica retention hint",
                    replica_retention_hint,
                    REPLICA_RETENTION_HINT_MAX_BYTES,
                )?;
            }
            if !peer_endpoint_hint_is_supported(endpoint) {
                return Err(AuthorizationError::UnsupportedPeerEndpoint);
            }
            if !peer_endpoint_hint_transport_is_consistent(endpoint, transport) {
                return Err(AuthorizationError::PeerEndpointTransportMismatch);
            }
            if !*is_backup_peer
                && (replica_storage_class.is_some() || replica_retention_hint.is_some())
            {
                return Err(AuthorizationError::ReplicaCapabilityRequiresBackupPeer);
            }
            Ok(())
        }
        EventBody::ContentKeyEpochPublished {
            key_id,
            previous_key_id,
            algorithm,
            ..
        } => {
            validate_event_text_size("content key ID", key_id, CONTENT_KEY_ID_MAX_BYTES)?;
            if let Some(previous_key_id) = previous_key_id {
                validate_event_text_size(
                    "previous content key ID",
                    previous_key_id,
                    CONTENT_KEY_ID_MAX_BYTES,
                )?;
            }
            validate_event_text_size(
                "content key algorithm",
                algorithm,
                CONTENT_KEY_ALGORITHM_MAX_BYTES,
            )
        }
        EventBody::MessageCreated {
            markdown,
            attachments,
            ..
        }
        | EventBody::MessageReplyCreated {
            markdown,
            attachments,
            ..
        } => {
            validate_event_text_size("message markdown", markdown, MESSAGE_MARKDOWN_MAX_BYTES)?;
            validate_message_attachments(attachments)
        }
        EventBody::MessageEdited { markdown, .. } => {
            validate_event_text_size("message markdown", markdown, MESSAGE_MARKDOWN_MAX_BYTES)
        }
        EventBody::MessageCreatedEncrypted {
            sealed_markdown,
            attachments,
            ..
        }
        | EventBody::MessageReplyCreatedEncrypted {
            sealed_markdown,
            attachments,
            ..
        } => {
            validate_sealed_markdown_payload(sealed_markdown)?;
            validate_message_attachments(attachments)
        }
        EventBody::MessageEditedEncrypted {
            sealed_markdown, ..
        } => validate_sealed_markdown_payload(sealed_markdown),
        EventBody::ReactionAdded { reaction, .. } | EventBody::ReactionRemoved { reaction, .. } => {
            validate_event_text_size("reaction", reaction, REACTION_TEXT_MAX_BYTES)
        }
        EventBody::DeviceKeyPackagePublished {
            protocol,
            key_package,
            ..
        } => {
            validate_event_text_size(
                "device key package protocol",
                protocol,
                DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
            )?;
            validate_event_payload_size(
                "OpenMLS key package",
                key_package,
                OPENMLS_KEY_PACKAGE_MAX_BYTES,
            )
        }
        EventBody::OpenMlsWorkspaceGroupMemberAdded {
            invitee_key_package_ref,
            protocol,
            ciphersuite,
            group_id,
            commit,
            welcome,
            ratchet_tree,
            ..
        }
        | EventBody::OpenMlsChannelGroupMemberAdded {
            invitee_key_package_ref,
            protocol,
            ciphersuite,
            group_id,
            commit,
            welcome,
            ratchet_tree,
            ..
        } => {
            validate_openmls_group_metadata(
                Some(invitee_key_package_ref.as_str()),
                protocol,
                ciphersuite,
                group_id,
            )?;
            validate_event_payload_size("OpenMLS commit", commit, OPENMLS_COMMIT_MAX_BYTES)?;
            validate_event_payload_size("OpenMLS welcome", welcome, OPENMLS_WELCOME_MAX_BYTES)?;
            validate_event_payload_size(
                "OpenMLS ratchet tree",
                ratchet_tree,
                OPENMLS_RATCHET_TREE_MAX_BYTES,
            )
        }
        EventBody::OpenMlsWorkspaceGroupMemberRemoved {
            protocol,
            ciphersuite,
            group_id,
            commit,
            ratchet_tree,
            ..
        }
        | EventBody::OpenMlsChannelGroupMemberRemoved {
            protocol,
            ciphersuite,
            group_id,
            commit,
            ratchet_tree,
            ..
        }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated {
            protocol,
            ciphersuite,
            group_id,
            commit,
            ratchet_tree,
            ..
        }
        | EventBody::OpenMlsChannelGroupSelfUpdated {
            protocol,
            ciphersuite,
            group_id,
            commit,
            ratchet_tree,
            ..
        } => {
            validate_openmls_group_metadata(None, protocol, ciphersuite, group_id)?;
            validate_event_payload_size("OpenMLS commit", commit, OPENMLS_COMMIT_MAX_BYTES)?;
            validate_event_payload_size(
                "OpenMLS ratchet tree",
                ratchet_tree,
                OPENMLS_RATCHET_TREE_MAX_BYTES,
            )
        }
        _ => Ok(()),
    }
}

fn validate_openmls_group_metadata(
    key_package_ref: Option<&str>,
    protocol: &str,
    ciphersuite: &str,
    group_id: &str,
) -> Result<(), AuthorizationError> {
    if let Some(key_package_ref) = key_package_ref {
        validate_event_text_size(
            "OpenMLS key package ref",
            key_package_ref,
            OPENMLS_KEY_PACKAGE_REF_MAX_BYTES,
        )?;
    }
    validate_event_text_size("OpenMLS protocol", protocol, OPENMLS_PROTOCOL_MAX_BYTES)?;
    validate_event_text_size(
        "OpenMLS ciphersuite",
        ciphersuite,
        OPENMLS_CIPHERSUITE_MAX_BYTES,
    )?;
    validate_event_text_size("OpenMLS group ID", group_id, OPENMLS_GROUP_ID_MAX_BYTES)
}

fn validate_message_attachments(attachments: &[AttachmentRef]) -> Result<(), AuthorizationError> {
    if attachments.len() > MESSAGE_ATTACHMENT_MAX_COUNT {
        return Err(AuthorizationError::EventItemCountTooLarge {
            label: "message attachments",
            actual_count: attachments.len(),
            max_count: MESSAGE_ATTACHMENT_MAX_COUNT,
        });
    }

    for attachment in attachments {
        validate_event_text_size(
            "attachment blob hash",
            &attachment.blob_hash,
            ATTACHMENT_BLOB_HASH_MAX_BYTES,
        )?;
        validate_event_text_size(
            "attachment media type",
            &attachment.media_type,
            ATTACHMENT_MEDIA_TYPE_MAX_BYTES,
        )?;
        validate_event_text_size(
            "attachment display name",
            &attachment.display_name,
            ATTACHMENT_DISPLAY_NAME_MAX_BYTES,
        )?;
        validate_event_text_size(
            "attachment id",
            &attachment.attachment_id,
            ATTACHMENT_ID_MAX_BYTES,
        )?;
        validate_event_u64_size(
            "attachment ciphertext length",
            attachment.byte_len,
            ATTACHMENT_CIPHERTEXT_MAX_BYTES,
        )?;

        if let Some(encryption) = &attachment.encryption {
            validate_event_text_size(
                "attachment encryption key id",
                &encryption.key_id,
                ATTACHMENT_KEY_ID_MAX_BYTES,
            )?;
            validate_event_payload_size(
                "attachment encryption nonce",
                &encryption.nonce,
                SEALED_PAYLOAD_NONCE_MAX_BYTES,
            )?;
            validate_event_payload_size(
                "attachment encryption aad",
                &encryption.aad,
                SEALED_PAYLOAD_AAD_MAX_BYTES,
            )?;
            validate_event_u64_size(
                "attachment plaintext length",
                encryption.plaintext_byte_len,
                ATTACHMENT_PLAINTEXT_MAX_BYTES,
            )?;
        }
    }

    Ok(())
}

fn validate_sealed_markdown_payload(payload: &SealedPayload) -> Result<(), AuthorizationError> {
    validate_event_text_size(
        "sealed message key ID",
        &payload.key_id,
        SEALED_PAYLOAD_KEY_ID_MAX_BYTES,
    )?;
    validate_event_payload_size(
        "sealed message ciphertext",
        &payload.bytes,
        SEALED_MESSAGE_MARKDOWN_MAX_BYTES,
    )?;
    validate_event_payload_size(
        "sealed message nonce",
        &payload.nonce,
        SEALED_PAYLOAD_NONCE_MAX_BYTES,
    )?;
    validate_event_payload_size(
        "sealed message aad",
        &payload.aad,
        SEALED_PAYLOAD_AAD_MAX_BYTES,
    )
}

fn validate_event_text_size(
    label: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), AuthorizationError> {
    let actual_bytes = value.len();
    if actual_bytes > max_bytes {
        return Err(AuthorizationError::EventPayloadTooLarge {
            label,
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}

fn validate_event_text_required(
    label: &'static str,
    value: &str,
) -> Result<(), AuthorizationError> {
    if value.trim().is_empty() {
        return Err(AuthorizationError::EventPayloadRequired { label });
    }
    Ok(())
}

fn validate_event_payload_size(
    label: &'static str,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), AuthorizationError> {
    if bytes.len() > max_bytes {
        return Err(AuthorizationError::EventPayloadTooLarge {
            label,
            actual_bytes: bytes.len(),
            max_bytes,
        });
    }
    Ok(())
}

fn validate_event_u64_size(
    label: &'static str,
    actual_bytes: u64,
    max_bytes: u64,
) -> Result<(), AuthorizationError> {
    if actual_bytes > max_bytes {
        return Err(AuthorizationError::EventPayloadTooLarge {
            label,
            actual_bytes: usize::try_from(actual_bytes).unwrap_or(usize::MAX),
            max_bytes: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Action {
    GrantOwner,
    InviteMember,
    RemoveMember,
    CreateChannel,
    RotateContentKey,
    ManageOpenMlsGroup,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::GrantOwner => "grant_owner",
            Self::InviteMember => "invite_member",
            Self::RemoveMember => "remove_member",
            Self::CreateChannel => "create_channel",
            Self::RotateContentKey => "rotate_content_key",
            Self::ManageOpenMlsGroup => "manage_openmls_group",
        }
    }
}

fn require_role(role: WorkspaceRole, action: Action) -> Result<(), AuthorizationError> {
    let allowed = match action {
        Action::GrantOwner => role == WorkspaceRole::Owner,
        Action::InviteMember => matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        Action::RemoveMember => matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        Action::CreateChannel => matches!(
            role,
            WorkspaceRole::Owner | WorkspaceRole::Admin | WorkspaceRole::Member
        ),
        Action::RotateContentKey => matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        Action::ManageOpenMlsGroup => matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
    };

    if allowed {
        Ok(())
    } else {
        Err(AuthorizationError::InsufficientRole {
            role,
            action: action.label(),
        })
    }
}

#[cfg(test)]
mod tests {
    use chaft_types::{
        ChannelId, ContentKeyScope, DeviceId, DeviceKeyPackageId, EventBody, MessageId,
        SignableEvent, SignedEvent, WorkspaceId,
    };

    use super::*;

    fn signed(event: SignableEvent) -> SignedEvent {
        SignedEvent::from_signed_bytes(event, vec![7, 7, 7])
    }

    fn assert_payload_too_large(
        error: AuthorizationError,
        expected_label: &'static str,
        expected_actual_bytes: usize,
        expected_max_bytes: usize,
    ) {
        match error {
            AuthorizationError::EventPayloadTooLarge {
                label,
                actual_bytes,
                max_bytes,
            } => {
                assert_eq!(label, expected_label);
                assert_eq!(actual_bytes, expected_actual_bytes);
                assert_eq!(max_bytes, expected_max_bytes);
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    fn assert_item_count_too_large(
        error: AuthorizationError,
        expected_label: &'static str,
        expected_actual_count: usize,
        expected_max_count: usize,
    ) {
        match error {
            AuthorizationError::EventItemCountTooLarge {
                label,
                actual_count,
                max_count,
            } => {
                assert_eq!(label, expected_label);
                assert_eq!(actual_count, expected_actual_count);
                assert_eq!(max_count, expected_max_count);
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    fn assert_payload_required(error: AuthorizationError, expected_label: &'static str) {
        match error {
            AuthorizationError::EventPayloadRequired { label } => {
                assert_eq!(label, expected_label);
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    fn assert_payload_rejected_before_materialization(
        workspace_id: WorkspaceId,
        history: &[SignedEvent],
        batch: &[SignedEvent],
        event: &SignedEvent,
        expected_label: &'static str,
        expected_actual_bytes: usize,
        expected_max_bytes: usize,
    ) {
        assert_payload_too_large(
            authorize_event_with_history(history, event).unwrap_err(),
            expected_label,
            expected_actual_bytes,
            expected_max_bytes,
        );
        match WorkspaceState::new(workspace_id)
            .apply_batch(batch)
            .unwrap_err()
        {
            CoreError::Authorization(error) => assert_payload_too_large(
                error,
                expected_label,
                expected_actual_bytes,
                expected_max_bytes,
            ),
            error => panic!("unexpected error: {error}"),
        }
    }

    fn assert_required_payload_rejected_before_materialization(
        workspace_id: WorkspaceId,
        history: &[SignedEvent],
        batch: &[SignedEvent],
        event: &SignedEvent,
        expected_label: &'static str,
    ) {
        assert_payload_required(
            authorize_event_with_history(history, event).unwrap_err(),
            expected_label,
        );
        match WorkspaceState::new(workspace_id)
            .apply_batch(batch)
            .unwrap_err()
        {
            CoreError::Authorization(error) => assert_payload_required(error, expected_label),
            error => panic!("unexpected error: {error}"),
        }
    }

    fn bounded_attachment(index: usize) -> AttachmentRef {
        AttachmentRef {
            blob_hash: format!("blob_hash_{index}"),
            media_type: "application/octet-stream".to_owned(),
            byte_len: 16,
            display_name: format!("file_{index}.bin"),
            attachment_id: format!("att_{index}"),
            encryption: Some(chaft_types::EncryptedBlobRef {
                mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
                key_id: "key".to_owned(),
                nonce: vec![0; SEALED_PAYLOAD_NONCE_MAX_BYTES],
                aad: b"attachment aad".to_vec(),
                plaintext_byte_len: 0,
            }),
        }
    }

    fn workspace_root(workspace_id: &WorkspaceId, owner: &DeviceId) -> SignedEvent {
        signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ))
    }

    fn public_channel(
        workspace_id: &WorkspaceId,
        owner: &DeviceId,
        channel_id: &ChannelId,
    ) -> SignedEvent {
        signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ))
    }

    fn plaintext_message(
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        owner: &DeviceId,
        message_id: &MessageId,
    ) -> SignedEvent {
        signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "hello".to_owned(),
                attachments: Vec::new(),
            },
        ))
    }

    fn sample_trust_snapshot() -> TrustSnapshot {
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let channel_id = ChannelId("chn_general".to_owned());

        TrustSnapshot {
            schema_version: 1,
            workspace_id: WorkspaceId("wrk_snapshot".to_owned()),
            root_event_id: EventId("evt_root".to_owned()),
            root_author_device_id: owner.clone(),
            roles: vec![TrustSnapshotRole {
                device_id: member.clone(),
                role: WorkspaceRole::Member,
            }],
            channels: vec![TrustSnapshotChannel {
                channel_id: channel_id.clone(),
                is_private: false,
                creator_device_id: owner,
                member_device_ids: vec![member],
            }],
            messages: vec![TrustSnapshotMessage {
                message_id: MessageId("msg_one".to_owned()),
                channel_id: channel_id.clone(),
            }],
            event_channels: vec![TrustSnapshotEventChannel {
                event_id: EventId("evt_channel".to_owned()),
                channel_id,
            }],
        }
    }

    fn assert_invalid_trust_snapshot(snapshot: TrustSnapshot) {
        assert!(matches!(
            WorkspaceAccessIndex::from_trust_snapshot(&snapshot),
            Err(AuthorizationError::InvalidTrustSnapshot)
        ));
    }

    #[test]
    fn materializes_channel_and_message_events() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let message_id = MessageId::new();
        let mut state = WorkspaceState::new(workspace_id.clone());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));

        state.apply(&root).unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::ChannelCreated {
                    channel_id: channel_id.clone(),
                    name: "general".to_owned(),
                    is_private: false,
                },
            )))
            .unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id),
                device_id,
                EventBody::MessageCreated {
                    message_id: message_id.clone(),
                    markdown: "hello".to_owned(),
                    attachments: Vec::new(),
                },
            )))
            .unwrap();

        assert_eq!(state.channels.len(), 1);
        assert_eq!(state.messages[&message_id].markdown, "hello");
    }

    #[test]
    fn materializes_reactions_once_per_author_device() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let message_id = MessageId::new();
        let mut state = WorkspaceState::new(workspace_id.clone());

        for event in [
            SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::WorkspaceCreated {
                    name: "Chaft".to_owned(),
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::MemberInvited {
                    invitee_device_id: member.clone(),
                    role: WorkspaceRole::Member,
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::ChannelCreated {
                    channel_id: channel_id.clone(),
                    name: "general".to_owned(),
                    is_private: false,
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::MessageCreated {
                    message_id: message_id.clone(),
                    markdown: "reactable".to_owned(),
                    attachments: Vec::new(),
                },
            ),
        ] {
            state.apply(&signed(event)).unwrap();
        }

        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::ReactionAdded {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::ReactionAdded {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        assert_eq!(state.messages[&message_id].reactions.get("+1"), Some(&1));
        assert_eq!(
            state.messages[&message_id].reactions_for_device(&owner),
            vec!["+1".to_owned()]
        );
        assert!(
            state.messages[&message_id]
                .reactions_for_device(&member)
                .is_empty()
        );

        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                member.clone(),
                EventBody::ReactionAdded {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        assert_eq!(state.messages[&message_id].reactions.get("+1"), Some(&2));
        assert_eq!(
            state.messages[&message_id].reactions_for_device(&member),
            vec!["+1".to_owned()]
        );

        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::ReactionRemoved {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                Some(channel_id.clone()),
                owner.clone(),
                EventBody::ReactionRemoved {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        assert_eq!(state.messages[&message_id].reactions.get("+1"), Some(&1));
        assert!(
            state.messages[&message_id]
                .reactions_for_device(&owner)
                .is_empty()
        );
        assert_eq!(
            state.messages[&message_id].reactions_for_device(&member),
            vec!["+1".to_owned()]
        );

        state
            .apply(&signed(SignableEvent::new(
                workspace_id,
                Some(channel_id),
                member.clone(),
                EventBody::ReactionRemoved {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            )))
            .unwrap();
        assert_eq!(state.messages[&message_id].reactions.get("+1"), None);
        assert!(
            state.messages[&message_id]
                .reactions_for_device(&member)
                .is_empty()
        );
    }

    #[test]
    fn materializes_encrypted_message_without_plaintext() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let message_id = MessageId::new();
        let sealed_markdown = SealedPayload {
            mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
            key_id: "workspace-key-1".to_owned(),
            nonce: vec![1; 12],
            aad: b"message context".to_vec(),
            bytes: b"ciphertext only".to_vec(),
        };
        let mut state = WorkspaceState::new(workspace_id.clone());

        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::WorkspaceCreated {
                    name: "Chaft".to_owned(),
                },
            )))
            .unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::ChannelCreated {
                    channel_id: channel_id.clone(),
                    name: "general".to_owned(),
                    is_private: false,
                },
            )))
            .unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id,
                Some(channel_id),
                device_id,
                EventBody::MessageCreatedEncrypted {
                    message_id: message_id.clone(),
                    sealed_markdown: sealed_markdown.clone(),
                    attachments: Vec::new(),
                },
            )))
            .unwrap();

        let message = &state.messages[&message_id];

        assert!(message.markdown.is_empty());
        assert_eq!(message.sealed_markdown, Some(sealed_markdown));
    }

    #[test]
    fn message_reply_requires_target_in_same_channel() {
        let workspace_id = WorkspaceId::new();
        let general_id = ChannelId::new();
        let random_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let parent_message_id = MessageId::new();
        let reply_message_id = MessageId::new();
        let mut state = WorkspaceState::new(workspace_id.clone());

        for event in [
            SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::WorkspaceCreated {
                    name: "Chaft".to_owned(),
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::ChannelCreated {
                    channel_id: general_id.clone(),
                    name: "general".to_owned(),
                    is_private: false,
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                None,
                device_id.clone(),
                EventBody::ChannelCreated {
                    channel_id: random_id.clone(),
                    name: "random".to_owned(),
                    is_private: false,
                },
            ),
            SignableEvent::new(
                workspace_id.clone(),
                Some(general_id),
                device_id.clone(),
                EventBody::MessageCreated {
                    message_id: parent_message_id.clone(),
                    markdown: "parent".to_owned(),
                    attachments: Vec::new(),
                },
            ),
        ] {
            state.apply(&signed(event)).unwrap();
        }

        let error = state
            .apply(&signed(SignableEvent::new(
                workspace_id,
                Some(random_id.clone()),
                device_id,
                EventBody::MessageReplyCreated {
                    message_id: reply_message_id,
                    reply_to_message_id: parent_message_id,
                    markdown: "cross-channel reply".to_owned(),
                    attachments: Vec::new(),
                },
            )))
            .unwrap_err();

        assert!(matches!(
            error,
            CoreError::Authorization(AuthorizationError::ChannelMismatch { actual, .. })
                if actual == random_id
        ));
    }

    #[test]
    fn materializes_read_marker_per_device_and_channel() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_reader".to_owned());
        let message_id = MessageId::new();
        let mut state = WorkspaceState::new(workspace_id.clone());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            device_id.clone(),
            EventBody::MessageCreated {
                message_id,
                markdown: "read through".to_owned(),
                attachments: Vec::new(),
            },
        ));

        state.apply(&root).unwrap();
        state.apply(&channel).unwrap();
        state.apply(&message).unwrap();
        state
            .apply(&signed(SignableEvent::new(
                workspace_id,
                Some(channel_id.clone()),
                device_id.clone(),
                EventBody::ReadMarkerUpdated {
                    channel_id: channel_id.clone(),
                    event_id: message.event_id.clone(),
                },
            )))
            .unwrap();

        assert_eq!(
            state
                .read_markers
                .get(&device_id)
                .and_then(|channels| channels.get(&channel_id)),
            Some(&message.event_id)
        );
    }

    #[test]
    fn apply_rejects_event_with_missing_causal_parent() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let message_id = MessageId::new();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreated {
                message_id,
                markdown: "partial history".to_owned(),
                attachments: Vec::new(),
            },
        );
        message.parents = vec![EventId("evt_missing_parent".to_owned())];
        let message = signed(message);
        let mut state = WorkspaceState::new(workspace_id);

        assert_eq!(
            state.apply(&message),
            Err(CoreError::MissingParents {
                event_id: message.event_id,
                missing_parent_ids: vec![EventId("evt_missing_parent".to_owned())],
            })
        );
    }

    #[test]
    fn apply_batch_orders_ready_events_before_children() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let message_id = MessageId::new();
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel_event = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            device_id.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "child event".to_owned(),
                attachments: Vec::new(),
            },
        );
        message.parents = vec![channel_event.event_id.clone()];
        let message = signed(message);
        let mut state = WorkspaceState::new(workspace_id);

        let report = state
            .apply_batch(&[message.clone(), channel_event.clone(), channel.clone()])
            .unwrap();

        assert!(report.gaps.is_empty());
        assert_eq!(
            report.applied_events,
            vec![channel.event_id, channel_event.event_id, message.event_id]
        );
        assert_eq!(state.messages[&message_id].markdown, "child event");
    }

    #[test]
    fn apply_batch_reports_unauthorized_ready_events_without_rendering_them() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let outsider = DeviceId("dev_outsider".to_owned());
        let message_id = MessageId::new();
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            outsider,
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "do not render".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let mut state = WorkspaceState::new(workspace_id);

        let report = state.apply_batch(std::slice::from_ref(&message)).unwrap();

        assert!(report.applied_events.is_empty());
        assert_eq!(
            report.gaps,
            vec![MissingHistoryGap {
                event_id: message.event_id,
                missing_parent_ids: Vec::new(),
            }]
        );
        assert!(!state.messages.contains_key(&message_id));
    }

    #[test]
    fn apply_batch_reports_gaps_without_applying_incomplete_events() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let message_id = MessageId::new();
        let missing_parent_id = EventId("evt_missing_parent".to_owned());
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            device_id,
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "hidden until gap fills".to_owned(),
                attachments: Vec::new(),
            },
        );
        message.parents = vec![missing_parent_id.clone()];
        let message = signed(message);
        let mut state = WorkspaceState::new(workspace_id);

        let report = state.apply_batch(std::slice::from_ref(&message)).unwrap();

        assert!(report.applied_events.is_empty());
        assert_eq!(
            report.gaps,
            vec![MissingHistoryGap {
                event_id: message.event_id,
                missing_parent_ids: vec![missing_parent_id],
            }]
        );
        assert!(!state.messages.contains_key(&message_id));
    }

    #[test]
    fn workspace_root_establishes_owner_role() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let mut index = WorkspaceAccessIndex::new(workspace_id);

        index.authorize_and_apply(&root).unwrap();

        assert_eq!(index.role_for(&owner), Some(WorkspaceRole::Owner));
    }

    #[test]
    fn invited_member_can_message() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id,
            Some(channel_id),
            member,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "authorized".to_owned(),
                attachments: Vec::new(),
            },
        ));

        assert!(authorize_event_with_history(&[root, invite, channel], &message).is_ok());
    }

    #[test]
    fn invited_member_can_publish_device_key_package() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let key_package_id = DeviceKeyPackageId::new();
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let package = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member.clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: "openmls/key-package".to_owned(),
                key_package: vec![1, 2, 3, 4],
            },
        ));
        let mut state = WorkspaceState::new(workspace_id);

        let report = state
            .apply_batch(&[root.clone(), invite.clone(), package.clone()])
            .unwrap();

        assert_eq!(report.applied_events.len(), 3);
        assert!(authorize_event_with_history(&[root, invite], &package).is_ok());
        let materialized = state.key_packages.get(&key_package_id).unwrap();
        assert_eq!(materialized.device_id, member);
        assert_eq!(materialized.protocol, "openmls/key-package");
        assert_eq!(materialized.key_package, vec![1, 2, 3, 4]);
        assert_eq!(materialized.published_event_id, package.event_id);
    }

    #[test]
    fn oversized_device_key_package_event_is_rejected_before_authorization_apply() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let package = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member,
            EventBody::DeviceKeyPackagePublished {
                key_package_id: DeviceKeyPackageId::new(),
                protocol: "openmls/key-package".to_owned(),
                key_package: vec![0; OPENMLS_KEY_PACKAGE_MAX_BYTES + 1],
            },
        ));
        let mut state = WorkspaceState::new(workspace_id);

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), invite.clone()], &package).unwrap_err(),
            "OpenMLS key package",
            OPENMLS_KEY_PACKAGE_MAX_BYTES + 1,
            OPENMLS_KEY_PACKAGE_MAX_BYTES,
        );
        match state.apply_batch(&[root, invite, package]).unwrap_err() {
            CoreError::Authorization(error) => assert_payload_too_large(
                error,
                "OpenMLS key package",
                OPENMLS_KEY_PACKAGE_MAX_BYTES + 1,
                OPENMLS_KEY_PACKAGE_MAX_BYTES,
            ),
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn oversized_openmls_event_artifacts_are_rejected_before_authorization_apply() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let openmls_add = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: member,
                invitee_key_package_id: DeviceKeyPackageId::new(),
                invitee_key_package_ref: "ref".to_owned(),
                protocol: "openmls/workspace-group/rfc9420".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "group".to_owned(),
                epoch: 1,
                commit: vec![1, 2, 3],
                welcome: vec![0; OPENMLS_WELCOME_MAX_BYTES + 1],
                ratchet_tree: vec![4, 5, 6],
            },
        ));

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), invite.clone()], &openmls_add)
                .unwrap_err(),
            "OpenMLS welcome",
            OPENMLS_WELCOME_MAX_BYTES + 1,
            OPENMLS_WELCOME_MAX_BYTES,
        );
        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, invite, openmls_add])
            .unwrap_err()
        {
            CoreError::Authorization(error) => assert_payload_too_large(
                error,
                "OpenMLS welcome",
                OPENMLS_WELCOME_MAX_BYTES + 1,
                OPENMLS_WELCOME_MAX_BYTES,
            ),
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn oversized_plaintext_message_markdown_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "x".repeat(MESSAGE_MARKDOWN_MAX_BYTES + 1),
                attachments: Vec::new(),
            },
        ));

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), channel.clone()], &message).unwrap_err(),
            "message markdown",
            MESSAGE_MARKDOWN_MAX_BYTES + 1,
            MESSAGE_MARKDOWN_MAX_BYTES,
        );
        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, channel, message])
            .unwrap_err()
        {
            CoreError::Authorization(error) => assert_payload_too_large(
                error,
                "message markdown",
                MESSAGE_MARKDOWN_MAX_BYTES + 1,
                MESSAGE_MARKDOWN_MAX_BYTES,
            ),
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn oversized_sealed_message_payload_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let oversized_ciphertext = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: SealedPayload {
                    mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
                    key_id: "key".to_owned(),
                    nonce: vec![0; SEALED_PAYLOAD_NONCE_MAX_BYTES],
                    aad: b"message aad".to_vec(),
                    bytes: vec![0; SEALED_MESSAGE_MARKDOWN_MAX_BYTES + 1],
                },
                attachments: Vec::new(),
            },
        ));
        let oversized_aad = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: SealedPayload {
                    mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
                    key_id: "key".to_owned(),
                    nonce: vec![0; SEALED_PAYLOAD_NONCE_MAX_BYTES],
                    aad: vec![0; SEALED_PAYLOAD_AAD_MAX_BYTES + 1],
                    bytes: vec![0; 16],
                },
                attachments: Vec::new(),
            },
        ));

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), channel.clone()], &oversized_ciphertext)
                .unwrap_err(),
            "sealed message ciphertext",
            SEALED_MESSAGE_MARKDOWN_MAX_BYTES + 1,
            SEALED_MESSAGE_MARKDOWN_MAX_BYTES,
        );
        match WorkspaceState::new(workspace_id).apply_batch(&[
            root.clone(),
            channel.clone(),
            oversized_ciphertext,
        ]) {
            Err(CoreError::Authorization(error)) => assert_payload_too_large(
                error,
                "sealed message ciphertext",
                SEALED_MESSAGE_MARKDOWN_MAX_BYTES + 1,
                SEALED_MESSAGE_MARKDOWN_MAX_BYTES,
            ),
            result => panic!("unexpected result: {result:?}"),
        }
        assert_payload_too_large(
            authorize_event_with_history(&[root, channel], &oversized_aad).unwrap_err(),
            "sealed message aad",
            SEALED_PAYLOAD_AAD_MAX_BYTES + 1,
            SEALED_PAYLOAD_AAD_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_message_attachment_count_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "with too many attachments".to_owned(),
                attachments: (0..=MESSAGE_ATTACHMENT_MAX_COUNT)
                    .map(bounded_attachment)
                    .collect(),
            },
        ));

        assert_item_count_too_large(
            authorize_event_with_history(&[root.clone(), channel.clone()], &message).unwrap_err(),
            "message attachments",
            MESSAGE_ATTACHMENT_MAX_COUNT + 1,
            MESSAGE_ATTACHMENT_MAX_COUNT,
        );
        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, channel, message])
            .unwrap_err()
        {
            CoreError::Authorization(error) => assert_item_count_too_large(
                error,
                "message attachments",
                MESSAGE_ATTACHMENT_MAX_COUNT + 1,
                MESSAGE_ATTACHMENT_MAX_COUNT,
            ),
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn oversized_message_attachment_metadata_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let mut oversized_display_name = bounded_attachment(0);
        oversized_display_name.display_name = "x".repeat(ATTACHMENT_DISPLAY_NAME_MAX_BYTES + 1);
        let display_name_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "with oversized metadata".to_owned(),
                attachments: vec![oversized_display_name],
            },
        ));
        let mut oversized_aad = bounded_attachment(1);
        oversized_aad
            .encryption
            .as_mut()
            .expect("bounded attachment is encrypted")
            .aad = vec![0; SEALED_PAYLOAD_AAD_MAX_BYTES + 1];
        let aad_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: SealedPayload {
                    mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
                    key_id: "key".to_owned(),
                    nonce: vec![0; SEALED_PAYLOAD_NONCE_MAX_BYTES],
                    aad: b"message aad".to_vec(),
                    bytes: vec![0; 16],
                },
                attachments: vec![oversized_aad],
            },
        ));

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), channel.clone()], &display_name_message)
                .unwrap_err(),
            "attachment display name",
            ATTACHMENT_DISPLAY_NAME_MAX_BYTES + 1,
            ATTACHMENT_DISPLAY_NAME_MAX_BYTES,
        );
        match WorkspaceState::new(workspace_id.clone()).apply_batch(&[
            root.clone(),
            channel.clone(),
            display_name_message,
        ]) {
            Err(CoreError::Authorization(error)) => assert_payload_too_large(
                error,
                "attachment display name",
                ATTACHMENT_DISPLAY_NAME_MAX_BYTES + 1,
                ATTACHMENT_DISPLAY_NAME_MAX_BYTES,
            ),
            result => panic!("unexpected result: {result:?}"),
        }

        assert_payload_too_large(
            authorize_event_with_history(&[root.clone(), channel.clone()], &aad_message)
                .unwrap_err(),
            "attachment encryption aad",
            SEALED_PAYLOAD_AAD_MAX_BYTES + 1,
            SEALED_PAYLOAD_AAD_MAX_BYTES,
        );
        match WorkspaceState::new(workspace_id).apply_batch(&[root, channel, aad_message]) {
            Err(CoreError::Authorization(error)) => assert_payload_too_large(
                error,
                "attachment encryption aad",
                SEALED_PAYLOAD_AAD_MAX_BYTES + 1,
                SEALED_PAYLOAD_AAD_MAX_BYTES,
            ),
            result => panic!("unexpected result: {result:?}"),
        }
    }

    #[test]
    fn oversized_workspace_channel_and_profile_metadata_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let oversized_root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "w".repeat(WORKSPACE_NAME_MAX_BYTES + 1),
            },
        ));

        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            &[],
            std::slice::from_ref(&oversized_root),
            &oversized_root,
            "workspace name",
            WORKSPACE_NAME_MAX_BYTES + 1,
            WORKSPACE_NAME_MAX_BYTES,
        );

        let root = workspace_root(&workspace_id, &owner);
        let oversized_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "c".repeat(CHANNEL_NAME_MAX_BYTES + 1),
                is_private: false,
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            std::slice::from_ref(&root),
            &[root.clone(), oversized_channel.clone()],
            &oversized_channel,
            "channel name",
            CHANNEL_NAME_MAX_BYTES + 1,
            CHANNEL_NAME_MAX_BYTES,
        );

        let oversized_profile = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "d".repeat(DEVICE_DISPLAY_NAME_MAX_BYTES + 1),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id,
            std::slice::from_ref(&root),
            &[root.clone(), oversized_profile.clone()],
            &oversized_profile,
            "display name",
            DEVICE_DISPLAY_NAME_MAX_BYTES + 1,
            DEVICE_DISPLAY_NAME_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_endpoint_reaction_and_content_key_metadata_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let message_id = MessageId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let channel = public_channel(&workspace_id, &owner, &channel_id);
        let message = plaintext_message(&workspace_id, &channel_id, &owner, &message_id);

        let oversized_endpoint = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "endpoint".to_owned(),
                endpoint: "e".repeat(PEER_ENDPOINT_MAX_BYTES + 1),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            std::slice::from_ref(&root),
            &[root.clone(), oversized_endpoint.clone()],
            &oversized_endpoint,
            "peer endpoint",
            PEER_ENDPOINT_MAX_BYTES + 1,
            PEER_ENDPOINT_MAX_BYTES,
        );

        let oversized_reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "r".repeat(REACTION_TEXT_MAX_BYTES + 1),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            &[root.clone(), channel.clone(), message.clone()],
            &[
                root.clone(),
                channel.clone(),
                message.clone(),
                oversized_reaction.clone(),
            ],
            &oversized_reaction,
            "reaction",
            REACTION_TEXT_MAX_BYTES + 1,
            REACTION_TEXT_MAX_BYTES,
        );

        let oversized_content_key = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: 2,
                key_id: "k".repeat(CONTENT_KEY_ID_MAX_BYTES + 1),
                previous_key_id: None,
                algorithm: "aes-256-gcm-siv".to_owned(),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id,
            std::slice::from_ref(&root),
            &[root.clone(), oversized_content_key.clone()],
            &oversized_content_key,
            "content key ID",
            CONTENT_KEY_ID_MAX_BYTES + 1,
            CONTENT_KEY_ID_MAX_BYTES,
        );
    }

    #[test]
    fn blank_peer_endpoint_metadata_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);

        for (label, endpoint_id, endpoint, transport) in [
            (
                "peer endpoint ID",
                " ",
                "direct+tcp://127.0.0.1:7777",
                "direct-tcp",
            ),
            ("peer endpoint", "desktop", " ", "direct-tcp"),
            (
                "peer endpoint transport",
                "desktop",
                "direct+tcp://127.0.0.1:7777",
                " ",
            ),
        ] {
            let endpoint_event = signed(SignableEvent::new(
                workspace_id.clone(),
                None,
                owner.clone(),
                EventBody::PeerEndpointPublished {
                    endpoint_id: endpoint_id.to_owned(),
                    endpoint: endpoint.to_owned(),
                    transport: transport.to_owned(),
                    is_backup_peer: true,
                    expires_at_ms: None,
                    replica_storage_class: None,
                    replica_retention_hint: None,
                },
            ));
            assert_required_payload_rejected_before_materialization(
                workspace_id.clone(),
                std::slice::from_ref(&root),
                &[root.clone(), endpoint_event.clone()],
                &endpoint_event,
                label,
            );
        }
    }

    #[test]
    fn unsupported_peer_endpoint_route_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let endpoint_event = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "centralized-ws".to_owned(),
                endpoint: "wss://central.example.invalid/sync".to_owned(),
                transport: "wss".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));

        assert_eq!(
            authorize_event_with_history(std::slice::from_ref(&root), &endpoint_event),
            Err(AuthorizationError::UnsupportedPeerEndpoint)
        );

        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, endpoint_event])
            .unwrap_err()
        {
            CoreError::Authorization(AuthorizationError::UnsupportedPeerEndpoint) => {}
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn malformed_direct_peer_endpoint_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let endpoint_event = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "bad-direct".to_owned(),
                endpoint: "direct+tcp://not-a-socket".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));

        assert_eq!(
            authorize_event_with_history(std::slice::from_ref(&root), &endpoint_event),
            Err(AuthorizationError::UnsupportedPeerEndpoint)
        );

        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, endpoint_event])
            .unwrap_err()
        {
            CoreError::Authorization(AuthorizationError::UnsupportedPeerEndpoint) => {}
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn mismatched_peer_endpoint_transport_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let endpoint_event = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "bad-label".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "iroh".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));

        assert_eq!(
            authorize_event_with_history(std::slice::from_ref(&root), &endpoint_event),
            Err(AuthorizationError::PeerEndpointTransportMismatch)
        );

        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, endpoint_event])
            .unwrap_err()
        {
            CoreError::Authorization(AuthorizationError::PeerEndpointTransportMismatch) => {}
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn replica_capability_requires_backup_peer_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let endpoint_event = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "member-route".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: Some(ReplicaStorageClass::FullHistoryWithBlobs),
                replica_retention_hint: Some("30d".to_owned()),
            },
        ));

        assert_eq!(
            authorize_event_with_history(std::slice::from_ref(&root), &endpoint_event),
            Err(AuthorizationError::ReplicaCapabilityRequiresBackupPeer)
        );

        match WorkspaceState::new(workspace_id)
            .apply_batch(&[root, endpoint_event])
            .unwrap_err()
        {
            CoreError::Authorization(AuthorizationError::ReplicaCapabilityRequiresBackupPeer) => {}
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn oversized_openmls_and_sealed_key_metadata_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let channel = public_channel(&workspace_id, &owner, &channel_id);

        let oversized_protocol = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: DeviceKeyPackageId::new(),
                protocol: "p".repeat(DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES + 1),
                key_package: vec![0; 16],
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            std::slice::from_ref(&root),
            &[root.clone(), oversized_protocol.clone()],
            &oversized_protocol,
            "device key package protocol",
            DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES + 1,
            DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES,
        );

        let oversized_group_id = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: member,
                invitee_key_package_id: DeviceKeyPackageId::new(),
                invitee_key_package_ref: "ref".to_owned(),
                protocol: "openmls/workspace-group/rfc9420".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "g".repeat(OPENMLS_GROUP_ID_MAX_BYTES + 1),
                epoch: 1,
                commit: Vec::new(),
                welcome: Vec::new(),
                ratchet_tree: Vec::new(),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            std::slice::from_ref(&root),
            &[root.clone(), oversized_group_id.clone()],
            &oversized_group_id,
            "OpenMLS group ID",
            OPENMLS_GROUP_ID_MAX_BYTES + 1,
            OPENMLS_GROUP_ID_MAX_BYTES,
        );

        let oversized_sealed_key = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId::new(),
                sealed_markdown: SealedPayload {
                    mode: chaft_types::PayloadEncryption::Aes256GcmSiv,
                    key_id: "key".repeat(SEALED_PAYLOAD_KEY_ID_MAX_BYTES + 1),
                    nonce: vec![0; SEALED_PAYLOAD_NONCE_MAX_BYTES],
                    aad: b"message aad".to_vec(),
                    bytes: vec![0; 16],
                },
                attachments: Vec::new(),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id,
            &[root.clone(), channel.clone()],
            &[root, channel, oversized_sealed_key.clone()],
            &oversized_sealed_key,
            "sealed message key ID",
            ("key".len()) * (SEALED_PAYLOAD_KEY_ID_MAX_BYTES + 1),
            SEALED_PAYLOAD_KEY_ID_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_event_envelope_ids_are_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let mut oversized_event_id = workspace_root(&workspace_id, &owner);
        oversized_event_id.event_id = EventId("e".repeat(EVENT_ID_MAX_BYTES + 1));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            &[],
            std::slice::from_ref(&oversized_event_id),
            &oversized_event_id,
            "event ID",
            EVENT_ID_MAX_BYTES + 1,
            EVENT_ID_MAX_BYTES,
        );

        let mut oversized_parent_id = workspace_root(&workspace_id, &owner);
        oversized_parent_id.event.parents = vec![EventId("p".repeat(EVENT_ID_MAX_BYTES + 1))];
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            &[],
            std::slice::from_ref(&oversized_parent_id),
            &oversized_parent_id,
            "parent event ID",
            EVENT_ID_MAX_BYTES + 1,
            EVENT_ID_MAX_BYTES,
        );

        let oversized_author = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            DeviceId("d".repeat(DEVICE_ID_MAX_BYTES + 1)),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id,
            &[],
            std::slice::from_ref(&oversized_author),
            &oversized_author,
            "author device ID",
            DEVICE_ID_MAX_BYTES + 1,
            DEVICE_ID_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_event_signature_material_is_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let mut oversized_public_key = workspace_root(&workspace_id, &owner);
        oversized_public_key.author_public_key = vec![0; EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES + 1];
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            &[],
            std::slice::from_ref(&oversized_public_key),
            &oversized_public_key,
            "author public key",
            EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES + 1,
            EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES,
        );

        let mut oversized_signature = workspace_root(&workspace_id, &owner);
        oversized_signature.signature = vec![0; EVENT_SIGNATURE_MAX_BYTES + 1];
        assert_payload_rejected_before_materialization(
            workspace_id,
            &[],
            std::slice::from_ref(&oversized_signature),
            &oversized_signature,
            "event signature",
            EVENT_SIGNATURE_MAX_BYTES + 1,
            EVENT_SIGNATURE_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_event_body_ids_are_rejected_before_materialization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let oversized_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: ChannelId("c".repeat(CHANNEL_ID_MAX_BYTES + 1)),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id.clone(),
            std::slice::from_ref(&root),
            &[root.clone(), oversized_channel.clone()],
            &oversized_channel,
            "channel ID",
            CHANNEL_ID_MAX_BYTES + 1,
            CHANNEL_ID_MAX_BYTES,
        );

        let channel = public_channel(&workspace_id, &owner, &channel_id);
        let oversized_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId("m".repeat(MESSAGE_ID_MAX_BYTES + 1)),
                markdown: "hello".to_owned(),
                attachments: Vec::new(),
            },
        ));
        assert_payload_rejected_before_materialization(
            workspace_id,
            &[root.clone(), channel.clone()],
            &[root, channel, oversized_message.clone()],
            &oversized_message,
            "message ID",
            MESSAGE_ID_MAX_BYTES + 1,
            MESSAGE_ID_MAX_BYTES,
        );
    }

    #[test]
    fn oversized_trust_snapshot_ids_are_rejected_before_indexing() {
        let mut oversized_root_event_id = sample_trust_snapshot();
        oversized_root_event_id.root_event_id = EventId("e".repeat(EVENT_ID_MAX_BYTES + 1));
        assert_payload_too_large(
            WorkspaceAccessIndex::from_trust_snapshot(&oversized_root_event_id).unwrap_err(),
            "trust snapshot root event ID",
            EVENT_ID_MAX_BYTES + 1,
            EVENT_ID_MAX_BYTES,
        );

        let mut oversized_role_device_id = sample_trust_snapshot();
        oversized_role_device_id.roles[0].device_id = DeviceId("d".repeat(DEVICE_ID_MAX_BYTES + 1));
        assert_payload_too_large(
            WorkspaceAccessIndex::from_trust_snapshot(&oversized_role_device_id).unwrap_err(),
            "trust snapshot role device ID",
            DEVICE_ID_MAX_BYTES + 1,
            DEVICE_ID_MAX_BYTES,
        );

        let mut oversized_message_id = sample_trust_snapshot();
        oversized_message_id.messages[0].message_id =
            MessageId("m".repeat(MESSAGE_ID_MAX_BYTES + 1));
        assert_payload_too_large(
            WorkspaceAccessIndex::from_trust_snapshot(&oversized_message_id).unwrap_err(),
            "trust snapshot message ID",
            MESSAGE_ID_MAX_BYTES + 1,
            MESSAGE_ID_MAX_BYTES,
        );
    }

    #[test]
    fn invited_member_can_publish_peer_endpoint_hint() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let first_endpoint = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "laptop-lan".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: Some(1_700_000_600_000),
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));
        let second_endpoint = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "laptop-lan".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:8888".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: None,
                replica_storage_class: Some(ReplicaStorageClass::FullHistoryWithBlobs),
                replica_retention_hint: Some("30d".to_owned()),
            },
        ));
        let removal = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberRemoved {
                removed_device_id: member.clone(),
            },
        ));
        let after_removal = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            member.clone(),
            EventBody::PeerEndpointPublished {
                endpoint_id: "removed".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:9999".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: false,
                expires_at_ms: None,
                replica_storage_class: None,
                replica_retention_hint: None,
            },
        ));
        let mut state = WorkspaceState::new(workspace_id);

        let report = state
            .apply_batch(&[
                root.clone(),
                invite.clone(),
                first_endpoint.clone(),
                second_endpoint.clone(),
            ])
            .unwrap();

        assert_eq!(report.applied_events.len(), 4);
        assert!(
            authorize_event_with_history(&[root.clone(), invite.clone()], &first_endpoint).is_ok()
        );
        let materialized = state
            .peer_endpoints
            .get(&(member.clone(), "laptop-lan".to_owned()))
            .unwrap();
        assert_eq!(materialized.endpoint, "direct+tcp://127.0.0.1:8888");
        assert_eq!(materialized.transport, "direct-tcp");
        assert!(materialized.is_backup_peer);
        assert_eq!(materialized.expires_at_ms, None);
        assert_eq!(
            materialized.replica_storage_class,
            Some(ReplicaStorageClass::FullHistoryWithBlobs)
        );
        assert_eq!(materialized.replica_retention_hint.as_deref(), Some("30d"));
        assert_eq!(materialized.published_event_id, second_endpoint.event_id);

        assert_eq!(
            authorize_event_with_history(&[root, invite, removal], &after_removal),
            Err(AuthorizationError::NotAMember { device_id: member })
        );
    }

    #[test]
    fn uninvited_device_cannot_publish_device_key_package() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let stranger = DeviceId("dev_stranger".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let package = signed(SignableEvent::new(
            workspace_id,
            None,
            stranger.clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: DeviceKeyPackageId::new(),
                protocol: "openmls/key-package".to_owned(),
                key_package: vec![1, 2, 3, 4],
            },
        ));

        assert!(matches!(
            authorize_event_with_history(&[root], &package),
            Err(AuthorizationError::NotAMember { device_id }) if device_id == stranger
        ));
    }

    #[test]
    fn workspace_key_epoch_rotation_requires_admin_or_owner() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let rotation = signed(SignableEvent::new(
            workspace_id,
            None,
            member,
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: 2,
                key_id: "workspace-key-v2".to_owned(),
                previous_key_id: Some("workspace-key-v1".to_owned()),
                algorithm: "aes-256-gcm-siv".to_owned(),
            },
        ));

        assert!(matches!(
            authorize_event_with_history(&[root, invite], &rotation),
            Err(AuthorizationError::InsufficientRole { action, .. })
                if action == "rotate_content_key"
        ));
    }

    #[test]
    fn private_channel_restricts_invited_member_access_to_creator() {
        let workspace_id = WorkspaceId::new();
        let private_channel_id = ChannelId::new();
        let public_channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let private_message_id = MessageId::new();
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let private_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: private_channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let public_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: public_channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let private_owner_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(private_channel_id.clone()),
            owner,
            EventBody::MessageCreated {
                message_id: private_message_id.clone(),
                markdown: "private".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let public_member_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(public_channel_id),
            member.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "public".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let private_member_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(private_channel_id.clone()),
            member.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "not allowed".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let private_member_reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(private_channel_id.clone()),
            member.clone(),
            EventBody::ReactionAdded {
                message_id: private_message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));
        let private_member_read_marker = signed(SignableEvent::new(
            workspace_id,
            Some(private_channel_id.clone()),
            member.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: private_channel_id.clone(),
                event_id: private_owner_message.event_id.clone(),
            },
        ));
        let history = [
            root.clone(),
            invite.clone(),
            private_channel.clone(),
            public_channel,
            private_owner_message.clone(),
        ];

        assert!(
            authorize_event_with_history(
                &[root.clone(), private_channel.clone()],
                &private_owner_message
            )
            .is_ok()
        );
        assert!(authorize_event_with_history(&history, &public_member_message).is_ok());
        assert_eq!(
            authorize_event_with_history(&history, &private_member_message),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: private_channel_id.clone(),
                device_id: member.clone()
            })
        );
        assert_eq!(
            authorize_event_with_history(&history, &private_member_reaction),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: private_channel_id.clone(),
                device_id: member.clone()
            })
        );
        assert_eq!(
            authorize_event_with_history(&history, &private_member_read_marker),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: private_channel_id,
                device_id: member
            })
        );
    }

    #[test]
    fn private_channel_member_grant_authorizes_invited_member() {
        let workspace_id = WorkspaceId::new();
        let private_channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let other_member = DeviceId("dev_other_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let other_invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: other_member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let private_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: private_channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let grant = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelMemberAdded {
                channel_id: private_channel_id.clone(),
                member_device_id: member.clone(),
            },
        ));
        let member_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(private_channel_id.clone()),
            member.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "authorized private reply".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let member_grants_other = signed(SignableEvent::new(
            workspace_id,
            None,
            member.clone(),
            EventBody::ChannelMemberAdded {
                channel_id: private_channel_id.clone(),
                member_device_id: other_member,
            },
        ));
        let history = [
            root,
            invite,
            other_invite,
            private_channel.clone(),
            grant.clone(),
        ];

        assert!(authorize_event_with_history(&history, &member_message).is_ok());
        assert_eq!(
            authorize_event_with_history(&history, &member_grants_other),
            Err(AuthorizationError::ChannelMemberGrantDenied {
                channel_id: private_channel_id,
                device_id: member
            })
        );
    }

    #[test]
    fn member_removal_revokes_future_workspace_authorization() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let removal = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberRemoved {
                removed_device_id: member.clone(),
            },
        ));
        let member_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            member.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "after removal".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let mut state = WorkspaceState::new(workspace_id.clone());
        let history = vec![
            root.clone(),
            invite.clone(),
            channel.clone(),
            removal.clone(),
        ];

        state.apply(&root).unwrap();
        state.apply(&invite).unwrap();
        state.apply(&channel).unwrap();
        assert!(state.members.contains_key(&member));
        state.apply(&removal).unwrap();

        assert!(!state.members.contains_key(&member));
        assert_eq!(
            authorize_event_with_history(&history, &member_message),
            Err(AuthorizationError::NotAMember {
                device_id: member.clone()
            })
        );

        let (snapshot, _) = trust_snapshot_from_events(workspace_id, &history).unwrap();
        assert!(!snapshot.roles.iter().any(|role| role.device_id == member));
    }

    #[test]
    fn root_workspace_owner_cannot_be_removed() {
        let workspace_id = WorkspaceId::new();
        let root_owner = DeviceId("dev_root_owner".to_owned());
        let admin = DeviceId("dev_admin".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            root_owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite_admin = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            root_owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: admin.clone(),
                role: WorkspaceRole::Admin,
            },
        ));
        let remove_root = signed(SignableEvent::new(
            workspace_id,
            None,
            admin,
            EventBody::MemberRemoved {
                removed_device_id: root_owner.clone(),
            },
        ));

        assert_eq!(
            authorize_event_with_history(&[root, invite_admin], &remove_root),
            Err(AuthorizationError::WorkspaceRootCannotBeRemoved {
                device_id: root_owner
            })
        );
    }

    #[test]
    fn channel_member_removal_revokes_future_private_channel_access() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let grant = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelMemberAdded {
                channel_id: channel_id.clone(),
                member_device_id: member.clone(),
            },
        ));
        let removal = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelMemberRemoved {
                channel_id: channel_id.clone(),
                member_device_id: member.clone(),
            },
        ));
        let member_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            member.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "after channel removal".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let history = vec![root, invite, channel, grant, removal];

        assert_eq!(
            authorize_event_with_history(&history, &member_message),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: channel_id.clone(),
                device_id: member.clone()
            })
        );

        let (snapshot, _) = trust_snapshot_from_events(workspace_id, &history).unwrap();
        let channel = snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
            .unwrap();
        assert!(!channel.member_device_ids.contains(&member));
    }

    #[test]
    fn trust_snapshot_matches_history_for_removed_private_channel_creator() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let admin = DeviceId("dev_admin".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite_admin = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: admin.clone(),
                role: WorkspaceRole::Admin,
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let grant_admin = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelMemberAdded {
                channel_id: channel_id.clone(),
                member_device_id: admin.clone(),
            },
        ));
        let remove_owner_from_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            admin,
            EventBody::ChannelMemberRemoved {
                channel_id: channel_id.clone(),
                member_device_id: owner.clone(),
            },
        ));
        let owner_message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "after channel removal".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let history = vec![
            root,
            invite_admin,
            channel,
            grant_admin,
            remove_owner_from_channel,
        ];

        assert_eq!(
            authorize_event_with_history(&history, &owner_message),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id: channel_id.clone(),
                device_id: owner.clone()
            })
        );

        let (snapshot, _) = trust_snapshot_from_events(workspace_id, &history).unwrap();
        let channel = snapshot
            .channels
            .iter()
            .find(|channel| channel.channel_id == channel_id)
            .unwrap();
        assert!(!channel.member_device_ids.contains(&owner));
        assert_eq!(
            authorize_event_with_trust_snapshot(&snapshot, &owner_message),
            Err(AuthorizationError::PrivateChannelAccessDenied {
                channel_id,
                device_id: owner,
            })
        );
    }

    #[test]
    fn trust_snapshot_for_event_omits_unrelated_channels_messages_and_roles() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let author = DeviceId("dev_author".to_owned());
        let bystander = DeviceId("dev_bystander".to_owned());
        let target_channel_id = ChannelId("chn_target".to_owned());
        let unrelated_channel_id = ChannelId("chn_unrelated".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let invite_author = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: author.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let invite_bystander = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: bystander.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let target_channel = public_channel(&workspace_id, &owner, &target_channel_id);
        let unrelated_channel = public_channel(&workspace_id, &owner, &unrelated_channel_id);
        let target_message = plaintext_message(
            &workspace_id,
            &target_channel_id,
            &author,
            &MessageId("msg_target".to_owned()),
        );
        let unrelated_message = plaintext_message(
            &workspace_id,
            &unrelated_channel_id,
            &bystander,
            &MessageId("msg_unrelated".to_owned()),
        );
        let history = vec![
            root.clone(),
            invite_author,
            invite_bystander,
            target_channel,
            unrelated_channel,
            target_message.clone(),
            unrelated_message.clone(),
        ];

        let (snapshot, root_event) =
            trust_snapshot_for_event_from_events(workspace_id, &history, &target_message).unwrap();

        assert_eq!(root_event.event_id, root.event_id);
        assert_eq!(snapshot.roles.len(), 1);
        assert_eq!(snapshot.roles[0].device_id, author);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.channels[0].channel_id, target_channel_id);
        assert!(snapshot.messages.is_empty());
        assert!(snapshot.event_channels.is_empty());
        assert!(authorize_event_with_trust_snapshot(&snapshot, &target_message).is_ok());
        assert!(authorize_event_with_trust_snapshot(&snapshot, &unrelated_message).is_err());
    }

    #[test]
    fn trust_snapshot_for_event_keeps_message_and_event_targets() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let channel_id = ChannelId("chn_general".to_owned());
        let message_id = MessageId("msg_original".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let channel = public_channel(&workspace_id, &owner, &channel_id);
        let original_message = plaintext_message(&workspace_id, &channel_id, &owner, &message_id);
        let reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));
        let read_marker = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: original_message.event_id.clone(),
            },
        ));
        let history = vec![
            root,
            channel,
            original_message.clone(),
            reaction.clone(),
            read_marker.clone(),
        ];

        let (reaction_snapshot, _) =
            trust_snapshot_for_event_from_events(workspace_id.clone(), &history, &reaction)
                .unwrap();
        assert_eq!(reaction_snapshot.channels.len(), 1);
        assert_eq!(reaction_snapshot.channels[0].channel_id, channel_id);
        assert_eq!(reaction_snapshot.messages.len(), 1);
        assert_eq!(reaction_snapshot.messages[0].message_id, message_id);
        assert!(reaction_snapshot.event_channels.is_empty());
        assert!(authorize_event_with_trust_snapshot(&reaction_snapshot, &reaction).is_ok());

        let (read_snapshot, _) =
            trust_snapshot_for_event_from_events(workspace_id, &history, &read_marker).unwrap();
        assert_eq!(read_snapshot.channels.len(), 1);
        assert_eq!(read_snapshot.event_channels.len(), 1);
        assert_eq!(
            read_snapshot.event_channels[0].event_id,
            original_message.event_id
        );
        assert!(read_snapshot.messages.is_empty());
        assert!(authorize_event_with_trust_snapshot(&read_snapshot, &read_marker).is_ok());
    }

    #[test]
    fn trust_snapshot_for_private_channel_event_keeps_author_membership() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId("chn_strategy".to_owned());
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let invite_member = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let private_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "strategy".to_owned(),
                is_private: true,
            },
        ));
        let grant_member = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelMemberAdded {
                channel_id: channel_id.clone(),
                member_device_id: member.clone(),
            },
        ));
        let member_message = plaintext_message(
            &workspace_id,
            &channel_id,
            &member,
            &MessageId("msg_private".to_owned()),
        );
        let history = vec![
            root,
            invite_member,
            private_channel,
            grant_member,
            member_message.clone(),
        ];

        let (snapshot, _) =
            trust_snapshot_for_event_from_events(workspace_id, &history, &member_message).unwrap();

        assert_eq!(snapshot.roles.len(), 1);
        assert_eq!(snapshot.roles[0].device_id, member);
        assert_eq!(snapshot.channels.len(), 1);
        assert!(snapshot.channels[0].is_private);
        assert!(snapshot.channels[0].member_device_ids.contains(&member));
        assert!(authorize_event_with_trust_snapshot(&snapshot, &member_message).is_ok());
    }

    #[test]
    fn trust_snapshot_for_events_scopes_batch_dependencies() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let author = DeviceId("dev_author".to_owned());
        let bystander = DeviceId("dev_bystander".to_owned());
        let target_channel_id = ChannelId("chn_target".to_owned());
        let unrelated_channel_id = ChannelId("chn_unrelated".to_owned());
        let target_message_id = MessageId("msg_target".to_owned());
        let root = workspace_root(&workspace_id, &owner);
        let invite_author = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: author.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let invite_bystander = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: bystander.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let target_channel = public_channel(&workspace_id, &owner, &target_channel_id);
        let unrelated_channel = public_channel(&workspace_id, &owner, &unrelated_channel_id);
        let target_message = plaintext_message(
            &workspace_id,
            &target_channel_id,
            &author,
            &target_message_id,
        );
        let reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(target_channel_id.clone()),
            author.clone(),
            EventBody::ReactionAdded {
                message_id: target_message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));
        let read_marker = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(target_channel_id.clone()),
            author.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: target_channel_id.clone(),
                event_id: target_message.event_id.clone(),
            },
        ));
        let unrelated_message = plaintext_message(
            &workspace_id,
            &unrelated_channel_id,
            &bystander,
            &MessageId("msg_unrelated".to_owned()),
        );
        let history = vec![
            root,
            invite_author,
            invite_bystander,
            target_channel,
            unrelated_channel,
            target_message.clone(),
            reaction.clone(),
            read_marker.clone(),
            unrelated_message.clone(),
        ];
        let targets = vec![
            target_message.clone(),
            reaction.clone(),
            read_marker.clone(),
        ];

        let (snapshot, _) =
            trust_snapshot_for_events_from_events(workspace_id, &history, &targets).unwrap();

        assert_eq!(snapshot.roles.len(), 1);
        assert_eq!(snapshot.roles[0].device_id, author);
        assert_eq!(snapshot.channels.len(), 1);
        assert_eq!(snapshot.channels[0].channel_id, target_channel_id);
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].message_id, target_message_id);
        assert_eq!(snapshot.event_channels.len(), 1);
        assert_eq!(snapshot.event_channels[0].event_id, target_message.event_id);
        for target in targets {
            assert!(authorize_event_with_trust_snapshot(&snapshot, &target).is_ok());
        }
        assert!(authorize_event_with_trust_snapshot(&snapshot, &unrelated_message).is_err());
    }

    #[test]
    fn trust_snapshot_rejects_duplicate_or_root_role_entries() {
        let mut snapshot = sample_trust_snapshot();
        snapshot.roles.push(snapshot.roles[0].clone());
        assert_invalid_trust_snapshot(snapshot);

        let mut snapshot = sample_trust_snapshot();
        snapshot.roles.push(TrustSnapshotRole {
            device_id: snapshot.root_author_device_id.clone(),
            role: WorkspaceRole::Guest,
        });
        assert_invalid_trust_snapshot(snapshot);
    }

    #[test]
    fn trust_snapshot_rejects_duplicate_snapshot_indexes() {
        let mut snapshot = sample_trust_snapshot();
        snapshot.channels.push(snapshot.channels[0].clone());
        assert_invalid_trust_snapshot(snapshot);

        let mut snapshot = sample_trust_snapshot();
        snapshot.messages.push(snapshot.messages[0].clone());
        assert_invalid_trust_snapshot(snapshot);

        let mut snapshot = sample_trust_snapshot();
        snapshot
            .event_channels
            .push(snapshot.event_channels[0].clone());
        assert_invalid_trust_snapshot(snapshot);
    }

    #[test]
    fn trust_snapshot_rejects_unknown_references() {
        let mut snapshot = sample_trust_snapshot();
        snapshot.messages[0].channel_id = ChannelId("chn_missing".to_owned());
        assert_invalid_trust_snapshot(snapshot);

        let mut snapshot = sample_trust_snapshot();
        snapshot.event_channels[0].channel_id = ChannelId("chn_missing".to_owned());
        assert_invalid_trust_snapshot(snapshot);
    }

    #[test]
    fn trust_snapshot_rejects_unknown_or_duplicate_channel_members() {
        let mut snapshot = sample_trust_snapshot();
        snapshot.channels[0]
            .member_device_ids
            .push(DeviceId("dev_unknown".to_owned()));
        assert_invalid_trust_snapshot(snapshot);

        let mut snapshot = sample_trust_snapshot();
        let member = snapshot.channels[0].member_device_ids[0].clone();
        snapshot.channels[0].member_device_ids.push(member);
        assert_invalid_trust_snapshot(snapshot);
    }

    #[test]
    fn authorization_history_is_order_independent() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id,
            Some(channel_id),
            member,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "authorized".to_owned(),
                attachments: Vec::new(),
            },
        ));

        assert!(authorize_event_with_history(&[invite, channel, root], &message).is_ok());
    }

    #[test]
    fn authorized_message_requires_existing_channel() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let missing_channel_id = ChannelId::new();
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id,
            Some(missing_channel_id.clone()),
            owner,
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "wrong channel".to_owned(),
                attachments: Vec::new(),
            },
        ));

        assert_eq!(
            authorize_event_with_history(&[root], &message),
            Err(AuthorizationError::ChannelNotFound {
                channel_id: missing_channel_id
            })
        );
    }

    #[test]
    fn message_actions_require_existing_message_and_channel_match() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let other_channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let message_id = MessageId::new();
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let other_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: other_channel_id.clone(),
                name: "other".to_owned(),
                is_private: false,
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "target".to_owned(),
                attachments: Vec::new(),
            },
        ));
        let missing_target_reaction = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReactionAdded {
                message_id: MessageId("msg_missing".to_owned()),
                reaction: "+1".to_owned(),
            },
        ));
        let wrong_channel_reaction = signed(SignableEvent::new(
            workspace_id,
            Some(other_channel_id.clone()),
            owner,
            EventBody::ReactionAdded {
                message_id: message_id.clone(),
                reaction: "+1".to_owned(),
            },
        ));

        assert_eq!(
            authorize_event_with_history(
                &[root.clone(), channel.clone()],
                &missing_target_reaction
            ),
            Err(AuthorizationError::MessageNotFound {
                message_id: MessageId("msg_missing".to_owned())
            })
        );
        assert_eq!(
            authorize_event_with_history(
                &[root, channel, other_channel, message],
                &wrong_channel_reaction
            ),
            Err(AuthorizationError::ChannelMismatch {
                expected: channel_id,
                actual: other_channel_id,
            })
        );
    }

    #[test]
    fn read_marker_requires_existing_channel_and_target_event() {
        let workspace_id = WorkspaceId::new();
        let channel_id = ChannelId::new();
        let other_channel_id = ChannelId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: channel_id.clone(),
                name: "general".to_owned(),
                is_private: false,
            },
        ));
        let other_channel = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::ChannelCreated {
                channel_id: other_channel_id.clone(),
                name: "other".to_owned(),
                is_private: false,
            },
        ));
        let missing_target_marker = signed(SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            owner.clone(),
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: EventId("evt_missing".to_owned()),
            },
        ));
        let wrong_target_marker = signed(SignableEvent::new(
            workspace_id,
            Some(channel_id.clone()),
            owner,
            EventBody::ReadMarkerUpdated {
                channel_id: channel_id.clone(),
                event_id: other_channel.event_id.clone(),
            },
        ));

        assert_eq!(
            authorize_event_with_history(&[root.clone(), channel.clone()], &missing_target_marker),
            Err(AuthorizationError::ReadMarkerTargetNotFound {
                event_id: EventId("evt_missing".to_owned())
            })
        );
        assert_eq!(
            authorize_event_with_history(&[root, channel, other_channel], &wrong_target_marker),
            Err(AuthorizationError::ChannelMismatch {
                expected: channel_id,
                actual: other_channel_id,
            })
        );
    }

    #[test]
    fn uninvited_device_cannot_message() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let outsider = DeviceId("dev_outsider".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let message = signed(SignableEvent::new(
            workspace_id,
            Some(ChannelId::new()),
            outsider.clone(),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "unauthorized".to_owned(),
                attachments: Vec::new(),
            },
        ));

        assert_eq!(
            authorize_event_with_history(&[root], &message),
            Err(AuthorizationError::NotAMember {
                device_id: outsider
            })
        );
    }

    #[test]
    fn admin_can_invite_member_but_member_cannot_invite() {
        let workspace_id = WorkspaceId::new();
        let owner = DeviceId("dev_owner".to_owned());
        let admin = DeviceId("dev_admin".to_owned());
        let member = DeviceId("dev_member".to_owned());
        let guest = DeviceId("dev_guest".to_owned());
        let root = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner.clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let admin_invite = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            owner,
            EventBody::MemberInvited {
                invitee_device_id: admin.clone(),
                role: WorkspaceRole::Admin,
            },
        ));
        let member_invite_by_admin = signed(SignableEvent::new(
            workspace_id.clone(),
            None,
            admin,
            EventBody::MemberInvited {
                invitee_device_id: member.clone(),
                role: WorkspaceRole::Member,
            },
        ));
        let guest_invite_by_member = signed(SignableEvent::new(
            workspace_id,
            None,
            member,
            EventBody::MemberInvited {
                invitee_device_id: guest,
                role: WorkspaceRole::Guest,
            },
        ));

        assert!(
            authorize_event_with_history(
                &[root.clone(), admin_invite.clone()],
                &member_invite_by_admin
            )
            .is_ok()
        );
        assert_eq!(
            authorize_event_with_history(
                &[root, admin_invite, member_invite_by_admin],
                &guest_invite_by_member
            ),
            Err(AuthorizationError::InsufficientRole {
                role: WorkspaceRole::Member,
                action: "invite_member"
            })
        );
    }
}
