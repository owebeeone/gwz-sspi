//! Aligned, initialized token scratch is wiped before release. SID/LUID are copied
//! only into fixed zeroizing owners; no ordinary secret byte/text intermediates.
use super::super::super::ports::{Origin, Primary};
use super::handles::Handle;
use crate::secret::Storage;
use crate::{Error, ErrorKind, SecretBytes};
use std::sync::Arc;
use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_TOKEN,
    GetLastError, HANDLE, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_INFORMATION_CLASS, TOKEN_QUERY, TOKEN_STATISTICS, TOKEN_USER,
    TokenSessionId, TokenStatistics, TokenUser,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentThread, OpenProcessToken, OpenThreadToken, WaitForSingleObject,
};
use zeroize::Zeroizing;
fn mismatch() -> Error {
    Error::new(ErrorKind::IdentityMismatch)
}
struct Scratch {
    words: Zeroizing<Box<[u64]>>,
    len: usize,
}
impl Scratch {
    fn get(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Result<Self, Error> {
        let mut required = 0;
        // SAFETY: size query has null output and an initialized size pointer.
        let first =
            unsafe { GetTokenInformation(token, class, std::ptr::null_mut(), 0, &mut required) };
        // SAFETY: read immediately after the size probe on this thread.
        let error = unsafe { GetLastError() };
        if first != 0 || error != ERROR_INSUFFICIENT_BUFFER || required == 0 || required > 65536 {
            return Err(mismatch());
        }
        let mut result = Self {
            words: Zeroizing::new(vec![0u64; (required as usize).div_ceil(8)].into_boxed_slice()),
            len: required as usize,
        };
        let mut actual = 0;
        // SAFETY: allocation has 8-byte alignment, is initialized, fixed, exclusive
        // and at least required bytes. It is wiped on success and every error.
        if unsafe {
            GetTokenInformation(
                token,
                class,
                result.words.as_mut_ptr().cast(),
                required,
                &mut actual,
            )
        } == 0
            || actual == 0
            || actual > required
        {
            return Err(mismatch());
        }
        result.len = actual as usize;
        Ok(result)
    }
    fn bytes(&self) -> &[u8] {
        // SAFETY: all bytes initialized, len checked against allocation above.
        unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast(), self.len) }
    }
    fn record<T>(&self) -> Result<&T, Error> {
        if self.len < std::mem::size_of::<T>() || std::mem::align_of::<T>() > 8 {
            return Err(mismatch());
        }
        // SAFETY: only POD Win32 output structures are requested here. Scratch
        // meets size/alignment and GetTokenInformation initialized their fields.
        Ok(unsafe { &*self.words.as_ptr().cast::<T>() })
    }
}
pub(super) fn primary() -> Result<Primary, Error> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: process pseudo handle is used only in this call; raw is initialized.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(mismatch());
    }
    let token = Handle::new(raw)?;
    let user = Scratch::get(token.0, TokenUser)?;
    let sid = user.record::<TOKEN_USER>()?.User.Sid as usize;
    let base = user.bytes().as_ptr() as usize;
    let offset = sid.checked_sub(base).ok_or_else(mismatch)?;
    let bytes = user.bytes().get(offset..).ok_or_else(mismatch)?;
    if bytes.len() < 8 || bytes[0] != 1 || bytes[1] > 15 {
        return Err(mismatch());
    }
    let length = 8 + 4 * usize::from(bytes[1]);
    let sid = SecretBytes::new(bytes.get(..length).ok_or_else(mismatch)?);
    let statistics = Scratch::get(token.0, TokenStatistics)?;
    let luid = &statistics.record::<TOKEN_STATISTICS>()?.AuthenticationId;
    let mut auth = Storage::zeroed(8);
    for (index, byte) in auth.as_mut().iter_mut().enumerate() {
        *byte = if index < 4 {
            (luid.LowPart >> (index * 8)) as u8
        } else {
            ((luid.HighPart as u32) >> ((index - 4) * 8)) as u8
        };
    }
    let session = Scratch::get(token.0, TokenSessionId)?;
    Ok(Primary {
        sid,
        luid: SecretBytes(auth),
        session: *session.record::<u32>()?,
    })
}
fn refuse(thread: HANDLE) -> Result<(), Error> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: held real or current pseudo thread handle, initialized token output.
    if unsafe { OpenThreadToken(thread, TOKEN_QUERY, 1, &mut raw) } != 0 {
        let token = Handle::new(raw)?;
        drop(token);
        return Err(mismatch());
    }
    // SAFETY: immediate error query for OpenThreadToken.
    if unsafe { GetLastError() } != ERROR_NO_TOKEN {
        return Err(mismatch());
    }
    Ok(())
}
pub(super) fn refuse_current_impersonation() -> Result<(), Error> {
    // SAFETY: current pseudo handle is not retained or closed.
    refuse(unsafe { GetCurrentThread() })
}
struct Captured {
    thread: Handle,
    expected: Arc<Primary>,
}
impl Origin for Captured {
    fn verify(&self) -> Result<(), Error> {
        // SAFETY: original thread is held, not inferred from this executor thread.
        if unsafe { WaitForSingleObject(self.thread.0, 0) } != WAIT_TIMEOUT {
            return Err(mismatch());
        }
        refuse(self.thread.0)?;
        let actual = primary()?;
        if actual.sid.as_bytes() != self.expected.sid.as_bytes()
            || actual.luid.as_bytes() != self.expected.luid.as_bytes()
            || actual.session != self.expected.session
        {
            return Err(mismatch());
        }
        Ok(())
    }
}
pub(super) fn origin(expected: Arc<Primary>) -> Result<Box<dyn Origin>, Error> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: duplicate current thread pseudo handle into owned noninheritable
    // real handle, retaining caller-at-start provenance before a future can move.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            GetCurrentThread(),
            GetCurrentProcess(),
            &mut raw,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(mismatch());
    }
    let captured = Captured {
        thread: Handle::new(raw)?,
        expected,
    };
    captured.verify()?;
    Ok(Box::new(captured))
}
