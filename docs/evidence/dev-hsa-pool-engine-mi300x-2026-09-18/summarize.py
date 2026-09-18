#!/usr/bin/env python3
"""Validate this closed experiment and report process-level diagnostics only."""

import json
from pathlib import Path
import re
import statistics


ARCHIVE = Path(__file__).resolve().parent
SCHEMA = "fe2o3.hsa-pool-engine-diagnostic.v1"
UID = "54f88318ca05093d"
BYTES = 268435456
ORDERS = ("ABDC", "BCAD", "CDBA", "DACB")
CELLS = {
    "A": ("fine", "1"),
    "B": ("fine", "2"),
    "C": ("coarse", "1"),
    "D": ("coarse", "2"),
}
EXPECTED = [
    (str(i), cell)
    for i, order in enumerate(ORDERS, 1)
    for cell in ("before", *order, "after")
]
CONTEXT = {
    "diagnostic": "shared-host-hsa-pool-engine",
    "git_commit": "3da2d25ac965afa9845b8ab1246e5fdf13c5d821",
    "gpu": "4",
    "unique_id": "0x" + UID,
    "bytes": str(BYTES),
    "depth": "1",
    "warmups": "3",
    "samples": "10",
    "repetitions": "4",
    "optional_context_journal": "disabled",
    "placement": "cpu48-95-memory1",
    "hsa_cpu_index": "0",
}


def fields(line):
    result = {}
    for item in line.split():
        if "=" not in item:
            continue
        key, value = item.split("=", 1)
        assert key and value and key not in result, ("duplicate or empty field", line)
        result[key] = value
    return result


def require(row, expected):
    for key, value in expected.items():
        assert row.get(key) == value, (key, value, row)


def number(row, key, *, zero=False):
    value = row[key]
    assert value.isascii() and value.isdecimal(), (key, value)
    value = int(value)
    assert value >= (0 if zero else 1), (key, value)
    return value


def percentile(values, numerator, denominator):
    assert values
    return sorted(values)[
        (len(values) * numerator + denominator - 1) // denominator - 1
    ]


def validate_hsa(lines, cell):
    rows = [fields(line) for line in lines]
    for row in rows:
        assert row["schema"] == SCHEMA
    pools = []
    while rows and rows[0]["record"] == "pool":
        pools.append(rows.pop(0))
    assert rows and len(rows) == 15, "one config, thirteen rounds, one completion"
    config, rounds, complete = rows[0], rows[1:-1], rows[-1]
    grain, engine = CELLS[cell]
    require(
        config,
        {
            "record": "config",
            "gpu_index": "0",
            "cpu_index": "0",
            "unique_id": UID,
            "target": "gfx942",
            "xnack": "disabled",
            "gpu_domain": "0",
            "gpu_bdf_id": "34048",
            "bytes": str(BYTES),
            "depth": "1",
            "warmups": "3",
            "samples": "10",
            "host_grain": grain,
            "requested_engine_mask": engine,
            "allocation_flags": "0",
            "force_copy_on_sdma": "0",
            "wait": "blocked_scacquire",
            "host_release": "signal_screlease_before_h2d_timing",
            "timing": "host_submit_wait_reset",
            "host_rounded_bytes": str(BYTES),
            "host_aggregate_bytes": str(2 * BYTES),
            "device_rounded_bytes": str(BYTES),
        },
    )
    for direction in ("h2d", "d2h"):
        assert number(config, direction + "_available_mask") & int(engine) == int(
            engine
        )
    assert number(config, "timeout_hint") == 60 * number(config, "timestamp_frequency")
    for owner, selected, flag, location in (
        ("cpu", "host_pool", "2" if grain == "fine" else "4", "cpu"),
        ("gpu", "device_pool", "4", "gpu"),
    ):
        eligible = []
        for pool in pools:
            if any(
                pool[k] != v
                for k, v in {
                    "owner": owner,
                    "location": location,
                    "global": "1",
                    "allocatable": "1",
                    "flags": flag,
                }.items()
            ):
                continue
            if pool["cpu_access"] not in ("1", "2") or pool["gpu_access"] not in (
                "1",
                "2",
            ):
                continue
            handle = number(pool, "handle", zero=True)
            granule = number(pool, "granule", zero=True)
            alignment = number(pool, "alignment", zero=True)
            if (
                not handle
                or not granule
                or alignment < 4
                or alignment & (alignment - 1)
            ):
                continue
            rounded = (BYTES + granule - 1) // granule * granule
            aggregate = rounded * (2 if owner == "cpu" else 1)
            if aggregate > min(
                number(pool, "max_aggregate_bytes", zero=True), 2**64 - 1
            ):
                continue
            eligible.append(pool)
            assert rounded == BYTES
        assert len(eligible) == 1 and eligible[0]["handle"] == config[selected], (
            "unique eligible pool role"
        )
    for index, row in enumerate(rounds):
        require(
            row,
            {
                "record": "round",
                "index": str(index),
                "phase": "warmup" if index < 3 else "sample",
                "pattern": str(((index % 251) * 67 + 1) % 251 + 1),
                "h2d_signal": "0",
                "d2h_signal": "0",
                "checked_bytes": str(BYTES),
            },
        )
        for direction in ("h2d", "d2h"):
            total = number(row, direction + "_total_ns")
            assert total == number(row, direction + "_submit_ns", zero=True) + number(
                row, direction + "_wait_reset_ns", zero=True
            )
    require(
        complete,
        {
            "record": "complete",
            "validated_rounds": "13",
            "measured_rounds": "10",
            "checked_bytes_per_round": str(BYTES),
            "signals_destroyed": "1",
            "allocations_freed": "3",
            "shutdown": "1",
        },
    )
    metrics = {}
    for direction in ("h2d", "d2h"):
        for component in ("submit", "wait_reset", "total"):
            values = [int(r[f"{direction}_{component}_ns"]) for r in rounds[3:]]
            for label, fraction in (("p50", (1, 2)), ("p95", (19, 20))):
                metrics[f"{direction}_{component}_{label}_ns"] = percentile(
                    values, *fraction
                )
    return {"config": config, "pools": pools, "rounds": rounds, "metrics": metrics}


def validate_kfd(lines):
    assert len(lines) == 1
    row = fields(lines[0])
    require(
        row,
        {
            "backend": "kfd",
            "schema": "fe2o3.async-copy-benchmark.v1",
            "unique_id": UID,
            "profile": "directional",
            "bytes": str(BYTES),
            "depth": "1",
            "queue_depth": "1",
            "batch_size": "1",
            "direction": "h2d-then-d2h",
            "concurrency": "1",
            "configured_queues": "1",
            "doorbells_per_batch": "2",
            "packets_per_transfer": "65",
            "windows_per_transfer": "2",
            "max_packets_per_window": "63",
            "warmups": "3",
            "samples": "10",
            "h2d_engine_index": "1",
            "d2h_engine_index": "0",
            "completion": "facade-wait-and-explicit-continuation-flush",
            "teardown": "explicit",
        },
    )
    for direction in ("h2d", "d2h"):
        p50 = number(row, direction + "_p50_ns")
        assert number(row, direction + "_p95_ns") >= p50
        assert abs(float(row[direction + "_p50_GBps"]) - BYTES / p50) <= 0.000501
    return row


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, ("duplicate JSON key", key)
        result[key] = value
    return result


def validate_guards(text, expected_count):
    observation = None
    processes = set()
    awaiting_devices = None
    count = 0
    for line in text.splitlines():
        if line.startswith('{"card'):
            assert observation is None
            observation = json.loads(line, object_pairs_hook=unique_object)["card4"]
            require(
                observation,
                {
                    "Unique ID": "0x" + UID,
                    "PCI Bus": "0000:85:00.0",
                    "GPU use (%)": "0",
                },
            )
            assert (
                number(observation, "VRAM Total Used Memory (B)", zero=True) < 536870912
            )
            processes = set()
            awaiting_devices = None
        elif line.startswith("admitted "):
            assert observation is not None and processes and awaiting_devices is None
            require(
                fields(line), {"gpu": "4", "uid": "0x" + UID, "bdf": "0000:85:00.0"}
            )
            observation = None
            count += 1
        elif observation is not None:
            if not line.strip() or line.startswith("="):
                continue
            if awaiting_devices is not None:
                devices = line.split()
                assert len(devices) == awaiting_devices
                assert all(
                    d.isascii() and d.isdecimal() and int(d) != 4 for d in devices
                )
                awaiting_devices = None
                continue
            match = re.fullmatch(
                r"PID ([0-9]+) is using ([0-9]+) DRM device\(s\)(:)?", line
            )
            assert match is not None, line
            pid, devices = int(match[1]), int(match[2])
            assert pid not in processes
            processes.add(pid)
            assert bool(match[3]) == (devices > 0)
            awaiting_devices = devices or None
    assert observation is None and count == expected_count, ("guard count", count)


def parse(text):
    validate_guards(text, 49)
    rows, contexts, final = {}, [], []
    current = None
    events = []
    payload = []
    stage = None
    final_guard = False
    pre_admitted = post_admitted = False
    for line in text.splitlines():
        assert not final or not line.strip(), "payload after terminal completion"
        kind = line.split(" ", 1)[0]
        if kind == "context":
            assert not contexts and not events and current is None
            contexts.append(fields(line))
        elif kind == "phase":
            assert current is None and contexts == [CONTEXT] and not final_guard
            row = fields(line)
            current = (row["repetition"], row["cell"])
            assert current == EXPECTED[len(rows)], "counterbalanced order"
            payload = []
            stage = "running"
            pre_admitted = post_admitted = False
            events.append(("phase", current))
        elif kind == "admitted":
            require(
                fields(line), {"gpu": "4", "uid": "0x" + UID, "bdf": "0000:85:00.0"}
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
                assert stage == "running" and pre_admitted
                assert current not in rows
                rows[current] = (
                    validate_hsa(payload, current[1])
                    if current[1] in CELLS
                    else validate_kfd(payload)
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
    placements = set()
    for key, row in rows.items():
        if key[1] in CELLS:
            config = row["config"]
            placements.add(
                (
                    config["cpu_driver_node"],
                    config["gpu_driver_node"],
                    config["cpu_agent"] == config["nearest_cpu_agent"],
                )
            )
    assert len(placements) == 1, (
        "CPU/GPU driver nodes and nearest-CPU relation must be stable"
    )
    return rows


def summarize(rows):
    comparisons = {}
    for first, second in (("B", "A"), ("D", "C"), ("C", "A"), ("D", "B")):
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
    for direction in ("h2d", "d2h"):
        metric = direction + "_p50_ns"
        values = [
            int(rows[(str(i), "after")][metric]) / int(rows[(str(i), "before")][metric])
            for i in range(1, 5)
        ]
        comparisons[f"kfd_after_over_before_{metric}"] = {
            "per_block": values,
            "median": statistics.median(values),
        }
        for cell in CELLS:
            values = [
                statistics.mean(
                    int(rows[(str(i), anchor)][metric])
                    for anchor in ("before", "after")
                )
                / rows[(str(i), cell)]["metrics"][direction + "_total_p50_ns"]
                for i in range(1, 5)
            ]
            comparisons[f"kfd_anchor_mean_over_{cell}_{metric}"] = {
                "per_block": values,
                "median": statistics.median(values),
            }
    return {
        "scope": "shared-host pool/engine diagnostic, not parity or physical DMA attribution",
        "percentiles": "per process nearest-rank; never pooled across processes",
        "cells": CELLS,
        "rows": [
            {"repetition": key[0], "cell": key[1], **rows[key]} for key in EXPECTED
        ],
        "paired_latency_ratios": comparisons,
    }


def main():
    assert (ARCHIVE / "raw/benchmark.exit").read_text().strip() == "0"
    assert (ARCHIVE / "raw/benchmark.finished").is_file()
    print(
        json.dumps(
            summarize(parse((ARCHIVE / "raw/benchmark.log").read_text())), indent=2
        )
    )


if __name__ == "__main__":
    main()
