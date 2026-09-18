#!/usr/bin/env python3
"""CPU-only tests of actual cleanup/absence command wiring and remote parsers."""

import json
import shlex
import unittest

import controller
import remote_control


class Recorder:
    def __init__(self, fail_first=False):
        self.calls = []
        self.fail_first = fail_first

    def run(self, name, command, seconds, stdin=None):
        self.calls.append((name, command, seconds, stdin))
        return {
            "status": 1 if self.fail_first and len(self.calls) == 1 else 0,
            "error": None,
            "group_absent": True,
        }, None


class ControllerTests(unittest.TestCase):
    def test_actual_cleanup_absence_calls_match_remote_parsers(self):
        recorder = Recorder()
        state = {"cleanup_closed": False, "independent_absence_closed": False}
        pids, digest = [123, 456], "a" * 64
        controller.cleanup_and_absence(recorder, state, pids, digest)
        self.assertEqual(len(recorder.calls), 2)
        cleanup, absence = [shlex.split(call[1][-1]) for call in recorder.calls]
        self.assertEqual(cleanup[3:5], ["cleanup", controller.OWNED])
        self.assertEqual(absence[3:5], ["absence", controller.OWNED])
        self.assertEqual(
            remote_control.cleanup_argument(cleanup[5]),
            {"pids": pids, "inventory_sha256": digest},
        )
        self.assertEqual(remote_control.absence_argument(absence[5]), pids)
        self.assertTrue(all(call[2] == 120 for call in recorder.calls))
        self.assertTrue(
            all(
                call[3] == (controller.HERE / "remote_control.py").read_bytes()
                for call in recorder.calls
            )
        )
        self.assertEqual(
            state, {"cleanup_closed": True, "independent_absence_closed": True}
        )

    def test_swapped_wire_types_are_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "cleanup requires"):
            remote_control.cleanup_argument(json.dumps([123, 456]))
        with self.assertRaisesRegex(RuntimeError, "PID roster"):
            remote_control.absence_argument(
                json.dumps({"pids": [123, 456], "inventory_sha256": "a" * 64})
            )

    def test_invalid_rosters_and_inventory_are_rejected(self):
        for pids in ([], [True], [0], [123, 123], "123"):
            with self.subTest(pids=pids), self.assertRaises(RuntimeError):
                remote_control.absence_argument(json.dumps(pids))
        for digest in (None, "bad", "a" * 63):
            with self.subTest(digest=digest), self.assertRaises(RuntimeError):
                remote_control.cleanup_argument(
                    json.dumps({"pids": [123], "inventory_sha256": digest})
                )

    def test_cleanup_failure_never_runs_absence(self):
        recorder = Recorder(fail_first=True)
        state = {"cleanup_closed": False, "independent_absence_closed": False}
        with self.assertRaisesRegex(RuntimeError, "cleanup completed"):
            controller.cleanup_and_absence(recorder, state, [123], "a" * 64)
        self.assertEqual(len(recorder.calls), 1)
        self.assertEqual(
            state, {"cleanup_closed": False, "independent_absence_closed": False}
        )


if __name__ == "__main__":
    unittest.main()
