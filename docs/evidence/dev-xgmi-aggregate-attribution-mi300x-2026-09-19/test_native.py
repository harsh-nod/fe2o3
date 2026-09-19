#!/usr/bin/env python3
"""CPU-only calibration of aggregate-attribution controls and admission."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

import copy
import importlib.util
import json
from pathlib import Path
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SPEC = importlib.util.spec_from_file_location(
    "xgmi_aggregate_attribution_native", HERE / "native.py"
)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("cannot load native.py")
N = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(N)
FIXTURE = (
    ROOT
    / "docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/remote/"
    "d1-ordinary-a1-before-gpu1/stdout"
)
FIXTURE_SHA = "625c9a4534da559156f9dbd12c23865804c9ecdda6098902a9b86a52dde3ff18"
N.need(N.sha(FIXTURE) == FIXTURE_SHA, "authenticated admitted endpoint fixture")


class EndpointCalibration(unittest.TestCase):
    def setUp(self):
        self.raw = FIXTURE.read_bytes()
        self.rows = [N.H.parse_json(line) for line in self.raw.splitlines()]

    @staticmethod
    def encoded(rows):
        return ("\n".join(json.dumps(row) for row in rows) + "\n").encode()

    def rejected(self, data):
        with self.assertRaises((RuntimeError, ValueError, KeyError, TypeError)):
            N.H.parse_endpoint(data, *N.DEVICES[0])

    def test_authenticated_admitted_fixture_replays(self):
        self.assertEqual(N.H.parse_endpoint(self.raw, *N.DEVICES[0]), self.rows[0])

    def test_raw_sysfs_activity_identity_and_errors_override_admitted_claim(self):
        for position in range(3):
            for field, value in (
                ("mem_busy_percent", "1"),
                ("gpu_busy_percent", "1"),
                ("mem_info_vram_used", str(512 * 1024 * 1024)),
                ("unique_id", "0000000000000001"),
                ("gpu_busy_percent", "00"),
            ):
                with self.subTest(position=position, field=field, value=value):
                    rows = copy.deepcopy(self.rows)
                    rows[0]["sysfs"][position]["values"][field] = value
                    self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["sysfs"][0]["errors"] = {"unique_id": "unreadable"}
        self.rejected(self.encoded(rows))

    def test_raw_smi_pid_claim_and_framing_fail_closed(self):
        for field, value in (
            ("GPU use (%)", "1"),
            ("VRAM Total Used Memory (B)", str(512 * 1024 * 1024)),
            ("Unique ID", N.DEVICES[1][2]),
            ("PCI Bus", N.DEVICES[1][1]),
        ):
            with self.subTest(field=field):
                rows = copy.deepcopy(self.rows)
                status = N.H.parse_json(rows[0]["status"]["stdout"])
                status["card1"][field] = value
                rows[0]["status"]["stdout"] = json.dumps(status)
                self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["pids"]["stdout"] = (
            "===== ROCm System Management Interface =====\n"
            "===== GPUs Indexed by PID =====\n"
            "PID 999999 is using 1 DRM device(s):\n1\n=====\n"
            "===== End of ROCm SMI Log =====\n"
        )
        self.rejected(self.encoded(rows))
        for data in (
            self.raw[:-1],
            self.raw + b"{}\n",
            self.raw.replace(b'"gpu_index": 1', b'"gpu_index": 1, "gpu_index": 1', 1),
        ):
            self.rejected(data)

    def test_false_claims_capture_failures_and_chronology_are_rejected(self):
        for key, value in (
            ("gpu_index", True),
            ("endpoint_admitted", False),
            ("selected_pids", [999999]),
        ):
            rows = copy.deepcopy(self.rows)
            rows[0][key] = value
            self.rejected(self.encoded(rows))
        for key, value in (
            ("exit", False),
            ("error", "failed"),
            ("stderr", "warning"),
            ("command", ["true"]),
        ):
            rows = copy.deepcopy(self.rows)
            rows[0]["pids"][key] = value
            self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["sysfs"][0]["started"]["monotonic_ns"] = (
            rows[0]["finished"]["monotonic_ns"] + 1
        )
        self.rejected(self.encoded(rows))


class CommandCalibration(unittest.TestCase):
    def test_trials_are_exact_off_on_on_off_and_share_one_elf(self):
        owned = Path(N.PREFIX + "0" * 16)
        trials = N.trial_specs(owned)
        self.assertEqual(
            [(name, mode) for name, mode, _, _ in trials],
            [("1-off", "off"), ("2-on", "on"), ("3-on", "on"), ("4-off", "off")],
        )
        uids = [device[2] for device in N.DEVICES]
        arguments = ["1048576", "1", "10", "30"]
        expected_env = N.environment(owned)
        for _, mode, command, env in trials:
            self.assertEqual(command[:7], [str(owned / N.BINARY), *uids, *arguments])
            self.assertEqual(
                command[7],
                "--aggregate-peer-batch-hot-diagnose"
                if mode == "on"
                else "--aggregate-peer-batch-hot-only",
            )
            self.assertEqual(env, expected_env)
        self.assertFalse(
            {"HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES", "HSA_XNACK"}
            & set(expected_env)
        )

    def test_plan_build_and_identity_controls_are_bounded(self):
        owned = Path(N.PREFIX + "0" * 16)
        self.assertEqual(
            N.PLAN,
            {
                "order": ["off", "on", "on", "off"],
                "bytes": 1_048_576,
                "depth": 1,
                "warmups": 10,
                "samples": 30,
                "devices": [1, 2],
                "settled_seconds": 2,
                "delayed_seconds": 20,
                "performance_acceptance": False,
            },
        )
        builds = N.build_specs(owned)
        self.assertEqual([row[0] for row in builds], ["rustc", "cargo", "rocm", "build-kfd"])
        self.assertEqual(
            N.final_specs(owned),
            [("after-" + name, command, seconds) for name, command, seconds in builds[:3]],
        )
        command = builds[-1][1]
        self.assertEqual(command.count("--features"), 1)
        self.assertEqual(command[command.index("--features") + 1], "hardware-diagnostic")
        self.assertEqual(command.count("--example"), 1)
        self.assertEqual(
            command[command.index("--example") + 1],
            "gfx942-runtime-xgmi-peer-benchmark",
        )
        for _, _, seconds in builds + N.final_specs(owned):
            self.assertIs(type(seconds), int)
            self.assertTrue(0 < seconds <= 1200)

    def test_payload_source_and_observer_controls_are_exact(self):
        self.assertEqual(
            N.PAYLOAD,
            ("native.py", "results.py", "hot.py", "base.py", "source.tar.gz"),
        )
        self.assertIn(N.OBSERVER, N.REQUIRED_SOURCE)
        self.assertIn(
            "crates/fe2o3-runtime/src/kfd_backend/xgmi_batch_diagnostic.rs",
            N.REQUIRED_SOURCE,
        )
        for index, bdf, uid in N.DEVICES:
            self.assertEqual(
                N.observe_spec("trial-before", index, bdf, uid),
                [
                    "/usr/bin/python3",
                    "-I",
                    N.OBSERVER,
                    "--gpu-index",
                    str(index),
                    "--pci-bdf",
                    bdf,
                    "--unique-id",
                    uid,
                ],
            )


if __name__ == "__main__":
    unittest.main()
