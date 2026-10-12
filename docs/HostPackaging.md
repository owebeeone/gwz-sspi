# Trusted host artifact sets

The build-only producer `scripts/artifact_set.py` supplies a SHA256 identifier
for an explicitly selected source artifact set. Cargo metadata resolves the host,
its local dependencies (including local native forks), and exactly one SSPI
package. Inputs include source/native/vendor directories, generated protocol
contracts, manifests, lock, effective workspace manifest/config, compiler,
target, profile, features, rustflags and recorded build options. Relocation of
identical package inputs does not change the identifier. It is neither a binary
hash nor a signature, and does not claim to cover arbitrary compiler plugins or
unrecorded external build inputs. Trusted release tooling owns those inputs.
There are no credentials in build metadata.

CLI and Python packaging wrappers use this producer from the Cargo-resolved
SSPI package. They compile identical expected bytes into their host and worker
entry. Python wheels carry an adjacent dedicated worker and a nonsecret receipt;
the receipt is diagnostic and is never used as runtime authority. The worker's
compiled Hello must match the descriptor's expected bytes before Begin.

`packaging::build_fingerprint(Option<&str>)` accepts exactly 64 ASCII hex digits.
Absent metadata returns WorkerUnavailable; malformed metadata returns
WorkerMismatch. `packaging::installed_worker(&Path, Option<&str>)` requires an
absolute loaded-image path, selects the fixed adjacent worker filename, rejects
missing/nonregular/symlink workers, and constructs WorkerExecutable. It creates
no Supervisor, process or I/O. The caller must supply a trusted loaded-image
path, not a Python attribute or caller-selected path. Executable replacement is
still checked by the existing Hello protocol; selection does not authenticate
file contents itself.

```rust
use gwz_sspi::{packaging, WorkerExecutable, Error};
use std::path::Path;
fn installed_descriptor(image: &Path) -> Result<WorkerExecutable, Error> {
    packaging::installed_worker(image, option_env!("GWZ_SSPI_BUILD_FINGERPRINT"))
}
```

Ordinary Cargo builds remain Python-free and unprovisioned. Provisioning is an
explicit host packaging operation. A provisioned portable build can construct a
descriptor, but worker entry/Supervisor still refuse UnsupportedPlatform outside
Windows. Digest remains refused. No HTTP bridge or platform activation is added.
This checkpoint does not establish Windows installed Hello mismatch, complete
authentication, EPA, native cleanup or endpoint operation; those need their
separate qualification rows. `publish = false` remains in effect; registry
availability of the pinned SSPI package is a release prerequisite, not bypassed
by local path builds.

Compiler executable/wrapper overrides are refused rather than reported as the
ordinary rustc compiler. Conventional Cargo config files from the workspace,
invocation ancestry and Cargo home are fingerprinted; configured compiler
overrides are conservatively refused without adding a Python3.11 parser.
Host wrappers document installation/removal and artifact output locations.

The producer conservatively records all supplied conventional rustflags channels:
CARGO_BUILD_RUSTFLAGS, RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS and target-specific Cargo
rustflags, together with configuration-file digests. Even a supplied channel
shadowed by precedence may change the identifier. Provisioned wheel validation
checks complete RECORD hashes before the host backend publishes owned staging;
receipts remain nonsecret diagnostic data and do not authenticate worker bytes.

## Installed paths and worker provenance (WH3, gwz-core plan step 4.10)

An installed CLI or wheel works wherever it is installed, including under a directory with spaces and one with
non-ASCII characters, and its worker is the file the install put there. These rows say what is tested where; the
Windows rows ran on a host with the artifacts built by this section's producer and installed under new directories
(`GwzTransportWindowsParityPlan.md`, step 4.10; the run is archived with the HTTPS qualification campaign).

| Row | Claim | Where it is tested |
|---|---|---|
| Worker at its packaged path | `packaging::installed_worker` selects the fixed adjacent file whatever the directory is called: plain, spaces, non-ASCII, a trailing dot | `tests/packaging.rs`, `installed_paths::the_worker_is_found_at_its_packaged_path_whatever_the_directory_is_called` |
| Wheel descriptor | The Python extension names the worker beside its own loaded image, from the loader and not from a Python attribute | gwz-py `native/src/worker_host.rs`, `the_wheels_worker_is_the_file_beside_the_extension_wherever_the_wheel_is_installed`; on the host, `wh3_python_rows.py` (`descriptor_is_beside_extension`) |
| CLI descriptor | The CLI's worker is its own executable under the name the loader gives it | gwz-cli `src/worker_host.rs`, `the_installed_descriptor_is_the_executable_wherever_it_is_installed`; on the host, the installed `gwz.exe` runs its NTLM and Negotiate rows from each directory |
| No `PATH` discovery | A worker elsewhere, including a valid one first on `PATH`, is never found; a missing worker is `WorkerUnavailable` | `tests/packaging.rs`, `a_worker_elsewhere_is_never_found_there_is_no_path_discovery`; on the host, the wheel with its worker deleted and a valid worker on `PATH`, and the CLI with a different build's worker first on `PATH` |
| Not a worker | A directory, or on Unix a symlink, named as the worker is not one | `tests/packaging.rs`, `a_directory_named_as_the_worker_is_not_a_worker`, `symlink::a_symlink_named_as_the_worker_is_not_a_worker` |
| Fingerprint first | A missing build fingerprint is `WorkerUnavailable`, a malformed one `WorkerMismatch`, whatever the filesystem holds | `tests/packaging.rs`, `the_build_fingerprint_is_judged_before_the_filesystem_is_touched`; gwz-cli `an_unprovisioned_or_mismatched_build_has_no_descriptor_and_no_fallback` |
| Mismatched worker | A worker whose compiled Hello is another build's is refused before `Begin`: no SSPI exchange reaches the server | `src/protocol/supervision_tests.rs` (the Hello check); on the host, the wheel with a different build's worker in place |
| Foreign file | A file that is not a worker (a system executable renamed) is refused and ended, with no hang | on the host, the wheel with a renamed system executable in place |
| Concurrency | Concurrent calls on one wheel client, and a second client beside it, complete with no pending work | on the host, `wh3_python_rows.py` (`concurrent_pass`) |
