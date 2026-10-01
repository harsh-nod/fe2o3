#!/usr/bin/env python3
"""Source-bound diagnostic controls; historical replays qualify zero kills."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

BASE = Path(__file__).resolve().parent
PARSER = BASE / "producer-input-diagnostics-v1.py"
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f", "platform": {"os": "linux", "arch": "x86_64"},
            "toolchain": "1.97.1-x86_64-unknown-linux-gnu", "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}
SPEC = importlib.util.spec_from_file_location("concrete_diagnostics", BASE / "producer-journal-composition-diagnostics-v1.py")
d = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(d)


def pinned(path, digest):
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("fixture custody: " + str(path))
    return json.loads(raw)


class DiagnosticControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location("portable_concrete_mutations", BASE / "producer-journal-composition-mutations-v1.py")
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        cases, original = builder.checked()
        inventory = json.loads(builder.canonical(builder.inventory(cases)))
        fixtures = pinned(BASE / "producer-journal-composition-diagnostic-fixtures-v1.json",
                          "2891b763f597b1b3b936509153377a8c2c55fdd4d0fa1e6cd80fdd2e6d980e3a")
        if (fixtures["historical_captures_are_qualified_kills"] is not False
                or fixtures["test_fixtures_are_fresh_solver_runs"] is not False
                or fixtures["original_timeout_campaign_complete"] is not False
                or fixtures["fresh_signed_roster_due"] != builder.COUNTS
                or fixtures["generator_sha256"] != "64bfe7dda8e7e7891c61a8148292e0dd66af3beddd023e7b8b6ebdc5c628e957"
                or set(fixtures["cases"]) != set(cases)):
            raise ValueError("complete portable calibration with no inherited qualification")
        selected = {}
        for key, row in fixtures["cases"].items():
            case = cases[key]
            sources = {path: original[path] for path in case["closure"]}
            sources[case["path"]] = case["text"]
            if (row["case"] != inventory[key]
                    or {str(path): hashlib.sha256(text.encode()).hexdigest() for path, text in sources.items()} != row["source_pins"]
                    or hashlib.sha256(row["stdout"].encode()).hexdigest() != row["original_stdout_sha256"]
                    or row["qualified_kill"] is not False):
                raise ValueError("exact reconstructed source-bound fixture: " + key)
            selected[key] = dict(shared_parser=PARSER, verifier=VERIFIER, sources=sources, expected_paths=set(sources),
                                 root=Path(row["root"]), case=row["case"], status=1, stdout=row["stdout"], stderr=row["stderr"])
        cls.old_fixtures = [value for value in selected.values() if value["case"]["family"] in ("leaf", "conditional")]
        cls.concrete = {value["case"]["name"]: value for value in selected.values() if value["case"]["family"] == "concrete"}
        cls.pending = cls.concrete["returned-pending-demoted"]

    def reject(self, value):
        with self.assertRaises((ValueError, KeyError, TypeError)):
            d.negative(**value)

    def stdout(self):
        return copy.deepcopy(self.pending), json.loads(self.pending["stdout"])

    def stderr(self):
        return copy.deepcopy(self.pending), [json.loads(line) for line in self.pending["stderr"].splitlines()]

    def test_all_59_old_closure_bound_fixtures_unqualified(self):
        self.assertEqual(len(self.old_fixtures), 59)
        for value in self.old_fixtures:
            with self.subTest(case=value["case"]["name"]):
                observed = d.negative(**value)
                self.assertFalse(observed["qualified_kill"])
                self.assertFalse(observed["signed_campaign_qualified"])

    def test_opaque_pending_raw_fixture_unqualified(self):
        observed = d.negative(**self.pending)
        self.assertFalse(observed["qualified_kill"])
        self.assertEqual(observed["family"], "concrete")

    def test_signal_timeout_success_and_boolean_status_rejected(self):
        for status in (0, 124, 137, -15, True):
            with self.subTest(status=status):
                self.reject(dict(self.pending, status=status))

    def test_exact_typed_full_root_result(self):
        for key, bad in (("verified", 212), ("verified", True), ("errors", 0), ("errors", True),
                         ("success", True), ("encountered-vir-error", True), ("is-verifying-entire-crate", False)):
            value, report = self.stdout()
            report["verification-results"][key] = bad
            value["stdout"] = json.dumps(report) + "\n"
            with self.subTest(key=key, bad=bad):
                self.reject(value)

    def test_wrong_tool_and_selected_request_rejected(self):
        value, report = self.stdout()
        report["verus"]["version"] = "other"
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)
        value = copy.deepcopy(self.pending)
        value["case"]["capture_selector"] = "selected_function"
        self.reject(value)

    def test_function_join_and_note_schema_rejected(self):
        value, report = self.stdout()
        key = d.PROOFS["concrete"].stem + "::" + value["case"]["boundary"]
        del report["func-details"][key]
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)
        value, report = self.stdout()
        report["func-details"][key]["failed_proof_notes"] = ["invented"]
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)

    def test_duplicate_json_and_truncated_streams_rejected(self):
        self.reject(dict(self.pending, stdout='{"verus": {}, "verus": {}}\n'))
        self.reject(dict(self.pending, stdout=self.pending["stdout"].rstrip("\n")))
        self.reject(dict(self.pending, stderr=self.pending["stderr"].rstrip("\n")))

    def test_source_closure_missing_extra_rejected(self):
        value = copy.deepcopy(self.pending)
        del value["sources"][next(iter(value["sources"]))]
        self.reject(value)
        value = copy.deepcopy(self.pending)
        value["sources"][Path("extra.rs")] = ""
        value["expected_paths"].add(Path("extra.rs"))
        self.reject(value)

    def test_exact_source_coordinates_rejected(self):
        for key, bad in (("byte_start", True), ("column_start", "17"), ("line_start", 1),
                         ("file_name", "/unbound.rs")):
            value, rows = self.stderr()
            primary = next(span for row in rows if row["message"] == "postcondition not satisfied" for span in row["spans"] if span["is_primary"])
            primary[key] = bad
            value["stderr"] = "".join(json.dumps(row) + "\n" for row in rows)
            with self.subTest(key=key):
                self.reject(value)

    def test_primary_text_and_recommendation_pair_rejected(self):
        value, rows = self.stderr()
        primary = next(span for row in rows if row["message"] == "postcondition not satisfied" for span in row["spans"] if span["is_primary"])
        primary["text"][0]["text"] = "wrong source text"
        value["stderr"] = "".join(json.dumps(row) + "\n" for row in rows)
        self.reject(value)
        value, rows = self.stderr()
        index = next(index for index, row in enumerate(rows) if row["message"] == "recommendation not met")
        del rows[index]
        value["stderr"] = "".join(json.dumps(row) + "\n" for row in rows)
        self.reject(value)

    def current(self, name="active_lookup-error-coerced"):
        return copy.deepcopy(self.concrete[name])

    def rows(self, value):
        return [json.loads(line) for line in value["stderr"].splitlines()]

    def set_rows(self, value, rows):
        value["stderr"] = "".join(json.dumps(row) + "\n" for row in rows)

    def test_all_30_current_concrete_boundaries_unqualified(self):
        self.assertEqual(len(self.concrete), 30)
        labels = []
        for name, value in self.concrete.items():
            with self.subTest(case=name):
                result = d.negative(**value)
                self.assertFalse(result["qualified_kill"])
                self.assertFalse(result["signed_campaign_qualified"])
                if "forwarding" in result:
                    labels.append(result["forwarding"]["boundary"])
                    self.assertFalse(result["forwarding"]["inner_query_call_count_proved"])
        self.assertEqual(labels.count("actual-journal-result-equality"), 8)
        self.assertEqual(labels.count("wrapper-ghost-trace-only"), 1)

    def test_forwarding_labels_cannot_be_swapped(self):
        for name, label in (("active_lookup-prefetch-status", "actual-journal-result-equality"),
                            ("active_lookup-error-coerced", "wrapper-ghost-trace-only")):
            value = self.current(name)
            value["case"]["boundary_label"] = label
            self.reject(value)

    def test_forwarding_exact_order_and_note_join(self):
        value = self.current()
        rows = self.rows(value)
        for changed in (rows[1:], rows + [rows[-1]], [rows[1], rows[0], rows[2]]):
            altered = self.current()
            self.set_rows(altered, changed)
            self.reject(altered)
        rows[0]["spans"] = [copy.deepcopy(rows[1]["spans"][1])]
        rows[0]["spans"][0]["label"] = None
        self.set_rows(value, rows)
        self.reject(value)

    def test_forwarding_nested_macro_and_definition_rejected(self):
        for target in ("macro", "definition", "synthetic", "source"):
            value = self.current("active_lookup-prefetch-status")
            rows = self.rows(value)
            expansion = rows[1]["spans"][0]["expansion"]
            if target == "macro":
                expansion["span"]["expansion"]["macro_decl_name"] = "producer_observe_queued_status_body_v1!"
            elif target == "definition":
                expansion["span"]["expansion"]["def_site_span"]["byte_end"] += 1
            elif target == "synthetic":
                expansion["def_site_span"]["is_primary"] = True
            else:
                expansion["span"]["text"][0]["text"] = "forged nested source"
            self.set_rows(value, rows)
            self.reject(value)

    def test_source_valid_wrong_forwarding_macro_rejected(self):
        value = self.current("active_lookup-prefetch-status")
        rows = self.rows(value)
        nested = rows[1]["spans"][0]["expansion"]["span"]["expansion"]

        def relocate(span, path, expression):
            text = value["sources"][path]
            self.assertEqual(text.count(expression), 1)
            lo, hi = text.index(expression), text.index(expression) + len(expression)
            line, column = text.count("\n", 0, lo) + 1, lo - text.rfind("\n", 0, lo)
            span.update(file_name=str(value["root"] / path), byte_start=lo, byte_end=hi,
                        line_start=line, line_end=line, column_start=column, column_end=column + len(expression),
                        text=[{"text": text.splitlines()[line - 1], "highlight_start": column, "highlight_end": column + len(expression)}])

        nested["macro_decl_name"] = "producer_observe_queued_status_body_v1!"
        relocate(nested["span"], d.PROOFS["concrete"], "producer_observe_queued_status_body_v1!(self, reference)")
        relocate(nested["def_site_span"], d.JOURNAL_BODY, "macro_rules! producer_observe_queued_status_body_v1")
        self.set_rows(value, rows)
        self.reject(value)

    def test_forwarding_primary_roles_and_logical_family(self):
        for key, bad in (("is_primary", False), ("label", "different"), ("suggested_replacement", "fixed")):
            value = self.current()
            rows = self.rows(value)
            rows[1]["spans"][1][key] = bad
            self.set_rows(value, rows)
            self.reject(value)
        for position, key, bad in ((1, "message", "type mismatch"), (0, "level", "warning"),
                                  (1, "code", {"code": "E0001"}), (1, "children", [{"message": "hidden"}]),
                                  (2, "message", "aborting due to 2 previous errors")):
            value = self.current()
            rows = self.rows(value)
            rows[position][key] = bad
            self.set_rows(value, rows)
            self.reject(value)

    def test_forwarding_wrong_method_or_failed_function_join(self):
        value = self.current()
        value["case"]["name"] = "queued_status-error-coerced"
        self.reject(value)
        value = self.current()
        report = json.loads(value["stdout"])
        del report["func-details"][d.PROOFS["concrete"].stem + "::" + value["case"]["boundary"]]
        value["stdout"] = json.dumps(report) + "\n"
        self.reject(value)


if __name__ == "__main__":
    unittest.main(verbosity=2)
