// The worker's pipe ends and its NUL stderr are inheritable only inside the
// creation window (P3-1): a concurrent spawn that inherits every inheritable
// handle must never receive one.
fn inheritable(handle: &handles::Handle) -> bool {
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};
    let mut flags = 0;
    // SAFETY: the handle is an open owned handle and the output is initialized.
    assert_ne!(unsafe { GetHandleInformation(handle.0, &mut flags) }, 0);
    flags & HANDLE_FLAG_INHERIT != 0
}
// The worker's ends, and the parent ends that keep the pipes open for the test.
fn ends() -> (launch::Ends, [handles::Handle; 2]) {
    let (input, input_parent) = launch::pipe().unwrap();
    let (output_parent, output) = launch::pipe().unwrap();
    let ends = launch::Ends {
        input,
        output,
        stderr: launch::nul().unwrap(),
    };
    (ends, [input_parent, output_parent])
}
fn system32(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32")
        .join(name)
}
fn all_closed(ends: &launch::Ends) -> bool {
    !inheritable(&ends.input) && !inheritable(&ends.output) && !inheritable(&ends.stderr)
}
#[test]
fn no_end_of_a_new_pipe_is_inheritable() {
    let (read, write) = launch::pipe().unwrap();
    assert!(!inheritable(&read), "the read end must not be inheritable");
    assert!(
        !inheritable(&write),
        "the write end must not be inheritable"
    );
}
#[test]
fn the_nul_handle_is_not_inheritable_until_the_window_opens() {
    assert!(!inheritable(&launch::nul().unwrap()));
}
#[test]
fn the_creation_window_makes_the_ends_inheritable_only_while_it_is_open() {
    let (ends, _parents) = ends();
    assert!(all_closed(&ends), "closed before the window opens");
    {
        let _window = launch::InheritWindow::open(&ends).unwrap();
        assert!(inheritable(&ends.input) && inheritable(&ends.output) && inheritable(&ends.stderr));
    }
    assert!(all_closed(&ends), "closed again when the window drops");
}
#[test]
fn a_creation_that_fails_leaves_every_end_closed_to_inheritance() {
    let primary = identity::primary().unwrap();
    let origin = identity::origin(std::sync::Arc::new(primary)).unwrap();
    let job = handles::Handle::new(
        // SAFETY: unnamed noninheritable Job; the sole owned handle.
        unsafe {
            windows_sys::Win32::System::JobObjects::CreateJobObjectW(
                std::ptr::null(),
                std::ptr::null(),
            )
        },
    )
    .unwrap();
    let missing = WorkerExecutable::new(system32("no such worker.exe"), [0; 32]).unwrap();
    let (ends, _parents) = ends();
    let error = launch::start(&missing, origin.as_ref(), &job, &ends, None)
        .err()
        .expect("a missing executable must fail inside CreateProcessW");
    assert_eq!(error.kind(), crate::ErrorKind::WorkerUnavailable);
    assert!(all_closed(&ends), "the window closes on the failure path");
}
#[test]
fn a_creation_through_the_window_succeeds() {
    // CreateProcessW's handle list refuses a handle that is not inheritable, so
    // a successful creation proves the window opened before the call. The child
    // is suspended and never runs.
    let origin = identity::origin(std::sync::Arc::new(identity::primary().unwrap())).unwrap();
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let owned = launch::create_owned(&executable, origin, None).unwrap();
    assert!(owned.contained);
    assert!(owned.process.terminate());
}

// The host's launch guard (0.1.2) is held across the creation window: taken before the ends turn inheritable
// and released after they turn back and after CreateProcessW returned. The rows watch the operating system's
// state (the ends' inherit flag, and how many processes the Job holds) from inside the guard callback and from
// inside the drop of the value it returns, so they observe the ordering rather than mirror the code.
use crate::supervisor::values::LaunchGuard;
use std::sync::{Arc, Mutex};
use windows_sys::Win32::System::JobObjects::{
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
    QueryInformationJobObject,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Seen {
    event: &'static str,
    /// Whether any of the worker's three ends is inheritable.
    inheritable: bool,
    /// How many processes the Job holds: 1 once CreateProcessW has created the worker.
    created: u32,
}
type Log = Arc<Mutex<Vec<Seen>>>;
fn seen(event: &'static str, ends: [usize; 3], job: usize) -> Seen {
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
    let inheritable = ends.iter().any(|end| {
        let mut flags = 0;
        // SAFETY: the end is an open handle owned by the test's launch, which outlives the callback.
        let read = unsafe { GetHandleInformation(*end as HANDLE, &mut flags) };
        assert_ne!(read, 0);
        flags & HANDLE_FLAG_INHERIT != 0
    });
    let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    // SAFETY: the Job is open and owned by the launch; the output is a correctly sized initialized struct.
    let queried = unsafe {
        QueryInformationJobObject(
            job as HANDLE,
            JobObjectBasicAccountingInformation,
            (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
            std::mem::size_of_val(&accounting) as u32,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(queried, 0);
    Seen {
        event,
        inheritable,
        created: accounting.ActiveProcesses,
    }
}
fn observing_guard(ends: [usize; 3], job: usize) -> (LaunchGuard, Log) {
    struct Released(Log, [usize; 3], usize);
    impl Drop for Released {
        fn drop(&mut self) {
            let state = seen("released", self.1, self.2);
            self.0.lock().unwrap().push(state);
        }
    }
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let taken = log.clone();
    let guard = LaunchGuard::new(move || {
        let state = seen("taken", ends, job);
        taken.lock().unwrap().push(state);
        Released(taken.clone(), ends, job)
    });
    (guard, log)
}
fn raw_ends(ends: &launch::Ends) -> [usize; 3] {
    [
        ends.input.0 as usize,
        ends.output.0 as usize,
        ends.stderr.0 as usize,
    ]
}
fn new_job() -> handles::Handle {
    handles::Handle::new(
        // SAFETY: unnamed noninheritable Job; the sole owned handle.
        unsafe {
            windows_sys::Win32::System::JobObjects::CreateJobObjectW(
                std::ptr::null(),
                std::ptr::null(),
            )
        },
    )
    .unwrap()
}
fn current_origin() -> Box<dyn crate::supervisor::ports::Origin> {
    identity::origin(Arc::new(identity::primary().unwrap())).unwrap()
}
#[test]
fn the_guard_spans_the_window_and_the_creation_of_the_worker() {
    // A real creation (the child is suspended and never runs). `created` is 0 when the guard is taken and 1 when
    // it is released, so the guard is held across CreateProcessW; the ends are closed to inheritance at both
    // points, so it is taken before the window opens and released after it has closed.
    let job = new_job();
    let job_raw = job.0 as usize;
    let (input, input_parent) = launch::pipe().unwrap();
    let (output_parent, output) = launch::pipe().unwrap();
    let ends = launch::Ends {
        input,
        output,
        stderr: launch::nul().unwrap(),
    };
    let (guard, log) = observing_guard(raw_ends(&ends), job_raw);
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let (process, thread) = launch::start(
        &executable,
        current_origin().as_ref(),
        &job,
        &ends,
        Some(&guard),
    )
    .unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Seen {
                event: "taken",
                inheritable: false,
                created: 0
            },
            Seen {
                event: "released",
                inheritable: false,
                created: 1
            },
        ]
    );
    assert!(all_closed(&ends));
    // SAFETY: the process handle is the one start returned; ending the suspended test child.
    unsafe { windows_sys::Win32::System::Threading::TerminateProcess(process.0, 1) };
    drop((thread, process, input_parent, output_parent));
}
#[test]
fn the_guard_is_released_after_the_window_closes_when_creation_fails() {
    let job = new_job();
    let (input, input_parent) = launch::pipe().unwrap();
    let (output_parent, output) = launch::pipe().unwrap();
    let ends = launch::Ends {
        input,
        output,
        stderr: launch::nul().unwrap(),
    };
    let (guard, log) = observing_guard(raw_ends(&ends), job.0 as usize);
    let missing = WorkerExecutable::new(system32("no such worker.exe"), [0; 32]).unwrap();
    let error = launch::start(
        &missing,
        current_origin().as_ref(),
        &job,
        &ends,
        Some(&guard),
    )
    .err()
    .expect("a missing executable must fail inside CreateProcessW");
    assert_eq!(error.kind(), crate::ErrorKind::WorkerUnavailable);
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Seen {
                event: "taken",
                inheritable: false,
                created: 0
            },
            Seen {
                event: "released",
                inheritable: false,
                created: 0
            },
        ],
        "taken once before the window, released once after it closed, on the failure path too"
    );
    drop((input_parent, output_parent));
}
#[test]
fn a_launch_refused_before_the_window_never_takes_the_guard() {
    struct Refused;
    impl crate::supervisor::ports::Origin for Refused {
        fn verify(&self) -> Result<(), crate::Error> {
            Err(crate::Error::new(crate::ErrorKind::IdentityMismatch))
        }
    }
    let job = new_job();
    let (ends, _parents) = ends();
    let (guard, log) = observing_guard(raw_ends(&ends), job.0 as usize);
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let error = launch::start(&executable, &Refused, &job, &ends, Some(&guard))
        .err()
        .unwrap();
    assert_eq!(error.kind(), crate::ErrorKind::IdentityMismatch);
    assert!(
        log.lock().unwrap().is_empty(),
        "no guard for a launch that never reaches the window"
    );
}
#[test]
fn a_creation_through_the_whole_launch_path_takes_the_guard_exactly_once() {
    let taken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = taken.clone();
    let guard = LaunchGuard::new(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let owned = launch::create_owned(&executable, current_origin(), Some(&guard)).unwrap();
    assert!(owned.contained);
    assert!(owned.process.terminate());
    assert_eq!(taken.load(std::sync::atomic::Ordering::SeqCst), 1);
}
