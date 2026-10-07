#!/usr/bin/env python3
"""Preserved raw captures are parser fixtures only, never qualified kills."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("journal_diagnostics", HERE / "producer-journal-diagnostics-v1.py")
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)
REPO = HERE.parents[2]
SPAN_HELPER = HERE / "producer-input-diagnostics-v1.py"
FIXTURE_SHA = "20bc9fd9aad00cd5b4051979ccd96a2bb30d9f3b375fd0b1a47d80a9cc3b8b92"
PREPARED_SHA = "f1508ef23c2420abed09be5e198faa7d2c8aa6f4ad70bb1853c3cff76ce62096"
RESULT_SHA = "2f4966f8a952e979961eb821e77c3d5dfdf7414ef9f163eb142865dfa8dc9056"


class DiagnosticControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = HERE / "check-producer-journal-observers.py"
        guard = types.ModuleType("portable_journal_fixture_guard")
        guard.__file__ = str(path)
        exec(compile(path.read_bytes(), str(path), "exec"), guard.__dict__)
        base = guard.snapshot()
        guard.audit(base)
        # Historical parser inputs precede the additive live-allocation query.
        # Recover their exact pinned bytes; do not reinterpret the old captures.
        path = HERE / "producer-live-source-extraction-v1.py"
        extraction = types.ModuleType("journal_fixture_predecessor")
        extraction.__file__ = str(path)
        exec(compile(path.read_bytes(), str(path), "exec"), extraction.__dict__)
        mutations = guard.mutations(base[guard.BODY])
        fixture_bytes = (HERE / "producer-journal-diagnostic-fixtures-v1.json").read_bytes()
        assert hashlib.sha256(fixture_bytes).hexdigest() == FIXTURE_SHA
        fixture = json.loads(fixture_bytes)
        assert fixture["prepared_sha256"] == PREPARED_SHA and fixture["result_sha256"] == RESULT_SHA
        assert fixture["historical_captures_are_qualified_kills"] is False
        assert fixture["test_fixtures_are_fresh_solver_runs"] is False
        cls.fixtures = {}
        for case, row in fixture["cases"].items():
            source = {path: extraction.source(path, base[path]) for path in guard.files()}
            source[guard.BODY] = mutations[case][0]
            assert {str(path): hashlib.sha256(text.encode()).hexdigest() for path, text in source.items()} == row["source_pins"]
            assert hashlib.sha256(row["stdout"].encode()).hexdigest() == row["original_stdout_sha256"]
            cls.fixtures[case] = dict(root=Path(row["root"]), sources=source, stdout=row["stdout"], stderr=row["stderr"],
                verifier=json.loads(row["stdout"])["verus"], expected_paths=set(source), status=1, case=case, shared_span_parser=SPAN_HELPER)
        assert set(cls.fixtures) == d.CASES

    def fixture(self, name="active_lookup-error-coerced"):
        return copy.deepcopy(self.fixtures[name])

    def diagnostic_rows(self, fixture):
        return [json.loads(line) for line in fixture["stderr"].splitlines()]

    def set_rows(self, fixture, rows):
        fixture["stderr"] = "".join(json.dumps(row) + "\n" for row in rows)

    def rejected(self, fixture):
        with self.assertRaises(ValueError):
            d.negative(**fixture)

    def test_all_nine_preserved_observations_match_distinct_boundaries_without_qualification(self):
        labels = []
        for fixture in self.fixtures.values():
            result = d.negative(**fixture)
            self.assertTrue(result["logical_diagnostic_accepted"])
            self.assertFalse(result["signed_campaign_qualified"])
            self.assertFalse(result["historical_capture_is_qualified_kill"])
            self.assertFalse(result["inner_query_call_count_proved"])
            labels.append(result["boundary"])
        self.assertEqual(labels.count("actual-journal-result-equality"), 8)
        self.assertEqual(labels.count("wrapper-ghost-trace-only"), 1)

    def test_eager_status_cannot_be_relabelled_as_result_equality(self):
        fixture = self.fixture("active_lookup-prefetch-status")
        rows = self.diagnostic_rows(fixture)
        other = self.diagnostic_rows(self.fixture())[1]["spans"][1]
        other["file_name"] = str(fixture["root"] / d.PROOF)
        rows[1]["spans"][1] = other
        self.set_rows(fixture, rows)
        self.rejected(fixture)

    def test_result_failure_cannot_be_relabelled_as_ghost_trace(self):
        fixture = self.fixture()
        rows = self.diagnostic_rows(fixture)
        other = self.diagnostic_rows(self.fixture("active_lookup-prefetch-status"))[1]["spans"][1]
        other["file_name"] = str(fixture["root"] / d.PROOF)
        rows[1]["spans"][1] = other
        self.set_rows(fixture, rows)
        self.rejected(fixture)

    def test_selected_frontend_success_or_wrong_counts_rejected(self):
        for key, value in (("is-verifying-entire-crate", False), ("encountered-vir-error", True),
                           ("success", True), ("verified", 165), ("verified", True), ("errors", True), ("errors", 2)):
            fixture = self.fixture()
            result = json.loads(fixture["stdout"])
            result["verification-results"][key] = value
            fixture["stdout"] = json.dumps(result) + "\n"
            self.rejected(fixture)

    def test_unknown_verifier_missing_mode_or_unknown_summary_field_rejected(self):
        fixture = self.fixture()
        fixture["verifier"]["version"] = "different"
        self.rejected(fixture)
        for operation in (lambda vr: vr.pop("success"), lambda vr: vr.update(extra=False)):
            fixture = self.fixture()
            result = json.loads(fixture["stdout"])
            operation(result["verification-results"])
            fixture["stdout"] = json.dumps(result) + "\n"
            self.rejected(fixture)

    def test_missing_actual_function_roster_entry_or_wrong_case_rejected(self):
        fixture = self.fixture()
        result = json.loads(fixture["stdout"])
        del result["func-details"]["context_producer_journal_observers_v1::native_observers::Observations::observe_active_lookup"]
        fixture["stdout"] = json.dumps(result) + "\n"
        self.rejected(fixture)
        fixture = self.fixture()
        fixture["case"] = "queued_status-error-coerced"
        self.rejected(fixture)

    def test_direct_span_source_text_offsets_and_typed_highlights_rejected(self):
        for key, value in (("byte_start", 0), ("line_start", True), ("column_start", 18)):
            fixture = self.fixture()
            rows = self.diagnostic_rows(fixture)
            rows[1]["spans"][1][key] = value
            self.set_rows(fixture, rows)
            self.rejected(fixture)
        for key, value in (("text", "forged source"), ("highlight_start", True)):
            fixture = self.fixture()
            rows = self.diagnostic_rows(fixture)
            rows[1]["spans"][1]["text"][0][key] = value
            self.set_rows(fixture, rows)
            self.rejected(fixture)

    def test_nested_macro_call_definition_and_builtin_span_rejected(self):
        for mutation in ("macro", "definition", "synthetic", "source"):
            fixture = self.fixture("active_lookup-prefetch-status")
            rows = self.diagnostic_rows(fixture)
            expansion = rows[1]["spans"][0]["expansion"]
            if mutation == "macro":
                expansion["span"]["expansion"]["macro_decl_name"] = "producer_observe_queued_status_body_v1!"
            elif mutation == "definition":
                expansion["span"]["expansion"]["def_site_span"]["byte_end"] += 1
            elif mutation == "synthetic":
                expansion["def_site_span"]["is_primary"] = True
            else:
                expansion["span"]["text"][0]["text"] = "forged nested source"
            self.set_rows(fixture, rows)
            self.rejected(fixture)

    def test_wrong_primary_roles_label_or_expansion_rejected(self):
        for key, value in (("is_primary", False), ("label", "different"), ("suggested_replacement", "fixed")):
            fixture = self.fixture()
            rows = self.diagnostic_rows(fixture)
            rows[1]["spans"][1][key] = value
            self.set_rows(fixture, rows)
            self.rejected(fixture)

    def test_source_valid_but_wrong_wrapper_macro_expansion_rejected(self):
        fixture = self.fixture("active_lookup-prefetch-status")
        rows = self.diagnostic_rows(fixture)
        nested = rows[1]["spans"][0]["expansion"]["span"]["expansion"]
        def relocate(span, path, expression):
            text = fixture["sources"][path]
            self.assertEqual(text.count(expression), 1)
            lo = text.index(expression)
            hi = lo + len(expression)
            line = text.count("\n", 0, lo) + 1
            column = lo - text.rfind("\n", 0, lo)
            span.update(file_name=str(fixture["root"] / path), byte_start=lo, byte_end=hi,
                line_start=line, line_end=line, column_start=column, column_end=column + len(expression),
                text=[{"text": text.splitlines()[line - 1], "highlight_start": column, "highlight_end": column + len(expression)}])
        nested["macro_decl_name"] = "producer_observe_queued_status_body_v1!"
        relocate(nested["span"], d.PROOF, "producer_observe_queued_status_body_v1!(self, reference)")
        relocate(nested["def_site_span"], d.BODY, "macro_rules! producer_observe_queued_status_body_v1")
        self.set_rows(fixture, rows)
        self.rejected(fixture)

    def test_reporting_note_must_join_the_exact_method_signature(self):
        fixture = self.fixture()
        rows = self.diagnostic_rows(fixture)
        rows[0]["spans"] = [copy.deepcopy(rows[1]["spans"][1])]
        rows[0]["spans"][0]["label"] = None
        self.set_rows(fixture, rows)
        self.rejected(fixture)

    def test_missing_extra_or_reordered_diagnostic_rejected(self):
        original = self.diagnostic_rows(self.fixture())
        for rows in (original[1:], original + [original[-1]], [original[1], original[0], original[2]]):
            fixture = self.fixture()
            self.set_rows(fixture, rows)
            self.rejected(fixture)

    def test_nonlogical_error_warning_code_children_or_summary_count_rejected(self):
        for position, key, value in ((1, "message", "type mismatch"), (0, "level", "warning"),
                (1, "code", {"code": "E0001"}), (1, "children", [{"message": "hidden"}]),
                (2, "message", "aborting due to 2 previous errors")):
            fixture = self.fixture()
            rows = self.diagnostic_rows(fixture)
            rows[position][key] = value
            self.set_rows(fixture, rows)
            self.rejected(fixture)

    def test_duplicate_keys_nonfinite_or_truncated_streams_rejected(self):
        fixture = self.fixture()
        fixture["stdout"] = fixture["stdout"].replace('"success": false', '"success": false, "success": false')
        self.rejected(fixture)
        fixture = self.fixture()
        fixture["stdout"] = fixture["stdout"].replace('"verified": 164', '"verified": NaN')
        self.rejected(fixture)
        for key in ("stdout", "stderr"):
            fixture = self.fixture()
            fixture[key] = fixture[key].rstrip("\n")
            self.rejected(fixture)

    def test_complete_source_closure_required_and_unknown_files_rejected(self):
        fixture = self.fixture()
        fixture["sources"].pop(d.BODY)
        self.rejected(fixture)
        fixture = self.fixture()
        fixture["sources"][Path("extra.rs")] = ""
        self.rejected(fixture)
        fixture = self.fixture()
        rows = self.diagnostic_rows(fixture)
        rows[1]["spans"][1]["file_name"] = str(fixture["root"] / "outside.rs")
        self.set_rows(fixture, rows)
        self.rejected(fixture)

    def test_no_timeout_signal_boolean_status_or_unknown_case(self):
        for status in (0, -15, -9, 124, 137, True, "1"):
            fixture = self.fixture()
            fixture["status"] = status
            self.rejected(fixture)
        fixture = self.fixture()
        fixture["case"] = "unknown"
        self.rejected(fixture)

    def test_span_parser_bytes_must_match_prior_review(self):
        with tempfile.TemporaryDirectory() as folder:
            fixture = self.fixture()
            path = Path(folder) / "helper.py"
            path.write_text("raise RuntimeError('must not execute')\n")
            fixture["shared_span_parser"] = path
            self.rejected(fixture)


if __name__ == "__main__":
    unittest.main(verbosity=2)
