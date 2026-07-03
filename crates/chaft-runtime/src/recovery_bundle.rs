use argon2::{Algorithm, Argon2, Params, Version};
use chaft_crypto::{ContentKey, SealedPayload, open_aes_256_gcm_siv, seal_aes_256_gcm_siv};
use chaft_types::{DeviceId, WorkspaceId};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::{
    ChannelKey, ChannelKeyExport, LocalRuntime, RuntimeError, WORKSPACE_KEY_LEN, WorkspaceKey,
    WorkspaceKeyExport,
};

pub(crate) const RECOVERY_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub(crate) const RECOVERY_BUNDLE_SALT_LEN: usize = 16;
pub(crate) const RECOVERY_BUNDLE_KDF_ARGON2ID: &str = "argon2id";
pub(crate) const RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY: &str = "blake3-derive-key";
pub(crate) const RECOVERY_BUNDLE_KDF_CONTEXT: &str = "Chaft workspace recovery bundle v1";
pub(crate) const RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB: u32 = 64 * 1024;
pub(crate) const RECOVERY_BUNDLE_ARGON2_TIME_COST: u32 = 3;
pub(crate) const RECOVERY_BUNDLE_ARGON2_PARALLELISM: u32 = 1;
pub(crate) const RECOVERY_BUNDLE_KDF_OUTPUT_LEN: u32 = WORKSPACE_KEY_LEN as u32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryBundleKdf {
    pub name: String,
    pub context: String,
    pub salt: Vec<u8>,
    #[serde(default)]
    pub memory_cost_kib: u32,
    #[serde(default)]
    pub time_cost: u32,
    #[serde(default)]
    pub parallelism: u32,
    #[serde(default)]
    pub output_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecoveryBundle {
    pub schema_version: u32,
    pub workspace_id: String,
    pub exporter_device_id: String,
    pub kdf: WorkspaceRecoveryBundleKdf,
    pub sealed_payload: SealedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspaceRecoveryBundle {
    pub workspace_id: String,
    pub workspace_key_id: String,
    #[serde(default)]
    pub imported_channel_count: usize,
    pub imported_channel_ids: Vec<String>,
    pub importer_device_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceRecoveryBundlePlaintext {
    pub(crate) schema_version: u32,
    pub(crate) workspace_key: WorkspaceKeyExport,
    #[serde(default)]
    pub(crate) channel_keys: Vec<ChannelKeyExport>,
}

impl LocalRuntime {
    pub fn export_workspace_recovery_bundle(
        &self,
        workspace_id: WorkspaceId,
        passphrase: &str,
    ) -> Result<WorkspaceRecoveryBundle, RuntimeError> {
        if passphrase.trim().is_empty() {
            return Err(RuntimeError::RecoveryBundlePassphraseRequired);
        }

        let workspace_key = self.export_workspace_key(workspace_id.clone())?;
        let channel_keys = self
            .local_private_channel_key_ids(&workspace_id)?
            .into_iter()
            .map(|channel_id| self.export_channel_key(workspace_id.clone(), channel_id))
            .collect::<Result<Vec<_>, _>>()?;
        let plaintext = WorkspaceRecoveryBundlePlaintext {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_key,
            channel_keys,
        };
        let plaintext = serde_json::to_vec(&plaintext)?;
        let mut salt = vec![0; RECOVERY_BUNDLE_SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let kdf = WorkspaceRecoveryBundleKdf {
            name: RECOVERY_BUNDLE_KDF_ARGON2ID.to_owned(),
            context: RECOVERY_BUNDLE_KDF_CONTEXT.to_owned(),
            salt,
            memory_cost_kib: RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB,
            time_cost: RECOVERY_BUNDLE_ARGON2_TIME_COST,
            parallelism: RECOVERY_BUNDLE_ARGON2_PARALLELISM,
            output_len: RECOVERY_BUNDLE_KDF_OUTPUT_LEN,
        };
        let wrapping_key = derive_recovery_bundle_key(passphrase, &kdf)?;
        let sealed_payload = seal_aes_256_gcm_siv(
            recovery_bundle_key_id(&workspace_id),
            &wrapping_key,
            &plaintext,
            &recovery_bundle_aad(
                &workspace_id,
                self.identity.device_id(),
                kdf.name.as_str(),
                kdf.context.as_str(),
                &kdf.salt,
            ),
        )?;

        Ok(WorkspaceRecoveryBundle {
            schema_version: RECOVERY_BUNDLE_SCHEMA_VERSION,
            workspace_id: workspace_id.0,
            exporter_device_id: self.identity.device_id().0.clone(),
            kdf,
            sealed_payload,
        })
    }

    pub fn import_workspace_recovery_bundle(
        &self,
        bundle: WorkspaceRecoveryBundle,
        passphrase: &str,
    ) -> Result<ImportedWorkspaceRecoveryBundle, RuntimeError> {
        if passphrase.trim().is_empty() {
            return Err(RuntimeError::RecoveryBundlePassphraseRequired);
        }
        if bundle.schema_version != RECOVERY_BUNDLE_SCHEMA_VERSION {
            return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
        }

        let workspace_id = WorkspaceId(bundle.workspace_id.clone());
        let exporter_device_id = DeviceId(bundle.exporter_device_id.clone());
        let wrapping_key = derive_recovery_bundle_key(passphrase, &bundle.kdf)?;
        let aad = recovery_bundle_aad(
            &workspace_id,
            &exporter_device_id,
            bundle.kdf.name.as_str(),
            bundle.kdf.context.as_str(),
            &bundle.kdf.salt,
        );
        if bundle.sealed_payload.aad != aad {
            return Err(RuntimeError::InvalidWorkspaceRecoveryBundle);
        }
        let plaintext = open_aes_256_gcm_siv(&wrapping_key, &bundle.sealed_payload)?;
        let plaintext = serde_json::from_slice::<WorkspaceRecoveryBundlePlaintext>(&plaintext)?;
        if plaintext.schema_version != RECOVERY_BUNDLE_SCHEMA_VERSION
            || plaintext.workspace_key.workspace_id != bundle.workspace_id
            || plaintext
                .channel_keys
                .iter()
                .any(|channel_key| channel_key.workspace_id != bundle.workspace_id)
        {
            return Err(RuntimeError::InvalidWorkspaceRecoveryBundle);
        }

        let workspace_key = WorkspaceKey::from_export(plaintext.workspace_key)?;
        let channel_keys = plaintext
            .channel_keys
            .into_iter()
            .map(ChannelKey::from_export)
            .collect::<Result<Vec<_>, _>>()?;
        let imported = ImportedWorkspaceRecoveryBundle {
            workspace_id: workspace_key.workspace_id.0.clone(),
            workspace_key_id: workspace_key.key_id.clone(),
            imported_channel_count: channel_keys.len(),
            imported_channel_ids: channel_keys
                .iter()
                .map(|channel_key| channel_key.channel_id.0.clone())
                .collect(),
            importer_device_id: self.identity.device_id().0.clone(),
        };

        self.save_workspace_key(&workspace_key)?;
        for channel_key in channel_keys {
            self.save_channel_key(&channel_key)?;
        }
        let _ = self.reindex_workspace_search(WorkspaceId(imported.workspace_id.clone()));
        Ok(imported)
    }
}

pub(crate) fn derive_recovery_bundle_key(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    match kdf.name.as_str() {
        RECOVERY_BUNDLE_KDF_ARGON2ID => derive_recovery_bundle_key_argon2id(passphrase, kdf),
        RECOVERY_BUNDLE_KDF_BLAKE3_DERIVE_KEY => derive_recovery_bundle_key_blake3(passphrase, kdf),
        _ => Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle),
    }
}

fn derive_recovery_bundle_key_argon2id(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    if kdf.context != RECOVERY_BUNDLE_KDF_CONTEXT
        || kdf.salt.len() != RECOVERY_BUNDLE_SALT_LEN
        || kdf.memory_cost_kib != RECOVERY_BUNDLE_ARGON2_MEMORY_COST_KIB
        || kdf.time_cost != RECOVERY_BUNDLE_ARGON2_TIME_COST
        || kdf.parallelism != RECOVERY_BUNDLE_ARGON2_PARALLELISM
        || kdf.output_len != RECOVERY_BUNDLE_KDF_OUTPUT_LEN
    {
        return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
    }

    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(WORKSPACE_KEY_LEN),
    )
    .map_err(|error| RuntimeError::RecoveryBundleKdf(format!("{error:?}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut bytes = [0; WORKSPACE_KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, &mut bytes)
        .map_err(|error| RuntimeError::RecoveryBundleKdf(format!("{error:?}")))?;
    Ok(ContentKey::from_bytes(bytes))
}

fn derive_recovery_bundle_key_blake3(
    passphrase: &str,
    kdf: &WorkspaceRecoveryBundleKdf,
) -> Result<ContentKey, RuntimeError> {
    if kdf.context != RECOVERY_BUNDLE_KDF_CONTEXT || kdf.salt.len() != RECOVERY_BUNDLE_SALT_LEN {
        return Err(RuntimeError::UnsupportedWorkspaceRecoveryBundle);
    }
    let mut input = Vec::with_capacity(kdf.salt.len() + passphrase.len());
    input.extend_from_slice(&kdf.salt);
    input.extend_from_slice(passphrase.as_bytes());
    Ok(ContentKey::from_bytes(blake3::derive_key(
        RECOVERY_BUNDLE_KDF_CONTEXT,
        &input,
    )))
}

pub(crate) fn recovery_bundle_key_id(workspace_id: &WorkspaceId) -> String {
    format!(
        "{}:recovery:v{}",
        workspace_id.0, RECOVERY_BUNDLE_SCHEMA_VERSION
    )
}

pub(crate) fn recovery_bundle_aad(
    workspace_id: &WorkspaceId,
    exporter_device_id: &DeviceId,
    kdf_name: &str,
    kdf_context: &str,
    salt: &[u8],
) -> Vec<u8> {
    let salt_hash = blake3::hash(salt);
    format!(
        "chaft:v1:workspace_recovery_bundle:{}:{}:{}:{}:{}",
        workspace_id.0, exporter_device_id.0, RECOVERY_BUNDLE_SCHEMA_VERSION, kdf_name, kdf_context
    )
    .into_bytes()
    .into_iter()
    .chain(salt_hash.as_bytes().iter().copied())
    .collect()
}
