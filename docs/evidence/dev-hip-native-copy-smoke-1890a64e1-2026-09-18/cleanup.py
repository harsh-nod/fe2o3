#!/usr/bin/env python3
"""Exact owned cleanup with explicit limits on unrelated /proc visibility."""

import json
import os
from pathlib import Path
import shutil
import sys
import time

OWNED = Path("/tmp/fe2o3-hip-smoke-1890a64e1-20260918.JA7uyMoS")
MARKER = "fe2o3-hip-smoke-1890a64e1911a2a346c5a9e70a8ff5ad45b19231\n"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def owned_absence(pids):
    observations = []
    for pid in pids:
        try:
            os.killpg(pid, 0)
            group_absent = False
        except ProcessLookupError:
            group_absent = True
        path_absent = not os.path.lexists(f"/proc/{pid}")
        observations.append(
            {
                "pid": pid,
                "pid_absent": path_absent,
                "process_group_absent": group_absent,
            }
        )
    require(
        all(row["pid_absent"] and row["process_group_absent"] for row in observations),
        "a recorded owned PID/group remains",
    )
    return observations


def scan(owned_pids):
    references, unreadable = [], []
    for process in Path("/proc").iterdir():
        if not process.name.isdecimal() or int(process.name) == os.getpid():
            continue
        pid = int(process.name)
        try:
            if process.stat().st_uid != os.getuid():
                continue
            entries = [process / "exe", process / "cwd"]
            try:
                entries.extend((process / "fd").iterdir())
            except PermissionError:
                unreadable.append({"pid": pid, "field": "fd-roster"})
            for entry in entries:
                try:
                    target = os.readlink(entry)
                except PermissionError:
                    unreadable.append({"pid": pid, "field": entry.name})
                    continue
                except FileNotFoundError:
                    continue
                if target == str(OWNED) or target.startswith(str(OWNED) + "/"):
                    references.append(
                        {"pid": pid, "field": entry.name, "target": target}
                    )
        except (FileNotFoundError, ProcessLookupError):
            continue
    for pid in sorted({row["pid"] for row in unreadable}):
        require(pid not in owned_pids, "unreadable recorded owned PID")
        try:
            raw = Path(f"/proc/{pid}/stat").read_text()
            group = int(raw[raw.rfind(")") + 2 :].split()[2])
            require(
                group not in owned_pids,
                "unreadable member of recorded owned process group",
            )
            for row in unreadable:
                if row["pid"] == pid:
                    row["process_group"] = group
        except FileNotFoundError:
            for row in unreadable:
                if row["pid"] == pid:
                    row["departed_during_scan"] = True
    require(not references, "an accessible process references the owned directory")
    return references, unreadable


def report(kind, pids, **extra):
    owners = owned_absence(pids)
    references, unreadable = scan(pids)
    row = {
        "record": kind,
        "utc_ns": time.time_ns(),
        "owned": str(OWNED),
        "owned_processes": owners,
        "accessible_references": references,
        "unreadable_unrelated_entries": unreadable,
        "scope": "recorded-owned-PIDs-and-groups-absent; accessible-reference-scan-only; not-global-reference-absence",
        **extra,
    }
    print(json.dumps(row, sort_keys=True), flush=True)


def main():
    require(
        len(sys.argv) == 3 and sys.argv[1] in ("--cleanup", "--absence"),
        "exact mode and PID roster required",
    )
    pids = [int(value) for value in sys.argv[2].split(",")]
    require(
        1 <= len(pids) <= 13
        and len(set(pids)) == len(pids)
        and all(pid > 0 for pid in pids),
        "unique complete recorded owned PID roster required",
    )
    os.chdir("/tmp")
    if sys.argv[1] == "--absence":
        require(not os.path.lexists(OWNED), "owned path remains")
        report("independent-absence", pids, absent=True)
        return
    require(
        OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED,
        "exact nonsymlink owned path required",
    )
    require(
        OWNED.stat().st_uid == os.getuid() and (OWNED / "owner").read_text() == MARKER,
        "ownership marker mismatch",
    )
    rows = [
        json.loads(path.read_text())
        for path in sorted((OWNED / "results/smoke").glob("*.json"))
    ]
    require(
        [row["pid"] for row in rows] == pids
        and all(row["group_absent"] for row in rows),
        "PID roster must match original recorded campaign",
    )
    report("before-cleanup-visible", pids)
    size = sum(
        path.stat().st_size
        for path in OWNED.rglob("*")
        if path.is_file() and not path.is_symlink()
    )
    shutil.rmtree(OWNED)
    require(not os.path.lexists(OWNED), "owned path still exists")
    report("after-cleanup-visible", pids, absent=True, removed_regular_bytes=size)


if __name__ == "__main__":
    main()
