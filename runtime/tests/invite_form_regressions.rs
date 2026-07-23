use std::path::Path;

use chaft_net::{PeerAddress, PeerId};
use chaft_net_direct::{DirectPeerServer, DirectTransport};
use chaft_runtime::{LocalRuntime, RuntimeError};
use chaft_store::EventStore;
use chaft_types::{ChannelId, DeviceId, WorkspaceId, WorkspaceRole};
use tokio::sync::oneshot;

struct ImportedInvite {
    workspace_id: WorkspaceId,
    channel_id: ChannelId,
    joiner_device_id: String,
}

fn import_invite_for_joiner(owner: &LocalRuntime, joiner: &LocalRuntime) -> ImportedInvite {
    let created = owner
        .create_workspace("Invite profile finalization", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "Engineering".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create invite");
    let claim = joiner
        .prepare_workspace_invite_claim(
            invite.artifact,
            "Sam Joiner".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare claim");
    let claimed = owner.claim_workspace_invite(claim).expect("claim invite");
    joiner
        .import_workspace_invite_response(claimed.response)
        .expect("import response");

    ImportedInvite {
        workspace_id,
        channel_id: ChannelId(created.channel_id),
        joiner_device_id: joiner.device_id().0.clone(),
    }
}

async fn sync_from_owner(
    owner_dir: &Path,
    joiner: &LocalRuntime,
    workspace_id: WorkspaceId,
) -> chaft_runtime::SyncedWorkspace {
    let owner_store = EventStore::open(owner_dir.join("events.db")).expect("open owner store");
    let server = DirectPeerServer::bind("127.0.0.1:0", owner_store)
        .await
        .expect("bind owner server");
    let peer = PeerAddress {
        peer_id: PeerId("owner".to_owned()),
        endpoint: server
            .local_addr()
            .expect("owner server address")
            .to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let synced = joiner
        .sync_workspace_direct(&DirectTransport, &peer, workspace_id)
        .await
        .expect("sync joined workspace");
    shutdown_tx.send(()).expect("stop owner server");
    server_task
        .await
        .expect("join owner server task")
        .expect("serve owner peer");
    synced
}

async fn pull_from_owner(
    owner_dir: &Path,
    joiner: &LocalRuntime,
    workspace_id: WorkspaceId,
) -> chaft_runtime::PulledWorkspace {
    let owner_store = EventStore::open(owner_dir.join("events.db")).expect("open owner store");
    let server = DirectPeerServer::bind("127.0.0.1:0", owner_store)
        .await
        .expect("bind owner server");
    let peer = PeerAddress {
        peer_id: PeerId("owner".to_owned()),
        endpoint: server
            .local_addr()
            .expect("owner server address")
            .to_string(),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move { server.serve_until_shutdown(shutdown_rx).await });
    let pulled = joiner
        .pull_workspace_direct(&DirectTransport, &peer, workspace_id)
        .await
        .expect("pull workspace history");
    shutdown_tx.send(()).expect("stop owner server");
    server_task
        .await
        .expect("join owner server task")
        .expect("serve owner peer");
    pulled
}

#[test]
fn invite_label_stays_distinct_from_the_joiners_display_name() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");

    let created = owner
        .create_workspace("Invite identity regression", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    owner
        .update_device_profile(workspace_id.clone(), "Avery Admin")
        .expect("set inviter profile");

    let invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "Design team".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create claimable invite");
    assert_eq!(invite.artifact.invite_label(), "Design team");
    assert_eq!(
        serde_json::to_value(&invite.artifact).expect("serialize invite")["displayName"],
        "Design team",
        "the legacy displayName wire field is invite metadata, not member identity"
    );

    let claim = joiner
        .prepare_workspace_invite_claim(
            invite.artifact,
            "Sam Joiner".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare invite claim");
    assert_eq!(claim.payload.display_name, "Sam Joiner");
    assert_ne!(claim.payload.display_name, "Design team");
    assert_eq!(claim.payload.source_display_name, "Avery Admin");

    let request_id = claim.payload.request_id.clone();
    owner
        .claim_workspace_invite(claim)
        .expect("accept signed invite claim");
    let snapshot = owner
        .workspace_snapshot(workspace_id)
        .expect("read workspace snapshot");
    let request = snapshot
        .join_requests
        .iter()
        .find(|request| request.request_id == request_id)
        .expect("claimed invite should record its join request");

    assert_eq!(request.display_name, "Sam Joiner");
    assert_ne!(request.display_name, "Design team");
    assert_eq!(request.source_display_name, "Avery Admin");
}

#[test]
fn recovery_bundle_passphrases_preserve_significant_surrounding_whitespace() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let importer_dir = tempfile::tempdir().expect("importer temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let importer = LocalRuntime::open(importer_dir.path(), None).expect("open importer runtime");

    let created = owner
        .create_workspace("Exact passphrase regression", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let exact_passphrase = "  correct horse battery staple\t";
    let bundle = owner
        .export_workspace_recovery_bundle(workspace_id.clone(), exact_passphrase)
        .expect("export recovery bundle");

    assert!(
        importer
            .import_workspace_recovery_bundle(bundle.clone(), exact_passphrase.trim())
            .is_err(),
        "trimming a non-blank recovery passphrase must change its cryptographic value"
    );

    let imported = importer
        .import_workspace_recovery_bundle(bundle, exact_passphrase)
        .expect("the exact recovery passphrase should decrypt the bundle");
    assert_eq!(imported.workspace_id, workspace_id.0);
}

#[test]
fn join_requests_require_the_joiners_display_name_at_runtime_boundaries() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");

    let created = owner
        .create_workspace("Required join identity", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            String::new(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create claimable invite");

    let claim_error = joiner
        .prepare_workspace_invite_claim(
            invite.artifact,
            " \t ".to_owned(),
            String::new(),
            String::new(),
        )
        .expect_err("an invite claim without a joiner name must fail");
    assert!(matches!(claim_error, RuntimeError::DisplayNameRequired));

    let request_error = owner
        .record_workspace_join_request(
            workspace_id,
            "req_missing_name".to_owned(),
            DeviceId("dev_requester".to_owned()),
            "   ".to_owned(),
            String::new(),
            "request_access".to_owned(),
            String::new(),
            String::new(),
            "approval_required".to_owned(),
        )
        .expect_err("an access request without a joiner name must fail");
    assert!(matches!(request_error, RuntimeError::DisplayNameRequired));
}

#[tokio::test]
async fn invite_profile_is_finalized_after_history_pull_and_published_without_a_user_message() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let imported = import_invite_for_joiner(&owner, &joiner);

    let empty = joiner
        .decrypted_workspace_snapshot(imported.workspace_id.clone())
        .expect("an imported key may precede workspace history");
    assert!(empty.members.is_empty());
    assert!(empty.profiles.is_empty());
    assert!(matches!(
        joiner.update_device_profile(imported.workspace_id.clone(), "Sam Joiner"),
        Err(RuntimeError::WorkspaceHasNoEvents { .. })
    ));

    drop(owner);
    let first_sync =
        sync_from_owner(owner_dir.path(), &joiner, imported.workspace_id.clone()).await;
    assert_eq!(first_sync.pulled.invite_profile_event_count, 3);
    assert_eq!(first_sync.pulled.invite_profile_event_ids.len(), 3);
    assert!(first_sync.pulled.invite_profile_event_ids.iter().all(|id| {
        first_sync
            .published
            .published_event_ids
            .iter()
            .any(|published| published == id)
    }));

    let joiner_snapshot = joiner
        .workspace_snapshot(imported.workspace_id.clone())
        .expect("read finalized joiner snapshot");
    assert!(joiner_snapshot.profiles.iter().any(|profile| {
        profile.device_id == imported.joiner_device_id && profile.display_name == "Sam Joiner"
    }));
    assert_eq!(joiner_snapshot.person_profiles.len(), 1);
    assert_eq!(
        joiner_snapshot.person_profiles[0].display_name,
        "Sam Joiner"
    );
    assert_eq!(joiner_snapshot.person_device_links.len(), 1);

    joiner
        .send_message(
            imported.workspace_id.clone(),
            imported.channel_id,
            "profile propagation is immediate",
        )
        .expect("send joiner message");
    let second_sync =
        sync_from_owner(owner_dir.path(), &joiner, imported.workspace_id.clone()).await;
    assert_eq!(second_sync.pulled.invite_profile_event_count, 0);

    let reopened_owner =
        LocalRuntime::open(owner_dir.path(), None).expect("reopen owner runtime after sync");
    let owner_snapshot = reopened_owner
        .decrypted_workspace_snapshot(imported.workspace_id)
        .expect("read owner snapshot");
    let message = owner_snapshot
        .timeline
        .iter()
        .find(|item| item.body == "profile propagation is immediate")
        .expect("owner receives joiner message");
    assert_eq!(
        message.author_device_id.as_deref(),
        Some(imported.joiner_device_id.as_str())
    );
    assert_eq!(message.author_display_name.as_deref(), Some("Sam Joiner"));
}

#[tokio::test]
async fn pending_invite_profile_finalization_survives_runtime_restart_and_is_idempotent() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let imported = import_invite_for_joiner(&owner, &joiner);
    drop(joiner);
    drop(owner);

    let reopened_joiner =
        LocalRuntime::open(joiner_dir.path(), None).expect("reopen joiner runtime");
    let first_sync = sync_from_owner(
        owner_dir.path(),
        &reopened_joiner,
        imported.workspace_id.clone(),
    )
    .await;
    assert_eq!(first_sync.pulled.invite_profile_event_count, 3);

    // Simulate a crash after all three profile events were durable but before
    // the receipt's final completion write. Recovery must observe the existing
    // events and finish without appending duplicates.
    let receipt_path = std::fs::read_dir(joiner_dir.path().join("invite-claims"))
        .expect("read claim receipts")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .expect("claim receipt path");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read claim receipt"))
            .expect("parse claim receipt");
    let receipt = receipt.as_object_mut().expect("claim receipt object");
    receipt.insert("profilePending".to_owned(), serde_json::Value::Bool(true));
    receipt.insert(
        "profileFinalized".to_owned(),
        serde_json::Value::Bool(false),
    );
    receipt.insert(
        "profileEventIds".to_owned(),
        serde_json::Value::Array(Vec::new()),
    );
    std::fs::write(
        &receipt_path,
        serde_json::to_vec(&receipt).expect("serialize interrupted receipt"),
    )
    .expect("write interrupted receipt");

    let second_sync = sync_from_owner(
        owner_dir.path(),
        &reopened_joiner,
        imported.workspace_id.clone(),
    )
    .await;
    assert_eq!(second_sync.pulled.invite_profile_event_count, 0);
    let snapshot = reopened_joiner
        .workspace_snapshot(imported.workspace_id)
        .expect("read restarted joiner snapshot");
    assert_eq!(
        snapshot
            .profiles
            .iter()
            .filter(|profile| profile.device_id == imported.joiner_device_id)
            .count(),
        1
    );
    assert_eq!(snapshot.person_profiles.len(), 1);
    assert_eq!(snapshot.person_device_links.len(), 1);
}

#[tokio::test]
async fn imported_key_recovers_modern_unmarked_receipt_without_a_global_directory_scan() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let imported = import_invite_for_joiner(&owner, &joiner);

    // Recreate the durable state left by the old key-before-marker ordering:
    // the key exists, but the modern receipt still says no profile work is
    // pending and there is no marker.
    let claims_dir = joiner_dir.path().join("invite-claims");
    let receipt_path = std::fs::read_dir(&claims_dir)
        .expect("read claim receipts")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .expect("claim receipt path");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read claim receipt"))
            .expect("parse claim receipt");
    let receipt = receipt.as_object_mut().expect("claim receipt object");
    receipt.insert("profilePending".to_owned(), serde_json::Value::Bool(false));
    receipt.insert(
        "profileFinalized".to_owned(),
        serde_json::Value::Bool(false),
    );
    receipt.insert(
        "profileEventIds".to_owned(),
        serde_json::Value::Array(Vec::new()),
    );
    std::fs::write(
        &receipt_path,
        serde_json::to_vec(receipt).expect("serialize interrupted receipt"),
    )
    .expect("write interrupted receipt");
    let marker_dir = joiner_dir.path().join("invite-profile-finalization");
    match std::fs::remove_dir_all(marker_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("remove profile marker directory: {error}"),
    }

    // These files would place the real receipt beyond the former global 1,024
    // entry cutoff. Active-chain recovery must load its exact hashed path.
    for index in 0..1_025 {
        std::fs::write(
            claims_dir.join(format!(".scan-padding-{index:04}.json")),
            b"{}",
        )
        .expect("write scan padding receipt");
    }
    drop(joiner);
    drop(owner);

    let reopened_joiner =
        LocalRuntime::open(joiner_dir.path(), None).expect("reopen interrupted joiner runtime");
    let synced = sync_from_owner(
        owner_dir.path(),
        &reopened_joiner,
        imported.workspace_id.clone(),
    )
    .await;
    assert_eq!(synced.pulled.invite_profile_event_count, 3);
    let snapshot = reopened_joiner
        .workspace_snapshot(imported.workspace_id)
        .expect("read recovered joiner profile");
    assert!(snapshot.profiles.iter().any(|profile| {
        profile.device_id == imported.joiner_device_id && profile.display_name == "Sam Joiner"
    }));
}

#[tokio::test]
async fn old_claim_receipt_without_a_name_or_pending_marker_recovers_from_exact_approved_history() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let imported = import_invite_for_joiner(&owner, &joiner);
    drop(joiner);
    drop(owner);

    let claims_dir = joiner_dir.path().join("invite-claims");
    let receipt_path = std::fs::read_dir(&claims_dir)
        .expect("read claim receipts")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .expect("claim receipt path");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read current claim receipt"))
            .expect("parse current claim receipt");
    let receipt_object = receipt.as_object_mut().expect("claim receipt object");
    receipt_object.remove("displayName");
    receipt_object.remove("profilePending");
    receipt_object.remove("profileFinalized");
    receipt_object.remove("profileEventIds");
    let mut legacy_bytes = serde_json::to_vec(&receipt).expect("serialize legacy receipt");
    legacy_bytes.push(b'\n');
    std::fs::write(&receipt_path, legacy_bytes).expect("write legacy claim receipt");
    let marker_dir = joiner_dir.path().join("invite-profile-finalization");
    match std::fs::remove_dir_all(marker_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("remove current pending marker: {error}"),
    }

    let reopened_joiner =
        LocalRuntime::open(joiner_dir.path(), None).expect("reopen legacy joiner runtime");
    let synced = sync_from_owner(
        owner_dir.path(),
        &reopened_joiner,
        imported.workspace_id.clone(),
    )
    .await;
    assert_eq!(synced.pulled.invite_profile_event_count, 3);
    let snapshot = reopened_joiner
        .workspace_snapshot(imported.workspace_id)
        .expect("read recovered legacy joiner snapshot");
    assert!(snapshot.profiles.iter().any(|profile| {
        profile.device_id == imported.joiner_device_id && profile.display_name == "Sam Joiner"
    }));
}

#[tokio::test]
async fn history_first_response_import_finalizes_immediately_and_next_sync_publishes_profile() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let created = owner
        .create_workspace("History first invite", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "History first".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create invite");
    let claim = joiner
        .prepare_workspace_invite_claim(
            invite.artifact,
            "History First Joiner".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare claim");
    let claimed = owner
        .claim_workspace_invite(claim)
        .expect("approve invite claim");
    drop(owner);

    let pulled = pull_from_owner(owner_dir.path(), &joiner, workspace_id.clone()).await;
    assert!(pulled.fetched_event_count > 0);
    assert_eq!(pulled.invite_profile_event_count, 0);
    assert!(matches!(
        joiner.export_workspace_key(workspace_id.clone()),
        Err(RuntimeError::InvalidWorkspaceKey)
    ));
    assert!(
        joiner
            .workspace_snapshot(workspace_id.clone())
            .expect("read pre-import snapshot")
            .profiles
            .is_empty()
    );

    joiner
        .import_workspace_invite_response(claimed.response)
        .expect("import response after history");
    let snapshot = joiner
        .workspace_snapshot(workspace_id.clone())
        .expect("read immediately finalized profile");
    let device_profile_event_id = snapshot
        .profiles
        .iter()
        .find(|profile| profile.device_id == joiner.device_id().0)
        .filter(|profile| profile.display_name == "History First Joiner")
        .map(|profile| profile.updated_event_id.clone())
        .expect("canonical device profile is written during import");
    let person_profile_event_id = snapshot
        .person_profiles
        .iter()
        .find(|profile| profile.display_name == "History First Joiner")
        .map(|profile| profile.updated_event_id.clone())
        .expect("canonical person profile is written during import");
    let person_link_event_id = snapshot
        .person_device_links
        .iter()
        .find(|link| link.device_id == joiner.device_id().0)
        .map(|link| link.linked_event_id.clone())
        .expect("canonical person link is written during import");

    let synced = sync_from_owner(owner_dir.path(), &joiner, workspace_id).await;
    assert_eq!(synced.pulled.invite_profile_event_count, 0);
    for event_id in [
        device_profile_event_id,
        person_profile_event_id,
        person_link_event_id,
    ] {
        assert!(
            synced.published.published_event_ids.contains(&event_id),
            "the next sync's initial publish includes {event_id}"
        );
    }
}

#[tokio::test]
async fn remove_and_reinvite_selects_the_active_claim_instead_of_a_stale_pending_marker() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let created = owner
        .create_workspace("Reinvite workspace", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);

    let stale_invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "First membership".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create first invite");
    let stale_claim = joiner
        .prepare_workspace_invite_claim(
            stale_invite.artifact,
            "Stale Invite Name".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare first claim");
    let stale_response = owner
        .claim_workspace_invite(stale_claim)
        .expect("approve first claim")
        .response;
    joiner
        .import_workspace_invite_response(stale_response)
        .expect("import first response before history");
    owner
        .remove_member(workspace_id.clone(), joiner.device_id().clone())
        .expect("remove first membership");

    let active_invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "Second membership".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create second invite");
    let active_claim = joiner
        .prepare_workspace_invite_claim(
            active_invite.artifact,
            "Current Reinvite Name".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare second claim");
    let active_response = owner
        .claim_workspace_invite(active_claim)
        .expect("approve second claim")
        .response;
    joiner
        .import_workspace_invite_response(active_response)
        .expect("a stale per-claim marker must not block the active response");
    drop(owner);

    let synced = sync_from_owner(owner_dir.path(), &joiner, workspace_id.clone()).await;
    assert_eq!(synced.pulled.invite_profile_event_count, 3);
    let snapshot = joiner
        .workspace_snapshot(workspace_id)
        .expect("read reinvited profile");
    assert!(snapshot.profiles.iter().any(|profile| {
        profile.device_id == joiner.device_id().0 && profile.display_name == "Current Reinvite Name"
    }));
    assert!(
        !snapshot
            .profiles
            .iter()
            .any(|profile| profile.display_name == "Stale Invite Name")
    );
}

#[tokio::test]
async fn modern_receipt_name_must_match_the_canonical_signed_join_request() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let imported = import_invite_for_joiner(&owner, &joiner);

    let receipt_path = std::fs::read_dir(joiner_dir.path().join("invite-claims"))
        .expect("read claim receipts")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .expect("claim receipt path");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read claim receipt"))
            .expect("parse claim receipt");
    receipt
        .as_object_mut()
        .expect("claim receipt object")
        .insert(
            "displayName".to_owned(),
            serde_json::Value::String("Locally Substituted Name".to_owned()),
        );
    std::fs::write(
        receipt_path,
        serde_json::to_vec(&receipt).expect("serialize changed receipt"),
    )
    .expect("write changed receipt");
    drop(owner);

    let synced = sync_from_owner(owner_dir.path(), &joiner, imported.workspace_id.clone()).await;
    assert_eq!(synced.pulled.invite_profile_event_count, 0);
    let snapshot = joiner
        .workspace_snapshot(imported.workspace_id)
        .expect("read safely declined profile snapshot");
    assert!(
        snapshot
            .profiles
            .iter()
            .all(|profile| profile.device_id != imported.joiner_device_id)
    );
}

#[tokio::test]
async fn duplicate_join_request_provenance_is_declined_instead_of_substituted() {
    let owner_dir = tempfile::tempdir().expect("owner temp directory");
    let joiner_dir = tempfile::tempdir().expect("joiner temp directory");
    let owner = LocalRuntime::open(owner_dir.path(), None).expect("open owner runtime");
    let joiner = LocalRuntime::open(joiner_dir.path(), None).expect("open joiner runtime");
    let created = owner
        .create_workspace("Ambiguous provenance", "general")
        .expect("create workspace");
    let workspace_id = WorkspaceId(created.workspace_id);
    let invite = owner
        .create_workspace_invite(
            workspace_id.clone(),
            "Ambiguous provenance".to_owned(),
            WorkspaceRole::Member,
            String::new(),
            String::new(),
            String::new(),
        )
        .expect("create invite");
    let invite_id = invite.invite_id.clone();
    let claim = joiner
        .prepare_workspace_invite_claim(
            invite.artifact,
            "Canonical Joiner".to_owned(),
            String::new(),
            String::new(),
        )
        .expect("prepare claim");
    let request_id = claim.payload.request_id.clone();
    let response = owner
        .claim_workspace_invite(claim)
        .expect("approve claim")
        .response;
    joiner
        .import_workspace_invite_response(response)
        .expect("import response before history");

    // Exercise the compatibility path too: an old receipt has no locally
    // persisted display name or profile-finalization state and therefore must
    // derive identity exclusively from one unambiguous signed request chain.
    let receipt_path = std::fs::read_dir(joiner_dir.path().join("invite-claims"))
        .expect("read claim receipts")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .expect("claim receipt path");
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).expect("read claim receipt"))
            .expect("parse claim receipt");
    let receipt = receipt.as_object_mut().expect("claim receipt object");
    receipt.remove("displayName");
    receipt.remove("profilePending");
    receipt.remove("profileFinalized");
    receipt.remove("profileEventIds");
    std::fs::write(
        receipt_path,
        serde_json::to_vec(receipt).expect("serialize legacy receipt"),
    )
    .expect("write legacy receipt");
    std::fs::remove_dir_all(joiner_dir.path().join("invite-profile-finalization"))
        .expect("remove modern pending marker");

    owner
        .record_workspace_join_request(
            workspace_id.clone(),
            request_id,
            joiner.device_id().clone(),
            "Substituted Name".to_owned(),
            String::new(),
            "invite_claim".to_owned(),
            invite_id,
            String::new(),
            "preapproved".to_owned(),
        )
        .expect("append a duplicate request projection");
    drop(owner);

    let synced = sync_from_owner(owner_dir.path(), &joiner, workspace_id.clone()).await;
    assert_eq!(synced.pulled.invite_profile_event_count, 0);
    let snapshot = joiner
        .workspace_snapshot(workspace_id)
        .expect("read declined ambiguous profile snapshot");
    assert!(
        snapshot
            .profiles
            .iter()
            .all(|profile| profile.device_id != joiner.device_id().0)
    );
}
