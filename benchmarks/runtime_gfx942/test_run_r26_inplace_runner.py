#!/usr/bin/env python3

"""Static contract checks for the fail-closed R26 qualification runner."""

from __future__ import annotations

import pathlib
import subprocess
import unittest


RUNNER = pathlib.Path(__file__).with_name("run-r26-inplace-mi300x.sh")


class R26RunnerContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = RUNNER.read_text(encoding="utf-8")

    def test_shell_is_syntactically_valid(self) -> None:
        subprocess.run(["/usr/bin/bash", "-n", str(RUNNER)], check=True)

    def test_repository_components_are_staged_and_reverified(self) -> None:
        required = (
            "inplace_transform.hsaco",
            "inplace_transform_hsa.cpp",
            "inplace_transform_hip.cpp",
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
        self.assertIn('"${qualification_env[@]}" /usr/bin/numactl', self.source)
        self.assertIn('--membind="${topology_numa_node}" /usr/bin/true', self.source)

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
