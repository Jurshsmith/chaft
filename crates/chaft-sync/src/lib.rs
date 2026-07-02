use std::collections::{BTreeSet, HashMap, HashSet};

use chaft_core::{CoreError, MaterializationReport, WorkspaceState};
use chaft_identity::{IdentityError, verify_self_contained_event};
use chaft_net::{ChaftTransport, NetError, PeerAddress};
use chaft_store::{EventStore, StoreError};
use chaft_types::{EventId, SignedEvent, WorkspaceId, is_canonical_event_id_str};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("network error")]
    Net(#[from] NetError),
    #[error("store error")]
    Store(#[from] StoreError),
    #[error("identity verification error")]
    Identity(#[from] IdentityError),
    #[error("materialization error")]
    Core(#[from] CoreError),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventInventory {
    pub event_ids: BTreeSet<EventId>,
}

impl EventInventory {
    pub fn from_event_ids(event_ids: impl IntoIterator<Item = EventId>) -> Self {
        Self {
            event_ids: event_ids.into_iter().collect(),
        }
    }

    pub fn missing_from(&self, remote: &EventInventory) -> Vec<EventId> {
        remote
            .event_ids
            .difference(&self.event_ids)
            .cloned()
            .collect()
    }

    pub fn has(&self, event_id: &EventId) -> bool {
        self.event_ids.contains(event_id)
    }
}

pub fn inventory_from_events(events: &[SignedEvent]) -> EventInventory {
    EventInventory::from_event_ids(events.iter().map(|event| event.event_id.clone()))
}

pub fn events_missing_from_local(
    local: &EventInventory,
    remote_events: &[SignedEvent],
) -> Vec<SignedEvent> {
    remote_events
        .iter()
        .filter(|event| !local.has(&event.event_id))
        .cloned()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullSyncReport {
    pub requested_event_ids: Vec<EventId>,
    pub fetched_event_ids: Vec<EventId>,
    pub ignored_event_ids: Vec<EventId>,
    pub materialization: MaterializationReport,
}

pub async fn pull_workspace_from_peer<T>(
    transport: &T,
    peer: &PeerAddress,
    local_store: &EventStore,
    workspace_id: WorkspaceId,
) -> Result<PullSyncReport, SyncError>
where
    T: ChaftTransport,
{
    let remote_event_ids = transport
        .fetch_workspace_inventory(peer, &workspace_id)
        .await?;
    pull_workspace_from_peer_with_inventory(
        transport,
        peer,
        local_store,
        workspace_id,
        remote_event_ids,
    )
    .await
}

pub async fn pull_workspace_from_peer_with_inventory<T>(
    transport: &T,
    peer: &PeerAddress,
    local_store: &EventStore,
    workspace_id: WorkspaceId,
    remote_event_ids: Vec<EventId>,
) -> Result<PullSyncReport, SyncError>
where
    T: ChaftTransport,
{
    let local_inventory = workspace_inventory_from_store(local_store, &workspace_id)?;
    validate_remote_inventory_event_ids(&remote_event_ids)?;
    let requested_event_ids = missing_remote_event_ids(&local_inventory, remote_event_ids);
    let fetched_events = if requested_event_ids.is_empty() {
        Vec::new()
    } else {
        transport
            .fetch_events(peer, requested_event_ids.clone())
            .await?
    };
    validate_fetched_events(&fetched_events, &requested_event_ids)?;
    let mut ignored_event_ids = Vec::new();
    let mut workspace_events = Vec::with_capacity(fetched_events.len());

    for event in fetched_events {
        verify_self_contained_event(&event)?;
        if event.event.workspace_id != workspace_id {
            ignored_event_ids.push(event.event_id.clone());
            continue;
        }

        workspace_events.push(event);
    }

    let (materialization_events, fetched_event_ids) = if workspace_events.is_empty() {
        let local_events =
            verified_sync_events(local_store.list_parseable_events_for_workspace(&workspace_id.0)?);
        (local_events, Vec::new())
    } else {
        let mut local_events =
            verified_sync_events(local_store.list_parseable_events_for_workspace(&workspace_id.0)?);
        let appended_events = append_workspace_events_in_materialized_order(
            local_store,
            &workspace_id,
            &local_events,
            &workspace_events,
        )?;
        let AppendedWorkspaceEvents { event_ids, events } = appended_events;
        local_events.extend(events);
        (local_events, event_ids)
    };

    let mut state = WorkspaceState::new(workspace_id.clone());
    let materialization = state.apply_batch(&materialization_events)?;

    Ok(PullSyncReport {
        requested_event_ids,
        fetched_event_ids,
        ignored_event_ids,
        materialization,
    })
}

#[derive(Debug, Clone, Default)]
struct AppendedWorkspaceEvents {
    event_ids: Vec<EventId>,
    events: Vec<SignedEvent>,
}

fn workspace_inventory_from_store(
    local_store: &EventStore,
    workspace_id: &WorkspaceId,
) -> Result<EventInventory, SyncError> {
    Ok(EventInventory::from_event_ids(
        local_store.list_servable_event_ids_for_workspace(&workspace_id.0)?,
    ))
}

fn missing_remote_event_ids(
    local_inventory: &EventInventory,
    remote_event_ids: Vec<EventId>,
) -> Vec<EventId> {
    let mut seen_remote = HashSet::new();
    let mut missing = Vec::new();

    for event_id in remote_event_ids {
        if local_inventory.has(&event_id) || !seen_remote.insert(event_id.clone()) {
            continue;
        }
        missing.push(event_id);
    }

    missing
}

pub fn validate_remote_inventory_event_ids(remote_event_ids: &[EventId]) -> Result<(), SyncError> {
    let mut seen = BTreeSet::new();

    for event_id in remote_event_ids {
        if !is_canonical_event_id_str(&event_id.0) {
            return Err(protocol_error(
                "peer returned non-canonical inventory event id",
            ));
        }
        if !seen.insert(event_id.clone()) {
            return Err(protocol_error(format!(
                "peer returned duplicate inventory event id {event_id}"
            )));
        }
    }

    Ok(())
}

fn validate_fetched_events(
    fetched_events: &[SignedEvent],
    requested_event_ids: &[EventId],
) -> Result<(), SyncError> {
    let requested = requested_event_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();

    for event in fetched_events {
        if !requested.contains(&event.event_id) {
            return Err(protocol_error(format!(
                "peer returned unrequested event {}",
                event.event_id
            )));
        }
        if !seen.insert(event.event_id.clone()) {
            return Err(protocol_error(format!(
                "peer returned duplicate event {}",
                event.event_id
            )));
        }
    }

    Ok(())
}

fn protocol_error(message: impl Into<String>) -> SyncError {
    SyncError::Net(NetError::Protocol(message.into()))
}

fn append_workspace_events_in_materialized_order(
    local_store: &EventStore,
    workspace_id: &WorkspaceId,
    local_events: &[SignedEvent],
    fetched_events: &[SignedEvent],
) -> Result<AppendedWorkspaceEvents, SyncError> {
    if fetched_events.is_empty() {
        return Ok(AppendedWorkspaceEvents::default());
    }

    let mut preview_events = Vec::with_capacity(local_events.len() + fetched_events.len());
    preview_events.extend_from_slice(local_events);
    preview_events.extend_from_slice(fetched_events);

    let mut preview_state = WorkspaceState::new(workspace_id.clone());
    let preview = preview_state.apply_batch(&preview_events)?;

    let mut fetched_by_id = HashMap::with_capacity(fetched_events.len());
    let mut fetched_order = Vec::with_capacity(fetched_events.len());
    for event in fetched_events {
        if fetched_by_id
            .insert(event.event_id.clone(), event)
            .is_none()
        {
            fetched_order.push(event.event_id.clone());
        }
    }

    let mut appended_events = AppendedWorkspaceEvents {
        event_ids: Vec::with_capacity(fetched_by_id.len()),
        events: Vec::with_capacity(fetched_by_id.len()),
    };
    let mut appended = HashSet::with_capacity(fetched_by_id.len());

    for event_id in preview
        .applied_events
        .iter()
        .chain(preview.gaps.iter().map(|gap| &gap.event_id))
    {
        if let Some(event) = fetched_by_id.get(event_id)
            && appended.insert(event_id.clone())
        {
            local_store.append_event(event)?;
            appended_events.event_ids.push(event_id.clone());
            appended_events.events.push((*event).clone());
        }
    }

    for event_id in fetched_order {
        if appended.contains(&event_id) {
            continue;
        }
        if let Some(event) = fetched_by_id.get(&event_id) {
            local_store.append_event(event)?;
            appended.insert(event_id.clone());
            appended_events.event_ids.push(event_id);
            appended_events.events.push((*event).clone());
        }
    }

    Ok(appended_events)
}

fn verified_sync_events(events: Vec<SignedEvent>) -> Vec<SignedEvent> {
    events
        .into_iter()
        .filter(|event| {
            event.author_public_key.is_empty() || verify_self_contained_event(event).is_ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_missing_events_from_remote_inventory() {
        let local = EventInventory::from_event_ids([EventId("evt_a".to_owned())]);
        let remote = EventInventory::from_event_ids([
            EventId("evt_a".to_owned()),
            EventId("evt_b".to_owned()),
        ]);

        assert_eq!(
            local.missing_from(&remote),
            vec![EventId("evt_b".to_owned())]
        );
    }

    #[test]
    fn selects_missing_remote_event_ids_without_sorting_or_duplicates() {
        let local = EventInventory::from_event_ids([EventId("evt_b".to_owned())]);

        let missing = missing_remote_event_ids(
            &local,
            vec![
                EventId("evt_c".to_owned()),
                EventId("evt_a".to_owned()),
                EventId("evt_c".to_owned()),
                EventId("evt_b".to_owned()),
            ],
        );

        assert_eq!(
            missing,
            vec![EventId("evt_c".to_owned()), EventId("evt_a".to_owned())]
        );
    }

    #[test]
    fn selects_missing_events_from_remote_batch() {
        use chaft_types::{DeviceId, EventBody, SignableEvent, WorkspaceId};

        let workspace_id = WorkspaceId::new();
        let device_id = DeviceId("dev_test".to_owned());
        let event = SignedEvent::from_signed_bytes(
            SignableEvent::new(
                workspace_id,
                None,
                device_id,
                EventBody::WorkspaceCreated {
                    name: "Chaft".to_owned(),
                },
            ),
            vec![1, 2, 3],
        );
        let local = EventInventory::default();

        let missing = events_missing_from_local(&local, std::slice::from_ref(&event));

        assert_eq!(missing, vec![event]);
    }

    #[test]
    fn builds_workspace_inventory_from_store_event_ids() {
        use chaft_identity::DeviceIdentity;
        use chaft_types::{EventBody, SignableEvent, WorkspaceId};

        let store = EventStore::open_in_memory().unwrap();
        let identity = DeviceIdentity::generate();
        let workspace_id = WorkspaceId::new();
        let other_workspace_id = WorkspaceId::new();
        let workspace_event = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Chaft".to_owned(),
            },
        ));
        let mut invalid_workspace_event = identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Invalid".to_owned(),
            },
        ));
        invalid_workspace_event.signature[0] ^= 1;
        let other_workspace_event = identity.sign_event(SignableEvent::new(
            other_workspace_id,
            None,
            identity.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "Other".to_owned(),
            },
        ));
        store.append_event(&workspace_event).unwrap();
        store.append_event(&invalid_workspace_event).unwrap();
        store.append_event(&other_workspace_event).unwrap();

        let inventory = workspace_inventory_from_store(&store, &workspace_id).unwrap();

        assert_eq!(
            inventory,
            EventInventory::from_event_ids([workspace_event.event_id])
        );
    }
}
