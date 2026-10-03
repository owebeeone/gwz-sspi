# gwz-sspi

Windows-specific SSPI authentication in a contained worker process, with a Rust
caller API. The CLI can self-execute its worker entry; Python can bundle the
matching standalone worker. This is not a network stream or generic worker system.

**Status: package scaffold, not an authentication implementation or release.**
The library compiles independently. The optional executable deliberately refuses
all invocations. Publication is disabled in Cargo.toml until implementation review
and Windows qualification pass. No configured remote or registry publication.

## Layout

| Path | Ownership |
|---|---|
| src/contract/ | Owned caller values, errors and lifecycle API |
| src/protocol/ and protocol/ | Private taut schema, generated codecs and secret adapters |
| src/supervisor/ | Admission, cancellation, cleanup and retained capacity |
| src/platform/windows/ | Native SSPI, Jobs, process creation and pipe ownership |
| src/worker/ and src/bin/ | Shared worker entry and minimal packaged executable |
| tests/contract/ and tests/support/ | Deterministic fake-port conformance |
| tests/replay/ | Seeded schedule and randomized fault/reassembly cases |
| tests/native/windows/ | Explicit native fixtures, separate from fast tests |
| scripts/ | Public repository checks and generation tooling |
| docs/ and dev-docs/ | Caller/testing documentation and review checkpoints |

Boundary directories have ownership notes; they are not unimplemented public Rust
modules. The caller API lands in the first reviewed implementation chunk.

## Development

```sh
cargo test --lib --locked
cargo test --locked --all-features
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
cargo package --locked --allow-dirty --no-verify
```

The first command is the future fast-library tier; it currently contains no
behavioral SSPI tests. All-features currently adds only the executable-refusal
test. Native authentication qualification has not run against this package.
Use an external CARGO_TARGET_DIR when working in gwz-dev.

[Architecture](docs/Architecture.md), [testing](docs/Testing.md),
[release instructions](RELEASE.md), [implementation status](dev-docs/Implementation.md).
Gearu owns release preparation; GitHub Actions owns crates.io publication.
