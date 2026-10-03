//! Explicit provider allocation and handle owners; cleanup success is checked.
use super::super::super::native::Output;
use crate::{Error, ErrorKind};
use std::ffi::c_void;
use windows_sys::Win32::Security::Authentication::Identity::{
    FreeContextBuffer, SECBUFFER_TOKEN, SECBUFFER_VERSION, SecBuffer, SecBufferDesc,
};
use zeroize::Zeroize;
pub(super) struct Buffer {
    pub(super) descriptor: SecBuffer,
    attempted: bool,
    extent: usize,
    pub(super) probe: crate::secret::audit::Probe,
}
impl Buffer {
    pub(super) fn empty() -> Self {
        Self {
            descriptor: SecBuffer {
                cbBuffer: 0,
                BufferType: SECBUFFER_TOKEN,
                pvBuffer: std::ptr::null_mut(),
            },
            attempted: false,
            extent: 0,
            probe: crate::secret::audit::Probe::default(),
        }
    }
    pub(super) fn desc(&mut self) -> SecBufferDesc {
        SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut self.descriptor,
        }
    }

    pub(super) fn capture_extent(&mut self) {
        self.extent = self.descriptor.cbBuffer as usize;
    }
    pub(super) fn check_completion(&mut self, before: SecBuffer) -> Result<(), Error> {
        if self.descriptor.pvBuffer != before.pvBuffer {
            let after = self.descriptor;
            self.descriptor = before;
            let _ = self.release();
            self.descriptor = after;
            self.attempted = false;
            self.capture_extent();
            return Err(Error::provider(None));
        }
        if self.descriptor.cbBuffer > before.cbBuffer
            || self.descriptor.BufferType != SECBUFFER_TOKEN
        {
            self.descriptor.cbBuffer = before.cbBuffer;
            return Err(Error::provider(None));
        }
        Ok(())
    }
    fn release(&mut self) -> Result<(), Error> {
        if self.attempted {
            return Ok(());
        }
        self.attempted = true;
        if !self.descriptor.pvBuffer.is_null() {
            // SAFETY: SSPI ALLOCATE_MEMORY returned this sole-owned allocation.
            // cbBuffer describes its initialized output bytes, retained across
            // ISC/Complete and wiped even on rejected oversized output/status.
            unsafe {
                std::slice::from_raw_parts_mut(self.descriptor.pvBuffer.cast::<u8>(), self.extent)
            }
            .zeroize();
            // SAFETY: the same still-live allocation was just wiped in full.
            self.probe.observe(unsafe {
                std::slice::from_raw_parts(self.descriptor.pvBuffer.cast::<u8>(), self.extent)
            });
            // SAFETY: exactly one FreeContextBuffer attempt for owned allocation.
            let status = unsafe { FreeContextBuffer(self.descriptor.pvBuffer) };
            if status != 0 {
                return Err(Error::provider(Some(status as u32)));
            }
            self.descriptor.pvBuffer = std::ptr::null_mut();
            self.descriptor.cbBuffer = 0;
        }
        Ok(())
    }
}
impl Output for Buffer {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn bytes(&self) -> Result<&[u8], Error> {
        if self.attempted {
            return Err(Error::new(ErrorKind::Protocol));
        }
        if self.descriptor.cbBuffer == 0 {
            return Ok(&[]);
        }
        if self.descriptor.pvBuffer.is_null() {
            return Err(Error::provider(None));
        }
        // SAFETY: trusted SSPI provider returned initialized buffer of reported
        // length; sole owner remains live until checked copy and release.
        Ok(unsafe {
            std::slice::from_raw_parts(
                self.descriptor.pvBuffer.cast(),
                self.descriptor.cbBuffer as usize,
            )
        })
    }
    fn dispose(&mut self) -> Result<(), Error> {
        self.release()
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
pub(super) struct Allocation {
    raw: *mut c_void,
    attempted: bool,
    status: Option<u32>,
}
impl Allocation {
    pub(super) fn new(raw: *mut c_void) -> Self {
        Self {
            raw,
            attempted: false,
            status: None,
        }
    }
    pub(super) fn release(&mut self) -> Result<(), Error> {
        if !self.attempted && !self.raw.is_null() {
            self.attempted = true;
            // SAFETY: sole owner of package/negotiation metadata allocated by
            // SSPI; exactly one free attempt, pointer retained on failure.
            let status = unsafe { FreeContextBuffer(self.raw) };
            if status == 0 {
                self.raw = std::ptr::null_mut();
            } else {
                self.status = Some(status as u32);
            }
        }
        if let Some(status) = self.status {
            return Err(Error::provider(Some(status)));
        }
        Ok(())
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        let _ = self.release();
    }
}
