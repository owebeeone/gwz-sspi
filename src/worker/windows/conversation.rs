//! Serial secur32 calls; owners outlive every possible provider use.
use super::super::super::native::{Output, Session, Step};
use super::{
    owners::Buffer,
    provider::package_name,
    storage::{Binding, Wide},
};
use crate::{AuthRequest, Error, Identity, Mechanism, MechanismObservation, Package};
use windows_sys::Win32::Security::Authentication::Identity::*;
use windows_sys::Win32::Security::Credentials::SecHandle;
use windows_sys::Win32::System::Rpc::{SEC_WINNT_AUTH_IDENTITY_UNICODE, SEC_WINNT_AUTH_IDENTITY_W};
struct Credentials {
    user: Wide,
    domain: Wide,
    password: Wide,
    record: Box<SEC_WINNT_AUTH_IDENTITY_W>,
}
impl Credentials {
    fn new(user: &str, domain: &str, password: &str, probe: &crate::secret::audit::Probe) -> Self {
        let mut user = Wide::new(user);
        let mut domain = Wide::new(domain);
        let mut password = Wide::new(password);
        user.probe = probe.clone();
        domain.probe = probe.clone();
        password.probe = probe.clone();
        let record = Box::new(SEC_WINNT_AUTH_IDENTITY_W {
            User: user.pointer().cast_mut(),
            UserLength: user.count(),
            Domain: domain.pointer().cast_mut(),
            DomainLength: domain.count(),
            Password: password.pointer().cast_mut(),
            PasswordLength: password.count(),
            Flags: SEC_WINNT_AUTH_IDENTITY_UNICODE,
        });
        Self {
            user,
            domain,
            password,
            record,
        }
    }
}
impl Drop for Credentials {
    fn drop(&mut self) {
        // Field owners stay live through the final FreeCredentialsHandle call.
        self.record.User = std::ptr::null_mut();
        self.record.Domain = std::ptr::null_mut();
        self.record.Password = std::ptr::null_mut();
        self.record.UserLength = 0;
        self.record.DomainLength = 0;
        self.record.PasswordLength = 0;
        let _ = (&self.user, &self.domain, &self.password);
    }
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
pub(super) struct Conversation {
    credential: SecHandle,
    context: SecHandle,
    credential_live: bool,
    context_live: bool,
    cleanup_status: Option<u32>,
    identity: Option<Credentials>,
    target: Wide,
    binding: Binding,
    package: Package,
    probe: crate::secret::audit::Probe,
}
impl Conversation {
    pub(super) fn acquire(request: &AuthRequest) -> Result<Self, Error> {
        Self::acquire_audited(request, &crate::secret::audit::Probe::default())
    }
    fn acquire_audited(
        request: &AuthRequest,
        probe: &crate::secret::audit::Probe,
    ) -> Result<Self, Error> {
        if request.package == Package::Digest {
            return Err(Error::provider(None));
        }
        let identity = match &request.identity {
            Identity::CurrentLogon => None,
            Identity::Explicit {
                user,
                domain,
                password,
            } => Some(Credentials::new(
                user.as_str(),
                domain.as_str(),
                password.as_str(),
                probe,
            )),
        };
        let mut result = Self {
            credential: invalid(),
            context: invalid(),
            credential_live: false,
            context_live: false,
            cleanup_status: None,
            identity,
            target: Wide::new(request.target.as_str()),
            binding: Binding::new(request.channel_binding.as_bytes()),
            package: request.package,
            probe: probe.clone(),
        };
        result.target.probe = probe.clone();
        result.binding.probe = probe.clone();
        let package = Wide::new(package_name(request.package));
        let mut expiry = 0;
        let auth = result.identity.as_ref().map_or(std::ptr::null(), |i| {
            (&*i.record as *const SEC_WINNT_AUTH_IDENTITY_W).cast()
        });
        // SAFETY: stable initialized identity record and fixed Unicode owners
        // are retained in result through all calls and final credential disposal.
        // NULL selects current primary logon; no impersonation/fallback is used.
        let status = unsafe {
            AcquireCredentialsHandleW(
                std::ptr::null(),
                package.pointer(),
                SECPKG_CRED_OUTBOUND,
                std::ptr::null(),
                auth,
                None,
                std::ptr::null(),
                &mut result.credential,
                &mut expiry,
            )
        };
        result.credential_live = valid(&result.credential);
        if status != 0 {
            return Err(Error::provider(Some(status as u32)));
        }
        if !result.credential_live {
            return Err(Error::provider(None));
        }
        Ok(result)
    }
    fn dispose_handles(&mut self) -> Result<(), Error> {
        if self.context_live {
            self.context_live = false;
            // SAFETY: serial owner attempts deletion exactly once after every
            // possible ISC/Complete/query call returns and before credential free.
            let status = unsafe { DeleteSecurityContext(&self.context) };
            if status != 0 {
                self.cleanup_status.get_or_insert(status as u32);
            }
            self.context = invalid();
        }
        if self.credential_live {
            self.credential_live = false;
            // SAFETY: sole initialized credential, retained until context cleanup
            // attempt completes; fixed identity remains live through this call.
            let status = unsafe { FreeCredentialsHandle(&self.credential) };
            if status != 0 {
                self.cleanup_status.get_or_insert(status as u32);
            }
            self.credential = invalid();
        }
        if let Some(status) = self.cleanup_status {
            return Err(Error::provider(Some(status)));
        }
        Ok(())
    }
}
impl Session for Conversation {
    fn initialize(&mut self, input: &[u8]) -> Result<Step, Error> {
        if !self.credential_live {
            return Err(Error::provider(None));
        }
        // Providers may modify input buffers; make fixed owned copies rather than
        // casting an immutable caller reference into a writable native buffer.
        let mut challenge = crate::secret::Storage::copy(input);
        challenge.probe = self.probe.clone();
        let mut buffers = [
            SecBuffer {
                cbBuffer: challenge.as_slice().len() as u32,
                BufferType: SECBUFFER_TOKEN,
                pvBuffer: challenge.as_mut().as_mut_ptr().cast(),
            },
            SecBuffer {
                cbBuffer: self.binding.len(),
                BufferType: SECBUFFER_CHANNEL_BINDINGS,
                pvBuffer: self.binding.pointer(),
            },
        ];
        let desc = SecBufferDesc {
            ulVersion: SECBUFFER_VERSION,
            cBuffers: 2,
            pBuffers: buffers.as_mut_ptr(),
        };
        let mut output = Buffer::empty();
        output.probe = self.probe.clone();
        let mut out = output.desc();
        let mut attributes = 0;
        let mut expiry = 0;
        let old = if self.context_live {
            &self.context as *const SecHandle
        } else {
            std::ptr::null()
        };
        // SAFETY: serial call; initialized mutable fixed input/output and stable
        // credential/context/UTF-16/CBT remain owned until this call returns.
        // Only accepted CONNECTION|ALLOCATE flags; no delegation/mutual demand.
        let status = unsafe {
            InitializeSecurityContextW(
                &self.credential,
                old,
                self.target.pointer(),
                ISC_REQ_CONNECTION | ISC_REQ_ALLOCATE_MEMORY,
                0,
                SECURITY_NATIVE_DREP,
                &desc,
                0,
                &mut self.context,
                &mut out,
                &mut attributes,
                &mut expiry,
            )
        };
        self.context_live = valid(&self.context);
        output.capture_extent();
        Ok(Step {
            status: status as u32,
            attributes,
            output: Box::new(output),
        })
    }
    fn complete(&mut self, output: &mut dyn Output) -> Result<(), Error> {
        if !self.context_live {
            return Err(Error::provider(None));
        }
        let output = output
            .as_any_mut()
            .downcast_mut::<Buffer>()
            .ok_or_else(|| Error::provider(None))?;
        let before = output.descriptor;
        let desc = output.desc();
        // SAFETY: held context and sole-owned provider output remain live across
        // synchronous CompleteAuthToken; no publication/copy precedes this call.
        let status = unsafe { CompleteAuthToken(&self.context, &desc) };
        output.check_completion(before)?;
        if status != 0 {
            return Err(Error::provider(Some(status as u32)));
        }
        Ok(())
    }
    fn observation(&mut self) -> Result<MechanismObservation, Error> {
        match self.package {
            Package::Ntlm => Ok(MechanismObservation::Selected {
                mechanism: Mechanism::Ntlm,
                authoritative: true,
            }),
            Package::Digest => Err(Error::provider(None)),
            Package::Negotiate => super::observation::query(&self.context),
        }
    }
    fn cleanup(&mut self) -> Result<(), Error> {
        self.dispose_handles()
    }
}
impl Drop for Conversation {
    fn drop(&mut self) {
        let _ = self.dispose_handles();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("native_owner_tests.rs");
}
