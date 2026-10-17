use gwz_sspi::{
    Cancellation, Deadline, ErrorKind, LaunchGuard, Options, Supervisor, WorkerExecutable,
};
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn owned_control_values_have_explicit_deadline_and_default_capacity() {
    assert_eq!(Options::default().max_workers, 8);
    for max_workers in [0, 65] {
        let executable = WorkerExecutable::new(
            std::env::current_dir().unwrap().join("synthetic-worker"),
            [0; 32],
        )
        .unwrap();
        assert_eq!(
            Supervisor::new(executable, Options { max_workers })
                .err()
                .unwrap()
                .kind(),
            ErrorKind::InvalidRequest
        );
    }
    let cancellation = Cancellation::new();
    let copy = cancellation.clone();
    copy.cancel();
    assert!(cancellation.is_cancelled());
    let instant = Instant::now();
    assert_eq!(Deadline::new(instant).instant(), instant);
    assert!(WorkerExecutable::new(PathBuf::from("relative.exe"), [0; 32]).is_err());
}

#[allow(dead_code)]
fn owned_future_contract(
    supervisor: Supervisor,
    request: gwz_sspi::AuthRequest,
    deadline: Deadline,
    cancellation: Cancellation,
) {
    fn accept_owned<F: std::future::Future + Send + 'static>(_: F) {}
    let start = supervisor.start(request, deadline, cancellation);
    let shutdown = supervisor.shutdown(deadline);
    drop(supervisor);
    accept_owned(start);
    accept_owned(shutdown);
}

fn synthetic_worker() -> WorkerExecutable {
    WorkerExecutable::new(
        std::env::current_dir().unwrap().join("synthetic-worker"),
        [0; 32],
    )
    .unwrap()
}

#[test]
fn a_supervisor_with_a_launch_guard_validates_its_options_like_any_other() {
    let taken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for max_workers in [0, 65] {
        let counted = taken.clone();
        let guard = LaunchGuard::new(move || {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let error =
            Supervisor::with_launch_guard(synthetic_worker(), Options { max_workers }, guard)
                .err()
                .unwrap();
        assert_eq!(error.kind(), ErrorKind::InvalidRequest);
    }
    assert_eq!(
        taken.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "construction launches nothing, so it takes no guard"
    );
}

#[cfg(not(windows))]
#[test]
fn a_supervisor_with_a_launch_guard_is_as_unsupported_as_any_other_off_windows() {
    let guard = LaunchGuard::new(|| ());
    let error = Supervisor::with_launch_guard(synthetic_worker(), Options::default(), guard)
        .err()
        .unwrap();
    assert_eq!(error.kind(), ErrorKind::UnsupportedPlatform);
}

#[test]
fn a_launch_guard_is_a_shareable_callback_that_holds_an_owned_value_until_dropped() {
    fn shareable<T: Clone + Send + Sync + 'static>() {}
    shareable::<LaunchGuard>();
    let held = std::sync::Arc::new(());
    let kept = held.clone();
    // The value the callback returns need only be `Send`: a non-`Sync` one is fine.
    let guard = LaunchGuard::new(move || std::cell::Cell::new(kept.clone()));
    assert_eq!(std::sync::Arc::strong_count(&held), 2);
    let taken = guard.hold();
    assert_eq!(
        std::sync::Arc::strong_count(&held),
        3,
        "the hold owns a clone"
    );
    drop(taken);
    assert_eq!(
        std::sync::Arc::strong_count(&held),
        2,
        "dropping it releases"
    );
}
