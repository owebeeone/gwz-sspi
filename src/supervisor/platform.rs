#[cfg(not(windows))]
mod unsupported {
    use super::super::ports::Platform;
    use crate::{Error, ErrorKind};
    use std::sync::Arc;
    pub(in crate::supervisor) fn system() -> Result<Arc<dyn Platform>, Error> {
        Err(Error::new(ErrorKind::UnsupportedPlatform))
    }
}
#[cfg(windows)]
mod windows {
    include!("windows/mod.rs");
}
#[cfg(not(windows))]
mod selected {
    pub(in crate::supervisor) use super::unsupported::system;
}
#[cfg(windows)]
mod selected {
    pub(in crate::supervisor) use super::windows::system;
}
pub(super) use selected::system;

#[cfg(windows)]
pub(crate) mod worker {
    pub(crate) use super::windows::worker_parts;
}
