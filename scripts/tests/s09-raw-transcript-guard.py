#!/usr/bin/env python3
"""Signal tests for fail-closed S09 raw transcript deletion."""

from __future__ import annotations

import ctypes
import os
import pathlib
import shlex
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
exit $?
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

    def test_pre_ready_cancellation_unlinks_before_runner_can_be_killed(self) -> None:
        previous_subreaper = self.child_subreaper_state()
        marker = self.directory / "supervisor.pid"
        wrapper = self.directory / "stalled-supervisor.sh"
        wrapper.write_text(
            f"""\
source {shlex.quote(str(GUARD))}
s09_guarded_group_supervisor() {{
  local supervisor_pgid
  supervisor_pgid="$(/usr/bin/ps -o pgid= -p "${{BASHPID}}" | /usr/bin/tr -d '[:space:]')"
  [[ "${{supervisor_pgid}}" == "${{BASHPID}}" ]] || return 125
  printf '%s\\n' "${{BASHPID}}" >{shlex.quote(str(marker))}
  kill -STOP -- "${{BASHPID}}"
  return 125
}}
""",
            encoding="ascii",
        )
        body = r"""
set -Eeuo pipefail
source "$1"
S09_RAW_GUARD_SOURCE="$3"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /bin/true
"""
        process: subprocess.Popen[str] | None = None
        supervisor_pid = 0
        reaped: list[int] = []
        self.set_child_subreaper(1)
        try:
            process = subprocess.Popen(
                [
                    "/bin/bash",
                    "-c",
                    body,
                    "raw-guard-pre-ready-test",
                    str(GUARD),
                    str(self.raw),
                    str(wrapper),
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            deadline = time.monotonic() + 5
            while not marker.exists() and time.monotonic() < deadline:
                time.sleep(0.005)
            self.assertTrue(marker.exists(), "supervisor did not stop before READY")
            supervisor_pid = int(marker.read_text(encoding="ascii"))
            self.assertTrue(self.raw.exists(), "test did not observe the pre-READY raw path")

            os.kill(process.pid, signal.SIGTERM)
            self.wait_for_raw_unlink(timeout=0.1)
            os.kill(process.pid, signal.SIGKILL)
            self.assertEqual(process.wait(timeout=3), -signal.SIGKILL)
            time.sleep(0.05)
            self.assertFalse(self.raw.exists())
        finally:
            try:
                if process is not None and process.poll() is None:
                    process.kill()
                    process.wait(timeout=3)
                if supervisor_pid != 0:
                    try:
                        os.killpg(supervisor_pid, signal.SIGKILL)
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
        self.assertIn(supervisor_pid, reaped)

    def test_no_group_operation_occurs_after_supervisor_reap(self) -> None:
        body = r"""
set -Eeuo pipefail
source "$1"
reaped=0
kill() {
  if ((reaped != 0)); then
    printf 'numeric operation after reap: %s\n' "$*" >&2
    return 99
  fi
  printf 'KILL %s\n' "$*"
}
wait() {
  reaped=1
  printf 'WAIT %s\n' "$*"
}
S09_GUARDED_CHILD_PID=424242
s09_stop_guarded_group
[[ "${reaped}" == 1 ]]
"""
        completed = subprocess.run(
            ["/bin/bash", "-c", body, "raw-guard-reap-test", str(GUARD)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        operations = completed.stdout.splitlines()
        self.assertEqual(operations[-1], "WAIT 424242")
        self.assertNotIn("numeric operation after reap", completed.stderr)


if __name__ == "__main__":
    unittest.main()
