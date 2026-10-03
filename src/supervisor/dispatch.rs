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
            join_launch(&context, &record);
        }

        let ticket = next_launch(&context);
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

pub(super) fn join_launch(context: &Context, record: &super::context::Record) {
    let handle = {
        let mut handle = record.launch.lock().unwrap_or_else(|p| p.into_inner());
        super::owners::take_finished(&mut handle)
    };
    if let Some(handle) = handle {
        let success = handle.join();
        context.update(record, context.clock.now(), |kernel| {
            kernel.proof.launch_finished = true;
            if !success {
                kernel.fail(super::kernel::Fault::new(
                    crate::ErrorKind::ContainmentFailed,
                ));
                kernel.quarantined = true;
            }
        });
        context.hub.notify();
    }
}

pub(super) fn next_launch(context: &Context) -> Option<super::context::LaunchTicket> {
    context
        .dispatch
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .pop_front()
}
