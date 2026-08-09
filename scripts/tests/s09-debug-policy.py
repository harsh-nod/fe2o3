#!/usr/bin/env python3
"""Unit tests for the fixed S09 production policy descriptor contract."""

from __future__ import annotations

import importlib.util
import hashlib
import pathlib
import re
import stat
import struct
import sys
import types
import unittest


CHECKER_PATH = pathlib.Path(__file__).parents[1] / "s09-debug-check.py"
LANE_PATH = pathlib.Path(__file__).parents[1] / "s09-debug-ci.sh"
SPEC = importlib.util.spec_from_file_location("s09_debug_check", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.dont_write_bytecode = True
SPEC.loader.exec_module(CHECKER)


def metadata(**overrides: int) -> types.SimpleNamespace:
    values = {
        "st_mode": stat.S_IFREG | 0o444,
        "st_nlink": 1,
        "st_uid": 0,
        "st_size": 128,
        "st_dev": 1,
        "st_ino": 2,
        "st_mtime_ns": 3,
        "st_ctime_ns": 4,
    }
    values.update(overrides)
    return types.SimpleNamespace(**values)


def digest(character: str) -> str:
    return character * 64


def valid_manifest(domain: str = "test-fixture-v2") -> dict[str, str]:
    values = {
        "manifest_schema": CHECKER.MANIFEST_SCHEMA,
        "trust_domain": domain,
        "claim": "source-debug-evidence-v2",
        "identity_section": CHECKER.S09_IDENTITY_SECTION,
        "semantic_admission_sha256": digest("1"),
        "semantic_schema": CHECKER.SEMANTIC_ADMISSION_SCHEMA,
        "semantic_crate": "fe2o3_typed_alias_spoof",
        "semantic_module": "general_genuine",
        "semantic_logical_name": "alpha",
        "semantic_export_name": "alpha",
        "semantic_profile": "general-scalar-slice-rustc-layout-v3",
        "semantic_source_path": CHECKER.S09_SOURCE,
        "semantic_source_sha256": CHECKER.S09_SOURCE_SHA256,
        "semantic_source_bytes": CHECKER.S09_SOURCE_LENGTH,
        "semantic_target": "gfx942:xnack-",
        "semantic_target_capabilities": "atomics,amd-wave",
        "semantic_code_object_version": "6",
        "semantic_rustc_opt_level": "0",
        "semantic_rustc_debug_info": "full",
        "semantic_injected_debug_policy": "dwarf-v5-full",
        "semantic_abi_sha256": digest("2"),
        "semantic_launch_sha256": digest("3"),
        "semantic_portable_mir_sha256": digest("4"),
        "build_observation_sha256": digest("5"),
        "build_schema": CHECKER.BUILD_OBSERVATION_SCHEMA,
        "build_semantic_admission_sha256": digest("1"),
        "build_cargo_metadata_sha256": digest("6"),
        "build_crate_binding": digest("7"),
        "build_kernel_binding": digest("8"),
        "build_observed_def_path": "metadata_specific::kernel_a",
        "build_observed_symbol": "metadata_specific_kernel_a",
        "build_rustc_mir_capture_sha256": digest("9"),
        "build_rustc_invocation_sha256": digest("a"),
        "build_rustc_executable_sha256": digest("b"),
        "build_cargo_fe2o3_executable_sha256": digest("c"),
        "build_cargo_executable_sha256": digest("d"),
        "build_codegen_backend_sha256": digest("e"),
        "build_worker_config_sha256": digest("f"),
        "build_worker_executable_sha256": digest("1"),
        "build_worker_build_identity_sha256": digest("2"),
        "build_llvm_build_identity_sha256": digest("3"),
        "source_commit": "3" * 40,
        "source_tree": "4" * 40,
        "hsaco_sha256": digest("5"),
        "host_executable_sha256": digest("6"),
        "host_executable_build_id": "7" * 40,
        "debug_archive_manifest_sha256": digest("8"),
        "artifact_facts_sha256": digest("9"),
        "hardware_facts_sha256": digest("a"),
        "dwarf_normalized_sha256": digest("b"),
        "rocgdb_normalized_sha256": digest("c"),
    }
    assert tuple(values) == CHECKER.MANIFEST_FIELDS
    return values


def identity_records() -> tuple[bytes, bytes]:
    semantic = {
        "schema": CHECKER.SEMANTIC_ADMISSION_SCHEMA,
        "crate": "fe2o3_typed_alias_spoof",
        "module": "general_genuine",
        "logical_name": "alpha",
        "export_name": "alpha",
        "profile": "general-scalar-slice-rustc-layout-v3",
        "source_path": CHECKER.S09_SOURCE,
        "source_sha256": CHECKER.S09_SOURCE_SHA256,
        "source_bytes": CHECKER.S09_SOURCE_LENGTH,
        "target": "gfx942:xnack-",
        "target_capabilities": "atomics,amd-wave",
        "code_object_version": "6",
        "rustc_opt_level": "0",
        "rustc_debug_info": "full",
        "injected_debug_policy": "dwarf-v5-full",
        "abi_sha256": digest("1"),
        "launch_sha256": digest("2"),
        "portable_mir_sha256": digest("3"),
    }
    semantic_record = CHECKER.serialize_ordered_fields(
        semantic, CHECKER.SEMANTIC_RECORD_FIELDS, "semantic admission"
    )
    build = {
        "schema": CHECKER.BUILD_OBSERVATION_SCHEMA,
        "semantic_admission_sha256": hashlib.sha256(semantic_record).hexdigest(),
        "cargo_metadata_sha256": digest("4"),
        "crate_binding": digest("5"),
        "kernel_binding": digest("6"),
        "observed_def_path": "metadata_a::kernel_a",
        "observed_symbol": "metadata_a_kernel_a",
        "rustc_mir_capture_sha256": digest("7"),
        "rustc_invocation_sha256": digest("8"),
        "rustc_executable_sha256": digest("9"),
        "cargo_fe2o3_executable_sha256": digest("a"),
        "cargo_executable_sha256": digest("b"),
        "codegen_backend_sha256": digest("c"),
        "worker_config_sha256": digest("d"),
        "worker_executable_sha256": digest("e"),
        "worker_build_identity_sha256": digest("f"),
        "llvm_build_identity_sha256": digest("1"),
    }
    build_record = CHECKER.serialize_ordered_fields(
        build, CHECKER.BUILD_RECORD_FIELDS, "build observation"
    )
    return semantic_record, build_record


def handoff(
    semantic_record: bytes | None = None,
    build_record: bytes | None = None,
    trailing: bytes = b"",
) -> bytes:
    default_semantic, default_build = identity_records()
    semantic_record = default_semantic if semantic_record is None else semantic_record
    build_record = default_build if build_record is None else build_record
    return (
        CHECKER.S09_IDENTITY_HANDOFF_DOMAIN
        + struct.pack("<I", len(semantic_record))
        + semantic_record
        + struct.pack("<I", len(build_record))
        + build_record
        + trailing
    )


def elf(identity: bytes, duplicate: bool = False) -> bytes:
    names = b"\0.shstrtab\0" + CHECKER.S09_IDENTITY_SECTION.encode("ascii") + b"\0"
    name_offset = len(b"\0.shstrtab\0")
    identity_count = 2 if duplicate else 1
    payload = names + identity * identity_count
    section_offset = 64 + len(payload)
    image = bytearray(section_offset + 64 * (2 + identity_count))
    image[:4] = b"\x7fELF"
    image[4:7] = b"\x02\x01\x01"
    struct.pack_into("<Q", image, 40, section_offset)
    struct.pack_into("<H", image, 58, 64)
    struct.pack_into("<H", image, 60, 2 + identity_count)
    struct.pack_into("<H", image, 62, 1)
    image[64 : 64 + len(payload)] = payload

    string_header = section_offset + 64
    struct.pack_into("<I", image, string_header, 1)
    struct.pack_into("<I", image, string_header + 4, 3)
    struct.pack_into("<Q", image, string_header + 24, 64)
    struct.pack_into("<Q", image, string_header + 32, len(names))
    for index in range(identity_count):
        header = section_offset + 64 * (2 + index)
        struct.pack_into("<I", image, header, name_offset)
        struct.pack_into("<I", image, header + 4, 1)
        struct.pack_into("<Q", image, header + 24, 64 + len(names) + index * len(identity))
        struct.pack_into("<Q", image, header + 32, len(identity))
    return bytes(image)


class ProductionPolicyMetadataTests(unittest.TestCase):
    def test_accepts_exact_protected_metadata(self) -> None:
        CHECKER.validate_production_policy_metadata(
            metadata(), CHECKER.FS_IMMUTABLE_FL
        )

    def test_rejects_non_root_owner(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.validate_production_policy_metadata(
                metadata(st_uid=1000), CHECKER.FS_IMMUTABLE_FL
            )

    def test_rejects_writable_policy(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.validate_production_policy_metadata(
                metadata(st_mode=stat.S_IFREG | 0o644), CHECKER.FS_IMMUTABLE_FL
            )

    def test_rejects_non_regular_policy(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.validate_production_policy_metadata(
                metadata(st_mode=stat.S_IFLNK | 0o444), CHECKER.FS_IMMUTABLE_FL
            )

    def test_rejects_multiple_links(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.validate_production_policy_metadata(
                metadata(st_nlink=2), CHECKER.FS_IMMUTABLE_FL
            )

    def test_rejects_missing_immutable_flag(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.validate_production_policy_metadata(metadata(), 0)

    def test_rejects_empty_or_oversized_policy(self) -> None:
        for size in (0, CHECKER.MAX_INPUT_BYTES + 1):
            with self.subTest(size=size), self.assertRaises(CHECKER.CheckError):
                CHECKER.validate_production_policy_metadata(
                    metadata(st_size=size), CHECKER.FS_IMMUTABLE_FL
                )

    def test_production_policy_path_is_fixed(self) -> None:
        self.assertEqual(
            CHECKER.PRODUCTION_POLICY_PATH,
            pathlib.Path("/etc/fe2o3/s09-trust-v2.tsv"),
        )

    def test_absent_installation_fails_closed(self) -> None:
        if CHECKER.PRODUCTION_POLICY_PATH.exists():
            self.skipTest("production policy is installed on this host")
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.read_production_policy()


class NormalizedEvidenceSchemaTests(unittest.TestCase):
    ARTIFACT_FACTS = (
        "format=fe2o3-s09-artifact-facts-v1\n"
        "object_format=elf64-amdgpu\n"
        "arch=amdgcn\n"
        "target=gfx942:xnack-\n"
        "optimization=O0\n"
        f"source_path={CHECKER.S09_SOURCE}\n"
        "kernel=alpha:alpha.kd\n"
        "kernel=zeta:zeta.kd\n"
    )

    def test_accepts_exact_artifact_schema(self) -> None:
        CHECKER.check_artifact_fact_schema(self.ARTIFACT_FACTS)

    def test_rejects_noncanonical_artifact_fields_and_values(self) -> None:
        probes = (
            self.ARTIFACT_FACTS.replace(
                f"source_path={CHECKER.S09_SOURCE}",
                "source_path=file:///home/private/main.rs",
            ),
            self.ARTIFACT_FACTS.replace("target=gfx942:xnack-", "target=gfx90a"),
            self.ARTIFACT_FACTS.replace("kernel=alpha:alpha.kd", "kernel=alpha=a.kd"),
            self.ARTIFACT_FACTS + "extra=value\n",
        )
        for probe in probes:
            with self.subTest(probe=probe), self.assertRaises(CHECKER.CheckError):
                CHECKER.check_artifact_fact_schema(probe)

    def test_rejects_absolute_and_uri_path_atoms(self) -> None:
        probes = (
            "/home/private/main.rs",
            "file:///home/private/main.rs",
            "https://host/etc/passwd",
            "file:%2Fhome%2Fprivate%2Fmain.rs",
            r"C:\private\main.rs",
            r"\\server\share\main.rs",
            r"\\?\C:\private\main.rs",
            "relative/../private/main.rs",
            "leak](/home/private/main.rs)",
        )
        for probe in probes:
            evidence = f"{CHECKER.S09_SOURCE}\n{probe}\n"
            with self.subTest(probe=probe), self.assertRaises(CHECKER.CheckError):
                CHECKER.require_path_hygiene(evidence)


class IdentityCodecConsumerTests(unittest.TestCase):
    def test_decodes_exact_records_and_build_binding(self) -> None:
        semantic_record, build_record = identity_records()
        image = elf(handoff(semantic_record, build_record))
        decoded_semantic, semantic, decoded_build, build = (
            CHECKER.decode_hsaco_identity_v2(image)
        )
        self.assertEqual(decoded_semantic, semantic_record)
        self.assertEqual(decoded_build, build_record)
        self.assertEqual(tuple(semantic), CHECKER.SEMANTIC_RECORD_FIELDS)
        self.assertEqual(tuple(build), CHECKER.BUILD_RECORD_FIELDS)
        self.assertEqual(
            build["semantic_admission_sha256"],
            hashlib.sha256(semantic_record).hexdigest(),
        )
        manifest_values = CHECKER.identity_manifest_values(image)
        self.assertEqual(tuple(manifest_values), CHECKER.IDENTITY_MANIFEST_FIELDS)
        self.assertEqual(len(CHECKER.MANIFEST_FIELDS), 51)

    def test_rejects_every_handoff_truncation_boundary(self) -> None:
        valid = handoff()
        for length in range(len(valid)):
            with self.subTest(length=length), self.assertRaises(CHECKER.CheckError):
                CHECKER.decode_hsaco_identity_v2(elf(valid[:length]))

    def test_rejects_missing_and_duplicate_identity_sections(self) -> None:
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.decode_hsaco_identity_v2(elf(handoff()).replace(
                CHECKER.S09_IDENTITY_SECTION.encode("ascii"), b".fe2o3.s09.identity.v1"
            ))
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.decode_hsaco_identity_v2(elf(handoff(), duplicate=True))

    def test_rejects_missing_duplicate_reordered_unknown_and_zero_fields(self) -> None:
        semantic_record, build_record = identity_records()
        lines = semantic_record.splitlines(keepends=True)
        mutations = {
            "missing": b"".join(lines[:-1]),
            "duplicate": b"".join(lines[:2] + [lines[1]] + lines[2:]),
            "reordered": b"".join([lines[1], lines[0], *lines[2:]]),
            "unknown": b"unknown\tvalue\n" + b"".join(lines[1:]),
            "zero": semantic_record.replace(
                f"source_sha256\t{CHECKER.S09_SOURCE_SHA256}".encode("ascii"),
                b"source_sha256\t" + b"0" * 64,
            ),
        }
        for name, mutated in mutations.items():
            with self.subTest(name=name), self.assertRaises(CHECKER.CheckError):
                CHECKER.decode_hsaco_identity_v2(elf(handoff(mutated, build_record)))

    def test_rejects_oversize_trailing_unknown_domain_and_broken_binding(self) -> None:
        semantic_record, build_record = identity_records()
        probes = {
            "oversize record": handoff(b"x" * (CHECKER.MAX_IDENTITY_RECORD_BYTES + 1), build_record),
            "trailing handoff": handoff(semantic_record, build_record, b"x"),
            "unknown domain": b"X" + handoff()[1:],
            "oversize handoff": handoff() + b"x" * CHECKER.MAX_IDENTITY_HANDOFF_BYTES,
            "trailing record": handoff(semantic_record + b"x", build_record),
            "oversize field": handoff(
                semantic_record.replace(
                    b"crate\tfe2o3_typed_alias_spoof",
                    b"crate\t" + b"x" * (CHECKER.MAX_IDENTITY_FIELD_VALUE_BYTES + 1),
                ),
                build_record,
            ),
            "broken binding": handoff(
                semantic_record,
                build_record.replace(
                    hashlib.sha256(semantic_record).hexdigest().encode("ascii"),
                    digest("f").encode("ascii"),
                ),
            ),
        }
        for name, probe in probes.items():
            with self.subTest(name=name), self.assertRaises(CHECKER.CheckError):
                CHECKER.decode_hsaco_identity_v2(elf(probe))


class ManifestV2SchemaTests(unittest.TestCase):
    def test_local_lane_serializes_the_exact_schema_order(self) -> None:
        lane = LANE_PATH.read_text(encoding="utf-8")
        fields = tuple(re.findall(r"printf '([a-z0-9_]+)\\t", lane))
        expected = (
            *CHECKER.MANIFEST_FIELDS[:3],
            *CHECKER.EVIDENCE_MANIFEST_FIELDS,
        )
        self.assertEqual(fields, expected)
        self.assertIn('"${CHECKER}" identity-fields', lane)
        self.assertIn('cat -- "${IDENTITY_FIELDS}"', lane)
        self.assertEqual(len(CHECKER.MANIFEST_FIELDS), 51)

    def test_serialization_is_deterministic_and_round_trips(self) -> None:
        values = valid_manifest()
        first = CHECKER.serialize_ordered_fields(
            values, CHECKER.MANIFEST_FIELDS, "protected manifest"
        )
        second = CHECKER.serialize_ordered_fields(
            dict(reversed(tuple(values.items()))),
            CHECKER.MANIFEST_FIELDS,
            "protected manifest",
        )
        self.assertEqual(first, second)
        self.assertEqual(
            CHECKER.parse_protected_manifest(first, "test-fixture-v2"), values
        )

    def test_every_field_is_required_and_duplicates_are_rejected(self) -> None:
        serialized = CHECKER.serialize_ordered_fields(
            valid_manifest(), CHECKER.MANIFEST_FIELDS, "protected manifest"
        )
        lines = serialized.splitlines(keepends=True)
        for index, field in enumerate(CHECKER.MANIFEST_FIELDS):
            with self.subTest(field=field, mutation="missing"):
                with self.assertRaises(CHECKER.CheckError):
                    CHECKER.parse_protected_manifest(
                        b"".join(lines[:index] + lines[index + 1 :]),
                        "test-fixture-v2",
                    )
            with self.subTest(field=field, mutation="duplicate"):
                with self.assertRaises(CHECKER.CheckError):
                    CHECKER.parse_protected_manifest(
                        b"".join(lines[:index] + [lines[index]] + lines[index:]),
                        "test-fixture-v2",
                    )

    def test_zero_is_rejected_except_where_the_codec_allows_it(self) -> None:
        for field in CHECKER.MANIFEST_FIELDS:
            if field in {
                "semantic_rustc_opt_level",
                "build_observed_def_path",
                "build_observed_symbol",
            }:
                continue
            values = valid_manifest()
            values[field] = "0"
            data = CHECKER.serialize_ordered_fields(
                values, CHECKER.MANIFEST_FIELDS, "protected manifest"
            )
            with self.subTest(field=field), self.assertRaises(CHECKER.CheckError):
                CHECKER.parse_protected_manifest(data, "test-fixture-v2")

    def test_every_field_mutation_changes_digest_and_breaks_policy_binding(self) -> None:
        manifest = valid_manifest("production-v2")
        canonical = CHECKER.serialize_ordered_fields(
            manifest, CHECKER.MANIFEST_FIELDS, "protected manifest"
        )
        policy = {
            "policy_schema": CHECKER.POLICY_SCHEMA,
            "manifest_path": "/var/lib/fe2o3/s09/manifest-v2.tsv",
            "manifest_sha256": hashlib.sha256(canonical).hexdigest(),
            **manifest,
        }
        for field in CHECKER.MANIFEST_FIELDS:
            mutated = dict(manifest)
            mutated[field] = f"{mutated[field]}-mutation"
            mutated_bytes = CHECKER.serialize_ordered_fields(
                mutated, CHECKER.MANIFEST_FIELDS, "protected manifest"
            )
            with self.subTest(field=field):
                self.assertNotEqual(
                    hashlib.sha256(mutated_bytes).digest(),
                    hashlib.sha256(canonical).digest(),
                )
                with self.assertRaises(CHECKER.CheckError):
                    CHECKER.validate_policy_manifest_binding(policy, mutated)

    def test_nonzero_portable_and_exact_digests_are_required(self) -> None:
        for field in CHECKER.MANIFEST_FIELDS:
            if not field.endswith("_sha256"):
                continue
            values = valid_manifest()
            values[field] = "0" * 64
            data = CHECKER.serialize_ordered_fields(
                values, CHECKER.MANIFEST_FIELDS, "protected manifest"
            )
            with self.subTest(field=field), self.assertRaises(CHECKER.CheckError):
                CHECKER.parse_protected_manifest(data, "test-fixture-v2")

    def test_dynamic_symbol_observation_is_not_semantic_admission(self) -> None:
        values = valid_manifest()
        values["build_observed_def_path"] = "metadata_b::renamed_kernel"
        values["build_observed_symbol"] = "metadata_b_renamed_kernel"
        data = CHECKER.serialize_ordered_fields(
            values, CHECKER.MANIFEST_FIELDS, "protected manifest"
        )
        self.assertEqual(
            CHECKER.parse_protected_manifest(data, "test-fixture-v2"), values
        )


if __name__ == "__main__":
    unittest.main()
