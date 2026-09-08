#!/usr/bin/env python3

from __future__ import annotations

from copy import deepcopy
import importlib.util
from pathlib import Path
import sys
import unittest


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import tutorial_kernel_manifest as MANIFEST
import tutorial_negative_fixtures as NEGATIVE


def documents() -> tuple[dict, dict, NEGATIVE.ValidatedDeclarations]:
    _, manifest = MANIFEST._load_json_unique(
        ROOT / "config" / MANIFEST.MANIFEST_NAME
    )
    declaration, validated = NEGATIVE.load_declarations(ROOT, manifest)
    return manifest, declaration, validated


def prerequisite_results(
    validated: NEGATIVE.ValidatedDeclarations,
) -> list[dict]:
    results = []
    for identity in sorted(validated.shared_prerequisites):
        prerequisite = validated.shared_prerequisites[identity]
        expected = prerequisite["expectedObservation"]
        results.append(
            {
                "commandSha256": NEGATIVE.command_sha256(prerequisite["command"]),
                "exitStatus": expected["exitStatus"],
                "prerequisiteId": identity,
                "stderrBytes": expected["stderrBytes"],
                "stderrSha256": expected["stderrSha256"],
                "stdoutBytes": expected["stdoutBytes"],
                "stdoutSha256": expected["stdoutSha256"],
                "status": "passed",
            }
        )
    return results


def synthetic_evidence(
    manifest: dict,
    declaration: dict,
    validated: NEGATIVE.ValidatedDeclarations,
) -> dict:
    fixture_results = NEGATIVE.execute_fixture_cases(ROOT, manifest, validated)
    return NEGATIVE.build_evidence(
        declaration,
        validated,
        prerequisite_results(validated),
        fixture_results,
    )


class TutorialNegativeFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest, cls.declaration, cls.validated = documents()
        cls.evidence = synthetic_evidence(
            cls.manifest, cls.declaration, cls.validated
        )

    def validate_declaration(self, declaration: dict) -> None:
        NEGATIVE.validate_declarations(
            ROOT, self.manifest, declaration, self.validated.raw_sha256
        )

    def test_shared_prerequisites_do_not_satisfy_fixture_cases(self) -> None:
        case_count = sum(
            len(fixture["cases"])
            for fixture in self.validated.fixtures.values()
        )
        self.assertEqual(47, len(self.validated.fixtures))
        self.assertEqual(7, len(self.validated.shared_prerequisites))
        self.assertGreater(case_count, 642)
        self.assertEqual(case_count, len(self.evidence["fixtureResults"]))
        self.assertEqual(case_count, len(self.evidence["cases"]))
        self.assertEqual("unavailable", self.evidence["status"])
        self.assertNotIn("executionId", self.validated.fixtures[next(iter(self.validated.fixtures))]["cases"][0])

    def test_every_case_binds_exact_fixture_property_mutation_and_boundary(self) -> None:
        kernels = {
            item["fixtureId"]: item for item in self.manifest["capabilityKernels"]
        }
        mutation_ids = set()
        for fixture_id, fixture in self.validated.fixtures.items():
            kernel = kernels[fixture_id]
            self.assertEqual(kernel["kernelSymbol"], fixture["kernelSymbol"])
            for case in fixture["cases"]:
                self.assertEqual(fixture_id, case["fixtureId"])
                self.assertEqual(fixture["target"], case["target"])
                self.assertIn(case["requiredProperty"], kernel["requiredProperties"])
                self.assertEqual(case["failureStage"], case["productionBoundary"])
                self.assertEqual(fixture["bindingSha256"], case["fixtureBindingSha256"])
                self.assertNotEqual(case["basePayloadSha256"], case["mutatedPayloadSha256"])
                self.assertNotIn(case["mutatedPayloadSha256"], mutation_ids)
                mutation_ids.add(case["mutatedPayloadSha256"])
                self.assertEqual(NEGATIVE.UNAVAILABLE_RESULT, case["expectedResult"])

    def test_exact_required_categories_and_advanced_functional_mutations(self) -> None:
        kernels = {
            item["fixtureId"]: item for item in self.manifest["capabilityKernels"]
        }
        advanced = 0
        expected_variants = {
            "functional-numerical",
            "functional-order",
            "functional-output",
            "functional-recurrence",
        }
        for fixture_id, fixture in self.validated.fixtures.items():
            kernel = kernels[fixture_id]
            categories = {case["category"] for case in fixture["cases"]}
            self.assertEqual(
                MANIFEST._required_negative_categories(
                    set(kernel["requiredProperties"])
                ),
                categories,
            )
            family = self._advanced_family(fixture_id, kernel["kernelSymbol"])
            if family is not None and "functional-refinement" in kernel["requiredProperties"]:
                advanced += 1
                classes = {case["mutation"]["class"] for case in fixture["cases"]}
                self.assertTrue(
                    {f"{family}-{variant}" for variant in expected_variants}.issubset(classes)
                )
        self.assertGreater(advanced, 0)

    @staticmethod
    def _advanced_family(fixture_id: str, symbol: str) -> str | None:
        identity = f"{fixture_id}:{symbol}"
        for family, tokens in (
            ("kda", ("kda",)),
            ("gemm", ("gemm",)),
            ("attention", ("attention", "attn", "flash")),
            ("moe", ("moe", "expert", "gpt-oss")),
        ):
            if any(token in identity for token in tokens):
                return family
        return None

    def test_generator_is_deterministic(self) -> None:
        path = ROOT / "scripts" / "update-tutorial-negative-fixture-declarations.py"
        spec = importlib.util.spec_from_file_location("negative_updater", path)
        assert spec is not None and spec.loader is not None
        updater = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(updater)
        self.assertEqual(self.declaration, updater.generated(self.manifest))

    def test_rejects_missing_required_category_and_stale_payload(self) -> None:
        missing = deepcopy(self.declaration)
        fixture = missing["fixtures"][0]
        removed_category = fixture["cases"][0]["category"]
        fixture["cases"] = [
            case for case in fixture["cases"] if case["category"] != removed_category
        ]
        with self.assertRaisesRegex(NEGATIVE.NegativeFixtureError, "coverage differs"):
            self.validate_declaration(missing)

        stale = deepcopy(self.declaration)
        stale["fixtures"][0]["cases"][0]["mutatedPayloadSha256"] = "f" * 64
        with self.assertRaisesRegex(NEGATIVE.NegativeFixtureError, "payload is stale"):
            self.validate_declaration(stale)

    def test_two_fixtures_cannot_share_payload_or_execution_result(self) -> None:
        first = self.evidence["fixtureResults"][0]
        second = next(
            result
            for result in self.evidence["fixtureResults"]
            if result["fixtureId"] != first["fixtureId"]
        )
        self.assertNotEqual(first["mutatedPayloadSha256"], second["mutatedPayloadSha256"])
        self.assertNotEqual(first["caseExecutionSha256"], second["caseExecutionSha256"])

        crossed = deepcopy(self.evidence)
        target = next(
            result
            for result in crossed["fixtureResults"]
            if result["fixtureId"] != crossed["fixtureResults"][0]["fixtureId"]
        )
        target["caseExecutionSha256"] = crossed["fixtureResults"][0]["caseExecutionSha256"]
        with self.assertRaisesRegex(NEGATIVE.NegativeFixtureError, "stale execution identity"):
            NEGATIVE.validate_evidence(self.declaration, self.validated, crossed)

    def test_rejects_missing_duplicate_and_cross_fixture_results(self) -> None:
        mutations = (
            lambda value: value.pop(),
            lambda value: value.append(deepcopy(value[0])),
            lambda value: value[0].update(fixtureId="crossed-fixture"),
        )
        for mutate in mutations:
            evidence = deepcopy(self.evidence)
            mutate(evidence["fixtureResults"])
            with self.assertRaisesRegex(
                NEGATIVE.NegativeFixtureError,
                "missing, duplicated, or cross-fixture|duplicated|stale or substituted",
            ):
                NEGATIVE.validate_evidence(self.declaration, self.validated, evidence)

    def test_promotion_fails_closed_while_production_lifecycle_is_unavailable(self) -> None:
        with self.assertRaisesRegex(
            NEGATIVE.NegativeFixtureError,
            "production lifecycle is unavailable",
        ):
            NEGATIVE.validate_promotion_records(
                ROOT, self.manifest, [], self.evidence
            )


if __name__ == "__main__":
    unittest.main()
