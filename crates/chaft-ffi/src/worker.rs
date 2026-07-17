use std::{future::Future, sync::OnceLock, thread};

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
    run_network_future(future)?.map_err(|error| {
        let message = runtime_error_message(&error);
        ffi_error(runtime_error_code(&error, failure_code), message)
    })
}

pub(crate) fn run_network_future<F>(future: F) -> Result<F::Output, FfiError>
where
    F: Future,
{
    static NETWORK_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

    let runtime = NETWORK_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("chaft-network")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())
    });
    match runtime {
        Ok(runtime) => Ok(runtime.block_on(future)),
        Err(error) => Err(ffi_error("tokio_runtime_failed", error.clone())),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_network_runtime_supports_concurrent_blocking_callers() {
        let callers = (0..8)
            .map(|value| {
                thread::spawn(move || {
                    run_network_future(async move {
                        tokio::task::yield_now().await;
                        value * 2
                    })
                    .expect("run network future")
                })
            })
            .collect::<Vec<_>>();
        let mut values = callers
            .into_iter()
            .map(|caller| caller.join().expect("join network caller"))
            .collect::<Vec<_>>();
        values.sort_unstable();

        assert_eq!(values, vec![0, 2, 4, 6, 8, 10, 12, 14]);
    }
}
