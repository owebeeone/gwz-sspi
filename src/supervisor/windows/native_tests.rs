use crate::supervisor::ports::{Child, ReadPort};
use crate::{AuthRequest, Identity, Package, SecretBytes, SecretText, TokenLimit};
use windows_sys::Win32::Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, GetProcessId, OpenProcess,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
fn executable() -> WorkerExecutable {
    let path = std::env::var_os("GWZ_SSPI_NATIVE_WORKER")
        .expect("opt-in fixture requires trusted absolute built worker path");
    WorkerExecutable::new(std::path::PathBuf::from(path), [0x42; 32]).unwrap()
}
fn launch_fixture() -> (launch::OwnedLaunch, Primary) {
    let primary = identity::primary().unwrap();
    let origin = identity::origin(std::sync::Arc::new(identity::primary().unwrap())).unwrap();
    let mut owned = launch::create_owned(&executable(), origin).unwrap();
    assert!(owned.contained);
    owned.process.resume().unwrap();
    let frame = crate::supervisor::io::read_frame(&mut owned.reader).unwrap();
    assert!(matches!(
        crate::protocol::supervision::reply(
            &frame,
            crate::supervisor::kernel::Expected::Bootstrap,
            &primary,
            &[0x42; 32],
            Package::Ntlm,
            TokenLimit::new(65536).unwrap()
        )
        .unwrap(),
        crate::protocol::supervision::Reply::Hello
    ));
    (owned, primary)
}
fn confirm_exit(process: &handles::Process) {
    // SAFETY: held actual child process; finite fixture wait, no PID inference.
    let waited = unsafe { WaitForSingleObject(process.process.0, 10000) };
    assert_eq!(waited, WAIT_OBJECT_0);
    let mut status = 0;
    // SAFETY: held completed process and initialized exit status output.
    let queried = unsafe { GetExitCodeProcess(process.process.0, &mut status) };
    assert_ne!(queried, 0);
    assert_eq!(status, 0);
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while process.observe() != (true, true) {
        assert!(std::time::Instant::now() < limit, "held Job did not drain");
        std::thread::yield_now();
    }
}
#[test]
#[ignore = "opt-in production worker EOF and native cleanup"]
fn worker_eof_before_begin_and_after_initial_token_closes_owned_pipes() {
    use crate::supervisor::ports::WritePort;
    for begin in [false, true] {
        let (mut owned, primary) = launch_fixture();
        if begin {
            let mut bytes = crate::secret::Storage::zeroed(53);
            bytes.as_mut()[..21].copy_from_slice(b"tls-server-end-point:");
            let request = AuthRequest {
                package: Package::Ntlm,
                target: SecretText::new("HTTP/localhost").unwrap(),
                identity: Identity::CurrentLogon,
                channel_binding: SecretBytes(bytes),
                token_limit: TokenLimit::new(65536).unwrap(),
                digest: None,
            };
            let frame = crate::protocol::supervision::begin(&request).unwrap();
            for bytes in [
                &(frame.as_slice().len() as u32).to_le_bytes()[..],
                frame.as_slice(),
            ] {
                let mut at = 0;
                while at < bytes.len() {
                    let count = owned.writer.write(&bytes[at..]).unwrap();
                    assert!(count > 0 && count <= bytes.len() - at);
                    at += count;
                }
            }
            let token = crate::supervisor::io::read_frame(&mut owned.reader).unwrap();
            assert!(matches!(
                crate::protocol::supervision::reply(
                    &token,
                    crate::supervisor::kernel::Expected::Begin,
                    &primary,
                    &[0x42; 32],
                    Package::Ntlm,
                    request.token_limit
                )
                .unwrap(),
                crate::protocol::supervision::Reply::Token { round: 1, .. }
            ));
        }
        drop(owned.writer);
        confirm_exit(&owned.process);
        let mut byte = crate::secret::Storage::zeroed(1);
        assert_eq!(owned.reader.read(byte.as_mut()).unwrap(), 0);
        drop(owned.reader);
        drop(owned.process);
    }
}
#[test]
#[ignore = "opt-in last Job handle closure containment"]
fn last_job_owner_drop_terminates_a_worker_with_held_process_observation() {
    let (owned, _) = launch_fixture();
    let mut raw = std::ptr::null_mut();
    // SAFETY: duplicate held worker process into independent noninherited owner
    // so actual exit can be observed after all launch owners (including Job) drop.
    let duplicated = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            owned.process.process.0,
            GetCurrentProcess(),
            &mut raw,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    assert_ne!(duplicated, 0);
    let held = handles::Handle::new(raw).unwrap();
    drop(owned);
    // SAFETY: held duplicated process, never relying on PID disappearance.
    assert_eq!(unsafe { WaitForSingleObject(held.0, 10000) }, WAIT_OBJECT_0);
}
#[test]
#[ignore = "private opt-in helper selected only by parent-loss fixture"]
fn parent_loss_child() {
    let Some(path) = std::env::var_os("GWZ_SSPI_NATIVE_COORD") else {
        return;
    };
    let (owned, _) = launch_fixture();
    // SAFETY: held worker process; number is nonsecret fixture coordination only.
    let pid = unsafe { GetProcessId(owned.process.process.0) };
    assert_ne!(pid, 0);
    std::fs::write(path, pid.to_string()).unwrap();
    loop {
        std::thread::park();
        std::hint::black_box(&owned);
    }
}
#[test]
#[ignore = "opt-in actual parent process loss and held worker exit"]
fn actual_parent_loss_closes_the_last_job_owner() {
    let scratch = std::path::PathBuf::from(
        std::env::var_os("GWZ_SSPI_NATIVE_SCRATCH")
            .expect("opt-in fixture requires external scratch directory"),
    );
    assert!(scratch.is_absolute() && scratch.is_dir());
    let filename = scratch.join(format!(
        "gwz-sspi-parent-loss-{}-{}.pid",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let name = concat!(module_path!(), "::parent_loss_child");
    let name = name.strip_prefix("gwz_sspi::").unwrap();
    let mut parent = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--ignored"])
        .env("GWZ_SSPI_NATIVE_COORD", &filename)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let pid = loop {
        if let Ok(text) = std::fs::read_to_string(&filename)
            && let Ok(pid) = text.parse::<u32>()
        {
            break pid;
        }
        if std::time::Instant::now() >= limit {
            let _ = parent.kill();
            let _ = parent.wait();
            panic!("parent-loss fixture did not publish held child identity");
        }
        std::thread::yield_now();
    };
    // SAFETY: parent helper stays alive until this test holds the worker process;
    // requesting only wait/query rights, no authentication or trust mutation.
    let held = handles::Handle::new(unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    })
    .unwrap();
    parent.kill().unwrap();
    parent.wait().unwrap();
    let _ = std::fs::remove_file(filename);
    // SAFETY: actual worker handle acquired before parent loss, not PID polling.
    assert_eq!(unsafe { WaitForSingleObject(held.0, 10000) }, WAIT_OBJECT_0);
}
