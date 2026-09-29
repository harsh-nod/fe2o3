#!/usr/bin/env python3
"""Check an immutable M0 snapshot and current continuity, never adoption or execution."""

from __future__ import annotations

import copy
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "config/issue272-m0-baseline-v1.json"
MANIFEST = ROOT / "config/tutorial-kernel-manifest-v1.json"
MAX_BASELINE_BYTES = 128 * 1024
# Canonical JSON of the historical baseline itself, not the evolving source manifest.
BASELINE_SHA256 = "d0390b1abd025eced0b9c8efe0a74e08c3b9799ad9cab928c7b18ac2fbebee9a"


def load_source_contract_tools():
    path = ROOT / "scripts/tests/tutorial_kernel_manifest.py"
    spec = importlib.util.spec_from_file_location("issue272_source_contract_tests", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module, module.load_validator()


SOURCE_CONTRACT, PARENT = load_source_contract_tools()


def digest(value):
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=True, allow_nan=False).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def require_equal(actual, expected, label):
    if actual != expected:
        PARENT.fail(f"M0 {label} differs")


def validate_history(baseline):
    """Content consistency of this documentary snapshot, not signed provenance."""
    PARENT.require_object(baseline, "M0 baseline")
    require_equal(digest(baseline), BASELINE_SHA256, "historical baseline content")


def validate_current_subset(baseline, manifest):
    """Retain historical selection meanings without freezing current inputs/results."""
    validate_history(baseline)
    fixtures = {row["fixtureId"]: row for row in manifest["compilerFixtures"]}
    require_equal(len(fixtures), len(manifest["compilerFixtures"]), "unique current fixtures")
    entries = {entry["lessonId"]: entry for entry in manifest["entries"]}
    for row in baseline["selections"]:
        fixture_id = row["fixtureId"]
        if fixture_id not in fixtures:
            PARENT.fail(f"M0 historical selection missing from current manifest: {fixture_id}")
        require_equal(fixtures[fixture_id]["target"], row["target"], f"{fixture_id} target")
        for lesson_id in row["sourceEntryIds"]:
            entry = entries.get(lesson_id)
            if entry is None or fixture_id not in entry["compilerFixtureIds"]:
                PARENT.fail(f"M0 historical source association missing: {fixture_id}/{lesson_id}")
            require_equal(entry["classification"], row["classification"],
                          f"{fixture_id}/{lesson_id} classification")
    # Input digests, commands, status, gates and total count belong to the current
    # manifest's existing validator. They never rewrite this historical observation.


def validate_pinned_source(baseline, manifest, manifest_sha256):
    """Optional offline audit of an explicitly supplied historical manifest file."""
    validate_history(baseline)
    basis = baseline["basis"]
    require_equal(manifest_sha256, basis["sourceManifestSha256"], "pinned manifest bytes")
    require_equal(digest(SOURCE_CONTRACT.recovered_scope(manifest)),
                  basis["original47ObligationsSha256"], "pinned original47 obligations")
    validate_current_subset(baseline, manifest)
    fixtures = {row["fixtureId"]: row for row in manifest["compilerFixtures"]}
    require_equal(set(fixtures), {row["fixtureId"] for row in baseline["selections"]},
                  "pinned selection roster")
    for row in baseline["selections"]:
        fixture = fixtures[row["fixtureId"]]
        entries = [entry for entry in manifest["entries"]
                   if row["fixtureId"] in entry["compilerFixtureIds"]]
        expected = {
            "compilerInputSha256": PARENT.fixture_input_contract_sha256(fixture),
            "sourceEntryIds": sorted(entry["lessonId"] for entry in entries),
            "requiredGates": sorted({gate for entry in entries for gate in entry["requiredGates"]}),
        }
        for key, value in expected.items():
            require_equal(row[key], value, f"pinned {row['fixtureId']} {key}")
        require_equal(row["simulator"]["sourceContractStatus"], fixture["simulation"]["status"],
                      f"pinned {row['fixtureId']} simulation obligation")


class Issue272M0ContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = PARENT.load_manifest(MANIFEST)
        cls.baseline = PARENT.load_manifest(BASELINE, MAX_BASELINE_BYTES)

    def test_historical_original47_and_three_additions_remain_unchanged(self):
        before = copy.deepcopy((self.baseline, self.manifest))
        validate_current_subset(self.baseline, self.manifest)
        self.assertEqual(Counter((r["cohort"], r["target"]) for r in self.baseline["selections"]), {
            ("original47", "gfx942"): 10, ("original47", "gfx950"): 37,
            ("addition", "gfx942"): 1, ("addition", "gfx950"): 2,
        })
        self.assertEqual((self.baseline, self.manifest), before)

    def test_future_fixture_does_not_rewrite_or_shrink_historical_baseline(self):
        before = copy.deepcopy(self.baseline)
        manifest = copy.deepcopy(self.manifest)
        fixture = copy.deepcopy(manifest["compilerFixtures"][0])
        fixture["fixtureId"] = "gfx942-future-m0-comparison-fixture"
        fixture["compilerInput"]["contractSha256"] = PARENT.fixture_input_contract_sha256(fixture)
        manifest["compilerFixtures"].append(fixture)
        entry = copy.deepcopy(manifest["entries"][0])
        entry.update(lessonId="future-m0-comparison", compilerFixtureIds=[fixture["fixtureId"]])
        manifest["entries"].append(entry)
        # A comparison fixture only; the existing source validator still owns
        # real source/feature/kernel validation, not this synthetic extension.
        validate_current_subset(self.baseline, manifest)
        self.assertEqual(len(manifest["compilerFixtures"]), len(self.manifest["compilerFixtures"]) + 1)
        self.assertEqual(self.baseline, before)
        self.assertEqual(len(self.baseline["selections"]), 50)

    def test_current_input_and_status_updates_do_not_relabel_old_observations(self):
        manifest = copy.deepcopy(self.manifest)
        fixture_id = self.baseline["selections"][0]["fixtureId"]
        fixture = next(row for row in manifest["compilerFixtures"] if row["fixtureId"] == fixture_id)
        fixture["compilerInput"]["sourceClosureSha256"] = "1" * 64
        fixture["compilerInput"]["contractSha256"] = PARENT.fixture_input_contract_sha256(fixture)
        fixture["simulation"]["status"] = "future-observed-status"
        manifest["qualification"]["status"] = "future-observed-status"
        # These exercise only M0 continuity, not current schema admission or
        # genuine execution. Future schema/result changes retain their own owners.
        validate_current_subset(self.baseline, manifest)
        for row in self.baseline["selections"]:
            self.assertEqual(row["production"], {"status": "not-run", "evidence": None})
            self.assertEqual(row["simulator"]["status"], "pending")
            self.assertEqual(row["hardware"], {"status": "unavailable", "evidence": None})

    def test_current_reordering_and_extra_metadata_are_not_historical_mutations(self):
        manifest = copy.deepcopy(self.manifest)
        manifest["compilerFixtures"].reverse()
        manifest["entries"].reverse()
        manifest["futureMetadata"] = {"recorded": True}
        validate_current_subset(self.baseline, manifest)

    def test_current_selection_loss_retargeting_and_downgrade_require_review(self):
        fixture_id = self.baseline["selections"][0]["fixtureId"]
        for change in ("missing", "target", "classification", "association"):
            manifest = copy.deepcopy(self.manifest)
            fixture = next(row for row in manifest["compilerFixtures"] if row["fixtureId"] == fixture_id)
            entry = next(row for row in manifest["entries"]
                         if row["lessonId"] == self.baseline["selections"][0]["sourceEntryIds"][0])
            if change == "missing":
                manifest["compilerFixtures"].remove(fixture)
            elif change == "target":
                fixture["target"] = "foreign-target"
            elif change == "classification":
                entry["classification"] = "design-only"
            else:
                entry["compilerFixtureIds"].remove(fixture_id)
            with self.subTest(change=change), self.assertRaises(SystemExit):
                validate_current_subset(self.baseline, manifest)

    def test_changed_historical_pins_membership_and_obligations_reject(self):
        for key in self.baseline["basis"]:
            baseline = copy.deepcopy(self.baseline)
            baseline["basis"][key] = "changed"
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, "historical baseline"):
                validate_history(baseline)
        for key in ("fixtureId", "cohort", "target", "compilerInputSha256",
                    "sourceEntryIds", "classification", "requiredGates"):
            baseline = copy.deepcopy(self.baseline)
            baseline["selections"][0][key] = "changed"
            with self.subTest(key=key), self.assertRaisesRegex(SystemExit, "historical baseline"):
                validate_history(baseline)
        baseline = copy.deepcopy(self.baseline)
        baseline["selections"].pop()
        with self.assertRaises(SystemExit):
            validate_history(baseline)

    def test_fake_historical_refusals_results_and_evidence_reject(self):
        for stage in ("production", "simulator", "hardware"):
            for change, value in (("status", "refused"), ("status", "passed"),
                                  ("evidence", {"claimedResult": "passed"})):
                baseline = copy.deepcopy(self.baseline)
                baseline["selections"][0][stage][change] = value
                with self.subTest(stage=stage, change=change, value=value), self.assertRaises(SystemExit):
                    validate_history(baseline)

    def test_pinned_source_check_is_separate_and_requires_exact_archived_bytes(self):
        with self.assertRaisesRegex(SystemExit, "pinned manifest bytes"):
            validate_pinned_source(self.baseline, self.manifest, "0" * 64)

    def test_strict_existing_parser_rejects_duplicate_nonfinite_and_oversized_json(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-m0-json-") as temporary:
            path = Path(temporary) / "baseline.json"
            for raw in ('{"status": "not-run", "status": "passed"}', '{"value": NaN}',
                        '{"value": 1e999}', '{"broken":', " " * (MAX_BASELINE_BYTES + 1)):
                path.write_text(raw, encoding="utf-8")
                with self.subTest(raw=raw[:80]), self.assertRaises(SystemExit):
                    PARENT.load_manifest(path, MAX_BASELINE_BYTES)
            link = Path(temporary) / "alias.json"
            link.symlink_to(BASELINE)
            with self.assertRaises(SystemExit):
                PARENT.load_manifest(link, MAX_BASELINE_BYTES)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--pinned-manifest"]:
        if len(sys.argv) != 3:
            PARENT.fail("usage: issue272_m0_contract.py --pinned-manifest ARCHIVED_MANIFEST")
        baseline = PARENT.load_manifest(BASELINE, MAX_BASELINE_BYTES)
        manifest, manifest_sha256 = PARENT.load_manifest(Path(sys.argv[2]), with_sha256=True)
        validate_pinned_source(baseline, manifest, manifest_sha256)
        print("M0 historical source correspondence checked; no execution or adoption certified")
    else:
        unittest.main()
