#!/usr/bin/env python3
"""Tests for exact clean source-state capture."""

from __future__ import annotations

import importlib.util
import json
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from collections.abc import Callable
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

    def start_check(self, *arguments: str) -> subprocess.Popen[str]:
        environment = os.environ.copy()
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        return subprocess.Popen(
            [str(CHECKER), "--root", str(self.repository), *arguments],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
        )

    def wait_for(self, predicate: Callable[[], object], description: str) -> object:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            value = predicate()
            if value:
                return value
            time.sleep(0.01)
        self.fail(f"timed out waiting for {description}")

    def read_json_when_ready(self, path: pathlib.Path) -> dict[str, int] | None:
        try:
            value = json.loads(path.read_text(encoding="ascii"))
        except (FileNotFoundError, json.JSONDecodeError):
            return None
        if not isinstance(value, dict):
            return None
        return {str(key): int(item) for key, item in value.items()}

    def child_processes(self, process_id: int) -> list[int]:
        try:
            value = pathlib.Path(
                f"/proc/{process_id}/task/{process_id}/children"
            ).read_text(encoding="ascii")
        except FileNotFoundError:
            return []
        return [int(item) for item in value.split()]

    def process_state(self, process_id: int) -> str | None:
        try:
            value = pathlib.Path(f"/proc/{process_id}/stat").read_text(
                encoding="ascii"
            )
        except FileNotFoundError:
            return None
        return value[value.rfind(")") + 2 :].split()[0]

    def assert_processes_gone(self, *process_ids: int) -> None:
        for process_id in process_ids:
            with self.subTest(process_id=process_id):
                self.assertFalse(pathlib.Path(f"/proc/{process_id}").exists())

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

    def test_public_pre_spawn_signal_is_retained_and_command_is_not_started(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as coordination_name:
            marker = pathlib.Path(coordination_name) / "command-started"
            process = self.start_check(
                "--",
                sys.executable,
                "-c",
                "import pathlib,sys; pathlib.Path(sys.argv[1]).write_text('bad')",
                str(marker),
            )
            self.wait_for(
                lambda: self.child_processes(process.pid),
                "the public checker to enter source capture",
            )
            os.kill(process.pid, signal.SIGTERM)
            stdout, stderr = process.communicate(timeout=15)

        self.assertEqual(process.returncode, 128 + signal.SIGTERM, stderr)
        self.assertEqual(stdout, "")
        self.assertEqual(stderr, "")
        self.assertFalse(marker.exists())

    def test_public_post_popen_signal_is_forwarded_at_publication_checkpoint(
        self,
    ) -> None:
        wrapper = r"""
import importlib.util
import os
import pathlib
import signal
import sys

checker = pathlib.Path(sys.argv[1])
repository = pathlib.Path(sys.argv[2])
marker = pathlib.Path(sys.argv[3])
spec = importlib.util.spec_from_file_location('publication_source_state', checker)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
real_popen = module.subprocess.Popen
injected = False

def racing_popen(arguments, *args, **kwargs):
    global injected
    process = real_popen(arguments, *args, **kwargs)
    if arguments[0] == '/bin/sleep' and not injected:
        injected = True
        marker.write_text(str(process.pid), encoding='ascii')
        os.kill(os.getpid(), signal.SIGTERM)
    return process

module.subprocess.Popen = racing_popen
sys.argv = [str(checker), '--root', str(repository), '--', '/bin/sleep', '30']
try:
    status = module.main()
except module.SourceStateError as error:
    print(f's09-source-state: {error}', file=sys.stderr)
    status = 2
raise SystemExit(status)
"""
        with tempfile.TemporaryDirectory() as coordination_name:
            marker = pathlib.Path(coordination_name) / "published-pid"
            process = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    wrapper,
                    str(CHECKER),
                    str(self.repository),
                    str(marker),
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"},
                check=False,
                timeout=15,
            )
            command_pid = int(marker.read_text(encoding="ascii"))

        self.assertEqual(process.returncode, 128 + signal.SIGTERM, process.stderr)
        self.assert_processes_gone(command_pid)

    def test_public_active_signal_forwards_and_drains_stubborn_descendant(
        self,
    ) -> None:
        code = r"""
import json
import os
import pathlib
import signal

marker = pathlib.Path(__import__('sys').argv[1])
descendant = os.fork()
if descendant == 0:
    for caught in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(caught, signal.SIG_IGN)
    while True:
        signal.pause()

def exit_cleanly(_signal_number, _frame):
    raise SystemExit(0)

for caught in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
    signal.signal(caught, exit_cleanly)
marker.write_text(json.dumps({
    'leader': os.getpid(),
    'descendant': descendant,
    'group': os.getpgrp(),
}), encoding='ascii')
while True:
    signal.pause()
"""
        with tempfile.TemporaryDirectory() as coordination_name:
            marker = pathlib.Path(coordination_name) / "active.json"
            process = self.start_check(
                "--", sys.executable, "-c", code, str(marker)
            )
            identities = self.wait_for(
                lambda: self.read_json_when_ready(marker), "active command readiness"
            )
            assert isinstance(identities, dict)
            self.assertEqual(identities["leader"], identities["group"])
            self.assertNotEqual(identities["group"], os.getpgrp())
            os.kill(process.pid, signal.SIGINT)
            _stdout, stderr = process.communicate(timeout=15)

        self.assertEqual(process.returncode, 128 + signal.SIGINT, stderr)
        self.assert_processes_gone(
            identities["leader"], identities["descendant"]
        )

    def test_public_final_reap_signal_wins_and_descendant_is_drained(self) -> None:
        code = r"""
import json
import os
import pathlib
import signal

marker = pathlib.Path(__import__('sys').argv[1])
descendant = os.fork()
if descendant == 0:
    for caught in (signal.SIGHUP, signal.SIGINT, signal.SIGTERM):
        signal.signal(caught, signal.SIG_IGN)
    while True:
        signal.pause()
marker.write_text(json.dumps({
    'leader': os.getpid(),
    'descendant': descendant,
    'group': os.getpgrp(),
}), encoding='ascii')
os._exit(0)
"""
        with tempfile.TemporaryDirectory() as coordination_name:
            marker = pathlib.Path(coordination_name) / "final-reap.json"
            process = self.start_check(
                "--", sys.executable, "-c", code, str(marker)
            )
            identities = self.wait_for(
                lambda: self.read_json_when_ready(marker), "final-reap command exit"
            )
            assert isinstance(identities, dict)
            self.wait_for(
                lambda: self.process_state(identities["leader"]) == "Z",
                "the protected unreaped group leader",
            )
            os.kill(process.pid, signal.SIGHUP)
            _stdout, stderr = process.communicate(timeout=15)

        self.assertEqual(process.returncode, 128 + signal.SIGHUP, stderr)
        self.assert_processes_gone(
            identities["leader"], identities["descendant"]
        )

    def test_public_cancellation_still_rechecks_sealed_source_state(self) -> None:
        code = r"""
import os
import pathlib
import signal
import sys

path = pathlib.Path(sys.argv[1])
marker = pathlib.Path(sys.argv[2])

def mutate_and_restore(_signal_number, _frame):
    before = path.stat()
    original = path.read_bytes()
    path.write_bytes(b'X' * len(original))
    with path.open('rb') as source:
        os.fsync(source.fileno())
    path.write_bytes(original)
    os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
    raise SystemExit(0)

signal.signal(signal.SIGTERM, mutate_and_restore)
marker.write_text('ready', encoding='ascii')
while True:
    signal.pause()
"""
        with tempfile.TemporaryDirectory() as coordination_name:
            marker = pathlib.Path(coordination_name) / "mutation-ready"
            process = self.start_check(
                "--",
                sys.executable,
                "-c",
                code,
                str(self.repository / "source.txt"),
                str(marker),
            )
            self.wait_for(marker.exists, "cancellation mutation readiness")
            os.kill(process.pid, signal.SIGTERM)
            _stdout, stderr = process.communicate(timeout=15)

        self.assertEqual(process.returncode, 2, stderr)
        self.assertIn("changed during evidence generation", stderr)
        self.assertEqual((self.repository / "source.txt").read_bytes(), b"source\n")

    def test_public_signal_allows_inner_raw_guard_to_finish_cleanup(self) -> None:
        guard = ROOT / "scripts" / "s09-raw-transcript-guard.sh"
        shell_code = r"""
source "$1"
s09_install_raw_transcript_guard "$2"
s09_run_guarded_raw_command /usr/bin/python3 -c "$3" "$4"
"""
        inner_code = r"""
import pathlib
import os
import signal
import sys

pathlib.Path(sys.argv[1]).write_text(str(os.getpid()), encoding='ascii')
while True:
    signal.pause()
"""
        with tempfile.TemporaryDirectory() as coordination_name:
            coordination = pathlib.Path(coordination_name)
            raw = coordination / "rocgdb.raw.txt"
            marker = coordination / "inner-pid"
            process = self.start_check(
                "--",
                "/bin/bash",
                "-c",
                shell_code,
                "source-state-raw-test",
                str(guard),
                str(raw),
                inner_code,
                str(marker),
            )
            self.wait_for(marker.exists, "inner raw-guard command readiness")
            inner_pid = int(marker.read_text(encoding="ascii"))
            self.assertTrue(raw.exists())
            os.kill(process.pid, signal.SIGTERM)
            _stdout, stderr = process.communicate(timeout=15)

        self.assertEqual(process.returncode, 128 + signal.SIGTERM, stderr)
        self.assertFalse(raw.exists())
        self.assert_processes_gone(inner_pid)

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
