#!/usr/bin/env python3

from __future__ import annotations

from dataclasses import replace
import hashlib
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
PURE_EXECUTABLE = b"pure executable fixture"
ROCMINFO_EXECUTABLE = b"rocminfo executable fixture"
COMPARATOR = b"comparator fixture"


def provenance() -> CHECKER.ProvenanceInputs:
    elf_digest = hashlib.sha256(PURE_EXECUTABLE).hexdigest()
    return CHECKER.ProvenanceInputs(
        runner_data=b"oracle runner fixture\n",
        policy_data=b"runtime policy fixture\n",
        auditor_data=b"runtime auditor fixture\n",
        cargo_lock_data=b"cargo lock fixture\n",
        metadata_audit_report_data=(
            b"pure-Rust runtime audit: OK (metadata roots=5 packages=17 "
            b"allowed_build_scripts=libc@0.2.189,rustix@1.1.4 sha256="
            + b"a" * 64
            + b"; profile=fe2o3.runtime.pure-rust.gfx942.v1)\n"
        ),
        elf_audit_report_data=(
            f"pure-Rust runtime audit: OK (ELF bytes={len(PURE_EXECUTABLE)} "
            f"needed=3 dynsym=75 sha256={elf_digest}; "
            "profile=fe2o3.runtime.pure-rust.gfx942.v1)\n"
        ).encode(),
        git_observation_data=b"head=1111111111111111111111111111111111111111\nworktree=clean\n",
        measurement_time_data=b"2026-08-18T12:34:56Z\n",
    )


class RuntimeIdentityOracleTests(unittest.TestCase):
    def render(self, pure: bytes = PURE, rocminfo: bytes = ROCMINFO) -> str:
        return CHECKER.compare_and_render(
            pure,
            rocminfo,
            ROCM_VERSION,
            PURE_EXECUTABLE,
            ROCMINFO_EXECUTABLE,
            COMPARATOR,
            provenance(),
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
        self.assertIn("currentness_claim_status=Contracted\n", evidence)
        self.assertIn("currentness=contracted-clear\n", evidence)
        self.assertIn("currentness_hsa_comparison=not-performed\n", evidence)
        self.assertIn("vram_lost_counter_source=pure-rust-only\n", evidence)
        self.assertIn("git_worktree=clean\n", evidence)
        self.assertIn("measurement_time_trust=untrusted-host-clock\n", evidence)
        for prohibited in ("claim_status=Checked", "Proved", "Verified"):
            self.assertNotIn(prohibited, evidence)
        gpu_lines = [line for line in evidence.splitlines() if line.startswith("gpu ")]
        self.assertEqual(len(gpu_lines), CHECKER.EXPECTED_GPU_COUNT)
        self.assertEqual(gpu_lines, sorted(gpu_lines))
        self.assertTrue(
            all(f"isa={CHECKER.EXPECTED_PRIMARY_ISA}" in line for line in gpu_lines)
        )
        self.assertTrue(all("differential_match=true" in line for line in gpu_lines))
        self.assertTrue(
            all("vram_lost_counter_source=pure-rust-only" in line for line in gpu_lines)
        )
        self.assertTrue(all("currentness=contracted-clear" in line for line in gpu_lines))
        self.assertNotIn(" match=true", evidence)

    def test_valid_fixtures_match_canonical_evidence_golden(self) -> None:
        expected = (FIXTURES / "measured-evidence-v1.txt").read_text(encoding="ascii")
        self.assertEqual(self.render(), expected)

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

    def test_pure_currentness_fields_are_required_and_bounded(self) -> None:
        missing = PURE.replace(b" currentness=contracted-clear", b"", 1)
        with self.assertRaisesRegex(CHECKER.OracleInputError, "fields differ from schema"):
            CHECKER.parse_pure_rust(missing)
        overflow = PURE.replace(
            b"vram_lost_counter=0", b"vram_lost_counter=4294967296", 1
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "outside the V1 bounds"):
            CHECKER.parse_pure_rust(overflow)
        substituted = PURE.replace(
            b"currentness=contracted-clear", b"currentness=proved-current", 1
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "currentness"):
            CHECKER.parse_pure_rust(substituted)

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
                PURE_EXECUTABLE,
                ROCMINFO_EXECUTABLE,
                COMPARATOR,
                provenance(),
            )

    def test_provenance_rejects_dirty_or_malformed_git_observation(self) -> None:
        dirty = provenance()
        dirty = replace(
            dirty,
            git_observation_data=dirty.git_observation_data.replace(
                b"worktree=clean", b"worktree=dirty"
            ),
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "exact clean commit"):
            CHECKER.parse_provenance(dirty, PURE_EXECUTABLE)

        malformed = provenance()
        malformed = replace(
            malformed,
            git_observation_data=b"head=not-a-commit\nworktree=clean\n",
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "exact clean commit"):
            CHECKER.parse_provenance(malformed, PURE_EXECUTABLE)

    def test_provenance_rejects_audit_report_substitution(self) -> None:
        inputs = provenance()
        wrong_elf = replace(
            inputs,
            elf_audit_report_data=inputs.elf_audit_report_data.replace(
                hashlib.sha256(PURE_EXECUTABLE).hexdigest().encode(), b"0" * 64
            ),
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "not bound"):
            CHECKER.parse_provenance(wrong_elf, PURE_EXECUTABLE)

        failed_metadata = replace(
            inputs,
            metadata_audit_report_data=inputs.metadata_audit_report_data.replace(
                b"audit: OK", b"audit: FAILED"
            ),
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "success schema"):
            CHECKER.parse_provenance(failed_metadata, PURE_EXECUTABLE)

        incomplete_roots = replace(
            inputs,
            metadata_audit_report_data=inputs.metadata_audit_report_data.replace(
                b"metadata roots=5", b"metadata roots=4"
            ),
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "V1 policy gate"):
            CHECKER.parse_provenance(incomplete_roots, PURE_EXECUTABLE)

    def test_provenance_rejects_invalid_utc_time(self) -> None:
        inputs = provenance()
        invalid = replace(
            inputs,
            measurement_time_data=b"2026-02-30T12:34:56Z\n",
        )
        with self.assertRaisesRegex(CHECKER.OracleInputError, "invalid"):
            CHECKER.parse_provenance(invalid, PURE_EXECUTABLE)

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
