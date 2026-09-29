#!/usr/bin/env python3
"""Check the frozen #272 M0 inventory, never execution or approval authority."""

from __future__ import annotations

import copy
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "config/issue272-m0-baseline-v1.json"
MANIFEST = ROOT / "config/tutorial-kernel-manifest-v1.json"
MAX_BASELINE_BYTES = 128 * 1024
BASIS = {
    "compilerCommit": "91d4b0a2675f5f8b4449768ae89060c06f83689e",
    "compilerTree": "7812f802abbd265d10d8dc5e24308692b9fba301",
    "sourceManifestPath": "config/tutorial-kernel-manifest-v1.json",
    "sourceManifestSha256": "faa743757f5b6da90e1578394982e0d99b3e347713f868f2bf140549a437deff",
    "original47ObligationsSha256": "db1d9a0d5c5aca3b9c76a4713ddd3b22417e41ee24667efae11773dfccdc4215",
}
ADDITIONS = [
    "gfx942-scalar-gemm",
    "gfx950-gpt-oss-pipelined-attention",
    "gfx950-gpt-oss-scalar-attention",
]
HEADER = {
    "schema": "fe2o3-issue272-m0-baseline-v1",
    "roadmapIssue": "https://github.com/harsh-nod/fe2o3/issues/272",
    "basis": BASIS,
    "status": "unqualified-observation-baseline",
    "observationScope": "no-execution-performed-for-this-baseline-audit",
    "productionLane": "single-protected-production-transaction",
    "classificationMeaning": "source-obligation-not-compilation-or-execution-success",
    "productionMeaning": "not-run-in-this-baseline-audit-not-a-claim-about-historical-runs",
    "hardwareMeaning": "no-target-matched-baseline-receipt-not-physical-host-unavailability",
    "additions": ADDITIONS,
}
ROW_KEYS = {
    "fixtureId", "cohort", "target", "compilerInputSha256", "sourceEntryIds",
    "classification", "requiredGates", "production", "simulator", "hardware",
}


def load_source_contract_tools():
    # Reuse both the strict JSON parser and the established original47 projection.
    path = ROOT / "scripts/tests/tutorial_kernel_manifest.py"
    spec = importlib.util.spec_from_file_location("issue272_source_contract_tests", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module, module.load_validator()


SOURCE_CONTRACT, PARENT = load_source_contract_tools()


def require_equal(actual, expected, label):
    if actual != expected:
        PARENT.fail(f"M0 {label} differs from the frozen baseline")


def validate_baseline(baseline, manifest, manifest_sha256):
    """Check descriptive consistency only; no subprocess or current-source claim."""
    PARENT.require_object(baseline, "M0 baseline")
    PARENT.require_exact_keys(baseline, set(HEADER) | {"selections"}, "M0 baseline")
    for key, expected in HEADER.items():
        require_equal(baseline[key], expected, key)
    require_equal(manifest_sha256, BASIS["sourceManifestSha256"], "manifest bytes")
    require_equal(manifest["baseline"], {
        "compilerCommit": None,
        "compilerTree": None,
        "status": "unqualified-source-contract",
    }, "source-contract baseline")

    original = json.dumps(
        SOURCE_CONTRACT.recovered_scope(manifest),
        sort_keys=True, separators=(",", ":"), ensure_ascii=True,
    ).encode("ascii")
    require_equal(hashlib.sha256(original).hexdigest(),
                  BASIS["original47ObligationsSha256"], "original47 obligations")

    fixtures = {row["fixtureId"]: row for row in manifest["compilerFixtures"]}
    require_equal(len(fixtures), len(manifest["compilerFixtures"]), "unique source fixtures")
    require_equal(len(fixtures), 50, "source fixture count")
    rows = baseline["selections"]
    if not isinstance(rows, list) or len(rows) != 50:
        PARENT.fail("M0 selections must contain the complete 50-row roster")
    seen = set()
    cohorts = Counter()
    for row in rows:
        PARENT.require_object(row, "M0 selection")
        PARENT.require_exact_keys(row, ROW_KEYS, "M0 selection")
        fixture_id = PARENT.require_string(row["fixtureId"], "M0 fixtureId")
        if fixture_id in seen or fixture_id not in fixtures:
            PARENT.fail("M0 duplicate or foreign fixtureId")
        seen.add(fixture_id)
        fixture = fixtures[fixture_id]
        entries = [entry for entry in manifest["entries"]
                   if fixture_id in entry["compilerFixtureIds"]]
        require_equal({entry["classification"] for entry in entries},
                      {"compiler-produced"}, f"{fixture_id} source classification")
        cohort = "addition" if fixture_id in ADDITIONS else "original47"
        expected = {
            "fixtureId": fixture_id,
            "cohort": cohort,
            "target": fixture["target"],
            "compilerInputSha256": PARENT.fixture_input_contract_sha256(fixture),
            "sourceEntryIds": sorted(entry["lessonId"] for entry in entries),
            "classification": "compiler-produced",
            "requiredGates": sorted({gate for entry in entries for gate in entry["requiredGates"]}),
            "production": {"status": "not-run", "evidence": None},
            "simulator": {
                "status": "pending", "sourceContractStatus": fixture["simulation"]["status"],
                "evidence": None,
            },
            "hardware": {"status": "unavailable", "evidence": None},
        }
        require_equal(fixture["compilerInput"]["contractSha256"],
                      expected["compilerInputSha256"], f"{fixture_id} source input digest")
        for key, value in expected.items():
            require_equal(row[key], value, f"{fixture_id} {key}")
        cohorts[(cohort, fixture["target"])] += 1
    require_equal(seen, set(fixtures), "complete selection roster")
    require_equal(cohorts, {
        ("original47", "gfx942"): 10,
        ("original47", "gfx950"): 37,
        ("addition", "gfx942"): 1,
        ("addition", "gfx950"): 2,
    }, "cohort target counts")


class Issue272M0ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest, cls.manifest_sha256 = PARENT.load_manifest(MANIFEST, with_sha256=True)
        cls.baseline = PARENT.load_manifest(BASELINE, MAX_BASELINE_BYTES)

    def check(self, baseline=None, manifest=None, digest=None):
        validate_baseline(
            self.baseline if baseline is None else baseline,
            self.manifest if manifest is None else manifest,
            self.manifest_sha256 if digest is None else digest,
        )

    def test_pinned_roster_preserves_original47_and_three_additions_without_promotion(self):
        original = copy.deepcopy((self.baseline, self.manifest))
        self.check()
        self.assertEqual((self.baseline, self.manifest), original)
        self.assertEqual(len(self.baseline["selections"]), 50)

    def test_missing_duplicate_and_foreign_selections_reject(self):
        for change in ("missing", "duplicate", "foreign"):
            baseline = copy.deepcopy(self.baseline)
            rows = baseline["selections"]
            if change == "missing":
                rows.pop()
            elif change == "duplicate":
                rows[-1] = copy.deepcopy(rows[0])
            else:
                rows[-1]["fixtureId"] = "invented-kernel"
            with self.subTest(change=change), self.assertRaises(SystemExit):
                self.check(baseline)

    def test_reordered_selections_preserve_the_same_inventory(self):
        baseline = copy.deepcopy(self.baseline)
        baseline["selections"].reverse()
        self.check(baseline)

    def test_selection_binding_classification_and_gates_cannot_be_substituted(self):
        for key, value in (
            ("cohort", "addition"), ("target", "gfx950"),
            ("compilerInputSha256", "0" * 64), ("sourceEntryIds", ["typed-vecadd"]),
            ("classification", "simulator-only"), ("requiredGates", ["production-compile"]),
        ):
            baseline = copy.deepcopy(self.baseline)
            baseline["selections"][0][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, key):
                self.check(baseline)

    def test_source_feature_or_semantic_obligation_change_rejects(self):
        for change in ("feature", "simulation", "policy"):
            manifest = copy.deepcopy(self.manifest)
            if change == "feature":
                manifest["compilerFixtures"][0]["compilerInput"]["features"] = ["invented"]
            elif change == "simulation":
                manifest["compilerFixtures"][0]["simulation"]["canonicalKirVersion"] = 15
            else:
                manifest["productionContract"]["allowsFallback"] = True
            with self.subTest(change=change), self.assertRaisesRegex(SystemExit, "original47 obligations"):
                self.check(manifest=manifest)

    def test_pins_and_addition_membership_cannot_be_relabelled(self):
        for key in BASIS:
            baseline = copy.deepcopy(self.baseline)
            baseline["basis"][key] = "changed"
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, "basis"):
                self.check(baseline)
        baseline = copy.deepcopy(self.baseline)
        baseline["additions"][0] = baseline["selections"][0]["fixtureId"]
        with self.assertRaisesRegex(SystemExit, "additions"):
            self.check(baseline)
        with self.assertRaisesRegex(SystemExit, "manifest bytes"):
            self.check(digest="0" * 64)

    def test_source_contract_null_baseline_is_not_reinterpreted(self):
        for key in ("compilerCommit", "compilerTree", "status"):
            manifest = copy.deepcopy(self.manifest)
            manifest["baseline"][key] = "approved"
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, "source-contract baseline"):
                self.check(manifest=manifest)

    def test_unevidenced_refusals_and_all_success_claims_reject(self):
        for stage, statuses in (
            ("production", ("refused", "passed", "qualified", "not-recorded")),
            ("simulator", ("passed", "qualified")),
            ("hardware", ("passed", "gpu-observed", "qualified")),
        ):
            for status in statuses:
                baseline = copy.deepcopy(self.baseline)
                baseline["selections"][0][stage]["status"] = status
                with self.subTest(stage=stage, status=status), self.assertRaisesRegex(SystemExit, stage):
                    self.check(baseline)

    def test_fabricated_evidence_and_historical_relabelling_reject(self):
        for stage in ("production", "simulator", "hardware"):
            baseline = copy.deepcopy(self.baseline)
            baseline["selections"][0][stage]["evidence"] = {
                "compilerCommit": BASIS["compilerCommit"], "claimedResult": "historical-pass",
            }
            with self.subTest(stage=stage), self.assertRaisesRegex(SystemExit, stage):
                self.check(baseline)
        baseline = copy.deepcopy(self.baseline)
        baseline["observationScope"] = "current-production-qualified"
        with self.assertRaisesRegex(SystemExit, "observationScope"):
            self.check(baseline)

    def test_unknown_fields_missing_fields_and_wrong_shapes_reject(self):
        for scope in ("baseline", "row", "observation"):
            for operation in ("add", "remove"):
                baseline = copy.deepcopy(self.baseline)
                target = baseline if scope == "baseline" else baseline["selections"][0]
                if scope == "observation":
                    target = target["production"]
                if operation == "add":
                    target["qualified"] = True
                else:
                    del target[next(iter(target))]
                with self.subTest(scope=scope, operation=operation), self.assertRaises(SystemExit):
                    self.check(baseline)
        for value in (None, {}, "50"):
            baseline = copy.deepcopy(self.baseline)
            baseline["selections"] = value
            with self.subTest(value=value), self.assertRaisesRegex(SystemExit, "50-row"):
                self.check(baseline)

    def test_strict_existing_parser_rejects_duplicate_and_nonfinite_json(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-m0-json-") as temporary:
            path = Path(temporary) / "baseline.json"
            for raw in ('{"status": "not-run", "status": "passed"}', '{"value": NaN}',
                        '{"value": 1e999}', '{"broken":'):
                path.write_text(raw, encoding="utf-8")
                with self.subTest(raw=raw), self.assertRaises(SystemExit):
                    PARENT.load_manifest(path, MAX_BASELINE_BYTES)

    def test_existing_parser_enforces_bounded_regular_file(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-m0-bound-") as temporary:
            path = Path(temporary) / "baseline.json"
            path.write_text(" " * (MAX_BASELINE_BYTES + 1), encoding="ascii")
            with self.assertRaisesRegex(SystemExit, "bounded regular file"):
                PARENT.load_manifest(path, MAX_BASELINE_BYTES)
            link = Path(temporary) / "alias.json"
            link.symlink_to(BASELINE)
            with self.assertRaisesRegex(SystemExit, "bounded regular file"):
                PARENT.load_manifest(link, MAX_BASELINE_BYTES)


if __name__ == "__main__":
    unittest.main()
