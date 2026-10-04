// Real, local SSPI verifier. No HTTP/TLS, account creation, policy change or logs
// of credentials/tokens. These rows qualify the worker/provider boundary only.
use super::{deadline, request, supervisor, wait};
use gwz_sspi::{
    Cancellation, CleanupStatus, Deadline, ErrorKind, Mechanism, MechanismObservation, Package,
    SecretBytes, TokenStatus,
};
use std::{ptr, sync::Arc};
use windows_sys::Win32::Foundation::{CloseHandle, SEC_E_BAD_BINDINGS, SEC_I_CONTINUE_NEEDED};
use windows_sys::Win32::Security::Authentication::Identity::*;
use windows_sys::Win32::Security::Credentials::SecHandle;
use windows_sys::Win32::Security::{ImpersonateSelf, RevertToSelf, SecurityImpersonation};
use zeroize::{Zeroize, Zeroizing};

struct Verifier {
    credentials: SecHandle,
    context: SecHandle,
    binding: Zeroizing<Box<[u64]>>,
    live_credentials: bool,
    live_context: bool,
}
fn invalid() -> SecHandle {
    SecHandle {
        dwLower: usize::MAX,
        dwUpper: usize::MAX,
    }
}
fn valid(handle: &SecHandle) -> bool {
    handle.dwLower != usize::MAX || handle.dwUpper != usize::MAX
}
impl Verifier {
    fn new(package: Package, wrong_binding: bool) -> Self {
        // 32-byte SEC_CHANNEL_BINDINGS followed by the 53-byte application data.
        let mut owner = Self {
            credentials: invalid(),
            context: invalid(),
            binding: Zeroizing::new(vec![0u64; 11].into_boxed_slice()),
            live_credentials: false,
            live_context: false,
        };
        let mut name: Vec<u16> = match package {
            Package::Ntlm => "NTLM\0",
            Package::Negotiate => "Negotiate\0",
            Package::Digest => panic!("Digest is outside this fixture"),
        }
        .encode_utf16()
        .collect();
        let mut expiry = 0;
        // SAFETY: fixed writable UTF-16 package name and initialized outputs;
        // null identity selects inbound local verifier credentials, no password.
        let status = unsafe {
            AcquireCredentialsHandleW(
                ptr::null(),
                name.as_mut_ptr(),
                SECPKG_CRED_INBOUND,
                ptr::null(),
                ptr::null(),
                None,
                ptr::null(),
                &mut owner.credentials,
                &mut expiry,
            )
        };
        owner.live_credentials = valid(&owner.credentials);
        assert_eq!(status, 0, "inbound credential acquisition failed");
        assert!(owner.live_credentials);
        // SAFETY: u64 storage is aligned, initialized, fixed and large enough.
        let header = unsafe { &mut *owner.binding.as_mut_ptr().cast::<SEC_CHANNEL_BINDINGS>() };
        header.cbApplicationDataLength = 53;
        header.dwApplicationDataOffset = 32;
        // SAFETY: all 88 bytes of the fixed owner are initialized and writable.
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(owner.binding.as_mut_ptr().cast::<u8>(), 88) };
        bytes[32..53].copy_from_slice(b"tls-server-end-point:");
        if wrong_binding {
            bytes[53] = 1;
        }
        owner
    }
    fn accept(&mut self, input: &[u8]) -> (i32, SecretBytes) {
        assert!(input.len() <= 65536);
        let mut incoming = Zeroizing::new(vec![0u8; input.len()].into_boxed_slice());
        incoming.copy_from_slice(input);
        let mut outgoing = Zeroizing::new(vec![0u8; 65536].into_boxed_slice());
        let mut inputs = [
            SecBuffer {
                cbBuffer: incoming.len() as u32,
                BufferType: SECBUFFER_TOKEN,
                pvBuffer: incoming.as_mut_ptr().cast(),
            },
            SecBuffer {
                cbBuffer: 85,
                BufferType: SECBUFFER_CHANNEL_BINDINGS,
                pvBuffer: self.binding.as_mut_ptr().cast(),
            },
        ];
        let input_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 2,
            pBuffers: inputs.as_mut_ptr(),
        };
        let mut output = SecBuffer {
            cbBuffer: outgoing.len() as u32,
            BufferType: SECBUFFER_TOKEN,
            pvBuffer: outgoing.as_mut_ptr().cast(),
        };
        let mut output_desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 1,
            pBuffers: &mut output,
        };
        let previous = if self.live_context {
            &self.context
        } else {
            ptr::null()
        };
        let mut next = invalid();
        let mut attributes = 0;
        let mut expiry = 0;
        // SAFETY: live credential/context; descriptors refer to exclusively owned,
        // fixed writable buffers, retained through this synchronous call. Do not
        // request ALLOCATE_MEMORY or ALLOW_MISSING_BINDINGS.
        let status = unsafe {
            AcceptSecurityContext(
                &self.credentials,
                previous,
                &input_desc,
                ASC_REQ_CONNECTION,
                SECURITY_NATIVE_DREP,
                &mut next,
                &mut output_desc,
                &mut attributes,
                &mut expiry,
            )
        };
        if valid(&next) {
            self.context = next;
            self.live_context = true;
        }
        assert_eq!(output.pvBuffer, outgoing.as_mut_ptr().cast());
        assert!(output.cbBuffer <= 65536);
        let token = SecretBytes::new(&outgoing[..output.cbBuffer as usize]);
        incoming.zeroize();
        outgoing.zeroize();
        (status, token)
    }
    fn has_authenticated_token(&self) -> bool {
        let mut token = ptr::null_mut();
        // SAFETY: successful accepted context remains owned and live; initialized
        // output takes ownership only after successful native query.
        let status = unsafe { QuerySecurityContextToken(&self.context, &mut token) };
        if status != 0 {
            return false;
        }
        assert!(!token.is_null());
        // SAFETY: sole token owner, no identity metadata is read or recorded.
        let closed = unsafe { CloseHandle(token) };
        assert_ne!(closed, 0);
        true
    }
    fn finish(&mut self) {
        if self.live_context {
            self.live_context = false;
            // SAFETY: sole context owner, all synchronous calls completed.
            assert_eq!(unsafe { DeleteSecurityContext(&self.context) }, 0);
        }
        if self.live_credentials {
            self.live_credentials = false;
            // SAFETY: sole credential owner after context disposal.
            assert_eq!(unsafe { FreeCredentialsHandle(&self.credentials) }, 0);
        }
    }
}
impl Drop for Verifier {
    fn drop(&mut self) {
        if self.live_context {
            // SAFETY: sole live owner; best effort on fixture assertion failure.
            let _ = unsafe { DeleteSecurityContext(&self.context) };
        }
        if self.live_credentials {
            // SAFETY: sole live credential, after context disposal attempt.
            let _ = unsafe { FreeCredentialsHandle(&self.credentials) };
        }
    }
}
fn exchange(package: Package, wrong_binding: bool) {
    let supervisor = supervisor();
    let mut input = request(false);
    input.package = package;
    let mut conversation = wait(supervisor.start(input, deadline(), Cancellation::new())).unwrap();
    let mut verifier = Verifier::new(package, wrong_binding);
    let mut challenge = None;
    let mut status = SEC_I_CONTINUE_NEEDED;
    let mut client_complete = false;
    for _round in 0..8 {
        let step = wait(conversation.step(challenge.take())).unwrap();
        client_complete = step.status == TokenStatus::Complete;
        if client_complete {
            assert!(matches!(
                step.observation,
                MechanismObservation::Selected {
                    mechanism: Mechanism::Ntlm,
                    authoritative: true
                }
            ));
        }
        if !step.payload.as_bytes().is_empty() {
            let (next_status, token) = verifier.accept(step.payload.as_bytes());
            status = next_status;
            if !token.as_bytes().is_empty() {
                challenge = Some(token);
            }
        } else {
            assert_eq!(status, 0, "empty client output requires completed verifier");
        }
        drop(step);
        if status < 0 || (status == 0 && client_complete) {
            break;
        }
        assert!(
            challenge.is_some(),
            "continuing exchange requires verifier token"
        );
    }
    // Settle our production worker before interpreting the verifier result.
    wait(conversation.finish()).unwrap();
    assert!(wait(supervisor.shutdown(deadline())).outstanding.is_empty());
    if wrong_binding {
        assert_eq!(
            status, SEC_E_BAD_BINDINGS,
            "native verifier must reject wrong binding"
        );
    } else {
        assert_eq!(status, 0, "native verifier must complete authentication");
        assert!(
            client_complete,
            "native worker must separately complete authentication"
        );
        assert!(verifier.has_authenticated_token());
    }
    verifier.finish();
}
#[test]
#[ignore = "opt-in completed local NTLM, production worker and native verifier"]
fn worker_completes_local_ntlm() {
    exchange(Package::Ntlm, false);
}
#[test]
#[ignore = "opt-in completed local Negotiate selecting NTLM, no Kerberos claim"]
fn worker_completes_local_negotiate() {
    exchange(Package::Negotiate, false);
}
#[test]
#[ignore = "opt-in actual native verifier rejects mismatched synthetic CBT"]
fn worker_wrong_binding_is_rejected_by_native_verifier() {
    exchange(Package::Ntlm, true);
}
#[test]
#[ignore = "opt-in originating thread capture across live and retired handoff"]
fn captured_origin_is_checked_after_executor_handoff() {
    for retired in [false, true] {
        let supervisor = Arc::new(supervisor());
        let origin_supervisor = supervisor.clone();
        let (capture_tx, capture_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let origin = std::thread::spawn(move || {
            capture_tx
                .send(origin_supervisor.capture_caller().unwrap())
                .unwrap();
            release_rx.recv().unwrap();
        });
        let capture = capture_rx.recv().unwrap();
        let mut origin = Some(origin);
        if retired {
            release_tx.send(()).unwrap();
            origin.take().unwrap().join().unwrap();
        }
        let result = wait(supervisor.start_captured(
            &capture,
            request(false),
            deadline(),
            Cancellation::new(),
        ));
        match result {
            Ok(conversation) => {
                wait(conversation.finish()).unwrap();
                assert!(!retired, "retired origin must refuse registration/launch");
            }
            Err(failure) => {
                assert!(retired, "live origin must remain usable across handoff");
                assert_eq!(failure.kind(), ErrorKind::IdentityMismatch);
            }
        }
        assert!(wait(supervisor.shutdown(deadline())).outstanding.is_empty());
        if let Some(origin) = origin {
            release_tx.send(()).unwrap();
            origin.join().unwrap();
        }
    }
}

#[test]
#[ignore = "opt-in fresh thread self-impersonation, restored before thread exit"]
fn captured_origin_impersonation_is_refused_after_handoff() {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: only this fresh fixture thread's own ImpersonateSelf token;
            // it had no inherited token and never changes another thread/account.
            assert_ne!(unsafe { RevertToSelf() }, 0);
        }
    }
    let supervisor = Arc::new(supervisor());
    let original = supervisor.clone();
    let (capture_tx, capture_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let origin = std::thread::spawn(move || {
        let capture = original.capture_caller().unwrap();
        // SAFETY: fresh fixture thread, own process identity, no supplied secrets.
        assert_ne!(unsafe { ImpersonateSelf(SecurityImpersonation) }, 0);
        let restore = Restore;
        assert_eq!(
            original.capture_caller().err().unwrap().kind(),
            ErrorKind::IdentityMismatch
        );
        capture_tx.send(capture).unwrap();
        release_rx.recv().unwrap();
        drop(restore);
        // An actual post-restoration capture proves the token was removed.
        assert!(original.capture_caller().is_ok());
    });
    let capture = capture_rx.recv().unwrap();
    let failure =
        wait(supervisor.start_captured(&capture, request(false), deadline(), Cancellation::new()))
            .err()
            .unwrap();
    release_tx.send(()).unwrap();
    origin.join().unwrap();
    assert_eq!(failure.kind(), ErrorKind::IdentityMismatch);
    assert!(wait(supervisor.shutdown(deadline())).outstanding.is_empty());
}

#[test]
#[ignore = "opt-in production idle-worker cancellation and fixed deadline cleanup"]
fn idle_worker_cancel_and_deadline_confirm_disposal() {
    for expired in [false, true] {
        let supervisor = supervisor();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut conversation =
            wait(supervisor.start(request(false), Deadline::new(until), Cancellation::new()))
                .unwrap();
        let step = wait(conversation.step(None)).unwrap();
        assert_eq!(step.status, TokenStatus::Continue);
        drop(step);
        if expired {
            std::thread::sleep(
                until.saturating_duration_since(std::time::Instant::now())
                    + std::time::Duration::from_millis(20),
            );
            let failure = wait(conversation.finish()).err().unwrap();
            assert_eq!(failure.kind(), ErrorKind::Timeout);
        } else {
            let receipt = conversation.cancel();
            let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while supervisor.cleanup_status(receipt.record_id.clone()) != CleanupStatus::Confirmed {
                assert!(
                    std::time::Instant::now() < limit,
                    "cancel cleanup remains unconfirmed"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let report = wait(supervisor.shutdown(deadline()));
        assert!(report.outstanding.is_empty());
        assert_eq!(report.confirmed, 1);
    }
}
