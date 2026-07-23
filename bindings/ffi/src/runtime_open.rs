use std::{
    ffi::c_char,
    path::{Path, PathBuf},
};

use chaft_runtime::LocalRuntime;

use crate::{
    envelope::{FfiError, ffi_error},
    identity_passphrase::{
        clear_runtime_identity_passphrase, identity_passphrase_from_env,
        runtime_identity_passphrase_for_path, set_runtime_identity_passphrase,
    },
    input::{optional_c_string, read_c_string},
};

pub(crate) fn open_runtime_from_ffi(
    data_dir: *const c_char,
    identity_file: *const c_char,
) -> Result<LocalRuntime, FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let identity_file = optional_c_string(identity_file, "identity_file")?.map(PathBuf::from);
    open_runtime_from_paths(&data_dir, identity_file)
}

pub(crate) fn open_runtime_from_paths(
    data_dir: impl AsRef<Path>,
    identity_file: Option<PathBuf>,
) -> Result<LocalRuntime, FfiError> {
    let data_dir = data_dir.as_ref();
    let identity_passphrase =
        runtime_identity_passphrase_for_path(data_dir).or_else(identity_passphrase_from_env);
    LocalRuntime::open_with_identity_passphrase(
        data_dir,
        identity_file,
        identity_passphrase
            .as_ref()
            .map(|passphrase| passphrase.as_str()),
    )
    .map_err(|error| ffi_error("runtime_open_failed", error.to_string()))
}

pub(crate) fn set_runtime_identity_passphrase_result(
    data_dir: *const c_char,
    passphrase: *const c_char,
) -> Result<(), FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    let passphrase = read_c_string(passphrase, "passphrase")?;
    set_runtime_identity_passphrase(Path::new(&data_dir), passphrase)
}

pub(crate) fn clear_runtime_identity_passphrase_result(
    data_dir: *const c_char,
) -> Result<(), FfiError> {
    let data_dir = read_c_string(data_dir, "data_dir")?;
    clear_runtime_identity_passphrase(Path::new(&data_dir))
}
