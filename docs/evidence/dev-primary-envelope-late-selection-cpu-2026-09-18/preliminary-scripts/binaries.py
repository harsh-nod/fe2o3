#!/usr/bin/env python3
"""Bind GNU and musl runtime harnesses to the recorded build outputs."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
result = {}
for target in ("gnu", "musl"):
    name = f"{target}-runtime"
    build = f"{name}-build"
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
