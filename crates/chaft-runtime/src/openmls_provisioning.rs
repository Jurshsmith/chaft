use std::collections::{BTreeMap, BTreeSet};

use chaft_mls::OPENMLS_KEY_PACKAGE_PROTOCOL;
use chaft_types::{ChannelId, DeviceId, DeviceKeyPackageId, EventBody, SignedEvent};

pub(crate) struct ProvisionedOpenMlsChannelMembers {
    pub(crate) channel_id: String,
    pub(crate) event_ids: Vec<String>,
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
    workspace_group_member_ids: BTreeSet<String>,
    channel_group_member_ids_by_channel_id: BTreeMap<String, BTreeSet<String>>,
}

impl OpenMlsAutoProvisionIndex {
    pub(crate) fn from_events(events: &[SignedEvent]) -> Self {
        let mut index = Self::default();
        for event in events {
            match &event.event.body {
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
                    ..
                } => {
                    index
                        .workspace_group_member_ids
                        .insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsWorkspaceGroupMemberRemoved {
                    removed_device_id, ..
                } => {
                    index
                        .workspace_group_member_ids
                        .remove(&removed_device_id.0);
                }
                EventBody::OpenMlsChannelGroupMemberAdded {
                    channel_id,
                    invitee_device_id,
                    invitee_key_package_id,
                    ..
                } => {
                    index
                        .channel_group_member_ids_by_channel_id
                        .entry(channel_id.0.clone())
                        .or_default()
                        .insert(invitee_device_id.0.clone());
                    index
                        .used_key_package_ids
                        .insert(invitee_key_package_id.0.clone());
                }
                EventBody::OpenMlsChannelGroupMemberRemoved {
                    channel_id,
                    removed_device_id,
                    ..
                } => {
                    if let Some(member_ids) = index
                        .channel_group_member_ids_by_channel_id
                        .get_mut(&channel_id.0)
                    {
                        member_ids.remove(&removed_device_id.0);
                    }
                }
                _ => {}
            }
        }
        index
    }

    pub(crate) fn workspace_group_has_device(&self, device_id: &DeviceId) -> bool {
        self.workspace_group_member_ids.contains(&device_id.0)
    }

    pub(crate) fn channel_group_has_device(
        &self,
        channel_id: &ChannelId,
        device_id: &DeviceId,
    ) -> bool {
        self.channel_group_member_ids_by_channel_id
            .get(&channel_id.0)
            .is_some_and(|member_ids| member_ids.contains(&device_id.0))
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
        device_id: &str,
        key_package_id: &str,
    ) {
        self.workspace_group_member_ids.insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }

    pub(crate) fn mark_channel_group_member_added(
        &mut self,
        channel_id: &str,
        device_id: &str,
        key_package_id: &str,
    ) {
        self.channel_group_member_ids_by_channel_id
            .entry(channel_id.to_owned())
            .or_default()
            .insert(device_id.to_owned());
        self.used_key_package_ids.insert(key_package_id.to_owned());
    }
}
