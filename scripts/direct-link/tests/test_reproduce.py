#!/usr/bin/env python3

from __future__ import annotations

import dataclasses
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parents[1]
MOCK_BUILD = Path(__file__).resolve().with_name("mock_build.py")
sys.path.insert(0, str(SCRIPT_DIR))

import reproduce  # noqa: E402
from common import typed_identity  # noqa: E402

TOOLCHAIN = typed_identity(reproduce.TOOLCHAIN_DOMAIN, b"llvm-22-test-toolchain")
WORKER = typed_identity(reproduce.WORKER_DOMAIN, b"worker-test-build")
REQUEST = typed_identity(reproduce.REQUEST_DOMAIN, b"canonical-link-request")
PYTHON = str(Path(sys.executable).resolve(strict=True))


class ReproducibilityCliTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.work = self.root / "work"
        self.source.mkdir()
        self.work.mkdir()
        shutil.copyfile(MOCK_BUILD, self.source / "mock_build.py")
        (self.source / "payload.txt").write_bytes(b"tracked source payload\n")
        self.git("init", "--quiet")
        self.git("config", "user.name", "G8 Test")
        self.git("config", "user.email", "g8@example.invalid")
        self.git("add", "mock_build.py", "payload.txt")
        self.git("commit", "--quiet", "-m", "fixture")
        self.commit = self.git("rev-parse", "HEAD").stdout.decode("ascii").strip()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            ["git", "-C", str(self.source), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )

    def run_cli(
        self, *arguments: str, environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT_DIR / "reproduce.py"), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
            check=False,
        )

    def run_arguments(
        self,
        target: str,
        mode: str = "stable",
        marker: Path | None = None,
        executable: str = PYTHON,
        extra: tuple[str, ...] = (),
    ) -> list[str]:
        arguments = [
            "run",
            "--commit",
            self.commit,
            "--target",
            target,
            "--linked-artifact",
            "output/linked.hsaco",
            "--final-artifact",
            "output/final.hsaco",
            "--source-dir",
            str(self.source),
            "--work-root",
            str(self.work),
            "--timeout",
            "1",
            "--llvm-toolchain-identity",
            TOOLCHAIN,
            "--worker-identity",
            WORKER,
            "--request-identity",
            REQUEST,
            "--",
            executable,
            "{source_dir}/mock_build.py",
            "--linked",
            "{build_dir}/output/linked.hsaco",
            "--final",
            "{build_dir}/output/final.hsaco",
            "--target",
            "{target}",
            "--mode",
            mode,
            "--require-clean-env",
        ]
        if marker is not None:
            arguments.extend(("--marker", str(marker)))
        arguments.extend(extra)
        return arguments

    def parse_stdout(
        self, completed: subprocess.CompletedProcess[bytes], name: str = "result.tsv"
    ) -> reproduce.ReproducibilityResult:
        path = self.root / name
        path.write_bytes(completed.stdout)
        return reproduce.parse_result(path)

    def write_result(self, result: reproduce.ReproducibilityResult, name: str) -> Path:
        path = self.root / name
        path.write_bytes(result.canonical_bytes())
        return path

    def validate_arguments(
        self, path: Path, result: reproduce.ReproducibilityResult
    ) -> list[str]:
        return [
            "validate",
            str(path),
            "--expect-commit",
            result.git_commit,
            "--expect-source-tree-identity",
            result.source_tree_identity,
            "--expect-source-snapshot-identity",
            result.source_snapshot_identity,
            "--expect-argv-identity",
            result.canonical_argv_identity,
            "--build-executable",
            PYTHON,
            "--expect-environment-identity",
            result.environment_identity,
            "--expect-llvm-toolchain-identity",
            result.llvm_toolchain_identity,
            "--expect-worker-identity",
            result.worker_identity,
            "--expect-request-identity",
            result.request_identity,
            "--expect-target",
            result.target,
        ]

    def test_run_uses_two_clean_detached_snapshots_and_emits_v3(self) -> None:
        environment = dict(os.environ)
        environment["FE2O3_TEST_POISON"] = "must-not-leak"
        completed = self.run_cli(*self.run_arguments("gfx942"), environment=environment)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        result = self.parse_stdout(completed)
        self.assertEqual(result.status, "pass")
        self.assertEqual(
            result.first_linked_artifact_identity,
            result.second_linked_artifact_identity,
        )
        self.assertEqual(
            result.first_final_artifact_identity,
            result.second_final_artifact_identity,
        )
        self.assertIn(result.first_source_dir, result.first_expanded_argv)
        self.assertIn(result.first_build_dir, result.first_expanded_argv)
        self.assertIn("gfx942", result.first_expanded_argv)
        self.assertNotEqual(result.first_expanded_argv, result.second_expanded_argv)
        self.assertEqual(list(self.work.iterdir()), [])
        self.assertEqual(self.git("status", "--porcelain").stdout, b"")

    def test_domain_is_part_of_identity_preimage(self) -> None:
        payload = b"same payload"
        first = typed_identity("fe2o3-domain-one-v1", payload)
        second = typed_identity("fe2o3-domain-two-v1", payload)
        self.assertNotEqual(first.rsplit("-", 1)[1], second.rsplit("-", 1)[1])

    def test_detects_nondeterministic_artifacts(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx1151", "unstable"))
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        result = self.parse_stdout(completed)
        self.assertEqual(result.status, "fail")
        self.assertEqual(result.reason, "artifact-mismatch")

    def test_large_artifacts_are_not_limited_by_log_bound(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx950", "large"))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertEqual(self.parse_stdout(completed).status, "pass")

    def test_timeout_kills_build(self) -> None:
        started = time.monotonic()
        completed = self.run_cli(*self.run_arguments("gfx942", "timeout"))
        self.assertLess(time.monotonic() - started, 4)
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "build-timeout")

    def test_post_exit_descendant_mutation_is_cleaned(self) -> None:
        marker = self.root / "descendant-marker"
        completed = self.run_cli(*self.run_arguments("gfx942", "descendant", marker))
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        time.sleep(0.8)
        self.assertFalse(marker.exists())

    def test_parent_owned_log_capture_is_bounded(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "noisy"))
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "build-log-limit")

    def test_source_mutation_is_rejected_and_original_is_unchanged(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "mutate-source"))
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "source-snapshot-mutated")
        self.assertEqual(
            (self.source / "payload.txt").read_bytes(), b"tracked source payload\n"
        )

    def test_source_mutate_then_restore_is_rejected_by_metadata_guard(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "mutate-source-restore"))
        self.assertEqual(completed.returncode, 1)
        self.assertEqual(self.parse_stdout(completed).reason, "source-snapshot-mutated")

    def test_intermediate_artifact_symlink_escape_is_rejected(self) -> None:
        escape = self.root / "escaped-artifacts"
        completed = self.run_cli(
            *self.run_arguments(
                "gfx942",
                "symlink-intermediate",
                extra=("--escape-root", str(escape)),
            )
        )
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        self.assertEqual(self.parse_stdout(completed).reason, "artifact-unmeasurable")
        self.assertTrue((escape / "linked.hsaco").exists())

    def test_executable_swap_runs_pinned_bytes_and_is_rejected(self) -> None:
        executable = self.root / "mutable-python"
        shutil.copyfile(PYTHON, executable)
        executable.chmod(0o755)
        completed = self.run_cli(
            *self.run_arguments(
                "gfx942",
                "swap-executable",
                executable=str(executable),
                extra=("--swap-executable", str(executable)),
            )
        )
        self.assertEqual(completed.returncode, 1, completed.stderr.decode())
        self.assertEqual(
            self.parse_stdout(completed).reason, "build-executable-mutated"
        )
        self.assertEqual(executable.read_bytes(), b"swapped executable")

    def test_fail_missing_and_unavailable_commands_are_explicit(self) -> None:
        cases = (
            (self.run_arguments("gfx950", "fail"), "build-command-failed"),
            (self.run_arguments("gfx950", "missing"), "artifact-unmeasurable"),
        )
        for index, (arguments, reason) in enumerate(cases):
            with self.subTest(reason=reason):
                completed = self.run_cli(*arguments)
                self.assertEqual(completed.returncode, 1)
                self.assertEqual(
                    self.parse_stdout(completed, f"failure-{index}.tsv").reason,
                    reason,
                )

        arguments = self.run_arguments("gfx950")
        separator = arguments.index("--")
        arguments[separator + 1] = "/definitely/unavailable/fe2o3-builder"
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"cannot pin build executable", completed.stderr)

    def test_inspect_accepts_failure_but_validate_returns_nonzero(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942", "unstable"))
        result = self.parse_stdout(completed)
        path = self.write_result(result, "failure.tsv")
        self.assertEqual(self.run_cli("inspect", str(path)).returncode, 0)
        self.assertEqual(
            self.run_cli(*self.validate_arguments(path, result)).returncode, 1
        )
        validated = self.run_cli(*self.validate_arguments(path, result))
        self.assertIn(reproduce.RELEASE_BLOCK_REASON.encode("ascii"), validated.stdout)

    def test_validate_blocks_fabricated_internally_consistent_pass(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942"))
        result = self.parse_stdout(completed)
        forged_artifact = typed_identity(
            reproduce.LINKED_ARTIFACT_DOMAIN, b"fabricated linked artifact"
        )
        forged_final = typed_identity(
            reproduce.FINAL_ARTIFACT_DOMAIN, b"fabricated final artifact"
        )
        forged = dataclasses.replace(
            result,
            source_tree_identity=typed_identity(
                reproduce.SOURCE_TREE_DOMAIN, b"fabricated tree"
            ),
            source_snapshot_identity=typed_identity(
                reproduce.SOURCE_SNAPSHOT_DOMAIN, b"fabricated snapshot"
            ),
            first_linked_artifact_identity=forged_artifact,
            second_linked_artifact_identity=forged_artifact,
            first_final_artifact_identity=forged_final,
            second_final_artifact_identity=forged_final,
        )
        path = self.write_result(forged, "fabricated-pass.tsv")
        inspected = self.run_cli("inspect", str(path))
        self.assertEqual(inspected.returncode, 0)
        validated = self.run_cli(*self.validate_arguments(path, forged))
        self.assertEqual(validated.returncode, 1)
        self.assertIn(reproduce.RELEASE_BLOCK_REASON.encode("ascii"), validated.stdout)

    def test_validate_rejects_command_source_and_toolchain_substitution(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942"))
        result = self.parse_stdout(completed)
        path = self.write_result(result, "valid.tsv")
        base = self.validate_arguments(path, result)
        substitutions = {
            "--expect-argv-identity": typed_identity(
                reproduce.ARGV_DOMAIN, b"substituted command"
            ),
            "--expect-source-tree-identity": typed_identity(
                reproduce.SOURCE_TREE_DOMAIN, b"substituted source"
            ),
            "--expect-source-snapshot-identity": typed_identity(
                reproduce.SOURCE_SNAPSHOT_DOMAIN, b"substituted snapshot"
            ),
            "--expect-llvm-toolchain-identity": typed_identity(
                reproduce.TOOLCHAIN_DOMAIN, b"substituted toolchain"
            ),
        }
        for option, value in substitutions.items():
            with self.subTest(option=option):
                arguments = list(base)
                arguments[arguments.index(option) + 1] = value
                rejected = self.run_cli(*arguments)
                self.assertEqual(rejected.returncode, 2)
                self.assertIn(b"mismatch", rejected.stderr)

    def test_record_rejects_forged_argv_identity_and_tampering(self) -> None:
        completed = self.run_cli(*self.run_arguments("gfx942"))
        result = self.parse_stdout(completed)
        forged = dataclasses.replace(
            result,
            canonical_argv_identity=typed_identity(
                reproduce.ARGV_DOMAIN, b"unbound command"
            ),
        )
        path = self.write_result(forged, "forged.tsv")
        with self.assertRaisesRegex(reproduce.EvidenceError, "argv_identity mismatch"):
            reproduce.parse_result(path)

        forged_expansion = dataclasses.replace(
            result, first_expanded_argv=result.second_expanded_argv
        )
        path = self.write_result(forged_expansion, "forged-expansion.tsv")
        with self.assertRaisesRegex(reproduce.EvidenceError, "expanded argv"):
            reproduce.parse_result(path)

        valid = self.write_result(result, "tampered.tsv")
        data = valid.read_bytes()
        valid.write_bytes(data[:-2] + (b"0" if data[-2:-1] != b"0" else b"1") + b"\n")
        with self.assertRaisesRegex(
            reproduce.EvidenceError, "record_identity mismatch"
        ):
            reproduce.parse_result(valid)

    def test_invalid_target_features_and_artifact_traversal_are_rejected(self) -> None:
        for target in ("gfx999", "gfx1151:xnack+", "gfx942:xnack+:sramecc-"):
            with self.subTest(target=target):
                completed = self.run_cli(*self.run_arguments(target))
                self.assertEqual(completed.returncode, 2)
        arguments = self.run_arguments("gfx942")
        arguments[arguments.index("output/final.hsaco")] = "../final.hsaco"
        completed = self.run_cli(*arguments)
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"without traversal", completed.stderr)

    def test_command_requires_source_build_and_target_placeholders(self) -> None:
        for placeholder in ("{source_dir}", "{build_dir}", "{target}"):
            with self.subTest(placeholder=placeholder):
                arguments = self.run_arguments("gfx942")
                arguments = [
                    argument.replace(placeholder, "fixed") for argument in arguments
                ]
                completed = self.run_cli(*arguments)
                self.assertEqual(completed.returncode, 2)
                self.assertIn(placeholder.encode("ascii"), completed.stderr)

    def test_release_like_matrix_command_is_removed(self) -> None:
        completed = self.run_cli("matrix")
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"invalid choice", completed.stderr)

    def test_unbound_submodule_is_rejected(self) -> None:
        self.git(
            "update-index",
            "--add",
            "--cacheinfo",
            f"160000,{self.commit},unbound-submodule",
        )
        self.git("commit", "--quiet", "-m", "add gitlink")
        self.commit = self.git("rev-parse", "HEAD").stdout.decode("ascii").strip()
        completed = self.run_cli(*self.run_arguments("gfx942"))
        self.assertEqual(completed.returncode, 2)
        self.assertIn(b"unbound submodule", completed.stderr)

    def test_global_git_filter_configuration_is_disabled(self) -> None:
        marker = self.root / "global-filter-ran"
        attributes = self.source / ".gitattributes"
        attributes.write_text("payload.txt filter=poison\n", encoding="ascii")
        self.git("add", ".gitattributes")
        self.git("commit", "--quiet", "-m", "add filter attribute")
        self.commit = self.git("rev-parse", "HEAD").stdout.decode("ascii").strip()

        global_config = self.root / "host-global-gitconfig"
        subprocess.run(
            [
                "git",
                "config",
                "--file",
                str(global_config),
                "filter.poison.smudge",
                f"touch {marker}; cat",
            ],
            check=True,
        )
        subprocess.run(
            [
                "git",
                "config",
                "--file",
                str(global_config),
                "filter.poison.required",
                "true",
            ],
            check=True,
        )
        environment = dict(os.environ)
        environment["GIT_CONFIG_GLOBAL"] = str(global_config)
        completed = self.run_cli(*self.run_arguments("gfx942"), environment=environment)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertFalse(marker.exists())


if __name__ == "__main__":
    unittest.main()
