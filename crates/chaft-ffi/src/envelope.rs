use std::{
    ffi::{CString, c_char},
    ptr,
};

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FfiResult<T>
where
    T: Serialize,
{
    ok: bool,
    value: Option<T>,
    error: Option<FfiError>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FfiError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

pub(crate) fn result_envelope<T, F>(build: F) -> FfiResult<T>
where
    T: Serialize,
    F: FnOnce() -> Result<T, FfiError>,
{
    match build() {
        Ok(value) => FfiResult {
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => FfiResult {
            ok: false,
            value: None,
            error: Some(error),
        },
    }
}

pub(crate) fn ffi_error(code: &'static str, message: impl Into<String>) -> FfiError {
    FfiError {
        code,
        message: message.into(),
    }
}

pub(crate) fn into_c_string<T>(value: &T) -> *mut c_char
where
    T: Serialize,
{
    match serde_json::to_string(value)
        .ok()
        .and_then(|json| CString::new(json).ok())
    {
        Some(value) => value.into_raw(),
        None => ptr::null_mut(),
    }
}
