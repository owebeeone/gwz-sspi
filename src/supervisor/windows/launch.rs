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
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
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
fn pipe() -> Result<(Handle, Handle), Error> {
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    // SAFETY: initialized outputs and exactly sized attributes; handles wrapped
    // immediately after success, before any further fallible operation.
    if unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) } == 0 {
        return Err(containment());
    }
    let read = Handle::new(read)?;
    let write = Handle::new(write)?;
    Ok((read, write))
}
fn noninherit(handle: &Handle) -> Result<(), Error> {
    // SAFETY: held owned handle; clearing inheritance does not change lifetime.
    if unsafe { SetHandleInformation(handle.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(containment());
    }
    Ok(())
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
    noninherit(&parent_input)?;
    noninherit(&parent_output)?;
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let nul: Vec<u16> = "NUL\0".encode_utf16().collect();
    // SAFETY: fixed NUL device path, initialized inheritance attributes.
    let stderr = Handle::new(unsafe {
        CreateFileW(
            nul.as_ptr(),
            FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    })?;
    let mut attributes = Attributes::new(&job, [child_input.0, child_output.0, stderr.0])?;
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
            child_input.0 as usize, child_output.0 as usize
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
    startup.StartupInfo.hStdInput = child_input.0;
    startup.StartupInfo.hStdOutput = child_output.0;
    startup.StartupInfo.hStdError = stderr.0;
    startup.lpAttributeList = attributes.pointer();
    let mut process = PROCESS_INFORMATION::default();
    // Charged owner rechecks original-thread impersonation and current primary
    // immediately before creation. These synchronous metadata calls never poll.
    origin.verify()?;
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
    drop(attributes);
    drop(child_input);
    drop(child_output);
    drop(stderr);
    if created == 0 {
        return Err(Error::new(ErrorKind::WorkerUnavailable));
    }
    // Successful CreateProcess guarantees both real handles; take ownership
    // without any intervening fallible path that could lose the suspended child.
    let process_handle = Handle(process.hProcess);
    let thread_handle = Handle(process.hThread);
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
