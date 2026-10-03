use crate::{SecretBytes, SecretText};

/// Fixed failure classification; errors never carry provider or identity text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Native authentication requires Windows.
    UnsupportedPlatform,
    /// Caller values violate the accepted profile.
    InvalidRequest,
    /// The child primary identity does not match.
    IdentityMismatch,
    /// The installed worker cannot be launched.
    WorkerUnavailable,
    /// Installed artifact fingerprints differ.
    WorkerMismatch,
    /// Required containment could not be established.
    ContainmentFailed,
    /// Private message or ordering violated the contract.
    Protocol,
    /// Native provider refused or exceeded a bound.
    ProviderRejected,
    /// The immutable deadline elapsed.
    Timeout,
    /// The caller cancelled.
    Cancelled,
    /// The conversation or supervisor is closed.
    Closed,
    /// Admission is closed or unavailable.
    CapacityUnavailable,
}
/// An owned fixed-class error with optional numeric native status.
#[derive(Debug, Eq, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    native_status: Option<u32>,
}
impl Error {
    pub(crate) const fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            native_status: None,
        }
    }
    pub(crate) const fn provider(status: Option<u32>) -> Self {
        Self {
            kind: ErrorKind::ProviderRejected,
            native_status: status,
        }
    }
    /// Return the fixed classification.
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Return the native SECURITY_STATUS bit pattern, when applicable.
    pub const fn native_status(&self) -> Option<u32> {
        self.native_status
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}
impl std::error::Error for Error {}

/// Required immutable raw token cap, with no default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenLimit(u32);
impl TokenLimit {
    /// Construct a raw-byte cap in 1..=65,536. The host accounts for HTTP
    /// scheme/base64 overhead before calling this constructor.
    pub fn new(raw_bytes: u32) -> Result<Self, Error> {
        if !(1..=65536).contains(&raw_bytes) {
            return Err(Error::new(ErrorKind::InvalidRequest));
        }
        Ok(Self(raw_bytes))
    }
    /// Return the exact checked cap.
    pub const fn raw_bytes(self) -> u32 {
        self.0
    }
}
/// Allowed caller package choice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Package {
    /// Permit Kerberos or NTLM selection.
    Negotiate,
    /// Request the NTLM provider.
    Ntlm,
    /// Request qualified Digest authentication.
    Digest,
}
/// Owned caller identity, already split into user and domain.
///
/// ```compile_fail
/// fn require<T: Clone>() {}
/// require::<gwz_sspi::Identity>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {}
/// require::<gwz_sspi::Identity>();
/// ```
pub enum Identity {
    /// Use the captured current primary logon identity.
    CurrentLogon,
    /// Explicit Unicode credentials.
    Explicit {
        /// Nonempty user, at most 8,192 UTF-8 bytes.
        user: SecretText,
        /// Domain, at most 8,192 UTF-8 bytes; may be empty.
        domain: SecretText,
        /// Password, at most 8,192 UTF-8 bytes; may be empty.
        password: SecretText,
    },
}
/// The actual Digest request values; no parsing, normalization or rebuilding.
///
/// ```compile_fail
/// fn require<T: Clone>() {}
/// require::<gwz_sspi::DigestInput>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {}
/// require::<gwz_sspi::DigestInput>();
/// ```
pub struct DigestInput {
    /// Opaque initial nonempty server challenge.
    pub initial_challenge: SecretBytes,
    /// Actual ASCII HTTP token method, at most 64 bytes.
    pub method: SecretText,
    /// Actual percent-encoded ASCII request URI, at most 8,192 bytes.
    pub uri: SecretText,
}
/// Owned request values. Validation occurs before the private Begin adapter.
///
/// ```compile_fail
/// fn require<T: Clone>() {}
/// require::<gwz_sspi::AuthRequest>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {}
/// require::<gwz_sspi::AuthRequest>();
/// ```
pub struct AuthRequest {
    /// Permitted package.
    pub package: Package,
    /// Native target; Negotiate/NTLM use HTTP/canonical-host.
    pub target: SecretText,
    /// Owned identity.
    pub identity: Identity,
    /// Verified origin certificate tls-server-end-point binding bytes.
    pub channel_binding: SecretBytes,
    /// Required immutable raw token allowance.
    pub token_limit: TokenLimit,
    /// Present exactly for Digest, which requires explicit identity.
    pub digest: Option<DigestInput>,
}
/// Observed provider mechanism.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mechanism {
    /// Kerberos selected by Negotiate.
    Kerberos,
    /// NTLM provider selection.
    Ntlm,
    /// Digest provider selection.
    Digest,
}
/// Native mechanism observation, independent of raw numeric attributes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MechanismObservation {
    /// No selected mechanism is known yet.
    Unresolved,
    /// A selected mechanism, provisional or authoritative.
    Selected {
        /// Selected native mechanism.
        mechanism: Mechanism,
        /// True only when native identity is authoritative.
        authoritative: bool,
    },
}
/// Native token-generation status, not proof of HTTP acceptance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenStatus {
    /// Another challenge may follow, subject to the eight-round bound.
    Continue,
    /// Native token generation completed.
    Complete,
}
/// Owned caller token result; no diagnostic formatting or cloning.
///
/// ```compile_fail
/// fn require<T: Clone>() {}
/// require::<gwz_sspi::TokenStep>();
/// ```
/// ```compile_fail
/// fn require<T: std::fmt::Debug>() {}
/// require::<gwz_sspi::TokenStep>();
/// ```
pub struct TokenStep {
    /// Native generation status.
    pub status: TokenStatus,
    /// Numeric context flags.
    pub attributes: u32,
    /// Native mechanism observation.
    pub observation: MechanismObservation,
    /// Opaque owned token.
    pub payload: SecretBytes,
}
