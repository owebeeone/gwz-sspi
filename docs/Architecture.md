# Architecture and package boundary

Role: **integration** library with owned contract/protocol values and narrow
injected OS, IPC and clock ports. One reviewed package is sufficient for this
initial boundary; do not create a global common crate or one crate per method.

Current production/development dependency allowlist: **none**. All dependency
kinds, including target, optional, build and dev, require explicit review before
addition. Native Windows bindings and secret/codec support are future dependency
choices, not silently added by this scaffold. Public caller types will not expose
OS handles, core/Git types or runtime-specific implementation objects.

The approved design is GWZ SSPI revision 2. Inside gwz-dev see
../../dev-docs/GwzSspiDesign.md and GwzSspiAcceptance.md. Those development documents
are not build/CI dependencies; this standalone repository carries its own public
contracts as implementation lands. Parent crate placement/secret exceptions are
specific to this Windows SSPI boundary.

No platform code yet. New platform sections use enclosing modules; keep the fast
state/contract tests available on non-Windows without pretending SSPI works there.
Publication, worker packaging and host activation remain gated. FFI unsafe code
must be confined to audited Windows modules with explicit lifetime/safety comments.
