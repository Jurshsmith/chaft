use std::{
    error::Error as StdError,
    fmt::{self, Display},
    net::{Ipv6Addr, SocketAddr},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const WORKSPACE_ID_MAX_BYTES: usize = 128;
pub const CHANNEL_ID_MAX_BYTES: usize = 128;
pub const MESSAGE_ID_MAX_BYTES: usize = 128;
pub const DEVICE_KEY_PACKAGE_ID_MAX_BYTES: usize = 128;
pub const EVENT_ID_MAX_BYTES: usize = 68;
pub const EVENT_ID_PREFIX: &str = "evt_";
pub const EVENT_ID_HASH_HEX_BYTES: usize = 64;
pub const DEVICE_ID_MAX_BYTES: usize = 512;
pub const EVENT_AUTHOR_PUBLIC_KEY_MAX_BYTES: usize = 32;
pub const EVENT_SIGNATURE_MAX_BYTES: usize = 64;
pub const WORKSPACE_NAME_MAX_BYTES: usize = 128;
pub const CHANNEL_NAME_MAX_BYTES: usize = 128;
pub const CHANNEL_TOPIC_MAX_BYTES: usize = 512;
pub const DEVICE_DISPLAY_NAME_MAX_BYTES: usize = 128;
pub const PERSON_ID_MAX_BYTES: usize = 128;
pub const PERSON_ID_PREFIX: &str = "person_";
pub const WORKSPACE_INVITE_ID_MAX_BYTES: usize = 128;
pub const WORKSPACE_INVITE_LABEL_MAX_BYTES: usize = 128;
pub const WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES: usize = 64;
pub const WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES: usize = 32;
pub const WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES: usize = 64;
pub const WORKSPACE_INVITE_CAPABILITY_PUBLIC_KEY_MAX_BYTES: usize = 64;
pub const WORKSPACE_INVITE_MAX_CLAIMS: u32 = 20;
pub const WORKSPACE_ACCESS_POLICY_MAX_BYTES: usize = 32;
pub const WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES: usize = 128;
pub const WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES: usize = 512;
pub const DEVICE_KEY_PACKAGE_PROTOCOL_MAX_BYTES: usize = 128;
pub const PEER_ENDPOINT_ID_MAX_BYTES: usize = 2304;
pub const PEER_ENDPOINT_MAX_BYTES: usize = 2048;
pub const PEER_ENDPOINT_LIST_MAX_ITEMS: usize = 33;
pub const PEER_ENDPOINT_TRANSPORT_MAX_BYTES: usize = 64;
pub const REPLICA_RETENTION_HINT_MAX_BYTES: usize = 128;
pub const IROH_ENDPOINT_ID_HEX_BYTES: usize = 64;
pub const IROH_ENDPOINT_ID_BASE32_BYTES: usize = 52;
pub const REACTION_TEXT_MAX_BYTES: usize = 64;
pub const CONTENT_KEY_ID_MAX_BYTES: usize = 512;
pub const CONTENT_KEY_ALGORITHM_MAX_BYTES: usize = 128;
pub const MESSAGE_MARKDOWN_MAX_BYTES: usize = 64 * 1024;
pub const SEALED_MESSAGE_MARKDOWN_MAX_BYTES: usize = MESSAGE_MARKDOWN_MAX_BYTES + 16;
pub const SEALED_PAYLOAD_KEY_ID_MAX_BYTES: usize = CONTENT_KEY_ID_MAX_BYTES;
pub const SEALED_PAYLOAD_NONCE_MAX_BYTES: usize = 12;
pub const SEALED_PAYLOAD_AAD_MAX_BYTES: usize = 512;
pub const MESSAGE_ATTACHMENT_MAX_COUNT: usize = 16;
pub const ATTACHMENT_BLOB_HASH_MAX_BYTES: usize = 128;
pub const ATTACHMENT_MEDIA_TYPE_MAX_BYTES: usize = 128;
pub const ATTACHMENT_DISPLAY_NAME_MAX_BYTES: usize = 256;
pub const ATTACHMENT_ID_MAX_BYTES: usize = 256;
pub const ATTACHMENT_KEY_ID_MAX_BYTES: usize = 512;
pub const ATTACHMENT_PLAINTEXT_MAX_BYTES: u64 = 128 * 1024 * 1024;
pub const ATTACHMENT_CIPHERTEXT_MAX_BYTES: u64 = ATTACHMENT_PLAINTEXT_MAX_BYTES + 16;
pub const OPENMLS_KEY_PACKAGE_MAX_BYTES: usize = 64 * 1024;
pub const OPENMLS_PRIVATE_KEY_PACKAGE_BUNDLE_MAX_BYTES: usize = 512 * 1024;
pub const OPENMLS_PRIVATE_GROUP_STATE_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const OPENMLS_WELCOME_MAX_BYTES: usize = 1024 * 1024;
pub const OPENMLS_COMMIT_MAX_BYTES: usize = 1024 * 1024;
pub const OPENMLS_RATCHET_TREE_MAX_BYTES: usize = 4 * 1024 * 1024;
pub const OPENMLS_PROTOCOL_MAX_BYTES: usize = 128;
pub const OPENMLS_CIPHERSUITE_MAX_BYTES: usize = 128;
pub const OPENMLS_GROUP_ID_MAX_BYTES: usize = 512;
pub const OPENMLS_KEY_PACKAGE_REF_MAX_BYTES: usize = 256;

pub const fn default_workspace_invite_max_claims() -> u32 {
    1
}

pub const fn normalize_workspace_invite_max_claims(max_claims: u32) -> u32 {
    if max_claims == 0 {
        default_workspace_invite_max_claims()
    } else {
        max_claims
    }
}

pub const fn effective_workspace_invite_max_claims(max_claims: Option<u32>) -> u32 {
    match max_claims {
        Some(max_claims) => normalize_workspace_invite_max_claims(max_claims),
        None => default_workspace_invite_max_claims(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceKeyPackageId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PersonId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdValidationError {
    pub field: &'static str,
    pub actual_bytes: usize,
    pub max_bytes: usize,
}

impl Display for IdValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is too large ({} bytes, max {})",
            self.field, self.actual_bytes, self.max_bytes
        )
    }
}

impl StdError for IdValidationError {}

pub fn validate_id_bytes(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), IdValidationError> {
    let actual_bytes = value.len();
    if actual_bytes > max_bytes {
        return Err(IdValidationError {
            field,
            actual_bytes,
            max_bytes,
        });
    }
    Ok(())
}

pub fn validate_workspace_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("workspace ID", value, WORKSPACE_ID_MAX_BYTES)
}

pub fn validate_channel_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("channel ID", value, CHANNEL_ID_MAX_BYTES)
}

pub fn validate_message_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("message ID", value, MESSAGE_ID_MAX_BYTES)
}

pub fn validate_device_key_package_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes(
        "device key package ID",
        value,
        DEVICE_KEY_PACKAGE_ID_MAX_BYTES,
    )
}

pub fn validate_event_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("event ID", value, EVENT_ID_MAX_BYTES)
}

pub fn validate_device_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("device ID", value, DEVICE_ID_MAX_BYTES)
}

pub fn validate_person_id_str(value: &str) -> Result<(), IdValidationError> {
    validate_id_bytes("person ID", value, PERSON_ID_MAX_BYTES)
}

pub fn validate_workspace_id(value: &WorkspaceId) -> Result<(), IdValidationError> {
    validate_workspace_id_str(&value.0)
}

pub fn validate_channel_id(value: &ChannelId) -> Result<(), IdValidationError> {
    validate_channel_id_str(&value.0)
}

pub fn validate_message_id(value: &MessageId) -> Result<(), IdValidationError> {
    validate_message_id_str(&value.0)
}

pub fn validate_device_key_package_id(value: &DeviceKeyPackageId) -> Result<(), IdValidationError> {
    validate_device_key_package_id_str(&value.0)
}

pub fn validate_event_id(value: &EventId) -> Result<(), IdValidationError> {
    validate_event_id_str(&value.0)
}

pub fn validate_device_id(value: &DeviceId) -> Result<(), IdValidationError> {
    validate_device_id_str(&value.0)
}

pub fn is_canonical_event_id_str(value: &str) -> bool {
    let Some(hash) = value.strip_prefix(EVENT_ID_PREFIX) else {
        return false;
    };

    hash.len() == EVENT_ID_HASH_HEX_BYTES
        && hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedPeerEndpointHintRoute {
    DirectTcp,
    NativeIrohDirect,
    NativeIrohRelay,
    NativeIrohDiscovery,
}

impl SupportedPeerEndpointHintRoute {
    pub fn primary_transport_label(self) -> &'static str {
        match self {
            Self::DirectTcp => "direct-tcp",
            Self::NativeIrohDirect => "iroh-direct",
            Self::NativeIrohRelay => "iroh-relay",
            Self::NativeIrohDiscovery => "iroh-discovery",
        }
    }

    pub fn allows_transport_label(self, transport: &str) -> bool {
        let transport = transport.trim();
        match self {
            Self::DirectTcp => transport == "direct-tcp",
            Self::NativeIrohDirect => matches!(transport, "iroh" | "iroh-direct"),
            Self::NativeIrohRelay => matches!(transport, "iroh" | "iroh-relay"),
            Self::NativeIrohDiscovery => matches!(transport, "iroh" | "iroh-discovery"),
        }
    }
}

pub fn supported_peer_endpoint_hint_route(
    endpoint: &str,
) -> Option<SupportedPeerEndpointHintRoute> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }

    if let Some(address) = endpoint.strip_prefix("direct+tcp://") {
        return supported_direct_tcp_hint_route(address);
    }
    if let Some(address) = endpoint.strip_prefix("tcp://") {
        return supported_direct_tcp_hint_route(address);
    }
    if endpoint.starts_with("iroh://") {
        return supported_native_iroh_hint_route(endpoint);
    }
    if endpoint.contains("://") {
        return None;
    }

    supported_direct_tcp_hint_route(endpoint)
}

pub fn peer_endpoint_hint_is_supported(endpoint: &str) -> bool {
    supported_peer_endpoint_hint_route(endpoint).is_some()
}

pub fn peer_endpoint_hint_transport_is_consistent(endpoint: &str, transport: &str) -> bool {
    supported_peer_endpoint_hint_route(endpoint)
        .is_some_and(|route| route.allows_transport_label(transport))
}

fn supported_direct_tcp_hint_route(address: &str) -> Option<SupportedPeerEndpointHintRoute> {
    if !direct_tcp_peer_endpoint_address_is_valid(address) {
        return None;
    }
    Some(SupportedPeerEndpointHintRoute::DirectTcp)
}

pub fn direct_tcp_peer_endpoint_address_is_valid(address: &str) -> bool {
    direct_tcp_address_is_valid(address, false)
}

pub fn direct_tcp_peer_listen_address_is_valid(address: &str) -> bool {
    direct_tcp_address_is_valid(address, true)
}

fn direct_tcp_address_is_valid(address: &str, allow_zero_port: bool) -> bool {
    let address = address.trim();
    if address.is_empty() || address.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }

    let (host, port, bracketed_ipv6) = if let Some(rest) = address.strip_prefix('[') {
        let Some((host, after_host)) = rest.split_once("]:") else {
            return false;
        };
        (host, after_host, true)
    } else {
        let Some((host, port)) = address.rsplit_once(':') else {
            return false;
        };
        if host.contains(':') {
            return false;
        }
        (host, port, false)
    };

    let host_is_valid = if bracketed_ipv6 {
        host.parse::<Ipv6Addr>().is_ok()
    } else {
        direct_tcp_host_is_valid(host)
    };
    host_is_valid && valid_tcp_port(port, allow_zero_port)
}

fn direct_tcp_host_is_valid(host: &str) -> bool {
    !host.is_empty()
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn valid_tcp_port(port: &str, allow_zero_port: bool) -> bool {
    !port.is_empty()
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port
            .parse::<u16>()
            .is_ok_and(|port| allow_zero_port || port > 0)
}

fn supported_native_iroh_hint_route(endpoint: &str) -> Option<SupportedPeerEndpointHintRoute> {
    let endpoint = endpoint.strip_prefix("iroh://")?;
    let (endpoint_id, query_and_fragment) = endpoint
        .split_once('?')
        .map(|(endpoint_id, query)| (endpoint_id, Some(query)))
        .unwrap_or((endpoint, None));
    if !native_iroh_endpoint_id_syntax_is_valid(endpoint_id) {
        return None;
    }

    let Some(query_and_fragment) = query_and_fragment else {
        return Some(SupportedPeerEndpointHintRoute::NativeIrohDiscovery);
    };

    let query = query_and_fragment
        .split_once('#')
        .map(|(query, _fragment)| query)
        .unwrap_or(query_and_fragment);
    let mut has_direct_addr = false;
    let mut has_relay = false;
    for parameter in query.split('&') {
        let (key, value) = parameter
            .split_once('=')
            .map(|(key, value)| (key.trim(), value.trim()))
            .unwrap_or((parameter.trim(), ""));
        match key {
            "addr" if native_iroh_direct_addr_is_valid(value) => {
                has_direct_addr = true;
            }
            "addr" => return None,
            "relay" if native_iroh_relay_url_is_valid(value) => {
                has_relay = true;
            }
            "relay" => return None,
            _ => return None,
        }
    }

    if has_relay {
        Some(SupportedPeerEndpointHintRoute::NativeIrohRelay)
    } else {
        has_direct_addr.then_some(SupportedPeerEndpointHintRoute::NativeIrohDirect)
    }
}

fn native_iroh_relay_url_is_valid(value: &str) -> bool {
    let Some(authority_and_path) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = authority_and_path.split('/').next().unwrap_or_default();
    !authority.is_empty()
        && !authority.contains('@')
        && !value.bytes().any(|byte| byte.is_ascii_whitespace())
}

fn native_iroh_direct_addr_is_valid(address: &str) -> bool {
    address
        .parse::<SocketAddr>()
        .is_ok_and(|address| address.port() > 0)
}

fn native_iroh_endpoint_id_syntax_is_valid(endpoint_id: &str) -> bool {
    if endpoint_id.len() == IROH_ENDPOINT_ID_HEX_BYTES {
        return endpoint_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
    }
    endpoint_id.len() == IROH_ENDPOINT_ID_BASE32_BYTES
        && endpoint_id
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'2'..=b'7'))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ContentKeyScope {
    Workspace,
    Channel { channel_id: ChannelId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HybridTimestamp {
    pub physical_ms: i64,
    pub logical: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub blob_hash: String,
    pub media_type: String,
    pub byte_len: u64,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub attachment_id: String,
    pub encryption: Option<EncryptedBlobRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedBlobRef {
    pub mode: PayloadEncryption,
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub aad: Vec<u8>,
    pub plaintext_byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadEncryption {
    DevelopmentPlaintext,
    Aes256GcmSiv,
    OpenMlsPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedPayload {
    pub mode: PayloadEncryption,
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub aad: Vec<u8>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicaStorageClass {
    EphemeralPeer,
    MetadataIndex,
    PartialHistory,
    FullHistory,
    FullHistoryWithBlobs,
}

impl ReplicaStorageClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EphemeralPeer => "ephemeral_peer",
            Self::MetadataIndex => "metadata_index",
            Self::PartialHistory => "partial_history",
            Self::FullHistory => "full_history",
            Self::FullHistoryWithBlobs => "full_history_with_blobs",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "ephemeral_peer" => Some(Self::EphemeralPeer),
            "metadata_index" => Some(Self::MetadataIndex),
            "partial_history" => Some(Self::PartialHistory),
            "full_history" => Some(Self::FullHistory),
            "full_history_with_blobs" => Some(Self::FullHistoryWithBlobs),
            _ => None,
        }
    }

    pub fn supported_wire_values() -> &'static [&'static str] {
        &[
            "ephemeral_peer",
            "metadata_index",
            "partial_history",
            "full_history",
            "full_history_with_blobs",
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventBody {
    WorkspaceCreated {
        name: String,
    },
    MemberInvited {
        invitee_device_id: DeviceId,
        role: WorkspaceRole,
    },
    MemberRoleUpdated {
        member_device_id: DeviceId,
        role: WorkspaceRole,
    },
    WorkspaceAccessPolicyUpdated {
        policy: WorkspaceAccessPolicy,
    },
    WorkspaceInviteRecorded {
        invite_id: String,
        invitee_device_id: DeviceId,
        display_name: String,
        role: WorkspaceRole,
        request_id: Option<String>,
        expires_at: String,
        approval_policy: String,
        sync_expectation: String,
    },
    WorkspaceInviteCapabilityCreated {
        invite_id: String,
        /// Inviter-defined label for organizing the invite. The legacy wire field is named
        /// `display_name`; it is not the claimant's member display name.
        display_name: String,
        role: WorkspaceRole,
        expires_at: String,
        capability_public_key: String,
        sync_expectation: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_claims: Option<u32>,
    },
    WorkspaceInviteClaimed {
        invite_id: String,
        invitee_device_id: DeviceId,
        request_id: String,
    },
    WorkspaceInviteResolved {
        invite_id: String,
        resolution: WorkspaceInviteResolution,
    },
    WorkspaceJoinRequestRecorded {
        request_id: String,
        requester_device_id: DeviceId,
        display_name: String,
        note: String,
        #[serde(default)]
        source_type: String,
        #[serde(default)]
        source_invite_id: String,
        #[serde(default)]
        source_display_name: String,
        #[serde(default)]
        source_approval_policy: String,
        #[serde(default)]
        response_peer_endpoint: String,
    },
    WorkspaceJoinRequestResolved {
        request_id: String,
        resolution: WorkspaceJoinRequestResolution,
    },
    MemberRemoved {
        removed_device_id: DeviceId,
    },
    ChannelCreated {
        channel_id: ChannelId,
        name: String,
        is_private: bool,
    },
    DirectMessageChannelCreated {
        channel_id: ChannelId,
        name: String,
        participant_device_ids: Vec<DeviceId>,
    },
    ChannelDetailsUpdated {
        channel_id: ChannelId,
        name: Option<String>,
        topic: Option<String>,
        archived: Option<bool>,
    },
    ChannelMemberAdded {
        channel_id: ChannelId,
        member_device_id: DeviceId,
    },
    ChannelMemberRemoved {
        channel_id: ChannelId,
        member_device_id: DeviceId,
    },
    DeviceProfileUpdated {
        display_name: String,
    },
    PersonDeviceLinked {
        person_id: PersonId,
        device_id: DeviceId,
    },
    PersonProfileUpdated {
        person_id: PersonId,
        display_name: String,
    },
    DeviceKeyPackagePublished {
        key_package_id: DeviceKeyPackageId,
        protocol: String,
        key_package: Vec<u8>,
    },
    PeerEndpointPublished {
        endpoint_id: String,
        endpoint: String,
        transport: String,
        is_backup_peer: bool,
        expires_at_ms: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replica_storage_class: Option<ReplicaStorageClass>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replica_retention_hint: Option<String>,
    },
    OpenMlsWorkspaceGroupMemberAdded {
        invitee_device_id: DeviceId,
        invitee_key_package_id: DeviceKeyPackageId,
        invitee_key_package_ref: String,
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        welcome: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    OpenMlsWorkspaceGroupMemberRemoved {
        removed_device_id: DeviceId,
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    OpenMlsChannelGroupMemberAdded {
        channel_id: ChannelId,
        invitee_device_id: DeviceId,
        invitee_key_package_id: DeviceKeyPackageId,
        invitee_key_package_ref: String,
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        welcome: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    OpenMlsChannelGroupMemberRemoved {
        channel_id: ChannelId,
        removed_device_id: DeviceId,
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    OpenMlsWorkspaceGroupSelfUpdated {
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    OpenMlsChannelGroupSelfUpdated {
        channel_id: ChannelId,
        protocol: String,
        ciphersuite: String,
        group_id: String,
        epoch: u64,
        commit: Vec<u8>,
        ratchet_tree: Vec<u8>,
    },
    ContentKeyEpochPublished {
        scope: ContentKeyScope,
        epoch: u64,
        key_id: String,
        previous_key_id: Option<String>,
        algorithm: String,
    },
    MessageCreated {
        message_id: MessageId,
        markdown: String,
        attachments: Vec<AttachmentRef>,
    },
    MessageReplyCreated {
        message_id: MessageId,
        reply_to_message_id: MessageId,
        markdown: String,
        attachments: Vec<AttachmentRef>,
    },
    MessageCreatedEncrypted {
        message_id: MessageId,
        sealed_markdown: SealedPayload,
        attachments: Vec<AttachmentRef>,
    },
    MessageReplyCreatedEncrypted {
        message_id: MessageId,
        reply_to_message_id: MessageId,
        sealed_markdown: SealedPayload,
        attachments: Vec<AttachmentRef>,
    },
    MessageEdited {
        message_id: MessageId,
        markdown: String,
    },
    MessageEditedEncrypted {
        message_id: MessageId,
        sealed_markdown: SealedPayload,
    },
    MessageDeleted {
        message_id: MessageId,
    },
    ReactionAdded {
        message_id: MessageId,
        reaction: String,
    },
    ReactionRemoved {
        message_id: MessageId,
        reaction: String,
    },
    ReadMarkerUpdated {
        channel_id: ChannelId,
        event_id: EventId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Owner,
    Admin,
    Member,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceAccessPolicy {
    InviteOnly,
    RequestAccess,
    Discoverable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceInviteResolution {
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceJoinRequestResolution {
    Approved,
    Declined,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignableEvent {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub channel_id: Option<ChannelId>,
    pub author_device_id: DeviceId,
    pub timestamp: HybridTimestamp,
    pub parents: Vec<EventId>,
    pub body: EventBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedEvent {
    pub event_id: EventId,
    pub event: SignableEvent,
    pub author_public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshotRole {
    pub device_id: DeviceId,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshotChannel {
    pub channel_id: ChannelId,
    pub is_private: bool,
    pub creator_device_id: DeviceId,
    pub member_device_ids: Vec<DeviceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshotMessage {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshotEventChannel {
    pub event_id: EventId,
    pub channel_id: ChannelId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshotPersonDeviceLink {
    pub person_id: PersonId,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustSnapshot {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub root_event_id: EventId,
    pub root_author_device_id: DeviceId,
    pub roles: Vec<TrustSnapshotRole>,
    pub channels: Vec<TrustSnapshotChannel>,
    pub messages: Vec<TrustSnapshotMessage>,
    pub event_channels: Vec<TrustSnapshotEventChannel>,
    #[serde(default)]
    pub person_device_links: Vec<TrustSnapshotPersonDeviceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTrustSnapshot {
    pub snapshot: TrustSnapshot,
    pub root_event: SignedEvent,
    pub author_public_key: Vec<u8>,
    pub signature: Vec<u8>,
}

impl WorkspaceId {
    pub fn new() -> Self {
        Self(format!("wrk_{}", Uuid::new_v4().simple()))
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelId {
    pub fn new() -> Self {
        Self(format!("chn_{}", Uuid::new_v4().simple()))
    }
}

impl Default for ChannelId {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageId {
    pub fn new() -> Self {
        Self(format!("msg_{}", Uuid::new_v4().simple()))
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceKeyPackageId {
    pub fn new() -> Self {
        Self(format!("dkp_{}", Uuid::new_v4().simple()))
    }
}

impl Default for DeviceKeyPackageId {
    fn default() -> Self {
        Self::new()
    }
}

impl DeviceId {
    pub fn from_public_key_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Self(format!("dev_{}", hash.to_hex()))
    }
}

impl PersonId {
    pub fn new() -> Self {
        Self(format!("{}{}", PERSON_ID_PREFIX, Uuid::new_v4().simple()))
    }
}

impl Default for PersonId {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridTimestamp {
    pub fn now() -> Self {
        let physical_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default();

        Self {
            physical_ms,
            logical: 0,
        }
    }
}

impl SignableEvent {
    pub fn new(
        workspace_id: WorkspaceId,
        channel_id: Option<ChannelId>,
        author_device_id: DeviceId,
        body: EventBody,
    ) -> Self {
        Self {
            schema_version: 1,
            workspace_id,
            channel_id,
            author_device_id,
            timestamp: HybridTimestamp::now(),
            parents: Vec::new(),
            body,
        }
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("signable event serialization is infallible")
    }
}

impl TrustSnapshot {
    pub fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("trust snapshot serialization is infallible")
    }
}

impl SignedEvent {
    pub fn from_signed_bytes(event: SignableEvent, signature: Vec<u8>) -> Self {
        Self::from_author_signature(event, Vec::new(), signature)
    }

    pub fn from_author_signature(
        event: SignableEvent,
        author_public_key: Vec<u8>,
        signature: Vec<u8>,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&event.signing_bytes());
        hasher.update(&signature);
        let event_id = EventId(format!("evt_{}", hasher.finalize().to_hex()));

        Self {
            event_id,
            event,
            author_public_key,
            signature,
        }
    }
}

impl Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_IROH_ENDPOINT_ID: &str =
        "ae58ff8833241ac82d6ff7611046ed67b5072d142c588d0063e942d9a75502b6";

    #[test]
    fn event_id_is_stable_for_same_signable_event_and_signature() {
        let event = SignableEvent {
            schema_version: 1,
            workspace_id: WorkspaceId("wrk_test".to_owned()),
            channel_id: Some(ChannelId("chn_test".to_owned())),
            author_device_id: DeviceId("dev_test".to_owned()),
            timestamp: HybridTimestamp {
                physical_ms: 10,
                logical: 1,
            },
            parents: vec![EventId("evt_parent".to_owned())],
            body: EventBody::MessageCreated {
                message_id: MessageId("msg_test".to_owned()),
                markdown: "hello".to_owned(),
                attachments: Vec::new(),
            },
        };

        let first = SignedEvent::from_signed_bytes(event.clone(), vec![1, 2, 3]);
        let second = SignedEvent::from_signed_bytes(event, vec![1, 2, 3]);

        assert_eq!(first.event_id, second.event_id);
    }

    #[test]
    fn capability_invite_json_keeps_legacy_presence_while_defaulting_effective_limit() {
        let legacy = serde_json::json!({
            "kind": "workspace_invite_capability_created",
            "invite_id": "inv_legacy",
            "display_name": "Team",
            "role": "member",
            "expires_at": "2026-07-14T12:00:00Z",
            "capability_public_key": "capability-key",
            "sync_expectation": "manual"
        });
        let zero = serde_json::json!({
            "kind": "workspace_invite_capability_created",
            "invite_id": "inv_zero",
            "display_name": "Team",
            "role": "member",
            "expires_at": "2026-07-14T12:00:00Z",
            "capability_public_key": "capability-key",
            "sync_expectation": "manual",
            "max_claims": 0
        });

        let legacy: EventBody = serde_json::from_value(legacy).unwrap();
        let zero: EventBody = serde_json::from_value(zero).unwrap();

        assert!(matches!(
            legacy,
            EventBody::WorkspaceInviteCapabilityCreated {
                max_claims: None,
                ..
            }
        ));
        assert!(matches!(
            zero,
            EventBody::WorkspaceInviteCapabilityCreated {
                max_claims: Some(0),
                ..
            }
        ));
        assert_eq!(effective_workspace_invite_max_claims(None), 1);
        assert_eq!(effective_workspace_invite_max_claims(Some(0)), 1);
    }

    #[test]
    fn capability_invite_json_preserves_explicit_max_claims() {
        let body: EventBody = serde_json::from_value(serde_json::json!({
            "kind": "workspace_invite_capability_created",
            "invite_id": "inv_group",
            "display_name": "Team",
            "role": "member",
            "expires_at": "2026-07-14T12:00:00Z",
            "capability_public_key": "capability-key",
            "sync_expectation": "manual",
            "max_claims": 3
        }))
        .unwrap();

        assert!(matches!(
            body,
            EventBody::WorkspaceInviteCapabilityCreated {
                max_claims: Some(3),
                ..
            }
        ));
    }

    #[test]
    fn legacy_capability_invite_signing_json_round_trips_without_adding_max_claims() {
        let legacy = br#"{"schema_version":1,"workspace_id":"wrk_legacy","channel_id":null,"author_device_id":"dev_owner","timestamp":{"physical_ms":10,"logical":0},"parents":[],"body":{"kind":"workspace_invite_capability_created","invite_id":"inv_legacy","display_name":"Team","role":"member","expires_at":"2026-07-14T12:00:00Z","capability_public_key":"capability-key","sync_expectation":"manual"}}"#;

        let event: SignableEvent = serde_json::from_slice(legacy).unwrap();
        let reserialized = serde_json::to_vec(&event).unwrap();

        assert_eq!(reserialized, legacy);
    }

    #[test]
    fn generated_ids_fit_declared_identifier_budgets() {
        assert!(WorkspaceId::new().0.len() <= WORKSPACE_ID_MAX_BYTES);
        assert!(ChannelId::new().0.len() <= CHANNEL_ID_MAX_BYTES);
        assert!(MessageId::new().0.len() <= MESSAGE_ID_MAX_BYTES);
        assert!(DeviceKeyPackageId::new().0.len() <= DEVICE_KEY_PACKAGE_ID_MAX_BYTES);
        assert!(DeviceId::from_public_key_bytes(b"public key").0.len() <= DEVICE_ID_MAX_BYTES);
        assert!(PersonId::new().0.len() <= PERSON_ID_MAX_BYTES);

        let signed = SignedEvent::from_signed_bytes(
            SignableEvent {
                schema_version: 1,
                workspace_id: WorkspaceId::new(),
                channel_id: Some(ChannelId::new()),
                author_device_id: DeviceId::from_public_key_bytes(b"author"),
                timestamp: HybridTimestamp {
                    physical_ms: 10,
                    logical: 1,
                },
                parents: Vec::new(),
                body: EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "hello".to_owned(),
                    attachments: Vec::new(),
                },
            },
            vec![1, 2, 3],
        );
        assert_eq!(signed.event_id.0.len(), EVENT_ID_MAX_BYTES);
    }

    #[test]
    fn canonical_event_id_validation_matches_wire_format() {
        let canonical = format!("{}{}", EVENT_ID_PREFIX, "a".repeat(EVENT_ID_HASH_HEX_BYTES));
        assert!(is_canonical_event_id_str(&canonical));
        assert!(validate_event_id_str(&canonical).is_ok());

        let signed = SignedEvent::from_signed_bytes(
            SignableEvent {
                schema_version: 1,
                workspace_id: WorkspaceId::new(),
                channel_id: Some(ChannelId::new()),
                author_device_id: DeviceId::from_public_key_bytes(b"author"),
                timestamp: HybridTimestamp {
                    physical_ms: 10,
                    logical: 1,
                },
                parents: Vec::new(),
                body: EventBody::MessageCreated {
                    message_id: MessageId::new(),
                    markdown: "hello".to_owned(),
                    attachments: Vec::new(),
                },
            },
            vec![1, 2, 3],
        );
        assert!(is_canonical_event_id_str(&signed.event_id.0));

        assert!(!is_canonical_event_id_str(&format!(
            "{}{}",
            EVENT_ID_PREFIX,
            "A".repeat(EVENT_ID_HASH_HEX_BYTES)
        )));
        assert!(!is_canonical_event_id_str(&format!(
            "{}{}",
            EVENT_ID_PREFIX,
            "g".repeat(EVENT_ID_HASH_HEX_BYTES)
        )));
        assert!(!is_canonical_event_id_str(&format!(
            "ev_{}",
            "a".repeat(EVENT_ID_HASH_HEX_BYTES)
        )));
        assert!(!is_canonical_event_id_str(&format!(
            "{}{}",
            EVENT_ID_PREFIX,
            "a".repeat(EVENT_ID_HASH_HEX_BYTES - 1)
        )));
        assert!(!is_canonical_event_id_str(&format!(
            "{}{}",
            EVENT_ID_PREFIX,
            "a".repeat(EVENT_ID_HASH_HEX_BYTES + 1)
        )));
    }

    #[test]
    fn peer_endpoint_hint_policy_accepts_default_p2p_routes_only() {
        assert_eq!(
            supported_peer_endpoint_hint_route(" direct+tcp://127.0.0.1:7777 "),
            Some(SupportedPeerEndpointHintRoute::DirectTcp)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route("tcp://127.0.0.1:7777"),
            Some(SupportedPeerEndpointHintRoute::DirectTcp)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route("127.0.0.1:7777"),
            Some(SupportedPeerEndpointHintRoute::DirectTcp)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route("localhost:7777"),
            Some(SupportedPeerEndpointHintRoute::DirectTcp)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route("[::1]:7777"),
            Some(SupportedPeerEndpointHintRoute::DirectTcp)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route(&format!(
                "iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"
            )),
            Some(SupportedPeerEndpointHintRoute::NativeIrohDirect)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route(&format!(
                "iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777&addr=127.0.0.1:8888"
            )),
            Some(SupportedPeerEndpointHintRoute::NativeIrohDirect)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route(&format!(
                "iroh://{VALID_IROH_ENDPOINT_ID}?relay=https://relay.example.invalid&addr=127.0.0.1:7777"
            )),
            Some(SupportedPeerEndpointHintRoute::NativeIrohRelay)
        );
        assert_eq!(
            supported_peer_endpoint_hint_route(&format!("iroh://{VALID_IROH_ENDPOINT_ID}")),
            Some(SupportedPeerEndpointHintRoute::NativeIrohDiscovery)
        );

        let rejected_endpoints = vec![
            "".to_owned(),
            "direct+tcp://".to_owned(),
            "tcp:// ".to_owned(),
            "not-a-socket".to_owned(),
            "direct+tcp://127.0.0.1".to_owned(),
            "direct+tcp://127.0.0.1:0".to_owned(),
            "direct+tcp://127.0.0.1:70000".to_owned(),
            "direct+tcp://127.0.0.1:http".to_owned(),
            "direct+tcp://host name:7777".to_owned(),
            "direct+tcp://bad/path:7777".to_owned(),
            "direct+tcp://user@host:7777".to_owned(),
            "direct+tcp://[not-ip]:7777".to_owned(),
            "::1:7777".to_owned(),
            "wss://central.example.invalid/sync".to_owned(),
            "https://central.example.invalid/sync".to_owned(),
            "relay://relay.example.invalid/dev".to_owned(),
            "iroh+relay://relay.example.invalid/dev".to_owned(),
            "discovery://workspace".to_owned(),
            "iroh+discovery://workspace".to_owned(),
            "iroh://node".to_owned(),
            "iroh://node?addr=127.0.0.1:7777".to_owned(),
            "iroh://AE58FF8833241AC82D6FF7611046ED67B5072D142C588D0063E942D9A75502B6?addr=127.0.0.1:7777".to_owned(),
            format!("iroh:// {VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID} ?addr=127.0.0.1:7777"),
            "iroh://node?relay=https://relay.example.invalid&addr=127.0.0.1:7777".to_owned(),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?relay=http://relay.example.invalid"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?relay=https://"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=localhost:7777"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:0"),
            format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777&foo=bar"),
            "iroh://?addr=127.0.0.1:7777".to_owned(),
            "unsupported://peer".to_owned(),
        ];
        for endpoint in rejected_endpoints {
            assert!(
                !peer_endpoint_hint_is_supported(&endpoint),
                "endpoint should be rejected: {endpoint}"
            );
        }
    }

    #[test]
    fn direct_tcp_listen_address_policy_allows_ephemeral_binds_only_as_bare_addresses() {
        for address in ["127.0.0.1:0", "127.0.0.1:7777", "localhost:0", "[::1]:0"] {
            assert!(
                direct_tcp_peer_listen_address_is_valid(address),
                "listen address should be accepted: {address}"
            );
        }

        for address in [
            "",
            "direct+tcp://127.0.0.1:0",
            "tcp://127.0.0.1:0",
            "https://central.example.invalid/sync",
            "relay://relay.example.invalid/device",
            "discovery://workspace",
            "127.0.0.1",
            "127.0.0.1:not-a-port",
            "127.0.0.1:70000",
            "::1:0",
        ] {
            assert!(
                !direct_tcp_peer_listen_address_is_valid(address),
                "listen address should be rejected: {address}"
            );
        }
    }

    #[test]
    fn peer_endpoint_hint_transport_must_match_route() {
        assert!(peer_endpoint_hint_transport_is_consistent(
            "direct+tcp://127.0.0.1:7777",
            "direct-tcp"
        ));
        assert!(peer_endpoint_hint_transport_is_consistent(
            "127.0.0.1:7777",
            "direct-tcp"
        ));
        assert!(peer_endpoint_hint_transport_is_consistent(
            &format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"),
            "iroh"
        ));
        assert!(peer_endpoint_hint_transport_is_consistent(
            &format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"),
            "iroh-direct"
        ));
        assert!(peer_endpoint_hint_transport_is_consistent(
            &format!("iroh://{VALID_IROH_ENDPOINT_ID}?relay=https://relay.example.invalid"),
            "iroh-relay"
        ));
        assert!(peer_endpoint_hint_transport_is_consistent(
            &format!("iroh://{VALID_IROH_ENDPOINT_ID}"),
            "iroh-discovery"
        ));

        let mismatched_transports = vec![
            ("direct+tcp://127.0.0.1:7777".to_owned(), "iroh"),
            ("direct+tcp://127.0.0.1:7777".to_owned(), "custom"),
            (
                format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"),
                "direct-tcp",
            ),
            (
                format!("iroh://{VALID_IROH_ENDPOINT_ID}?addr=127.0.0.1:7777"),
                "iroh-discovery",
            ),
            ("wss://central.example.invalid/sync".to_owned(), "wss"),
        ];
        for (endpoint, transport) in mismatched_transports {
            assert!(
                !peer_endpoint_hint_transport_is_consistent(&endpoint, transport),
                "{endpoint} with {transport} should be rejected"
            );
        }
    }

    #[test]
    fn replica_storage_class_wire_values_are_stable() {
        for value in ReplicaStorageClass::supported_wire_values() {
            let storage_class = ReplicaStorageClass::from_wire(value).unwrap();
            assert_eq!(storage_class.as_str(), *value);
        }
        assert_eq!(ReplicaStorageClass::from_wire("full-history"), None);
        assert_eq!(ReplicaStorageClass::from_wire("unknown"), None);
    }

    #[test]
    fn peer_endpoint_events_decode_without_replica_capability_fields() {
        let body = serde_json::from_value::<EventBody>(serde_json::json!({
            "kind": "peer_endpoint_published",
            "endpoint_id": "desktop",
            "endpoint": "direct+tcp://127.0.0.1:7777",
            "transport": "direct-tcp",
            "is_backup_peer": true,
            "expires_at_ms": null
        }))
        .unwrap();

        match body {
            EventBody::PeerEndpointPublished {
                replica_storage_class,
                replica_retention_hint,
                ..
            } => {
                assert_eq!(replica_storage_class, None);
                assert_eq!(replica_retention_hint, None);
            }
            other => panic!("unexpected event body: {other:?}"),
        }
    }

    #[test]
    fn identifier_validation_rejects_oversized_values() {
        let oversized = "x".repeat(WORKSPACE_ID_MAX_BYTES + 1);
        assert_eq!(
            validate_workspace_id_str(&oversized),
            Err(IdValidationError {
                field: "workspace ID",
                actual_bytes: WORKSPACE_ID_MAX_BYTES + 1,
                max_bytes: WORKSPACE_ID_MAX_BYTES,
            })
        );

        let oversized_event_id = "x".repeat(EVENT_ID_MAX_BYTES + 1);
        assert_eq!(
            validate_event_id_str(&oversized_event_id),
            Err(IdValidationError {
                field: "event ID",
                actual_bytes: EVENT_ID_MAX_BYTES + 1,
                max_bytes: EVENT_ID_MAX_BYTES,
            })
        );

        let oversized_person_id = "x".repeat(PERSON_ID_MAX_BYTES + 1);
        assert_eq!(
            validate_person_id_str(&oversized_person_id),
            Err(IdValidationError {
                field: "person ID",
                actual_bytes: PERSON_ID_MAX_BYTES + 1,
                max_bytes: PERSON_ID_MAX_BYTES,
            })
        );
    }
}
