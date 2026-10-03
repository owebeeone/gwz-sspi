# SSPI implementation checkpoint

2026-10-03. **Scaffold accepted** after independent Code/State GO on root
e6772cbeda821deb7f2f42d63f73d9a3581e60ec and member
bd807b0403d502fd6f25ef2f93389ce432f0c0c0. Repository/package/release
structure only. The Code review's nonblocking missing packaged status link was
corrected by including this file explicitly; extracted-archive links pass.

Accepted design tuple in gwz-dev: root f2029e4b1739c0214138675dfb16abdb44f6a0d7,
core d78a664e3c5a325c6f12be409eb7645c1c1b51d0, evidence
1beb1d204c824701ddbd033c7f89df9a3561f5e5. Design GO is not implementation GO.

Private taut schema/design and required TokenLimit amendment are accepted after
Consistency/Safety/Surface GO at root 05403cea8018cf14a793977ddec2c0c7136ccc54,
member 7ea900e77f272dd6e0d64f566c59fb29322f5738, core
8cb3a3f01d79699a5ad07b6ec7cfc78321224d31. The captured WireProtocol.md pre-review
DRAFT banner is historical; its exact reviewed bytes remain unchanged because
they participate in the contract fingerprint. Status is recorded here separately.
See protocol/README.md and docs/WireProtocol.md; exported IR and fingerprints are
reproducible with taut-proto 0.10.0. The exact authored schema, exported IR,
WireProtocol.md and contract bytes remain unchanged.

Caller values and the private secret-rust-v1 projection are now drafted for dual
Code/State secret-boundary review. Implemented: fixed zeroizing SecretBytes/Text,
checked required TokenLimit, owned request/identity/Digest/token/error values,
IR-generated borrowed/owned records and canonical CBOR walks, borrowed admission
before secret copies, validated two-pass encoding, pure exact partial framing
and explicit request/token/error adapters. See docs/CallerValues.md and
[testing](../docs/Testing.md) for exact implemented and deferred obligations.

The sole dependency is reviewed zeroize =1.9.0, defaults disabled, alloc only.
No generic owned CBOR tree, ordinary owned secret temporary, Clone/Debug secret
records, global observers, native handles, runtime registration or process effect
is introduced. Test audits inspect live storage after wiping and before release.
Normal Drop is not physical erasure, native UTF-16/provider disposal or caller
source wiping. All private protocol records remain crate-private.

Next: settle dual Code/State review, then supervision kernel and native worker,
review, CLI/Python composition, installed-worker qualification and Windows parity.
Supervisor/Conversation and conversation-phase/terminal publication arbitration
remain unimplemented. The worker exits with its original fixed refusal. Keep
publish=false until implementation/qualification and registry/remote setup are
concrete. No release, tag, push or registry mutation.
