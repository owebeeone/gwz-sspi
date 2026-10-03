use super::owners::Allocation;
use crate::{Error, Mechanism, MechanismObservation};
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
pub(super) fn query(context: &SecHandle) -> Result<MechanismObservation, Error> {
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
        allocation.release()?;
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
    allocation.release()?;
    Ok(
        selected.map_or(MechanismObservation::Unresolved, |mechanism| {
            MechanismObservation::Selected {
                mechanism,
                authoritative: value.NegotiationState == SECPKG_NEGOTIATION_COMPLETE,
            }
        }),
    )
}
