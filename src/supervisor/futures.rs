//! Poll only pure state/owned bytes; all native and blocking work is offloaded.
use super::{
    api::Conversation,
    context::{Context, Record, Waiter},
    control::WakeSlot,
    kernel::{Command, Fault, Stage},
    ports::Origin,
    values::*,
};
use crate::protocol::supervision;
use crate::{AuthRequest, ErrorKind, SecretBytes, TokenStep};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use std::time::Instant;
fn terminal(context: &Context, record: &Record) -> Option<Fault> {
    context
        .update(record, Instant::now(), |kernel| kernel.terminal)
        .flatten()
        .or_else(|| record.completion.get().and_then(|done| done.fault))
        .or_else(|| {
            if record.cancel.is_cancelled() {
                Some(Fault::new(ErrorKind::Cancelled))
            } else if context.clock.now() >= record.deadline {
                Some(Fault::new(ErrorKind::Timeout))
            } else {
                None
            }
        })
}
pub(super) struct Start {
    pub(super) context: Arc<Context>,
    pub(super) waiter: Arc<Waiter>,
    pub(super) request: Option<AuthRequest>,
    pub(super) origin: Option<Result<Box<dyn Origin>, Fault>>,
    pub(super) record: Option<Arc<Record>>,
    pub(super) done: bool,
}
impl Future for Start {
    type Output = Result<Conversation, Failure>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.waiter.wake.register(cx.waker());
        if this.done {
            return Poll::Pending;
        }
        if this.record.is_none() {
            if let Some(fault) = this
                .origin
                .as_ref()
                .and_then(|result| result.as_ref().err().copied())
            {
                this.done = true;
                this.context.remove_waiter(&this.waiter);
                this.request.take();
                this.origin.take();
                this.waiter.wake.clear();
                return Poll::Ready(Err(this.context.failure(None, fault)));
            }
            let admitted =
                this.context
                    .register(&this.waiter, this.request.as_ref().unwrap(), Instant::now());
            let record = match admitted {
                Ok(None) => return Poll::Pending,
                Ok(Some(record)) => record,
                Err(fault) => {
                    this.done = true;
                    this.context.remove_waiter(&this.waiter);
                    this.request.take();
                    this.origin.take();
                    this.waiter.wake.clear();
                    return Poll::Ready(Err(this.context.failure(None, fault)));
                }
            };
            record
                .payload
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .request = this.request.take();
            this.record = Some(record.clone());
            let origin = this
                .origin
                .take()
                .unwrap()
                .unwrap_or_else(|_| unreachable!("origin was admitted"));
            if let Err(fault) = this.context.spawn(record, origin) {
                this.done = true;
                return Poll::Ready(Err(this.context.failure(this.record.as_deref(), fault)));
            }
        }
        let record = this.record.as_ref().unwrap();
        if let Some(fault) = terminal(&this.context, record) {
            this.done = true;
            return Poll::Ready(Err(this.context.failure(Some(record), fault)));
        }
        let ready = this
            .context
            .update(record, Instant::now(), |kernel| {
                kernel.stage == Stage::Ready
            })
            .unwrap_or(false);
        if ready {
            this.done = true;
            return Poll::Ready(Ok(Conversation {
                context: this.context.clone(),
                record: record.clone(),
                armed: true,
            }));
        }
        Poll::Pending
    }
}
impl Drop for Start {
    fn drop(&mut self) {
        this_remove(self);
        if !self.done
            && let Some(record) = &self.record
        {
            self.context.fault(record, Fault::new(ErrorKind::Cancelled));
        }
    }
}
fn this_remove(start: &Start) {
    start.context.remove_waiter(&start.waiter);
}
pub(super) struct Step<'a> {
    pub(super) conversation: &'a mut Conversation,
    pub(super) challenge: Option<SecretBytes>,
    pub(super) started: bool,
    pub(super) done: bool,
}
fn send(
    context: &Context,
    record: &Record,
    command: Command,
    frame: crate::secret::Storage,
) -> Result<(), Fault> {
    let issued = context
        .update(record, Instant::now(), |kernel| kernel.command(command))
        .ok_or_else(|| Fault::new(ErrorKind::Closed))?;
    issued?;
    let sender = record
        .payload
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .sender
        .clone()
        .ok_or_else(|| Fault::new(ErrorKind::Protocol))?;
    sender
        .try_send(frame)
        .map_err(|_| Fault::new(ErrorKind::Protocol))?;
    context.hub.notify();
    Ok(())
}
impl Future for Step<'_> {
    type Output = Result<TokenStep, Failure>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let context = &this.conversation.context;
        let record = &this.conversation.record;
        record.wake.register(cx.waker());
        if this.done {
            return Poll::Pending;
        }
        if let Some(fault) = terminal(context, record) {
            this.done = true;
            return Poll::Ready(Err(context.failure(Some(record), fault)));
        }
        if !this.started {
            let phase = context.update(record, context.clock.now(), |kernel| kernel.stage);
            let prepared = match (phase, this.challenge.as_ref()) {
                (Some(Stage::Ready), None) => {
                    let payload = record.payload.lock().unwrap_or_else(|p| p.into_inner());
                    payload
                        .request
                        .as_ref()
                        .ok_or_else(|| Fault::new(ErrorKind::Protocol))
                        .and_then(|request| {
                            supervision::begin(request)
                                .map(|frame| (Command::Begin, frame))
                                .map_err(Fault::from_error)
                        })
                }
                (Some(Stage::Between(round)), Some(challenge)) => {
                    supervision::challenge(round + 1, challenge, record.cap)
                        .map(|frame| (Command::Challenge(round + 1), frame))
                        .map_err(Fault::from_error)
                }
                (Some(Stage::Ready | Stage::Between(_)), _) => {
                    Err(Fault::new(ErrorKind::InvalidRequest))
                }
                _ => Err(Fault::new(ErrorKind::Protocol)),
            };
            let result =
                prepared.and_then(|(command, frame)| send(context, record, command, frame));
            if let Err(fault) = result {
                context.fault(record, fault);
                this.done = true;
                return Poll::Ready(Err(
                    context.failure(Some(record), terminal(context, record).unwrap_or(fault))
                ));
            }
            let request = record
                .payload
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .request
                .take();
            drop(request);
            this.challenge.take();
            this.started = true;
        }
        let ready = context
            .update(record, Instant::now(), |kernel| kernel.publishable())
            .unwrap_or(false);
        if ready {
            let token = record
                .payload
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .token
                .take();
            let published =
                context.update(record, context.clock.now(), |kernel| kernel.published());
            match (published, token) {
                (Some(Ok(())), Some(token)) => {
                    this.done = true;
                    return Poll::Ready(Ok(token));
                }
                (Some(Err(fault)), _) => {
                    context.fault(record, fault);
                    this.done = true;
                    return Poll::Ready(Err(
                        context.failure(Some(record), terminal(context, record).unwrap_or(fault))
                    ));
                }
                _ => {
                    let fault = Fault::new(ErrorKind::Protocol);
                    context.fault(record, fault);
                    this.done = true;
                    return Poll::Ready(Err(context.failure(Some(record), fault)));
                }
            }
        }
        if let Some(fault) = terminal(context, record) {
            this.done = true;
            return Poll::Ready(Err(context.failure(Some(record), fault)));
        }
        Poll::Pending
    }
}
impl Drop for Step<'_> {
    fn drop(&mut self) {
        if !self.done {
            self.conversation
                .context
                .fault(&self.conversation.record, Fault::new(ErrorKind::Cancelled));
        }
    }
}
pub(super) struct Finish {
    pub(super) conversation: Conversation,
    pub(super) started: bool,
    pub(super) done: bool,
}
impl Future for Finish {
    type Output = Result<(), Failure>;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let context = &this.conversation.context;
        let record = &this.conversation.record;
        record.wake.register(cx.waker());
        if this.done {
            return Poll::Pending;
        }
        if let Some(fault) = terminal(context, record) {
            this.done = true;
            return Poll::Ready(Err(context.failure(Some(record), fault)));
        }
        if !this.started {
            let result = supervision::finish()
                .map_err(Fault::from_error)
                .and_then(|frame| send(context, record, Command::Finish, frame));
            if let Err(fault) = result {
                context.fault(record, fault);
                this.done = true;
                return Poll::Ready(Err(
                    context.failure(Some(record), terminal(context, record).unwrap_or(fault))
                ));
            }
            let request = record
                .payload
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .request
                .take();
            drop(request);
            this.started = true;
        }
        if record
            .completion
            .get()
            .is_some_and(|completion| completion.normal && completion.fault.is_none())
        {
            this.done = true;
            this.conversation.armed = false;
            return Poll::Ready(Ok(()));
        }
        Poll::Pending
    }
}
pub(super) struct Shutdown {
    pub(super) context: Arc<Context>,
    pub(super) deadline: Instant,
    pub(super) wake: Arc<WakeSlot>,
}
impl Future for Shutdown {
    type Output = ShutdownReport;
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.wake.register(cx.waker());
        let report = self.context.snapshot();
        if report.outstanding.is_empty() || self.context.clock.now() >= self.deadline {
            Poll::Ready(report)
        } else {
            Poll::Pending
        }
    }
}
