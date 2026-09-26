#!/usr/bin/env python3
"""Seal the completed packet without rewriting retained evidence."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
cleanup = json.loads((root / "cleanup-after.json").read_text())
if cleanup["retained_hashes_match"] is not True or not cleanup["absent_paths"]:
    raise ValueError("completed cleanup required")
lines = []
for path in sorted(root.rglob("*")):
    if path.is_symlink():
        raise ValueError("ordinary packet entries required")
    if path.is_file():
        if path.name == "SHA256SUMS":
            raise ValueError("packet already sealed")
        lines.append(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + str(path.relative_to(root)) + "\n")
with (root / "SHA256SUMS").open("x") as output:
    output.writelines(lines)
print("Sealed " + str(len(lines)) + " files")
