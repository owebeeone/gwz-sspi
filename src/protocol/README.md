# Ownership

Private generated borrowed/owned records and direct strict CBOR walks come from
protocol/sspi.ir.json through scripts/rust_projection.py. Generated files are not
edited by hand. No generic owned CBOR tree or reference owned Rust binding is used.

Borrowed records point into held zeroizing frames. profile.rs admits the closed
wire profile and supplied fingerprint/identity/cap/package/round context before
owned secret fields are copied. Encoding validates and counts the complete body
before allocating initialized fixed storage and copying payloads. framing.rs is
pure exact partial-byte accounting, with no real I/O or process effects.

adapters.rs explicitly maps caller requests and admitted private token/error
values. Token conversion alone is not permission to publish after cancellation:
conversation phase, Error.phase and terminal arbitration are deferred supervisor
responsibilities. All private protocol code currently has a scoped dead_code
allowance because the supervisor/worker runtime remains unimplemented.
