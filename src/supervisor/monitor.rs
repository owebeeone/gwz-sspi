//! Charged launch owner becomes the per-slot native reaper.
use super::{
    context::{Context, Record},
    kernel::{Fault, Proof},
    ports::{Origin, ReadPort, WritePort},
};
use crate::ErrorKind;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
pub(super) fn run(context: Arc<Context>, record: Arc<Record>, origin: Box<dyn Origin>) {
    if let Some((reader, writer)) = launch(&context, &record, origin) {
        let receiver = command_channel(&record);
        let owned_context = context.clone();
        let owned_record = record.clone();
        let writer = thread::Builder::new()
            .name("gwz-sspi-write".into())
            .spawn(move || super::io::writer(owned_context, owned_record, writer, receiver));
        match writer {
            Ok(handle) => {
                record
                    .tasks
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .writer = Some(super::owners::thread(handle))
            }
            Err(_) => context.fault(&record, Fault::new(ErrorKind::ContainmentFailed)),
        }
        let owned_context = context.clone();
        let owned_record = record.clone();
        let reader = thread::Builder::new()
            .name("gwz-sspi-read".into())
            .spawn(move || super::io::reader(owned_context, owned_record, reader));
        match reader {
            Ok(handle) => {
                record
                    .tasks
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .reader = Some(super::owners::thread(handle))
            }
            Err(_) => context.fault(&record, Fault::new(ErrorKind::ContainmentFailed)),
        }
    }
    context.hub.notify();
    reap(context, record);
}
pub(super) fn command_channel(record: &Record) -> mpsc::Receiver<crate::secret::Storage> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let previous = record
        .payload
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .sender
        .replace(sender);
    drop(previous);
    receiver
}
type Pipes = (Box<dyn ReadPort>, Box<dyn WritePort>);
pub(super) fn launch(context: &Context, record: &Record, origin: Box<dyn Origin>) -> Option<Pipes> {
    if record.stop.load(Ordering::Acquire) {
        no_child(context, record, Fault::new(ErrorKind::Cancelled));
        return None;
    }
    let launched = match origin
        .verify()
        .and_then(|()| context.platform.launch(&context.executable, origin))
    {
        Ok(launched) => launched,
        Err(error) => {
            no_child(context, record, Fault::from_error(error));
            return None;
        }
    };
    let child = launched.child;
    record.tasks.lock().unwrap_or_else(|p| p.into_inner()).child = Some(child.clone());
    let allowed = context.update(record, Instant::now(), |kernel| {
        kernel.launch_returned(launched.contained)
    });
    let mut resume = match allowed {
        Some(Ok(allowed)) => allowed,
        Some(Err(fault)) => {
            context.fault(record, fault);
            false
        }
        None => false,
    };
    if record.stop.load(Ordering::Acquire) {
        resume = false;
    }
    if resume && let Err(error) = child.resume() {
        context.fault(record, Fault::from_error(error));
        resume = false;
    }
    if resume && !record.stop.load(Ordering::Acquire) {
        Some((launched.reader, launched.writer))
    } else {
        None
    }
}
pub(super) fn reap(context: Arc<Context>, record: Arc<Record>) {
    while reap_once(&context, &record) == Some(false) {
        thread::sleep(Duration::from_millis(10));
    }
}
pub(super) fn reap_once(context: &Context, record: &Record) -> Option<bool> {
    let child = record
        .tasks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .child
        .clone()?;
    let stop = record.stop.load(Ordering::Acquire);
    let normal = context
        .update(record, Instant::now(), |kernel| kernel.normal_ack())
        .unwrap_or(false);
    if stop || normal {
        let sender = record
            .payload
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sender
            .take();
        drop(sender);
    }
    if stop {
        let _requested = child.terminate();
        // This task-ownership lock is never nested with the phase lock or a
        // payload lock. Handles cannot disappear during advisory cancellation.
        let tasks = record.tasks.lock().unwrap_or_else(|p| p.into_inner());
        for handle in [&tasks.reader, &tasks.writer].into_iter().flatten() {
            handle.cancel(child.as_ref());
        }
    }
    let (reader, writer) = {
        let mut tasks = record.tasks.lock().unwrap_or_else(|p| p.into_inner());
        (
            super::owners::take_finished(&mut tasks.reader),
            super::owners::take_finished(&mut tasks.writer),
        )
    };
    for handle in [reader, writer].into_iter().flatten() {
        if !handle.join() {
            context.fault(record, Fault::new(ErrorKind::Protocol));
        }
    }
    let (reader_finished, writer_finished) = {
        let tasks = record.tasks.lock().unwrap_or_else(|p| p.into_inner());
        (tasks.reader.is_none(), tasks.writer.is_none())
    };
    let (process_exited, job_empty) = child.observe();
    context.update(record, Instant::now(), |kernel| {
        kernel.proof = Proof {
            process_exited,
            job_empty,
            launch_finished: false,
            reader_finished,
            writer_finished,
        }
    });
    context.hub.notify();
    if process_exited && job_empty && reader_finished && writer_finished {
        record.discard_payload();
        let retired = record
            .tasks
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .child
            .take();
        drop(retired);
        return Some(true);
    }
    Some(false)
}
fn no_child(context: &Context, record: &Record, fault: Fault) {
    record.discard_payload();
    context.update(record, Instant::now(), |kernel| {
        kernel.fail(fault);
        kernel.proof = Proof {
            process_exited: true,
            job_empty: true,
            launch_finished: false,
            reader_finished: true,
            writer_finished: true,
        };
    });
    context.hub.notify();
}
