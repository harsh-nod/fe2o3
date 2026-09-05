#!/usr/bin/env python3

"""Static contract checks for the fail-closed R26 qualification runner."""

from __future__ import annotations

import pathlib
import subprocess
import unittest


RUNNER = pathlib.Path(__file__).with_name("run-r26-inplace-mi300x.sh")
HSA_COMPARATOR = pathlib.Path(__file__).with_name("inplace_transform_hsa.cpp")
COMMON_HEADER = pathlib.Path(__file__).with_name("inplace_benchmark_common.hpp")


class R26RunnerContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = RUNNER.read_text(encoding="utf-8")
        cls.hsa_source = HSA_COMPARATOR.read_text(encoding="utf-8")
        cls.common_source = COMMON_HEADER.read_text(encoding="utf-8")

    def test_shell_is_syntactically_valid(self) -> None:
        subprocess.run(["/usr/bin/bash", "-n", str(RUNNER)], check=True)

    def test_runner_uses_the_v3_persistent_control_evidence_contract(self) -> None:
        self.assertIn("fe2o3.r26-inplace-benchmark.v3", self.source)
        self.assertNotIn("fe2o3.r26-inplace-benchmark.v2", self.source)
        self.assertIn("fe2o3.r26-inplace-benchmark.v3", self.common_source)
        self.assertIn("control_path=n/a", self.common_source)

    def test_repository_components_are_staged_and_reverified(self) -> None:
        required = (
            "inplace_transform.hsaco",
            "inplace_transform_hsa.cpp",
            "inplace_transform_hip.cpp",
            "bounded_binary_file_reader.hpp",
            "r26_hsa_pool_policy.hpp",
            "inplace_benchmark_common.hpp",
            "check-parity.py",
            "r26-host-guard.py",
            "r26-system-identity.py",
            "run-r26-inplace-mi300x.sh",
        )
        for name in required:
            self.assertIn(f'"${{snapshot_dir}}/{name}"', self.source)
        self.assertGreaterEqual(self.source.count("verify_staged_inputs"), 3)
        final_verification = self.source.rindex("verify_staged_inputs")
        final_publish = self.source.rindex('"${artifact_dir}"')
        self.assertLess(final_verification, final_publish)
        self.assertIn('"${checker}" "${slot_logs[@]}"', self.source)
        self.assertIn('/usr/bin/python3 "${host_guard}" monitor', self.source)
        self.assertIn('/usr/bin/python3 "${system_identity_collector}"', self.source)

    def test_cargo_build_uses_one_read_only_git_archive(self) -> None:
        self.assertIn(
            '/usr/bin/git -C "${repo_root}" archive \\\n  --format=tar "${git_commit}"',
            self.source,
        )
        self.assertIn('/usr/bin/tar --extract --file="${source_archive}"', self.source)
        self.assertIn('/usr/bin/chmod -R u-w,go-rwx -- "${source_tree}"', self.source)
        self.assertIn('/usr/bin/chmod -R u+rwX -- "${source_tree}"', self.source)
        self.assertIn('cd "${source_tree}"', self.source)
        self.assertIn('--manifest-path "${source_tree}/Cargo.toml"', self.source)
        self.assertIn(
            '"${archived_benchmark_dir}/run-r26-inplace-mi300x.sh"', self.source
        )
        self.assertNotIn('cd "${repo_root}"', self.source)

    def test_build_and_measurement_environments_are_explicit(self) -> None:
        self.assertIn("readonly -a qualification_env=(", self.source)
        self.assertIn("readonly -a rust_build_env=(", self.source)
        self.assertIn("readonly -a native_build_env=(", self.source)
        self.assertIn("/usr/bin/env -i", self.source)
        self.assertIn("CARGO_INCREMENTAL=0", self.source)
        self.assertIn('CARGO_TARGET_DIR="${build_dir}/target"', self.source)
        self.assertIn('readonly rust_tool_path="${build_home}/.cargo/bin"', self.source)
        self.assertIn('"${rust_build_env[@]}" "${cargo_executable}" build', self.source)
        self.assertIn('"${native_build_env[@]}" /usr/bin/g++', self.source)
        self.assertIn(
            '"${qualification_env[@]}" /usr/bin/taskset --cpu-list "${observer_cpu}"',
            self.source,
        )
        self.assertIn('--membind="${topology_numa_node}" /usr/bin/true', self.source)
        self.assertIn("/usr/bin/taskset", self.source)
        self.assertEqual(
            self.source.count('/usr/bin/taskset --cpu-list "${measurement_cpu_list}"'),
            4,
        )
        self.assertEqual(
            self.source.count(
                '/usr/bin/taskset --cpu-list "${measurement_cpu_list}"\n'
                "        /usr/bin/numactl"
            ),
            3,
        )
        self.assertIn(
            "placement=taskset-cpulist-then-numactl-physcpubind-membind-v1",
            self.source,
        )

    def test_context_authenticates_collector_and_declares_build_contract(self) -> None:
        self.assertIn("system_identity_collector_sha256=%s", self.source)
        self.assertIn(
            "build_environment="
            "env-i-explicit-home-toolchain-path-cargo-incremental-0-"
            "private-target-v1",
            self.source,
        )
        self.assertIn(
            'system_identity_collector_sha256="$(sha256_file '
            '"${system_identity_collector}")"',
            self.source,
        )
        self.assertIn(
            'hsa_pool_policy_sha256="$(sha256_file "${hsa_pool_policy}")"',
            self.source,
        )
        self.assertIn("hsa_pool_policy_sha256=%s", self.source)

    def test_context_requires_the_v2_process_tree_monitor(self) -> None:
        self.assertIn(
            "interference_monitor=selected-kfd-gpu-process-tree-census-v2",
            self.source,
        )
        self.assertIn("monitor schema=fe2o3.r26-kfd-queue-monitor.v2 ", self.source)
        self.assertNotIn(
            "interference_monitor=selected-kfd-gpu-process-tree-census-v1",
            self.source,
        )
        self.assertNotIn("monitor schema=fe2o3.r26-kfd-queue-monitor.v1 ", self.source)

    def test_runner_tracks_and_forwards_signals_to_the_monitor(self) -> None:
        self.assertIn('active_monitor_pid=""', self.source)
        self.assertIn("active_monitor_pid=$!", self.source)
        self.assertIn('kill -s "${signal_name}" "${active_monitor_pid}"', self.source)
        self.assertGreaterEqual(
            self.source.count('wait "${active_monitor_pid}"'),
            2,
        )
        for signal_name, exit_code in (
            ("HUP", "129"),
            ("INT", "130"),
            ("QUIT", "131"),
            ("TERM", "143"),
        ):
            self.assertIn(
                f"trap 'forward_signal {signal_name} {exit_code}' {signal_name}",
                self.source,
            )
        self.assertIn('} >"${slot_log}"', self.source)
        self.assertNotIn(
            '} | "${qualification_env[@]}" /usr/bin/tee "${slot_log}"',
            self.source,
        )

    def test_hsa_comparator_reads_one_bounded_exact_size_code_object(self) -> None:
        self.assertIn("kMaximumHsacoBytes", self.hsa_source)
        self.assertIn("fe2o3::r26::read_bounded_binary_file", self.hsa_source)
        self.assertIn('#include "bounded_binary_file_reader.hpp"', self.hsa_source)
        self.assertNotIn("istreambuf_iterator", self.hsa_source)

    def test_hsa_comparator_resolves_the_code_object_descriptor_symbol(self) -> None:
        self.assertIn(
            'constexpr char kKernelDescriptor[] = "inplace_transform.kd"',
            self.common_source,
        )
        self.assertIn(
            "constexpr std::uint32_t kHsaKernargAlignment = 16",
            self.common_source,
        )
        self.assertIn(
            "kernel.executable, fe2o3::r26::kKernelDescriptor, &gpu, &symbol",
            self.hsa_source,
        )
        self.assertIn(
            "kernel.kernarg_alignment != fe2o3::r26::kHsaKernargAlignment",
            self.hsa_source,
        )
        self.assertNotIn(
            "kernel.executable, fe2o3::r26::kKernel, &gpu, &symbol",
            self.hsa_source,
        )

    def test_hsa_comparator_validates_kernarg_allocation_alignment(self) -> None:
        kernel_load = self.hsa_source.index(
            "const Kernel kernel = load_kernel(code_object, gpu)"
        )
        pool_query = self.hsa_source.index(
            "kernarg_pool, HSA_AMD_MEMORY_POOL_INFO_RUNTIME_ALLOC_ALIGNMENT"
        )
        self.assertIn(
            "kernarg_pool_alignment < kernel.kernarg_alignment",
            self.hsa_source,
        )
        allocation = self.hsa_source.index("hsa_amd_memory_pool_allocate(kernarg_pool")
        pointer_check = self.hsa_source.index(
            "reinterpret_cast<std::uintptr_t>(kernarg) % kernel.kernarg_alignment"
        )
        first_kernarg_write = self.hsa_source.index(
            "std::memcpy(static_cast<std::byte *>(kernarg)"
        )
        dispatch = self.hsa_source.index(
            "publish_dispatch(queue, kernel, kernarg, dispatch_signal)"
        )
        self.assertLess(kernel_load, pool_query)
        self.assertLess(pool_query, allocation)
        self.assertLess(allocation, pointer_check)
        self.assertLess(pointer_check, first_kernarg_write)
        self.assertLess(pointer_check, dispatch)

    def test_hsa_comparator_enforces_exact_pool_and_access_policy(self) -> None:
        self.assertIn('#include "r26_hsa_pool_policy.hpp"', self.hsa_source)
        self.assertEqual(self.hsa_source.count("collect_pools("), 3)
        self.assertIn("select_hsa_pool_roles", self.hsa_source)
        self.assertNotIn("HSA_STATUS_INFO_BREAK", self.hsa_source)
        self.assertIn("HSA_AGENT_INFO_DEVICE, &nearest_cpu_type", self.hsa_source)
        self.assertIn("unique_enumerated_nearest_cpu", self.hsa_source)
        self.assertEqual(
            self.hsa_source.count("HSA_AMD_AGENT_MEMORY_POOL_INFO_ACCESS"), 2
        )
        self.assertIn(
            "hsa_amd_agents_allow_access(1, &cpu, nullptr, device)",
            self.hsa_source,
        )
        self.assertNotIn(
            "hsa_amd_agents_allow_access(1, &gpu, nullptr, device)",
            self.hsa_source,
        )

    def test_rocm_smi_and_persistence_tools_use_fixed_environments(self) -> None:
        rocm_invocations = [
            line
            for line in self.source.splitlines()
            if '"${rocm_path}/bin/rocm-smi"' in line
            and '"${qualification_env[@]}"' in line
        ]
        self.assertEqual(len(rocm_invocations), 3)
        self.assertTrue(
            all('"${qualification_env[@]}"' in line for line in rocm_invocations)
        )
        for tool in ("cat", "cmp", "cp", "mkdir", "mv", "tee"):
            self.assertIn(f'"${{qualification_env[@]}}" /usr/bin/{tool}', self.source)


if __name__ == "__main__":
    unittest.main()
