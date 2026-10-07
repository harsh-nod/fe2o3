#!/usr/bin/env python3
"""Source/campaign checker tests. Synthetic diagnostics confer no proof credit."""
import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

BASE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), BASE / name)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class PlannerInputCompositionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.guard = load("check-producer-planner-input-composition.py")
        cls.runner = load("qualify-producer-planner-input-v1.py")
        cls.base = load("producer-journal-composition-diagnostics-v1.py")
        cls.parser = cls.base.parser(BASE / "producer-input-diagnostics-v1.py")
        cls.mutations = load("producer-planner-input-mutations-v1.py")
        cls.values, cls.closure = cls.guard.sources()
        cls.cases = cls.mutations.construct(cls.values, cls.parser)

    def test_exact_additive_root_and_closure(self):
        self.assertEqual(len(self.closure), 51)
        source = self.values[self.guard.PROOF]
        combined = self.guard.augmented(source)
        self.assertEqual(combined.replace(self.guard.INSERTION, ""), source)
        self.assertEqual(combined.count(self.guard.INSERTION), 1)
        self.assertEqual(len(set(self.guard.BINDINGS) - self.closure), 4)

    def test_predecessor_drift_rejected(self):
        source = self.values[self.guard.PROOF]
        for changed in (source + "\n", source.replace("self.reconcile()", "self.reconcile()", 1) + " ",
                        source.replace("credit: bool", "credit: u8", 1)):
            with self.assertRaises(ValueError):
                self.guard.augmented(changed)

    def test_runtime_adapter_drift_rejected(self):
        original = self.guard.ordinary
        path = self.guard.ROOT / self.guard.BINDINGS[0]
        changed = self.values[self.guard.BINDINGS[0]].replace(
            "self.journal_result_v1(result)", "Ok(None)")
        with patch.object(self.guard, "ordinary", side_effect=lambda selected:
                          changed.encode() if selected == path else original(selected)):
            with self.assertRaisesRegex(ValueError, "runtime directed status"):
                self.guard.sources()

    def test_runtime_error_quarantine_drift_rejected(self):
        original = self.guard.ordinary
        path = self.guard.ROOT / self.guard.BINDINGS[1]
        changed = self.values[self.guard.BINDINGS[1]].replace(
            "self.quarantine_submission_writers_v1();", "let _ = self;")
        with patch.object(self.guard, "ordinary", side_effect=lambda selected:
                          changed.encode() if selected == path else original(selected)):
            with self.assertRaisesRegex(ValueError, "error/quarantine"):
                self.guard.sources()

    def test_eleven_unique_body_only_controls(self):
        self.assertEqual(len(self.cases), 11)
        seen = set()
        for name, case in self.cases.items():
            with self.subTest(name=name):
                old = self.values[case["path"]]
                new = case["text"]
                lo, hi = self.mutations.function_interval(old, case["method"], self.parser)
                start = old.rfind("\n", 0, lo) + 1
                prefix = old[start:lo]
                indentation = prefix[:len(prefix) - len(prefix.lstrip())]
                body = old.index("\n" + indentation + "{", lo)
                self.assertEqual(old[:body], new[:body])
                self.assertEqual(old[hi:], new[hi + len(new) - len(old):])
                self.assertEqual(old[lo:hi].count(case["primary"]), 1)
                self.assertNotEqual(old, new)
                seen.add((case["path"], new))
        self.assertEqual(len(seen), 11)

    def positive(self):
        return {"verus": self.runner.VERIFIER,
                "verification-results": {"encountered-error": False, "encountered-vir-error": False,
                    "success": True, "verified": 260, "errors": 0, "is-verifying-entire-crate": True},
                "func-details": {"synthetic::only_a_parser_test": {
                    "obligation_proof_notes": [], "failed_proof_notes": []}}}

    def classify(self, value, status=0, stderr=""):
        return self.runner.report(self.guard, self.parser, self.base, status,
                                  json.dumps(value) + "\n", stderr, Path("/synthetic"), {}, None)

    def test_synthetic_positive_schema(self):
        self.assertEqual(self.classify(self.positive())["verified"], 260)

    def test_timeout_signal_and_frontend_rejected(self):
        for code in (1, 124, 137, -9, -15):
            with self.subTest(code=code), self.assertRaises(ValueError):
                self.classify(self.positive(), status=code)
        with self.assertRaises(ValueError):
            self.classify(self.positive(), stderr="frontend warning\n")

    def test_filtered_partial_counts_and_tool_substitution_rejected(self):
        for key, value in (("is-verifying-entire-crate", False), ("verified", 259),
                           ("encountered-vir-error", True), ("errors", 1), ("verified", True)):
            data = self.positive()
            data["verification-results"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                self.classify(data)
        data = self.positive()
        data["verus"] = {**data["verus"], "commit": "0" * 40}
        with self.assertRaises(ValueError):
            self.classify(data)

    def negative(self):
        case = self.cases["unknown-coerced"]
        values = {**self.values, case["path"]: case["text"]}
        source = case["text"]
        lo, hi = self.mutations.function_interval(source, case["method"], self.parser)
        begin = source.index(case["primary"], lo, hi)
        end = begin + len(case["primary"])
        line = source.count("\n", 0, begin) + 1
        column = begin - source.rfind("\n", 0, begin)
        span = {"file_name": str(Path("/synthetic") / case["path"]),
                "byte_start": begin, "byte_end": end, "line_start": line, "line_end": line,
                "column_start": column, "column_end": column + end - begin,
                "is_primary": True, "text": [{"text": source.splitlines()[line - 1],
                    "highlight_start": column, "highlight_end": column + end - begin}],
                "label": "failed this postcondition", "suggested_replacement": None,
                "suggestion_applicability": None, "expansion": None}
        data = self.positive()
        data["func-details"] = {self.guard.PROOF.stem + "::concrete_composition::project_input": {
            "obligation_proof_notes": [], "failed_proof_notes": []}}
        data["verification-results"].update({"encountered-error": True, "success": False,
                                              "verified": 259, "errors": 1})
        row = {"$message_type": "diagnostic", "message": "postcondition not satisfied", "code": None,
               "level": "error", "spans": [span], "children": [], "rendered": "synthetic checker test"}
        summary = {**row, "message": "aborting due to 1 previous error", "spans": []}
        return case, values, data, [row, summary]

    def classify_negative(self, case, values, data, rows, status=1):
        return self.runner.report(self.guard, self.parser, self.base, status,
            json.dumps(data) + "\n", "".join(json.dumps(row) + "\n" for row in rows),
            Path("/synthetic"), values, case)

    def test_synthetic_exact_negative_schema(self):
        self.assertEqual(self.classify_negative(*self.negative())["logical_messages"], 1)

    def test_negative_wrong_function_or_primary_rejected(self):
        case, values, data, rows = self.negative()
        data["func-details"] = {"synthetic::wrong": {
            "obligation_proof_notes": [], "failed_proof_notes": []}}
        with self.assertRaises(ValueError):
            self.classify_negative(case, values, data, rows)
        case, values, data, rows = self.negative()
        rows[0]["spans"][0]["byte_start"] += 1
        with self.assertRaises(ValueError):
            self.classify_negative(case, values, data, rows)

    def test_negative_extra_notes_missing_summary_and_frontend_rejected(self):
        for mode in ("note", "missing-summary", "frontend", "duplicate-error", "after-summary"):
            case, values, data, rows = self.negative()
            if mode == "note":
                rows.insert(0, {**rows[0], "level": "note", "message": self.runner.BODY_ENUMERATION})
            elif mode == "missing-summary":
                rows.pop()
            elif mode == "frontend":
                rows[0]["message"] = "cannot find value in this scope"
            elif mode == "duplicate-error":
                rows.insert(0, rows[0])
            else:
                rows.append(rows[0])
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                self.classify_negative(case, values, data, rows)


if __name__ == "__main__":
    unittest.main()
