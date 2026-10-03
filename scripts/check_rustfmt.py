#!/usr/bin/env python3
"""Check every Rust source, including include! files and disabled platform branches."""
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
paths = sorted(str(path.relative_to(root)) for directory in ("src", "tests")
               for path in (root / directory).rglob("*.rs"))
result = subprocess.run(["rustfmt", "--edition", "2024", "--check", *paths],
                        cwd=root, check=False)
sys.exit(result.returncode)
