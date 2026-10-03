use super::super::super::ports::{Child, ReadPort, WritePort};
use crate::{Error, ErrorKind};
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::JobObjects::{
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation,
    QueryInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{ResumeThread, TerminateProcess, WaitForSingleObject};
pub(super) struct Handle(pub(super) HANDLE);
// SAFETY: these are owned real kernel handles, never pseudo handles; kernel APIs
// support cross-thread use. Owners retain them until all calls and joins finish.
unsafe impl Send for Handle {}
// SAFETY: shared operations never close a handle; closure requires exclusive Drop.
unsafe impl Sync for Handle {}
impl Handle {
    pub(super) fn new(raw: HANDLE) -> Result<Self, Error> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(Error::new(ErrorKind::ContainmentFailed));
        }
        Ok(Self(raw))
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: sole owner of this valid real handle, with no call in flight.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(super) struct Process {
    pub(super) process: Handle,
    pub(super) thread: Handle,
    pub(super) job: Handle,
}
impl Child for Process {
    fn resume(&self) -> Result<(), Error> {
        // SAFETY: held primary thread of the suspended child.
        if unsafe { ResumeThread(self.thread.0) } == u32::MAX {
            return Err(Error::new(ErrorKind::ContainmentFailed));
        }
        Ok(())
    }
    fn terminate(&self) -> bool {
        // SAFETY: held Job and process, neither can be recycled during these calls.
        // TerminateProcess also contains a suspended child whose Job check failed.
        unsafe {
            let job = TerminateJobObject(self.job.0, 2);
            let process = TerminateProcess(self.process.0, 2);
            job != 0 || process != 0
        }
    }
    fn observe(&self) -> (bool, bool) {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: held process/Job; output is initialized and exactly sized.
        unsafe {
            let exited = WaitForSingleObject(self.process.0, 0) == WAIT_OBJECT_0;
            let queried = QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of_val(&accounting) as u32,
                std::ptr::null_mut(),
            ) != 0;
            (exited, queried && accounting.ActiveProcesses == 0)
        }
    }
    fn cancel_io(&self, thread: &JoinHandle<()>) {
        // SAFETY: JoinHandle is held until actual thread completion. This request
        // is advisory; buffers/pipe handles remain in that thread until it returns.
        unsafe {
            CancelSynchronousIo(thread.as_raw_handle());
        }
    }
}
pub(super) struct Input(pub(super) Handle);
pub(super) struct Output(pub(super) Handle);
impl ReadPort for Input {
    fn read(&mut self, bytes: &mut [u8]) -> Result<usize, Error> {
        let mut count = 0;
        // SAFETY: fixed initialized frame slice remains exclusively held through
        // the synchronous call, including advisory cancellation and late return.
        let success = unsafe {
            ReadFile(
                self.0.0,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if success == 0 {
            return Err(Error::new(ErrorKind::Protocol));
        }
        Ok(count as usize)
    }
}
impl WritePort for Output {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let mut count = 0;
        // SAFETY: held immutable fixed frame slice and pipe outlive this call.
        let success = unsafe {
            WriteFile(
                self.0.0,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut count,
                std::ptr::null_mut(),
            )
        };
        if success == 0 {
            return Err(Error::new(ErrorKind::Protocol));
        }
        Ok(count as usize)
    }
}
pub(super) fn child(process: Handle, thread: Handle, job: Handle) -> Arc<dyn Child> {
    Arc::new(Process {
        process,
        thread,
        job,
    })
}
