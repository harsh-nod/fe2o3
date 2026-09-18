#!/usr/bin/env python3
"""CPU-only adversarial calibration of the complete LogicalMux evidence audit."""

import copy
from pathlib import Path
import tempfile
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
        self.assertEqual(result["native_commands_passed"], 5)
        self.assertEqual(result["strict_endpoints"], 15)
        self.assertEqual(result["refused_endpoints"], 0)
        self.assertEqual(result["recorded_native_pids_groups_absent"], 23)
        self.assertEqual(result["remote_files_collected"], 97)
        self.assertEqual(result["remote_files_retained"], 96)
        self.assertFalse(result["formal_refinement"])
        self.assertFalse(result["performance_claim"])

    def test_controller_rejects_incomplete_execution_and_cleanup(self):
        value = A.V.load(A.ROOT / "raw/native/controller.json")
        for key, replacement in (("native_outer_passed", False), ("native_attempted", False), ("collected_verified", False), ("cleanup_closed", False), ("independent_absence_closed", False), ("failure", "rejected"), ("local_only", True), ("owned", "/tmp/unrelated"), ("payload_sha256", "0" * 64)):
            changed = copy.deepcopy(value)
            changed[key] = replacement
            with self.subTest(key=key), self.assertRaises(ValueError):
                A.controller(changed)

    def test_all_five_transcripts_require_exact_logical_mux_release(self):
        for case in A.CASES:
            text = (self.payload / f"results/{case}-test/stdout.log").read_text()
            self.P.transcript(case, text, "")
            for wrong in (text + "extra\n", text.splitlines()[0] + "\n", text.replace("public_root_drop=completed", "public_root_drop=pending"), text.replace("retry=rejected", "retry=accepted"), text.replace("native_queue_count=2", "native_queue_count=16"), text.replace("engine_placement=0,1", "engine_placement=1,0"), text.replace(self.P.UID[2:], "0" * 16)):
                with self.subTest(case=case), self.assertRaises(ValueError):
                    self.P.transcript(case, wrong, "")

    def test_source_binding_rejects_export_identity_drift(self):
        path = self.payload / "binding.json"
        for key, value in (("commit", "0" * 40), ("source_files_matched", 5556), ("binary_sha256", "0" * 64), ("cpu_manifest_sha256", "0" * 64), ("cohort_sha256", "0" * 64), ("signature_exit", 1), ("cohort_exit", 1), ("native_authorized", True), ("source_base_field_ignored_only", False)):
            with self.subTest(key=key):
                self.audit_mutation(path, lambda row: row.update({key: value}))

    def test_native_command_identity_and_bounds_are_exact(self):
        path = self.payload / "results/logical-mux-16-test/record.json"
        for change in (
            lambda row: row["command"].__setitem__(-3, "--retained-release-striped-sdma"),
            lambda row: row["command"].__setitem__(-2, "12"),
            lambda row: row["command"].__setitem__(-1, "0xd2e26fef80cf5c33"),
            lambda row: row.update(outer_bound_seconds=1000),
            lambda row: row["environment"].update(HSA_XNACK="1"),
            lambda row: row.update(group_absent=False),
        ):
            self.audit_mutation(path, change)

    def test_five_case_roster_cannot_be_shortened_or_relabelled(self):
        path = self.payload / "results/campaign.json"
        for change in (
            lambda row: row["cases"].pop(),
            lambda row: row["cases"].reverse(),
            lambda row: row.update(failure="delayed refusal"),
            lambda row: row["cases"][-1]["post_observations"].update(delayed="attempted"),
            lambda row: row.update(performance_claim=True),
            lambda row: row.update(logical_lane_execution_claim=True),
            lambda row: row.update(cursor_observation_claim=True),
        ):
            self.audit_mutation(path, change)

    def test_raw_endpoint_and_fixed_delayed_window_are_required(self):
        value = A.V.lines(self.payload / "results/logical-mux-16-delayed/stdout.log")[0]
        t0 = A.V.load(self.payload / "results/logical-mux-16-test/record.json")["t0"]["monotonic_ns"]
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
            lambda row: row["files"].pop("results/logical-mux-16-test/stderr.log"),
            lambda row: row["files"].update({"queue-example": "0" * 64}),
            lambda row: row["recorded_pids"].pop(),
            lambda row: row.update(owned="/tmp/foreign"),
        ):
            self.audit_mutation(path, mutate)

    def test_nested_cpu_seal_is_manifested_and_cannot_be_omitted(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-logical-mux-native-manifest-") as temporary:
            root = Path(temporary)
            (root / "cpu").mkdir()
            nested = root / "cpu/SHA256SUMS"
            nested.write_text("nested CPU seal fixture\n")
            (root / "README.md").write_text("fixture\n")
            contents = "".join(f"{A.sha(path)}  {path.relative_to(root).as_posix()}\n" for path in sorted(root.rglob("*")) if path.is_file() and path != root / "SHA256SUMS")
            self.assertIn("  cpu/SHA256SUMS\n", contents)
            outer = root / "SHA256SUMS"
            outer.write_text(contents)
            self.assertTrue(A.V.manifest(root, False))
            nested.write_text("changed CPU seal\n")
            with self.assertRaises(ValueError):
                A.V.manifest(root, True)
            nested.write_text("nested CPU seal fixture\n")
            outer.write_text("".join(line for line in contents.splitlines(keepends=True) if not line.endswith("  cpu/SHA256SUMS\n")))
            with self.assertRaises(ValueError):
                A.V.manifest(root, False)
            outer.unlink()
            with self.assertRaises(ValueError):
                A.V.manifest(root, False)
            self.assertFalse(A.V.manifest(root, True))

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
