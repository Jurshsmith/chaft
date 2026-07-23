use std::{fs, path::Path, sync::Mutex};

use chaft_core::{WorkspaceInviteStatus, WorkspaceJoinRequestStatus, authorize_event_with_history};
use chaft_crypto::{ContentKey, SealedPayload, open_aes_256_gcm_siv, seal_aes_256_gcm_siv};
use chaft_identity::{
    InvitationCapability, verify_detached_signature, verify_device_detached_signature,
};
use chaft_store::StoreError;
use chaft_types::{
    DEVICE_DISPLAY_NAME_MAX_BYTES, DeviceId, EventBody, PEER_ENDPOINT_MAX_BYTES, SignableEvent,
    WORKSPACE_ACCESS_POLICY_MAX_BYTES, WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES,
    WORKSPACE_INVITE_CAPABILITY_PUBLIC_KEY_MAX_BYTES, WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
    WORKSPACE_INVITE_ID_MAX_BYTES, WORKSPACE_INVITE_LABEL_MAX_BYTES, WORKSPACE_INVITE_MAX_CLAIMS,
    WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES, WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
    WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES, WorkspaceId, WorkspaceRole,
    effective_workspace_invite_max_claims,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::{
    LocalRuntime, RuntimeError, WorkspaceKeyExport, WorkspaceWriteContext,
    local_file_io::{read_local_metadata_file_with_limit, write_secret_file},
    runtime_validation::{
        validate_device_id_reference, validate_metadata_field_size, validate_peer_endpoint_input,
        validate_workspace_id_reference,
    },
};

pub const WORKSPACE_INVITE_ARTIFACT_KIND: &str = "chaft.workspace-invite.v2";
pub const WORKSPACE_INVITE_CLAIM_KIND: &str = "chaft.workspace-invite-claim.v1";
pub const WORKSPACE_INVITE_RESPONSE_KIND: &str = "chaft.workspace-invite-response.v1";
const WORKSPACE_INVITE_SCHEMA_VERSION: u32 = 2;
const WORKSPACE_INVITE_CLAIM_SCHEMA_VERSION: u32 = 1;
const WORKSPACE_INVITE_RESPONSE_SCHEMA_VERSION: u32 = 1;
const RESPONSE_KEY_CONTEXT: &str = "Chaft workspace invite response key v1";
const RESPONSE_IDENTITY_KEY_CONTEXT: &str = "Chaft workspace invite response identity key v1";
const CAPABILITY_SECRET_BYTES: usize = 32;
const INVITE_CLAIM_RECEIPT_MAX_BYTES: usize = 8 * 1024;
const INVITE_PROFILE_FINALIZATION_KIND: &str = "chaft.workspace-invite-profile-finalization.v1";
const INVITE_PROFILE_FINALIZATION_SCHEMA_VERSION: u32 = 1;
const INVITE_PROFILE_FINALIZATION_MAX_BYTES: usize = 8 * 1024;
const INVITE_RESPONSE_RECEIPT_KIND: &str = "chaft.workspace-invite-response-receipt.v1";
const INVITE_RESPONSE_RECEIPT_SCHEMA_VERSION: u32 = 1;
const INVITE_RESPONSE_RECEIPT_MAX_BYTES: usize = 64 * 1024;

static INVITE_RESPONSE_RECEIPT_LOCK: Mutex<()> = Mutex::new(());
static INVITE_CLAIM_MUTATION_LOCK: Mutex<()> = Mutex::new(());
static INVITE_PROFILE_FINALIZATION_LOCK: Mutex<()> = Mutex::new(());
const INVITE_CLAIM_HISTORY_RETRY_LIMIT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub workspace_id: String,
    pub workspace_name: String,
    pub invite_id: String,
    /// Inviter-defined metadata. The historical Rust and signed-wire name is retained for
    /// compatibility; it must never be interpreted as the claimant's member display name.
    pub display_name: String,
    pub role: WorkspaceRole,
    pub expires_at: String,
    pub capability_secret: String,
    pub capability_public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_claims: Option<u32>,
    pub inviter_device_id: String,
    pub inviter_display_name: String,
    pub inviter_public_key: String,
    pub inviter_signature: String,
    pub peer_endpoint: String,
    pub sync_expectation: String,
    pub created_at: String,
}

impl WorkspaceInviteArtifact {
    /// Returns the inviter-defined label using its unambiguous product meaning.
    pub fn invite_label(&self) -> &str {
        &self.display_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWorkspaceInvite {
    pub workspace_id: String,
    pub invite_id: String,
    pub event_id: String,
    pub artifact: WorkspaceInviteArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteClaimPayload {
    pub kind: String,
    pub schema_version: u32,
    pub request_id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub invite_id: String,
    pub device_id: String,
    pub device_public_key: String,
    pub response_encryption_public_key: String,
    pub display_name: String,
    pub note: String,
    pub delivery_device_id: String,
    pub delivery_display_name: String,
    pub delivery_peer_endpoint: String,
    pub response_peer_endpoint: String,
    pub source_type: String,
    pub source_invite_id: String,
    pub source_display_name: String,
    pub source_approval_policy: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteClaim {
    #[serde(flatten)]
    pub payload: WorkspaceInviteClaimPayload,
    pub device_signature: String,
    pub capability_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceInviteResponsePayload {
    kind: String,
    schema_version: u32,
    request_id: String,
    workspace_id: String,
    workspace_name: String,
    invite_id: String,
    invitee_device_id: String,
    role: WorkspaceRole,
    expires_at: String,
    responder_device_id: String,
    responder_public_key: String,
    sender_ephemeral_public_key: String,
    sealed_workspace_key: SealedPayload,
    peer_endpoint: String,
    created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteResponse {
    #[serde(flatten)]
    payload: WorkspaceInviteResponsePayload,
    pub responder_signature: String,
}

impl WorkspaceInviteResponse {
    pub fn request_id(&self) -> &str {
        &self.payload.request_id
    }

    pub fn workspace_id(&self) -> &str {
        &self.payload.workspace_id
    }

    pub fn invitee_device_id(&self) -> &str {
        &self.payload.invitee_device_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedWorkspaceInvite {
    pub workspace_id: String,
    pub invite_id: String,
    pub request_id: String,
    pub invitee_device_id: String,
    pub role: WorkspaceRole,
    pub member_event_id: String,
    pub claim_event_id: String,
    pub response: WorkspaceInviteResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspaceInviteResponse {
    pub workspace_id: String,
    pub invite_id: String,
    pub request_id: String,
    pub importer_device_id: String,
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceInviteClaimReceipt {
    workspace_id: String,
    invite_id: String,
    request_id: String,
    expected_responder_device_id: String,
    capability_public_key: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    profile_pending: bool,
    #[serde(default)]
    profile_finalized: bool,
    #[serde(default)]
    profile_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingWorkspaceInviteProfileFinalization {
    kind: String,
    schema_version: u32,
    workspace_id: String,
    invite_id: String,
    request_id: String,
}

#[derive(Debug)]
struct StoredWorkspaceInviteClaimReceipt {
    receipt: WorkspaceInviteClaimReceipt,
    path: std::path::PathBuf,
}

#[derive(Debug)]
struct CanonicalWorkspaceInviteClaim {
    display_name: String,
    member_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceInviteResponseReceipt {
    kind: String,
    schema_version: u32,
    response: WorkspaceInviteResponse,
}

impl LocalRuntime {
    pub fn create_workspace_invite(
        &self,
        workspace_id: WorkspaceId,
        invite_label: String,
        role: WorkspaceRole,
        expires_at: String,
        peer_endpoint: String,
        sync_expectation: String,
    ) -> Result<CreatedWorkspaceInvite, RuntimeError> {
        self.create_workspace_invite_with_max_claims(
            workspace_id,
            invite_label,
            role,
            1,
            expires_at,
            peer_endpoint,
            sync_expectation,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_workspace_invite_with_max_claims(
        &self,
        workspace_id: WorkspaceId,
        invite_label: String,
        role: WorkspaceRole,
        max_claims: u32,
        expires_at: String,
        peer_endpoint: String,
        sync_expectation: String,
    ) -> Result<CreatedWorkspaceInvite, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        let max_claims = effective_workspace_invite_max_claims(Some(max_claims));
        if max_claims > WORKSPACE_INVITE_MAX_CLAIMS {
            return Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace invite claims",
                actual_bytes: max_claims as usize,
                max_bytes: WORKSPACE_INVITE_MAX_CLAIMS as usize,
            });
        }
        let invite_label = invite_label.trim().to_owned();
        validate_metadata_field_size(
            "invite label",
            &invite_label,
            WORKSPACE_INVITE_LABEL_MAX_BYTES,
        )?;
        validate_metadata_field_size(
            "invite expiry",
            &expires_at,
            WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES,
        )?;
        if !peer_endpoint.is_empty() {
            validate_peer_endpoint_input(&peer_endpoint)?;
        }
        validate_metadata_field_size(
            "invite sync expectation",
            &sync_expectation,
            WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES,
        )?;

        let context = self.workspace_write_context(&workspace_id)?;
        let capability = InvitationCapability::generate();
        let capability_public_key = encode_hex(&capability.verifying_key_bytes());
        validate_metadata_field_size(
            "invite capability public key",
            &capability_public_key,
            WORKSPACE_INVITE_CAPABILITY_PUBLIC_KEY_MAX_BYTES,
        )?;
        let invite_id = generated_id("inv", capability.verifying_key_bytes().as_slice());
        validate_metadata_field_size("invite ID", &invite_id, WORKSPACE_INVITE_ID_MAX_BYTES)?;

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceInviteCapabilityCreated {
                invite_id: invite_id.clone(),
                display_name: invite_label.clone(),
                role,
                expires_at: expires_at.clone(),
                capability_public_key: capability_public_key.clone(),
                sync_expectation: sync_expectation.clone(),
                max_claims: Some(max_claims),
            },
        );
        event.parents = context.head_event_ids;
        let event = self.sign_authorize_and_append_with_history(event, &context.events)?;
        let inviter_display_name = context
            .state
            .profiles
            .get(self.identity.device_id())
            .map(|profile| profile.display_name.clone())
            .unwrap_or_default();
        let mut artifact = WorkspaceInviteArtifact {
            kind: WORKSPACE_INVITE_ARTIFACT_KIND.to_owned(),
            schema_version: WORKSPACE_INVITE_SCHEMA_VERSION,
            workspace_id: workspace_id.0.clone(),
            workspace_name: context.state.name.unwrap_or_default(),
            invite_id: invite_id.clone(),
            display_name: invite_label,
            role,
            expires_at,
            capability_secret: encode_hex(&capability.secret_bytes()),
            capability_public_key,
            max_claims: Some(max_claims),
            inviter_device_id: self.identity.device_id().0.clone(),
            inviter_display_name,
            inviter_public_key: encode_hex(&self.identity.verifying_key_bytes()),
            inviter_signature: String::new(),
            peer_endpoint,
            sync_expectation,
            created_at: current_timestamp(),
        };
        artifact.inviter_signature = encode_hex(
            &self
                .identity
                .sign_bytes(&workspace_invite_artifact_signing_bytes(&artifact)?),
        );
        Ok(CreatedWorkspaceInvite {
            workspace_id: workspace_id.0,
            invite_id,
            event_id: event.event_id.0,
            artifact,
        })
    }

    pub fn prepare_workspace_invite_claim(
        &self,
        artifact: WorkspaceInviteArtifact,
        display_name: String,
        note: String,
        response_peer_endpoint: String,
    ) -> Result<WorkspaceInviteClaim, RuntimeError> {
        validate_invite_artifact(&artifact)?;
        let display_name = display_name.trim().to_owned();
        if display_name.is_empty() {
            return Err(RuntimeError::DisplayNameRequired);
        }
        validate_metadata_field_size("display name", &display_name, DEVICE_DISPLAY_NAME_MAX_BYTES)?;
        let note = note.trim().to_owned();
        validate_metadata_field_size(
            "join request note",
            &note,
            WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
        )?;
        let capability_secret = decode_hex_32(&artifact.capability_secret)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
        let capability = InvitationCapability::from_secret_bytes(capability_secret);
        if encode_hex(&capability.verifying_key_bytes()) != artifact.capability_public_key {
            return Err(RuntimeError::InvalidWorkspaceInviteClaim);
        }
        if !response_peer_endpoint.is_empty() {
            validate_peer_endpoint_input(&response_peer_endpoint)?;
        }
        let request_id = generated_id("req", &random_bytes());
        validate_metadata_field_size(
            "join request ID",
            &request_id,
            WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
        )?;
        let response_secret = self.invite_response_secret();
        let response_public_key = X25519PublicKey::from(&response_secret);
        let expected_responder_device_id = artifact.inviter_device_id.clone();
        let capability_public_key = artifact.capability_public_key.clone();
        let payload = WorkspaceInviteClaimPayload {
            kind: WORKSPACE_INVITE_CLAIM_KIND.to_owned(),
            schema_version: WORKSPACE_INVITE_CLAIM_SCHEMA_VERSION,
            request_id,
            workspace_id: artifact.workspace_id,
            workspace_name: artifact.workspace_name,
            invite_id: artifact.invite_id.clone(),
            device_id: self.identity.device_id().0.clone(),
            device_public_key: encode_hex(&self.identity.verifying_key_bytes()),
            response_encryption_public_key: encode_hex(response_public_key.as_bytes()),
            display_name,
            note,
            delivery_device_id: artifact.inviter_device_id,
            delivery_display_name: artifact.inviter_display_name.clone(),
            delivery_peer_endpoint: artifact.peer_endpoint,
            response_peer_endpoint,
            source_type: "invite_claim".to_owned(),
            source_invite_id: artifact.invite_id,
            source_display_name: artifact.inviter_display_name,
            source_approval_policy: "preapproved".to_owned(),
            created_at: current_timestamp(),
        };
        let signing_bytes = serde_json::to_vec(&payload)?;
        let claim = WorkspaceInviteClaim {
            device_signature: encode_hex(&self.identity.sign_bytes(&signing_bytes)),
            capability_signature: encode_hex(&capability.sign(&signing_bytes)),
            payload,
        };
        let receipt = WorkspaceInviteClaimReceipt {
            workspace_id: claim.payload.workspace_id.clone(),
            invite_id: claim.payload.invite_id.clone(),
            request_id: claim.payload.request_id.clone(),
            expected_responder_device_id,
            capability_public_key,
            display_name: claim.payload.display_name.clone(),
            profile_pending: false,
            profile_finalized: false,
            profile_event_ids: Vec::new(),
        };
        self.write_workspace_invite_claim_receipt(
            &self.workspace_invite_claim_receipt_path(&receipt.invite_id, &receipt.request_id),
            &receipt,
        )?;
        Ok(claim)
    }

    pub fn claim_workspace_invite(
        &self,
        claim: WorkspaceInviteClaim,
    ) -> Result<ClaimedWorkspaceInvite, RuntimeError> {
        let _claim_guard = INVITE_CLAIM_MUTATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.verify_workspace_invite_claim(&claim)?;
        let workspace_id = WorkspaceId(claim.payload.workspace_id.clone());

        for _ in 0..INVITE_CLAIM_HISTORY_RETRY_LIMIT {
            let expected_event_ids = self.store.list_event_ids_for_workspace(&workspace_id.0)?;
            let context = self.workspace_write_context(&workspace_id)?;
            if self.store.list_event_ids_for_workspace(&workspace_id.0)? != expected_event_ids {
                continue;
            }
            match self.claim_workspace_invite_against_history(
                &claim,
                &workspace_id,
                context,
                &expected_event_ids,
            ) {
                Err(RuntimeError::Store(StoreError::WorkspaceHistoryChanged)) => continue,
                result => return result,
            }
        }

        Err(StoreError::WorkspaceHistoryChanged.into())
    }

    fn claim_workspace_invite_against_history(
        &self,
        claim: &WorkspaceInviteClaim,
        workspace_id: &WorkspaceId,
        context: WorkspaceWriteContext,
        expected_event_ids: &[chaft_types::EventId],
    ) -> Result<ClaimedWorkspaceInvite, RuntimeError> {
        let invite = context
            .state
            .invites
            .get(&claim.payload.invite_id)
            .cloned()
            .ok_or_else(|| RuntimeError::WorkspaceInviteNotFound {
                invite_id: claim.payload.invite_id.clone(),
            })?;
        if invite.capability_public_key.is_empty() {
            return Err(RuntimeError::WorkspaceInviteNotClaimable {
                invite_id: invite.invite_id,
            });
        }
        let invitee_device_id = DeviceId(claim.payload.device_id.clone());

        // The materialized invite view intentionally retains only the latest
        // claim. Use persisted history to make an older exact retry idempotent
        // and to keep device/request IDs unique across all claims.
        let existing_claim = context
            .events
            .iter()
            .enumerate()
            .find_map(|(index, event)| match &event.event.body {
                EventBody::WorkspaceInviteClaimed {
                    invite_id,
                    invitee_device_id: claimed_device_id,
                    request_id,
                } if invite_id == &claim.payload.invite_id
                    && claimed_device_id == &invitee_device_id
                    && request_id == &claim.payload.request_id =>
                {
                    Some((index, event.event_id.0.clone()))
                }
                _ => None,
            });
        let has_claim_collision = context.events.iter().any(|event| match &event.event.body {
            EventBody::WorkspaceInviteClaimed {
                invite_id,
                invitee_device_id: claimed_device_id,
                request_id,
            } => {
                let exact = invite_id == &claim.payload.invite_id
                    && claimed_device_id == &invitee_device_id
                    && request_id == &claim.payload.request_id;
                !exact
                    && ((invite_id == &claim.payload.invite_id
                        && claimed_device_id == &invitee_device_id)
                        || request_id == &claim.payload.request_id)
            }
            _ => false,
        });
        let has_join_request_collision =
            context.events.iter().any(|event| match &event.event.body {
                EventBody::WorkspaceJoinRequestRecorded {
                    request_id,
                    requester_device_id,
                    source_type,
                    source_invite_id,
                    ..
                } if request_id == &claim.payload.request_id => {
                    existing_claim.is_none()
                        || requester_device_id != &invitee_device_id
                        || source_type != "invite_claim"
                        || source_invite_id != &claim.payload.invite_id
                }
                _ => false,
            });
        if has_claim_collision || has_join_request_collision {
            return Err(RuntimeError::WorkspaceInviteAlreadyClaimed {
                invite_id: invite.invite_id,
            });
        }

        if let Some((claim_index, claim_event_id)) = existing_claim {
            let current_member =
                context
                    .state
                    .members
                    .get(&invitee_device_id)
                    .ok_or_else(|| RuntimeError::WorkspaceInviteAlreadyClaimed {
                        invite_id: invite.invite_id.clone(),
                    })?;
            let member_event_id = context.events[..claim_index]
                .iter()
                .rev()
                .find_map(|event| match &event.event.body {
                    EventBody::MemberInvited {
                        invitee_device_id: member_device_id,
                        ..
                    } if member_device_id == &invitee_device_id => Some(event.event_id.0.clone()),
                    _ => None,
                })
                .ok_or_else(|| RuntimeError::WorkspaceInviteAlreadyClaimed {
                    invite_id: invite.invite_id.clone(),
                })?;
            if current_member.membership_event_id.0 != member_event_id {
                return Err(RuntimeError::WorkspaceInviteAlreadyClaimed {
                    invite_id: invite.invite_id,
                });
            }
            let response = self.workspace_invite_response_receipt_or_store(
                claim,
                invite.role,
                &invite.expires_at,
                None,
            )?;
            return Ok(ClaimedWorkspaceInvite {
                workspace_id: workspace_id.0.clone(),
                invite_id: claim.payload.invite_id.clone(),
                request_id: claim.payload.request_id.clone(),
                invitee_device_id: invitee_device_id.0,
                role: invite.role,
                member_event_id,
                claim_event_id,
                response,
            });
        }

        if invite_is_expired(&invite.expires_at)? {
            return Err(RuntimeError::WorkspaceInviteExpired {
                invite_id: invite.invite_id,
            });
        }
        if invite.status == WorkspaceInviteStatus::Revoked {
            return Err(RuntimeError::WorkspaceInviteNotClaimable {
                invite_id: invite.invite_id,
            });
        }
        if invite.claim_count >= invite.max_claims
            || invite.status == WorkspaceInviteStatus::Accepted
            || context.state.members.contains_key(&invitee_device_id)
        {
            return Err(RuntimeError::WorkspaceInviteAlreadyClaimed {
                invite_id: invite.invite_id,
            });
        }
        validate_workspace_invite_claim_record_fields(claim, &invitee_device_id)?;

        // Build the encrypted handoff before recording any membership changes.
        // A structurally valid, correctly signed claim can still contain an
        // unusable response key; accepting it first would consume one use and
        // leave a member without credentials.
        let workspace_key = self.export_workspace_key(workspace_id.clone())?;
        let response = self.seal_workspace_invite_response(
            claim,
            invite.role,
            &invite.expires_at,
            workspace_key,
        )?;

        let mut history = context.events;
        let mut request = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceJoinRequestRecorded {
                request_id: claim.payload.request_id.clone(),
                requester_device_id: invitee_device_id.clone(),
                display_name: claim.payload.display_name.trim().to_owned(),
                note: claim.payload.note.trim().to_owned(),
                source_type: "invite_claim".to_owned(),
                source_invite_id: claim.payload.invite_id.clone(),
                source_display_name: claim.payload.source_display_name.trim().to_owned(),
                source_approval_policy: "preapproved".to_owned(),
                response_peer_endpoint: claim.payload.response_peer_endpoint.trim().to_owned(),
            },
        );
        request.parents = context.head_event_ids;
        let request = self.identity.sign_event(request);
        authorize_event_with_history(&history, &request)?;
        history.push(request.clone());

        let mut member = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::MemberInvited {
                invitee_device_id: invitee_device_id.clone(),
                role: invite.role,
            },
        );
        member.parents = vec![request.event_id.clone()];
        let member = self.identity.sign_event(member);
        authorize_event_with_history(&history, &member)?;
        history.push(member.clone());

        let mut claimed = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::WorkspaceInviteClaimed {
                invite_id: claim.payload.invite_id.clone(),
                invitee_device_id: invitee_device_id.clone(),
                request_id: claim.payload.request_id.clone(),
            },
        );
        claimed.parents = vec![member.event_id.clone()];
        let claimed = self.identity.sign_event(claimed);
        authorize_event_with_history(&history, &claimed)?;

        self.store
            .append_events_atomically_if_workspace_history_matches(
                &workspace_id.0,
                expected_event_ids,
                &[request, member.clone(), claimed.clone()],
            )?;
        let _ = self.auto_add_openmls_workspace_member_if_ready(workspace_id, &invitee_device_id);
        let response = self.workspace_invite_response_receipt_or_store(
            claim,
            invite.role,
            &invite.expires_at,
            Some(response),
        )?;

        Ok(ClaimedWorkspaceInvite {
            workspace_id: workspace_id.0.clone(),
            invite_id: claim.payload.invite_id.clone(),
            request_id: claim.payload.request_id.clone(),
            invitee_device_id: invitee_device_id.0,
            role: invite.role,
            member_event_id: member.event_id.0,
            claim_event_id: claimed.event_id.0,
            response,
        })
    }

    pub fn import_workspace_invite_response(
        &self,
        response: WorkspaceInviteResponse,
    ) -> Result<ImportedWorkspaceInviteResponse, RuntimeError> {
        if response.payload.kind != WORKSPACE_INVITE_RESPONSE_KIND
            || response.payload.schema_version != WORKSPACE_INVITE_RESPONSE_SCHEMA_VERSION
            || response.payload.invitee_device_id != self.identity.device_id().0
        {
            return Err(RuntimeError::InvalidWorkspaceInviteResponse);
        }
        let receipt = self.workspace_invite_claim_receipt(
            &response.payload.invite_id,
            &response.payload.request_id,
        )?;
        if receipt.workspace_id != response.payload.workspace_id
            || receipt.invite_id != response.payload.invite_id
            || receipt.request_id != response.payload.request_id
            || receipt.expected_responder_device_id != response.payload.responder_device_id
        {
            return Err(RuntimeError::InvalidWorkspaceInviteResponse);
        }
        let responder_public_key = decode_hex_32(&response.payload.responder_public_key)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        let response_signature = decode_hex(&response.responder_signature)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        let signing_bytes = serde_json::to_vec(&response.payload)?;
        verify_device_detached_signature(
            &DeviceId(response.payload.responder_device_id.clone()),
            &responder_public_key,
            &signing_bytes,
            &response_signature,
        )
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        let sender_public_key = X25519PublicKey::from(
            decode_hex_32(&response.payload.sender_ephemeral_public_key)
                .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?,
        );
        let shared = self
            .invite_response_secret()
            .diffie_hellman(&sender_public_key);
        let key = response_content_key(shared.as_bytes());
        let workspace_key_bytes =
            open_aes_256_gcm_siv(&key, &response.payload.sealed_workspace_key)
                .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        let workspace_key: WorkspaceKeyExport = serde_json::from_slice(&workspace_key_bytes)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        if workspace_key.workspace_id != response.payload.workspace_id {
            return Err(RuntimeError::InvalidWorkspaceInviteResponse);
        }
        // Persist recovery intent before the key write. A process crash after
        // the key becomes durable must never leave the joiner's profile with no
        // discoverable finalization work. Finalization independently requires
        // the workspace key, so a failed key import cannot create profile data.
        self.mark_workspace_invite_profile_pending(
            &response.payload.workspace_id,
            &response.payload.invite_id,
            &response.payload.request_id,
        )?;
        let imported = self.import_workspace_key(workspace_key)?;
        // Manual/history-first transfers can already have the signed membership
        // chain locally when the sealed key response arrives. Finalize now in
        // that case; a normal response-first join simply keeps its pending
        // marker until the first history pull. Any generated events are part of
        // the local store and the next sync's initial publish sends them.
        let _ = self.finalize_pending_workspace_invite_profile(&WorkspaceId(
            response.payload.workspace_id.clone(),
        ))?;
        Ok(ImportedWorkspaceInviteResponse {
            workspace_id: imported.workspace_id,
            invite_id: response.payload.invite_id,
            request_id: response.payload.request_id,
            importer_device_id: imported.importer_device_id,
            key_id: imported.key_id,
        })
    }

    fn verify_workspace_invite_claim(
        &self,
        claim: &WorkspaceInviteClaim,
    ) -> Result<(), RuntimeError> {
        if claim.payload.kind != WORKSPACE_INVITE_CLAIM_KIND
            || claim.payload.schema_version != WORKSPACE_INVITE_CLAIM_SCHEMA_VERSION
            || claim.payload.workspace_id.is_empty()
            || claim.payload.invite_id.is_empty()
            || claim.payload.request_id.is_empty()
            || claim.payload.device_id.is_empty()
            || decode_hex_32(&claim.payload.response_encryption_public_key).is_err()
        {
            return Err(RuntimeError::InvalidWorkspaceInviteClaim);
        }
        let signing_bytes = serde_json::to_vec(&claim.payload)?;
        let device_public_key = decode_hex_32(&claim.payload.device_public_key)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
        let device_signature = decode_hex(&claim.device_signature)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
        verify_device_detached_signature(
            &DeviceId(claim.payload.device_id.clone()),
            &device_public_key,
            &signing_bytes,
            &device_signature,
        )
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
        let context =
            self.workspace_write_context(&WorkspaceId(claim.payload.workspace_id.clone()))?;
        let invite = context
            .state
            .invites
            .get(&claim.payload.invite_id)
            .ok_or_else(|| RuntimeError::WorkspaceInviteNotFound {
                invite_id: claim.payload.invite_id.clone(),
            })?;
        if invite.created_by_device_id != *self.identity.device_id()
            || claim.payload.delivery_device_id != self.identity.device_id().0
            || claim.payload.source_type != "invite_claim"
            || claim.payload.source_invite_id != claim.payload.invite_id
            || claim.payload.source_approval_policy != "preapproved"
        {
            return Err(RuntimeError::InvalidWorkspaceInviteClaim);
        }
        let capability_public_key = decode_hex_32(&invite.capability_public_key).map_err(|_| {
            RuntimeError::WorkspaceInviteNotClaimable {
                invite_id: invite.invite_id.clone(),
            }
        })?;
        let capability_signature = decode_hex(&claim.capability_signature)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
        verify_detached_signature(
            &capability_public_key,
            &signing_bytes,
            &capability_signature,
        )
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)
    }

    fn seal_workspace_invite_response(
        &self,
        claim: &WorkspaceInviteClaim,
        role: WorkspaceRole,
        expires_at: &str,
        workspace_key: WorkspaceKeyExport,
    ) -> Result<WorkspaceInviteResponse, RuntimeError> {
        let recipient_public_key = X25519PublicKey::from(
            decode_hex_32(&claim.payload.response_encryption_public_key)
                .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?,
        );
        let sender_secret = StaticSecret::random_from_rng(OsRng);
        let sender_public_key = X25519PublicKey::from(&sender_secret);
        let shared = sender_secret.diffie_hellman(&recipient_public_key);
        let key = response_content_key(shared.as_bytes());
        let aad = response_aad(
            &claim.payload.workspace_id,
            &claim.payload.invite_id,
            &claim.payload.device_id,
            &claim.payload.request_id,
        );
        let workspace_key_json = serde_json::to_vec(&workspace_key)?;
        let sealed_workspace_key = seal_aes_256_gcm_siv(
            format!("invite-response:{}", claim.payload.invite_id),
            &key,
            &workspace_key_json,
            &aad,
        )?;
        let context =
            self.workspace_write_context(&WorkspaceId(claim.payload.workspace_id.clone()))?;
        let payload = WorkspaceInviteResponsePayload {
            kind: WORKSPACE_INVITE_RESPONSE_KIND.to_owned(),
            schema_version: WORKSPACE_INVITE_RESPONSE_SCHEMA_VERSION,
            request_id: claim.payload.request_id.clone(),
            workspace_id: claim.payload.workspace_id.clone(),
            workspace_name: context.state.name.unwrap_or_default(),
            invite_id: claim.payload.invite_id.clone(),
            invitee_device_id: claim.payload.device_id.clone(),
            role,
            expires_at: expires_at.to_owned(),
            responder_device_id: self.identity.device_id().0.clone(),
            responder_public_key: encode_hex(&self.identity.verifying_key_bytes()),
            sender_ephemeral_public_key: encode_hex(sender_public_key.as_bytes()),
            sealed_workspace_key,
            peer_endpoint: claim.payload.delivery_peer_endpoint.clone(),
            created_at: current_timestamp(),
        };
        let signature = self.identity.sign_bytes(&serde_json::to_vec(&payload)?);
        Ok(WorkspaceInviteResponse {
            payload,
            responder_signature: encode_hex(&signature),
        })
    }

    fn workspace_invite_response_receipt_or_store(
        &self,
        claim: &WorkspaceInviteClaim,
        role: WorkspaceRole,
        expires_at: &str,
        candidate: Option<WorkspaceInviteResponse>,
    ) -> Result<WorkspaceInviteResponse, RuntimeError> {
        let _receipt_guard = INVITE_RESPONSE_RECEIPT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = self.workspace_invite_response_receipt_path(
            &claim.payload.invite_id,
            &claim.payload.request_id,
        );
        if let Some(bytes) = read_local_metadata_file_with_limit(
            &path,
            INVITE_RESPONSE_RECEIPT_MAX_BYTES,
            "workspace invite response receipt",
        )? {
            let receipt = serde_json::from_slice::<WorkspaceInviteResponseReceipt>(&bytes)
                .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
            self.validate_workspace_invite_response_receipt(&receipt, claim, role, expires_at)?;
            return Ok(receipt.response);
        }

        let response = match candidate {
            Some(response) => response,
            None => {
                let workspace_key =
                    self.export_workspace_key(WorkspaceId(claim.payload.workspace_id.clone()))?;
                self.seal_workspace_invite_response(claim, role, expires_at, workspace_key)?
            }
        };
        let receipt = WorkspaceInviteResponseReceipt {
            kind: INVITE_RESPONSE_RECEIPT_KIND.to_owned(),
            schema_version: INVITE_RESPONSE_RECEIPT_SCHEMA_VERSION,
            response: response.clone(),
        };
        self.validate_workspace_invite_response_receipt(&receipt, claim, role, expires_at)?;
        let bytes = serde_json::to_vec(&receipt)?;
        if bytes.len().saturating_add(1) > INVITE_RESPONSE_RECEIPT_MAX_BYTES {
            return Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace invite response receipt",
                actual_bytes: bytes.len().saturating_add(1),
                max_bytes: INVITE_RESPONSE_RECEIPT_MAX_BYTES,
            });
        }
        write_secret_file(&path, &bytes)?;
        Ok(response)
    }

    fn validate_workspace_invite_response_receipt(
        &self,
        receipt: &WorkspaceInviteResponseReceipt,
        claim: &WorkspaceInviteClaim,
        role: WorkspaceRole,
        expires_at: &str,
    ) -> Result<(), RuntimeError> {
        let response = &receipt.response;
        if receipt.kind != INVITE_RESPONSE_RECEIPT_KIND
            || receipt.schema_version != INVITE_RESPONSE_RECEIPT_SCHEMA_VERSION
            || response.payload.kind != WORKSPACE_INVITE_RESPONSE_KIND
            || response.payload.schema_version != WORKSPACE_INVITE_RESPONSE_SCHEMA_VERSION
            || response.payload.workspace_id != claim.payload.workspace_id
            || response.payload.invite_id != claim.payload.invite_id
            || response.payload.request_id != claim.payload.request_id
            || response.payload.invitee_device_id != claim.payload.device_id
            || response.payload.role != role
            || response.payload.expires_at != expires_at
            || response.payload.responder_device_id != self.identity.device_id().0
            || response.payload.responder_public_key
                != encode_hex(&self.identity.verifying_key_bytes())
            || decode_hex_32(&response.payload.sender_ephemeral_public_key).is_err()
        {
            return Err(RuntimeError::InvalidWorkspaceInviteResponse);
        }
        let signature = decode_hex(&response.responder_signature)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        let signing_bytes = serde_json::to_vec(&response.payload)?;
        verify_device_detached_signature(
            self.identity.device_id(),
            &self.identity.verifying_key_bytes(),
            &signing_bytes,
            &signature,
        )
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)
    }

    fn mark_workspace_invite_profile_pending(
        &self,
        workspace_id: &str,
        invite_id: &str,
        request_id: &str,
    ) -> Result<(), RuntimeError> {
        let _profile_guard = INVITE_PROFILE_FINALIZATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (mut receipt, receipt_path) =
            self.workspace_invite_claim_receipt_with_path(invite_id, request_id)?;
        if receipt.workspace_id != workspace_id
            || receipt.invite_id != invite_id
            || receipt.request_id != request_id
        {
            return Err(RuntimeError::InvalidWorkspaceInviteResponse);
        }

        let marker_path =
            self.workspace_invite_profile_finalization_path(workspace_id, invite_id, request_id);
        if receipt.profile_finalized {
            remove_local_marker_file(&marker_path)?;
            return Ok(());
        }

        // Markers are per claim. A stale claim from a removed membership must
        // never prevent a later re-invite from recording its own pending work.
        // The marker is written first so a crash cannot leave a pending receipt
        // that later pulls have no deterministic way to discover.
        self.write_workspace_invite_profile_finalization(
            &marker_path,
            &PendingWorkspaceInviteProfileFinalization {
                kind: INVITE_PROFILE_FINALIZATION_KIND.to_owned(),
                schema_version: INVITE_PROFILE_FINALIZATION_SCHEMA_VERSION,
                workspace_id: workspace_id.to_owned(),
                invite_id: invite_id.to_owned(),
                request_id: request_id.to_owned(),
            },
        )?;
        receipt.profile_pending = true;
        self.write_workspace_invite_claim_receipt(&receipt_path, &receipt)
    }

    pub(crate) fn finalize_pending_workspace_invite_profile(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Vec<String>, RuntimeError> {
        let _profile_guard = INVITE_PROFILE_FINALIZATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.load_workspace_key(workspace_id)?.is_none() {
            // Response-first and failed-import states may have durable pending
            // intent before the workspace key exists. History alone must never
            // be enough to materialize the joiner's identity.
            return Ok(Vec::new());
        }
        let context = match self.workspace_write_context(workspace_id) {
            Ok(context) => context,
            Err(RuntimeError::WorkspaceHasNoEvents { .. }) => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let local_device_id = self.identity.device_id();
        let Some(active_member_event_id) =
            self.active_invite_membership_event_id(&context, local_device_id)
        else {
            // The response can be imported before its membership history arrives.
            // Keep the durable marker so a later pull can finish the identity.
            return Ok(Vec::new());
        };
        let Some((mut stored_receipt, canonical_claim)) =
            self.active_invite_profile_claim(workspace_id, &context, &active_member_event_id)?
        else {
            return Ok(Vec::new());
        };

        // Existing explicit profile data wins over the invite-time default. This
        // keeps finalization idempotent and avoids reverting a user edit that won
        // a race with the pull.
        let linked_person_profile_name = context
            .state
            .person_device_links
            .get(local_device_id)
            .and_then(|link| context.state.person_profiles.get(&link.person_id))
            .map(|profile| profile.display_name.trim())
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
        let effective_display_name = context
            .state
            .profiles
            .get(local_device_id)
            .map(|profile| profile.display_name.trim())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .or(linked_person_profile_name)
            .unwrap_or(canonical_claim.display_name);

        let mut event_ids = Vec::new();
        let device_profile_missing = context
            .state
            .profiles
            .get(local_device_id)
            .is_none_or(|profile| profile.display_name.trim().is_empty());
        if device_profile_missing {
            let updated =
                self.update_device_profile(workspace_id.clone(), &effective_display_name)?;
            event_ids.push(updated.event_id);
        }

        let refreshed = self.workspace_write_context(workspace_id)?;
        match refreshed.state.person_device_links.get(local_device_id) {
            Some(link)
                if refreshed
                    .state
                    .person_profiles
                    .get(&link.person_id)
                    .is_none_or(|profile| profile.display_name.trim().is_empty()) =>
            {
                let updated = self.update_person_profile(
                    workspace_id.clone(),
                    link.person_id.clone(),
                    &effective_display_name,
                )?;
                if let Some(link_event_id) = updated.link_event_id {
                    event_ids.push(link_event_id);
                }
                event_ids.push(updated.profile_event_id);
            }
            Some(_) => {}
            None => {
                let updated = self
                    .update_local_person_profile(workspace_id.clone(), &effective_display_name)?;
                if let Some(link_event_id) = updated.link_event_id {
                    event_ids.push(link_event_id);
                }
                event_ids.push(updated.profile_event_id);
            }
        }

        stored_receipt.receipt.profile_pending = false;
        stored_receipt.receipt.profile_finalized = true;
        stored_receipt
            .receipt
            .profile_event_ids
            .clone_from(&event_ids);
        self.write_workspace_invite_claim_receipt(&stored_receipt.path, &stored_receipt.receipt)?;
        self.remove_workspace_invite_profile_markers(
            &workspace_id.0,
            &stored_receipt.receipt.invite_id,
            &stored_receipt.receipt.request_id,
        )?;
        Ok(event_ids)
    }

    fn active_invite_profile_claim(
        &self,
        workspace_id: &WorkspaceId,
        context: &WorkspaceWriteContext,
        active_member_event_id: &str,
    ) -> Result<
        Option<(
            StoredWorkspaceInviteClaimReceipt,
            CanonicalWorkspaceInviteClaim,
        )>,
        RuntimeError,
    > {
        let Some((invite_id, request_id)) = self.active_invite_claim_coordinates(
            context,
            active_member_event_id,
            self.identity.device_id(),
        ) else {
            return Ok(None);
        };
        let Some(stored) =
            self.workspace_invite_claim_receipt_stored_optional(&invite_id, &request_id)?
        else {
            return Ok(None);
        };
        let receipt = &stored.receipt;
        if receipt.workspace_id != workspace_id.0
            || receipt.invite_id != invite_id
            || receipt.request_id != request_id
            || receipt.profile_finalized
        {
            return Ok(None);
        }
        let Some(canonical) = self.canonical_invite_claim(receipt, context)? else {
            return Ok(None);
        };
        if canonical.member_event_id != active_member_event_id {
            return Ok(None);
        }
        Ok(Some((stored, canonical)))
    }

    fn active_invite_claim_coordinates(
        &self,
        context: &WorkspaceWriteContext,
        active_member_event_id: &str,
        local_device_id: &DeviceId,
    ) -> Option<(String, String)> {
        let member_event = context
            .events
            .iter()
            .find(|signed| signed.event_id.0 == active_member_event_id)?;
        let EventBody::MemberInvited {
            invitee_device_id, ..
        } = &member_event.event.body
        else {
            return None;
        };
        if invitee_device_id != local_device_id {
            return None;
        }
        let [request_event_id] = member_event.event.parents.as_slice() else {
            return None;
        };
        let request_event = context
            .events
            .iter()
            .find(|signed| signed.event_id == *request_event_id)?;
        let EventBody::WorkspaceJoinRequestRecorded {
            request_id,
            requester_device_id,
            source_type,
            source_invite_id,
            ..
        } = &request_event.event.body
        else {
            return None;
        };
        if requester_device_id != local_device_id
            || source_type != "invite_claim"
            || source_invite_id.is_empty()
            || request_id.is_empty()
        {
            return None;
        }
        Some((source_invite_id.clone(), request_id.clone()))
    }

    fn canonical_invite_claim(
        &self,
        receipt: &WorkspaceInviteClaimReceipt,
        context: &WorkspaceWriteContext,
    ) -> Result<Option<CanonicalWorkspaceInviteClaim>, RuntimeError> {
        let local_device_id = self.identity.device_id();
        let request_events = context
            .events
            .iter()
            .filter(|signed| {
                matches!(
                    &signed.event.body,
                    EventBody::WorkspaceJoinRequestRecorded { request_id, .. }
                        if request_id == &receipt.request_id
                )
            })
            .collect::<Vec<_>>();
        let [request_event] = request_events.as_slice() else {
            // Duplicate request IDs make the materialized projection mutable;
            // never derive identity from whichever duplicate happened to win.
            return Ok(None);
        };
        let EventBody::WorkspaceJoinRequestRecorded {
            requester_device_id,
            display_name,
            source_type,
            source_invite_id,
            source_approval_policy,
            ..
        } = &request_event.event.body
        else {
            unreachable!("filtered to workspace join request events")
        };
        let expected_responder = DeviceId(receipt.expected_responder_device_id.clone());
        let canonical_display_name = display_name.trim();
        if requester_device_id != local_device_id
            || request_event.event.author_device_id != expected_responder
            || source_type != "invite_claim"
            || source_invite_id != &receipt.invite_id
            || source_approval_policy != "preapproved"
            || canonical_display_name.is_empty()
        {
            return Ok(None);
        }
        validate_metadata_field_size(
            "display name",
            canonical_display_name,
            DEVICE_DISPLAY_NAME_MAX_BYTES,
        )?;
        let receipt_display_name = receipt.display_name.trim();
        if !receipt_display_name.is_empty() && receipt_display_name != canonical_display_name {
            // Modern receipts remember the locally signed claim name. The
            // inviter's canonical record must agree exactly after normalization.
            return Ok(None);
        }

        let capability_events = context
            .events
            .iter()
            .filter(|signed| {
                matches!(
                    &signed.event.body,
                    EventBody::WorkspaceInviteCapabilityCreated { invite_id, .. }
                        if invite_id == &receipt.invite_id
                )
            })
            .collect::<Vec<_>>();
        let [capability_event] = capability_events.as_slice() else {
            return Ok(None);
        };
        let EventBody::WorkspaceInviteCapabilityCreated {
            capability_public_key,
            ..
        } = &capability_event.event.body
        else {
            unreachable!("filtered to workspace invite capability events")
        };
        if capability_event.event.author_device_id != expected_responder
            || capability_public_key != &receipt.capability_public_key
        {
            return Ok(None);
        }

        let member_events = context
            .events
            .iter()
            .filter(|signed| {
                matches!(
                    &signed.event.body,
                    EventBody::MemberInvited { invitee_device_id, .. }
                        if invitee_device_id == local_device_id
                            && signed.event.parents.as_slice()
                                == std::slice::from_ref(&request_event.event_id)
                )
            })
            .collect::<Vec<_>>();
        let [member_event] = member_events.as_slice() else {
            return Ok(None);
        };
        if member_event.event.author_device_id != expected_responder {
            return Ok(None);
        }

        let claim_events = context
            .events
            .iter()
            .filter(|signed| {
                matches!(
                    &signed.event.body,
                    EventBody::WorkspaceInviteClaimed { request_id, .. }
                        if request_id == &receipt.request_id
                )
            })
            .collect::<Vec<_>>();
        let [claim_event] = claim_events.as_slice() else {
            return Ok(None);
        };
        let EventBody::WorkspaceInviteClaimed {
            invite_id,
            invitee_device_id,
            request_id,
        } = &claim_event.event.body
        else {
            unreachable!("filtered to workspace invite claim events")
        };
        if invite_id != &receipt.invite_id
            || invitee_device_id != local_device_id
            || request_id != &receipt.request_id
            || claim_event.event.author_device_id != expected_responder
            || claim_event.event.parents.as_slice() != std::slice::from_ref(&member_event.event_id)
        {
            return Ok(None);
        }

        let Some(request) = context.state.join_requests.get(&receipt.request_id) else {
            return Ok(None);
        };
        if request.requested_event_id != request_event.event_id
            || request.requester_device_id != *local_device_id
            || request.requested_by_device_id != expected_responder
            || request.display_name.trim() != canonical_display_name
            || request.source_type != "invite_claim"
            || request.source_invite_id != receipt.invite_id
            || request.source_approval_policy != "preapproved"
            || request.status != WorkspaceJoinRequestStatus::Approved
            || request.resolved_event_id.as_ref() != Some(&member_event.event_id)
            || request.resolved_by_device_id.as_ref() != Some(&expected_responder)
        {
            return Ok(None);
        }

        Ok(Some(CanonicalWorkspaceInviteClaim {
            display_name: canonical_display_name.to_owned(),
            member_event_id: member_event.event_id.0.clone(),
        }))
    }

    fn active_invite_membership_event_id(
        &self,
        context: &WorkspaceWriteContext,
        local_device_id: &DeviceId,
    ) -> Option<String> {
        context
            .state
            .members
            .get(local_device_id)
            .map(|member| member.membership_event_id.0.clone())
    }

    fn invite_response_secret(&self) -> StaticSecret {
        StaticSecret::from(blake3::derive_key(
            RESPONSE_IDENTITY_KEY_CONTEXT,
            &self.identity.signing_key_bytes(),
        ))
    }

    fn workspace_invite_claim_receipt_path(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> std::path::PathBuf {
        let file_id = blake3::hash(format!("{invite_id}\0{request_id}").as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-claims")
            .join(format!("{file_id}.json"))
    }

    fn workspace_invite_response_receipt_path(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> std::path::PathBuf {
        let file_id = blake3::hash(format!("{invite_id}\0{request_id}").as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-response-receipts")
            .join(format!("{file_id}.json"))
    }

    fn workspace_invite_profile_finalization_path(
        &self,
        workspace_id: &str,
        invite_id: &str,
        request_id: &str,
    ) -> std::path::PathBuf {
        let file_id =
            blake3::hash(format!("{workspace_id}\0{invite_id}\0{request_id}").as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-profile-finalization")
            .join(format!("{file_id}.json"))
    }

    fn legacy_workspace_invite_profile_finalization_path(
        &self,
        workspace_id: &str,
    ) -> std::path::PathBuf {
        let file_id = blake3::hash(workspace_id.as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-profile-finalization")
            .join(format!("{file_id}.json"))
    }

    fn legacy_workspace_invite_claim_receipt_path(&self, invite_id: &str) -> std::path::PathBuf {
        let file_id = blake3::hash(invite_id.as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-claims")
            .join(format!("{file_id}.json"))
    }

    fn workspace_invite_claim_receipt(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> Result<WorkspaceInviteClaimReceipt, RuntimeError> {
        self.workspace_invite_claim_receipt_with_path(invite_id, request_id)
            .map(|(receipt, _)| receipt)
    }

    fn workspace_invite_claim_receipt_with_path(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> Result<(WorkspaceInviteClaimReceipt, std::path::PathBuf), RuntimeError> {
        self.workspace_invite_claim_receipt_stored(invite_id, request_id)
            .map(|stored| (stored.receipt, stored.path))
    }

    fn workspace_invite_claim_receipt_stored(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> Result<StoredWorkspaceInviteClaimReceipt, RuntimeError> {
        self.workspace_invite_claim_receipt_stored_optional(invite_id, request_id)?
            .ok_or(RuntimeError::InvalidWorkspaceInviteResponse)
    }

    fn workspace_invite_claim_receipt_stored_optional(
        &self,
        invite_id: &str,
        request_id: &str,
    ) -> Result<Option<StoredWorkspaceInviteClaimReceipt>, RuntimeError> {
        let receipt_path = self.workspace_invite_claim_receipt_path(invite_id, request_id);
        let legacy_path = self.legacy_workspace_invite_claim_receipt_path(invite_id);
        let (bytes, path) = match read_local_metadata_file_with_limit(
            &receipt_path,
            INVITE_CLAIM_RECEIPT_MAX_BYTES,
            "workspace invite claim receipt",
        )? {
            Some(bytes) => (bytes, receipt_path),
            None => match read_local_metadata_file_with_limit(
                &legacy_path,
                INVITE_CLAIM_RECEIPT_MAX_BYTES,
                "workspace invite claim receipt",
            )? {
                Some(bytes) => (bytes, legacy_path),
                None => return Ok(None),
            },
        };
        self.parse_workspace_invite_claim_receipt(&bytes, path)
            .map(Some)
    }

    fn parse_workspace_invite_claim_receipt(
        &self,
        bytes: &[u8],
        path: std::path::PathBuf,
    ) -> Result<StoredWorkspaceInviteClaimReceipt, RuntimeError> {
        let receipt = serde_json::from_slice(bytes)
            .map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)?;
        Ok(StoredWorkspaceInviteClaimReceipt { receipt, path })
    }

    fn write_workspace_invite_claim_receipt(
        &self,
        path: &Path,
        receipt: &WorkspaceInviteClaimReceipt,
    ) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec(receipt)?;
        if bytes.len().saturating_add(1) > INVITE_CLAIM_RECEIPT_MAX_BYTES {
            return Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace invite claim receipt",
                actual_bytes: bytes.len().saturating_add(1),
                max_bytes: INVITE_CLAIM_RECEIPT_MAX_BYTES,
            });
        }
        write_secret_file(path, &bytes)
    }

    fn remove_workspace_invite_profile_markers(
        &self,
        workspace_id: &str,
        invite_id: &str,
        request_id: &str,
    ) -> Result<(), RuntimeError> {
        remove_local_marker_file(&self.workspace_invite_profile_finalization_path(
            workspace_id,
            invite_id,
            request_id,
        ))?;
        // The old implementation keyed one marker by workspace. Only remove it
        // when its embedded coordinates match the claim we just finalized.
        let legacy_path = self.legacy_workspace_invite_profile_finalization_path(workspace_id);
        if let Some(bytes) = read_local_metadata_file_with_limit(
            &legacy_path,
            INVITE_PROFILE_FINALIZATION_MAX_BYTES,
            "workspace invite profile finalization",
        )? && let Ok(marker) =
            serde_json::from_slice::<PendingWorkspaceInviteProfileFinalization>(&bytes)
            && marker.workspace_id == workspace_id
            && marker.invite_id == invite_id
            && marker.request_id == request_id
        {
            remove_local_marker_file(&legacy_path)?;
        }
        Ok(())
    }

    fn write_workspace_invite_profile_finalization(
        &self,
        path: &Path,
        marker: &PendingWorkspaceInviteProfileFinalization,
    ) -> Result<(), RuntimeError> {
        let bytes = serde_json::to_vec(marker)?;
        if bytes.len().saturating_add(1) > INVITE_PROFILE_FINALIZATION_MAX_BYTES {
            return Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace invite profile finalization",
                actual_bytes: bytes.len().saturating_add(1),
                max_bytes: INVITE_PROFILE_FINALIZATION_MAX_BYTES,
            });
        }
        write_secret_file(path, &bytes)
    }
}

fn remove_local_marker_file(path: &Path) -> Result<(), RuntimeError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_workspace_invite_claim_record_fields(
    claim: &WorkspaceInviteClaim,
    invitee_device_id: &DeviceId,
) -> Result<(), RuntimeError> {
    validate_device_id_reference(invitee_device_id)?;
    if claim.payload.display_name.trim().is_empty() {
        return Err(RuntimeError::DisplayNameRequired);
    }
    validate_metadata_field_size(
        "join request ID",
        &claim.payload.request_id,
        WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "display name",
        claim.payload.display_name.trim(),
        DEVICE_DISPLAY_NAME_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "join request note",
        claim.payload.note.trim(),
        WORKSPACE_JOIN_REQUEST_NOTE_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "join request source",
        &claim.payload.source_type,
        WORKSPACE_ACCESS_POLICY_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "source invite ID",
        &claim.payload.source_invite_id,
        WORKSPACE_INVITE_ID_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "source display name",
        claim.payload.source_display_name.trim(),
        DEVICE_DISPLAY_NAME_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "source approval policy",
        &claim.payload.source_approval_policy,
        WORKSPACE_INVITE_APPROVAL_POLICY_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "join request response peer endpoint",
        claim.payload.response_peer_endpoint.trim(),
        PEER_ENDPOINT_MAX_BYTES,
    )?;
    Ok(())
}

fn validate_invite_artifact(artifact: &WorkspaceInviteArtifact) -> Result<(), RuntimeError> {
    let max_claims = effective_workspace_invite_max_claims(artifact.max_claims);
    if artifact.kind != WORKSPACE_INVITE_ARTIFACT_KIND
        || artifact.schema_version != WORKSPACE_INVITE_SCHEMA_VERSION
        || artifact.workspace_id.is_empty()
        || artifact.invite_id.is_empty()
        || artifact.capability_secret.len() != CAPABILITY_SECRET_BYTES * 2
        || max_claims > WORKSPACE_INVITE_MAX_CLAIMS
    {
        return Err(RuntimeError::InvalidWorkspaceInviteClaim);
    }
    validate_workspace_id_reference(&WorkspaceId(artifact.workspace_id.clone()))?;
    validate_metadata_field_size(
        "invite ID",
        &artifact.invite_id,
        WORKSPACE_INVITE_ID_MAX_BYTES,
    )?;
    validate_metadata_field_size(
        "invite label",
        artifact.invite_label(),
        WORKSPACE_INVITE_LABEL_MAX_BYTES,
    )?;
    if invite_is_expired(&artifact.expires_at)? {
        return Err(RuntimeError::WorkspaceInviteExpired {
            invite_id: artifact.invite_id.clone(),
        });
    }
    let inviter_public_key = decode_hex_32(&artifact.inviter_public_key)
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
    let inviter_signature = decode_hex(&artifact.inviter_signature)
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
    verify_device_detached_signature(
        &DeviceId(artifact.inviter_device_id.clone()),
        &inviter_public_key,
        &workspace_invite_artifact_signing_bytes(artifact)?,
        &inviter_signature,
    )
    .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
    Ok(())
}

fn workspace_invite_artifact_signing_bytes(
    artifact: &WorkspaceInviteArtifact,
) -> Result<Vec<u8>, RuntimeError> {
    let mut unsigned = artifact.clone();
    unsigned.inviter_signature.clear();
    Ok(serde_json::to_vec(&unsigned)?)
}

fn invite_is_expired(expires_at: &str) -> Result<bool, RuntimeError> {
    if expires_at.trim().is_empty() {
        return Ok(false);
    }
    let expires = OffsetDateTime::parse(expires_at, &Rfc3339)
        .map_err(|_| RuntimeError::InvalidWorkspaceInviteClaim)?;
    Ok(expires <= OffsetDateTime::now_utc())
}

fn response_content_key(shared: &[u8; 32]) -> ContentKey {
    ContentKey::from_bytes(blake3::derive_key(RESPONSE_KEY_CONTEXT, shared))
}

fn response_aad(workspace_id: &str, invite_id: &str, device_id: &str, request_id: &str) -> Vec<u8> {
    format!(
        "chaft:v1:workspace-invite-response:{workspace_id}:{invite_id}:{device_id}:{request_id}"
    )
    .into_bytes()
}

fn generated_id(prefix: &str, entropy: &[u8]) -> String {
    let hash = blake3::hash(entropy).to_hex().to_string();
    format!("{prefix}_{}", &hash[..32])
}

fn random_bytes() -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn current_timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_default()
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], ()> {
    decode_hex(value)?.try_into().map_err(|_| ())
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2) {
        return Err(());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Ok((decode_nibble(pair[0])? << 4) | decode_nibble(pair[1])?))
        .collect()
}

fn decode_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::Duration as StdDuration,
    };

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn failed_key_import_keeps_pending_intent_but_history_alone_cannot_finalize_profile() {
        let owner_dir = tempdir().unwrap();
        let joiner_dir = tempdir().unwrap();
        let owner = LocalRuntime::open(owner_dir.path(), None).unwrap();
        let joiner = LocalRuntime::open(joiner_dir.path(), None).unwrap();
        let created = owner
            .create_workspace("Failed key import", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = owner
            .create_workspace_invite(
                workspace_id.clone(),
                "Failed key import".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let expires_at = invite.artifact.expires_at.clone();
        let claim = joiner
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Pending Joiner".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let retained_claim = claim.clone();
        owner.claim_workspace_invite(claim).unwrap();
        for event in owner.workspace_write_context(&workspace_id).unwrap().events {
            joiner.store.append_event(&event).unwrap();
        }

        let invalid_response = owner
            .seal_workspace_invite_response(
                &retained_claim,
                WorkspaceRole::Member,
                &expires_at,
                WorkspaceKeyExport {
                    schema_version: crate::CONTENT_KEY_EXPORT_SCHEMA_VERSION,
                    workspace_id: workspace_id.0.clone(),
                    epoch: 1,
                    key_id: "invalid-key-id".to_owned(),
                    exporter_device_id: owner.identity.device_id().0.clone(),
                    aes_256_gcm_siv_key: vec![0; 32],
                    previous_keys: Vec::new(),
                },
            )
            .unwrap();
        assert!(matches!(
            joiner.import_workspace_invite_response(invalid_response),
            Err(RuntimeError::InvalidWorkspaceKey)
        ));

        let receipt = joiner
            .workspace_invite_claim_receipt(
                &retained_claim.payload.invite_id,
                &retained_claim.payload.request_id,
            )
            .unwrap();
        assert!(receipt.profile_pending);
        assert!(!receipt.profile_finalized);
        assert!(
            joiner
                .workspace_invite_profile_finalization_path(
                    &workspace_id.0,
                    &retained_claim.payload.invite_id,
                    &retained_claim.payload.request_id,
                )
                .is_file()
        );
        assert!(joiner.load_workspace_key(&workspace_id).unwrap().is_none());
        assert!(
            joiner
                .finalize_pending_workspace_invite_profile(&workspace_id)
                .unwrap()
                .is_empty()
        );
        let snapshot = joiner.workspace_snapshot(workspace_id).unwrap();
        assert!(snapshot.profiles.is_empty());
        assert!(snapshot.person_profiles.is_empty());
        assert!(snapshot.person_device_links.is_empty());
    }

    #[test]
    fn pending_invite_profile_repairs_blank_existing_device_and_person_profiles() {
        let owner_dir = tempdir().unwrap();
        let joiner_dir = tempdir().unwrap();
        let owner = LocalRuntime::open(owner_dir.path(), None).unwrap();
        let joiner = LocalRuntime::open(joiner_dir.path(), None).unwrap();
        let created = owner.create_workspace("Blank profiles", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = owner
            .create_workspace_invite(
                workspace_id.clone(),
                "Blank profile repair".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claim = joiner
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Canonical Invite Name".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claimed = owner.claim_workspace_invite(claim).unwrap();
        joiner
            .import_workspace_invite_response(claimed.response)
            .unwrap();
        for event in owner.workspace_write_context(&workspace_id).unwrap().events {
            joiner.store.append_event(&event).unwrap();
        }

        let person_id = chaft_types::PersonId::new();
        let avatar_id = "relay-v1:g07:p06:c05";
        joiner
            .update_device_profile_with_avatar(
                workspace_id.clone(),
                "Temporary Device Name",
                avatar_id,
            )
            .unwrap();
        joiner
            .update_person_profile_with_avatar(
                workspace_id.clone(),
                person_id.clone(),
                "Temporary Name",
                avatar_id,
            )
            .unwrap();
        let context = joiner.workspace_write_context(&workspace_id).unwrap();
        let mut blank_device = SignableEvent::new(
            workspace_id.clone(),
            None,
            joiner.identity.device_id().clone(),
            EventBody::DeviceProfileUpdated {
                display_name: "   ".to_owned(),
                avatar_id: String::new(),
            },
        );
        blank_device.parents = context.head_event_ids;
        let blank_device = joiner
            .sign_authorize_and_append_with_history(blank_device, &context.events)
            .unwrap();
        let mut history = context.events;
        history.push(blank_device.clone());
        let mut blank_person = SignableEvent::new(
            workspace_id.clone(),
            None,
            joiner.identity.device_id().clone(),
            EventBody::PersonProfileUpdated {
                person_id: person_id.clone(),
                display_name: "\t".to_owned(),
                avatar_id: String::new(),
            },
        );
        blank_person.parents = vec![blank_device.event_id];
        joiner
            .sign_authorize_and_append_with_history(blank_person, &history)
            .unwrap();

        let before = joiner.workspace_write_context(&workspace_id).unwrap();
        assert!(
            before
                .state
                .profiles
                .get(joiner.identity.device_id())
                .is_some_and(|profile| profile.display_name.trim().is_empty())
        );
        assert!(
            before
                .state
                .person_profiles
                .get(&person_id)
                .is_some_and(|profile| profile.display_name.trim().is_empty())
        );

        let repaired = joiner
            .finalize_pending_workspace_invite_profile(&workspace_id)
            .unwrap();
        assert_eq!(repaired.len(), 2);
        let after = joiner.workspace_write_context(&workspace_id).unwrap();
        assert_eq!(
            after
                .state
                .profiles
                .get(joiner.identity.device_id())
                .map(|profile| profile.display_name.as_str()),
            Some("Canonical Invite Name")
        );
        assert_eq!(
            after
                .state
                .profiles
                .get(joiner.identity.device_id())
                .map(|profile| profile.avatar_id.as_str()),
            Some(avatar_id)
        );
        assert_eq!(
            after
                .state
                .person_profiles
                .get(&person_id)
                .map(|profile| profile.avatar_id.as_str()),
            Some(avatar_id)
        );
        assert_eq!(
            after
                .state
                .person_profiles
                .get(&person_id)
                .map(|profile| profile.display_name.as_str()),
            Some("Canonical Invite Name")
        );
    }

    #[test]
    fn claimable_invite_grants_membership_only_after_a_signed_claim() {
        let alice_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Claimable", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id.clone());

        let invite = alice
            .create_workspace_invite(
                workspace_id.clone(),
                "  Design team  ".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                "history_after_claim".to_owned(),
            )
            .unwrap();
        let before_claim = alice.workspace_write_context(&workspace_id).unwrap();
        assert_eq!(before_claim.state.members.len(), 1);
        let pending = before_claim.state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(pending.status, WorkspaceInviteStatus::Invited);
        assert!(pending.invitee_device_id.0.is_empty());
        assert_eq!(pending.display_name, "Design team");
        assert_eq!(pending.invite_label, "Design team");
        assert_eq!(pending.invitee_display_name, None);
        assert_eq!(invite.artifact.invite_label(), "Design team");
        let artifact_json = serde_json::to_string(&invite.artifact).unwrap();
        let artifact_value = serde_json::from_str::<serde_json::Value>(&artifact_json).unwrap();
        assert_eq!(artifact_value["displayName"], "Design team");
        assert!(artifact_value.get("inviteLabel").is_none());
        assert!(!artifact_json.contains("workspaceKey"));
        assert!(!artifact_json.contains("aes256GcmSivKey"));

        let claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claimed = alice.claim_workspace_invite(claim).unwrap();
        assert_eq!(claimed.invitee_device_id, bob.identity.device_id().0);

        let after_claim = alice.workspace_write_context(&workspace_id).unwrap();
        assert!(
            after_claim
                .state
                .members
                .contains_key(bob.identity.device_id())
        );
        let accepted = after_claim.state.invites.get(&claimed.invite_id).unwrap();
        assert_eq!(accepted.status, WorkspaceInviteStatus::Accepted);
        assert_eq!(accepted.invitee_device_id, *bob.identity.device_id());
        assert_eq!(accepted.display_name, "Design team");
        assert_eq!(accepted.invite_label, "Design team");
        assert_eq!(accepted.invitee_display_name, None);

        let imported = bob
            .import_workspace_invite_response(claimed.response)
            .unwrap();
        assert_eq!(imported.workspace_id, workspace_id.0);
        assert_eq!(imported.importer_device_id, bob.identity.device_id().0);
    }

    #[test]
    fn claimable_invite_is_single_use_and_response_is_device_bound() {
        let alice_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = alice.create_workspace("Single use", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = alice
            .create_workspace_invite(
                workspace_id.clone(),
                String::new(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_eq!(invite.artifact.max_claims, Some(1));
        let bob_claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claimed = alice.claim_workspace_invite(bob_claim).unwrap();
        assert!(matches!(
            charlie.import_workspace_invite_response(claimed.response.clone()),
            Err(RuntimeError::InvalidWorkspaceInviteResponse)
        ));

        let charlie_claim = charlie
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert!(matches!(
            alice.claim_workspace_invite(charlie_claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));
        let state = alice.workspace_write_context(&workspace_id).unwrap().state;
        let exhausted = state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(exhausted.max_claims, 1);
        assert_eq!(exhausted.claim_count, 1);
    }

    #[test]
    fn bounded_invite_admits_two_devices_and_replays_an_older_claim() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let dana_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let dana = LocalRuntime::open(dana_dir.path(), None).unwrap();
        let created = admin.create_workspace("Bounded invite", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                "Launch team".to_owned(),
                WorkspaceRole::Member,
                2,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_eq!(invite.artifact.max_claims, Some(2));
        let bob_claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let bob_claimed = admin.claim_workspace_invite(bob_claim.clone()).unwrap();
        let after_bob = admin.workspace_write_context(&workspace_id).unwrap().state;
        let partially_used = after_bob.invites.get(&invite.invite_id).unwrap();
        assert_eq!(partially_used.status, WorkspaceInviteStatus::Invited);
        assert_eq!(partially_used.claim_count, 1);
        assert_eq!(partially_used.max_claims, 2);

        let charlie_claim = charlie
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let charlie_claimed = admin.claim_workspace_invite(charlie_claim).unwrap();
        let dana_claim = dana
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Dana".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert!(matches!(
            admin.claim_workspace_invite(dana_claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));

        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        let exhausted = state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(exhausted.status, WorkspaceInviteStatus::Accepted);
        assert_eq!(exhausted.claim_count, 2);
        assert_eq!(state.members.len(), 3);
        assert!(state.members.contains_key(bob.identity.device_id()));
        assert!(state.members.contains_key(charlie.identity.device_id()));
        assert!(!state.members.contains_key(dana.identity.device_id()));
        assert_eq!(bob_claimed.response.payload.role, WorkspaceRole::Member);
        assert_eq!(charlie_claimed.response.payload.role, WorkspaceRole::Member);

        drop(admin);
        let reopened = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob_retry = reopened.claim_workspace_invite(bob_claim).unwrap();
        assert_eq!(bob_retry.claim_event_id, bob_claimed.claim_event_id);
        assert_eq!(bob_retry.member_event_id, bob_claimed.member_event_id);
        assert_eq!(bob_retry.response, bob_claimed.response);
    }

    #[test]
    fn bounded_invite_admits_only_two_of_three_simultaneous_claims() {
        let admin_dir = tempdir().unwrap();
        let invitee_dirs = [tempdir().unwrap(), tempdir().unwrap(), tempdir().unwrap()];
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitees = invitee_dirs
            .iter()
            .map(|dir| LocalRuntime::open(dir.path(), None).unwrap())
            .collect::<Vec<_>>();
        let created = admin
            .create_workspace("Concurrent claims", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                "Launch team".to_owned(),
                WorkspaceRole::Member,
                2,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claims = invitees
            .iter()
            .enumerate()
            .map(|(index, invitee)| {
                invitee
                    .prepare_workspace_invite_claim(
                        invite.artifact.clone(),
                        format!("Invitee {index}"),
                        String::new(),
                        String::new(),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let admin_runtimes = (0..claims.len())
            .map(|_| LocalRuntime::open(admin_dir.path(), None).unwrap())
            .collect::<Vec<_>>();
        let barrier = Arc::new(Barrier::new(claims.len() + 1));
        let handles = admin_runtimes
            .into_iter()
            .zip(claims)
            .map(|(runtime, claim)| {
                let barrier = barrier.clone();
                thread::spawn(move || {
                    barrier.wait();
                    runtime.claim_workspace_invite(claim)
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 2);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
                ))
                .count(),
            1
        );
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        let exhausted = state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(exhausted.claim_count, 2);
        assert_eq!(state.members.len(), 3);
        assert_eq!(state.join_requests.len(), 2);
    }

    #[test]
    fn atomic_claim_batch_failure_leaves_no_member_request_or_consumed_claim() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin.create_workspace("Atomic claim", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite(
                workspace_id.clone(),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let connection = rusqlite::Connection::open(&admin.paths().event_store).unwrap();
        connection
            .execute_batch(
                "
                CREATE TRIGGER fail_workspace_invite_claim_insert
                BEFORE INSERT ON events
                WHEN CAST(NEW.event_json AS TEXT) LIKE '%workspace_invite_claimed%'
                BEGIN
                    SELECT RAISE(ABORT, 'injected claim append failure');
                END;
                ",
            )
            .unwrap();
        drop(connection);

        assert!(matches!(
            admin.claim_workspace_invite(claim),
            Err(RuntimeError::Store(StoreError::Sqlite(_)))
        ));
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        assert_eq!(state.members.len(), 1);
        assert!(state.join_requests.is_empty());
        let open = state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(open.claim_count, 0);
        assert_eq!(open.status, WorkspaceInviteStatus::Invited);
    }

    #[test]
    fn accepted_claim_replay_survives_later_revocation_and_expiry() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Replay recovery", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        let revoked_invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                "Revoked later".to_owned(),
                WorkspaceRole::Member,
                2,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let bob_claim = bob
            .prepare_workspace_invite_claim(
                revoked_invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let bob_first = admin.claim_workspace_invite(bob_claim.clone()).unwrap();
        admin
            .resolve_workspace_invite(
                workspace_id.clone(),
                revoked_invite.invite_id,
                chaft_types::WorkspaceInviteResolution::Revoked,
            )
            .unwrap();
        let bob_retry = admin.claim_workspace_invite(bob_claim).unwrap();
        assert_eq!(bob_retry.member_event_id, bob_first.member_event_id);
        assert_eq!(bob_retry.claim_event_id, bob_first.claim_event_id);
        assert_eq!(bob_retry.response, bob_first.response);

        let expires_at = (OffsetDateTime::now_utc() + time::Duration::seconds(1))
            .format(&Rfc3339)
            .unwrap();
        let expiring_invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id,
                "Expires later".to_owned(),
                WorkspaceRole::Member,
                2,
                expires_at,
                String::new(),
                String::new(),
            )
            .unwrap();
        let charlie_claim = charlie
            .prepare_workspace_invite_claim(
                expiring_invite.artifact,
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let charlie_first = admin.claim_workspace_invite(charlie_claim.clone()).unwrap();
        thread::sleep(StdDuration::from_millis(1_100));
        let charlie_retry = admin.claim_workspace_invite(charlie_claim).unwrap();
        assert_eq!(charlie_retry.member_event_id, charlie_first.member_event_id);
        assert_eq!(charlie_retry.claim_event_id, charlie_first.claim_event_id);
        assert_eq!(charlie_retry.response, charlie_first.response);
    }

    #[test]
    fn legacy_artifact_without_max_claims_keeps_one_use_signing_semantics() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = admin.create_workspace("Legacy invite", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite(
                workspace_id,
                String::new(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let mut legacy_artifact = invite.artifact;
        legacy_artifact.max_claims = None;
        legacy_artifact.inviter_signature = encode_hex(
            &admin
                .identity
                .sign_bytes(&workspace_invite_artifact_signing_bytes(&legacy_artifact).unwrap()),
        );
        let legacy_signing_bytes =
            workspace_invite_artifact_signing_bytes(&legacy_artifact).unwrap();
        let legacy_json = serde_json::to_string(&legacy_artifact).unwrap();
        assert!(!legacy_json.contains("maxClaims"));
        let round_tripped = serde_json::from_str::<WorkspaceInviteArtifact>(&legacy_json).unwrap();
        assert_eq!(round_tripped.max_claims, None);
        assert_eq!(
            workspace_invite_artifact_signing_bytes(&round_tripped).unwrap(),
            legacy_signing_bytes
        );

        let bob_claim = bob
            .prepare_workspace_invite_claim(
                round_tripped.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        admin.claim_workspace_invite(bob_claim).unwrap();
        let charlie_claim = charlie
            .prepare_workspace_invite_claim(
                round_tripped,
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert!(matches!(
            admin.claim_workspace_invite(charlie_claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));
    }

    const FROZEN_PRE_LABEL_INVITE_ARTIFACT_JSON: &str = concat!(
        r#"{"kind":"chaft.workspace-invite.v2","schemaVersion":2,"workspaceId":"wrk_fixture_legacy","workspaceName":"Fixture workspace","inviteId":"inv_fixture_legacy","displayName":"Design team","role":"member","expiresAt":"","#,
        r#""capabilitySecret":"0909090909090909090909090909090909090909090909090909090909090909","capabilityPublicKey":"fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618","#,
        r#""inviterDeviceId":"dev_0871f3aabc26e4582c508af5c03884e6a96f0989d1dd8cfb49cd17ed25792433","inviterDisplayName":"Fixture Admin","inviterPublicKey":"ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c","#,
        r#""inviterSignature":"1299a633b2be13f226584ee60bcddcfc155abdb3e60b8e3d9da453433bfd08cb91590488f91849fdef907232400b99e8d66e2139531a468144cc25574e4dde0e","peerEndpoint":"","syncExpectation":"history_after_claim","createdAt":"2025-01-02T03:04:05Z"}"#,
    );

    #[test]
    fn frozen_pre_label_signed_artifact_still_deserializes_and_validates() {
        let artifact =
            serde_json::from_str::<WorkspaceInviteArtifact>(FROZEN_PRE_LABEL_INVITE_ARTIFACT_JSON)
                .unwrap();

        assert_eq!(artifact.display_name, "Design team");
        assert_eq!(artifact.invite_label(), "Design team");
        assert_eq!(artifact.max_claims, None);
        assert!(FROZEN_PRE_LABEL_INVITE_ARTIFACT_JSON.contains("\"displayName\""));
        assert!(!FROZEN_PRE_LABEL_INVITE_ARTIFACT_JSON.contains("inviteLabel"));
        assert_eq!(
            serde_json::to_string(&artifact).unwrap(),
            FROZEN_PRE_LABEL_INVITE_ARTIFACT_JSON
        );
        validate_invite_artifact(&artifact).unwrap();

        let mut tampered = artifact;
        tampered.display_name = "Different label".to_owned();
        assert!(matches!(
            validate_invite_artifact(&tampered),
            Err(RuntimeError::InvalidWorkspaceInviteClaim)
        ));
    }

    #[test]
    fn signed_invite_artifact_rejects_an_oversized_invite_label() {
        let admin_dir = tempdir().unwrap();
        let joiner_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let joiner = LocalRuntime::open(joiner_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Oversized invite label", "general")
            .unwrap();
        let mut artifact = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                String::new(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap()
            .artifact;
        artifact.display_name = "x".repeat(WORKSPACE_INVITE_LABEL_MAX_BYTES + 1);
        artifact.inviter_signature = encode_hex(
            &admin
                .identity
                .sign_bytes(&workspace_invite_artifact_signing_bytes(&artifact).unwrap()),
        );

        let error = joiner
            .prepare_workspace_invite_claim(
                artifact,
                "Joiner".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::MetadataFieldTooLarge {
                field: "invite label",
                ..
            }
        ));
    }

    #[test]
    fn bounded_invite_rejects_device_and_request_id_collisions() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Claim collisions", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                String::new(),
                WorkspaceRole::Member,
                2,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let first_bob_claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let second_bob_claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let used_request_id = first_bob_claim.payload.request_id.clone();
        admin.claim_workspace_invite(first_bob_claim).unwrap();
        assert!(matches!(
            admin.claim_workspace_invite(second_bob_claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));

        let capability = InvitationCapability::from_secret_bytes(
            decode_hex_32(&invite.artifact.capability_secret).unwrap(),
        );
        let mut colliding_charlie_claim = charlie
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        colliding_charlie_claim.payload.request_id = used_request_id;
        let signing_bytes = serde_json::to_vec(&colliding_charlie_claim.payload).unwrap();
        colliding_charlie_claim.device_signature =
            encode_hex(&charlie.identity.sign_bytes(&signing_bytes));
        colliding_charlie_claim.capability_signature = encode_hex(&capability.sign(&signing_bytes));
        assert!(matches!(
            admin.claim_workspace_invite(colliding_charlie_claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));

        let valid_charlie_claim = charlie
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        admin.claim_workspace_invite(valid_charlie_claim).unwrap();
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        let exhausted = state.invites.get(&invite.invite_id).unwrap();
        assert_eq!(exhausted.claim_count, 2);
        assert_eq!(state.members.len(), 3);
    }

    #[test]
    fn partially_used_bounded_invite_cannot_be_claimed_after_revocation() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = admin.create_workspace("Revoked invite", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                String::new(),
                WorkspaceRole::Member,
                2,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let bob_claim = bob
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let charlie_claim = charlie
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Charlie".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        admin.claim_workspace_invite(bob_claim).unwrap();
        admin
            .resolve_workspace_invite(
                workspace_id.clone(),
                invite.invite_id.clone(),
                chaft_types::WorkspaceInviteResolution::Revoked,
            )
            .unwrap();

        assert!(matches!(
            admin.claim_workspace_invite(charlie_claim),
            Err(RuntimeError::WorkspaceInviteNotClaimable { .. })
        ));
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        assert_eq!(state.invites.get(&invite.invite_id).unwrap().claim_count, 1);
        assert!(!state.members.contains_key(charlie.identity.device_id()));
    }

    #[test]
    fn workspace_invite_claim_limit_accepts_100_rejects_101_and_defaults_to_one() {
        assert_eq!(WORKSPACE_INVITE_MAX_CLAIMS, 100);

        let admin_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let created = admin.create_workspace("Invite limits", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);

        let defaulted = admin
            .create_workspace_invite(
                workspace_id.clone(),
                String::new(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_eq!(defaulted.artifact.max_claims, Some(1));

        let maximum = admin
            .create_workspace_invite_with_max_claims(
                workspace_id.clone(),
                String::new(),
                WorkspaceRole::Member,
                WORKSPACE_INVITE_MAX_CLAIMS,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_eq!(
            maximum.artifact.max_claims,
            Some(WORKSPACE_INVITE_MAX_CLAIMS)
        );

        let excessive = admin.create_workspace_invite_with_max_claims(
            workspace_id.clone(),
            String::new(),
            WorkspaceRole::Member,
            WORKSPACE_INVITE_MAX_CLAIMS + 1,
            String::new(),
            String::new(),
            String::new(),
        );
        assert!(matches!(
            excessive,
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "workspace invite claims",
                actual_bytes,
                max_bytes,
            }) if actual_bytes == (WORKSPACE_INVITE_MAX_CLAIMS + 1) as usize
                && max_bytes == WORKSPACE_INVITE_MAX_CLAIMS as usize
        ));
        let normalized = admin
            .create_workspace_invite_with_max_claims(
                workspace_id,
                String::new(),
                WorkspaceRole::Member,
                0,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_eq!(normalized.artifact.max_claims, Some(1));
    }

    #[test]
    fn preparing_a_retry_does_not_overwrite_the_original_claim_receipt() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin.create_workspace("Retry receipt", "general").unwrap();
        let invite = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let first_claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact.clone(),
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let retry_claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        assert_ne!(
            first_claim.payload.request_id,
            retry_claim.payload.request_id
        );

        let claimed = admin.claim_workspace_invite(first_claim).unwrap();
        let imported = invitee
            .import_workspace_invite_response(claimed.response)
            .unwrap();
        assert_eq!(imported.request_id, claimed.request_id);
    }

    #[test]
    fn accepted_claim_retry_returns_the_identical_response_after_reopen() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Stable response", "general")
            .unwrap();
        let invite = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();

        let first = admin.claim_workspace_invite(claim.clone()).unwrap();
        let first_bytes = serde_json::to_vec(&first.response).unwrap();
        let immediate_retry = admin.claim_workspace_invite(claim.clone()).unwrap();
        assert_eq!(immediate_retry.response, first.response);
        assert_eq!(
            serde_json::to_vec(&immediate_retry.response).unwrap(),
            first_bytes
        );

        drop(admin);
        let reopened = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let reopened_retry = reopened.claim_workspace_invite(claim).unwrap();
        assert_eq!(reopened_retry.response, first.response);
        assert_eq!(
            serde_json::to_vec(&reopened_retry.response).unwrap(),
            first_bytes
        );
    }

    #[test]
    fn accepted_claim_without_a_response_receipt_is_stabilized_once() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Recovered response", "general")
            .unwrap();
        let invite = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        admin.claim_workspace_invite(claim.clone()).unwrap();
        let receipt_path = admin.workspace_invite_response_receipt_path(
            &claim.payload.invite_id,
            &claim.payload.request_id,
        );
        std::fs::remove_file(&receipt_path).unwrap();

        let recovered = admin.claim_workspace_invite(claim.clone()).unwrap();
        assert!(receipt_path.is_file());
        drop(admin);

        let reopened = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let retry = reopened.claim_workspace_invite(claim).unwrap();
        assert_eq!(retry.response, recovered.response);
    }

    #[test]
    fn accepted_claim_retry_is_denied_after_removal_even_if_the_device_is_reinvited() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Removed invitee", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let invite = admin
            .create_workspace_invite(
                workspace_id.clone(),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap();
        let claim = invitee
            .prepare_workspace_invite_claim(
                invite.artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();
        admin.claim_workspace_invite(claim.clone()).unwrap();
        let receipt_path = admin.workspace_invite_response_receipt_path(
            &claim.payload.invite_id,
            &claim.payload.request_id,
        );
        std::fs::remove_file(&receipt_path).unwrap();
        admin
            .remove_member(workspace_id.clone(), invitee.identity.device_id().clone())
            .unwrap();

        assert!(matches!(
            admin.claim_workspace_invite(claim.clone()),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));
        admin
            .invite_member(
                workspace_id,
                invitee.identity.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        assert!(matches!(
            admin.claim_workspace_invite(claim),
            Err(RuntimeError::WorkspaceInviteAlreadyClaimed { .. })
        ));
        assert!(!receipt_path.exists());
    }

    #[test]
    fn two_distinct_invites_add_two_devices_to_one_workspace() {
        let admin_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let charlie_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let charlie = LocalRuntime::open(charlie_dir.path(), None).unwrap();
        let created = admin.create_workspace("Three people", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let mut invite_ids = Vec::new();

        for (invitee, display_name) in [(&bob, "Bob"), (&charlie, "Charlie")] {
            let invite = admin
                .create_workspace_invite(
                    workspace_id.clone(),
                    display_name.to_owned(),
                    WorkspaceRole::Member,
                    String::new(),
                    String::new(),
                    "history_after_claim".to_owned(),
                )
                .unwrap();
            invite_ids.push(invite.invite_id.clone());
            let claim = invitee
                .prepare_workspace_invite_claim(
                    invite.artifact,
                    display_name.to_owned(),
                    String::new(),
                    String::new(),
                )
                .unwrap();
            let claimed = admin.claim_workspace_invite(claim).unwrap();
            let imported = invitee
                .import_workspace_invite_response(claimed.response)
                .unwrap();
            assert_eq!(imported.workspace_id, workspace_id.0);
            assert_eq!(imported.importer_device_id, invitee.identity.device_id().0);
        }

        assert_ne!(invite_ids[0], invite_ids[1]);
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        assert_eq!(state.members.len(), 3);
        assert!(state.members.contains_key(bob.identity.device_id()));
        assert!(state.members.contains_key(charlie.identity.device_id()));
        for invite_id in invite_ids {
            assert_eq!(
                state.invites.get(&invite_id).unwrap().status,
                WorkspaceInviteStatus::Accepted
            );
        }
    }

    #[test]
    fn claimable_invite_rejects_tampered_admin_routing_metadata() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin.create_workspace("Signed invite", "general").unwrap();
        let mut artifact = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap()
            .artifact;
        artifact.inviter_device_id = invitee.identity.device_id().0.clone();

        assert!(matches!(
            invitee.prepare_workspace_invite_claim(
                artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            ),
            Err(RuntimeError::InvalidWorkspaceInviteClaim)
        ));
    }

    #[test]
    fn claimable_invite_claim_is_bound_to_its_delivery_device() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin.create_workspace("Bound inviter", "general").unwrap();
        let artifact = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap()
            .artifact;
        let capability_secret = decode_hex_32(&artifact.capability_secret).unwrap();
        let capability = InvitationCapability::from_secret_bytes(capability_secret);
        let mut claim = invitee
            .prepare_workspace_invite_claim(
                artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();

        claim.payload.delivery_device_id = invitee.identity.device_id().0.clone();
        let signing_bytes = serde_json::to_vec(&claim.payload).unwrap();
        claim.device_signature = encode_hex(&invitee.identity.sign_bytes(&signing_bytes));
        claim.capability_signature = encode_hex(&capability.sign(&signing_bytes));

        assert!(matches!(
            admin.claim_workspace_invite(claim),
            Err(RuntimeError::InvalidWorkspaceInviteClaim)
        ));
    }

    #[test]
    fn malformed_response_key_does_not_consume_invite_or_add_member() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let created = admin
            .create_workspace("Reject malformed response key", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let artifact = admin
            .create_workspace_invite(
                workspace_id.clone(),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap()
            .artifact;
        let invite_id = artifact.invite_id.clone();
        let capability = InvitationCapability::from_secret_bytes(
            decode_hex_32(&artifact.capability_secret).unwrap(),
        );
        let mut claim = invitee
            .prepare_workspace_invite_claim(
                artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();

        claim.payload.response_encryption_public_key = "00".to_owned();
        let signing_bytes = serde_json::to_vec(&claim.payload).unwrap();
        claim.device_signature = encode_hex(&invitee.identity.sign_bytes(&signing_bytes));
        claim.capability_signature = encode_hex(&capability.sign(&signing_bytes));

        assert!(matches!(
            admin.claim_workspace_invite(claim),
            Err(RuntimeError::InvalidWorkspaceInviteClaim)
        ));
        let state = admin.workspace_write_context(&workspace_id).unwrap().state;
        assert_eq!(state.members.len(), 1);
        assert!(state.join_requests.is_empty());
        assert_eq!(
            state.invites.get(&invite_id).unwrap().status,
            WorkspaceInviteStatus::Invited
        );
    }

    #[test]
    fn claimable_invite_can_only_be_processed_by_its_creating_device() {
        let admin_dir = tempdir().unwrap();
        let invitee_dir = tempdir().unwrap();
        let admin = LocalRuntime::open(admin_dir.path(), None).unwrap();
        let invitee = LocalRuntime::open(invitee_dir.path(), None).unwrap();
        let other_admin_identity = admin_dir.path().join("other-admin-identity.json");
        let other_admin = LocalRuntime::open(admin_dir.path(), Some(other_admin_identity)).unwrap();
        let created = admin.create_workspace("Bound creator", "general").unwrap();
        let artifact = admin
            .create_workspace_invite(
                WorkspaceId(created.workspace_id),
                "Bob".to_owned(),
                WorkspaceRole::Member,
                String::new(),
                String::new(),
                String::new(),
            )
            .unwrap()
            .artifact;
        let capability_secret = decode_hex_32(&artifact.capability_secret).unwrap();
        let capability = InvitationCapability::from_secret_bytes(capability_secret);
        let mut claim = invitee
            .prepare_workspace_invite_claim(
                artifact,
                "Bob".to_owned(),
                String::new(),
                String::new(),
            )
            .unwrap();

        claim.payload.delivery_device_id = other_admin.identity.device_id().0.clone();
        let signing_bytes = serde_json::to_vec(&claim.payload).unwrap();
        claim.device_signature = encode_hex(&invitee.identity.sign_bytes(&signing_bytes));
        claim.capability_signature = encode_hex(&capability.sign(&signing_bytes));

        assert!(matches!(
            other_admin.claim_workspace_invite(claim),
            Err(RuntimeError::InvalidWorkspaceInviteClaim)
        ));
    }
}
