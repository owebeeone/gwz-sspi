# Test tiers

| Tier | Command / selection | Rules |
|---|---|---|
| Fast | cargo test --lib --locked | Pure/contract unit tests; fake time and ports; no process, sleeps or service. Currently zero behavioral tests. |
| Contract | Dedicated integration targets under tests/contract | Same lifecycle/framing invariants against fake ports and later native adapters. Add explicit Cargo test targets when implemented. |
| Replay | Dedicated targets under tests/replay | Fixed default seeds, bounded cases; print seed/input/trace on failure. Larger campaigns opt in, never expand the fast loop silently. |
| Schema | python -B -m unittest discover -s tests/schema -v | Pinned taut-proto==0.10.0; synthetic values only; no native/process I/O; not proof of production admission or zeroization. |
| Bootstrap | cargo test --locked --features worker-bin --test worker_bootstrap | Real child process; currently proves fail-closed scaffold and no echoed arguments. |
| Native Windows | Dedicated opt-in targets under tests/native/windows | Real SSPI/Job/IPC/identity tests. Not implemented; platform compile checks alone are not qualification. |

Fixtures required by public CI live here with synthetic credentials only. Raw
experimental campaigns/runners go in GWZ's private evidence member, build outputs
outside both repositories. Public tests never require that member. Native tests
must own/reap every process; no shared trust/credential/agent mutation by default.

When adding executable conformance targets, document their exact invocation and
measure warm/cold execution separately. No measured performance budget is claimed
by this scaffold. Review stops: codec/API secrets before supervision implementation;
native credential storage/disposal before host integration; aggregate acceptance
before production use. Do not publish zero tests as proof of native correctness.

The schema cap model specifies the required caller input, exact Begin transfer
and bound/error cases with fixed synthetic data. It is not a Rust implementation
or proof that future IPC/native adapters enforce those cases.
