#!/usr/bin/env python3
"""Remove only this task's two build trees after retained failure replay."""
import importlib.util
import json
from pathlib import Path
import shutil
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("observer_cleanup_replay", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)
C = V.R.load(HERE.parent / "dev-mixed-duration-2026-09-25/collect.py",
             "fa2d32564ba31db930d1fef48b24ea8e24529f580fea100a4e1a424e889a0f82", "observer_cleanup_inventory")
TARGETS = (
    Path("/home/harsh/.codex-tmp/fe2o3-observer-target-20260925-AExFS7XF"),
    Path("/home/harsh/.codex-tmp/fe2o3-observer-cold-20260925-vwjyYj7e"),
)


def main():
    C.need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    C.need(shutil.rmtree.avoids_symlink_attacks, "descriptor-relative cleanup")
    replay = V.verify()
    C.need(Path(V.V.read(HERE / "raw/archive.json")["source"]).parent == TARGETS[1], "exact cold target")
    records = [V.V.read(path) for directory in ("raw", "continuation", "validation")
               for path in (HERE / directory).glob("*/record.json")]
    C.need(len(records) == replay["commands"] and all(row["group_absent"] is True and type(row["status"]) is int for row in records), "all controlled commands terminated")
    # Process IDs cannot be rechecked in a different sandbox PID namespace.
    # The controller receipts record group absence in the execution namespace.
    before = {str(root): C.inventory(root) for root in TARGETS}
    C.save(HERE / "cleanup-before.json", before)
    C.need({str(root): C.inventory(root) for root in TARGETS} == before, "owned tree continuity")
    for root in TARGETS:
        shutil.rmtree(root)
    C.need(all(not root.exists() and not root.is_symlink() for root in TARGETS), "independent owned path absence")
    C.need(V.verify() == replay, "retained replay survives cleanup")
    unique = {(row["device"], row["inode"]): row["allocated"] for tree in before.values() for row in tree.values()}
    result = dict(removed=[str(root) for root in TARGETS], paths_absent=True,
                  inode_accounted_allocated_bytes=sum(unique.values()),
                  process_absence_evidence="retained controlled commands: terminal receipts in original PID namespace",
                  preliminary_development_evidence="earlier build/test tool handles no longer available; their terminal output was not retained",
                  remote_resources_created=False, verification=replay)
    C.save(HERE / "cleanup-after.json", result)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
