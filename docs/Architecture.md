# Architecture and package boundary

Role: **integration** library with owned contract/protocol values and narrow
injected OS, IPC and clock ports. One reviewed package is sufficient for this
initial boundary; do not create a global common crate or one crate per method.

Dependency allowlist: **zeroize =1.9.0, default features disabled, alloc only**.
Explicit lane-owner source review before addition is recorded in the GWZ secret
codec checkpoint; the subsequent dual review judges its actual ownership usage.
No other production/development dependency is approved. All dependency
kinds, including target, optional, build and dev, require explicit review before
addition. Native Windows bindings remain a future dependency choice. The reviewed
zeroize dependency supplies normal owned storage wiping for this checkpoint. Public caller types will not expose
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


Implemented owned caller values live in src/secret.rs and src/values.rs. Private
protocol generation, profile, adapters and pure framing live in src/protocol;
see CallerValues.md and Testing.md. Borrowed generated records are tied to a
held zeroizing frame; semantic/context admission precedes owned secret copies.
Encoding validates and computes its entire size before allocating zeroed fixed
storage. No live payload reallocation or generic owned CBOR tree is used.

This checkpoint does not implement Supervisor/Conversation, actual anonymous
pipes, registration, terminal arbitration, native UTF-16/provider buffers or
process containment/disposal. The worker still refuses. Codec cap/fingerprint/
identity/round checks do not establish phase, publication eligibility or cleanup.
