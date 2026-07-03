use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
};

use zeroize::Zeroizing;

use crate::{
    envelope::{FfiError, ffi_error},
    input::FFI_PASSPHRASE_MAX_BYTES,
};

static RUNTIME_IDENTITY_PASSPHRASES: OnceLock<Mutex<HashMap<String, Zeroizing<String>>>> =
    OnceLock::new();

pub(crate) fn identity_passphrase_from_env() -> Option<Zeroizing<String>> {
    std::env::var("CHAFT_IDENTITY_PASSPHRASE")
        .ok()
        .filter(|passphrase| env_identity_passphrase_is_usable(passphrase))
        .map(Zeroizing::new)
}

pub(crate) fn env_identity_passphrase_is_usable(passphrase: &str) -> bool {
    !passphrase.trim().is_empty() && passphrase.len() <= FFI_PASSPHRASE_MAX_BYTES
}

pub(crate) fn set_runtime_identity_passphrase(
    data_dir: &Path,
    passphrase: String,
) -> Result<(), FfiError> {
    if passphrase.trim().is_empty() {
        return Err(ffi_error(
            "runtime_passphrase_required",
            "passphrase is required",
        ));
    }

    runtime_identity_passphrase_registry()
        .lock()
        .map_err(|_| ffi_error("runtime_passphrase_registry_failed", "registry poisoned"))?
        .insert(runtime_passphrase_key(data_dir), Zeroizing::new(passphrase));
    Ok(())
}

pub(crate) fn clear_runtime_identity_passphrase(data_dir: &Path) -> Result<(), FfiError> {
    runtime_identity_passphrase_registry()
        .lock()
        .map_err(|_| ffi_error("runtime_passphrase_registry_failed", "registry poisoned"))?
        .remove(&runtime_passphrase_key(data_dir));
    Ok(())
}

pub(crate) fn runtime_identity_passphrase_for_path(data_dir: &Path) -> Option<Zeroizing<String>> {
    runtime_identity_passphrase_registry()
        .lock()
        .ok()
        .and_then(|passphrases| passphrases.get(&runtime_passphrase_key(data_dir)).cloned())
}

fn runtime_identity_passphrase_registry() -> &'static Mutex<HashMap<String, Zeroizing<String>>> {
    RUNTIME_IDENTITY_PASSPHRASES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn runtime_passphrase_key(data_dir: &Path) -> String {
    data_dir.to_string_lossy().into_owned()
}
