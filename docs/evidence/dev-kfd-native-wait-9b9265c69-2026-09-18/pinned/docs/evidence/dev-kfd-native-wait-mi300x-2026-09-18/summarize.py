#!/usr/bin/env python3
"""Closed matched campaign; host observations are not DMA durations or proof."""

import importlib.util
import hashlib
import json
from pathlib import Path
import re
import statistics
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
HELPER = ARCHIVE.parent / "dev-kfd-copy-progress-mi300x-2026-09-18/summarize.py"
assert (
    hashlib.sha256(HELPER.read_bytes()).hexdigest()
    == "d294b958931219530e0d5cd7e69a657e4e953bde93800720f9c7887b9476baf4"
)
spec = importlib.util.spec_from_file_location("prior_copy_protocol", HELPER)
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
fields, require, number, percentile = (
    base.fields,
    base.require,
    base.number,
    base.percentile,
)
SCHEMA = "fe2o3.kfd-directional-native-wait-diagnostic.v1"
COMMIT = "fcd5a89a113c6538338598bfcdb6769fb5c06042"
BYTES = 268435456
FIRST_BYTES = 4194272 * 63
ORDERS = ("ABDC", "BCAD", "CDBA", "DACB")
EXPECTED = [(str(i), cell) for i, order in enumerate(ORDERS, 1) for cell in order]
CELLS = {
    "A": "KFD slice50us",
    "B": "KFD profiled sleep1ms",
    "C": "KFD profiled sleep25us",
    "D": "HSA fine/mask2/CPU1-nearest",
}
CONTEXT = dict(
    base.CONTEXT, diagnostic="shared-host-kfd-native-wait", git_commit=COMMIT
)
OWNED = "/tmp/fe2o3-kfd-native-wait-20260918.LiBKebIz"
BINARIES = (
    OWNED + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
    OWNED + "/hsa-copy-pool-engine",
)
TIMESTAMP = r"2026-09-18T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{9}Z"
TOPOLOGY = "topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=4 pci_bdf=0000:85:00.0 unique_id=0x54f88318ca05093d numa_node=1 device_local_cpu_list=48-95 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=48-95 observer_cpu=47 kfd_node=6 kfd_gpu_id=53458 topology_sha256=8c7286133d538e8f9b84cdf997f6f3a343ba1b9e31ed73b0aa4ecf90561e35eb"
RUNNER_METADATA = {
    TOPOLOGY,
    "policy: bind",
    "preferred node: 1",
    "physcpubind: " + " ".join(str(i) for i in range(48, 96)),
    "cpubind: 1",
    "nodebind: 1",
    "membind: 1",
    "preferred: 1",
    "post_run_porcelain=",
} | {binary + ": OK" for binary in BINARIES}


def runner_metadata(line):
    return (
        not line.strip()
        or line.rstrip() in RUNNER_METADATA
        or re.fullmatch(TIMESTAMP, line)
    )


def guard_line_indices(lines):
    """Exclude only complete, independently validated guard blocks from parsing."""
    start, indices = None, set()
    for index, line in enumerate(lines):
        if line.startswith('{"card'):
            assert start is None
            start = index
        elif line.startswith("admitted "):
            assert start is not None
            base.validate_guards("\n".join(lines[start : index + 1]), 1)
            indices.update(range(start, index))
            start = None
    assert start is None
    return indices


def validate_native(lines, cell):
    assert cell in ("B", "C") and len(lines) == 67
    rows = [fields(line) for line in lines]
    assert all(row.get("schema") == SCHEMA for row in rows)
    ceiling = 1000000 if cell == "B" else 25000
    config, complete = rows[0], rows[-1]
    assert config == {
        "schema": SCHEMA,
        "record": "config",
        "backend": "kfd",
        "unique_id": base.UID,
        "target": "gfx942:xnack-",
        "bytes": str(BYTES),
        "depth": "1",
        "warmups": "3",
        "samples": "10",
        "wait_policy": "native-sleep1ms" if cell == "B" else "native-sleep25us",
        "outer_timeout_ns": "60000000000",
        "active_spin_floor_ns": "50000",
        "native_sleep_ceiling_ns": str(ceiling),
        "packets_per_transfer": "65",
        "windows_per_transfer": "2",
        "max_packets_per_window": "63",
        "h2d_engine_index": "1",
        "d2h_engine_index": "0",
        "optional_context_journal": "disabled",
        "timing": "host-submit-and-progress",
        "native_scan_timing": "host-scan-including-cpu-observation-overhead",
    }
    assert complete == {
        "schema": SCHEMA,
        "record": "complete",
        "validated_rounds": "13",
        "measured_rounds": "10",
        "checked_bytes_per_round": str(BYTES),
        "native_windows": "52",
        "validation": "full-returned-buffer-every-round",
        "teardown": "explicit-complete",
    }
    rounds, windows, prior_submission = [], [], 0
    for index in range(13):
        row = rows[1 + index * 5]
        group = rows[2 + index * 5 : 6 + index * 5]
        phase = "warmup" if index < 3 else "sample"
        require(
            row,
            {
                "record": "round",
                "index": str(index),
                "phase": phase,
                "pattern": str(((index % 251) * 67 + 1) % 251 + 1),
                "checked_bytes": str(BYTES),
            },
        )
        expected_round_keys = {
            "schema",
            "record",
            "index",
            "phase",
            "pattern",
            "checked_bytes",
        }
        for direction_index, direction in enumerate(("h2d", "d2h")):
            expected_round_keys.update(
                direction + "_" + suffix
                for suffix in (
                    "submit_ns",
                    "progress_ns",
                    "total_ns",
                    "wait_calls",
                    "flush_calls",
                )
            )
            assert number(row, direction + "_total_ns") == number(
                row, direction + "_submit_ns", zero=True
            ) + number(row, direction + "_progress_ns", zero=True)
            assert (
                number(row, direction + "_wait_calls")
                == number(row, direction + "_flush_calls")
                == 2
            )
            pair = group[direction_index * 2 : direction_index * 2 + 2]
            submission = number(pair[0], "backend_submission")
            assert prior_submission < submission < 2**64
            prior_submission = submission
            for window, observed in enumerate(pair):
                offset, packets = (0, 63) if window == 0 else (FIRST_BYTES, 2)
                fixed = {
                    "schema": SCHEMA,
                    "record": "native-window",
                    "round": str(index),
                    "phase": phase,
                    "direction": direction,
                    "window": str(window),
                    "backend_submission": str(submission),
                    "completed_prefix_bytes": str(offset),
                    "host_offset": str(offset),
                    "device_offset": str(offset),
                    "window_bytes": str(
                        FIRST_BYTES if window == 0 else BYTES - FIRST_BYTES
                    ),
                    "packet_count": str(packets),
                    "native_sleep_ceiling_ns": str(ceiling),
                }
                require(observed, fixed)
                counters = (
                    "scan_rounds",
                    "completion_observations",
                    "spin_pauses",
                    "yield_pauses",
                    "sleep_pauses",
                    "requested_sleep_ns",
                    "max_requested_sleep_ns",
                    "scan_ns",
                )
                costs = (
                    "thread_cpu_ns",
                    "voluntary_context_switches",
                    "involuntary_context_switches",
                )
                assert set(observed) == set(fixed) | set(counters) | set(costs) | {
                    "cpu_status"
                }
                values = {key: number(observed, key, zero=True) for key in counters}
                assert all(value < 2**64 for value in values.values())
                assert (
                    values["scan_rounds"]
                    == values["spin_pauses"]
                    + values["yield_pauses"]
                    + values["sleep_pauses"]
                    + 1
                )
                assert (
                    values["completion_observations"] == values["scan_rounds"] * packets
                )
                assert values["max_requested_sleep_ns"] <= min(
                    ceiling, values["requested_sleep_ns"]
                )
                assert (
                    values["requested_sleep_ns"]
                    <= values["sleep_pauses"] * values["max_requested_sleep_ns"]
                )
                assert values["sleep_pauses"] or values["max_requested_sleep_ns"] == 0
                if observed["cpu_status"] == "available":
                    assert all(
                        number(observed, key, zero=True) < 2**64 for key in costs
                    )
                else:
                    assert observed["cpu_status"] in ("unavailable", "invalid")
                    assert all(observed[key] == "none" for key in costs)
            assert sum(int(w["scan_ns"]) for w in pair) <= int(
                row[direction + "_progress_ns"]
            )
        assert set(row) == expected_round_keys
        rounds.append(row)
        windows.append(group)
    metrics = {}
    for direction_index, direction in enumerate(("h2d", "d2h")):
        for component in (
            "submit_ns",
            "progress_ns",
            "total_ns",
            "wait_calls",
            "flush_calls",
        ):
            values = [int(row[direction + "_" + component]) for row in rounds[3:]]
            stem, unit = component.rsplit("_", 1)
            for label, fraction in (("p50", (1, 2)), ("p95", (19, 20))):
                metrics[f"{direction}_{stem}_{label}_{unit}"] = percentile(
                    values, *fraction
                )
        pairs = [
            group[direction_index * 2 : direction_index * 2 + 2]
            for group in windows[3:]
        ]
        for component in (
            "scan_ns",
            "sleep_pauses",
            "requested_sleep_ns",
            "thread_cpu_ns",
            "voluntary_context_switches",
            "involuntary_context_switches",
        ):
            available = [
                sum(int(w[component]) for w in pair)
                for pair in pairs
                if all(w[component] != "none" for w in pair)
            ]
            metrics[f"{direction}_{component}_available_rounds"] = len(available)
            metrics[f"{direction}_{component}_p50"] = (
                percentile(available, 1, 2) if len(available) == 10 else None
            )
        metrics[f"{direction}_maximum_requested_sleep_ns"] = max(
            int(w["max_requested_sleep_ns"]) for pair in pairs for w in pair
        )
    return {"config": config, "rounds": rounds, "windows": windows, "metrics": metrics}


def parse(text):
    base.validate_guards(text, 33)
    lines = text.splitlines()
    guard_indices = guard_line_indices(lines)
    rows, contexts, final = {}, [], []
    current = None
    events, payload = [], []
    stage = None
    final_guard = pre_admitted = post_admitted = False
    for index, line in enumerate(lines):
        assert not final or not line.strip(), "payload after terminal completion"
        if index in guard_indices:
            continue
        kind = line.split(" ", 1)[0]
        if kind == "context":
            assert not contexts and not events and current is None
            contexts.append(fields(line))
        elif kind == "phase":
            assert current is None and contexts == [CONTEXT] and not final_guard
            row = fields(line)
            current = (row["repetition"], row["cell"])
            assert current == EXPECTED[len(rows)], "counterbalanced order"
            payload, stage = [], "running"
            pre_admitted = post_admitted = False
            events.append(("phase", current))
        elif kind == "admitted":
            require(
                fields(line),
                {"gpu": "4", "uid": "0x" + base.UID, "bdf": "0000:85:00.0"},
            )
            if current is None:
                assert len(events) == len(EXPECTED) * 3 and not final_guard
                final_guard = True
            elif stage == "running":
                assert not pre_admitted and not payload
                pre_admitted = True
            else:
                assert stage == "completed" and not post_admitted
                post_admitted = True
        elif line.startswith(("schema=", "backend=")):
            assert current and stage == "running" and pre_admitted
            payload.append(line)
        elif kind in ("completed", "postflight"):
            row = fields(line)
            assert current == (row["repetition"], row["cell"]) and row["exit"] == "0"
            if kind == "completed":
                assert stage == "running" and pre_admitted and current not in rows
                cell = current[1]
                rows[current] = (
                    base.validate_kfd(payload, "A")
                    if cell == "A"
                    else base.validate_hsa(payload, "D")
                    if cell == "D"
                    else validate_native(payload, cell)
                )
                stage = "completed"
            else:
                assert stage == "completed" and post_admitted
            events.append((kind, current))
            if kind == "postflight":
                current = None
        elif kind == "finished":
            assert current is None and final_guard
            final.append(fields(line))
        else:
            assert runner_metadata(line), ("unexpected runner output", line)
    assert current is None and contexts == [CONTEXT]
    assert events == [
        (event, key)
        for key in EXPECTED
        for event in ("phase", "completed", "postflight")
    ]
    assert final == [
        {
            "exit": "0",
            "source_after_exit": "0",
            "binaries_after_exit": "0",
            "clean_exit": "0",
            "occupancy_exit": "0",
        }
    ]
    return rows


def summarize(rows):
    comparisons = {}
    for first, second in (("C", "B"), ("C", "D"), ("B", "D"), ("A", "D")):
        for direction in ("h2d", "d2h"):
            metric = direction + "_total_p50_ns"
            values = [
                rows[(str(i), first)]["metrics"][metric]
                / rows[(str(i), second)]["metrics"][metric]
                for i in range(1, 5)
            ]
            comparisons[f"{first}_over_{second}_{metric}"] = {
                "per_block": values,
                "median": statistics.median(values),
            }
    return {
        "scope": "host-time diagnostic only; shared-host observations, no reservation, no device-duration or parity claim",
        "source": COMMIT,
        "accepted_campaign": True,
        "cells": CELLS,
        "processes": 16,
        "validated_rounds": 208,
        "comparisons": comparisons,
        "process_metrics": {"-".join(key): row["metrics"] for key, row in rows.items()},
        "interpretation": "B/C changes only the requested sleep ceiling inside a profiled full-deadline route; A also changes re-entry and instrumentation; HSA mask2 is not established as physical KFD engine equivalence",
    }


if __name__ == "__main__":
    assert (ARCHIVE / "raw/benchmark.exit").read_text() == "0\n"
    print(
        json.dumps(
            summarize(parse((ARCHIVE / "raw/benchmark.log").read_text())), indent=2
        )
    )
