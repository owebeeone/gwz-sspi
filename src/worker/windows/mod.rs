use super::super::storage;
mod conversation;
mod observation;
mod owners;
mod provider;
use super::super::{bootstrap::WorkerBootstrap, session};
use crate::Error;
pub(in crate::worker) fn entry(bootstrap: WorkerBootstrap, build: [u8; 32]) -> Result<(), Error> {
    let (primary, mut input, mut output) =
        crate::supervisor::platform::worker::worker_parts(bootstrap.input, bootstrap.output)?;
    session::run(
        &mut *input,
        &mut *output,
        &mut provider::System,
        &primary,
        &build,
    )
}
