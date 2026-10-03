# gwz-sspi

Windows-specific SSPI library and worker, independently buildable without core,
CLI, Python, git2 or gwz-transport. In a GWZ workspace, follow its AGENTS.md and
AGENTS_GWZ.md and use GWZ for development staging/commits and membership changes.

Use TDD. Accepted scope/API lives in the GWZ SSPI design revision 2; this scaffold
implements no authentication. No success-shaped placeholders. Native buffers,
identity, cancellation, admission and private IPC stay inside the approved boundary.
Do not introduce a generic worker framework or new application/network protocol.

All control-flow bodies require braces. Conditional compilation must use enclosing
platform modules or cfg_if blocks, never individual conditional imports/declarations.
No mutable globals/thread-locals. Keep owned secrets out of Debug/Clone/logs.
Protocol payloads are taut-authored; generated code is never edited by hand.

Fast tests are deterministic and process-free. Seeded schedule/random tests record
seed plus input/trace for reproduction. Native/process tests are separate. Public
CI must not depend on private evidence. Targets/caches stay external; raw campaigns
belong in gwz-core-evidence when running inside the GWZ workspace.

Read docs/Architecture.md and docs/Testing.md before extending the scaffold.

<!-- gearu:agents:start -->
## Releases

- This repository uses [Gearu](https://owebeeone.github.io/gearu/) for release
  preparation.
- Read `RELEASE.md` before planning or performing a release.
- `gearu plan VERSION` and `gearu plan --bump LEVEL` are read-only. Do not run
  `gearu release`, push a release tag, or create a GitHub Release unless the
  user explicitly requests it.
- Never move or reuse a release tag. Correct released content with a new version.
- Never publish directly to PyPI, crates.io, or npm from a local checkout.
  Registry publication belongs in the repository's release workflow.
<!-- gearu:agents:end -->
