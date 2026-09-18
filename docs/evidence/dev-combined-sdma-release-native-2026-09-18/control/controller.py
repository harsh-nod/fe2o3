#!/usr/bin/env python3
"""Bounded local controller for one exact root-approved combined-SDMA directory."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import time

HERE = Path(__file__).resolve().parent
BUNDLE = Path(
    "/home/harsh/.codex-tmp/fe2o3-combined-sdma-native-bundle-v2-20260918"
)
OWNED = "/home/harsh/fe2o3-combined-sdma-20260918.d3b7bc43"
PAYLOAD = "1aed06aa903f7131a8d7d64249ebf516e6dd6f042f4c42f5002cb3a7e47a30e0"
SSH_OPTIONS = [
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=10",
    "-o",
    "ServerAliveCountMax=3",
]
MANAGED = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)


def need(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def dump(path, value):
    with path.open("x") as target:
        target.write(json.dumps(value, indent=2) + "\n")


def exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def stop(process):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if exists(process.pid):
            os.killpg(process.pid, sig)
        deadline = time.monotonic() + 5
        while exists(process.pid) and time.monotonic() < deadline:
            process.poll()
            time.sleep(0.02)
    process.wait(timeout=5)
    need(not exists(process.pid), "local owned process group remains")


def interrupted(number, _frame):
    for managed in MANAGED:
        signal.signal(managed, signal.SIG_IGN)
    raise RuntimeError(f"controller interrupted by signal={number}")


class Recorder:
    def __init__(self, output):
        self.output = output
        self.output.mkdir()
        self.rows = []

    def run(self, name, command, seconds, stdin=None):
        folder = self.output / name
        folder.mkdir()
        row = {
            "command": command,
            "cwd": str(BUNDLE),
            "started_ns": time.time_ns(),
            "bound_seconds": seconds,
            "process_group": None,
            "status": None,
            "error": None,
            "group_absent": False,
            "stdin_sha256": hashlib.sha256(stdin).hexdigest()
            if stdin is not None
            else None,
        }
        process = None
        try:
            with (
                (folder / "stdout.log").open("xb") as stdout,
                (folder / "stderr.log").open("xb") as stderr,
            ):
                blocked = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    process = subprocess.Popen(
                        command,
                        cwd=BUNDLE,
                        stdin=subprocess.PIPE
                        if stdin is not None
                        else subprocess.DEVNULL,
                        stdout=stdout,
                        stderr=stderr,
                        start_new_session=True,
                        preexec_fn=lambda: signal.pthread_sigmask(
                            signal.SIG_SETMASK, blocked
                        ),
                    )
                    row["process_group"] = process.pid
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
                process.communicate(input=stdin, timeout=seconds)
                row["status"] = process.returncode
                need(not exists(process.pid), "local owned descendants remain")
                row["group_absent"] = True
        except BaseException as error:
            row["error"] = f"{type(error).__name__}: {error}"
            if process is not None:
                try:
                    stop(process)
                    row["group_absent"] = True
                except BaseException as cleanup:
                    row["error"] += f"; cleanup={cleanup}"
                row["status"] = process.returncode
        row["finished_ns"] = time.time_ns()
        row["stdout_sha256"], row["stderr_sha256"] = (
            sha(folder / "stdout.log"),
            sha(folder / "stderr.log"),
        )
        dump(folder / "record.json", row)
        self.rows.append(
            {
                "name": name,
                "status": row["status"],
                "error": row["error"],
                "group_absent": row["group_absent"],
            }
        )
        print(json.dumps(self.rows[-1]), flush=True)
        return row, folder


def passed(row):
    return row["status"] == 0 and row["error"] is None and row["group_absent"] is True


def remote(recorder, tag, mode, bound, pids=None):
    arguments = ["/usr/bin/python3", "-B", "-", mode, OWNED]
    if pids is not None:
        arguments.append(json.dumps(pids, separators=(",", ":")))
    return recorder.run(
        tag,
        ["ssh", "-T", *SSH_OPTIONS, "mi300x", shlex.join(arguments)],
        bound,
        (HERE / "remote_control.py").read_bytes(),
    )


def qualify(recorder):
    need(sha(BUNDLE / "payload.json") == PAYLOAD, "reviewed payload manifest")
    tests = recorder.run(
        "local-protocol-tests",
        ["/usr/bin/python3", "-B", str(BUNDLE / "test_protocol.py"), "-v"],
        30,
    )
    need(passed(tests[0]), "frozen protocol tests pass")
    code = "import json,pathlib,protocol; root=pathlib.Path.cwd(); manifest=protocol.payload(root); print(json.dumps({'payload_sha256':protocol.sha(root/'payload.json'),'files':len(manifest),'verified':True},sort_keys=True))"
    payload = recorder.run(
        "local-payload-verify", ["/usr/bin/python3", "-B", "-c", code], 30
    )
    need(passed(payload[0]), "frozen payload verification passes")
    wiring = recorder.run(
        "local-controller-wiring-tests",
        ["/usr/bin/python3", "-B", str(HERE / "test_controller.py"), "-v"],
        30,
    )
    need(passed(wiring[0]), "cleanup/absence controller wiring tests pass")


def cleanup_and_absence(recorder, state, pids, inventory_sha256):
    cleanup = remote(
        recorder,
        "remote-cleanup",
        "cleanup",
        120,
        {"pids": pids, "inventory_sha256": inventory_sha256},
    )
    need(passed(cleanup[0]), "exact owned cleanup completed")
    state["cleanup_closed"] = True
    absent = remote(recorder, "remote-independent-absence", "absence", 120, pids)
    need(passed(absent[0]), "independent path/PID/group absence completed")
    state["independent_absence_closed"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--qualify-only", action="store_true")
    parser.add_argument("--root-ready-approved", action="store_true")
    parser.add_argument("--remote-directory")
    args = parser.parse_args()
    need(
        args.qualify_only != args.root_ready_approved,
        "choose local-only qualification or explicit root-approved execution",
    )
    if not args.qualify_only:
        need(
            args.remote_directory == OWNED, "exact root-created marked remote directory"
        )
    for number in MANAGED:
        signal.signal(number, interrupted)
    recorder = Recorder(args.output.resolve())
    state = {
        "owned": OWNED,
        "payload_sha256": PAYLOAD,
        "native_attempted": False,
        "collected_verified": False,
        "cleanup_closed": False,
        "independent_absence_closed": False,
        "failure": None,
        "local_only": args.qualify_only,
    }
    try:
        qualify(recorder)
        if args.qualify_only:
            return 0
        need(
            passed(remote(recorder, "remote-empty", "empty", 45)[0]),
            "fresh marked remote directory",
        )
        upload = recorder.run(
            "upload",
            ["scp", "-r", *SSH_OPTIONS, str(BUNDLE) + "/.", "mi300x:" + OWNED + "/"],
            180,
        )
        need(passed(upload[0]), "complete payload upload")
        need(
            passed(remote(recorder, "remote-approve", "approve", 90)[0]),
            "exact remote payload approval",
        )
        state["native_attempted"] = True
        launched = remote(recorder, "native-outer", "launch", 3700)
        state["native_outer_passed"] = passed(launched[0])
        # SSH completion never substitutes for remote owned-process absence.
        checked, folder = remote(recorder, "remote-inventory", "inventory", 120)
        lines = [
            json.loads(line)
            for line in (folder / "stdout.log").read_text().splitlines()
        ]
        inventories = [
            row for row in lines if row.get("record") == "complete-owned-inventory"
        ]
        need(len(inventories) == 1, "complete inventory retained before collection")
        inventory = inventories[0]
        need(inventory["owned"] == OWNED, "inventory exact path")
        collection = recorder.run(
            "collect",
            [
                "scp",
                "-r",
                *SSH_OPTIONS,
                "mi300x:" + OWNED,
                str(recorder.output / "collected"),
            ],
            180,
        )
        need(passed(collection[0]), "complete exact directory collection")
        collected = recorder.output / "collected"
        files = {}
        for path in collected.rglob("*"):
            need(
                not path.is_symlink() and (path.is_file() or path.is_dir()),
                "ordinary collected entries",
            )
            if path.is_file():
                files[path.relative_to(collected).as_posix()] = sha(path)
        need(
            files == inventory["files"],
            "collected payload and every result match remote inventory",
        )
        need(files["payload.json"] == PAYLOAD, "collected frozen payload identity")
        state["collected_verified"] = True
        dump(
            recorder.output / "collection-verified.json",
            {
                "owned": OWNED,
                "files": files,
                "recorded_pids": inventory["recorded_pids"],
            },
        )
        need(
            passed(checked),
            "recorded remote PIDs/groups and visible path references absent before removal",
        )
        inventory_sha256 = hashlib.sha256(
            json.dumps(files, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        cleanup_and_absence(
            recorder, state, inventory["recorded_pids"], inventory_sha256
        )
        need(
            state["native_outer_passed"],
            "native campaign rejected; receipts and cleanup retained",
        )
    except BaseException as error:
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        state["records"] = recorder.rows
        dump(recorder.output / "controller.json", state)
        print(json.dumps(state, indent=2), flush=True)
    return int(state["failure"] is not None)


if __name__ == "__main__":
    sys.exit(main())
