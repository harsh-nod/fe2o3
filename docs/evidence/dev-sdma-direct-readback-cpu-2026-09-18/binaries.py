#!/usr/bin/env python3
"""Bind GNU and musl runtime harnesses to the recorded build outputs."""

import hashlib
import json
from pathlib import Path
import re
import sys

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
assert len(sys.argv) in (1, 2)
suffix = ""
if len(sys.argv) == 2:
    assert sys.argv[1] == "final"
    suffix = "-final"
result = {}
for target in ("gnu", "musl"):
    name = f"{target}-runtime"
    build = f"{name}-build{suffix}"
    assert (ARCHIVE / f"raw/{build}.exit").read_text() == "0\n"
    output = (ARCHIVE / f"raw/{build}.log").read_text()
    paths = re.findall(r"^  Executable .+ \((target/[^)]+)\)$", output, re.MULTILINE)
    assert len(paths) == 1
    path = ROOT / paths[0]
    assert path.is_file() and not path.is_symlink()
    result[name] = {
        "path": paths[0],
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }
print(json.dumps(result, indent=2))
