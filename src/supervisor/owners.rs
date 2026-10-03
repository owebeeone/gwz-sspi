//! Private owned completion port. Production owns the actual JoinHandle; tests
//! supply per-instance finished/join outcomes without spawning native threads.
use super::ports::Child;
use std::thread::JoinHandle;
pub(super) trait Owner: Send {
    fn is_finished(&self) -> bool;
    fn join(self: Box<Self>) -> bool;
    fn cancel(&self, child: &dyn Child);
}
struct Thread(JoinHandle<()>);
impl Owner for Thread {
    fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
    fn join(self: Box<Self>) -> bool {
        self.0.join().is_ok()
    }
    fn cancel(&self, child: &dyn Child) {
        child.cancel_io(&self.0);
    }
}
pub(super) fn thread(handle: JoinHandle<()>) -> Box<dyn Owner> {
    Box::new(Thread(handle))
}
pub(super) fn take_finished(owner: &mut Option<Box<dyn Owner>>) -> Option<Box<dyn Owner>> {
    if owner.as_ref().is_some_and(|owner| owner.is_finished()) {
        owner.take()
    } else {
        None
    }
}
