#!/usr/bin/env python3
"""Freeze only reviewed local wrapper/checker scripts, separately from Git source."""

import hashlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
OWNED = "/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.EvupxcTh"
NAMES = [
    "check.py",
    "cleanup.py",
    "dependency-snapshot.py",
    "hip_copy_diagnostic.py",
    "prepare.sh",
    "protocol.py",
    "run.py",
    "topology.py",
]
manifest = "".join(
    f"{hashlib.sha256((HERE / name).read_bytes()).hexdigest()}  {OWNED}/{name}\n"
    for name in NAMES
)
(HERE / "scripts.sha256").write_text(manifest)
print(manifest, end="")
