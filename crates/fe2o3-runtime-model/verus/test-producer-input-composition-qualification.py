#!/usr/bin/env python3
"""Source-only negative construction controls; no solver or compiler execution."""
import hashlib
import json
from pathlib import Path
import types
import unittest

path = Path(__file__).with_name("check-producer-input-composition.py")
check = types.ModuleType("composition_qualification_controls")
check.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)


def without_body(text, name):
    anchor = "    fn " + name + "("
    if text.count(anchor) != 1:
        raise ValueError("unique audited implementation function")
    start = text.index("\n    {", text.index(anchor)) + len("\n    ")
    depth, end = 1, start + 1
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    if depth:
        raise ValueError("balanced audited implementation body")
    return text[:start], text[end:]


class MutationControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = check.snapshot()
        cls.rows = check.mutations(cls.sources)

    def test_complete_four_file_closure_and_observed_count(self):
        check.audit(self.sources)
        self.assertEqual(check.FILES, [check.PROOF, check.DEFINITIONS, check.SPEC, check.BODY])
        self.assertEqual(check.EXPECTED_VERIFIED, 64)
        self.assertEqual(check.DIAGNOSTIC_CALIBRATION["selected_fixtures"], 4)
        self.assertEqual(check.DIAGNOSTIC_CALIBRATION["full_composition_fixtures"], 21)
        self.assertFalse(check.DIAGNOSTIC_CALIBRATION["captured_kills_qualified"])
        self.assertEqual(check.diagnostic_classifier().COUNTS, {"leaf": 42, "fold": 13, "composition": 64})
        for path in check.FILES:
            changed = dict(self.sources)
            changed[path] += "\n"
            with self.assertRaises(ValueError):
                check.audit(changed)
            del changed[path]
            with self.assertRaises(ValueError):
                check.audit(changed)

    def test_mutation_roster_is_exact_and_unfiltered(self):
        names = {
            "per-input-answer-index-zero", "actual-launch-flag-inverted",
            "actual-submission-generation-zero", "actual-family-cursors-swapped",
            "returned-error-substituted", "returned-pending-demoted", "returned-unknown-demoted",
            "consumed-only-on-success", "receipt-advance-derived-from-success",
            "trace-prefix-dropped", "trace-prefix-reversed", "receipt-credit-answer-inverted",
            "observed-credit-allocation-generation-zero", "observed-credit-allocation-local-zero",
            "observed-credit-device-generation-zero", "observed-credit-device-local-zero",
            "observed-credit-byte-length-zero", "actual-active-count-cross-wired",
            "actual-queued-count-cross-wired", "composed-unknown-stops-later-validation",
            "composed-error-swallowed",
        }
        self.assertEqual(set(self.rows), names)
        self.assertEqual(len(self.rows), check.MUTANT_COUNT)
        payload = json.dumps(check.mutation_inventory(self.rows), sort_keys=True, separators=(",", ":"))
        self.assertEqual(hashlib.sha256(payload.encode()).hexdigest(), check.MUTATION_ROSTER_SHA)
        for name, row in self.rows.items():
            self.assertIsNone(row["selector"], name)
            self.assertEqual(self.sources[row["path"]].count(row["before"]), 1, name)
            self.assertNotEqual(row["before"], row["after"], name)
            self.assertEqual(self.sources[row["path"]].replace(row["before"], row["after"]), row["text"], name)
            changed = {**self.sources, row["path"]: row["text"]}
            with self.assertRaises(ValueError, msg=name):
                check.audit(changed)

    def test_only_target_implementation_body_changes(self):
        fold = check.load_checker("check-producer-input-fold.py")
        for name, row in self.rows.items():
            original = self.sources[row["path"]]
            if row["path"] == check.BODY:
                for text in (original, row["text"]):
                    self.assertEqual(text.count(fold.FOLD_ANCHOR), 1, name)
                    self.assertEqual(text.count(fold.NATIVE_ANCHOR), 1, name)
                self.assertEqual(original[:original.index(fold.FOLD_ANCHOR)],
                                 row["text"][:row["text"].index(fold.FOLD_ANCHOR)], name)
                self.assertEqual(original[original.index(fold.NATIVE_ANCHOR):],
                                 row["text"][row["text"].index(fold.NATIVE_ANCHOR):], name)
            else:
                function = row["boundary"].split("::")[1]
                self.assertEqual(without_body(original, function), without_body(row["text"], function), name)
            self.assertNotEqual(row["path"], check.SPEC, name)

    def test_reused_fold_faults_are_exact_original_payloads(self):
        fold = check.load_checker("check-producer-input-fold.py")
        old = fold.mutations(self.sources[check.BODY])
        for current, previous in (
            ("composed-error-swallowed", "error-swallowed-as-success"),
            ("composed-unknown-stops-later-validation", "unknown-stops-later-validation"),
        ):
            self.assertEqual(self.rows[current]["text"], old[previous][0])
        self.assertEqual(sum(row["path"] == check.PROOF for row in self.rows.values()), 14)
        self.assertEqual(sum(row["path"] == check.DEFINITIONS for row in self.rows.values()), 5)
        self.assertEqual(sum(row["path"] == check.BODY for row in self.rows.values()), 2)

    def test_roster_and_count_drift_rejected(self):
        for path in (check.SRC / "unbound.rs", check.V / "unbound.rs"):
            with self.assertRaises(ValueError):
                check.audit({**self.sources, path: "// not in candidate\n"})
        original = check.EXPECTED_VERIFIED
        try:
            for value in (None, True, "64", 63, 65):
                check.EXPECTED_VERIFIED = value
                with self.assertRaises(ValueError):
                    check.audit(self.sources)
        finally:
            check.EXPECTED_VERIFIED = original


if __name__ == "__main__":
    unittest.main()
