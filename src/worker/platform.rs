#[cfg(not(windows))]
mod selected {
    use super::super::bootstrap::WorkerBootstrap;
    use crate::{Error, ErrorKind};
    pub(in crate::worker) fn entry(bootstrap: WorkerBootstrap, _: [u8; 32]) -> Result<(), Error> {
        let _ = (bootstrap.input, bootstrap.output);
        Err(Error::new(ErrorKind::UnsupportedPlatform))
    }
}
#[cfg(windows)]
mod selected {
    include!("windows/mod.rs");
}
pub(super) use selected::entry;
