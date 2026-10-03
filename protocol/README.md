# Private worker protocol

sspi.taut.py is the sole field/tag/type authority. sspi.ir.json is its pinned
exported IR; contract.json fingerprints its exact bytes and the unchanged
[wire semantics](../docs/WireProtocol.md). That document's historical DRAFT and
future-codec status are retained because its reviewed bytes are fingerprinted;
[implementation status](../dev-docs/Implementation.md) records current progress.

The secret-rust-v1 projection generates private borrowed/owned records, enums,
tags and direct CBOR read/write walks under src/protocol/generated. Handwritten
profile checks own semantic bounds and relations, never a second field schema.
Synthetic pinned-taut golden fixtures are generated under src/protocol.
Reference String/Vec codecs must never receive production secrets.

Install taut-proto==0.10.0 into a dedicated Python environment and use Rust 1.95.0
with the exact rustfmt version in generator.json. From this repository run:

```sh
python -B scripts/regen_schema.py --check
python -B -m unittest discover -s tests/schema -v
```

Omit --check only for deliberate regeneration. Version/origin mismatches refuse.
Fresh contract bytes are passed into the Rust projection in the same generation;
no old on-disk manifest is used. Cargo itself never invokes Python or rustfmt.
No other GWZ member, application envelope or transport schema is required.
