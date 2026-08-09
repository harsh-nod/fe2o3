#!/usr/bin/env python3
"""Unit tests for the fixed S09 production policy descriptor contract."""

from __future__ import annotations

import importlib.util
import pathlib
import stat
import sys
import types
import unittest


CHECKER_PATH = pathlib.Path(__file__).parents[1] / "s09-debug-check.py"
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
            pathlib.Path("/etc/fe2o3/s09-trust-v1.tsv"),
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


if __name__ == "__main__":
    unittest.main()
