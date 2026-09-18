#!/usr/bin/env python3
"""CPU-only parser fixtures, independent of native acceptance."""

import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


report = load("smoke_report", ARCHIVE / "report.py")
helper = report.PRIOR / "test_reports.py"
assert (
    hashlib.sha256(helper.read_bytes()).hexdigest()
    == "1ec2885590a94029a43ff75ad35f77d4176fbcec703026c1a81d8be4f8227c18"
)
fixtures = load("prior_fixtures", helper)


def fixture():
    output = ["context " + fixtures.row(report.CONTEXT)]
    for cell in "BC":
        output.extend(
            [
                "phase cell=" + cell,
                fixtures.GUARD.rstrip(),
                *fixtures.native_fixture(cell),
                f"completed cell={cell} exit=0",
                fixtures.GUARD.rstrip(),
                f"postflight cell={cell} exit=0",
            ]
        )
    output.extend(
        [
            fixtures.GUARD.rstrip(),
            "finished exit=0 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0",
        ]
    )
    return "\n".join(output) + "\n"


class ReportTests(unittest.TestCase):
    def test_synthetic_two_policy_smoke(self):
        result = report.report(report.parse(fixture()))
        self.assertTrue(result["native_smoke_accepted"])
        self.assertIsNone(result["performance_comparison"])
        self.assertEqual(result["cells"]["B"]["native_windows"], 52)
        self.assertEqual(result["cells"]["C"]["sleep_ceiling_ns"], 25000)
        self.assertEqual(
            result["cells"]["B"]["cpu_status_counts"],
            {"available": 4, "unavailable": 4, "invalid": 44},
        )

    def test_missing_or_reordered_events_rejected(self):
        good = fixture()
        variants = [
            good.replace("phase cell=B", "phase cell=C", 1),
            good.replace("postflight cell=B exit=0", "postflight cell=B exit=1", 1),
            good.replace("completed cell=C exit=0", "completed cell=C exit=1", 1),
            good.replace("source_after_exit=0", "source_after_exit=1"),
            good.replace("occupancy_exit=0", "occupancy_exit=1"),
            good.replace("postflight cell=B exit=0\n", "", 1),
            good.rsplit(fixtures.GUARD.rstrip(), 1)[0]
            + "finished exit=0 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0\n",
            good + "unexpected\n",
            good.replace("phase cell=B", "unexpected\nphase cell=B", 1),
            good.replace(
                "completed cell=B exit=0",
                "completed cell=B exit=0\n" + fixtures.native_fixture("B")[1],
                1,
            ),
        ]
        for index, text in enumerate(variants):
            with (
                self.subTest(index=index),
                self.assertRaises((AssertionError, KeyError, IndexError)),
            ):
                report.parse(text)

    def test_wrong_identity_counters_and_shape_rejected(self):
        good = fixture()
        for old, new in (
            ("native_sleep_ceiling_ns=1000000", "native_sleep_ceiling_ns=25000"),
            ("packet_count=63", "packet_count=62"),
            ("completed_prefix_bytes=264239136", "completed_prefix_bytes=264239137"),
            ("backend_submission=1", "backend_submission=2"),
            ("completion_observations=126", "completion_observations=125"),
            ("checked_bytes=268435456", "checked_bytes=1"),
            (
                '"card4": {"Unique ID": "0x54f88318ca05093d"',
                '"card4": {"Unique ID": "0x0000000000000000"',
            ),
        ):
            with self.subTest(field=old), self.assertRaises(AssertionError):
                report.parse(good.replace(old, new, 1))

    def test_occupancy_is_bound_to_selected_gpu(self):
        good = fixture()
        original = next(line for line in good.splitlines() if line.startswith('{"card'))
        for gpu in (0, 4):
            status = json.loads(original)
            status[f"card{gpu}"]["GPU use (%)"] = "1"
            changed = good.replace(original, json.dumps(status), 1)
            if gpu == 4:
                with self.assertRaises(AssertionError):
                    report.parse(changed)
            else:
                self.assertEqual(set(report.parse(changed)), {"B", "C"})

    def test_prior_interruption_never_becomes_smoke_success(self):
        with self.assertRaises(AssertionError):
            report.parse(fixtures.RAW)


if __name__ == "__main__":
    unittest.main()
