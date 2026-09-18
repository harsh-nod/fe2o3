#!/usr/bin/env python3
"""Synthetic parser calibration is never hardware or performance evidence."""

import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("summary", ARCHIVE / "summarize.py")
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)
fixture = summary.load(
    "fixture",
    summary.PRIOR / "test_reports.py",
    "1ec2885590a94029a43ff75ad35f77d4176fbcec703026c1a81d8be4f8227c18",
)
fixture.s = summary.protocol
NativeParserTests = fixture.NativeParserTests
spec = importlib.util.spec_from_file_location(
    "interruption", ARCHIVE / "interrupted.py"
)
interruption = importlib.util.module_from_spec(spec)
spec.loader.exec_module(interruption)


class CampaignIdentityTests(unittest.TestCase):
    def test_old_source_context_rejected(self):
        good = fixture.campaign_fixture()
        bad = good.replace(summary.COMMIT, "fcd5a89a113c6538338598bfcdb6769fb5c06042")
        self.assertNotEqual(good, bad)
        with self.assertRaises(AssertionError):
            summary.protocol.parse(bad)

    def test_old_binary_paths_rejected(self):
        good = fixture.campaign_fixture()
        old = "/tmp/fe2o3-kfd-native-wait-20260918.LiBKebIz/hsa-copy-pool-engine: OK\n"
        with self.assertRaises(AssertionError):
            summary.protocol.parse(old + good)

    def test_current_binary_metadata_and_source_accepted(self):
        text = "\n".join(path + ": OK" for path in summary.protocol.BINARIES)
        parsed = summary.protocol.parse(text + "\n" + fixture.campaign_fixture())
        self.assertEqual(summary.protocol.summarize(parsed)["source"], summary.COMMIT)
        self.assertEqual(len(parsed), 16)


class InterruptionTests(unittest.TestCase):
    def test_closed_native_interruption_has_no_performance_result(self):
        result = interruption.parse((ARCHIVE / "raw/benchmark.log").read_text())
        self.assertFalse(result["accepted_campaign"])
        self.assertEqual(result["completed_legacy_rounds"], 13)
        self.assertEqual(result["performance_qualified_processes"], 0)
        self.assertEqual(result["postflight_attached_gpu_indices"], list(range(8)))
        self.assertIsNone(result["comparisons"])

    def test_interruption_corruption_rejected(self):
        text = (ARCHIVE / "raw/benchmark.log").read_text()
        for bad in (
            text + "unexpected output\n",
            text.replace("cell=A exit=0", "cell=A exit=1", 1),
            text.replace("PID 536718", "PID 536719", 1),
            text.replace("0 2 4 6 1 3 5 7", "0 2 0 6 1 3 5 7", 1),
            text.replace("298659840", "298659841"),
        ):
            self.assertNotEqual(text, bad)
            with (
                self.subTest(bad=bad[-80:]),
                self.assertRaises((AssertionError, KeyError)),
            ):
                interruption.parse(bad)

    def test_pid_rosters_reject_malformed_or_missing_members(self):
        for text in (
            "PID 7 is using 2 DRM device(s):\n4\n",
            "PID 7 is using 2 DRM device(s):\n4 4\n",
            "PID 7 is using 1 DRM device(s):\n8\n",
            "PID 7 is using 0 DRM device(s)\nPID 7 is using 0 DRM device(s)\n",
            "unexpected output\n",
        ):
            with self.subTest(text=text), self.assertRaises(AssertionError):
                interruption.parse_pids(text.splitlines())


if __name__ == "__main__":
    unittest.main()
