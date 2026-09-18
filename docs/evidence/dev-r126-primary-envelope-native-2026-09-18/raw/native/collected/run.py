#!/usr/bin/env python3
"""Prepared remote runner. Requires later root approval and closed HIP handoff."""

import argparse
import os
from pathlib import Path
import re
import resource
import signal
import subprocess
import sys
import time

sys.dont_write_bytecode = True
import protocol as P  # noqa: E402

MANAGED = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
ACTIVE = None


def stamp():
    return {"realtime_ns": time.time_ns(), "monotonic_ns": time.monotonic_ns()}


def write(path, value):
    with path.open("x") as target:
        target.write(P.json.dumps(value, indent=2) + "\n")


def group_exists(pid):
    try:
        os.killpg(pid, 0)
        return True
    except ProcessLookupError:
        return False


def stop_owned(process):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if group_exists(process.pid):
            os.killpg(process.pid, sig)
        deadline = time.monotonic() + 5
        while group_exists(process.pid) and time.monotonic() < deadline:
            process.poll()
            time.sleep(0.02)
    process.wait(timeout=5)
    P.need(not group_exists(process.pid), "owned process group survived cleanup")


def interrupted(number, _frame):
    for managed in MANAGED:
        signal.signal(managed, signal.SIG_IGN)
    raise RuntimeError(f"interrupted signal={number}")


class Recorder:
    def __init__(self, root):
        self.root = root
        self.output = root / "results"
        self.output.mkdir()
        self.environment = {
            "HOME": "/home/harsh",
            "PATH": "/usr/bin:/bin:/opt/rocm/bin",
            "PYTHONDONTWRITEBYTECODE": "1",
            "HSA_XNACK": "0",
            "FE2O3_TEST_NATIVE_UNIQUE_ID": P.UID,
        }

    def run(self, name, command, bound, fresh_after=None):
        global ACTIVE
        folder = self.output / name
        folder.mkdir()
        stdout, stderr = folder / "stdout.log", folder / "stderr.log"
        row = {
            "name": name,
            "command": command,
            "cwd": str(self.root),
            "environment": self.environment,
            "started": stamp(),
            "spawned": None,
            "status": None,
            "error": None,
            "process_group": None,
            "group_absent": False,
            "t0": None,
            "outer_bound_seconds": bound,
        }
        process = None
        try:
            with stdout.open("xb") as out, stderr.open("xb") as err:
                blocked = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    if fresh_after is not None:
                        P.need(
                            0 <= time.monotonic_ns() - fresh_after <= 1_000_000_000,
                            "fresh preflight launch window",
                        )
                    process = subprocess.Popen(
                        command,
                        cwd=self.root,
                        env=self.environment,
                        stdout=out,
                        stderr=err,
                        start_new_session=True,
                        preexec_fn=lambda: signal.pthread_sigmask(
                            signal.SIG_SETMASK, blocked
                        ),
                    )
                    ACTIVE = process
                    row["process_group"] = process.pid
                    row["spawned"] = stamp()
                finally:
                    signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
                row["status"] = process.wait(timeout=bound)
                P.need(
                    not group_exists(process.pid),
                    "descendants remain after parent exit",
                )
                row["group_absent"] = True
                row["t0"] = stamp()
                if fresh_after is not None:
                    P.need(
                        0
                        <= row["spawned"]["monotonic_ns"] - fresh_after
                        <= 1_000_000_000,
                        "recorded fresh preflight launch window",
                    )
        except BaseException as error:
            row["error"] = f"{type(error).__name__}: {error}"
            blocked = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
            try:
                if process is not None:
                    try:
                        stop_owned(process)
                        row["group_absent"] = True
                        row["t0"] = stamp()
                    except BaseException as cleanup:
                        row["error"] += f"; cleanup={type(cleanup).__name__}: {cleanup}"
                    row["status"] = process.returncode
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, blocked)
        row["finished"] = stamp()
        row["stdout_sha256"], row["stderr_sha256"] = P.sha(stdout), P.sha(stderr)
        write(folder / "record.json", row)
        if row["group_absent"]:
            ACTIVE = None
        return row, stdout, stderr


def passed(row):
    return row["status"] == 0 and row["error"] is None and row["group_absent"] is True


def observe(recorder, name):
    command = [
        "/usr/bin/python3",
        "-B",
        str(recorder.root / "source/copy-host-observe.py"),
        "--gpu-index",
        str(P.GPU),
        "--pci-bdf",
        P.BDF,
        "--unique-id",
        P.UID,
        "--samples",
        "1",
    ]
    row, stdout, stderr = recorder.run(name, command, 75)
    lines = [P.parse(line) for line in stdout.read_text().splitlines()]
    P.need(len(lines) == 2, "complete single observer transcript")
    value, complete = lines
    P.need(
        complete
        == {
            "schema": "fe2o3.copy-host-observation.v1",
            "record": "complete",
            "observations": 1,
            "refused": int(not value["endpoint_admitted"]),
            "all_endpoints_admitted": value["endpoint_admitted"],
            "performance_accepted": False,
        },
        "exact observer completion",
    )
    return row, value, stderr.read_text()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--parent-ready-approved", action="store_true", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    P.need(args.parent_ready_approved, "root authorization required")
    P.need(
        re.fullmatch(
            r"/tmp/fe2o3-r126-primary-8b0021ba-20260918\.[A-Za-z0-9]{8}", str(root)
        ),
        "exact fresh remote path shape",
    )
    P.need(
        root.stat().st_uid == os.getuid() and not root.is_symlink(),
        "owned ordinary remote directory",
    )
    P.payload(root)
    identity = {
        "commit": P.COMMIT,
        "path": str(root),
        "payload_sha256": P.sha(root / "payload.json"),
    }
    P.need(P.load(root / "owner.json") == identity, "exact owned-directory marker")
    approval = P.load(root / "approval.json")
    P.need(
        approval
        == {
            **identity,
            "root_reviewed": True,
            "hip_handoff": "closed-after-observations-cleanup-and-absence",
            "post_observation_policy": "both-strict-t0-group-closed-immediate-0-1-delayed-20-21-v1",
        },
        "specific root approval and HIP handoff",
    )
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for number in MANAGED:
        signal.signal(number, interrupted)
    recorder = Recorder(root)
    state = {
        "commit": P.COMMIT,
        "started": stamp(),
        "cases": [],
        "failure": None,
        "native_ioctl_failure_claim": False,
        "performance_claim": False,
    }
    try:
        topology = recorder.run(
            "topology",
            [
                "/usr/bin/python3",
                "-B",
                str(root / "source/r26-host-guard.py"),
                "topology",
                "--gpu-index",
                str(P.GPU),
                "--pci-bdf",
                P.BDF,
                "--unique-id",
                P.UID,
            ],
            75,
        )
        P.need(passed(topology[0]), "fixed GPU topology admitted")
        P.need(
            topology[1].read_text() == P.TOPOLOGY
            and not topology[2].read_text().strip(),
            "exact GPU/KFD/NUMA/CPU topology",
        )
        placement = recorder.run(
            "placement",
            [
                "/usr/bin/numactl",
                "--physcpubind=48-95",
                "--membind=1",
                "/usr/bin/numactl",
                "--show",
            ],
            15,
        )
        P.need(passed(placement[0]), "fixed NUMA placement")
        P.need(not placement[2].read_text().strip(), "no placement diagnostics")
        P.placement(placement[1].read_text())
        for case, name in P.TESTS.items():
            P.payload(root)
            pre_row, before, pre_stderr = observe(recorder, f"{case}-preflight")
            P.need(
                passed(pre_row) and not pre_stderr.strip(),
                "fresh preflight command passed",
            )
            fresh_after = P.endpoint(before)
            command = [
                "/usr/bin/timeout",
                "--signal=TERM",
                "--kill-after=5s",
                "180s",
                "/usr/bin/prlimit",
                "--core=0:0",
                "--fsize=16777216:16777216",
                "--",
                "/usr/bin/numactl",
                "--physcpubind=48-95",
                "--membind=1",
                str(root / "runtime-tests"),
                "--exact",
                name,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
                "--color=never",
            ]
            test, stdout, stderr = recorder.run(
                f"{case}-test", command, 200, fresh_after
            )
            item = {
                "case": case,
                "test_record": f"{case}-test/record.json",
                "failures": [],
                "post_observations": {
                    "immediate": "not_attempted",
                    "delayed": "not_attempted",
                },
            }
            state["cases"].append(item)
            P.need(
                test["group_absent"] and test["t0"] is not None,
                "test process group must close before post-observation",
            )
            t0 = test["t0"]["monotonic_ns"]
            for label, offset in (("immediate", 0), ("delayed", 20)):
                if offset:
                    delay = (t0 + 20_000_000_000 - time.monotonic_ns()) / 1_000_000_000
                    if delay > 0:
                        time.sleep(delay)
                try:
                    item["post_observations"][label] = "attempted"
                    row, observation, error = observe(recorder, f"{case}-{label}")
                    P.need(
                        passed(row) and not error.strip(),
                        "post-observation command passed",
                    )
                    P.endpoint(observation, t0=t0, offset=offset)
                    item["post_observations"][label] = "strict_pass"
                except BaseException as error:
                    item["failures"].append(f"{label}: {type(error).__name__}: {error}")
            try:
                P.need(passed(test), "native test command passed")
                item["transcript"] = P.transcript(
                    case, stdout.read_text(), stderr.read_text()
                )
            except BaseException as error:
                item["failures"].append(f"test: {type(error).__name__}: {error}")
            P.need(not item["failures"], "case rejected; no later case may run")
        P.payload(root)
    except BaseException as error:
        state["failure"] = f"{type(error).__name__}: {error}"
    finally:
        if ACTIVE is not None:
            try:
                stop_owned(ACTIVE)
            except BaseException as error:
                state["failure"] = f"final owned cleanup failed: {error}"
        try:
            P.payload(root)
            state["payload_after"] = "matched"
        except BaseException as error:
            state["payload_after"] = f"rejected: {error}"
            state["failure"] = state["failure"] or "final payload identity rejected"
        state["finished"] = stamp()
        write(recorder.output / "campaign.json", state)
    print(P.json.dumps(state, indent=2), flush=True)
    return int(state["failure"] is not None)


if __name__ == "__main__":
    sys.exit(main())
