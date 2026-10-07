#!/usr/bin/env python3
"""Source-only controls; no solver or native CPU execution."""
from pathlib import Path
import types
import unittest

path = Path(__file__).with_name("check-producer-journal-observers.py")
check = types.ModuleType("producer_journal_observer_controls")
check.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = check.snapshot()

    def reject(self, function, sources):
        with self.assertRaises(ValueError):
            function(sources)

    def test_actual_complete_closure_and_unqualified_scope(self):
        check.audit(self.sources)
        self.assertEqual(len(check.files()), 38)
        self.assertEqual(check.EXPECTED_VERIFIED, 165)
        self.assertIs(check.QUALIFIED, False)
        self.assertEqual(check.diagnostic_classifier().CASES, set(check.mutations(self.sources[check.BODY])))

    def test_field_projection_and_borrowed_signature_are_bound(self):
        for path, old, new in (
            (check.OWNER, "versions: &'a ContextVersionsV1", "versions: &'a OtherVersions"),
            (check.OWNER, "root: &'a RetainedProducerReadV1", "root: RetainedProducerReadV1"),
            (check.VERSIONS, "journal: ContextQueuedWriterJournalV1", "journal: ContextProducerReadJournalV1"),
            (check.OWNER, "fn observe_active_lookup(\n        &mut self", "fn observe_active_lookup(\n        &self"),
        ):
            altered = dict(self.sources)
            selected = (check.block(altered[path], "struct ProducerInputObservationsV1<'a, B: RuntimeBackendV1>")
                        if path == check.OWNER and old.startswith(("versions:", "root:")) else altered[path])
            self.assertEqual(selected.count(old), 1)
            self.assertEqual(altered[path].count(selected), 1)
            altered[path] = altered[path].replace(selected, selected.replace(old, new))
            with self.subTest(old=old):
                self.reject(check.forwarders, altered)

    def test_preparation_module_move_preserves_the_historical_owner(self):
        check.forwarders(self.sources)
        reconstructed = check.reconstruct_preparation(self.sources)
        self.assertNotIn("mod preparation;", reconstructed)
        self.assertIn("    fn prepare_pending_input_v1(", reconstructed)
        self.assertIn("    pub(super) fn prepare_producer_read_v1(", reconstructed)

    def test_preparation_module_missing_redirected_or_mutated_is_refused(self):
        altered = dict(self.sources)
        del altered[check.PREPARATION]
        self.reject(check.forwarders, altered)
        for path, old, new in (
            (check.OWNER, "mod preparation;", "mod other_preparation;"),
            (check.OWNER, "mod preparation;", '#[path = "other.rs"] mod preparation;'),
            (check.PREPARATION, "use super::*;", "use another::*;"),
            (check.PREPARATION, "fn prepare_pending_input_v1(", "fn extra_prepare_pending_input_v1("),
            (check.PREPARATION, "pub(super) fn prepare_pending_input_v1(",
             "pub fn prepare_pending_input_v1("),
            (check.PREPARATION, "pub(in crate::context::versions) fn prepare_producer_read_v1(",
             "pub(crate) fn prepare_producer_read_v1("),
            (check.PREPARATION, "self.allocations.get(&source.region.allocation)",
             "self.allocations.get(&other.region.allocation)"),
        ):
            altered = dict(self.sources)
            self.assertEqual(altered[path].count(old), 1)
            altered[path] = altered[path].replace(old, new)
            with self.subTest(path=path, old=old):
                self.reject(check.forwarders, altered)
        for extra in ("\nfn extra() {}\n", "\nimpl Other {}\n"):
            altered = dict(self.sources)
            altered[check.PREPARATION] += extra
            self.reject(check.forwarders, altered)

    def test_each_route_is_one_actual_call_with_exact_reference(self):
        for name, (_reference, _result, query) in check.ROUTES.items():
            macro = "producer_observe_" + name + "_body_v1"
            for path, old, new in (
                (check.OWNER, macro + "!(self, reference)", macro + "!(self, other_reference)"),
                (check.BODY, "$observer.versions.journal." + query + "($reference)",
                 "$observer.versions.other_journal." + query + "($reference)"),
                (check.PROOF, macro + "!(self, reference)", "supplied_answer"),
            ):
                altered = dict(self.sources)
                self.assertEqual(altered[path].count(old), 1)
                altered[path] = altered[path].replace(old, new)
                with self.subTest(name=name, path=path):
                    self.reject(check.forwarders, altered)

    def test_schema_copies_and_supplied_returns_are_rejected(self):
        for extra in ("\nstruct ContextProducerReadV1 {}\n", "\nstruct Returns {}\n"):
            altered = dict(self.sources)
            altered[check.PROOF] += extra
            self.reject(check.forwarders, altered)

    def test_missing_extra_and_redirected_closure_are_rejected(self):
        altered = dict(self.sources)
        del altered[check.inherited().PROOF]
        self.reject(check.closure, altered)
        altered = dict(self.sources)
        altered[check.PROOF] += '\ninclude!("unexpected.rs");\n'
        self.reject(check.closure, altered)
        altered[check.PROOF] = self.sources[check.PROOF] + '\n#[path = "unexpected.rs"] mod extra;\n'
        self.reject(check.closure, altered)

    def test_complete_source_roster_excludes_shadowing(self):
        for path in (check.RUNTIME / "shadow.rs", check.MODEL / "shadow.rs"):
            altered = dict(self.sources)
            altered[path] = "impl ContextQueuedWriterJournalV1 {}\n"
            self.reject(check.audit, altered)
        altered = dict(self.sources)
        altered[check.VERSIONS] += "\n// drift\n"
        self.reject(check.audit, altered)

    def test_all_nine_actual_body_mutants_are_distinct_and_unqualified(self):
        body = self.sources[check.BODY]
        rows = check.mutations(body)
        expected = {name + "-" + suffix for name in check.ROUTES for suffix in ("constant-error", "error-coerced")}
        expected.add("active_lookup-prefetch-status")
        self.assertEqual(set(rows), expected)
        self.assertEqual(len({text for text, _ in rows.values()}), 9)
        originals = {name: check.block(body, "macro_rules! producer_observe_" + name + "_body_v1 {")
                     for name in check.ROUTES}
        for name, (text, boundary) in rows.items():
            target = boundary.removeprefix("native_observers::Observations::observe_")
            self.assertIn(target, check.ROUTES)
            changed = [route for route in check.ROUTES if check.block(text,
                "macro_rules! producer_observe_" + route + "_body_v1 {") != originals[route]]
            self.assertEqual(changed, [target], name)
            altered = dict(self.sources)
            altered[check.BODY] = text
            self.reject(check.forwarders, altered)

    def test_active_lookup_terminal_asymmetry_remains_explicit(self):
        prior = check.inherited()
        outer = self.sources[prior.OUTER_BODY]
        lookup = check.block(outer, "macro_rules! queued_active_lookup_body_v1 {")
        status = check.block(outer, "macro_rules! queued_active_status_body_v1 {")
        self.assertNotIn("ensure_usable", lookup)
        self.assertIn("ensure_usable", status)
        self.assertIn("producer_read_status($reference)?;", check.mutations(self.sources[check.BODY])["active_lookup-prefetch-status"][0])


if __name__ == "__main__":
    unittest.main(verbosity=2)
