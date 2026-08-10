#!/usr/bin/env python3
"""Signal tests for fail-closed S09 raw transcript deletion."""

from __future__ import annotations

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
        started = time.monotonic()
        os.kill(process.pid, signal.SIGTERM)
        _, stderr = process.communicate(timeout=3)
        self.assertEqual(process.returncode, 143, stderr)
        self.assertLess(time.monotonic() - started, 2.0)
        with self.assertRaises(ProcessLookupError):
            os.kill(child_pid, 0)
        with self.assertRaises(ProcessLookupError):
            os.killpg(child_pid, 0)
        self.assertFalse(self.raw.exists())


if __name__ == "__main__":
    unittest.main()
