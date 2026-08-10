#!/usr/bin/env python3
"""Tests for exact clean source-state capture."""

from __future__ import annotations

import pathlib
import subprocess
import sys
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

    def supervise(self, code: str) -> subprocess.CompletedProcess[str]:
        return self.check(
            "--",
            sys.executable,
            "-c",
            code,
            str(self.repository / "source.txt"),
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

    def test_supervisor_substitutes_exact_commit_and_tree(self) -> None:
        commit = self.git("rev-parse", "HEAD")
        tree = self.git("rev-parse", "HEAD^{tree}")
        completed = self.check(
            "--",
            sys.executable,
            "-c",
            "import sys; print(sys.argv[1] + ':' + sys.argv[2])",
            "{source_commit}",
            "{source_tree}",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout, f"{commit}:{tree}\n")

    def test_same_size_mutate_restore_is_rejected_by_ctime(self) -> None:
        code = r"""
import os
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
before = path.stat()
original = path.read_bytes()
path.write_bytes(b'X' * len(original))
with path.open('rb') as source:
    os.fsync(source.fileno())
path.write_bytes(original)
os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
"""
        completed = self.supervise(code)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("changed during evidence generation", completed.stderr)
        self.assertEqual((self.repository / "source.txt").read_bytes(), b"source\n")

    def test_path_swap_and_restore_is_rejected(self) -> None:
        code = r"""
import os
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
held = path.with_name('held-source')
replacement = path.with_name('replacement-source')
replacement.write_bytes(path.read_bytes())
replacement.chmod(path.stat().st_mode & 0o777)
path.rename(held)
replacement.rename(path)
path.unlink()
held.rename(path)
"""
        completed = self.supervise(code)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("changed during evidence generation", completed.stderr)

    def test_mode_change_and_restore_is_rejected(self) -> None:
        code = r"""
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
mode = path.stat().st_mode & 0o777
path.chmod(mode | 0o100)
path.chmod(mode)
"""
        completed = self.supervise(code)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("changed during evidence generation", completed.stderr)


if __name__ == "__main__":
    unittest.main()
