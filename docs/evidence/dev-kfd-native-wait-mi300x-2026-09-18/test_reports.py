#!/usr/bin/env python3
"""CPU-only parser fixtures. Synthetic success is not hardware evidence."""

import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, ARCHIVE / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


s = load("summarize")
interrupted = load("interrupted")
RAW = (ARCHIVE / "raw/benchmark.log").read_text()
LEGACY = [line for line in RAW.splitlines() if line.startswith("schema=")]
GUARD = RAW.split("phase repetition=1 cell=A\n", 1)[1].split("schema=", 1)[0]


def row(values):
    return " ".join(f"{key}={value}" for key, value in values.items())


def native_fixture(cell):
    ceiling, policy = (
        (1000000, "native-sleep1ms") if cell == "B" else (25000, "native-sleep25us")
    )
    config = s.fields(LEGACY[0])
    del config["wait_slice_ns"]
    config.update(
        schema=s.SCHEMA,
        wait_policy=policy,
        active_spin_floor_ns="50000",
        native_sleep_ceiling_ns=str(ceiling),
        native_scan_timing="host-scan-including-cpu-observation-overhead",
    )
    output = [row(config)]
    for index in range(13):
        timing = s.fields(LEGACY[index + 1])
        timing["schema"] = s.SCHEMA
        for direction in ("h2d", "d2h"):
            timing.update(
                {
                    direction + "_submit_ns": "10",
                    direction + "_progress_ns": "100",
                    direction + "_total_ns": "110",
                    direction + "_wait_calls": "2",
                    direction + "_flush_calls": "2",
                }
            )
        output.append(row(timing))
        for direction_index, direction in enumerate(("h2d", "d2h")):
            for window in range(2):
                offset = 0 if window == 0 else s.FIRST_BYTES
                packets = 63 if window == 0 else 2
                observation = {
                    "schema": s.SCHEMA,
                    "record": "native-window",
                    "round": index,
                    "phase": timing["phase"],
                    "direction": direction,
                    "window": window,
                    "backend_submission": index * 2 + direction_index + 1,
                    "completed_prefix_bytes": offset,
                    "host_offset": offset,
                    "device_offset": offset,
                    "window_bytes": s.FIRST_BYTES
                    if window == 0
                    else s.BYTES - s.FIRST_BYTES,
                    "packet_count": packets,
                    "native_sleep_ceiling_ns": ceiling,
                    "scan_rounds": 2,
                    "completion_observations": 2 * packets,
                    "spin_pauses": 0,
                    "yield_pauses": 0,
                    "sleep_pauses": 1,
                    "requested_sleep_ns": 17,
                    "max_requested_sleep_ns": 17,
                    "scan_ns": 20 + index,
                    "cpu_status": "available"
                    if index == 0
                    else "unavailable"
                    if index == 1
                    else "invalid",
                    "thread_cpu_ns": 3 if index == 0 else "none",
                    "voluntary_context_switches": 1 if index == 0 else "none",
                    "involuntary_context_switches": 0 if index == 0 else "none",
                }
                output.append(row(observation))
    complete = s.fields(LEGACY[-1])
    complete.update(schema=s.SCHEMA, native_windows="52")
    output.append(row(complete))
    return output


def campaign_fixture():
    # The HSA payload is reused only as a schema fixture, never a matched result.
    old = (
        ARCHIVE.parent / "dev-kfd-copy-progress-mi300x-2026-09-18/raw/benchmark.log"
    ).read_text()
    hsa = old.split("phase repetition=1 cell=D\n", 1)[1].split(
        "completed repetition=1 cell=D", 1
    )[0]
    payloads = {
        "A": LEGACY,
        "B": native_fixture("B"),
        "C": native_fixture("C"),
        "D": [line for line in hsa.splitlines() if line.startswith("schema=")],
    }
    output = ["context " + row(s.CONTEXT)]
    for repetition, cell in s.EXPECTED:
        output.extend(
            [
                f"phase repetition={repetition} cell={cell}",
                GUARD.rstrip(),
                *payloads[cell],
                f"completed repetition={repetition} cell={cell} exit=0",
                GUARD.rstrip(),
                f"postflight repetition={repetition} cell={cell} exit=0",
            ]
        )
    output.extend(
        [
            GUARD.rstrip(),
            "finished exit=0 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0",
        ]
    )
    return "\n".join(output) + "\n"


class NativeParserTests(unittest.TestCase):
    def test_closed_policy_and_window_fixtures(self):
        for cell in ("B", "C"):
            with self.subTest(cell=cell):
                result = s.validate_native(native_fixture(cell), cell)
                self.assertEqual(len(result["rounds"]), 13)
                self.assertEqual(sum(map(len, result["windows"])), 52)
                self.assertEqual(result["metrics"]["h2d_total_p50_ns"], 110)
                self.assertIsNone(result["metrics"]["h2d_thread_cpu_ns_p50"])

    def test_available_cpu_metrics(self):
        lines = native_fixture("B")
        for index, line in enumerate(lines):
            if "record=native-window" in line:
                values = s.fields(line)
                values.update(
                    cpu_status="available",
                    thread_cpu_ns="3",
                    voluntary_context_switches="1",
                    involuntary_context_switches="0",
                )
                lines[index] = row(values)
        result = s.validate_native(lines, "B")
        self.assertEqual(result["metrics"]["h2d_thread_cpu_ns_available_rounds"], 10)
        self.assertEqual(result["metrics"]["h2d_thread_cpu_ns_p50"], 6)

    def test_malformed_native_rows_rejected(self):
        mutations = [
            (0, "unique_id", "0000000000000000"),
            (0, "extra", "1"),
            (1, "h2d_total_ns", "111"),
            (1, "h2d_wait_calls", "3"),
            (2, "packet_count", "62"),
            (3, "backend_submission", "2"),
            (3, "host_offset", "0"),
            (7, "backend_submission", "1"),
            (2, "scan_rounds", "3"),
            (2, "completion_observations", "125"),
            (2, "spin_pauses", str(2**64)),
            (2, "yield_pauses", "-1"),
            (2, "max_requested_sleep_ns", "1000001"),
            (2, "requested_sleep_ns", "35"),
            (2, "sleep_pauses", "0"),
            (2, "scan_ns", "101"),
            (2, "cpu_status", "unknown"),
            (2, "thread_cpu_ns", "none"),
            (7, "thread_cpu_ns", "0"),
            (2, "extra", "1"),
            (-1, "native_windows", "51"),
        ]
        for index, key, value in mutations:
            with self.subTest(index=index, key=key):
                lines = native_fixture("B")
                values = s.fields(lines[index])
                values[key] = value
                lines[index] = row(values)
                with self.assertRaises((AssertionError, KeyError, ValueError)):
                    s.validate_native(lines, "B")

    def test_missing_reordered_and_duplicate_rows_rejected(self):
        original = native_fixture("C")
        variants = [
            original[:-1],
            original[:2] + original[3:],
            original[:2] + [original[3], original[2]] + original[4:],
            [original[0] + " depth=1"] + original[1:],
        ]
        for lines in variants:
            with self.subTest(length=len(lines)), self.assertRaises(AssertionError):
                s.validate_native(lines, "C")

    def test_synthetic_full_campaign(self):
        parsed = s.parse(campaign_fixture())
        result = s.summarize(parsed)
        self.assertEqual(len(parsed), 16)
        self.assertEqual(result["validated_rounds"], 208)
        self.assertEqual(
            result["comparisons"]["C_over_B_h2d_total_p50_ns"]["median"], 1.0
        )

    def test_incomplete_and_malformed_campaign_rejected(self):
        good = campaign_fixture()
        for text in (
            RAW,
            good.replace("cell=B", "cell=C", 1),
            good.replace('"GPU use (%)": "0"', '"GPU use (%)": "1"'),
            good.replace("exit=0", "exit=1", 1),
            good.replace(
                "phase repetition=1 cell=A",
                "unexpected-output\nphase repetition=1 cell=A",
            ),
            good + "unexpected-output\n",
            good.replace("postflight repetition=1 cell=A exit=0\n", "", 1),
            good.replace(
                "PID 3161403 is using 1 DRM device(s):\n0 ",
                "PID 3161403 is using 1 DRM device(s):\n4 ",
                1,
            ),
        ):
            with (
                self.subTest(tail=text[-90:]),
                self.assertRaises((AssertionError, KeyError, IndexError)),
            ):
                s.parse(text)


class InterruptionTests(unittest.TestCase):
    def test_real_interruption_has_no_performance_result(self):
        report = interrupted.parse(RAW)
        self.assertFalse(report["accepted_campaign"])
        self.assertEqual(report["completed_legacy_rounds"], 13)
        self.assertEqual(report["performance_qualified_processes"], 0)
        self.assertEqual(report["native_profiled_processes"], 0)
        self.assertIsNone(report["comparisons"])

    def test_interruption_mutations_rejected(self):
        replacements = [
            ("cell=A", "cell=B"),
            (
                "postflight repetition=1 cell=A exit=1",
                "postflight repetition=1 cell=A exit=0",
            ),
            ("occupancy_exit=1", "occupancy_exit=0"),
            ("source_after_exit=0", "source_after_exit=1"),
            ("binaries_after_exit=0", "binaries_after_exit=1"),
            ("clean_exit=0", "clean_exit=1"),
            ('"633720832"', '"298647552"'),
            ('"GPU use (%)": "2"', '"GPU use (%)": "0"'),
            ("validated_rounds=13", "validated_rounds=12"),
            ("h2d_submit_ns=70805", "h2d_submit_ns=70806"),
            ("post_run_porcelain=", "post_run_porcelain=dirty"),
            (
                "completed repetition=1 cell=A exit=0",
                "completed repetition=1 cell=A exit=1",
            ),
            (
                "schema=fe2o3.kfd-directional-progress-diagnostic.v1",
                "schema=" + s.SCHEMA,
            ),
        ]
        variants = [RAW.replace(old, new) for old, new in replacements]
        variants.extend(
            [
                RAW + "phase repetition=1 cell=B\n",
                RAW + "unexpected-output\n",
                RAW.replace("admitted gpu=4", "unknown gpu=4"),
                RAW.replace(
                    '"card4": {"Unique ID":', '"card4": {}, "card4": {"Unique ID":'
                ),
            ]
        )
        for index, text in enumerate(variants):
            with (
                self.subTest(index=index),
                self.assertRaises((AssertionError, KeyError, ValueError)),
            ):
                interrupted.parse(text)


if __name__ == "__main__":
    unittest.main()
