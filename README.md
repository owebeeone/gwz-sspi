# gwz-sspi

Windows-specific SSPI authentication in a contained worker process, with a Rust
caller API. The CLI can self-execute its worker entry; Python can bundle the
matching standalone worker. This is not a network stream or generic worker system.

**Status: caller values, private secret codec and parent supervision implemented; authentication and release remain gated.**
The library compiles independently. The optional executable deliberately refuses
all invocations. Publication is disabled in Cargo.toml until implementation review
and Windows qualification pass. No configured remote or registry publication.

## Layout

| Path | Ownership |
|---|---|
| src/secret.rs and src/values.rs | Owned caller secret/request/token/error values |
| src/protocol/ and protocol/ | Private taut schema, generated codecs and secret adapters |
| src/supervisor/ | Admission, cancellation, cleanup and retained capacity |
| src/supervisor/windows/ | Windows identity, Jobs, process creation and pipe ownership |
| src/platform/windows/ | Future native SSPI ownership notes |
| src/worker/ and src/bin/ | Shared worker entry and minimal packaged executable |
| tests/contract/ and tests/support/ | Deterministic fake-port conformance |
| tests/replay/ | Seeded schedule and randomized fault/reassembly cases |
| tests/native/windows/ | Explicit native fixtures, separate from fast tests |
| scripts/ | Public repository checks and generation tooling |
| docs/ and dev-docs/ | Caller/testing documentation and review checkpoints |

Boundary directories have ownership notes; they are not unimplemented public Rust
modules. The implemented [caller values](docs/CallerValues.md) carry no native handles.

## Development

```sh
cargo test --lib --locked
cargo test --locked --all-features
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
cargo package --locked --allow-dirty --no-verify
```

The fast tier checks secret ownership, strict generated codecs, private parent
lifecycle/kernel admission and fake byte ports. All-features adds caller values, negative trait doctests and executable
refusal. Native authentication qualification has not run against this package.
Use an external CARGO_TARGET_DIR when working in gwz-dev.

[Architecture](docs/Architecture.md), [testing](docs/Testing.md),
[release instructions](RELEASE.md), [implementation status](dev-docs/Implementation.md).
Gearu owns release preparation; GitHub Actions owns crates.io publication.

Private [message schema and checks](protocol/README.md) and [wire design](docs/WireProtocol.md)
are preserved from the accepted schema checkpoint. The private IR-driven codec
and caller values passed the dual secret-boundary review. Parent supervision is
implemented pending its separate Code/State/Surface gate; native SSPI remains next.
See [caller lifecycle](docs/Supervision.md).
