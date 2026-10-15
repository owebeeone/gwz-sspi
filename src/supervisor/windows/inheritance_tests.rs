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
    let error = launch::start(&missing, origin.as_ref(), &job, &ends)
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
    let owned = launch::create_owned(&executable, origin).unwrap();
    assert!(owned.contained);
    assert!(owned.process.terminate());
}
