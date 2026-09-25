#!/usr/bin/env python3
"""Retain this completed development run, then remove its remaining owned trees."""

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

HERE = Path(__file__).resolve().parent
SCRATCH = Path("/home/harsh/.codex-tmp/fe2o3-reconciliation-proof-requal-20260925-pWhjaneC")
TARGET = Path("/dev/shm/fe2o3-reconciliation-proof-requal-20260925-9SGsXJDH")
SOURCE = "7935918fcb08b931a70a41015a039f0b7c1e08c2"
PHASES = {"baseline-signature", "source-signature", "source-tests", "source-body", "closure-before", "proof-before",
          "gnu", "musl", "doctests", "default", "clippy", "format", "proof-after", "closure-after"}
SCOPE = "Signed-source planner development; not final logical-mutation, outcome-witness, relocated-replay or native qualification"


def need(value, message):
    if not value:
        raise ValueError(message)


def write(path, value):
    with path.open("x") as stream:
        stream.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        info = path.lstat()
        need(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), "ordinary retention tree")
        if stat.S_ISREG(info.st_mode):
            result[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def snapshot(root):
    need(root.resolve() == root and root.is_dir() and not root.is_symlink(), "canonical owned root")
    result = {}
    device = root.lstat().st_dev
    for path in (root, *sorted(root.rglob("*"))):
        info = path.lstat()
        need(info.st_uid == os.getuid() and info.st_dev == device, "owned same-device entry")
        need(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode) or stat.S_ISLNK(info.st_mode), "ordinary owned entry")
        result[str(path.relative_to(root))] = [info.st_dev, info.st_ino, info.st_uid, info.st_mode,
                                              info.st_size, info.st_mtime_ns, info.st_blocks * 512]
    return result


def groups_absent(groups):
    for group in groups:
        need(type(group) is int and group > 1, "recorded process group")
        try:
            os.killpg(group, 0)
        except ProcessLookupError:
            continue
        raise ValueError("recorded group still exists: " + str(group))


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    qualification = SCRATCH / "qualification"
    before = json.loads((qualification / "source-before.json").read_text())
    after = json.loads((qualification / "source-after.json").read_text())
    need(before == after and before["commit"] == SOURCE, "complete signed source bracket")
    results = json.loads((qualification / "results.json").read_text())
    need(set(results) == PHASES and all(row["passed"] is True and row["status"] == 0 for row in results.values()), "all final phases passed")
    groups = []
    for phase in sorted(PHASES):
        row = json.loads((qualification / phase / "record.json").read_text())
        need(row["group_absent"] is True and row["status"] == 0 and row["finished_ns"] >= row["started_ns"], "terminal phase record")
        groups.append(row["process_group"])
    groups_absent(groups)
    need(not (HERE / "retained").exists(), "new retained destination")
    owned = [SCRATCH]
    previously_absent = []
    if os.path.lexists(TARGET):
        owned.append(TARGET)
    else:
        need(TARGET.name not in os.listdir(TARGET.parent), "independent preexisting target absence")
        previously_absent.append({"path": str(TARGET), "cause": "Unknown", "cleanup_claim": False,
                                  "allocated_bytes_removed_claim": 0})
    roots = {str(root): snapshot(root) for root in owned}
    original = inventory(SCRATCH)
    shutil.copytree(SCRATCH, HERE / "retained")
    need(inventory(HERE / "retained") == original and inventory(SCRATCH) == original, "byte-exact complete retention")
    write(HERE / "retention.json", {"scope": SCOPE, "source": str(SCRATCH), "files": original})
    allocated = {}
    for root, rows in roots.items():
        seen = set()
        total = 0
        for row in rows.values():
            identity = tuple(row[:2])
            if identity not in seen:
                total += row[-1]
                seen.add(identity)
        allocated[root] = total
    write(HERE / "cleanup-before.json", {"scope": SCOPE, "source_commit": SOURCE, "source_inputs": len(before["inputs"]),
          "retained_files": len(original), "owned_roots": roots, "allocated_bytes": allocated,
          "previously_absent": previously_absent, "pid_namespace": os.readlink("/proc/self/ns/pid"),
          "terminal_groups": groups, "observed_at": datetime.now(timezone.utc).isoformat()})
    for root in owned:
        need(snapshot(root) == roots[str(root)], "owned snapshot changed")
    groups_absent(groups)
    need(inventory(HERE / "retained") == original, "retained hashes before deletion")
    for root in owned:
        shutil.rmtree(root)
    absence = []
    for root in (SCRATCH, TARGET):
        check = subprocess.run(["/usr/bin/test", "!", "-e", str(root)], capture_output=True, check=True)
        need(not check.stdout and not check.stderr and not os.path.lexists(root)
             and root.name not in os.listdir(root.parent), "independent path absence")
        absence.append(str(root))
    groups_absent(groups)
    need(inventory(HERE / "retained") == original, "retained hashes after deletion")
    write(HERE / "cleanup-after.json", {"scope": SCOPE, "absent_paths": absence, "terminal_groups_absent": groups,
          "removed_paths": [str(root) for root in owned], "previously_absent": previously_absent,
          "retained_files": len(original), "retained_hashes_match": True,
          "path_accounted_allocated_bytes_removed": sum(allocated.values()), "remote_resources_created": False,
          "observed_at": datetime.now(timezone.utc).isoformat()})
    print(json.dumps({"retained_files": len(original), "source_inputs": len(before["inputs"]),
                      "removed_allocated_bytes": sum(allocated.values()), "absent": True}, sort_keys=True))


if __name__ == "__main__":
    main()
