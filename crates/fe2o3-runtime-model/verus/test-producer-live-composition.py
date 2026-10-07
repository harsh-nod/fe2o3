#!/usr/bin/env python3
"""Source controls for the live extension; these do not execute a proof."""
import importlib.util
import copy
import json
from pathlib import Path
import re
import unittest

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name, BASE / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.m = load("producer-live-validation-mutations-v1.py")
        pending, cls.sources = [cls.m.PROOF], {}
        while pending:
            path = pending.pop()
            if path in cls.sources:
                continue
            text = (ROOT / path).read_text()
            cls.sources[path] = text
            pending.extend((ROOT / path.parent / name).resolve().relative_to(ROOT)
                           for name in re.findall(r'include!\("([^"\n]+)"\);', text))
        cls.closure = list(cls.sources)
        cls.rows = cls.m.construct(cls.sources, cls.closure)

    def test_full_new_roster_and_actual_shared_source_paths(self):
        self.assertEqual(len(self.closure), 47)
        self.assertEqual(tuple(self.rows), self.m.NAMES)
        self.assertEqual(len(self.rows), 19)
        self.assertTrue(all(row["capture_selector"] is None for row in self.rows.values()))
        self.assertEqual({row["path"] for row in self.rows.values()},
                         {self.m.PROOF, self.m.BODY, self.m.FORWARD})

    def test_all_changes_stay_in_executable_body(self):
        for row in self.rows.values():
            with self.subTest(name=row["name"]):
                old = self.sources[row["path"]]
                start, end = row["implementation_span"]
                lo, hi = self.m.interval(row["text"], row["implementation_anchor"])
                self.assertEqual(start, lo)
                self.assertEqual(old[:start], row["text"][:lo])
                self.assertEqual(old[end:], row["text"][hi:])
                self.assertNotEqual(old[start:end], row["text"][lo:hi])

    def test_no_missing_duplicate_or_selected_closure(self):
        for closure in (self.closure[:-1], self.closure + [self.closure[0]], [self.m.PROOF]):
            with self.assertRaises(ValueError):
                self.m.construct(self.sources, closure)

    def test_ambiguous_or_missing_body_rejected(self):
        for changed in ("", self.sources[self.m.BODY] * 2):
            with self.assertRaises(ValueError):
                self.m.construct({**self.sources, self.m.BODY: changed}, self.closure)

    def test_no_supplied_live_return_or_admission_premise(self):
        self.assertNotIn("external.live", self.sources[self.m.PROOF])
        definition = self.sources[self.m.DEFINITIONS]
        self.assertNotIn("requires", definition)
        self.assertNotRegex(definition, r"\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external")
        self.assertIn("self.versions.validate_live(allocation, record)", self.sources[self.m.PROOF])

    def test_exact_runner_full_profile_has_no_selection_or_limit_option(self):
        runner = load("qualify-producer-live-composition-v1.py")
        self.assertEqual(runner.TIMEOUT, 120)
        self.assertEqual(runner.CASE_COUNTS, {"leaf": 38, "conditional": 21, "concrete": 49})
        self.assertEqual(runner.COUNTS, {"leaf": 42, "conditional": 64, "concrete": 218})
        source = (BASE / "qualify-producer-live-composition-v1.py").read_text()
        self.assertNotIn("--verify-function", source)
        self.assertNotIn("--rlimit", source)
        self.assertNotIn('add_argument("--timeout"', source)

    def test_positive_result_schema_rejects_status_count_and_mode_substitution(self):
        runner = load("qualify-producer-live-composition-v1.py")
        diagnostic = load("producer-live-composition-diagnostics-v1.py").load_base()
        parser = diagnostic.parser(BASE / "producer-input-diagnostics-v1.py")
        report = {"verus": runner.VERIFIER, "verification-results": {
            "encountered-error": False, "encountered-vir-error": False, "success": True,
            "verified": 218, "errors": 0, "is-verifying-entire-crate": True},
            "func-details": {"fixture": {"obligation_proof_notes": [], "failed_proof_notes": []}}}
        for status in (1, 124, -15, True):
            with self.assertRaises(ValueError):
                runner.full_positive(diagnostic, parser, status, json.dumps(report) + "\n", "", "concrete")
        for key, value in (("verified", True), ("verified", 214), ("errors", 1),
                           ("is-verifying-entire-crate", False), ("success", False)):
            altered = copy.deepcopy(report)
            altered["verification-results"][key] = value
            with self.assertRaises(ValueError):
                runner.full_positive(diagnostic, parser, 0, json.dumps(altered) + "\n", "", "concrete")

    def test_reporting_profile_preserves_inherited_and_new_capture_commands(self):
        runner = load("qualify-producer-live-composition-v1.py")
        self.assertEqual(runner.reporting_options(None), [])
        for row in self.rows.values():
            self.assertEqual(runner.reporting_options(row), [])
        for boundary in ("actual-journal-result-equality", "wrapper-ghost-trace-only",
                         "conditional-validator-result", "leaf-validator-result"):
            self.assertEqual(runner.reporting_options({"boundary_label": boundary}),
                             ["--multiple-errors", "1"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
