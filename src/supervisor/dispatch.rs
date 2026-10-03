//! Bounded by charged records. OS thread creation is never done by Future::poll
//! or the independent deadline driver. A stalled spawn retains its ticket/permit.
use super::context::Context;
use std::sync::Arc;
use std::time::Duration;
pub(super) fn run(context: Arc<Context>) {
    loop {
        // Actual launch joins belong to this blocking dispatcher, never the
        // deadline/control driver. A stalled native join retains the permit.
        let records = {
            let state = context.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .records
                .values()
                .map(|entry| entry.record.clone())
                .collect::<Vec<_>>()
        };
        for record in records {
            let handle = {
                let mut handle = record.launch.lock().unwrap_or_else(|p| p.into_inner());
                if handle
                    .as_ref()
                    .is_some_and(std::thread::JoinHandle::is_finished)
                {
                    handle.take()
                } else {
                    None
                }
            };
            if let Some(handle) = handle {
                let result = handle.join();
                context.update(&record, context.clock.now(), |kernel| {
                    kernel.proof.launch_finished = true;
                    if result.is_err() {
                        kernel.fail(super::kernel::Fault::new(
                            crate::ErrorKind::ContainmentFailed,
                        ));
                        kernel.quarantined = true;
                    }
                });
                context.hub.notify();
            }
        }

        let ticket = context
            .dispatch
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front();
        if let Some((record, origin)) = ticket {
            let _result = context.spawn_launch(record, origin);
            continue;
        }
        let done = {
            let state = context.state.lock().unwrap_or_else(|p| p.into_inner());
            state.closed && state.records.is_empty()
        };
        if done {
            return;
        }
        let wait = context.hub.wait.lock().unwrap_or_else(|p| p.into_inner());
        let _wait = context
            .hub
            .condition
            .wait_timeout(wait, Duration::from_millis(10))
            .unwrap_or_else(|p| p.into_inner());
    }
}
