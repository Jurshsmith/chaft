use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn cli(data_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_chaft-cli"));
    command.arg("--data-dir").arg(data_dir);
    command
}

fn run(mut command: Command, stdin: Option<&[u8]>) -> Output {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }

    let mut child = command.spawn().expect("CLI subprocess should start");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin should be available")
            .write_all(input)
            .expect("test input should be written");
    }
    child
        .wait_with_output()
        .expect("CLI subprocess should finish")
}

fn assert_secret_absent(output: &Output, secret: &str) {
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains(secret),
        "stdout contained the sentinel secret"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains(secret),
        "stderr contained the sentinel secret"
    );
}

fn assert_success(output: &Output) {
    assert!(output.status.success(), "CLI subprocess should succeed");
}

fn assert_failure(output: &Output) {
    assert!(!output.status.success(), "CLI subprocess should fail");
}

fn create_workspace(data_dir: &Path) -> String {
    let mut command = cli(data_dir);
    command.args(["init-workspace", "--name", "Secret input integration"]);
    let output = run(command, None);
    assert_success(&output);

    let created: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("workspace output should be JSON");
    created["workspaceId"]
        .as_str()
        .expect("workspace output should contain workspaceId")
        .to_owned()
}

fn write_secret_file(path: &Path, contents: &[u8], mode: u32) {
    std::fs::write(path, contents).expect("secret fixture should be written");
    set_file_mode(path, mode);
}

#[cfg(unix)]
fn set_file_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .expect("secret fixture permissions should be set");
}

#[cfg(not(unix))]
fn set_file_mode(_path: &Path, _mode: u32) {}

#[test]
fn recovery_stdin_secret_never_reaches_process_output() {
    let directory = tempfile::tempdir().unwrap();
    let workspace_id = create_workspace(directory.path());
    let secret = "SUBPROCESS_STDIN_SECRET_5e44806a";
    let input = format!("  {secret}  \n");

    let mut command = cli(directory.path());
    command.args([
        "export-recovery-bundle",
        "--workspace-id",
        &workspace_id,
        "--passphrase-stdin",
    ]);
    let output = run(command, Some(input.as_bytes()));

    assert_secret_absent(&output, secret);
    assert_success(&output);
}

#[test]
fn recovery_file_secret_never_reaches_process_output() {
    let directory = tempfile::tempdir().unwrap();
    let workspace_id = create_workspace(directory.path());
    let secret = "SUBPROCESS_FILE_SECRET_701f64c4";
    let secret_file = directory.path().join("recovery-passphrase");
    write_secret_file(&secret_file, format!("\t{secret} \r\n").as_bytes(), 0o600);

    let mut command = cli(directory.path());
    command
        .arg("export-recovery-bundle")
        .arg("--workspace-id")
        .arg(workspace_id)
        .arg("--passphrase-file")
        .arg(&secret_file);
    let output = run(command, None);

    assert_secret_absent(&output, secret);
    #[cfg(unix)]
    assert_success(&output);
    #[cfg(not(unix))]
    assert_failure(&output);
}

#[test]
fn deprecated_direct_secret_never_reaches_process_output() {
    let directory = tempfile::tempdir().unwrap();
    let workspace_id = create_workspace(directory.path());
    let secret = "SUBPROCESS_DEPRECATED_SECRET_f184065f";

    let mut command = cli(directory.path());
    command
        .arg("export-recovery-bundle")
        .arg("--workspace-id")
        .arg(workspace_id)
        .arg("--passphrase")
        .arg(secret);
    let output = run(command, None);

    assert_secret_absent(&output, secret);
    assert_success(&output);
}

#[test]
fn conflicting_sources_and_invalid_file_do_not_echo_secrets() {
    let directory = tempfile::tempdir().unwrap();
    let conflict_secret = "SUBPROCESS_CONFLICT_SECRET_2100703d";

    let mut conflict = cli(directory.path());
    conflict.args([
        "export-recovery-bundle",
        "--workspace-id",
        "wrk_cli_local",
        "--passphrase",
        conflict_secret,
        "--passphrase-stdin",
    ]);
    let conflict_output = run(conflict, None);

    assert_secret_absent(&conflict_output, conflict_secret);
    assert_failure(&conflict_output);

    let file_secret = "SUBPROCESS_INVALID_FILE_SECRET_19070abc";
    let secret_file = directory.path().join("insecure-recovery-passphrase");
    write_secret_file(&secret_file, format!("{file_secret}\n").as_bytes(), 0o644);

    let mut invalid_file = cli(directory.path());
    invalid_file
        .arg("export-recovery-bundle")
        .arg("--workspace-id")
        .arg("wrk_cli_local")
        .arg("--passphrase-file")
        .arg(&secret_file);
    let invalid_file_output = run(invalid_file, None);

    assert_secret_absent(&invalid_file_output, file_secret);
    assert_failure(&invalid_file_output);
}

#[test]
fn wrong_recovery_passphrase_runtime_failure_does_not_echo_secrets() {
    let source = tempfile::tempdir().unwrap();
    let workspace_id = create_workspace(source.path());
    let correct_secret = "SUBPROCESS_CORRECT_SECRET_c763f83a";
    let correct_input = format!("{correct_secret}\n");

    let mut export = cli(source.path());
    export.args([
        "export-recovery-bundle",
        "--workspace-id",
        &workspace_id,
        "--passphrase-stdin",
    ]);
    let export_output = run(export, Some(correct_input.as_bytes()));
    assert_secret_absent(&export_output, correct_secret);
    assert_success(&export_output);

    let bundle_file = source.path().join("recovery-bundle.json");
    std::fs::write(&bundle_file, &export_output.stdout)
        .expect("exported recovery bundle should be written");

    let destination = tempfile::tempdir().unwrap();
    let wrong_secret = "SUBPROCESS_WRONG_SECRET_34ecc3a5";
    let wrong_input = format!("{wrong_secret}\n");
    let mut import = cli(destination.path());
    import
        .arg("import-recovery-bundle")
        .arg("--bundle-file")
        .arg(&bundle_file)
        .arg("--passphrase-stdin");
    let import_output = run(import, Some(wrong_input.as_bytes()));

    assert_secret_absent(&import_output, correct_secret);
    assert_secret_absent(&import_output, wrong_secret);
    assert_failure(&import_output);
}
