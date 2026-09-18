#!/usr/bin/env python3
"""Bind the two runtime test executables to their actual build records."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
result = {}
for target in ("gnu", "musl"):
    assert (ARCHIVE / f"raw/{target}-build.exit").read_text() == "0\n"
    text = (ARCHIVE / f"raw/{target}-build.log").read_text()
    paths = re.findall(r"^  Executable .+ \((target/[^)]+)\)$", text, re.MULTILINE)
    assert len(paths) == 1
    path = ROOT / paths[0]
    assert path.is_file() and not path.is_symlink()
    result[target] = {
        "path": paths[0],
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }
print(json.dumps(result, indent=2))
