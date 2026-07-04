use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use chaft_crypto::{ContentKey, seal_message_markdown};
use chaft_identity::DeviceIdentity;
use chaft_net::{ChaftTransport, PeerAddress, PeerId};
use chaft_net_direct::AuthorizedPublishTransport;
use chaft_net_iroh::IrohTransport;
use chaft_runtime::{
    ChannelKeyExport, LocalRuntime, PublishPeerEndpointRequest, RuntimePaths, WorkspaceKeyExport,
    WorkspaceRecoveryBundle,
};
use chaft_types::{
    ChannelId, DeviceId, DeviceKeyPackageId, EventBody, EventId, MessageId,
    PEER_ENDPOINT_ID_MAX_BYTES, PEER_ENDPOINT_LIST_MAX_ITEMS, PEER_ENDPOINT_MAX_BYTES,
    PEER_ENDPOINT_TRANSPORT_MAX_BYTES, REPLICA_RETENTION_HINT_MAX_BYTES, ReplicaStorageClass,
    SignableEvent, SignedEvent, WorkspaceId, WorkspaceRole, is_canonical_event_id_str,
    peer_endpoint_hint_is_supported, peer_endpoint_hint_transport_is_consistent,
    validate_channel_id_str, validate_device_id_str, validate_device_key_package_id_str,
    validate_event_id_str, validate_message_id_str, validate_workspace_id_str,
};
use clap::{Parser, Subcommand, ValueEnum};

const DEVICE_KEY_PACKAGE_FILE_MAX_BYTES: u64 = 64 * 1024;
const KEY_TRANSFER_JSON_FILE_MAX_BYTES: u64 = 256 * 1024;
const RECOVERY_BUNDLE_JSON_FILE_MAX_BYTES: u64 = 4 * 1024 * 1024;
const CLI_PATH_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Parser)]
#[command(name = "chaft")]
#[command(about = "Developer CLI for the Chaft local-first runtime")]
struct Cli {
    #[arg(long, global = true, default_value = "./data/chaft-cli")]
    data_dir: PathBuf,

    #[arg(long, global = true)]
    identity_file: Option<PathBuf>,

    #[arg(long, global = true)]
    identity_passphrase: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    DeviceId,
    Paths,
    ListWorkspaces,
    InitWorkspace {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "general")]
        channel: String,
    },
    CreateChannel {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        name: String,
        #[arg(long = "private")]
        is_private: bool,
    },
    UpdateDeviceProfile {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        display_name: String,
    },
    PublishDeviceKeyPackage {
        #[arg(long)]
        workspace_id: String,
        #[arg(long, default_value = "openmls/key-package")]
        protocol: String,
        #[arg(long)]
        key_package_file: PathBuf,
    },
    PublishPeerEndpoint {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        endpoint_id: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long, default_value = "auto")]
        transport: String,
        #[arg(long = "backup-peer")]
        is_backup_peer: bool,
        #[arg(long)]
        expires_at_ms: Option<i64>,
        #[arg(long)]
        replica_storage_class: Option<String>,
        #[arg(long)]
        replica_retention_hint: Option<String>,
    },
    PublishOpenMlsDeviceKeyPackage {
        #[arg(long)]
        workspace_id: String,
    },
    CreateOpenMlsWorkspaceGroup {
        #[arg(long)]
        workspace_id: String,
    },
    AddOpenMlsWorkspaceGroupMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        key_package_id: String,
    },
    RemoveOpenMlsWorkspaceGroupMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        device_id: String,
    },
    JoinOpenMlsWorkspaceGroup {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        source_event_id: Option<String>,
    },
    UpdateOpenMlsWorkspaceGroup {
        #[arg(long)]
        workspace_id: String,
    },
    UpdateWorkspaceOpenMlsGroups {
        #[arg(long)]
        workspace_id: String,
    },
    RotateWorkspaceForSuspectedCompromise {
        #[arg(long)]
        workspace_id: String,
    },
    DetectCompromise {
        #[arg(long)]
        workspace_id: String,
    },
    RespondCompromise {
        #[arg(long)]
        workspace_id: String,
    },
    ApplyOpenMlsWorkspaceGroupCommits {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        source_event_id: Option<String>,
    },
    CreateOpenMlsChannelGroup {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
    },
    AddOpenMlsChannelGroupMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        key_package_id: String,
    },
    RemoveOpenMlsChannelGroupMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        device_id: String,
    },
    JoinOpenMlsChannelGroup {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        source_event_id: Option<String>,
    },
    UpdateOpenMlsChannelGroup {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
    },
    ApplyOpenMlsChannelGroupCommits {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        source_event_id: Option<String>,
    },
    SendMessage {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        reply_to: Option<String>,
        #[arg(long)]
        text: String,
    },
    SendAttachment {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        reply_to: Option<String>,
        #[arg(long)]
        text: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "", hide_default_value = true)]
        media_type: String,
    },
    SaveAttachment {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        message_id: String,
        #[arg(long)]
        attachment_id: Option<String>,
        #[arg(long)]
        blob_hash: Option<String>,
        #[arg(long)]
        output: PathBuf,
    },
    PruneBlobs,
    EditMessage {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        message_id: String,
        #[arg(long)]
        text: String,
    },
    DeleteMessage {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        message_id: String,
    },
    AddReaction {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        message_id: String,
        #[arg(long)]
        reaction: String,
    },
    RemoveReaction {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        message_id: String,
        #[arg(long)]
        reaction: String,
    },
    MarkChannelRead {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
    },
    InviteMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        device_id: String,
        #[arg(long, value_enum, default_value_t = CliWorkspaceRole::Member)]
        role: CliWorkspaceRole,
    },
    RemoveMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        device_id: String,
    },
    RemoveMemberWithOpenMls {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        device_id: String,
    },
    RemoveMemberWithKeyRotation {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        device_id: String,
    },
    AddChannelMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        device_id: String,
    },
    RemoveChannelMember {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        device_id: String,
    },
    RemoveChannelMemberWithOpenMls {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        device_id: String,
    },
    RemoveChannelMemberWithKeyRotation {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
        #[arg(long)]
        device_id: String,
    },
    Snapshot {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        decrypt: bool,
    },
    SearchWorkspace {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        query: String,
    },
    ReindexWorkspaceSearch {
        #[arg(long)]
        workspace_id: String,
    },
    PublishQueue {
        #[arg(long)]
        workspace_id: String,
    },
    StorageHealth {
        #[arg(long)]
        workspace_id: String,
    },
    RepairStorageMetadata {
        #[arg(long)]
        workspace_id: String,
    },
    PublishWorkspace {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        peer: String,
    },
    PublishEventWithTrustSnapshot {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        event_id: String,
        #[arg(long)]
        peer: String,
    },
    BackupWorkspace {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        peer: String,
    },
    PullWorkspace {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        peer: String,
    },
    SyncWorkspace {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        peer: String,
    },
    RetryBlobTransfers {
        #[arg(long)]
        workspace_id: String,
        #[arg(long = "peer", required = true)]
        peers: Vec<String>,
    },
    RotateWorkspaceManualKeys {
        #[arg(long)]
        workspace_id: String,
    },
    ExportWorkspaceKey {
        #[arg(long)]
        workspace_id: String,
    },
    ImportWorkspaceKey {
        #[arg(long)]
        key_file: PathBuf,
    },
    ExportChannelKey {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        channel_id: String,
    },
    ImportChannelKey {
        #[arg(long)]
        key_file: PathBuf,
    },
    ExportRecoveryBundle {
        #[arg(long)]
        workspace_id: String,
        #[arg(long)]
        passphrase: String,
    },
    ImportRecoveryBundle {
        #[arg(long)]
        bundle_file: PathBuf,
        #[arg(long)]
        passphrase: String,
    },
    ExportTrustSnapshot {
        #[arg(long)]
        workspace_id: String,
    },
    SampleEvent,
    PublishSample {
        #[arg(long)]
        peer: String,
    },
    Inventory {
        #[arg(long)]
        peer: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliWorkspaceRole {
    Owner,
    Admin,
    Member,
    Guest,
}

impl From<CliWorkspaceRole> for WorkspaceRole {
    fn from(role: CliWorkspaceRole) -> Self {
        match role {
            CliWorkspaceRole::Owner => Self::Owner,
            CliWorkspaceRole::Admin => Self::Admin,
            CliWorkspaceRole::Member => Self::Member,
            CliWorkspaceRole::Guest => Self::Guest,
        }
    }
}

#[cfg(windows)]
const WINDOWS_CLI_STACK_BYTES: usize = 8 * 1024 * 1024;

#[cfg(windows)]
fn main() -> Result<()> {
    std::thread::Builder::new()
        .name("chaft-cli".to_string())
        .stack_size(WINDOWS_CLI_STACK_BYTES)
        .spawn(run_cli)?
        .join()
        .map_err(|_| anyhow!("chaft-cli worker thread panicked"))?
}

#[cfg(not(windows))]
fn main() -> Result<()> {
    run_cli()
}

#[tokio::main]
async fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = checked_cli_path_arg(cli.data_dir.clone(), "data directory")?;
    let identity_file = checked_optional_cli_path_arg(cli.identity_file.clone(), "identity file")?;

    match cli.command {
        Command::DeviceId => {
            let identity = resolve_identity(
                &data_dir,
                identity_file.as_deref(),
                cli.identity_passphrase.as_deref(),
            )?;
            println!("{}", identity.device_id().0);
        }
        Command::Paths => {
            let paths = checked_runtime_paths(&data_dir, identity_file.clone())?;
            println!("{}", serde_json::to_string_pretty(&paths)?);
        }
        Command::ListWorkspaces => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let workspaces = runtime.list_workspaces()?;
            println!("{}", serde_json::to_string_pretty(&workspaces)?);
        }
        Command::InitWorkspace { name, channel } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let created = runtime.create_workspace(name, channel)?;
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::CreateChannel {
            workspace_id,
            name,
            is_private,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let created =
                runtime.create_channel(workspace_id_arg(workspace_id)?, name, is_private)?;
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::UpdateDeviceProfile {
            workspace_id,
            display_name,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let updated =
                runtime.update_device_profile(workspace_id_arg(workspace_id)?, display_name)?;
            println!("{}", serde_json::to_string_pretty(&updated)?);
        }
        Command::PublishDeviceKeyPackage {
            workspace_id,
            protocol,
            key_package_file,
        } => {
            let key_package_file =
                checked_cli_path_arg(key_package_file, "device key package file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let published = runtime.publish_device_key_package(
                workspace_id_arg(workspace_id)?,
                protocol,
                read_device_key_package_file(&key_package_file)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::PublishPeerEndpoint {
            workspace_id,
            endpoint_id,
            endpoint,
            transport,
            is_backup_peer,
            expires_at_ms,
            replica_storage_class,
            replica_retention_hint,
        } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let (endpoint_id, endpoint, transport) =
                normalize_peer_endpoint_hint_inputs(endpoint_id, endpoint, transport)?;
            let replica_storage_class =
                parse_optional_replica_storage_class(replica_storage_class)?;
            let replica_retention_hint =
                normalize_optional_replica_retention_hint(replica_retention_hint)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let published = runtime.publish_peer_endpoint_with_replica_capability(
                PublishPeerEndpointRequest {
                    workspace_id,
                    endpoint_id,
                    endpoint,
                    transport,
                    is_backup_peer,
                    expires_at_ms,
                    replica_storage_class,
                    replica_retention_hint,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::PublishOpenMlsDeviceKeyPackage { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let published =
                runtime.publish_openmls_device_key_package(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::CreateOpenMlsWorkspaceGroup { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let created =
                runtime.create_openmls_workspace_group(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::AddOpenMlsWorkspaceGroupMember {
            workspace_id,
            key_package_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let added = runtime.add_openmls_workspace_group_member(
                workspace_id_arg(workspace_id)?,
                device_key_package_id_arg(key_package_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&added)?);
        }
        Command::RemoveOpenMlsWorkspaceGroupMember {
            workspace_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_openmls_workspace_group_member(
                workspace_id_arg(workspace_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::JoinOpenMlsWorkspaceGroup {
            workspace_id,
            source_event_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let joined = runtime.join_openmls_workspace_group(
                workspace_id_arg(workspace_id)?,
                source_event_id_arg(source_event_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&joined)?);
        }
        Command::UpdateOpenMlsWorkspaceGroup { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let updated =
                runtime.update_openmls_workspace_group(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&updated)?);
        }
        Command::UpdateWorkspaceOpenMlsGroups { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let updated =
                runtime.update_workspace_openmls_groups(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&updated)?);
        }
        Command::RotateWorkspaceForSuspectedCompromise { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let rotated = runtime
                .rotate_workspace_for_suspected_compromise(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&rotated)?);
        }
        Command::DetectCompromise { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let report =
                runtime.detect_workspace_compromise_signals(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Command::RespondCompromise { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let response =
                runtime.respond_to_workspace_compromise_signals(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::ApplyOpenMlsWorkspaceGroupCommits {
            workspace_id,
            source_event_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let applied = runtime.apply_openmls_workspace_group_commits(
                workspace_id_arg(workspace_id)?,
                source_event_id_arg(source_event_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&applied)?);
        }
        Command::CreateOpenMlsChannelGroup {
            workspace_id,
            channel_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let created = runtime.create_openmls_channel_group(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::AddOpenMlsChannelGroupMember {
            workspace_id,
            channel_id,
            key_package_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let added = runtime.add_openmls_channel_group_member(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_key_package_id_arg(key_package_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&added)?);
        }
        Command::RemoveOpenMlsChannelGroupMember {
            workspace_id,
            channel_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_openmls_channel_group_member(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::JoinOpenMlsChannelGroup {
            workspace_id,
            channel_id,
            source_event_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let joined = runtime.join_openmls_channel_group(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                source_event_id_arg(source_event_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&joined)?);
        }
        Command::UpdateOpenMlsChannelGroup {
            workspace_id,
            channel_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let updated = runtime.update_openmls_channel_group(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&updated)?);
        }
        Command::ApplyOpenMlsChannelGroupCommits {
            workspace_id,
            channel_id,
            source_event_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let applied = runtime.apply_openmls_channel_group_commits(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                source_event_id_arg(source_event_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&applied)?);
        }
        Command::SendMessage {
            workspace_id,
            channel_id,
            reply_to,
            text,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let workspace_id = workspace_id_arg(workspace_id)?;
            let channel_id = channel_id_arg(channel_id)?;
            let created = if let Some(reply_to) = reply_to {
                runtime.send_message_reply(
                    workspace_id,
                    channel_id,
                    message_id_arg(reply_to)?,
                    text,
                )?
            } else {
                runtime.send_message(workspace_id, channel_id, text)?
            };
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::SendAttachment {
            workspace_id,
            channel_id,
            reply_to,
            text,
            file,
            media_type,
        } => {
            let file = checked_cli_path_arg(file, "attachment file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let workspace_id = workspace_id_arg(workspace_id)?;
            let channel_id = channel_id_arg(channel_id)?;
            let created = runtime.send_message_with_attachment_file_reply(
                workspace_id,
                channel_id,
                reply_to.map(message_id_arg).transpose()?,
                text,
                file,
                media_type,
            )?;
            println!("{}", serde_json::to_string_pretty(&created)?);
        }
        Command::SaveAttachment {
            workspace_id,
            message_id,
            attachment_id,
            blob_hash,
            output,
        } => {
            let attachment_selector = match (attachment_id, blob_hash) {
                (Some(attachment_id), None) => attachment_id,
                (None, Some(blob_hash)) => blob_hash,
                (None, None) => {
                    return Err(anyhow!(
                        "save-attachment requires --attachment-id or legacy --blob-hash"
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(anyhow!(
                        "save-attachment accepts only one selector: --attachment-id or --blob-hash"
                    ));
                }
            };
            let output = checked_cli_path_arg(output, "attachment output file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let saved = runtime.save_attachment_to_file(
                workspace_id_arg(workspace_id)?,
                message_id_arg(message_id)?,
                attachment_selector,
                output,
            )?;
            println!("{}", serde_json::to_string_pretty(&saved)?);
        }
        Command::PruneBlobs => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let pruned = runtime.prune_unreferenced_blobs()?;
            println!("{}", serde_json::to_string_pretty(&pruned)?);
        }
        Command::EditMessage {
            workspace_id,
            message_id,
            text,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let edited = runtime.edit_message(
                workspace_id_arg(workspace_id)?,
                message_id_arg(message_id)?,
                text,
            )?;
            println!("{}", serde_json::to_string_pretty(&edited)?);
        }
        Command::DeleteMessage {
            workspace_id,
            message_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let deleted = runtime
                .delete_message(workspace_id_arg(workspace_id)?, message_id_arg(message_id)?)?;
            println!("{}", serde_json::to_string_pretty(&deleted)?);
        }
        Command::AddReaction {
            workspace_id,
            message_id,
            reaction,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let added = runtime.add_reaction(
                workspace_id_arg(workspace_id)?,
                message_id_arg(message_id)?,
                reaction,
            )?;
            println!("{}", serde_json::to_string_pretty(&added)?);
        }
        Command::RemoveReaction {
            workspace_id,
            message_id,
            reaction,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_reaction(
                workspace_id_arg(workspace_id)?,
                message_id_arg(message_id)?,
                reaction,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::MarkChannelRead {
            workspace_id,
            channel_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let marked = runtime
                .mark_channel_read(workspace_id_arg(workspace_id)?, channel_id_arg(channel_id)?)?;
            println!("{}", serde_json::to_string_pretty(&marked)?);
        }
        Command::InviteMember {
            workspace_id,
            device_id,
            role,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let invited = runtime.invite_member(
                workspace_id_arg(workspace_id)?,
                device_id_arg(device_id)?,
                role.into(),
            )?;
            println!("{}", serde_json::to_string_pretty(&invited)?);
        }
        Command::RemoveMember {
            workspace_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime
                .remove_member(workspace_id_arg(workspace_id)?, device_id_arg(device_id)?)?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::RemoveMemberWithOpenMls {
            workspace_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_member_with_openmls(
                workspace_id_arg(workspace_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::RemoveMemberWithKeyRotation {
            workspace_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_member_with_key_rotation(
                workspace_id_arg(workspace_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::AddChannelMember {
            workspace_id,
            channel_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let added = runtime.add_channel_member(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&added)?);
        }
        Command::RemoveChannelMember {
            workspace_id,
            channel_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_channel_member(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::RemoveChannelMemberWithOpenMls {
            workspace_id,
            channel_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_channel_member_with_openmls(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::RemoveChannelMemberWithKeyRotation {
            workspace_id,
            channel_id,
            device_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let removed = runtime.remove_channel_member_with_key_rotation(
                workspace_id_arg(workspace_id)?,
                channel_id_arg(channel_id)?,
                device_id_arg(device_id)?,
            )?;
            println!("{}", serde_json::to_string_pretty(&removed)?);
        }
        Command::Snapshot {
            workspace_id,
            decrypt,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let workspace_id = workspace_id_arg(workspace_id)?;
            let snapshot = if decrypt {
                runtime.decrypted_workspace_snapshot(workspace_id)?
            } else {
                runtime.workspace_snapshot(workspace_id)?
            };
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        Command::SearchWorkspace {
            workspace_id,
            query,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let results =
                runtime.search_workspace_messages(workspace_id_arg(workspace_id)?, query)?;
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        Command::ReindexWorkspaceSearch { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let indexed = runtime.reindex_workspace_search(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&indexed)?);
        }
        Command::PublishQueue { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let queue = runtime.workspace_publish_queue(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&queue)?);
        }
        Command::StorageHealth { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let health = runtime.workspace_storage_health(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&health)?);
        }
        Command::RepairStorageMetadata { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let repair =
                runtime.repair_workspace_storage_metadata(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&repair)?);
        }
        Command::PublishWorkspace { workspace_id, peer } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let peer = peer_address(peer)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let published = runtime
                .publish_workspace_direct(&transport, &peer, workspace_id)
                .await?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::PublishEventWithTrustSnapshot {
            workspace_id,
            event_id,
            peer,
        } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let event_id = event_id_arg(event_id)?;
            let peer = peer_address(peer)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let published = runtime
                .publish_event_direct_with_trust_snapshot(&transport, &peer, workspace_id, event_id)
                .await?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::BackupWorkspace { workspace_id, peer } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let peer = peer_address(peer)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let published = runtime
                .backup_workspace_direct_with_trust_snapshot(&transport, &peer, workspace_id)
                .await?;
            println!("{}", serde_json::to_string_pretty(&published)?);
        }
        Command::PullWorkspace { workspace_id, peer } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let peer = peer_address(peer)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let pulled = runtime
                .pull_workspace_direct(&transport, &peer, workspace_id)
                .await?;
            println!("{}", serde_json::to_string_pretty(&pulled)?);
        }
        Command::SyncWorkspace { workspace_id, peer } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let peer = peer_address(peer)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let synced = runtime
                .sync_workspace_direct(&transport, &peer, workspace_id)
                .await?;
            println!("{}", serde_json::to_string_pretty(&synced)?);
        }
        Command::RetryBlobTransfers {
            workspace_id,
            peers,
        } => {
            let workspace_id = workspace_id_arg(workspace_id)?;
            let peers = peer_addresses(peers)?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let transport = IrohTransport::from_environment();
            let retried = runtime
                .retry_pending_blob_transfers_direct(&transport, workspace_id, &peers)
                .await?;
            println!("{}", serde_json::to_string_pretty(&retried)?);
        }
        Command::RotateWorkspaceManualKeys { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let rotated = runtime.rotate_workspace_manual_keys(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&rotated)?);
        }
        Command::ExportWorkspaceKey { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let exported = runtime.export_workspace_key(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&exported)?);
        }
        Command::ImportWorkspaceKey { key_file } => {
            let key_file = checked_cli_path_arg(key_file, "key transfer JSON file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let key_bytes = read_key_transfer_json_file(&key_file)?;
            let exported = serde_json::from_slice::<WorkspaceKeyExport>(&key_bytes)?;
            let imported = runtime.import_workspace_key(exported)?;
            println!("{}", serde_json::to_string_pretty(&imported)?);
        }
        Command::ExportChannelKey {
            workspace_id,
            channel_id,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let exported = runtime
                .export_channel_key(workspace_id_arg(workspace_id)?, channel_id_arg(channel_id)?)?;
            println!("{}", serde_json::to_string_pretty(&exported)?);
        }
        Command::ImportChannelKey { key_file } => {
            let key_file = checked_cli_path_arg(key_file, "key transfer JSON file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let key_bytes = read_key_transfer_json_file(&key_file)?;
            let exported = serde_json::from_slice::<ChannelKeyExport>(&key_bytes)?;
            let imported = runtime.import_channel_key(exported)?;
            println!("{}", serde_json::to_string_pretty(&imported)?);
        }
        Command::ExportRecoveryBundle {
            workspace_id,
            passphrase,
        } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let exported = runtime
                .export_workspace_recovery_bundle(workspace_id_arg(workspace_id)?, &passphrase)?;
            println!("{}", serde_json::to_string_pretty(&exported)?);
        }
        Command::ImportRecoveryBundle {
            bundle_file,
            passphrase,
        } => {
            let bundle_file = checked_cli_path_arg(bundle_file, "recovery bundle JSON file")?;
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let bundle_bytes = read_recovery_bundle_json_file(&bundle_file)?;
            let bundle = serde_json::from_slice::<WorkspaceRecoveryBundle>(&bundle_bytes)?;
            let imported = runtime.import_workspace_recovery_bundle(bundle, &passphrase)?;
            println!("{}", serde_json::to_string_pretty(&imported)?);
        }
        Command::ExportTrustSnapshot { workspace_id } => {
            let runtime = open_runtime(
                &data_dir,
                identity_file.clone(),
                cli.identity_passphrase.as_deref(),
            )?;
            let snapshot = runtime.export_trust_snapshot(workspace_id_arg(workspace_id)?)?;
            println!("{}", serde_json::to_string_pretty(&snapshot)?);
        }
        Command::SampleEvent => {
            let identity = resolve_identity(
                &data_dir,
                identity_file.as_deref(),
                cli.identity_passphrase.as_deref(),
            )?;
            let signed = signed_sample_events(&identity)?;
            println!("{}", serde_json::to_string_pretty(&signed)?);
        }
        Command::PublishSample { peer } => {
            let peer = peer_address(peer)?;
            let transport = IrohTransport::from_environment();
            let identity = resolve_identity(
                &data_dir,
                identity_file.as_deref(),
                cli.identity_passphrase.as_deref(),
            )?;
            let signed = signed_sample_events(&identity)?;
            let event_ids = signed
                .iter()
                .map(|event| event.event_id.clone())
                .collect::<Vec<_>>();
            transport
                .publish_events_with_authorization(&peer, signed, Vec::new(), Vec::new())
                .await?;
            for event_id in event_ids {
                println!("{event_id}");
            }
        }
        Command::Inventory { peer } => {
            let peer = peer_address(peer)?;
            let transport = IrohTransport::from_environment();
            for event_id in transport.fetch_inventory(&peer).await? {
                println!("{event_id}");
            }
        }
    }

    Ok(())
}

fn open_runtime(
    data_dir: &Path,
    identity_file: Option<PathBuf>,
    identity_passphrase: Option<&str>,
) -> Result<LocalRuntime> {
    validate_runtime_paths(data_dir, identity_file.clone())?;
    LocalRuntime::open_with_identity_passphrase(data_dir, identity_file, identity_passphrase)
        .map_err(Into::into)
}

fn checked_cli_path_arg(path: PathBuf, label: &str) -> Result<PathBuf> {
    validate_cli_path(&path, label)?;
    Ok(path)
}

fn checked_optional_cli_path_arg(path: Option<PathBuf>, label: &str) -> Result<Option<PathBuf>> {
    path.map(|path| checked_cli_path_arg(path, label))
        .transpose()
}

fn checked_runtime_paths(data_dir: &Path, identity_file: Option<PathBuf>) -> Result<RuntimePaths> {
    let paths = RuntimePaths::new(data_dir, identity_file);
    validate_runtime_path_set(&paths)?;
    Ok(paths)
}

fn validate_runtime_paths(data_dir: &Path, identity_file: Option<PathBuf>) -> Result<()> {
    checked_runtime_paths(data_dir, identity_file).map(|_| ())
}

fn validate_runtime_path_set(paths: &RuntimePaths) -> Result<()> {
    validate_cli_path(&paths.data_dir, "data directory")?;
    validate_cli_path(&paths.identity_file, "identity file")?;
    validate_cli_path(&paths.event_store, "event store path")?;
    validate_cli_path(&paths.search_index, "search index path")?;
    validate_cli_path(&paths.blob_store, "blob store path")?;
    validate_cli_path(&paths.workspace_keys_dir, "workspace keys path")?;
    validate_cli_path(&paths.blob_transfer_ledger, "blob transfer ledger path")?;
    validate_cli_path(
        &paths.compromise_response_ledger,
        "compromise response ledger path",
    )?;
    Ok(())
}

fn validate_cli_path(path: &Path, label: &str) -> Result<()> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.is_empty() {
        return Err(anyhow!("{label} cannot be empty"));
    }
    if bytes.len() > CLI_PATH_MAX_BYTES {
        return Err(anyhow!(
            "{label} is too large ({} bytes, max {})",
            bytes.len(),
            CLI_PATH_MAX_BYTES
        ));
    }
    Ok(())
}

fn read_device_key_package_file(path: &Path) -> Result<Vec<u8>> {
    read_file_with_limit(
        path,
        DEVICE_KEY_PACKAGE_FILE_MAX_BYTES,
        "device key package",
    )
}

fn read_key_transfer_json_file(path: &Path) -> Result<Vec<u8>> {
    read_file_with_limit(path, KEY_TRANSFER_JSON_FILE_MAX_BYTES, "key transfer JSON")
}

fn read_recovery_bundle_json_file(path: &Path) -> Result<Vec<u8>> {
    read_file_with_limit(
        path,
        RECOVERY_BUNDLE_JSON_FILE_MAX_BYTES,
        "recovery bundle JSON",
    )
}

fn read_file_with_limit(path: &Path, max_bytes: u64, label: &str) -> Result<Vec<u8>> {
    validate_cli_path(path, label)?;
    let metadata = fs::metadata(path)?;
    if metadata.len() > max_bytes {
        return Err(anyhow!(
            "{} is too large ({} bytes, max {})",
            label,
            metadata.len(),
            max_bytes
        ));
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(max_bytes + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(max_bytes) as usize);
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(anyhow!(
            "{} is too large ({} bytes, max {})",
            label,
            bytes.len(),
            max_bytes
        ));
    }
    Ok(bytes)
}

fn resolve_identity(
    data_dir: &Path,
    identity_file: Option<&Path>,
    identity_passphrase: Option<&str>,
) -> Result<DeviceIdentity> {
    let identity_path = identity_file
        .map(Path::to_path_buf)
        .unwrap_or_else(|| data_dir.join("device.json"));
    validate_cli_path(&identity_path, "identity file")?;
    DeviceIdentity::load_or_generate_with_passphrase(identity_path, identity_passphrase)
        .map_err(Into::into)
}

fn signed_sample_events(identity: &DeviceIdentity) -> Result<Vec<SignedEvent>> {
    let workspace_id = WorkspaceId::new();
    let channel_id = ChannelId::new();
    let message_id = MessageId::new();
    let content_key = ContentKey::generate();
    let sealed_markdown = seal_message_markdown(
        "dev-sample-workspace-key",
        &content_key,
        &workspace_id,
        &channel_id,
        &message_id,
        "hello from an encrypted signed local-first event",
    )?;

    let workspace = SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::WorkspaceCreated {
            name: "Chaft Local".to_owned(),
        },
    );
    let channel = SignableEvent::new(
        workspace_id.clone(),
        None,
        identity.device_id().clone(),
        EventBody::ChannelCreated {
            channel_id: channel_id.clone(),
            name: "general".to_owned(),
            is_private: false,
        },
    );
    let message = SignableEvent::new(
        workspace_id,
        Some(channel_id),
        identity.device_id().clone(),
        EventBody::MessageCreatedEncrypted {
            message_id,
            sealed_markdown,
            attachments: Vec::new(),
        },
    );

    Ok(vec![
        identity.sign_event(workspace),
        identity.sign_event(channel),
        identity.sign_event(message),
    ])
}

fn workspace_id_arg(value: String) -> Result<WorkspaceId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("workspace ID is required"));
    }
    validate_workspace_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    Ok(WorkspaceId(value))
}

fn channel_id_arg(value: String) -> Result<ChannelId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("channel ID is required"));
    }
    validate_channel_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    Ok(ChannelId(value))
}

fn message_id_arg(value: String) -> Result<MessageId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("message ID is required"));
    }
    validate_message_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    Ok(MessageId(value))
}

fn event_id_arg(value: String) -> Result<EventId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("event ID is required"));
    }
    validate_event_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    if !is_canonical_event_id_str(&value) {
        return Err(anyhow!("event ID must be canonical"));
    }
    Ok(EventId(value))
}

fn source_event_id_arg(value: Option<String>) -> Result<Option<EventId>> {
    value.map(event_id_arg).transpose()
}

fn device_id_arg(value: String) -> Result<DeviceId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("device ID is required"));
    }
    validate_device_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    Ok(DeviceId(value))
}

fn device_key_package_id_arg(value: String) -> Result<DeviceKeyPackageId> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(anyhow!("device key package ID is required"));
    }
    validate_device_key_package_id_str(&value).map_err(|error| anyhow!(error.to_string()))?;
    Ok(DeviceKeyPackageId(value))
}

fn peer_address(endpoint: String) -> Result<PeerAddress> {
    let endpoint = normalize_peer_endpoint(endpoint)?;
    Ok(PeerAddress {
        peer_id: PeerId(endpoint.clone()),
        endpoint,
    })
}

fn normalize_peer_endpoint(endpoint: String) -> Result<String> {
    let endpoint = endpoint.trim().to_owned();
    if endpoint.is_empty() {
        return Err(anyhow!("peer endpoint is required"));
    }
    if endpoint.len() > PEER_ENDPOINT_MAX_BYTES {
        return Err(anyhow!(
            "peer endpoint is too large ({} bytes, max {})",
            endpoint.len(),
            PEER_ENDPOINT_MAX_BYTES
        ));
    }
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(anyhow!(
            "peer endpoint must be a direct TCP or native Iroh direct route"
        ));
    }
    Ok(endpoint)
}

fn peer_addresses(endpoints: Vec<String>) -> Result<Vec<PeerAddress>> {
    let endpoints = deduplicate_normalized_peer_endpoints(endpoints)?;
    if endpoints.len() > PEER_ENDPOINT_LIST_MAX_ITEMS {
        return Err(anyhow!(
            "peer endpoint list is too large ({} endpoints, max {})",
            endpoints.len(),
            PEER_ENDPOINT_LIST_MAX_ITEMS
        ));
    }
    Ok(endpoints
        .into_iter()
        .map(|endpoint| PeerAddress {
            peer_id: PeerId(endpoint.clone()),
            endpoint,
        })
        .collect())
}

fn deduplicate_normalized_peer_endpoints(endpoints: Vec<String>) -> Result<Vec<String>> {
    let mut deduplicated = Vec::new();
    let mut seen = BTreeSet::new();
    for endpoint in endpoints {
        let endpoint = normalize_peer_endpoint(endpoint)?;
        if seen.insert(endpoint.clone()) {
            deduplicated.push(endpoint);
        }
    }
    Ok(deduplicated)
}

fn normalize_peer_endpoint_hint_inputs(
    endpoint_id: String,
    endpoint: String,
    transport: String,
) -> Result<(String, String, String)> {
    let endpoint_id = endpoint_id.trim().to_owned();
    if endpoint_id.is_empty() {
        return Err(anyhow!("peer endpoint ID is required"));
    }
    if endpoint_id.len() > PEER_ENDPOINT_ID_MAX_BYTES {
        return Err(anyhow!(
            "peer endpoint ID is too large ({} bytes, max {})",
            endpoint_id.len(),
            PEER_ENDPOINT_ID_MAX_BYTES
        ));
    }

    let endpoint = normalize_peer_endpoint(endpoint)?;
    if !peer_endpoint_hint_is_supported(&endpoint) {
        return Err(anyhow!(
            "peer endpoint must be a direct TCP or native Iroh direct route"
        ));
    }

    let transport = infer_peer_endpoint_transport(&endpoint, &transport);
    if transport.is_empty() {
        return Err(anyhow!("peer endpoint transport is required"));
    }
    if transport.len() > PEER_ENDPOINT_TRANSPORT_MAX_BYTES {
        return Err(anyhow!(
            "peer endpoint transport is too large ({} bytes, max {})",
            transport.len(),
            PEER_ENDPOINT_TRANSPORT_MAX_BYTES
        ));
    }
    if !peer_endpoint_hint_transport_is_consistent(&endpoint, &transport) {
        return Err(anyhow!(
            "peer endpoint transport does not match the endpoint route"
        ));
    }

    Ok((endpoint_id, endpoint, transport))
}

fn parse_optional_replica_storage_class(
    value: Option<String>,
) -> Result<Option<ReplicaStorageClass>> {
    value
        .map(|value| {
            let normalized = value.trim().replace('-', "_");
            ReplicaStorageClass::from_wire(&normalized).ok_or_else(|| {
                anyhow!(
                    "replica storage class must be one of: {}",
                    ReplicaStorageClass::supported_wire_values().join(", ")
                )
            })
        })
        .transpose()
}

fn normalize_optional_replica_retention_hint(value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| {
            let value = value.trim().to_owned();
            if value.is_empty() {
                return Err(anyhow!("replica retention hint is required when provided"));
            }
            if value.len() > REPLICA_RETENTION_HINT_MAX_BYTES {
                return Err(anyhow!(
                    "replica retention hint is too large ({} bytes, max {})",
                    value.len(),
                    REPLICA_RETENTION_HINT_MAX_BYTES
                ));
            }
            Ok(value)
        })
        .transpose()
}

fn infer_peer_endpoint_transport(endpoint: &str, transport: &str) -> String {
    let transport = transport.trim();
    if !transport.eq_ignore_ascii_case("auto") {
        return transport.to_owned();
    }
    if endpoint.trim_start().starts_with("iroh://") {
        "iroh".to_owned()
    } else {
        "direct-tcp".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use chaft_types::{
        CHANNEL_ID_MAX_BYTES, DEVICE_ID_MAX_BYTES, DEVICE_KEY_PACKAGE_ID_MAX_BYTES,
        MESSAGE_ID_MAX_BYTES, WORKSPACE_ID_MAX_BYTES,
    };

    #[test]
    fn workspace_id_arg_trims_values() {
        let workspace_id = workspace_id_arg("  wrk_cli_local  ".to_owned()).unwrap();

        assert_eq!(workspace_id.0, "wrk_cli_local");
    }

    #[test]
    fn workspace_id_arg_rejects_blank_values() {
        let error = workspace_id_arg("   ".to_owned()).unwrap_err();

        assert!(error.to_string().contains("workspace ID is required"));
    }

    #[test]
    fn workspace_id_arg_rejects_oversized_values() {
        let error = workspace_id_arg("w".repeat(WORKSPACE_ID_MAX_BYTES + 1)).unwrap_err();

        assert!(error.to_string().contains("workspace ID is too large"));
    }

    #[test]
    fn cli_path_args_reject_blank_paths() {
        let error = checked_cli_path_arg(PathBuf::new(), "data directory").unwrap_err();

        assert!(error.to_string().contains("data directory cannot be empty"));
    }

    #[test]
    fn cli_path_args_reject_oversized_paths() {
        let error = checked_cli_path_arg(
            PathBuf::from("d".repeat(CLI_PATH_MAX_BYTES + 1)),
            "data directory",
        )
        .unwrap_err();

        assert!(error.to_string().contains("data directory is too large"));
    }

    #[test]
    fn cli_optional_path_args_reject_oversized_paths() {
        let error = checked_optional_cli_path_arg(
            Some(PathBuf::from("i".repeat(CLI_PATH_MAX_BYTES + 1))),
            "identity file",
        )
        .unwrap_err();

        assert!(error.to_string().contains("identity file is too large"));
    }

    #[test]
    fn cli_runtime_paths_reject_oversized_derived_paths() {
        let data_dir = PathBuf::from("d".repeat(CLI_PATH_MAX_BYTES));
        let error = checked_runtime_paths(&data_dir, None).unwrap_err();

        assert!(error.to_string().contains("identity file is too large"));
    }

    #[test]
    fn cli_file_reads_reject_oversized_paths_before_stat() {
        let error = read_file_with_limit(
            &PathBuf::from("k".repeat(CLI_PATH_MAX_BYTES + 1)),
            KEY_TRANSFER_JSON_FILE_MAX_BYTES,
            "key transfer JSON",
        )
        .unwrap_err();

        assert!(error.to_string().contains("key transfer JSON is too large"));
    }

    #[test]
    fn channel_id_arg_trims_and_rejects_invalid_values() {
        assert_eq!(
            channel_id_arg("  chn_cli_local  ".to_owned()).unwrap().0,
            "chn_cli_local"
        );

        let blank = channel_id_arg(" \n\t ".to_owned()).unwrap_err();
        assert!(blank.to_string().contains("channel ID is required"));

        let oversized = channel_id_arg("c".repeat(CHANNEL_ID_MAX_BYTES + 1)).unwrap_err();
        assert!(oversized.to_string().contains("channel ID is too large"));
    }

    #[test]
    fn message_id_arg_trims_and_rejects_invalid_values() {
        assert_eq!(
            message_id_arg("  msg_cli_local  ".to_owned()).unwrap().0,
            "msg_cli_local"
        );

        let blank = message_id_arg(" \n\t ".to_owned()).unwrap_err();
        assert!(blank.to_string().contains("message ID is required"));

        let oversized = message_id_arg("m".repeat(MESSAGE_ID_MAX_BYTES + 1)).unwrap_err();
        assert!(oversized.to_string().contains("message ID is too large"));
    }

    #[test]
    fn event_id_arg_requires_canonical_event_ids() {
        let canonical = format!("evt_{}", "0".repeat(64));
        assert_eq!(event_id_arg(canonical.clone()).unwrap().0, canonical);
        assert_eq!(
            event_id_arg(format!("  {canonical}  ")).unwrap().0,
            canonical
        );

        let blank = event_id_arg("   ".to_owned()).unwrap_err();
        assert!(blank.to_string().contains("event ID is required"));

        let non_canonical = event_id_arg("evt_NOT_CANONICAL".to_owned()).unwrap_err();
        assert!(
            non_canonical
                .to_string()
                .contains("event ID must be canonical")
        );
    }

    #[test]
    fn source_event_id_arg_uses_canonical_event_id_rules() {
        let canonical = format!("evt_{}", "1".repeat(64));
        assert_eq!(
            source_event_id_arg(Some(format!("  {canonical}  ")))
                .unwrap()
                .unwrap()
                .0,
            canonical
        );
        assert!(source_event_id_arg(None).unwrap().is_none());

        let non_canonical = source_event_id_arg(Some("evt_NOT_CANONICAL".to_owned())).unwrap_err();
        assert!(
            non_canonical
                .to_string()
                .contains("event ID must be canonical")
        );
    }

    #[test]
    fn device_id_arg_trims_and_rejects_invalid_values() {
        assert_eq!(
            device_id_arg("  dev_cli_local  ".to_owned()).unwrap().0,
            "dev_cli_local"
        );

        let blank = device_id_arg(" \n\t ".to_owned()).unwrap_err();
        assert!(blank.to_string().contains("device ID is required"));

        let oversized = device_id_arg("d".repeat(DEVICE_ID_MAX_BYTES + 1)).unwrap_err();
        assert!(oversized.to_string().contains("device ID is too large"));
    }

    #[test]
    fn device_key_package_id_arg_trims_and_rejects_invalid_values() {
        assert_eq!(
            device_key_package_id_arg("  dkp_cli_local  ".to_owned())
                .unwrap()
                .0,
            "dkp_cli_local"
        );

        let blank = device_key_package_id_arg(" \n\t ".to_owned()).unwrap_err();
        assert!(
            blank
                .to_string()
                .contains("device key package ID is required")
        );

        let oversized =
            device_key_package_id_arg("k".repeat(DEVICE_KEY_PACKAGE_ID_MAX_BYTES + 1)).unwrap_err();
        assert!(
            oversized
                .to_string()
                .contains("device key package ID is too large")
        );
    }

    #[test]
    fn peer_addresses_deduplicate_retry_peers_before_limit() {
        let endpoints = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| {
                if index % 2 == 0 {
                    " 127.0.0.1:7001 ".to_owned()
                } else {
                    "127.0.0.1:7002".to_owned()
                }
            })
            .collect::<Vec<_>>();

        let peers = peer_addresses(endpoints).unwrap();

        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].endpoint, "127.0.0.1:7001");
        assert_eq!(peers[1].endpoint, "127.0.0.1:7002");
    }

    #[test]
    fn peer_addresses_reject_oversized_unique_retry_peer_lists() {
        let endpoints = (0..=PEER_ENDPOINT_LIST_MAX_ITEMS)
            .map(|index| format!("127.0.0.1:{}", 10_000 + index))
            .collect::<Vec<_>>();

        let error = peer_addresses(endpoints).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("peer endpoint list is too large")
        );
    }

    #[test]
    fn peer_address_rejects_unsupported_routes() {
        for endpoint in [
            "https://central.example.invalid/sync",
            "wss://central.example.invalid/sync",
            "relay://relay.example.invalid/device",
            "discovery://workspace",
        ] {
            let error = peer_address(endpoint.to_owned()).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("direct TCP or native Iroh direct route"),
                "unexpected error for {endpoint}: {error}"
            );
        }
    }

    #[test]
    fn peer_addresses_reject_malformed_retry_peer_routes() {
        for endpoint in [
            "direct+tcp://127.0.0.1:0",
            "tcp://127.0.0.1:0",
            "127.0.0.1:0",
            "127.0.0.1:not-a-port",
            "direct+tcp://127.0.0.1",
        ] {
            let error = peer_addresses(vec![endpoint.to_owned()]).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("direct TCP or native Iroh direct route"),
                "unexpected error for {endpoint}: {error}"
            );
        }
    }

    #[test]
    fn peer_endpoint_hint_inputs_trim_infer_and_validate_policy() {
        let (endpoint_id, endpoint, transport) = normalize_peer_endpoint_hint_inputs(
            " desktop ".to_owned(),
            " direct+tcp://127.0.0.1:7777 ".to_owned(),
            " auto ".to_owned(),
        )
        .unwrap();

        assert_eq!(endpoint_id, "desktop");
        assert_eq!(endpoint, "direct+tcp://127.0.0.1:7777");
        assert_eq!(transport, "direct-tcp");
    }

    #[test]
    fn peer_endpoint_hint_inputs_reject_unsupported_routes() {
        let error = normalize_peer_endpoint_hint_inputs(
            "desktop".to_owned(),
            "relay://relay.example.invalid/device".to_owned(),
            "iroh-relay".to_owned(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("direct TCP or native Iroh direct route")
        );
    }

    #[test]
    fn peer_endpoint_hint_inputs_reject_transport_mismatches() {
        let error = normalize_peer_endpoint_hint_inputs(
            "desktop".to_owned(),
            "direct+tcp://127.0.0.1:7777".to_owned(),
            "iroh".to_owned(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("transport does not match the endpoint route")
        );
    }

    #[test]
    fn replica_storage_class_arg_accepts_wire_and_cli_spellings() {
        assert_eq!(
            parse_optional_replica_storage_class(Some(" full_history_with_blobs ".to_owned()))
                .unwrap(),
            Some(ReplicaStorageClass::FullHistoryWithBlobs)
        );
        assert_eq!(
            parse_optional_replica_storage_class(Some("full-history".to_owned())).unwrap(),
            Some(ReplicaStorageClass::FullHistory)
        );
    }

    #[test]
    fn replica_storage_class_arg_rejects_unknown_values() {
        let error =
            parse_optional_replica_storage_class(Some("central-server".to_owned())).unwrap_err();

        assert!(error.to_string().contains("replica storage class"));
    }

    #[test]
    fn replica_retention_hint_arg_trims_and_rejects_blank_or_oversized_values() {
        assert_eq!(
            normalize_optional_replica_retention_hint(Some(" 30d ".to_owned())).unwrap(),
            Some("30d".to_owned())
        );

        let blank = normalize_optional_replica_retention_hint(Some(" ".to_owned())).unwrap_err();
        assert!(blank.to_string().contains("retention hint is required"));

        let oversized = normalize_optional_replica_retention_hint(Some(
            "x".repeat(REPLICA_RETENTION_HINT_MAX_BYTES + 1),
        ))
        .unwrap_err();
        assert!(
            oversized
                .to_string()
                .contains("retention hint is too large")
        );
    }
}
