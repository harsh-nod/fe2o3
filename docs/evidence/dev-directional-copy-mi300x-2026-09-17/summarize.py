#!/usr/bin/env python3
"""Summarize this closed diagnostic, without applying a parity acceptance policy."""

import hashlib
import json
from pathlib import Path
import statistics
import sys

archive = Path(__file__).resolve().parent
root = archive.parents[2]
helper_path = "benchmarks/runtime_gfx942/check-parity.py"
manifest = (archive / "settled-results/source-files.sha256").read_text().splitlines()
pins = [line.split("  ", 1)[0] for line in manifest if line.endswith("  " + helper_path)]
helper_bytes = (root / helper_path).read_bytes()
assert len(pins) == 1 and hashlib.sha256(helper_bytes).hexdigest() == pins[0], "archived helper identity"
helpers = {"__name__": "diagnostic_parity_helpers", "__file__": str(root / helper_path)}
exec(compile(helper_bytes, str(root / helper_path), "exec"), helpers)
parse = helpers["parse_fields"]
positive = helpers["positive_number"]
name = sys.argv[1] if len(sys.argv) == 2 else "benchmark-settled"
assert (archive / "raw" / f"{name}.exit").read_text().strip() == "0", "closed successful record required"
assert (archive / "raw" / f"{name}.finished").is_file()
boundary = name == "benchmark-boundary"
numa = name == "benchmark-numa"
sizes = ("264239136", "264239137") if boundary else ("268435456",)
expected = [(str(i + 1), backend, size) for i, order in enumerate([
    ("kfd", "hsa", "hip"), ("hsa", "hip", "kfd"), ("hip", "kfd", "hsa")
]) for size in (tuple(reversed(sizes)) if i == 1 else sizes) for backend in order]
context_expected = {"diagnostic": "shared-host-window-boundary" if boundary else "shared-host-numa-local-copy" if numa else "shared-host-directional-copy",
                    "git_commit": "ed5b5d64bf95116c21e9bf350c30132ff2bbb524", "gpu": "1",
                    "unique_id": "0xab83d2ffef0d3cdf", "depth": "1", "warmups": "3",
                    "samples": "10", "repetitions": "3", "optional_context_journal": "disabled"}
context_expected["byte_sizes" if boundary else "bytes"] = ",".join(sizes)
if numa:
    context_expected["placement"] = "cpu0-47-memory0"
events, rows, current, final, contexts = [], {}, None, [], []
for number, line in enumerate((archive / "raw" / f"{name}.log").read_text().splitlines(), 1):
    kind = line.split(" ", 1)[0]
    if kind not in {"context", "phase", "completed", "postflight", "finished"} and not line.startswith("backend="):
        continue
    fields = parse(line, number)
    if kind == "context":
        contexts.append(fields)
    elif kind == "phase":
        current = (fields["repetition"], fields["backend"], fields["bytes"] if boundary else sizes[0])
        events.append(("phase", current))
    elif line.startswith("backend="):
        assert current and current[1] == fields["backend"] and current not in rows
        for key, value in {"schema": "fe2o3.async-copy-benchmark.v1", "unique_id": "ab83d2ffef0d3cdf",
                           "bytes": current[2], "depth": "1", "warmups": "3", "samples": "10"}.items():
            assert fields[key] == value, (key, fields)
        if current[1] == "kfd":
            packets = (int(current[2]) + 4194271) // 4194272
            windows = (packets + 62) // 63
            for key, value in {"profile": "directional", "queue_depth": "1", "batch_size": "1",
                               "direction": "h2d-then-d2h", "concurrency": "1", "configured_queues": "1",
                               "doorbells_per_batch": str(windows), "packets_per_transfer": str(packets),
                               "windows_per_transfer": str(windows), "max_packets_per_window": "63",
                               "h2d_engine_index": "1", "d2h_engine_index": "0",
                               "completion": "facade-wait-and-explicit-continuation-flush", "teardown": "explicit"}.items():
                assert fields[key] == value, (key, fields)
        else:
            assert fields["xnack"] == "disabled"
            assert fields["target"] == ("gfx942" if current[1] == "hsa" else "gfx942:sramecc+:xnack-")
            assert fields["gpu_index" if current[1] == "hsa" else "device_index"] == "0"
        for direction in ("h2d", "d2h"):
            assert positive(fields, f"{direction}_p95_ns") >= positive(fields, f"{direction}_p50_ns")
            positive(fields, f"{direction}_p50_GBps")
        rows[current] = fields
        events.append(("row", current))
    elif kind in {"completed", "postflight"}:
        assert fields["exit"] == "0"
        key = (fields["repetition"], fields["backend"], fields["bytes"] if boundary else sizes[0])
        assert key == current
        events.append((kind, key))
    else:
        final.append(fields)
assert events == [(kind, key) for key in expected for kind in ("phase", "row", "completed", "postflight")]
assert contexts == [context_expected]
assert final == [{"exit": "0", "source_after_exit": "0", "binaries_after_exit": "0", "clean_exit": "0", "occupancy_exit": "0"}]
ratios = {}
for size in sizes:
    for baseline in ("hsa", "hip"):
        for direction in ("h2d", "d2h"):
            metric = f"{direction}_p50_ns"
            values = [positive(rows[(str(i), "kfd", size)], metric) / positive(rows[(str(i), baseline, size)], metric) for i in (1, 2, 3)]
            ratios[f"bytes_{size}_kfd_over_{baseline}_{metric}"] = {
                "per_repetition": [float(value) for value in values],
                "median": float(statistics.median(values)),
            }
deltas = {}
if boundary:
    for backend in ("kfd", "hsa", "hip"):
        for direction in ("h2d", "d2h"):
            metric = f"{direction}_p50_ns"
            values = [int(rows[(str(i), backend, sizes[1])][metric]) - int(rows[(str(i), backend, sizes[0])][metric]) for i in (1, 2, 3)]
            deltas[f"{backend}_{metric}_B_minus_A"] = {"per_repetition": values, "median": statistics.median(values)}
print(json.dumps({"scope": "shared-host diagnostic; not parity acceptance", "helper_sha256": pins[0], "rows": [
    {"repetition": key[0], **rows[key]} for key in expected
], "paired_latency_ratios": ratios, "boundary_deltas_ns": deltas}, indent=2))
