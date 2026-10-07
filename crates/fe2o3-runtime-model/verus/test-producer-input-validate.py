#!/usr/bin/env python3
"""Source-only calibration; constructing mutants never qualifies them."""
from pathlib import Path
import types
import unittest

path = Path(__file__).with_name("check-producer-input-validate.py")
check = types.ModuleType("producer_input_validate_controls")
check.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)


class Calibration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = check.snapshot()

    def test_exact_production_closure_and_schemas(self):
        check.audit(self.sources)
        self.assertEqual(check.FILES, [check.PROOF, *check.PARTS, check.BODY, check.LIVE_BODY])
        self.assertEqual(len(check.FILES), 7)
        self.assertNotIn("producer_input_fold_body!(", self.sources[check.DEFINITIONS])
        self.assertIn("producer_input_validate_body!(", self.sources[check.DEFINITIONS])
        check.controller_source()

    def test_complete_fields_and_native_forwarders(self):
        for old, new in (
            ("content_lineage: u64", "content_lineage: u32"),
            ("backend_submission: u64", "backend_submission: u32"),
            ("journal: Option<ContextAllocationReferenceV1>", "journal: Option<u64>"),
            ("enum RuntimeAccessV1 { Read, Write, ReadWrite }", "enum RuntimeAccessV1 { Read, Write }"),
        ):
            paths = [path for path in check.PARTS if old in self.sources[path]]
            self.assertEqual(len(paths), 1)
            path = paths[0]
            proof = self.sources[path]
            self.assertEqual(proof.count(old), 1)
            altered = dict(self.sources, **{})
            altered[path] = proof.replace(old, new)
            with self.assertRaises(ValueError):
                check.schemas(altered)
        for name in ("observe_active_lookup", "observe_active_status", "observe_queued_lookup", "observe_queued_status", "observe_live", "observe_expected_credit"):
            self.assertEqual(self.sources[check.OWNER].count("fn " + name + "("), 1)
        altered = dict(self.sources)
        altered[check.OWNER] += "\n// changed source\n"
        with self.assertRaises(ValueError):
            check.audit(altered)

    def test_actual_body_mutations_do_not_touch_fold_or_proof(self):
        body = self.sources[check.BODY]
        rows = check.mutations(body)
        self.assertEqual(len(rows), 38)
        expected = {family + "-" + suffix for family in ("active", "queued") for suffix in (
            "lookup-error-substituted", "status-error-substituted", "lookup-duplicated",
            "cursor-not-advanced", "lookup-value-ignored", "stored-request-ignored",
        )} | {
            "bound-root-ignored", "launch-dependency-membership-ignored", "peer-dependency-membership-ignored",
            "launch-absent-root-accepted", "peer-absent-root-accepted", "producer-local-order-ignored",
            "allocation-order-ignored", "record-map-ignored", "backend-membership-ignored",
            "credit-observation-skipped", "credit-answer-ignored", "live-error-substituted",
            "live-observation-skipped", "extent-ignored", "offset-ignored", "length-ignored",
            "dependency-scan-match-rejected", "dependency-scan-exhaustion-accepted",
            "dependency-scan-predicate-ignored", "dependency-scan-first-mismatch-terminal", "dependency-scan-local-only",
            "source-scan-match-rejected", "source-scan-exhaustion-accepted", "source-scan-region-ignored",
            "source-scan-record-ignored", "source-scan-first-mismatch-terminal",
        }
        self.assertEqual(set(rows), expected)
        prefix = body.split("macro_rules! producer_dependency_contains_body {", 1)[0]
        macros = ("producer_input_fold_body", "producer_dependency_contains_body",
                  "producer_source_pair_contains_body", "producer_input_validate_body")
        original = [check.block(body, "macro_rules! " + name + " {") for name in macros]
        for name, (mutant, selector) in rows.items():
            self.assertIn(selector, (check.SELECTOR, *check.SCAN_SELECTORS), name)
            self.assertNotEqual(mutant, body, name)
            self.assertTrue(mutant.startswith(prefix), name)
            actual = [check.block(mutant, "macro_rules! " + macro + " {") for macro in macros]
            target = {check.SELECTOR: 3, check.SCAN_SELECTORS[0]: 1, check.SCAN_SELECTORS[1]: 2}[selector]
            self.assertEqual([i for i in range(4) if actual[i] != original[i]], [target], name)
        self.assertIn("let _ = byte_offset; false", rows["offset-ignored"][0])
        self.assertIn("let _ = byte_len; false", rows["length-ignored"][0])
        self.assertIn("let _first =", rows["active-lookup-duplicated"][0])
        self.assertIn("let _first =", rows["queued-lookup-duplicated"][0])
        for branch in ("launch", "peer"):
            self.assertNotIn("&" + branch + ".dependencies,\n                                &input.dependency,",
                             rows[branch + "-dependency-membership-ignored"][0])
            self.assertIn("None => true", rows[branch + "-absent-root-accepted"][0])
        self.assertEqual(sum(focus == check.SELECTOR for _body, focus in rows.values()), 28)
        for focus in check.SCAN_SELECTORS:
            self.assertEqual(sum(selected == focus for _body, selected in rows.values()), 5)

    def test_private_scan_bodies_are_shared_not_assumed(self):
        check.scan_bridges(self.sources)
        for old, new in (
            ("producer_dependency_contains_body!(", "different_dependency_body!("),
            ("producer_source_pair_contains_body!(", "different_source_body!("),
        ):
            changed = dict(self.sources)
            self.assertEqual(changed[check.OWNER].count(old), 1)
            changed[check.OWNER] = changed[check.OWNER].replace(old, new)
            with self.assertRaises(ValueError):
                check.scan_bridges(changed)
        proof = self.sources[check.OUTCOMES]
        self.assertEqual(proof.count("decreases "), 2)
        self.assertIn("dependencies@.contains(*dependency)", proof)
        self.assertIn("sources@[j].region == source.region && sources@[j].record == source.record", proof)

    def test_measured_count_and_exact_selector_policy(self):
        self.assertIsNone(check.EXPECTED_VERIFIED)
        leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied"})
        self.assertIsNone(check.SELECTION_NOTES)
        for focus in (check.SELECTOR, *check.SCAN_SELECTORS):
            with self.assertRaises(ValueError):
                check.selection_notes(leaf, focus)
        for focus in ("*", "validate", "*Observations::reconcile", "*other_helper"):
            with self.assertRaises(ValueError):
                check.selection_notes(leaf, focus)
        with self.assertRaises(ValueError):
            check.campaign()
        with self.assertRaises(ValueError):
            check.controller()
        original = check.EXPECTED_VERIFIED
        try:
            for value in (None, True, 0, -1, 41, 43, "42"):
                check.EXPECTED_VERIFIED = value
                with self.assertRaises(ValueError):
                    check.controller()
        finally:
            check.EXPECTED_VERIFIED = original


if __name__ == "__main__":
    unittest.main()
