use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ffi::OsString,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use chaft_core::WorkspaceState;
use chaft_crypto::{
    open_attachment_blob, open_message_markdown, sealed_payload_from_encrypted_blob_ref,
};
use chaft_identity::verify_self_contained_event;
use chaft_types::{
    AttachmentRef, ChannelId, DeviceId, EventBody, MessageId, PersonId, SignedEvent, WorkspaceId,
    WorkspaceRole,
};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{LocalRuntime, RuntimeError, WorkspaceKey, validate_runtime_path};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[cfg(not(unix))]
use std::fs::OpenOptions;

pub const PORTABLE_EXPORT_SCHEMA_VERSION: u32 = 2;
pub const PORTABLE_EXPORT_KIND: &str = "chaft.portable-workspace.v2";
const PORTABLE_EXPORT_SCHEMA_PATH: &str = "schemas/chaft-portable-workspace-v2.schema.json";

static PORTABLE_EXPORT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableWorkspaceExport {
    pub schema_version: u32,
    pub kind: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub generated_at: String,
    pub output_path: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub channel_count: usize,
    pub member_count: usize,
    pub message_count: usize,
    pub attachment_count: usize,
    pub included_attachment_count: usize,
    pub missing_attachment_count: usize,
    pub unavailable_message_body_count: usize,
    pub gap_count: usize,
    pub invalid_signature_count: usize,
    pub corrupt_event_count: Option<usize>,
    pub storage_integrity_assessment: String,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableExportManifest {
    schema_version: u32,
    kind: String,
    generated_at: String,
    generator: PortableGenerator,
    workspace: PortableWorkspaceRecord,
    selection: PortableSelection,
    cutoff: PortableCutoff,
    counts: PortableCounts,
    files: PortableFiles,
    integrity: PortableIntegrity,
    completeness: PortableCompletenessSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableGenerator {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableSelection {
    scope: String,
    coverage_scope: String,
    sync_completeness: String,
    readable_channels_only: bool,
    includes_private_channels: bool,
    includes_direct_messages: bool,
    includes_attachments: bool,
    message_state: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableCutoff {
    captured_at: String,
    accepted_event_count: usize,
    parseable_event_count: usize,
    applied_event_count: usize,
    event_inventory_blake3: String,
    causal_frontier_event_ids: Vec<String>,
    source_changed_during_capture: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableCounts {
    channels: usize,
    members: usize,
    messages: usize,
    attachments: usize,
    included_attachments: usize,
    missing_attachments: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableFiles {
    offline_index: String,
    workspace: String,
    channels: String,
    members: String,
    messages: String,
    attachments: String,
    completeness: String,
    schema: String,
    checksums: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableIntegrity {
    algorithm: String,
    checksums_file: String,
    coverage: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableCompletenessSummary {
    status: String,
    warning_count: usize,
    missing_attachment_count: usize,
    unavailable_message_body_count: usize,
    gap_count: usize,
    invalid_signature_count: usize,
    corrupt_event_count: Option<usize>,
    storage_integrity_assessment: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableCompletenessReport {
    schema_version: u32,
    status: String,
    source_changed_during_capture: bool,
    missing_attachments: Vec<PortableMissingAttachment>,
    unavailable_message_bodies: Vec<PortableUnavailableMessageBody>,
    history_gaps: Vec<PortableHistoryGap>,
    invalid_signature_count: usize,
    corrupt_event_count: Option<usize>,
    storage_integrity_assessment: String,
    warnings: Vec<PortableWarning>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableWarning {
    code: String,
    count: usize,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableMissingAttachment {
    attachment_id: String,
    message_id: String,
    display_name: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableUnavailableMessageBody {
    message_id: String,
    channel_id: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableHistoryGap {
    event_id: String,
    missing_parent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableWorkspaceRecord {
    schema_version: u32,
    workspace_id: String,
    name: String,
    access_policy: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableChannelRecord {
    schema_version: u32,
    channel_id: String,
    name: String,
    topic: String,
    archived: bool,
    is_private: bool,
    direct_message: bool,
    member_device_ids: Vec<String>,
    direct_message_participant_device_ids: Vec<String>,
    html_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableMemberRecord {
    schema_version: u32,
    actor_id: String,
    actor_kind: String,
    device_id: String,
    person_id: Option<String>,
    display_name: String,
    avatar_id: String,
    role: Option<String>,
    active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableIdentity {
    actor_id: String,
    actor_kind: String,
    device_id: String,
    person_id: Option<String>,
    display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableReaction {
    value: String,
    count: u32,
    actor_device_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableMessageRecord {
    schema_version: u32,
    message_id: String,
    channel_id: String,
    reply_to_message_id: Option<String>,
    author: PortableIdentity,
    created_event_id: String,
    created_at: String,
    created_at_unix_ms: i64,
    created_at_logical: u32,
    edited_event_id: Option<String>,
    edited_at: Option<String>,
    deleted: bool,
    body_state: String,
    markdown: String,
    attachment_ids: Vec<String>,
    reactions: Vec<PortableReaction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableAttachmentRecord {
    schema_version: u32,
    attachment_id: String,
    source_attachment_id: Option<String>,
    message_id: String,
    channel_id: String,
    attachment_index: usize,
    display_name: String,
    media_type: String,
    declared_plaintext_bytes: u64,
    source_blob_hash: String,
    availability: String,
    archive_path: Option<String>,
    plaintext_sha256: Option<String>,
}

#[derive(Debug, Clone)]
struct AttachmentPlan {
    record: PortableAttachmentRecord,
    attachment: AttachmentRef,
}

#[derive(Debug)]
struct PortableProjection {
    generated_at: String,
    workspace: PortableWorkspaceRecord,
    channels: Vec<PortableChannelRecord>,
    members: Vec<PortableMemberRecord>,
    messages: Vec<PortableMessageRecord>,
    attachments: Vec<AttachmentPlan>,
    unavailable_message_bodies: Vec<PortableUnavailableMessageBody>,
    history_gaps: Vec<PortableHistoryGap>,
    invalid_signature_count: usize,
    corrupt_event_count: Option<usize>,
    accepted_event_count: usize,
    parseable_event_count: usize,
    applied_event_count: usize,
    event_inventory_blake3: String,
    causal_frontier_event_ids: Vec<String>,
    source_changed_during_capture: bool,
}

struct PortableProjectionContext {
    projection: PortableProjection,
    state: WorkspaceState,
    workspace_key: Option<WorkspaceKey>,
}

enum AttachmentLoad {
    Included(Vec<u8>),
    Missing(&'static str),
}

enum PortableMessageBodyLoad {
    Available(String),
    KeyUnavailable,
    DecryptionFailed,
}

struct PortableAttachmentLink {
    path: String,
    display_name: String,
}

struct PortableExportPublication {
    #[cfg(not(unix))]
    output_path: PathBuf,
    #[cfg(unix)]
    parent_path: PathBuf,
    #[cfg(unix)]
    parent_dir: fs::File,
    #[cfg(unix)]
    parent_device: u64,
    #[cfg(unix)]
    parent_inode: u64,
    #[cfg(unix)]
    temp_name: OsString,
    #[cfg(unix)]
    temp_device: u64,
    #[cfg(unix)]
    temp_inode: u64,
    #[cfg(unix)]
    output_name: OsString,
    #[cfg(not(unix))]
    temp_path: PathBuf,
}

struct ArchiveBuilder {
    zip: ZipWriter<fs::File>,
    checksums: BTreeMap<String, String>,
}

struct CheckedEntryWriter<'a> {
    zip: &'a mut ZipWriter<fs::File>,
    hasher: Sha256,
}

impl Write for CheckedEntryWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let written = self.zip.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.zip.flush()
    }
}

impl ArchiveBuilder {
    fn new(file: fs::File) -> Self {
        Self {
            zip: ZipWriter::new(file).set_auto_large_file(),
            checksums: BTreeMap::new(),
        }
    }

    fn write_entry<F>(
        &mut self,
        path: &str,
        compression: CompressionMethod,
        write: F,
    ) -> Result<(), RuntimeError>
    where
        F: FnOnce(&mut CheckedEntryWriter<'_>) -> Result<(), RuntimeError>,
    {
        if path == "SHA256SUMS" || self.checksums.contains_key(path) {
            return Err(RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("duplicate or reserved portable export archive path: {path}"),
            )));
        }
        let options = SimpleFileOptions::default()
            .compression_method(compression)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o600);
        self.zip.start_file(path, options)?;
        let checksum = {
            let mut writer = CheckedEntryWriter {
                zip: &mut self.zip,
                hasher: Sha256::new(),
            };
            write(&mut writer)?;
            lower_hex(&writer.hasher.finalize())
        };
        self.checksums.insert(path.to_owned(), checksum);
        Ok(())
    }

    fn write_bytes(
        &mut self,
        path: &str,
        bytes: &[u8],
        compression: CompressionMethod,
    ) -> Result<(), RuntimeError> {
        self.write_entry(path, compression, |writer| {
            writer.write_all(bytes)?;
            Ok(())
        })
    }

    fn write_json<T: Serialize>(&mut self, path: &str, value: &T) -> Result<(), RuntimeError> {
        self.write_entry(path, CompressionMethod::Deflated, |writer| {
            serde_json::to_writer_pretty(&mut *writer, value)?;
            writer.write_all(b"\n")?;
            Ok(())
        })
    }

    fn write_json_lines<T: Serialize>(
        &mut self,
        path: &str,
        values: &[T],
    ) -> Result<(), RuntimeError> {
        self.write_entry(path, CompressionMethod::Deflated, |writer| {
            for value in values {
                serde_json::to_writer(&mut *writer, value)?;
                writer.write_all(b"\n")?;
            }
            Ok(())
        })
    }

    fn finish(mut self) -> Result<fs::File, RuntimeError> {
        let mut checksum_file = String::new();
        for (path, checksum) in &self.checksums {
            checksum_file.push_str(checksum);
            checksum_file.push_str("  ");
            checksum_file.push_str(path);
            checksum_file.push('\n');
        }
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o600);
        self.zip.start_file("SHA256SUMS", options)?;
        self.zip.write_all(checksum_file.as_bytes())?;
        Ok(self.zip.finish()?)
    }
}

impl LocalRuntime {
    /// Creates a portable interoperability archive containing only the current
    /// device's readable workspace projection. It deliberately excludes raw
    /// signed events, encryption keys, MLS state, invites, recovery material,
    /// peer addresses, and device credentials.
    pub fn export_portable_workspace_archive(
        &self,
        workspace_id: WorkspaceId,
        output_path: impl AsRef<Path>,
    ) -> Result<PortableWorkspaceExport, RuntimeError> {
        let output_path = output_path.as_ref();
        let reported_output_path = output_path.to_path_buf();
        validate_runtime_path(output_path, "portable export output path")?;
        self.validate_portable_export_destination_before_create(output_path)?;
        let parent = output_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let output_path = self.validate_portable_export_destination(output_path, parent)?;

        let mut context = self.portable_export_projection(&workspace_id)?;
        let (publication, temp_file) = create_portable_export_temp_file(&output_path)?;
        let export_result = (|| -> Result<PortableWorkspaceExport, RuntimeError> {
            let mut archive = ArchiveBuilder::new(temp_file);
            let mut missing_attachments = Vec::new();
            let mut included_attachment_count = 0_usize;

            for plan in &mut context.projection.attachments {
                if plan.record.availability == "excluded_deleted" {
                    continue;
                }
                match self.load_portable_attachment(
                    &workspace_id,
                    &context.state,
                    context.workspace_key.as_ref(),
                    plan,
                )? {
                    AttachmentLoad::Included(plaintext) => {
                        let path = plan
                            .record
                            .archive_path
                            .as_deref()
                            .expect("attachment plan has archive path");
                        archive.write_bytes(path, &plaintext, CompressionMethod::Stored)?;
                        plan.record.availability = "included".to_owned();
                        plan.record.plaintext_sha256 = Some(sha256_bytes(&plaintext));
                        included_attachment_count += 1;
                    }
                    AttachmentLoad::Missing(reason) => {
                        plan.record.availability = reason.to_owned();
                        plan.record.archive_path = None;
                        missing_attachments.push(PortableMissingAttachment {
                            attachment_id: plan.record.attachment_id.clone(),
                            message_id: plan.record.message_id.clone(),
                            display_name: plan.record.display_name.clone(),
                            reason: reason.to_owned(),
                        });
                    }
                }
            }

            let attachment_records = context
                .projection
                .attachments
                .iter()
                .map(|plan| plan.record.clone())
                .collect::<Vec<_>>();
            validate_portable_export_records(&context.projection, &attachment_records)?;
            let attachment_links = attachment_records
                .iter()
                .filter_map(|record| {
                    record.archive_path.as_ref().map(|path| {
                        (
                            record.attachment_id.clone(),
                            PortableAttachmentLink {
                                path: path.clone(),
                                display_name: record.display_name.clone(),
                            },
                        )
                    })
                })
                .collect::<HashMap<_, _>>();
            let completeness =
                portable_completeness_report(&context.projection, missing_attachments);
            let warning_count = completeness_warning_count(&completeness);

            archive.write_json("data/workspace.json", &context.projection.workspace)?;
            archive.write_json_lines("data/channels.jsonl", &context.projection.channels)?;
            archive.write_json_lines("data/members.jsonl", &context.projection.members)?;
            archive.write_json_lines("data/messages.jsonl", &context.projection.messages)?;
            archive.write_json_lines("data/attachments.jsonl", &attachment_records)?;
            write_offline_html(
                &mut archive,
                &context.projection,
                &attachment_links,
                warning_count > 0,
            )?;
            archive.write_json(PORTABLE_EXPORT_SCHEMA_PATH, &portable_export_json_schema())?;

            archive.write_json("completeness.json", &completeness)?;

            let manifest = PortableExportManifest {
                schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                kind: PORTABLE_EXPORT_KIND.to_owned(),
                generated_at: context.projection.generated_at.clone(),
                generator: PortableGenerator {
                    name: "Chaft".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                workspace: context.projection.workspace.clone(),
                selection: PortableSelection {
                    scope: "workspace".to_owned(),
                    coverage_scope: "local_device_snapshot".to_owned(),
                    sync_completeness: "not_assessed".to_owned(),
                    readable_channels_only: true,
                    includes_private_channels: true,
                    includes_direct_messages: true,
                    includes_attachments: true,
                    message_state: "current".to_owned(),
                },
                cutoff: PortableCutoff {
                    captured_at: context.projection.generated_at.clone(),
                    accepted_event_count: context.projection.accepted_event_count,
                    parseable_event_count: context.projection.parseable_event_count,
                    applied_event_count: context.projection.applied_event_count,
                    event_inventory_blake3: context.projection.event_inventory_blake3.clone(),
                    causal_frontier_event_ids: context.projection.causal_frontier_event_ids.clone(),
                    source_changed_during_capture: context.projection.source_changed_during_capture,
                },
                counts: PortableCounts {
                    channels: context.projection.channels.len(),
                    members: context.projection.members.len(),
                    messages: context.projection.messages.len(),
                    attachments: attachment_records.len(),
                    included_attachments: included_attachment_count,
                    missing_attachments: completeness.missing_attachments.len(),
                },
                files: PortableFiles {
                    offline_index: "index.html".to_owned(),
                    workspace: "data/workspace.json".to_owned(),
                    channels: "data/channels.jsonl".to_owned(),
                    members: "data/members.jsonl".to_owned(),
                    messages: "data/messages.jsonl".to_owned(),
                    attachments: "data/attachments.jsonl".to_owned(),
                    completeness: "completeness.json".to_owned(),
                    schema: PORTABLE_EXPORT_SCHEMA_PATH.to_owned(),
                    checksums: "SHA256SUMS".to_owned(),
                },
                integrity: PortableIntegrity {
                    algorithm: "sha-256".to_owned(),
                    checksums_file: "SHA256SUMS".to_owned(),
                    coverage: "every archive entry except SHA256SUMS".to_owned(),
                },
                completeness: PortableCompletenessSummary {
                    status: completeness.status.clone(),
                    warning_count,
                    missing_attachment_count: completeness.missing_attachments.len(),
                    unavailable_message_body_count: completeness.unavailable_message_bodies.len(),
                    gap_count: completeness.history_gaps.len(),
                    invalid_signature_count: completeness.invalid_signature_count,
                    corrupt_event_count: completeness.corrupt_event_count,
                    storage_integrity_assessment: completeness.storage_integrity_assessment.clone(),
                },
            };
            archive.write_json("manifest.json", &manifest)?;
            archive.write_bytes(
                "README.txt",
                portable_export_readme().as_bytes(),
                CompressionMethod::Deflated,
            )?;

            let mut file = archive.finish()?;
            file.sync_all()?;
            let archive_bytes = file.metadata()?.len();
            let archive_sha256 = hash_open_file(&mut file)?;
            #[cfg(unix)]
            publication.publish(&file)?;
            #[cfg(not(unix))]
            {
                drop(file);
                publication.publish()?;
            }
            let _ = publication.sync_parent();
            Ok(PortableWorkspaceExport {
                schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                kind: PORTABLE_EXPORT_KIND.to_owned(),
                workspace_id: context.projection.workspace.workspace_id.clone(),
                workspace_name: context.projection.workspace.name.clone(),
                generated_at: context.projection.generated_at.clone(),
                output_path: reported_output_path.to_string_lossy().into_owned(),
                archive_bytes,
                archive_sha256,
                channel_count: context.projection.channels.len(),
                member_count: context.projection.members.len(),
                message_count: context.projection.messages.len(),
                attachment_count: attachment_records.len(),
                included_attachment_count,
                missing_attachment_count: completeness.missing_attachments.len(),
                unavailable_message_body_count: completeness.unavailable_message_bodies.len(),
                gap_count: completeness.history_gaps.len(),
                invalid_signature_count: completeness.invalid_signature_count,
                corrupt_event_count: completeness.corrupt_event_count,
                storage_integrity_assessment: completeness.storage_integrity_assessment.clone(),
                warning_count,
            })
        })();

        if export_result.is_err() {
            let _ = publication.cleanup_temp();
        }
        export_result
    }

    fn validate_portable_export_destination(
        &self,
        output_path: &Path,
        parent: &Path,
    ) -> Result<PathBuf, RuntimeError> {
        let file_name = output_path.file_name().ok_or_else(|| {
            RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "portable export path has no file name",
            ))
        })?;
        let canonical_parent = fs::canonicalize(parent)?;
        let resolved_output = canonical_parent.join(file_name);
        let canonical_data_dir = fs::canonicalize(&self.paths.data_dir)?;
        if resolved_output.starts_with(&canonical_data_dir) {
            return Err(RuntimeError::PortableExportDestinationInsideRuntime);
        }

        let identity_parent = self
            .paths
            .identity_file
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if let Ok(canonical_identity_parent) = fs::canonicalize(identity_parent)
            && self
                .paths
                .identity_file
                .file_name()
                .is_some_and(|identity_name| {
                    resolved_output == canonical_identity_parent.join(identity_name)
                })
        {
            return Err(RuntimeError::PortableExportDestinationInsideRuntime);
        }

        match fs::symlink_metadata(&resolved_output) {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir() => {
                Err(RuntimeError::PortableExportDestinationUnsafe)
            }
            Ok(_) => Err(RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "portable export destination already exists",
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(resolved_output),
            Err(error) => Err(error.into()),
        }
    }

    fn validate_portable_export_destination_before_create(
        &self,
        output_path: &Path,
    ) -> Result<(), RuntimeError> {
        let destination_exists = match fs::symlink_metadata(output_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir() => {
                return Err(RuntimeError::PortableExportDestinationUnsafe);
            }
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };
        let resolved_output = resolve_path_from_existing_ancestor(output_path)?;
        let canonical_data_dir = fs::canonicalize(&self.paths.data_dir)?;
        if resolved_output.starts_with(&canonical_data_dir) {
            return Err(RuntimeError::PortableExportDestinationInsideRuntime);
        }

        let resolved_identity = resolve_path_from_existing_ancestor(&self.paths.identity_file)?;
        if resolved_output == resolved_identity {
            return Err(RuntimeError::PortableExportDestinationInsideRuntime);
        }

        if destination_exists {
            Err(RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "portable export destination already exists",
            )))
        } else {
            Ok(())
        }
    }

    fn portable_export_projection(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<PortableProjectionContext, RuntimeError> {
        let (raw_events, source_changed_during_capture) =
            self.stable_portable_export_events(workspace_id)?;
        if raw_events.is_empty() {
            return Err(RuntimeError::WorkspaceHasNoEvents {
                workspace_id: workspace_id.clone(),
            });
        }

        let verified = crate::verified_local_events_for_runtime(&raw_events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        let report = state.apply_batch(&verified)?;
        if !state.members.contains_key(self.identity.device_id()) {
            return Err(chaft_core::AuthorizationError::NotAMember {
                device_id: self.identity.device_id().clone(),
            }
            .into());
        }
        let events_by_id = verified
            .iter()
            .map(|event| (event.event_id.clone(), event.clone()))
            .collect::<HashMap<_, _>>();
        let applied_events = report
            .applied_events
            .iter()
            .filter_map(|event_id| events_by_id.get(event_id).cloned())
            .collect::<Vec<_>>();
        let event_by_id = applied_events
            .iter()
            .map(|event| (event.event_id.clone(), event))
            .collect::<HashMap<_, _>>();

        let readable_channel_ids = state
            .channels
            .keys()
            .filter(|channel_id| state.channel_accessible_to(channel_id, self.identity.device_id()))
            .cloned()
            .collect::<HashSet<_>>();
        let visible_raw_events = raw_events
            .iter()
            .filter(|event| portable_event_is_visible(event, &state, &readable_channel_ids))
            .cloned()
            .collect::<Vec<_>>();
        let visible_raw_event_ids = visible_raw_events
            .iter()
            .map(|event| event.event_id.clone())
            .collect::<HashSet<_>>();
        let invalid_signature_count = visible_raw_events
            .iter()
            .filter(|event| {
                !event.author_public_key.is_empty() && verify_self_contained_event(event).is_err()
            })
            .count();
        let accepted_event_count = verified
            .iter()
            .filter(|event| portable_event_is_visible(event, &state, &readable_channel_ids))
            .count();
        let workspace_key = self.load_workspace_key(workspace_id)?;
        let encrypted_body_event_ids = state
            .messages
            .values()
            .filter(|message| {
                readable_channel_ids.contains(&message.channel_id)
                    && !message.deleted
                    && message.sealed_markdown.is_some()
            })
            .map(|message| message.author_event_id.clone())
            .collect::<BTreeSet<_>>();
        let body_loads = self.portable_message_body_loads_for_event_ids(
            workspace_id,
            &state,
            workspace_key.as_ref(),
            &encrypted_body_event_ids,
        )?;
        let (last_known_roles, last_known_person_links) = portable_actor_history(&applied_events);
        let mut actor_device_ids = state.members.keys().cloned().collect::<HashSet<_>>();
        for message in state
            .messages
            .values()
            .filter(|message| readable_channel_ids.contains(&message.channel_id))
        {
            if let Some(created_event) = event_by_id.get(&message.author_event_id) {
                actor_device_ids.insert(created_event.event.author_device_id.clone());
            }
            for reaction in message.reactions.keys() {
                actor_device_ids.extend(message.reaction_author_device_ids(reaction));
            }
        }

        let generated_at = format_datetime(OffsetDateTime::now_utc());
        let workspace = PortableWorkspaceRecord {
            schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
            workspace_id: workspace_id.0.clone(),
            name: state.name.clone().unwrap_or_else(|| "Workspace".to_owned()),
            access_policy: workspace_access_policy_name(state.access_policy).to_owned(),
        };
        let mut channels = readable_channel_ids
            .iter()
            .filter_map(|channel_id| state.channels.get(channel_id))
            .map(|channel| PortableChannelRecord {
                schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                channel_id: channel.channel_id.0.clone(),
                name: channel.name.clone(),
                topic: channel.topic.clone(),
                archived: channel.archived,
                is_private: channel.is_private,
                direct_message: channel.direct_message,
                member_device_ids: sorted_device_ids(&channel.member_device_ids),
                direct_message_participant_device_ids: sorted_device_ids(
                    &channel.direct_message_participant_device_ids,
                ),
                html_path: channel_html_path(&channel.name, &channel.channel_id),
            })
            .collect::<Vec<_>>();
        channels.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.channel_id.cmp(&right.channel_id))
        });

        let mut members = actor_device_ids
            .into_iter()
            .map(|device_id| {
                let identity = portable_identity(&state, &last_known_person_links, &device_id);
                let avatar_id = identity_avatar_id(&state, &last_known_person_links, &device_id);
                let active_member = state.members.get(&device_id);
                let role = active_member
                    .map(|member| member.role)
                    .or_else(|| last_known_roles.get(&device_id).copied())
                    .map(workspace_role_name)
                    .map(str::to_owned);
                PortableMemberRecord {
                    schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                    actor_id: identity.actor_id,
                    actor_kind: identity.actor_kind,
                    device_id: device_id.0,
                    person_id: identity.person_id,
                    display_name: identity.display_name,
                    avatar_id,
                    role,
                    active: active_member.is_some(),
                }
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
                .then_with(|| left.device_id.cmp(&right.device_id))
        });

        let mut latest_edits = HashMap::<MessageId, &SignedEvent>::new();
        for event in &applied_events {
            if let EventBody::MessageEdited { message_id, .. }
            | EventBody::MessageEditedEncrypted { message_id, .. } = &event.event.body
            {
                latest_edits.insert(message_id.clone(), event);
            }
        }

        let mut unavailable_message_bodies = Vec::new();
        let mut messages = Vec::new();
        let mut attachments = Vec::new();
        for message in state
            .messages
            .values()
            .filter(|message| readable_channel_ids.contains(&message.channel_id))
        {
            let Some(created_event) = event_by_id.get(&message.author_event_id).copied() else {
                continue;
            };
            let (body_state, markdown) = if message.deleted {
                ("deleted".to_owned(), String::new())
            } else if message.sealed_markdown.is_some() {
                match body_loads.get(&message.author_event_id.0) {
                    Some(PortableMessageBodyLoad::Available(markdown)) => {
                        ("available".to_owned(), markdown.clone())
                    }
                    Some(PortableMessageBodyLoad::DecryptionFailed) => {
                        unavailable_message_bodies.push(PortableUnavailableMessageBody {
                            message_id: message.message_id.0.clone(),
                            channel_id: message.channel_id.0.clone(),
                            reason: "decryption_failed".to_owned(),
                        });
                        ("unavailable_encrypted".to_owned(), String::new())
                    }
                    Some(PortableMessageBodyLoad::KeyUnavailable) | None => {
                        unavailable_message_bodies.push(PortableUnavailableMessageBody {
                            message_id: message.message_id.0.clone(),
                            channel_id: message.channel_id.0.clone(),
                            reason: "decryption_key_unavailable".to_owned(),
                        });
                        ("unavailable_encrypted".to_owned(), String::new())
                    }
                }
            } else {
                ("available".to_owned(), message.markdown.clone())
            };

            let mut attachment_ids = Vec::new();
            for (attachment_index, attachment) in message.attachments.iter().enumerate() {
                let attachment_id = portable_attachment_id(
                    &message.channel_id,
                    &message.message_id,
                    attachment_index,
                );
                if !message.deleted {
                    attachment_ids.push(attachment_id.clone());
                }
                attachments.push(AttachmentPlan {
                    record: PortableAttachmentRecord {
                        schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                        source_attachment_id: (!attachment.attachment_id.trim().is_empty())
                            .then(|| attachment.attachment_id.clone()),
                        message_id: message.message_id.0.clone(),
                        channel_id: message.channel_id.0.clone(),
                        attachment_index,
                        display_name: attachment.display_name.clone(),
                        media_type: attachment.media_type.clone(),
                        declared_plaintext_bytes: attachment
                            .encryption
                            .as_ref()
                            .map(|encrypted| encrypted.plaintext_byte_len)
                            .unwrap_or(attachment.byte_len),
                        source_blob_hash: attachment.blob_hash.clone(),
                        availability: if message.deleted {
                            "excluded_deleted".to_owned()
                        } else {
                            "pending".to_owned()
                        },
                        archive_path: (!message.deleted).then(|| {
                            attachment_archive_path(
                                &message.channel_id,
                                &message.message_id,
                                attachment_index,
                                attachment,
                            )
                        }),
                        plaintext_sha256: None,
                        attachment_id,
                    },
                    attachment: attachment.clone(),
                });
            }

            let edited_event = latest_edits.get(&message.message_id).copied();
            messages.push(PortableMessageRecord {
                schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
                message_id: message.message_id.0.clone(),
                channel_id: message.channel_id.0.clone(),
                reply_to_message_id: message
                    .reply_to_message_id
                    .as_ref()
                    .map(|message_id| message_id.0.clone()),
                author: portable_identity(
                    &state,
                    &last_known_person_links,
                    &created_event.event.author_device_id,
                ),
                created_event_id: created_event.event_id.0.clone(),
                created_at: format_unix_ms(created_event.event.timestamp.physical_ms),
                created_at_unix_ms: created_event.event.timestamp.physical_ms,
                created_at_logical: created_event.event.timestamp.logical,
                edited_event_id: edited_event.map(|event| event.event_id.0.clone()),
                edited_at: edited_event
                    .map(|event| format_unix_ms(event.event.timestamp.physical_ms)),
                deleted: message.deleted,
                body_state,
                markdown,
                attachment_ids,
                reactions: {
                    let mut reactions = message
                        .reactions
                        .iter()
                        .map(|(value, count)| PortableReaction {
                            value: value.clone(),
                            count: *count,
                            actor_device_ids: message
                                .reaction_author_device_ids(value)
                                .into_iter()
                                .map(|device_id| device_id.0)
                                .collect(),
                        })
                        .collect::<Vec<_>>();
                    reactions.sort_by(|left, right| left.value.cmp(&right.value));
                    reactions
                },
            });
        }
        messages.sort_by(|left, right| {
            left.created_at_unix_ms
                .cmp(&right.created_at_unix_ms)
                .then_with(|| left.created_at_logical.cmp(&right.created_at_logical))
                .then_with(|| left.created_event_id.cmp(&right.created_event_id))
        });
        attachments.sort_by(|left, right| {
            left.record
                .channel_id
                .cmp(&right.record.channel_id)
                .then_with(|| left.record.message_id.cmp(&right.record.message_id))
                .then_with(|| {
                    left.record
                        .attachment_index
                        .cmp(&right.record.attachment_index)
                })
        });

        let history_gaps = report
            .gaps
            .into_iter()
            .filter(|gap| {
                events_by_id.get(&gap.event_id).is_some_and(|event| {
                    portable_event_is_visible(event, &state, &readable_channel_ids)
                })
            })
            .map(|gap| PortableHistoryGap {
                event_id: gap.event_id.0,
                missing_parent_ids: gap
                    .missing_parent_ids
                    .into_iter()
                    .filter(|event_id| visible_raw_event_ids.contains(event_id))
                    .map(|event_id| event_id.0)
                    .collect(),
            })
            .collect::<Vec<_>>();
        let visible_applied_events = applied_events
            .iter()
            .filter(|event| portable_event_is_visible(event, &state, &readable_channel_ids))
            .cloned()
            .collect::<Vec<_>>();
        let causal_frontier_event_ids = causal_frontier(&visible_applied_events);
        let event_inventory_blake3 = event_inventory_fingerprint(&visible_raw_events)?;

        Ok(PortableProjectionContext {
            projection: PortableProjection {
                generated_at,
                workspace,
                channels,
                members,
                messages,
                attachments,
                unavailable_message_bodies,
                history_gaps,
                invalid_signature_count,
                // Corrupt storage rows cannot be safely attributed to a readable
                // channel. Reporting the workspace-global count would disclose
                // activity in channels this device cannot read.
                corrupt_event_count: None,
                accepted_event_count,
                parseable_event_count: visible_raw_events.len(),
                applied_event_count: visible_applied_events.len(),
                event_inventory_blake3,
                causal_frontier_event_ids,
                source_changed_during_capture,
            },
            state,
            workspace_key,
        })
    }

    fn stable_portable_export_events(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<(Vec<SignedEvent>, bool), RuntimeError> {
        let mut latest_events = self
            .store
            .list_parseable_events_for_workspace(&workspace_id.0)?;
        let mut latest_fingerprint = authorized_event_inventory_fingerprint(
            &latest_events,
            workspace_id,
            self.identity.device_id(),
        )?;
        for _ in 0..2 {
            let events = self
                .store
                .list_parseable_events_for_workspace(&workspace_id.0)?;
            let fingerprint = authorized_event_inventory_fingerprint(
                &events,
                workspace_id,
                self.identity.device_id(),
            )?;
            if latest_fingerprint == fingerprint {
                return Ok((events, false));
            }
            latest_events = events;
            latest_fingerprint = fingerprint;
        }
        Ok((latest_events, true))
    }

    fn portable_message_body_loads_for_event_ids(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        workspace_key: Option<&WorkspaceKey>,
        event_ids: &BTreeSet<chaft_types::EventId>,
    ) -> Result<HashMap<String, PortableMessageBodyLoad>, RuntimeError> {
        let mut body_loads = HashMap::new();
        let mut resolved_content_keys = HashMap::new();
        for message in state.messages.values() {
            if !event_ids.contains(&message.author_event_id)
                || !state.channel_accessible_to(&message.channel_id, self.identity.device_id())
            {
                continue;
            }
            let Some(sealed_markdown) = message.sealed_markdown.as_ref() else {
                continue;
            };
            let cache_key = (message.channel_id.0.clone(), sealed_markdown.key_id.clone());
            if !resolved_content_keys.contains_key(&cache_key) {
                let content_key = self.content_key_for_materialized_payload(
                    workspace_id,
                    &message.channel_id,
                    state,
                    workspace_key,
                    &sealed_markdown.key_id,
                )?;
                resolved_content_keys.insert(cache_key.clone(), content_key);
            }
            let body_load = match resolved_content_keys
                .get(&cache_key)
                .and_then(Option::as_ref)
            {
                Some(content_key) => match open_message_markdown(
                    content_key.content_key(),
                    sealed_markdown,
                    workspace_id,
                    &message.channel_id,
                    &message.message_id,
                ) {
                    Ok(markdown) => PortableMessageBodyLoad::Available(markdown),
                    Err(_) => PortableMessageBodyLoad::DecryptionFailed,
                },
                None => PortableMessageBodyLoad::KeyUnavailable,
            };
            body_loads.insert(message.author_event_id.0.clone(), body_load);
        }
        Ok(body_loads)
    }

    fn load_portable_attachment(
        &self,
        workspace_id: &WorkspaceId,
        state: &WorkspaceState,
        workspace_key: Option<&WorkspaceKey>,
        plan: &AttachmentPlan,
    ) -> Result<AttachmentLoad, RuntimeError> {
        let Some(encrypted) = plan.attachment.encryption.as_ref() else {
            return Ok(AttachmentLoad::Missing("unsupported_encryption_metadata"));
        };
        let ciphertext = match self
            .open_blob_store()?
            .get_complete_bytes(&plan.attachment.blob_hash)
        {
            Ok(Some(ciphertext)) => ciphertext,
            Ok(None) => return Ok(AttachmentLoad::Missing("missing_local_blob")),
            Err(_) => return Ok(AttachmentLoad::Missing("invalid_local_blob")),
        };
        let sealed = sealed_payload_from_encrypted_blob_ref(encrypted, ciphertext);
        let Some(content_key) = self.content_key_for_materialized_payload(
            workspace_id,
            &ChannelId(plan.record.channel_id.clone()),
            state,
            workspace_key,
            &sealed.key_id,
        )?
        else {
            return Ok(AttachmentLoad::Missing("decryption_key_unavailable"));
        };
        match open_attachment_blob(
            content_key.content_key(),
            &sealed,
            workspace_id,
            &ChannelId(plan.record.channel_id.clone()),
            &MessageId(plan.record.message_id.clone()),
            plan.record.attachment_index as u32,
        ) {
            Ok(plaintext) => Ok(AttachmentLoad::Included(plaintext)),
            Err(_) => Ok(AttachmentLoad::Missing("decryption_failed")),
        }
    }
}

fn validate_portable_export_records(
    projection: &PortableProjection,
    attachments: &[PortableAttachmentRecord],
) -> Result<(), RuntimeError> {
    let mut channel_ids = BTreeSet::new();
    let mut channel_paths = BTreeSet::new();
    for channel in &projection.channels {
        if !channel_ids.insert(channel.channel_id.as_str()) {
            return Err(portable_export_contract_error("duplicate channel id"));
        }
        if !channel_paths.insert(channel.html_path.as_str()) {
            return Err(portable_export_contract_error(
                "duplicate channel HTML archive path",
            ));
        }
    }

    let mut member_ids = BTreeSet::new();
    for member in &projection.members {
        if !member_ids.insert(member.device_id.as_str()) {
            return Err(portable_export_contract_error("duplicate member device id"));
        }
    }

    let mut messages_by_id = HashMap::new();
    for message in &projection.messages {
        if !channel_ids.contains(message.channel_id.as_str()) {
            return Err(portable_export_contract_error(
                "message references an unexported channel",
            ));
        }
        if messages_by_id
            .insert(message.message_id.as_str(), message)
            .is_some()
        {
            return Err(portable_export_contract_error("duplicate message id"));
        }
        if message.attachment_ids.iter().collect::<BTreeSet<_>>().len()
            != message.attachment_ids.len()
        {
            return Err(portable_export_contract_error(
                "message contains duplicate attachment ids",
            ));
        }
    }

    let mut attachment_ids = BTreeSet::new();
    let mut attachment_paths = BTreeSet::new();
    let mut attachment_ids_by_message = HashMap::<&str, BTreeSet<&str>>::new();
    for attachment in attachments {
        if !attachment_ids.insert(attachment.attachment_id.as_str()) {
            return Err(portable_export_contract_error("duplicate attachment id"));
        }
        let Some(message) = messages_by_id.get(attachment.message_id.as_str()) else {
            return Err(portable_export_contract_error(
                "attachment references an unexported message",
            ));
        };
        if attachment.channel_id != message.channel_id {
            return Err(portable_export_contract_error(
                "attachment channel does not match its message",
            ));
        }
        match attachment.availability.as_str() {
            "included"
                if attachment.archive_path.is_some() && attachment.plaintext_sha256.is_some() =>
            {
                let path = attachment.archive_path.as_deref().expect("checked above");
                if !attachment_paths.insert(path) {
                    return Err(portable_export_contract_error(
                        "duplicate attachment archive path",
                    ));
                }
            }
            "included" => {
                return Err(portable_export_contract_error(
                    "included attachment lacks archive integrity metadata",
                ));
            }
            _ if attachment.archive_path.is_none() && attachment.plaintext_sha256.is_none() => {}
            _ => {
                return Err(portable_export_contract_error(
                    "unavailable attachment retains archive integrity metadata",
                ));
            }
        }
        if attachment.availability != "excluded_deleted" {
            attachment_ids_by_message
                .entry(attachment.message_id.as_str())
                .or_default()
                .insert(attachment.attachment_id.as_str());
        }
    }

    for message in &projection.messages {
        let referenced_ids = message
            .attachment_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let record_ids = attachment_ids_by_message
            .get(message.message_id.as_str())
            .cloned()
            .unwrap_or_default();
        if referenced_ids != record_ids {
            return Err(portable_export_contract_error(
                "message attachment references do not match attachment records",
            ));
        }
    }
    Ok(())
}

fn portable_export_contract_error(detail: &'static str) -> RuntimeError {
    RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("portable export contract violation: {detail}"),
    ))
}

fn portable_completeness_report(
    projection: &PortableProjection,
    mut missing_attachments: Vec<PortableMissingAttachment>,
) -> PortableCompletenessReport {
    missing_attachments.sort_by(|left, right| {
        left.message_id
            .cmp(&right.message_id)
            .then_with(|| left.attachment_id.cmp(&right.attachment_id))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    let mut unavailable_message_bodies = projection.unavailable_message_bodies.clone();
    unavailable_message_bodies.sort_by(|left, right| {
        left.channel_id
            .cmp(&right.channel_id)
            .then_with(|| left.message_id.cmp(&right.message_id))
            .then_with(|| left.reason.cmp(&right.reason))
    });
    let mut history_gaps = projection.history_gaps.clone();
    for gap in &mut history_gaps {
        gap.missing_parent_ids.sort();
        gap.missing_parent_ids.dedup();
    }
    history_gaps.sort_by(|left, right| left.event_id.cmp(&right.event_id));

    let mut warnings = Vec::new();
    push_warning(
        &mut warnings,
        "missing_attachments",
        missing_attachments.len(),
        "Referenced files that were unavailable or could not be safely decrypted were not included.",
    );
    push_warning(
        &mut warnings,
        "unavailable_message_bodies",
        unavailable_message_bodies.len(),
        "Encrypted message bodies that lacked readable local key material or failed authentication were emitted as metadata-only records.",
    );
    push_warning(
        &mut warnings,
        "history_gaps",
        history_gaps.len(),
        "Events with missing causal history or unavailable authorization context were not materialized.",
    );
    push_warning(
        &mut warnings,
        "invalid_signatures",
        projection.invalid_signature_count,
        "Events with invalid self-contained signatures were excluded.",
    );
    if let Some(corrupt_event_count) = projection.corrupt_event_count {
        push_warning(
            &mut warnings,
            "corrupt_events",
            corrupt_event_count,
            "Unparseable event rows in the readable export scope were excluded.",
        );
    }
    push_warning(
        &mut warnings,
        "source_changed_during_capture",
        usize::from(projection.source_changed_during_capture),
        "The local event inventory changed during both capture attempts; the manifest identifies the exact captured inventory.",
    );
    PortableCompletenessReport {
        schema_version: PORTABLE_EXPORT_SCHEMA_VERSION,
        status: if warnings.is_empty() {
            "complete".to_owned()
        } else {
            "complete_with_warnings".to_owned()
        },
        source_changed_during_capture: projection.source_changed_during_capture,
        missing_attachments,
        unavailable_message_bodies,
        history_gaps,
        invalid_signature_count: projection.invalid_signature_count,
        corrupt_event_count: projection.corrupt_event_count,
        storage_integrity_assessment: "not_assessed_for_privacy".to_owned(),
        warnings,
    }
}

fn push_warning(warnings: &mut Vec<PortableWarning>, code: &str, count: usize, detail: &str) {
    if count > 0 {
        warnings.push(PortableWarning {
            code: code.to_owned(),
            count,
            detail: detail.to_owned(),
        });
    }
}

fn completeness_warning_count(report: &PortableCompletenessReport) -> usize {
    report.warnings.iter().fold(0_usize, |total, warning| {
        total.saturating_add(warning.count)
    })
}

fn write_offline_html(
    archive: &mut ArchiveBuilder,
    projection: &PortableProjection,
    attachment_links: &HashMap<String, PortableAttachmentLink>,
    has_completeness_warnings: bool,
) -> Result<(), RuntimeError> {
    archive.write_entry("index.html", CompressionMethod::Deflated, |writer| {
        write_html_head(writer, &projection.workspace.name)?;
        write!(
            writer,
            "<main><p class=eyebrow>Portable workspace export</p><h1>{}</h1><p class=lede>Readable offline history captured on {}.</p>",
            html_escape(&projection.workspace.name),
            html_escape(&projection.generated_at)
        )?;
        if has_completeness_warnings {
            write!(
                writer,
                "<aside class=warning>This copy has completeness warnings. See <a href=\"completeness.json\">completeness.json</a>.</aside>"
            )?;
        }
        write!(writer, "<section><h2>Conversations</h2><ul class=channels>")?;
        for channel in &projection.channels {
            let label = if channel.direct_message {
                "Direct message"
            } else if channel.is_private {
                "Private room"
            } else {
                "Room"
            };
            write!(
                writer,
                "<li><a href=\"{}\"><strong>{}</strong><span>{}</span></a></li>",
                html_escape(&channel.html_path),
                html_escape(&channel.name),
                label
            )?;
        }
        write!(
            writer,
            "</ul></section><footer>Structured records are in <code>data/</code>. Integrity metadata is in <code>manifest.json</code> and <code>SHA256SUMS</code>.</footer></main></body></html>"
        )?;
        Ok(())
    })?;

    let mut messages_by_channel = HashMap::<&str, Vec<&PortableMessageRecord>>::new();
    for message in &projection.messages {
        messages_by_channel
            .entry(message.channel_id.as_str())
            .or_default()
            .push(message);
    }

    for channel in &projection.channels {
        archive.write_entry(
            &channel.html_path,
            CompressionMethod::Deflated,
            |writer| {
                write_html_head(writer, &channel.name)?;
                write!(
                    writer,
                    "<main><p><a href=\"../../index.html\">← All conversations</a></p><p class=eyebrow>{}</p><h1>{}</h1>",
                    if channel.direct_message {
                        "Direct message"
                    } else if channel.is_private {
                        "Private room"
                    } else {
                        "Room"
                    },
                    html_escape(&channel.name)
                )?;
                if !channel.topic.is_empty() {
                    write!(writer, "<p class=lede>{}</p>", html_escape(&channel.topic))?;
                }
                write!(writer, "<section class=messages>")?;
                for message in messages_by_channel
                    .get(channel.channel_id.as_str())
                    .into_iter()
                    .flatten()
                {
                    write!(
                        writer,
                        "<article id=\"{}\"><header><strong>{}</strong><time>{}</time></header>",
                        html_escape(&safe_component(&message.message_id, "message", 80)),
                        html_escape(&message.author.display_name),
                        html_escape(&message.created_at)
                    )?;
                    if let Some(parent_id) = message.reply_to_message_id.as_ref() {
                        write!(
                            writer,
                            "<p class=reply>Reply to {}</p>",
                            html_escape(parent_id)
                        )?;
                    }
                    if message.deleted {
                        write!(writer, "<p class=muted>Message deleted</p>")?;
                    } else if message.body_state != "available" {
                        write!(writer, "<p class=muted>Encrypted body unavailable on this device</p>")?;
                    } else {
                        write!(
                            writer,
                            "<div class=body>{}</div>",
                            html_escape(&message.markdown)
                        )?;
                    }
                    if !message.attachment_ids.is_empty() {
                        write!(writer, "<ul class=files>")?;
                        for attachment_id in &message.attachment_ids {
                            if let Some(link) = attachment_links.get(attachment_id) {
                                write!(
                                    writer,
                                    "<li><a download=\"{}\" href=\"../../{}\">{}</a></li>",
                                    html_escape(&link.display_name),
                                    html_escape(&link.path),
                                    html_escape(&link.display_name)
                                )?;
                            } else {
                                write!(writer, "<li class=muted>Attachment unavailable</li>")?;
                            }
                        }
                        write!(writer, "</ul>")?;
                    }
                    if !message.reactions.is_empty() {
                        write!(writer, "<p class=reactions>")?;
                        for reaction in &message.reactions {
                            write!(
                                writer,
                                "<span>{} {}</span>",
                                html_escape(&reaction.value),
                                reaction.count
                            )?;
                        }
                        write!(writer, "</p>")?;
                    }
                    write!(writer, "</article>")?;
                }
                write!(writer, "</section></main></body></html>")?;
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn write_html_head(writer: &mut impl Write, title: &str) -> Result<(), RuntimeError> {
    write!(
        writer,
        "<!doctype html><html lang=en><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'\"><title>{}</title><style>{}</style></head><body>",
        html_escape(title),
        PORTABLE_EXPORT_CSS
    )?;
    Ok(())
}

const PORTABLE_EXPORT_CSS: &str = r#"
:root{color-scheme:light dark;font-family:ui-sans-serif,system-ui,sans-serif;line-height:1.5}body{margin:0;background:#101615;color:#edf5f3}main{max-width:860px;margin:auto;padding:48px 24px 80px}.eyebrow{color:#61d8ca;text-transform:uppercase;letter-spacing:.12em;font-size:.75rem;font-weight:700}h1{font-size:2.25rem;line-height:1.1;margin:.25rem 0}.lede,.muted,footer{color:#aebbb8}.warning{padding:14px 16px;margin:24px 0;background:#3b3015;border:1px solid #a6842b;border-radius:10px}.channels{list-style:none;padding:0;display:grid;gap:10px}.channels a{display:flex;justify-content:space-between;gap:16px;padding:14px 16px;background:#17201f;border:1px solid #2b3936;border-radius:10px;text-decoration:none;color:inherit}.channels span{color:#aebbb8;font-size:.875rem}.messages{display:grid;gap:12px;margin-top:28px}article{padding:16px;background:#17201f;border:1px solid #2b3936;border-radius:12px}article header{display:flex;justify-content:space-between;gap:16px}time,.reply{color:#91a29e;font-size:.8rem}.body{white-space:pre-wrap;overflow-wrap:anywhere;margin-top:10px}.files{margin:.75rem 0 0}.reactions{display:flex;gap:8px;flex-wrap:wrap}.reactions span{padding:2px 8px;background:#22302d;border-radius:999px}a{color:#61d8ca}code{font-family:ui-monospace,monospace}footer{margin-top:48px;font-size:.85rem}@media(prefers-color-scheme:light){body{background:#f6faf9;color:#16201e}.channels a,article{background:#fff;border-color:#d7e2df}.warning{background:#fff8df;border-color:#c7a33f}.lede,.muted,footer,.channels span,time,.reply{color:#596966}}
"#;

fn portable_export_json_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://chaft.app/schemas/portable-workspace-v2.schema.json",
        "title": "Chaft portable workspace export v2",
        "description": "Schemas for manifest.json and records in data/*.jsonl. Each JSONL line is one independent JSON object.",
        "$defs": {
            "workspace": {
                "type": "object",
                "required": ["schemaVersion", "workspaceId", "name", "accessPolicy"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "workspaceId": {"type": "string", "minLength": 1},
                    "name": {"type": "string"},
                    "accessPolicy": {"enum": ["invite_only", "request_access", "discoverable"]}
                },
                "additionalProperties": true
            },
            "channel": {
                "type": "object",
                "required": ["schemaVersion", "channelId", "name", "topic", "archived", "isPrivate", "directMessage", "memberDeviceIds", "directMessageParticipantDeviceIds", "htmlPath"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "channelId": {"type": "string", "minLength": 1},
                    "name": {"type": "string"},
                    "topic": {"type": "string"},
                    "archived": {"type": "boolean"},
                    "isPrivate": {"type": "boolean"},
                    "directMessage": {"type": "boolean"},
                    "memberDeviceIds": {"type": "array", "items": {"type": "string"}, "uniqueItems": true},
                    "directMessageParticipantDeviceIds": {"type": "array", "items": {"type": "string"}, "uniqueItems": true},
                    "htmlPath": {"type": "string", "minLength": 1}
                },
                "additionalProperties": true
            },
            "member": {
                "type": "object",
                "required": ["schemaVersion", "actorId", "actorKind", "deviceId", "personId", "displayName", "avatarId", "role", "active"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "actorId": {"type": "string", "minLength": 1},
                    "actorKind": {"enum": ["person", "device"]},
                    "deviceId": {"type": "string", "minLength": 1},
                    "personId": {"type": ["string", "null"]},
                    "displayName": {"type": "string"},
                    "avatarId": {"type": "string"},
                    "role": {"type": ["string", "null"], "enum": ["owner", "admin", "member", "guest", null]},
                    "active": {"type": "boolean"}
                },
                "additionalProperties": true
            },
            "identity": {
                "type": "object",
                "required": ["actorId", "actorKind", "deviceId", "personId", "displayName"],
                "properties": {
                    "actorId": {"type": "string", "minLength": 1},
                    "actorKind": {"enum": ["person", "device"]},
                    "deviceId": {"type": "string", "minLength": 1},
                    "personId": {"type": ["string", "null"]},
                    "displayName": {"type": "string"}
                },
                "additionalProperties": true
            },
            "reaction": {
                "type": "object",
                "required": ["value", "count", "actorDeviceIds"],
                "properties": {
                    "value": {"type": "string"},
                    "count": {"type": "integer", "minimum": 1},
                    "actorDeviceIds": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 1, "uniqueItems": true}
                },
                "additionalProperties": true
            },
            "message": {
                "type": "object",
                "required": ["schemaVersion", "messageId", "channelId", "replyToMessageId", "author", "createdEventId", "createdAt", "createdAtUnixMs", "createdAtLogical", "editedEventId", "editedAt", "deleted", "bodyState", "markdown", "attachmentIds", "reactions"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "messageId": {"type": "string", "minLength": 1},
                    "channelId": {"type": "string", "minLength": 1},
                    "replyToMessageId": {"type": ["string", "null"]},
                    "author": {"$ref": "#/$defs/identity"},
                    "createdEventId": {"type": "string", "minLength": 1},
                    "createdAt": {"type": "string", "format": "date-time"},
                    "createdAtUnixMs": {"type": "integer"},
                    "createdAtLogical": {"type": "integer", "minimum": 0},
                    "editedEventId": {"type": ["string", "null"]},
                    "editedAt": {"type": ["string", "null"], "format": "date-time"},
                    "deleted": {"type": "boolean"},
                    "bodyState": {"enum": ["available", "unavailable_encrypted", "deleted"]},
                    "markdown": {"type": "string"},
                    "attachmentIds": {"type": "array", "items": {"type": "string"}, "uniqueItems": true},
                    "reactions": {"type": "array", "items": {"$ref": "#/$defs/reaction"}}
                },
                "allOf": [
                    {
                        "if": {"properties": {"deleted": {"const": true}}, "required": ["deleted"]},
                        "then": {"properties": {"bodyState": {"const": "deleted"}, "markdown": {"const": ""}}}
                    },
                    {
                        "if": {"properties": {"bodyState": {"enum": ["unavailable_encrypted", "deleted"]}}, "required": ["bodyState"]},
                        "then": {"properties": {"markdown": {"const": ""}}}
                    }
                ],
                "additionalProperties": true
            },
            "attachment": {
                "type": "object",
                "required": ["schemaVersion", "attachmentId", "sourceAttachmentId", "messageId", "channelId", "attachmentIndex", "displayName", "mediaType", "declaredPlaintextBytes", "sourceBlobHash", "availability", "archivePath", "plaintextSha256"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "attachmentId": {"type": "string", "minLength": 1},
                    "sourceAttachmentId": {"type": ["string", "null"]},
                    "messageId": {"type": "string", "minLength": 1},
                    "channelId": {"type": "string", "minLength": 1},
                    "attachmentIndex": {"type": "integer", "minimum": 0},
                    "displayName": {"type": "string"},
                    "mediaType": {"type": "string"},
                    "declaredPlaintextBytes": {"type": "integer", "minimum": 0},
                    "sourceBlobHash": {"type": "string"},
                    "availability": {"enum": ["included", "excluded_deleted", "missing_local_blob", "invalid_local_blob", "decryption_key_unavailable", "decryption_failed", "unsupported_encryption_metadata"]},
                    "archivePath": {"type": ["string", "null"]},
                    "plaintextSha256": {"type": ["string", "null"], "pattern": "^[0-9a-f]{64}$"}
                },
                "allOf": [
                    {
                        "if": {"properties": {"availability": {"const": "included"}}, "required": ["availability"]},
                        "then": {"properties": {"archivePath": {"type": "string", "minLength": 1}, "plaintextSha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}}}
                    },
                    {
                        "if": {"properties": {"availability": {"not": {"const": "included"}}}, "required": ["availability"]},
                        "then": {"properties": {"archivePath": {"type": "null"}, "plaintextSha256": {"type": "null"}}}
                    }
                ],
                "additionalProperties": true
            },
            "missingAttachment": {
                "type": "object",
                "required": ["attachmentId", "messageId", "displayName", "reason"],
                "properties": {
                    "attachmentId": {"type": "string", "minLength": 1},
                    "messageId": {"type": "string", "minLength": 1},
                    "displayName": {"type": "string"},
                    "reason": {"type": "string", "minLength": 1}
                },
                "additionalProperties": true
            },
            "unavailableMessageBody": {
                "type": "object",
                "required": ["messageId", "channelId", "reason"],
                "properties": {
                    "messageId": {"type": "string", "minLength": 1},
                    "channelId": {"type": "string", "minLength": 1},
                    "reason": {"enum": ["decryption_key_unavailable", "decryption_failed"]}
                },
                "additionalProperties": true
            },
            "historyGap": {
                "type": "object",
                "required": ["eventId", "missingParentIds"],
                "properties": {
                    "eventId": {"type": "string", "minLength": 1},
                    "missingParentIds": {"type": "array", "items": {"type": "string"}, "uniqueItems": true}
                },
                "additionalProperties": true
            },
            "warning": {
                "type": "object",
                "required": ["code", "count", "detail"],
                "properties": {
                    "code": {"type": "string", "minLength": 1},
                    "count": {"type": "integer", "minimum": 1},
                    "detail": {"type": "string", "minLength": 1}
                },
                "additionalProperties": true
            },
            "completeness": {
                "type": "object",
                "required": ["schemaVersion", "status", "sourceChangedDuringCapture", "missingAttachments", "unavailableMessageBodies", "historyGaps", "invalidSignatureCount", "corruptEventCount", "storageIntegrityAssessment", "warnings"],
                "properties": {
                    "schemaVersion": {"const": 2},
                    "status": {"enum": ["complete", "complete_with_warnings"]},
                    "sourceChangedDuringCapture": {"type": "boolean"},
                    "missingAttachments": {"type": "array", "items": {"$ref": "#/$defs/missingAttachment"}},
                    "unavailableMessageBodies": {"type": "array", "items": {"$ref": "#/$defs/unavailableMessageBody"}},
                    "historyGaps": {"type": "array", "items": {"$ref": "#/$defs/historyGap"}},
                    "invalidSignatureCount": {"type": "integer", "minimum": 0},
                    "corruptEventCount": {"type": ["integer", "null"], "minimum": 0},
                    "storageIntegrityAssessment": {"const": "not_assessed_for_privacy"},
                    "warnings": {"type": "array", "items": {"$ref": "#/$defs/warning"}}
                },
                "additionalProperties": true
            }
        },
        "type": "object",
        "required": ["schemaVersion", "kind", "generatedAt", "generator", "workspace", "selection", "cutoff", "counts", "files", "integrity", "completeness"],
        "properties": {
            "schemaVersion": {"const": 2},
            "kind": {"const": "chaft.portable-workspace.v2"},
            "generatedAt": {"type": "string", "format": "date-time"},
            "generator": {
                "type": "object",
                "required": ["name", "version"],
                "properties": {"name": {"type": "string"}, "version": {"type": "string"}},
                "additionalProperties": true
            },
            "workspace": {"$ref": "#/$defs/workspace"},
            "selection": {
                "type": "object",
                "required": ["scope", "coverageScope", "syncCompleteness", "readableChannelsOnly", "includesPrivateChannels", "includesDirectMessages", "includesAttachments", "messageState"],
                "properties": {
                    "scope": {"const": "workspace"},
                    "coverageScope": {"const": "local_device_snapshot"},
                    "syncCompleteness": {"const": "not_assessed"},
                    "readableChannelsOnly": {"const": true},
                    "includesPrivateChannels": {"type": "boolean"},
                    "includesDirectMessages": {"type": "boolean"},
                    "includesAttachments": {"type": "boolean"},
                    "messageState": {"const": "current"}
                },
                "additionalProperties": true
            },
            "cutoff": {
                "type": "object",
                "required": ["capturedAt", "acceptedEventCount", "parseableEventCount", "appliedEventCount", "eventInventoryBlake3", "causalFrontierEventIds", "sourceChangedDuringCapture"],
                "properties": {
                    "capturedAt": {"type": "string", "format": "date-time"},
                    "acceptedEventCount": {"type": "integer", "minimum": 0},
                    "parseableEventCount": {"type": "integer", "minimum": 0},
                    "appliedEventCount": {"type": "integer", "minimum": 0},
                    "eventInventoryBlake3": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "causalFrontierEventIds": {"type": "array", "items": {"type": "string"}, "uniqueItems": true},
                    "sourceChangedDuringCapture": {"type": "boolean"}
                },
                "additionalProperties": true
            },
            "counts": {
                "type": "object",
                "required": ["channels", "members", "messages", "attachments", "includedAttachments", "missingAttachments"],
                "additionalProperties": {"type": "integer", "minimum": 0}
            },
            "files": {
                "type": "object",
                "required": ["offlineIndex", "workspace", "channels", "members", "messages", "attachments", "completeness", "schema", "checksums"],
                "additionalProperties": {"type": "string", "minLength": 1}
            },
            "integrity": {
                "type": "object",
                "required": ["algorithm", "checksumsFile", "coverage"],
                "properties": {
                    "algorithm": {"const": "sha-256"},
                    "checksumsFile": {"const": "SHA256SUMS"},
                    "coverage": {"type": "string"}
                },
                "additionalProperties": true
            },
            "completeness": {
                "type": "object",
                "required": ["status", "warningCount", "missingAttachmentCount", "unavailableMessageBodyCount", "gapCount", "invalidSignatureCount", "corruptEventCount", "storageIntegrityAssessment"],
                "properties": {
                    "status": {"enum": ["complete", "complete_with_warnings"]},
                    "warningCount": {"type": "integer", "minimum": 0},
                    "missingAttachmentCount": {"type": "integer", "minimum": 0},
                    "unavailableMessageBodyCount": {"type": "integer", "minimum": 0},
                    "gapCount": {"type": "integer", "minimum": 0},
                    "invalidSignatureCount": {"type": "integer", "minimum": 0},
                    "corruptEventCount": {"type": ["integer", "null"], "minimum": 0},
                    "storageIntegrityAssessment": {"const": "not_assessed_for_privacy"}
                },
                "additionalProperties": true
            }
        },
        "additionalProperties": true
    })
}

fn portable_export_readme() -> String {
    format!(
        "Chaft portable workspace export\n\n\
         Open index.html for readable offline history. Structured records are in data/.\n\
         Each .jsonl file contains one JSON object per line. The schema is in schemas/.\n\
         completeness.json lists anything that could not be included. SHA256SUMS covers every\n\
         archive entry except the checksum file itself.\n\n\
         This is an interoperability copy, not a Chaft backup or recovery kit. It intentionally\n\
         excludes encryption keys, MLS state, device credentials, invite secrets, recovery\n\
         material, peer addresses, raw signed events, and signatures. Keep the archive private:\n\
         it contains readable workspace content and decrypted file attachments.\n\n\
         Format: {PORTABLE_EXPORT_KIND}\n"
    )
}

fn portable_actor_history(
    events: &[SignedEvent],
) -> (
    HashMap<DeviceId, WorkspaceRole>,
    HashMap<DeviceId, PersonId>,
) {
    let mut roles = HashMap::new();
    let mut person_links = HashMap::new();
    for event in events {
        match &event.event.body {
            EventBody::WorkspaceCreated { .. } => {
                roles.insert(event.event.author_device_id.clone(), WorkspaceRole::Owner);
            }
            EventBody::MemberInvited {
                invitee_device_id,
                role,
            } => {
                roles.insert(invitee_device_id.clone(), *role);
            }
            EventBody::MemberRoleUpdated {
                member_device_id,
                role,
            } => {
                roles.insert(member_device_id.clone(), *role);
            }
            EventBody::PersonDeviceLinked {
                person_id,
                device_id,
            } => {
                person_links.insert(device_id.clone(), person_id.clone());
            }
            _ => {}
        }
    }
    (roles, person_links)
}

fn portable_identity(
    state: &WorkspaceState,
    historical_person_links: &HashMap<DeviceId, PersonId>,
    device_id: &DeviceId,
) -> PortableIdentity {
    let person_id = state
        .person_device_links
        .get(device_id)
        .map(|link| link.person_id.clone())
        .or_else(|| historical_person_links.get(device_id).cloned());
    let person_name = person_id
        .as_ref()
        .and_then(|person_id| state.person_profiles.get(person_id))
        .map(|profile| profile.display_name.as_str())
        .filter(|name| !name.trim().is_empty());
    let device_name = state
        .profiles
        .get(device_id)
        .map(|profile| profile.display_name.as_str())
        .filter(|name| !name.trim().is_empty());
    let (actor_id, actor_kind) = person_id
        .as_ref()
        .map(|person_id| (person_id.0.clone(), "person".to_owned()))
        .unwrap_or_else(|| (device_id.0.clone(), "device".to_owned()));
    PortableIdentity {
        actor_id,
        actor_kind,
        device_id: device_id.0.clone(),
        person_id: person_id.map(|person_id| person_id.0),
        display_name: person_name
            .or(device_name)
            .map(str::to_owned)
            .unwrap_or_else(|| fallback_device_name(device_id)),
    }
}

fn identity_avatar_id(
    state: &WorkspaceState,
    historical_person_links: &HashMap<DeviceId, PersonId>,
    device_id: &DeviceId,
) -> String {
    state
        .person_device_links
        .get(device_id)
        .map(|link| link.person_id.clone())
        .or_else(|| historical_person_links.get(device_id).cloned())
        .and_then(|person_id| state.person_profiles.get(&person_id))
        .map(|profile| profile.avatar_id.trim())
        .filter(|avatar_id| !avatar_id.is_empty())
        .or_else(|| {
            state
                .profiles
                .get(device_id)
                .map(|profile| profile.avatar_id.trim())
                .filter(|avatar_id| !avatar_id.is_empty())
        })
        .unwrap_or_default()
        .to_owned()
}

fn fallback_device_name(device_id: &DeviceId) -> String {
    let prefix = device_id.0.chars().take(10).collect::<String>();
    if prefix.is_empty() {
        "Unknown member".to_owned()
    } else {
        format!("Device {prefix}")
    }
}

fn sorted_device_ids(device_ids: &[DeviceId]) -> Vec<String> {
    let mut values = device_ids
        .iter()
        .map(|device_id| device_id.0.clone())
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn workspace_role_name(role: WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Owner => "owner",
        WorkspaceRole::Admin => "admin",
        WorkspaceRole::Member => "member",
        WorkspaceRole::Guest => "guest",
    }
}

fn workspace_access_policy_name(policy: chaft_types::WorkspaceAccessPolicy) -> &'static str {
    match policy {
        chaft_types::WorkspaceAccessPolicy::InviteOnly => "invite_only",
        chaft_types::WorkspaceAccessPolicy::RequestAccess => "request_access",
        chaft_types::WorkspaceAccessPolicy::Discoverable => "discoverable",
    }
}

fn portable_event_is_visible(
    event: &SignedEvent,
    state: &WorkspaceState,
    readable_channel_ids: &HashSet<ChannelId>,
) -> bool {
    match portable_event_scope(event, state) {
        PortableEventScope::Workspace => true,
        PortableEventScope::Channel(channel_id) => readable_channel_ids.contains(&channel_id),
        PortableEventScope::UnresolvedChannel => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PortableEventScope {
    Workspace,
    Channel(ChannelId),
    UnresolvedChannel,
}

fn portable_event_scope(event: &SignedEvent, state: &WorkspaceState) -> PortableEventScope {
    let body_scope = portable_event_body_scope(&event.event.body, state);
    match (event.event.channel_id.as_ref(), body_scope) {
        (None, PortableEventScope::Workspace) => PortableEventScope::Workspace,
        (None, PortableEventScope::Channel(channel_id))
            if !portable_event_requires_channel_envelope(&event.event.body) =>
        {
            PortableEventScope::Channel(channel_id)
        }
        (Some(envelope_channel_id), PortableEventScope::Channel(body_channel_id))
            if envelope_channel_id == &body_channel_id =>
        {
            PortableEventScope::Channel(body_channel_id)
        }
        // The envelope is not an independent authority boundary. A mismatched
        // or unexpected channel context is fail-closed so a rejected event
        // cannot make unreadable activity affect export diagnostics.
        (Some(_), _) => PortableEventScope::UnresolvedChannel,
        (None, _) => PortableEventScope::UnresolvedChannel,
    }
}

fn portable_event_requires_channel_envelope(body: &EventBody) -> bool {
    matches!(
        body,
        EventBody::OpenMlsChannelGroupMemberAdded { .. }
            | EventBody::OpenMlsChannelGroupMemberRemoved { .. }
            | EventBody::OpenMlsChannelGroupSelfUpdated { .. }
            | EventBody::ContentKeyEpochPublished {
                scope: chaft_types::ContentKeyScope::Channel { .. },
                ..
            }
            | EventBody::MessageCreated { .. }
            | EventBody::MessageReplyCreated { .. }
            | EventBody::MessageCreatedEncrypted { .. }
            | EventBody::MessageReplyCreatedEncrypted { .. }
            | EventBody::MessageEdited { .. }
            | EventBody::MessageEditedEncrypted { .. }
            | EventBody::MessageDeleted { .. }
            | EventBody::ReactionAdded { .. }
            | EventBody::ReactionRemoved { .. }
            | EventBody::ReadMarkerUpdated { .. }
    )
}

fn portable_event_body_scope(body: &EventBody, state: &WorkspaceState) -> PortableEventScope {
    match body {
        EventBody::ChannelCreated { channel_id, .. }
        | EventBody::DirectMessageChannelCreated { channel_id, .. }
        | EventBody::ChannelDetailsUpdated { channel_id, .. }
        | EventBody::ChannelMemberAdded { channel_id, .. }
        | EventBody::ChannelMemberRemoved { channel_id, .. }
        | EventBody::OpenMlsChannelGroupMemberAdded { channel_id, .. }
        | EventBody::OpenMlsChannelGroupMemberRemoved { channel_id, .. }
        | EventBody::OpenMlsChannelGroupSelfUpdated { channel_id, .. }
        | EventBody::ReadMarkerUpdated { channel_id, .. } => {
            PortableEventScope::Channel(channel_id.clone())
        }
        EventBody::ContentKeyEpochPublished {
            scope: chaft_types::ContentKeyScope::Channel { channel_id },
            ..
        } => PortableEventScope::Channel(channel_id.clone()),
        EventBody::ContentKeyEpochPublished {
            scope: chaft_types::ContentKeyScope::Workspace,
            ..
        } => PortableEventScope::Workspace,
        EventBody::MessageCreated { message_id, .. }
        | EventBody::MessageReplyCreated { message_id, .. }
        | EventBody::MessageCreatedEncrypted { message_id, .. }
        | EventBody::MessageReplyCreatedEncrypted { message_id, .. }
        | EventBody::MessageEdited { message_id, .. }
        | EventBody::MessageEditedEncrypted { message_id, .. }
        | EventBody::MessageDeleted { message_id }
        | EventBody::ReactionAdded { message_id, .. }
        | EventBody::ReactionRemoved { message_id, .. } => state
            .messages
            .get(message_id)
            .map(|message| PortableEventScope::Channel(message.channel_id.clone()))
            .unwrap_or(PortableEventScope::UnresolvedChannel),
        EventBody::WorkspaceCreated { .. }
        | EventBody::MemberInvited { .. }
        | EventBody::MemberRoleUpdated { .. }
        | EventBody::WorkspaceAccessPolicyUpdated { .. }
        | EventBody::WorkspaceInviteRecorded { .. }
        | EventBody::WorkspaceInviteCapabilityCreated { .. }
        | EventBody::WorkspaceInviteClaimed { .. }
        | EventBody::WorkspaceInviteResolved { .. }
        | EventBody::WorkspaceJoinRequestRecorded { .. }
        | EventBody::WorkspaceJoinRequestResolved { .. }
        | EventBody::MemberRemoved { .. }
        | EventBody::DeviceProfileUpdated { .. }
        | EventBody::PersonDeviceLinked { .. }
        | EventBody::PersonProfileUpdated { .. }
        | EventBody::DeviceKeyPackagePublished { .. }
        | EventBody::PeerEndpointPublished { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberAdded { .. }
        | EventBody::OpenMlsWorkspaceGroupMemberRemoved { .. }
        | EventBody::OpenMlsWorkspaceGroupSelfUpdated { .. } => PortableEventScope::Workspace,
    }
}

fn authorized_event_inventory_fingerprint(
    events: &[SignedEvent],
    workspace_id: &WorkspaceId,
    device_id: &DeviceId,
) -> Result<String, RuntimeError> {
    let verified = crate::verified_local_events_for_runtime(events);
    let mut state = WorkspaceState::new(workspace_id.clone());
    state.apply_batch(&verified)?;
    let readable_channel_ids = state
        .channels
        .keys()
        .filter(|channel_id| state.channel_accessible_to(channel_id, device_id))
        .cloned()
        .collect::<HashSet<_>>();
    let visible_events = events
        .iter()
        .filter(|event| portable_event_is_visible(event, &state, &readable_channel_ids))
        .cloned()
        .collect::<Vec<_>>();
    event_inventory_fingerprint(&visible_events)
}

fn event_inventory_fingerprint(events: &[SignedEvent]) -> Result<String, RuntimeError> {
    let mut hasher = blake3::Hasher::new();
    let mut records = events
        .iter()
        .map(|event| Ok((event.event_id.0.as_str(), serde_json::to_vec(event)?)))
        .collect::<Result<Vec<_>, RuntimeError>>()?;
    records.sort_unstable_by(|left, right| {
        left.0
            .cmp(right.0)
            .then_with(|| left.1.as_slice().cmp(right.1.as_slice()))
    });
    for (event_id, bytes) in records {
        hasher.update(&(event_id.len() as u64).to_le_bytes());
        hasher.update(event_id.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn causal_frontier(events: &[SignedEvent]) -> Vec<String> {
    let event_ids = events
        .iter()
        .map(|event| event.event_id.clone())
        .collect::<BTreeSet<_>>();
    let parent_ids = events
        .iter()
        .flat_map(|event| event.event.parents.iter())
        .filter(|parent_id| event_ids.contains(*parent_id))
        .cloned()
        .collect::<BTreeSet<_>>();
    event_ids
        .difference(&parent_ids)
        .map(|event_id| event_id.0.clone())
        .collect()
}

fn portable_attachment_id(
    channel_id: &ChannelId,
    message_id: &MessageId,
    attachment_index: usize,
) -> String {
    let mut hasher = blake3::Hasher::new();
    for value in [channel_id.0.as_bytes(), message_id.0.as_bytes()] {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    hasher.update(&(attachment_index as u64).to_le_bytes());
    format!("att_{}", hasher.finalize().to_hex())
}

fn attachment_archive_path(
    channel_id: &ChannelId,
    message_id: &MessageId,
    attachment_index: usize,
    attachment: &AttachmentRef,
) -> String {
    let channel = safe_component(&channel_id.0, "channel", 64);
    let message = safe_component(&message_id.0, "message", 64);
    let display_name = safe_component(&attachment.display_name, "attachment", 96);
    let attachment_id = portable_attachment_id(channel_id, message_id, attachment_index);
    format!(
        "files/{channel}/{message}/{attachment_index:03}-{}-{display_name}",
        &attachment_id[4..28]
    )
}

fn channel_html_path(name: &str, channel_id: &ChannelId) -> String {
    format!(
        "html/channels/{}-{}.html",
        safe_component(name, "conversation", 72),
        short_hash(&channel_id.0)
    )
}

fn safe_component(value: &str, fallback: &str, max_len: usize) -> String {
    let mut result = String::new();
    let mut pending_separator = false;
    for character in value.chars() {
        if result.len() >= max_len {
            break;
        }
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            if pending_separator && !result.is_empty() && result.len() < max_len {
                result.push('-');
            }
            pending_separator = false;
            result.push(character.to_ascii_lowercase());
        } else {
            pending_separator = true;
        }
    }
    result = result.trim_matches(['.', '-', '_']).to_owned();
    if result.is_empty() {
        fallback.to_owned()
    } else {
        result
    }
}

fn short_hash(value: &str) -> String {
    blake3::hash(value.as_bytes()).to_hex()[..24].to_owned()
}

fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn format_unix_ms(unix_ms: i64) -> String {
    let nanos = i128::from(unix_ms).saturating_mul(1_000_000);
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map(format_datetime)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn format_datetime(value: OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

fn resolve_path_from_existing_ancestor(path: &Path) -> Result<PathBuf, RuntimeError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }

    let mut existing = normalized.as_path();
    let mut missing_components = Vec::<OsString>::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| {
                    RuntimeError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "portable export path has no existing ancestor",
                    ))
                })?;
                missing_components.push(name.to_os_string());
                existing = existing.parent().ok_or_else(|| {
                    RuntimeError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "portable export path has no parent",
                    ))
                })?;
            }
            Err(error) => return Err(error.into()),
        }
    }

    let mut resolved = fs::canonicalize(existing)?;
    for component in missing_components.iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn portable_export_temp_name(file_name: &std::ffi::OsStr, counter: u64) -> OsString {
    let mut temp_name = OsString::from(".");
    temp_name.push(file_name);
    temp_name.push(format!(".tmp.{}.{}", process::id(), counter));
    temp_name
}

#[cfg(unix)]
fn create_portable_export_temp_file(
    path: &Path,
) -> Result<(PortableExportPublication, fs::File), RuntimeError> {
    let file_name = path.file_name().ok_or_else(|| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "portable export path has no file name",
        ))
    })?;
    let parent_path = path.parent().ok_or_else(|| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "portable export path has no parent",
        ))
    })?;
    let parent_fd = rustix::fs::open(
        parent_path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let parent_dir = fs::File::from(parent_fd);
    let parent_metadata = parent_dir.metadata()?;
    let reopened_parent_path = fs::canonicalize(parent_path)?;
    let reopened_parent_metadata = fs::metadata(&reopened_parent_path)?;
    if reopened_parent_path != parent_path
        || reopened_parent_metadata.dev() != parent_metadata.dev()
        || reopened_parent_metadata.ino() != parent_metadata.ino()
    {
        return Err(RuntimeError::PortableExportDestinationUnsafe);
    }
    for _ in 0..32 {
        let counter = PORTABLE_EXPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_name = portable_export_temp_name(file_name, counter);
        match rustix::fs::openat(
            &parent_dir,
            &temp_name,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        ) {
            Ok(file) => {
                let file = fs::File::from(file);
                let temp_metadata = file.metadata()?;
                return Ok((
                    PortableExportPublication {
                        parent_path: parent_path.to_path_buf(),
                        parent_device: parent_metadata.dev(),
                        parent_inode: parent_metadata.ino(),
                        parent_dir,
                        temp_name,
                        temp_device: temp_metadata.dev(),
                        temp_inode: temp_metadata.ino(),
                        output_name: file_name.to_os_string(),
                    },
                    file,
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique portable export temporary file",
    )))
}

#[cfg(not(unix))]
fn create_portable_export_temp_file(
    path: &Path,
) -> Result<(PortableExportPublication, fs::File), RuntimeError> {
    let file_name = path.file_name().ok_or_else(|| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "portable export path has no file name",
        ))
    })?;
    for _ in 0..32 {
        let counter = PORTABLE_EXPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = path.with_file_name(portable_export_temp_name(file_name, counter));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                return Ok((
                    PortableExportPublication {
                        output_path: path.to_path_buf(),
                        temp_path,
                    },
                    file,
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique portable export temporary file",
    )))
}

impl PortableExportPublication {
    #[cfg(unix)]
    fn ensure_parent_identity(&self) -> Result<(), RuntimeError> {
        let metadata = fs::metadata(&self.parent_path)?;
        if metadata.dev() != self.parent_device || metadata.ino() != self.parent_inode {
            return Err(RuntimeError::PortableExportDestinationUnsafe);
        }
        Ok(())
    }

    #[cfg(unix)]
    fn ensure_temp_identity(&self, completed_file: &fs::File) -> Result<(), RuntimeError> {
        let reopened = rustix::fs::openat(
            &self.parent_dir,
            &self.temp_name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        let reopened = fs::File::from(reopened);
        let expected = completed_file.metadata()?;
        let actual = reopened.metadata()?;
        if expected.dev() != self.temp_device
            || expected.ino() != self.temp_inode
            || actual.dev() != self.temp_device
            || actual.ino() != self.temp_inode
            || expected.len() != actual.len()
        {
            return Err(RuntimeError::PortableExportDestinationUnsafe);
        }
        Ok(())
    }

    #[cfg(unix)]
    fn publish(&self, completed_file: &fs::File) -> Result<(), RuntimeError> {
        self.ensure_parent_identity()?;
        self.ensure_temp_identity(completed_file)?;
        #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
        rustix::fs::renameat_with(
            &self.parent_dir,
            &self.temp_name,
            &self.parent_dir,
            &self.output_name,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
        #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
        {
            rustix::fs::linkat(
                &self.parent_dir,
                &self.temp_name,
                &self.parent_dir,
                &self.output_name,
                rustix::fs::AtFlags::empty(),
            )
            .map_err(std::io::Error::from)?;
            rustix::fs::unlinkat(
                &self.parent_dir,
                &self.temp_name,
                rustix::fs::AtFlags::empty(),
            )
            .map_err(std::io::Error::from)?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn publish(&self) -> Result<(), RuntimeError> {
        tempfile::TempPath::try_from_path(self.temp_path.clone())?
            .persist_noclobber(&self.output_path)
            .map_err(|error| RuntimeError::Io(error.error))?;
        Ok(())
    }

    #[cfg(unix)]
    fn cleanup_temp(&self) -> Result<(), RuntimeError> {
        let reopened = match rustix::fs::openat(
            &self.parent_dir,
            &self.temp_name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            Ok(file) => fs::File::from(file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        let metadata = reopened.metadata()?;
        if metadata.dev() != self.temp_device || metadata.ino() != self.temp_inode {
            return Err(RuntimeError::PortableExportDestinationUnsafe);
        }
        drop(reopened);
        match rustix::fs::unlinkat(
            &self.parent_dir,
            &self.temp_name,
            rustix::fs::AtFlags::empty(),
        ) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(std::io::Error::from(error).into()),
        }
    }

    #[cfg(not(unix))]
    fn cleanup_temp(&self) -> Result<(), RuntimeError> {
        match fs::remove_file(&self.temp_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    #[cfg(unix)]
    fn sync_parent(&self) -> Result<(), RuntimeError> {
        self.parent_dir.sync_all()?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn sync_parent(&self) -> Result<(), RuntimeError> {
        let parent = self.output_path.parent().unwrap_or_else(|| Path::new("."));
        let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
        Ok(())
    }
}

fn hash_open_file(file: &mut fs::File) -> Result<String, RuntimeError> {
    let original_position = file.stream_position()?;
    file.seek(SeekFrom::Start(0))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut hasher = Sha256::new();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    file.seek(SeekFrom::Start(original_position))?;
    Ok(lower_hex(&hasher.finalize()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    lower_hex(&hasher.finalize())
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[usize::from(byte >> 4)] as char);
        encoded.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use chaft_crypto::{PayloadEncryption, SealedPayload};
    use chaft_types::SignableEvent;
    use tempfile::tempdir;
    use zip::ZipArchive;

    use super::*;

    #[test]
    fn safe_components_never_create_archive_paths() {
        assert_eq!(safe_component("../../A room", "room", 64), "a-room");
        assert_eq!(safe_component(" \\ / ", "attachment", 64), "attachment");
        assert!(
            !attachment_archive_path(
                &ChannelId("../channel".to_owned()),
                &MessageId("../message".to_owned()),
                0,
                &AttachmentRef {
                    blob_hash: "a".repeat(64),
                    media_type: "text/plain".to_owned(),
                    byte_len: 1,
                    display_name: "../../secret.txt".to_owned(),
                    attachment_id: String::new(),
                    encryption: None,
                },
            )
            .contains("..")
        );
    }

    #[test]
    fn attachment_export_ids_and_paths_do_not_trust_source_ids() {
        let channel_id = ChannelId("channel".to_owned());
        let first_message_id = MessageId("first-message".to_owned());
        let second_message_id = MessageId("second-message".to_owned());
        let attachment = AttachmentRef {
            blob_hash: "a".repeat(64),
            media_type: "application/pdf".to_owned(),
            byte_len: 1,
            display_name: "Quarterly report.pdf".to_owned(),
            attachment_id: "duplicated-source-id".to_owned(),
            encryption: None,
        };

        let first_id = portable_attachment_id(&channel_id, &first_message_id, 0);
        let second_id = portable_attachment_id(&channel_id, &second_message_id, 0);
        let first_path = attachment_archive_path(&channel_id, &first_message_id, 0, &attachment);
        let second_path = attachment_archive_path(&channel_id, &second_message_id, 0, &attachment);

        assert_ne!(first_id, second_id);
        assert!(first_id.starts_with("att_"));
        assert_eq!(first_id.len(), 68);
        assert_ne!(first_path, second_path);
        assert!(first_path.ends_with("quarterly-report.pdf"));
    }

    #[test]
    fn archive_builder_rejects_duplicate_and_reserved_entries() {
        let output_dir = tempdir().unwrap();
        let output_path = output_dir.path().join("duplicate.zip");
        let file = fs::File::create(&output_path).unwrap();
        let mut archive = ArchiveBuilder::new(file);
        archive
            .write_bytes("data/messages.jsonl", b"first", CompressionMethod::Stored)
            .unwrap();

        let duplicate = archive
            .write_bytes("data/messages.jsonl", b"second", CompressionMethod::Stored)
            .unwrap_err();
        assert!(matches!(
            duplicate,
            RuntimeError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData
        ));
        let reserved = archive
            .write_bytes("SHA256SUMS", b"forged", CompressionMethod::Stored)
            .unwrap_err();
        assert!(matches!(
            reserved,
            RuntimeError::Io(error) if error.kind() == std::io::ErrorKind::InvalidData
        ));
        archive.finish().unwrap();

        assert_eq!(
            archive_entry_names(&output_path)
                .iter()
                .filter(|name| name.as_str() == "data/messages.jsonl")
                .count(),
            1
        );
    }

    #[test]
    fn html_escaping_blocks_markup_and_attribute_injection() {
        assert_eq!(
            html_escape("<script data-x=\"1\">'&"),
            "&lt;script data-x=&quot;1&quot;&gt;&#39;&amp;"
        );
    }

    #[test]
    fn event_inventory_fingerprint_covers_content_and_ignores_row_order() {
        let runtime_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime.create_workspace("Fingerprint", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let events = runtime.workspace_events(&workspace_id).unwrap();
        let mut reversed = events.clone();
        reversed.reverse();
        let mut changed = events.clone();
        let EventBody::WorkspaceCreated { name } = &mut changed[0].event.body else {
            panic!("first event should create the workspace");
        };
        *name = "Same id, changed content".to_owned();

        assert_eq!(
            event_inventory_fingerprint(&events).unwrap(),
            event_inventory_fingerprint(&reversed).unwrap()
        );
        assert_ne!(
            event_inventory_fingerprint(&events).unwrap(),
            event_inventory_fingerprint(&changed).unwrap()
        );
    }

    #[test]
    fn unresolved_message_events_are_not_treated_as_workspace_scoped() {
        let state = WorkspaceState::new(WorkspaceId("wrk_scope_test".to_owned()));
        assert_eq!(
            portable_event_body_scope(
                &EventBody::MessageDeleted {
                    message_id: MessageId("msg_orphan".to_owned()),
                },
                &state,
            ),
            PortableEventScope::UnresolvedChannel
        );
        assert_eq!(
            portable_event_body_scope(
                &EventBody::WorkspaceCreated {
                    name: "Workspace".to_owned(),
                },
                &state,
            ),
            PortableEventScope::Workspace
        );
    }

    #[test]
    fn channel_event_scope_reconciliation_fails_closed() {
        let runtime_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime.create_workspace("Scope", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let public_channel_id = ChannelId(created.channel_id);
        let reader_device_id = DeviceId("dev_scope_reader".to_owned());
        runtime
            .invite_member(
                workspace_id.clone(),
                reader_device_id.clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let private = runtime
            .create_channel(workspace_id.clone(), "private", true)
            .unwrap();
        let private_channel_id = ChannelId(private.channel_id);
        let sent = runtime
            .send_message(
                workspace_id.clone(),
                private_channel_id.clone(),
                "private body",
            )
            .unwrap();
        let message_id = MessageId(sent.message_id);
        let context = runtime.workspace_write_context(&workspace_id).unwrap();
        let signed_reaction = |channel_id| {
            let mut event = SignableEvent::new(
                workspace_id.clone(),
                channel_id,
                runtime.device_id().clone(),
                EventBody::ReactionAdded {
                    message_id: message_id.clone(),
                    reaction: "+1".to_owned(),
                },
            );
            event.parents = context.head_event_ids.clone();
            runtime.identity.sign_event(event)
        };
        let matching = signed_reaction(Some(private_channel_id.clone()));
        let mismatched = signed_reaction(Some(public_channel_id.clone()));
        let missing_envelope = signed_reaction(None);
        let unexpected_workspace_envelope = runtime.identity.sign_event(SignableEvent::new(
            workspace_id.clone(),
            Some(public_channel_id),
            runtime.device_id().clone(),
            EventBody::WorkspaceCreated {
                name: "unexpected".to_owned(),
            },
        ));

        let events = runtime.workspace_events(&workspace_id).unwrap();
        let verified = crate::verified_local_events_for_runtime(&events);
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&verified).unwrap();

        assert_eq!(
            portable_event_scope(&matching, &state),
            PortableEventScope::Channel(private_channel_id)
        );
        assert_eq!(
            portable_event_scope(&mismatched, &state),
            PortableEventScope::UnresolvedChannel
        );
        assert_eq!(
            portable_event_scope(&missing_envelope, &state),
            PortableEventScope::UnresolvedChannel
        );
        assert_eq!(
            portable_event_scope(&unexpected_workspace_envelope, &state),
            PortableEventScope::UnresolvedChannel
        );

        let base =
            authorized_event_inventory_fingerprint(&events, &workspace_id, &reader_device_id)
                .unwrap();
        for malformed in [mismatched, missing_envelope] {
            let mut with_malformed = events.clone();
            with_malformed.push(malformed);
            assert_eq!(
                authorized_event_inventory_fingerprint(
                    &with_malformed,
                    &workspace_id,
                    &reader_device_id,
                )
                .unwrap(),
                base
            );
        }
    }

    #[test]
    fn portable_format_fixtures_preserve_v1_and_validate_v2_dispatch() {
        let schema = portable_export_json_schema();
        let legacy = serde_json::from_str::<serde_json::Value>(include_str!(
            "../tests/fixtures/portable-workspace-v1.json"
        ))
        .unwrap();
        let current = serde_json::from_str::<serde_json::Value>(include_str!(
            "../tests/fixtures/portable-workspace-v2.json"
        ))
        .unwrap();

        assert_eq!(legacy["manifest"]["schemaVersion"], 1);
        assert_eq!(legacy["manifest"]["kind"], "chaft.portable-workspace.v1");
        assert!(legacy["manifest"]["completeness"]["corruptEventCount"].is_number());
        assert_eq!(
            legacy["attachments"][0]["attachmentId"],
            "legacy_fixture_attachment"
        );
        assert!(legacy["attachments"][0].get("sourceAttachmentId").is_none());
        assert!(legacy["members"][0].get("actorId").is_none());

        assert_eq!(current["manifest"]["schemaVersion"], 2);
        assert_eq!(current["manifest"]["kind"], PORTABLE_EXPORT_KIND);
        assert_matches_portable_schema(&schema, None, &current["manifest"]);
        assert_matches_portable_schema(&schema, Some("workspace"), &current["workspace"]);
        assert_matches_portable_schema(&schema, Some("completeness"), &current["completeness"]);
        for (collection, definition) in [
            ("channels", "channel"),
            ("members", "member"),
            ("messages", "message"),
            ("attachments", "attachment"),
        ] {
            for record in current[collection].as_array().unwrap() {
                assert_matches_portable_schema(&schema, Some(definition), record);
            }
        }
    }

    #[test]
    fn portable_archive_contains_readable_and_structured_records() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Export <Team>", "general")
            .unwrap();
        let sent = runtime
            .send_message(
                WorkspaceId(created.workspace_id.clone()),
                ChannelId(created.channel_id.clone()),
                "Hello <script>alert(1)</script>",
            )
            .unwrap();
        runtime
            .add_reaction(
                WorkspaceId(created.workspace_id.clone()),
                MessageId(sent.message_id),
                "+1",
            )
            .unwrap();
        let output_path = output_dir.path().join("workspace.zip");

        let report = runtime
            .export_portable_workspace_archive(WorkspaceId(created.workspace_id), &output_path)
            .unwrap();

        assert_eq!(report.schema_version, 2);
        assert_eq!(report.kind, PORTABLE_EXPORT_KIND);
        assert_eq!(report.message_count, 1);
        assert_eq!(report.warning_count, 0);
        assert!(report.archive_bytes > 0);
        assert_eq!(report.archive_sha256.len(), 64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&output_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let file = fs::File::open(&output_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        for required in [
            "index.html",
            "manifest.json",
            "completeness.json",
            "SHA256SUMS",
            "data/workspace.json",
            "data/channels.jsonl",
            "data/members.jsonl",
            "data/messages.jsonl",
            "data/attachments.jsonl",
            PORTABLE_EXPORT_SCHEMA_PATH,
        ] {
            let entry = archive
                .by_name(required)
                .unwrap_or_else(|_| panic!("missing {required}"));
            assert_eq!(entry.unix_mode().unwrap() & 0o777, 0o600);
        }

        let mut index = String::new();
        archive
            .by_name("index.html")
            .unwrap()
            .read_to_string(&mut index)
            .unwrap();
        assert!(index.contains("Export &lt;Team&gt;"));
        assert!(!index.contains("<script>"));

        let mut messages = String::new();
        archive
            .by_name("data/messages.jsonl")
            .unwrap()
            .read_to_string(&mut messages)
            .unwrap();
        let row = serde_json::from_str::<serde_json::Value>(messages.trim()).unwrap();
        assert_eq!(row["markdown"], "Hello <script>alert(1)</script>");
        assert_eq!(row["bodyState"], "available");
        assert_eq!(row["author"]["actorKind"], "device");
        assert_eq!(row["author"]["actorId"], runtime.device_id().0.as_str());
        assert_eq!(row["reactions"][0]["value"], "+1");
        assert_eq!(row["reactions"][0]["count"], 1);
        assert_eq!(
            row["reactions"][0]["actorDeviceIds"][0],
            runtime.device_id().0.as_str()
        );

        let member = serde_json::from_str::<serde_json::Value>(
            String::from_utf8(read_zip_entry(&output_path, "data/members.jsonl"))
                .unwrap()
                .trim(),
        )
        .unwrap();
        assert_eq!(member["actorKind"], "device");
        assert_eq!(member["actorId"], runtime.device_id().0.as_str());
        assert_eq!(member["role"], "owner");
        assert_eq!(member["active"], true);

        let manifest = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            "manifest.json",
        ))
        .unwrap();
        assert_eq!(
            manifest["selection"]["coverageScope"],
            "local_device_snapshot"
        );
        assert_eq!(manifest["selection"]["syncCompleteness"], "not_assessed");
        assert!(manifest["completeness"]["corruptEventCount"].is_null());
        assert_eq!(
            manifest["completeness"]["storageIntegrityAssessment"],
            "not_assessed_for_privacy"
        );

        let schema = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            PORTABLE_EXPORT_SCHEMA_PATH,
        ))
        .unwrap();
        assert_eq!(
            schema["$defs"]["attachment"]["properties"]["sourceAttachmentId"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            schema["properties"]["selection"]["properties"]["coverageScope"]["const"],
            "local_device_snapshot"
        );
        jsonschema::draft202012::meta::validate(&schema)
            .expect("portable export schema must satisfy the Draft 2020-12 meta-schema");
        assert_matches_portable_schema(&schema, None, &manifest);
        let workspace = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            "data/workspace.json",
        ))
        .unwrap();
        assert_matches_portable_schema(&schema, Some("workspace"), &workspace);
        let completeness = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            "completeness.json",
        ))
        .unwrap();
        assert_matches_portable_schema(&schema, Some("completeness"), &completeness);
        for (entry, definition) in [
            ("data/channels.jsonl", "channel"),
            ("data/members.jsonl", "member"),
            ("data/messages.jsonl", "message"),
            ("data/attachments.jsonl", "attachment"),
        ] {
            let rows = String::from_utf8(read_zip_entry(&output_path, entry)).unwrap();
            for row in rows.lines() {
                let value = serde_json::from_str::<serde_json::Value>(row).unwrap();
                assert_matches_portable_schema(&schema, Some(definition), &value);
            }
        }

        let checksums = read_zip_entry(&output_path, "SHA256SUMS");
        let checksums = String::from_utf8(checksums).unwrap();
        for line in checksums.lines() {
            let (expected, path) = line.split_once("  ").expect("strict sha256sum row");
            assert_eq!(expected.len(), 64);
            assert!(expected.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(!path.contains("  #"));
            assert_eq!(sha256_bytes(&read_zip_entry(&output_path, path)), expected);
        }
    }

    #[test]
    fn portable_export_does_not_overwrite_an_existing_destination() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime.create_workspace("No clobber", "general").unwrap();
        let output_path = output_dir.path().join("workspace.zip");
        fs::write(&output_path, b"keep this file").unwrap();

        let error = runtime
            .export_portable_workspace_archive(
                WorkspaceId(created.workspace_id.clone()),
                &output_path,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read(&output_path).unwrap(), b"keep this file");
        assert!(fs::read_dir(output_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")
        }));
    }

    #[test]
    fn portable_export_final_publish_does_not_clobber_a_raced_destination() {
        let output_dir = tempdir().unwrap();
        let output_path = fs::canonicalize(output_dir.path())
            .unwrap()
            .join("workspace.zip");
        let (publication, mut temp_file) = create_portable_export_temp_file(&output_path).unwrap();
        temp_file.write_all(b"new archive").unwrap();
        temp_file.sync_all().unwrap();
        fs::write(&output_path, b"file created while export was running").unwrap();

        #[cfg(unix)]
        let error = publication.publish(&temp_file).unwrap_err();
        #[cfg(not(unix))]
        let error = {
            drop(temp_file);
            publication.publish().unwrap_err()
        };

        assert!(matches!(
            error,
            RuntimeError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(
            fs::read(&output_path).unwrap(),
            b"file created while export was running"
        );
        publication.cleanup_temp().unwrap();
        assert!(fs::read_dir(output_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp.")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn portable_export_rejects_replaced_parent_directory_before_publish() {
        let root = tempdir().unwrap();
        let original_parent = root.path().join("destination");
        let moved_parent = root.path().join("original-destination");
        fs::create_dir(&original_parent).unwrap();
        let output_path = fs::canonicalize(&original_parent)
            .unwrap()
            .join("workspace.zip");
        let (publication, mut temp_file) = create_portable_export_temp_file(&output_path).unwrap();
        temp_file.write_all(b"completed archive").unwrap();
        temp_file.sync_all().unwrap();
        let temp_name = publication.temp_name.clone();

        fs::rename(&original_parent, &moved_parent).unwrap();
        fs::create_dir(&original_parent).unwrap();
        fs::write(original_parent.join(&temp_name), b"substitute").unwrap();

        let error = publication.publish(&temp_file).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationUnsafe
        ));
        assert!(!original_parent.join("workspace.zip").exists());
        assert!(!moved_parent.join("workspace.zip").exists());

        publication.cleanup_temp().unwrap();
        assert!(!moved_parent.join(&temp_name).exists());
        assert_eq!(
            fs::read(original_parent.join(&temp_name)).unwrap(),
            b"substitute"
        );
    }

    #[cfg(unix)]
    #[test]
    fn portable_export_rejects_substituted_temp_file_before_publish() {
        let output_dir = tempdir().unwrap();
        let output_path = fs::canonicalize(output_dir.path())
            .unwrap()
            .join("workspace.zip");
        let (publication, mut temp_file) = create_portable_export_temp_file(&output_path).unwrap();
        temp_file.write_all(b"completed archive").unwrap();
        temp_file.sync_all().unwrap();
        let temp_path = output_dir.path().join(&publication.temp_name);
        fs::remove_file(&temp_path).unwrap();
        fs::write(&temp_path, b"substitute").unwrap();

        let error = publication.publish(&temp_file).unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationUnsafe
        ));
        assert!(!output_path.exists());
        assert!(matches!(
            publication.cleanup_temp().unwrap_err(),
            RuntimeError::PortableExportDestinationUnsafe
        ));
        assert_eq!(fs::read(temp_path).unwrap(), b"substitute");
    }

    #[cfg(unix)]
    #[test]
    fn portable_export_rejects_ancestor_redirect_before_parent_open() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let selected_ancestor = root.path().join("selected");
        let selected_parent = selected_ancestor.join("destination");
        let moved_ancestor = root.path().join("moved-selected");
        let redirected_ancestor = root.path().join("redirected");
        let redirected_parent = redirected_ancestor.join("destination");
        fs::create_dir_all(&selected_parent).unwrap();
        fs::create_dir_all(&redirected_parent).unwrap();
        let validated_output = fs::canonicalize(&selected_parent)
            .unwrap()
            .join("workspace.zip");

        fs::rename(&selected_ancestor, &moved_ancestor).unwrap();
        symlink(&redirected_ancestor, &selected_ancestor).unwrap();

        let error = match create_portable_export_temp_file(&validated_output) {
            Ok(_) => panic!("redirected ancestor must not receive an export temporary file"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationUnsafe
        ));
        assert!(fs::read_dir(&redirected_parent).unwrap().next().is_none());
        assert!(
            fs::read_dir(moved_ancestor.join("destination"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn portable_export_rejects_runtime_owned_destinations() {
        let runtime_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Runtime destination", "general")
            .unwrap();
        let output_path = runtime_dir.path().join("workspace.zip");

        let error = runtime
            .export_portable_workspace_archive(
                WorkspaceId(created.workspace_id.clone()),
                &output_path,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationInsideRuntime
        ));
        assert!(!output_path.exists());

        let nested_parent = runtime_dir.path().join("must-not-be-created");
        let nested_output = nested_parent.join("workspace.zip");
        let error = runtime
            .export_portable_workspace_archive(WorkspaceId(created.workspace_id), &nested_output)
            .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationInsideRuntime
        ));
        assert!(!nested_parent.exists());
    }

    #[test]
    fn portable_export_rejects_an_external_runtime_identity_path() {
        let runtime_dir = tempdir().unwrap();
        let identity_dir = tempdir().unwrap();
        let identity_path = identity_dir.path().join("external-device.json");
        let runtime = LocalRuntime::open(runtime_dir.path(), Some(identity_path.clone())).unwrap();
        let created = runtime
            .create_workspace("External identity", "general")
            .unwrap();

        let error = runtime
            .export_portable_workspace_archive(WorkspaceId(created.workspace_id), &identity_path)
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationInsideRuntime
        ));
        assert!(identity_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn portable_export_rejects_a_symbolic_link_destination() {
        use std::os::unix::fs::symlink;

        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Symlink destination", "general")
            .unwrap();
        let target_path = output_dir.path().join("target.zip");
        let output_path = output_dir.path().join("workspace.zip");
        symlink(&target_path, &output_path).unwrap();

        let error = runtime
            .export_portable_workspace_archive(WorkspaceId(created.workspace_id), &output_path)
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::PortableExportDestinationUnsafe
        ));
        assert!(!target_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn portable_export_uses_the_resolved_parent_for_archive_creation() {
        use std::os::unix::fs::symlink;

        let runtime_dir = tempdir().unwrap();
        let output_root = tempdir().unwrap();
        let real_parent = output_root.path().join("real");
        let linked_parent = output_root.path().join("linked");
        fs::create_dir(&real_parent).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();
        let requested_path = linked_parent.join("workspace.zip");
        let resolved_path = fs::canonicalize(&real_parent)
            .unwrap()
            .join("workspace.zip");
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Resolved destination", "general")
            .unwrap();

        let report = runtime
            .export_portable_workspace_archive(WorkspaceId(created.workspace_id), &requested_path)
            .unwrap();

        assert_eq!(PathBuf::from(report.output_path), requested_path);
        assert!(resolved_path.exists());
        assert!(archive_entry_names(&resolved_path).contains(&"manifest.json".to_owned()));
    }

    #[test]
    fn deleted_attachment_metadata_is_retained_without_plaintext() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Deleted attachment", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let input_path = output_dir.path().join("private-note.txt");
        fs::write(&input_path, b"must not survive deletion").unwrap();
        let sent = runtime
            .send_message_with_attachment_file(
                workspace_id.clone(),
                ChannelId(created.channel_id),
                "deleted file",
                &input_path,
                "text/plain",
            )
            .unwrap();
        runtime
            .delete_message(workspace_id.clone(), MessageId(sent.message_id))
            .unwrap();
        let output_path = output_dir.path().join("deleted.zip");

        let report = runtime
            .export_portable_workspace_archive(workspace_id, &output_path)
            .unwrap();

        assert_eq!(report.attachment_count, 1);
        assert_eq!(report.included_attachment_count, 0);
        assert_eq!(report.missing_attachment_count, 0);
        let attachments =
            String::from_utf8(read_zip_entry(&output_path, "data/attachments.jsonl")).unwrap();
        let record = serde_json::from_str::<serde_json::Value>(attachments.trim()).unwrap();
        assert_eq!(record["availability"], "excluded_deleted");
        assert!(record["archivePath"].is_null());
        assert!(record["plaintextSha256"].is_null());
        assert!(
            archive_entry_names(&output_path)
                .iter()
                .all(|name| !name.starts_with("files/"))
        );
    }

    #[test]
    fn duplicate_source_attachment_ids_remain_distinct_in_export() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Duplicate source attachments", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let channel_id = ChannelId(created.channel_id);
        let context = runtime.workspace_write_context(&workspace_id).unwrap();
        let message_id = MessageId::new();
        let duplicate_source_id = "legacy-attachment-id";
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            runtime.device_id().clone(),
            EventBody::MessageCreated {
                message_id: message_id.clone(),
                markdown: "two legacy attachments".to_owned(),
                attachments: vec![
                    AttachmentRef {
                        blob_hash: "a".repeat(64),
                        media_type: "text/plain".to_owned(),
                        byte_len: 1,
                        display_name: "first.txt".to_owned(),
                        attachment_id: duplicate_source_id.to_owned(),
                        encryption: None,
                    },
                    AttachmentRef {
                        blob_hash: "b".repeat(64),
                        media_type: "text/plain".to_owned(),
                        byte_len: 1,
                        display_name: "second.txt".to_owned(),
                        attachment_id: duplicate_source_id.to_owned(),
                        encryption: None,
                    },
                ],
            },
        );
        message.parents = context.head_event_ids.clone();
        runtime
            .sign_authorize_and_append_with_history(message, &context.events)
            .unwrap();
        let output_path = output_dir.path().join("duplicate-source-ids.zip");

        let report = runtime
            .export_portable_workspace_archive(workspace_id, &output_path)
            .unwrap();

        assert_eq!(report.attachment_count, 2);
        assert_eq!(report.missing_attachment_count, 2);
        let attachments =
            String::from_utf8(read_zip_entry(&output_path, "data/attachments.jsonl")).unwrap();
        let rows = attachments
            .lines()
            .map(|row| serde_json::from_str::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["sourceAttachmentId"], duplicate_source_id);
        assert_eq!(rows[1]["sourceAttachmentId"], duplicate_source_id);
        assert_ne!(rows[0]["attachmentId"], rows[1]["attachmentId"]);

        let messages =
            String::from_utf8(read_zip_entry(&output_path, "data/messages.jsonl")).unwrap();
        let row = serde_json::from_str::<serde_json::Value>(messages.trim()).unwrap();
        assert_eq!(row["messageId"], message_id.0);
        assert_eq!(row["attachmentIds"].as_array().unwrap().len(), 2);
        assert_ne!(row["attachmentIds"][0], row["attachmentIds"][1]);
    }

    #[test]
    fn corrupt_encrypted_message_body_becomes_a_completeness_warning() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Corrupt body export", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let channel_id = ChannelId(created.channel_id);
        let context = runtime.workspace_write_context(&workspace_id).unwrap();
        let content_key = runtime
            .content_key_for_local_write_in_state(
                &workspace_id,
                &channel_id,
                &context.state,
                &context.events,
            )
            .unwrap();
        let message_id = MessageId::new();
        let mut message = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id),
            runtime.device_id().clone(),
            EventBody::MessageCreatedEncrypted {
                message_id: message_id.clone(),
                sealed_markdown: SealedPayload {
                    mode: PayloadEncryption::Aes256GcmSiv,
                    key_id: content_key.key_id().to_owned(),
                    nonce: vec![0; 12],
                    aad: Vec::new(),
                    bytes: b"not authenticated ciphertext".to_vec(),
                },
                attachments: Vec::new(),
            },
        );
        message.parents = context.head_event_ids.clone();
        runtime
            .sign_authorize_and_append_with_history(message, &context.events)
            .unwrap();
        let output_path = output_dir.path().join("corrupt-body.zip");

        let report = runtime
            .export_portable_workspace_archive(workspace_id, &output_path)
            .unwrap();

        assert_eq!(report.message_count, 1);
        assert_eq!(report.unavailable_message_body_count, 1);
        let messages =
            String::from_utf8(read_zip_entry(&output_path, "data/messages.jsonl")).unwrap();
        let message = serde_json::from_str::<serde_json::Value>(messages.trim()).unwrap();
        assert_eq!(message["messageId"], message_id.0);
        assert_eq!(message["bodyState"], "unavailable_encrypted");
        assert_eq!(message["markdown"], "");
        let completeness = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            "completeness.json",
        ))
        .unwrap();
        assert_eq!(
            completeness["unavailableMessageBodies"][0]["reason"],
            "decryption_failed"
        );
    }

    #[test]
    fn unattributable_corrupt_rows_do_not_leak_through_export_diagnostics() {
        let runtime_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let runtime = LocalRuntime::open(runtime_dir.path(), None).unwrap();
        let created = runtime
            .create_workspace("Private diagnostics", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let connection = rusqlite::Connection::open(runtime.event_store_path()).unwrap();
        connection
            .execute(
                "
                INSERT INTO events (
                    event_id,
                    workspace_id,
                    channel_id,
                    author_device_id,
                    physical_ms,
                    logical,
                    self_contained_signature_valid,
                    event_json
                ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)
                ",
                rusqlite::params![
                    "evt_unattributable_corrupt_export_row",
                    workspace_id.0.as_str(),
                    "dev_corrupt",
                    1_i64,
                    0_i64,
                    1_i64,
                    b"not valid signed event json".as_slice()
                ],
            )
            .unwrap();
        drop(connection);
        assert_eq!(
            runtime
                .workspace_storage_health(workspace_id.clone())
                .unwrap()
                .corrupt_event_count,
            1
        );
        let output_path = output_dir.path().join("private-diagnostics.zip");

        let report = runtime
            .export_portable_workspace_archive(workspace_id, &output_path)
            .unwrap();

        assert_eq!(report.corrupt_event_count, None);
        let completeness = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &output_path,
            "completeness.json",
        ))
        .unwrap();
        assert!(completeness["corruptEventCount"].is_null());
        assert_eq!(
            completeness["storageIntegrityAssessment"],
            "not_assessed_for_privacy"
        );
        assert!(
            completeness["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|warning| warning["code"] != "corrupt_events")
        );
    }

    #[test]
    fn removed_message_and_reaction_actors_remain_in_member_directory() {
        let alice_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice.create_workspace("Actor history", "general").unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        let channel_id = ChannelId(created.channel_id);
        let bob_device_id = bob.device_id().clone();

        alice
            .invite_member(
                workspace_id.clone(),
                bob_device_id.clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        let workspace_key = alice.export_workspace_key(workspace_id.clone()).unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }
        bob.import_workspace_key(workspace_key).unwrap();
        let sent = bob
            .send_message(
                workspace_id.clone(),
                channel_id,
                "message from a former member",
            )
            .unwrap();
        bob.add_reaction(
            workspace_id.clone(),
            MessageId(sent.message_id.clone()),
            "+1",
        )
        .unwrap();
        for event in bob.workspace_events(&workspace_id).unwrap() {
            alice.store.append_event(&event).unwrap();
        }
        alice
            .remove_member(workspace_id.clone(), bob_device_id.clone())
            .unwrap();

        let output_path = output_dir.path().join("removed-actor.zip");
        alice
            .export_portable_workspace_archive(workspace_id, &output_path)
            .unwrap();
        let members = String::from_utf8(read_zip_entry(&output_path, "data/members.jsonl"))
            .unwrap()
            .lines()
            .map(|row| serde_json::from_str::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();
        let messages = String::from_utf8(read_zip_entry(&output_path, "data/messages.jsonl"))
            .unwrap()
            .lines()
            .map(|row| serde_json::from_str::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();

        let former_member = members
            .iter()
            .find(|member| member["deviceId"] == bob_device_id.0)
            .expect("former message author remains in actor directory");
        assert_eq!(former_member["role"], "member");
        assert_eq!(former_member["active"], false);

        let message = messages
            .iter()
            .find(|message| message["messageId"] == sent.message_id)
            .unwrap();
        assert_eq!(message["author"]["deviceId"], bob_device_id.0);
        assert_eq!(
            message["reactions"][0]["actorDeviceIds"][0],
            bob_device_id.0
        );

        let directory_device_ids = members
            .iter()
            .map(|member| member["deviceId"].as_str().unwrap())
            .collect::<HashSet<_>>();
        for message in &messages {
            assert!(directory_device_ids.contains(message["author"]["deviceId"].as_str().unwrap()));
            for actor_device_id in message["reactions"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|reaction| reaction["actorDeviceIds"].as_array().unwrap())
            {
                assert!(directory_device_ids.contains(actor_device_id.as_str().unwrap()));
            }
        }
    }

    #[test]
    fn unreadable_private_channel_inventory_is_absent_from_export() {
        let alice_dir = tempdir().unwrap();
        let bob_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();
        let alice = LocalRuntime::open(alice_dir.path(), None).unwrap();
        let bob = LocalRuntime::open(bob_dir.path(), None).unwrap();
        let created = alice
            .create_workspace("Authorized export", "general")
            .unwrap();
        let workspace_id = WorkspaceId(created.workspace_id);
        alice
            .invite_member(
                workspace_id.clone(),
                bob.device_id().clone(),
                WorkspaceRole::Member,
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let before_path = output_dir.path().join("before.zip");
        bob.export_portable_workspace_archive(workspace_id.clone(), &before_path)
            .unwrap();
        let before_manifest = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &before_path,
            "manifest.json",
        ))
        .unwrap();

        let private = alice
            .create_channel(workspace_id.clone(), "acquisition-secret", true)
            .unwrap();
        alice
            .send_message(
                workspace_id.clone(),
                ChannelId(private.channel_id.clone()),
                "confidential-target.example",
            )
            .unwrap();
        for event in alice.workspace_events(&workspace_id).unwrap() {
            bob.store.append_event(&event).unwrap();
        }

        let after_path = output_dir.path().join("after.zip");
        let report = bob
            .export_portable_workspace_archive(workspace_id, &after_path)
            .unwrap();
        let after_manifest = serde_json::from_slice::<serde_json::Value>(&read_zip_entry(
            &after_path,
            "manifest.json",
        ))
        .unwrap();

        assert_eq!(report.channel_count, 1);
        assert_eq!(report.message_count, 0);
        for field in [
            "acceptedEventCount",
            "parseableEventCount",
            "appliedEventCount",
            "eventInventoryBlake3",
            "causalFrontierEventIds",
        ] {
            assert_eq!(
                before_manifest["cutoff"][field],
                after_manifest["cutoff"][field]
            );
        }
        let archive_text = archive_entry_names(&after_path)
            .into_iter()
            .map(|name| String::from_utf8_lossy(&read_zip_entry(&after_path, &name)).into_owned())
            .collect::<String>();
        assert!(!archive_text.contains("acquisition-secret"));
        assert!(!archive_text.contains("confidential-target.example"));
        assert!(!archive_text.contains(&private.channel_id));
    }

    fn read_zip_entry(path: &Path, entry_name: &str) -> Vec<u8> {
        let file = fs::File::open(path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name(entry_name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    fn archive_entry_names(path: &Path) -> Vec<String> {
        let file = fs::File::open(path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_owned())
            .collect()
    }

    fn assert_matches_portable_schema(
        schema: &serde_json::Value,
        definition: Option<&str>,
        instance: &serde_json::Value,
    ) {
        let validation_schema = definition.map_or_else(
            || schema.clone(),
            |definition| {
                json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "$defs": schema["$defs"].clone(),
                    "$ref": format!("#/$defs/{definition}"),
                })
            },
        );
        let validator = jsonschema::draft202012::new(&validation_schema)
            .expect("portable export schema definition must compile");
        let errors = validator
            .iter_errors(instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(
            errors.is_empty(),
            "portable export record did not satisfy {definition:?}: {errors:#?}\n{instance:#}"
        );
    }
}
