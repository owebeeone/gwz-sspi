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
    let error = launch::start(
        &missing,
        origin.as_ref(),
        &job,
        &ends,
        None,
        &AtomicBool::new(false),
        &mut None,
    )
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
    let owned = launch::create_owned(&executable, origin, None, &AtomicBool::new(false)).unwrap();
    assert!(owned.contained);
    assert!(owned.process.terminate());
}

// The host's launch guard (0.1.2) is held across the creation: taken before the ends turn inheritable, released only
// after CreateProcessW returned and the child's copies of the ends are closed. The rows watch the operating system's
// state (how many processes the Job holds, whether the pipe's write end is closed) from inside the guard callback and
// from inside the drop of the value it returns, so they observe the ordering rather than mirror the code.
use crate::supervisor::values::LaunchGuard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows_sys::Win32::System::JobObjects::{
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
    QueryInformationJobObject,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Seen {
    event: &'static str,
    /// How many processes the Job holds: 1 once CreateProcessW has created the worker.
    created: u32,
    /// Whether any of the worker's three ends is still open and inheritable: not when the guard is taken (the window
    /// has not opened) and not when it is released (the window has closed and the ends are closed).
    inheritable: bool,
    /// Whether the write end of the worker's stdout pipe has been closed by every holder, which a parent end sees as a
    /// broken pipe. (A created worker holds its own copy, so this is the failed creations' observation.)
    output_closed: bool,
}
type Log = Arc<Mutex<Vec<Seen>>>;
fn job_count(job: usize) -> u32 {
    use windows_sys::Win32::Foundation::HANDLE;
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
    accounting.ActiveProcesses
}
fn output_closed(parent_output: usize) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_BROKEN_PIPE, GetLastError, HANDLE};
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;
    let nothing = std::ptr::null_mut::<u32>();
    // SAFETY: the parent's read end is open and owned by the test for the whole launch.
    let peeked = unsafe {
        PeekNamedPipe(
            parent_output as HANDLE,
            std::ptr::null_mut(),
            0,
            nothing,
            nothing,
            nothing,
        )
    };
    // SAFETY: the immediate error query for PeekNamedPipe.
    peeked == 0 && unsafe { GetLastError() } == ERROR_BROKEN_PIPE
}
fn ends_inheritable(ends: [usize; 3]) -> bool {
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};
    ends.iter().any(|end| {
        let mut flags = 0;
        // SAFETY: a value that is no open handle only makes the call fail, which is a closed end.
        let open = unsafe { GetHandleInformation(*end as HANDLE, &mut flags) } != 0;
        open && flags & HANDLE_FLAG_INHERIT != 0
    })
}
fn seen(event: &'static str, job: usize, parent_output: usize, ends: [usize; 3]) -> Seen {
    Seen {
        inheritable: ends_inheritable(ends),
        event,
        created: job_count(job),
        output_closed: output_closed(parent_output),
    }
}
fn observing_guard(job: usize, parent_output: usize, ends: [usize; 3]) -> (LaunchGuard, Log) {
    struct Released(Log, usize, usize, [usize; 3]);
    impl Drop for Released {
        fn drop(&mut self) {
            let state = seen("released", self.1, self.2, self.3);
            self.0.lock().unwrap().push(state);
        }
    }
    let log: Log = Arc::new(Mutex::new(Vec::new()));
    let taken = log.clone();
    let guard = LaunchGuard::new(move || {
        let state = seen("taken", job, parent_output, ends);
        taken.lock().unwrap().push(state);
        Released(taken.clone(), job, parent_output, ends)
    });
    (guard, log)
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
/// The worker's ends, and the parent's: the test keeps the parents' so that the pipe stays open for it.
struct Launch {
    ends: launch::Ends,
    _input_parent: handles::Handle,
    output_parent: handles::Handle,
}
impl Launch {
    /// The worker's three ends as raw values, for the guard to look at; the ends themselves move into the launch.
    fn raw_ends(&self) -> [usize; 3] {
        [
            self.ends.input.0 as usize,
            self.ends.output.0 as usize,
            self.ends.stderr.0 as usize,
        ]
    }
}
fn launch_ends() -> Launch {
    let (input, input_parent) = launch::pipe().unwrap();
    let (output_parent, output) = launch::pipe().unwrap();
    Launch {
        ends: launch::Ends {
            input,
            output,
            stderr: launch::nul().unwrap(),
        },
        _input_parent: input_parent,
        output_parent,
    }
}
/// An origin that verifies a number of times and then refuses, as a caller that changed while the guard blocked does.
struct Changing(std::sync::atomic::AtomicUsize, usize);
impl crate::supervisor::ports::Origin for Changing {
    fn verify(&self) -> Result<(), crate::Error> {
        if self.0.fetch_add(1, Ordering::SeqCst) < self.1 {
            Ok(())
        } else {
            Err(crate::Error::new(crate::ErrorKind::IdentityMismatch))
        }
    }
}
#[test]
fn the_guard_spans_the_creation_of_the_worker() {
    // A real creation (the child is suspended and never runs). `created` is 0 when the guard is taken and 1 when it
    // is released, so the guard is held across CreateProcessW.
    let job = new_job();
    let launch = launch_ends();
    let (guard, log) = observing_guard(
        job.0 as usize,
        launch.output_parent.0 as usize,
        launch.raw_ends(),
    );
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let (process, thread) = launch::start_and_close(
        &executable,
        current_origin().as_ref(),
        &job,
        launch.ends,
        Some(&guard),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Seen {
                event: "taken",
                created: 0,
                inheritable: false,
                output_closed: false
            },
            Seen {
                event: "released",
                created: 1,
                inheritable: false,
                output_closed: false
            },
        ]
    );
    // SAFETY: the process handle is the one start returned; ending the suspended test child.
    unsafe { windows_sys::Win32::System::Threading::TerminateProcess(process.0, 1) };
    drop((thread, process));
}
#[test]
fn the_guard_is_released_only_after_the_child_ends_are_closed_when_creation_fails() {
    // No worker, so nothing else holds the write end of the stdout pipe: the parent sees it broken exactly when the
    // child's copy is closed, and the guard must still be held until then. (Ends closed before the guard and not
    // after: the guard is released with the pipe still open.)
    let job = new_job();
    let launch = launch_ends();
    let (guard, log) = observing_guard(
        job.0 as usize,
        launch.output_parent.0 as usize,
        launch.raw_ends(),
    );
    let missing = WorkerExecutable::new(system32("no such worker.exe"), [0; 32]).unwrap();
    let error = launch::start_and_close(
        &missing,
        current_origin().as_ref(),
        &job,
        launch.ends,
        Some(&guard),
        &AtomicBool::new(false),
    )
    .err()
    .expect("a missing executable must fail inside CreateProcessW");
    assert_eq!(error.kind(), crate::ErrorKind::WorkerUnavailable);
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Seen {
                event: "taken",
                created: 0,
                inheritable: false,
                output_closed: false
            },
            Seen {
                event: "released",
                created: 0,
                inheritable: false,
                output_closed: true
            },
        ],
        "taken once before the window, released once after the child's ends were closed"
    );
}
#[test]
fn a_launch_refused_before_the_window_never_takes_the_guard() {
    let job = new_job();
    let launch = launch_ends();
    let (guard, log) = observing_guard(
        job.0 as usize,
        launch.output_parent.0 as usize,
        launch.raw_ends(),
    );
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let error = launch::start_and_close(
        &executable,
        &Changing(std::sync::atomic::AtomicUsize::new(0), 0),
        &job,
        launch.ends,
        Some(&guard),
        &AtomicBool::new(false),
    )
    .err()
    .unwrap();
    assert_eq!(error.kind(), crate::ErrorKind::IdentityMismatch);
    assert!(
        log.lock().unwrap().is_empty(),
        "no guard for a launch that never reaches the window"
    );
}
#[test]
fn a_caller_that_changes_while_the_guard_is_held_creates_no_worker() {
    // The wait for the guard has no bound. The caller verifies once before it and not after, as it did in a first
    // draft: the worker would be created for a caller that is no longer the one that asked.
    let job = new_job();
    let launch = launch_ends();
    let (guard, log) = observing_guard(
        job.0 as usize,
        launch.output_parent.0 as usize,
        launch.raw_ends(),
    );
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let error = launch::start_and_close(
        &executable,
        &Changing(std::sync::atomic::AtomicUsize::new(0), 1),
        &job,
        launch.ends,
        Some(&guard),
        &AtomicBool::new(false),
    )
    .err()
    .expect("the second verification must refuse");
    assert_eq!(error.kind(), crate::ErrorKind::IdentityMismatch);
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            Seen {
                event: "taken",
                created: 0,
                inheritable: false,
                output_closed: false
            },
            Seen {
                event: "released",
                created: 0,
                inheritable: false,
                output_closed: true
            },
        ]
    );
    assert_eq!(job_count(job.0 as usize), 0, "no worker was created");
}
#[test]
fn a_record_that_ends_while_the_guard_is_held_creates_no_worker() {
    // The deadline (or a cancellation) falls while the guard blocks: the record's stop flag is set when the guard
    // returns, and the launch must not create the worker.
    let job = new_job();
    let launch = launch_ends();
    let stop = Arc::new(AtomicBool::new(false));
    let (inner, log) = observing_guard(
        job.0 as usize,
        launch.output_parent.0 as usize,
        launch.raw_ends(),
    );
    let blocked = stop.clone();
    let guard = LaunchGuard::new(move || {
        let held = inner.hold();
        blocked.store(true, Ordering::Release);
        held
    });
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let error = launch::start_and_close(
        &executable,
        current_origin().as_ref(),
        &job,
        launch.ends,
        Some(&guard),
        &stop,
    )
    .err()
    .expect("a record that ended must create nothing");
    assert_eq!(error.kind(), crate::ErrorKind::Cancelled);
    assert_eq!(log.lock().unwrap().len(), 2, "taken and released");
    assert_eq!(job_count(job.0 as usize), 0, "no worker was created");
}
#[test]
fn a_launch_without_a_guard_does_not_look_at_the_record_again() {
    // 0.1.1's launch: no guard, no second look (the monitor checked the record immediately before).
    let job = new_job();
    let launch = launch_ends();
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let (process, thread) = launch::start_and_close(
        &executable,
        current_origin().as_ref(),
        &job,
        launch.ends,
        None,
        &AtomicBool::new(true),
    )
    .unwrap();
    // SAFETY: the process handle is the one start returned; ending the suspended test child.
    unsafe { windows_sys::Win32::System::Threading::TerminateProcess(process.0, 1) };
    drop((thread, process));
}
#[test]
fn a_creation_through_the_whole_launch_path_takes_the_guard_exactly_once() {
    let taken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = taken.clone();
    let guard = LaunchGuard::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    let executable = WorkerExecutable::new(system32("cmd.exe"), [0; 32]).unwrap();
    let owned = launch::create_owned(
        &executable,
        current_origin(),
        Some(&guard),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(owned.contained);
    assert!(owned.process.terminate());
    assert_eq!(taken.load(Ordering::SeqCst), 1);
}
