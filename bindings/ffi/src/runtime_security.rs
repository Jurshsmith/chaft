use std::ffi::c_char;

use chaft_runtime::{
    ChannelKeyExport, ImportedChannelKey, ImportedWorkspaceKey, ImportedWorkspaceRecoveryBundle,
    RotatedChannelKey, RotatedWorkspaceForSuspectedCompromise, RotatedWorkspaceKey,
    RotatedWorkspaceManualKeys, WorkspaceCompromiseReport, WorkspaceCompromiseResponse,
    WorkspaceKeyExport, WorkspaceRecoveryBundle,
};
use chaft_types::{ChannelId, SignedTrustSnapshot, WorkspaceId};

use crate::{
    envelope::{FfiResult, ffi_error, result_envelope},
    id_args::{ffi_channel_id_arg, ffi_workspace_id_arg},
    input::{
        KEY_TRANSFER_JSON_MAX_BYTES, RECOVERY_BUNDLE_JSON_MAX_BYTES, read_c_string,
        read_c_string_with_max_bytes, validate_json_payload_size,
    },
    open_runtime_from_ffi,
    result_sampling::{
        sample_compromise_response_report_with_rotation_samples,
        sample_imported_workspace_recovery_bundle_report,
        sample_rotated_workspace_for_suspected_compromise_report,
        sample_rotated_workspace_manual_keys_report, sample_workspace_compromise_report,
    },
};

pub(crate) fn runtime_export_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceKeyExport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .export_workspace_key(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_export_workspace_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_rotate_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_key(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_rotate_workspace_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_rotate_workspace_manual_keys_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceManualKeys> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_manual_keys(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_rotate_workspace_manual_keys_failed",
                    error.to_string(),
                )
            })
            .map(sample_rotated_workspace_manual_keys_report)
    })
}

pub(crate) fn runtime_rotate_workspace_for_suspected_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<RotatedWorkspaceForSuspectedCompromise> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .rotate_workspace_for_suspected_compromise(WorkspaceId(workspace_id))
            .map_err(|error| {
                ffi_error(
                    "runtime_rotate_workspace_for_suspected_compromise_failed",
                    error.to_string(),
                )
            })
            .map(sample_rotated_workspace_for_suspected_compromise_report)
    })
}

pub(crate) fn runtime_detect_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceCompromiseReport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .detect_workspace_compromise_signals(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_detect_compromise_failed", error.to_string()))
            .map(sample_workspace_compromise_report)
    })
}

pub(crate) fn runtime_respond_compromise_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<WorkspaceCompromiseResponse> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .respond_to_workspace_compromise_signals(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_respond_compromise_failed", error.to_string()))
            .map(sample_compromise_response_report_with_rotation_samples)
    })
}

pub(crate) fn runtime_export_trust_snapshot_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
) -> FfiResult<SignedTrustSnapshot> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        runtime
            .export_trust_snapshot(WorkspaceId(workspace_id))
            .map_err(|error| ffi_error("runtime_export_trust_snapshot_failed", error.to_string()))
    })
}

pub(crate) fn runtime_import_workspace_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> FfiResult<ImportedWorkspaceKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let key_json = read_c_string_with_max_bytes(
            key_json,
            "key_json",
            KEY_TRANSFER_JSON_MAX_BYTES,
            "workspace_key_json_too_large",
            "workspace key JSON",
        )?;
        validate_json_payload_size(
            &key_json,
            KEY_TRANSFER_JSON_MAX_BYTES,
            "workspace_key_json_too_large",
            "workspace key JSON",
        )?;
        let key = serde_json::from_str::<WorkspaceKeyExport>(&key_json)
            .map_err(|error| ffi_error("invalid_workspace_key_json", error.to_string()))?;
        runtime
            .import_workspace_key(key)
            .map_err(|error| ffi_error("runtime_import_workspace_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_export_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<ChannelKeyExport> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .export_channel_key(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_export_channel_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_rotate_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    channel_id: *const c_char,
) -> FfiResult<RotatedChannelKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let channel_id = ffi_channel_id_arg(read_c_string(channel_id, "channel_id")?)?;
        runtime
            .rotate_channel_key(WorkspaceId(workspace_id), ChannelId(channel_id))
            .map_err(|error| ffi_error("runtime_rotate_channel_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_import_channel_key_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    key_json: *const c_char,
) -> FfiResult<ImportedChannelKey> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let key_json = read_c_string_with_max_bytes(
            key_json,
            "key_json",
            KEY_TRANSFER_JSON_MAX_BYTES,
            "channel_key_json_too_large",
            "channel key JSON",
        )?;
        validate_json_payload_size(
            &key_json,
            KEY_TRANSFER_JSON_MAX_BYTES,
            "channel_key_json_too_large",
            "channel key JSON",
        )?;
        let key = serde_json::from_str::<ChannelKeyExport>(&key_json)
            .map_err(|error| ffi_error("invalid_channel_key_json", error.to_string()))?;
        runtime
            .import_channel_key(key)
            .map_err(|error| ffi_error("runtime_import_channel_key_failed", error.to_string()))
    })
}

pub(crate) fn runtime_export_recovery_bundle_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    passphrase: *const c_char,
) -> FfiResult<WorkspaceRecoveryBundle> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let workspace_id = ffi_workspace_id_arg(read_c_string(workspace_id, "workspace_id")?)?;
        let passphrase = read_c_string(passphrase, "passphrase")?;
        runtime
            .export_workspace_recovery_bundle(WorkspaceId(workspace_id), &passphrase)
            .map_err(|error| ffi_error("runtime_export_recovery_bundle_failed", error.to_string()))
    })
}

pub(crate) fn runtime_import_recovery_bundle_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    bundle_json: *const c_char,
    passphrase: *const c_char,
) -> FfiResult<ImportedWorkspaceRecoveryBundle> {
    result_envelope(|| {
        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        let bundle_json = read_c_string(bundle_json, "bundle_json")?;
        validate_json_payload_size(
            &bundle_json,
            RECOVERY_BUNDLE_JSON_MAX_BYTES,
            "recovery_bundle_json_too_large",
            "recovery bundle JSON",
        )?;
        let passphrase = read_c_string(passphrase, "passphrase")?;
        let bundle = serde_json::from_str::<WorkspaceRecoveryBundle>(&bundle_json)
            .map_err(|error| ffi_error("invalid_recovery_bundle_json", error.to_string()))?;
        runtime
            .import_workspace_recovery_bundle(bundle, &passphrase)
            .map_err(|error| ffi_error("runtime_import_recovery_bundle_failed", error.to_string()))
            .map(sample_imported_workspace_recovery_bundle_report)
    })
}
