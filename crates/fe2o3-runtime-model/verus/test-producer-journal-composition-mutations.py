#!/usr/bin/env python3
"""Source-only mutation custody controls; never invokes Verus or Cargo."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import types
import unittest
from unittest.mock import patch

BASE = Path(__file__).resolve().parent
path = BASE / "producer-journal-composition-mutations-v1.py"
m = types.ModuleType("concrete_mutation_controls")
m.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), m.__dict__)


class MutationControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows, cls.sources = m.checked()
        cls.g = m.guard()
        cls.leaf = cls.g.helper("check-producer-input-validate.py")
        cls.conditional = cls.g.helper("check-producer-input-composition.py")
        cls.wrapper = cls.g.helper("check-producer-journal-observers.py")

    def test_exact_roster_and_full_roots(self):
        self.assertEqual(len(self.rows), 89)
        self.assertEqual(m.sha(m.canonical(m.inventory(self.rows))), m.ROSTER_SHA)
        self.assertTrue(all(row["capture_selector"] is None for row in self.rows.values()))
        self.assertEqual({family: {len(row["closure"]) for row in self.rows.values() if row["family"] == family}
                          for family in m.COUNTS}, {"leaf": {6}, "conditional": {8}, "concrete": {44}})

    def test_all_38_leaf_bytes_and_original_selectors_retained(self):
        original = self.leaf.mutations(self.sources[self.leaf.BODY])
        self.assertEqual({row["name"] for row in self.rows.values() if row["family"] == "leaf"}, set(original))
        for name, (text, selector) in original.items():
            row = self.rows["leaf/" + name]
            self.assertEqual((row["text"], row["origin_selector"]), (text, selector))

    def test_all_21_conditional_bytes_and_deltas_retained(self):
        original = self.conditional.mutations(self.conditional.snapshot())
        self.assertEqual(tuple(original), m.ADAPTER_NAMES)
        for name, old in original.items():
            row = self.rows["conditional/" + name]
            for key in ("path", "text", "before", "after", "boundary"):
                self.assertEqual(row[key], old[key], (name, key))

    def test_all_21_concrete_counterparts_and_expected_methods(self):
        for name in m.ADAPTER_NAMES:
            row = self.rows["concrete/" + name]
            old = self.rows["conditional/" + name]
            self.assertEqual(row["boundary"], "concrete_composition::" + old["boundary"])
            self.assertEqual(row["root"], self.g.PROOF)
        for name in m.ADAPTER_NAMES[-2:]:
            self.assertEqual(self.rows["concrete/" + name]["text"], self.rows["conditional/" + name]["text"])

    def test_all_nine_native_forwarding_bytes_retained(self):
        original = self.wrapper.mutations(self.sources[self.wrapper.BODY])
        self.assertEqual(tuple(original), m.FORWARD_NAMES)
        for name, (text, method) in original.items():
            row = self.rows["concrete/" + name]
            self.assertEqual((row["text"], row["origin_selector"]), (text, method))
            self.assertEqual(row["boundary"], method.replace("native_observers::", "concrete_composition::"))

    def test_eight_result_equality_and_one_ghost_only_labels(self):
        labels = [self.rows["concrete/" + name]["boundary_label"] for name in m.FORWARD_NAMES]
        self.assertEqual(labels.count("actual-journal-result-equality"), 8)
        self.assertEqual(labels.count("wrapper-ghost-trace-only"), 1)
        changed = copy.deepcopy(self.rows)
        changed["concrete/active_lookup-prefetch-status"]["boundary_label"] = "actual-journal-result-equality"
        with self.assertRaisesRegex(ValueError, "eight result-equality"):
            m.validate(changed, self.sources, self.leaf)

    def test_credit_argument_cases_are_ghost_boundary_not_lock_proofs(self):
        selected = [row for row in self.rows.values() if row["family"] == "concrete"
                    and row["name"].startswith("observed-credit-")]
        self.assertEqual(len(selected), 5)
        self.assertTrue(all(row["boundary_label"] == "external-credit-argument-ghost-trace" for row in selected))
        for row in selected:
            self.assertIn("self.external.credit", row["text"])
            self.assertEqual(row["before"], "proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); }")

    def test_each_delta_is_inside_exact_implementation_body(self):
        for row in self.rows.values():
            original = self.sources[row["path"]]
            start, end = row["implementation_span"]
            altered = m.span(row["text"], row["implementation_scope"], row["implementation_anchor"], self.leaf)
            self.assertEqual(original[:start], row["text"][:altered[0]])
            self.assertEqual(original[end:], row["text"][altered[1]:])
            self.assertEqual(m.replacement(original, row["before"], row["after"]), row["text"])

    def test_contract_or_oracle_edit_is_rejected(self):
        row = self.rows["concrete/actual-family-cursors-swapped"]
        original = self.sources[row["path"]]
        for changed in (row["text"].replace("final(self).wf(), final(self).same_binding(old(self)),",
                                           "true, final(self).same_binding(old(self)),", 1),
                        row["text"].replace("credit: external.credit,", "credit: !external.credit,", 1)):
            with self.assertRaisesRegex(ValueError, "inside intended body"):
                m.in_body(original, changed, row["implementation_span"])

    def test_missing_and_extra_cases_are_rejected(self):
        missing = dict(self.rows)
        del missing[next(iter(missing))]
        extra = dict(self.rows)
        extra["extra"] = next(iter(extra.values()))
        for changed in (missing, extra):
            with self.assertRaisesRegex(ValueError, "case counts"):
                m.validate(changed, self.sources, self.leaf)

    def test_filtered_capture_and_changed_root_are_rejected(self):
        changed = copy.deepcopy(self.rows)
        changed["leaf/credit-answer-ignored"]["capture_selector"] = self.leaf.SELECTOR
        with self.assertRaisesRegex(ValueError, "full-root case"):
            m.validate(changed, self.sources, self.leaf)
        changed = copy.deepcopy(self.rows)
        changed["concrete/actual-family-cursors-swapped"]["root"] = self.leaf.PROOF
        with self.assertRaisesRegex(ValueError, "custody"):
            m.validate(changed, self.sources, self.leaf)

    def test_source_hash_and_mutation_drift_are_rejected(self):
        for key, value in (("original_sha256", "0" * 64), ("text", "changed")):
            changed = copy.deepcopy(self.rows)
            changed["concrete/actual-launch-flag-inverted"][key] = value
            with self.assertRaises(ValueError):
                m.validate(changed, self.sources, self.leaf)
        with patch.object(m, "ROSTER_SHA", "0" * 64), patch.object(m, "construct", return_value=(self.rows, self.sources)):
            with self.assertRaisesRegex(ValueError, "immutable 89-case roster"):
                m.checked()

    def test_ambiguous_and_vacuous_replacements_are_rejected(self):
        for args in (("aa", "a", "b"), ("a", "a", "a"), ("a", "b", "c")):
            with self.assertRaisesRegex(ValueError, "one exact nonvacuous"):
                m.replacement(*args)

    def test_complete_source_snapshot_and_guard_are_bound(self):
        self.assertEqual(self.g.snapshot(), self.sources)
        changed = dict(self.sources)
        changed[self.g.PROOF] += "\n"
        with self.assertRaises(ValueError):
            m.construct(changed)
        with patch.object(m, "GUARD_SHA", "0" * 64):
            with self.assertRaisesRegex(ValueError, "exact frozen"):
                m.guard()

    def test_standalone_fold_and_wrapper_closures_unchanged_from_signed_base(self):
        fold = self.conditional.load_checker("check-producer-input-fold.py")
        paths = sorted(set(fold.FILES) | set(self.wrapper.files()))
        self.assertEqual((len(fold.FILES), len(self.wrapper.files())), (3, 38))
        result = subprocess.run(["git", "-C", str(m.ROOT), "diff", "--exit-code",
            "6793b910c2aa662df20763fbbd349cec0994677f", "--", *map(str, paths)],
            capture_output=True, timeout=15, check=False)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"", b""))

    def test_exact_portable_calibration_has_no_inherited_qualification(self):
        raw = (BASE / "producer-journal-composition-diagnostic-fixtures-v1.json").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), "2891b763f597b1b3b936509153377a8c2c55fdd4d0fa1e6cd80fdd2e6d980e3a")
        fixture = json.loads(raw)
        self.assertEqual({key: row["case"] for key, row in fixture["cases"].items()},
                         json.loads(m.canonical(m.inventory(self.rows))))
        self.assertFalse(fixture["historical_captures_are_qualified_kills"])
        self.assertFalse(fixture["test_fixtures_are_fresh_solver_runs"])
        self.assertFalse(fixture["original_timeout_campaign_complete"])
        self.assertEqual(fixture["fresh_signed_roster_due"], m.COUNTS)
        self.assertEqual(fixture["generator_sha256"], "64bfe7dda8e7e7891c61a8148292e0dd66af3beddd023e7b8b6ebdc5c628e957")


if __name__ == "__main__":
    unittest.main(verbosity=2)
