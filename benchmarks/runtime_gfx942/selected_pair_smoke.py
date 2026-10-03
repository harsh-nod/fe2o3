#!/usr/bin/env python3
"""Opt-in local two-GPU copy smoke; never an exclusive GPU reservation.

Build gfx942-runtime-peer-copy-smoke separately with --no-default-features.
Supply its reviewed SHA256 and two explicit index,PCI-BDF,unique-ID tuples.
This controller never builds, uploads, resets devices or deletes any files.
It exercises production deny-all kernel authorities, not compute authorization.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import signal
import stat
import subprocess
import sys
import time
from types import ModuleType

HERE = Path(__file__).resolve().parent
OBSERVER = HERE / "copy-host-observe.py"
OBSERVER_SHA = "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51"
EXAMPLE = "gfx942-runtime-peer-copy-smoke"
CASE_SECONDS = 180
OBSERVATION_SECONDS = 35
MAX_OUTPUT = 1 << 20
ENV = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}
CASES = (("native-forward", False, False), ("native-reverse", False, True),
         ("staged-forward", True, False), ("staged-reverse", True, True))
MANAGED = (signal.SIGTERM, signal.SIGINT, signal.SIGHUP)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def stamp():
    return {"unix_ns": time.time_ns(), "monotonic_ns": time.monotonic_ns()}


def save(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, sort_keys=True, indent=2, allow_nan=False)
        stream.write("\n")


def device(text):
    match = re.fullmatch(r"(0|[1-9][0-9]*),(0000:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]),(0x[0-9a-f]{16})", text)
    require(match is not None, "device must be canonical index,PCI-BDF,0x16-digit-UID")
    index, bdf, uid = int(match[1]), match[2], match[3]
    require(index < 64 and int(uid, 16) != 0, "bounded device index and nonzero UID required")
    return (index, bdf, uid)


def endpoints(values):
    require(len(values) == 2, "exactly two selected devices required")
    result = tuple(device(value) for value in values)
    require(all(len({row[column] for row in result}) == 2 for column in range(3)),
            "selected device indexes, PCI addresses and UIDs must be distinct")
    return result


def fingerprint(fd):
    before = os.fstat(fd)
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= 512 << 20,
            "bounded ordinary executable required")
    os.lseek(fd, 0, os.SEEK_SET)
    digest, size, header = hashlib.sha256(), 0, b""
    while chunk := os.read(fd, 1 << 20):
        if not header:
            header = chunk[:20]
        digest.update(chunk)
        size += len(chunk)
    after = os.fstat(fd)
    facts = lambda row: (row.st_dev, row.st_ino, row.st_size, row.st_mtime_ns, row.st_ctime_ns)
    require(facts(before) == facts(after) and size == before.st_size, "executable changed while hashing")
    require(header[:6] == b"\x7fELF\x02\x01" and header[18:20] == b"\x3e\x00",
            "x86-64 little-endian ELF required")
    require(before.st_mode & 0o111, "executable mode required")
    return {"sha256": digest.hexdigest(), "device": before.st_dev, "inode": before.st_ino,
            "bytes": size, "mtime_ns": before.st_mtime_ns, "ctime_ns": before.st_ctime_ns}


def load_observer():
    require(not OBSERVER.is_symlink() and OBSERVER.is_file(), "ordinary observation helper required")
    raw = OBSERVER.read_bytes()
    require(hashlib.sha256(raw).hexdigest() == OBSERVER_SHA, "reviewed observation helper changed")
    module = ModuleType("selected_pair_observer")
    module.__file__ = str(OBSERVER)
    exec(compile(raw, str(OBSERVER), "exec"), module.__dict__)
    return module


def group_members(group):
    members = set()
    for path in Path("/proc").iterdir():
        if not path.name.isdecimal():
            continue
        try:
            raw = (path / "stat").read_bytes()
        except (FileNotFoundError, ProcessLookupError):
            continue
        fields = raw[raw.rfind(b")") + 2:].split()
        require(len(fields) >= 3, "malformed process census")
        if int(fields[2]) == group:
            members.add(int(path.name))
    return members


def await_unreaped(process, seconds):
    deadline = time.monotonic() + seconds
    while True:
        status = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if status is not None:
            return status
        if time.monotonic() >= deadline:
            raise subprocess.TimeoutExpired(process.args, seconds)
        time.sleep(0.01)


def child_setup(mask):
    # This controller is single-threaded. Limits apply only to its child.
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_OUTPUT, MAX_OUTPUT))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    signal.pthread_sigmask(signal.SIG_SETMASK, mask)


class Recorder:
    def __init__(self, output):
        self.output = output
        self.commands = []

    def run(self, name, argv, seconds, *, executable=None, pass_fds=()):
        directory = self.output / name
        directory.mkdir()
        receipt = {"argv": list(argv), "executable": executable, "bound_seconds": seconds,
                   "environment": ENV, "started": stamp(), "exit": None, "error": None}
        process, observed_exit = None, None
        try:
            with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr:
                prior = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    process = subprocess.Popen(argv, executable=executable, pass_fds=pass_fds,
                                               stdout=stdout, stderr=stderr, env=ENV,
                                               start_new_session=True, stdin=subprocess.DEVNULL,
                                               preexec_fn=lambda: child_setup(prior))
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, prior)
                receipt["pid"] = process.pid
                observed_exit = await_unreaped(process, seconds)
        except BaseException as error:
            receipt["error"] = f"{type(error).__name__}: {error}"
            if not isinstance(error, Exception):
                raise
        finally:
            prior = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
            try:
                if process is not None:
                    # WNOWAIT retains the leader PID through census and signaling.
                    # No group is ever signaled after its leader has been reaped.
                    members = None
                    try:
                        members = group_members(process.pid)
                        receipt["members_before_reap"] = sorted(members)
                    except Exception as error:
                        # A failed census rejects the run, but must not skip
                        # closure of the still-unreaped, authenticated group.
                        receipt["cleanup_error"] = f"{type(error).__name__}: {error}"
                        receipt["error"] = receipt["error"] or receipt["cleanup_error"]
                    if observed_exit is None or members != {process.pid}:
                        receipt["error"] = receipt["error"] or "command left process-group descendants"
                        require(os.getpgid(process.pid) == process.pid, "owned child group changed")
                        os.killpg(process.pid, signal.SIGKILL)
                    receipt["exit"] = process.wait(timeout=10)
                    deadline = time.monotonic() + 2
                    while group_members(process.pid) and time.monotonic() < deadline:
                        time.sleep(0.02)
                    receipt["group_absent"] = not group_members(process.pid)
                    require(receipt["group_absent"], "owned group did not close")
            except Exception as error:
                receipt["cleanup_error"] = "; ".join(filter(None, (
                    receipt.get("cleanup_error"), f"{type(error).__name__}: {error}")))
                receipt["error"] = receipt["error"] or receipt["cleanup_error"]
            try:
                receipt["finished"] = stamp()
                for name in ("stdout", "stderr"):
                    path = directory / name
                    data = path.read_bytes() if path.exists() and path.stat().st_size <= MAX_OUTPUT else None
                    receipt[name + "_sha256"] = hashlib.sha256(data).hexdigest() if data is not None else None
                    receipt[name + "_bytes"] = path.stat().st_size if path.exists() else None
                    if data is None:
                        receipt["error"] = receipt["error"] or "missing or oversized output"
                save(directory / "receipt.json", receipt)
                self.commands.append(str(directory.relative_to(self.output)))
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, prior)
        return receipt


def command_text(recorder, name, receipt):
    require(receipt["exit"] == 0 and receipt["error"] is None and receipt.get("group_absent") is True,
            f"command failed or retained process-group custody: {name}")
    folder = recorder.output / name
    stdout = (folder / "stdout").read_text(encoding="ascii")
    stderr = (folder / "stderr").read_text(encoding="ascii")
    require(not stderr.strip(), f"unexpected stderr: {name}")
    return stdout


def observe_pair(recorder, helper, pair, smi, label):
    rows = []
    for index, bdf, uid in pair:
        ordinal = 0

        def run(argv):
            nonlocal ordinal
            name = f"{label}-gpu{index}-{ordinal}"
            ordinal += 1
            record = recorder.run(name, argv, OBSERVATION_SECONDS)
            folder = recorder.output / name
            return {"exit": record["exit"], "error": record["error"],
                    "stdout": (folder / "stdout").read_text(encoding="ascii") if record["stdout_sha256"] else "",
                    "stderr": (folder / "stderr").read_text(encoding="ascii") if record["stderr_sha256"] else ""}

        row = helper.observe(index, bdf, uid, smi, run=run)
        save(recorder.output / f"{label}-gpu{index}.json", row)
        rows.append(row)
    require(all(row["schema"] == helper.SCHEMA and row["endpoint_admitted"] is True
                and row["selected_pids"] == [] and row["reasons"] == [] for row in rows),
            f"selected pair not conclusively idle/unattached: {label}")
    now = time.monotonic_ns()
    require(all(0 <= now - row["started"]["monotonic_ns"] <= 150 * 10**9 for row in rows),
            "endpoint admission is stale")
    return rows


def expected(pair, staged):
    return {"schema": "fe2o3.production-peer-copy-smoke.v1", "authority": "production-deny-all",
            "transport": "HOST-STAGED" if staged else "NATIVE-XGMI", "devices": "2", "launches": "0",
            "peer_copies": "2", "bytes_per_copy": "8388581", "native_packets": "0" if staged else "6",
            "observed_native_copies": "0" if staged else "2", "source_unique_id": pair[0][2],
            "destination_unique_id": pair[1][2], "allocations": "4", "streams": "2", "readbacks": "8",
            "rounds_changed": "true", "pre_flush_observers": "pending", "expired_drain": "rejected",
            "logical_counter": "final-only", "source_unchanged": "full-byte-pass",
            "destination_sentinel": "full-byte-pass", "output": "full-byte-pass",
            "cleanup": "logical-and-native-explicit", "performance_acceptance": "false", "formal_refinement": "false"}


def parse_pass(stdout, wanted):
    lines = stdout.splitlines()
    require(len(lines) == 1 and lines[0].startswith("PASS "), "exact one PASS line required")
    values = {}
    for field in lines[0][5:].split():
        require(field.count("=") == 1, "invalid PASS field")
        key, value = field.split("=")
        require(key and value and key not in values, "duplicate/empty PASS field")
        values[key] = value
    require(values == wanted, "PASS fields differ from exact selected case/cleanup contract")
    return values


def campaign(recorder, helper, pair, binary, fd, identity, smi):
    report = {"schema": "fe2o3.selected-pair-smoke.v1", "accepted": False,
              "binary": str(binary), "binary_identity": identity, "observer_sha256": OBSERVER_SHA,
              "devices": pair, "started": stamp(), "cases": [], "errors": [],
              "scope": "copy-only-selected-pair; point-observations-not-reservation; no-native-fault-injection"}
    try:
        for name, staged, reverse in CASES:
            selected = tuple(reversed(pair)) if reverse else pair
            row = {"name": name, "accepted": False, "cleanup_observed": False}
            report["cases"].append(row)
            require(fingerprint(fd) == identity, "pinned executable changed before case")
            observe_pair(recorder, helper, pair, smi, name + "-before")
            try:
                require(fingerprint(fd) == identity, "pinned executable changed during admission")
                argv = [str(binary), *(["--staged-default"] if staged else []), selected[0][2], selected[1][2]]
                receipt = recorder.run(name, argv, CASE_SECONDS,
                                       executable=f"/proc/self/fd/{fd}", pass_fds=(fd,))
                row["pass"] = parse_pass(command_text(recorder, name, receipt), expected(selected, staged))
                require(fingerprint(fd) == identity, "pinned executable changed during case")
                path_stat = binary.stat()
                require((path_stat.st_dev, path_stat.st_ino) == (identity["device"], identity["inode"]),
                        "executable pathname was replaced")
            finally:
                observe_pair(recorder, helper, pair, smi, name + "-after")
                row["cleanup_observed"] = True
            row["accepted"] = True
        report["accepted"] = True
    except BaseException as error:
        report["errors"].append(f"{type(error).__name__}: {error}")
        if not isinstance(error, Exception):
            raise
    finally:
        report["finished"] = stamp()
        report["commands"] = recorder.commands
        save(recorder.output / "result.json", report)
    return report


def interrupted(number, _frame):
    raise InterruptedError(f"controller interrupted by signal {number}")


def main(argv=None):
    require(sys.flags.isolated and sys.flags.dont_write_bytecode, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-hardware", action="store_true", required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--device", action="append", required=True)
    parser.add_argument("--output", type=Path, required=True, help="fresh result directory, never overwritten")
    parser.add_argument("--rocm-smi", type=Path, default=Path("/opt/rocm/bin/rocm-smi"))
    args = parser.parse_args(argv)
    pair = endpoints(args.device)
    require(re.fullmatch(r"[0-9a-f]{64}", args.binary_sha256), "canonical SHA256 required")
    binary = args.binary.absolute()
    require(binary.resolve(strict=True) == binary and binary.name == EXAMPLE,
            "canonical path to the exact production peer-copy example required")
    require(args.rocm_smi.is_absolute() and args.rocm_smi.is_file(), "absolute existing rocm-smi required")
    require(os.access("/dev/kfd", os.R_OK | os.W_OK), "read/write KFD access required")
    helper = load_observer()
    for number in MANAGED:
        signal.signal(number, interrupted)
    fd = os.open(binary, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        identity = fingerprint(fd)
        require(identity["sha256"] == args.binary_sha256, "binary SHA256 mismatch")
        args.output.mkdir(parents=False, exist_ok=False)
        result = campaign(Recorder(args.output), helper, pair, binary, fd, identity, args.rocm_smi)
    finally:
        os.close(fd)
    print(json.dumps({"accepted": result["accepted"], "results": str(args.output.absolute()),
                      "cases": len(result["cases"]), "errors": result["errors"]}, sort_keys=True))
    return 0 if result["accepted"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(f"selected-pair smoke refused: {error}", file=sys.stderr)
        sys.exit(1)
