#!/usr/bin/env python3
"""Unit tests for the fixed S09 production policy descriptor contract."""

from __future__ import annotations

import importlib.util
import hashlib
import pathlib
import re
import stat
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
    kernel_binding = digest("b")
    generated_name = f"__fe2o3_host_kernel_v1_{kernel_binding}"
    values = {
        "manifest_schema": CHECKER.MANIFEST_SCHEMA,
        "trust_domain": domain,
        "claim": "source-debug-evidence-v2",
        "semantic_admission_schema": CHECKER.SEMANTIC_ADMISSION_SCHEMA,
        "source_path": CHECKER.S09_SOURCE,
        "source_sha256": CHECKER.S09_SOURCE_SHA256,
        "source_length": CHECKER.S09_SOURCE_LENGTH,
        "logical_crate": "fe2o3_typed_alias_spoof",
        "logical_module": "general_genuine",
        "logical_kernel": "alpha",
        "logical_export": "alpha",
        "logical_owner": "fe2o3_typed_alias_spoof::general_genuine::alpha",
        "owner_authentication": "collector-authenticated-kernel-owner-v1",
        "profile": "general-scalar-slice-v3",
        "portable_mir_sha256": digest("1"),
        "portable_abi_sha256": digest("2"),
        "target": "gfx942:xnack-",
        "optimization": "O0",
        "code_object_version": "6",
        "target_policy": "gfx942:xnack-/cov6/o0/source-debug-v1",
        "debug_policy": "s09-alpha-source-dwarf-v1",
        "build_observation_schema": CHECKER.BUILD_OBSERVATION_SCHEMA,
        "source_commit": "3" * 40,
        "source_tree": "4" * 40,
        "ordered_cargo_metadata_sha256": digest("5"),
        "crate_binding_id": digest("a"),
        "kernel_binding_id": kernel_binding,
        "observed_def_path": f"general_genuine::{generated_name}",
        "observed_symbol": generated_name,
        "rustc_mir_capture_sha256": digest("6"),
        "cargo_sha256": digest("7"),
        "rustc_sha256": digest("8"),
        "backend_sha256": digest("9"),
        "llvm_sha256": digest("c"),
        "llvm_link_worker_sha256": digest("d"),
        "lld_sha256": digest("e"),
        "llvm_dwarfdump_sha256": digest("f"),
        "llvm_readobj_sha256": digest("1"),
        "rocgdb_sha256": digest("2"),
        "checker_sha256": digest("3"),
        "harness_source_sha256": digest("4"),
        "hsaco_sha256": digest("5"),
        "host_executable_sha256": digest("6"),
        "host_executable_build_id": "7" * 40,
        "debug_archive_manifest_sha256": digest("8"),
        "artifact_facts_sha256": digest("9"),
        "hardware_facts_sha256": digest("a"),
        "dwarf_normalized_sha256": digest("b"),
        "rocgdb_normalized_sha256": digest("c"),
        "hardware_test": CHECKER.HARDWARE_TEST,
        "execution_closure": {
            "test-fixture-v2": "test-fixture-v2",
            "local-capability-v2": "local-capability-v2",
            "production-v2": "protected-controller-v2",
        }[domain],
    }
    assert tuple(values) == CHECKER.MANIFEST_FIELDS
    return values


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


class ManifestV2SchemaTests(unittest.TestCase):
    def test_local_lane_serializes_the_exact_schema_order(self) -> None:
        lane = LANE_PATH.read_text(encoding="utf-8")
        fields = tuple(re.findall(r"printf '([a-z0-9_]+)\\t", lane))
        self.assertEqual(fields, CHECKER.MANIFEST_FIELDS)

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

    def test_every_field_rejects_zero(self) -> None:
        for field in CHECKER.MANIFEST_FIELDS:
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

    def test_dynamic_symbol_observation_must_match_kernel_binding(self) -> None:
        values = valid_manifest()
        values["observed_symbol"] = "__fe2o3_host_kernel_v1_" + digest("c")
        data = CHECKER.serialize_ordered_fields(
            values, CHECKER.MANIFEST_FIELDS, "protected manifest"
        )
        with self.assertRaises(CHECKER.CheckError):
            CHECKER.parse_protected_manifest(data, "test-fixture-v2")


if __name__ == "__main__":
    unittest.main()
