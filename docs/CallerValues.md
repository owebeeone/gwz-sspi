# Implemented caller values

This checkpoint provides owned values; it does not perform authentication.
Supervisor, Conversation, worker_entry, deadlines, cancellation, capacity and
cleanup receipts remain unimplemented. The packaged worker refuses every call.

`SecretBytes::new(&[u8]) -> SecretBytes` and
`SecretText::new(&str) -> Result<SecretText, Error>` allocate zeroed fixed storage
before copying. Text rejects NUL with InvalidRequest. Their borrowed accessors
`as_bytes` and `as_str` last only while their owner is held. The caller still owns
the constructor source and must arrange its own wiping; neither constructor can
wipe borrowed input. There is no ordinary owned secret String/Vec or UTF-16
conversion in this implementation. Owned bytes are wiped on normal Drop; this
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
a future supervisor must first establish phase and publication eligibility.
Direct NTLM/Digest use their selected provider; unresolved Continue observations
require a Negotiate context. Complete requires authoritative Selected.
