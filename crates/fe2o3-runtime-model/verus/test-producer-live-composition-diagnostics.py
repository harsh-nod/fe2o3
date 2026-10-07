#!/usr/bin/env python3
"""Regression parsing of retained captures; fixtures qualify no current proof."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
FIXTURE_SHA = "c0677c416a73de7d3a49360aee14ff9477e11ebdc9511be2b9285f01b58f56b2"


def load(name):
    spec = importlib.util.spec_from_file_location(name, BASE / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class DiagnosticControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.d = load("producer-live-composition-diagnostics-v1.py")
        cls.m = load("producer-live-validation-mutations-v1.py")
        cls.extraction = load("producer-live-source-extraction-v1.py")
        guard = load("check-producer-journal-composition.py")
        sources = guard.snapshot()
        guard.audit(sources)
        cases = cls.m.construct(sources, guard.files())
        raw = (BASE / "producer-live-diagnostic-fixtures-v1.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != FIXTURE_SHA:
            raise ValueError("exact retained live diagnostic fixture")
        packet = json.loads(raw)
        if (packet["fixtures_are_fresh_solver_runs"] is not False
                or packet["historical_captures_are_qualified_kills"] is not False
                or set(packet["cases"]) != set(cases)):
            raise ValueError("exact unqualified fixture roster")
        cls.values = {}
        for name, row in packet["cases"].items():
            case = cases[name]
            selected = {path: cls.extraction.live_fixture_source(path, sources[path])
                        for path in case["closure"]}
            selected[case["path"]] = case["text"]
            if ({str(p): hashlib.sha256(t.encode()).hexdigest() for p, t in selected.items()} != row["source_pins"]
                    or hashlib.sha256(row["stdout"].encode()).hexdigest() != row["stdout_sha256"]
                    or hashlib.sha256(row["stderr"].encode()).hexdigest() != row["stderr_sha256"]):
                raise ValueError("retained diagnostic source and stream binding: " + name)
            cls.values[name] = dict(shared_parser=BASE / "producer-input-diagnostics-v1.py",
                verifier=packet["verifier"], sources=selected, expected_paths=set(selected),
                root=Path(row["root"]), case=case, status=1, stdout=row["stdout"], stderr=row["stderr"])
        cls.subject = cls.values["live-phase-skipped"]

    def test_eof_inverse_rejects_any_unreviewed_source_change(self):
        for path, (_current, prior) in self.extraction.LIVE_FIXTURE_EOF.items():
            source = (ROOT / path).read_text()
            self.assertEqual(hashlib.sha256(
                self.extraction.live_fixture_source(path, source).encode()).hexdigest(), prior)
            for changed in (source + "\n", source + "// drift\n", source[:-1]):
                with self.assertRaises(ValueError):
                    self.extraction.live_fixture_source(path, changed)

    def reject(self, value):
        with self.assertRaises((ValueError, KeyError, TypeError)):
            self.d.negative(**value)

    def report(self):
        value = copy.deepcopy(self.subject)
        return value, json.loads(value["stdout"])

    def diagnostics(self):
        value = copy.deepcopy(self.subject)
        return value, [json.loads(line) for line in value["stderr"].splitlines()]

    def test_all_retained_live_boundaries_remain_unqualified(self):
        self.assertEqual(len(self.values), 19)
        for name, value in self.values.items():
            with self.subTest(name=name):
                observed = self.d.negative(**value)
                self.assertFalse(observed["qualified_kill"])
                self.assertFalse(observed["signed_campaign_qualified"])
                self.assertTrue(observed["source_and_process_binding_required"])

    def test_timeout_signal_success_and_boolean_status_rejected(self):
        for status in (0, 124, 137, -9, -15, True, 1.0):
            self.reject(dict(self.subject, status=status))

    def test_exact_full_root_counts_and_tool(self):
        for key, bad in (("verified", 216), ("verified", True), ("errors", 2), ("errors", True),
                         ("is-verifying-entire-crate", False), ("encountered-vir-error", True), ("success", True)):
            value, report = self.report()
            report["verification-results"][key] = bad
            value["stdout"] = json.dumps(report) + "\n"
            self.reject(value)
        value, report = self.report()
        report["verus"]["commit"] = "unreviewed"
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)

    def test_filtered_extra_missing_or_wrong_function_rejected(self):
        value = copy.deepcopy(self.subject)
        value["case"]["capture_selector"] = "Versions::validate_live"
        self.reject(value)
        value = copy.deepcopy(self.subject)
        del value["sources"][next(iter(value["sources"]))]
        self.reject(value)
        value = copy.deepcopy(self.subject)
        value["sources"][Path("extra.rs")] = ""
        value["expected_paths"].add(Path("extra.rs"))
        self.reject(value)
        value, report = self.report()
        del report["func-details"][self.m.PROOF.stem + "::" + value["case"]["boundary"]]
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)

    def test_exact_primary_coordinates_roles_and_source_text(self):
        for key, bad in (("byte_start", True), ("line_start", 1), ("column_start", "1"),
                         ("is_primary", False), ("label", "another contract"), ("file_name", "/wrong.rs")):
            value, rows = self.diagnostics()
            row = next(r for r in rows if r["message"] == "postcondition not satisfied")
            primary = next(s for s in row["spans"] if s["is_primary"])
            primary[key] = bad
            value["stderr"] = "".join(json.dumps(r) + "\n" for r in rows)
            self.reject(value)
        value, rows = self.diagnostics()
        primary = next(s for r in rows if r["message"] == "postcondition not satisfied" for s in r["spans"] if s["is_primary"])
        primary["text"][0]["text"] = "not actual source"
        value["stderr"] = "".join(json.dumps(r) + "\n" for r in rows)
        self.reject(value)

    def test_exact_diagnostic_multiplicity_and_summary(self):
        for transform in (lambda rows: rows[1:], lambda rows: rows[:-1], lambda rows: rows + [rows[-1]],
                          lambda rows: rows[:1] + rows):
            value, rows = self.diagnostics()
            value["stderr"] = "".join(json.dumps(r) + "\n" for r in transform(rows))
            self.reject(value)

    def test_compiler_errors_recommendations_and_unknown_diagnostics_rejected(self):
        for message in ("precondition not satisfied", "recommendation not met", "resource limit exceeded", "compiler error"):
            value, rows = self.diagnostics()
            next(r for r in rows if r["message"] == "postcondition not satisfied")["message"] = message
            value["stderr"] = "".join(json.dumps(r) + "\n" for r in rows)
            self.reject(value)

    def test_duplicate_json_truncated_and_unknown_case_rejected(self):
        self.reject(dict(self.subject, stdout='{"verus":{},"verus":{}}\n'))
        self.reject(dict(self.subject, stdout=self.subject["stdout"].rstrip("\n")))
        self.reject(dict(self.subject, stderr=self.subject["stderr"].rstrip("\n")))
        value = copy.deepcopy(self.subject)
        value["case"]["name"] = "unreviewed-case"
        self.reject(value)


if __name__ == "__main__":
    unittest.main(verbosity=2)
