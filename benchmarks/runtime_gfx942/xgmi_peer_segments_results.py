#!/usr/bin/env python3
"""Validate complete ordered-list receipts; summarize sample host latency only."""

import math
import re
import statistics

SCHEMA = "fe2o3.xgmi-ordered-segments.v1"
PROGRESS = {
    "kfd": "single-ticket-sequence-full-currentness",
    "hip": "enqueue-list-stream-query",
    "hsa": "enqueue-predecessor-chain-signal-load",
}
DEADLINE_NS = 60_000_000_000


def geometry(useful_bytes, descriptor_count, warmups, samples):
    values = (useful_bytes, descriptor_count, warmups, samples)
    if any(type(value) is not int for value in values):
        raise ValueError("integer workload controls required")
    if not (1 <= descriptor_count <= min(useful_bytes, 4096)
            and useful_bytes <= 2 * 1024 * 1024
            and warmups >= 0 and samples > 0 and warmups + samples <= 64):
        raise ValueError("unbounded workload controls")
    slot_bytes = ((useful_bytes // descriptor_count + 18 + 63) // 64) * 64
    band_bytes = ((64 + descriptor_count * slot_bytes + 4095) // 4096) * 4096
    bands = 1 + warmups + samples
    if bands * band_bytes > 256 * 1024 * 1024:
        raise ValueError("destination budget")
    return band_bytes, bands


def _fields(line):
    result = {}
    for item in line.split(" "):
        if not re.fullmatch(r"[a-z_]+=[a-zA-Z0-9.,_-]+", item):
            raise ValueError("noncanonical key/value record")
        key, value = item.split("=", 1)
        if key in result:
            raise ValueError("duplicate field")
        result[key] = value
    return result


def parse_receipt(raw, *, backend, unique_ids, useful_bytes, descriptor_count, warmups, samples):
    """Controls and IDs must come from the admitted trial, not its stdout."""
    band_bytes, bands = geometry(useful_bytes, descriptor_count, warmups, samples)
    if backend not in PROGRESS or len(unique_ids) != 2:
        raise ValueError("backend or identity roster")
    if (any(type(uid) is not int or not 0 < uid < 2**64 for uid in unique_ids)
            or unique_ids[0] == unique_ids[1]):
        raise ValueError("distinct physical identities required")
    if type(raw) is not bytes or not raw or len(raw) > 200_000 or not raw.endswith(b"\n"):
        raise ValueError("bounded newline-terminated receipt required")
    try:
        lines = raw.decode("ascii").split("\n")[:-1]
    except UnicodeDecodeError as error:
        raise ValueError("ASCII receipt required") from error
    if len(lines) != 2 * bands + 1:
        raise ValueError("incomplete or extra list roster")
    expected = {
        "schema": SCHEMA, "record": "complete", "backend": backend,
        "unique_ids": ",".join(f"{uid:016x}" for uid in unique_ids),
        "useful_bytes": str(useful_bytes), "descriptor_count": str(descriptor_count),
        "logical_depth": "1", "warmups": str(warmups), "samples": str(samples),
        "prime_lists": "1", "band_bytes": str(band_bytes), "source_bytes": str(band_bytes),
        "destination_bytes": str(bands * band_bytes), "layout": "reversed-ragged-slots-v1",
        "progress": PROGRESS[backend], "deadline_ns": str(DEADLINE_NS),
        "timing": "list-admission-through-observed-completion",
        "mapping_lifetime": "retained-pair-no-allocation-host-readwrite-between-lists",
        "completion_cleanup": "outside-timing", "correctness": "passed", "teardown": "explicit",
    }
    if _fields(lines[-1]) != expected:
        raise ValueError("complete record does not match admitted workload")
    durations = [[], []]
    sample_times = [[], []]
    for index, line in enumerate(lines[:-1]):
        band, direction = divmod(index, 2)
        population = "prime" if band == 0 else "warmup" if band <= warmups else "sample"
        row = _fields(line)
        elapsed = row.pop("elapsed_ns", "")
        expected_row = {
            "schema": SCHEMA, "record": "list", "backend": backend,
            "band": str(band), "direction": str(direction), "population": population,
        }
        if row != expected_row or not re.fullmatch(r"[1-9][0-9]{0,10}", elapsed):
            raise ValueError("list roster or duration mismatch")
        elapsed = int(elapsed)
        if elapsed >= DEADLINE_NS:
            raise ValueError("whole-list deadline exceeded")
        durations[direction].append(elapsed)
        if population == "sample":
            sample_times[direction].append(elapsed)
    result = []
    for direction, times in enumerate(sample_times):
        median = statistics.median(times)
        p95 = sorted(times)[math.ceil(0.95 * len(times)) - 1]
        result.append({
            "direction": direction, "samples": len(times), "sample_ns": times,
            "p50_ns_median": median, "p95_ns_nearest_rank": p95,
            "effective_useful_GBps_at_p50": useful_bytes / median,
        })
    return {"backend": backend, "all_list_ns_by_direction": durations, "directions": result}
