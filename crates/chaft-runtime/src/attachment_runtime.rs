use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use chaft_core::WorkspaceState;
use chaft_crypto::{
    encrypted_blob_ref_from_payload, open_attachment_blob, seal_attachment_blob,
    sealed_payload_from_encrypted_blob_ref,
};
use chaft_media::{BlobPruneReport, BlobStore};
use chaft_types::{
    ATTACHMENT_PLAINTEXT_MAX_BYTES, AttachmentRef, ChannelId, MessageId, WorkspaceId,
};

use crate::{
    LocalRuntime, PrunedBlobCache, ResolvedContentKey, RuntimeError, SavedAttachment,
    attachment_blob_hashes, validate_message_id_reference, validate_message_markdown_size,
    validate_runtime_path, validate_workspace_id_reference,
};

pub(crate) const ATTACHMENT_FILE_MAX_BYTES: u64 = ATTACHMENT_PLAINTEXT_MAX_BYTES;
static ATTACHMENT_EXPORT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct PendingAttachment {
    pub(crate) display_name: String,
    pub(crate) media_type: String,
    pub(crate) plaintext: Vec<u8>,
}

pub(crate) fn attachment_media_type_for_path(
    file_path: &Path,
    requested_media_type: &str,
) -> String {
    let requested_media_type = requested_media_type.trim();
    if !requested_media_type.is_empty() {
        return requested_media_type.to_owned();
    }

    let extension = file_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "json" => "application/json",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
    .to_owned()
}

pub(crate) fn attachment_id_for_message_slot(
    message_id: &MessageId,
    attachment_index: usize,
) -> String {
    format!("att_{}_{}", message_id.0, attachment_index)
}

pub(crate) fn validate_attachment_plaintext_size(actual_bytes: u64) -> Result<(), RuntimeError> {
    if actual_bytes > ATTACHMENT_FILE_MAX_BYTES {
        return Err(RuntimeError::AttachmentFileTooLarge {
            actual_bytes,
            max_bytes: ATTACHMENT_FILE_MAX_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn read_attachment_file_with_limit(file_path: &Path) -> Result<Vec<u8>, RuntimeError> {
    validate_runtime_path(file_path, "attachment file path")?;
    let metadata = fs::metadata(file_path)?;
    validate_attachment_plaintext_size(metadata.len())?;

    let file = fs::File::open(file_path)?;
    let capacity = metadata.len().min(ATTACHMENT_FILE_MAX_BYTES) as usize;
    let mut plaintext = Vec::with_capacity(capacity);
    let mut limited_file = file.take(ATTACHMENT_FILE_MAX_BYTES + 1);
    limited_file.read_to_end(&mut plaintext)?;
    validate_attachment_plaintext_size(plaintext.len() as u64)?;
    Ok(plaintext)
}

pub(crate) fn write_attachment_export_file(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    validate_runtime_path(path, "attachment output path")?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_attachment_export_temp_file(path)?;
    let result = (|| -> Result<(), RuntimeError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_attachment_export_parent_directory(parent)?;
        }
        Ok(())
    })();

    if result.is_err() {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }

    result
}

fn create_unique_attachment_export_temp_file(
    path: &Path,
) -> Result<(PathBuf, fs::File), RuntimeError> {
    let file_name = path.file_name().ok_or_else(|| {
        RuntimeError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "attachment export path has no file name",
        ))
    })?;

    for _ in 0..32 {
        let counter = ATTACHMENT_EXPORT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut temp_file_name = OsString::from(".");
        temp_file_name.push(file_name);
        temp_file_name.push(format!(".tmp.{}.{}", process::id(), counter));
        let temp_path = path.with_file_name(temp_file_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique attachment export temp file",
    )))
}

fn sync_attachment_export_parent_directory(parent: &Path) -> Result<(), RuntimeError> {
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        let _ = fs::File::open(parent).and_then(|directory| directory.sync_all());
    }
    Ok(())
}

impl LocalRuntime {
    pub fn send_message_with_attachment_file(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        markdown: impl AsRef<str>,
        file_path: impl AsRef<Path>,
        media_type: impl AsRef<str>,
    ) -> Result<crate::CreatedMessage, RuntimeError> {
        self.send_message_with_attachment_file_reply(
            workspace_id,
            channel_id,
            None,
            markdown,
            file_path,
            media_type,
        )
    }

    pub fn send_message_with_attachment_file_reply(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
        reply_to_message_id: Option<MessageId>,
        markdown: impl AsRef<str>,
        file_path: impl AsRef<Path>,
        media_type: impl AsRef<str>,
    ) -> Result<crate::CreatedMessage, RuntimeError> {
        let markdown = markdown.as_ref();
        validate_message_markdown_size(markdown)?;
        let file_path = file_path.as_ref();
        validate_runtime_path(file_path, "attachment file path")?;
        let display_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("attachment")
            .to_owned();
        let attachment = PendingAttachment {
            display_name,
            media_type: attachment_media_type_for_path(file_path, media_type.as_ref()),
            plaintext: read_attachment_file_with_limit(file_path)?,
        };
        self.send_message_with_attachments(
            workspace_id,
            channel_id,
            markdown,
            reply_to_message_id,
            vec![attachment],
        )
    }

    pub fn save_attachment_to_file(
        &self,
        workspace_id: WorkspaceId,
        message_id: MessageId,
        attachment_selector: impl AsRef<str>,
        output_path: impl AsRef<Path>,
    ) -> Result<SavedAttachment, RuntimeError> {
        validate_workspace_id_reference(&workspace_id)?;
        validate_message_id_reference(&message_id)?;
        let attachment_selector = attachment_selector.as_ref().to_owned();
        let output_path = output_path.as_ref();
        validate_runtime_path(output_path, "attachment output path")?;
        let events = self.materialized_workspace_events(&workspace_id)?;
        let mut state = WorkspaceState::new(workspace_id.clone());
        state.apply_batch(&events)?;
        let (channel_id, attachment, attachment_index) = Self::message_attachment_from_state(
            &state,
            self.identity.device_id(),
            &workspace_id,
            &message_id,
            &attachment_selector,
        )?;
        let encrypted =
            attachment
                .encryption
                .as_ref()
                .ok_or_else(|| RuntimeError::AttachmentNotEncrypted {
                    blob_hash: attachment.blob_hash.clone(),
                })?;
        let ciphertext = self
            .open_blob_store()?
            .get_complete_bytes(&attachment.blob_hash)?
            .ok_or_else(|| RuntimeError::AttachmentBlobMissing {
                blob_hash: attachment.blob_hash.clone(),
            })?;
        let sealed = sealed_payload_from_encrypted_blob_ref(encrypted, ciphertext);
        let workspace_key = self.load_workspace_key(&workspace_id)?;
        let content_key = self
            .content_key_for_materialized_payload(
                &workspace_id,
                &channel_id,
                &state,
                workspace_key.as_ref(),
                &sealed.key_id,
            )?
            .ok_or_else(|| RuntimeError::ContentKeyMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
                key_id: sealed.key_id.clone(),
            })?;
        let plaintext = open_attachment_blob(
            content_key.content_key(),
            &sealed,
            &workspace_id,
            &channel_id,
            &message_id,
            attachment_index as u32,
        )?;

        write_attachment_export_file(output_path, &plaintext)?;

        Ok(SavedAttachment {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            message_id: message_id.0,
            blob_hash: attachment.blob_hash,
            attachment_id: attachment.attachment_id,
            display_name: attachment.display_name,
            media_type: attachment.media_type,
            byte_len: attachment.byte_len,
            output_path: output_path.to_string_lossy().into_owned(),
        })
    }

    pub fn prune_unreferenced_blobs(&self) -> Result<PrunedBlobCache, RuntimeError> {
        let workspace_ids = self
            .store
            .list_workspace_ids()?
            .into_iter()
            .map(WorkspaceId)
            .collect::<Vec<_>>();
        let mut referenced_blob_hashes = BTreeSet::new();

        for workspace_id in &workspace_ids {
            let events = self.materialized_workspace_events(workspace_id)?;
            referenced_blob_hashes.extend(attachment_blob_hashes(&events));
        }

        let BlobPruneReport {
            referenced_blob_hashes,
            removed_blob_hashes,
            removed_manifest_hashes,
            removed_chunk_hashes,
            removed_temp_file_paths,
        } = self
            .open_blob_store()?
            .prune_unreferenced(&referenced_blob_hashes)?;

        Ok(PrunedBlobCache::from_parts(
            workspace_ids
                .into_iter()
                .map(|workspace_id| workspace_id.0)
                .collect(),
            referenced_blob_hashes,
            removed_blob_hashes,
            removed_manifest_hashes,
            removed_chunk_hashes,
            removed_temp_file_paths,
        ))
    }

    pub(crate) fn open_blob_store(&self) -> Result<BlobStore, RuntimeError> {
        Ok(BlobStore::open(&self.paths.blob_store)?)
    }

    pub(crate) fn seal_and_store_attachments(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
        message_id: &MessageId,
        content_key: &ResolvedContentKey,
        pending_attachments: Vec<PendingAttachment>,
    ) -> Result<Vec<AttachmentRef>, RuntimeError> {
        if pending_attachments.is_empty() {
            return Ok(Vec::new());
        }

        let blob_store = self.open_blob_store()?;
        let mut attachments = Vec::with_capacity(pending_attachments.len());
        for (index, pending) in pending_attachments.into_iter().enumerate() {
            validate_attachment_plaintext_size(pending.plaintext.len() as u64)?;
            let sealed = seal_attachment_blob(
                content_key.key_id(),
                content_key.content_key(),
                workspace_id,
                channel_id,
                message_id,
                index as u32,
                &pending.plaintext,
            )?;
            let encryption =
                encrypted_blob_ref_from_payload(&sealed, pending.plaintext.len() as u64)?;
            let descriptor = blob_store.put_bytes(&sealed.bytes)?;
            attachments.push(AttachmentRef {
                blob_hash: descriptor.hash,
                media_type: if pending.media_type.is_empty() {
                    "application/octet-stream".to_owned()
                } else {
                    pending.media_type
                },
                byte_len: descriptor.byte_len,
                display_name: pending.display_name,
                attachment_id: attachment_id_for_message_slot(message_id, index),
                encryption: Some(encryption),
            });
        }

        Ok(attachments)
    }
}
