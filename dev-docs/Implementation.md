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
reproducible with taut-proto 0.10.0. Twelve synthetic schema/tooling/cap-model tests pass. No
Rust reference binding or production codec is generated. This does not close the
secret-boundary gate.

Next: caller values and IR-driven zeroizing codecs, fake contract tests,
then dual Code/State secret-boundary review. Then supervision/native worker,
review, CLI/Python composition, installed-worker qualification and Windows parity.
No authentication API or codec exists in this scaffold. The worker exits with a
fixed refusal. Keep publish=false until the accepted implementation/qualification
and registry/remote setup are concrete. No release, tag, push or registry mutation.
