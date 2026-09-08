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
ROOT = Path(__file__).resolve().parents[2]


def load(name: str, relative: str):
    path = ROOT / relative
    specification = importlib.util.spec_from_file_location(name, path)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


RUNNER = load("tutorial_semantic_runner", "scripts/run-tutorial-semantic-simulation.py")
GENERATOR = load("tutorial_semantic_generator", "scripts/generate-tutorial-semantic-fixtures.py")
UPDATER = load("tutorial_semantic_updater", "scripts/update-tutorial-semantic-simulation-manifest.py")


class TutorialSemanticFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = json.loads(RUNNER.MANIFEST.read_bytes())
        cls.fixture_ids = {item["fixtureId"] for item in cls.manifest["compilerFixtures"]}

    def load_from(self, root: Path, identity: str) -> dict:
        with mock.patch.object(RUNNER, "FIXTURE_ROOT", root):
            return RUNNER.load_fixture(self.manifest, identity)[0]

    def mutated_root(self, identity: str, mutate) -> tempfile.TemporaryDirectory[str]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        value = json.loads((RUNNER.FIXTURE_ROOT / f"{identity}.json").read_bytes())
        mutate(value)
        (root / f"{identity}.json").write_bytes(RUNNER._canonical(value) + b"\n")
        return temporary

    @staticmethod
    def available_export(target: str) -> dict:
        verified = "1" * 64
        return {
            "coordinates": {
                "bundleContentIdentitySha256": "2" * 64,
                "bundleSubjectIdentitySha256": "3" * 64,
                "bundleVersion": 8,
                "finalOptimizedKirSha256": verified,
                "kirVersion": 13,
                "optimizedKirV13ContentSha256": "4" * 64,
                "productionKirIdentitySha256": verified,
                "simulationBundleV8ContentSha256": "5" * 64,
                "simulatorSubjectSha256": verified,
                "target": target,
            },
            "diagnostic": None,
            "diagnosticCode": None,
            "diagnosticIdentitySha256": None,
            "identityContract": RUNNER.PRODUCTION_EXPORT_IDENTITY_CONTRACT,
            "stage": "production-transaction",
            "status": "available",
        }

    def test_exact_47_file_suite_and_command_coordinates(self) -> None:
        files = {path.stem for path in RUNNER.FIXTURE_ROOT.glob("*.json")}
        self.assertEqual(47, len(self.fixture_ids))
        self.assertEqual(self.fixture_ids, files)
        suites = [suite for suite in self.manifest["qualification"]["suites"] if suite["gate"] == "semantic-simulation"]
        self.assertEqual(47, len(suites))
        roster = RUNNER.manifest_simulation_roster(self.manifest)
        self.assertEqual(self.fixture_ids, set(roster))
        self.assertEqual(47, len({RUNNER.command_sha256(item["command"]) for item in roster.values()}))
        self.assertTrue(all(item["command"]["arguments"] == ["--fixture", identity] for identity, item in roster.items()))
        manifest_commands = {
            suite["coverage"][0]["fixtureIds"][0]: suite["command"] for suite in suites
        }
        self.assertEqual(
            {identity: item["command"] for identity, item in roster.items()},
            manifest_commands,
        )
        self.assertTrue(all(suite["availability"] == "available" and suite["unavailableReason"] is None for suite in suites))

    def test_all_records_have_exact_abi_complete_outputs_and_target(self) -> None:
        targets = {item["fixtureId"]: item["target"] for item in self.manifest["compilerFixtures"]}
        for identity in sorted(self.fixture_ids):
            with self.subTest(fixture=identity):
                record, payload, _oracle = RUNNER.load_fixture(self.manifest, identity)
                self.assertEqual(targets[identity], record["target"])
                self.assertEqual(record, json.loads(payload))
                self.assertEqual(len(record["physicalAbi"]["arguments"]), len(record["request"]["arguments"]))
                self.assertEqual(len(record["request"]["arguments"]), len(record["expectedArguments"]))
                self.assertTrue(record["oracle"]["function"])
                self.assertEqual("existing-rust-cpu-reference-v1", record["oracle"]["kind"])
                self.assertEqual("blocked", record["productionExport"]["status"])
                self.assertRegex(
                    record["productionExport"]["diagnosticCode"],
                    r"FE2O3-TUTORIAL-(?:PROBE|TXN)-[0-9]{3}",
                )
                self.assertEqual(
                    RUNNER.PRODUCER_STAGES[record["productionExport"]["diagnosticCode"]],
                    record["productionExport"]["stage"],
                )
                self.assertNotIn("rustc-codegen-fe2o3 cannot build", record["productionExport"]["diagnostic"])
                self.assertRegex(
                    record["productionExport"]["diagnosticIdentitySha256"],
                    r"[0-9a-f]{64}",
                )
                self.assertEqual(
                    RUNNER.PRODUCTION_EXPORT_IDENTITY_CONTRACT,
                    record["productionExport"]["identityContract"],
                )

    def test_separate_fill_wave_and_workgroup_cpu_suites(self) -> None:
        expected = {
            "gfx942-fill-simulation": "cpu-reference-fill",
            "gfx942-wave64-collectives": "cpu-reference-wave64-collectives",
            "gfx942-workgroup-collectives": "cpu-reference-workgroup-collectives",
        }
        for identity, suite_id in expected.items():
            suite = RUNNER._cpu_suite(self.manifest, identity)
            self.assertEqual(suite_id, suite["suiteId"])
            self.assertEqual("available", suite["availability"])
            self.assertEqual("scripts/run-tutorial-cpu-reference.sh", suite["command"]["executable"])

    def test_records_are_deterministic_and_checked_in_exactly(self) -> None:
        exports = {
            identity: json.loads((RUNNER.FIXTURE_ROOT / f"{identity}.json").read_bytes())["productionExport"]
            for identity in self.fixture_ids
        }
        first = GENERATOR.records(exports)
        second = GENERATOR.records(exports)
        self.assertEqual(first, second)
        self.assertEqual(self.fixture_ids, set(first))
        for identity, record in first.items():
            expected = json.dumps(record, ensure_ascii=True, indent=2, allow_nan=False).encode("ascii") + b"\n"
            self.assertEqual(expected, (RUNNER.FIXTURE_ROOT / f"{identity}.json").read_bytes())

    def test_generator_consumes_probe_results_without_hardcoded_blocker(self) -> None:
        diagnostic = "FE2O3-TUTORIAL-TXN-007: probe-derived test blocker"
        export = GENERATOR._blocked_export(
            f"fe2o3 tutorial production transaction: {diagnostic}\n".encode("ascii")
        )
        exports = {identity: copy.deepcopy(export) for identity in self.fixture_ids}
        with mock.patch.object(GENERATOR, "probe_production_exports", return_value=exports):
            generated = GENERATOR.records()
        self.assertEqual({diagnostic}, {item["productionExport"]["diagnostic"] for item in generated.values()})
        source = Path(GENERATOR.__file__).read_text(encoding="utf-8")
        self.assertNotIn("EXPORT_BLOCKER", source)
        self.assertNotIn("references missing encode_non_clean_witness", source)

    def test_build_failure_diagnostic_is_bounded_and_repository_relative(self) -> None:
        stderr = (
            f"Compiling fixture\n{GENERATOR.ROOT}/crates/example/src/lib.rs:7:9: "
            "error[E0425]: cannot find function `required_accessor` in this scope\n"
        ).encode("ascii")
        first = GENERATOR._build_failure_export(
            "FE2O3-TUTORIAL-PROBE-001", "production exporter", stderr
        )
        second = GENERATOR._build_failure_export(
            "FE2O3-TUTORIAL-PROBE-001", "production exporter", stderr
        )
        self.assertEqual(first, second)
        self.assertEqual("exporter-build", first["stage"])
        self.assertNotIn(str(GENERATOR.ROOT), first["diagnostic"])
        self.assertIn("required_accessor", first["diagnostic"])

    def test_rejects_stale_or_mismatched_producer_diagnostic(self) -> None:
        identity = "gfx942-fill-simulation"
        mutations = [
            lambda value: value["productionExport"].update(
                diagnostic="rustc-codegen-fe2o3 cannot build: obsolete W4 diagnostic",
                diagnosticCode=None,
            ),
            lambda value: value["productionExport"].__setitem__("stage", "publication"),
            lambda value: value["productionExport"].__setitem__(
                "diagnosticIdentitySha256", "0" * 64
            ),
        ]
        patterns = (
            "exact production export blocker",
            "exact production export blocker",
            "production diagnostic identity is stale",
        )
        for mutate, pattern in zip(mutations, patterns):
            temporary = self.mutated_root(identity, mutate)
            with temporary, self.assertRaisesRegex(
                RUNNER.SimulationQualificationError,
                pattern,
            ):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_one_field_diagnostic_and_identity_mutations(self) -> None:
        identity = "gfx942-fill-simulation"
        mutations = (
            lambda value: value["productionExport"].__setitem__(
                "diagnostic", value["productionExport"]["diagnostic"] + " substituted"
            ),
            lambda value: value["productionExport"].__setitem__(
                "identityContract",
                {
                    **value["productionExport"]["identityContract"],
                    "rawContentField": "substituted",
                },
            ),
            lambda value: value.__setitem__("compilerInputContractSha256", "0" * 64),
            lambda value: value.__setitem__("sourceClosureSha256", "0" * 64),
        )
        patterns = (
            "production diagnostic identity is stale",
            "conflates canonical KIR and raw content identities",
            "stale or substituted",
            "stale or substituted",
        )
        for mutate, pattern in zip(mutations, patterns):
            temporary = self.mutated_root(identity, mutate)
            with temporary, self.assertRaisesRegex(
                RUNNER.SimulationQualificationError, pattern
            ):
                self.load_from(Path(temporary.name), identity)

    def test_v13_canonical_identity_and_raw_content_hash_are_distinct_domains(self) -> None:
        identity = "gfx942-fill-simulation"
        target = "gfx942"
        contract = RUNNER.PRODUCTION_EXPORT_IDENTITY_CONTRACT
        self.assertEqual(
            [
                "productionEvidence.finalOptimizedKirSha256",
                "graph.productionKirIdentitySha256",
                "simulator.subjectSha256",
            ],
            contract["verifiedCanonicalKernelIrV13Fields"],
        )
        self.assertEqual("evidenceFiles[optimized-kir-v13].sha256", contract["rawContentField"])
        temporary = self.mutated_root(
            identity,
            lambda value: value.__setitem__("productionExport", self.available_export(target)),
        )
        with temporary:
            self.load_from(Path(temporary.name), identity)

        mutations = [
            lambda export: export["coordinates"].__setitem__("productionKirIdentitySha256", "6" * 64),
            lambda export: export["coordinates"].__setitem__("optimizedKirV13ContentSha256", "1" * 64),
        ]
        patterns = ("inconsistent VerifiedCanonicalKernelIrV13", "conflates canonical KIR and raw content")
        for mutate, pattern in zip(mutations, patterns):
            def replace(value, mutate=mutate):
                export = self.available_export(target)
                mutate(export)
                value["productionExport"] = export

            temporary = self.mutated_root(identity, replace)
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, pattern):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_stale_source_and_contract_identities(self) -> None:
        identity = "gfx942-fill-simulation"
        for field in ("sourceClosureSha256", "compilerInputContractSha256"):
            temporary = self.mutated_root(identity, lambda value, field=field: value.__setitem__(field, "0" * 64))
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, "stale or substituted"):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_cross_fixture_and_cross_target_records(self) -> None:
        identity = "gfx942-fill-simulation"
        for field, replacement in (("fixtureId", "gfx950-fp4-gemm"), ("target", "gfx950")):
            temporary = self.mutated_root(identity, lambda value, field=field, replacement=replacement: value.__setitem__(field, replacement))
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, "stale or substituted"):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_abi_type_count_and_signature_mutations(self) -> None:
        identity = "gfx942-wave64-collectives"
        mutations = [
            lambda value: value["physicalAbi"]["arguments"].pop(),
            lambda value: value["physicalAbi"]["arguments"][1].__setitem__("type", "u32"),
            lambda value: value["physicalAbi"].__setitem__("signatureSha256", "0" * 64),
        ]
        patterns = ("argument count", "differs from its ABI", "signature is stale")
        for mutate, pattern in zip(mutations, patterns):
            temporary = self.mutated_root(identity, mutate)
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, pattern):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_incomplete_output_and_immutable_input_mutation(self) -> None:
        identity = "gfx942-typed-vecadd-source"
        mutations = [
            lambda value: value["expectedArguments"][2].__setitem__("bytes", "0x00"),
            lambda value: value["expectedArguments"][0].__setitem__(
                "bytes", "0x00000000" + value["expectedArguments"][0]["bytes"][10:]
            ),
        ]
        patterns = ("expected extent is incomplete", "immutable argument")
        for mutate, pattern in zip(mutations, patterns):
            temporary = self.mutated_root(identity, mutate)
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, pattern):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_missing_oracle_and_stale_oracle_input(self) -> None:
        identity = "gfx942-fill-simulation"
        mutations = [
            lambda value: value["oracle"].__setitem__("function", ""),
            lambda value: value["oracle"].__setitem__("inputSha256", "0" * 64),
        ]
        patterns = ("no CPU reference function", "oracle input identity is stale")
        for mutate, pattern in zip(mutations, patterns):
            temporary = self.mutated_root(identity, mutate)
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, pattern):
                self.load_from(Path(temporary.name), identity)

    def test_rejects_missing_canary_or_padding_policy(self) -> None:
        identities = ("gfx942-fill-simulation", "gfx942-row-softmax")
        labels = ("canaries", "padding")
        for identity, label in zip(identities, labels):
            temporary = self.mutated_root(identity, lambda value, label=label: value[label].clear())
            with temporary, self.assertRaisesRegex(RUNNER.SimulationQualificationError, f"{label} check is incomplete"):
                self.load_from(Path(temporary.name), identity)

    def test_manifest_migration_is_idempotent_and_fail_closed(self) -> None:
        once = UPDATER.migrated(self.manifest)
        twice = UPDATER.migrated(once)
        self.assertEqual(once, twice)
        self.assertEqual(self.manifest, once)
        self.assertEqual(0, sum(kernel["simulatorCommand"]["status"] == "capability-path-qualified" for kernel in once["capabilityKernels"]))
        self.assertTrue(all(kernel["simulatorCommand"]["status"] == "unavailable" for kernel in once["capabilityKernels"]))


if __name__ == "__main__":
    unittest.main()
