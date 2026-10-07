#!/usr/bin/env python3
"""Retained predecessor classifier controls; not changed-root qualification."""
import copy
import hashlib
import json
from pathlib import Path
import types
import unittest

BASE = Path(__file__).resolve().parent


def load(path, name):
    module = types.ModuleType(name)
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
    return module


composition = load(BASE / "check-producer-input-composition.py", "diagnostic_composition_source")
check = composition.diagnostic_classifier()
ROOT = Path("/tmp/fe2o3-diagnostic-fixture-never-executed")
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f", "platform": {"os": "linux", "arch": "x86_64"},
            "toolchain": "1.97.1-x86_64-unknown-linux-gnu", "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}


def materialize(value):
    if isinstance(value, dict):
        return {key: materialize(item) for key, item in value.items()}
    if isinstance(value, list):
        return [materialize(item) for item in value]
    return value.replace("$SOURCE", str(ROOT)) if type(value) is str else value


class DiagnosticControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.checks = {}
        for family, name in (("leaf", "validate"), ("fold", "fold"), ("composition", "composition")):
            cls.checks[family] = load(BASE / ("check-producer-input-" + name + ".py"), "final_source_" + family)
        cls.snapshots = {family: checker.snapshot() for family, checker in cls.checks.items()}
        cls.rows = {}
        for family in ("leaf", "fold"):
            checker = cls.checks[family]
            for name, (text, selector) in checker.mutations(cls.snapshots[family][check.BODY]).items():
                cls.rows[family + "/" + name] = {"family": family, "name": name, "path": check.BODY,
                                              "text": text, "selector": selector}
        for name, row in cls.checks["composition"].mutations(cls.snapshots["composition"]).items():
            cls.rows["composition/" + name] = dict(row, family="composition", name=name)
        # The fixtures retain exact old spans and bodies. Recover their signed
        # predecessor bytes; never reinterpret them against the changed closure.
        extraction = load(BASE / "test-producer-input-composition.py", "historical_source_extraction")
        predecessor = extraction.predecessor_sources()
        for family in ("leaf", "composition"):
            cls.snapshots[family][check.DEFINITIONS] = predecessor[extraction.DEFINITIONS]
        cls.snapshots["composition"][check.PROOFS["composition"]] = predecessor[extraction.COMPOSITION]
        cls.fixture_files = {
            "leaf": [check.PROOFS["leaf"], check.DEFINITIONS, check.BODY],
            "fold": cls.checks["fold"].FILES,
            "composition": [check.PROOFS["composition"], check.DEFINITIONS, cls.checks["composition"].SPEC, check.BODY],
        }
        for row in cls.rows.values():
            if row["family"] == "composition":
                original = cls.snapshots["composition"][row["path"]]
                if original.count(row["before"]) != 1:
                    raise ValueError("exact predecessor mutation anchor")
                row["text"] = original.replace(row["before"], row["after"])
        raw = (BASE / "producer-input-diagnostic-fixtures-v1.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != "50bbc125bf95aa9ee283d89a6aa784c8e8b935b1fb2d106ee942a3dc35c3041d":
            raise ValueError("exact reviewed normalized fixture corpus")
        cls.corpus = json.loads(raw)

    def fixture(self, key="composition/per-input-answer-index-zero"):
        item = materialize(copy.deepcopy(self.corpus["fixtures"][key]))
        mutation = self.rows[key]
        family = mutation["family"]
        sources = {path: self.snapshots[family][path] for path in self.fixture_files[family]}
        sources[mutation["path"]] = mutation["text"]
        self.assertEqual({str(path): hashlib.sha256(text.encode()).hexdigest() for path, text in sources.items()}, item["sources"])
        return sources, item["case"], item["result"], item["diagnostics"]

    def classify(self, fixture, status=1):
        sources, case, result, diagnostics = fixture
        return check.negative(VERIFIER, sources, ROOT, case, status, json.dumps(result),
                              "\n".join(json.dumps(row) for row in diagnostics))

    def test_all_actual_fixtures_pass_without_becoming_qualified_kills(self):
        self.assertEqual(len(self.corpus["fixtures"]), 25)
        self.assertFalse(self.corpus["qualified_kills"])
        expected = {"leaf/bound-root-ignored", "leaf/dependency-scan-match-rejected",
                    "leaf/source-scan-match-rejected", "fold/error-substitutes-invalid-reference"} \
                   | {key for key in self.rows if key.startswith("composition/")}
        self.assertEqual(set(self.corpus["fixtures"]), expected)
        self.assertEqual(len(self.rows), 81)
        for key in self.corpus["fixtures"]:
            with self.subTest(case=key):
                result = self.classify(self.fixture(key))
                self.assertTrue(result["logical_diagnostic_accepted"])
                self.assertFalse(result["signed_campaign_qualified"])
                self.assertTrue(result["source_and_process_binding_required"])

    def test_every_original_roster_is_preserved(self):
        self.assertEqual(len(self.checks["leaf"].mutations(self.snapshots["leaf"][check.BODY])), 38)
        self.assertEqual(len(self.checks["fold"].mutations(self.snapshots["fold"][check.BODY])), 22)
        self.assertEqual(sum(row["family"] == "composition" for row in self.rows.values()), 21)
        for path, family in ((check.DEFINITIONS, "leaf"), (check.PROOFS["fold"], "fold")):
            for row in (row for row in self.rows.values() if row["family"] == family):
                sources = {p: self.snapshots[family][p] for p in self.fixture_files[family]}
                sources[row["path"]] = row["text"]
                case = {"family": family, "selector": row["selector"]}
                ranges = check.target_ranges(sources, ROOT, case)
                self.assertIn(str(ROOT / path), ranges)

    def test_exit_mode_identity_and_boolean_counts_rejected(self):
        for status in (True, 0, 101, 124, -9, "1"):
            with self.assertRaises(ValueError):
                self.classify(self.fixture(), status)
        for key, value in (("success", True), ("success", 0), ("verified", False), ("verified", -1),
                           ("verified", 64), ("errors", True), ("errors", 0), ("errors", 65),
                           ("encountered-error", 1), ("encountered-vir-error", True), ("is-verifying-entire-crate", False)):
            fixture = self.fixture()
            fixture[2]["verification-results"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.classify(fixture)
        fixture = self.fixture()
        fixture[2]["verus"]["commit"] = "unknown"
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_exact_selected_and_full_schema_are_distinct(self):
        for key in ("composition/per-input-answer-index-zero", "leaf/bound-root-ignored"):
            fixture = self.fixture(key)
            vr = fixture[2]["verification-results"]
            if "success" in vr:
                del vr["success"]
            else:
                vr["success"] = False
            with self.assertRaises(ValueError):
                self.classify(fixture)

    def test_frontend_warning_unknown_note_and_hidden_children_rejected(self):
        for change in ({"message": "mismatched types"}, {"level": "warning"}, {"code": {"code": "E0308"}},
                       {"children": [{"message": "hidden failure"}]}, {"extra": None}, {"rendered": None}):
            fixture = self.fixture()
            fixture[3][0].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.classify(fixture)
        fixture = self.fixture()
        fixture[3][1]["message"] = "unknown enumeration note"
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_direct_source_bytes_lines_columns_text_and_roles_rejected_on_drift(self):
        for change in ({"file_name": "/tmp/unrelated.rs"}, {"file_name": "relative.rs"},
                       {"byte_start": -1}, {"byte_end": True}, {"line_start": 1},
                       {"column_start": 1}, {"is_primary": 1}, {"is_primary": False},
                       {"text": []}, {"suggested_replacement": "repair"}, {"extra": None}):
            fixture = self.fixture()
            fixture[3][0]["spans"][0].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.classify(fixture)
        fixture = self.fixture()
        fixture[3][0]["spans"][0]["text"][0]["highlight_start"] = True
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_recursive_macro_spans_declaration_and_desugaring_checked(self):
        key = "leaf/bound-root-ignored"
        fixture = self.fixture(key)
        row_index = next(i for i, row in enumerate(fixture[3]) if row["level"] == "error" and row["spans"])
        for kind in ("outside", "coordinate", "macro", "zero-primary", "definition", "cycle"):
            fixture = self.fixture(key)
            expansion = fixture[3][row_index]["spans"][0]["expansion"]
            if kind == "outside":
                expansion["span"]["file_name"] = "/tmp/unrelated.rs"
            elif kind == "coordinate":
                expansion["span"]["byte_end"] += 1
            elif kind == "macro":
                expansion["macro_decl_name"] = "unreviewed!"
            elif kind == "zero-primary":
                expansion["def_site_span"]["is_primary"] = True
            elif kind == "definition":
                expansion["span"]["expansion"]["def_site_span"]["text"][0]["text"] = "not the declaration"
            else:
                nested = copy.deepcopy(expansion)
                cursor = nested
                for _ in range(10):
                    cursor["span"]["expansion"] = copy.deepcopy(expansion)
                    cursor = cursor["span"]["expansion"]
                fixture[3][row_index]["spans"][0]["expansion"] = nested
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.classify(fixture)

    def test_every_error_primary_must_belong_to_declared_family(self):
        fixture = self.fixture()
        fixture[1]["boundary"] = "Composition::active_count"
        with self.assertRaises(ValueError):
            self.classify(fixture)
        fixture = self.fixture()
        source = self.fixture("composition/actual-active-count-cross-wired")[3]
        foreign = next(row for row in source if row["level"] == "error" and row["spans"])
        fixture[3].insert(1, foreign)
        fixture[3][-1]["message"] = "aborting due to 2 previous errors"
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_known_reporting_notes_still_need_intended_source_and_unique_identity(self):
        fixture = self.fixture()
        fixture[3].insert(1, copy.deepcopy(fixture[3][1]))
        with self.assertRaises(ValueError):
            self.classify(fixture)
        fixture = self.fixture()
        fixture[3][1]["spans"] = []
        with self.assertRaises(ValueError):
            self.classify(fixture)
        fixture = self.fixture()
        fixture[3][:] = [row for row in fixture[3] if row["message"] not in check.ENUMERATION]
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_recommendations_exact_pair_and_target_family(self):
        for mutate in (lambda rows: rows.pop(2), lambda rows: rows.insert(2, copy.deepcopy(rows[2])),
                       lambda rows: rows[2]["spans"].reverse(),
                       lambda rows: rows[2]["spans"][1].update(label="not the measured call")):
            fixture = self.fixture()
            mutate(fixture[3])
            with self.assertRaises(ValueError):
                self.classify(fixture)
        fixture = self.fixture("composition/actual-active-count-cross-wired")
        fixture[3][1:1] = self.fixture()[3][2:4]
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_selection_note_exact_inert_once_and_absent_from_full_runs(self):
        for mutate in (lambda rows: rows.pop(0), lambda rows: rows.insert(0, copy.deepcopy(rows[0])),
                       lambda rows: rows[0].update(message="verifying other selected function"),
                       lambda rows: rows[0].update(spans=copy.deepcopy(rows[1]["spans"]))):
            fixture = self.fixture("leaf/bound-root-ignored")
            mutate(fixture[3])
            with self.assertRaises(ValueError):
                self.classify(fixture)
        fixture = self.fixture()
        fixture[3].insert(0, self.fixture("leaf/bound-root-ignored")[3][0])
        with self.assertRaises(ValueError):
            self.classify(fixture)

    def test_terminal_summary_is_exact_complete_and_last(self):
        for mutate in (lambda rows: rows.pop(), lambda rows: rows.append(rows[-1]),
                       lambda rows: rows.insert(0, rows.pop()),
                       lambda rows: rows[-1].update(message="aborting due to 2 previous errors"),
                       lambda rows: rows.append(rows.pop(2))):
            fixture = self.fixture()
            mutate(fixture[3])
            with self.assertRaises(ValueError):
                self.classify(fixture)

    def test_closure_missing_added_or_nonascii_sources_rejected(self):
        for mutate in (lambda sources: sources.pop(check.SPEC),
                       lambda sources: sources.update({Path("extra.rs"): "// no"}),
                       lambda sources: sources.update({check.SPEC: sources[check.SPEC] + chr(233)})):
            fixture = self.fixture()
            mutate(fixture[0])
            with self.assertRaises(ValueError):
                self.classify(fixture)

    def test_duplicate_json_keys_rejected_in_both_streams(self):
        sources, case, result, diagnostics = self.fixture()
        with self.assertRaises(ValueError):
            check.negative(VERIFIER, sources, ROOT, case, 1, '{"verus":{},"verus":{}}', "")
        with self.assertRaises(ValueError):
            check.negative(VERIFIER, sources, ROOT, case, 1, json.dumps(result), '{"message":"x","message":"x"}')


if __name__ == "__main__":
    unittest.main()
