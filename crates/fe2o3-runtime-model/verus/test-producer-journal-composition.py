#!/usr/bin/env python3
"""Concrete composition source controls only; no compiler or logical qualification."""
import hashlib
from pathlib import Path
import types
import unittest

path = Path(__file__).with_name("check-producer-journal-composition.py")
check = types.ModuleType("concrete_journal_composition")
check.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = check.snapshot()
        cls.leaf = check.helper("check-producer-input-validate.py")
        cls.conditional = check.helper("check-producer-input-composition.py")
        cls.wrapper = check.helper("check-producer-journal-observers.py")

    def changed(self, old, new, path=check.PROOF):
        sources = dict(self.sources)
        self.assertEqual(sources[path].count(old), 1)
        sources[path] = sources[path].replace(old, new)
        return sources

    def test_complete_actual_closure_not_inherited_acceptance(self):
        check.audit(self.sources)
        files = check.files()
        self.assertEqual(len(files), 44)
        self.assertEqual(set(self.wrapper.inherited().FILES), set(files) & set(self.wrapper.inherited().FILES))
        self.assertIn(check.ENROLLMENT, files)
        self.assertNotIn(self.leaf.JOURNAL_DECLARATIONS, files)
        self.assertNotIn(self.leaf.DEFINITIONS, files)
        self.assertNotIn(self.wrapper.PROOF, files)
        self.assertIsNone(check.EXPECTED_VERIFIED)
        self.assertFalse(check.QUALIFIED)

    def test_all_actual_inputs_are_bound(self):
        for path in check.files():
            with self.subTest(path=path):
                changed = dict(self.sources)
                changed[path] += "\n"
                with self.assertRaises(ValueError):
                    check.audit(changed)
                del changed[path]
                with self.assertRaises(ValueError):
                    check.audit(changed)

    def test_no_unbound_include_or_added_source(self):
        for changed in (
            {**self.sources, check.V / "unbound.rs": "// not an input\n"},
            self.changed('include!("context_queued_query_execution_v1.rs");',
                         'include!("context_queued_query_execution_v1.rs");\ninclude!("unbound.rs");'),
        ):
            with self.assertRaises(ValueError):
                check.audit(changed)

    def test_actual_types_and_only_two_external_boundaries(self):
        for old, new in (
            ("credit: bool,\n        live:", "credit: bool,\n        active_status: StatusResult,\n        live:"),
            ("journal: ContextQueuedWriterJournalV1,\n        external:", "journal: u64,\n        external:"),
            ('    use super::*;', '    use super::*;\n    struct ContextProducerReadV1 { value: u64 }'),
            ("spec fn owner_view(&self) -> Owner", "fn owner_view(&self) -> Owner"),
        ):
            # The owner view is deliberately repeated in both concrete adapters.
            changed = dict(self.sources)
            changed[check.PROOF] = changed[check.PROOF].replace(old, new)
            with self.assertRaises(ValueError):
                check.semantics(changed)

    def test_each_actual_forwarder_and_incoming_cursor(self):
        for name, decision, reference in (
            ("active_lookup", "queued_active_lookup_decision_v1", "root.references@[active as int]"),
            ("active_status", "queued_active_status_decision_v1", "root.references@[active as int]"),
            ("queued_lookup", "reads::lookup_decision_v1", "root.queued_references@[queued as int]"),
            ("queued_status", "reads::status_decision_v1", "root.queued_references@[queued as int]"),
        ):
            for old, new in (
                ("let result = producer_observe_" + name + "_body_v1!(self, reference);",
                 "let result = Err(ContextVersionJournalErrorV1::InvalidReference);"),
                (decision + "(journal, " + reference + ")", decision + "(journal, " + reference.replace(" as int", " as int + 1") + ")"),
            ):
                with self.subTest(name=name, anchor=old), self.assertRaises(ValueError):
                    check.semantics(self.changed(old, new))
        changed = dict(self.sources)
        changed[check.PROOF] = changed[check.PROOF].replace(
            "old(self).answers(*old(active), *old(queued))", "old(self).answers(*final(active), *final(queued))")
        with self.assertRaises(ValueError):
            check.semantics(changed)

    def test_full_owner_frame_and_per_input_external_values(self):
        for old, new in (
            ("self.versions == before.versions", "true"),
            ("let external = self.returns[index];", "let external = self.returns[0];"),
            ("producer_input_validate_body!(verus_exec_expr, context, root, id,", "other_validate_body!(verus_exec_expr, context, root, id,"),
        ):
            changed = dict(self.sources)
            changed[check.PROOF] = changed[check.PROOF].replace(old, new)
            with self.assertRaises(ValueError):
                check.semantics(changed)

    def test_actual_validator_error_receipt_and_fold_are_not_skipped(self):
        for old, new in (
            ("let result = validation.validate(index, active, queued);",
             "let result = validation.validate(index, active, queued)?;"),
            ("self.calls@ = self.calls@ + validation.calls@;", "self.calls@ = validation.calls@;"),
            ("self.consumed@ = (index + 1) as nat;", "self.consumed@ = index as nat;"),
            ("producer_input_fold_body!(verus_exec_expr, self, invalid_reference,", "other_fold_body!(verus_exec_expr, self, invalid_reference,"),
        ):
            with self.assertRaises(ValueError):
                check.semantics(self.changed(old, new))

    def test_shared_prefix_uses_reached_incoming_cursor(self):
        path = self.conditional.LOGIC
        old = "source_answers(owner, answers, index, before.active, before.queued)"
        with self.assertRaises(ValueError):
            check.semantics(self.changed(old, "source_answers(owner, answers, index, 0, 0)", path))
        old = "ProducerReadRequestV1::Active(_) => (fold::Family::Active, active_after != active_before)"
        with self.assertRaises(ValueError):
            check.semantics(self.changed(old, old.replace("active_after != active_before", "result is Ok"), path))

    def test_standalone_fold_and_wrapper_proof_closures_unchanged(self):
        fold = self.conditional.load_checker("check-producer-input-fold.py")
        folded = fold.snapshot()
        fold.audit(folded)
        self.wrapper.audit(self.wrapper.snapshot())
        for path in set(fold.FILES) | set(self.wrapper.files()):
            actual = (check.ROOT / path).read_bytes()
            expected = self.sources.get(path, folded.get(path))
            self.assertIsNotNone(expected)
            self.assertEqual(hashlib.sha256(actual).hexdigest(), check.sha(expected))

    def test_no_assumed_execution_or_inherited_count(self):
        changed = self.changed("let external = self.returns[index];", "assume(false);\n            let external = self.returns[index];")
        with self.assertRaises(ValueError):
            check.semantics(changed)
        old_count, old_qualified = check.EXPECTED_VERIFIED, check.QUALIFIED
        try:
            for count in (42, 64, 165, True):
                check.EXPECTED_VERIFIED = count
                with self.assertRaises(ValueError):
                    check.audit(self.sources)
            check.EXPECTED_VERIFIED = None
            check.QUALIFIED = True
            with self.assertRaises(ValueError):
                check.audit(self.sources)
        finally:
            check.EXPECTED_VERIFIED, check.QUALIFIED = old_count, old_qualified


if __name__ == "__main__":
    unittest.main()

