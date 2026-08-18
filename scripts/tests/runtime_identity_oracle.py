#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT_DIR = Path(__file__).resolve().parent
CHECKER_PATH = SCRIPT_DIR.parent / "runtime_identity_oracle.py"
FIXTURES = SCRIPT_DIR / "fixtures" / "runtime-identity-oracle"
SPEC = importlib.util.spec_from_file_location("runtime_identity_oracle", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
SPEC.loader.exec_module(CHECKER)

PURE = (FIXTURES / "pure-rust-v1.txt").read_bytes()
ROCMINFO = (FIXTURES / "rocminfo-v1.txt").read_bytes()
ROCM_VERSION = (FIXTURES / "rocm-version").read_bytes()


class RuntimeIdentityOracleTests(unittest.TestCase):
    def render(self, pure: bytes = PURE, rocminfo: bytes = ROCMINFO) -> str:
        return CHECKER.compare_and_render(
            pure,
            rocminfo,
            ROCM_VERSION,
            b"pure executable fixture",
            b"rocminfo executable fixture",
            b"comparator fixture",
        )

    def test_valid_fixtures_emit_only_measured_non_authority(self) -> None:
        evidence = self.render()
        self.assertIn(
            "schema=fe2o3-r1-device-identity-oracle-measurement-v1\n", evidence
        )
        self.assertIn("claim_status=Measured\n", evidence)
        self.assertIn("authority=none\n", evidence)
        self.assertIn("proof_effect=none\n", evidence)
        self.assertIn("runtime_authority_effect=none\n", evidence)
        for prohibited in ("claim_status=Checked", "Proved", "Verified"):
            self.assertNotIn(prohibited, evidence)
        gpu_lines = [line for line in evidence.splitlines() if line.startswith("gpu ")]
        self.assertEqual(len(gpu_lines), CHECKER.EXPECTED_GPU_COUNT)
        self.assertEqual(gpu_lines, sorted(gpu_lines))
        self.assertTrue(
            all(f"isa={CHECKER.EXPECTED_PRIMARY_ISA}" in line for line in gpu_lines)
        )

    def test_ansi_color_on_module_banner_is_bounded_and_accepted(self) -> None:
        colored = ROCMINFO.replace(
            b"ROCk module version 6.16.13 is loaded",
            b"\x1b[37mROCk module version 6.16.13 is loaded\x1b[0m",
            1,
        )
        self.assertEqual(len(CHECKER.parse_rocminfo(colored).gpus), 8)

    def test_pure_gpu_count_is_exact(self) -> None:
        with self.assertRaisesRegex(CHECKER.OracleInputError, "reports 7 GPUs"):
            CHECKER.parse_pure_rust(b"\n".join(PURE.splitlines()[:-1]) + b"\n")

    def test_pure_duplicate_uuid_is_rejected(self) -> None:
        duplicate = PURE.replace(b"10a254ce4987e716", b"6ced1647a296545c", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "duplicate unique ID"):
            CHECKER.parse_pure_rust(duplicate)

    def test_pure_unknown_field_is_rejected(self) -> None:
        mutated = PURE.replace(b" target=gfx942", b" extra=1 target=gfx942", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "fields differ from schema"):
            CHECKER.parse_pure_rust(mutated)

    def test_pure_profile_digest_substitution_is_rejected(self) -> None:
        mutated = PURE.replace(
            CHECKER.EXPECTED_PROFILE_SHA256.encode(), b"0" * 64, 1
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "profile_sha256"):
            CHECKER.parse_pure_rust(mutated)

    def test_pure_non_ascii_and_oversized_line_are_rejected(self) -> None:
        with self.assertRaisesRegex(CHECKER.OracleInputError, "not ASCII"):
            CHECKER.parse_pure_rust(PURE + b"\xff\n")
        with self.assertRaisesRegex(CHECKER.OracleInputError, "line over"):
            CHECKER.parse_pure_rust(b"x" * (CHECKER.MAX_LINE_BYTES + 1) + b"\n")
        with self.assertRaisesRegex(CHECKER.OracleInputError, "control byte"):
            CHECKER.parse_pure_rust(PURE.replace(b"profile=", b"profile=\t", 1))

    def test_pure_device_numbers_are_bounded(self) -> None:
        mutated = PURE.replace(b"node=2", b"node=0", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "outside the V1 bounds"):
            CHECKER.parse_pure_rust(mutated)

    def test_rocminfo_gpu_count_is_exact(self) -> None:
        marker = b"Agent 10\n"
        truncated = ROCMINFO[: ROCMINFO.index(marker)]
        with self.assertRaisesRegex(CHECKER.OracleInputError, "reports 7 GPUs"):
            CHECKER.parse_rocminfo(truncated)

    def test_rocminfo_duplicate_uuid_is_rejected(self) -> None:
        duplicate = ROCMINFO.replace(b"GPU-10a254ce4987e716", b"GPU-6ced1647a296545c", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "duplicate GPU UUID"):
            CHECKER.parse_rocminfo(duplicate)

    def test_rocminfo_wrong_wavefront_is_rejected(self) -> None:
        mutated = ROCMINFO.replace(b"64(0x40)", b"32(0x20)", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "Wavefront Size"):
            CHECKER.parse_rocminfo(mutated)

    def test_rocminfo_wrong_isa_feature_is_rejected(self) -> None:
        mutated = ROCMINFO.replace(b":xnack-", b":xnack+", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "ISA set"):
            CHECKER.parse_rocminfo(mutated)

    def test_rocminfo_firmware_drift_is_rejected(self) -> None:
        mutated = ROCMINFO.replace(
            b"Packet Processor uCode:: 192", b"Packet Processor uCode:: 191", 1
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "outside the V1 bounds"):
            CHECKER.parse_rocminfo(mutated)

    def test_rocminfo_missing_and_duplicate_security_fields_are_rejected(self) -> None:
        missing = ROCMINFO.replace(b"  Vendor Name:             AMD\n", b"", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "lacks GPU fields"):
            CHECKER.parse_rocminfo(missing)
        duplicate = ROCMINFO.replace(
            b"  Vendor Name:             AMD\n",
            b"  Vendor Name:             AMD\n  Vendor Name:             AMD\n",
            1,
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "repeats Vendor Name"):
            CHECKER.parse_rocminfo(duplicate)

    def test_rocminfo_header_substitution_is_rejected(self) -> None:
        mutated = ROCMINFO.replace(
            b"Runtime Version:         1.18", b"Runtime Version:         1.17"
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "headers differ"):
            CHECKER.parse_rocminfo(mutated)

    def test_uuid_set_disagreement_emits_no_measurement(self) -> None:
        mutated = PURE.replace(b"10a254ce4987e716", b"20a254ce4987e716", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "UUID sets disagree"):
            self.render(pure=mutated)

    def test_bdf_disagreement_emits_no_measurement(self) -> None:
        mutated = ROCMINFO.replace(
            b"BDFID:                   1280", b"BDFID:                   1281", 1
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "property disagreement"):
            self.render(rocminfo=mutated)

    def test_rocm_release_is_exact(self) -> None:
        with self.assertRaisesRegex(CHECKER.OracleInputError, "ROCm release"):
            CHECKER.compare_and_render(
                PURE,
                ROCMINFO,
                b"7.2.5\n",
                b"pure",
                b"oracle",
                b"checker",
            )

    def test_stable_reader_rejects_symlink_and_oversized_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            regular = root / "regular"
            regular.write_bytes(b"ok\n")
            symlink = root / "symlink"
            symlink.symlink_to(regular)
            with self.assertRaisesRegex(CHECKER.OracleInputError, "cannot open"):
                CHECKER._read_stable_regular(symlink, 16, "fixture")
            with self.assertRaisesRegex(CHECKER.OracleInputError, "not executable"):
                CHECKER._read_stable_regular(
                    regular, 16, "fixture", require_executable=True
                )
            regular.write_bytes(b"x" * 17)
            with self.assertRaisesRegex(CHECKER.OracleInputError, "exceeds 16 bytes"):
                CHECKER._read_stable_regular(regular, 16, "fixture")

    def test_hardware_runner_requires_explicit_opt_in(self) -> None:
        runner = SCRIPT_DIR.parent / "runtime-identity-oracle.sh"
        environment = os.environ.copy()
        environment.pop("FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE", None)
        result = subprocess.run(
            ["bash", str(runner)],
            check=False,
            capture_output=True,
            env=environment,
            text=True,
            timeout=5,
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn("refusing runtime identity oracle", result.stderr)


if __name__ == "__main__":
    unittest.main()
