use std::path::PathBuf;

use chaft_net_iroh::IrohTransport;
use chaft_runtime::LocalRuntime;

use crate::{envelope::FfiError, worker::run_on_worker_thread};

pub(crate) fn run_direct_runtime_command<T, F>(
    data_dir: String,
    identity_file: Option<PathBuf>,
    operation: F,
) -> Result<T, FfiError>
where
    T: Send + 'static,
    F: FnOnce(LocalRuntime, IrohTransport) -> Result<T, FfiError> + Send + 'static,
{
    run_on_worker_thread(move || {
        let runtime = crate::open_runtime_from_paths(&data_dir, identity_file)?;
        let transport = IrohTransport::from_environment();
        operation(runtime, transport)
    })
}
