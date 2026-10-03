#!/usr/bin/env python3
"""Exercise the production core dispatcher without building or skipping a group."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CI_LOCAL = ROOT / "scripts/ci-local.sh"
POLICY = [
    "run_workspace_dependency_policy",
    "run_standalone_lockfiles",
    "run_runtime_pure_rust_policy",
    "example-manifest",
    "bounded-moe-docs",
    "run_shard_policy",
    "run_parity_matrix_checks",
    "run_format",
    "run_check",
    "run_backend_build",
    "simulation-expectation-tests",
    "tutorial-scalar-gemm-corpus-tests",
    "quickstart-shell-tests",
    "kernel-compile-matrix-shell-tests",
    "tutorial-cpu-reference-tests",
    "no-gpu-source-quickstart",
    "kir-sim-capability-matrix",
    "kir-sim-scalar-differential",
    "kir-sim-semantic-differential",
    "kir-sim-f32-differential",
    "ci-local-test-gate",
    "generic-core-group-tests",
]
GROUPS = {
    "policy": POLICY,
    "cpu": ["run_cpu_tests"],
    "auxiliary": ["run_rustc_codegen_lib_tests", "run_auxiliary_tests"],
}
ALL = sum(GROUPS.values(), [])
LEAVES = [name for name in ALL if name.startswith("run_")]
HARNESS = r'''
set -Eeuo pipefail
source "$1"
shift
observe() {
  printf '%s\n' "$1"
  if [[ "$1" == "${FAIL_AT:-}" ]]; then
    return 29
  fi
}
run_step() { observe "$1"; }
''' + "\n".join(
    f"{name}() {{ observe {name}; }}"
    for name in LEAVES + ["run_all_rustc_codegen_shards"]
) + '\n"$@"\n'


class GenericCoreGroupTests(unittest.TestCase):
    def invoke(self, *arguments: str, failure: str = "") -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory(prefix="fe2o3-core-group-test-") as directory:
            environment = {
                "PATH": os.environ["PATH"],
                "HOME": os.environ["HOME"],
                "CARGO_TARGET_DIR": str(Path(directory) / "target"),
                "CI_LOG_DIR": str(Path(directory) / "logs"),
                "FAIL_AT": failure,
            }
            return subprocess.run(
                ["bash", "-c", HARNESS, "bash", str(CI_LOCAL), *arguments],
                cwd=ROOT, env=environment, text=True, capture_output=True,
                check=False, timeout=30,
            )

    def assert_trace(self, result: subprocess.CompletedProcess[str], expected: list[str]) -> None:
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), expected)

    def test_default_and_explicit_all_retain_every_existing_group(self) -> None:
        self.assert_trace(self.invoke("run_generic_core"), ALL)
        self.assert_trace(self.invoke("run_generic_core", "all"), ALL)
        self.assert_trace(self.invoke("main", "generic-core"), ALL)

    def test_hosted_groups_partition_the_same_ordered_work_exactly_once(self) -> None:
        actual = []
        for group, expected in GROUPS.items():
            with self.subTest(group=group):
                result = self.invoke("main", "generic-core", group)
                self.assert_trace(result, expected)
                actual.extend(result.stdout.splitlines())
        self.assertEqual(actual, ALL)
        self.assertEqual(len(actual), len(set(actual)))

    def test_generic_still_includes_all_core_and_codegen_shards(self) -> None:
        self.assert_trace(self.invoke("main", "generic"), ALL + ["run_all_rustc_codegen_shards"])

    def test_unknown_empty_or_extra_group_fails_before_any_work(self) -> None:
        for arguments in [("unknown",), ("",), ("--cpu",), ("policy", "cpu"), ("all", "all")]:
            for entry in [("run_generic_core",), ("main", "generic-core")]:
                with self.subTest(arguments=arguments, entry=entry):
                    result = self.invoke(*entry, *arguments)
                    self.assertEqual(result.returncode, 2)
                    self.assertEqual(result.stdout, "")

    def test_each_group_propagates_first_failure_and_stops(self) -> None:
        for group, expected in GROUPS.items():
            with self.subTest(group=group):
                result = self.invoke("main", "generic-core", group, failure=expected[0])
                self.assertEqual(result.returncode, 29, result.stderr)
                self.assertEqual(result.stdout.splitlines(), expected[:1])

    def test_full_core_stops_before_later_groups_after_failure(self) -> None:
        for failed in [POLICY[-1], "run_cpu_tests", "run_rustc_codegen_lib_tests"]:
            with self.subTest(failed=failed):
                result = self.invoke("main", "generic-core", failure=failed)
                self.assertEqual(result.returncode, 29, result.stderr)
                self.assertEqual(result.stdout.splitlines(), ALL[:ALL.index(failed) + 1])


if __name__ == "__main__":
    unittest.main()
