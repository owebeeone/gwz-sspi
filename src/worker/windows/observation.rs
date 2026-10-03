use super::owners::Allocation;
use crate::{Error, Mechanism, MechanismObservation};
pub(super) use audit::Audit;
use windows_sys::Win32::Security::Authentication::Identity::{
    QueryContextAttributesW, SECPKG_ATTR_NEGOTIATION_INFO, SECPKG_NEGOTIATION_COMPLETE,
    SecPkgContext_NegotiationInfoW,
};
use windows_sys::Win32::Security::Credentials::SecHandle;
fn name(pointer: *const u16, expected: &[u8]) -> bool {
    if pointer.is_null() {
        return false;
    }
    for (index, byte) in expected.iter().enumerate() {
        // SAFETY: SSPI package Name is a provider-owned NUL-terminated UTF-16
        // string. Stop on first mismatch/NUL, bounded to known package length.
        let word = unsafe { *pointer.add(index) };
        if word > 127 || !(word as u8).eq_ignore_ascii_case(byte) {
            return false;
        }
    }
    // SAFETY: matched prefix of known bounded package name; check final NUL.
    unsafe { *pointer.add(expected.len()) == 0 }
}
pub(super) fn query(context: &SecHandle, audit: &Audit) -> Result<MechanismObservation, Error> {
    let mut value = SecPkgContext_NegotiationInfoW::default();
    // SAFETY: held initialized context and properly sized initialized output.
    let status = unsafe {
        QueryContextAttributesW(
            context,
            SECPKG_ATTR_NEGOTIATION_INFO,
            (&mut value as *mut SecPkgContext_NegotiationInfoW).cast(),
        )
    };
    let mut allocation = Allocation::new(value.PackageInfo.cast());
    if status != 0 {
        let freed = allocation.release();
        audit.record(status as u32, !value.PackageInfo.is_null(), freed.is_ok());
        freed?;
        return Ok(MechanismObservation::Unresolved);
    }
    let selected = if value.PackageInfo.is_null() {
        None
    } else {
        // SAFETY: successful query owns valid package info through allocation.
        let pointer = unsafe { (*value.PackageInfo).Name };
        if name(pointer, b"Kerberos") {
            Some(Mechanism::Kerberos)
        } else if name(pointer, b"NTLM") {
            Some(Mechanism::Ntlm)
        } else {
            None
        }
    };
    let freed = allocation.release();
    audit.record(status as u32, !value.PackageInfo.is_null(), freed.is_ok());
    freed?;
    Ok(
        selected.map_or(MechanismObservation::Unresolved, |mechanism| {
            MechanismObservation::Selected {
                mechanism,
                authoritative: value.NegotiationState == SECPKG_NEGOTIATION_COMPLETE,
            }
        }),
    )
}

#[cfg(not(test))]
mod audit {
    #[derive(Default)]
    pub(in crate::worker) struct Audit {}
    impl Audit {
        pub(in crate::worker) fn record(&self, _: u32, _: bool, _: bool) {}
    }
}
#[cfg(test)]
mod audit {
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct Proof {
        pub(crate) query_status: u32,
        pub(crate) allocation_returned: bool,
        pub(crate) release_success: bool,
    }
    #[derive(Default)]
    pub(crate) struct Audit(pub(crate) Option<std::sync::Arc<std::sync::Mutex<Option<Proof>>>>);
    impl Audit {
        pub(crate) fn record(
            &self,
            query_status: u32,
            allocation_returned: bool,
            release_success: bool,
        ) {
            if let Some(receipt) = &self.0 {
                *receipt.lock().unwrap() = Some(Proof {
                    query_status,
                    allocation_returned,
                    release_success,
                });
            }
        }
    }
}
