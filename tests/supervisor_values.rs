use gwz_sspi::{Cancellation, Deadline, ErrorKind, Options, Supervisor, WorkerExecutable};
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
