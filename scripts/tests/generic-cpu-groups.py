#!/usr/bin/env python3
"""Compare real CI dispatch commands; tool execution is replaced, not selection."""

from __future__ import annotations

from collections import Counter
import json
import os
import stat
import sys
import re
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ci-local.sh"
PACKAGE_GROUPS = ("foundation", "analysis", "lowering", "pliron", "finalize")
CPU_GROUPS = (*PACKAGE_GROUPS, "integration")
HOSTED_CPU_GROUPS = ("foundation", "analysis", "lowering", "pliron", "finalize-0", "finalize-1",
                     "finalize-2", "finalize-3", "integration")
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
repeat_package_group() {
  run_cpu_package_group "$1"
  run_cpu_package_group "$1"
}
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
        if row[0] == "cpu-foundation-tests":
            self.assertEqual(arguments[-2:], ("--", "--nocapture"))
            arguments = arguments[:-2]
        self.assertTrue(arguments, "empty -p set would run unrelated workspace targets")
        self.assertEqual(len(arguments) % 2, 0)
        self.assertTrue(all(flag == "-p" for flag in arguments[::2]))
        return arguments[1::2]

    def semantics(self, rows):
        result = []
        finalizer_shards = []
        for row in rows:
            if (row[0].endswith("-workspace-dependencies") or row[0].startswith("driver-")
                    or row[0] == "standalone-lockfiles"):
                continue
            if re.fullmatch(r"cpu-finalize-[0-3]-tests", row[0]):
                finalizer_shards.append(int(row[0].split("-")[2]))
            elif row[0] == "cpu-tests" or row[0] in {f"cpu-{group}-tests" for group in CPU_GROUPS}:
                result.extend(("default-package", package) for package in self.package_command(row))
            else:
                result.append(row)
        if finalizer_shards:
            self.assertCountEqual(finalizer_shards, [0, 1, 2, 3])
            result.append(("default-package", "fe2o3-hsaco-finalize"))
        return Counter(result)

    def test_hosted_union_preserves_every_legacy_package_and_nonpackage_command(self):
        legacy = self.successful("run_cpu_tests")
        self.assertEqual(legacy, self.successful("run_cpu_tests", "all"))
        self.assertEqual(legacy, self.successful("main", "generic-core", "cpu"))
        hosted = []
        for group in HOSTED_CPU_GROUPS:
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
            "fe2o3-runtime-release-tests", "fe2o3-device-release-tests", "wrapper-managed-cpu-tests",
            "native-data-copy-application-binding-check",
            "tiled-gemm-capability-ui", "cpu-reference-tiled-gemm-paired-default",
            "cpu-reference-tiled-gemm-paired-simt", "cpu-test-partition-revalidation",
            "cpu-test-binding-projection-revalidation", "dialect-mir-pliron-tests",
        ]:
            self.assertEqual(names[name], 1, name)
        self.assertEqual(sum(name.startswith("host-reference-") for name in names), 35)


    def test_native_data_copy_binding_check_is_explicit_compile_only_and_driver_validated(self):
        expected = (
            "native-data-copy-application-binding-check", "env",
            "-u", "LD_PRELOAD", "-u", "LD_LIBRARY_PATH", "FE2O3_HIP_SYS_DISABLE=1",
            "/sealed-production-driver", "check", "--locked", "--bins",
            "--features", "data-copy-observation", "--manifest-path",
            "crates/cargo-fe2o3/tests/fixtures/conditional-custodian-application/Cargo.toml",
        )
        for arguments in [("run_cpu_tests",), ("run_cpu_tests", "integration"),
                          ("main", "generic-core", "cpu-integration")]:
            with self.subTest(arguments=arguments):
                rows = self.successful(*arguments)
                actual = [row for row in rows if row[0] == expected[0]]
                self.assertEqual(actual, [expected])
                position = rows.index(expected)
                self.assertEqual(rows[position - 1], ("driver-validate",))
                self.assertEqual(rows[position + 1][0], "tiled-gemm-capability-ui")

    def test_native_timestamp_cpu_lane_preserves_closed_roster_and_order(self):
        expected = [
            ("native-timestamp-cpu-discovery", "cargo", "test", "--locked", "-p", "fe2o3-kfd",
             "--features", "engineering-native-packet-diagnostics", "--lib", "native_timestamp_",
             "--", "--list"),
            ("native-timestamp-cpu-discovery-check", "python3", "-I", "-B",
             "scripts/check-native-timestamp-cpu.py", "list",
             "<private-root>/logs/native-timestamp-cpu-discovery.log"),
            ("native-timestamp-cpu-tests", "cargo", "test", "--locked", "-p", "fe2o3-kfd",
             "--features", "engineering-native-packet-diagnostics", "--lib", "native_timestamp_",
             "--", "--test-threads=1"),
            ("native-timestamp-cpu-result-check", "python3", "-I", "-B",
             "scripts/check-native-timestamp-cpu.py", "run",
             "<private-root>/logs/native-timestamp-cpu-tests.log"),
        ]
        for arguments in [("run_cpu_tests",), ("run_cpu_tests", "integration"),
                          ("main", "generic-core", "cpu-integration")]:
            rows = self.successful(*arguments)
            actual = [row for row in rows if row[0].startswith("native-timestamp-cpu-")]
            self.assertEqual(actual, expected)
            for row in actual:
                self.assertNotIn("--ignored", row)
                self.assertNotIn("--include-ignored", row)
        self.assertEqual(self.successful("run_native_timestamp_cpu_tests"), expected)


    def test_device_release_tests_disable_assertions_and_run_the_complete_library(self):
        for arguments in [("run_cpu_tests",), ("run_cpu_tests", "integration"),
                          ("main", "generic-core", "cpu-integration")]:
            with self.subTest(arguments=arguments):
                rows = self.successful(*arguments)
                actual = [row for row in rows if row[0] == "fe2o3-device-release-tests"]
                self.assertEqual(actual, [(
                    "fe2o3-device-release-tests", "env",
                    "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=false", "cargo", "test",
                    "--locked", "--release", "-p", "fe2o3-device", "--lib",
                )])

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
                self.assertEqual(rows[1], (
                    "standalone-lockfiles", "bash", str(ROOT / "scripts/check-standalone-lockfiles.sh"),
                ))
                self.assertEqual(rows[2], ("driver-bootstrap", f"cpu-{group}", "create-private"))
                self.assertEqual(rows[-1][0], f"cpu-{group}-tests")
                self.assertEqual(self.package_command(rows[-1]), expected[group])
                self.assertEqual(len(rows), 5 if group == "pliron" else 4)
                if group == "pliron":
                    self.assertEqual(rows[3], (
                        "fe2o3-pliron-default-api-ui", "cargo", "test", "--locked",
                        "-p", "fe2o3-pliron", "--no-default-features",
                        "--test", "middle_end_evidence_ui", "default_api_cannot_self_authorize",
                        "--", "--exact",
                    ))

    def test_standalone_preflight_is_reused_only_after_successful_completion(self):
        for group in PACKAGE_GROUPS:
            with self.subTest(group=group):
                baseline = self.successful("run_cpu_package_group", group)
                rows = self.successful("repeat_package_group", group)
                self.assertEqual(rows, baseline + [
                    row for row in baseline if row[0] != "standalone-lockfiles"
                ])
                result, failed = self.invoke("repeat_package_group", group,
                                             failure="standalone-lockfiles")
                self.assertEqual(result.returncode, 29, result.stderr.decode())
                self.assertEqual(failed, baseline[:2])

    def test_integration_and_legacy_cpu_require_standalone_preflight_before_consumers(self):
        for arguments in [("run_cpu_tests",), ("run_cpu_tests", "integration"),
                          ("main", "generic-core", "cpu-integration")]:
            with self.subTest(arguments=arguments):
                rows = self.successful(*arguments)
                self.assertEqual(rows[0][0], "cpu-workspace-dependencies")
                self.assertEqual(rows[1], (
                    "standalone-lockfiles", "bash", str(ROOT / "scripts/check-standalone-lockfiles.sh"),
                ))
                self.assertTrue(rows[2][0].startswith("host-reference-"))
                self.assertEqual(sum(row[0] == "standalone-lockfiles" for row in rows), 1)
                result, failed = self.invoke(*arguments, failure="standalone-lockfiles")
                self.assertEqual(result.returncode, 29, result.stderr.decode())
                self.assertEqual(failed, rows[:2])

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
            ("run_cpu_finalizer_shard",), ("run_cpu_finalizer_shard", "4"),
            ("run_cpu_finalizer_shard", "-1"), ("run_cpu_finalizer_shard", "00"),
            ("run_cpu_finalizer_shard", "0", "extra"),
            ("main", "generic-core", "cpu-finalize-4"),
        ]:
            with self.subTest(arguments=arguments):
                result, rows = self.invoke(*arguments)
                self.assertEqual(result.returncode, 2, result.stderr.decode())
                self.assertEqual(rows, [])

    def test_finalizer_shards_keep_cold_prerequisites_and_one_original_outer_bound(self):
        for shard in range(4):
            group = f"cpu-finalize-{shard}"
            rows = self.successful("main", "generic-core", group)
            self.assertEqual([row[0] for row in rows], [f"{group}-workspace-dependencies",
                "standalone-lockfiles", "driver-bootstrap", f"{group}-tests"])
            self.assertEqual(rows[2], ("driver-bootstrap", group, "create-private"))
            self.assertEqual(rows[-1], (f"{group}-tests", "env", "FE2O3_HIP_SYS_DISABLE=1", "python3",
                "-I", "-B", str(ROOT / "scripts/finalizer-test-shards.py"), "--shard", str(shard),
                "--output", f"<private-root>/logs/finalizer-shard-{shard}"))
            for row in rows:
                result, stopped = self.invoke("main", "generic-core", group, failure=row[0])
                self.assertEqual(result.returncode, 29)
                self.assertEqual(stopped, rows[:rows.index(row) + 1])

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
            "policy", *(f"cpu-{group}" for group in HOSTED_CPU_GROUPS), "auxiliary",
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
        self.assertIn("      - generic-core\n      - rustc-codegen-shards\n      - native-static-cpu\n", aggregate)
        self.assertIn("scripts/require-ci-success.sh", aggregate)
        self.assertIn('"${GENERIC_CORE_RESULT}"', aggregate)
        self.assertIn('"${RUSTC_CODEGEN_SHARDS_RESULT}"', aggregate)
        self.assertIn("NATIVE_STATIC_CPU_RESULT: ${{ needs.native-static-cpu.result }}", aggregate)
        self.assertIn('"${NATIVE_STATIC_CPU_RESULT}"', aggregate)
        self.assertIn("--aggregate target/finalizer-shard-receipts", aggregate)
        self.assertIn('--expected-commit "${{ github.sha }}"', aggregate)
        self.assertIn("pattern: finalizer-shard-*-receipt", aggregate)
        self.assertIn("name: Upload complete finalizer shard receipts", core)
        self.assertIn("if-no-files-found: error", core)
        receipt_upload = core.split("      - name: Upload complete finalizer shard receipts\n", 1)[1].split("      - name:", 1)[0]
        self.assertIn("name: finalizer-shard-${{ matrix.group }}-receipt\n", receipt_upload)
        self.assertIn("overwrite: true", receipt_upload)
        self.assertIn("success() && startsWith(matrix.group, 'cpu-finalize-')", receipt_upload)
        self.assertNotIn("github.run_attempt", receipt_upload)
        receipt_download = aggregate.split("      - name: Download all four finalizer shard receipts\n", 1)[1].split("      - name:", 1)[0]
        for override in ("github.run_attempt", "run-id:", "repository:", "github-token:"):
            self.assertNotIn(override, receipt_download)
        self.assertLess(aggregate.index("scripts/require-ci-success.sh"),
                        aggregate.index("Download all four finalizer shard receipts"))
        script = SCRIPT.read_text()
        self.assertIn('CI_STEP_TIMEOUT_SECONDS="${FE2O3_CI_STEP_TIMEOUT_SECONDS:-3000}"', script)
        self.assertIn('CI_STEP_KILL_AFTER_SECONDS="${FE2O3_CI_STEP_KILL_AFTER_SECONDS:-15}"', script)

    def test_strict_aggregate_rejects_failure_cancellation_skips_and_missing_results(self):
        checker = ROOT / "scripts/require-ci-success.sh"
        success = ("success", "success", "success")
        cases = [success]
        for index in range(3):
            for rejected in ("failure", "cancelled", "skipped", ""):
                cases.append(success[:index] + (rejected,) + success[index + 1:])
        for statuses in cases:
            with self.subTest(statuses=statuses):
                result = subprocess.run(
                    ["bash", str(checker), *statuses], cwd=ROOT,
                    capture_output=True, check=False, timeout=10,
                )
                self.assertEqual(result.returncode == 0, statuses == success)



class NativeTimestampLogTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import importlib.util
        path = ROOT / "scripts/check-native-timestamp-cpu.py"
        spec = importlib.util.spec_from_file_location("native_timestamp_cpu", path)
        cls.checker = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.checker)

    def listing(self):
        return "\n".join(name + ": test" for name in self.checker.NAMES) + "\n\n3 tests, 0 benchmarks\n"

    def outcome(self):
        return ("running 3 tests\n" + "\n".join("test " + name + " ... ok" for name in self.checker.NAMES)
                + "\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 2100 filtered out; finished in 0.01s\n")

    def test_actual_three_case_shapes_pass(self):
        self.checker.validate("list", self.listing())
        self.checker.validate("run", self.outcome())

    def test_discovery_refuses_zero_missing_duplicate_foreign_benchmark_and_summary_changes(self):
        raw = self.listing()
        first, second, _ = self.checker.NAMES
        for bad in [
            "0 tests, 0 benchmarks\n", raw.replace(first + ": test\n", ""),
            raw.replace(second, first), raw.replace(first, "foreign"),
            raw.replace(first + ": test", first + ": benchmark"),
            raw.replace("3 tests", "4 tests"), raw + "3 tests, 0 benchmarks\n",
            raw + "foreign: test\n", raw.rsplit("\n\n", 1)[0],
        ]:
            with self.subTest(raw=bad), self.assertRaises(ValueError):
                self.checker.validate("list", bad)

    def test_outcome_refuses_missing_duplicate_foreign_ignored_failed_and_extra_cases(self):
        raw = self.outcome()
        first, second, _ = self.checker.NAMES
        for bad in [
            raw.replace("test " + first + " ... ok\n", ""),
            raw.replace(second, first), raw.replace(first, "foreign"),
            raw.replace(first + " ... ok", first + " ... ignored"),
            raw.replace(first + " ... ok", first + " ... FAILED"),
            raw + "test foreign ... ok\n", raw.replace("running 3", "running 1"),
            raw.replace("3 passed", "0 passed"), raw.replace("0 ignored", "1 ignored"),
            raw + raw, raw.split("test result:")[0], "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        ]:
            with self.subTest(raw=bad), self.assertRaises(ValueError):
                self.checker.validate("run", bad)


def standalone_metadata_commands():
    inventory = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "--", "*/Cargo.lock"],
        capture_output=True, check=True, timeout=10,
    ).stdout
    locks = sorted(Path(path.decode()) for path in inventory.split(b"\0") if path)
    return [
        ["metadata", "--locked", "--format-version", "1", "--manifest-path",
         str(ROOT / lock.with_name("Cargo.toml"))]
        for lock in locks
    ]


COLD_HARNESS = r'''
set -Eeuo pipefail
source "$1"
shift
run_step() {
  printf 'stage:%s\n' "$1"
  shift
  "$@"
}
stat() {
  if [[ "${@: -1}" == "${CARGO_TARGET_DIR}" ]]; then
    if [[ "$2" == %a && "${CUSTODY_FAULT:-}" == mode ]] ||
      [[ "$2" == %u && "${CUSTODY_FAULT:-}" == owner ]]; then
      return 41
    fi
    if [[ "$2" == %u && "${FAKE_FOREIGN_OWNER:-0}" == 1 ]]; then
      printf '%s\n' "$(( $(id -u) + 1 ))"
      return
    fi
  fi
  command stat "$@"
}
id() {
  if [[ "${CUSTODY_FAULT:-}" == uid ]]; then return 41; fi
  command id "$@"
}
if [[ -n "${REALPATH_STATUS:-}" ]]; then
  realpath() {
    if [[ "${@: -1}" == "${CARGO_TARGET_DIR}" ]]; then
      return "${REALPATH_STATUS}"
    fi
    command realpath "$@"
  }
fi
if [[ "${COLD_ENTRY:-group}" == resolve ]]; then
  resolved="$(resolve_cargo_target_directory)"
  printf '%s\n' "${resolved}"
else
  run_cpu_package_group "$1"
  validate_cargo_fe2o3_driver
  printf 'sealed:%s\n' "${CARGO_FE2O3_BINARY}"
  printf 'sealed-mode:%s:%s\n' \
    "$(stat -c '%a' -- "${CARGO_FE2O3_DRIVER_ROOT}")" \
    "$(stat -c '%a' -- "${CARGO_FE2O3_BINARY}")"
  if [[ "${TAMPER_SEAL:-0}" == 1 ]]; then
    chmod 700 -- "${CARGO_FE2O3_BINARY}"
    printf '# changed\n' >>"${CARGO_FE2O3_BINARY}"
    chmod 500 -- "${CARGO_FE2O3_BINARY}"
    validate_cargo_fe2o3_driver
  fi
fi
'''
COLD_CARGO = r'''#!__PYTHON__
import json
import os
import sys
from pathlib import Path

arguments = sys.argv[1:]
calls = Path(os.environ["COLD_CALLS"])
prior = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
with calls.open("a") as output:
    print(json.dumps(arguments), file=output)
command = arguments[0]
if command == "fetch":
    sys.exit(int(os.environ.get("FETCH_STATUS", "0")))
if command == "metadata":
    if "--manifest-path" in arguments:
        manifest = arguments[arguments.index("--manifest-path") + 1]
        if manifest == os.environ.get("STANDALONE_FAIL_MANIFEST"):
            sys.exit(47)
        print("{}")
        sys.exit(0)
    if os.environ.get("INVALID_METADATA"):
        print("not JSON")
        sys.exit(0)
    target = os.environ["CARGO_TARGET_DIR"]
    if os.environ.get("METADATA_TARGET"):
        target = os.environ["METADATA_TARGET"]
    if os.environ.get("METADATA_DRIFT") and any(
        row[0] == "metadata" and "--manifest-path" not in row for row in prior
    ):
        target += "/substituted"
    root = Path(os.environ["COLD_REPO"])
    print(json.dumps({
        "target_directory": target,
        "packages": [{
            "id": "cargo-fe2o3#fixture",
            "name": "cargo-fe2o3",
            "manifest_path": str(root / "crates/cargo-fe2o3/Cargo.toml"),
            "targets": [{
                "name": "cargo-fe2o3", "kind": ["bin"],
                "src_path": str(root / "crates/cargo-fe2o3/src/main.rs"),
            }],
        }],
    }))
    sys.exit(0)
if command == "build":
    target = Path(os.environ["CARGO_TARGET_DIR"])
    if not target.is_dir() or target.is_symlink():
        sys.exit(81)
    status = int(os.environ.get("BUILD_STATUS", "0"))
    if status:
        sys.exit(status)
    binary = target / "debug/cargo-fe2o3"
    binary.parent.mkdir(mode=0o700)
    binary.write_text("#!/usr/bin/env bash\nexit 0\n")
    binary.chmod(0o700)
    print(json.dumps({
        "reason": "compiler-artifact",
        "package_id": "hostile#fixture" if os.environ.get("WRONG_RECEIPT") else "cargo-fe2o3#fixture",
        "target": {
            "name": "cargo-fe2o3", "kind": ["bin"], "crate_types": ["bin"],
            "src_path": str(Path(os.environ["COLD_REPO"]) / "crates/cargo-fe2o3/src/main.rs"),
        },
        "profile": {"test": False, "opt_level": "0"},
        "executable": str(binary),
    }))
    sys.exit(0)
if command == "test":
    sys.exit(0)
sys.exit(82)
'''


class ColdTargetBootstrapTests(unittest.TestCase):
    """Run the real directory, receipt and seal path with a fixture Cargo tool."""

    def invoke(self, group="finalize", *, shape="cold", entry="group", **overrides):
        with tempfile.TemporaryDirectory(prefix="fe2o3-cpu-cold-") as directory:
            root = Path(directory).resolve()
            target = root / "nested/target"
            sentinel = root / "retained"
            sentinel.write_bytes(b"unrelated retained file\n")
            real = root / "real"
            if shape in ("private", "public", "foreign"):
                target.mkdir(parents=True, mode=0o700)
                target.chmod(0o755 if shape == "public" else 0o700)
            elif shape == "file":
                target.parent.mkdir(mode=0o700)
                target.write_bytes(b"not a directory\n")
            elif shape in ("symlink", "dangling"):
                target.parent.mkdir(mode=0o700)
                if shape == "symlink":
                    real.mkdir(mode=0o700)
                target.symlink_to(real, target_is_directory=True)
            elif shape == "parent-symlink":
                real.mkdir(mode=0o700)
                target.parent.symlink_to(real, target_is_directory=True)
            binary_dir = root / "bin"
            binary_dir.mkdir(mode=0o700)
            cargo = binary_dir / "cargo"
            cargo.write_text(COLD_CARGO.replace("__PYTHON__", sys.executable))
            cargo.chmod(0o700)
            calls = root / "calls.jsonl"
            temporary = root / "private-tmp"
            temporary.mkdir(mode=0o700)
            environment = {
                "PATH": f"{binary_dir}:{os.environ['PATH']}",
                "HOME": os.environ["HOME"],
                "CARGO_TARGET_DIR": str(target),
                "CI_LOG_DIR": str(root / "separate-logs"),
                "TMPDIR": str(temporary),
                "COLD_REPO": str(ROOT),
                "COLD_CALLS": str(calls),
                "COLD_ENTRY": entry,
                **overrides,
            }
            if shape == "foreign":
                environment["FAKE_FOREIGN_OWNER"] = "1"
            result = subprocess.run(
                ["bash", "-c", COLD_HARNESS, "bash", str(SCRIPT), group],
                cwd=ROOT, env=environment, capture_output=True, text=True,
                check=False, timeout=30,
            )
            observed = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            standalone = [
                row for row in observed if row[0] == "metadata" and "--manifest-path" in row
            ]
            expected_standalone = []
            if entry == "group" and int(overrides.get("FETCH_STATUS", "0")) == 0:
                expected_standalone = standalone_metadata_commands()
                failed_manifest = overrides.get("STANDALONE_FAIL_MANIFEST")
                if failed_manifest:
                    stop = next(index for index, row in enumerate(expected_standalone)
                                if row[-1] == failed_manifest)
                    expected_standalone = expected_standalone[:stop + 1]
            self.assertEqual(standalone, expected_standalone)
            if standalone:
                self.assertEqual(observed[0][0], "fetch")
                self.assertEqual(observed[1:1 + len(standalone)], standalone)
            # Check the full prerequisite above, then retain the existing custody trace.
            observed = [
                row for row in observed if not (row[0] == "metadata" and "--manifest-path" in row)
            ]
            target_mode = stat.S_IMODE(target.lstat().st_mode) if target.exists() or target.is_symlink() else None
            sealed_modes = [
                (int(parent, 8), int(binary, 8))
                for parent, binary in re.findall(r"^sealed-mode:([0-7]+):([0-7]+)$",
                                                result.stdout, re.MULTILINE)
            ]
            self.assertEqual(list(temporary.glob("fe2o3-ci-driver-*")), [])
            self.assertEqual(sentinel.read_bytes(), b"unrelated retained file\n")
            if shape in ("symlink", "dangling"):
                self.assertTrue(target.is_symlink())
                self.assertEqual(target.readlink(), real)
                self.assertFalse((real / "debug").exists())
            if shape == "parent-symlink":
                self.assertTrue(target.parent.is_symlink())
                self.assertFalse((real / "target").exists())
            if shape == "file":
                self.assertEqual(target.read_bytes(), b"not a directory\n")
            return result, observed, target_mode, sealed_modes

    def test_all_package_groups_bootstrap_from_a_real_missing_target(self):
        for group in PACKAGE_GROUPS:
            with self.subTest(group=group):
                result, calls, mode, sealed = self.invoke(group)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(mode, 0o700)
                self.assertEqual(sealed, [(0o500, 0o500)])
                commands = [row[0] for row in calls]
                self.assertEqual(commands[:4], ["fetch", "metadata", "metadata", "build"])
                self.assertTrue(all(command == "test" for command in commands[4:]))
                self.assertEqual(commands.count("test"), 2 if group == "pliron" else 1)
                build = next(row for row in calls if row[0] == "build")
                self.assertEqual(build, [
                    "build", "--locked", "-p", "cargo-fe2o3", "--bin", "cargo-fe2o3",
                    "--message-format=json-render-diagnostics",
                ])
                self.assertIn("sealed:", result.stdout)

    def test_real_standalone_preflight_covers_declared_macro_and_device_fixture_locks(self):
        commands = standalone_metadata_commands()
        self.assertTrue(commands)
        manifests = [Path(row[-1]) for row in commands]
        self.assertEqual(len(manifests), len(set(manifests)))
        required = [
            "crates/fe2o3-device/tests/fixtures/complete-body-packing-consumer/Cargo.toml",
            "crates/fe2o3-macros/tests/fixtures/generic-worker-v3-adapter/Cargo.toml",
            "crates/fe2o3-macros/tests/fixtures/renamed-device/Cargo.toml",
            "crates/fe2o3-macros/tests/fixtures/renamed-typed-host/Cargo.toml",
            "crates/fe2o3-macros/tests/fixtures/typed-invalid/Cargo.toml",
            "examples/tiled_gemm_general_v1/Cargo.toml",
        ]
        for relative in required:
            self.assertIn(ROOT / relative, manifests)
        tiled = ROOT / "examples/tiled_gemm_general_v1/Cargo.toml"
        owner = tomllib.loads(tiled.read_text())
        self.assertIn("device-api", owner["workspace"]["members"])
        self.assertTrue(tiled.with_name("device-api").joinpath("Cargo.toml").is_file())
        snapshot = {
            path: path.read_bytes()
            for manifest in manifests for path in (manifest, manifest.with_name("Cargo.lock"))
        }
        result, calls, mode, sealed = self.invoke("foundation")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("stage:standalone-lockfiles", result.stdout)
        self.assertIn("build", [row[0] for row in calls])
        self.assertEqual((mode, sealed), (0o700, [(0o500, 0o500)]))
        self.assertEqual({path: path.read_bytes() for path in snapshot}, snapshot)

    def test_real_standalone_failure_stops_every_group_before_target_driver_or_tests(self):
        commands = standalone_metadata_commands()
        failed_manifests = (
            commands[0][-1],
            str(ROOT / "crates/fe2o3-macros/tests/fixtures/generic-worker-v3-adapter/Cargo.toml"),
            commands[-1][-1],
        )
        for group in PACKAGE_GROUPS:
            for manifest in failed_manifests:
                with self.subTest(group=group, manifest=manifest):
                    result, calls, mode, sealed = self.invoke(
                        group, STANDALONE_FAIL_MANIFEST=manifest)
                    self.assertEqual(result.returncode, 47, result.stderr)
                    self.assertEqual([row[0] for row in calls], ["fetch"])
                    self.assertEqual((mode, sealed), (None, []))
                    self.assertNotIn(f"stage:cpu-{group}-cargo-fe2o3-bootstrap", result.stdout)
                    self.assertNotIn(f"stage:cpu-{group}-tests", result.stdout)

    def test_existing_private_target_and_legacy_existing_only_policy(self):
        result, _, mode, sealed = self.invoke(shape="private")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((mode, sealed), (0o700, [(0o500, 0o500)]))
        result, calls, mode, sealed = self.invoke(entry="resolve")
        self.assertEqual(result.returncode, 2)
        self.assertIn("not a real directory", result.stderr)
        self.assertEqual([row[0] for row in calls], ["metadata"])
        self.assertIsNone(mode)
        self.assertEqual(sealed, [])

    def test_symlinks_non_directories_and_public_targets_are_not_repaired(self):
        for shape in ("symlink", "dangling", "parent-symlink", "file", "public"):
            for entry in ("group", "resolve"):
                with self.subTest(shape=shape, entry=entry):
                    result, calls, mode, sealed = self.invoke(shape=shape, entry=entry)
                    self.assertEqual(result.returncode, 2, result.stderr)
                    expected = ["fetch", "metadata"] if entry == "group" else ["metadata"]
                    self.assertEqual([row[0] for row in calls], expected)
                    self.assertEqual(sealed, [])
                    if entry == "resolve":
                        self.assertEqual(result.stdout, "")
                    if shape == "public":
                        self.assertEqual(mode, 0o755)

    def test_foreign_target_owner_refuses_before_driver_build(self):
        for entry in ("group", "resolve"):
            with self.subTest(entry=entry):
                result, calls, mode, sealed = self.invoke(shape="foreign", entry=entry)
                self.assertEqual(result.returncode, 2)
                self.assertIn("owner-held, and private", result.stderr)
                expected = ["fetch", "metadata"] if entry == "group" else ["metadata"]
                self.assertEqual([row[0] for row in calls], expected)
                self.assertEqual((mode, sealed), (0o700, []))
                if entry == "resolve":
                    self.assertEqual(result.stdout, "")

    def test_invalid_metadata_and_changed_target_cannot_reach_build(self):
        for target in ("relative/target", "/first\n/second"):
            with self.subTest(target=target):
                result, calls, mode, sealed = self.invoke(METADATA_TARGET=target)
                self.assertEqual(result.returncode, 2)
                self.assertIn("invalid target directory", result.stderr)
                self.assertEqual([row[0] for row in calls], ["fetch", "metadata"])
                self.assertEqual((mode, sealed), (None, []))
        result, calls, _, sealed = self.invoke(METADATA_DRIFT="1")
        self.assertEqual(result.returncode, 2)
        self.assertIn("target_directory changed", result.stderr)
        self.assertNotIn("build", [row[0] for row in calls])
        self.assertEqual(sealed, [])

    def test_fetch_build_and_receipt_failures_never_run_package_tests(self):
        for fault, status, expected in (
            ({"FETCH_STATUS": "29"}, 29, ["fetch"]),
            ({"BUILD_STATUS": "31"}, 31, ["fetch", "metadata", "metadata", "build"]),
            ({"WRONG_RECEIPT": "1"}, 1, ["fetch", "metadata", "metadata", "build"]),
        ):
            with self.subTest(fault=fault):
                result, calls, _, sealed = self.invoke(**fault)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertEqual([row[0] for row in calls], expected)
                self.assertEqual(sealed, [])

    def test_metadata_and_realpath_failures_propagate_without_target_or_build(self):
        for fault, status in (({"INVALID_METADATA": "1"}, 1), ({"REALPATH_STATUS": "37"}, 37)):
            for shape, entry in (("cold", "group"), ("private", "group"), ("private", "resolve")):
                with self.subTest(fault=fault, shape=shape, entry=entry):
                    result, calls, mode, sealed = self.invoke(shape=shape, entry=entry, **fault)
                    self.assertEqual(result.returncode, status, result.stderr)
                    expected = ["fetch", "metadata"] if entry == "group" else ["metadata"]
                    self.assertEqual([row[0] for row in calls], expected)
                    self.assertEqual(sealed, [])
                    if shape == "cold":
                        self.assertIsNone(mode)
                    if entry == "resolve":
                        self.assertEqual(result.stdout, "")

    def test_custody_query_failures_propagate_inside_both_resolver_assignments(self):
        for fault in ("mode", "owner", "uid"):
            for entry in ("group", "resolve"):
                with self.subTest(fault=fault, entry=entry):
                    result, calls, mode, sealed = self.invoke(
                        shape="private", entry=entry, CUSTODY_FAULT=fault)
                    self.assertEqual(result.returncode, 41, result.stderr)
                    expected = ["fetch", "metadata"] if entry == "group" else ["metadata"]
                    self.assertEqual([row[0] for row in calls], expected)
                    self.assertEqual((mode, sealed), (0o700, []))
                    if entry == "resolve":
                        self.assertEqual(result.stdout, "")

    def test_real_sealed_driver_validator_still_rejects_changed_bytes(self):
        result, calls, _, sealed = self.invoke(TAMPER_SEAL="1")
        self.assertEqual(result.returncode, 2)
        self.assertIn("test", [row[0] for row in calls])
        self.assertEqual(sealed, [(0o500, 0o500)])
        self.assertIn("sealed driver identity or private custody changed", result.stderr)


if __name__ == "__main__":
    unittest.main()
