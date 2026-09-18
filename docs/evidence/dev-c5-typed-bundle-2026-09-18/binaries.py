#!/usr/bin/env python3
"""Hash the four actual --no-run test executables before and after execution."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
result = {}
for target in ("gnu", "musl"):
    for crate in ("host", "runtime"):
        name = target + "-" + crate
        assert (ARCHIVE / f"raw/{name}-build.exit").read_text() == "0\n"
        text = (ARCHIVE / f"raw/{name}-build.log").read_text()
        paths = re.findall(r"^  Executable .+ \((target/[^)]+)\)$", text, re.MULTILINE)
        assert len(paths) == 1, (name, paths)
        path = ROOT / paths[0]
        assert path.is_file() and not path.is_symlink()
        result[name] = {
            "path": paths[0],
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
print(json.dumps(result, indent=2))
