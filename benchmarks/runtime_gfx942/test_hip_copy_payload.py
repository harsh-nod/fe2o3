#!/usr/bin/env python3
"""Strict payload calibration; synthetic output is not native execution evidence."""

from dataclasses import replace
import io
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
import tempfile
import unittest

import hip_copy_diagnostic as check


EXPECTED = check.Expected(0, 0x54F88318CA05093D, "gfx942:sramecc+:xnack-", 4096, 1, 2)


def fixture(expected=EXPECTED):
    total = expected.warmups + expected.samples
    config = (
        f"schema={check.SCHEMA} record=config device_index={expected.device_index} "
        f"unique_id={expected.unique_id:016x} target={expected.target} xnack=disabled "
        f"bytes={expected.byte_count} depth=1 warmups={expected.warmups} samples={expected.samples} "
        "host_allocation=hipHostMallocDefault stream=nonblocking engine=runtime_selected allocator_benchmark=disabled\n"
    )
    rounds = "".join(
        f"schema={check.SCHEMA} record=round index={index} "
        f"phase={'warmup' if index < expected.warmups else 'sample'} "
        f"pattern={(index * 67 + 1) % 251 + 1} checked_bytes={expected.byte_count} "
        f"h2d_total_ns={100 + index} d2h_total_ns={200 + index}\n"
        for index in range(total)
    )
    complete = (
        f"schema={check.SCHEMA} record=complete validated_rounds={total} measured_rounds={expected.samples} "
        "allocations_released=3 streams_destroyed=1\n"
    )
    return config + rounds + complete


class HipCopyPayloadTests(unittest.TestCase):
    def check(self, output, *, stderr="", exit_code=0, expected=EXPECTED):
        return check.validate(output, stderr, exit_code, expected)

    def test_exact_payload_without_performance_authority(self):
        result = self.check(fixture())
        self.assertTrue(result["payload_valid"])
        self.assertFalse(result["performance_accepted"])
        self.assertEqual(result["validated_rounds"], 3)
        self.assertEqual(result["measured_rounds"], 2)
        self.assertEqual(
            result["rounds"],
            [
                {
                    "index": i,
                    "phase": "warmup" if i == 0 else "sample",
                    "h2d_ns": 100 + i,
                    "d2h_ns": 200 + i,
                }
                for i in range(3)
            ],
        )

    def test_missing_duplicate_reordered_and_extra_rows(self):
        rows = fixture().splitlines(keepends=True)
        variants = [
            "",
            "".join(rows[1:]),
            "".join(rows[:-1]),
            "".join(rows[:2] + rows[3:]),
            "".join(rows[:2] + [rows[1]] + rows[3:]),
            "".join([rows[0], rows[2], rows[1], *rows[3:]]),
            fixture() + rows[-1],
            rows[-1] + fixture(),
        ]
        for changed in variants:
            with self.subTest(changed=changed):
                with self.assertRaises(ValueError):
                    self.check(changed)

    def test_every_configuration_field_is_bound(self):
        first, rest = fixture().split("\n", 1)
        fields = first.split(" ")
        for index, field in enumerate(fields):
            with self.subTest(field=field):
                changed = fields.copy()
                changed[index] = field.split("=", 1)[0] + "=foreign"
                with self.assertRaises(ValueError):
                    self.check(" ".join(changed) + "\n" + rest)

    def test_external_expected_configuration_cannot_be_self_selected(self):
        for changes in (
            {"device_index": 1},
            {"unique_id": 1},
            {"target": "gfx942:sramecc-:xnack-"},
            {"byte_count": 8192},
            {"warmups": 0},
            {"samples": 3},
        ):
            with self.subTest(changes=changes):
                with self.assertRaises(ValueError):
                    self.check(fixture(), expected=replace(EXPECTED, **changes))

    def test_field_syntax_and_closed_rosters(self):
        baseline = fixture()
        for changed in (
            baseline.replace("record=config", "record=config record=config", 1),
            baseline.replace("record=config", "record=config unknown=1", 1),
            baseline.replace("record=round", "record=round unknown=1", 1),
            baseline.replace("record=complete", "record=complete unknown=1", 1),
            baseline.replace("depth=1 ", "", 1),
            baseline.replace("h2d_total_ns=100 ", "", 1),
            baseline.replace("streams_destroyed=1", ""),
            baseline.replace(" ", "  ", 1),
            baseline.replace(" ", "\t", 1),
            "\n" + baseline,
            baseline + "\n",
            baseline[:-1],
            baseline.replace("\n", "\r\n"),
            baseline + "\u00e9\n",
        ):
            with self.subTest(changed=changed):
                with self.assertRaises(ValueError):
                    self.check(changed)

    def test_wrong_round_or_release_facts(self):
        baseline = fixture()
        for old, new in (
            ("index=0", "index=1"),
            ("phase=warmup", "phase=sample"),
            ("pattern=2 ", "pattern=3 "),
            ("checked_bytes=4096", "checked_bytes=4095"),
            ("validated_rounds=3", "validated_rounds=2"),
            ("measured_rounds=2", "measured_rounds=1"),
            ("allocations_released=3", "allocations_released=2"),
            ("streams_destroyed=1", "streams_destroyed=0"),
        ):
            with self.subTest(old=old):
                with self.assertRaises(ValueError):
                    self.check(baseline.replace(old, new, 1))

    def test_noncanonical_zero_and_overflow_intervals(self):
        for field, current in (("h2d_total_ns", "100"), ("d2h_total_ns", "200")):
            for bad in ("0", "-1", "+1", "01", "1.0", "0x10", str(1 << 63), "9" * 21):
                with self.subTest(field=field, bad=bad):
                    with self.assertRaises(ValueError):
                        self.check(
                            fixture().replace(f"{field}={current}", f"{field}={bad}", 1)
                        )

    def test_nonzero_exit_or_stderr_overrides_success_payload(self):
        for code in (1, -9, 124, False, 0.0):
            with self.subTest(code=code):
                with self.assertRaises(ValueError):
                    self.check(fixture(), exit_code=code)
        with self.assertRaises(ValueError):
            self.check(fixture(), stderr="warning\n")

    def test_configuration_limits(self):
        for key, bad_values in {
            "device_index": (-1, 1 << 31, False),
            "unique_id": (0, 1 << 64),
            "byte_count": (0, 268435457),
            "warmups": (-1, 10000),
            "samples": (0, 10000),
            "target": (
                "gfx9420:xnack-",
                "gfx942:xnack+",
                "gfx942:xnack-:xnack+",
                "gfx942:xnack-:xnack-",
                "gfx942:xnack--",
                "gfx942:xnack",
            ),
        }.items():
            for value in bad_values:
                with self.subTest(key=key, value=value):
                    with self.assertRaises(ValueError):
                        replace(EXPECTED, **{key: value})

    def test_zero_warmup_and_maximum_workload(self):
        expected = replace(EXPECTED, warmups=0, samples=10000, byte_count=268435456)
        result = self.check(fixture(expected), expected=expected)
        self.assertEqual(len(result["rounds"]), 10000)
        self.assertTrue(all(row["phase"] == "sample" for row in result["rounds"]))

    def test_bounded_file_and_cli_failure_emit_no_result(self):
        with tempfile.TemporaryDirectory(prefix="fe2o3-hip-payload-") as directory:
            out, err = Path(directory) / "stdout", Path(directory) / "stderr"
            out.write_text(fixture())
            err.write_text("")
            args = [
                "--stdout",
                str(out),
                "--stderr",
                str(err),
                "--exit-code",
                "0",
                "--device-index",
                "0",
                "--unique-id",
                hex(EXPECTED.unique_id),
                "--target",
                EXPECTED.target,
                "--bytes",
                "4096",
                "--warmups",
                "1",
                "--samples",
                "2",
            ]
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                self.assertEqual(check.main(args), 0)
            self.assertIn('"payload_valid": true', stdout.getvalue())
            self.assertEqual(stderr.getvalue(), "")
            out.write_bytes(b"x" * (check.MAX_PAYLOAD + 1))
            stdout, stderr = io.StringIO(), io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                self.assertEqual(check.main(args), 1)
            self.assertEqual(stdout.getvalue(), "")
            self.assertIn("oversized input file", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
