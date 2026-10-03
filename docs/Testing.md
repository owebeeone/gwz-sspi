# Test tiers

| Tier | Command | Scope |
|---|---|---|
| Fast | cargo test --lib --locked | Codec, parent kernel/context/futures, serial worker/native bridge, fake partial ports and wipe audit; no process/thread/sleep/service. |
| Caller | cargo test --test caller_values --locked | Checked TokenLimit and borrowed-source ownership. |
| Full | cargo test --all-features --locked | Fast/caller tests, negative Clone/Debug doctests, compiled caller/early-dispatch walkthroughs and silent bootstrap refusal. |
| Schema | python -B -m unittest discover -s tests/schema -v | 13 pinned-taut/tooling/cap-model tests; synthetic data only. |
| Bootstrap | cargo test --locked --features worker-bin --test worker_bootstrap | Real child process; silent malformed/metadata refusal with no argument echo. |
| Native Windows | See NativeFixtures.md; ignored unit and native_worker targets | Opt-in production Windows owner/process/provider fixtures, no server. Platform compilation is not execution or full qualification. |

The fast suite uses 14 independently generated pinned-taut reference vectors
covering all seven message bodies, CurrentLogon/Explicit/Digest identities,
Unresolved/provisional/authoritative mechanisms, empty tokens and native status
bit patterns. Rust borrowed and owned walks must exactly reproduce those vectors.
Malformed tests cover every fixture truncation, map count/key closure at every
nested record, scalar/enum type errors, unknown/duplicate/missing fields,
noncanonical encodings, NUL/invalid UTF-8, huge lengths, depth and trailing data.
Semantic tests exercise important bound±1 cases, package/identity/Digest/binding
relations, fingerprints and SID/LUID/session validation, cap/provider intersection,
round correlation, attributes/status limits and the mechanism matrix.

Fake framing owns one fixed zeroizing buffer. Tests cover every partial
header/body offset for EOF, abort, Drop, unfinished writer completion, zero writes
and over-accounted writes. Seed 0x78c010635e3a2941 runs 128 schedules for each
reference vector, prints seed/case/fixture/chunk trace on failure, and keeps the
fast loop bounded. These fake byte transfers never start pipes or workers.

Wipe probes are per-test owned Arc/Mutex records. Storage Drop zeroizes first,
then a probe examines the still-live initialized buffer and records only
(length, all_zero), before deallocation. Probes cover all owned secret fields,
whole frames, refused decoding and abandoned partial readers/writers. No global
observer, freed-memory read, public callback or secret capture exists. The
production probe is zero-size. Normal owned Drop evidence does not prove native
provider disposal, process termination erasure, or caller-source wiping.

Parent conversation phase/allowed kind, Error.phase, immutable deadlines, terminal
publication arbitration and registration/capacity retention are now tested with
private fake ports/clock. Native provider UTF-16 buffers and Windows process/Job disposal have opt-in
fixtures; complete runtime/provider qualification remains a separate gate. Codec context alone
is not a live state machine. The matching worker now runs the shared serial native entry; absent/malformed
compile-time metadata still refuses.

Generation requires taut-proto==0.10.0 and the exact Rust 1.95 rustfmt named in
protocol/generator.json. Check both authored IR/fingerprints and Rust projection:

```sh
python -B scripts/regen_schema.py --check
cargo fmt --all -- --check
python -B scripts/check_rustfmt.py
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Cargo builds/tests are Python-free. Public CI/archive fixtures are self-contained,
synthetic and need no private workspace member. Inside gwz-dev use an external
CARGO_TARGET_DIR; experimental campaigns belong in the private evidence member,
with compiled outputs outside it. No measured performance budget is claimed.

## Parent supervision checkpoint

The fast tier now also runs the production parent phase bridge, partial ReadPort/
WritePort framing, private future polling, Context admission/tombstones and pure
Kernel transitions. Fake clock storage is per context behind an enclosing test
module; no public injection API or mutable global is exposed. Production samples
Instant inside the state guard; a stale pre-lock timestamp cannot bypass expiry.
Tests use no sleeps/processes/services or production launch/reaper loops.

Schedules cover FIFO and capacities 1/8/64, cancellation/drop before and after
registration, closed/saturated/quarantined admission, retained failed start input
wiping, ID overflow, 256-entry FIFO eviction/foreign IDs, queued launch permits,
late successful creation without resume, all five required completion proofs,
strict Hello/Token/Finished kind/round/Error.phase, malformed bootstrap frames,
Finish before Begin and after each complete round, both write/reply orders,
every cancel/write/reply permutation, and ready result polled after cancellation
or expiry (including already reaped normal Finish). Seeded kernel tests use seeds
1, 0x82a513c8, 0xdeadc0de and 0xffffffffffffffff, 128 cases ×128 actions each;
failures print seed and complete action trace. Reentrant/panicking Waker tests
inspect state-lock availability during wake and final replacement/drop.

The fake ports exercise actual fixed frame reader/writer partial EOF/error/zero/
over-accounting paths. Existing live-before-deallocation audits remain in force;
a retained failed future additionally proves its unregistered owned request wipes
at Ready refusal. Send/Sync checks and negative Clone/Debug lifecycle doctests
cover the public ownership surface. The lifecycle recipe in Supervision.md compiles
without a runtime and performs no authentication during doctests.

Local host verification is Darwin/macOS. Windows MSVC and GNU target checks and
strict Clippy inspect Windows branches; they are not execution evidence. Deferred
Windows runtime rows include actual origin-thread impersonation/primary changes
across executor moves, CreateProcess attribute/handle inheritance, creation-time
Job containment/refusal/late launch, blocked synchronous read/write cancellation,
failed kill/wait/Job observations, descendant draining and handle/thread cleanup.
The shared production worker now implements local Negotiate/NTLM native work,
with opt-in owner/process fixtures in [NativeFixtures.md](NativeFixtures.md).
Digest remains unavailable before credential/context work. Windows cross-target
checks do not close the listed runtime rows; installed-worker composition and
provider/HTTP mechanism qualification remain separate gates. Missing/malformed
compile-time metadata and malformed bootstrap still refuse silently.

## Supervisor remediation regressions

Public compilation checks require start/shutdown futures to be Future + Send +
'static, retaining both after dropping Supervisor; step still borrows Conversation.
Focused RED reproduced the original receiver-lifetime errors, Protocol replacing
saved Cancelled after reap, and a retained failed challenge with no wipe event.
The corrected tests cover cancellation signal and fake-clock expiry, both taken
and retired token orders, and oversized/wrong-phase/cancelled/expired challenges
with expected error kinds, wiping before Ready refusal and no frame sent.

Fake successful Origin capture checks observable synchronous disposal during all
unregistered refusal paths and Drop, outside the state lock. The docs explicitly
permit that captured-handle close, without a hard OS-time bound; metadata/provider
queries, worker/thread creation, IPC and process waits/joins remain absent from poll.

Production bounded launch/read/write/reap/join iterations now run against fake
Platform/Child/ports and owned task completions. Tests cover late successful launch
without resume or secret writes, failed containment/resume/launch, failed terminate
and held exit/Job observations, unfinished reader/writer/launch owners, full
reader/writer-loop errors and caught panics, join-error outcomes and eventual
payload/port/child disposal before joining launch and releasing capacity. An actual
fake-port Hello→Begin→Token→Finish→Finished lifecycle uses the production bridge and
future polls. Another pauses after real Token readiness and reaps before publication
for each cancellation/expiry and token ownership order.

Kernel seeded schedules remain kernel evidence. Separate production reaper
schedules use seeds 1, 0x4a2ff930, 0xc1a58b01 and 0xffffffffffffffff, 32 cases per seed
and up to 64 completion actions per case, printing seed/case/actions/effects on
failure. These tests execute the real reaper decisions and owned joins/disposal;
they do not manually set all completion proof bits. Fake owners implement the same
private completion port as production actual JoinHandles. No fast test creates a
thread/process or sleeps. Native thread creation/join and Windows runtime rows
remain deferred qualification; fake completion outcomes are not native evidence.

The trusted-host packaging construction and owned-future handoff walkthrough
compiles in Rustdoc. Its build_fingerprint is explicit synthetic metadata whose
production producer remains step 4; there is no invented runtime hash derivation.


## Serial native worker checkpoint

Meaningful initial RED: the production worker bridge returned Protocol for both
legal pre-Begin Finish and clean EOF (two failing tests). Both pass after the
serial phase/ownership bridge implementation. Later fixture-name typos were test
harness errors, not claimed native RED evidence. The worker fake suite exercises
that same production bridge with explicit private Provider/Session/Output ports,
no processes/threads/sleeps or public fake injection. Initial default and Unicode
explicit identity, Hello-before-native, strict package cap before acquisition,
all four supported ISC statuses/Complete branches, unknown statuses, unavailable
or incompatible mechanism observations, every acquisition/init/completion/query/
output-free/cleanup error, empty and bound±1 output, exact challenge correlation,
Complete-only Finish and round-8 Continue refusal are covered. Digest cap/profile
validation is followed by ProviderRejected with zero acquisition/context calls,
reflecting the documented missing H(Entity) contract input.

Private per-instance probes attach to the actual owned worker input/output frames,
request fields, challenge/token copies and fake provider output. They inspect live
initialized storage after wiping and before release, including partial/error
paths and retained owners. Portable fixed aligned UTF-16 and CBT tests cover
Unicode code-unit lengths and the whole padded allocation; Windows native owner
fixtures additionally inspect actual provider bytes before checked free. Caller
borrowed source wiping remains independent. No callback/global/freed-memory read
or physical-erasure claim exists.

Every truncation of the worker Begin/Finish input stream and every outbound byte
offset failure is exercised through actual framing and serial cleanup. Seeded
worker schedules use 1, 0x649f109d, 0xda26cb46 and u64::MAX, 64 cases each, varying
round count, explicit/default identity, partial read/write chunks and native
failure point. Failures print seed/case/chunks/rounds/fault and nonsecret action
trace. Pure parent schedules and fake completion outcomes remain distinct from
native evidence.

Public early-dispatch tests check malformed/overflow/equal/signed handles, no
argument echo, no runtime metadata activation, platform refusal and Send/Sync.
The minimal binary unit test checks missing/malformed/exact compile-time metadata
parsing without source mutation. WorkerBootstrap negative trait doctests and the
compiled trusted fingerprint early handoff are part of full cargo test.

Use NativeFixtures.md for the ignored production process/native owner rows. Their
inputs, trusted compile-time fingerprint and CBT are synthetic; initial NTLM tokens
stay local. Cross-target checks inspect both Windows branches, but neither those
checks nor fake output prove completed NTLM/Kerberos, actual completion-status
provider reachability, TLS/EPA, real blocked providers, Digest, HTTP/Git success
or installed host provenance. Digest and host packaging producer remain open;
this checkpoint does not close all plan step 3 or qualify Windows activation.

## Native fixture remediation

Portable test-only cleanup guards are attached immediately after helper spawn,
before PID publication or observation acquisition. Deterministic fake cleanup
ports cover each unwind boundary and failed cleanup while still disposing owned
scratch. The original guard tests were RED with no cleanup events and missing
scratch disposal, then GREEN with the guarded implementation.

Opt-in Windows failures additionally force after-spawn, after-PID and observer
acquisition/unwind failures. They require finite held-handle exit observation
before consuming Child::wait and report unconfirmed cleanup as failure; a kill
request is never confirmation. Scratch creation uses create_new and its owner
removes only that file. Native execution requires separate campaign receipts.

Job closure and actual parent death now hold production children still suspended,
so bootstrap/EOF cannot cause their exit. A fixture-owned Job without kill-on-close
is a runtime negative control and has guarded explicit terminate/exit cleanup.
Earlier resumed-child receipts had EOF as a competing exit explanation; they do
not prove this stronger row. The production Job policy remains unchanged.

The local initial Negotiate fixture executes actual QueryContextAttributesW and
checks release of any returned package allocation. Its per-instance receipt records
only query status, allocation presence and successful checked release. Initial
Unresolved, provisional or authoritative selection is allowed according to native
status; this is not completed remote Kerberos/NTLM evidence.
