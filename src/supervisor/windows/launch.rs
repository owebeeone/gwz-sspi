//! Exact executable, private anonymous pipes and creation-time Job membership.
use super::super::super::{
    ports::{Launched, Origin},
    values::WorkerExecutable,
};
use super::{
    attributes::Attributes,
    handles::{Handle, Input, Output, Process},
};
use crate::{Error, ErrorKind};
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    CreateJobObjectW, IsProcessInJob, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, GetWindowsDirectoryW};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
    EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};
fn containment() -> Error {
    Error::new(ErrorKind::ContainmentFailed)
}
pub(super) fn pipe() -> Result<(Handle, Handle), Error> {
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    // SAFETY: initialized outputs and null attributes, which make both ends
    // noninheritable; handles wrapped immediately after success, before any
    // further fallible operation.
    if unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) } == 0 {
        return Err(containment());
    }
    let read = Handle::new(read)?;
    let write = Handle::new(write)?;
    Ok((read, write))
}
fn set_inheritable(handle: &Handle, inheritable: bool) -> Result<(), Error> {
    let flag = if inheritable { HANDLE_FLAG_INHERIT } else { 0 };
    // SAFETY: held owned handle; changing inheritance does not change lifetime.
    if unsafe { SetHandleInformation(handle.0, HANDLE_FLAG_INHERIT, flag) } == 0 {
        return Err(containment());
    }
    Ok(())
}
pub(super) fn nul() -> Result<Handle, Error> {
    let nul: Vec<u16> = "NUL\0".encode_utf16().collect();
    // SAFETY: fixed NUL device path; null attributes make the handle noninheritable.
    Handle::new(unsafe {
        CreateFileW(
            nul.as_ptr(),
            FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    })
}
fn roots() -> Result<(Vec<u16>, Vec<u16>), Error> {
    let mut windows = vec![0u16; 32768];
    let mut system = vec![0u16; 32768];
    // SAFETY: initialized fixed output allocations, capacities match call sizes.
    let (win_len, sys_len) = unsafe {
        (
            GetWindowsDirectoryW(windows.as_mut_ptr(), windows.len() as u32),
            GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32),
        )
    };
    if win_len == 0
        || win_len as usize >= windows.len()
        || sys_len == 0
        || sys_len as usize >= system.len()
    {
        return Err(containment());
    }
    windows.truncate(win_len as usize);
    system.truncate(sys_len as usize);
    Ok((windows, system))
}
fn environment(windows: &[u16], system: &[u16]) -> Vec<u16> {
    let mut result = Vec::new();
    for (key, value) in [
        ("PATH=", system),
        ("SystemRoot=", windows),
        ("WINDIR=", windows),
    ] {
        result.extend(key.encode_utf16());
        result.extend_from_slice(value);
        result.push(0);
    }
    result.push(0);
    result
}
/// The worker's ends of its pipes, and its stderr. Not inheritable until an
/// [`InheritWindow`] opens around `CreateProcessW`.
pub(super) struct Ends {
    pub(super) input: Handle,
    pub(super) output: Handle,
    pub(super) stderr: Handle,
}
/// The creation window: the worker's ends are inheritable from [`InheritWindow::open`]
/// to its drop and at no other time. It spans the `CreateProcessW` call and closes on
/// every path, a failed call included. During it, a concurrent spawn that names no
/// handle list (std `Command`, the git CLI) still inherits the ends; only a host that
/// serializes such spawns with this launch closes that.
pub(super) struct InheritWindow<'a> {
    ends: &'a Ends,
}
impl<'a> InheritWindow<'a> {
    pub(super) fn open(ends: &'a Ends) -> Result<Self, Error> {
        // The guard exists before the first end opens, so a failure part way
        // through closes the ends already opened.
        let window = Self { ends };
        for end in [&ends.input, &ends.output, &ends.stderr] {
            set_inheritable(end, true)?;
        }
        Ok(window)
    }
}
impl Drop for InheritWindow<'_> {
    fn drop(&mut self) {
        for end in [&self.ends.input, &self.ends.output, &self.ends.stderr] {
            // Nothing can be done about a failure here; the handle is closed next.
            let _ = set_inheritable(end, false);
        }
    }
}
/// Creates the suspended worker in `job`. The ends are inheritable only inside this call.
pub(super) fn start(
    executable: &WorkerExecutable,
    origin: &dyn Origin,
    job: &Handle,
    ends: &Ends,
) -> Result<(Handle, Handle), Error> {
    let mut attributes = Attributes::new(job, [ends.input.0, ends.output.0, ends.stderr.0])?;
    let application: Vec<u16> = executable
        .path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    if application[..application.len() - 1].contains(&0)
        || application[..application.len() - 1].contains(&(b'"' as u16))
    {
        return Err(Error::new(ErrorKind::WorkerUnavailable));
    }
    // Bootstrap contains only trusted path and numeric pipe handles, no secret.
    let mut command = vec![b'"' as u16];
    command.extend_from_slice(&application[..application.len() - 1]);
    command.push(b'"' as u16);
    command.extend(
        format!(
            " --gwz-sspi-worker {} {}",
            ends.input.0 as usize, ends.output.0 as usize
        )
        .encode_utf16(),
    );
    command.push(0);
    let (windows, mut directory) = roots()?;
    let environment = environment(&windows, &directory);
    directory.push(0);
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = ends.input.0;
    startup.StartupInfo.hStdOutput = ends.output.0;
    startup.StartupInfo.hStdError = ends.stderr.0;
    startup.lpAttributeList = attributes.pointer();
    let mut process = PROCESS_INFORMATION::default();
    // Charged owner rechecks original-thread impersonation and current primary
    // immediately before creation. These synchronous metadata calls never poll.
    origin.verify()?;
    let window = InheritWindow::open(ends)?;
    // SAFETY: all pointer storage and attribute references remain alive through
    // this call and Delete. Exact application, double-NUL environment and cwd.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    // The window closes before anything else is done, created or not.
    drop(window);
    drop(attributes);
    if created == 0 {
        return Err(Error::new(ErrorKind::WorkerUnavailable));
    }
    // Successful CreateProcess guarantees both real handles; take ownership
    // without any intervening fallible path that could lose the suspended child.
    Ok((Handle(process.hProcess), Handle(process.hThread)))
}
pub(super) struct OwnedLaunch {
    pub(super) process: Process,
    pub(super) reader: Input,
    pub(super) writer: Output,
    pub(super) contained: bool,
}
pub(super) fn create(
    executable: &WorkerExecutable,
    origin: Box<dyn Origin>,
) -> Result<Launched, Error> {
    let result = create_owned(executable, origin)?;
    Ok(Launched {
        child: std::sync::Arc::new(result.process),
        reader: Box::new(result.reader),
        writer: Box::new(result.writer),
        contained: result.contained,
    })
}
pub(super) fn create_owned(
    executable: &WorkerExecutable,
    origin: Box<dyn Origin>,
) -> Result<OwnedLaunch, Error> {
    // SAFETY: unnamed noninheritable Job; sole owned handle returned on success.
    let job = Handle::new(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: initialized correctly sized limits; no breakaway flag is set.
    if unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(containment());
    }
    create_in_job(executable, origin, job)
}
// Production always supplies the configured kill-on-close Job above. Test-only
// fixtures may supply their independently owned no-kill Job as a negative control.
pub(super) fn create_in_job(
    executable: &WorkerExecutable,
    origin: Box<dyn Origin>,
    job: Handle,
) -> Result<OwnedLaunch, Error> {
    let (child_input, parent_input) = pipe()?;
    let (parent_output, child_output) = pipe()?;
    let ends = Ends {
        input: child_input,
        output: child_output,
        stderr: nul()?,
    };
    let (process_handle, thread_handle) = start(executable, origin.as_ref(), &job, &ends)?;
    // The worker has its own copies of the child ends.
    drop(ends);
    let mut member = 0;
    // SAFETY: held process/Job; check occurs before resume, with no fallback.
    let contained =
        unsafe { IsProcessInJob(process_handle.0, job.0, &mut member) } != 0 && member != 0;
    Ok(OwnedLaunch {
        process: Process {
            process: process_handle,
            thread: thread_handle,
            job,
        },
        reader: Input(parent_output),
        writer: Output(parent_input),
        contained,
    })
}
