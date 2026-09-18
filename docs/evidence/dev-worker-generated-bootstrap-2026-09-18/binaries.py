#!/usr/bin/env python3
"""Bind four library harnesses to their recorded build outputs."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
result = {}
for target in ("gnu", "musl"):
    for crate in ("runtime", "host"):
        name = f"{target}-{crate}"
        assert (ARCHIVE / f"raw/{name}-build.exit").read_text() == "0\n"
        output = (ARCHIVE / f"raw/{name}-build.log").read_text()
        paths = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$", output, re.MULTILINE
        )
        assert len(paths) == 1
        path = ROOT / paths[0]
        assert path.is_file() and not path.is_symlink()
        result[name] = {
            "path": paths[0],
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
print(json.dumps(result, indent=2))
