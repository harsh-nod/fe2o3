#!/usr/bin/env python3
"""Tests for exact clean source-state capture."""

from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "s09-source-state.py"


class SourceStateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repository = pathlib.Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.name", "S09 Test")
        self.git("config", "user.email", "s09@example.invalid")
        (self.repository / "source.txt").write_text("source\n", encoding="ascii")
        self.git("add", "source.txt")
        self.git("commit", "-qm", "source")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> str:
        return subprocess.check_output(
            ["git", "-C", str(self.repository), *arguments], text=True
        ).strip()

    def check(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [str(CHECKER), "--root", str(self.repository), *arguments],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_exact_head_and_tree_are_reported_and_rechecked(self) -> None:
        commit = self.git("rev-parse", "HEAD")
        tree = self.git("rev-parse", "HEAD^{tree}")
        observed = self.check()
        self.assertEqual(observed.returncode, 0, observed.stderr)
        self.assertEqual(
            observed.stdout,
            f"source_commit\t{commit}\nsource_tree\t{tree}\n",
        )
        rechecked = self.check("--expected-commit", commit, "--expected-tree", tree)
        self.assertEqual(rechecked.returncode, 0, rechecked.stderr)
        mismatch = self.check("--expected-commit", "1" * 40, "--expected-tree", tree)
        self.assertNotEqual(mismatch.returncode, 0)
        self.assertIn("changed during evidence generation", mismatch.stderr)

    def test_untracked_and_tracked_dirty_worktrees_are_rejected(self) -> None:
        marker = self.repository / "untracked"
        marker.write_text("dirty\n", encoding="ascii")
        untracked = self.check()
        self.assertNotEqual(untracked.returncode, 0)
        self.assertIn("exactly clean", untracked.stderr)
        marker.unlink()
        (self.repository / "source.txt").write_text("changed\n", encoding="ascii")
        tracked = self.check()
        self.assertNotEqual(tracked.returncode, 0)
        self.assertIn("exactly clean", tracked.stderr)


if __name__ == "__main__":
    unittest.main()
