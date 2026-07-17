use std::collections::{BTreeMap, BTreeSet};

use chaft_mls::OPENMLS_KEY_PACKAGE_PROTOCOL;
use chaft_types::{ChannelId, DeviceId, DeviceKeyPackageId, EventBody, SignedEvent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelAccessProvisioningState {
    #[default]
    Ready,
    MlsWelcomePublished,
    KeyPackagePending,
    GroupPending,
    RevocationPending,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAccessProvisioningOutcome {
    pub channel_id: String,
    pub member_device_id: String,
    pub provisioning_state: ChannelAccessProvisioningState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openmls_epoch: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openmls_member_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning_error: Option<String>,
}

pub(crate) struct ProvisionedOpenMlsChannelMembers {
    pub(crate) channel_id: String,
    pub(crate) event_ids: Vec<String>,
    pub(crate) outcomes: Vec<ChannelAccessProvisioningOutcome>,
}

pub(crate) fn private_channel_creator_device_id_from_events(
    events: &[SignedEvent],
    expected_channel_id: &ChannelId,
) -> Option<DeviceId> {
    events.iter().find_map(|event| match &event.event.body {
        EventBody::ChannelCreated {
            channel_id,
            is_private: true,
            ..
        }
        | EventBody::DirectMessageChannelCreated { channel_id, .. }
            if channel_id == expected_channel_id =>
        {
            Some(event.event.author_device_id.clone())
        }
        _ => None,
    })
}

pub(crate) fn workspace_creator_device_id_from_events(events: &[SignedEvent]) -> Option<DeviceId> {
    events.iter().find_map(|event| {
        matches!(event.event.body, EventBody::WorkspaceCreated { .. })
            .then(|| event.event.author_device_id.clone())
    })
}

pub(crate) fn current_private_channel_member_ids_from_events(
    events: &[SignedEvent],
    expected_channel_id: &ChannelId,
) -> BTreeSet<String> {
    let mut member_ids = BTreeSet::new();
    for event in events {
        match &event.event.body {
            EventBody::ChannelCreated {
                channel_id,
                is_private,
                ..
            } if channel_id == expected_channel_id && *is_private => {
                member_ids.insert(event.event.author_device_id.0.clone());
            }
            EventBody::DirectMessageChannelCreated {
                channel_id,
                participant_device_ids,
                ..
            } if channel_id == expected_channel_id => {
                for participant_device_id in participant_device_ids {
                    member_ids.insert(participant_device_id.0.clone());
                }
                member_ids.insert(event.event.author_device_id.0.clone());
            }
            EventBody::ChannelMemberAdded {
                channel_id,
                member_device_id,
            } if channel_id == expected_channel_id => {
                member_ids.insert(member_device_id.0.clone());
            }
            EventBody::ChannelMemberRemoved {
                channel_id,
                member_device_id,
            } if channel_id == expected_channel_id => {
                member_ids.remove(&member_device_id.0);
            }
            EventBody::MemberRemoved { removed_device_id } => {
                member_ids.remove(&removed_device_id.0);
            }
            _ => {}
        }
    }
    member_ids
}

#[derive(Default)]
pub(crate) struct OpenMlsAutoProvisionIndex {
    used_key_package_ids: BTreeSet<String>,
    key_package_ids_by_device_id: BTreeMap<String, Vec<DeviceKeyPackageId>>,
    workspace_group_member_ids_by_group_id: BTreeMap<String, BTreeSet<String>>,
    channel_group_member_ids_by_channel_and_group_id:
        BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    workspace_commit_event_id_by_group_epoch: BTreeMap<(String, u64), String>,
    channel_commit_event_id_by_channel_group_epoch: BTreeMap<(String, String, u64), String>,
    forked_workspace_group_ids: BTreeSet<String>,
    forked_channel_group_ids: BTreeSet<(String, String)>,
    workspace_revoked_device_ids: BTreeSet<String>,
    channel_revoked_device_ids_by_channel_id: BTreeMap<String, BTreeSet<String>>,
}

impl OpenMlsAutoProvisionIndex {
    pub(crate) fn from_events(events: &[SignedEvent]) -> Self {
        let mut index = Self::default();
        for event in events {
            match &event.event.body {
                EventBody::OpenMlsWorkspaceGroupMemberAdded {
                    group_id, epoch, ..
                }
                | EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                    group_id, epoch, ..
                }
                | EventBody::OpenMlsWorkspaceGroupSelfUpdated {
                    group_id, epoch, ..
                } => {
                    let key = (group_id.clone(), *epoch);
                    if index
                        .workspace_commit_event_id_by_group_epoch
                        .insert(key, event.event_id.0.clone())
                        .is_some_and(|existing| existing != event.event_id.0)
                    {
                        index.forked_workspace_group_ids.insert(group_id.clone());
                    }
                }
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id,
                    group_id,
                    epoch,
                    ..
                }
                | EventBody::OpenMlsChannelGroupMemberRemoved {
                    channel_id,
                    group_id,
                    epoch,
                    ..
                }
                | EventBody::OpenMlsChannelGroupSelfUpdated {
                    channel_id,
                    group_id,
                    epoch,
                    ..
                } => {
                    let key = (channel_id.0.clone(), group_id.clone(), *epoch);
                    if index
                        .channel_commit_event_id_by_channel_group_epoch
                        .insert(key, event.event_id.0.clone())
                        .is_some_and(|existing| existing != event.event_id.0)
                    {
                        index
                            .forked_channel_group_ids
                            .insert((channel_id.0.clone(), group_id.clone()));
                    }
                }
                _ => {}
            }
            match &event.event.body {
                EventBody::MemberInvited {
                    invitee_device_id, ..
                } => {
                    index
                        .workspace_revoked_device_ids
                        .remove(&invitee_device_id.0);
                }
                EventBody::ChannelMemberAdded {
                    channel_id,
                    member_device_id,
                } => {
                    if let Some(device_ids) = index
                        .channel_revoked_device_ids_by_channel_id
                        .get_mut(&channel_id.0)
                    {
                        device_ids.remove(&member_device_id.0);
                    }
                }
                EventBody::DeviceKeyPackagePublished {
                    key_package_id,
                    protocol,
                    ..
                } if protocol == OPENMLS_KEY_PACKAGE_PROTOCOL => {
                    index
                        .key_package_ids_by_device_id
                        .entry(event.event.author_device_id.0.clone())
                        .or_default()
                        .push(key_package_id.clone());
                }
                EventBody::OpenMlsWorkspaceGroupMemberAdded {
                    invitee_device_id,
                    invitee_key_package_id,
                    group_id,
                    ..
                } => {
                    let group_members = index
                        .workspace_group_member_ids_by_group_id
                        .entry(group_id.clone())
                        .or_default();
                    group_members.insert(event.event.author_device_id.0.clone());
                    group_members.insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                    removed_device_id,
                    group_id,
                    ..
                } => {
                    if let Some(member_ids) = index
                        .workspace_group_member_ids_by_group_id
                        .get_mut(group_id)
                    {
                        member_ids.remove(&removed_device_id.0);
                    }
                    index
                        .workspace_revoked_device_ids
                        .insert(removed_device_id.0.clone());
                }
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id,
                    invitee_device_id,
                    invitee_key_package_id,
                    group_id,
                    ..
                } => {
                    let channel_members = index
                        .channel_group_member_ids_by_channel_and_group_id
                        .entry(channel_id.0.clone())
                        .or_default()
                        .entry(group_id.clone())
                        .or_default();
                    channel_members.insert(event.event.author_device_id.0.clone());
                    channel_members.insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsChannelGroupMemberRemoved {
                    channel_id,
                    removed_device_id,
                    group_id,
                    ..
                } => {
                    if let Some(member_ids) = index
                        .channel_group_member_ids_by_channel_and_group_id
                        .get_mut(&channel_id.0)
                        .and_then(|groups| groups.get_mut(group_id))
                    {
                        member_ids.remove(&removed_device_id.0);
                    }
                    index
                        .channel_revoked_device_ids_by_channel_id
                        .entry(channel_id.0.clone())
                        .or_default()
                        .insert(removed_device_id.0.clone());
                }
                _ => {}
            }
        }
        index
    }

    pub(crate) fn workspace_group_has_device(&self, device_id: &DeviceId) -> bool {
        !self.workspace_group_ids_for_device(device_id).is_empty()
    }

    pub(crate) fn workspace_device_is_revoked(&self, device_id: &DeviceId) -> bool {
        self.workspace_revoked_device_ids.contains(&device_id.0)
    }

    pub(crate) fn workspace_group_has_device_in_group(
        &self,
        group_id: &str,
        device_id: &DeviceId,
    ) -> bool {
        self.workspace_group_member_ids_by_group_id
            .get(group_id)
            .is_some_and(|member_ids| member_ids.contains(&device_id.0))
    }

    pub(crate) fn workspace_group_ids_for_device(&self, device_id: &DeviceId) -> Vec<String> {
        self.workspace_group_member_ids_by_group_id
            .iter()
            .filter(|(_, member_ids)| member_ids.contains(&device_id.0))
            .map(|(group_id, _)| group_id.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn workspace_group_is_forked(&self, group_id: &str) -> bool {
        self.forked_workspace_group_ids.contains(group_id)
    }

    pub(crate) fn has_forked_workspace_group(&self) -> bool {
        !self.forked_workspace_group_ids.is_empty()
    }

    pub(crate) fn channel_group_has_device(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> bool {
        self.channel_group_member_ids_by_channel_and_group_id
            .get(&channel_id.0)
            .is_some_and(|groups| {
                groups
                    .values()
                    .any(|member_ids| member_ids.contains(&device_id.0))
            })
    }

    pub(crate) fn channel_device_is_revoked(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> bool {
        self.channel_revoked_device_ids_by_channel_id
            .get(&channel_id.0)
            .is_some_and(|device_ids| device_ids.contains(&device_id.0))
    }

    pub(crate) fn channel_group_has_device_in_group(
        &self,
        channel_id: &ChannelId,
        group_id: &str,
        device_id: &DeviceId,
    ) -> bool {
        self.channel_group_member_ids_by_channel_and_group_id
            .get(&channel_id.0)
            .and_then(|groups| groups.get(group_id))
            .is_some_and(|member_ids| member_ids.contains(&device_id.0))
    }

    pub(crate) fn channel_group_ids_for_device_in_channel(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> Vec<String> {
        self.channel_group_member_ids_by_channel_and_group_id
            .get(&channel_id.0)
            .into_iter()
            .flat_map(|groups| groups.iter())
            .filter(|(_, member_ids)| member_ids.contains(&device_id.0))
            .map(|(group_id, _)| group_id.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn channel_group_is_forked(&self, channel_id: &ChannelId, group_id: &str) -> bool {
        self.forked_channel_group_ids
            .contains(&(channel_id.0.clone(), group_id.to_owned()))
    }

    pub(crate) fn channel_has_forked_group(&self, channel_id: &ChannelId) -> bool {
        self.forked_channel_group_ids
            .iter()
            .any(|(fork_channel_id, _)| fork_channel_id == &channel_id.0)
    }

    pub(crate) fn first_forked_channel_id(&self) -> Option<ChannelId> {
        self.forked_channel_group_ids
            .iter()
            .next()
            .map(|(channel_id, _)| ChannelId(channel_id.clone()))
    }

    pub(crate) fn channel_group_has_events(&self, channel_id: &ChannelId) -> bool {
        self.channel_commit_event_id_by_channel_group_epoch
            .keys()
            .any(|(event_channel_id, _, _)| event_channel_id == &channel_id.0)
    }

    pub(crate) fn workspace_group_has_events(&self) -> bool {
        !self.workspace_commit_event_id_by_group_epoch.is_empty()
    }

    pub(crate) fn channel_group_ids_for_device(&self, device_id: &DeviceId) -> Vec<ChannelId> {
        self.channel_group_member_ids_by_channel_and_group_id
            .iter()
            .filter(|(_, groups)| {
                groups
                    .values()
                    .any(|member_ids| member_ids.contains(&device_id.0))
            })
            .map(|(channel_id, _)| ChannelId(channel_id.clone()))
            .collect()
    }

    pub(crate) fn key_package_is_used(&self, key_package_id: &DeviceKeyPackageId) -> bool {
        self.used_key_package_ids.contains(&key_package_id.0)
    }

    pub(crate) fn latest_unused_key_package_id_for_device(
        &self,
        device_id: &DeviceId,
    ) -> Option<DeviceKeyPackageId> {
        self.key_package_ids_by_device_id
            .get(&device_id.0)?
            .iter()
            .rev()
            .find(|key_package_id| !self.used_key_package_ids.contains(&key_package_id.0))
            .cloned()
    }

    pub(crate) fn mark_workspace_group_member_added(
        &mut self,
        group_id: &str,
        device_id: &str,
        key_package_id: &str,
    ) {
        self.workspace_group_member_ids_by_group_id
            .entry(group_id.to_owned())
            .or_default()
            .insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }

    pub(crate) fn mark_channel_group_member_added(
        &mut self,
        channel_id: &str,
        group_id: &str,
        device_id: &str,
        key_package_id: &str,
    ) {
        self.channel_group_member_ids_by_channel_and_group_id
            .entry(channel_id.to_owned())
            .or_default()
            .entry(group_id.to_owned())
            .or_default()
            .insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }
}
