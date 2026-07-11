use chaft_core::WorkspaceInviteStatus;
use chaft_crypto::{ContentKey, SealedPayload, open_aes_256_gcm_siv, seal_aes_256_gcm_siv};
use chaft_identity::{
    InvitationCapability, verify_detached_signature, verify_device_detached_signature,
};
use chaft_types::{
    DeviceId, EventBody, SignableEvent, WORKSPACE_INVITE_CAPABILITY_PUBLIC_KEY_MAX_BYTES,
    WORKSPACE_INVITE_EXPIRES_AT_MAX_BYTES, WORKSPACE_INVITE_ID_MAX_BYTES,
    WORKSPACE_INVITE_SYNC_EXPECTATION_MAX_BYTES, WORKSPACE_JOIN_REQUEST_ID_MAX_BYTES, WorkspaceId,
    WorkspaceRole,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::{
    LocalRuntime, RuntimeError, WorkspaceKeyExport,
    local_file_io::{read_local_metadata_file_with_limit, write_secret_file},
    runtime_validation::{
        validate_metadata_field_size, validate_peer_endpoint_input, validate_workspace_id_reference,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInviteArtifact {
    pub kind: String,
    pub schema_version: u32,
    pub workspace_id: String,
    pub workspace_name: String,
    pub invite_id: String,
    pub display_name: String,
    pub role: WorkspaceRole,
    pub expires_at: String,
    pub capability_secret: String,
    pub capability_public_key: String,
    pub inviter_device_id: String,
    pub inviter_display_name: String,
    pub inviter_public_key: String,
    pub inviter_signature: String,
    pub peer_endpoint: String,
    pub sync_expectation: String,
    pub created_at: String,
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
}

impl LocalRuntime {
    pub fn create_workspace_invite(
        &self,
        workspace_id: WorkspaceId,
        display_name: String,
        role: WorkspaceRole,
        expires_at: String,
        peer_endpoint: String,
        sync_expectation: String,
    ) -> Result<CreatedWorkspaceInvite, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_metadata_field_size("display name", &display_name, 128)?;
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
                display_name: display_name.clone(),
                role,
                expires_at: expires_at.clone(),
                capability_public_key: capability_public_key.clone(),
                sync_expectation: sync_expectation.clone(),
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
            display_name,
            role,
            expires_at,
            capability_secret: encode_hex(&capability.secret_bytes()),
            capability_public_key,
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
        };
        write_secret_file(
            &self.workspace_invite_claim_receipt_path(&receipt.invite_id),
            &serde_json::to_vec(&receipt)?,
        )?;
        Ok(claim)
    }

    pub fn claim_workspace_invite(
        &self,
        claim: WorkspaceInviteClaim,
    ) -> Result<ClaimedWorkspaceInvite, RuntimeError> {
        self.verify_workspace_invite_claim(&claim)?;
        let workspace_id = WorkspaceId(claim.payload.workspace_id.clone());
        let mut context = self.workspace_write_context(&workspace_id)?;
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
        if invite_is_expired(&invite.expires_at)? {
            return Err(RuntimeError::WorkspaceInviteExpired {
                invite_id: invite.invite_id,
            });
        }
        let invitee_device_id = DeviceId(claim.payload.device_id.clone());
        let already_claimed = invite.status == WorkspaceInviteStatus::Accepted;
        if already_claimed
            && (invite.invitee_device_id != invitee_device_id
                || invite.request_id.as_deref() != Some(claim.payload.request_id.as_str()))
        {
            return Err(RuntimeError::WorkspaceInviteAlreadyClaimed {
                invite_id: invite.invite_id,
            });
        }
        if invite.status == WorkspaceInviteStatus::Revoked {
            return Err(RuntimeError::WorkspaceInviteNotClaimable {
                invite_id: invite.invite_id,
            });
        }

        let (member_event_id, claim_event_id) = if already_claimed {
            let member_event_id = context
                .state
                .members
                .get(&invitee_device_id)
                .map(|member| member.membership_event_id.0.clone())
                .ok_or_else(|| RuntimeError::WorkspaceInviteAlreadyClaimed {
                    invite_id: invite.invite_id.clone(),
                })?;
            (
                member_event_id,
                invite
                    .accepted_event_id
                    .as_ref()
                    .map(|event_id| event_id.0.clone())
                    .unwrap_or_default(),
            )
        } else {
            self.record_workspace_join_request_with_response_route(
                workspace_id.clone(),
                claim.payload.request_id.clone(),
                invitee_device_id.clone(),
                claim.payload.display_name.clone(),
                claim.payload.note.clone(),
                "invite_claim".to_owned(),
                claim.payload.invite_id.clone(),
                claim.payload.source_display_name.clone(),
                "preapproved".to_owned(),
                claim.payload.response_peer_endpoint.clone(),
            )?;
            let member =
                self.invite_member(workspace_id.clone(), invitee_device_id.clone(), invite.role)?;
            context = self.workspace_write_context(&workspace_id)?;
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
            claimed.parents = context.head_event_ids;
            let claimed = self.sign_authorize_and_append_with_history(claimed, &context.events)?;
            (member.event_id, claimed.event_id.0)
        };

        let workspace_key = self.export_workspace_key(workspace_id.clone())?;
        let response = self.seal_workspace_invite_response(
            &claim,
            invite.role,
            &invite.expires_at,
            workspace_key,
        )?;
        Ok(ClaimedWorkspaceInvite {
            workspace_id: workspace_id.0,
            invite_id: claim.payload.invite_id,
            request_id: claim.payload.request_id,
            invitee_device_id: invitee_device_id.0,
            role: invite.role,
            member_event_id,
            claim_event_id,
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
        let receipt = self.workspace_invite_claim_receipt(&response.payload.invite_id)?;
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
        let imported = self.import_workspace_key(workspace_key)?;
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

    fn invite_response_secret(&self) -> StaticSecret {
        StaticSecret::from(blake3::derive_key(
            RESPONSE_IDENTITY_KEY_CONTEXT,
            &self.identity.signing_key_bytes(),
        ))
    }

    fn workspace_invite_claim_receipt_path(&self, invite_id: &str) -> std::path::PathBuf {
        let file_id = blake3::hash(invite_id.as_bytes()).to_hex();
        self.paths
            .data_dir
            .join("invite-claims")
            .join(format!("{file_id}.json"))
    }

    fn workspace_invite_claim_receipt(
        &self,
        invite_id: &str,
    ) -> Result<WorkspaceInviteClaimReceipt, RuntimeError> {
        let bytes = read_local_metadata_file_with_limit(
            &self.workspace_invite_claim_receipt_path(invite_id),
            INVITE_CLAIM_RECEIPT_MAX_BYTES,
            "workspace invite claim receipt",
        )?
        .ok_or(RuntimeError::InvalidWorkspaceInviteResponse)?;
        serde_json::from_slice(&bytes).map_err(|_| RuntimeError::InvalidWorkspaceInviteResponse)
    }
}

fn validate_invite_artifact(artifact: &WorkspaceInviteArtifact) -> Result<(), RuntimeError> {
    if artifact.kind != WORKSPACE_INVITE_ARTIFACT_KIND
        || artifact.schema_version != WORKSPACE_INVITE_SCHEMA_VERSION
        || artifact.workspace_id.is_empty()
        || artifact.invite_id.is_empty()
        || artifact.capability_secret.len() != CAPABILITY_SECRET_BYTES * 2
    {
        return Err(RuntimeError::InvalidWorkspaceInviteClaim);
    }
    validate_workspace_id_reference(&WorkspaceId(artifact.workspace_id.clone()))?;
    validate_metadata_field_size(
        "invite ID",
        &artifact.invite_id,
        WORKSPACE_INVITE_ID_MAX_BYTES,
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
    use super::*;
    use tempfile::tempdir;

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
                "Bob".to_owned(),
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
        let artifact_json = serde_json::to_string(&invite.artifact).unwrap();
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
                workspace_id,
                String::new(),
                WorkspaceRole::Member,
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
}
