#!/usr/bin/env python3
"""Summarize this closed diagnostic, without applying a parity acceptance policy."""

import json
from pathlib import Path
import runpy
import statistics
import sys

archive = Path(__file__).resolve().parent
root = archive.parents[2]
helpers = runpy.run_path(str(root / "benchmarks/runtime_gfx942/check-parity.py"))
parse = helpers["parse_fields"]
positive = helpers["positive_number"]
name = sys.argv[1] if len(sys.argv) == 2 else "benchmark-settled"
assert (archive / "raw" / f"{name}.exit").read_text().strip() == "0", "closed successful record required"
assert (archive / "raw" / f"{name}.finished").is_file()
expected = [(str(i + 1), backend) for i, order in enumerate([
    ("kfd", "hsa", "hip"), ("hsa", "hip", "kfd"), ("hip", "kfd", "hsa")
]) for backend in order]
events, rows, current, final = [], {}, None, []
for number, line in enumerate((archive / "raw" / f"{name}.log").read_text().splitlines(), 1):
    kind = line.split(" ", 1)[0]
    if kind not in {"phase", "completed", "postflight", "finished"} and not line.startswith("backend="):
        continue
    fields = parse(line, number)
    if kind == "phase":
        current = (fields["repetition"], fields["backend"])
        events.append(("phase", current))
    elif line.startswith("backend="):
        assert current and current[1] == fields["backend"] and current not in rows
        for key, value in {"schema": "fe2o3.async-copy-benchmark.v1", "unique_id": "ab83d2ffef0d3cdf",
                           "bytes": "268435456", "depth": "1", "warmups": "3", "samples": "10"}.items():
            assert fields[key] == value, (key, fields)
        for direction in ("h2d", "d2h"):
            assert positive(fields, f"{direction}_p95_ns") >= positive(fields, f"{direction}_p50_ns")
            positive(fields, f"{direction}_p50_GBps")
        rows[current] = fields
        events.append(("row", current))
    elif kind in {"completed", "postflight"}:
        assert fields["exit"] == "0"
        key = (fields["repetition"], fields["backend"])
        assert key == current
        events.append((kind, key))
    else:
        final.append(fields)
assert events == [(kind, key) for key in expected for kind in ("phase", "row", "completed", "postflight")]
assert final == [{"exit": "0", "source_after_exit": "0", "binaries_after_exit": "0", "clean_exit": "0", "occupancy_exit": "0"}]
ratios = {}
for baseline in ("hsa", "hip"):
    for direction in ("h2d", "d2h"):
        metric = f"{direction}_p50_ns"
        values = [positive(rows[(str(i), "kfd")], metric) / positive(rows[(str(i), baseline)], metric) for i in (1, 2, 3)]
        ratios[f"kfd_over_{baseline}_{metric}"] = {
            "per_repetition": [float(value) for value in values],
            "median": float(statistics.median(values)),
        }
print(json.dumps({"scope": "shared-host diagnostic; not parity acceptance", "rows": [
    {"repetition": key[0], **rows[key]} for key in expected
], "paired_latency_ratios": ratios}, indent=2))
