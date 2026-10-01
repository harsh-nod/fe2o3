#!/usr/bin/env python3
"""Deterministic source rejection controls, not solver or native results."""
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("cadence_guard", Path(__file__).with_name("check-retained-pair-cadence.py"))
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.sources = guard.snapshot()

    def reject(self, path, before, after, anchor=None):
        prefix, selected = (self.sources[path].split(anchor, 1) if anchor else ("", self.sources[path]))
        self.assertIn(before, selected)
        self.sources[path] = prefix + (anchor or "") + selected.replace(before, after, 1)
        with self.assertRaises(ValueError):
            guard.audit(self.sources)

    def test_exact_source_binding_has_no_solver_or_native_authority(self):
        result = guard.audit(self.sources)
        self.assertTrue(result["source_binding"])
        self.assertFalse(result["solver_executed"])
        self.assertFalse(result["formal_refinement"])
        self.assertFalse(result["performance_acceptance"])
        self.assertEqual(len(result["proof_closure"]), 3)

    def test_missing_or_extra_source_rejected(self):
        for operation in (lambda rows: rows.pop(guard.DECL),
                          lambda rows: rows.update({Path("extra.rs"): ""})):
            rows = dict(self.sources)
            operation(rows)
            with self.assertRaises(ValueError):
                guard.audit(rows)

    def test_logical_mutation_candidates_are_distinct_and_source_rejected(self):
        mutants = guard.mutations(self.sources[guard.BODY])
        self.assertEqual(len(mutants), 3)
        for body, focus in mutants.values():
            self.assertEqual(focus, "*retained_pair_cadence_ceiling_v1")
            rows = dict(self.sources)
            rows[guard.BODY] = body
            with self.assertRaises(ValueError):
                guard.audit(rows)

    def test_selector_contract_cannot_weaken_or_add_trust(self):
        for before, after in (("0 < ceiling <= 1_000_000", "true"),
                              ("    ensures", "    requires false,\n    ensures"),
                              ("    retained_pair_cadence_ceiling_body_v1!(cadence)", "    assume(false); 0")):
            rows = dict(self.sources)
            self.sources = rows
            self.reject(guard.PROOF, before, after)
            self.sources = guard.snapshot()

    def test_fresh_deadline_and_active_spin_floor_are_rejected(self):
        for replacement in ("MonotonicWaitV1::until(Instant::now())",
                            "MonotonicWaitV1::until_with_active_spin_floor(deadline, Duration::from_secs(1))"):
            self.reject(guard.RUST, "MonotonicWaitV1::until(deadline)", replacement)
            self.sources = guard.snapshot()

    def test_existing_ordinary_entrypoint_cannot_select_short(self):
        self.reject(guard.SRC / "sdma.rs", "XgmiWaitCadence::Ordinary1ms,", "XgmiWaitCadence::Ceiling25us,",
                    "    fn wait_many_xgmi_for_in_current_scope(")

    def test_retained_scope_cannot_skip_custody_or_enable_timer(self):
        path = guard.SRC / "sdma/retained_pair.rs"
        for before, after in (("run_operation(&mut self.scope.context", "skip_operation(&mut self.scope.context"),
                              ("&mut XgmiWaitTimer::<false>::new()", "&mut XgmiWaitTimer::<true>::new()")):
            self.reject(path, before, after, "    pub fn wait_batch_for_cadence_experiment_v1(")
            self.sources = guard.snapshot()
        self.reject(path, '#[cfg(feature = "hardware-diagnostic")]\n    pub fn wait_batch_for_cadence_experiment_v1',
                    'pub fn wait_batch_for_cadence_experiment_v1')

    def test_scan_deadline_checks_and_retirement_are_bound(self):
        for before, after, anchor in (("let deadline = deadline.resolve()?;", "let deadline = Instant::now();",
                                      "    fn wait_many_xgmi_with_timer<"),
                                     ("Self::validate_route_currentness(", "Self::skip_route_currentness(",
                                      "    fn wait_batch_with_timer<"),
                                     ("let mut ready = vec![false; slots.len()];", "let mut ready = vec![true; slots.len()];",
                                      "    fn wait_many_xgmi_with_timer<")):
            self.reject(guard.SRC / "sdma.rs", before, after, anchor)
            self.sources = guard.snapshot()

    def test_wait_production_change_rejected_but_test_comment_allowed(self):
        self.sources[guard.WAIT] += "\n// source-only test comment\n"
        guard.audit(self.sources)
        self.reject(guard.WAIT, "const SPIN_ATTEMPTS_V1: u32 = 64;", "const SPIN_ATTEMPTS_V1: u32 = 640;")


if __name__ == "__main__":
    unittest.main()
