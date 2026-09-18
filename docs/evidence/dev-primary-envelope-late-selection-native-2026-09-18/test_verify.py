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
        self.assertEqual(value["error"], "passed")
        self.assertEqual(
            value["panic"], "transcript-passed-immediate-observation-refused"
        )
        self.assertEqual(value["harness_passes"], [1, 2, 2])
        self.assertFalse(value["current_binary_revalidated"])

    def test_missing_seal_requires_explicit_review_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(ValueError):
                V.manifest(Path(folder), False)

    def test_cpu_binary_map_bridge(self):
        before = V.load(self.payload / "cpu/binaries-before-final.log")
        for field, replacement in (
            ("sha256", "0" * 64),
            ("path", "unqualified-runtime"),
        ):
            with self.subTest(field=field), self.assertRaises(ValueError):
                changed = copy.deepcopy(before)
                changed["musl-runtime"][field] = replacement
                V.binary_maps(changed, changed, self.P.BINARY_SHA)
        with self.assertRaises(ValueError):
            V.binary_maps(before, {}, self.P.BINARY_SHA)

    def test_observer_command_contract(self):
        value = V.load(self.payload / "results/positive-preflight/record.json")
        for label in ("argv", "environment", "bound"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "argv":
                    changed["command"][-1] = "2"
                elif label == "environment":
                    changed["environment"]["HSA_XNACK"] = "1"
                else:
                    changed["outer_bound_seconds"] = 1000
                V.observer_command(changed, self.P)

    def test_endpoint_capture_clock_bracket(self):
        row = V.load(self.payload / "results/positive-preflight/record.json")
        value = V.lines(self.payload / "results/positive-preflight/stdout.log")[0]
        for label in ("start", "finish"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "start":
                    changed["started"]["monotonic_ns"] = (
                        row["started"]["monotonic_ns"] - 1
                    )
                else:
                    changed["finished"]["monotonic_ns"] = (
                        row["finished"]["monotonic_ns"] + 1
                    )
                V.endpoint_bracket(row, changed)

    def test_outer_timeout_contract(self):
        row = V.load(self.payload / "results/controller-launch.json")
        changed = copy.deepcopy(row)
        changed["command"][3] = "3600s"
        with self.assertRaises(ValueError):
            V.outer_command(changed)

    def test_controller_cannot_upgrade_or_erase_failure(self):
        value = V.load(self.native / "controller.json")
        for field, replacement in (
            ("native_outer_passed", True),
            ("failure", None),
            ("cleanup_closed", False),
            ("owned", "/tmp/unrelated"),
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

    def test_error_and_panic_require_their_exact_markers(self):
        out = (self.payload / "results/error-test/stdout.log").read_text()
        panic = (self.payload / "results/panic-test/stdout.log").read_text()
        with self.assertRaises(ValueError):
            self.P.transcript(
                "error",
                out.replace(
                    "retained_original_root=true", "retained_original_root=false"
                ),
                "",
            )
        with self.assertRaises(ValueError):
            self.P.transcript(
                "panic",
                panic.replace("original_payload=true", "original_payload=false"),
                "",
            )

    def test_refusal_replay_preserves_original_capture(self):
        value = V.lines(self.payload / "results/panic-immediate/stdout.log")[0]
        original = copy.deepcopy(value)
        t0 = V.load(self.payload / "results/panic-test/record.json")["t0"][
            "monotonic_ns"
        ]
        V.busy_refusal(self.P, value, t0=t0)
        self.assertEqual(value, original)
        with self.assertRaises(ValueError):
            self.P.endpoint(value, t0=t0, offset=0)

    def test_refusal_cannot_be_normalized_or_change_cause(self):
        value = V.lines(self.payload / "results/panic-immediate/stdout.log")[0]
        t0 = V.load(self.payload / "results/panic-test/record.json")["t0"][
            "monotonic_ns"
        ]
        for label in (
            "accept",
            "no-reasons",
            "clear-busy",
            "different-busy",
            "later-busy",
            "extra-snapshot",
            "bad-command",
            "vram",
        ):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "accept":
                    changed["endpoint_admitted"] = True
                elif label == "no-reasons":
                    changed["reasons"] = []
                elif label == "clear-busy":
                    changed["sysfs"][0]["values"]["gpu_busy_percent"] = "0"
                elif label == "different-busy":
                    changed["sysfs"][0]["values"]["gpu_busy_percent"] = "2"
                elif label == "later-busy":
                    changed["sysfs"][2]["values"]["gpu_busy_percent"] = "1"
                elif label == "extra-snapshot":
                    changed["sysfs"].append(copy.deepcopy(changed["sysfs"][-1]))
                elif label == "bad-command":
                    changed["pids"]["command"][-1] = "--different-query"
                else:
                    changed["sysfs"][0]["values"]["mem_info_vram_used"] = "536870912"
                V.busy_refusal(self.P, changed, t0=t0)

    def test_refused_endpoint_window_remains_binding(self):
        value = V.lines(self.payload / "results/panic-immediate/stdout.log")[0]
        t0 = V.load(self.payload / "results/panic-test/record.json")["t0"][
            "monotonic_ns"
        ]
        with self.assertRaises(ValueError):
            V.busy_refusal(self.P, value, t0=t0 - 2_000_000_000)

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
                    changed["files"]["results/unplanned-test/stdout.log"] = "0" * 64
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
