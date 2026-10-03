#!/usr/bin/env python3
"""Validate/export the private taut IR and semantic contract digest and private Rust projection."""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import site
import sys
import sysconfig

ROOT = Path(__file__).resolve().parents[1]


def released_taut():
    """Refuse non-release metadata and any generator version other than the pin."""
    pin = json.loads((ROOT / "protocol/generator.json").read_text())
    if pin["projection"] != "secret-rust-v1":
        raise SystemExit("unsupported schema projection")
    import subprocess
    formatter = subprocess.run(["rustfmt", "--version"], check=True, capture_output=True, text=True).stdout.strip()
    if formatter != pin["rustfmt"]:
        raise SystemExit("rustfmt release version mismatch")
    try:
        distribution = importlib.metadata.distribution("taut-proto")
    except importlib.metadata.PackageNotFoundError:
        raise SystemExit("install the pinned taut-proto release") from None
    sites = {Path(sysconfig.get_paths()[key]).resolve() for key in ("purelib", "platlib")}
    sites.update(Path(path).resolve() for path in site.getsitepackages())
    sites.add(Path(site.getusersitepackages()).resolve())
    location = Path(distribution.locate_file("")).resolve()
    if location not in sites or distribution.version != pin["taut-proto"]:
        raise SystemExit("taut-proto release location/version mismatch")
    return Path(distribution.locate_file("taut")).resolve()


def verify_taut_modules(package):
    """Reject editable/workspace/PYTHONPATH modules shadowing the pinned release."""
    for name, module in list(sys.modules.items()):
        if name == "taut" or name.startswith("taut."):
            origin = getattr(module, "__file__", None)
            if not isinstance(origin, str) or package not in Path(origin).resolve().parents:
                raise SystemExit("taut module origin mismatch")


def contract_digest(ir, semantics):
    return hashlib.sha256(b"gwz-sspi-contract-v1\0" + ir + semantics).hexdigest()


def artifacts(schema):
    from taut.ir.export import schema_json
    ir = (json.dumps(schema_json(schema), indent=2, sort_keys=True) + "\n").encode("utf-8")
    semantics = (ROOT / "docs/WireProtocol.md").read_bytes()
    manifest = {
        "protocol_version": 1,
        "schema_sha256": hashlib.sha256(ir).hexdigest(),
        "semantics_sha256": hashlib.sha256(semantics).hexdigest(),
        "contract_sha256": contract_digest(ir, semantics),
    }
    outputs = {
        ROOT / "protocol/sspi.ir.json": ir,
        ROOT / "protocol/contract.json": (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode("utf-8"),
    }
    from rust_projection import artifacts as rust_artifacts
    outputs.update(rust_artifacts(ir, ROOT, manifest["contract_sha256"]))
    from reference_vectors import vectors
    outputs.update(vectors(schema, ROOT, manifest["contract_sha256"]))
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="refuse drift; write nothing")
    args = parser.parse_args()
    if any(name == "taut" or name.startswith("taut.") for name in sys.modules):
        raise SystemExit("use a fresh interpreter for generation")
    package = released_taut()
    import taut
    verify_taut_modules(package)
    from taut.ir.load import load_schema
    from taut.ir.validate import validate_or_raise
    schema = load_schema(ROOT / "protocol/sspi.taut.py")
    validate_or_raise(schema)
    outputs = artifacts(schema)
    verify_taut_modules(package)
    for path, expected in outputs.items():
        if args.check:
            if not path.exists() or path.read_bytes() != expected:
                raise SystemExit(f"stale generated artifact: {path.name}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(expected)
    print(f"{len(outputs)} schema artifacts {'verified' if args.check else 'written'}")


if __name__ == "__main__":
    main()
