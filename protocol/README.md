# Private worker protocol

`sspi.taut.py` authors the SSPI IPC messages; `sspi.ir.json` is the pinned exported
IR, `generator.json` names the tool release, and `contract.json` fingerprints both
IR and the [wire semantics](../docs/WireProtocol.md). No production codec exists.
Reference bindings are unsafe for secrets; synthetic schema tests only.

Install taut-proto==0.10.0 into a dedicated Python environment, then from this
repository run:

```sh
python -B scripts/regen_schema.py --check
python -B -m unittest discover -s tests/schema -v
```

Omit --check only to regenerate after deliberate authored changes. A mismatched
release or shadow import refuses. Cargo builds remain independent of Python.
No gwz-core application envelope or gwz-transport schema is changed.
