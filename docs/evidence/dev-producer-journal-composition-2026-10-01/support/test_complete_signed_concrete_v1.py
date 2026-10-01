#!/usr/bin/env python3
"""Source and mock controls for the minimal109+3 completion, no solver."""
import json
from pathlib import Path
import types
import unittest

BASE = Path(__file__).resolve().parent
m = types.ModuleType("tested_completion")
m.__file__ = str(BASE / "complete_signed_concrete_v1.py")
exec(compile(Path(m.__file__).read_bytes(), m.__file__, "exec"), m.__dict__)


class CompletionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.base = m.module(BASE / "signed_concrete_campaign_v1.py", m.OWNER_SHA, "tested_original_owner")
        cls.original = json.loads(cls.base.PREPARED.read_bytes())
        cls.prefix = json.loads((cls.base.OUTPUT / "results.json").read_bytes())

    def admission(self, values, delays=()):
        self.now, self.saved = 0, []
        values, delays = iter(values), iter(delays)
        resources = types.SimpleNamespace(RAM_FLOOR=16 * 1024**3, save=lambda path, row: self.saved.append(row.copy()))
        def observe():
            self.now += next(delays, 0)
            return next(values)
        def sleep(seconds):
            self.now += int(seconds * 10**9)
        return m.AdmissionBudget(resources, lambda: self.now, sleep, observe)

    def test_immediate_admission_at_exact_floor(self):
        budget = self.admission([16 * 1024**3])
        budget.wait(Path("/unused"))
        self.assertTrue(self.saved[-1]["accepted"])

    def test_recovery_wait_is_cumulative_across_stages(self):
        budget = self.admission([0, 16 * 1024**3, 0, 16 * 1024**3])
        budget.wait(Path("/one"))
        budget.wait(Path("/two"))
        self.assertEqual(budget.spent_ns, 2 * 10**9)
        self.assertEqual(self.saved[1]["spent_ns_before"], 10**9)

    def test_late_recovery_after_slow_observer_rejected(self):
        budget = self.admission([16 * 1024**3], [m.WAIT_BUDGET_NS])
        with self.assertRaises(ValueError):
            budget.wait(Path("/unused"))
        self.assertFalse(self.saved[-1]["accepted"])

    def test_late_recovery_after_oversleep_rejected(self):
        budget = self.admission([0, 16 * 1024**3])
        def oversleep(_):
            self.now += m.WAIT_BUDGET_NS + 1
        budget.sleep = oversleep
        with self.assertRaises(ValueError):
            budget.wait(Path("/unused"))
        self.assertFalse(self.saved[-1]["accepted"])

    def test_final_budget_check_cannot_acknowledge_late_admission(self):
        budget = self.admission([16 * 1024**3])
        clocks = iter([0, 1, m.WAIT_BUDGET_NS])
        budget.clock = lambda: next(clocks)
        with self.assertRaises(ValueError):
            budget.wait(Path("/unused"))
        self.assertFalse(self.saved[-1]["accepted"])

    def test_exhausted_shared_budget_cannot_restart(self):
        budget = self.admission([0] * 301 + [16 * 1024**3])
        with self.assertRaises(ValueError):
            budget.wait(Path("/one"))
        with self.assertRaises(ValueError):
            budget.wait(Path("/two"))

    def test_exact_original_three_commands_and_bounds(self):
        rows = m.closing_plan(self.original)
        self.assertEqual(rows, self.original["commands"][-3:])
        self.assertEqual([row["timeout"] for row in rows], [130, 130, 60])
        self.assertEqual(rows[0]["argv"][:5], ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120"])
        self.assertEqual(rows[0]["argv"][-1], str(self.base.REPO / self.base.V / "context_producer_journal_composition_v1.rs"))

    def test_original_qualifier_composes_only_exact109_plus3(self):
        commands = m.closing_plan(self.original)
        rows = [dict(row, accepted=True, group_absent=True) for row in commands]
        self.assertTrue(m.composed_qualified(self.base, self.original, self.prefix, commands, rows, [], True))
        for bad in (rows[:-1], rows[::-1], [dict(rows[0], accepted=False)] + rows[1:]):
            self.assertFalse(m.composed_qualified(self.base, self.original, self.prefix, commands, bad, [], True))
        self.assertFalse(m.composed_qualified(self.base, self.original, self.prefix, commands, rows, ["error"], True))
        self.assertFalse(m.composed_qualified(self.base, self.original, self.prefix, commands, rows, [], False))
        self.assertFalse(self.prefix["signed_campaign_checks_passed"])

    def test_main_transform_is_exactly_reversible(self):
        text = Path(self.base.__file__).read_text()
        text = text[text.index("def main():"):text.index('\n\nif __name__ == "__main__":')]
        original = text
        for before, after in m.MAIN_EDITS:
            self.assertEqual(text.count(before), 1)
            text = text.replace(before, after)
        for before, after in reversed(m.MAIN_EDITS):
            self.assertEqual(text.count(after), 1)
            text = text.replace(after, before)
        self.assertEqual(text, original)
        self.assertEqual(m.build().OUTPUT, m.OUTPUT)

    def test_disk_guard_removes_only_ram_check(self):
        path = Path("/home/harsh/.codex-tmp/fe2o3-a2-concrete-journal-validation-records-20261001-audit/mutation_capture_v1.py")
        resources = m.module(path, m.RESOURCE_SHA, "tested_disk_resources")
        old = resources.footprint
        m.disk_only(resources)
        self.assertIsNot(old, resources.footprint)
        self.assertNotIn("available_memory", resources.footprint.__code__.co_names)
        self.assertIn("disk_usage", resources.footprint.__code__.co_names)
        self.assertEqual(resources.RAM_FLOOR, 16 * 1024**3)


if __name__ == "__main__":
    unittest.main(verbosity=2)
