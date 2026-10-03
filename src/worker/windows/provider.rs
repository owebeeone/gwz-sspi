use super::super::super::native::{Provider, Session};
use super::{conversation::Conversation, owners::Allocation, storage::Wide};
use crate::{AuthRequest, Error, Package};
use windows_sys::Win32::Security::Authentication::Identity::{
    QuerySecurityPackageInfoW, SecPkgInfoW,
};
pub(super) struct System;
pub(super) fn package_name(package: Package) -> &'static str {
    match package {
        Package::Negotiate => "Negotiate",
        Package::Ntlm => "NTLM",
        Package::Digest => "WDigest",
    }
}
impl Provider for System {
    fn maximum(&mut self, package: Package) -> Result<u32, Error> {
        let name = Wide::new(package_name(package));
        let mut raw = std::ptr::null_mut();
        // SAFETY: owned NUL-terminated name and initialized pointer output.
        let status = unsafe { QuerySecurityPackageInfoW(name.pointer(), &mut raw) };
        let mut owner = Allocation::new(raw.cast());
        if status != 0 {
            return Err(Error::provider(Some(status as u32)));
        }
        if raw.is_null() {
            return Err(Error::provider(None));
        }
        // SAFETY: successful query returned valid package info until release.
        let max = unsafe { (*raw.cast::<SecPkgInfoW>()).cbMaxToken };
        owner.release()?;
        if max == 0 {
            return Err(Error::provider(None));
        }
        Ok(max)
    }
    fn acquire(&mut self, request: &AuthRequest) -> Result<Box<dyn Session>, Error> {
        Ok(Box::new(Conversation::acquire(request)?))
    }
}
