//! UpdateProcThreadAttribute retains references until Delete, not just until the
//! CreateProcess call. Box allocations keep those references stable when moved.
use super::handles::Handle;
use crate::{Error, ErrorKind};
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::{
    DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_JOB_LIST, UpdateProcThreadAttribute,
};
pub(super) struct Attributes {
    storage: Box<[usize]>,
    jobs: Box<[HANDLE; 1]>,
    handles: Box<[HANDLE; 3]>,
    initialized: bool,
}
impl Attributes {
    pub(super) fn new(job: &Handle, handles: [HANDLE; 3]) -> Result<Self, Error> {
        let mut size = 0;
        // SAFETY: sizing probe, null output and initialized size.
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut size);
        }
        if size == 0 || size > 65536 {
            return Err(Error::new(ErrorKind::ContainmentFailed));
        }
        let mut result = Self {
            storage: vec![0usize; size.div_ceil(std::mem::size_of::<usize>())].into_boxed_slice(),
            jobs: Box::new([job.0]),
            handles: Box::new(handles),
            initialized: false,
        };
        // SAFETY: initialized aligned allocation is fixed and at least size bytes.
        if unsafe { InitializeProcThreadAttributeList(result.pointer(), 2, 0, &mut size) } == 0 {
            return Err(Error::new(ErrorKind::ContainmentFailed));
        }
        result.initialized = true;
        // SAFETY: both fixed boxed arrays remain live and immovable until Delete.
        // No creation/assignment fallback exists if this supported feature fails.
        let success = unsafe {
            UpdateProcThreadAttribute(
                result.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                result.jobs.as_ptr().cast(),
                std::mem::size_of_val(result.jobs.as_ref()),
                std::ptr::null_mut(),
                std::ptr::null(),
            ) != 0
                && UpdateProcThreadAttribute(
                    result.pointer(),
                    0,
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                    result.handles.as_ptr().cast(),
                    std::mem::size_of_val(result.handles.as_ref()),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                ) != 0
        };
        if !success {
            return Err(Error::new(ErrorKind::ContainmentFailed));
        }
        Ok(result)
    }
    pub(super) fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: attribute allocation and both referenced arrays are still
            // live; field destructors execute only after this Drop returns.
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}
