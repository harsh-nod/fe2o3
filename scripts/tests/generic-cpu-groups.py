#!/usr/bin/env python3
"""Compare real CI dispatch commands; tool execution is replaced, not selection."""

from __future__ import annotations

from collections import Counter
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ci-local.sh"
PACKAGE_GROUPS = ("foundation", "analysis", "lowering", "pliron", "finalize")
CPU_GROUPS = (*PACKAGE_GROUPS, "integration")
RAW = ("fe2o3-raw-a", "fe2o3-raw-b")
MANAGED = ("fe2o3-managed-a", "fe2o3-managed-b")
HEAVY = {
    "analysis": ("fe2o3-kernel-analysis", "fe2o3-kernel-ir", "fe2o3-mir-model", "fe2o3-verifier"),
    "lowering": ("fe2o3-lower-mir-kernel",),
    "pliron": ("fe2o3-pliron", "fe2o3-pliron-conformance"),
    "finalize": ("fe2o3-hsaco-finalize",),
}
HARNESS = r'''
set -Eeuo pipefail
source "$1"
shift
record() {
  printf '%s\0' "$@"
  printf '\n'
  [[ "$1" != "${FAIL_AT:-}" ]] || return 29
}
run_step() { record "$@"; }
ensure_production_cargo_fe2o3_driver() {
  record driver-bootstrap "$@"
  CARGO_FE2O3_BINARY=/sealed-production-driver
}
validate_cargo_fe2o3_driver() { record driver-validate; }
load_dynamic_loader_environment_removals() {
  local -n output="$1"
  output=(-u LD_PRELOAD -u LD_LIBRARY_PATH)
}
load_example_packages() {
  local -n output="$2"
  case "$1" in
    cpu-test-raw)
      output=(fe2o3-raw-a fe2o3-raw-b)
      if [[ "${EMPTY_RAW:-0}" == 1 ]]; then output=(); fi
      if [[ "${DUPLICATE_RAW:-0}" == 1 ]]; then output+=(fe2o3-raw-a); fi
      ;;
    cpu-test-wrapper-managed)
      output=(fe2o3-managed-a fe2o3-managed-b)
      if [[ "${OVERLAP:-0}" == 1 ]]; then output+=(fe2o3-raw-a); fi
      ;;
    wrapper-managed) output=(fe2o3-managed-a fe2o3-managed-b) ;;
    *) return 2 ;;
  esac
}
show_cpu_roster() { record roster "${CPU_TEST_PACKAGES[@]}"; }
"$@"
'''


class CpuGroupTests(unittest.TestCase):
    def invoke(self, *arguments: str, failure: str = "", **overrides: str):
        with tempfile.TemporaryDirectory(prefix="fe2o3-cpu-groups-") as directory:
            environment = {
                "PATH": os.environ["PATH"],
                "HOME": os.environ["HOME"],
                "CARGO_TARGET_DIR": str(Path(directory) / "target"),
                "CI_LOG_DIR": str(Path(directory) / "logs"),
                "FAIL_AT": failure,
                **overrides,
            }
            result = subprocess.run(
                ["bash", "-c", HARNESS, "bash", str(SCRIPT), *arguments],
                cwd=ROOT, env=environment, capture_output=True, check=False, timeout=30,
            )
            rows = [
                tuple(field.decode().replace(directory, "<private-root>")
                      for field in line.split(b"\0")[:-1])
                for line in result.stdout.splitlines()
            ]
            return result, rows

    def successful(self, *arguments: str, **overrides: str):
        result, rows = self.invoke(*arguments, **overrides)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        return rows

    def roster(self):
        rows = self.successful("show_cpu_roster")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][0], "roster")
        roster = rows[0][1:]
        self.assertTrue(roster)
        self.assertEqual(len(roster), len(set(roster)))
        return roster

    def package_command(self, row):
        self.assertEqual(row[1:6], ("env", "FE2O3_HIP_SYS_DISABLE=1", "cargo", "test", "--locked"))
        arguments = row[6:]
        self.assertTrue(arguments, "empty -p set would run unrelated workspace targets")
        self.assertEqual(len(arguments) % 2, 0)
        self.assertTrue(all(flag == "-p" for flag in arguments[::2]))
        return arguments[1::2]

    def semantics(self, rows):
        result = []
        for row in rows:
            if row[0].endswith("-workspace-dependencies") or row[0].startswith("driver-"):
                continue
            if row[0] == "cpu-tests" or row[0] in {f"cpu-{group}-tests" for group in CPU_GROUPS}:
                result.extend(("default-package", package) for package in self.package_command(row))
            else:
                result.append(row)
        return Counter(result)

    def test_hosted_union_preserves_every_legacy_package_and_nonpackage_command(self):
        legacy = self.successful("run_cpu_tests")
        self.assertEqual(legacy, self.successful("run_cpu_tests", "all"))
        self.assertEqual(legacy, self.successful("main", "generic-core", "cpu"))
        hosted = []
        for group in CPU_GROUPS:
            hosted.extend(self.successful("main", "generic-core", f"cpu-{group}"))
        self.assertEqual(self.semantics(hosted), self.semantics(legacy))
        packages = [
            row[1] for row, count in self.semantics(hosted).items()
            if row[0] == "default-package" for _ in range(count)
        ]
        self.assertCountEqual(packages, (*self.roster(), *RAW))
        self.assertEqual(len(packages), len(set(packages)))
        names = Counter(row[0] for row in hosted)
        for name in [
            "cargo-fe2o3-tests", "cargo-fe2o3-worker-v3-envelope-tests",
            "fe2o3-pliron-default-api-ui", "fe2o3-artifact-transaction-tests",
            "fe2o3-runtime-release-tests", "wrapper-managed-cpu-tests",
            "tiled-gemm-capability-ui", "cpu-reference-tiled-gemm-paired-default",
            "cpu-reference-tiled-gemm-paired-simt", "cpu-test-partition-revalidation",
            "cpu-test-binding-projection-revalidation", "dialect-mir-pliron-tests",
        ]:
            self.assertEqual(names[name], 1, name)
        self.assertEqual(sum(name.startswith("host-reference-") for name in names), 35)

    def test_package_groups_keep_exact_unfiltered_rosters_and_cold_prerequisites(self):
        roster = self.roster()
        heavy = {package for packages in HEAVY.values() for package in packages}
        expected = {"foundation": tuple(package for package in roster if package not in heavy), **HEAVY}
        for group in PACKAGE_GROUPS:
            with self.subTest(group=group):
                rows = self.successful("run_cpu_package_group", group)
                self.assertEqual(rows[0], (
                    f"cpu-{group}-workspace-dependencies",
                    "cargo", "fetch", "--locked", "--manifest-path", str(ROOT / "Cargo.toml"),
                ))
                self.assertEqual(rows[1], ("driver-bootstrap", f"cpu-{group}"))
                self.assertEqual(rows[-1][0], f"cpu-{group}-tests")
                self.assertEqual(self.package_command(rows[-1]), expected[group])
                self.assertEqual(len(rows), 4 if group == "pliron" else 3)
                if group == "pliron":
                    self.assertEqual(rows[2], (
                        "fe2o3-pliron-default-api-ui", "cargo", "test", "--locked",
                        "-p", "fe2o3-pliron", "--no-default-features",
                        "--test", "middle_end_evidence_ui", "default_api_cannot_self_authorize",
                        "--", "--exact",
                    ))

    def test_every_lane_stops_at_each_failed_stage_including_prefetch_and_driver(self):
        for group in CPU_GROUPS:
            arguments = ("main", "generic-core", f"cpu-{group}")
            baseline = self.successful(*arguments)
            for stage in dict.fromkeys(row[0] for row in baseline):
                with self.subTest(group=group, stage=stage):
                    result, rows = self.invoke(*arguments, failure=stage)
                    self.assertEqual(result.returncode, 29, result.stderr.decode())
                    stop = next(index for index, row in enumerate(baseline) if row[0] == stage)
                    self.assertEqual(rows, baseline[:stop + 1])

    def test_invalid_groups_modes_and_arity_refuse_before_any_prerequisite(self):
        for arguments in [
            ("run_cpu_package_group",), ("run_cpu_package_group", ""),
            ("run_cpu_package_group", "unknown"), ("run_cpu_package_group", "analysis", "extra"),
            ("run_cpu_tests", ""), ("run_cpu_tests", "unknown"),
            ("run_cpu_tests", "integration", "extra"),
            ("main", "generic-core", "cpu-unknown"),
            ("main", "generic-core", "cpu-finalize", "extra"),
        ]:
            with self.subTest(arguments=arguments):
                result, rows = self.invoke(*arguments)
                self.assertEqual(result.returncode, 2, result.stderr.decode())
                self.assertEqual(rows, [])

    def test_empty_raw_examples_do_not_fall_back_to_workspace_tests(self):
        rows = self.successful("run_cpu_tests", "integration", EMPTY_RAW="1")
        self.assertNotIn("cpu-integration-tests", [row[0] for row in rows])
        self.assertEqual(sum(row[0] == "wrapper-managed-cpu-tests" for row in rows), 1)
        partition = next(row for row in rows if row[0] == "cpu-test-partition-revalidation")
        self.assertEqual(partition[-3:], ("--", *MANAGED))
        all_rows = self.successful("run_cpu_tests", EMPTY_RAW="1")
        raw = next(row for row in all_rows if row[0] == "cpu-tests")
        self.assertEqual(self.package_command(raw), self.roster())

    def test_duplicate_or_overlapping_example_partition_is_still_refused(self):
        for fault in [{"DUPLICATE_RAW": "1"}, {"OVERLAP": "1"}]:
            for mode in [(), ("integration",)]:
                with self.subTest(fault=fault, mode=mode):
                    result, rows = self.invoke("run_cpu_tests", *mode, **fault)
                    self.assertEqual(result.returncode, 2, result.stderr.decode())
                    self.assertNotIn("cargo-fe2o3-tests", [row[0] for row in rows])

    def test_hosted_matrix_limits_environment_and_strict_aggregate_are_preserved(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        core = workflow.split("\n  generic-core:\n", 1)[1].split("\n  rustc-codegen-shards:\n", 1)[0]
        matrix = core.split("        group:\n", 1)[1].split("    env:\n", 1)[0]
        self.assertEqual(re.findall(r"^          - (.+)$", matrix, re.MULTILINE), [
            "policy", *(f"cpu-{group}" for group in CPU_GROUPS), "auxiliary",
        ])
        self.assertIn("    timeout-minutes: 90\n", core)
        self.assertIn("      fail-fast: false\n", core)
        self.assertNotIn("continue-on-error", core)
        self.assertIn('scripts/ci-local.sh generic-core "${{ matrix.group }}"', core)
        for line in [
            "      CARGO_TARGET_DIR: ${{ github.workspace }}/target/ci/generic-core-${{ matrix.group }}",
            "      CI_LOG_DIR: ${{ github.workspace }}/target/ci-logs/generic-core-${{ matrix.group }}",
            "      CARGO_PROFILE_DEV_DEBUG: '1'", "      CARGO_PROFILE_TEST_DEBUG: '1'",
            "      CARGO_INCREMENTAL: '0'",
        ]:
            self.assertIn(line, core)
        self.assertIn("resource_parent", core)
        self.assertIn("generic-core-${{ matrix.group }}-logs-${{ github.run_attempt }}", core)
        aggregate = workflow.split("\n  generic-validation:\n", 1)[1]
        self.assertIn("    if: ${{ always() }}", aggregate)
        self.assertIn("      - generic-core\n      - rustc-codegen-shards", aggregate)
        self.assertIn("scripts/require-ci-success.sh", aggregate)
        self.assertIn('"${GENERIC_CORE_RESULT}"', aggregate)
        self.assertIn('"${RUSTC_CODEGEN_SHARDS_RESULT}"', aggregate)
        script = SCRIPT.read_text()
        self.assertIn('CI_STEP_TIMEOUT_SECONDS="${FE2O3_CI_STEP_TIMEOUT_SECONDS:-3000}"', script)
        self.assertIn('CI_STEP_KILL_AFTER_SECONDS="${FE2O3_CI_STEP_KILL_AFTER_SECONDS:-15}"', script)

    def test_strict_aggregate_rejects_failure_cancellation_skips_and_missing_results(self):
        checker = ROOT / "scripts/require-ci-success.sh"
        for statuses in [
            ("success", "success"), ("success", "failure"), ("cancelled", "success"),
            ("success", "skipped"), ("", "success"), ("success", ""),
        ]:
            with self.subTest(statuses=statuses):
                result = subprocess.run(
                    ["bash", str(checker), *statuses], cwd=ROOT,
                    capture_output=True, check=False, timeout=10,
                )
                self.assertEqual(result.returncode == 0, statuses == ("success", "success"))


if __name__ == "__main__":
    unittest.main()
