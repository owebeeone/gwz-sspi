//! Narrow owned OS ports, private to contained SSPI supervision.
use super::values::{LaunchGuard, WorkerExecutable};
use crate::{Error, SecretBytes};
use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;
pub(crate) struct Primary {
    pub(crate) sid: SecretBytes,
    pub(crate) luid: SecretBytes,
    pub(crate) session: u32,
}
pub(super) trait Origin: Send + Sync {
    fn verify(&self) -> Result<(), Error>;
}
pub(crate) trait ReadPort: Send {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Error>;
}
pub(crate) trait WritePort: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error>;
}
pub(super) trait Child: Send + Sync {
    fn resume(&self) -> Result<(), Error>;
    fn terminate(&self) -> bool;
    fn observe(&self) -> (bool, bool);
    fn cancel_io(&self, thread: &JoinHandle<()>);
}
pub(super) struct Launched {
    pub(super) child: Arc<dyn Child>,
    pub(super) reader: Box<dyn ReadPort>,
    pub(super) writer: Box<dyn WritePort>,
    pub(super) contained: bool,
}
pub(super) trait Platform: Send + Sync {
    fn primary(&self) -> &Primary;
    fn capture_origin(&self) -> Result<Box<dyn Origin>, Error>;
    /// Err guarantees that CreateProcess did not succeed. After success, return
    /// owned held process/Job even if containment verification refused. `guard`,
    /// when the host gave one, is held around the creation window and until the child
    /// pipe ends are closed. `stop` is the record's: it is set once the record has ended
    /// (a deadline, a cancellation, a close), and a launch that finds it set after it
    /// waited for the guard creates nothing.
    fn launch(
        &self,
        executable: &WorkerExecutable,
        origin: Box<dyn Origin>,
        guard: Option<&LaunchGuard>,
        stop: &AtomicBool,
    ) -> Result<Launched, Error>;
}
