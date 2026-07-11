use chaft_types::{
    AttachmentRef, ChannelId, ContentKeyScope, DeviceId, DeviceKeyPackageId, EncryptedBlobRef,
    EventBody, EventId, HybridTimestamp, MessageId, PayloadEncryption, PersonId,
    ReplicaStorageClass, SealedPayload, SignableEvent, SignedEvent, SignedTrustSnapshot,
    TrustSnapshot, TrustSnapshotChannel, TrustSnapshotEventChannel, TrustSnapshotMessage,
    TrustSnapshotPersonDeviceLink, TrustSnapshotRole, WorkspaceAccessPolicy, WorkspaceId,
    WorkspaceInviteResolution, WorkspaceJoinRequestResolution, WorkspaceRole,
};
use prost::{Enumeration, Message, Oneof};
use thiserror::Error;

pub const SYNC_FRAME_MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum WireError {
    #[error("protobuf decode failed")]
    Decode(#[from] prost::DecodeError),
    #[error("sync frame length {len} exceeds max {max}")]
    SyncFrameTooLarge { len: usize, max: usize },
    #[error("event body decode failed")]
    EventBody(#[from] serde_json::Error),
    #[error("event body protobuf kind missing")]
    EventBodyKindMissing,
    #[error("content key scope missing")]
    ContentKeyScopeMissing,
    #[error("unknown payload encryption mode {0}")]
    PayloadEncryption(i32),
    #[error("unknown workspace role {0}")]
    WorkspaceRole(i32),
    #[error("unknown workspace access policy {0}")]
    WorkspaceAccessPolicy(i32),
    #[error("unknown workspace invite resolution {0}")]
    WorkspaceInviteResolution(i32),
    #[error("unknown workspace join request resolution {0}")]
    WorkspaceJoinRequestResolution(i32),
    #[error("unknown replica storage class {0}")]
    ReplicaStorageClass(String),
    #[error("event id mismatch: expected {expected}, recomputed {actual}")]
    EventIdMismatch { expected: String, actual: String },
    #[error("trust snapshot protobuf field missing: {0}")]
    TrustSnapshotFieldMissing(&'static str),
    #[error("trust snapshot decode failed")]
    TrustSnapshot(#[source] serde_json::Error),
}

#[derive(Clone, PartialEq, Message)]
pub struct WireEventEnvelope {
    #[prost(string, tag = "1")]
    pub event_id: String,
    #[prost(string, tag = "2")]
    pub workspace_id: String,
    #[prost(string, optional, tag = "3")]
    pub channel_id: Option<String>,
    #[prost(string, tag = "4")]
    pub author_device_id: String,
    #[prost(int64, tag = "5")]
    pub physical_ms: i64,
    #[prost(uint32, tag = "6")]
    pub logical: u32,
    #[prost(bytes, repeated, tag = "7")]
    pub parent_ids: Vec<Vec<u8>>,
    #[prost(bytes, tag = "8")]
    pub body_json: Vec<u8>,
    #[prost(bytes, tag = "9")]
    pub signature: Vec<u8>,
    #[prost(bytes, tag = "10")]
    pub author_public_key: Vec<u8>,
    #[prost(message, optional, tag = "11")]
    pub body: Option<WireEventBody>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WireWorkspaceRole {
    Unspecified = 0,
    Owner = 1,
    Admin = 2,
    Member = 3,
    Guest = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WireWorkspaceAccessPolicy {
    Unspecified = 0,
    InviteOnly = 1,
    RequestAccess = 2,
    Discoverable = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WireWorkspaceJoinRequestResolution {
    Unspecified = 0,
    Approved = 1,
    Declined = 2,
    Revoked = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WireWorkspaceInviteResolution {
    Unspecified = 0,
    Revoked = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WirePayloadEncryption {
    Unspecified = 0,
    DevelopmentPlaintext = 1,
    Aes256GcmSiv = 2,
    OpenMlsPending = 3,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceScope {}

#[derive(Clone, PartialEq, Message)]
pub struct WireContentKeyScope {
    #[prost(oneof = "wire_content_key_scope::Kind", tags = "1, 2")]
    pub kind: Option<wire_content_key_scope::Kind>,
}

pub mod wire_content_key_scope {
    use super::*;

    #[derive(Clone, PartialEq, Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        Workspace(WireWorkspaceScope),
        #[prost(string, tag = "2")]
        ChannelId(String),
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct WireEncryptedBlobRef {
    #[prost(enumeration = "WirePayloadEncryption", tag = "1")]
    pub mode: i32,
    #[prost(string, tag = "2")]
    pub key_id: String,
    #[prost(bytes, tag = "3")]
    pub nonce: Vec<u8>,
    #[prost(bytes, tag = "4")]
    pub aad: Vec<u8>,
    #[prost(uint64, tag = "5")]
    pub plaintext_byte_len: u64,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireSealedPayload {
    #[prost(enumeration = "WirePayloadEncryption", tag = "1")]
    pub mode: i32,
    #[prost(string, tag = "2")]
    pub key_id: String,
    #[prost(bytes, tag = "3")]
    pub nonce: Vec<u8>,
    #[prost(bytes, tag = "4")]
    pub aad: Vec<u8>,
    #[prost(bytes, tag = "5")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireAttachmentRef {
    #[prost(string, tag = "1")]
    pub blob_hash: String,
    #[prost(string, tag = "2")]
    pub media_type: String,
    #[prost(uint64, tag = "3")]
    pub byte_len: u64,
    #[prost(string, tag = "4")]
    pub display_name: String,
    #[prost(message, optional, tag = "5")]
    pub encryption: Option<WireEncryptedBlobRef>,
    #[prost(string, tag = "6")]
    pub attachment_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceCreated {
    #[prost(string, tag = "1")]
    pub name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMemberInvited {
    #[prost(string, tag = "1")]
    pub invitee_device_id: String,
    #[prost(enumeration = "WireWorkspaceRole", tag = "2")]
    pub role: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMemberRoleUpdated {
    #[prost(string, tag = "1")]
    pub member_device_id: String,
    #[prost(enumeration = "WireWorkspaceRole", tag = "2")]
    pub role: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceAccessPolicyUpdated {
    #[prost(enumeration = "WireWorkspaceAccessPolicy", tag = "1")]
    pub policy: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceJoinRequestRecorded {
    #[prost(string, tag = "1")]
    pub request_id: String,
    #[prost(string, tag = "2")]
    pub requester_device_id: String,
    #[prost(string, tag = "3")]
    pub display_name: String,
    #[prost(string, tag = "4")]
    pub note: String,
    #[prost(string, tag = "5")]
    pub source_type: String,
    #[prost(string, tag = "6")]
    pub source_invite_id: String,
    #[prost(string, tag = "7")]
    pub source_display_name: String,
    #[prost(string, tag = "8")]
    pub source_approval_policy: String,
    #[prost(string, tag = "9")]
    pub response_peer_endpoint: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceInviteRecorded {
    #[prost(string, tag = "1")]
    pub invite_id: String,
    #[prost(string, tag = "2")]
    pub invitee_device_id: String,
    #[prost(string, tag = "3")]
    pub display_name: String,
    #[prost(enumeration = "WireWorkspaceRole", tag = "4")]
    pub role: i32,
    #[prost(string, optional, tag = "5")]
    pub request_id: Option<String>,
    #[prost(string, tag = "6")]
    pub expires_at: String,
    #[prost(string, tag = "7")]
    pub approval_policy: String,
    #[prost(string, tag = "8")]
    pub sync_expectation: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceInviteResolved {
    #[prost(string, tag = "1")]
    pub invite_id: String,
    #[prost(enumeration = "WireWorkspaceInviteResolution", tag = "2")]
    pub resolution: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceInviteCapabilityCreated {
    #[prost(string, tag = "1")]
    pub invite_id: String,
    #[prost(string, tag = "2")]
    pub display_name: String,
    #[prost(enumeration = "WireWorkspaceRole", tag = "3")]
    pub role: i32,
    #[prost(string, tag = "4")]
    pub expires_at: String,
    #[prost(string, tag = "5")]
    pub capability_public_key: String,
    #[prost(string, tag = "6")]
    pub sync_expectation: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceInviteClaimed {
    #[prost(string, tag = "1")]
    pub invite_id: String,
    #[prost(string, tag = "2")]
    pub invitee_device_id: String,
    #[prost(string, tag = "3")]
    pub request_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireWorkspaceJoinRequestResolved {
    #[prost(string, tag = "1")]
    pub request_id: String,
    #[prost(enumeration = "WireWorkspaceJoinRequestResolution", tag = "2")]
    pub resolution: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMemberRemoved {
    #[prost(string, tag = "1")]
    pub removed_device_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireChannelCreated {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(bool, tag = "3")]
    pub is_private: bool,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireDirectMessageChannelCreated {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, repeated, tag = "3")]
    pub participant_device_ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireChannelDetailsUpdated {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub topic: Option<String>,
    #[prost(bool, optional, tag = "4")]
    pub archived: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireChannelMemberChanged {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub member_device_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireDeviceProfileUpdated {
    #[prost(string, tag = "1")]
    pub display_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WirePersonDeviceLinked {
    #[prost(string, tag = "1")]
    pub person_id: String,
    #[prost(string, tag = "2")]
    pub device_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WirePersonProfileUpdated {
    #[prost(string, tag = "1")]
    pub person_id: String,
    #[prost(string, tag = "2")]
    pub display_name: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireDeviceKeyPackagePublished {
    #[prost(string, tag = "1")]
    pub key_package_id: String,
    #[prost(string, tag = "2")]
    pub protocol: String,
    #[prost(bytes, tag = "3")]
    pub key_package: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WirePeerEndpointPublished {
    #[prost(string, tag = "1")]
    pub endpoint_id: String,
    #[prost(string, tag = "2")]
    pub endpoint: String,
    #[prost(string, tag = "3")]
    pub transport: String,
    #[prost(bool, tag = "4")]
    pub is_backup_peer: bool,
    #[prost(int64, optional, tag = "5")]
    pub expires_at_ms: Option<i64>,
    #[prost(string, optional, tag = "6")]
    pub replica_storage_class: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub replica_retention_hint: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsWorkspaceGroupMemberAdded {
    #[prost(string, tag = "1")]
    pub invitee_device_id: String,
    #[prost(string, tag = "2")]
    pub invitee_key_package_id: String,
    #[prost(string, tag = "3")]
    pub invitee_key_package_ref: String,
    #[prost(string, tag = "4")]
    pub protocol: String,
    #[prost(string, tag = "5")]
    pub ciphersuite: String,
    #[prost(string, tag = "6")]
    pub group_id: String,
    #[prost(uint64, tag = "7")]
    pub epoch: u64,
    #[prost(bytes, tag = "8")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "9")]
    pub welcome: Vec<u8>,
    #[prost(bytes, tag = "10")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsWorkspaceGroupMemberRemoved {
    #[prost(string, tag = "1")]
    pub removed_device_id: String,
    #[prost(string, tag = "2")]
    pub protocol: String,
    #[prost(string, tag = "3")]
    pub ciphersuite: String,
    #[prost(string, tag = "4")]
    pub group_id: String,
    #[prost(uint64, tag = "5")]
    pub epoch: u64,
    #[prost(bytes, tag = "6")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "7")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsChannelGroupMemberAdded {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub invitee_device_id: String,
    #[prost(string, tag = "3")]
    pub invitee_key_package_id: String,
    #[prost(string, tag = "4")]
    pub invitee_key_package_ref: String,
    #[prost(string, tag = "5")]
    pub protocol: String,
    #[prost(string, tag = "6")]
    pub ciphersuite: String,
    #[prost(string, tag = "7")]
    pub group_id: String,
    #[prost(uint64, tag = "8")]
    pub epoch: u64,
    #[prost(bytes, tag = "9")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "10")]
    pub welcome: Vec<u8>,
    #[prost(bytes, tag = "11")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsChannelGroupMemberRemoved {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub removed_device_id: String,
    #[prost(string, tag = "3")]
    pub protocol: String,
    #[prost(string, tag = "4")]
    pub ciphersuite: String,
    #[prost(string, tag = "5")]
    pub group_id: String,
    #[prost(uint64, tag = "6")]
    pub epoch: u64,
    #[prost(bytes, tag = "7")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "8")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsWorkspaceGroupSelfUpdated {
    #[prost(string, tag = "1")]
    pub protocol: String,
    #[prost(string, tag = "2")]
    pub ciphersuite: String,
    #[prost(string, tag = "3")]
    pub group_id: String,
    #[prost(uint64, tag = "4")]
    pub epoch: u64,
    #[prost(bytes, tag = "5")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "6")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireOpenMlsChannelGroupSelfUpdated {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub protocol: String,
    #[prost(string, tag = "3")]
    pub ciphersuite: String,
    #[prost(string, tag = "4")]
    pub group_id: String,
    #[prost(uint64, tag = "5")]
    pub epoch: u64,
    #[prost(bytes, tag = "6")]
    pub commit: Vec<u8>,
    #[prost(bytes, tag = "7")]
    pub ratchet_tree: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireContentKeyEpochPublished {
    #[prost(message, optional, tag = "1")]
    pub scope: Option<WireContentKeyScope>,
    #[prost(uint64, tag = "2")]
    pub epoch: u64,
    #[prost(string, tag = "3")]
    pub key_id: String,
    #[prost(string, optional, tag = "4")]
    pub previous_key_id: Option<String>,
    #[prost(string, tag = "5")]
    pub algorithm: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageCreated {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub markdown: String,
    #[prost(message, repeated, tag = "3")]
    pub attachments: Vec<WireAttachmentRef>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageReplyCreated {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub reply_to_message_id: String,
    #[prost(string, tag = "3")]
    pub markdown: String,
    #[prost(message, repeated, tag = "4")]
    pub attachments: Vec<WireAttachmentRef>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageCreatedEncrypted {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(message, optional, tag = "2")]
    pub sealed_markdown: Option<WireSealedPayload>,
    #[prost(message, repeated, tag = "3")]
    pub attachments: Vec<WireAttachmentRef>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageReplyCreatedEncrypted {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub reply_to_message_id: String,
    #[prost(message, optional, tag = "3")]
    pub sealed_markdown: Option<WireSealedPayload>,
    #[prost(message, repeated, tag = "4")]
    pub attachments: Vec<WireAttachmentRef>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageEdited {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub markdown: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageEditedEncrypted {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(message, optional, tag = "2")]
    pub sealed_markdown: Option<WireSealedPayload>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireMessageDeleted {
    #[prost(string, tag = "1")]
    pub message_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireReactionAdded {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub reaction: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireReactionRemoved {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub reaction: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireReadMarkerUpdated {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(string, tag = "2")]
    pub event_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireEventBody {
    #[prost(
        oneof = "wire_event_body::Kind",
        tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38"
    )]
    pub kind: Option<wire_event_body::Kind>,
}

pub mod wire_event_body {
    use super::*;

    #[allow(clippy::large_enum_variant)]
    #[derive(Clone, PartialEq, Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        WorkspaceCreated(WireWorkspaceCreated),
        #[prost(message, tag = "2")]
        MemberInvited(WireMemberInvited),
        #[prost(message, tag = "3")]
        MemberRemoved(WireMemberRemoved),
        #[prost(message, tag = "4")]
        ChannelCreated(WireChannelCreated),
        #[prost(message, tag = "5")]
        ChannelMemberAdded(WireChannelMemberChanged),
        #[prost(message, tag = "6")]
        ChannelMemberRemoved(WireChannelMemberChanged),
        #[prost(message, tag = "7")]
        DeviceProfileUpdated(WireDeviceProfileUpdated),
        #[prost(message, tag = "8")]
        DeviceKeyPackagePublished(WireDeviceKeyPackagePublished),
        #[prost(message, tag = "9")]
        OpenMlsWorkspaceGroupMemberAdded(WireOpenMlsWorkspaceGroupMemberAdded),
        #[prost(message, tag = "10")]
        OpenMlsWorkspaceGroupMemberRemoved(WireOpenMlsWorkspaceGroupMemberRemoved),
        #[prost(message, tag = "11")]
        OpenMlsChannelGroupMemberAdded(WireOpenMlsChannelGroupMemberAdded),
        #[prost(message, tag = "12")]
        OpenMlsChannelGroupMemberRemoved(WireOpenMlsChannelGroupMemberRemoved),
        #[prost(message, tag = "13")]
        OpenMlsWorkspaceGroupSelfUpdated(WireOpenMlsWorkspaceGroupSelfUpdated),
        #[prost(message, tag = "14")]
        OpenMlsChannelGroupSelfUpdated(WireOpenMlsChannelGroupSelfUpdated),
        #[prost(message, tag = "15")]
        ContentKeyEpochPublished(WireContentKeyEpochPublished),
        #[prost(message, tag = "16")]
        MessageCreated(WireMessageCreated),
        #[prost(message, tag = "17")]
        MessageCreatedEncrypted(WireMessageCreatedEncrypted),
        #[prost(message, tag = "18")]
        MessageEdited(WireMessageEdited),
        #[prost(message, tag = "19")]
        MessageEditedEncrypted(WireMessageEditedEncrypted),
        #[prost(message, tag = "20")]
        MessageDeleted(WireMessageDeleted),
        #[prost(message, tag = "21")]
        ReactionAdded(WireReactionAdded),
        #[prost(message, tag = "22")]
        ReadMarkerUpdated(WireReadMarkerUpdated),
        #[prost(message, tag = "23")]
        MessageReplyCreated(WireMessageReplyCreated),
        #[prost(message, tag = "24")]
        MessageReplyCreatedEncrypted(WireMessageReplyCreatedEncrypted),
        #[prost(message, tag = "25")]
        ReactionRemoved(WireReactionRemoved),
        #[prost(message, tag = "26")]
        PeerEndpointPublished(WirePeerEndpointPublished),
        #[prost(message, tag = "27")]
        DirectMessageChannelCreated(WireDirectMessageChannelCreated),
        #[prost(message, tag = "28")]
        ChannelDetailsUpdated(WireChannelDetailsUpdated),
        #[prost(message, tag = "29")]
        MemberRoleUpdated(WireMemberRoleUpdated),
        #[prost(message, tag = "30")]
        WorkspaceJoinRequestRecorded(WireWorkspaceJoinRequestRecorded),
        #[prost(message, tag = "31")]
        WorkspaceJoinRequestResolved(WireWorkspaceJoinRequestResolved),
        #[prost(message, tag = "32")]
        WorkspaceInviteRecorded(WireWorkspaceInviteRecorded),
        #[prost(message, tag = "33")]
        WorkspaceInviteResolved(WireWorkspaceInviteResolved),
        #[prost(message, tag = "34")]
        WorkspaceAccessPolicyUpdated(WireWorkspaceAccessPolicyUpdated),
        #[prost(message, tag = "35")]
        PersonDeviceLinked(WirePersonDeviceLinked),
        #[prost(message, tag = "36")]
        PersonProfileUpdated(WirePersonProfileUpdated),
        #[prost(message, tag = "37")]
        WorkspaceInviteCapabilityCreated(WireWorkspaceInviteCapabilityCreated),
        #[prost(message, tag = "38")]
        WorkspaceInviteClaimed(WireWorkspaceInviteClaimed),
    }
}

fn role_to_wire(role: WorkspaceRole) -> i32 {
    match role {
        WorkspaceRole::Owner => WireWorkspaceRole::Owner as i32,
        WorkspaceRole::Admin => WireWorkspaceRole::Admin as i32,
        WorkspaceRole::Member => WireWorkspaceRole::Member as i32,
        WorkspaceRole::Guest => WireWorkspaceRole::Guest as i32,
    }
}

fn role_from_wire(role: i32) -> Result<WorkspaceRole, WireError> {
    match WireWorkspaceRole::try_from(role).map_err(|_| WireError::WorkspaceRole(role))? {
        WireWorkspaceRole::Owner => Ok(WorkspaceRole::Owner),
        WireWorkspaceRole::Admin => Ok(WorkspaceRole::Admin),
        WireWorkspaceRole::Member => Ok(WorkspaceRole::Member),
        WireWorkspaceRole::Guest => Ok(WorkspaceRole::Guest),
        WireWorkspaceRole::Unspecified => Err(WireError::WorkspaceRole(role)),
    }
}

fn access_policy_to_wire(policy: WorkspaceAccessPolicy) -> i32 {
    match policy {
        WorkspaceAccessPolicy::InviteOnly => WireWorkspaceAccessPolicy::InviteOnly as i32,
        WorkspaceAccessPolicy::RequestAccess => WireWorkspaceAccessPolicy::RequestAccess as i32,
        WorkspaceAccessPolicy::Discoverable => WireWorkspaceAccessPolicy::Discoverable as i32,
    }
}

fn access_policy_from_wire(policy: i32) -> Result<WorkspaceAccessPolicy, WireError> {
    match WireWorkspaceAccessPolicy::try_from(policy)
        .map_err(|_| WireError::WorkspaceAccessPolicy(policy))?
    {
        WireWorkspaceAccessPolicy::InviteOnly => Ok(WorkspaceAccessPolicy::InviteOnly),
        WireWorkspaceAccessPolicy::RequestAccess => Ok(WorkspaceAccessPolicy::RequestAccess),
        WireWorkspaceAccessPolicy::Discoverable => Ok(WorkspaceAccessPolicy::Discoverable),
        WireWorkspaceAccessPolicy::Unspecified => Err(WireError::WorkspaceAccessPolicy(policy)),
    }
}

fn invite_resolution_to_wire(resolution: WorkspaceInviteResolution) -> i32 {
    match resolution {
        WorkspaceInviteResolution::Revoked => WireWorkspaceInviteResolution::Revoked as i32,
    }
}

fn invite_resolution_from_wire(resolution: i32) -> Result<WorkspaceInviteResolution, WireError> {
    match WireWorkspaceInviteResolution::try_from(resolution)
        .map_err(|_| WireError::WorkspaceInviteResolution(resolution))?
    {
        WireWorkspaceInviteResolution::Revoked => Ok(WorkspaceInviteResolution::Revoked),
        WireWorkspaceInviteResolution::Unspecified => {
            Err(WireError::WorkspaceInviteResolution(resolution))
        }
    }
}

fn join_request_resolution_to_wire(resolution: WorkspaceJoinRequestResolution) -> i32 {
    match resolution {
        WorkspaceJoinRequestResolution::Approved => {
            WireWorkspaceJoinRequestResolution::Approved as i32
        }
        WorkspaceJoinRequestResolution::Declined => {
            WireWorkspaceJoinRequestResolution::Declined as i32
        }
        WorkspaceJoinRequestResolution::Revoked => {
            WireWorkspaceJoinRequestResolution::Revoked as i32
        }
    }
}

fn join_request_resolution_from_wire(
    resolution: i32,
) -> Result<WorkspaceJoinRequestResolution, WireError> {
    match WireWorkspaceJoinRequestResolution::try_from(resolution)
        .map_err(|_| WireError::WorkspaceJoinRequestResolution(resolution))?
    {
        WireWorkspaceJoinRequestResolution::Approved => {
            Ok(WorkspaceJoinRequestResolution::Approved)
        }
        WireWorkspaceJoinRequestResolution::Declined => {
            Ok(WorkspaceJoinRequestResolution::Declined)
        }
        WireWorkspaceJoinRequestResolution::Revoked => Ok(WorkspaceJoinRequestResolution::Revoked),
        WireWorkspaceJoinRequestResolution::Unspecified => {
            Err(WireError::WorkspaceJoinRequestResolution(resolution))
        }
    }
}

fn encryption_to_wire(mode: &PayloadEncryption) -> i32 {
    match mode {
        PayloadEncryption::DevelopmentPlaintext => {
            WirePayloadEncryption::DevelopmentPlaintext as i32
        }
        PayloadEncryption::Aes256GcmSiv => WirePayloadEncryption::Aes256GcmSiv as i32,
        PayloadEncryption::OpenMlsPending => WirePayloadEncryption::OpenMlsPending as i32,
    }
}

fn encryption_from_wire(mode: i32) -> Result<PayloadEncryption, WireError> {
    match WirePayloadEncryption::try_from(mode).map_err(|_| WireError::PayloadEncryption(mode))? {
        WirePayloadEncryption::DevelopmentPlaintext => Ok(PayloadEncryption::DevelopmentPlaintext),
        WirePayloadEncryption::Aes256GcmSiv => Ok(PayloadEncryption::Aes256GcmSiv),
        WirePayloadEncryption::OpenMlsPending => Ok(PayloadEncryption::OpenMlsPending),
        WirePayloadEncryption::Unspecified => Err(WireError::PayloadEncryption(mode)),
    }
}

fn encode_content_key_scope(scope: &ContentKeyScope) -> WireContentKeyScope {
    let kind = match scope {
        ContentKeyScope::Workspace => {
            wire_content_key_scope::Kind::Workspace(WireWorkspaceScope {})
        }
        ContentKeyScope::Channel { channel_id } => {
            wire_content_key_scope::Kind::ChannelId(channel_id.0.clone())
        }
    };
    WireContentKeyScope { kind: Some(kind) }
}

fn decode_content_key_scope(scope: WireContentKeyScope) -> Result<ContentKeyScope, WireError> {
    match scope.kind.ok_or(WireError::ContentKeyScopeMissing)? {
        wire_content_key_scope::Kind::Workspace(_) => Ok(ContentKeyScope::Workspace),
        wire_content_key_scope::Kind::ChannelId(channel_id) => Ok(ContentKeyScope::Channel {
            channel_id: ChannelId(channel_id),
        }),
    }
}

fn encode_encrypted_blob_ref(encryption: &EncryptedBlobRef) -> WireEncryptedBlobRef {
    WireEncryptedBlobRef {
        mode: encryption_to_wire(&encryption.mode),
        key_id: encryption.key_id.clone(),
        nonce: encryption.nonce.clone(),
        aad: encryption.aad.clone(),
        plaintext_byte_len: encryption.plaintext_byte_len,
    }
}

fn decode_encrypted_blob_ref(
    encryption: WireEncryptedBlobRef,
) -> Result<EncryptedBlobRef, WireError> {
    Ok(EncryptedBlobRef {
        mode: encryption_from_wire(encryption.mode)?,
        key_id: encryption.key_id,
        nonce: encryption.nonce,
        aad: encryption.aad,
        plaintext_byte_len: encryption.plaintext_byte_len,
    })
}

fn encode_sealed_payload(payload: &SealedPayload) -> WireSealedPayload {
    WireSealedPayload {
        mode: encryption_to_wire(&payload.mode),
        key_id: payload.key_id.clone(),
        nonce: payload.nonce.clone(),
        aad: payload.aad.clone(),
        bytes: payload.bytes.clone(),
    }
}

fn decode_sealed_payload(payload: WireSealedPayload) -> Result<SealedPayload, WireError> {
    Ok(SealedPayload {
        mode: encryption_from_wire(payload.mode)?,
        key_id: payload.key_id,
        nonce: payload.nonce,
        aad: payload.aad,
        bytes: payload.bytes,
    })
}

fn encode_attachment(attachment: &AttachmentRef) -> WireAttachmentRef {
    WireAttachmentRef {
        blob_hash: attachment.blob_hash.clone(),
        media_type: attachment.media_type.clone(),
        byte_len: attachment.byte_len,
        display_name: attachment.display_name.clone(),
        encryption: attachment
            .encryption
            .as_ref()
            .map(encode_encrypted_blob_ref),
        attachment_id: attachment.attachment_id.clone(),
    }
}

fn decode_attachment(attachment: WireAttachmentRef) -> Result<AttachmentRef, WireError> {
    Ok(AttachmentRef {
        blob_hash: attachment.blob_hash,
        media_type: attachment.media_type,
        byte_len: attachment.byte_len,
        display_name: attachment.display_name,
        attachment_id: attachment.attachment_id,
        encryption: attachment
            .encryption
            .map(decode_encrypted_blob_ref)
            .transpose()?,
    })
}

fn encode_attachments(attachments: &[AttachmentRef]) -> Vec<WireAttachmentRef> {
    attachments.iter().map(encode_attachment).collect()
}

fn decode_attachments(
    attachments: Vec<WireAttachmentRef>,
) -> Result<Vec<AttachmentRef>, WireError> {
    attachments.into_iter().map(decode_attachment).collect()
}

fn encode_event_body(body: &EventBody) -> WireEventBody {
    use wire_event_body::Kind;

    let kind = match body {
        EventBody::WorkspaceCreated { name } => {
            Kind::WorkspaceCreated(WireWorkspaceCreated { name: name.clone() })
        }
        EventBody::MemberInvited {
            invitee_device_id,
            role,
        } => Kind::MemberInvited(WireMemberInvited {
            invitee_device_id: invitee_device_id.0.clone(),
            role: role_to_wire(*role),
        }),
        EventBody::MemberRoleUpdated {
            member_device_id,
            role,
        } => Kind::MemberRoleUpdated(WireMemberRoleUpdated {
            member_device_id: member_device_id.0.clone(),
            role: role_to_wire(*role),
        }),
        EventBody::WorkspaceAccessPolicyUpdated { policy } => {
            Kind::WorkspaceAccessPolicyUpdated(WireWorkspaceAccessPolicyUpdated {
                policy: access_policy_to_wire(*policy),
            })
        }
        EventBody::WorkspaceInviteRecorded {
            invite_id,
            invitee_device_id,
            display_name,
            role,
            request_id,
            expires_at,
            approval_policy,
            sync_expectation,
        } => Kind::WorkspaceInviteRecorded(WireWorkspaceInviteRecorded {
            invite_id: invite_id.clone(),
            invitee_device_id: invitee_device_id.0.clone(),
            display_name: display_name.clone(),
            role: role_to_wire(*role),
            request_id: request_id.clone(),
            expires_at: expires_at.clone(),
            approval_policy: approval_policy.clone(),
            sync_expectation: sync_expectation.clone(),
        }),
        EventBody::WorkspaceInviteCapabilityCreated {
            invite_id,
            display_name,
            role,
            expires_at,
            capability_public_key,
            sync_expectation,
        } => Kind::WorkspaceInviteCapabilityCreated(WireWorkspaceInviteCapabilityCreated {
            invite_id: invite_id.clone(),
            display_name: display_name.clone(),
            role: role_to_wire(*role),
            expires_at: expires_at.clone(),
            capability_public_key: capability_public_key.clone(),
            sync_expectation: sync_expectation.clone(),
        }),
        EventBody::WorkspaceInviteClaimed {
            invite_id,
            invitee_device_id,
            request_id,
        } => Kind::WorkspaceInviteClaimed(WireWorkspaceInviteClaimed {
            invite_id: invite_id.clone(),
            invitee_device_id: invitee_device_id.0.clone(),
            request_id: request_id.clone(),
        }),
        EventBody::WorkspaceInviteResolved {
            invite_id,
            resolution,
        } => Kind::WorkspaceInviteResolved(WireWorkspaceInviteResolved {
            invite_id: invite_id.clone(),
            resolution: invite_resolution_to_wire(*resolution),
        }),
        EventBody::WorkspaceJoinRequestRecorded {
            request_id,
            requester_device_id,
            display_name,
            note,
            source_type,
            source_invite_id,
            source_display_name,
            source_approval_policy,
            response_peer_endpoint,
        } => Kind::WorkspaceJoinRequestRecorded(WireWorkspaceJoinRequestRecorded {
            request_id: request_id.clone(),
            requester_device_id: requester_device_id.0.clone(),
            display_name: display_name.clone(),
            note: note.clone(),
            source_type: source_type.clone(),
            source_invite_id: source_invite_id.clone(),
            source_display_name: source_display_name.clone(),
            source_approval_policy: source_approval_policy.clone(),
            response_peer_endpoint: response_peer_endpoint.clone(),
        }),
        EventBody::WorkspaceJoinRequestResolved {
            request_id,
            resolution,
        } => Kind::WorkspaceJoinRequestResolved(WireWorkspaceJoinRequestResolved {
            request_id: request_id.clone(),
            resolution: join_request_resolution_to_wire(*resolution),
        }),
        EventBody::MemberRemoved { removed_device_id } => Kind::MemberRemoved(WireMemberRemoved {
            removed_device_id: removed_device_id.0.clone(),
        }),
        EventBody::ChannelCreated {
            channel_id,
            name,
            is_private,
        } => Kind::ChannelCreated(WireChannelCreated {
            channel_id: channel_id.0.clone(),
            name: name.clone(),
            is_private: *is_private,
        }),
        EventBody::DirectMessageChannelCreated {
            channel_id,
            name,
            participant_device_ids,
        } => Kind::DirectMessageChannelCreated(WireDirectMessageChannelCreated {
            channel_id: channel_id.0.clone(),
            name: name.clone(),
            participant_device_ids: participant_device_ids
                .iter()
                .map(|device_id| device_id.0.clone())
                .collect(),
        }),
        EventBody::ChannelDetailsUpdated {
            channel_id,
            name,
            topic,
            archived,
        } => Kind::ChannelDetailsUpdated(WireChannelDetailsUpdated {
            channel_id: channel_id.0.clone(),
            name: name.clone(),
            topic: topic.clone(),
            archived: *archived,
        }),
        EventBody::ChannelMemberAdded {
            channel_id,
            member_device_id,
        } => Kind::ChannelMemberAdded(WireChannelMemberChanged {
            channel_id: channel_id.0.clone(),
            member_device_id: member_device_id.0.clone(),
        }),
        EventBody::ChannelMemberRemoved {
            channel_id,
            member_device_id,
        } => Kind::ChannelMemberRemoved(WireChannelMemberChanged {
            channel_id: channel_id.0.clone(),
            member_device_id: member_device_id.0.clone(),
        }),
        EventBody::DeviceProfileUpdated { display_name } => {
            Kind::DeviceProfileUpdated(WireDeviceProfileUpdated {
                display_name: display_name.clone(),
            })
        }
        EventBody::PersonDeviceLinked {
            person_id,
            device_id,
        } => Kind::PersonDeviceLinked(WirePersonDeviceLinked {
            person_id: person_id.0.clone(),
            device_id: device_id.0.clone(),
        }),
        EventBody::PersonProfileUpdated {
            person_id,
            display_name,
        } => Kind::PersonProfileUpdated(WirePersonProfileUpdated {
            person_id: person_id.0.clone(),
            display_name: display_name.clone(),
        }),
        EventBody::DeviceKeyPackagePublished {
            key_package_id,
            protocol,
            key_package,
        } => Kind::DeviceKeyPackagePublished(WireDeviceKeyPackagePublished {
            key_package_id: key_package_id.0.clone(),
            protocol: protocol.clone(),
            key_package: key_package.clone(),
        }),
        EventBody::PeerEndpointPublished {
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            expires_at_ms,
            replica_storage_class,
            replica_retention_hint,
        } => Kind::PeerEndpointPublished(WirePeerEndpointPublished {
            endpoint_id: endpoint_id.clone(),
            endpoint: endpoint.clone(),
            transport: transport.clone(),
            is_backup_peer: *is_backup_peer,
            expires_at_ms: *expires_at_ms,
            replica_storage_class: replica_storage_class
                .map(|storage_class| storage_class.as_str().to_owned()),
            replica_retention_hint: replica_retention_hint.clone(),
        }),
        EventBody::OpenMlsWorkspaceGroupMemberAdded {
            invitee_device_id,
            invitee_key_package_id,
            invitee_key_package_ref,
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            welcome,
            ratchet_tree,
        } => Kind::OpenMlsWorkspaceGroupMemberAdded(WireOpenMlsWorkspaceGroupMemberAdded {
            invitee_device_id: invitee_device_id.0.clone(),
            invitee_key_package_id: invitee_key_package_id.0.clone(),
            invitee_key_package_ref: invitee_key_package_ref.clone(),
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            welcome: welcome.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::OpenMlsWorkspaceGroupMemberRemoved {
            removed_device_id,
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            ratchet_tree,
        } => Kind::OpenMlsWorkspaceGroupMemberRemoved(WireOpenMlsWorkspaceGroupMemberRemoved {
            removed_device_id: removed_device_id.0.clone(),
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::OpenMlsChannelGroupMemberAdded {
            channel_id,
            invitee_device_id,
            invitee_key_package_id,
            invitee_key_package_ref,
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            welcome,
            ratchet_tree,
        } => Kind::OpenMlsChannelGroupMemberAdded(WireOpenMlsChannelGroupMemberAdded {
            channel_id: channel_id.0.clone(),
            invitee_device_id: invitee_device_id.0.clone(),
            invitee_key_package_id: invitee_key_package_id.0.clone(),
            invitee_key_package_ref: invitee_key_package_ref.clone(),
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            welcome: welcome.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::OpenMlsChannelGroupMemberRemoved {
            channel_id,
            removed_device_id,
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            ratchet_tree,
        } => Kind::OpenMlsChannelGroupMemberRemoved(WireOpenMlsChannelGroupMemberRemoved {
            channel_id: channel_id.0.clone(),
            removed_device_id: removed_device_id.0.clone(),
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::OpenMlsWorkspaceGroupSelfUpdated {
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            ratchet_tree,
        } => Kind::OpenMlsWorkspaceGroupSelfUpdated(WireOpenMlsWorkspaceGroupSelfUpdated {
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::OpenMlsChannelGroupSelfUpdated {
            channel_id,
            protocol,
            ciphersuite,
            group_id,
            epoch,
            commit,
            ratchet_tree,
        } => Kind::OpenMlsChannelGroupSelfUpdated(WireOpenMlsChannelGroupSelfUpdated {
            channel_id: channel_id.0.clone(),
            protocol: protocol.clone(),
            ciphersuite: ciphersuite.clone(),
            group_id: group_id.clone(),
            epoch: *epoch,
            commit: commit.clone(),
            ratchet_tree: ratchet_tree.clone(),
        }),
        EventBody::ContentKeyEpochPublished {
            scope,
            epoch,
            key_id,
            previous_key_id,
            algorithm,
        } => Kind::ContentKeyEpochPublished(WireContentKeyEpochPublished {
            scope: Some(encode_content_key_scope(scope)),
            epoch: *epoch,
            key_id: key_id.clone(),
            previous_key_id: previous_key_id.clone(),
            algorithm: algorithm.clone(),
        }),
        EventBody::MessageCreated {
            message_id,
            markdown,
            attachments,
        } => Kind::MessageCreated(WireMessageCreated {
            message_id: message_id.0.clone(),
            markdown: markdown.clone(),
            attachments: encode_attachments(attachments),
        }),
        EventBody::MessageReplyCreated {
            message_id,
            reply_to_message_id,
            markdown,
            attachments,
        } => Kind::MessageReplyCreated(WireMessageReplyCreated {
            message_id: message_id.0.clone(),
            reply_to_message_id: reply_to_message_id.0.clone(),
            markdown: markdown.clone(),
            attachments: encode_attachments(attachments),
        }),
        EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            attachments,
        } => Kind::MessageCreatedEncrypted(WireMessageCreatedEncrypted {
            message_id: message_id.0.clone(),
            sealed_markdown: Some(encode_sealed_payload(sealed_markdown)),
            attachments: encode_attachments(attachments),
        }),
        EventBody::MessageReplyCreatedEncrypted {
            message_id,
            reply_to_message_id,
            sealed_markdown,
            attachments,
        } => Kind::MessageReplyCreatedEncrypted(WireMessageReplyCreatedEncrypted {
            message_id: message_id.0.clone(),
            reply_to_message_id: reply_to_message_id.0.clone(),
            sealed_markdown: Some(encode_sealed_payload(sealed_markdown)),
            attachments: encode_attachments(attachments),
        }),
        EventBody::MessageEdited {
            message_id,
            markdown,
        } => Kind::MessageEdited(WireMessageEdited {
            message_id: message_id.0.clone(),
            markdown: markdown.clone(),
        }),
        EventBody::MessageEditedEncrypted {
            message_id,
            sealed_markdown,
        } => Kind::MessageEditedEncrypted(WireMessageEditedEncrypted {
            message_id: message_id.0.clone(),
            sealed_markdown: Some(encode_sealed_payload(sealed_markdown)),
        }),
        EventBody::MessageDeleted { message_id } => Kind::MessageDeleted(WireMessageDeleted {
            message_id: message_id.0.clone(),
        }),
        EventBody::ReactionAdded {
            message_id,
            reaction,
        } => Kind::ReactionAdded(WireReactionAdded {
            message_id: message_id.0.clone(),
            reaction: reaction.clone(),
        }),
        EventBody::ReactionRemoved {
            message_id,
            reaction,
        } => Kind::ReactionRemoved(WireReactionRemoved {
            message_id: message_id.0.clone(),
            reaction: reaction.clone(),
        }),
        EventBody::ReadMarkerUpdated {
            channel_id,
            event_id,
        } => Kind::ReadMarkerUpdated(WireReadMarkerUpdated {
            channel_id: channel_id.0.clone(),
            event_id: event_id.0.clone(),
        }),
    };

    WireEventBody { kind: Some(kind) }
}

fn decode_event_body(body: WireEventBody) -> Result<EventBody, WireError> {
    use wire_event_body::Kind;

    match body.kind.ok_or(WireError::EventBodyKindMissing)? {
        Kind::WorkspaceCreated(body) => Ok(EventBody::WorkspaceCreated { name: body.name }),
        Kind::MemberInvited(body) => Ok(EventBody::MemberInvited {
            invitee_device_id: DeviceId(body.invitee_device_id),
            role: role_from_wire(body.role)?,
        }),
        Kind::MemberRoleUpdated(body) => Ok(EventBody::MemberRoleUpdated {
            member_device_id: DeviceId(body.member_device_id),
            role: role_from_wire(body.role)?,
        }),
        Kind::WorkspaceAccessPolicyUpdated(body) => Ok(EventBody::WorkspaceAccessPolicyUpdated {
            policy: access_policy_from_wire(body.policy)?,
        }),
        Kind::WorkspaceInviteRecorded(body) => Ok(EventBody::WorkspaceInviteRecorded {
            invite_id: body.invite_id,
            invitee_device_id: DeviceId(body.invitee_device_id),
            display_name: body.display_name,
            role: role_from_wire(body.role)?,
            request_id: body.request_id,
            expires_at: body.expires_at,
            approval_policy: body.approval_policy,
            sync_expectation: body.sync_expectation,
        }),
        Kind::WorkspaceInviteCapabilityCreated(body) => {
            Ok(EventBody::WorkspaceInviteCapabilityCreated {
                invite_id: body.invite_id,
                display_name: body.display_name,
                role: role_from_wire(body.role)?,
                expires_at: body.expires_at,
                capability_public_key: body.capability_public_key,
                sync_expectation: body.sync_expectation,
            })
        }
        Kind::WorkspaceInviteClaimed(body) => Ok(EventBody::WorkspaceInviteClaimed {
            invite_id: body.invite_id,
            invitee_device_id: DeviceId(body.invitee_device_id),
            request_id: body.request_id,
        }),
        Kind::WorkspaceInviteResolved(body) => Ok(EventBody::WorkspaceInviteResolved {
            invite_id: body.invite_id,
            resolution: invite_resolution_from_wire(body.resolution)?,
        }),
        Kind::WorkspaceJoinRequestRecorded(body) => Ok(EventBody::WorkspaceJoinRequestRecorded {
            request_id: body.request_id,
            requester_device_id: DeviceId(body.requester_device_id),
            display_name: body.display_name,
            note: body.note,
            source_type: body.source_type,
            source_invite_id: body.source_invite_id,
            source_display_name: body.source_display_name,
            source_approval_policy: body.source_approval_policy,
            response_peer_endpoint: body.response_peer_endpoint,
        }),
        Kind::WorkspaceJoinRequestResolved(body) => Ok(EventBody::WorkspaceJoinRequestResolved {
            request_id: body.request_id,
            resolution: join_request_resolution_from_wire(body.resolution)?,
        }),
        Kind::MemberRemoved(body) => Ok(EventBody::MemberRemoved {
            removed_device_id: DeviceId(body.removed_device_id),
        }),
        Kind::ChannelCreated(body) => Ok(EventBody::ChannelCreated {
            channel_id: ChannelId(body.channel_id),
            name: body.name,
            is_private: body.is_private,
        }),
        Kind::DirectMessageChannelCreated(body) => Ok(EventBody::DirectMessageChannelCreated {
            channel_id: ChannelId(body.channel_id),
            name: body.name,
            participant_device_ids: body
                .participant_device_ids
                .into_iter()
                .map(DeviceId)
                .collect(),
        }),
        Kind::ChannelDetailsUpdated(body) => Ok(EventBody::ChannelDetailsUpdated {
            channel_id: ChannelId(body.channel_id),
            name: body.name,
            topic: body.topic,
            archived: body.archived,
        }),
        Kind::ChannelMemberAdded(body) => Ok(EventBody::ChannelMemberAdded {
            channel_id: ChannelId(body.channel_id),
            member_device_id: DeviceId(body.member_device_id),
        }),
        Kind::ChannelMemberRemoved(body) => Ok(EventBody::ChannelMemberRemoved {
            channel_id: ChannelId(body.channel_id),
            member_device_id: DeviceId(body.member_device_id),
        }),
        Kind::DeviceProfileUpdated(body) => Ok(EventBody::DeviceProfileUpdated {
            display_name: body.display_name,
        }),
        Kind::PersonDeviceLinked(body) => Ok(EventBody::PersonDeviceLinked {
            person_id: PersonId(body.person_id),
            device_id: DeviceId(body.device_id),
        }),
        Kind::PersonProfileUpdated(body) => Ok(EventBody::PersonProfileUpdated {
            person_id: PersonId(body.person_id),
            display_name: body.display_name,
        }),
        Kind::DeviceKeyPackagePublished(body) => Ok(EventBody::DeviceKeyPackagePublished {
            key_package_id: DeviceKeyPackageId(body.key_package_id),
            protocol: body.protocol,
            key_package: body.key_package,
        }),
        Kind::PeerEndpointPublished(body) => {
            let replica_storage_class = body
                .replica_storage_class
                .filter(|value| !value.trim().is_empty())
                .map(|value| {
                    ReplicaStorageClass::from_wire(&value)
                        .ok_or(WireError::ReplicaStorageClass(value))
                })
                .transpose()?;
            Ok(EventBody::PeerEndpointPublished {
                endpoint_id: body.endpoint_id,
                endpoint: body.endpoint,
                transport: body.transport,
                is_backup_peer: body.is_backup_peer,
                expires_at_ms: body.expires_at_ms,
                replica_storage_class,
                replica_retention_hint: body
                    .replica_retention_hint
                    .filter(|value| !value.trim().is_empty()),
            })
        }
        Kind::OpenMlsWorkspaceGroupMemberAdded(body) => {
            Ok(EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: DeviceId(body.invitee_device_id),
                invitee_key_package_id: DeviceKeyPackageId(body.invitee_key_package_id),
                invitee_key_package_ref: body.invitee_key_package_ref,
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                welcome: body.welcome,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::OpenMlsWorkspaceGroupMemberRemoved(body) => {
            Ok(EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                removed_device_id: DeviceId(body.removed_device_id),
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::OpenMlsChannelGroupMemberAdded(body) => {
            Ok(EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id: ChannelId(body.channel_id),
                invitee_device_id: DeviceId(body.invitee_device_id),
                invitee_key_package_id: DeviceKeyPackageId(body.invitee_key_package_id),
                invitee_key_package_ref: body.invitee_key_package_ref,
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                welcome: body.welcome,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::OpenMlsChannelGroupMemberRemoved(body) => {
            Ok(EventBody::OpenMlsChannelGroupMemberRemoved {
                channel_id: ChannelId(body.channel_id),
                removed_device_id: DeviceId(body.removed_device_id),
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::OpenMlsWorkspaceGroupSelfUpdated(body) => {
            Ok(EventBody::OpenMlsWorkspaceGroupSelfUpdated {
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::OpenMlsChannelGroupSelfUpdated(body) => {
            Ok(EventBody::OpenMlsChannelGroupSelfUpdated {
                channel_id: ChannelId(body.channel_id),
                protocol: body.protocol,
                ciphersuite: body.ciphersuite,
                group_id: body.group_id,
                epoch: body.epoch,
                commit: body.commit,
                ratchet_tree: body.ratchet_tree,
            })
        }
        Kind::ContentKeyEpochPublished(body) => Ok(EventBody::ContentKeyEpochPublished {
            scope: decode_content_key_scope(body.scope.ok_or(WireError::ContentKeyScopeMissing)?)?,
            epoch: body.epoch,
            key_id: body.key_id,
            previous_key_id: body.previous_key_id,
            algorithm: body.algorithm,
        }),
        Kind::MessageCreated(body) => Ok(EventBody::MessageCreated {
            message_id: MessageId(body.message_id),
            markdown: body.markdown,
            attachments: decode_attachments(body.attachments)?,
        }),
        Kind::MessageReplyCreated(body) => Ok(EventBody::MessageReplyCreated {
            message_id: MessageId(body.message_id),
            reply_to_message_id: MessageId(body.reply_to_message_id),
            markdown: body.markdown,
            attachments: decode_attachments(body.attachments)?,
        }),
        Kind::MessageCreatedEncrypted(body) => Ok(EventBody::MessageCreatedEncrypted {
            message_id: MessageId(body.message_id),
            sealed_markdown: decode_sealed_payload(
                body.sealed_markdown
                    .ok_or(WireError::EventBodyKindMissing)?,
            )?,
            attachments: decode_attachments(body.attachments)?,
        }),
        Kind::MessageReplyCreatedEncrypted(body) => Ok(EventBody::MessageReplyCreatedEncrypted {
            message_id: MessageId(body.message_id),
            reply_to_message_id: MessageId(body.reply_to_message_id),
            sealed_markdown: decode_sealed_payload(
                body.sealed_markdown
                    .ok_or(WireError::EventBodyKindMissing)?,
            )?,
            attachments: decode_attachments(body.attachments)?,
        }),
        Kind::MessageEdited(body) => Ok(EventBody::MessageEdited {
            message_id: MessageId(body.message_id),
            markdown: body.markdown,
        }),
        Kind::MessageEditedEncrypted(body) => Ok(EventBody::MessageEditedEncrypted {
            message_id: MessageId(body.message_id),
            sealed_markdown: decode_sealed_payload(
                body.sealed_markdown
                    .ok_or(WireError::EventBodyKindMissing)?,
            )?,
        }),
        Kind::MessageDeleted(body) => Ok(EventBody::MessageDeleted {
            message_id: MessageId(body.message_id),
        }),
        Kind::ReactionAdded(body) => Ok(EventBody::ReactionAdded {
            message_id: MessageId(body.message_id),
            reaction: body.reaction,
        }),
        Kind::ReactionRemoved(body) => Ok(EventBody::ReactionRemoved {
            message_id: MessageId(body.message_id),
            reaction: body.reaction,
        }),
        Kind::ReadMarkerUpdated(body) => Ok(EventBody::ReadMarkerUpdated {
            channel_id: ChannelId(body.channel_id),
            event_id: EventId(body.event_id),
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enumeration)]
#[repr(i32)]
pub enum WireSyncRequestKind {
    Unspecified = 0,
    Inventory = 1,
    FetchEvents = 2,
    PublishEvents = 3,
    PutBlobs = 4,
    FetchBlobs = 5,
    FetchBlobAvailability = 6,
    SubmitJoinRequest = 7,
    SubmitJoinResponse = 8,
    FetchJoinRequests = 9,
    FetchJoinResponses = 10,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireBlobEnvelope {
    #[prost(string, tag = "1")]
    pub hash: String,
    #[prost(bytes, tag = "2")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireBlobDescriptor {
    #[prost(string, tag = "1")]
    pub hash: String,
    #[prost(uint64, tag = "2")]
    pub byte_len: u64,
    #[prost(uint64, tag = "3")]
    pub chunk_size: u64,
    #[prost(string, repeated, tag = "4")]
    pub chunk_hashes: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireBlobAvailability {
    #[prost(string, tag = "1")]
    pub hash: String,
    #[prost(bool, tag = "2")]
    pub has_whole_blob: bool,
    #[prost(message, optional, tag = "3")]
    pub descriptor: Option<WireBlobDescriptor>,
    #[prost(string, repeated, tag = "4")]
    pub available_chunk_hashes: Vec<String>,
    #[prost(string, repeated, tag = "5")]
    pub missing_chunk_hashes: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireSyncRequest {
    #[prost(enumeration = "WireSyncRequestKind", tag = "1")]
    pub kind: i32,
    #[prost(string, repeated, tag = "2")]
    pub event_ids: Vec<String>,
    #[prost(bytes, repeated, tag = "3")]
    pub events: Vec<Vec<u8>>,
    #[prost(bytes, repeated, tag = "4")]
    pub authorization_events: Vec<Vec<u8>>,
    #[prost(bytes, repeated, tag = "9")]
    pub authorization_snapshots: Vec<Vec<u8>>,
    #[prost(string, repeated, tag = "5")]
    pub blob_hashes: Vec<String>,
    #[prost(message, repeated, tag = "6")]
    pub blobs: Vec<WireBlobEnvelope>,
    #[prost(message, repeated, tag = "7")]
    pub blob_descriptors: Vec<WireBlobDescriptor>,
    #[prost(string, optional, tag = "8")]
    pub workspace_id: Option<String>,
    #[prost(message, repeated, tag = "10")]
    pub event_envelopes: Vec<WireEventEnvelope>,
    #[prost(message, repeated, tag = "11")]
    pub authorization_event_envelopes: Vec<WireEventEnvelope>,
    #[prost(message, repeated, tag = "12")]
    pub authorization_snapshot_envelopes: Vec<WireSignedTrustSnapshot>,
    #[prost(uint64, optional, tag = "13")]
    pub inventory_start_index: Option<u64>,
    #[prost(uint64, optional, tag = "14")]
    pub inventory_limit: Option<u64>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshotRole {
    #[prost(string, tag = "1")]
    pub device_id: String,
    #[prost(enumeration = "WireWorkspaceRole", tag = "2")]
    pub role: i32,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshotChannel {
    #[prost(string, tag = "1")]
    pub channel_id: String,
    #[prost(bool, tag = "2")]
    pub is_private: bool,
    #[prost(string, tag = "3")]
    pub creator_device_id: String,
    #[prost(string, repeated, tag = "4")]
    pub member_device_ids: Vec<String>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshotMessage {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub channel_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshotEventChannel {
    #[prost(string, tag = "1")]
    pub event_id: String,
    #[prost(string, tag = "2")]
    pub channel_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshotPersonDeviceLink {
    #[prost(string, tag = "1")]
    pub person_id: String,
    #[prost(string, tag = "2")]
    pub device_id: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireTrustSnapshot {
    #[prost(uint32, tag = "1")]
    pub schema_version: u32,
    #[prost(string, tag = "2")]
    pub workspace_id: String,
    #[prost(string, tag = "3")]
    pub root_event_id: String,
    #[prost(string, tag = "4")]
    pub root_author_device_id: String,
    #[prost(message, repeated, tag = "5")]
    pub roles: Vec<WireTrustSnapshotRole>,
    #[prost(message, repeated, tag = "6")]
    pub channels: Vec<WireTrustSnapshotChannel>,
    #[prost(message, repeated, tag = "7")]
    pub messages: Vec<WireTrustSnapshotMessage>,
    #[prost(message, repeated, tag = "8")]
    pub event_channels: Vec<WireTrustSnapshotEventChannel>,
    #[prost(message, repeated, tag = "9")]
    pub person_device_links: Vec<WireTrustSnapshotPersonDeviceLink>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireSignedTrustSnapshot {
    #[prost(message, optional, tag = "1")]
    pub snapshot: Option<WireTrustSnapshot>,
    #[prost(message, optional, tag = "2")]
    pub root_event: Option<WireEventEnvelope>,
    #[prost(bytes, tag = "3")]
    pub author_public_key: Vec<u8>,
    #[prost(bytes, tag = "4")]
    pub signature: Vec<u8>,
}

#[derive(Clone, PartialEq, Message)]
pub struct WireSyncResponse {
    #[prost(string, repeated, tag = "1")]
    pub event_ids: Vec<String>,
    #[prost(bytes, repeated, tag = "2")]
    pub events: Vec<Vec<u8>>,
    #[prost(string, optional, tag = "3")]
    pub error: Option<String>,
    #[prost(message, repeated, tag = "4")]
    pub blobs: Vec<WireBlobEnvelope>,
    #[prost(message, repeated, tag = "5")]
    pub blob_descriptors: Vec<WireBlobDescriptor>,
    #[prost(message, repeated, tag = "6")]
    pub blob_availability: Vec<WireBlobAvailability>,
    #[prost(message, repeated, tag = "7")]
    pub event_envelopes: Vec<WireEventEnvelope>,
    #[prost(uint64, optional, tag = "8")]
    pub inventory_total_count: Option<u64>,
}

pub fn encode_event_envelope(event: &SignedEvent) -> WireEventEnvelope {
    WireEventEnvelope {
        event_id: event.event_id.0.clone(),
        workspace_id: event.event.workspace_id.0.clone(),
        channel_id: event.event.channel_id.as_ref().map(|id| id.0.clone()),
        author_device_id: event.event.author_device_id.0.clone(),
        physical_ms: event.event.timestamp.physical_ms,
        logical: event.event.timestamp.logical,
        parent_ids: event
            .event
            .parents
            .iter()
            .map(|id| id.0.as_bytes().to_vec())
            .collect(),
        body_json: Vec::new(),
        signature: event.signature.clone(),
        author_public_key: event.author_public_key.clone(),
        body: Some(encode_event_body(&event.event.body)),
    }
}

pub fn encode_event(event: &SignedEvent) -> Vec<u8> {
    encode_event_envelope(event).encode_to_vec()
}

pub fn decode_wire_envelope(bytes: &[u8]) -> Result<WireEventEnvelope, WireError> {
    Ok(WireEventEnvelope::decode(bytes)?)
}

pub fn decode_event_envelope(wire: WireEventEnvelope) -> Result<SignedEvent, WireError> {
    let body = match wire.body {
        Some(body) => decode_event_body(body)?,
        None => serde_json::from_slice::<EventBody>(&wire.body_json)?,
    };
    let event = SignableEvent {
        schema_version: 1,
        workspace_id: WorkspaceId(wire.workspace_id),
        channel_id: wire.channel_id.map(ChannelId),
        author_device_id: DeviceId(wire.author_device_id),
        timestamp: HybridTimestamp {
            physical_ms: wire.physical_ms,
            logical: wire.logical,
        },
        parents: wire
            .parent_ids
            .into_iter()
            .map(|id| EventId(String::from_utf8_lossy(&id).into_owned()))
            .collect(),
        body,
    };
    let recomputed =
        SignedEvent::from_author_signature(event, wire.author_public_key, wire.signature);
    if recomputed.event_id.0 != wire.event_id {
        return Err(WireError::EventIdMismatch {
            expected: wire.event_id,
            actual: recomputed.event_id.0,
        });
    }

    Ok(recomputed)
}

pub fn decode_event(bytes: &[u8]) -> Result<SignedEvent, WireError> {
    decode_event_envelope(decode_wire_envelope(bytes)?)
}

fn encode_trust_snapshot_role(role: &TrustSnapshotRole) -> WireTrustSnapshotRole {
    WireTrustSnapshotRole {
        device_id: role.device_id.0.clone(),
        role: role_to_wire(role.role),
    }
}

fn decode_trust_snapshot_role(role: WireTrustSnapshotRole) -> Result<TrustSnapshotRole, WireError> {
    Ok(TrustSnapshotRole {
        device_id: DeviceId(role.device_id),
        role: role_from_wire(role.role)?,
    })
}

fn encode_trust_snapshot_channel(channel: &TrustSnapshotChannel) -> WireTrustSnapshotChannel {
    WireTrustSnapshotChannel {
        channel_id: channel.channel_id.0.clone(),
        is_private: channel.is_private,
        creator_device_id: channel.creator_device_id.0.clone(),
        member_device_ids: channel
            .member_device_ids
            .iter()
            .map(|device_id| device_id.0.clone())
            .collect(),
    }
}

fn decode_trust_snapshot_channel(channel: WireTrustSnapshotChannel) -> TrustSnapshotChannel {
    TrustSnapshotChannel {
        channel_id: ChannelId(channel.channel_id),
        is_private: channel.is_private,
        creator_device_id: DeviceId(channel.creator_device_id),
        member_device_ids: channel
            .member_device_ids
            .into_iter()
            .map(DeviceId)
            .collect(),
    }
}

fn encode_trust_snapshot_message(message: &TrustSnapshotMessage) -> WireTrustSnapshotMessage {
    WireTrustSnapshotMessage {
        message_id: message.message_id.0.clone(),
        channel_id: message.channel_id.0.clone(),
    }
}

fn decode_trust_snapshot_message(message: WireTrustSnapshotMessage) -> TrustSnapshotMessage {
    TrustSnapshotMessage {
        message_id: MessageId(message.message_id),
        channel_id: ChannelId(message.channel_id),
    }
}

fn encode_trust_snapshot_event_channel(
    event_channel: &TrustSnapshotEventChannel,
) -> WireTrustSnapshotEventChannel {
    WireTrustSnapshotEventChannel {
        event_id: event_channel.event_id.0.clone(),
        channel_id: event_channel.channel_id.0.clone(),
    }
}

fn decode_trust_snapshot_event_channel(
    event_channel: WireTrustSnapshotEventChannel,
) -> TrustSnapshotEventChannel {
    TrustSnapshotEventChannel {
        event_id: EventId(event_channel.event_id),
        channel_id: ChannelId(event_channel.channel_id),
    }
}

fn encode_trust_snapshot_person_device_link(
    link: &TrustSnapshotPersonDeviceLink,
) -> WireTrustSnapshotPersonDeviceLink {
    WireTrustSnapshotPersonDeviceLink {
        person_id: link.person_id.0.clone(),
        device_id: link.device_id.0.clone(),
    }
}

fn decode_trust_snapshot_person_device_link(
    link: WireTrustSnapshotPersonDeviceLink,
) -> TrustSnapshotPersonDeviceLink {
    TrustSnapshotPersonDeviceLink {
        person_id: PersonId(link.person_id),
        device_id: DeviceId(link.device_id),
    }
}

fn encode_trust_snapshot_inner(snapshot: &TrustSnapshot) -> WireTrustSnapshot {
    WireTrustSnapshot {
        schema_version: snapshot.schema_version,
        workspace_id: snapshot.workspace_id.0.clone(),
        root_event_id: snapshot.root_event_id.0.clone(),
        root_author_device_id: snapshot.root_author_device_id.0.clone(),
        roles: snapshot
            .roles
            .iter()
            .map(encode_trust_snapshot_role)
            .collect(),
        channels: snapshot
            .channels
            .iter()
            .map(encode_trust_snapshot_channel)
            .collect(),
        messages: snapshot
            .messages
            .iter()
            .map(encode_trust_snapshot_message)
            .collect(),
        event_channels: snapshot
            .event_channels
            .iter()
            .map(encode_trust_snapshot_event_channel)
            .collect(),
        person_device_links: snapshot
            .person_device_links
            .iter()
            .map(encode_trust_snapshot_person_device_link)
            .collect(),
    }
}

fn decode_trust_snapshot_inner(snapshot: WireTrustSnapshot) -> Result<TrustSnapshot, WireError> {
    Ok(TrustSnapshot {
        schema_version: snapshot.schema_version,
        workspace_id: WorkspaceId(snapshot.workspace_id),
        root_event_id: EventId(snapshot.root_event_id),
        root_author_device_id: DeviceId(snapshot.root_author_device_id),
        roles: snapshot
            .roles
            .into_iter()
            .map(decode_trust_snapshot_role)
            .collect::<Result<Vec<_>, _>>()?,
        channels: snapshot
            .channels
            .into_iter()
            .map(decode_trust_snapshot_channel)
            .collect(),
        messages: snapshot
            .messages
            .into_iter()
            .map(decode_trust_snapshot_message)
            .collect(),
        event_channels: snapshot
            .event_channels
            .into_iter()
            .map(decode_trust_snapshot_event_channel)
            .collect(),
        person_device_links: snapshot
            .person_device_links
            .into_iter()
            .map(decode_trust_snapshot_person_device_link)
            .collect(),
    })
}

fn looks_like_json(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'{')
}

pub fn encode_trust_snapshot(snapshot: &SignedTrustSnapshot) -> Vec<u8> {
    encode_trust_snapshot_envelope(snapshot).encode_to_vec()
}

pub fn encode_trust_snapshot_envelope(snapshot: &SignedTrustSnapshot) -> WireSignedTrustSnapshot {
    WireSignedTrustSnapshot {
        snapshot: Some(encode_trust_snapshot_inner(&snapshot.snapshot)),
        root_event: Some(encode_event_envelope(&snapshot.root_event)),
        author_public_key: snapshot.author_public_key.clone(),
        signature: snapshot.signature.clone(),
    }
}

pub fn decode_trust_snapshot_envelope(
    wire: WireSignedTrustSnapshot,
) -> Result<SignedTrustSnapshot, WireError> {
    Ok(SignedTrustSnapshot {
        snapshot: decode_trust_snapshot_inner(
            wire.snapshot
                .ok_or(WireError::TrustSnapshotFieldMissing("snapshot"))?,
        )?,
        root_event: decode_event_envelope(
            wire.root_event
                .ok_or(WireError::TrustSnapshotFieldMissing("root_event"))?,
        )?,
        author_public_key: wire.author_public_key,
        signature: wire.signature,
    })
}

pub fn decode_trust_snapshot(bytes: &[u8]) -> Result<SignedTrustSnapshot, WireError> {
    if looks_like_json(bytes) {
        return serde_json::from_slice(bytes).map_err(WireError::TrustSnapshot);
    }

    decode_trust_snapshot_envelope(WireSignedTrustSnapshot::decode(bytes)?)
}

pub fn encode_sync_request(request: &WireSyncRequest) -> Vec<u8> {
    request.encode_to_vec()
}

pub fn decode_sync_request(bytes: &[u8]) -> Result<WireSyncRequest, WireError> {
    validate_sync_frame_len(bytes.len())?;
    Ok(WireSyncRequest::decode(bytes)?)
}

pub fn encode_sync_response(response: &WireSyncResponse) -> Vec<u8> {
    response.encode_to_vec()
}

pub fn decode_sync_response(bytes: &[u8]) -> Result<WireSyncResponse, WireError> {
    validate_sync_frame_len(bytes.len())?;
    Ok(WireSyncResponse::decode(bytes)?)
}

fn validate_sync_frame_len(len: usize) -> Result<(), WireError> {
    if len <= SYNC_FRAME_MAX_BYTES {
        return Ok(());
    }

    Err(WireError::SyncFrameTooLarge {
        len,
        max: SYNC_FRAME_MAX_BYTES,
    })
}

#[cfg(test)]
mod tests {
    use chaft_types::{
        AttachmentRef, ChannelId, ContentKeyScope, DeviceId, DeviceKeyPackageId, EncryptedBlobRef,
        EventBody, EventId, MessageId, PayloadEncryption, PersonId, SealedPayload, SignableEvent,
        SignedEvent, SignedTrustSnapshot, TrustSnapshot, TrustSnapshotChannel,
        TrustSnapshotEventChannel, TrustSnapshotMessage, TrustSnapshotPersonDeviceLink,
        TrustSnapshotRole, WorkspaceId, WorkspaceRole,
    };
    use prost::Message as _;

    use super::*;

    fn message_created_body(markdown: impl Into<String>) -> EventBody {
        EventBody::MessageCreated {
            message_id: MessageId("msg_wire".to_owned()),
            markdown: markdown.into(),
            attachments: Vec::new(),
        }
    }

    fn signed_with_body(body: EventBody) -> SignedEvent {
        let event = SignableEvent::new(
            WorkspaceId("wrk_wire".to_owned()),
            Some(ChannelId("chn_wire".to_owned())),
            DeviceId("dev_test".to_owned()),
            body,
        );
        SignedEvent::from_signed_bytes(event, vec![1, 2, 3])
    }

    fn sample_signed_trust_snapshot() -> SignedTrustSnapshot {
        let workspace_id = WorkspaceId("wrk_snapshot".to_owned());
        let root_author_device_id = DeviceId("dev_root".to_owned());
        let root_event = SignedEvent::from_author_signature(
            SignableEvent::new(
                workspace_id.clone(),
                None,
                root_author_device_id.clone(),
                EventBody::WorkspaceCreated {
                    name: "Snapshot Workspace".to_owned(),
                },
            ),
            vec![9, 8, 7],
            vec![6, 5, 4],
        );

        SignedTrustSnapshot {
            snapshot: TrustSnapshot {
                schema_version: 1,
                workspace_id,
                root_event_id: root_event.event_id.clone(),
                root_author_device_id,
                roles: vec![
                    TrustSnapshotRole {
                        device_id: DeviceId("dev_admin".to_owned()),
                        role: WorkspaceRole::Admin,
                    },
                    TrustSnapshotRole {
                        device_id: DeviceId("dev_member".to_owned()),
                        role: WorkspaceRole::Member,
                    },
                ],
                channels: vec![TrustSnapshotChannel {
                    channel_id: ChannelId("chn_private".to_owned()),
                    is_private: true,
                    creator_device_id: DeviceId("dev_admin".to_owned()),
                    member_device_ids: vec![
                        DeviceId("dev_admin".to_owned()),
                        DeviceId("dev_member".to_owned()),
                    ],
                }],
                messages: vec![TrustSnapshotMessage {
                    message_id: MessageId("msg_snapshot".to_owned()),
                    channel_id: ChannelId("chn_private".to_owned()),
                }],
                event_channels: vec![TrustSnapshotEventChannel {
                    event_id: EventId("evt_snapshot".to_owned()),
                    channel_id: ChannelId("chn_private".to_owned()),
                }],
                person_device_links: vec![TrustSnapshotPersonDeviceLink {
                    person_id: PersonId("person_member".to_owned()),
                    device_id: DeviceId("dev_member".to_owned()),
                }],
            },
            root_event,
            author_public_key: vec![9, 8, 7],
            signature: vec![3, 2, 1],
        }
    }

    fn sample_encrypted_blob_ref() -> EncryptedBlobRef {
        EncryptedBlobRef {
            mode: PayloadEncryption::Aes256GcmSiv,
            key_id: "key_wire".to_owned(),
            nonce: vec![1, 2, 3],
            aad: vec![4, 5, 6],
            plaintext_byte_len: 128,
        }
    }

    fn sample_sealed_payload() -> SealedPayload {
        SealedPayload {
            mode: PayloadEncryption::Aes256GcmSiv,
            key_id: "key_wire".to_owned(),
            nonce: vec![7, 8, 9],
            aad: vec![10, 11, 12],
            bytes: vec![13, 14, 15],
        }
    }

    fn sample_attachment() -> AttachmentRef {
        AttachmentRef {
            blob_hash: "blob_wire".to_owned(),
            media_type: "text/plain".to_owned(),
            byte_len: 32,
            display_name: "note.txt".to_owned(),
            attachment_id: "att_wire_0".to_owned(),
            encryption: Some(sample_encrypted_blob_ref()),
        }
    }

    fn all_event_body_variants() -> Vec<EventBody> {
        vec![
            EventBody::WorkspaceCreated {
                name: "Wire Workspace".to_owned(),
            },
            EventBody::MemberInvited {
                invitee_device_id: DeviceId("dev_invitee".to_owned()),
                role: WorkspaceRole::Admin,
            },
            EventBody::MemberRoleUpdated {
                member_device_id: DeviceId("dev_invitee".to_owned()),
                role: WorkspaceRole::Member,
            },
            EventBody::WorkspaceAccessPolicyUpdated {
                policy: WorkspaceAccessPolicy::RequestAccess,
            },
            EventBody::WorkspaceJoinRequestRecorded {
                request_id: "req_wire".to_owned(),
                requester_device_id: DeviceId("dev_requester".to_owned()),
                display_name: "Mina Requester".to_owned(),
                note: "Joining the launch room".to_owned(),
                source_type: "approval_invite".to_owned(),
                source_invite_id: "inv_wire_source".to_owned(),
                source_display_name: "Mira Admin".to_owned(),
                source_approval_policy: "admin_required".to_owned(),
                response_peer_endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
            },
            EventBody::WorkspaceJoinRequestResolved {
                request_id: "req_wire".to_owned(),
                resolution: WorkspaceJoinRequestResolution::Approved,
            },
            EventBody::WorkspaceInviteRecorded {
                invite_id: "inv_wire".to_owned(),
                invitee_device_id: DeviceId("dev_invitee".to_owned()),
                display_name: "Mina Invitee".to_owned(),
                role: WorkspaceRole::Member,
                request_id: Some("req_wire".to_owned()),
                expires_at: "2026-07-14T12:00:00Z".to_owned(),
                approval_policy: "preapproved".to_owned(),
                sync_expectation: "auto_fetch_from_invite_source".to_owned(),
            },
            EventBody::WorkspaceInviteResolved {
                invite_id: "inv_wire".to_owned(),
                resolution: WorkspaceInviteResolution::Revoked,
            },
            EventBody::MemberRemoved {
                removed_device_id: DeviceId("dev_removed".to_owned()),
            },
            EventBody::ChannelCreated {
                channel_id: ChannelId("chn_created".to_owned()),
                name: "wire".to_owned(),
                is_private: true,
            },
            EventBody::DirectMessageChannelCreated {
                channel_id: ChannelId("chn_dm".to_owned()),
                name: "Mina".to_owned(),
                participant_device_ids: vec![
                    DeviceId("dev_test".to_owned()),
                    DeviceId("dev_mina".to_owned()),
                ],
            },
            EventBody::ChannelDetailsUpdated {
                channel_id: ChannelId("chn_created".to_owned()),
                name: Some("renamed".to_owned()),
                topic: Some("Launch planning".to_owned()),
                archived: Some(true),
            },
            EventBody::ChannelMemberAdded {
                channel_id: ChannelId("chn_created".to_owned()),
                member_device_id: DeviceId("dev_member".to_owned()),
            },
            EventBody::ChannelMemberRemoved {
                channel_id: ChannelId("chn_created".to_owned()),
                member_device_id: DeviceId("dev_member".to_owned()),
            },
            EventBody::DeviceProfileUpdated {
                display_name: "Wire Tester".to_owned(),
            },
            EventBody::PersonDeviceLinked {
                person_id: PersonId("person_wire".to_owned()),
                device_id: DeviceId("dev_test".to_owned()),
            },
            EventBody::PersonProfileUpdated {
                person_id: PersonId("person_wire".to_owned()),
                display_name: "Wire Person".to_owned(),
            },
            EventBody::DeviceKeyPackagePublished {
                key_package_id: DeviceKeyPackageId("dkp_wire".to_owned()),
                protocol: "openmls".to_owned(),
                key_package: vec![1, 2, 3],
            },
            EventBody::PeerEndpointPublished {
                endpoint_id: "wire-lan".to_owned(),
                endpoint: "direct+tcp://127.0.0.1:7777".to_owned(),
                transport: "direct-tcp".to_owned(),
                is_backup_peer: true,
                expires_at_ms: Some(1_700_000_600_000),
                replica_storage_class: Some(ReplicaStorageClass::FullHistoryWithBlobs),
                replica_retention_hint: Some("30d".to_owned()),
            },
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: DeviceId("dev_invitee".to_owned()),
                invitee_key_package_id: DeviceKeyPackageId("dkp_invitee".to_owned()),
                invitee_key_package_ref: "ref_invitee".to_owned(),
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_workspace".to_owned(),
                epoch: 2,
                commit: vec![1],
                welcome: vec![2],
                ratchet_tree: vec![3],
            },
            EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                removed_device_id: DeviceId("dev_removed".to_owned()),
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_workspace".to_owned(),
                epoch: 3,
                commit: vec![4],
                ratchet_tree: vec![5],
            },
            EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id: ChannelId("chn_created".to_owned()),
                invitee_device_id: DeviceId("dev_invitee".to_owned()),
                invitee_key_package_id: DeviceKeyPackageId("dkp_invitee".to_owned()),
                invitee_key_package_ref: "ref_invitee".to_owned(),
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_channel".to_owned(),
                epoch: 4,
                commit: vec![6],
                welcome: vec![7],
                ratchet_tree: vec![8],
            },
            EventBody::OpenMlsChannelGroupMemberRemoved {
                channel_id: ChannelId("chn_created".to_owned()),
                removed_device_id: DeviceId("dev_removed".to_owned()),
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_channel".to_owned(),
                epoch: 5,
                commit: vec![9],
                ratchet_tree: vec![10],
            },
            EventBody::OpenMlsWorkspaceGroupSelfUpdated {
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_workspace".to_owned(),
                epoch: 6,
                commit: vec![11],
                ratchet_tree: vec![12],
            },
            EventBody::OpenMlsChannelGroupSelfUpdated {
                channel_id: ChannelId("chn_created".to_owned()),
                protocol: "openmls".to_owned(),
                ciphersuite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_owned(),
                group_id: "grp_channel".to_owned(),
                epoch: 7,
                commit: vec![13],
                ratchet_tree: vec![14],
            },
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: 8,
                key_id: "key_current".to_owned(),
                previous_key_id: Some("key_previous".to_owned()),
                algorithm: "AES-256-GCM-SIV".to_owned(),
            },
            EventBody::MessageCreated {
                message_id: MessageId("msg_created".to_owned()),
                markdown: "hello typed wire".to_owned(),
                attachments: vec![sample_attachment()],
            },
            EventBody::MessageReplyCreated {
                message_id: MessageId("msg_reply_created".to_owned()),
                reply_to_message_id: MessageId("msg_created".to_owned()),
                markdown: "reply typed wire".to_owned(),
                attachments: vec![sample_attachment()],
            },
            EventBody::MessageCreatedEncrypted {
                message_id: MessageId("msg_created_encrypted".to_owned()),
                sealed_markdown: sample_sealed_payload(),
                attachments: vec![sample_attachment()],
            },
            EventBody::MessageReplyCreatedEncrypted {
                message_id: MessageId("msg_reply_created_encrypted".to_owned()),
                reply_to_message_id: MessageId("msg_created_encrypted".to_owned()),
                sealed_markdown: sample_sealed_payload(),
                attachments: vec![sample_attachment()],
            },
            EventBody::MessageEdited {
                message_id: MessageId("msg_created".to_owned()),
                markdown: "edited typed wire".to_owned(),
            },
            EventBody::MessageEditedEncrypted {
                message_id: MessageId("msg_created_encrypted".to_owned()),
                sealed_markdown: sample_sealed_payload(),
            },
            EventBody::MessageDeleted {
                message_id: MessageId("msg_deleted".to_owned()),
            },
            EventBody::ReactionAdded {
                message_id: MessageId("msg_created".to_owned()),
                reaction: "ship".to_owned(),
            },
            EventBody::ReactionRemoved {
                message_id: MessageId("msg_created".to_owned()),
                reaction: "ship".to_owned(),
            },
            EventBody::ReadMarkerUpdated {
                channel_id: ChannelId("chn_created".to_owned()),
                event_id: EventId("evt_read".to_owned()),
            },
        ]
    }

    #[test]
    fn encodes_and_decodes_event_envelope() {
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            DeviceId("dev_test".to_owned()),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "wire hello".to_owned(),
                attachments: Vec::new(),
            },
        );
        let signed = SignedEvent::from_signed_bytes(event, vec![1, 2, 3]);

        let decoded = decode_wire_envelope(&encode_event(&signed)).unwrap();

        assert_eq!(decoded.event_id, signed.event_id.0);
        assert_eq!(decoded.signature, vec![1, 2, 3]);
    }

    #[test]
    fn event_encoding_uses_typed_protobuf_body() {
        let signed = signed_with_body(message_created_body("typed wire hello"));

        let decoded = decode_wire_envelope(&encode_event(&signed)).unwrap();

        assert!(decoded.body.is_some());
        assert!(decoded.body_json.is_empty());
    }

    #[test]
    fn legacy_json_body_still_decodes() {
        let signed = signed_with_body(message_created_body("legacy wire hello"));
        let wire = WireEventEnvelope {
            event_id: signed.event_id.0.clone(),
            workspace_id: signed.event.workspace_id.0.clone(),
            channel_id: signed.event.channel_id.as_ref().map(|id| id.0.clone()),
            author_device_id: signed.event.author_device_id.0.clone(),
            physical_ms: signed.event.timestamp.physical_ms,
            logical: signed.event.timestamp.logical,
            parent_ids: signed
                .event
                .parents
                .iter()
                .map(|id| id.0.as_bytes().to_vec())
                .collect(),
            body_json: serde_json::to_vec(&signed.event.body).unwrap(),
            signature: signed.signature.clone(),
            author_public_key: signed.author_public_key.clone(),
            body: None,
        };

        let decoded = decode_event(&wire.encode_to_vec()).unwrap();

        assert_eq!(decoded, signed);
    }

    #[test]
    fn event_round_trip_preserves_signed_event() {
        let event = SignableEvent::new(
            WorkspaceId::new(),
            Some(ChannelId::new()),
            DeviceId("dev_test".to_owned()),
            EventBody::MessageCreated {
                message_id: MessageId::new(),
                markdown: "wire hello".to_owned(),
                attachments: Vec::new(),
            },
        );
        let signed = SignedEvent::from_signed_bytes(event, vec![1, 2, 3]);

        let decoded = decode_event(&encode_event(&signed)).unwrap();

        assert_eq!(decoded, signed);
    }

    #[test]
    fn event_body_round_trips_all_typed_variants() {
        for body in all_event_body_variants() {
            let signed = signed_with_body(body);
            let decoded = decode_event(&encode_event(&signed)).unwrap();

            assert_eq!(decoded, signed);
        }
    }

    #[test]
    fn trust_snapshot_encoding_uses_typed_protobuf_body() {
        let signed = sample_signed_trust_snapshot();
        let encoded = encode_trust_snapshot(&signed);

        let decoded = WireSignedTrustSnapshot::decode(encoded.as_slice()).unwrap();

        assert_ne!(encoded.first(), Some(&b'{'));
        assert!(decoded.snapshot.is_some());
        assert!(decoded.root_event.is_some());
    }

    #[test]
    fn trust_snapshot_round_trip_preserves_signed_snapshot() {
        let signed = sample_signed_trust_snapshot();

        let decoded = decode_trust_snapshot(&encode_trust_snapshot(&signed)).unwrap();

        assert_eq!(decoded, signed);
    }

    #[test]
    fn legacy_json_trust_snapshot_still_decodes() {
        let signed = sample_signed_trust_snapshot();
        let legacy = serde_json::to_vec(&signed).unwrap();

        let decoded = decode_trust_snapshot(&legacy).unwrap();

        assert_eq!(decoded, signed);
    }

    #[test]
    fn sync_request_round_trip_preserves_event_ids() {
        let request = WireSyncRequest {
            kind: WireSyncRequestKind::FetchEvents as i32,
            event_ids: vec!["evt_a".to_owned(), "evt_b".to_owned()],
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            workspace_id: Some("wrk_a".to_owned()),
            event_envelopes: Vec::new(),
            authorization_event_envelopes: Vec::new(),
            authorization_snapshot_envelopes: Vec::new(),
            inventory_start_index: Some(32),
            inventory_limit: Some(128),
        };

        let decoded = decode_sync_request(&encode_sync_request(&request)).unwrap();

        assert_eq!(decoded.event_ids, request.event_ids);
        assert_eq!(decoded.kind, WireSyncRequestKind::FetchEvents as i32);
        assert_eq!(decoded.workspace_id, request.workspace_id);
        assert_eq!(decoded.inventory_start_index, request.inventory_start_index);
        assert_eq!(decoded.inventory_limit, request.inventory_limit);
    }

    #[test]
    fn sync_request_decode_rejects_oversized_payload_before_prost() {
        let oversized = vec![0; SYNC_FRAME_MAX_BYTES + 1];

        let error = decode_sync_request(&oversized).unwrap_err();

        assert!(matches!(
            error,
            WireError::SyncFrameTooLarge {
                len,
                max: SYNC_FRAME_MAX_BYTES
            } if len == SYNC_FRAME_MAX_BYTES + 1
        ));
    }

    #[test]
    fn sync_response_decode_rejects_oversized_payload_before_prost() {
        let oversized = vec![0; SYNC_FRAME_MAX_BYTES + 1];

        let error = decode_sync_response(&oversized).unwrap_err();

        assert!(matches!(
            error,
            WireError::SyncFrameTooLarge {
                len,
                max: SYNC_FRAME_MAX_BYTES
            } if len == SYNC_FRAME_MAX_BYTES + 1
        ));
    }

    #[test]
    fn sync_request_round_trip_preserves_typed_event_envelopes() {
        let signed = signed_with_body(message_created_body("typed sync batch"));
        let snapshot = sample_signed_trust_snapshot();
        let request = WireSyncRequest {
            kind: WireSyncRequestKind::PublishEvents as i32,
            event_ids: Vec::new(),
            events: Vec::new(),
            authorization_events: Vec::new(),
            authorization_snapshots: Vec::new(),
            blob_hashes: Vec::new(),
            blobs: Vec::new(),
            blob_descriptors: Vec::new(),
            workspace_id: None,
            event_envelopes: vec![encode_event_envelope(&signed)],
            authorization_event_envelopes: vec![encode_event_envelope(&snapshot.root_event)],
            authorization_snapshot_envelopes: vec![encode_trust_snapshot_envelope(&snapshot)],
            inventory_start_index: None,
            inventory_limit: None,
        };

        let decoded = decode_sync_request(&encode_sync_request(&request)).unwrap();

        assert!(decoded.events.is_empty());
        assert_eq!(decoded.event_envelopes.len(), 1);
        assert_eq!(
            decode_event_envelope(decoded.event_envelopes.into_iter().next().unwrap()).unwrap(),
            signed
        );
        assert_eq!(decoded.authorization_event_envelopes.len(), 1);
        assert_eq!(decoded.authorization_snapshot_envelopes.len(), 1);
    }
}
