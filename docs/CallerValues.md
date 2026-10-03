# Implemented caller values

These owned values are inert until admitted by Supervisor/Conversation. Parent
deadlines, cancellation, capacity and cleanup receipts are implemented; see
[Supervision.md](Supervision.md). The shared [worker entry](WorkerEntry.md) runs
native Negotiate/NTLM on Windows with matching trusted packaging metadata.
Missing/malformed metadata or bootstrap refuses silently; Digest remains refused
before credential/context work. Native runtime qualification is a separate gate;
see [NativeFixtures.md](NativeFixtures.md).

`SecretBytes::new(&[u8]) -> SecretBytes` and
`SecretText::new(&str) -> Result<SecretText, Error>` allocate zeroed fixed storage
before copying. Text rejects NUL with InvalidRequest. Their borrowed accessors
`as_bytes` and `as_str` last only while their owner is held. The caller still owns
the constructor source and must arrange its own wiping; neither constructor can
wipe borrowed input. Value constructors use no ordinary owned secret String/Vec
and do not convert to UTF-16. The Windows worker separately uses fixed, zeroizing
UTF-16 native owners. Owned bytes are wiped on normal Drop; this
is not physical erasure or disposal of provider/LSASS memory.

`TokenLimit::new(raw_bytes: u32) -> Result<TokenLimit, Error>` admits 1–65,536.
`raw_bytes()` returns exactly the supplied cap. It is immutable and has no
Default. The host derives it from its existing HTTP bound after scheme/base64
overhead. The private Begin adapter copies the cap exactly and refuses invalid
request profiles with InvalidRequest before encoding.

The public owned request records are:

| Value | Fields |
|---|---|
| Package | Negotiate, Ntlm, Digest |
| Identity | CurrentLogon or Explicit { user, domain, password: SecretText } |
| DigestInput | initial_challenge: SecretBytes, method/uri: SecretText |
| AuthRequest | package, target: SecretText, identity, channel_binding: SecretBytes, token_limit: TokenLimit, digest: Option<DigestInput> |
| TokenStep | status: TokenStatus, attributes: u32, observation: MechanismObservation, payload: SecretBytes |
| TokenStatus | Continue, Complete |
| Mechanism | Kerberos, Ntlm, Digest |
| MechanismObservation | Unresolved or Selected { mechanism, authoritative: bool } |

Request fields remain owned; struct construction alone does not admit the full
request profile. The private adapter validates before encoding. Digest requires
explicit identity and its actual nonempty challenge/method/percent-encoded URI.
Other packages forbid Digest fields. Identity strings are already split; no
password normalization or token/challenge parsing occurs. Target host
canonicalization belongs to the host; Negotiate/NTLM use HTTP/canonical-host
without port/path, with IPv6 brackets removed. Channel binding must identify the
verified origin certificate, using tls-server-end-point plus 32/48/64 digest bytes.

SecretBytes, SecretText, Identity, DigestInput, AuthRequest and TokenStep implement
neither Clone nor Debug. They are Send + Sync. Nonsecret enums and TokenLimit
are Copy/Clone/Debug. Error is an owned fixed classification with `kind()` and
`native_status()`; Display prints only its fixed kind. ErrorKind retains the
accepted public list: UnsupportedPlatform, InvalidRequest, IdentityMismatch,
WorkerUnavailable, WorkerMismatch, ContainmentFailed, Protocol, ProviderRejected,
Timeout, Cancelled, Closed and CapacityUnavailable. Private Internal maps to
public Protocol; optional native status belongs only to ProviderRejected.

All private message records and codecs remain inaccessible to callers. The pure
codec validates structure, fingerprints/primary identity against supplied
expected values, caps/package/mechanism relations and expected round. It does
not establish conversation phase, terminal arbitration, native identity, process
exit, containment or cleanup. Its token projection is a value conversion only;
the parent supervisor separately establishes phase and publication eligibility.
Direct NTLM/Digest use their selected provider; unresolved Continue observations
require a Negotiate context. Complete requires authoritative Selected.

## Construct, inspect and dispose

All public types below are exported from the `gwz_sspi` crate root. This synthetic
example uses `zeroize::Zeroizing` for caller-owned sources, so a downstream host
using this recipe also declares `zeroize = "=1.9.0"` with the `alloc` feature.
Dropping those sources is separate from dropping the copies owned by gwz-sspi.
The binding below is a synthetic fixture, not evidence of a verified TLS origin.
Constructing AuthRequest does not validate its complete profile or authenticate.

```rust
use gwz_sspi::{AuthRequest, Identity, Package, SecretBytes, SecretText, TokenLimit};
use zeroize::Zeroizing;

fn main() -> Result<(), gwz_sspi::Error> {
    let bytes_source = Zeroizing::new(*b"synthetic challenge");
    let bytes = SecretBytes::new(&bytes_source[..]);
    assert_eq!(bytes.as_bytes().len(), bytes_source.len());
    drop(bytes_source); // Wipe the caller's source, independently of its copy.
    drop(bytes); // Wipe the library-owned copy.

    let user_source = Zeroizing::new(*b"synthetic-user");
    let domain_source = Zeroizing::new(*b"EXAMPLE");
    let password_source = Zeroizing::new(*b"synthetic-password");
    let identity = Identity::Explicit {
        user: SecretText::new(std::str::from_utf8(&user_source[..]).unwrap())?,
        domain: SecretText::new(std::str::from_utf8(&domain_source[..]).unwrap())?,
        password: SecretText::new(std::str::from_utf8(&password_source[..]).unwrap())?,
    };
    drop((user_source, domain_source, password_source)); // Source owners wipe.

    let mut binding_source = Zeroizing::new([0u8; 53]);
    binding_source[..21].copy_from_slice(b"tls-server-end-point:");
    let request = AuthRequest {
        package: Package::Ntlm,
        target: SecretText::new("HTTP/example.test")?,
        identity,
        channel_binding: SecretBytes::new(&binding_source[..]),
        token_limit: TokenLimit::new(512)?,
        digest: None,
    };
    drop(binding_source); // Wipe the caller's binding fixture.
    assert_eq!(request.target.as_str(), "HTTP/example.test");
    assert_eq!(request.token_limit.raw_bytes(), 512);
    drop(request); // Wipe its owned identity, target and binding copies.
    Ok(())
}
```

The literal target is public fixture metadata. Real mutable credential sources
need their own wiping owner on error paths as well as success; Zeroizing supplies
that Drop behavior here. No Supervisor or worker is called by this example.
