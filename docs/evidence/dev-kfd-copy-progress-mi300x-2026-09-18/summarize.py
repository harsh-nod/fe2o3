#!/usr/bin/env python3
"""Validate this closed experiment and report process-level diagnostics only."""

import json
from pathlib import Path
import re
import statistics


ARCHIVE = Path(__file__).resolve().parent
SCHEMA = "fe2o3.hsa-pool-engine-diagnostic.v1"
KFD_SCHEMA = "fe2o3.kfd-directional-progress-diagnostic.v1"
UID = "54f88318ca05093d"
BYTES = 268435456
ORDERS = ("ABDC", "BCAD", "CDBA", "DACB")
CELLS = {
    "A": "KFD slice50us",
    "B": "KFD window-deadline",
    "C": "HSA fine/mask2/CPU0",
    "D": "HSA fine/mask2/CPU1-nearest",
}
EXPECTED = [(str(i), cell) for i, order in enumerate(ORDERS, 1) for cell in order]
CONTEXT = {
    "diagnostic": "shared-host-kfd-copy-progress",
    "git_commit": "3e12ef82bbb41fb116afbb7ddf7cffdad735ec7d",
    "gpu": "4",
    "unique_id": "0x" + UID,
    "bytes": str(BYTES),
    "depth": "1",
    "warmups": "3",
    "samples": "10",
    "repetitions": "4",
    "optional_context_journal": "disabled",
    "placement": "cpu48-95-memory1",
}


def fields(line):
    result = {}
    for index, item in enumerate(line.split()):
        if "=" not in item:
            assert index == 0 and item in (
                "context",
                "phase",
                "completed",
                "postflight",
                "finished",
                "admitted",
            ), ("unexpected token", line)
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
    assert cell in ("C", "D")
    grain, engine = "fine", "2"
    cpu_index = "0" if cell == "C" else "1"
    require(
        config,
        {
            "record": "config",
            "gpu_index": "0",
            "cpu_index": cpu_index,
            "cpu_driver_node": cpu_index,
            "gpu_driver_node": "6",
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
    assert (config["cpu_agent"] == config["nearest_cpu_agent"]) == (cell == "D")
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


def validate_kfd(lines, cell):
    assert cell in ("A", "B") and len(lines) == 15
    rows = [fields(line) for line in lines]
    assert all(row["schema"] == KFD_SCHEMA for row in rows)
    config, rounds, complete = rows[0], rows[1:-1], rows[-1]
    require(
        config,
        {
            "record": "config",
            "backend": "kfd",
            "unique_id": UID,
            "target": "gfx942:xnack-",
            "bytes": str(BYTES),
            "depth": "1",
            "warmups": "3",
            "samples": "10",
            "wait_policy": "slice50us" if cell == "A" else "window-deadline",
            "wait_slice_ns": "50000" if cell == "A" else "0",
            "outer_timeout_ns": "60000000000",
            "packets_per_transfer": "65",
            "windows_per_transfer": "2",
            "max_packets_per_window": "63",
            "h2d_engine_index": "1",
            "d2h_engine_index": "0",
            "optional_context_journal": "disabled",
            "timing": "host-submit-and-progress",
        },
    )
    for index, row in enumerate(rounds):
        require(
            row,
            {
                "record": "round",
                "index": str(index),
                "phase": "warmup" if index < 3 else "sample",
                "pattern": str(((index % 251) * 67 + 1) % 251 + 1),
                "checked_bytes": str(BYTES),
            },
        )
        for direction in ("h2d", "d2h"):
            assert number(row, direction + "_total_ns") == (
                number(row, direction + "_submit_ns", zero=True)
                + number(row, direction + "_progress_ns", zero=True)
            )
            waits = number(row, direction + "_wait_calls")
            assert waits == number(row, direction + "_flush_calls") and waits >= 2
            if cell == "B":
                assert waits == 2, (
                    "this completed two-window diagnostic expects one wait per window"
                )
    require(
        complete,
        {
            "record": "complete",
            "validated_rounds": "13",
            "measured_rounds": "10",
            "checked_bytes_per_round": str(BYTES),
            "validation": "full-returned-buffer-every-round",
            "teardown": "explicit-complete",
        },
    )
    metrics = {}
    for direction in ("h2d", "d2h"):
        for component in (
            "submit_ns",
            "progress_ns",
            "total_ns",
            "wait_calls",
            "flush_calls",
        ):
            values = [int(r[f"{direction}_{component}"]) for r in rounds[3:]]
            metrics[f"{direction}_{component}_range"] = [min(values), max(values)]
            stem, unit = component.rsplit("_", 1)
            for label, fraction in (("p50", (1, 2)), ("p95", (19, 20))):
                metrics[f"{direction}_{stem}_{label}_{unit}"] = percentile(
                    values, *fraction
                )
    return {"config": config, "rounds": rounds, "metrics": metrics}


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
    validate_guards(text, 33)
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
                    if current[1] in ("C", "D")
                    else validate_kfd(payload, current[1])
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
    return rows


def summarize(rows):
    comparisons = {}
    for first, second in (("B", "A"), ("D", "C"), ("A", "D"), ("B", "D")):
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
    aggregates = {
        cell: {
            direction: {
                "median_process_p50_ns": statistics.median(
                    rows[(str(i), cell)]["metrics"][direction + "_total_p50_ns"]
                    for i in range(1, 5)
                ),
                "process_p50_range_ns": [
                    min(
                        rows[(str(i), cell)]["metrics"][direction + "_total_p50_ns"]
                        for i in range(1, 5)
                    ),
                    max(
                        rows[(str(i), cell)]["metrics"][direction + "_total_p50_ns"]
                        for i in range(1, 5)
                    ),
                ],
            }
            for direction in ("h2d", "d2h")
        }
        for cell in CELLS
    }
    return {
        "scope": "shared-host wait/pool-owner diagnostic, not parity or physical DMA attribution",
        "percentiles": "per process nearest-rank; never pooled across processes",
        "cells": CELLS,
        "aggregates": aggregates,
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
