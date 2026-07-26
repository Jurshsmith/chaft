use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::RuntimeError;

pub(crate) const RUNTIME_PATH_MAX_BYTES: usize = 64 * 1024;
pub(crate) const RUNTIME_PASSPHRASE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePaths {
    pub data_dir: PathBuf,
    pub identity_file: PathBuf,
    pub event_store: PathBuf,
    pub search_index: PathBuf,
    pub blob_store: PathBuf,
    pub workspace_keys_dir: PathBuf,
    pub blob_transfer_ledger: PathBuf,
    pub compromise_response_ledger: PathBuf,
}

impl RuntimePaths {
    pub fn new(data_dir: impl AsRef<Path>, identity_file: Option<PathBuf>) -> Self {
        let data_dir = data_dir.as_ref().to_path_buf();
        Self {
            identity_file: identity_file.unwrap_or_else(|| data_dir.join("device.json")),
            event_store: data_dir.join("events.db"),
            search_index: data_dir.join("search.db"),
            blob_store: data_dir.join("blobs"),
            workspace_keys_dir: data_dir.join("keys"),
            blob_transfer_ledger: data_dir.join("blob-transfer-ledger.json"),
            compromise_response_ledger: data_dir.join("compromise-response-ledger.json"),
            data_dir,
        }
    }
}

pub(crate) fn validate_runtime_paths(paths: &RuntimePaths) -> Result<(), RuntimeError> {
    validate_runtime_path(&paths.data_dir, "data directory")?;
    validate_runtime_path(&paths.identity_file, "identity file")?;
    validate_runtime_path(&paths.event_store, "event store path")?;
    validate_runtime_path(&paths.search_index, "search index path")?;
    validate_runtime_path(&paths.blob_store, "blob store path")?;
    validate_runtime_path(&paths.workspace_keys_dir, "workspace keys path")?;
    validate_runtime_path(&paths.blob_transfer_ledger, "blob transfer ledger path")?;
    validate_runtime_path(
        &paths.compromise_response_ledger,
        "compromise response ledger path",
    )?;
    Ok(())
}

pub(crate) fn validate_runtime_path(path: &Path, field: &'static str) -> Result<(), RuntimeError> {
    let actual_bytes = path.as_os_str().as_encoded_bytes().len();
    if actual_bytes == 0 {
        return Err(RuntimeError::MetadataFieldRequired { field });
    }
    if actual_bytes > RUNTIME_PATH_MAX_BYTES {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes,
            max_bytes: RUNTIME_PATH_MAX_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn normalize_runtime_identity_passphrase(
    passphrase: Option<&str>,
) -> Result<Option<Zeroizing<String>>, RuntimeError> {
    match passphrase {
        Some(passphrase) if passphrase.len() > RUNTIME_PASSPHRASE_MAX_BYTES => {
            Err(RuntimeError::MetadataFieldTooLarge {
                field: "identity passphrase",
                actual_bytes: passphrase.len(),
                max_bytes: RUNTIME_PASSPHRASE_MAX_BYTES,
            })
        }
        Some(passphrase) if passphrase.trim().is_empty() => Ok(None),
        Some(passphrase) => Ok(Some(Zeroizing::new(passphrase.to_owned()))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use super::{RUNTIME_PASSPHRASE_MAX_BYTES, normalize_runtime_identity_passphrase};
    use crate::RuntimeError;

    fn assert_zeroizing_string(_: &Zeroizing<String>) {}

    #[test]
    fn identity_passphrase_normalization_zeroizes_owned_copy_and_preserves_whitespace() {
        let normalized =
            normalize_runtime_identity_passphrase(Some("  significant passphrase \t")).unwrap();
        let normalized = normalized.as_ref().expect("passphrase should be retained");

        assert_zeroizing_string(normalized);
        assert_eq!(normalized.as_str(), "  significant passphrase \t");
    }

    #[test]
    fn identity_passphrase_normalization_preserves_blank_semantics() {
        assert!(
            normalize_runtime_identity_passphrase(None)
                .unwrap()
                .is_none()
        );
        assert!(
            normalize_runtime_identity_passphrase(Some(" \t\r\n "))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn identity_passphrase_normalization_rejects_oversized_input_without_echoing_it() {
        let passphrase = "p".repeat(RUNTIME_PASSPHRASE_MAX_BYTES + 1);
        let error = normalize_runtime_identity_passphrase(Some(&passphrase)).unwrap_err();
        let rendered = error.to_string();

        assert!(matches!(
            error,
            RuntimeError::MetadataFieldTooLarge {
                field: "identity passphrase",
                actual_bytes,
                max_bytes: RUNTIME_PASSPHRASE_MAX_BYTES,
            } if actual_bytes == RUNTIME_PASSPHRASE_MAX_BYTES + 1
        ));
        assert!(!rendered.contains(&passphrase));
    }
}
