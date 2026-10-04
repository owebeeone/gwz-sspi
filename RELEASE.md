# Release Process

<!-- gearu:release:start -->
## Gearu Release Process

Gearu prepares and verifies the repository, creates an immutable tag, and can
create the GitHub Release that starts this repository's publication workflow.
It does not publish directly to package registries.

Full documentation: <https://owebeeone.github.io/gearu/>

### Install

Install the released tool with:

```sh
uv tool install gearu
```

Upgrade an existing installation with:

```sh
uv tool upgrade gearu
```

To test the unreleased `main` branch, install it directly from its repository:

```sh
uv tool install git+https://github.com/owebeeone/gearu.git
```

Verify the installation with `gearu --version`.

### Preconditions

- Read `gearu.toml` and this repository's release workflow.
- Choose an explicit release version or an explicit major, minor, or patch bump.
  Gearu does not infer release intent from commits.
- Use a clean checkout on the branch configured by `project.branch`.
- Synchronize configured release and source branches with their remote.
- Release required cross-repository dependencies first.
- Install and authenticate `gh` before requesting GitHub Release creation.

### Plan

Always inspect the read-only plan first:

```sh
gearu plan VERSION
```

Or ask Gearu to select the next version:

```sh
gearu plan --bump patch
gearu plan --bump minor
gearu plan --bump major
```

Gearu compares configured package versions with valid local and remote release
tags, then bumps the highest version. It reads remote tags directly and does not
fetch or create local tags while planning.

For a release candidate, use a numbered version such as `1.2.3-rc.1`.

Override a configured dependency tag only when the release intentionally uses a
different version:

```sh
gearu plan VERSION --dependency-tag DEPENDENCY=TAG
```

### Prepare the Local Release

After reviewing the plan:

```sh
gearu release VERSION
```

The release command can select the version itself:

```sh
gearu release --bump minor
```

This recalculates the next version at release time. To lock the version reviewed
in a prior bump plan, pass that plan's reported `VERSION` explicitly.

Gearu builds and tests in a temporary worktree. Only a successful candidate is
applied to the local release branch and tagged. This step does not change a
remote repository.

### Push and Create the GitHub Release

Push the exact release commit and tag atomically:

```sh
gearu release VERSION --push
```

Create the GitHub Release after that push:

```sh
gearu release VERSION --push --github-release
```

The final command starts workflows listening for `release.published`, including
package publication and documentation deployment where configured.

### Recovery

- If candidate checks fail, fix the problem and rerun; the normal checkout is
  left unchanged.
- If local preparation succeeds, rerun the same version with `--push`.
- If the push succeeds but GitHub Release creation fails, rerun with
  `--push --github-release`.
- If released contents must change, use a new patch or release-candidate version.
  Never move or replace the existing tag.
- If only a publication workflow fails, repair and rerun that workflow for the
  same GitHub Release.
<!-- gearu:release:end -->

## SSPI release gate

This repository is scaffold-only. Cargo.toml has publish=false and the release
check refuses preparation/publication until that guard is deliberately lifted
following implementation acceptance and Windows qualification. Do not bypass it
for an empty bootstrap package: the placeholder in .github/bootstrap-crate is a
separate package and leaves this gate untouched. There are no release tags yet;
the GitHub repository, github.com/owebeeone/gwz-sspi, is configured and public.

After the implementation gates pass:

1. Commit clean source to main through GWZ during ordinary workspace
   development (origin is configured; see above).
2. Arrange first crates.io registration and the trusted publisher for this exact
   repository, release.yml workflow and crates-io environment. The first
   registration is .github/workflows/bootstrap-crate.yml, run once by hand with
   the CRATES_IO_BOOTSTRAP_TOKEN secret: it publishes the placeholder
   0.0.0-bootstrap.1 from .github/bootstrap-crate. Then delete the secret and
   revoke the token. release.yml has no token fallback.
3. Replace scaffold status with accepted implementation documentation, qualify
   the packaged library and Windows worker, and lift publish=false in a reviewed
   activation change. Add packaged-worker assets/wheel composition at that gate.
4. Run gearu plan with the intended version; inspect it, then authorize Gearu's
   release operation explicitly. Gearu release creates a commit/tag locally;
   --push and --github-release are separate external actions.

Gearu's `checks` run the format, lint and test gates on the uncommitted
candidate; its `exact_checks` (`release_checks.py --exact`) package the release
commit, because `cargo package` refuses uncommitted files. Both stages refuse
while publish=false stands, and release.yml reruns both on the tag.

Gearu is the selected release-preparation tool, including its deliberate release
commit/tag mechanism; ordinary development Git mutations still use GWZ. Managed
Gearu guidance above is generated by gearu init, not hand-edited. This scaffold
runs init/configuration validation only, never release, push or publication.

GitHub Actions release.yml listens to release.published, validates immutable tag
against Cargo version and all release checks, then authenticates using crates.io
Trusted Publishing. It does not run from ordinary pushes or PRs. ci.yml validates
standalone package/tests on Linux, macOS and Windows; compilation of this scaffold
is not native SSPI qualification.
