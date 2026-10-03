//! Owned notification; RawWaker clone/drop/wake never occur under a state lock.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::task::Waker;
#[derive(Default)]
pub(super) struct WakeSlot(Mutex<Option<Waker>>);
impl WakeSlot {
    pub(super) fn register(&self, waker: &Waker) {
        let new = waker.clone();
        let old = self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .replace(new);
        dispose(old);
    }
    pub(super) fn clear(&self) {
        let old = self.0.lock().unwrap_or_else(|p| p.into_inner()).take();
        dispose(old);
    }
    pub(super) fn wake(&self) {
        let waker = self.0.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(waker) = waker {
            let _panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| waker.wake()));
        }
    }
}
fn dispose(waker: Option<Waker>) {
    let _panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(waker)));
}
impl Drop for WakeSlot {
    fn drop(&mut self) {
        let old = self.0.get_mut().unwrap_or_else(|p| p.into_inner()).take();
        dispose(old);
    }
}
#[derive(Default)]
pub(super) struct WakeHub {
    pub(super) condition: Condvar,
    pub(super) wait: Mutex<()>,
    slots: Mutex<Vec<Weak<WakeSlot>>>,
}
impl WakeHub {
    pub(super) fn register(&self, slot: &Arc<WakeSlot>) {
        let mut slots = self.slots.lock().unwrap_or_else(|p| p.into_inner());
        slots.retain(|slot| slot.strong_count() > 0);
        if !slots
            .iter()
            .any(|entry| entry.ptr_eq(&Arc::downgrade(slot)))
        {
            slots.push(Arc::downgrade(slot));
        }
    }
    pub(super) fn notify(&self) {
        self.condition.notify_all();
        let slots: Vec<_> = self
            .slots
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for slot in slots {
            slot.wake();
        }
    }
}
#[derive(Default)]
pub(super) struct Signal {
    cancelled: AtomicBool,
    hubs: Mutex<Vec<Weak<WakeHub>>>,
}
impl Signal {
    pub(super) fn subscribe(&self, hub: &Arc<WakeHub>) {
        let weak = Arc::downgrade(hub);
        let mut hubs = self.hubs.lock().unwrap_or_else(|p| p.into_inner());
        hubs.retain(|entry| entry.strong_count() > 0);
        if !hubs.iter().any(|entry| entry.ptr_eq(&weak)) {
            hubs.push(weak);
        }
    }
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        let hubs: Vec<_> = self
            .hubs
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for hub in hubs {
            hub.notify();
        }
    }
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}
