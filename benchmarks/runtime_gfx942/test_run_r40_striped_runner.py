#!/usr/bin/env python3

from __future__ import annotations

import pathlib
import re
import shlex
import subprocess
import unittest


RUNNER = pathlib.Path(__file__).with_name("run-r40-striped-mi300x.sh")


class R40StripedRunnerContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = RUNNER.read_text(encoding="utf-8")

    def test_shell_is_syntactically_valid(self) -> None:
        subprocess.run(["/usr/bin/bash", "-n", str(RUNNER)], check=True)

    def test_exact_gpu_and_workload_shape_are_frozen(self) -> None:
        for contract in (
            "readonly gpu_index=2",
            "readonly expected_unique_id=0xd2e26fef80cf5c33",
            "readonly depth=112",
            "readonly warmups=10",
            "readonly samples=30",
            "readonly phase_timeout=180",
        ):
            self.assertIn(contract, self.source)
        self.assertIn("bytes4096-q2-combined", self.source)
        self.assertIn("bytes1048576-q16-standalone", self.source)
        self.assertIn("workload_reverse", self.source)
        self.assertIn("workload_rotate5", self.source)

    def test_backend_and_workload_orders_are_counterbalanced(self) -> None:
        self.assertIn("'kfd hsa hip' 'hsa hip kfd' 'hip kfd hsa'", self.source)
        self.assertIn(
            "cyclic-latin-square-3-backends-workload-forward-reverse-rotate5-v1",
            self.source,
        )
        self.assertIn("for slot in 0 1 2; do", self.source)
        self.assertIn('for workload_id in "${workload_order[@]}"; do', self.source)
        self.assertIn('for backend in "${backend_order[@]}"; do', self.source)

    def test_exactly_ninety_guarded_phases_are_declared(self) -> None:
        def array(name: str) -> list[str]:
            match = re.search(rf"readonly -a {name}=\((.*?)\)", self.source, re.DOTALL)
            self.assertIsNotNone(match)
            assert match is not None
            return shlex.split(match.group(1))

        backends = shlex.split(array("backend_orders")[0])
        self.assertEqual(len(array("backend_orders")), 3)
        self.assertEqual(backends, ["kfd", "hsa", "hip"])
        forward = array("workload_forward")
        self.assertEqual(len(forward), 10)
        self.assertEqual(set(array("workload_reverse")), set(forward))
        self.assertEqual(set(array("workload_rotate5")), set(forward))
        self.assertIn("phase_count=0", self.source)
        self.assertIn("((phase_count += 1))", self.source)
        self.assertIn("((sequence == 30))", self.source)
        self.assertIn("((phase_count == 90))", self.source)
        self.assertEqual(3 * len(forward) * len(backends), 90)

    def test_kfd_invocation_requires_the_future_aggregate_contract(self) -> None:
        self.assertIn('"${profile}" aggregate)', self.source)
        self.assertIn('"${depth}" "${warmups}" "${samples}"', self.source)
        self.assertIn("--example kfd-sdma-copy-benchmark", self.source)

    def test_native_comparators_use_request_count_cursor_continuation(self) -> None:
        common = RUNNER.with_name("striped_copy_benchmark_common.hpp").read_text(
            encoding="utf-8"
        )
        self.assertIn('"fe2o3.async-copy-striped-benchmark.v3"', common)
        self.assertIn('"continuing-round-robin-v1"', common)
        self.assertIn('"cursor-queue-major-v1"', common)
        for source_name in ("striped_copy_hip.cpp", "striped_copy_hsa.cpp"):
            source = RUNNER.with_name(source_name).read_text(encoding="utf-8")
            self.assertEqual(source.count("continuation_cursor("), 2)
            self.assertNotIn("submission_ordinal", source)

    def test_native_comparators_reuse_and_poison_one_host_buffer_per_request(
        self,
    ) -> None:
        for source_name in ("striped_copy_hip.cpp", "striped_copy_hsa.cpp"):
            source = RUNNER.with_name(source_name).read_text(encoding="utf-8")
            self.assertIn("std::vector<std::uint8_t *> host(config.depth)", source)
            self.assertNotIn("upload(config.depth)", source)
            self.assertNotIn("download(config.depth)", source)
            h2d = source.index("run_phase(true, cursor, &h2d)")
            poison = source.index("value ^ 0xffU", h2d)
            d2h = source.index("run_phase(false, cursor, &d2h)", poison)
            validate = source.index("validate_buffers(host", d2h)
            self.assertLess(h2d, poison)
            self.assertLess(poison, d2h)
            self.assertLess(d2h, validate)

    def test_every_phase_is_guarded_and_reaped(self) -> None:
        self.assertIn('"${host_guard}" monitor', self.source)
        self.assertIn("--observer-cpu", self.source)
        self.assertIn("--target-output", self.source)
        self.assertIn("active_monitor_pid=$!", self.source)
        self.assertIn('wait "${active_monitor_pid}"', self.source)
        self.assertIn("require_gpu_load_at_most 0", self.source)
        self.assertIn(
            "R40 target must emit exactly one LF-terminated text row", self.source
        )

    def test_retryable_census_discards_and_relaunches_the_whole_phase(self) -> None:
        guard = RUNNER.with_name("r26-host-guard.py").read_text(encoding="utf-8")
        checker = RUNNER.with_name("check-r40-striped.py").read_text(encoding="utf-8")
        self.assertIn("readonly max_guard_census_retries=8", self.source)
        self.assertIn("if ((monitor_status != 75)); then", self.source)
        self.assertIn(
            "retryable-census schema=fe2o3.r55-kfd-census-retry.v1", self.source
        )
        self.assertIn("RETRYABLE_CENSUS_EXIT = 75", guard)
        self.assertIn(
            'RETRYABLE_CENSUS_SCHEMA = "fe2o3.r55-kfd-census-retry.v1"', guard
        )
        self.assertIn(
            'RETRYABLE_CENSUS_SCHEMA = "fe2o3.r55-kfd-census-retry.v1"', checker
        )
        self.assertIn("reason=observation-gap-exceeded", self.source)
        self.assertIn('"observation-gap-exceeded"', checker)
        self.assertIn("discard-target-process-and-relaunch-phase-v1", self.source)
        self.assertIn("discarded-census slot=%s sequence=%s", self.source)
        self.assertIn("discarded_guard_census_sha256=%s", self.source)
        self.assertIn(
            '[[ ! -e "${target_output}" && ! -s "${monitor_output}" ]]',
            self.source,
        )
        self.assertIn(
            "retryable census must be one bounded LF-terminated record", self.source
        )
        self.assertIn('[[ "$(require_gpu_load_at_most 0)" == 0 ]]', self.source)

    def test_retryable_census_never_reuses_the_rejected_target_process(self) -> None:
        loop = self.source.index("while true; do", self.source.index("run_phase()"))
        launch = self.source.index('"${host_guard}" monitor', loop)
        retry = self.source.index("monitor_status != 75", launch)
        continuation = self.source.index("done", retry)
        self.assertLess(launch, retry)
        self.assertLess(retry, continuation)
        self.assertIn("start_topology=", self.source[loop:launch])
        self.assertIn("start_telemetry=", self.source[loop:launch])

    def test_environment_and_topology_are_fail_closed(self) -> None:
        self.assertIn("/usr/bin/env -i", self.source)
        self.assertIn("HSA_XNACK=0 ROCR_VISIBLE_DEVICES=2", self.source)
        self.assertIn("HSA_XNACK=0 HIP_VISIBLE_DEVICES=2", self.source)
        self.assertIn(
            '/usr/bin/taskset --cpu-list "${measurement_cpu_list}"', self.source
        )
        self.assertIn('--membind="${topology_numa_node}"', self.source)
        self.assertIn('[[ "${observed}" == "${host_topology}" ]]', self.source)

    def test_sources_and_evidence_are_sealed_and_reverified(self) -> None:
        for name in (
            "check-r40-striped.py",
            "check-parity.py",
            "r26-host-guard.py",
            "r26-system-identity.py",
            "striped_copy_hip.cpp",
            "striped_copy_hsa.cpp",
            "striped_copy_benchmark_common.hpp",
            "native_benchmark_args.hpp",
            "r26_hsa_pool_policy.hpp",
            "run-r40-striped-mi300x.sh",
        ):
            self.assertIn(name, self.source)
        self.assertGreaterEqual(self.source.count("verify_staged_inputs"), 3)
        self.assertIn("evidence-sha256.txt", self.source)
        self.assertIn("--sort=name --mtime=@0", self.source)
        self.assertIn('sha256_file "${artifact_archive}"', self.source)
        self.assertIn('"${persist_staging}/slot-0.log"', self.source)
        self.assertIn('"${persist_staging}/slot-2.log"', self.source)

    def test_cleanup_is_scoped_to_owned_paths(self) -> None:
        self.assertIn("fe2o3-r40-striped-qual.XXXXXX", self.source)
        self.assertIn("fe2o3-r40-striped-verify.XXXXXX", self.source)
        self.assertNotIn("sudo", self.source)
        self.assertNotIn("pkill", self.source)
        self.assertNotIn("killall", self.source)
        self.assertNotIn("gpu-reset", self.source)
        self.assertIn('kill -s "${signal_name}" "${active_monitor_pid}"', self.source)
        self.assertIn('/usr/bin/find "${owned_path}" -depth -delete', self.source)
        self.assertIn(
            "if ((publication_cleanup_armed == 1 && publication_complete == 0)); then",
            self.source,
        )
        self.assertLess(
            self.source.index("R40 evidence destination already exists"),
            self.source.index("\npublication_cleanup_armed=1\n"),
        )
        self.assertIn('"${artifact_archive_digest}"; do', self.source)
        self.assertIn("publication_complete=1", self.source)

    def test_native_hsa_lane_uses_the_deterministic_r26_pool_policy(self) -> None:
        hsa_source = RUNNER.with_name("striped_copy_hsa.cpp").read_text(
            encoding="utf-8"
        )
        self.assertIn('#include "r26_hsa_pool_policy.hpp"', hsa_source)
        self.assertIn("unique_enumerated_nearest_cpu", hsa_source)
        self.assertIn("select_hsa_pool_roles", hsa_source)
        self.assertIn('resource_profile.c_str(), "not-observed"', hsa_source)


if __name__ == "__main__":
    unittest.main()
