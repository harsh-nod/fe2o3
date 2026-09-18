#!/usr/bin/env python3
"""One owned, serial campaign. Invocation requires the parent's ready decision."""

import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import sys
import time

OWNED = Path("/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD")
COMMIT = "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f"
SOURCE = OWNED / "source"
RESULTS = OWNED / "results"
ORDERS = ("ABDC", "BCAD", "CDBA", "DACB")
VISIBILITY = (
    "HIP_VISIBLE_DEVICES",
    "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES",
    "GPU_DEVICE_ORDINAL",
)
OBSERVER = [
    "/usr/bin/python3",
    "-B",
    str(SOURCE / "benchmarks/runtime_gfx942/copy-host-observe.py"),
    "--gpu-index",
    "4",
    "--pci-bdf",
    "0000:85:00.0",
    "--unique-id",
    "0x54f88318ca05093d",
    "--samples",
    "1",
]
PREFIX = [
    "/usr/bin/timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "180s",
    "/usr/bin/prlimit",
    "--core=0:0",
    "--",
    "/usr/bin/numactl",
    "--physcpubind=48-95",
    "--membind=1",
]
MODES = {
    "A": "diagnostic-slice50us",
    "B": "diagnostic-native-sleep1ms",
    "C": "diagnostic-native-sleep25us",
}
MANAGED = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)


def stamp():
    now = time.time_ns()
    return {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(now // 10**9))
        + f".{now % 10**9:09d}Z",
        "monotonic_ns": time.monotonic_ns(),
    }


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


class Interrupted(RuntimeError):
    pass


def interrupted(number, _frame):
    # Once interrupted, finish bounded owned cleanup and endpoint collection.
    for managed in MANAGED:
        signal.signal(managed, signal.SIG_IGN)
    raise Interrupted(f"signal={number}")


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def stop_owned(process):
    if group_exists(process.pid):
        os.killpg(process.pid, signal.SIGTERM)
    deadline = time.monotonic() + 5
    while group_exists(process.pid) and time.monotonic() < deadline:
        process.poll()
        time.sleep(0.02)
    if group_exists(process.pid):
        os.killpg(process.pid, signal.SIGKILL)
    process.wait(timeout=5)
    deadline = time.monotonic() + 5
    while group_exists(process.pid) and time.monotonic() < deadline:
        time.sleep(0.02)
    if group_exists(process.pid):
        raise RuntimeError("owned child process group survived cleanup")


class Recorder:
    def __init__(self):
        self.root = RESULTS / "campaign"
        self.root.mkdir()
        self.names = []

    def run(self, tag, argv, *, environment=None, cwd=SOURCE, bound=200):
        name = f"{len(self.names):03d}-{tag}"
        self.names.append(name)
        output, errors = self.root / f"{name}.stdout", self.root / f"{name}.stderr"
        row = {
            "schema": "fe2o3.matched-command.v1",
            "name": name,
            "argv": argv,
            "cwd": str(cwd),
            "environment_override": environment or {},
            "visibility_unset": list(VISIBILITY),
            "started": stamp(),
            "pid": None,
            "exit": None,
            "error": None,
            "group_absent": False,
            "outer_bound_seconds": bound,
        }
        env = dict(os.environ)
        for key in VISIBILITY:
            env.pop(key, None)
        env.update(environment or {})
        process = None
        try:
            with output.open("xb") as stdout, errors.open("xb") as stderr:
                old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    process = subprocess.Popen(
                        argv,
                        cwd=cwd,
                        env=env,
                        stdout=stdout,
                        stderr=stderr,
                        start_new_session=True,
                        preexec_fn=lambda: signal.pthread_sigmask(
                            signal.SIG_SETMASK, old_mask
                        ),
                    )
                    row["pid"] = process.pid
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
                row["exit"] = process.wait(timeout=bound)
                if group_exists(process.pid):
                    raise RuntimeError("owned descendants remain after command exit")
                row["group_absent"] = True
        except BaseException as error:
            row["error"] = f"{type(error).__name__}: {error}"
            old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
            try:
                if process is not None:
                    stop_owned(process)
                    row["exit"] = process.returncode
                    row["group_absent"] = not group_exists(process.pid)
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        row["finished"] = stamp()
        row["stdout_sha256"], row["stderr_sha256"] = digest(output), digest(errors)
        (self.root / f"{name}.json").write_text(json.dumps(row, indent=2) + "\n")
        print(
            json.dumps({"record": name, "exit": row["exit"], "error": row["error"]}),
            flush=True,
        )
        return row


def passed(row):
    return row["exit"] == 0 and row["error"] is None and row["group_absent"]


def main():
    if sys.argv[1:] != ["--parent-ready-confirmed"]:
        raise SystemExit(
            "requires explicit parent ready confirmation before invocation"
        )
    assert OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED
    assert OWNED.stat().st_uid == os.getuid()
    assert (OWNED / "owner").read_text() == f"fe2o3-kfd-matched-{COMMIT}\n"
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for number in MANAGED:
        signal.signal(number, interrupted)
    recorder = Recorder()
    state = {
        "schema": "fe2o3.matched-campaign.v1",
        "source_commit": COMMIT,
        "orders": list(ORDERS),
        "started": stamp(),
        "cells": [],
        "failure": None,
        "scope": "guarded-shared-host-endpoints-not-reservation",
    }
    checks = [
        ("source", ["/usr/bin/sha256sum", "-c", str(OWNED / "source-files.sha256")]),
        ("binaries", ["/usr/bin/sha256sum", "-c", str(RESULTS / "binaries.sha256")]),
        ("platform", ["/usr/bin/sha256sum", "-c", str(RESULTS / "platform.sha256")]),
        ("scripts", ["/usr/bin/sha256sum", "-c", str(OWNED / "scripts.sha256")]),
    ]
    try:
        for tag, command in checks:
            if not passed(recorder.run(f"{tag}-before", command)):
                raise RuntimeError(f"{tag}-before failed")
        topology = recorder.run(
            "topology",
            [
                "/usr/bin/python3",
                "-B",
                str(SOURCE / "benchmarks/runtime_gfx942/r26-host-guard.py"),
                "topology",
                "--gpu-index",
                "4",
                "--pci-bdf",
                "0000:85:00.0",
                "--unique-id",
                "0x54f88318ca05093d",
            ],
        )
        if not passed(topology):
            raise RuntimeError("topology failed")
        placement = recorder.run(
            "placement",
            [
                "/usr/bin/numactl",
                "--physcpubind=48-95",
                "--membind=1",
                "/usr/bin/numactl",
                "--show",
            ],
        )
        if not passed(placement):
            raise RuntimeError("placement failed")
        topology_check = recorder.run(
            "topology-check",
            [
                "/usr/bin/python3",
                "-B",
                str(OWNED / "check.py"),
                "--topology",
                str(recorder.root / f"{topology['name']}.stdout"),
                str(recorder.root / f"{placement['name']}.stdout"),
            ],
        )
        if not passed(topology_check):
            raise RuntimeError("topology-check failed")
        for repetition, order in enumerate(ORDERS, 1):
            for cell in order:
                key = f"{repetition}-{cell}"
                item = {"key": key, "records": [], "admitted": False}
                state["cells"].append(item)
                pre = recorder.run(f"{key}-pre", OBSERVER)
                item["records"].append(pre["name"])
                if not passed(pre):
                    raise RuntimeError(f"{key}: preflight refused; native not launched")
                item["admitted"] = True
                if cell in MODES:
                    command = PREFIX + [
                        str(
                            OWNED
                            / "target/release/examples/gfx942-runtime-directional-window-benchmark"
                        ),
                        "0x54f88318ca05093d",
                        "268435456",
                        "3",
                        "10",
                        MODES[cell],
                    ]
                    environment = {}
                else:
                    command = PREFIX + [
                        str(OWNED / "hsa-copy-pool-engine"),
                        "0",
                        "1",
                        "268435456",
                        "3",
                        "10",
                        "0x54f88318ca05093d",
                        "fine",
                        "engine1",
                    ]
                    environment = {"HSA_XNACK": "0", "ROCR_VISIBLE_DEVICES": "4"}
                native = recorder.run(f"{key}-native", command, environment=environment)
                item["records"].append(native["name"])
                immediate = recorder.run(f"{key}-immediate", OBSERVER)
                item["records"].append(immediate["name"])
                delay = recorder.run(f"{key}-delay", ["/usr/bin/sleep", "20"])
                item["records"].append(delay["name"])
                delayed = recorder.run(f"{key}-delayed", OBSERVER)
                item["records"].append(delayed["name"])
                payload = recorder.run(
                    f"{key}-payload",
                    [
                        "/usr/bin/python3",
                        "-B",
                        str(OWNED / "check.py"),
                        "--payload",
                        cell,
                        str(recorder.root / f"{native['name']}.stdout"),
                        str(recorder.root / f"{native['name']}.stderr"),
                    ],
                )
                item["records"].append(payload["name"])
                failed = [
                    row["name"]
                    for row in (native, immediate, delay, delayed, payload)
                    if not passed(row)
                ]
                if failed:
                    raise RuntimeError("sticky cell failure: " + ", ".join(failed))
    except BaseException as error:
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        for tag, command in checks:
            result = recorder.run(f"{tag}-after", command)
            if not passed(result) and state["failure"] is None:
                state["failure"] = f"{tag}-after failed"
        state["finished"] = stamp()
        state["records"] = recorder.names
        state["exit"] = int(state["failure"] is not None)
        (RESULTS / "campaign.json").write_text(json.dumps(state, indent=2) + "\n")
        print(json.dumps(state), flush=True)
    return state["exit"]


if __name__ == "__main__":
    raise SystemExit(main())
