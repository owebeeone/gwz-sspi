//! Charged launch owner becomes the per-slot native reaper.
use super::{
    context::{Context, Record},
    kernel::{Fault, Proof},
    ports::Origin,
};
use crate::ErrorKind;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
pub(super) fn run(context: Arc<Context>, record: Arc<Record>, origin: Box<dyn Origin>) {
    if record.stop.load(Ordering::Acquire) {
        no_child(&context, &record, Fault::new(ErrorKind::Cancelled));
        return;
    }
    let launched = match origin
        .verify()
        .and_then(|()| context.platform.launch(&context.executable, origin))
    {
        Ok(launched) => launched,
        Err(error) => {
            no_child(&context, &record, Fault::from_error(error));
            return;
        }
    };
    let child = launched.child;
    record.tasks.lock().unwrap_or_else(|p| p.into_inner()).child = Some(child.clone());
    let allowed = context.update(&record, Instant::now(), |kernel| {
        kernel.launch_returned(launched.contained)
    });
    let mut resume = match allowed {
        Some(Ok(allowed)) => allowed,
        Some(Err(fault)) => {
            context.fault(&record, fault);
            false
        }
        None => false,
    };
    if record.stop.load(Ordering::Acquire) {
        resume = false;
    }
    if resume && let Err(error) = child.resume() {
        context.fault(&record, Fault::from_error(error));
        resume = false;
    }
    if resume && !record.stop.load(Ordering::Acquire) {
        let (sender, receiver) = mpsc::sync_channel(1);
        record
            .payload
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sender = Some(sender);
        let owned_context = context.clone();
        let owned_record = record.clone();
        let writer = thread::Builder::new()
            .name("gwz-sspi-write".into())
            .spawn(move || {
                super::io::writer(owned_context, owned_record, launched.writer, receiver)
            });
        match writer {
            Ok(handle) => {
                record
                    .tasks
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .writer = Some(handle)
            }
            Err(_) => context.fault(&record, Fault::new(ErrorKind::ContainmentFailed)),
        }
        let owned_context = context.clone();
        let owned_record = record.clone();
        let reader = thread::Builder::new()
            .name("gwz-sspi-read".into())
            .spawn(move || super::io::reader(owned_context, owned_record, launched.reader));
        match reader {
            Ok(handle) => {
                record
                    .tasks
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .reader = Some(handle)
            }
            Err(_) => context.fault(&record, Fault::new(ErrorKind::ContainmentFailed)),
        }
    } else {
        drop(launched.reader);
        drop(launched.writer);
    }
    context.hub.notify();
    reap(context, record);
}
pub(super) fn reap(context: Arc<Context>, record: Arc<Record>) {
    let child = record
        .tasks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .child
        .clone();
    let Some(child) = child else {
        return;
    };
    loop {
        let stop = record.stop.load(Ordering::Acquire);
        let normal = context
            .update(&record, Instant::now(), |kernel| kernel.normal_ack())
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
                child.cancel_io(handle);
            }
        }
        let (reader, writer) = {
            let mut tasks = record.tasks.lock().unwrap_or_else(|p| p.into_inner());
            (
                take_finished(&mut tasks.reader),
                take_finished(&mut tasks.writer),
            )
        };
        for handle in [reader, writer].into_iter().flatten() {
            if handle.join().is_err() {
                context.fault(&record, Fault::new(ErrorKind::Protocol));
            }
        }
        let (reader_finished, writer_finished) = {
            let tasks = record.tasks.lock().unwrap_or_else(|p| p.into_inner());
            (tasks.reader.is_none(), tasks.writer.is_none())
        };
        let (process_exited, job_empty) = child.observe();
        context.update(&record, Instant::now(), |kernel| {
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
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    record.discard_payload();
    let retired = record
        .tasks
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .child
        .take();
    drop(retired);
    // The local held child is released on this charged launch/reaper thread.
    // The dispatcher joins only after handle closure and wiping finish.
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
fn take_finished(handle: &mut Option<JoinHandle<()>>) -> Option<JoinHandle<()>> {
    if handle.as_ref().is_some_and(JoinHandle::is_finished) {
        handle.take()
    } else {
        None
    }
}
