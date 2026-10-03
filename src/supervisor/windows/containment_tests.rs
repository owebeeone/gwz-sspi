#[test]
#[ignore = "opt-in still-suspended production child and no-kill runtime control"]
fn last_job_owner_drop_terminates_a_worker_with_held_process_observation() {
    for no_kill in [false, true] {
        let mut owner = suspended(no_kill);
        let raw = owner.owner.0.as_ref().unwrap().process.process.0;
        let proof = receipt();
        let mut observer = Guard::new(Observer(duplicate(raw)), Empty, proof.clone());
        // Creation is still suspended: neither bootstrap nor clean EOF can execute.
        drop(owner.owner.0.take());
        drop(owner);
        // SAFETY: independent actual worker handle retained across Job closure.
        let observed =
            unsafe { WaitForSingleObject(observer.owner.0.0, if no_kill { 250 } else { 10000 }) };
        assert_eq!(observed, if no_kill { WAIT_TIMEOUT } else { WAIT_OBJECT_0 });
        assert!(
            observer.settle().helper_confirmed,
            "negative-control fallback cleanup not confirmed"
        );
        drop(observer);
        assert!(proof.lock().unwrap().unwrap().helper_confirmed);
    }
}
#[test]
#[ignore = "private opt-in suspended helper selected only by parent-loss fixture"]
fn parent_loss_child() {
    use std::io::Read;
    let Some(path) = std::env::var_os("GWZ_SSPI_NATIVE_COORD") else {
        return;
    };
    let mut gate = [0u8; 1];
    std::io::stdin().read_exact(&mut gate).unwrap();
    assert_eq!(gate, [b'x']);
    let owner = suspended(false);
    // SAFETY: still-suspended held process. PID is only nonsecret coordination;
    // root obtains an independent process handle before terminating this helper.
    let pid = unsafe { GetProcessId(owner.owner.0.as_ref().unwrap().process.process.0) };
    assert_ne!(pid, 0);
    std::fs::write(path, pid.to_string()).unwrap();
    loop {
        std::thread::park();
        std::hint::black_box(&owner);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailurePoint {
    None,
    AfterSpawn,
    AfterPid,
    Observer,
    AfterObserver,
}
fn parent_loss_case(point: FailurePoint) {
    use std::io::Write;
    let scratch = Scratch::create();
    let path = scratch.path.clone();
    let proof = receipt();
    let mut helper = helper_start(scratch, proof.clone());
    let held_helper = duplicate(helper.owner.0.as_raw_handle() as HANDLE);
    let mut worker = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Guard was attached immediately after spawn. Helper cannot create any
        // worker before this controller sends its explicit nonsecret gate byte.
        if point == FailurePoint::AfterSpawn {
            panic!("forced after spawn");
        }
        helper
            .owner
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"x")
            .unwrap();
        let pid = published(&path);
        // Independent verification owner survives deliberate observer failure.
        worker = Some(Guard::new(Observer(open(pid)), Empty, receipt()));
        if point == FailurePoint::AfterPid {
            panic!("forced after PID publication");
        }
        let observer = if point == FailurePoint::Observer {
            open(0)
        } else {
            open(pid)
        };
        if point == FailurePoint::AfterObserver {
            panic!("forced after observation acquisition");
        }
        assert!(
            helper.settle().helper_confirmed,
            "helper cleanup unconfirmed"
        );
        // SAFETY: held suspended process, observed after actual helper exit.
        let observed = unsafe { WaitForSingleObject(observer.0, 10000) };
        assert_eq!(observed, WAIT_OBJECT_0);
    }));
    // Drop even on failed observation/unwind, then assert actual observations
    // outside the catch. A failed kill/wait is explicitly reported, never proof.
    drop(helper);
    assert_eq!(
        result.is_err(),
        point != FailurePoint::None,
        "point={point:?}"
    );
    // SAFETY: independent helper observer remains held through cleanup/unwind.
    let observed = unsafe { WaitForSingleObject(held_helper.0, 0) };
    assert_eq!(observed, WAIT_OBJECT_0);
    assert_eq!(
        *proof.lock().unwrap(),
        Some(Report {
            helper_confirmed: true,
            scratch_removed: true
        })
    );
    assert!(!path.exists());
    if let Some(mut held) = worker {
        // SAFETY: independent worker observer acquired before forced failure.
        let observed = unsafe { WaitForSingleObject(held.owner.0.0, 10000) };
        assert_eq!(observed, WAIT_OBJECT_0);
        assert!(held.settle().helper_confirmed);
        drop(held);
    }
}
#[test]
#[ignore = "opt-in actual parent death while production child remains suspended"]
fn actual_parent_loss_closes_the_last_job_owner() {
    parent_loss_case(FailurePoint::None);
}
#[test]
#[ignore = "opt-in helper ownership failures after spawn/PID/observation"]
fn helper_failures_cleanup_on_every_intermediate_unwind() {
    for point in [
        FailurePoint::AfterSpawn,
        FailurePoint::AfterPid,
        FailurePoint::Observer,
        FailurePoint::AfterObserver,
    ] {
        parent_loss_case(point);
    }
}
