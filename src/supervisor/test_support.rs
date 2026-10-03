use super::{
    api::{Conversation, Supervisor},
    context::{Context, Record, Waiter},
    control::WakeSlot,
    kernel::{Fault, Proof, Stage},
    ports::{Launched, Origin, Platform, Primary},
    values::*,
};
use crate::{
    AuthRequest, Error, ErrorKind, Identity, MechanismObservation, Package, SecretBytes,
    SecretText, TokenLimit, TokenStatus, TokenStep,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context as TaskContext, Poll, Wake, Waker};
use std::time::{Duration, Instant};
struct FakePlatform {
    primary: Primary,
    captures: Arc<AtomicUsize>,
    checks: Arc<AtomicUsize>,
}
struct FakeOrigin {
    checks: Arc<AtomicUsize>,
}
impl Origin for FakeOrigin {
    fn verify(&self) -> Result<(), Error> {
        self.checks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl Platform for FakePlatform {
    fn primary(&self) -> &Primary {
        &self.primary
    }
    fn capture_origin(&self) -> Result<Box<dyn Origin>, Error> {
        self.captures.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeOrigin {
            checks: self.checks.clone(),
        }))
    }
    fn launch(&self, _: &WorkerExecutable, _: Box<dyn Origin>) -> Result<Launched, Error> {
        panic!("pure tests must not dispatch an OS launch")
    }
}
fn context(capacity: usize) -> Arc<Context> {
    let executable = WorkerExecutable::new(
        std::env::current_dir().unwrap().join("synthetic-worker"),
        [b'b'; 32],
    )
    .unwrap();
    Context::new(
        executable,
        capacity,
        Arc::new(FakePlatform {
            primary: Primary {
                sid: SecretBytes::new(&[1, 0, 0, 0, 0, 0, 0, 0]),
                luid: SecretBytes::new(&[0; 8]),
                session: 0,
            },
            captures: Arc::new(AtomicUsize::new(0)),
            checks: Arc::new(AtomicUsize::new(0)),
        }),
    )
}
fn request() -> AuthRequest {
    AuthRequest {
        package: Package::Ntlm,
        target: SecretText::new("HTTP/fixture.invalid").unwrap(),
        identity: Identity::CurrentLogon,
        channel_binding: SecretBytes::new(b"tls-server-end-point:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        token_limit: TokenLimit::new(64).unwrap(),
        digest: None,
    }
}
fn waiter(context: &Arc<Context>, deadline: Instant) -> Arc<Waiter> {
    let waiter = Arc::new(Waiter {
        wake: Arc::new(WakeSlot::default()),
        deadline,
        cancel: Cancellation::new(),
    });
    context.enqueue(&waiter);
    waiter
}
fn register(context: &Arc<Context>) -> Arc<Record> {
    let waiter = waiter(context, Instant::now() + Duration::from_secs(3600));
    context
        .register(&waiter, &request(), Instant::now())
        .unwrap()
        .unwrap()
}
fn complete(context: &Arc<Context>, record: &Record) {
    context.update(record, Instant::now(), |kernel| {
        kernel.fail(Fault::new(ErrorKind::Cancelled));
        kernel.proof = Proof {
            process_exited: true,
            job_empty: true,
            launch_finished: true,
            reader_finished: true,
            writer_finished: true,
        };
    });
    drop(context.reap(record.sequence).unwrap());
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(Noop));
    Pin::new(future).poll(&mut TaskContext::from_waker(&waker))
}
fn token() -> TokenStep {
    TokenStep {
        status: TokenStatus::Complete,
        attributes: 0,
        observation: MechanismObservation::Unresolved,
        payload: SecretBytes::new(b"synthetic"),
    }
}
