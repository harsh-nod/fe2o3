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
