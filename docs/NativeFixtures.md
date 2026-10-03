# Opt-in Windows fixtures

These self-contained public fixtures execute the production worker and native
owners, never a success-shaped worker stub. Default fast tests do not execute
them. Use pinned Rust 1.95, an external build target and a fresh external Windows
runtime/scratch directory (inside this workspace follow EVIDENCE.md). Build with
synthetic metadata only for the fixtures:

```powershell
$environmentNames = @('GWZ_SSPI_BUILD_FINGERPRINT', 'CARGO_TARGET_DIR',
    'GWZ_SSPI_NATIVE_SCRATCH', 'GWZ_SSPI_NATIVE_WORKER')
$savedEnvironment = @{}
foreach ($name in $environmentNames) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
$fixtureRoot = Join-Path 'D:/gwz-tests' ('sspi-native-' + [guid]::NewGuid().ToString('N'))
$fixtureRootOwned = $false
$evidenceRetained = $false
try {
    # No -Force: claim only a newly created root, never an existing directory.
    New-Item -ItemType Directory -Path $fixtureRoot -ErrorAction Stop | Out-Null
    $fixtureRootOwned = $true
    $env:GWZ_SSPI_BUILD_FINGERPRINT = '42' * 32
    $env:CARGO_TARGET_DIR = Join-Path $fixtureRoot 'target'
    $env:GWZ_SSPI_NATIVE_SCRATCH = Join-Path $fixtureRoot 'scratch'
    New-Item -ItemType Directory -Path $env:GWZ_SSPI_NATIVE_SCRATCH -ErrorAction Stop | Out-Null
    cargo build --locked --features worker-bin
    if ($LASTEXITCODE -ne 0) { throw 'Worker build failed' }
    $env:GWZ_SSPI_NATIVE_WORKER = (Resolve-Path "$env:CARGO_TARGET_DIR/debug/gwz-sspi-worker.exe" -ErrorAction Stop).Path
    cargo test --all-features --locked --test native_worker -- --ignored --test-threads=1 --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Public native fixtures failed' }
    cargo test --lib --all-features --locked -- --ignored --test-threads=1 --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Native owner fixtures failed' }
    # Retain required receipts outside this root, then set $evidenceRetained = $true.
} finally {
    foreach ($name in $environmentNames) {
        # Null restores absence; existing values are restored exactly.
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
    }
    if ($fixtureRootOwned -and $evidenceRetained) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -ErrorAction Stop
        $fixtureRootOwned = $false
    } elseif ($fixtureRootOwned) {
        Write-Host "Retain receipts, then dispose owned fixture root: $fixtureRoot"
    }
}
# After success or a caught failure, retain receipts before this explicit teardown:
# $evidenceRetained = $true
# if ($fixtureRootOwned -and $evidenceRetained) {
#     Remove-Item -LiteralPath $fixtureRoot -Recurse -ErrorAction Stop
#     $fixtureRootOwned = $false
# }
```

The four environment values, including originally absent values, are restored on
success or failure. Each cargo failure throws into the same finally block. Set
the retention flag only after copying the required receipts outside the fresh
owned root; teardown then removes that root. Without retained evidence a failed
run deliberately preserves its fresh root, with the explicit guarded teardown
shown above. No existing directory is removed. Use ordinary exit status to assess
each command; there is no expected-count gate.
The public native_worker target exercises Supervisor with verified actual primary
Hello, Finish before Begin and initial default/Unicode explicit NTLM followed by
confirmed disposal, plus initial local Negotiate observation. Credentials are
synthetic for explicit identity, tokens stay local and no server/account/trust
mutation is required. The binding is synthetic,
so this is not EPA or real TLS fidelity evidence.

Ignored Windows unit fixtures call production creation/bootstrap/native owners:

- Normal EOF before Begin and after an initial native token; worker exit status 0,
  held process exit, Job active count zero and pipe EOF after owned writers close.
  No Finished is inferred from EOF.
- Last owned Job closure while an independently held process handle observes exit
  of a still-suspended production child. A fixture-owned Job without the kill flag
  must leave its suspended child alive; guarded terminate/exit cleanup follows
  even on assertion failure. No production Job policy is configurable.
- Actual parent helper death while the root fixture holds the still-suspended
  worker process handle. Bootstrap and EOF cannot execute in these children.
  Forced failures after spawn, PID publication and observer acquisition exercise
  the immediate helper guard, finite held-handle exit proof and owned scratch
  disposal. The nonsecret PID file is create_new in external scratch; PID
  disappearance and kill requests are never exit proof. Earlier resumed-child
  receipts had EOF as a competing exit explanation and do not prove these rows.
- Real NTLM package query/acquire/ISC and successful Delete/Free statuses with
  per-instance live-before-release audits of UTF-16, padded CBT and provider output.
  The provider allocation is examined after wiping and before FreeContextBuffer,
  never after freeing. Normal cleanup status differs from merely requesting it.
- Real initial Negotiate QueryContextAttributesW, actual Unresolved/provisional/
  authoritative observation and checked release of any returned allocation. The
  receipt prints only fixed status/selection and allocation/release booleans; no
  token or credential contents. This is local initial negotiation only.

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
