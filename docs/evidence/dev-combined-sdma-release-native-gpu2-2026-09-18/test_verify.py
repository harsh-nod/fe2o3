#!/usr/bin/env python3
"""CPU-only adversarial calibration of the complete GPU2 evidence audit."""

import copy
import unittest
from unittest.mock import patch

import verify as A


class ArchiveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.payload = A.ROOT / A.COLLECTED
        cls.P = A.V.module(cls.payload / "protocol.py")

    def audit_mutation(self, path, mutate):
        original = A.V.load

        def changed(candidate):
            value = original(candidate)
            if candidate == path:
                mutate(value)
            return value

        with patch.object(A.V, "load", side_effect=changed), self.assertRaises(ValueError):
            A.audit(A.ROOT, allow_unsealed=True)

    def test_complete_historical_audit(self):
        result = A.audit(A.ROOT, allow_unsealed=True)
        self.assertEqual(result["native_campaign"], "passed")
        self.assertEqual(result["profiles"], A.CASES)
        self.assertEqual(result["native_commands_passed"], 7)
        self.assertEqual(result["strict_endpoints"], 21)
        self.assertEqual(result["refused_endpoints"], 0)
        self.assertEqual(result["recorded_native_pids_groups_absent"], 31)
        self.assertEqual(result["remote_files_collected"], 120)
        self.assertEqual(result["remote_files_retained"], 119)
        self.assertEqual(result["prior_campaign"], "rejected-delayed-shared-host-observation")
        self.assertFalse(result["formal_refinement"])
        self.assertFalse(result["performance_claim"])

    def test_controller_rejects_incomplete_execution_and_cleanup(self):
        value = A.V.load(A.ROOT / "raw/native/controller.json")
        for key, replacement in (("native_outer_passed", False), ("native_attempted", False), ("collected_verified", False), ("cleanup_closed", False), ("independent_absence_closed", False), ("failure", "rejected"), ("local_only", True), ("owned", "/tmp/unrelated"), ("payload_sha256", "0" * 64)):
            changed = copy.deepcopy(value)
            changed[key] = replacement
            with self.subTest(key=key), self.assertRaises(ValueError):
                A.controller(changed)

    def test_all_seven_transcripts_require_exact_combined_release(self):
        for case in A.CASES:
            text = (self.payload / f"results/{case}-test/stdout.log").read_text()
            self.P.transcript(case, text, "")
            for wrong in (text + "extra\n", text.splitlines()[0] + "\n", text.replace("public_root_drop=completed", "public_root_drop=pending"), text.replace("retry=rejected", "retry=accepted"), text.replace("maximum_striped_queue_count=14", "maximum_striped_queue_count=16"), text.replace("directional_engine_placement=1,0", "directional_engine_placement=0,1"), text.replace(self.P.UID[2:], "0" * 16)):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    self.P.transcript(case, wrong, "")

    def test_source_binding_rejects_export_identity_drift(self):
        path = self.payload / "binding.json"
        for key, value in (("commit", "0" * 40), ("source_files_matched", 5556), ("binary_sha256", "0" * 64), ("cpu_manifest_sha256", "0" * 64), ("cohort_sha256", "0" * 64), ("signature_exit", 1), ("cohort_exit", 1), ("native_authorized", True), ("source_base_field_ignored_only", False)):
            with self.subTest(key=key):
                self.audit_mutation(path, lambda row: row.update({key: value}))

    def test_native_command_identity_and_bounds_are_exact(self):
        path = self.payload / "results/combined-14-test/record.json"
        for change in (
            lambda row: row["command"].__setitem__(-3, "--retained-release-striped-sdma"),
            lambda row: row["command"].__setitem__(-2, "12"),
            lambda row: row["command"].__setitem__(-1, "0xab83d2ffef0d3cdf"),
            lambda row: row.update(outer_bound_seconds=1000),
            lambda row: row["environment"].update(HSA_XNACK="1"),
            lambda row: row.update(group_absent=False),
        ):
            self.audit_mutation(path, change)

    def test_seven_case_roster_cannot_be_shortened_or_relabelled(self):
        path = self.payload / "results/campaign.json"
        for change in (
            lambda row: row["cases"].pop(),
            lambda row: row["cases"].reverse(),
            lambda row: row.update(failure="delayed refusal"),
            lambda row: row["cases"][-1]["post_observations"].update(delayed="attempted"),
            lambda row: row.update(performance_claim=True),
        ):
            self.audit_mutation(path, change)

    def test_raw_endpoint_and_fixed_delayed_window_are_required(self):
        value = A.V.lines(self.payload / "results/combined-14-delayed/stdout.log")[0]
        t0 = A.V.load(self.payload / "results/combined-14-test/record.json")["t0"]["monotonic_ns"]
        self.P.endpoint(value, t0=t0, offset=20)
        for mutate in (
            lambda row: row["sysfs"][0]["values"].update(gpu_busy_percent="1"),
            lambda row: row["sysfs"][0]["values"].update(mem_busy_percent="1"),
            lambda row: row.update(selected_pids=[123]),
            lambda row: row["status"].update(exit=1),
            lambda row: row.update(endpoint_admitted=False),
        ):
            changed = copy.deepcopy(value)
            mutate(changed)
            with self.assertRaises(ValueError):
                self.P.endpoint(changed, t0=t0, offset=20)
        with self.assertRaises(ValueError):
            self.P.endpoint(value, t0=t0 - 2_000_000_000, offset=20)

    def test_collected_inventory_and_cleanup_must_agree(self):
        path = A.ROOT / "raw/native/collection-verified.json"
        for mutate in (
            lambda row: row["files"].pop("results/combined-14-test/stderr.log"),
            lambda row: row["files"].update({"queue-example": "0" * 64}),
            lambda row: row["recorded_pids"].pop(),
            lambda row: row.update(owned="/tmp/foreign"),
        ):
            self.audit_mutation(path, mutate)

    def test_protocol_delta_cannot_change_parsing_semantics(self):
        prior = A.PRIOR / A.V.COLLECTED
        protocol = (self.payload / "protocol.py").read_text()
        old_protocol = (prior / "protocol.py").read_text()
        fixture = (self.payload / "test_protocol.py").read_text()
        old_fixture = (prior / "test_protocol.py").read_text()
        A.protocol_delta(protocol, old_protocol, fixture, old_fixture)
        for text in (
            protocol.replace("def transcript(case, stdout, stderr):", "def transcript(case, stdout, stderr):\n    return {}"),
            protocol.replace("def endpoint(value, *, t0=None, offset=None):", "def endpoint(value, *, t0=None, offset=None):\n    return 1"),
            protocol.replace("GPU, UID, BDF = 2,", "GPU, UID, BDF = 3,"),
            protocol + "\ndef unreviewed_function():\n    return True\n",
        ):
            self.assertNotEqual(text, protocol)
            with self.assertRaises(ValueError):
                A.protocol_delta(text, old_protocol, fixture, old_fixture)
        changed = fixture.replace("def fixture(case):", "def fixture(case):\n    return ''")
        self.assertNotEqual(changed, fixture)
        with self.assertRaises(ValueError):
            A.protocol_delta(protocol, old_protocol, changed, old_fixture)

    def test_every_controller_command_binds_bounds_and_stdin(self):
        for name in A.LOCAL_NAMES:
            path = A.ROOT / "raw/native" / name / "record.json"
            for change in (
                lambda row: row.update(bound_seconds=row["bound_seconds"] + 1),
                lambda row: row.update(stdin_sha256="0" * 64),
                lambda row: row["command"].__setitem__(0, "unreviewed-program"),
            ):
                with self.subTest(command=name):
                    self.audit_mutation(path, change)


if __name__ == "__main__":
    unittest.main()
