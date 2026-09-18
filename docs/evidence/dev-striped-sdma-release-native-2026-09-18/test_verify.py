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
        self.assertEqual(value["native_campaign"], "passed")
        self.assertEqual(value["profiles"], list(self.P.TESTS))
        self.assertEqual(value["native_commands_passed"], 8)
        self.assertEqual(value["strict_endpoints"], 24)
        self.assertEqual(value["prior_attempt"]["disposition"], "rejected-before-runtime-binary-launch")
        self.assertEqual(value["prior_attempt"]["native_commands"], 0)
        self.assertEqual(value["setup_rejection"]["exact_cleanup"], "closed")
        self.assertFalse(value["current_binary_revalidated"])

    def test_missing_seal_requires_explicit_review_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(ValueError):
                V.manifest(Path(folder), False)

    def test_whole_native_transcript_rejects_unconsumed_text(self):
        for case in self.P.TESTS:
            text = (self.payload / f"results/{case}-test/stdout.log").read_text()
            V.native_transcript(self.P, case, text, "")
            for changed in (
                text + "garbage\n",
                text.splitlines()[0] + "\n",
                text + "running 0 tests\n",
                text.replace("public_root_drop=completed", "public_root_drop=pending"),
            ):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    V.native_transcript(self.P, case, changed, "")

    def test_serial_command_chains_reject_overlap_and_bad_intervals(self):
        for native in (False, True):
            rows = {
                "one": {"started_ns": 1, "finished_ns": 4},
                "two": {"started_ns": 5, "finished_ns": 8},
            }
            if native:
                rows = {
                    name: {
                        "started": {"monotonic_ns": row["started_ns"]},
                        "finished": {"monotonic_ns": row["finished_ns"]},
                    }
                    for name, row in rows.items()
                }
            V.serial_chain(rows, ["one", "two"], native=native)
            with self.assertRaises(ValueError):
                V.serial_chain(rows, ["two", "one"], native=native)
            changed = copy.deepcopy(rows)
            if native:
                changed["two"]["finished"]["monotonic_ns"] = 2
            else:
                changed["two"]["finished_ns"] = 2
            with self.assertRaises(ValueError):
                V.serial_chain(changed, ["one", "two"], native=native)

    def test_cleanup_commands_bind_mode_roster_and_archived_helper(self):
        inventory = V.lines(self.native / "remote-inventory/stdout.log")[0]
        pids, digest = inventory["recorded_pids"], V.digest_map(inventory["files"])
        helper = V.sha(V.ROOT / "control/remote_control.py")
        for name, mode in (
            ("remote-cleanup", "cleanup"),
            ("remote-independent-absence", "absence"),
        ):
            row = V.load(self.native / name / "record.json")
            V.cleanup_command(row, mode, pids, digest, helper)
            for field in ("command", "stdin_sha256", "bound_seconds"):
                changed = copy.deepcopy(row)
                if field == "command":
                    changed[field][-1] = changed[field][-1].replace(
                        V.NEW, "/tmp/foreign"
                    )
                elif field == "stdin_sha256":
                    changed[field] = "0" * 64
                else:
                    changed[field] = 121
                with (
                    self.subTest(mode=mode, field=field),
                    self.assertRaises(ValueError),
                ):
                    V.cleanup_command(changed, mode, pids, digest, helper)
            with self.assertRaises(ValueError):
                V.cleanup_command(row, mode, pids[:-1], digest, helper)

    def test_cpu_binary_receipt_bridge(self):
        text = (self.payload / "cpu/binary.log").read_text()
        V.binary_receipt(text, self.P.BINARY_SHA)
        for changed in (
            text.replace(self.P.BINARY_SHA, "0" * 64),
            text.replace("kfd-compute-aql-queue", "unqualified-example"),
            text + "extra",
        ):
            with self.assertRaises(ValueError):
                V.binary_receipt(changed, self.P.BINARY_SHA)

    def test_observer_command_contract(self):
        value = V.load(self.payload / "results/striped-2-preflight/record.json")
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
        row = V.load(self.payload / "results/striped-2-preflight/record.json")
        value = V.lines(self.payload / "results/striped-2-preflight/stdout.log")[0]
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
        changed["command"][3] = "1200s"
        with self.assertRaises(ValueError):
            V.outer_command(changed)

    def test_controller_requires_success_and_cleanup(self):
        value = V.load(self.native / "controller.json")
        for field, replacement in (
            ("native_outer_passed", False),
            ("failure", "injected failure"),
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
            ("unreadable-owned-pid", lambda row: row["unreadable_same_uid_entries"].append({"pid": pids[0], "field": "maps", "process_group": pids[0]})),
            ("unreadable-owned-group", lambda row: row["unreadable_same_uid_entries"].append({"pid": 2147483647, "field": "maps", "process_group": pids[0]})),
        ]
        for label, mutate in mutations:
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                mutate(changed)
                V.absence(changed, V.NEW, pids)

    def test_inventory_cannot_omit_or_replace_retained_files(self):
        value = V.lines(self.native / "remote-inventory/stdout.log")[0]
        for label in ("missing", "wrong-elf", "extra"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "missing":
                    changed["files"].pop("results/striped-16-test/stderr.log")
                elif label == "wrong-elf":
                    changed["files"]["queue-example"] = "0" * 64
                else:
                    changed["files"]["results/unplanned-test/stdout.log"] = "0" * 64
                V.verify_inventory(self.payload, changed, self.P.BINARY_SHA)

    def test_endpoint_rederives_raw_values(self):
        value = V.lines(self.payload / "results/striped-2-immediate/stdout.log")[0]
        for label in ("busy", "memory-busy", "identity", "selected-pid", "raw-capture-failed"):
            with self.subTest(label=label), self.assertRaises(ValueError):
                changed = copy.deepcopy(value)
                if label == "busy":
                    changed["sysfs"][0]["values"]["gpu_busy_percent"] = "1"
                elif label == "memory-busy":
                    changed["sysfs"][0]["values"]["mem_busy_percent"] = "1"
                elif label == "identity":
                    changed["sysfs"][0]["values"]["unique_id"] = "0000000000000000"
                elif label == "selected-pid":
                    changed["selected_pids"] = [12345]
                else:
                    changed["status"]["exit"] = 1
                self.P.endpoint(changed)

    def test_delayed_endpoint_cannot_move_to_a_later_window(self):
        value = V.lines(self.payload / "results/striped-2-delayed/stdout.log")[0]
        t0 = V.load(self.payload / "results/striped-2-test/record.json")["t0"][
            "monotonic_ns"
        ]
        with self.assertRaises(ValueError):
            self.P.endpoint(value, t0=t0 - 2_000_000_000, offset=20)

    def test_native_cases_require_exact_profile_and_complete_release(self):
        for case in self.P.TESTS:
            out = (self.payload / f"results/{case}-test/stdout.log").read_text()
            for changed in (
                out.replace(f"resources_returned={5 + 3 * self.P.TESTS[case]}", "resources_returned=5"),
                out.replace(self.P.PROFILE_SHA, "0" * 64),
                out.replace("retry=rejected", "retry=accepted"),
                out.replace("engine_placement=0,1", "engine_placement=1,0"),
                out.replace(f"host_delta_bytes={4096 * self.P.TESTS[case]}", "host_delta_bytes=0"),
                out.replace(f"host_delta_records={self.P.TESTS[case]}", "host_delta_records=0"),
                out.replace("cursor=initial-0-no-advance", "cursor=native-observed"),
                out.replace("packets=0", "packets=1"),
            ):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    self.assertNotEqual(changed, out)
                    self.P.transcript(case, changed, "")

    def test_rejected_preflight_rederives_original_raw_refusal(self):
        rejected = V.module(V.ROOT / "rejected_verify.py")
        payload = V.ROOT / "rejected/raw/native/collected"
        protocol = V.module(payload / "protocol.py")
        value = V.lines(payload / "results/striped-2-preflight/stdout.log")[0]
        rejected.endpoint(protocol, value)
        for fault in ("admitted", "busy", "pid", "reasons", "identity", "capture", "clock"):
            changed = copy.deepcopy(value)
            if fault == "admitted":
                changed["endpoint_admitted"] = True
            elif fault == "busy":
                changed["sysfs"][0]["values"]["gpu_busy_percent"] = "0"
            elif fault == "pid":
                changed["selected_pids"] = []
            elif fault == "reasons":
                changed["reasons"].pop()
            elif fault == "identity":
                changed["pci_bdf"] = "0000:26:00.0"
            elif fault == "capture":
                changed["pids"]["exit"] = 1
            else:
                changed["finished"]["monotonic_ns"] = 1
            with self.subTest(fault=fault), self.assertRaises(ValueError):
                rejected.endpoint(protocol, changed)

    def test_marker_only_rejection_cleanup_is_retained(self):
        self.assertEqual(V.setup_history(V.ROOT)["native_commands"], 0)

    def test_outer_json_stream_consumes_all_objects(self):
        text = (self.native / "native-outer/stdout.log").read_text()
        self.assertEqual(len(V.json_stream(text)), 3)
        for changed in (text + "trailing garbage", text + "{", '{"a":1,"a":2}', '{"a":NaN}'):
            with self.assertRaises(ValueError):
                V.json_stream(changed)


if __name__ == "__main__":
    unittest.main()
