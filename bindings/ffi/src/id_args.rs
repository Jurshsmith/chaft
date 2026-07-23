use chaft_types::{
    EventId, MessageId, is_canonical_event_id_str, validate_channel_id_str, validate_device_id_str,
    validate_device_key_package_id_str, validate_event_id_str, validate_message_id_str,
    validate_workspace_id_str,
};

use crate::envelope::{FfiError, ffi_error};

pub(crate) fn direct_workspace_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "workspace_id_required",
            "workspace ID is required",
        ));
    }
    validate_workspace_id_str(&value)
        .map_err(|error| ffi_error("workspace_id_too_large", error.to_string()))?;
    Ok(value)
}

pub(crate) fn direct_event_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("event_id_required", "event ID is required"));
    }
    validate_event_id_str(&value)
        .map_err(|error| ffi_error("event_id_too_large", error.to_string()))?;
    if !is_canonical_event_id_str(&value) {
        return Err(ffi_error(
            "event_id_not_canonical",
            "event ID must be canonical",
        ));
    }
    Ok(value)
}

pub(crate) fn ffi_workspace_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "workspace_id_required",
            "workspace ID is required",
        ));
    }
    validate_workspace_id_str(&value)
        .map_err(|error| ffi_error("workspace_id_too_large", error.to_string()))?;
    Ok(value)
}

pub(crate) fn ffi_channel_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("channel_id_required", "channel ID is required"));
    }
    validate_channel_id_str(&value)
        .map_err(|error| ffi_error("channel_id_too_large", error.to_string()))?;
    Ok(value)
}

pub(crate) fn ffi_message_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("message_id_required", "message ID is required"));
    }
    validate_message_id_str(&value)
        .map_err(|error| ffi_error("message_id_too_large", error.to_string()))?;
    Ok(value)
}

pub(crate) fn ffi_optional_message_id_arg(
    value: Option<String>,
) -> Result<Option<MessageId>, FfiError> {
    value
        .map(|value| {
            let value = value.trim().to_owned();
            if value.is_empty() {
                Ok(None)
            } else {
                ffi_message_id_arg(value).map(MessageId).map(Some)
            }
        })
        .transpose()
        .map(Option::flatten)
}

pub(crate) fn ffi_event_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("event_id_required", "event ID is required"));
    }
    validate_event_id_str(&value)
        .map_err(|error| ffi_error("event_id_too_large", error.to_string()))?;
    if !is_canonical_event_id_str(&value) {
        return Err(ffi_error(
            "event_id_not_canonical",
            "event ID must be canonical",
        ));
    }
    Ok(value)
}

pub(crate) fn ffi_optional_event_id_arg(
    value: Option<String>,
) -> Result<Option<EventId>, FfiError> {
    value
        .map(|value| ffi_event_id_arg(value).map(EventId))
        .transpose()
}

pub(crate) fn ffi_device_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error("device_id_required", "device ID is required"));
    }
    validate_device_id_str(&value)
        .map_err(|error| ffi_error("device_id_too_large", error.to_string()))?;
    Ok(value)
}

pub(crate) fn ffi_device_key_package_id_arg(value: String) -> Result<String, FfiError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ffi_error(
            "key_package_id_required",
            "device key package ID is required",
        ));
    }
    validate_device_key_package_id_str(&value)
        .map_err(|error| ffi_error("key_package_id_too_large", error.to_string()))?;
    Ok(value)
}
