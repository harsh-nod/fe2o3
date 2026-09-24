#!/usr/bin/env python3
"""Finalize the copied campaign after the exclusive-write receipt failure."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import os
from pathlib import Path
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
VERIFY = HERE / "verify.py"
if not stat.S_ISREG(VERIFY.lstat().st_mode):
    raise RuntimeError("ordinary verifier required")
RAW = VERIFY.read_bytes()
if hashlib.sha256(RAW).hexdigest() != "8aacaff8fb1e8880dfa74067e8bf9d5f010add1c74039db09ae34e57d3951f4f":
    raise RuntimeError("verifier identity mismatch")
V = ModuleType("producer_launch_recovery_verify")
V.__file__ = str(VERIFY)
exec(compile(RAW, str(VERIFY), "exec"), V.__dict__)
R = V.R


def main():
    raw = HERE / "raw"
    source = R.PRIVATE / "campaign1"
    V.iteration_inventory(raw)
    V.failed_collection_inventory(raw)
    V.need({path.name for path in raw.iterdir()} == {
        "iterations", "campaign1", "campaign1.log", "cleanup.json", "collection-failure",
    }, "exact post-failure evidence roster")
    for path in (R.PRIVATE, source, raw, *raw.rglob("*")):
        V.need(not path.is_symlink() and path.resolve() == path and path.stat().st_uid == os.getuid()
               and (path.is_file() or path.is_dir()) and not path.is_mount(), "owned ordinary recovery input")
    V.need(R.sha(raw / "cleanup.json") == V.INITIAL_CLEANUP_SHA, "original provisional cleanup receipt")
    initial = V.read(raw / "cleanup.json")
    retained = raw / "campaign1"
    V.validate_campaign(source, retained=False)
    V.validate_campaign(retained, retained=True)
    V.need(V.inventory(retained) == V.inventory(source) | {"runner.py": V.RUNNER_SHA}, "unchanged byte-exact collection")
    V.need(R.sha(raw / "campaign1.log") == R.sha(R.PRIVATE / "campaign1.log"), "unchanged campaign log")
    c = R.helpers()
    for name, _, _ in R.stages():
        row = V.read(retained / "commands" / name / "receipt.json")
        V.need(not c.B.group_exists(row["pid"]), "completed command group remains")
    target = R.PRIVATE / "target"
    V.need(not target.exists() and not target.is_symlink(), "owned cache already absent; no deletion allowed")
    final = initial | {"absent": True}
    recovery = {
        "cache_path": str(target), "initial_receipt_sha256": V.INITIAL_CLEANUP_SHA,
        "original_collector_sha256": V.FAILED_COLLECTION["collect.py"],
        "original_collection": "failed-final-receipt-exclusive-create",
        "observation": "owned-cache-absent", "deletion_repeated": False,
        "command_groups_absent": True,
    }
    V.cleanup_recovery(initial, final, recovery)
    c.B.write_json(raw / "cleanup-result.json", final)
    c.B.write_json(raw / "cleanup-recovery.json", recovery)
    c.B.write_json(HERE / "artifacts.json", V.inventory(raw))
    V.main()


if __name__ == "__main__":
    main()
