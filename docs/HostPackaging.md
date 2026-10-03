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
