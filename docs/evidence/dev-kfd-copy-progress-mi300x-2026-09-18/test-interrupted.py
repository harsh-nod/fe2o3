#!/usr/bin/env python3
"""Test the real interrupted receipt without fabricating a successful campaign."""

import json
from pathlib import Path
import re
import types
import unittest

archive = Path(__file__).resolve().parent
partial = types.ModuleType("interrupted")
partial.__file__ = str(archive / "interrupted.py")
exec(
    compile(Path(partial.__file__).read_bytes(), partial.__file__, "exec"),
    partial.__dict__,
)
summary = partial.summary


class InterruptedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.raw = (archive / "raw/benchmark.log").read_text()
        cls.report = partial.parse(cls.raw)

    def reject(self, old, new):
        changed = self.raw.replace(old, new, 1)
        self.assertNotEqual(changed, self.raw)
        with self.assertRaises((AssertionError, IndexError, KeyError, ValueError)):
            partial.parse(changed)

    def payload(self, cell):
        text = self.raw.split(f"phase repetition=1 cell={cell}\n", 1)[1].split(
            f"completed repetition=1 cell={cell} exit=0", 1
        )[0]
        return [line for line in text.splitlines() if line.startswith("schema=")]

    def test_exact_incomplete_roster(self):
        self.assertFalse(self.report["accepted_campaign"])
        self.assertEqual(self.report["planned_processes"], 16)
        self.assertEqual(self.report["completed_processes"], 8)
        self.assertEqual(
            [(r["repetition"], r["cell"]) for r in self.report["rows"]],
            summary.EXPECTED[:8],
        )
        self.assertEqual(sum(len(r["rounds"]) for r in self.report["rows"]), 104)

    def test_full_campaign_refuses_actual_receipt(self):
        self.assertEqual((archive / "raw/benchmark.exit").read_text(), "1\n")
        with self.assertRaises(AssertionError):
            summary.parse(self.raw)
        with self.assertRaises(AssertionError):
            summary.main()

    def test_policy_and_geometry(self):
        for old, new in (
            ("wait_policy=slice50us", "wait_policy=window-deadline"),
            ("wait_slice_ns=50000", "wait_slice_ns=0"),
            ("packets_per_transfer=65", "packets_per_transfer=64"),
            ("windows_per_transfer=2", "windows_per_transfer=1"),
            ("requested_engine_mask=2", "requested_engine_mask=1"),
            ("host_grain=fine", "host_grain=coarse"),
        ):
            with self.subTest(old=old):
                self.reject(old, new)

    def test_counts(self):
        for cell in ("A", "B"):
            lines = self.payload(cell)
            for count in [0, 1] if cell == "A" else [0, 1, 3]:
                changed = lines.copy()
                for key in ("h2d_wait_calls", "h2d_flush_calls"):
                    changed[1] = re.sub(rf"{key}=[0-9]+", f"{key}={count}", changed[1])
                with self.assertRaises(AssertionError):
                    summary.validate_kfd(changed, cell)
            changed = lines.copy()
            changed[1] = re.sub(
                r"d2h_flush_calls=[0-9]+", "d2h_flush_calls=99999", changed[1]
            )
            with self.assertRaises(AssertionError):
                summary.validate_kfd(changed, cell)

    def test_nearest_cpu(self):
        for cell in ("C", "D"):
            line = next(line for line in self.payload(cell) if "record=config" in line)
            config = summary.fields(line)
            handle = config["cpu_agent"] if cell == "C" else "1"
            changed = re.sub(
                r"nearest_cpu_agent=[0-9]+", f"nearest_cpu_agent={handle}", line
            )
            self.reject(line, changed)
        self.reject("cpu_driver_node=1", "cpu_driver_node=0")

    def test_pool_access(self):
        config = next(
            row["config"] for row in self.report["rows"] if row["cell"] == "C"
        )
        pool = next(
            line
            for line in self.payload("C")
            if "record=pool owner=cpu" in line
            and f"handle={config['host_pool']} " in line
        )
        for key, value in (
            ("cpu_access", "0"),
            ("gpu_access", "0"),
            ("alignment", "3"),
            ("granule", "0"),
            ("max_aggregate_bytes", "1"),
        ):
            self.reject(pool, re.sub(rf"{key}=[0-9]+", f"{key}={value}", pool))

    def test_rounds_and_totals(self):
        for cell in ("A", "B", "C", "D"):
            lines = self.payload(cell)
            row = next(line for line in lines if "record=round" in line)
            for changed in (
                "",
                row.replace("index=0", "index=1"),
                row.replace("phase=warmup", "phase=sample"),
                row.replace("h2d_total_ns=", "h2d_total_ns=99"),
                row + " h2d_total_ns=0",
            ):
                self.reject(row, changed)
            completion = lines[-1]
            self.reject(completion, "")

    def test_warmup_exclusion_and_nearest_rank(self):
        values = [9999, 9999, 9999, 10, 1, 7, 3, 5, 2, 8, 4, 9, 6]
        for cell in ("A", "B", "C", "D"):
            lines = self.payload(cell)
            component = "progress" if cell in ("A", "B") else "wait_reset"
            for i, line in enumerate(lines):
                if "record=round" not in line:
                    continue
                index = int(summary.fields(line)["index"])
                for direction in ("h2d", "d2h"):
                    for field, value in (
                        ("submit", 0),
                        (component, values[index]),
                        ("total", values[index]),
                    ):
                        line = re.sub(
                            rf"{direction}_{field}_ns=[0-9]+",
                            f"{direction}_{field}_ns={value}",
                            line,
                        )
                lines[i] = line
            row = (
                summary.validate_kfd(lines, cell)
                if cell in ("A", "B")
                else summary.validate_hsa(lines, cell)
            )
            self.assertEqual(row["metrics"]["h2d_total_p50_ns"], 5)
            self.assertEqual(row["metrics"]["d2h_total_p95_ns"], 10)

    def test_event_order_and_exits(self):
        for old, new in (
            ("phase repetition=1 cell=A", "phase repetition=1 cell=B"),
            (
                "completed repetition=1 cell=A exit=0",
                "completed repetition=1 cell=A exit=2",
            ),
            (
                "postflight repetition=1 cell=A exit=0",
                "postflight repetition=1 cell=A exit=1",
            ),
            ("teardown=explicit-complete", "teardown=incomplete"),
            ("shutdown=1", "shutdown=0"),
            (partial.STOP, "phase repetition=3 cell=D\n"),
        ):
            self.reject(old, new)

    def test_successful_guard_corruption(self):
        first = next(
            line for line in self.raw.splitlines() if line.startswith('{"card')
        )
        for key, value in (
            ("GPU use (%)", "1"),
            ("VRAM Total Used Memory (B)", "536870912"),
            ("Unique ID", "bad"),
        ):
            changed = json.loads(first)
            changed["card4"][key] = value
            self.reject(first, json.dumps(changed))
        self.reject('"card4": {', '"card4": {"Unique ID": "bad",')
        self.reject("\n0 \n", "\n4 \n")
        self.reject("admitted gpu=4 uid=0x54f88318ca05093d bdf=0000:85:00.0", "")

    def test_failed_guard_must_retain_evidence(self):
        tail = self.raw.split(partial.STOP)[1]
        for line in tail.splitlines():
            if not line.startswith('{"card'):
                continue
            changed = json.loads(line)
            changed["card4"]["VRAM Total Used Memory (B)"] = "298647552"
            self.reject(line, json.dumps(changed))

    def test_terminal_cleanup_and_truncation(self):
        for old, new in (
            ("finished exit=1", "finished exit=0"),
            ("source_after_exit=0", "source_after_exit=1"),
            ("binaries_after_exit=0", "binaries_after_exit=1"),
            ("occupancy_exit=1", "occupancy_exit=0"),
        ):
            self.reject(old, new)
        for changed in (
            self.raw + "extra payload\n",
            self.raw[: self.raw.rfind("finished exit=")],
        ):
            with self.assertRaises(AssertionError):
                partial.parse(changed)


if __name__ == "__main__":
    unittest.main()
