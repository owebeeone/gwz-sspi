#!/usr/bin/env python3
"""Fail closed until the SSPI implementation's reviewed publication gate opens.

Gearu checks its release candidate before committing it, and `cargo package`
refuses uncommitted files, so packaging is the exact stage (`--exact`), run on the
release commit. Both stages refuse while the gate stands.
"""
import argparse
import subprocess
import sys
import tomllib
from pathlib import Path


def gates(exact):
    """Each gate's command, in order."""
    if exact:
        return [["cargo", "package", "--locked"]]
    return [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "clippy", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"],
        ["cargo", "test", "--all-features", "--locked"],
    ]


def check(root, version, run=subprocess.run, exact=False):
    with (root / "Cargo.toml").open("rb") as source:
        package = tomllib.load(source)["package"]
    if package["version"] != version:
        print("release refused: Cargo version does not match requested version", file=sys.stderr)
        return 1
    if package.get("publish") is False:
        print("release refused: scaffold publication is disabled pending implementation and Windows acceptance", file=sys.stderr)
        return 1
    for command in gates(exact):
        result = run(command, cwd=root, check=False)
        if result.returncode != 0:
            return result.returncode
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--exact", action="store_true", help="the release commit's stage: package it")
    args = parser.parse_args()
    return check(Path(__file__).resolve().parents[1], args.version, exact=args.exact)


if __name__ == "__main__":
    raise SystemExit(main())
