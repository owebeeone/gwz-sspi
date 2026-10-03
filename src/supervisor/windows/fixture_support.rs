use crate::supervisor::fixture_cleanup::{Cleanup, Guard, Receipt, Report};
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Foundation::{HANDLE, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::TerminateProcess;
fn receipt() -> Receipt {
    std::sync::Arc::new(std::sync::Mutex::new(None))
}
struct Helper(std::process::Child);
impl Cleanup for Helper {
    fn cleanup(&mut self) -> bool {
        let _requested = self.0.kill();
        // SAFETY: std Child retains this actual process handle until completion.
        let observed = unsafe { WaitForSingleObject(self.0.as_raw_handle() as HANDLE, 10000) }
            == WAIT_OBJECT_0;
        // Consume wait only after actual finite held-handle exit observation.
        observed && self.0.wait().is_ok()
    }
}
struct Scratch {
    path: std::path::PathBuf,
    owned: bool,
}
impl Scratch {
    fn create() -> Self {
        let root = std::path::PathBuf::from(
            std::env::var_os("GWZ_SSPI_NATIVE_SCRATCH").expect("external fixture scratch required"),
        );
        assert!(root.is_absolute() && root.is_dir());
        let path = root.join(format!(
            "gwz-sspi-parent-loss-{}-{}.pid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        Self { path, owned: true }
    }
}
impl Cleanup for Scratch {
    fn cleanup(&mut self) -> bool {
        if !self.owned {
            return true;
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => {
                self.owned = false;
                true
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.owned = false;
                true
            }
            Err(_) => false,
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
struct Empty;
impl Cleanup for Empty {
    fn cleanup(&mut self) -> bool {
        true
    }
}
struct Observer(handles::Handle);
impl Cleanup for Observer {
    fn cleanup(&mut self) -> bool {
        // SAFETY: held worker process, used only by this opt-in fixture owner.
        unsafe {
            TerminateProcess(self.0.0, 2);
        }
        // SAFETY: the same independently held process; kill request is not proof.
        (unsafe { WaitForSingleObject(self.0.0, 10000) }) == WAIT_OBJECT_0
    }
}
fn duplicate(raw: HANDLE) -> handles::Handle {
    let mut result = std::ptr::null_mut();
    // SAFETY: duplicate a retained real process handle for independent observation.
    let good = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            GetCurrentProcess(),
            &mut result,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    assert_ne!(good, 0);
    handles::Handle::new(result).unwrap()
}
struct Suspended(Option<launch::OwnedLaunch>);
impl Cleanup for Suspended {
    fn cleanup(&mut self) -> bool {
        let Some(owned) = self.0.as_ref() else {
            return true;
        };
        owned.process.terminate();
        // SAFETY: held fixture process retained through finite exit observation.
        let exited =
            unsafe { WaitForSingleObject(owned.process.process.0, 10000) } == WAIT_OBJECT_0;
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while exited && owned.process.observe() != (true, true) && std::time::Instant::now() < limit
        {
            std::thread::yield_now();
        }
        exited && owned.process.observe() == (true, true)
    }
}
fn suspended(no_kill: bool) -> Guard<Suspended, Empty> {
    let proof = receipt();
    let origin = identity::origin(std::sync::Arc::new(identity::primary().unwrap())).unwrap();
    let owned = if no_kill {
        // SAFETY: fixture-only unnamed Job without kill flag; never production policy.
        let job = handles::Handle::new(unsafe {
            windows_sys::Win32::System::JobObjects::CreateJobObjectW(
                std::ptr::null(),
                std::ptr::null(),
            )
        })
        .unwrap();
        launch::create_in_job(&executable(), origin, job).unwrap()
    } else {
        launch::create_owned(&executable(), origin).unwrap()
    };
    let owner = Guard::new(Suspended(Some(owned)), Empty, proof);
    assert!(owner.owner.0.as_ref().unwrap().contained);
    owner
}
fn helper_start(scratch: Scratch, proof: Receipt) -> Guard<Helper, Scratch> {
    let name = concat!(module_path!(), "::parent_loss_child");
    let name = name.strip_prefix("gwz_sspi::").unwrap();
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--ignored"])
        .env("GWZ_SSPI_NATIVE_COORD", &scratch.path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    // Infallible ownership transfer immediately after spawn, before observation.
    Guard::new(Helper(child), scratch, proof)
}
fn published(path: &std::path::Path) -> u32 {
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && let Ok(pid) = text.parse::<u32>()
        {
            return pid;
        }
        assert!(
            std::time::Instant::now() < limit,
            "helper did not publish suspended child identity"
        );
        std::thread::yield_now();
    }
}
fn open(pid: u32) -> handles::Handle {
    // SAFETY: helper remains held/alive while fixture obtains actual worker observer.
    let raw = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE
                | PROCESS_QUERY_LIMITED_INFORMATION
                | windows_sys::Win32::System::Threading::PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    handles::Handle::new(raw).unwrap()
}
