// The host's launch guard (0.1.2): the context hands it to the platform's launch, once per worker launch, and a
// context without one hands the platform none. The platform is the one that holds it around its creation window
// (`windows/inheritance_tests.rs` observes that on Windows).
fn recording_guard() -> (LaunchGuard, Arc<Mutex<Vec<&'static str>>>) {
    struct Released(Arc<Mutex<Vec<&'static str>>>);
    impl Drop for Released {
        fn drop(&mut self) {
            self.0.lock().unwrap().push("released");
        }
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let taken = log.clone();
    let guard = LaunchGuard::new(move || {
        taken.lock().unwrap().push("taken");
        Released(taken.clone())
    });
    (guard, log)
}
#[test]
fn a_context_with_a_launch_guard_hands_it_to_the_platforms_launch() {
    let (guard, log) = recording_guard();
    let (context, effects) = host_guarded(true, Some(guard));
    let (record, origin) = ticket(&context, &effects);
    assert!(monitor::launch(&context, &record, origin).is_some());
    assert_eq!(*effects.guards.lock().unwrap(), vec![true]);
    assert_eq!(*log.lock().unwrap(), vec!["taken", "released"]);
}
#[test]
fn a_context_without_a_launch_guard_hands_the_platform_none() {
    let (context, effects) = host(true);
    let (record, origin) = ticket(&context, &effects);
    assert!(monitor::launch(&context, &record, origin).is_some());
    assert_eq!(*effects.guards.lock().unwrap(), vec![false]);
}
#[test]
fn each_launch_takes_the_guard_once_and_a_refused_launch_releases_it() {
    let (guard, log) = recording_guard();
    for round in 1..=2 {
        // One guard (a clone is the same callback) over two launches in two contexts.
        let (context, effects) = host_guarded(true, Some(guard.clone()));
        effects.launch_fail.store(true, Ordering::SeqCst);
        let (record, origin) = ticket(&context, &effects);
        assert!(monitor::launch(&context, &record, origin).is_none());
        assert_eq!(log.lock().unwrap().len(), round * 2, "round {round}");
    }
    assert_eq!(
        *log.lock().unwrap(),
        vec!["taken", "released", "taken", "released"]
    );
}
#[test]
fn a_launch_cancelled_before_it_starts_never_takes_the_guard() {
    let (guard, log) = recording_guard();
    let (context, effects) = host_guarded(true, Some(guard));
    let (record, origin) = ticket(&context, &effects);
    record.stop.store(true, Ordering::SeqCst);
    assert!(monitor::launch(&context, &record, origin).is_none());
    assert!(effects.guards.lock().unwrap().is_empty());
    assert!(log.lock().unwrap().is_empty());
}
