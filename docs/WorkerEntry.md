# Shared early worker entry

`WorkerBootstrap::from_args(impl Iterator<Item = OsString>)` inspects arguments
excluding argv[0] and returns `Result<Option<WorkerBootstrap>, Error>`. None means
ordinary host invocation. The exact private marker `--gwz-sspi-worker` requires
two distinct nonzero decimal handle numbers and no further arguments. Malformed
internal invocation returns InvalidRequest without echoing arguments. Bootstrap
is an owned Send/Sync value with no Clone/Debug; it has no OS effect and holds no
native handle owner before entry validation. Never put credentials in arguments.

`worker_entry(WorkerBootstrap, [u8;32]) -> Result<(), Error>` is synchronous child
execution. Call during early dispatch before ordinary argument parsing, Git,
logging or runtime setup. It validates both inherited byte-pipe ends before
adopting them, removes onward inheritance, refuses thread impersonation and
queries the actual child primary SID/authentication LUID/session for Hello.
Hello contains the compiled schema fingerprint and supplied trusted build bytes;
no credential acquisition occurs until Begin. Entry blocks on private IPC and
serial native calls. Non-Windows entry returns UnsupportedPlatform. A fixed Error
is safe to classify; do not log any IPC/native payload or provider string.

The host's trusted packaging producer supplies identical `build_fingerprint`
bytes to WorkerExecutable::new and the early worker entry. This field is not a
runtime file hash, Cargo version, environment-selected executable or library
default. The explicit host packaging producer and callable CLI/Python
descriptors are documented in [HostPackaging.md](HostPackaging.md). HTTP
composition and Windows endpoint activation remain future work.

This compiled recipe names the early handoff. Functions are not executed in the
example, so doctests create no handles/processes or native work:

```rust
use gwz_sspi::{Error, WorkerBootstrap, worker_entry};
use std::ffi::OsString;

fn early_internal_dispatch(
    arguments_without_executable: impl Iterator<Item = OsString>,
    trusted_host_build_fingerprint: [u8; 32],
) -> Result<Option<()>, Error> {
    match WorkerBootstrap::from_args(arguments_without_executable)? {
        None => Ok(None), // Host can reopen argv for its ordinary parser.
        Some(bootstrap) => {
            worker_entry(bootstrap, trusted_host_build_fingerprint)?;
            Ok(Some(())) // Host exits before ordinary initialization.
        }
    }
}
```

The minimal `worker-bin` executable calls exactly this shared entry. Its trusted
compile-time packaging field `GWZ_SSPI_BUILD_FINGERPRINT` must contain exactly 64
ASCII hexadecimal characters, encoding the same 32 expected host bytes. Missing
or malformed metadata causes silent exit 2 before handle ownership, native calls
or Hello. Runtime environment variables cannot supply/override this field. A
valid metadata build still validates the internal invocation and inherited pipes.
The child uses exit 0 only after successful normal cleanup; fixed refusal/error
uses exit 2. Neither stdout/stderr is used for logging (stdout may be the private
protocol pipe). This is the step-3 handoff, not a trusted packaging producer.

The worker owns one serial conversation. Negotiate permits Kerberos or NTLM;
direct NTLM selects its known provider. It queries package maximum and intersects
it with the immutable token limit before credential acquisition. Explicit Unicode
identity, NUL-terminated target and aligned native CBT live in fixed initialized
zeroizing allocations through all native use. Only CONNECTION|ALLOCATE_MEMORY
flags are requested. ISC Complete statuses call CompleteAuthToken before copy;
unknown statuses refuse. Negotiate queries actual negotiation info and frees the
query allocation; unknown/unavailable intermediate observation is unresolved,
while Complete requires an authoritative known mechanism. Provider output is
bounded, copied, wiped across all reported initialized bytes and freed before a
Token can be written. Native context and credential cleanup is serial and exactly
once per initialized handle; failed cleanup cannot acknowledge Finished.

Finish is legal before Begin, between rounds and after Complete. Continue in round
8 refuses. Clean input EOF disposes native owners and closes pipes; it sends no
Finished and cannot establish parent cleanup or successful conversation by itself.
Partial/malformed input, state errors and I/O failures are terminal. Error frames
contain fixed enum/status only. Forced Job exit contains worker ownership, but is
not evidence of physical wiping or external-provider cancellation.

Digest remains unavailable and returns ProviderRejected before credential/context
work, after strict Begin/package-cap validation. Microsoft's [HTTP Digest input
contract](https://learn.microsoft.com/en-us/windows/win32/secauthn/input-buffers-for-the-digest-challenge-response)
requires H(Entity) in a PARAMS buffer in addition to challenge/method. The current
accepted request cannot supply that value. URI is the native target, not a body
hash substitute. No empty-body assumption, challenge/qop parsing, hashing or wire
amendment is introduced. A reviewed amendment and native parity evidence are
required before implementing/activating that path.

Opt-in production Windows fixtures use **synthetic** compile-time metadata
`4242424242424242424242424242424242424242424242424242424242424242` and matching
expected `[0x42;32]`. They do not prove production package provenance. Initial NTLM
fixtures need no server, account mutation or supplied real password; their tokens
are never sent to a network. Complete NTLM/Kerberos, EPA, blocking providers,
Digest, installation and HTTP parity remain separate qualification rows.
