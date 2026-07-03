use std::{future::Future, thread};

use chaft_runtime::RuntimeError;

use crate::envelope::{FfiError, ffi_error};

pub(crate) fn run_on_worker_thread<T, F>(operation: F) -> Result<T, FfiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, FfiError> + Send + 'static,
{
    thread::spawn(operation)
        .join()
        .map_err(|_| ffi_error("runtime_network_worker_panicked", "network worker panicked"))?
}

pub(crate) fn run_runtime_future<T, F>(future: F, failure_code: &'static str) -> Result<T, FfiError>
where
    F: Future<Output = Result<T, RuntimeError>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| ffi_error("tokio_runtime_failed", error.to_string()))?;
    runtime.block_on(future).map_err(|error| {
        let message = runtime_error_message(&error);
        ffi_error(runtime_error_code(&error, failure_code), message)
    })
}

fn runtime_error_code(error: &RuntimeError, fallback_code: &'static str) -> &'static str {
    if error.is_peer_protocol_error() {
        "runtime_peer_protocol_failed"
    } else {
        fallback_code
    }
}

fn runtime_error_message(error: &RuntimeError) -> String {
    error
        .peer_protocol_error_message()
        .unwrap_or_else(|| error.to_string())
}
