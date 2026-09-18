#!/usr/bin/env python3
"""Validate native execution without treating this smoke run as a comparison."""

from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
PRIOR = ARCHIVE.parent / "dev-kfd-native-wait-mi300x-2026-09-18"
HELPER = PRIOR / "summarize.py"
assert (
    hashlib.sha256(HELPER.read_bytes()).hexdigest()
    == "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9"
)
spec = importlib.util.spec_from_file_location("native_protocol", HELPER)
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)
OWNED = "/tmp/fe2o3-kfd-native-wait-20260918.UB2Je4Dz"
BINARIES = tuple(binary.replace(s.OWNED, OWNED) for binary in s.BINARIES)
CONTEXT = {
    "diagnostic": "native-wait-smoke",
    "git_commit": s.COMMIT,
    "gpu": "4",
    "unique_id": "0x" + s.base.UID,
    "bytes": str(s.BYTES),
    "depth": "1",
    "warmups": "3",
    "samples": "10",
    "placement": "cpu48-95-memory1",
    "performance_comparison": "none",
}


def parse(text):
    s.base.validate_guards(text, 5)
    lines = text.splitlines()
    guards = s.guard_line_indices(lines)
    events, payload, results = [], [], {}
    current, terminal = None, False
    for index, line in enumerate(lines):
        assert not terminal or not line.strip(), "output after terminal record"
        if index in guards:
            continue
        kind = line.split(" ", 1)[0]
        if kind == "context":
            assert not events and s.fields(line) == CONTEXT
            events.append("context")
        elif kind == "phase":
            assert current is None and len(results) < 2
            current = "BC"[len(results)]
            assert line == "phase cell=" + current
            payload = []
            events.append("phase:" + current)
        elif kind == "admitted":
            assert s.fields(line) == {
                "gpu": "4",
                "uid": "0x" + s.base.UID,
                "bdf": "0000:85:00.0",
            }
            events.append("admitted")
        elif line.startswith("schema="):
            assert current and current not in results and events[-1] == "admitted"
            payload.append(line)
        elif kind == "completed":
            assert (
                current
                and current not in results
                and line == f"completed cell={current} exit=0"
            )
            results[current] = s.validate_native(payload, current)
            events.append("completed:" + current)
        elif kind == "postflight":
            assert current in results and line == f"postflight cell={current} exit=0"
            events.append("postflight:" + current)
            current = None
        elif kind == "finished":
            assert (
                current is None
                and line
                == "finished exit=0 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0"
            )
            events.append("finished")
            terminal = True
        else:
            normalized = line.replace(OWNED, s.OWNED)
            assert s.runner_metadata(normalized), ("unexpected output", line)
    assert events == [
        "context",
        "phase:B",
        "admitted",
        "completed:B",
        "admitted",
        "postflight:B",
        "phase:C",
        "admitted",
        "completed:C",
        "admitted",
        "postflight:C",
        "admitted",
        "finished",
    ]
    return results


def report(results):
    cells = {}
    for cell, result in results.items():
        windows = [window for group in result["windows"] for window in group]
        cells[cell] = {
            "policy": result["config"]["wait_policy"],
            "validated_rounds": len(result["rounds"]),
            "native_windows": len(windows),
            "cpu_status_counts": dict(
                Counter(window["cpu_status"] for window in windows)
            ),
            "sleep_ceiling_ns": int(result["config"]["native_sleep_ceiling_ns"]),
            "maximum_requested_sleep_ns": max(
                int(window["max_requested_sleep_ns"]) for window in windows
            ),
            "directions": sorted({window["direction"] for window in windows}),
            "packet_counts": sorted(
                {int(window["packet_count"]) for window in windows}
            ),
        }
    return {
        "source": s.COMMIT,
        "native_smoke_accepted": True,
        "cells": cells,
        "bytes_checked_per_round": s.BYTES,
        "full_buffer_validation": True,
        "explicit_teardown_completed": True,
        "performance_comparison": None,
        "scope": "Two native diagnostic success paths on one gfx942 GPU; no performance comparison, fault-path coverage, formal refinement, or HIP/HSA parity claim.",
    }


if __name__ == "__main__":
    assert (ARCHIVE / "raw/native.exit").read_text() == "0\n"
    print(json.dumps(report(parse((ARCHIVE / "raw/native.log").read_text())), indent=2))
