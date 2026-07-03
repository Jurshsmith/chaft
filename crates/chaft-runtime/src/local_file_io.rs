use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::RuntimeError;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

static SECRET_FILE_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn read_local_metadata_file_with_limit(
    path: &Path,
    max_bytes: usize,
    field: &'static str,
) -> Result<Option<Vec<u8>>, RuntimeError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() > max_bytes as u64 {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
            max_bytes,
        });
    }

    let file = fs::File::open(path)?;
    let mut limited_file = file.take(max_bytes as u64 + 1);
    let mut bytes = Vec::with_capacity(metadata.len().min(max_bytes as u64) as usize);
    limited_file.read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(RuntimeError::MetadataFieldTooLarge {
            field,
            actual_bytes: bytes.len(),
            max_bytes,
        });
    }
    Ok(Some(bytes))
}

pub(crate) fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), RuntimeError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    let (temp_path, mut file) = create_unique_secret_temp_file(path)?;
    let result = (|| -> Result<(), RuntimeError> {
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_secret_parent_directory(parent)?;
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
    result?;

    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn create_unique_secret_temp_file(path: &Path) -> Result<(PathBuf, fs::File), RuntimeError> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            RuntimeError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "secret file path has no file name",
            ))
        })?;

    for _ in 0..32 {
        let counter = SECRET_FILE_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path =
            path.with_file_name(format!(".{file_name}.tmp.{}.{}", process::id(), counter));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(RuntimeError::Io(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique secret temp file",
    )))
}

fn sync_secret_parent_directory(parent: &Path) -> Result<(), RuntimeError> {
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
