#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "run-tutorial-semantic-simulation.py"
SPEC = importlib.util.spec_from_file_location("tutorial_semantic_simulation", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)
ROOT = SCRIPT.parent.parent
IDENTITY = "1" * 64
OTHER_IDENTITY = "2" * 64


def real_manifest() -> dict:
    return json.loads((ROOT / "config" / "tutorial-kernel-manifest-v1.json").read_bytes())


def document() -> dict:
    command = {
        "arguments": ["examples/fake/Cargo.toml", "lib"],
        "environment": [],
        "executable": "scripts/run-tutorial-cpu-reference.sh",
        "timeoutSeconds": 1200,
        "workingDirectory": ".",
    }
    return {
        "compilerFixtures": [
            {
                "compilerInput": {
                    "contractSha256": IDENTITY,
                    "sourceClosureSha256": OTHER_IDENTITY,
                },
                "fixtureId": "gfx942-test",
                "target": "gfx942",
            }
        ],
        "capabilityKernels": [
            {"fixtureId": "gfx942-test", "kernelSymbol": "test_kernel"}
        ],
        "entries": [
            {"compilerFixtureIds": ["gfx942-test"], "lessonId": "test-lesson"}
        ],
        "qualification": {
            "suites": [
                {
                    "availability": "available",
                    "command": command,
                    "coverage": [
                        {"fixtureIds": ["gfx942-test"], "lessonId": "test-lesson"}
                    ],
                    "gate": "cpu-reference",
                    "suiteId": "cpu-reference-test",
                    "unavailableReason": None,
                }
            ]
        },
    }


def fixture() -> dict:
    request_argument = {
        "access": "read_write",
        "alignment": 4,
        "bytes": "0x00000000a5a5a5a5",
        "element": "f32",
        "kind": "buffer",
    }
    request = {
        "arguments": [request_argument],
        "grid": [1, 1, 1],
        "kernel": "test_kernel",
        "schema": RUNNER.REQUEST_SCHEMA,
        "workgroup": [1, 1, 1],
    }
    oracle_input = {
        "arguments": request["arguments"],
        "grid": request["grid"],
        "kernel": request["kernel"],
        "workgroup": request["workgroup"],
    }
    result = {
        "argumentRoles": ["output"],
        "canaries": [{"argument": 0, "bytes": "0xa5a5a5a5", "offset": 4}],
        "checkApplicability": {
            "canaries": {"reason": None, "status": "checked"},
            "padding": {"reason": None, "status": "checked"},
        },
        "compilerInputContractSha256": IDENTITY,
        "expectedArguments": [
            {"bytes": "0x0000803fa5a5a5a5", "element": "f32", "kind": "buffer"}
        ],
        "fixtureId": "gfx942-test",
        "kernelSymbol": "test_kernel",
        "numericalPolicy": {"mode": "exact-bits"},
        "oracle": {
            "function": "fake::cpu_reference",
            "inputSha256": RUNNER._sha256(RUNNER._canonical(oracle_input)),
            "kind": "existing-rust-cpu-reference-v1",
        },
        "oracleSuiteId": "cpu-reference-test",
        "padding": [{"argument": 0, "bytes": "0xa5a5a5a5", "offset": 4}],
        "physicalAbi": {
            "arguments": [
                {
                    "access": "read_write",
                    "element": "f32",
                    "kind": "buffer",
                    "name": "output",
                }
            ],
            "schema": RUNNER.ABI_SCHEMA,
            "signatureSha256": IDENTITY,
            "sourcePath": "examples/fake/src/lib.rs",
        },
        "productionExport": {
            "coordinates": None,
            "diagnostic": "FE2O3-TUTORIAL-PROBE-001: synthetic test exporter is unavailable",
            "diagnosticCode": "FE2O3-TUTORIAL-PROBE-001",
            "diagnosticIdentitySha256": None,
            "identityContract": RUNNER.PRODUCTION_EXPORT_IDENTITY_CONTRACT,
            "stage": "exporter-build",
            "status": "blocked",
        },
        "request": request,
        "schema": RUNNER.FIXTURE_SCHEMA,
        "sourceClosureSha256": OTHER_IDENTITY,
        "target": "gfx942",
    }
    export = result["productionExport"]
    diagnostic_subject = {
        "compilerInputContractSha256": IDENTITY,
        "diagnostic": export["diagnostic"],
        "diagnosticCode": export["diagnosticCode"],
        "fixtureId": "gfx942-test",
        "identityContract": export["identityContract"],
        "kernelSymbol": "test_kernel",
        "sourceClosureSha256": OTHER_IDENTITY,
        "stage": export["stage"],
        "status": export["status"],
        "target": "gfx942",
    }
    export["diagnosticIdentitySha256"] = RUNNER._sha256(
        RUNNER.DIAGNOSTIC_IDENTITY_DOMAIN + RUNNER._canonical(diagnostic_subject)
    )
    return result


def encoded_fixture(value: dict) -> bytes:
    return RUNNER._canonical(value) + b"\n"


def simulator_result(output: str = "0x0000803fa5a5a5a5") -> bytes:
    return RUNNER._canonical(
        {
            "arguments": [{"kind": "buffer", "value": {"bytes": output}}],
            "authority": "observation_only",
            "conflict_assessment": {"status": "no_conflicts_observed"},
            "hardware_observed": False,
            "hardware_validation": False,
            "kir": {"sha256": IDENTITY},
            "performance_prediction": False,
            "schedule": {"coverage": {"complete": True}},
            "schema": RUNNER.SIMULATOR_RESULT_SCHEMA,
            "simulated": True,
            "status": "ok",
        }
    ) + b"\n"


def schedule(value: dict, bundle: bytes = b"bundle-v8") -> bytes:
    request = RUNNER._canonical(value["request"]) + b"\n"
    return RUNNER._canonical(
        {
            "artifact": {
                "bundle_sha256": RUNNER._sha256(bundle),
                "final_graph_epoch": 7,
                "kind": "simulation_bundle_v8",
                "kir_sha256": IDENTITY,
                "subject_sha256": OTHER_IDENTITY,
            },
            "request": {"bytes": len(request), "sha256": RUNNER._sha256(request)},
            "schema": RUNNER.SCHEDULE_SCHEMA,
            "target": {"identity": "amdgpu_64_little_endian_v1"},
        }
    ) + b"\n"


def validate(value: dict, *, output: str = "0x0000803fa5a5a5a5", bundle: bytes = b"bundle-v8") -> dict:
    manifest = document()
    return RUNNER.validate_observation(
        manifest,
        value,
        encoded_fixture(value),
        manifest["qualification"]["suites"][0],
        bundle,
        schedule(value, bundle),
        simulator_result(output),
        b"",
        b"cpu oracle passed",
    )


class TutorialSemanticSimulationTests(unittest.TestCase):
    def test_source_abi_maps_only_exact_macro_capability_roles(self) -> None:
        expected = {
            "Global<'_, f32, ReadOnly>": ("f32", "read_only"),
            "Global<'_, u32, DisjointWrite<GridExclusive>>": ("u32", "write_only"),
            "Global<'_, f32, ExclusiveReadWrite>": ("f32", "read_write"),
            "Global<'_, u32, AtomicReadWrite<SystemScope>>": ("u32", "read_write"),
            "WriteOnlyDisjointSlice<u32, GridExclusive>": ("u32", "write_only"),
        }
        for source_type, abi in expected.items():
            with self.subTest(source_type=source_type):
                self.assertEqual(abi, RUNNER._source_buffer_abi(source_type))
        for source_type in (
            "Global<'_, f32, UnknownRole>",
            "Global<'_, f32, DisjointWrite<Index1D, Index1D>>",
            "Global<'_, f32, AtomicReadWrite<>>",
        ):
            with self.subTest(source_type=source_type), self.assertRaisesRegex(
                RUNNER.SimulationQualificationError,
                "unsupported Global capability role",
            ):
                RUNNER._source_buffer_abi(source_type)

    def test_manifest_roster_has_one_exact_command_for_all_47_fixtures(self) -> None:
        manifest = real_manifest()
        roster = RUNNER.manifest_simulation_roster(manifest)
        fixture_ids = {fixture["fixtureId"] for fixture in manifest["compilerFixtures"]}
        self.assertEqual(47, len(fixture_ids))
        self.assertEqual(fixture_ids, set(roster))
        self.assertEqual(47, len({item["suiteId"] for item in roster.values()}))
        self.assertEqual(47, len({RUNNER.command_sha256(item["command"]) for item in roster.values()}))
        for fixture_id, item in roster.items():
            self.assertEqual(["--fixture", fixture_id], item["command"]["arguments"])
            covered = {
                covered_fixture
                for coverage in item["coverage"]
                for covered_fixture in coverage["fixtureIds"]
            }
            self.assertEqual({fixture_id}, covered)

    def test_accepts_complete_exact_observation(self) -> None:
        evidence = validate(fixture())
        self.assertEqual(RUNNER.SUITE_RESULT_SCHEMA, evidence["schema"])
        self.assertEqual("observation_only", evidence["authority"])
        self.assertEqual(IDENTITY, evidence["bindings"]["optimizedKirV13Sha256"])
        self.assertTrue(all(evidence["checks"].values()))

    def test_rejects_output_mutation(self) -> None:
        with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "CPU oracle"):
            validate(fixture(), output="0x00000040a5a5a5a5")

    def test_rejects_canary_mutation(self) -> None:
        with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "CPU oracle|canaries"):
            validate(fixture(), output="0x0000803f00000000")

    def test_rejects_stale_bundle_schedule(self) -> None:
        value = fixture()
        manifest = document()
        with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "stale.*Bundle"):
            RUNNER.validate_observation(
                manifest,
                value,
                encoded_fixture(value),
                manifest["qualification"]["suites"][0],
                b"new-bundle",
                schedule(value, b"old-bundle"),
                simulator_result(),
                b"",
                b"oracle",
            )

    def test_rejects_cross_target_schedule(self) -> None:
        value = fixture()
        raw = json.loads(schedule(value))
        raw["target"]["identity"] = "little_endian_index32_v1"
        manifest = document()
        with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "cross-target"):
            RUNNER.validate_observation(
                manifest,
                value,
                encoded_fixture(value),
                manifest["qualification"]["suites"][0],
                b"bundle-v8",
                RUNNER._canonical(raw) + b"\n",
                simulator_result(),
                b"",
                b"oracle",
            )

    def test_rejects_stale_and_cross_fixture_coordinates(self) -> None:
        manifest = document()
        value = fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "gfx942-test.json"
            path.write_bytes(encoded_fixture(value))
            with mock.patch.object(RUNNER, "FIXTURE_ROOT", root), mock.patch.object(
                RUNNER, "_validate_physical_abi"
            ):
                loaded, _, _ = RUNNER.load_fixture(manifest, "gfx942-test")
                self.assertEqual("gfx942-test", loaded["fixtureId"])
                stale = copy.deepcopy(value)
                stale["compilerInputContractSha256"] = "3" * 64
                path.write_bytes(encoded_fixture(stale))
                with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "stale or substituted"):
                    RUNNER.load_fixture(manifest, "gfx942-test")
                cross = copy.deepcopy(value)
                cross["fixtureId"] = "gfx950-other"
                path.write_bytes(encoded_fixture(cross))
                with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "stale or substituted"):
                    RUNNER.load_fixture(manifest, "gfx942-test")

    def test_rejects_missing_oracle_and_fixture(self) -> None:
        manifest = document()
        manifest["qualification"]["suites"].clear()
        with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "CPU-reference oracle"):
            RUNNER._cpu_suite(manifest, "gfx942-test")
        with tempfile.TemporaryDirectory() as temporary:
            with mock.patch.object(RUNNER, "FIXTURE_ROOT", Path(temporary)):
                with self.assertRaisesRegex(RUNNER.SimulationQualificationError, "no genuine typed"):
                    RUNNER.load_fixture(document(), "gfx942-test")


if __name__ == "__main__":
    unittest.main()
