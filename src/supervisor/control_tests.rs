struct Reentrant {
    context: Arc<Context>,
    wakes: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    panic: bool,
}
impl Wake for Reentrant {
    fn wake(self: Arc<Self>) {
        assert!(
            self.context.state.try_lock().is_ok(),
            "wake under state lock"
        );
        self.wakes.fetch_add(1, Ordering::SeqCst);
        if self.panic {
            panic!("synthetic host waker panic");
        }
        let _snapshot = self.context.snapshot();
    }
}
impl Drop for Reentrant {
    fn drop(&mut self) {
        assert!(
            self.context.state.try_lock().is_ok(),
            "drop under state lock"
        );
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn reentrant_panicking_wake_and_replacement_drop_are_outside_state_lock() {
    let context = context(1);
    let slot = Arc::new(WakeSlot::default());
    context.hub.register(&slot);
    let wakes = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    for panic in [false, true, false] {
        let waker = Waker::from(Arc::new(Reentrant {
            context: context.clone(),
            wakes: wakes.clone(),
            drops: drops.clone(),
            panic,
        }));
        slot.register(&waker);
        drop(waker);
        context.hub.notify();
    }
    assert_eq!(wakes.load(Ordering::SeqCst), 3);
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    let waker = Waker::from(Arc::new(Reentrant {
        context: context.clone(),
        wakes,
        drops: drops.clone(),
        panic: false,
    }));
    slot.register(&waker);
    drop(waker);
    slot.register(&Waker::from(Arc::new(Noop)));
    assert_eq!(drops.load(Ordering::SeqCst), 4);
    slot.clear();
}
#[test]
fn expired_cancelled_and_dropped_fifo_front_never_drops_host_waker_under_state_lock() {
    let context = context(1);
    let retained = register(&context);
    let first = waiter(&context, Instant::now() + Duration::from_secs(3600));
    let drops = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(Reentrant {
        context: context.clone(),
        wakes: Arc::new(AtomicUsize::new(0)),
        drops: drops.clone(),
        panic: false,
    }));
    first.wake.register(&waker);
    drop(waker);
    drop(first);
    let next = waiter(&context, Instant::now() + Duration::from_secs(3600));
    assert!(
        context
            .register(&next, &request(), Instant::now())
            .unwrap()
            .is_none()
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    next.cancel.cancel();
    complete(&context, &retained);
    let final_waiter = waiter(&context, Instant::now() + Duration::from_secs(3600));
    assert!(
        context
            .register(&final_waiter, &request(), Instant::now())
            .unwrap()
            .is_some()
    );
}
#[test]
fn owned_api_and_futures_are_send_without_public_injection() {
    fn send<T: Send>() {}
    fn sync<T: Sync>() {}
    send::<Supervisor>();
    sync::<Supervisor>();
    send::<Conversation>();
    send::<Cancellation>();
    sync::<Cancellation>();
    send::<super::super::futures::Start>();
    send::<super::super::futures::Finish>();
    send::<super::super::futures::Shutdown>();
}
