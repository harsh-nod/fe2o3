#!/usr/bin/env python3
"""Validate the closed engine diagnostic and summarize paired process medians."""

import hashlib
import json
from pathlib import Path
import statistics
import sys

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
HELPER = ROOT / "benchmarks/runtime_gfx942/check-parity.py"
HELPER_SHA256 = "03f798a9f94b88359a5e9ec309d1b74abb495afdc1a728d82303da465019c60b"
source = HELPER.read_bytes()
assert hashlib.sha256(source).hexdigest() == HELPER_SHA256, "pinned parser source"
helpers = {"__name__": "engine_diagnostic_helpers", "__file__": str(HELPER)}
exec(compile(source, str(HELPER), "exec"), helpers)
parse, positive = helpers["parse_fields"], helpers["positive_number"]
EXPECTED = [(str(i), profile) for i in range(1, 5)
            for profile in (("engine0", "engine1") if i % 2 else ("engine1", "engine0"))]
CONTEXT = {"diagnostic": "single-packet-engine-policy", "git_commit": "04d9f3ca37cd17536901d4d7cab405bf06f54454",
           "gpu": "1", "unique_id": "0xab83d2ffef0d3cdf", "bytes": "4194272", "depth": "1",
           "warmups": "3", "samples": "10", "repetitions": "4", "cpu_affinity": "0-47", "memory_node": "0"}
FINAL = {"exit": "0", "source_after_exit": "0", "binaries_after_exit": "0", "clean_exit": "0", "occupancy_exit": "0"}
ADMITTED = {"gpu": "1", "uid": "0xab83d2ffef0d3cdf", "bdf": "0000:26:00.0"}
METRICS = [f"{direction}_{metric}_ns" for direction in ("h2d", "d2h")
           for metric in ("p50", "submit_p50", "wait_p50")]
METRICS += [f"combined_{direction}_p50_ns" for direction in ("h2d", "d2h")]


def analyze(text):
    events, rows, contexts, final, current = [], {}, [], [], None
    for number, line in enumerate(text.splitlines(), 1):
        kind = line.split(" ", 1)[0]
        if kind not in {"context", "phase", "completed", "postflight", "finished", "admitted"} and not line.startswith("backend="):
            continue
        fields = parse(line, number)
        if kind == "context":
            contexts.append(fields)
        elif kind == "phase":
            assert set(fields) == {"repetition", "profile"}
            current = (fields["repetition"], fields["profile"])
            events.append((kind, current))
        elif kind == "admitted":
            assert fields == ADMITTED
            events.append((kind, current))
        elif kind in {"completed", "postflight"}:
            assert fields == {"repetition": current[0], "profile": current[1], "exit": "0"}
            events.append((kind, current))
        elif kind == "finished":
            final.append(fields)
        else:
            assert current in EXPECTED and current not in rows
            expected = {"backend": "kfd", "schema": "fe2o3.async-copy-benchmark.v1",
                        "unique_id": "ab83d2ffef0d3cdf", "profile": current[1], "bytes": "4194272",
                        "depth": "1", "queue_depth": "1", "batch_size": "1", "direction": "h2d-then-d2h",
                        "concurrency": "1", "configured_queues": "1", "doorbells_per_batch": "1",
                        "warmups": "3", "samples": "10", "h2d_engine_index": current[1][-1],
                        "d2h_engine_index": current[1][-1]}
            assert all(fields[key] == value for key, value in expected.items())
            for metric in METRICS:
                positive(fields, metric)
            for direction in ("h2d", "d2h"):
                assert positive(fields, f"{direction}_p95_ns") >= positive(fields, f"{direction}_p50_ns")
                positive(fields, f"{direction}_p50_GBps")
                positive(fields, f"combined_{direction}_p50_GBps")
            rows[current] = fields
            events.append(("row", current))
    expected_events = [(kind, key) for key in EXPECTED
                       for kind in ("phase", "admitted", "row", "completed", "admitted", "postflight")]
    assert events == expected_events + [("admitted", EXPECTED[-1])]
    assert contexts == [CONTEXT] and final == [FINAL]
    ratios = {}
    for metric in METRICS:
        values = [positive(rows[(str(i), "engine1")], metric) / positive(rows[(str(i), "engine0")], metric)
                  for i in range(1, 5)]
        ratios[metric] = {"per_repetition": [float(value) for value in values], "median": float(statistics.median(values))}
    return {"scope": "lower-level single-packet shared-host diagnostic; not facade or parity acceptance",
            "helper_sha256": HELPER_SHA256, "rows": [{"repetition": key[0], **rows[key]} for key in EXPECTED],
            "engine1_over_engine0_paired_latency_ratios": ratios}


def self_test(text):
    analyze(text)
    row = next(line for line in text.splitlines() if line.startswith("backend="))
    phase = "phase repetition=1 profile=engine0"
    adverse = [text.replace(row, "", 1), text + "\n" + row, text.replace(phase, "", 1),
               text.replace(phase, phase.replace("repetition=1", "repetition=2"), 1),
               text.replace("postflight repetition=1 profile=engine0 exit=0", "postflight repetition=1 profile=engine0 exit=1", 1),
               text.replace("finished exit=0", "finished exit=1", 1),
               text.replace("source_after_exit=0", "source_after_exit=1", 1),
               text.replace("admitted gpu=1", "admitted gpu=0", 1),
               text.replace("admitted gpu=1 uid=0xab83d2ffef0d3cdf bdf=0000:26:00.0", "", 1),
               text.replace("bytes=4194272", "bytes=4194273", 1),
               text.replace(row, row.replace("h2d_engine_index=0", "h2d_engine_index=1"), 1),
               text.replace(row, row + " profile=engine0", 1)]
    for changed in adverse:
        assert changed != text
        try:
            analyze(changed)
        except (AssertionError, ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError("adverse diagnostic accepted")
    print(f"PASS: engine summary self-test (1 positive, {len(adverse)} rejected adverse records)")


if __name__ == "__main__":
    assert (ARCHIVE / "raw/benchmark.exit").read_text().strip() == "0", "closed successful record required"
    assert (ARCHIVE / "raw/benchmark.finished").is_file()
    text = (ARCHIVE / "raw/benchmark.log").read_text()
    if sys.argv[1:] == ["--self-test"]:
        self_test(text)
    else:
        assert not sys.argv[1:]
        print(json.dumps(analyze(text), indent=2))
