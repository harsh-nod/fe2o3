#!/usr/bin/env python3
"""Reviewed stdin-only control and exact-owned cleanup; never signal foreign work."""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

COMMIT = "602fda830307f9818cbff5d57d4be68a897e75b7"
PAYLOAD = "503683e379673cebf6711e99f82dcccf9a39f1c4c8d3b787c6f940a375c164c2"


def need(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def emit(value):
    print(json.dumps(value, sort_keys=True), flush=True)


def marker(owned):
    return {"commit": COMMIT, "path": str(owned), "payload_sha256": PAYLOAD}


def require_pids(value):
    need(
        type(value) is list
        and bool(value)
        and all(type(pid) is int and pid > 0 for pid in value)
        and len(set(value)) == len(value),
        "valid recorded PID roster",
    )
    return value


def cleanup_argument(text):
    value = json.loads(text)
    need(
        type(value) is dict and set(value) == {"pids", "inventory_sha256"},
        "cleanup requires PID/inventory object",
    )
    require_pids(value["pids"])
    need(
        type(value["inventory_sha256"]) is str
        and re.fullmatch(r"[0-9a-f]{64}", value["inventory_sha256"]),
        "cleanup inventory SHA-256",
    )
    return value


def absence_argument(text):
    return require_pids(json.loads(text))


def validate(owned):
    need(
        owned.is_dir() and not owned.is_symlink() and owned.resolve() == owned,
        "exact nonsymlink directory",
    )
    need(
        owned.stat().st_uid == os.getuid() and owned.stat().st_mode & 0o077 == 0,
        "private owned directory",
    )
    need(
        json.loads((owned / "owner.json").read_text()) == marker(owned),
        "exact directory marker",
    )


def roster(owned):
    paths = sorted((owned / "results").glob("*/record.json"))
    launch = owned / "results/controller-launch.json"
    need(launch.is_file(), "closed outer launch receipt required")
    rows = [json.loads(launch.read_text())] + [json.loads(p.read_text()) for p in paths]
    pids = []
    for row in rows:
        pid = row["process_group"]
        if pid is not None:
            need(type(pid) is int and pid > 0, "recorded process group type")
            pids.append(pid)
    need(pids and len(set(pids)) == len(pids), "unique nonempty owned process roster")
    return sorted(pids)


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def absence(owned, pids):
    rows = []
    for pid in pids:
        rows.append(
            {
                "pid": pid,
                "pid_absent": not os.path.lexists(f"/proc/{pid}"),
                "process_group_absent": not group_exists(pid),
            }
        )
    references, limitations = [], []
    for process in Path("/proc").iterdir():
        if not process.name.isdecimal() or int(process.name) == os.getpid():
            continue
        pid = int(process.name)
        try:
            uid = process.stat().st_uid
            if uid != os.getuid():
                continue
            entries = [process / "exe", process / "cwd"]
            try:
                entries += list((process / "fd").iterdir())
            except PermissionError:
                limitations.append({"pid": pid, "field": "fd-roster"})
            for entry in entries:
                try:
                    target = os.readlink(entry)
                except PermissionError:
                    limitations.append(
                        {"pid": pid, "field": str(entry.relative_to(process))}
                    )
                    continue
                except (FileNotFoundError, ProcessLookupError):
                    continue
                if target == str(owned) or target.startswith(str(owned) + "/"):
                    references.append(
                        {
                            "pid": pid,
                            "field": str(entry.relative_to(process)),
                            "target": target,
                        }
                    )
            try:
                maps = (process / "maps").read_text()
                for line in maps.splitlines():
                    parts = line.split(maxsplit=5)
                    if len(parts) == 6 and (
                        parts[5] == str(owned) or parts[5].startswith(str(owned) + "/")
                    ):
                        references.append(
                            {"pid": pid, "field": "maps", "target": parts[5]}
                        )
            except PermissionError:
                limitations.append({"pid": pid, "field": "maps"})
        except (FileNotFoundError, ProcessLookupError):
            continue
    for pid in sorted({row["pid"] for row in limitations}):
        need(pid not in pids, "unreadable recorded owned PID")
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
            group = int(stat[stat.rfind(")") + 2 :].split()[2])
            need(group not in pids, "unreadable member of recorded owned group")
            for row in limitations:
                if row["pid"] == pid:
                    row["process_group"] = group
        except (FileNotFoundError, ProcessLookupError):
            for row in limitations:
                if row["pid"] == pid:
                    row["departed_during_scan"] = True
    result = {
        "record": "owned-absence-observation",
        "utc_ns": time.time_ns(),
        "owned": str(owned),
        "recorded_processes": rows,
        "accessible_references": references,
        "unreadable_same_uid_entries": limitations,
        "scope": "recorded-owned-PIDs/groups and accessible same-UID exe/cwd/fd/maps only; no all-user or inaccessible-reference absence claim",
    }
    emit(result)
    need(
        all(row["pid_absent"] and row["process_group_absent"] for row in rows),
        "recorded owned PID/group remains",
    )
    need(not references, "visible reference to exact owned directory remains")
    return result


def inventory(owned):
    output = {}
    for path in sorted(owned.rglob("*")):
        need(not path.is_symlink(), "no owned-directory symlinks")
        need(path.is_dir() or path.is_file(), "ordinary owned-directory entries only")
        if path.is_file():
            need(path.stat().st_uid == os.getuid(), "owned file identity")
            output[path.relative_to(owned).as_posix()] = sha(path)
    return output


def launch(owned):
    need(not (owned / "results").exists(), "never rerun a started campaign")
    command = [
        "/usr/bin/timeout",
        "--signal=TERM",
        "--kill-after=15s",
        "3600s",
        "/usr/bin/python3",
        "-B",
        str(owned / "run.py"),
        "--parent-ready-approved",
    ]
    row = {
        "command": command,
        "process_group": None,
        "started_ns": time.time_ns(),
        "status": None,
        "group_absent": False,
        "error": None,
    }
    process = None
    try:
        process = subprocess.Popen(
            command, cwd="/tmp", stdin=subprocess.DEVNULL, start_new_session=True
        )
        row["process_group"] = process.pid
        emit(
            {
                "record": "outer-launch-started",
                "process_group": process.pid,
                "owned": str(owned),
                "started_ns": row["started_ns"],
            }
        )
        row["status"] = process.wait(timeout=3630)
        need(not group_exists(process.pid), "outer process group remains")
        row["group_absent"] = True
    except BaseException as error:
        row["error"] = f"{type(error).__name__}: {error}"
        if process is not None:
            for sig in (signal.SIGTERM, signal.SIGKILL):
                if group_exists(process.pid):
                    os.killpg(process.pid, sig)
                deadline = time.monotonic() + 5
                while group_exists(process.pid) and time.monotonic() < deadline:
                    process.poll()
                    time.sleep(0.02)
            process.wait(timeout=5)
            row["status"], row["group_absent"] = (
                process.returncode,
                not group_exists(process.pid),
            )
    finally:
        row["finished_ns"] = time.time_ns()
        (owned / "results").mkdir(exist_ok=True)
        with (owned / "results/controller-launch.json").open("x") as target:
            target.write(json.dumps(row, indent=2) + "\n")
        emit({"record": "outer-launch-finished", **row})
    return int(
        row["status"] != 0 or row["error"] is not None or not row["group_absent"]
    )


def main():
    need(len(sys.argv) in (3, 4), "exact mode/path/optional PID roster")
    mode, owned = sys.argv[1], Path(sys.argv[2])
    need(
        re.fullmatch(
            r"/tmp/fe2o3-striped-sdma-20260918\.[A-Za-z0-9]{8}", str(owned)
        ),
        "exact approved path shape",
    )
    os.chdir("/tmp")
    if mode == "absence":
        pids = absence_argument(sys.argv[3])
        need(not os.path.lexists(owned), "owned path remains")
        absence(owned, pids)
        emit(
            {"record": "independent-path-absence", "owned": str(owned), "absent": True}
        )
        return 0
    validate(owned)
    if mode == "empty":
        need(
            {p.name for p in owned.iterdir()} == {"owner.json"},
            "fresh marked directory only",
        )
        emit({"record": "fresh-marked-directory", "owned": str(owned)})
        return 0
    need(sha(owned / "payload.json") == PAYLOAD, "frozen payload manifest identity")
    manifest = json.loads((owned / "payload.json").read_text())
    for path, digest in manifest.items():
        need(sha(owned / path) == digest, "frozen payload byte identity")
    if mode == "approve":
        with (owned / "approval.json").open("x") as target:
            json.dump(
                {
                    **marker(owned),
                    "root_reviewed": True,
                    "shared_host_permission": "user-permits-currently-free-gpu",
                    "post_observation_policy": "both-strict-t0-group-closed-immediate-0-1-delayed-20-21-v1",
                },
                target,
                indent=2,
            )
            target.write("\n")
        emit(
            {
                "record": "exact-payload-approved",
                "owned": str(owned),
                "payload_sha256": PAYLOAD,
            }
        )
        return 0
    if mode == "launch":
        return launch(owned)
    pids = roster(owned)
    if mode == "inventory":
        emit(
            {
                "record": "complete-owned-inventory",
                "owned": str(owned),
                "files": inventory(owned),
                "recorded_pids": pids,
            }
        )
        absence(owned, pids)
        return 0
    need(mode == "cleanup", "known control mode")
    collected = cleanup_argument(sys.argv[3])
    need(collected["pids"] == pids, "cleanup roster matches original receipts")
    absence(owned, pids)
    before = inventory(owned)
    inventory_sha256 = hashlib.sha256(
        json.dumps(before, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    need(
        inventory_sha256 == collected["inventory_sha256"],
        "last remote inventory matches fully collected bytes",
    )
    shutil.rmtree(owned)
    need(not os.path.lexists(owned), "exact owned path removed")
    emit(
        {
            "record": "exact-owned-directory-removed",
            "owned": str(owned),
            "removed_regular_files": len(before),
            "inventory_sha256": inventory_sha256,
        }
    )
    absence(owned, pids)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except BaseException as error:
        if isinstance(error, SystemExit):
            raise
        emit(
            {
                "record": "control-refused",
                "error": f"{type(error).__name__}: {error}",
                "utc_ns": time.time_ns(),
            }
        )
        raise
