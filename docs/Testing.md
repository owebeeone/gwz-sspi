# Test tiers

| Tier | Command | Scope |
|---|---|---|
| Fast | cargo test --lib --locked | Pure codec, parent kernel/context/futures, fake partial ports and wipe audit; no process/sleep/service. |
| Caller | cargo test --test caller_values --locked | Checked TokenLimit and borrowed-source ownership. |
| Full | cargo test --all-features --locked | Fast/caller tests, 16 negative Clone/Debug doctests, compiled caller walkthrough and bootstrap refusal. |
| Schema | python -B -m unittest discover -s tests/schema -v | 13 pinned-taut/tooling/cap-model tests; synthetic data only. |
| Bootstrap | cargo test --locked --features worker-bin --test worker_bootstrap | Real child process; fixed refusal with no argument echo. |
| Native Windows | Future opt-in targets under tests/native/windows | No native qualification is implemented. Platform compilation is not qualification. |

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
private fake ports/clock. Native provider UTF-16 buffers and Windows runtime
process/Job/thread disposal qualification remain later gates. Codec context alone
is not a live state machine. The worker still refuses all invocations.

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
Native provider UTF-16 buffers/SSPI disposal, installed-worker composition and
provider/HTTP mechanism qualification remain later gates. No synthetic child
success target or native campaign is included in this checkpoint. Production
worker bootstrap continues its fixed refusal.

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
