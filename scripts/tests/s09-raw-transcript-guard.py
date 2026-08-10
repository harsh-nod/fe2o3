#!/usr/bin/env python3
"""Signal tests for fail-closed S09 raw transcript deletion."""

from __future__ import annotations

import os
import pathlib
import signal
import subprocess
import tempfile
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


if __name__ == "__main__":
    unittest.main()
