# Opt-in Windows fixtures

These self-contained public fixtures execute the production worker and native
owners, never a success-shaped worker stub. Default fast tests do not execute
them. Use pinned Rust 1.95, an external build target and a fresh external Windows
runtime/scratch directory (inside this workspace follow EVIDENCE.md). Build with
synthetic metadata only for the fixtures:

```powershell
$env:GWZ_SSPI_BUILD_FINGERPRINT = '42' * 32
$fixtureRoot = Join-Path 'D:/gwz-tests' ('sspi-native-' + [guid]::NewGuid().ToString('N'))
$env:CARGO_TARGET_DIR = Join-Path $fixtureRoot 'target'
$env:GWZ_SSPI_NATIVE_SCRATCH = Join-Path $fixtureRoot 'scratch'
New-Item -ItemType Directory -Force $env:GWZ_SSPI_NATIVE_SCRATCH | Out-Null
cargo build --locked --features worker-bin
$env:GWZ_SSPI_NATIVE_WORKER = (Resolve-Path "$env:CARGO_TARGET_DIR/debug/gwz-sspi-worker.exe").Path
cargo test --all-features --locked --test native_worker -- --ignored --test-threads=1
cargo test --lib --all-features --locked -- --ignored --test-threads=1
```

Use ordinary exit status to assess each command; there is no expected-count gate.
The public native_worker target exercises Supervisor with verified actual primary
Hello, Finish before Begin and initial default/Unicode explicit NTLM followed by
confirmed disposal. Credentials are synthetic for explicit identity, tokens stay
local and no server/account/trust mutation is required. The binding is synthetic,
so this is not EPA or real TLS fidelity evidence.

Ignored Windows unit fixtures call production creation/bootstrap/native owners:

- Normal EOF before Begin and after an initial native token; worker exit status 0,
  held process exit, Job active count zero and pipe EOF after owned writers close.
  No Finished is inferred from EOF.
- Last owned Job closure while an independently held process handle observes exit.
- Actual parent helper process death while the root fixture already holds the
  worker process handle. The helper owns the last Job, and the worker is blocked
  waiting for Begin. A nonsecret PID coordination file lives in the external
  scratch directory and is removed; PID disappearance is never the exit proof.
- Real NTLM package query/acquire/ISC and successful Delete/Free statuses with
  per-instance live-before-release audits of UTF-16, padded CBT and provider output.
  The provider allocation is examined after wiping and before FreeContextBuffer,
  never after freeing. Normal cleanup status differs from merely requesting it.

The private parent_loss_child test is a helper selected by the parent-loss test;
when invoked directly without its private coordination variable it returns
without launching. Fixtures have finite held-process waits. Native blocking IPC
may still require an external campaign timeout; no realtime OS bound is promised.

These rows do not prove completed remote NTLM/Kerberos, Digest, actual CompleteAuthToken
provider reachability, TLS/EPA, blocking provider cancellation, descendant behavior,
full identity/impersonation qualification, installed host provenance or HTTP/Git
success. Job kill is containment, never a physical wipe or external LSASS/provider
abort claim. Native execution results belong to the owner/evidence campaign and
must be distinguished from cross-target checks.
