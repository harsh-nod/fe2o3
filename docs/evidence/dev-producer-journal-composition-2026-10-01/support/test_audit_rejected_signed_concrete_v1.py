#!/usr/bin/env python3
"""Readback predicate controls; never inspect an unfinished campaign."""
import copy
from pathlib import Path
import types
import unittest

BASE = Path(__file__).resolve().parent
m = types.ModuleType("tested_signed_concrete_auditor")
m.__file__ = str(BASE / "audit_rejected_signed_concrete_v1.py")
exec(compile(Path(m.__file__).read_bytes(), m.__file__, "exec"), m.__dict__)
p = m.module(BASE / "post_audit_v1.py", m.HELPER_SHA, "tested_readback_helpers")


class AuditControls(unittest.TestCase):
    def test_terminal_gate_rejects_active_missing_extra_or_wrong_hash(self):
        packet, durable = Path("/packet"), Path("/durable")
        pins = {str(packet / name): "a" * 64 for name in ("results.json", "fresh-process-closure.json")}
        pins.update({str(durable / name): "a" * 64 for name in ("packet.tar", "members.json", "receipt.json")})
        m.terminal(pins, packet, durable, lambda _: "a" * 64)
        for variant in ({}, dict(list(pins.items())[1:]), dict(pins, extra="a" * 64), dict(pins, **{"/packet/results.json": "b" * 64})):
            with self.assertRaises(ValueError):
                m.terminal(variant, packet, durable, lambda _: "a" * 64)

    def test_exact_complete_roster(self):
        commands = [{"name": str(i), "kind": "negative"} for i in range(112)]
        rows = [dict(row, accepted=True, owned_launch_attempted=True, group_absent=True) for row in commands[:109]]
        m.roster(commands, rows)
        for variant in (rows[:-1], rows[::-1], [dict(rows[0], accepted=False)] + rows[1:],
                        [dict(rows[0], group_absent=False)] + rows[1:], [dict(rows[0], kind="positive")] + rows[1:]):
            with self.assertRaises(ValueError):
                m.roster(commands, variant)

    def test_resource_only_rejection_cannot_be_promoted(self):
        result = {"signed_campaign_checks_passed": False, "errors": list(m.ERRORS)}
        m.rejection(result)
        for variant in (dict(result, signed_campaign_checks_passed=True), dict(result, errors=[]),
                        dict(result, errors=m.ERRORS[:-1]), dict(result, errors=m.ERRORS + ["unexpected proof error"])):
            with self.assertRaises(ValueError):
                m.rejection(variant)

    def test_every_boundary_and_nonpromotion_flag(self):
        commands, details = [], []
        for family, count in (("leaf", 38), ("conditional", 21), ("concrete", 30)):
            for i in range(count):
                commands.append({"kind": "negative", "family": family})
                row = {"logical_diagnostic_accepted": True, "qualified_kill": False,
                       "historical_capture_is_qualified_kill": False, "signed_campaign_qualified": False}
                if family == "concrete" and i < 9:
                    row["forwarding"] = {"boundary": "actual-journal-result-equality" if i < 8 else "wrapper-ghost-trace-only",
                        "actual_result_failure_observed": i < 8, "inner_query_call_count_proved": False}
                details.append({"classification": row})
        self.assertEqual(m.boundary_counts(commands, details), {"leaf": 38, "conditional": 21, "concrete": 30})
        for key in ("qualified_kill", "historical_capture_is_qualified_kill", "signed_campaign_qualified"):
            wrong = copy.deepcopy(details)
            wrong[0]["classification"][key] = True
            with self.assertRaises(ValueError):
                m.boundary_counts(commands, wrong)
        for key in ("actual_result_failure_observed", "inner_query_call_count_proved"):
            wrong = copy.deepcopy(details)
            wrong[67]["classification"]["forwarding"][key] = True
            with self.assertRaises(ValueError):
                m.boundary_counts(commands, wrong)
        with self.assertRaises(ValueError):
            m.boundary_counts(commands[:-1], details[:-1])

    def test_exact_archive_key_hash_and_mapping_join(self):
        pins = {"/one": "a" * 64, "/two": "b" * 64}
        index = {path: {"archive": "raw-helpers/" + path.lstrip("/") + ".source", "sha256": value} for path, value in pins.items()}
        p.archive_join(index, pins)
        for variant in ({"/one": index["/one"]}, dict(index, extra=index["/one"]),
                        dict(index, **{"/one": dict(index["/one"], sha256="c" * 64)}),
                        dict(index, **{"/one": dict(index["/one"], archive=index["/two"]["archive"])})):
            with self.assertRaises(ValueError):
                p.archive_join(variant, pins)

    def test_saved_groups_are_original_unique_and_complete(self):
        groups, names, namespace = list(range(1000, 1112)), list(map(str, range(112))), {"namespace": "fixture"}
        roster = [name + "/owned/record.json" for name in names]
        census = {"recorded_groups": groups, "namespace": namespace, "all_recorded_groups_absent": True, "members": [], "uncertain": []}
        p.saved_census(census, groups, namespace, roster, names)
        for changed in (dict(census, recorded_groups=groups[:-1]), dict(census, all_recorded_groups_absent=False),
                        dict(census, members=[1]), dict(census, uncertain=[1])):
            with self.assertRaises(ValueError):
                p.saved_census(changed, groups, namespace, roster, names)
        with self.assertRaises(ValueError):
            p.saved_census(census, groups, namespace, roster[:-1], names)

    def test_auditor_has_no_solver_process_or_packet_write_path(self):
        text = Path(m.__file__).read_text()
        self.assertNotIn("run_owned(", text)
        self.assertNotIn("independent_absence(", text)
        self.assertNotIn("subprocess", text)
        self.assertNotIn(".unlink(", text)
        self.assertEqual(text.count('.open("xb")'), 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
