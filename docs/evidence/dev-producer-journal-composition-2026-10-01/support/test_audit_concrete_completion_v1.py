#!/usr/bin/env python3
"""Narrow readback predicates; never audit the live completion packet."""
import copy
from pathlib import Path
import types
import unittest

BASE = Path(__file__).resolve().parent
m = types.ModuleType("tested_completion_readback")
m.__file__ = str(BASE / "audit_concrete_completion_v1.py")
exec(compile(Path(m.__file__).read_bytes(), m.__file__, "exec"), m.__dict__)


class CompletionAuditControls(unittest.TestCase):
    def row(self):
        return {"accepted": True, "budget_ns": 300 * 10**9, "floor_bytes": 16 * 1024**3,
            "spent_ns_before": 0, "spent_ns_after": 10**9 + 2, "started_monotonic_ns": 100,
            "samples": [{"elapsed_ns": 1, "available_bytes": 0}, {"elapsed_ns": 10**9 + 1, "available_bytes": 16 * 1024**3}]}

    def test_exact_typed_ordered_admission(self):
        self.assertEqual(m.admission(self.row(), 0, 0), (10**9 + 2, 10**9 + 102))

    def test_low_or_expired_or_error_admission_rejected(self):
        original = self.row()
        for key, value in (("accepted", False), ("budget_ns", 600 * 10**9), ("floor_bytes", 1),
                           ("spent_ns_after", 300 * 10**9), ("spent_ns_before", 1), ("error", "failure")):
            with self.assertRaises(ValueError):
                m.admission(dict(original, **{key: value}), 0, 0)
        changed = copy.deepcopy(original)
        changed["samples"][-1]["available_bytes"] = 16 * 1024**3 - 1
        with self.assertRaises(ValueError):
            m.admission(changed, 0, 0)

    def test_cumulative_budget_and_order_cannot_restart(self):
        first = self.row()
        spent, end = m.admission(first, 0, 0)
        with self.assertRaises(ValueError):
            m.admission(first, spent, end)
        second = dict(first, spent_ns_before=spent, spent_ns_after=spent + 10**9 + 2, started_monotonic_ns=end)
        m.admission(second, spent, end)
        with self.assertRaises(ValueError):
            m.admission(dict(second, started_monotonic_ns=end - 1), spent, end)

    def test_sample_types_bounds_and_first_recovery(self):
        for index, key, value in ((0, "available_bytes", 16 * 1024**3), (1, "elapsed_ns", -1),
                                  (1, "elapsed_ns", 10**9 + 3), (1, "available_bytes", True)):
            changed = self.row()
            changed["samples"][index][key] = value
            with self.assertRaises(ValueError):
                m.admission(changed, 0, 0)

    def test_terminal_gate_requires_complete_exact_six(self):
        packet, durable = Path("/packet"), Path("/durable")
        pins = {str(packet / name): "a" * 64 for name in ("results.json", "fresh-process-closure.json", "closing-inputs.json")}
        pins.update({str(durable / name): "a" * 64 for name in ("packet.tar", "members.json", "receipt.json")})
        m.terminal(pins, packet, durable, lambda _: "a" * 64)
        for wrong in ({}, dict(list(pins.items())[1:]), dict(pins, extra="a" * 64)):
            with self.assertRaises(ValueError):
                m.terminal(wrong, packet, durable, lambda _: "a" * 64)

    def test_only_external_report_write_no_solver_or_pid_probe(self):
        source = Path(m.__file__).read_text()
        self.assertNotIn("run_owned(", source)
        self.assertNotIn("independent_absence(", source)
        self.assertNotIn("subprocess", source)
        self.assertEqual(source.count('.open("xb")'), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
