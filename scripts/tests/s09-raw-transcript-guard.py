#!/usr/bin/env python3
"""Signal tests for fail-closed S09 raw transcript deletion."""

from __future__ import annotations

import ctypes
import os
import pathlib
import signal
import subprocess
import tempfile
import time
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
GUARD = ROOT / "scripts" / "s09-raw-transcript-guard.sh"


class RawTranscriptGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = pathlib.Path(self.temporary.name)
        self.raw = self.directory / "rocgdb.raw.txt"
        self.normalized = self.directory / "rocgdb.normalized.txt"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def command(self, wait: bool) -> list[str]:
        body = """
set -Eeuo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
printf 'sensitive raw transcript\n' >"$2"
printf 'normalized evidence\n' >"$3"
printf 'READY\n'
"""
        if wait:
            body += "while :; do :; done\n"
        return [
            "/bin/bash",
            "-c",
            body,
            "raw-guard-test",
            str(GUARD),
            str(self.raw),
            str(self.normalized),
        ]

    def blocking_group_command(
        self, child_pid: pathlib.Path, ready: pathlib.Path
    ) -> list[str]:
        body = r"""
set -Eeuo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/bash -c '
  trap "" TERM
  printf "%s\n" "$$" >"$1"
  printf "sensitive raw transcript\n"
  : >"$2"
  while :; do /usr/bin/sleep 1; done
' guarded-child "$3" "$4"
"""
        return [
            "/bin/bash",
            "-c",
            body,
            "raw-guard-group-test",
            str(GUARD),
            str(self.raw),
            str(child_pid),
            str(ready),
        ]

    def assert_evidence_cleanup(self) -> None:
        self.assertFalse(self.raw.exists())
        self.assertEqual(self.normalized.read_text(encoding="ascii"), "normalized evidence\n")

    def start_blocking_group(
        self,
    ) -> tuple[subprocess.Popen[str], int, int]:
        child_pid_path = self.directory / "child.pid"
        ready = self.directory / "ready"
        process = subprocess.Popen(
            self.blocking_group_command(child_pid_path, ready),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        deadline = time.monotonic() + 5
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertTrue(ready.exists(), "guarded child did not become ready")
        child_pid = int(child_pid_path.read_text(encoding="ascii"))
        return process, child_pid, os.getpgid(child_pid)

    def wait_for_raw_unlink(self, timeout: float = 0.3) -> None:
        deadline = time.monotonic() + timeout
        while self.raw.exists() and time.monotonic() < deadline:
            time.sleep(0.005)
        self.assertFalse(self.raw.exists(), "raw pathname survived cancellation entry")

    def wait_for_path(self, path: pathlib.Path, message: str) -> None:
        deadline = time.monotonic() + 5
        while not path.exists() and time.monotonic() < deadline:
            time.sleep(0.005)
        self.assertTrue(path.exists(), message)

    def set_child_subreaper(self, enabled: int) -> None:
        libc = ctypes.CDLL(None, use_errno=True)
        if libc.prctl(36, enabled, 0, 0, 0) != 0:
            errno = ctypes.get_errno()
            raise OSError(errno, os.strerror(errno))

    def child_subreaper_state(self) -> int:
        libc = ctypes.CDLL(None, use_errno=True)
        state = ctypes.c_int()
        if libc.prctl(37, ctypes.byref(state), 0, 0, 0) != 0:
            errno = ctypes.get_errno()
            raise OSError(errno, os.strerror(errno))
        return state.value

    def reap_all_adopted_children(self) -> list[int]:
        reaped: list[int] = []
        deadline = time.monotonic() + 3
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                return reaped
            if pid == 0:
                if time.monotonic() >= deadline:
                    self.fail("subreaper still owns live or unreaped test children")
                time.sleep(0.01)
                continue
            reaped.append(pid)

    def test_exit_removes_only_raw_transcript(self) -> None:
        completed = subprocess.run(
            self.command(False),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assert_evidence_cleanup()

    def test_guarded_command_status_is_preserved(self) -> None:
        body = r"""
set -Euo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/bash -c 'printf "raw output\n"; exit 37'
status=$?
[[ "$(<"$2")" == "raw output" ]] || exit 125
exit "${status}"
"""
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-status-test",
                str(GUARD),
                str(self.raw),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 37, completed.stderr)
        self.assertFalse(self.raw.exists())

    def test_hostile_pythonpath_cannot_execute_or_forge_launcher_protocol(self) -> None:
        hostile = self.directory / "hostile-pythonpath"
        hostile.mkdir()
        marker = self.directory / "sitecustomize-ran"
        (hostile / "sitecustomize.py").write_text(
            f"""\
import os
import pathlib
import sys

pathlib.Path({str(marker)!r}).write_text("executed\\n", encoding="ascii")
raw_fd = int(sys.argv[1])
os.write(raw_fd, b"forged raw output\\n")
try:
    os.setsid()
except OSError:
    pass
leader = os.getpid()
print(f"ANCHOR {{leader}} {{os.getpgrp()}}", flush=True)
print(f"READY {{leader}} {{os.getpgrp()}}", flush=True)
sys.stdin.buffer.readline()
print("STATUS 0", flush=True)
os._exit(0)
""",
            encoding="ascii",
        )
        body = r"""
set -Euo pipefail
source "$1"
export PYTHONPATH="$3"
export FE2O3_RAW_GUARD_PASSTHROUGH="$5"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/bash -c \
  'printf "%s\n" "$FE2O3_RAW_GUARD_PASSTHROUGH"; exit 1'
status=$?
[[ ! -e "$4" && ! -L "$4" ]] || exit 124
[[ "$(<"$2")" == "$5" ]] || exit 123
exit "${status}"
"""
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-hostile-pythonpath-test",
                str(GUARD),
                str(self.raw),
                str(hostile),
                str(marker),
                "preserved-non-python-environment",
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=5,
        )
        self.assertEqual(completed.returncode, 1, completed.stderr)
        self.assertFalse(marker.exists())
        self.assertFalse(self.raw.exists())

    def test_guarded_command_stdin_is_devnull_and_control_fd_is_closed(self) -> None:
        helper = self.directory / "consume-stdin.py"
        marker = self.directory / "stdin-fds.txt"
        helper.write_text(
            """\
import os
import pathlib
import sys

payload = sys.stdin.buffer.read()
targets = []
for entry in pathlib.Path("/proc/self/fd").iterdir():
    try:
        targets.append((int(entry.name), os.readlink(entry)))
    except FileNotFoundError:
        pass
with open(sys.argv[1], "w", encoding="ascii") as output:
    output.write(f"payload={payload!r}\\n")
    for descriptor, target in sorted(targets):
        output.write(f"fd{descriptor}={target}\\n")
print("raw output")
sys.exit(0 if payload == b"" else 91)
""",
            encoding="ascii",
        )
        body = r"""
set -Euo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /usr/bin/python3 -I -S "$3" "$4"
exit $?
"""
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-stdin-consumer-test",
                str(GUARD),
                str(self.raw),
                str(helper),
                str(marker),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=5,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        observations = marker.read_text(encoding="ascii").splitlines()
        self.assertEqual(observations[0], "payload=b''")
        self.assertIn("fd0=/dev/null", observations)
        self.assertFalse(any("pipe:[" in line for line in observations))
        self.assertFalse(self.raw.exists())

    def test_teardown_failure_overrides_status_and_deletes_raw(self) -> None:
        launcher = self.directory / "failed-drain-launcher.py"
        launcher.write_text(
            """\
import os
import sys

raw_fd = int(sys.argv[1])
os.setsid()
leader = os.getpid()
print(f"ANCHOR {leader} {os.getpgrp()}", flush=True)
print(f"READY {leader} {os.getpgrp()}", flush=True)
assert sys.stdin.buffer.readline() == b"START\\n"
os.write(raw_fd, b"raw output\\n")
print("STATUS 0", flush=True)
assert sys.stdin.buffer.readline() == b"DRAIN\\n"
os.close(raw_fd)
os._exit(125)
""",
            encoding="ascii",
        )
        body = r"""
set -Euo pipefail
source "$1"
TEST_LAUNCHER="$3"
s09_guarded_launcher() {
  exec /usr/bin/python3 "${TEST_LAUNCHER}" "$@"
}
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
status=$?
[[ ! -e "$2" && ! -L "$2" ]] || exit 124
exit "${status}"
"""
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-teardown-failure-test",
                str(GUARD),
                str(self.raw),
                str(launcher),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=5,
        )
        self.assertEqual(completed.returncode, 125, completed.stderr)
        self.assertFalse(self.raw.exists())

    def test_direct_exit_with_live_descendant_drains_before_returning_status(self) -> None:
        helper = self.directory / "leak-descendant.py"
        marker = self.directory / "descendant.pid"
        helper.write_text(
            """\
import os
import pathlib
import signal
import sys

ready_read, ready_write = os.pipe()
descendant = os.fork()
if descendant != 0:
    os.close(ready_write)
    os.read(ready_read, 1)
    os._exit(int(sys.argv[2]))
os.close(ready_read)
signal.signal(signal.SIGTERM, signal.SIG_IGN)
pathlib.Path(sys.argv[1]).write_text(
    f"{os.getpid()} {os.getpgrp()}\\n", encoding="ascii"
)
os.write(ready_write, b"1")
os.close(ready_write)
while True:
    signal.pause()
""",
            encoding="ascii",
        )
        body = r"""
set -Euo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /usr/bin/python3 "$3" "$4" "$5"
exit $?
"""
        previous_subreaper = self.child_subreaper_state()
        self.set_child_subreaper(1)
        try:
            for direct_status in (0, 37):
                with self.subTest(direct_status=direct_status):
                    marker.unlink(missing_ok=True)
                    completed = subprocess.run(
                        [
                            "/bin/bash",
                            "-c",
                            body,
                            "raw-guard-descendant-test",
                            str(GUARD),
                            str(self.raw),
                            str(helper),
                            str(marker),
                            str(direct_status),
                        ],
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        text=True,
                        check=False,
                        timeout=5,
                    )
                    self.assertEqual(
                        completed.returncode, direct_status, completed.stderr
                    )
                    self.assertFalse(self.raw.exists())
                    descendant_pid, descendant_pgid = map(
                        int, marker.read_text(encoding="ascii").split()
                    )
                    reaped = self.reap_all_adopted_children()
                    self.assertIn(descendant_pid, reaped)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(descendant_pid, 0)
                    with self.assertRaises(ProcessLookupError):
                        os.killpg(descendant_pgid, 0)
        finally:
            self.set_child_subreaper(previous_subreaper)

    def test_cancellation_during_final_wait_retries_reap_and_drains_group(self) -> None:
        marker = self.directory / "final-wait"
        retried = self.directory / "wait-retried"
        body = r"""
set -Eeuo pipefail
source "$1"
s09_install_raw_transcript_guard "$2"
WAIT_MARKER="$3"
WAIT_RETRIED="$4"
wait_calls=0
wait() {
  wait_calls=$((wait_calls + 1))
  if ((wait_calls == 1)); then
    : >"${WAIT_MARKER}"
    while [[ -z "${S09_GUARDED_DEFERRED_SIGNAL:-}" ]]; do :; done
    return "${S09_GUARDED_DEFERRED_SIGNAL}"
  fi
  : >"${WAIT_RETRIED}"
  builtin wait "$@"
}
s09_run_guarded_raw_command /bin/true
"""
        process = subprocess.Popen(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-final-wait-test",
                str(GUARD),
                str(self.raw),
                str(marker),
                str(retried),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            self.wait_for_path(marker, "guard did not enter final leader wait")
            os.kill(process.pid, signal.SIGTERM)
            self.wait_for_raw_unlink()
            _, stderr = process.communicate(timeout=3)
            self.assertEqual(process.returncode, 143, stderr)
            self.assertTrue(retried.exists(), "interrupted wait was not retried")
            self.assertFalse(self.raw.exists())
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=3)
            if process.stdout is not None:
                process.stdout.close()
            if process.stderr is not None:
                process.stderr.close()

    def test_protocol_failure_drains_verified_group_and_deletes_raw(self) -> None:
        marker = self.directory / "protocol-descendant.pid"
        launcher = self.directory / "invalid-status-launcher.py"
        launcher.write_text(
            """\
import os
import signal
import sys
import time

raw_fd = int(sys.argv[1])
marker = sys.argv[-1]
os.setsid()
signal.signal(signal.SIGTERM, lambda _signum, _frame: None)
leader = os.getpid()
group = os.getpgrp()
print(f"ANCHOR {leader} {group}", flush=True)
print(f"READY {leader} {group}", flush=True)
assert sys.stdin.buffer.readline() == b"START\\n"
descendant = os.fork()
if descendant == 0:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    with open(marker, "w", encoding="ascii") as output:
        output.write(f"{os.getpid()} {group}\\n")
    while True:
        signal.pause()
while not os.path.exists(marker):
    time.sleep(0.005)
print("INVALID-STATUS", flush=True)
assert sys.stdin.buffer.readline() == b"DRAIN\\n"
os.close(raw_fd)
os.killpg(group, signal.SIGTERM)
time.sleep(0.05)
os.killpg(group, signal.SIGKILL)
""",
            encoding="ascii",
        )
        body = r"""
set -Eeuo pipefail
source "$1"
TEST_LAUNCHER="$3"
TEST_MARKER="$4"
s09_guarded_launcher() {
  exec /usr/bin/python3 "${TEST_LAUNCHER}" "$@" "${TEST_MARKER}"
}
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
"""
        previous_subreaper = self.child_subreaper_state()
        descendant_pid = 0
        descendant_pgid = 0
        self.set_child_subreaper(1)
        try:
            completed = subprocess.run(
                [
                    "/bin/bash",
                    "-c",
                    body,
                    "raw-guard-protocol-test",
                    str(GUARD),
                    str(self.raw),
                    str(launcher),
                    str(marker),
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
                timeout=5,
            )
            self.assertEqual(completed.returncode, 125, completed.stderr)
            self.assertIn("returned invalid status", completed.stderr)
            self.assertFalse(self.raw.exists())
            descendant_pid, descendant_pgid = map(
                int, marker.read_text(encoding="ascii").split()
            )
            reaped = self.reap_all_adopted_children()
            self.assertIn(descendant_pid, reaped)
            with self.assertRaises(ProcessLookupError):
                os.kill(descendant_pid, 0)
            with self.assertRaises(ProcessLookupError):
                os.killpg(descendant_pgid, 0)
        finally:
            if descendant_pgid != 0:
                try:
                    os.killpg(descendant_pgid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            self.set_child_subreaper(previous_subreaper)

    def test_hup_int_and_term_remove_raw_transcript(self) -> None:
        for caught, expected in (
            (signal.SIGHUP, 129),
            (signal.SIGINT, 130),
            (signal.SIGTERM, 143),
        ):
            with self.subTest(signal=caught):
                process = subprocess.Popen(
                    self.command(True),
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                assert process.stdout is not None
                self.assertEqual(process.stdout.readline(), "READY\n")
                os.kill(process.pid, caught)
                _, stderr = process.communicate(timeout=5)
                self.assertEqual(process.returncode, expected, stderr)
                self.assert_evidence_cleanup()

    def test_signal_terminates_and_reaps_active_process_group_immediately(self) -> None:
        process, child_pid, child_pgid = self.start_blocking_group()
        started = time.monotonic()
        os.kill(process.pid, signal.SIGTERM)
        self.wait_for_raw_unlink()
        os.kill(child_pid, 0)
        _, stderr = process.communicate(timeout=3)
        self.assertEqual(process.returncode, 143, stderr)
        self.assertLess(time.monotonic() - started, 2.0)
        with self.assertRaises(ProcessLookupError):
            os.kill(child_pid, 0)
        with self.assertRaises(ProcessLookupError):
            os.killpg(child_pgid, 0)
        self.assertFalse(self.raw.exists())

    def test_sigkill_after_cancellation_unlink_cannot_restore_path(self) -> None:
        previous_subreaper = self.child_subreaper_state()
        process: subprocess.Popen[str] | None = None
        child_pid = 0
        child_pgid = 0
        reaped: list[int] = []
        self.set_child_subreaper(1)
        try:
            process, child_pid, child_pgid = self.start_blocking_group()
            os.kill(process.pid, signal.SIGTERM)
            self.wait_for_raw_unlink()
            os.kill(child_pid, 0)

            os.kill(process.pid, signal.SIGKILL)
            self.assertEqual(process.wait(timeout=3), -signal.SIGKILL)
            time.sleep(0.1)
            self.assertFalse(self.raw.exists())
        finally:
            try:
                if process is not None and process.poll() is None:
                    process.kill()
                    process.wait(timeout=3)
                if child_pgid != 0:
                    try:
                        os.killpg(child_pgid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                reaped = self.reap_all_adopted_children()
            finally:
                if process is not None:
                    if process.stdout is not None:
                        process.stdout.close()
                    if process.stderr is not None:
                        process.stderr.close()
                self.set_child_subreaper(previous_subreaper)
        self.assertIn(child_pgid, reaped)

    def test_pre_ready_hup_int_and_term_drain_without_external_cleanup(self) -> None:
        marker = self.directory / "pre-ready.pid"
        launcher = self.directory / "pre-ready-launcher.py"
        launcher.write_text(
            """\
import os
import signal
import sys
import time

raw_fd = int(sys.argv[1])
marker = sys.argv[-1]
os.setsid()
signal.signal(signal.SIGTERM, lambda _signum, _frame: None)
leader = os.getpid()
group = os.getpgrp()
print(f"ANCHOR {leader} {group}", flush=True)
with open(marker, "w", encoding="ascii") as output:
    output.write(f"{leader} {group}\\n")
assert sys.stdin.buffer.readline() == b"DRAIN\\n"
os.close(raw_fd)
os.killpg(group, signal.SIGTERM)
time.sleep(0.05)
os.killpg(group, signal.SIGKILL)
""",
            encoding="ascii",
        )
        body = r"""
set -Eeuo pipefail
source "$1"
TEST_LAUNCHER="$3"
TEST_MARKER="$4"
s09_guarded_launcher() {
  exec /usr/bin/python3 "${TEST_LAUNCHER}" "$@" "${TEST_MARKER}"
}
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
"""
        for caught, expected in (
            (signal.SIGHUP, 129),
            (signal.SIGINT, 130),
            (signal.SIGTERM, 143),
        ):
            with self.subTest(signal=caught):
                marker.unlink(missing_ok=True)
                process = subprocess.Popen(
                    [
                        "/bin/bash",
                        "-c",
                        body,
                        "raw-guard-pre-ready-test",
                        str(GUARD),
                        str(self.raw),
                        str(launcher),
                        str(marker),
                    ],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                try:
                    self.wait_for_path(marker, "launcher did not anchor before READY")
                    leader, group = map(
                        int, marker.read_text(encoding="ascii").split()
                    )
                    self.assertTrue(self.raw.exists())
                    started = time.monotonic()
                    os.kill(process.pid, caught)
                    self.wait_for_raw_unlink(timeout=0.1)
                    _, stderr = process.communicate(timeout=3)
                    self.assertEqual(process.returncode, expected, stderr)
                    self.assertLess(time.monotonic() - started, 2.0)
                    with self.assertRaises(ProcessLookupError):
                        os.kill(leader, 0)
                    with self.assertRaises(ProcessLookupError):
                        os.killpg(group, 0)
                    self.assertFalse(self.raw.exists())
                finally:
                    if process.poll() is None:
                        process.kill()
                        process.wait(timeout=3)
                    if process.stdout is not None:
                        process.stdout.close()
                    if process.stderr is not None:
                        process.stderr.close()

    def test_same_uid_stopped_launcher_times_out_fail_closed(self) -> None:
        marker = self.directory / "stopped-launcher.pid"
        launcher = self.directory / "stopped-launcher.py"
        launcher.write_text(
            """\
import os
import signal
import sys

raw_fd = int(sys.argv[1])
marker = sys.argv[-1]
os.setsid()
leader = os.getpid()
group = os.getpgrp()
print(f"ANCHOR {leader} {group}", flush=True)
print(f"READY {leader} {group}", flush=True)
assert sys.stdin.buffer.readline() == b"START\\n"
print("STATUS 0", flush=True)
assert sys.stdin.buffer.readline() == b"DRAIN\\n"
os.close(raw_fd)
with open(marker, "w", encoding="ascii") as output:
    output.write(f"{leader} {group}\\n")
os.kill(leader, signal.SIGSTOP)
""",
            encoding="ascii",
        )
        body = r"""
set -Euo pipefail
source "$1"
TEST_LAUNCHER="$3"
TEST_MARKER="$4"
s09_guarded_launcher() {
  exec /usr/bin/python3 -I -S "${TEST_LAUNCHER}" "$@" "${TEST_MARKER}"
}
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
exit $?
"""
        previous_subreaper = self.child_subreaper_state()
        leader = 0
        group = 0
        self.set_child_subreaper(1)
        started = time.monotonic()
        try:
            completed = subprocess.run(
                [
                    "/bin/bash",
                    "-c",
                    body,
                    "raw-guard-stopped-launcher-test",
                    str(GUARD),
                    str(self.raw),
                    str(launcher),
                    str(marker),
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
                timeout=6,
            )
            self.assertEqual(completed.returncode, 125, completed.stderr)
            self.assertLess(time.monotonic() - started, 5.0)
            leader, group = map(int, marker.read_text(encoding="ascii").split())
            self.assertFalse(self.raw.exists())
        finally:
            if marker.exists() and group == 0:
                leader, group = map(int, marker.read_text(encoding="ascii").split())
            if group != 0:
                try:
                    os.killpg(group, signal.SIGCONT)
                    os.killpg(group, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            reaped = self.reap_all_adopted_children()
            self.set_child_subreaper(previous_subreaper)
        self.assertIn(leader, reaped)

    def test_recycled_identity_is_never_signalled_after_launcher_reap(self) -> None:
        body = r"""
set -Eeuo pipefail
source "$1"
reaped=0
kill() {
  printf 'forbidden numeric operation (reaped=%s): %s\n' "${reaped}" "$*" >&2
  return 99
}
wait() {
  local status
  builtin wait "$@" || status=$?
  status="${status:-0}"
  reaped=1
  S09_GUARDED_CHILD_PID=424242
  return "${status}"
}
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
[[ "${reaped}" == 1 ]]
"""
        completed = subprocess.run(
            [
                "/bin/bash",
                "-c",
                body,
                "raw-guard-recycled-identity-test",
                str(GUARD),
                str(self.raw),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertNotIn("forbidden numeric operation", completed.stderr)
        self.assertFalse(self.raw.exists())


if __name__ == "__main__":
    unittest.main()
