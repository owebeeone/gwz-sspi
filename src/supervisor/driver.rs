//! Dedicated deadline/control driver; never calls OS ports or waits on native work.
use super::context::Context;
use std::sync::Arc;
use std::time::{Duration, Instant};
pub(super) fn start(context: Arc<Context>) -> Result<(), crate::Error> {
    let dispatch = context.clone();
    std::thread::Builder::new()
        .name("gwz-sspi-dispatch".into())
        .spawn(move || super::dispatch::run(dispatch))
        .map_err(|_| crate::Error::new(crate::ErrorKind::ContainmentFailed))?;
    let control = context.clone();
    std::thread::Builder::new()
        .name("gwz-sspi-control".into())
        .spawn(move || run(control))
        .map(|_| ())
        .map_err(|_| {
            context.close();
            crate::Error::new(crate::ErrorKind::ContainmentFailed)
        })
}
fn run(context: Arc<Context>) {
    loop {
        let now = Instant::now();
        let records = {
            let mut state = context.state.lock().unwrap_or_else(|p| p.into_inner());
            state.waiters.retain(|entry| {
                entry.owner.strong_count() > 0
                    && !entry.cancel.is_cancelled()
                    && now < entry.deadline
            });
            state
                .records
                .values()
                .map(|entry| entry.record.clone())
                .collect::<Vec<_>>()
        };
        for record in records {
            context.update(&record, now, |kernel| {
                if kernel.terminal.is_some() && !kernel.proof.complete() {
                    kernel.quarantined = true;
                }
            });
            let removed = context.reap(record.sequence);
            drop(removed);
        }
        context.hub.notify();
        let done = {
            let state = context.state.lock().unwrap_or_else(|p| p.into_inner());
            state.closed && state.records.is_empty()
        };
        if done {
            break;
        }
        let wait = context.hub.wait.lock().unwrap_or_else(|p| p.into_inner());
        let _wait = context
            .hub
            .condition
            .wait_timeout(wait, Duration::from_millis(10))
            .unwrap_or_else(|p| p.into_inner());
    }
}
