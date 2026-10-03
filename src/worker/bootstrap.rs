use crate::{Error, ErrorKind};
use std::ffi::OsString;
/// Private inherited-pipe bootstrap, recognized before ordinary host parsing.
/// Contains only nonsecret handle numbers; cannot be cloned or formatted.
/// ```compile_fail
/// fn require<T: Clone>() {}
/// require::<gwz_sspi::WorkerBootstrap>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {}
/// require::<gwz_sspi::WorkerBootstrap>();
/// ```
pub struct WorkerBootstrap {
    pub(super) input: usize,
    pub(super) output: usize,
}
impl WorkerBootstrap {
    /// Inspect arguments excluding argv[0]. Non-worker invocations return None.
    /// The internal marker requires exactly two distinct nonzero decimal handles.
    pub fn from_args(mut args: impl Iterator<Item = OsString>) -> Result<Option<Self>, Error> {
        let Some(marker) = args.next() else {
            return Ok(None);
        };
        if marker != "--gwz-sspi-worker" {
            return Ok(None);
        }
        let mut number = || -> Result<usize, Error> {
            let arg = args.next().ok_or_else(bad)?;
            let text = arg.to_str().ok_or_else(bad)?;
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            let value = text.parse::<usize>().map_err(|_| bad())?;
            if value == 0 || value == usize::MAX {
                return Err(bad());
            }
            Ok(value)
        };
        let input = number()?;
        let output = number()?;
        if input == output || args.next().is_some() {
            return Err(bad());
        }
        Ok(Some(Self { input, output }))
    }
}
fn bad() -> Error {
    Error::new(ErrorKind::InvalidRequest)
}
/// Run one serial native conversation on validated private inherited pipes.
/// Call only during the host's early internal dispatch, before logging/runtime.
/// Fingerprint bytes must come from trusted matching host packaging metadata.
/// Blocks on private IPC/native calls; it is the dedicated child entry, not a
/// parent API. Returns after native cleanup; success does not prove remote auth.
pub fn worker_entry(bootstrap: WorkerBootstrap, build_fingerprint: [u8; 32]) -> Result<(), Error> {
    super::platform::entry(bootstrap, build_fingerprint)
}
