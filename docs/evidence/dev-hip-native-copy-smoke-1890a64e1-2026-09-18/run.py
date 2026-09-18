#!/usr/bin/env python3
"""One HIP process only. Requires an explicit parent READY before invocation."""

import json
import os
import resource
import signal
import subprocess
import sys
import time

import check
from protocol import (
    CHECKS,
    COMMIT,
    NATIVE,
    NATIVE_ENV,
    OBSERVER,
    OWNED,
    RESULTS,
    SOURCE,
    TOPOLOGY,
    VISIBILITY,
)

MANAGED = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)


def stamp():
    now = time.time_ns()
    return {
        "utc": time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(now // 10**9))
        + f".{now % 10**9:09d}Z",
        "monotonic_ns": time.monotonic_ns(),
    }


class Interrupted(RuntimeError):
    pass


def interrupted(number, _frame):
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
        self.root = RESULTS / "smoke"
        self.root.mkdir()
        self.names = []

    def run(self, tag, argv, *, environment=None, bound=70):
        name = f"{len(self.names):03d}-{tag}"
        self.names.append(name)
        output, errors = self.root / f"{name}.stdout", self.root / f"{name}.stderr"
        row = {
            "schema": "fe2o3.hip-smoke-command.v1",
            "name": name,
            "argv": argv,
            "cwd": str(SOURCE),
            "environment_override": environment or {},
            "visibility_unset": VISIBILITY,
            "started": stamp(),
            "reaped": None,
            "pid": None,
            "exit": None,
            "error": None,
            "group_absent": False,
            "outer_bound_seconds": bound,
        }
        env = {key: value for key, value in os.environ.items() if key not in VISIBILITY}
        env.update(environment or {})
        process = None
        try:
            with output.open("xb") as stdout, errors.open("xb") as stderr:
                old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, MANAGED)
                try:
                    process = subprocess.Popen(
                        argv,
                        cwd=SOURCE,
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
                row["reaped"] = stamp()
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
                    if row["reaped"] is None:
                        row["reaped"] = stamp()
                    row["group_absent"] = not group_exists(process.pid)
            except BaseException as cleanup_error:
                row["error"] += (
                    f"; cleanup: {type(cleanup_error).__name__}: {cleanup_error}"
                )
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        row["finished"] = stamp()
        row["stdout_sha256"], row["stderr_sha256"] = (
            check.digest(output),
            check.digest(errors),
        )
        (self.root / f"{name}.json").write_text(json.dumps(row, indent=2) + "\n")
        print(
            json.dumps({"record": name, "exit": row["exit"], "error": row["error"]}),
            flush=True,
        )
        return row


def sleep_until(deadline_ns):
    remaining = deadline_ns - time.monotonic_ns()
    if remaining > 0:
        time.sleep(remaining / 1_000_000_000)


def execute(recorder, state):
    def problem(message):
        state["failures"].append(message)

    def checked_endpoint(phase, row, t0=None):
        try:
            check.need(
                (recorder.root / (row["name"] + ".stderr")).read_text() == "",
                "observer stderr",
            )
            observed = check.endpoint(
                (recorder.root / (row["name"] + ".stdout")).read_text(), row
            )
            accepted = check.phase_accepts(phase, observed, t0)
            state["observations"][phase] = {
                **observed,
                "protocol_phase_accepted": bool(accepted),
            }
            if not accepted:
                problem(f"{phase}: refused by fixed protocol")
            return accepted
        except BaseException as error:
            problem(f"{phase}: {type(error).__name__}: {error}")
            return False

    try:
        for tag, argv in CHECKS:
            if not check.passed(recorder.run(tag + "-before", argv)):
                raise RuntimeError(tag + "-before failed")
        if not check.passed(recorder.run("topology", TOPOLOGY)):
            raise RuntimeError("topology/placement failed")
        pre = recorder.run("pre", OBSERVER)
        if not checked_endpoint("pre", pre):
            return
        if time.monotonic_ns() - pre["finished"]["monotonic_ns"] > 1_000_000_000:
            raise RuntimeError("preflight-to-native launch window missed; no launch")
        state["native_launched"] = True
        native = recorder.run("native", NATIVE, environment=NATIVE_ENV, bound=200)
        t0 = native["reaped"]
        state["native_reaped"] = t0
        # Post-observations are always attempted after native execution, even on failure.
        try:
            immediate = recorder.run("immediate", OBSERVER)
            checked_endpoint("immediate", immediate, t0)
            if not check.passed(native):
                problem("native process failure")
            else:
                try:
                    check.hip.validate(
                        (recorder.root / (native["name"] + ".stdout")).read_text(),
                        (recorder.root / (native["name"] + ".stderr")).read_text(),
                        native["exit"],
                        check.EXPECTED_HIP,
                    )
                except BaseException as error:
                    problem(f"native payload: {type(error).__name__}: {error}")
        finally:
            if t0 is not None:
                try:
                    sleep_until(t0["monotonic_ns"] + 20_000_000_000)
                except Interrupted as error:
                    problem(f"delayed schedule interrupted: {error}")
                    sleep_until(t0["monotonic_ns"] + 20_000_000_000)
            delayed = recorder.run("delayed", OBSERVER)
            checked_endpoint("delayed", delayed, t0)
    except BaseException as error:
        problem(f"attempt: {type(error).__name__}: {error}")
    finally:
        for tag, argv in CHECKS:
            try:
                if not check.passed(recorder.run(tag + "-after", argv)):
                    problem(tag + "-after failed")
            except BaseException as error:
                problem(f"{tag}-after: {type(error).__name__}: {error}")


def main():
    if sys.argv[1:] != ["--parent-ready-confirmed"]:
        raise SystemExit("requires explicit parent READY before invocation")
    check.need(
        OWNED.is_dir() and not OWNED.is_symlink() and OWNED.resolve() == OWNED,
        "exact owned directory",
    )
    check.need(OWNED.stat().st_uid == os.getuid(), "owned UID")
    check.need(
        (OWNED / "owner").read_text() == f"fe2o3-hip-smoke-{COMMIT}\n", "owner marker"
    )
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for number in MANAGED:
        signal.signal(number, interrupted)
    recorder = Recorder()
    state = {
        "schema": "fe2o3.hip-smoke-fixed-deadline.v1",
        "source_commit": COMMIT,
        "started": stamp(),
        "native_launched": False,
        "native_reaped": None,
        "observations": {},
        "failures": [],
        "performance_accepted": False,
    }
    execute(recorder, state)
    state["smoke_accepted"] = state["native_launched"] and not state["failures"]
    state["records"] = recorder.names
    state["finished"] = stamp()
    (RESULTS / "smoke.json").write_text(json.dumps(state, indent=2) + "\n")
    print(json.dumps(state, indent=2), flush=True)
    return int(not state["smoke_accepted"])


if __name__ == "__main__":
    raise SystemExit(main())
