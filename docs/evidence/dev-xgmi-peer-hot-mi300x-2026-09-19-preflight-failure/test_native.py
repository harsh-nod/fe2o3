#!/usr/bin/env python3
"""CPU-only calibration of hot-trial controls and raw endpoint admission."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import copy
import importlib.util
import json
from pathlib import Path
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
spec = importlib.util.spec_from_file_location("peer_hot_native", HERE / "native.py")
N = importlib.util.module_from_spec(spec)
spec.loader.exec_module(N)
FIXTURE = ROOT / "docs/evidence/dev-xgmi-peer-batch-mi300x-2026-09-18/remote/d1-ordinary-a1-before-gpu1/stdout"
FIXTURE_SHA = "625c9a4534da559156f9dbd12c23865804c9ecdda6098902a9b86a52dde3ff18"
N.need(N.sha(FIXTURE) == FIXTURE_SHA, "authenticated admitted endpoint fixture")


class EndpointCalibration(unittest.TestCase):
    def setUp(self):
        self.raw = FIXTURE.read_bytes()
        self.rows = [N.parse_json(line) for line in self.raw.splitlines()]

    def encoded(self, rows):
        return ("\n".join(json.dumps(row) for row in rows) + "\n").encode()

    def rejected(self, data):
        with self.assertRaises((RuntimeError, ValueError, KeyError, TypeError)):
            N.parse_endpoint(data, *N.DEVICES[0])

    def test_authenticated_admitted_fixture_replays(self):
        self.assertEqual(N.parse_endpoint(self.raw, *N.DEVICES[0]), self.rows[0])

    def test_raw_sysfs_activity_identity_and_errors_override_admitted_claim(self):
        for position in range(3):
            for field, value in (
                ("mem_busy_percent", "1"), ("gpu_busy_percent", "1"),
                ("mem_info_vram_used", str(512 * 1024 * 1024)),
                ("unique_id", "0000000000000001"), ("gpu_busy_percent", "00"),
            ):
                with self.subTest(position=position, field=field, value=value):
                    rows = copy.deepcopy(self.rows)
                    rows[0]["sysfs"][position]["values"][field] = value
                    self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["sysfs"][0]["errors"] = {"unique_id": "unreadable"}
        self.rejected(self.encoded(rows))

    def test_raw_smi_status_and_pid_attribution_override_admitted_claim(self):
        for field, value in (
            ("GPU use (%)", "1"), ("VRAM Total Used Memory (B)", str(512 * 1024 * 1024)),
            ("Unique ID", N.DEVICES[1][2]), ("PCI Bus", N.DEVICES[1][1]),
        ):
            with self.subTest(field=field):
                rows = copy.deepcopy(self.rows)
                status = N.parse_json(rows[0]["status"]["stdout"])
                status["card1"][field] = value
                rows[0]["status"]["stdout"] = json.dumps(status)
                self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["pids"]["stdout"] = (
            "===== ROCm System Management Interface =====\n===== GPUs Indexed by PID =====\n"
            "PID 999999 is using 1 DRM device(s):\n1\n=====\n===== End of ROCm SMI Log =====\n"
        )
        self.rejected(self.encoded(rows))
        for name in ("status", "pids"):
            rows = copy.deepcopy(self.rows)
            rows[0][name]["stdout"] = rows[0][name]["stdout"][:10]
            self.rejected(self.encoded(rows))

    def test_claim_types_capture_failures_and_chronology_are_rejected(self):
        for key, value in (("gpu_index", True), ("endpoint_admitted", False), ("selected_pids", [999999])):
            rows = copy.deepcopy(self.rows)
            rows[0][key] = value
            self.rejected(self.encoded(rows))
        for key, value in (("exit", False), ("error", "failed"), ("stderr", "warning"), ("command", ["true"])):
            rows = copy.deepcopy(self.rows)
            rows[0]["pids"][key] = value
            self.rejected(self.encoded(rows))
        rows = copy.deepcopy(self.rows)
        rows[0]["sysfs"][0]["started"]["monotonic_ns"] = rows[0]["finished"]["monotonic_ns"] + 1
        self.rejected(self.encoded(rows))

    def test_truncated_extra_and_duplicate_json_transcripts_are_rejected(self):
        normal = self.encoded(self.rows)
        for data in (
            normal[:-1], normal + b"{}\n", normal[:20],
            normal.replace(b'"gpu_index": 1', b'"gpu_index": 1, "gpu_index": 1', 1),
            normal.replace(b'"observations": 1', b'"observations": 1, "observations": 1', 1),
        ):
            self.rejected(data)


class CommandCalibration(unittest.TestCase):
    def test_balanced_trials_bind_physical_identities_and_logical_masks(self):
        owned = Path(N.PREFIX + "0" * 16)
        trials = N.trial_specs(owned)
        self.assertEqual([row[0] for row in trials], ["1-kfd", "2-hsa", "3-hip", "4-hip", "5-hsa", "6-kfd"])
        self.assertEqual(N.PLAN, {"order": ["kfd", "hsa", "hip", "hip", "hsa", "kfd"], "bytes": 1048576, "depth": 1, "warmups": 10, "samples": 30, "devices": [1, 2], "settled_seconds": 2, "delayed_seconds": 20, "performance_acceptance": False})
        uids, arguments = [row[2] for row in N.DEVICES], ["1048576", "1", "10", "30"]
        base = N.environment(owned)
        self.assertEqual(base["CARGO_BUILD_JOBS"], "2")
        self.assertEqual(base["CARGO_TARGET_DIR"], str(owned / "target"))
        self.assertEqual(base["TMPDIR"], str(owned / "tmp"))
        self.assertFalse({"HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES", "HSA_XNACK"} & set(base))
        for _, backend, command, env in trials:
            expected = [str(owned / N.BINARIES[backend])]
            wanted_env = dict(base)
            if backend == "kfd":
                expected += [*uids, *arguments, "--aggregate-peer-batch-hot-only"]
            else:
                expected += ["0", "1", *arguments, *uids, "--persistent-hot"]
                wanted_env.update({"HSA_XNACK": "0", "ROCR_VISIBLE_DEVICES" if backend == "hsa" else "HIP_VISIBLE_DEVICES": "1,2"})
            self.assertEqual(command, expected)
            self.assertEqual(env, wanted_env)
        for index, bdf, uid in N.DEVICES:
            self.assertEqual(N.observe_spec("trial-before", index, bdf, uid), ["/usr/bin/python3", "-I", N.OBSERVER, "--gpu-index", str(index), "--pci-bdf", bdf, "--unique-id", uid])

    def test_builds_are_frozen_bounded_and_warning_clean(self):
        owned = Path(N.PREFIX + "0" * 16)
        builds = N.build_specs(owned)
        self.assertEqual([row[0] for row in builds], ["rustc", "cargo", "hipcc", "g++", "rocm", "build-kfd", "build-hip", "build-hsa"])
        for _, _, seconds in builds + N.final_specs(owned):
            self.assertIs(type(seconds), int)
            self.assertTrue(0 < seconds <= 1200)
        self.assertEqual(N.final_specs(owned), [("after-" + name, command, seconds) for name, command, seconds in builds[:5]])
        self.assertEqual(builds[5][1], ["cargo", "build", "--frozen", "--release", "-p", "fe2o3-runtime", "--example", "gfx942-runtime-xgmi-peer-benchmark"])
        for _, command, seconds in builds[6:]:
            self.assertTrue({"-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror"} <= set(command))
            self.assertEqual(seconds, 180)
        self.assertIn("--offload-arch=gfx942", builds[6][1])
        self.assertIn("-lhsa-runtime64", builds[7][1])


if __name__ == "__main__":
    unittest.main()
