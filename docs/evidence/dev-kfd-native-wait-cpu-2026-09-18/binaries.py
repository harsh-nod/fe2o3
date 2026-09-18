#!/usr/bin/env python3
"""Hash the actual executable paths named by completed qualification receipts."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
result = {}
for name in ("gnu-kfd-sdma-final", "gnu-example-default", "gnu-example-qualified", "gnu-runtime-fixed",
             "musl-example", "musl-runtime", "model-r39", "unsafe-source"):
    assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n"
    text = (ARCHIVE / f"raw/{name}.log").read_text()
    paths = re.findall(r"^     Running .+ \((target/[^)]+)\)$", text, re.MULTILINE)
    assert len(paths) == 1, name
    path = ROOT / paths[0]
    assert path.is_file() and not path.is_symlink()
    result[name] = {"path": paths[0], "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
print(json.dumps(result, indent=2))
