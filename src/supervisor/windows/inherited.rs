//! Adopt only the exact inherited byte-pipe endpoints of the early bootstrap.
use super::{
    handles::{Handle, Input, Output},
    identity,
};
use crate::supervisor::ports::{Primary, ReadPort, WritePort};
use crate::{Error, ErrorKind};
use windows_sys::Win32::Foundation::{
    GetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType};
use windows_sys::Win32::System::Pipes::{GetNamedPipeInfo, PIPE_CLIENT_END, PIPE_SERVER_END};
fn bad() -> Error {
    Error::new(ErrorKind::Protocol)
}
fn validate(raw: HANDLE, end: u32) -> Result<(), Error> {
    let mut inherit = 0;
    let mut flags = 0;
    // SAFETY: bootstrap numbers are only queried, never dereferenced. No handle
    // ownership is assumed until BOTH endpoints validate. Outputs initialized.
    let good = unsafe {
        GetFileType(raw) == FILE_TYPE_PIPE
            && GetHandleInformation(raw, &mut inherit) != 0
            && GetNamedPipeInfo(
                raw,
                &mut flags,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) != 0
    };
    if !good || inherit & HANDLE_FLAG_INHERIT == 0 || flags != end {
        return Err(bad());
    }
    Ok(())
}
pub(super) type Parts = (Primary, Box<dyn ReadPort>, Box<dyn WritePort>);
pub(super) fn adopt(input: usize, output: usize) -> Result<Parts, Error> {
    let input = input as HANDLE;
    let output = output as HANDLE;
    validate(input, PIPE_SERVER_END)?;
    validate(output, PIPE_CLIENT_END)?;
    let input = Handle::new(input)?;
    let output = Handle::new(output)?;
    for handle in [&input, &output] {
        // SAFETY: exclusively owned validated real handle; prevent onward
        // inheritance, retaining it through all synchronous child I/O calls.
        if unsafe { SetHandleInformation(handle.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(bad());
        }
    }
    identity::refuse_current_impersonation()?;
    let primary = identity::primary()?;
    Ok((primary, Box::new(Input(input)), Box::new(Output(output))))
}
