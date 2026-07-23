use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("manifest serialization error")]
    Serde(#[from] serde_json::Error),
    #[error("blob hash mismatch: expected {expected}, actual {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("invalid blob hash")]
    InvalidHash,
    #[error("invalid blob descriptor")]
    InvalidDescriptor,
    #[error("blob manifest is too large ({actual_bytes} bytes, max {max_bytes})")]
    ManifestTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("blob chunk is too large ({actual_bytes} bytes, max {max_bytes})")]
    ChunkTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("blob file is too large ({actual_bytes} bytes, max {max_bytes})")]
    BlobTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("blob store path is required")]
    BlobStorePathRequired,
    #[error("blob store path is too large ({actual_bytes} bytes, max {max_bytes})")]
    BlobStorePathTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

pub const BLOB_FILE_MAX_BYTES: usize = (128 * 1024 * 1024) + 1024;
pub const BLOB_MANIFEST_MAX_BYTES: usize = 1024 * 1024;
pub const BLOB_CHUNK_FILE_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const BLOB_DESCRIPTOR_MAX_CHUNKS: usize = 16 * 1024;
pub const BLOB_STORE_PATH_MAX_BYTES: usize = 64 * 1024;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobDescriptor {
    pub hash: String,
    pub byte_len: u64,
    pub chunk_size: usize,
    pub chunk_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobAvailability {
    pub hash: String,
    pub has_whole_blob: bool,
    pub descriptor: Option<BlobDescriptor>,
    pub available_chunk_hashes: Vec<String>,
    pub missing_chunk_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobPruneReport {
    pub referenced_blob_hashes: Vec<String>,
    pub removed_blob_hashes: Vec<String>,
    pub removed_manifest_hashes: Vec<String>,
    pub removed_chunk_hashes: Vec<String>,
    pub removed_temp_file_paths: Vec<String>,
}

impl BlobAvailability {
    pub fn is_complete(&self) -> bool {
        if validate_blob_availability(self).is_err() {
            return false;
        }
        if self.has_whole_blob {
            return true;
        }

        let Some(descriptor) = self.descriptor.as_ref() else {
            return false;
        };
        if descriptor.hash != self.hash || !self.missing_chunk_hashes.is_empty() {
            return false;
        }
        if self.available_chunk_hashes.len() != descriptor.chunk_hashes.len() {
            return false;
        }

        let mut available = self.available_chunk_hashes.clone();
        let mut expected = descriptor.chunk_hashes.clone();
        available.sort();
        expected.sort();
        available == expected
    }
}

pub fn describe_blob(bytes: &[u8], chunk_size: usize) -> BlobDescriptor {
    let chunk_size = chunk_size.max(1);
    let chunk_hashes = bytes
        .chunks(chunk_size)
        .map(|chunk| blake3::hash(chunk).to_hex().to_string())
        .collect();

    BlobDescriptor {
        hash: blake3::hash(bytes).to_hex().to_string(),
        byte_len: bytes.len() as u64,
        chunk_size,
        chunk_hashes,
    }
}

pub fn blob_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, MediaError> {
        let root = root.as_ref().to_path_buf();
        validate_blob_store_path(&root)?;
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn put_bytes(&self, bytes: &[u8]) -> Result<BlobDescriptor, MediaError> {
        let descriptor = describe_blob(bytes, 1024 * 1024);
        self.put_bytes_with_hash(&descriptor.hash, bytes)?;
        Ok(descriptor)
    }

    pub fn put_bytes_chunked(
        &self,
        bytes: &[u8],
        chunk_size: usize,
    ) -> Result<BlobDescriptor, MediaError> {
        let chunk_size = chunk_size.max(1);
        validate_chunk_plan(bytes.len(), chunk_size)?;
        let descriptor = describe_blob(bytes, chunk_size);
        self.put_manifest(&descriptor)?;

        for (chunk_hash, chunk) in descriptor
            .chunk_hashes
            .iter()
            .zip(bytes.chunks(descriptor.chunk_size))
        {
            self.put_chunk_with_hash(chunk_hash, chunk)?;
        }

        Ok(descriptor)
    }

    pub fn put_bytes_with_hash(&self, expected_hash: &str, bytes: &[u8]) -> Result<(), MediaError> {
        validate_hash(expected_hash)?;
        validate_blob_file_size(bytes.len())?;
        let actual = blob_hash(bytes);
        if actual != expected_hash {
            return Err(MediaError::HashMismatch {
                expected: expected_hash.to_owned(),
                actual,
            });
        }

        let path = self.blob_path(expected_hash)?;
        if self.has_blob(expected_hash)? {
            return Ok(());
        }
        if path.exists() {
            fs::remove_file(&path)?;
        }

        write_file_atomically(&path, bytes)?;
        Ok(())
    }

    pub fn get_bytes(&self, hash: &str) -> Result<Option<Vec<u8>>, MediaError> {
        validate_hash(hash)?;
        let path = self.blob_path(hash)?;
        if !path.exists() {
            return Ok(None);
        }

        let bytes = read_blob_file(&path)?;
        let actual = blob_hash(&bytes);
        if actual != hash {
            return Err(MediaError::HashMismatch {
                expected: hash.to_owned(),
                actual,
            });
        }
        Ok(Some(bytes))
    }

    pub fn get_bytes_chunked(&self, hash: &str) -> Result<Option<Vec<u8>>, MediaError> {
        let Some(descriptor) = self.get_manifest(hash)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();

        for (chunk_index, chunk_hash) in descriptor.chunk_hashes.iter().enumerate() {
            let Some(chunk) = self.get_chunk(chunk_hash)? else {
                return Ok(None);
            };
            validate_chunk_payload(&descriptor, chunk_index, &chunk)?;
            bytes.extend_from_slice(&chunk);
        }

        validate_reassembled_blob(&descriptor, &bytes)?;
        Ok(Some(bytes))
    }

    pub fn get_complete_bytes(&self, hash: &str) -> Result<Option<Vec<u8>>, MediaError> {
        if let Some(bytes) = self.get_bytes(hash)? {
            return Ok(Some(bytes));
        }
        self.get_bytes_chunked(hash)
    }

    pub fn has_blob(&self, hash: &str) -> Result<bool, MediaError> {
        validate_hash(hash)?;
        let metadata = match fs::metadata(self.blob_path(hash)?) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        Ok(metadata.is_file() && metadata.len() <= BLOB_FILE_MAX_BYTES as u64)
    }

    pub fn has_complete_blob(&self, hash: &str) -> Result<bool, MediaError> {
        if self.has_blob(hash)? {
            return Ok(true);
        }
        Ok(self
            .availability(hash)?
            .is_some_and(|availability| availability.is_complete()))
    }

    pub fn put_chunk_with_hash(&self, expected_hash: &str, bytes: &[u8]) -> Result<(), MediaError> {
        validate_hash(expected_hash)?;
        validate_chunk_file_size(bytes.len())?;
        let actual = blob_hash(bytes);
        if actual != expected_hash {
            return Err(MediaError::HashMismatch {
                expected: expected_hash.to_owned(),
                actual,
            });
        }

        let path = self.chunk_path(expected_hash)?;
        if path.exists() {
            return Ok(());
        }

        write_file_atomically(&path, bytes)?;
        Ok(())
    }

    pub fn get_chunk(&self, hash: &str) -> Result<Option<Vec<u8>>, MediaError> {
        validate_hash(hash)?;
        let path = self.chunk_path(hash)?;
        if !path.exists() {
            return Ok(None);
        }

        let bytes = read_chunk_file(&path)?;
        let actual = blob_hash(&bytes);
        if actual != hash {
            return Err(MediaError::HashMismatch {
                expected: hash.to_owned(),
                actual,
            });
        }
        Ok(Some(bytes))
    }

    pub fn has_chunk(&self, hash: &str) -> Result<bool, MediaError> {
        validate_hash(hash)?;
        Ok(self.chunk_path(hash)?.exists())
    }

    pub fn put_manifest(&self, descriptor: &BlobDescriptor) -> Result<(), MediaError> {
        validate_descriptor(descriptor)?;
        let path = self.manifest_path(&descriptor.hash)?;
        let bytes = serde_json::to_vec(descriptor)?;
        write_file_atomically(&path, &bytes)?;
        Ok(())
    }

    pub fn get_manifest(&self, hash: &str) -> Result<Option<BlobDescriptor>, MediaError> {
        validate_hash(hash)?;
        let path = self.manifest_path(hash)?;
        if !path.exists() {
            return Ok(None);
        }

        let descriptor: BlobDescriptor = serde_json::from_slice(&read_manifest_file(&path)?)?;
        validate_descriptor(&descriptor)?;
        if descriptor.hash != hash {
            return Err(MediaError::HashMismatch {
                expected: hash.to_owned(),
                actual: descriptor.hash,
            });
        }
        Ok(Some(descriptor))
    }

    pub fn missing_chunks(&self, descriptor: &BlobDescriptor) -> Result<Vec<String>, MediaError> {
        validate_descriptor(descriptor)?;
        let mut missing = Vec::new();
        for (chunk_index, chunk_hash) in descriptor.chunk_hashes.iter().enumerate() {
            if !self.has_expected_chunk_len(descriptor, chunk_index)? {
                missing.push(chunk_hash.clone());
            }
        }
        Ok(missing)
    }

    pub fn availability(&self, hash: &str) -> Result<Option<BlobAvailability>, MediaError> {
        validate_hash(hash)?;
        let has_whole_blob = self.has_blob(hash)?;
        let descriptor = self.get_manifest(hash)?;

        if !has_whole_blob && descriptor.is_none() {
            return Ok(None);
        }

        let (available_chunk_hashes, missing_chunk_hashes) =
            if let Some(descriptor) = descriptor.as_ref() {
                let mut available = Vec::new();
                let mut missing = Vec::new();
                for (chunk_index, chunk_hash) in descriptor.chunk_hashes.iter().enumerate() {
                    if self.has_expected_chunk_len(descriptor, chunk_index)? {
                        available.push(chunk_hash.clone());
                    } else {
                        missing.push(chunk_hash.clone());
                    }
                }
                (available, missing)
            } else {
                (Vec::new(), Vec::new())
            };

        Ok(Some(BlobAvailability {
            hash: hash.to_owned(),
            has_whole_blob,
            descriptor,
            available_chunk_hashes,
            missing_chunk_hashes,
        }))
    }

    pub fn prune_unreferenced(
        &self,
        referenced_blob_hashes: &BTreeSet<String>,
    ) -> Result<BlobPruneReport, MediaError> {
        for hash in referenced_blob_hashes {
            validate_hash(hash)?;
        }

        let mut referenced_chunk_hashes = BTreeSet::new();
        for hash in referenced_blob_hashes {
            if let Some(descriptor) = self.get_manifest(hash)? {
                for chunk_hash in descriptor.chunk_hashes {
                    referenced_chunk_hashes.insert(chunk_hash);
                }
            }
        }

        let mut removed_blob_hashes = Vec::new();
        for (hash, path) in self.stored_blob_files()? {
            if referenced_blob_hashes.contains(&hash) {
                continue;
            }
            fs::remove_file(path)?;
            removed_blob_hashes.push(hash);
        }

        let mut removed_manifest_hashes = Vec::new();
        for (hash, path) in self.stored_manifest_files()? {
            if referenced_blob_hashes.contains(&hash) {
                continue;
            }
            fs::remove_file(path)?;
            removed_manifest_hashes.push(hash);
        }

        let mut removed_chunk_hashes = Vec::new();
        for (hash, path) in self.stored_chunk_files()? {
            if referenced_chunk_hashes.contains(&hash) {
                continue;
            }
            fs::remove_file(path)?;
            removed_chunk_hashes.push(hash);
        }

        let removed_temp_file_paths = self.remove_stale_temp_artifacts()?;
        self.remove_empty_directories()?;

        Ok(BlobPruneReport {
            referenced_blob_hashes: referenced_blob_hashes.iter().cloned().collect(),
            removed_blob_hashes,
            removed_manifest_hashes,
            removed_chunk_hashes,
            removed_temp_file_paths,
        })
    }

    fn blob_path(&self, hash: &str) -> Result<PathBuf, MediaError> {
        validate_hash(hash)?;
        let prefix = &hash[..2];
        Ok(self.root.join(prefix).join(hash))
    }

    fn chunk_path(&self, hash: &str) -> Result<PathBuf, MediaError> {
        validate_hash(hash)?;
        let prefix = &hash[..2];
        Ok(self.root.join("chunks").join(prefix).join(hash))
    }

    fn has_expected_chunk_len(
        &self,
        descriptor: &BlobDescriptor,
        chunk_index: usize,
    ) -> Result<bool, MediaError> {
        let chunk_hash = descriptor
            .chunk_hashes
            .get(chunk_index)
            .ok_or(MediaError::InvalidDescriptor)?;
        let path = self.chunk_path(chunk_hash)?;
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_file() {
            return Ok(false);
        }
        Ok(metadata.len() == expected_chunk_byte_len(descriptor, chunk_index)? as u64)
    }

    fn manifest_path(&self, hash: &str) -> Result<PathBuf, MediaError> {
        validate_hash(hash)?;
        let prefix = &hash[..2];
        Ok(self
            .root
            .join("manifests")
            .join(prefix)
            .join(format!("{hash}.json")))
    }

    fn stored_blob_files(&self) -> Result<Vec<(String, PathBuf)>, MediaError> {
        let mut files = Vec::new();
        if !self.root.exists() {
            return Ok(files);
        }

        for prefix in fs::read_dir(&self.root)? {
            let prefix = prefix?;
            if !prefix.file_type()?.is_dir() {
                continue;
            }
            let name = prefix.file_name();
            if name == "chunks" || name == "manifests" {
                continue;
            }
            collect_hash_files(&prefix.path(), None, &mut files)?;
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(files)
    }

    fn stored_chunk_files(&self) -> Result<Vec<(String, PathBuf)>, MediaError> {
        let mut files = Vec::new();
        collect_hash_files(&self.root.join("chunks"), None, &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(files)
    }

    fn stored_manifest_files(&self) -> Result<Vec<(String, PathBuf)>, MediaError> {
        let mut files = Vec::new();
        collect_hash_files(&self.root.join("manifests"), Some("json"), &mut files)?;
        files.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(files)
    }

    fn remove_empty_directories(&self) -> Result<(), MediaError> {
        remove_empty_directories_under(&self.root.join("chunks"))?;
        remove_empty_directories_under(&self.root.join("manifests"))?;
        remove_empty_directories_under(&self.root)?;
        fs::create_dir_all(&self.root)?;
        Ok(())
    }

    fn remove_stale_temp_artifacts(&self) -> Result<Vec<String>, MediaError> {
        let mut artifacts = Vec::new();
        collect_stale_temp_artifact_files(&self.root, &self.root, &mut artifacts)?;
        artifacts.sort_by(|left, right| left.0.cmp(&right.0));

        let mut removed = Vec::new();
        for (relative_path, path) in artifacts {
            match fs::remove_file(path) {
                Ok(()) => removed.push(relative_path),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(removed)
    }
}

fn validate_blob_store_path(path: &Path) -> Result<(), MediaError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(MediaError::BlobStorePathRequired);
    }
    if actual_bytes > BLOB_STORE_PATH_MAX_BYTES {
        return Err(MediaError::BlobStorePathTooLarge {
            actual_bytes,
            max_bytes: BLOB_STORE_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

pub fn plan_missing_chunks(
    local_store: &BlobStore,
    remote: &BlobAvailability,
) -> Result<Vec<String>, MediaError> {
    validate_blob_availability(remote)?;
    let Some(descriptor) = remote.descriptor.as_ref() else {
        return Ok(Vec::new());
    };

    let mut needed = Vec::new();
    let mut seen = BTreeSet::new();
    for chunk_hash in &remote.available_chunk_hashes {
        if descriptor.chunk_hashes.contains(chunk_hash)
            && seen.insert(chunk_hash.clone())
            && !local_store.has_chunk(chunk_hash)?
        {
            needed.push(chunk_hash.clone());
        }
    }
    Ok(needed)
}

pub fn validate_chunk_payload(
    descriptor: &BlobDescriptor,
    chunk_index: usize,
    bytes: &[u8],
) -> Result<(), MediaError> {
    validate_descriptor(descriptor)?;
    let expected_hash = descriptor
        .chunk_hashes
        .get(chunk_index)
        .ok_or(MediaError::InvalidDescriptor)?;
    if bytes.len() != expected_chunk_byte_len(descriptor, chunk_index)? {
        return Err(MediaError::InvalidDescriptor);
    }
    let actual = blob_hash(bytes);
    if actual != *expected_hash {
        return Err(MediaError::HashMismatch {
            expected: expected_hash.clone(),
            actual,
        });
    }
    Ok(())
}

pub fn validate_blob_descriptor(descriptor: &BlobDescriptor) -> Result<(), MediaError> {
    validate_descriptor(descriptor)
}

pub fn validate_blob_availability(availability: &BlobAvailability) -> Result<(), MediaError> {
    validate_hash(&availability.hash)?;

    let Some(descriptor) = availability.descriptor.as_ref() else {
        if availability.available_chunk_hashes.is_empty()
            && availability.missing_chunk_hashes.is_empty()
        {
            return Ok(());
        }
        return Err(MediaError::InvalidDescriptor);
    };

    validate_descriptor(descriptor)?;
    if descriptor.hash != availability.hash {
        return Err(MediaError::InvalidDescriptor);
    }

    let expected = count_chunk_hashes(&descriptor.chunk_hashes)?;
    let available = count_chunk_hashes(&availability.available_chunk_hashes)?;
    let missing = count_chunk_hashes(&availability.missing_chunk_hashes)?;

    for (chunk_hash, available_count) in &available {
        let expected_count = expected
            .get(chunk_hash)
            .ok_or(MediaError::InvalidDescriptor)?;
        let missing_count = missing.get(chunk_hash).copied().unwrap_or(0);
        if missing_count > 0 || available_count.saturating_add(missing_count) > *expected_count {
            return Err(MediaError::InvalidDescriptor);
        }
    }
    for (chunk_hash, missing_count) in &missing {
        let expected_count = expected
            .get(chunk_hash)
            .ok_or(MediaError::InvalidDescriptor)?;
        if missing_count > expected_count {
            return Err(MediaError::InvalidDescriptor);
        }
    }
    for (chunk_hash, expected_count) in &expected {
        let available_count = available.get(chunk_hash).copied().unwrap_or(0);
        let missing_count = missing.get(chunk_hash).copied().unwrap_or(0);
        if available_count.saturating_add(missing_count) != *expected_count {
            return Err(MediaError::InvalidDescriptor);
        }
    }

    Ok(())
}

fn count_chunk_hashes(hashes: &[String]) -> Result<BTreeMap<&str, usize>, MediaError> {
    let mut counts = BTreeMap::new();
    for hash in hashes {
        validate_hash(hash)?;
        let count = counts.entry(hash.as_str()).or_insert(0usize);
        *count = count.checked_add(1).ok_or(MediaError::InvalidDescriptor)?;
    }
    Ok(counts)
}

pub fn validate_reassembled_blob(
    descriptor: &BlobDescriptor,
    bytes: &[u8],
) -> Result<(), MediaError> {
    validate_descriptor(descriptor)?;
    if u64::try_from(bytes.len()).map_err(|_| MediaError::InvalidDescriptor)? != descriptor.byte_len
    {
        return Err(MediaError::InvalidDescriptor);
    }
    let actual = blob_hash(bytes);
    if actual != descriptor.hash {
        return Err(MediaError::HashMismatch {
            expected: descriptor.hash.clone(),
            actual,
        });
    }
    Ok(())
}

fn validate_hash(hash: &str) -> Result<(), MediaError> {
    if hash.len() == 64
        && hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(MediaError::InvalidHash)
    }
}

fn validate_descriptor(descriptor: &BlobDescriptor) -> Result<(), MediaError> {
    validate_hash(&descriptor.hash)?;
    let expected_chunk_count = expected_chunk_count(descriptor.byte_len, descriptor.chunk_size)?;
    if descriptor.chunk_hashes.len() != expected_chunk_count {
        return Err(MediaError::InvalidDescriptor);
    }
    for chunk_hash in &descriptor.chunk_hashes {
        validate_hash(chunk_hash)?;
    }
    Ok(())
}

fn expected_chunk_byte_len(
    descriptor: &BlobDescriptor,
    chunk_index: usize,
) -> Result<usize, MediaError> {
    validate_descriptor(descriptor)?;
    if chunk_index >= descriptor.chunk_hashes.len() {
        return Err(MediaError::InvalidDescriptor);
    }
    if chunk_index + 1 < descriptor.chunk_hashes.len() {
        return Ok(descriptor.chunk_size);
    }

    let chunks_before = u64::try_from(chunk_index).map_err(|_| MediaError::InvalidDescriptor)?;
    let consumed_before = chunks_before
        .checked_mul(descriptor.chunk_size as u64)
        .ok_or(MediaError::InvalidDescriptor)?;
    let remaining = descriptor
        .byte_len
        .checked_sub(consumed_before)
        .ok_or(MediaError::InvalidDescriptor)?;
    usize::try_from(remaining).map_err(|_| MediaError::InvalidDescriptor)
}

fn collect_hash_files(
    root: &Path,
    extension: Option<&str>,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), MediaError> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_hash_files(&entry.path(), extension, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        let hash = if let Some(extension) = extension {
            if path.extension().and_then(|value| value.to_str()) != Some(extension) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            stem.to_owned()
        } else {
            if path.extension().is_some() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            name.to_owned()
        };

        if validate_hash(&hash).is_ok() {
            files.push((hash, path));
        }
    }

    Ok(())
}

fn collect_stale_temp_artifact_files(
    root: &Path,
    store_root: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), MediaError> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_stale_temp_artifact_files(&path, store_root, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if !is_stale_blob_temp_artifact(name) {
            continue;
        }

        let relative_path = path
            .strip_prefix(store_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        files.push((relative_path, path));
    }

    Ok(())
}

fn is_stale_blob_temp_artifact(file_name: &str) -> bool {
    let Some(pid) = blob_temp_artifact_process_id(file_name) else {
        return false;
    };
    pid != process::id()
}

fn blob_temp_artifact_process_id(file_name: &str) -> Option<u32> {
    if !file_name.starts_with('.') {
        return None;
    }
    let marker_index = file_name.rfind(".tmp.")?;
    let tail = &file_name[marker_index + ".tmp.".len()..];
    let pid = tail.split('.').next()?;
    pid.parse().ok()
}

fn validate_chunk_plan(byte_len: usize, chunk_size: usize) -> Result<(), MediaError> {
    let byte_len = u64::try_from(byte_len).map_err(|_| MediaError::InvalidDescriptor)?;
    expected_chunk_count(byte_len, chunk_size)?;
    Ok(())
}

fn expected_chunk_count(byte_len: u64, chunk_size: usize) -> Result<usize, MediaError> {
    if chunk_size == 0 || chunk_size > BLOB_CHUNK_FILE_MAX_BYTES {
        return Err(MediaError::InvalidDescriptor);
    }
    let expected_chunk_count = if byte_len == 0 {
        0
    } else {
        let chunk_size = chunk_size as u64;
        let chunks = ((byte_len - 1) / chunk_size) + 1;
        usize::try_from(chunks).map_err(|_| MediaError::InvalidDescriptor)?
    };
    if expected_chunk_count > BLOB_DESCRIPTOR_MAX_CHUNKS {
        return Err(MediaError::InvalidDescriptor);
    }
    Ok(expected_chunk_count)
}

fn validate_blob_file_size(actual_bytes: usize) -> Result<(), MediaError> {
    if actual_bytes > BLOB_FILE_MAX_BYTES {
        return Err(MediaError::BlobTooLarge {
            actual_bytes,
            max_bytes: BLOB_FILE_MAX_BYTES,
        });
    }
    Ok(())
}

fn validate_chunk_file_size(actual_bytes: usize) -> Result<(), MediaError> {
    if actual_bytes > BLOB_CHUNK_FILE_MAX_BYTES {
        return Err(MediaError::ChunkTooLarge {
            actual_bytes,
            max_bytes: BLOB_CHUNK_FILE_MAX_BYTES,
        });
    }
    Ok(())
}

fn read_manifest_file(path: &Path) -> Result<Vec<u8>, MediaError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > BLOB_MANIFEST_MAX_BYTES as u64 {
        return Err(MediaError::ManifestTooLarge {
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes: BLOB_MANIFEST_MAX_BYTES,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(BLOB_MANIFEST_MAX_BYTES as u64 + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(BLOB_MANIFEST_MAX_BYTES as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() > BLOB_MANIFEST_MAX_BYTES {
        return Err(MediaError::ManifestTooLarge {
            actual_bytes: bytes.len(),
            max_bytes: BLOB_MANIFEST_MAX_BYTES,
        });
    }
    Ok(bytes)
}

fn read_blob_file(path: &Path) -> Result<Vec<u8>, MediaError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > BLOB_FILE_MAX_BYTES as u64 {
        return Err(MediaError::BlobTooLarge {
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes: BLOB_FILE_MAX_BYTES,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(BLOB_FILE_MAX_BYTES as u64 + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(BLOB_FILE_MAX_BYTES as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    validate_blob_file_size(bytes.len())?;
    Ok(bytes)
}

fn read_chunk_file(path: &Path) -> Result<Vec<u8>, MediaError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > BLOB_CHUNK_FILE_MAX_BYTES as u64 {
        return Err(MediaError::ChunkTooLarge {
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes: BLOB_CHUNK_FILE_MAX_BYTES,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(BLOB_CHUNK_FILE_MAX_BYTES as u64 + 1);
    let mut bytes =
        Vec::with_capacity(metadata.len().min(BLOB_CHUNK_FILE_MAX_BYTES as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    validate_chunk_file_size(bytes.len())?;
    Ok(bytes)
}

fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<(), MediaError> {
    let Some(parent) = path.parent() else {
        return Err(MediaError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "blob path has no parent directory",
        )));
    };
    fs::create_dir_all(parent)?;

    let (temp_path, mut file) = create_unique_temp_file(path)?;
    let result = (|| -> Result<(), MediaError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        sync_parent_directory(parent)?;
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

fn create_unique_temp_file(path: &Path) -> Result<(PathBuf, fs::File), MediaError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("blob");
    for _ in 0..32 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_file_name(format!(".{file_name}.tmp.{}.{}", process::id(), counter));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(MediaError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique blob temp file",
    )))
}

fn sync_parent_directory(parent: &Path) -> Result<(), MediaError> {
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

fn remove_empty_directories_under(root: &Path) -> Result<bool, MediaError> {
    if !root.exists() {
        return Ok(true);
    }

    let mut is_empty = true;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if remove_empty_directories_under(&entry.path())? {
                fs::remove_dir(entry.path())?;
            } else {
                is_empty = false;
            }
        } else {
            is_empty = false;
        }
    }

    Ok(is_empty)
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;

    fn temp_artifacts_under(root: &Path) -> Vec<PathBuf> {
        let mut artifacts = Vec::new();
        collect_temp_artifacts(root, &mut artifacts);
        artifacts.sort();
        artifacts
    }

    fn collect_temp_artifacts(root: &Path, artifacts: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                collect_temp_artifacts(&path, artifacts);
                continue;
            }
            if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.contains(".tmp."))
            {
                artifacts.push(path);
            }
        }
    }

    fn assert_blob_store_path_too_large<T>(result: Result<T, MediaError>) {
        match result {
            Err(MediaError::BlobStorePathTooLarge {
                actual_bytes,
                max_bytes,
            }) if actual_bytes > BLOB_STORE_PATH_MAX_BYTES
                && max_bytes == BLOB_STORE_PATH_MAX_BYTES => {}
            Ok(_) => panic!("expected oversized blob store path error, got ok"),
            Err(error) => panic!("expected oversized blob store path error, got {error}"),
        }
    }

    #[test]
    fn describes_blob_with_chunk_hashes() {
        let descriptor = describe_blob(b"abcdef", 2);

        assert_eq!(descriptor.byte_len, 6);
        assert_eq!(descriptor.chunk_hashes.len(), 3);
    }

    #[test]
    fn blob_store_rejects_blank_root_before_filesystem_work() {
        assert!(matches!(
            BlobStore::open(PathBuf::new()),
            Err(MediaError::BlobStorePathRequired)
        ));
    }

    #[test]
    fn blob_store_rejects_oversized_root_before_filesystem_work() {
        assert_blob_store_path_too_large(BlobStore::open(PathBuf::from(
            "b".repeat(BLOB_STORE_PATH_MAX_BYTES + 1),
        )));
    }

    #[test]
    fn stores_and_reads_content_addressed_blob() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = store.put_bytes(b"hello blob").unwrap();

        assert!(store.has_blob(&descriptor.hash).unwrap());
        assert_eq!(
            store.get_bytes(&descriptor.hash).unwrap(),
            Some(b"hello blob".to_vec())
        );
    }

    #[test]
    fn concurrent_same_hash_whole_blob_writes_succeed_without_temp_artifacts() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = Arc::new(BlobStore::open(tempdir.path()).unwrap());
        let bytes = Arc::new(b"same concurrent whole blob".repeat(1024));
        let hash = blob_hash(&bytes);
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let bytes = Arc::clone(&bytes);
                let hash = hash.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.put_bytes_with_hash(&hash, bytes.as_slice()).unwrap();
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(store.get_bytes(&hash).unwrap(), Some(bytes.to_vec()));
        assert!(temp_artifacts_under(tempdir.path()).is_empty());
    }

    #[test]
    fn concurrent_same_hash_chunk_writes_succeed_without_temp_artifacts() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = Arc::new(BlobStore::open(tempdir.path()).unwrap());
        let bytes = Arc::new(b"same concurrent chunk".repeat(1024));
        let hash = blob_hash(&bytes);
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let bytes = Arc::clone(&bytes);
                let hash = hash.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.put_chunk_with_hash(&hash, bytes.as_slice()).unwrap();
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(store.get_chunk(&hash).unwrap(), Some(bytes.to_vec()));
        assert!(temp_artifacts_under(tempdir.path()).is_empty());
    }

    #[test]
    fn concurrent_same_hash_manifest_writes_succeed_without_temp_artifacts() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = Arc::new(BlobStore::open(tempdir.path()).unwrap());
        let descriptor = Arc::new(describe_blob(b"same concurrent manifest", 4));
        let barrier = Arc::new(Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let descriptor = Arc::clone(&descriptor);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    store.put_manifest(&descriptor).unwrap();
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(
            store.get_manifest(&descriptor.hash).unwrap(),
            Some((*descriptor).clone())
        );
        assert!(temp_artifacts_under(tempdir.path()).is_empty());
    }

    #[test]
    fn rejects_bytes_that_do_not_match_expected_hash() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let expected = blob_hash(b"expected");

        assert!(matches!(
            store.put_bytes_with_hash(&expected, b"actual"),
            Err(MediaError::HashMismatch { .. })
        ));
    }

    #[test]
    fn rejects_uppercase_blob_hashes() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let mut hash = blob_hash(b"uppercase hash target");
        hash.make_ascii_uppercase();

        assert!(matches!(
            store.put_bytes_with_hash(&hash, b"uppercase hash target"),
            Err(MediaError::InvalidHash)
        ));
        assert!(matches!(
            store.get_bytes(&hash),
            Err(MediaError::InvalidHash)
        ));
        assert!(matches!(
            store.put_chunk_with_hash(&hash, b"uppercase chunk"),
            Err(MediaError::InvalidHash)
        ));
        assert!(matches!(
            store.get_chunk(&hash),
            Err(MediaError::InvalidHash)
        ));
    }

    #[test]
    fn rejects_uppercase_descriptor_and_availability_hashes() {
        let mut descriptor = describe_blob(b"uppercase descriptor", 8);
        descriptor.hash.make_ascii_uppercase();
        assert!(matches!(
            validate_blob_descriptor(&descriptor),
            Err(MediaError::InvalidHash)
        ));

        let mut descriptor = describe_blob(b"uppercase chunk descriptor", 8);
        descriptor.chunk_hashes[0].make_ascii_uppercase();
        assert!(matches!(
            validate_blob_descriptor(&descriptor),
            Err(MediaError::InvalidHash)
        ));

        let descriptor = describe_blob(b"uppercase availability", 8);
        let mut availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: vec![descriptor.chunk_hashes[0].clone()],
            missing_chunk_hashes: descriptor.chunk_hashes[1..].to_vec(),
        };
        availability.available_chunk_hashes[0].make_ascii_uppercase();
        assert!(matches!(
            validate_blob_availability(&availability),
            Err(MediaError::InvalidHash)
        ));
    }

    #[test]
    fn rejects_oversized_whole_blob_file_before_hashing() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = b"oversized whole blob target";
        let hash = blob_hash(bytes);
        let path = store.blob_path(&hash).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(&path).unwrap();
        file.set_len(BLOB_FILE_MAX_BYTES as u64 + 1).unwrap();

        assert!(!store.has_blob(&hash).unwrap());
        assert!(matches!(
            store.get_bytes(&hash),
            Err(MediaError::BlobTooLarge {
                actual_bytes,
                max_bytes: BLOB_FILE_MAX_BYTES,
            }) if actual_bytes == BLOB_FILE_MAX_BYTES + 1
        ));

        store.put_bytes_with_hash(&hash, bytes).unwrap();
        assert!(store.has_blob(&hash).unwrap());
        assert_eq!(store.get_bytes(&hash).unwrap(), Some(bytes.to_vec()));
    }

    #[test]
    fn stores_and_reassembles_chunked_blob() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = store.put_bytes_chunked(b"abcdef", 2).unwrap();

        assert_eq!(descriptor.chunk_hashes.len(), 3);
        assert!(store.has_chunk(&descriptor.chunk_hashes[0]).unwrap());
        assert!(store.has_complete_blob(&descriptor.hash).unwrap());
        assert_eq!(
            store.get_bytes_chunked(&descriptor.hash).unwrap(),
            Some(b"abcdef".to_vec())
        );
        assert_eq!(
            store.get_complete_bytes(&descriptor.hash).unwrap(),
            Some(b"abcdef".to_vec())
        );
    }

    #[test]
    fn rejects_chunked_manifest_with_inconsistent_chunk_count() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let mut descriptor = describe_blob(b"abcdef", 2);
        descriptor.chunk_hashes.pop();

        assert!(matches!(
            store.put_manifest(&descriptor),
            Err(MediaError::InvalidDescriptor)
        ));
        assert!(matches!(store.get_manifest(&descriptor.hash), Ok(None)));
    }

    #[test]
    fn rejects_oversized_manifest_file_before_parse() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let hash = blob_hash(b"oversized manifest target");
        let path = store.manifest_path(&hash).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(&path).unwrap();
        file.set_len(BLOB_MANIFEST_MAX_BYTES as u64 + 1).unwrap();

        assert!(matches!(
            store.get_manifest(&hash),
            Err(MediaError::ManifestTooLarge {
                actual_bytes,
                max_bytes: BLOB_MANIFEST_MAX_BYTES,
            }) if actual_bytes == BLOB_MANIFEST_MAX_BYTES + 1
        ));
    }

    #[test]
    fn rejects_oversized_chunk_file_before_hashing() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let hash = blob_hash(b"oversized chunk target");
        let path = store.chunk_path(&hash).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = fs::File::create(&path).unwrap();
        file.set_len(BLOB_CHUNK_FILE_MAX_BYTES as u64 + 1).unwrap();

        assert!(matches!(
            store.get_chunk(&hash),
            Err(MediaError::ChunkTooLarge {
                actual_bytes,
                max_bytes: BLOB_CHUNK_FILE_MAX_BYTES,
            }) if actual_bytes == BLOB_CHUNK_FILE_MAX_BYTES + 1
        ));
    }

    #[test]
    fn rejects_descriptor_with_oversized_chunk_size() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = BlobDescriptor {
            hash: blob_hash(b"oversized chunk descriptor"),
            byte_len: BLOB_CHUNK_FILE_MAX_BYTES as u64 + 1,
            chunk_size: BLOB_CHUNK_FILE_MAX_BYTES + 1,
            chunk_hashes: vec![blob_hash(b"oversized chunk")],
        };

        assert!(matches!(
            store.put_manifest(&descriptor),
            Err(MediaError::InvalidDescriptor)
        ));
    }

    #[test]
    fn rejects_chunked_write_plan_with_too_many_chunks_before_hashing() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let bytes = vec![0; BLOB_DESCRIPTOR_MAX_CHUNKS + 1];

        assert!(matches!(
            store.put_bytes_chunked(&bytes, 1),
            Err(MediaError::InvalidDescriptor)
        ));
    }

    #[test]
    fn rejects_descriptor_with_too_many_chunks() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = BlobDescriptor {
            hash: blob_hash(b"too many chunk descriptor"),
            byte_len: BLOB_DESCRIPTOR_MAX_CHUNKS as u64 + 1,
            chunk_size: 1,
            chunk_hashes: vec![blob_hash(b"chunk"); BLOB_DESCRIPTOR_MAX_CHUNKS + 1],
        };

        assert!(matches!(
            store.put_manifest(&descriptor),
            Err(MediaError::InvalidDescriptor)
        ));
    }

    #[test]
    fn rejects_reassembly_when_manifest_lies_about_chunk_lengths() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let chunks = [b"abc".as_slice(), b"def".as_slice()];
        let descriptor = BlobDescriptor {
            hash: blob_hash(b"abcdef"),
            byte_len: 1000,
            chunk_size: 500,
            chunk_hashes: chunks.iter().map(|chunk| blob_hash(chunk)).collect(),
        };

        store.put_manifest(&descriptor).unwrap();
        for (chunk_hash, chunk) in descriptor.chunk_hashes.iter().zip(chunks) {
            store.put_chunk_with_hash(chunk_hash, chunk).unwrap();
        }
        let availability = store.availability(&descriptor.hash).unwrap().unwrap();

        assert!(!availability.is_complete());
        assert_eq!(availability.missing_chunk_hashes, descriptor.chunk_hashes);
        assert!(!store.has_complete_blob(&descriptor.hash).unwrap());
        assert!(matches!(
            store.get_bytes_chunked(&descriptor.hash),
            Err(MediaError::InvalidDescriptor)
        ));
    }

    #[test]
    fn prunes_unreferenced_whole_blobs() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let kept = store.put_bytes(b"kept").unwrap();
        let removed = store.put_bytes(b"removed").unwrap();
        let referenced = BTreeSet::from([kept.hash.clone()]);

        let report = store.prune_unreferenced(&referenced).unwrap();

        assert_eq!(report.referenced_blob_hashes, vec![kept.hash.clone()]);
        assert_eq!(report.removed_blob_hashes, vec![removed.hash.clone()]);
        assert!(store.has_blob(&kept.hash).unwrap());
        assert!(!store.has_blob(&removed.hash).unwrap());
    }

    #[test]
    fn prunes_unreferenced_chunked_blobs_but_keeps_referenced_chunks() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let kept = store.put_bytes_chunked(b"abcdef", 2).unwrap();
        let removed = store.put_bytes_chunked(b"uvwxyz", 2).unwrap();
        let referenced = BTreeSet::from([kept.hash.clone()]);

        let report = store.prune_unreferenced(&referenced).unwrap();

        assert_eq!(report.referenced_blob_hashes, vec![kept.hash.clone()]);
        assert_eq!(report.removed_manifest_hashes, vec![removed.hash.clone()]);
        assert!(store.get_manifest(&kept.hash).unwrap().is_some());
        assert!(store.get_manifest(&removed.hash).unwrap().is_none());
        for chunk_hash in kept.chunk_hashes {
            assert!(store.has_chunk(&chunk_hash).unwrap());
        }
        for chunk_hash in removed.chunk_hashes {
            assert!(!store.has_chunk(&chunk_hash).unwrap());
        }
    }

    #[test]
    fn prunes_stale_temp_artifacts_without_touching_current_process_temps() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let target_hash = blob_hash(b"temp artifact target");
        let blob_path = store.blob_path(&target_hash).unwrap();
        fs::create_dir_all(blob_path.parent().unwrap()).unwrap();
        let old_pid = if process::id() == u32::MAX {
            process::id() - 1
        } else {
            process::id() + 1
        };
        let old_blob_temp = blob_path.with_file_name(format!(".{target_hash}.tmp.{old_pid}.0"));
        let current_blob_temp =
            blob_path.with_file_name(format!(".{target_hash}.tmp.{}.0", process::id()));
        fs::write(&old_blob_temp, b"stale whole blob temp").unwrap();
        fs::write(&current_blob_temp, b"active whole blob temp").unwrap();

        let manifest_path = store.manifest_path(&target_hash).unwrap();
        fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
        let old_manifest_temp =
            manifest_path.with_file_name(format!(".{target_hash}.json.tmp.{old_pid}.1"));
        fs::write(&old_manifest_temp, b"stale manifest temp").unwrap();

        let report = store.prune_unreferenced(&BTreeSet::new()).unwrap();

        assert_eq!(report.removed_temp_file_paths.len(), 2);
        assert!(
            report
                .removed_temp_file_paths
                .iter()
                .any(|path| { path.ends_with(&format!(".{target_hash}.tmp.{old_pid}.0")) })
        );
        assert!(
            report
                .removed_temp_file_paths
                .iter()
                .any(|path| { path.ends_with(&format!(".{target_hash}.json.tmp.{old_pid}.1")) })
        );
        assert!(!old_blob_temp.exists());
        assert!(!old_manifest_temp.exists());
        assert!(current_blob_temp.exists());
    }

    #[test]
    fn reports_missing_chunks_for_partial_blob() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = describe_blob(b"abcdef", 2);

        store.put_manifest(&descriptor).unwrap();
        store
            .put_chunk_with_hash(&descriptor.chunk_hashes[0], b"ab")
            .unwrap();

        assert_eq!(
            store.missing_chunks(&descriptor).unwrap(),
            descriptor.chunk_hashes[1..].to_vec()
        );
        assert!(!store.has_complete_blob(&descriptor.hash).unwrap());
        assert_eq!(store.get_bytes_chunked(&descriptor.hash).unwrap(), None);
        assert_eq!(store.get_complete_bytes(&descriptor.hash).unwrap(), None);
    }

    #[test]
    fn reports_blob_availability_for_partial_manifest() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = BlobStore::open(tempdir.path()).unwrap();
        let descriptor = describe_blob(b"abcdef", 2);

        store.put_manifest(&descriptor).unwrap();
        store
            .put_chunk_with_hash(&descriptor.chunk_hashes[0], b"ab")
            .unwrap();

        let availability = store.availability(&descriptor.hash).unwrap().unwrap();

        assert!(!availability.is_complete());
        assert_eq!(
            availability.available_chunk_hashes,
            vec![descriptor.chunk_hashes[0].clone()]
        );
        assert_eq!(
            availability.missing_chunk_hashes,
            descriptor.chunk_hashes[1..].to_vec()
        );
    }

    #[test]
    fn chunked_availability_requires_exact_chunk_set_for_completion() {
        let descriptor = describe_blob(b"abcdef", 2);
        let complete = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: descriptor.chunk_hashes.clone(),
            missing_chunk_hashes: Vec::new(),
        };

        assert!(complete.is_complete());

        let mut duplicate_available = complete.clone();
        duplicate_available.available_chunk_hashes = vec![
            descriptor.chunk_hashes[0].clone(),
            descriptor.chunk_hashes[0].clone(),
            descriptor.chunk_hashes[1].clone(),
        ];
        assert!(!duplicate_available.is_complete());

        let mut wrong_descriptor = complete.clone();
        wrong_descriptor.descriptor = Some(describe_blob(b"uvwxyz", 2));
        assert!(!wrong_descriptor.is_complete());

        let mut missing_reported = complete;
        missing_reported.missing_chunk_hashes = vec![descriptor.chunk_hashes[2].clone()];
        assert!(!missing_reported.is_complete());

        let omitted_status = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: vec![descriptor.chunk_hashes[0].clone()],
            missing_chunk_hashes: Vec::new(),
        };
        assert!(matches!(
            validate_blob_availability(&omitted_status),
            Err(MediaError::InvalidDescriptor)
        ));

        let whole_blob = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: true,
            descriptor: None,
            available_chunk_hashes: Vec::new(),
            missing_chunk_hashes: Vec::new(),
        };
        assert!(whole_blob.is_complete());

        let mut whole_blob_with_chunk_claim = whole_blob.clone();
        whole_blob_with_chunk_claim.available_chunk_hashes =
            vec![descriptor.chunk_hashes[0].clone()];
        assert!(!whole_blob_with_chunk_claim.is_complete());

        let mut whole_blob_with_wrong_descriptor = whole_blob;
        whole_blob_with_wrong_descriptor.descriptor = Some(describe_blob(b"uvwxyz", 2));
        assert!(!whole_blob_with_wrong_descriptor.is_complete());
    }

    #[test]
    fn validates_repeated_chunk_availability_accounting() {
        let descriptor = describe_blob(b"abab", 2);
        assert_eq!(descriptor.chunk_hashes[0], descriptor.chunk_hashes[1]);
        let available_once = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: vec![descriptor.chunk_hashes[0].clone()],
            missing_chunk_hashes: Vec::new(),
        };
        assert!(matches!(
            validate_blob_availability(&available_once),
            Err(MediaError::InvalidDescriptor)
        ));

        let missing_twice = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: Vec::new(),
            missing_chunk_hashes: descriptor.chunk_hashes.clone(),
        };
        assert!(validate_blob_availability(&missing_twice).is_ok());
        assert!(!missing_twice.is_complete());

        let available_twice = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(descriptor.clone()),
            available_chunk_hashes: descriptor.chunk_hashes.clone(),
            missing_chunk_hashes: Vec::new(),
        };
        assert!(validate_blob_availability(&available_twice).is_ok());
        assert!(available_twice.is_complete());
    }

    #[test]
    fn plans_missing_chunks_from_remote_availability() {
        let local_dir = tempfile::tempdir().unwrap();
        let remote_dir = tempfile::tempdir().unwrap();
        let local = BlobStore::open(local_dir.path()).unwrap();
        let remote = BlobStore::open(remote_dir.path()).unwrap();
        let descriptor = remote.put_bytes_chunked(b"abcdef", 2).unwrap();

        local.put_manifest(&descriptor).unwrap();
        local
            .put_chunk_with_hash(&descriptor.chunk_hashes[0], b"ab")
            .unwrap();

        let needed = plan_missing_chunks(
            &local,
            &remote.availability(&descriptor.hash).unwrap().unwrap(),
        )
        .unwrap();

        assert_eq!(needed, descriptor.chunk_hashes[1..].to_vec());
    }

    #[test]
    fn plans_repeated_missing_chunk_hash_once() {
        let local_dir = tempfile::tempdir().unwrap();
        let remote_dir = tempfile::tempdir().unwrap();
        let local = BlobStore::open(local_dir.path()).unwrap();
        let remote = BlobStore::open(remote_dir.path()).unwrap();
        let descriptor = remote.put_bytes_chunked(b"abab", 2).unwrap();
        assert_eq!(descriptor.chunk_hashes.len(), 2);
        assert_eq!(descriptor.chunk_hashes[0], descriptor.chunk_hashes[1]);

        local.put_manifest(&descriptor).unwrap();

        let availability = remote.availability(&descriptor.hash).unwrap().unwrap();
        assert_eq!(availability.available_chunk_hashes, descriptor.chunk_hashes);

        let needed = plan_missing_chunks(&local, &availability).unwrap();

        assert_eq!(needed, vec![descriptor.chunk_hashes[0].clone()]);
    }

    #[test]
    fn rejects_missing_chunk_plan_when_availability_descriptor_hash_mismatches() {
        let local_dir = tempfile::tempdir().unwrap();
        let local = BlobStore::open(local_dir.path()).unwrap();
        let descriptor = describe_blob(b"abcdef", 2);
        let wrong_descriptor = describe_blob(b"uvwxyz", 2);
        let availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: Some(wrong_descriptor),
            available_chunk_hashes: descriptor.chunk_hashes.clone(),
            missing_chunk_hashes: Vec::new(),
        };

        assert!(matches!(
            plan_missing_chunks(&local, &availability),
            Err(MediaError::InvalidDescriptor)
        ));
    }

    #[test]
    fn rejects_missing_chunk_plan_when_descriptorless_availability_lists_chunks() {
        let local_dir = tempfile::tempdir().unwrap();
        let local = BlobStore::open(local_dir.path()).unwrap();
        let descriptor = describe_blob(b"abcdef", 2);
        let availability = BlobAvailability {
            hash: descriptor.hash.clone(),
            has_whole_blob: false,
            descriptor: None,
            available_chunk_hashes: vec![descriptor.chunk_hashes[0].clone()],
            missing_chunk_hashes: Vec::new(),
        };

        assert!(matches!(
            plan_missing_chunks(&local, &availability),
            Err(MediaError::InvalidDescriptor)
        ));
    }
}
