#[cfg(test)]
use std::{fs, path::Path};

use chaft_crypto::ContentKey;
use chaft_types::{ChannelId, ContentKeyScope, EventBody, SignableEvent, WorkspaceId};
use serde::{Deserialize, Serialize};

use crate::{CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV, LocalRuntime, RuntimeError};

pub(crate) const WORKSPACE_KEY_LEN: usize = 32;
pub(crate) const CONTENT_KEY_EXPORT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceKeyExport {
    pub schema_version: u32,
    pub workspace_id: String,
    #[serde(default = "default_content_key_epoch")]
    pub epoch: u64,
    pub key_id: String,
    pub exporter_device_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    pub previous_keys: Vec<ExportedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspaceKey {
    pub workspace_id: String,
    pub key_id: String,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceKey {
    pub workspace_id: String,
    pub previous_key_id: String,
    pub key_id: String,
    pub epoch: u64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelKeyExport {
    pub schema_version: u32,
    pub workspace_id: String,
    pub channel_id: String,
    #[serde(default = "default_content_key_epoch")]
    pub epoch: u64,
    pub key_id: String,
    pub exporter_device_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    pub previous_keys: Vec<ExportedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedChannelKey {
    pub workspace_id: String,
    pub channel_id: String,
    pub key_id: String,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedChannelKey {
    pub workspace_id: String,
    pub channel_id: String,
    pub previous_key_id: String,
    pub key_id: String,
    pub epoch: u64,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedWorkspaceManualKeys {
    pub workspace_id: String,
    pub workspace_key_rotation: RotatedWorkspaceKey,
    #[serde(default)]
    pub channel_key_rotation_count: usize,
    pub channel_key_rotations: Vec<RotatedChannelKey>,
    #[serde(default)]
    pub rotated_event_count: usize,
    pub rotated_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedContentKeyMaterial {
    pub key_id: String,
    pub aes_256_gcm_siv_key: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct ResolvedContentKey {
    key_id: String,
    content_key: ContentKey,
}

impl ResolvedContentKey {
    pub(crate) fn new(key_id: String, content_key: ContentKey) -> Self {
        Self {
            key_id,
            content_key,
        }
    }

    pub(crate) fn key_id(&self) -> &str {
        &self.key_id
    }

    pub(crate) fn content_key(&self) -> &ContentKey {
        &self.content_key
    }
}

impl From<WorkspaceKey> for ResolvedContentKey {
    fn from(key: WorkspaceKey) -> Self {
        Self {
            key_id: key.key_id,
            content_key: key.content_key,
        }
    }
}

impl From<ChannelKey> for ResolvedContentKey {
    fn from(key: ChannelKey) -> Self {
        Self {
            key_id: key.key_id,
            content_key: key.content_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedWorkspaceKey {
    schema_version: u32,
    workspace_id: WorkspaceId,
    #[serde(default = "default_content_key_epoch")]
    epoch: u64,
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    previous_keys: Vec<PersistedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PersistedChannelKey {
    schema_version: u32,
    workspace_id: WorkspaceId,
    channel_id: ChannelId,
    #[serde(default = "default_content_key_epoch")]
    epoch: u64,
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
    #[serde(default)]
    previous_keys: Vec<PersistedContentKeyMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PersistedContentKeyMaterial {
    key_id: String,
    aes_256_gcm_siv_key: Vec<u8>,
}

#[derive(Clone)]
struct ContentKeyMaterial {
    key_id: String,
    content_key: ContentKey,
}

impl ContentKeyMaterial {
    fn exported(&self) -> ExportedContentKeyMaterial {
        ExportedContentKeyMaterial {
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
        }
    }

    fn persisted(&self) -> PersistedContentKeyMaterial {
        PersistedContentKeyMaterial {
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
        }
    }

    fn resolved(&self) -> ResolvedContentKey {
        ResolvedContentKey {
            key_id: self.key_id.clone(),
            content_key: self.content_key.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct WorkspaceKey {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) epoch: u64,
    pub(crate) key_id: String,
    pub(crate) content_key: ContentKey,
    previous_keys: Vec<ContentKeyMaterial>,
}

impl WorkspaceKey {
    pub(crate) fn generate(workspace_id: WorkspaceId) -> Self {
        let epoch = 1;
        let key_id = Self::key_id_for_epoch(&workspace_id, epoch);
        Self {
            workspace_id,
            epoch,
            key_id,
            content_key: ContentKey::generate(),
            previous_keys: Vec::new(),
        }
    }

    fn key_id_for_epoch(workspace_id: &WorkspaceId, epoch: u64) -> String {
        format!("{}:content:v{}", workspace_id.0, epoch)
    }

    #[cfg(test)]
    pub(crate) fn key_id(&self) -> &str {
        &self.key_id
    }

    #[cfg(test)]
    pub(crate) fn content_key(&self) -> &ContentKey {
        &self.content_key
    }

    #[cfg(test)]
    pub(crate) fn load(path: &Path) -> Result<Self, RuntimeError> {
        Self::from_bytes(&fs::read(path)?)
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let persisted: PersistedWorkspaceKey = serde_json::from_slice(bytes)?;
        if !content_key_schema_supported(persisted.schema_version) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let epoch = persisted.epoch.max(1);
        if persisted.key_id != Self::key_id_for_epoch(&persisted.workspace_id, epoch) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let content_key =
            decode_workspace_key_material(persisted.key_id.clone(), persisted.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = persisted
            .previous_keys
            .into_iter()
            .map(|key| decode_workspace_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id: persisted.workspace_id,
            epoch,
            key_id: persisted.key_id,
            content_key,
            previous_keys,
        })
    }

    pub(crate) fn from_export(exported: WorkspaceKeyExport) -> Result<Self, RuntimeError> {
        if !content_key_schema_supported(exported.schema_version) {
            return Err(RuntimeError::UnsupportedWorkspaceKeyExport);
        }
        let workspace_id = WorkspaceId(exported.workspace_id);
        let epoch = exported.epoch.max(1);
        if exported.key_id != Self::key_id_for_epoch(&workspace_id, epoch) {
            return Err(RuntimeError::InvalidWorkspaceKey);
        }
        let content_key =
            decode_workspace_key_material(exported.key_id.clone(), exported.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = exported
            .previous_keys
            .into_iter()
            .map(|key| decode_workspace_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id,
            epoch,
            key_id: exported.key_id,
            content_key,
            previous_keys,
        })
    }

    pub(crate) fn rotate(&mut self) {
        let next_epoch = self.epoch + 1;
        let next_key_id = Self::key_id_for_epoch(&self.workspace_id, next_epoch);
        let previous_key_id = std::mem::replace(&mut self.key_id, next_key_id);
        let previous_content_key = std::mem::replace(&mut self.content_key, ContentKey::generate());
        self.previous_keys.push(ContentKeyMaterial {
            key_id: previous_key_id,
            content_key: previous_content_key,
        });
        self.epoch = next_epoch;
    }

    pub(crate) fn resolve_content_key(&self, key_id: &str) -> Option<ResolvedContentKey> {
        if self.key_id == key_id {
            return Some(ResolvedContentKey {
                key_id: self.key_id.clone(),
                content_key: self.content_key.clone(),
            });
        }
        self.previous_keys
            .iter()
            .find(|key| key.key_id == key_id)
            .map(ContentKeyMaterial::resolved)
    }

    pub(crate) fn exported_previous_keys(&self) -> Vec<ExportedContentKeyMaterial> {
        self.previous_keys
            .iter()
            .map(ContentKeyMaterial::exported)
            .collect()
    }

    pub(crate) fn persisted(&self) -> PersistedWorkspaceKey {
        PersistedWorkspaceKey {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: self.workspace_id.clone(),
            epoch: self.epoch,
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
            previous_keys: self
                .previous_keys
                .iter()
                .map(ContentKeyMaterial::persisted)
                .collect(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ChannelKey {
    pub(crate) workspace_id: WorkspaceId,
    pub(crate) channel_id: ChannelId,
    pub(crate) epoch: u64,
    pub(crate) key_id: String,
    pub(crate) content_key: ContentKey,
    previous_keys: Vec<ContentKeyMaterial>,
}

impl ChannelKey {
    pub(crate) fn generate(workspace_id: WorkspaceId, channel_id: ChannelId) -> Self {
        let epoch = 1;
        let key_id = Self::key_id_for_epoch(&workspace_id, &channel_id, epoch);
        Self {
            workspace_id,
            channel_id,
            epoch,
            key_id,
            content_key: ContentKey::generate(),
            previous_keys: Vec::new(),
        }
    }

    fn key_id_for_epoch(workspace_id: &WorkspaceId, channel_id: &ChannelId, epoch: u64) -> String {
        format!("{}:{}:content:v{}", workspace_id.0, channel_id.0, epoch)
    }

    #[cfg(test)]
    pub(crate) fn load(path: &Path) -> Result<Self, RuntimeError> {
        Self::from_bytes(&fs::read(path)?)
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, RuntimeError> {
        let persisted: PersistedChannelKey = serde_json::from_slice(bytes)?;
        if !content_key_schema_supported(persisted.schema_version) {
            return Err(RuntimeError::InvalidChannelKey);
        }
        let epoch = persisted.epoch.max(1);
        let expected_key_id =
            Self::key_id_for_epoch(&persisted.workspace_id, &persisted.channel_id, epoch);
        if persisted.key_id != expected_key_id {
            return Err(RuntimeError::InvalidChannelKey);
        }

        let content_key =
            decode_channel_key_material(persisted.key_id.clone(), persisted.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = persisted
            .previous_keys
            .into_iter()
            .map(|key| decode_channel_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id: persisted.workspace_id,
            channel_id: persisted.channel_id,
            epoch,
            key_id: persisted.key_id,
            content_key,
            previous_keys,
        })
    }

    pub(crate) fn from_export(exported: ChannelKeyExport) -> Result<Self, RuntimeError> {
        if !content_key_schema_supported(exported.schema_version) {
            return Err(RuntimeError::UnsupportedChannelKeyExport);
        }

        let workspace_id = WorkspaceId(exported.workspace_id);
        let channel_id = ChannelId(exported.channel_id);
        let epoch = exported.epoch.max(1);
        if exported.key_id != Self::key_id_for_epoch(&workspace_id, &channel_id, epoch) {
            return Err(RuntimeError::InvalidChannelKey);
        }
        let content_key =
            decode_channel_key_material(exported.key_id.clone(), exported.aes_256_gcm_siv_key)?
                .content_key;
        let previous_keys = exported
            .previous_keys
            .into_iter()
            .map(|key| decode_channel_key_material(key.key_id, key.aes_256_gcm_siv_key))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            workspace_id,
            channel_id,
            epoch,
            key_id: exported.key_id,
            content_key,
            previous_keys,
        })
    }

    pub(crate) fn rotate(&mut self) {
        let next_epoch = self.epoch + 1;
        let next_key_id = Self::key_id_for_epoch(&self.workspace_id, &self.channel_id, next_epoch);
        let previous_key_id = std::mem::replace(&mut self.key_id, next_key_id);
        let previous_content_key = std::mem::replace(&mut self.content_key, ContentKey::generate());
        self.previous_keys.push(ContentKeyMaterial {
            key_id: previous_key_id,
            content_key: previous_content_key,
        });
        self.epoch = next_epoch;
    }

    pub(crate) fn resolve_content_key(&self, key_id: &str) -> Option<ResolvedContentKey> {
        if self.key_id == key_id {
            return Some(ResolvedContentKey {
                key_id: self.key_id.clone(),
                content_key: self.content_key.clone(),
            });
        }
        self.previous_keys
            .iter()
            .find(|key| key.key_id == key_id)
            .map(ContentKeyMaterial::resolved)
    }

    pub(crate) fn exported_previous_keys(&self) -> Vec<ExportedContentKeyMaterial> {
        self.previous_keys
            .iter()
            .map(ContentKeyMaterial::exported)
            .collect()
    }

    pub(crate) fn persisted(&self) -> PersistedChannelKey {
        PersistedChannelKey {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: self.workspace_id.clone(),
            channel_id: self.channel_id.clone(),
            epoch: self.epoch,
            key_id: self.key_id.clone(),
            aes_256_gcm_siv_key: self.content_key.as_bytes().to_vec(),
            previous_keys: self
                .previous_keys
                .iter()
                .map(ContentKeyMaterial::persisted)
                .collect(),
        }
    }
}

pub(crate) fn content_key_from_mls_export(raw_key: Vec<u8>) -> Result<ContentKey, RuntimeError> {
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidWorkspaceKey)?;
    Ok(ContentKey::from_bytes(bytes))
}

fn content_key_schema_supported(schema_version: u32) -> bool {
    schema_version == 1 || schema_version == CONTENT_KEY_EXPORT_SCHEMA_VERSION
}

fn decode_workspace_key_material(
    key_id: String,
    raw_key: Vec<u8>,
) -> Result<ContentKeyMaterial, RuntimeError> {
    if raw_key.len() != WORKSPACE_KEY_LEN {
        return Err(RuntimeError::InvalidWorkspaceKey);
    }
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidWorkspaceKey)?;
    Ok(ContentKeyMaterial {
        key_id,
        content_key: ContentKey::from_bytes(bytes),
    })
}

fn decode_channel_key_material(
    key_id: String,
    raw_key: Vec<u8>,
) -> Result<ContentKeyMaterial, RuntimeError> {
    if raw_key.len() != WORKSPACE_KEY_LEN {
        return Err(RuntimeError::InvalidChannelKey);
    }
    let bytes = raw_key
        .try_into()
        .map_err(|_| RuntimeError::InvalidChannelKey)?;
    Ok(ContentKeyMaterial {
        key_id,
        content_key: ContentKey::from_bytes(bytes),
    })
}

fn default_content_key_epoch() -> u64 {
    1
}

impl LocalRuntime {
    pub fn rotate_workspace_key(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceKey, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let mut workspace_key = self
            .load_workspace_key(&workspace_id)?
            .ok_or(RuntimeError::InvalidWorkspaceKey)?;
        let previous_key_id = workspace_key.key_id.clone();
        workspace_key.rotate();

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            None,
            self.identity.device_id().clone(),
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Workspace,
                epoch: workspace_key.epoch,
                key_id: workspace_key.key_id.clone(),
                previous_key_id: Some(previous_key_id.clone()),
                algorithm: CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV.to_owned(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| runtime.save_workspace_key(&workspace_key),
        )?;

        Ok(RotatedWorkspaceKey {
            workspace_id: workspace_id.0,
            previous_key_id,
            key_id: workspace_key.key_id,
            epoch: workspace_key.epoch,
            event_id: event.event_id.0,
        })
    }

    pub fn rotate_channel_key(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<RotatedChannelKey, RuntimeError> {
        let context = self.materialized_workspace_write_context(&workspace_id)?;
        let mut channel_key = self
            .load_channel_key(&workspace_id, &channel_id)?
            .ok_or_else(|| RuntimeError::ChannelKeyMissing {
                workspace_id: workspace_id.clone(),
                channel_id: channel_id.clone(),
            })?;
        let previous_key_id = channel_key.key_id.clone();
        channel_key.rotate();

        let mut event = SignableEvent::new(
            workspace_id.clone(),
            Some(channel_id.clone()),
            self.identity.device_id().clone(),
            EventBody::ContentKeyEpochPublished {
                scope: ContentKeyScope::Channel {
                    channel_id: channel_id.clone(),
                },
                epoch: channel_key.epoch,
                key_id: channel_key.key_id.clone(),
                previous_key_id: Some(previous_key_id.clone()),
                algorithm: CONTENT_KEY_ALGORITHM_AES_256_GCM_SIV.to_owned(),
            },
        );
        event.parents = context.head_event_ids.clone();
        let event = self.sign_authorize_save_key_and_append_with_history(
            event,
            &context.events,
            |runtime| runtime.save_channel_key(&channel_key),
        )?;

        Ok(RotatedChannelKey {
            workspace_id: workspace_id.0,
            channel_id: channel_id.0,
            previous_key_id,
            key_id: channel_key.key_id,
            epoch: channel_key.epoch,
            event_id: event.event_id.0,
        })
    }

    pub fn rotate_workspace_manual_keys(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<RotatedWorkspaceManualKeys, RuntimeError> {
        let workspace_key_rotation = self.rotate_workspace_key(workspace_id.clone())?;
        let mut channel_key_rotations = Vec::new();
        for channel_id in self.local_private_channel_key_ids(&workspace_id)? {
            channel_key_rotations.push(self.rotate_channel_key(workspace_id.clone(), channel_id)?);
        }
        let mut rotated_event_ids = Vec::with_capacity(1 + channel_key_rotations.len());
        rotated_event_ids.push(workspace_key_rotation.event_id.clone());
        rotated_event_ids.extend(
            channel_key_rotations
                .iter()
                .map(|rotation| rotation.event_id.clone()),
        );

        Ok(RotatedWorkspaceManualKeys {
            workspace_id: workspace_id.0,
            workspace_key_rotation,
            channel_key_rotation_count: channel_key_rotations.len(),
            channel_key_rotations,
            rotated_event_count: rotated_event_ids.len(),
            rotated_event_ids,
        })
    }

    pub fn export_workspace_key(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceKeyExport, RuntimeError> {
        let workspace_key = self
            .load_workspace_key(&workspace_id)?
            .ok_or(RuntimeError::InvalidWorkspaceKey)?;
        Ok(WorkspaceKeyExport {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: workspace_key.workspace_id.0.clone(),
            epoch: workspace_key.epoch,
            key_id: workspace_key.key_id.clone(),
            exporter_device_id: self.identity.device_id().0.clone(),
            aes_256_gcm_siv_key: workspace_key.content_key.as_bytes().to_vec(),
            previous_keys: workspace_key.exported_previous_keys(),
        })
    }

    pub fn import_workspace_key(
        &self,
        exported: WorkspaceKeyExport,
    ) -> Result<ImportedWorkspaceKey, RuntimeError> {
        let workspace_key = WorkspaceKey::from_export(exported)?;
        let imported = ImportedWorkspaceKey {
            workspace_id: workspace_key.workspace_id.0.clone(),
            key_id: workspace_key.key_id.clone(),
            importer_device_id: self.identity.device_id().0.clone(),
        };
        self.save_workspace_key(&workspace_key)?;
        let _ = self.reindex_workspace_search(workspace_key.workspace_id.clone());
        Ok(imported)
    }

    pub fn export_channel_key(
        &self,
        workspace_id: WorkspaceId,
        channel_id: ChannelId,
    ) -> Result<ChannelKeyExport, RuntimeError> {
        let channel_key = self
            .load_channel_key(&workspace_id, &channel_id)?
            .ok_or(RuntimeError::InvalidChannelKey)?;
        Ok(ChannelKeyExport {
            schema_version: CONTENT_KEY_EXPORT_SCHEMA_VERSION,
            workspace_id: channel_key.workspace_id.0.clone(),
            channel_id: channel_key.channel_id.0.clone(),
            epoch: channel_key.epoch,
            key_id: channel_key.key_id.clone(),
            exporter_device_id: self.identity.device_id().0.clone(),
            aes_256_gcm_siv_key: channel_key.content_key.as_bytes().to_vec(),
            previous_keys: channel_key.exported_previous_keys(),
        })
    }

    pub fn import_channel_key(
        &self,
        exported: ChannelKeyExport,
    ) -> Result<ImportedChannelKey, RuntimeError> {
        let channel_key = ChannelKey::from_export(exported)?;
        let imported = ImportedChannelKey {
            workspace_id: channel_key.workspace_id.0.clone(),
            channel_id: channel_key.channel_id.0.clone(),
            key_id: channel_key.key_id.clone(),
            importer_device_id: self.identity.device_id().0.clone(),
        };
        self.save_channel_key(&channel_key)?;
        let _ = self.reindex_workspace_search(channel_key.workspace_id.clone());
        Ok(imported)
    }
}
