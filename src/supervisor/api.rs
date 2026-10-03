use super::{
    context::{Context, Waiter},
    control::WakeSlot,
    futures::{Finish, Shutdown, Start, Step},
    kernel::Fault,
    ports::Platform,
    values::*,
};
use crate::{AuthRequest, Error, ErrorKind, SecretBytes, TokenStep};
use std::future::Future;
use std::sync::Arc;

/// Parent supervision with bounded retained capacity and dedicated owned threads.
/// Native construction captures the primary Windows identity synchronously.
/// ```compile_fail
/// fn require<T: Clone>() {} require::<gwz_sspi::Supervisor>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {} require::<gwz_sspi::Supervisor>();
/// ```
pub struct Supervisor {
    pub(super) context: Arc<Context>,
}
impl Supervisor {
    /// Capture the primary token and refuse caller-thread impersonation. This
    /// constructor makes synchronous OS metadata calls; no realtime bound is
    /// promised. Non-Windows returns UnsupportedPlatform.
    pub fn new(executable: WorkerExecutable, options: Options) -> Result<Self, Error> {
        let options = options.validate()?;
        let platform = super::platform::system()?;
        Self::from_platform(executable, options, platform)
    }
    pub(super) fn from_platform(
        executable: WorkerExecutable,
        options: Options,
        platform: Arc<dyn Platform>,
    ) -> Result<Self, Error> {
        let options = options.validate()?;
        let context = Context::new(executable, usize::from(options.max_workers), platform);
        super::driver::start(context.clone())?;
        Ok(Self { context })
    }
    /// Own the request and wait for capacity/verified Hello. No credentials are
    /// sent until step(None). This method synchronously snapshots/refuses the
    /// originating thread and captures its owned handle. Subsequent metadata,
    /// launch and I/O waits are charged/offloaded. Poll performs no metadata or
    /// provider query, worker/thread creation, IPC, process wait or join. Refusal
    /// before registration and Drop may synchronously close the captured origin
    /// handle outside state locks; no hard OS time bound is promised.
    /// Moving the owned Send + 'static future does not substitute executor identity.
    pub fn start(
        &self,
        request: AuthRequest,
        deadline: Deadline,
        cancellation: Cancellation,
    ) -> impl Future<Output = Result<Conversation, Failure>> + Send + use<> {
        let validation =
            crate::protocol::supervision::validate_request(&request).map_err(Fault::from_error);
        let origin = validation.and_then(|()| {
            self.context
                .platform
                .capture_origin()
                .map_err(Fault::from_error)
        });
        let waiter = Arc::new(Waiter {
            wake: Arc::new(WakeSlot::default()),
            deadline: deadline.instant(),
            cancel: cancellation,
        });
        self.context.enqueue(&waiter);
        Start {
            context: self.context.clone(),
            waiter,
            request: Some(request),
            origin: Some(origin),
            record: None,
            done: false,
        }
    }
    /// Observe only this context's retained record or bounded FIFO tombstone.
    pub fn cleanup_status(&self, id: RecordId) -> CleanupStatus {
        self.context.status(&id)
    }
    /// Close admission and cancel all records immediately. The owned future waits
    /// only until this explicit separate shutdown deadline; dropping it leaves
    /// supervision running. Its Send + 'static future does not borrow Supervisor.
    /// Repeated calls are idempotent observations.
    pub fn shutdown(
        &self,
        deadline: Deadline,
    ) -> impl Future<Output = ShutdownReport> + Send + use<> {
        self.context.close();
        let wake = Arc::new(WakeSlot::default());
        self.context.hub.register(&wake);
        Shutdown {
            context: self.context.clone(),
            deadline: deadline.instant(),
            wake,
        }
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.context.close();
    }
}
/// Exclusive mutable conversation; Drop cancels rather than abandoning ownership.
/// ```compile_fail
/// fn require<T: Clone>() {} require::<gwz_sspi::Conversation>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {} require::<gwz_sspi::Conversation>();
/// ```
pub struct Conversation {
    pub(super) context: Arc<Context>,
    pub(super) record: Arc<super::context::Record>,
    pub(super) armed: bool,
}
impl Conversation {
    /// Begin with None, then pass each opaque nonempty challenge. At most eight
    /// rounds, serialized through mutable ownership. Complete forbids another step.
    /// Dropping this future cancels the entire conversation.
    pub fn step(
        &mut self,
        challenge: Option<SecretBytes>,
    ) -> impl Future<Output = Result<TokenStep, Failure>> + Send + '_ {
        Step {
            conversation: self,
            challenge,
            started: false,
            done: false,
        }
    }
    /// Dispose normally and confirm held exit, empty Job and all owner threads.
    /// Legal before Begin and between steps. Uses the original immutable deadline;
    /// a failure may return Pending cleanup, which retained supervision continues.
    pub fn finish(self) -> impl Future<Output = Result<(), Failure>> + Send {
        Finish {
            conversation: self,
            started: false,
            done: false,
        }
    }
    /// Revoke publication and initiate owned cleanup, returning its current status.
    pub fn cancel(mut self) -> CancellationReceipt {
        self.context
            .fault(&self.record, Fault::new(ErrorKind::Cancelled));
        self.armed = false;
        let record_id = self.context.id(self.record.sequence);
        let cleanup = self.context.status(&record_id);
        CancellationReceipt { record_id, cleanup }
    }
}
impl Drop for Conversation {
    fn drop(&mut self) {
        if self.armed {
            self.context
                .fault(&self.record, Fault::new(ErrorKind::Cancelled));
        }
    }
}
