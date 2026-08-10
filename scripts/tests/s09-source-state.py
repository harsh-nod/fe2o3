#!/usr/bin/env python3
"""Tests for exact clean source-state capture."""

from __future__ import annotations

import importlib.util
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "s09-source-state.py"
SPEC = importlib.util.spec_from_file_location("s09_source_state", CHECKER)
assert SPEC is not None and SPEC.loader is not None
SOURCE_STATE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SOURCE_STATE)


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

    def check(
        self, *arguments: str, environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        child_environment = os.environ.copy()
        if environment is not None:
            child_environment.update(environment)
        return subprocess.run(
            [str(CHECKER), "--root", str(self.repository), *arguments],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=child_environment,
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

    def test_inherited_git_environment_and_configs_are_ignored(self) -> None:
        with tempfile.NamedTemporaryFile("w", encoding="ascii") as config:
            config.write("[core]\n\tbare = true\n")
            config.flush()
            completed = self.check(
                environment={
                    "GIT_CONFIG_GLOBAL": config.name,
                    "GIT_CONFIG_SYSTEM": config.name,
                    "GIT_CONFIG_COUNT": "1",
                    "GIT_CONFIG_KEY_0": "core.bare",
                    "GIT_CONFIG_VALUE_0": "true",
                    "GIT_DIR": "/does/not/exist",
                    "GIT_INDEX_FILE": "/does/not/exist",
                    "GIT_WORK_TREE": "/does/not/exist",
                }
            )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_repository_local_executable_helpers_are_disabled(self) -> None:
        with tempfile.TemporaryDirectory() as helper_name:
            helper_root = pathlib.Path(helper_name)
            fsmonitor_marker = helper_root / "fsmonitor-ran"
            fsmonitor = helper_root / "fsmonitor"
            fsmonitor.write_text(
                "#!/bin/sh\nprintf invoked > "
                + str(fsmonitor_marker)
                + "\nprintf '2\\n'\n",
                encoding="ascii",
            )
            fsmonitor.chmod(0o755)
            hooks = helper_root / "hooks"
            hooks.mkdir()
            hook_marker = helper_root / "hook-ran"
            hook = hooks / "post-index-change"
            hook.write_text(
                "#!/bin/sh\nprintf invoked > " + str(hook_marker) + "\n",
                encoding="ascii",
            )
            hook.chmod(0o755)
            self.git("config", "core.fsmonitor", str(fsmonitor))
            self.git("status", "--porcelain=v1")
            self.assertTrue(fsmonitor_marker.exists())
            fsmonitor_marker.unlink()
            self.git("config", "core.hooksPath", str(hooks))

            completed = self.check()
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertFalse(fsmonitor_marker.exists())
            with SOURCE_STATE.PinnedGit() as git_tool:
                with self.assertRaises(SOURCE_STATE.SourceStateError):
                    git_tool.run(
                        self.repository,
                        "hook",
                        "run",
                        "post-index-change",
                        "--",
                        "0",
                        "0",
                    )
            self.assertFalse(hook_marker.exists())

    def test_replace_refs_do_not_change_reported_source_objects(self) -> None:
        original_commit = self.git("rev-parse", "HEAD")
        original_tree = self.git("rev-parse", "HEAD^{tree}")
        (self.repository / "source.txt").write_text("replacement\n", encoding="ascii")
        self.git("add", "source.txt")
        self.git("commit", "-qm", "replacement")
        replacement_commit = self.git("rev-parse", "HEAD")
        self.git("reset", "--hard", "-q", original_commit)
        self.git("replace", original_commit, replacement_commit)
        self.assertNotEqual(self.git("rev-parse", "HEAD^{tree}"), original_tree)

        completed = self.check()
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(
            completed.stdout,
            f"source_commit\t{original_commit}\nsource_tree\t{original_tree}\n",
        )

    def test_pinned_git_bounds_stdout_and_stderr_while_running(self) -> None:
        with tempfile.TemporaryDirectory() as helper_name:
            helper = pathlib.Path(helper_name) / "output-helper"
            helper.write_text(
                "#!/usr/bin/python3\n"
                "import os\n"
                "import sys\n"
                "import time\n"
                "stream = 1 if sys.argv[1] == 'stdout' else 2\n"
                "for _ in range(16):\n"
                "    os.write(stream, b'x' * 1024)\n"
                "time.sleep(30)\n",
                encoding="ascii",
            )
            helper.chmod(0o755)
            for stream in ("stdout", "stderr"):
                self.git(
                    "config",
                    f"alias.emit-{stream}",
                    f"!{helper} {stream}",
                )
            with SOURCE_STATE.PinnedGit() as git_tool:
                with mock.patch.object(SOURCE_STATE, "MAX_GIT_OUTPUT_BYTES", 4096):
                    for stream in ("stdout", "stderr"):
                        with self.subTest(stream=stream):
                            with self.assertRaisesRegex(
                                SOURCE_STATE.SourceStateError,
                                f"Git {stream} exceeds the 4096-byte",
                            ):
                                git_tool.run(self.repository, f"emit-{stream}")

    def test_pinned_git_failure_preserves_bounded_stderr_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory() as executable_name:
            executable = pathlib.Path(executable_name) / "failing-git"
            executable.write_text(
                "#!/bin/sh\nprintf 'bounded diagnostic' >&2\nexit 7\n",
                encoding="ascii",
            )
            executable.chmod(0o755)
            with SOURCE_STATE.PinnedGit(executable) as git_tool:
                with self.assertRaisesRegex(
                    SOURCE_STATE.SourceStateError, "bounded diagnostic"
                ):
                    git_tool.run(self.repository, "status")

    def test_tracked_symlink_is_rejected(self) -> None:
        (self.repository / "source-link").symlink_to("source.txt")
        self.git("add", "source-link")
        self.git("commit", "-qm", "symlink")
        completed = self.check()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("unsupported tracked Git mode 120000", completed.stderr)

    def test_tracked_gitlink_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as source_name:
            source = pathlib.Path(source_name)
            subprocess.check_call(["git", "-C", str(source), "init", "-q"])
            subprocess.check_call(
                ["git", "-C", str(source), "config", "user.name", "S09 Test"]
            )
            subprocess.check_call(
                [
                    "git",
                    "-C",
                    str(source),
                    "config",
                    "user.email",
                    "s09@example.invalid",
                ]
            )
            (source / "nested.txt").write_text("nested\n", encoding="ascii")
            subprocess.check_call(["git", "-C", str(source), "add", "nested.txt"])
            subprocess.check_call(["git", "-C", str(source), "commit", "-qm", "nested"])
            self.git(
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                str(source),
                "nested",
            )
            self.git("commit", "-qm", "gitlink")
            completed = self.check()
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("unsupported tracked Git mode 160000", completed.stderr)

    def test_pinned_git_path_substitution_is_rejected(self) -> None:
        executable = self.repository / "pinned-git"
        parked = self.repository / "held-git"
        shutil.copy2("/usr/bin/git", executable)
        with SOURCE_STATE.PinnedGit(executable) as git_tool:
            git_tool.run(self.repository, "rev-parse", "--verify", "HEAD")
            executable.rename(parked)
            shutil.copy2("/usr/bin/git", executable)
            with self.assertRaisesRegex(
                SOURCE_STATE.SourceStateError,
                "identity or content changed",
            ):
                git_tool.run(self.repository, "rev-parse", "--verify", "HEAD")


if __name__ == "__main__":
    unittest.main()
