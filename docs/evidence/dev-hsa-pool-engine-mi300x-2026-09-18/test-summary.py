#!/usr/bin/env python3
"""Negative checks against the complete raw campaign; no hardware execution."""

import copy
from pathlib import Path
import re
import types
import unittest

archive = Path(__file__).resolve().parent
summary = types.ModuleType("pool_engine_summary")
summary.__file__ = str(archive / "summarize.py")
exec(
    compile(Path(summary.__file__).read_bytes(), summary.__file__, "exec"),
    summary.__dict__,
)


class SummaryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.raw = (archive / "raw/benchmark.log").read_text()
        assert (archive / "raw/benchmark.exit").read_text().strip() == "0"
        cls.rows = summary.parse(cls.raw)

    def test_exact_roster(self):
        self.assertEqual(list(self.rows), summary.EXPECTED)
        report = summary.summarize(self.rows)
        self.assertEqual(len(report["rows"]), 24)
        self.assertEqual(len(report["paired_latency_ratios"]), 18)
        self.assertTrue(
            all(
                len(v["per_block"]) == 4
                for v in report["paired_latency_ratios"].values()
            )
        )

    def test_nearest_rank(self):
        values = [10, 1, 7, 3, 5, 2, 8, 4, 9, 6]
        self.assertEqual(summary.percentile(values, 1, 2), 5)
        self.assertEqual(summary.percentile(values, 19, 20), 10)

    def test_exact_pairing(self):
        rows = copy.deepcopy(self.rows)
        for i in range(1, 5):
            for direction in ("h2d", "d2h"):
                for cell, value in (("A", 10), ("B", 10 * i), ("C", 20), ("D", 20 * i)):
                    rows[(str(i), cell)]["metrics"][direction + "_total_p50_ns"] = value
                rows[(str(i), "before")][direction + "_p50_ns"] = "10"
                rows[(str(i), "after")][direction + "_p50_ns"] = str(10 * i)
        ratios = summary.summarize(rows)["paired_latency_ratios"]
        self.assertEqual(
            ratios["B_over_A_h2d_total_p50_ns"],
            {"per_block": [1, 2, 3, 4], "median": 2.5},
        )
        self.assertEqual(
            ratios["D_over_C_d2h_total_p50_ns"],
            {"per_block": [1, 2, 3, 4], "median": 2.5},
        )
        self.assertEqual(
            ratios["C_over_A_h2d_total_p50_ns"],
            {"per_block": [2, 2, 2, 2], "median": 2},
        )
        self.assertEqual(
            ratios["kfd_anchor_mean_over_A_h2d_p50_ns"],
            {"per_block": [1, 1.5, 2, 2.5], "median": 1.75},
        )

    def test_warmup_exclusion(self):
        payload = self.raw.split("phase repetition=1 cell=A\n", 1)[1].split(
            "completed repetition=1 cell=A", 1
        )[0]
        lines = [line for line in payload.splitlines() if line.startswith("schema=")]
        values = [9999, 9999, 9999, 10, 1, 7, 3, 5, 2, 8, 4, 9, 6]
        for i, line in enumerate(lines):
            if "record=round" not in line:
                continue
            index = int(summary.fields(line)["index"])
            for direction in ("h2d", "d2h"):
                for component, value in (
                    ("submit", 0),
                    ("wait_reset", values[index]),
                    ("total", values[index]),
                ):
                    line = re.sub(
                        rf"{direction}_{component}_ns=[0-9]+",
                        f"{direction}_{component}_ns={value}",
                        line,
                    )
            lines[i] = line
        metrics = summary.validate_hsa(lines, "A")["metrics"]
        self.assertEqual(metrics["h2d_total_p50_ns"], 5)
        self.assertEqual(metrics["d2h_total_p95_ns"], 10)

    def test_terminal_order(self):
        finished = next(
            line for line in self.raw.splitlines() if line.startswith("finished exit=")
        )
        for mutated in (
            finished + "\n" + self.raw.replace(finished, "", 1),
            self.raw + "unexpected payload\n",
        ):
            with self.assertRaises(AssertionError):
                summary.parse(mutated)
        guard_start = self.raw.rfind('{"card')
        guard_end = self.raw.index("\n", self.raw.index("admitted ", guard_start)) + 1
        guard = self.raw[guard_start:guard_end]
        relocated = guard + self.raw[:guard_start] + self.raw[guard_end:]
        with self.assertRaises(AssertionError):
            summary.parse(relocated)

    def test_pool_eligibility(self):
        config = self.rows[("1", "A")]["config"]
        pool = next(
            line
            for line in self.raw.splitlines()
            if "record=pool owner=cpu" in line
            and f"handle={config['host_pool']} " in line
        )
        for field, value in (
            ("gpu_access", "0"),
            ("cpu_access", "0"),
            ("granule", "0"),
            ("alignment", "3"),
            ("max_aggregate_bytes", "1"),
        ):
            changed = re.sub(rf"{field}=[0-9]+", f"{field}={value}", pool)
            self.assertNotEqual(changed, pool)
            with self.assertRaises(AssertionError):
                summary.parse(self.raw.replace(pool, changed, 1))
        with self.assertRaises(AssertionError):
            summary.parse(
                self.raw.replace(
                    pool, pool + "\n" + pool.replace("handle=", "handle=9"), 1
                )
            )

    def test_duplicate_json_keys(self):
        with self.assertRaises(AssertionError):
            summary.parse(
                self.raw.replace('"card4": {', '"card4": {"Unique ID": "bad",', 1)
            )

    def test_reject_mutations(self):
        first_round = next(
            line for line in self.raw.splitlines() if "record=round" in line
        )
        first_config = next(
            line for line in self.raw.splitlines() if "record=config" in line
        )
        first_completion = next(
            line for line in self.raw.splitlines() if "record=complete" in line
        )
        first_guard = next(
            line for line in self.raw.splitlines() if line.startswith('{"card')
        )
        mutations = [
            ("phase repetition=1 cell=before", "phase repetition=1 cell=after"),
            (
                "completed repetition=1 cell=before exit=0",
                "completed repetition=1 cell=before exit=2",
            ),
            (
                "postflight repetition=1 cell=before exit=0",
                "postflight repetition=1 cell=before exit=1",
            ),
            ("source_after_exit=0", "source_after_exit=1"),
            ("binaries_after_exit=0", "binaries_after_exit=1"),
            ("occupancy_exit=0", "occupancy_exit=1"),
            ("unique_id=54f88318ca05093d", "unique_id=0000000000000001"),
            ("requested_engine_mask=1", "requested_engine_mask=2"),
            ("host_grain=fine", "host_grain=coarse"),
            ("checked_bytes=268435456", "checked_bytes=268435455"),
            ("h2d_signal=0", "h2d_signal=-1"),
            ("d2h_signal=0", "d2h_signal=1"),
            ("phase=warmup", "phase=sample"),
            ("signals_destroyed=1", "signals_destroyed=0"),
            ("allocations_freed=3", "allocations_freed=2"),
            ("shutdown=1", "shutdown=0"),
            ("packets_per_transfer=65", "packets_per_transfer=64"),
            (first_round, ""),
            (first_round, first_round + " h2d_signal=0"),
            (first_round, first_round.replace("h2d_total_ns=", "h2d_total_ns=999")),
            (
                first_config,
                first_config.replace("cpu_driver_node=", "cpu_driver_node=99"),
            ),
            (first_completion, ""),
            (first_guard, ""),
            (
                first_guard,
                first_guard.replace(
                    '"PCI Bus": "0000:85:00.0"', '"PCI Bus": "0000:00:00.0"'
                ),
            ),
            ("admitted gpu=4 uid=0x54f88318ca05093d bdf=0000:85:00.0", ""),
        ]
        for old, new in mutations:
            with self.subTest(old=old[:100], new=new[:100]):
                mutated = self.raw.replace(old, new, 1)
                self.assertNotEqual(mutated, self.raw)
                with self.assertRaises(
                    (AssertionError, KeyError, ValueError, IndexError)
                ):
                    summary.parse(mutated)

    def test_reject_truncation(self):
        with self.assertRaises(AssertionError):
            summary.parse(self.raw[: self.raw.rfind("finished exit=")])

    def test_guard_negative_cases(self):
        raw = (archive / "raw/preflight.log").read_text()
        summary.validate_guards(raw, 1)
        for old, new in (
            ("\n0 \n", "\n4 \n"),
            ('"GPU use (%)": "0"', '"GPU use (%)": "1"'),
        ):
            # Replace every GPU-use field so the selected device is mutated too.
            mutated = raw.replace(old, new)
            self.assertNotEqual(mutated, raw)
            with self.assertRaises(AssertionError):
                summary.validate_guards(mutated, 1)


if __name__ == "__main__":
    unittest.main()
