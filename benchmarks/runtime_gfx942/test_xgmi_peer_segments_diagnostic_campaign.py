#!/usr/bin/env python3
"""CPU-only diagnostic campaign checks; no SSH or native execution."""

import importlib.util
from pathlib import Path
import shlex
import unittest

HERE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = load("xgmi_peer_segments_campaign")
N = load("xgmi_peer_segments_diagnostic_native")


class DiagnosticCampaignTests(unittest.TestCase):
    def test_same_binary_fixed_order_and_only_one_flag_differs(self):
        owned = Path(N.PREFIX + "0123456789abcdef")
        devices = [[1, "0000:26:00.0", "0xab83d2ffef0d3cdf"],
                   [2, "0000:46:00.0", "0xd2e26fef80cf5c33"]]
        specs = N.trial_specs(owned, devices)
        self.assertEqual([s[0] for s in specs], ["1-off", "2-on", "3-on", "4-off"])
        self.assertEqual([s[1] for s in specs], [False, True, True, False])
        self.assertEqual(N.CONTROLS, dict(useful_bytes=65536, descriptor_count=65, warmups=2, samples=10))
        base = [str(owned / "kfd-segments"), *(d[2] for d in devices), "65536", "65", "2", "10"]
        for _, enabled, command, env in specs:
            self.assertEqual(command, base + (["--diagnose-ordered-segments"] if enabled else []))
            self.assertEqual(env, specs[0][3])
            self.assertEqual(env["TMPDIR"], str(owned / "tmp"))
            for key in ["LD_PRELOAD", "HIP_VISIBLE_DEVICES", "ROCR_VISIBLE_DEVICES"]:
                self.assertNotIn(key, env)
            self.assertEqual(shlex.split(shlex.join(command)), command)

    def test_pinned_admission_and_scoped_controller_prefixes(self):
        self.assertEqual(N.sha(Path(N.H.__file__)), N.HOT_SHA)
        self.assertEqual(N.sha(Path(N.B.__file__)), N.H.BASE_SHA)
        self.assertIn(N.PREFIX.encode(), C.control_bytes(N.PREFIX))
        self.assertIn(C.N.PREFIX.encode(), C.control_bytes())
        self.assertNotEqual(N.PREFIX, C.N.PREFIX)
        self.assertEqual(N.PAYLOAD, {"native.py", "results.py", "xgmi_peer_segments_results.py",
                                    "hot.py", "base.py", "source.tar.gz", "kfd-segments"})

    def test_native_protocol_checks_source_before_each_trial_and_settles_failures(self):
        source = (HERE / "xgmi_peer_segments_diagnostic_native.py").read_text()
        trial = source.split("for name, diagnostic, command, phase_env in trial_specs(", 1)[1]
        self.assertLess(trial.index("identities()"), trial.index('observe(name + "-before")'))
        self.assertLess(trial.index('observe(name + "-before")'), trial.index("rec.run(name, command, 300"))
        self.assertLess(trial.index("B.settled_postflight(observe, name, failure)"), trial.index("raise failure"))
        self.assertIn('binary = sha(owned / "kfd-segments")', source)
        self.assertIn('value["features"] == "default,hardware-diagnostic"', source)
        self.assertIn('resource.setrlimit(resource.RLIMIT_CORE, (0, 0))', source)
        self.assertNotIn("subprocess", source)
        self.assertNotIn("rm ", source)


if __name__ == "__main__":
    unittest.main()
