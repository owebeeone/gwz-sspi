#!/usr/bin/env python3
"""Fail closed until the SSPI implementation's reviewed publication gate opens."""
import argparse
import subprocess
import sys
import tomllib
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    with (root / "Cargo.toml").open("rb") as source:
        package = tomllib.load(source)["package"]
    if package["version"] != args.version:
        print("release refused: Cargo version does not match requested version", file=sys.stderr)
        return 1
    if package.get("publish") is False:
        print("release refused: scaffold publication is disabled pending implementation and Windows acceptance", file=sys.stderr)
        return 1
    commands = [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "clippy", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"],
        ["cargo", "test", "--all-features", "--locked"],
        ["cargo", "package", "--locked"],
    ]
    for command in commands:
        result = subprocess.run(command, cwd=root, check=False)
        if result.returncode != 0:
            return result.returncode
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
