use chaft_runtime::{LocalRuntime, RuntimeError};
use chaft_types::{DeviceId, WorkspaceId, WorkspaceRole};

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
