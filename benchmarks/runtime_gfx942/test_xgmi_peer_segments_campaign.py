#!/usr/bin/env python3
"""CPU-only campaign command and pinned-helper checks; never touches GPUs."""

import importlib.util
from pathlib import Path
import shlex
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("segments_campaign", HERE / "xgmi_peer_segments_campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
N = C.N


class CampaignTests(unittest.TestCase):
    def test_collection_and_cleanup_failures_retain_custody(self):
        calls = []
        ok, failures = C.settle_remote(calls.append, collected=False, native_attempted=True)
        self.assertFalse(ok)
        self.assertEqual(calls, [])
        self.assertIn("retained", failures[0])
        for collected, attempted in ((True, True), (False, False)):
            calls.clear()
            ok, failures = C.settle_remote(calls.append, collected=collected, native_attempted=attempted)
            self.assertTrue(ok)
            self.assertEqual(failures, [])
            self.assertEqual(calls, ["cleanup", "absence"])
        for fault in ("cleanup", "absence"):
            calls.clear()
            def control(name):
                calls.append(name)
                if name == fault:
                    raise RuntimeError("injected")
            ok, failures = C.settle_remote(control, collected=True, native_attempted=True)
            self.assertFalse(ok)
            self.assertEqual(len(failures), 1)
            self.assertEqual(calls, ["cleanup", "absence"])

    def test_matched_order_controls_and_visibility(self):
        owned = Path(N.PREFIX + "0123456789abcdef")
        devices = [[1, "0000:26:00.0", "0xab83d2ffef0d3cdf"], [2, "0000:46:00.0", "0xd2e26fef80cf5c33"]]
        specs = N.trial_specs(owned, devices)
        self.assertEqual([s[1] for s in specs], ["kfd", "hsa", "hip", "hip", "hsa", "kfd"])
        for name, backend, command, env in specs:
            self.assertEqual(command[0], str(owned / N.BINARIES[backend]))
            self.assertEqual(env["TMPDIR"], str(owned / "tmp"))
            self.assertNotIn("LD_PRELOAD", env)
            if backend == "kfd":
                self.assertEqual(command[1:], [devices[0][2], devices[1][2], "65536", "65", "2", "10"])
                self.assertNotIn("HIP_VISIBLE_DEVICES", env)
                self.assertNotIn("ROCR_VISIBLE_DEVICES", env)
            else:
                self.assertEqual(command[1:], ["0", "1", "65536", "1", "2", "10", devices[0][2], devices[1][2], "--ordered-segments", "65"])
                self.assertEqual(env["HIP_VISIBLE_DEVICES" if backend == "hip" else "ROCR_VISIBLE_DEVICES"], "1,2")
                self.assertEqual(env["HSA_XNACK"], "0")
            self.assertEqual(shlex.split(shlex.join(command)), command)

    def test_remote_builds_are_owned_and_bounded(self):
        owned = Path(N.PREFIX + "0123456789abcdef")
        specs = N.build_specs(owned)
        self.assertEqual([s[0] for s in specs], ["build-hip", "build-hsa"])
        for _, command, seconds in specs:
            self.assertEqual(seconds, 180)
            self.assertIn("-O3", command)
            self.assertIn("-Werror", command)
            self.assertTrue(command[-1].startswith(str(owned) + "/"))
        self.assertEqual(N.H.sha(Path(N.H.__file__)), N.HOT_SHA)
        self.assertEqual(N.H.sha(Path(N.B.__file__)), N.H.BASE_SHA)
        self.assertIn(N.PREFIX.encode(), C.control_bytes())


if __name__ == "__main__":
    unittest.main()
