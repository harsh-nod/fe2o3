#!/usr/bin/env python3
"""CPU-only adversarial calibrations for the retained historical audit."""

import copy
from pathlib import Path
import tempfile
import unittest

import verify as V


class ArchiveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.payload = V.ROOT / V.COLLECTED
        cls.native = V.ROOT / "raw/native"
        cls.P = V.module(cls.payload / "protocol.py")

    def test_complete_historical_audit(self):
        value = V.audit(V.ROOT, allow_unsealed=True)
        self.assertEqual(value["native_campaign"], "rejected")
        self.assertEqual(value["positive"], "passed")
        self.assertEqual(value["error"], "failed-before-injection")
        self.assertEqual(value["panic"], "not-run")
        self.assertFalse(value["current_binary_revalidated"])

    def test_missing_seal_requires_explicit_review_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(ValueError):
                V.manifest(Path(folder), False)

    def test_controller_cannot_upgrade_or_erase_failure(self):
        value = V.load(self.native / "controller.json")
        for field, replacement in (
            ("native_outer_passed", True),
            ("failure", None),
            ("cleanup_closed", False),
            ("owned", V.OLD),
        ):
            with self.subTest(field=field), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                changed[field] = replacement
                V.controller(changed)

    def test_absence_requires_exact_roster_and_scope(self):
        value = V.lines(self.native / "remote-independent-absence/stdout.log")[0]
        pids = [row["pid"] for row in value["recorded_processes"]]
        mutations = [
            ("missing-pid", lambda row: row["recorded_processes"].pop()),
            (
                "present-group",
                lambda row: row["recorded_processes"][0].update(
                    process_group_absent=False
                ),
            ),
            (
                "visible-reference",
                lambda row: row["accessible_references"].append({"path": V.NEW}),
            ),
            ("overclaimed-scope", lambda row: row.update(scope="all processes absent")),
        ]
        for label, mutate in mutations:
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                mutate(changed)
                V.absence(changed, V.NEW, pids)

    def test_error_cannot_be_reported_as_injection_success(self):
        out = (self.payload / "results/error-test/stdout.log").read_text()
        err = (self.payload / "results/error-test/stderr.log").read_text()
        for changed_out, changed_err in (
            (out + self.P.MARKERS["error"], err),
            (
                out,
                err.replace("primary_envelope.rs:165:5", "primary_envelope.rs:999:5"),
            ),
            (
                out.replace("FAILED. 0 passed; 1 failed;", "ok. 1 passed; 0 failed;"),
                err,
            ),
        ):
            with self.subTest(), self.assertRaises(ValueError):
                V.error_transcript(changed_out, changed_err, self.P.TESTS["error"])

    def test_inventory_cannot_omit_or_replace_retained_files(self):
        value = V.lines(self.native / "remote-inventory/stdout.log")[0]
        for label in ("missing", "wrong-elf", "extra"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "missing":
                    changed["files"].pop("results/error-test/stderr.log")
                elif label == "wrong-elf":
                    changed["files"]["runtime-tests"] = "0" * 64
                else:
                    changed["files"]["results/panic-test/stdout.log"] = "0" * 64
                V.verify_inventory(self.payload, changed, self.P.BINARY_SHA)

    def test_endpoint_rederives_raw_values(self):
        value = V.lines(self.payload / "results/positive-immediate/stdout.log")[0]
        for label in ("busy", "identity", "selected-pid", "raw-capture-failed"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "busy":
                    changed["sysfs"][0]["values"]["gpu_busy_percent"] = "1"
                elif label == "identity":
                    changed["sysfs"][0]["values"]["unique_id"] = "0000000000000000"
                elif label == "selected-pid":
                    changed["selected_pids"] = [12345]
                else:
                    changed["status"]["exit"] = 1
                self.P.endpoint(changed)

    def test_delayed_endpoint_cannot_move_to_a_later_window(self):
        value = V.lines(self.payload / "results/positive-delayed/stdout.log")[0]
        t0 = V.load(self.payload / "results/positive-test/record.json")["t0"][
            "monotonic_ns"
        ]
        with self.assertRaises(ValueError):
            self.P.endpoint(value, t0=t0 - 2_000_000_000, offset=20)

    def test_positive_requires_exact_marker_and_profiler(self):
        out = (self.payload / "results/positive-test/stdout.log").read_text()
        for changed in (
            out.replace("host_account_refund=complete", "host_account_refund=missing"),
            out.replace('"dropped_events":0', '"dropped_events":1'),
        ):
            with self.subTest(), self.assertRaises(ValueError):
                self.P.transcript("positive", changed, "")


if __name__ == "__main__":
    unittest.main()
