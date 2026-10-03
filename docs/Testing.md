# Test tiers

| Tier | Command | Scope |
|---|---|---|
| Fast | cargo test --lib --locked | Pure Rust strict codec, borrowed/owned records, bounds, fake partial framing and wipe audit; no process/sleep/service. |
| Caller | cargo test --test caller_values --locked | Checked TokenLimit and borrowed-source ownership. |
| Full | cargo test --all-features --locked | Fast/caller tests, 12 negative Clone/Debug doctests and bootstrap refusal. |
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

Conversation phase/allowed kind, Error.phase, immutable deadlines, terminal
publication arbitration, registration/capacity retention, native UTF-16 storage,
provider buffers and process/Job/thread disposal remain later gates. The supplied
codec context is admission input, not a live state machine. No lifecycle/native
success is claimed by these tests. The worker still refuses all invocations.

Generation requires taut-proto==0.10.0 and the exact Rust 1.95 rustfmt named in
protocol/generator.json. Check both authored IR/fingerprints and Rust projection:

```sh
python -B scripts/regen_schema.py --check
cargo fmt --all -- --check
rustfmt --edition 2024 --check src/protocol/tests.rs src/protocol/bounds_tests.rs src/protocol/identity_adapter_tests.rs src/protocol/framing_tests.rs
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Cargo builds/tests are Python-free. Public CI/archive fixtures are self-contained,
synthetic and need no private workspace member. Inside gwz-dev use an external
CARGO_TARGET_DIR; experimental campaigns belong in the private evidence member,
with compiled outputs outside it. No measured performance budget is claimed.
