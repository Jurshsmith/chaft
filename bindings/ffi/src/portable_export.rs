use std::{ffi::c_char, io::ErrorKind, path::PathBuf};

use chaft_runtime::{PortableWorkspaceExport, RuntimeError};
use chaft_types::WorkspaceId;

use crate::{
    envelope::{FfiError, FfiResult, ffi_error, result_envelope},
    id_args::ffi_workspace_id_arg,
    input::read_c_string,
    open_runtime_from_ffi,
};

pub(crate) fn export_portable_workspace_archive_result(
    data_dir: *const c_char,
    identity_file: *const c_char,
    workspace_id: *const c_char,
    output_path: *const c_char,
) -> FfiResult<PortableWorkspaceExport> {
    result_envelope(|| {
        let workspace_id = WorkspaceId(ffi_workspace_id_arg(read_c_string(
            workspace_id,
            "workspace_id",
        )?)?);
        let output_path = read_c_string(output_path, "output_path")?;
        if output_path.is_empty() {
            return Err(ffi_error("output_path", "output path is required"));
        }

        let runtime = open_runtime_from_ffi(data_dir, identity_file)?;
        runtime
            .export_portable_workspace_archive(workspace_id, PathBuf::from(output_path))
            .map_err(portable_export_error)
    })
}

fn portable_export_error(error: RuntimeError) -> FfiError {
    match error {
        RuntimeError::PortableExportDestinationInsideRuntime => ffi_error(
            "portable_export_destination_inside_runtime",
            "export destination must be outside Chaft runtime and identity storage",
        ),
        RuntimeError::PortableExportDestinationUnsafe => ffi_error(
            "portable_export_destination_unsafe",
            "export destination cannot be a symbolic link or directory",
        ),
        RuntimeError::Io(error) if error.kind() == ErrorKind::AlreadyExists => ffi_error(
            "portable_export_destination_exists",
            "export destination already exists",
        ),
        RuntimeError::Io(error) if error.kind() == ErrorKind::PermissionDenied => ffi_error(
            "portable_export_permission_denied",
            "permission denied while writing the export archive",
        ),
        RuntimeError::Authorization(_) => ffi_error(
            "portable_export_not_authorized",
            "current device is not authorized to export this workspace",
        ),
        RuntimeError::WorkspaceHasNoEvents { .. } => ffi_error(
            "portable_export_workspace_unavailable",
            "workspace is unavailable in this local runtime",
        ),
        RuntimeError::PortableExportArchive(_) => ffi_error(
            "portable_export_archive_failed",
            "failed to create the portable workspace archive",
        ),
        error => ffi_error("portable_export_failed", error.to_string()),
    }
}
