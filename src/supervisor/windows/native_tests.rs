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
include!("fixture_support.rs");
include!("containment_tests.rs");
