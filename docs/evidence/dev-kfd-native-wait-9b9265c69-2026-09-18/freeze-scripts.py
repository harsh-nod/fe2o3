#!/usr/bin/env python3
"""Freeze only reviewed owned staging scripts, with their remote absolute paths."""

import hashlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
OWNED = "/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD"
NAMES = ("run.py", "check.py", "cleanup.py", "prepare.sh")
(HERE / "scripts.sha256").write_text(
    "".join(
        f"{hashlib.sha256((HERE / name).read_bytes()).hexdigest()}  {OWNED}/{name}\n"
        for name in NAMES
    )
)
print((HERE / "scripts.sha256").read_text(), end="")
