use chaft_mls::{
    OPENMLS_CHANNEL_GROUP_PROTOCOL, OPENMLS_KEY_PACKAGE_PROTOCOL, OPENMLS_WORKSPACE_GROUP_PROTOCOL,
};
use chaft_types::{
    ChannelId, DeviceId, DeviceKeyPackageId, EventBody, EventId, SignableEvent, WorkspaceId,
};
use serde::{Deserialize, Serialize};

use crate::{
    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP, LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP, LocalRuntime, RuntimeError,
    channel_openmls_commit_event, validate_channel_id_reference,
    validate_device_key_package_id_reference, validate_workspace_id_reference,
    workspace_openmls_commit_event,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedOpenMlsKeyPackage {
    pub workspace_id: String,
    pub device_id: String,
    pub key_package_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub key_package_ref: String,
    pub byte_len: usize,
    pub private_bundle_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedOpenMlsWorkspaceGroupMember {
    pub workspace_id: String,
    pub device_id: String,
    pub invitee_device_id: String,
    pub invitee_key_package_id: String,
    pub invitee_key_package_ref: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub welcome_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedOpenMlsWorkspaceGroupMember {
    pub workspace_id: String,
    pub device_id: String,
    pub removed_device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
    pub source_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedOpenMlsWorkspaceGroup {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOpenMlsWorkspaceGroupCommits {
    pub workspace_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub self_removed: bool,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedOpenMlsChannelGroupMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub invitee_device_id: String,
    pub invitee_key_package_id: String,
    pub invitee_key_package_ref: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub welcome_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedOpenMlsChannelGroupMember {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub removed_device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub private_group_state_path: String,
    pub source_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedOpenMlsChannelGroup {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub commit_byte_len: usize,
    pub ratchet_tree_byte_len: usize,
    pub private_group_state_path: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedWorkspaceOpenMlsGroups {
    pub workspace_id: String,
    pub workspace_update: Option<UpdatedOpenMlsWorkspaceGroup>,
    #[serde(default)]
    pub channel_update_count: usize,
    pub channel_updates: Vec<UpdatedOpenMlsChannelGroup>,
    #[serde(default)]
    pub updated_event_count: usize,
    pub updated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOpenMlsChannelGroupCommits {
    pub workspace_id: String,
    pub channel_id: String,
    pub device_id: String,
    pub protocol: String,
    pub ciphersuite: String,
    pub group_id: String,
    pub epoch: u64,
    pub member_count: usize,
    pub applied_event_count: usize,
    pub applied_event_ids: Vec<String>,
    pub self_removed: bool,
    pub private_group_state_path: String,
}

impl LocalRuntime {
    pub fn publish_openmls_device_key_package(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<PublishedOpenMlsKeyPackage, RuntimeError> {
        let generated = chaft_mls::generate_device_key_package(&self.identity.device_id().0)?;
        debug_assert_eq!(generated.protocol, OPENMLS_KEY_PACKAGE_PROTOCOL);
        let key_package_id = DeviceKeyPackageId::new();
        let private_bundle_path =
            self.openmls_key_package_path(&workspace_id, &generated.key_package_ref);
        let context = self.workspace_write_context(&workspace_id)?;
        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::DeviceKeyPackagePublished {
                key_package_id: key_package_id.clone(),
                protocol: generated.protocol.clone(),
                key_package: generated.key_package.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_bundle_path,
                    LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
                    &generated.private_bundle,
                )
            },
        )?;

        Ok(PublishedOpenMlsKeyPackage {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            key_package_id: key_package_id.0,
            protocol: generated.protocol,
            ciphersuite: generated.ciphersuite,
            key_package_ref: generated.key_package_ref,
            byte_len: generated.key_package.len(),
            private_bundle_path: private_bundle_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn create_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<CreatedOpenMlsWorkspaceGroup, RuntimeError> {
        self.require_workspace_admin(&workspace_id, "create_openmls_workspace_group")?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists { workspace_id });
        }

        let created =
            chaft_mls::create_workspace_group(&self.identity.device_id().0, &workspace_id.0)?;
        debug_assert_eq!(created.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
            &created.private_group_state,
        )?;

        Ok(CreatedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: created.protocol,
            ciphersuite: created.ciphersuite,
            group_id: created.group_id,
            epoch: created.epoch,
            member_count: created.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn add_openmls_workspace_group_member(
        &self,
        workspace_id: WorkspaceId,
        key_package_id: DeviceKeyPackageId,
    ) -> Result<AddedOpenMlsWorkspaceGroupMember, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_device_key_package_id_reference(&key_package_id)?;
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_workspace_admin_in_state(
            &context.state,
            "add_openmls_workspace_group_member",
        )?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let key_package = context
            .state
            .key_packages
            .get(&key_package_id)
            .ok_or_else(|| RuntimeError::DeviceKeyPackageNotFound {
                workspace_id: workspace_id.clone(),
                key_package_id: key_package_id.clone(),
            })?;
        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let added = chaft_mls::add_member_to_workspace_group(
            &private_group_state,
            &key_package.key_package,
        )?;
        debug_assert_eq!(added.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupMemberAdded {
                invitee_device_id: key_package.device_id.clone(),
                invitee_key_package_id: key_package_id.clone(),
                invitee_key_package_ref: added.invitee_key_package_ref.clone(),
                protocol: added.protocol.clone(),
                ciphersuite: added.ciphersuite.clone(),
                group_id: added.group_id.clone(),
                epoch: added.epoch,
                commit: added.commit.clone(),
                welcome: added.welcome.clone(),
                ratchet_tree: added.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &added.updated_private_group_state,
                )
            },
        )?;

        Ok(AddedOpenMlsWorkspaceGroupMember {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            invitee_device_id: key_package.device_id.0.clone(),
            invitee_key_package_id: key_package_id.0,
            invitee_key_package_ref: added.invitee_key_package_ref,
            protocol: added.protocol,
            ciphersuite: added.ciphersuite,
            group_id: added.group_id,
            epoch: added.epoch,
            member_count: added.member_count,
            commit_byte_len: added.commit.len(),
            welcome_byte_len: added.welcome.len(),
            ratchet_tree_byte_len: added.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn remove_openmls_workspace_group_member(
        &self,
        workspace_id: WorkspaceId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedOpenMlsWorkspaceGroupMember, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_workspace_admin_in_state(
            &context.state,
            "remove_openmls_workspace_group_member",
        )?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let removed =
            chaft_mls::remove_member_from_group(&private_group_state, &removed_device_id.0)?;
        debug_assert_eq!(removed.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                removed_device_id: removed_device_id.clone(),
                protocol: removed.protocol.clone(),
                ciphersuite: removed.ciphersuite.clone(),
                group_id: removed.group_id.clone(),
                epoch: removed.epoch,
                commit: removed.commit.clone(),
                ratchet_tree: removed.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &removed.updated_private_group_state,
                )
            },
        )?;

        Ok(RemovedOpenMlsWorkspaceGroupMember {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            removed_device_id: removed_device_id.0,
            protocol: removed.protocol,
            ciphersuite: removed.ciphersuite,
            group_id: removed.group_id,
            epoch: removed.epoch,
            member_count: removed.member_count,
            commit_byte_len: removed.commit.len(),
            ratchet_tree_byte_len: removed.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn join_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
        source_event_id: Option<EventId>,
    ) -> Result<JoinedOpenMlsWorkspaceGroup, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupAlreadyExists {
                workspace_id: workspace_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let selected = events.iter().rev().find_map(|event| {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                return None;
            }
            match &event.event.body {
                EventBody::OpenMlsWorkspaceGroupMemberAdded {
                    invitee_device_id,
                    invitee_key_package_ref,
                    welcome,
                    ratchet_tree,
                    ..
                } if invitee_device_id == self.identity.device_id() => Some((
                    event.event_id.clone(),
                    invitee_key_package_ref.clone(),
                    welcome.clone(),
                    ratchet_tree.clone(),
                )),
                _ => None,
            }
        });
        let Some((event_id, key_package_ref, welcome, ratchet_tree)) = selected else {
            return Err(RuntimeError::OpenMlsWorkspaceGroupInviteNotFound {
                workspace_id,
                device_id: self.identity.device_id().clone(),
            });
        };

        let private_bundle_path = self.openmls_key_package_path(&workspace_id, &key_package_ref);
        if !private_bundle_path.exists() {
            return Err(RuntimeError::OpenMlsPrivateKeyPackageMissing {
                workspace_id,
                key_package_ref,
            });
        }

        let joined = chaft_mls::join_workspace_group_from_welcome(
            &self.read_openmls_secret_file(
                &private_bundle_path,
                LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
            )?,
            &welcome,
            &ratchet_tree,
        )?;
        debug_assert_eq!(joined.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
            &joined.private_group_state,
        )?;

        Ok(JoinedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: joined.protocol,
            ciphersuite: joined.ciphersuite,
            group_id: joined.group_id,
            epoch: joined.epoch,
            member_count: joined.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            source_event_id: event_id.0,
        })
    }

    pub fn update_openmls_workspace_group(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<UpdatedOpenMlsWorkspaceGroup, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let updated = chaft_mls::update_own_leaf_in_group(&private_group_state)?;
        debug_assert_eq!(updated.protocol, OPENMLS_WORKSPACE_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::OpenMlsWorkspaceGroupSelfUpdated {
                protocol: updated.protocol.clone(),
                ciphersuite: updated.ciphersuite.clone(),
                group_id: updated.group_id.clone(),
                epoch: updated.epoch,
                commit: updated.commit.clone(),
                ratchet_tree: updated.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                    &updated.updated_private_group_state,
                )
            },
        )?;

        Ok(UpdatedOpenMlsWorkspaceGroup {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: updated.protocol,
            ciphersuite: updated.ciphersuite,
            group_id: updated.group_id,
            epoch: updated.epoch,
            member_count: updated.member_count,
            commit_byte_len: updated.commit.len(),
            ratchet_tree_byte_len: updated.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn apply_openmls_workspace_group_commits(
        &self,
        workspace_id: WorkspaceId,
        source_event_id: Option<EventId>,
    ) -> Result<AppliedOpenMlsWorkspaceGroupCommits, RuntimeError> {
        let private_group_state_path = self.openmls_workspace_group_path(&workspace_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsWorkspaceGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
        )?;
        let mut validated =
            chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
        let mut protocol = validated.protocol.clone();
        let mut ciphersuite = validated.ciphersuite.clone();
        let mut group_id = validated.group_id.clone();
        let mut epoch = validated.epoch;
        let mut member_count = validated.member_count;
        let mut applied_event_ids = Vec::new();
        let mut self_removed = false;
        let mut selected_event_found = source_event_id.is_none();

        for event in &events {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                continue;
            }
            if source_event_id.is_some() {
                selected_event_found = true;
            }

            let Some((event_group_id, event_epoch, commit)) = workspace_openmls_commit_event(event)
            else {
                continue;
            };
            if event_group_id != validated.group_id || event_epoch <= validated.epoch {
                continue;
            }

            let applied = chaft_mls::apply_group_commit(&private_group_state, commit)?;
            private_group_state = applied.updated_private_group_state.clone();
            self_removed |= applied.self_removed;
            applied_event_ids.push(event.event_id.0.clone());
            protocol = applied.protocol.clone();
            ciphersuite = applied.ciphersuite.clone();
            group_id = applied.group_id.clone();
            epoch = applied.epoch;
            member_count = applied.member_count;
            if applied.self_removed {
                break;
            }
            validated = chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
            protocol = validated.protocol.clone();
            ciphersuite = validated.ciphersuite.clone();
            group_id = validated.group_id.clone();
            epoch = validated.epoch;
            member_count = validated.member_count;
        }

        if !selected_event_found && let Some(source_event_id) = source_event_id {
            return Err(RuntimeError::EventNotFound {
                workspace_id,
                event_id: source_event_id,
            });
        }

        if !applied_event_ids.is_empty() {
            self.write_openmls_secret_file(
                &private_group_state_path,
                LOCAL_SECRET_KIND_OPENMLS_WORKSPACE_GROUP,
                &private_group_state,
            )?;
        }

        Ok(AppliedOpenMlsWorkspaceGroupCommits {
            workspace_id: workspace_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol,
            ciphersuite,
            group_id,
            epoch,
            member_count,
            applied_event_count: applied_event_ids.len(),
            applied_event_ids,
            self_removed,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn create_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<CreatedOpenMlsChannelGroup, RuntimeError> {
        self.require_local_channel_access(&workspace_id, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupAlreadyExists {
                workspace_id,
                channel_id,
            });
        }

        let created = chaft_mls::create_channel_group(
            &self.identity.device_id().0,
            &workspace_id.0,
            &channel_id.0,
        )?;
        debug_assert_eq!(created.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
            &created.private_group_state,
        )?;

        Ok(CreatedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: created.protocol,
            ciphersuite: created.ciphersuite,
            group_id: created.group_id,
            epoch: created.epoch,
            member_count: created.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }

    pub fn add_openmls_channel_group_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        key_package_id: DeviceKeyPackageId,
    ) -> Result<AddedOpenMlsChannelGroupMember, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_channel_id_reference(&channel_id)?;
        validate_device_key_package_id_reference(&key_package_id)?;
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let key_package = context
            .state
            .key_packages
            .get(&key_package_id)
            .ok_or_else(|| RuntimeError::DeviceKeyPackageNotFound {
                workspace_id: workspace_id.clone(),
                key_package_id: key_package_id.clone(),
            })?;
        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let added = chaft_mls::add_member_to_workspace_group(
            &private_group_state,
            &key_package.key_package,
        )?;
        debug_assert_eq!(added.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupMemberAdded {
                channel_id: channel_id.clone(),
                invitee_device_id: key_package.device_id.clone(),
                invitee_key_package_id: key_package_id.clone(),
                invitee_key_package_ref: added.invitee_key_package_ref.clone(),
                protocol: added.protocol.clone(),
                ciphersuite: added.ciphersuite.clone(),
                group_id: added.group_id.clone(),
                epoch: added.epoch,
                commit: added.commit.clone(),
                welcome: added.welcome.clone(),
                ratchet_tree: added.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &added.updated_private_group_state,
                )
            },
        )?;

        Ok(AddedOpenMlsChannelGroupMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            invitee_device_id: key_package.device_id.0.clone(),
            invitee_key_package_id: key_package_id.0,
            invitee_key_package_ref: added.invitee_key_package_ref,
            protocol: added.protocol,
            ciphersuite: added.ciphersuite,
            group_id: added.group_id,
            epoch: added.epoch,
            member_count: added.member_count,
            commit_byte_len: added.commit.len(),
            welcome_byte_len: added.welcome.len(),
            ratchet_tree_byte_len: added.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn remove_openmls_channel_group_member(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        removed_device_id: DeviceId,
    ) -> Result<RemovedOpenMlsChannelGroupMember, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_local_channel_access_in_state(&context.state, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let removed =
            chaft_mls::remove_member_from_group(&private_group_state, &removed_device_id.0)?;
        debug_assert_eq!(removed.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupMemberRemoved {
                channel_id: channel_id.clone(),
                removed_device_id: removed_device_id.clone(),
                protocol: removed.protocol.clone(),
                ciphersuite: removed.ciphersuite.clone(),
                group_id: removed.group_id.clone(),
                epoch: removed.epoch,
                commit: removed.commit.clone(),
                ratchet_tree: removed.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &removed.updated_private_group_state,
                )
            },
        )?;

        Ok(RemovedOpenMlsChannelGroupMember {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            removed_device_id: removed_device_id.0,
            protocol: removed.protocol,
            ciphersuite: removed.ciphersuite,
            group_id: removed.group_id,
            epoch: removed.epoch,
            member_count: removed.member_count,
            commit_byte_len: removed.commit.len(),
            ratchet_tree_byte_len: removed.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn join_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        source_event_id: Option<EventId>,
    ) -> Result<JoinedOpenMlsChannelGroup, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupAlreadyExists {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let selected = events.iter().rev().find_map(|event| {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                return None;
            }
            match &event.event.body {
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id: event_channel_id,
                    invitee_device_id,
                    invitee_key_package_ref,
                    welcome,
                    ratchet_tree,
                    ..
                } if event_channel_id == &channel_id
                    && invitee_device_id == self.identity.device_id() =>
                {
                    Some((
                        event.event_id.clone(),
                        invitee_key_package_ref.clone(),
                        welcome.clone(),
                        ratchet_tree.clone(),
                    ))
                }
                _ => None,
            }
        });
        let Some((event_id, key_package_ref, welcome, ratchet_tree)) = selected else {
            return Err(RuntimeError::OpenMlsChannelGroupInviteNotFound {
                workspace_id,
                channel_id,
                device_id: self.identity.device_id().clone(),
            });
        };

        let private_bundle_path = self.openmls_key_package_path(&workspace_id, &key_package_ref);
        if !private_bundle_path.exists() {
            return Err(RuntimeError::OpenMlsPrivateKeyPackageMissing {
                workspace_id,
                key_package_ref,
            });
        }

        let joined = chaft_mls::join_channel_group_from_welcome(
            &self.read_openmls_secret_file(
                &private_bundle_path,
                LOCAL_SECRET_KIND_OPENMLS_KEY_PACKAGE,
            )?,
            &welcome,
            &ratchet_tree,
        )?;
        debug_assert_eq!(joined.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);
        self.write_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
            &joined.private_group_state,
        )?;

        Ok(JoinedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: joined.protocol,
            ciphersuite: joined.ciphersuite,
            group_id: joined.group_id,
            epoch: joined.epoch,
            member_count: joined.member_count,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            source_event_id: event_id.0,
        })
    }

    pub fn update_openmls_channel_group(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<UpdatedOpenMlsChannelGroup, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        self.require_local_channel_access_in_state(&context.state, &channel_id)?;
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let updated = chaft_mls::update_own_leaf_in_group(&private_group_state)?;
        debug_assert_eq!(updated.protocol, OPENMLS_CHANNEL_GROUP_PROTOCOL);

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::OpenMlsChannelGroupSelfUpdated {
                channel_id: channel_id.clone(),
                protocol: updated.protocol.clone(),
                ciphersuite: updated.ciphersuite.clone(),
                group_id: updated.group_id.clone(),
                epoch: updated.epoch,
                commit: updated.commit.clone(),
                ratchet_tree: updated.ratchet_tree.clone(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| {
                runtime.write_openmls_secret_file(
                    &private_group_state_path,
                    LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                    &updated.updated_private_group_state,
                )
            },
        )?;

        Ok(UpdatedOpenMlsChannelGroup {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol: updated.protocol,
            ciphersuite: updated.ciphersuite,
            group_id: updated.group_id,
            epoch: updated.epoch,
            member_count: updated.member_count,
            commit_byte_len: updated.commit.len(),
            ratchet_tree_byte_len: updated.ratchet_tree.len(),
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
            event_id: event.event_id.0,
        })
    }

    pub fn update_workspace_openmls_groups(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<UpdatedWorkspaceOpenMlsGroups, RuntimeError> {
        let has_workspace_group = self.openmls_workspace_group_path(&workspace_id).exists();
        let channel_ids = self.local_updatable_openmls_channel_group_ids(&workspace_id)?;
        if !has_workspace_group && channel_ids.is_empty() {
            return Err(RuntimeError::OpenMlsLocalGroupMissing {
                workspace_id: workspace_id.clone(),
            });
        }

        let workspace_update = if has_workspace_group {
            Some(self.update_openmls_workspace_group(workspace_id.clone())?)
        } else {
            None
        };
        let mut channel_updates = Vec::with_capacity(channel_ids.len());
        for channel_id in channel_ids {
            channel_updates
                .push(self.update_openmls_channel_group(workspace_id.clone(), channel_id)?);
        }

        let mut updated_event_ids = Vec::with_capacity(
            if workspace_update.is_some() { 1 } else { 0 } + channel_updates.len(),
        );
        if let Some(workspace_update) = &workspace_update {
            updated_event_ids.push(workspace_update.event_id.clone());
        }
        updated_event_ids.extend(channel_updates.iter().map(|update| update.event_id.clone()));

        Ok(UpdatedWorkspaceOpenMlsGroups {
            workspace_id: workspace_id.0,
            workspace_update,
            channel_update_count: channel_updates.len(),
            channel_updates,
            updated_event_count: updated_event_ids.len(),
            updated_event_ids,
        })
    }

    pub fn apply_openmls_channel_group_commits(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        source_event_id: Option<EventId>,
    ) -> Result<AppliedOpenMlsChannelGroupCommits, RuntimeError> {
        let private_group_state_path = self.openmls_channel_group_path(&workspace_id, &channel_id);
        if !private_group_state_path.exists() {
            return Err(RuntimeError::OpenMlsChannelGroupMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            });
        }

        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut private_group_state = self.read_openmls_secret_file(
            &private_group_state_path,
            LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
        )?;
        let mut validated =
            chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
        let mut protocol = validated.protocol.clone();
        let mut ciphersuite = validated.ciphersuite.clone();
        let mut group_id = validated.group_id.clone();
        let mut epoch = validated.epoch;
        let mut member_count = validated.member_count;
        let mut applied_event_ids = Vec::new();
        let mut self_removed = false;
        let mut selected_event_found = source_event_id.is_none();

        for event in &events {
            if source_event_id
                .as_ref()
                .is_some_and(|source_event_id| source_event_id != &event.event_id)
            {
                continue;
            }
            if source_event_id.is_some() {
                selected_event_found = true;
            }

            let Some((event_group_id, event_epoch, commit)) =
                channel_openmls_commit_event(event, &channel_id)
            else {
                continue;
            };
            if event_group_id != validated.group_id || event_epoch <= validated.epoch {
                continue;
            }

            let applied = chaft_mls::apply_group_commit(&private_group_state, commit)?;
            private_group_state = applied.updated_private_group_state.clone();
            self_removed |= applied.self_removed;
            applied_event_ids.push(event.event_id.0.clone());
            protocol = applied.protocol.clone();
            ciphersuite = applied.ciphersuite.clone();
            group_id = applied.group_id.clone();
            epoch = applied.epoch;
            member_count = applied.member_count;
            if applied.self_removed {
                break;
            }
            validated = chaft_mls::validate_private_workspace_group_state(&private_group_state)?;
            protocol = validated.protocol.clone();
            ciphersuite = validated.ciphersuite.clone();
            group_id = validated.group_id.clone();
            epoch = validated.epoch;
            member_count = validated.member_count;
        }

        if !selected_event_found && let Some(source_event_id) = source_event_id {
            return Err(RuntimeError::EventNotFound {
                workspace_id,
                event_id: source_event_id,
            });
        }

        if !applied_event_ids.is_empty() {
            self.write_openmls_secret_file(
                &private_group_state_path,
                LOCAL_SECRET_KIND_OPENMLS_CHANNEL_GROUP,
                &private_group_state,
            )?;
        }

        Ok(AppliedOpenMlsChannelGroupCommits {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            device_id: self.identity.device_id().0.clone(),
            protocol,
            ciphersuite,
            group_id,
            epoch,
            member_count,
            applied_event_count: applied_event_ids.len(),
            applied_event_ids,
            self_removed,
            private_group_state_path: private_group_state_path.to_string_lossy().into_owned(),
        })
    }
}
