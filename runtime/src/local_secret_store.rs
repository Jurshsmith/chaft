use std::path::{Path, PathBuf};

use chaft_types::{ChannelId, WorkspaceId};
use serde::Serialize;

use crate::{
    ChannelKey, LOCAL_SECRET_FILE_MAX_BYTES, LOCAL_SECRET_KIND_CHANNEL_KEY,
    LOCAL_SECRET_KIND_WORKSPACE_KEY, LocalRuntime, RuntimeError, WorkspaceKey,
    encrypt_local_secret, open_serialized_local_secret, read_local_metadata_file_with_limit,
    write_secret_file,
};

impl LocalRuntime {
    pub(crate) fn workspace_key_path(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(format!("{}.json", workspace_id.0))
    }

    pub(crate) fn channel_key_path(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("channels")
            .join(format!("{}.json", channel_id.0))
    }

    pub(crate) fn openmls_key_package_path(
        &self,
        workspace_id: &WorkspaceId,
        key_package_ref: &str,
    ) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-key-packages")
            .join(format!("{key_package_ref}.json"))
    }

    pub(crate) fn openmls_workspace_group_path(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-groups")
            .join("workspace.json")
    }

    pub(crate) fn openmls_channel_groups_dir(&self, workspace_id: &WorkspaceId) -> PathBuf {
        self.paths
            .workspace_keys_dir
            .clone()
            .join(&workspace_id.0)
            .join("mls-groups")
            .join("channels")
    }

    pub(crate) fn openmls_channel_group_path(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> PathBuf {
        self.openmls_channel_groups_dir(workspace_id)
            .join(format!("{}.json", channel_id.0))
    }

    fn local_secret_path_hint(&self, path: &Path) -> String {
        path.strip_prefix(&self.paths.data_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }

    pub(crate) fn read_local_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Vec<u8>, RuntimeError> {
        let Some(bytes) = read_local_metadata_file_with_limit(
            path,
            LOCAL_SECRET_FILE_MAX_BYTES,
            "local secret file",
        )?
        else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("local secret file not found: {}", path.display()),
            )
            .into());
        };
        let path_hint = self.local_secret_path_hint(path);
        if let Some(plaintext) = open_serialized_local_secret(
            &bytes,
            secret_kind,
            &path_hint,
            self.identity_passphrase
                .as_ref()
                .map(|passphrase| passphrase.as_str()),
        )? {
            return Ok(plaintext);
        }
        Ok(bytes)
    }

    pub(crate) fn write_local_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
        plaintext: &[u8],
    ) -> Result<(), RuntimeError> {
        let bytes = match self.identity_passphrase.as_deref() {
            Some(passphrase) => {
                let path_hint = self.local_secret_path_hint(path);
                encrypt_local_secret(secret_kind, &path_hint, passphrase, plaintext)?
            }
            None => plaintext.to_vec(),
        };
        write_secret_file(path, &bytes)
    }

    pub(crate) fn load_workspace_key(
        &self,
        workspace_id: &WorkspaceId,
    ) -> Result<Option<WorkspaceKey>, RuntimeError> {
        let path = self.workspace_key_path(workspace_id);
        match self.read_local_secret_file(&path, LOCAL_SECRET_KIND_WORKSPACE_KEY) {
            Ok(bytes) => Ok(Some(WorkspaceKey::from_bytes(&bytes)?)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn save_workspace_key(&self, key: &WorkspaceKey) -> Result<(), RuntimeError> {
        let path = self.workspace_key_path(&key.workspace_id);
        self.write_key_file(&path, LOCAL_SECRET_KIND_WORKSPACE_KEY, &key.persisted())
    }

    pub(crate) fn load_channel_key(
        &self,
        workspace_id: &WorkspaceId,
        channel_id: &ChannelId,
    ) -> Result<Option<ChannelKey>, RuntimeError> {
        let path = self.channel_key_path(workspace_id, channel_id);
        match self.read_local_secret_file(&path, LOCAL_SECRET_KIND_CHANNEL_KEY) {
            Ok(bytes) => Ok(Some(ChannelKey::from_bytes(&bytes)?)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn save_channel_key(&self, key: &ChannelKey) -> Result<(), RuntimeError> {
        let path = self.channel_key_path(&key.workspace_id, &key.channel_id);
        self.write_key_file(&path, LOCAL_SECRET_KIND_CHANNEL_KEY, &key.persisted())
    }

    pub(crate) fn read_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.read_local_secret_file(path, secret_kind)
    }

    pub(crate) fn read_optional_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
    ) -> Result<Option<Vec<u8>>, RuntimeError> {
        match self.read_openmls_secret_file(path, secret_kind) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(RuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn write_openmls_secret_file(
        &self,
        path: &Path,
        secret_kind: &str,
        plaintext: &[u8],
    ) -> Result<(), RuntimeError> {
        self.write_local_secret_file(path, secret_kind, plaintext)
    }

    fn write_key_file<T>(
        &self,
        path: &Path,
        secret_kind: &str,
        persisted: &T,
    ) -> Result<(), RuntimeError>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec_pretty(persisted)?;
        self.write_local_secret_file(path, secret_kind, &bytes)
    }
}
