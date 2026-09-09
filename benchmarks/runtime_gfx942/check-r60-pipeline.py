#!/usr/bin/env python3
"""Validate three matched R60 batch logs and report workload-scoped ratios."""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import re
import sys
from typing import Any

SCHEMA = "fe2o3.r60-pipeline-benchmark.v1"
HSACO_SHA256 = "3a25e364dd1e1931d1a16c24b37aa998df2c6ef1cbcf0ec2afb6372cbc878bab"
OUTPUT_SHA256 = "79fd0768604fe9de0ced87297f7d653343e998926b59e2cce7df5e38194c52b3"
MAX_LOG_BYTES = 65536
TIMEOUT_NS = 10_000_000_000
BACKENDS = ("kfd", "hip", "hsa")
METRICS = ("issue_batch_ns", "tail_wait_batch_ns", "total_batch_ns")
FIXED_CONFIG = {
    "record": "config",
    "schema": SCHEMA,
    "target": "gfx942:xnack-",
    "hsaco_sha256": HSACO_SHA256,
    "workload": "trusted-gfx942-vecadd-v1",
    "elements": 1048576,
    "bytes_per_buffer": 4194304,
    "grid": [1048576, 1, 1],
    "workgroup": [256, 1, 1],
    "access": ["read", "read", "write"],
    "memory": "host-visible-coherent",
    "launches_per_batch": 64,
    "warmups": 10,
    "samples": 30,
    "ordering": "same-stream-ordered",
    "host_waits_during_issue": 0,
    "reset_timed": False,
    "explicit_allocation_api_timed": False,
    "validation": "byte-exact-every-batch",
    "clock": "steady-monotonic-ns",
    "wait_timeout_ns": TIMEOUT_NS,
}
CONFIG_FIELDS = frozenset(FIXED_CONFIG) | {
    "backend", "run_id", "source_commit", "unique_id", "issue_api_calls_per_batch"
}
BATCH_FIELDS = frozenset({
    "record", "phase", "index", "issued_launches", "completed_launches",
    "output_sha256", *METRICS,
})


class CheckError(ValueError):
    pass


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            raise CheckError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def reject_constant(value: str) -> None:
    raise CheckError(f"nonfinite JSON number {value}")


def exact(actual: Any, expected: Any) -> bool:
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            exact(left, right) for left, right in zip(actual, expected)
        )
    return actual == expected


def require_fields(record: dict[str, Any], fields: frozenset[str], where: str) -> None:
    if frozenset(record) != fields:
        raise CheckError(f"{where}: missing or unexpected fields")


def checked_integer(value: Any, low: int, high: int, where: str) -> int:
    if type(value) is not int or not low <= value <= high:
        raise CheckError(f"{where}: expected integer in [{low}, {high}]")
    return value


def validate_records(records: list[dict[str, Any]]) -> dict[str, Any]:
    if len(records) != 42:
        raise CheckError("expected config, 10 warmups, 30 samples, and complete")
    if any(type(record) is not dict for record in records):
        raise CheckError("every record must be a JSON object")
    config = records[0]
    require_fields(config, CONFIG_FIELDS, "config")
    for key, expected in FIXED_CONFIG.items():
        if not exact(config[key], expected):
            raise CheckError(f"config: unexpected {key}")
    if type(config["backend"]) is not str or config["backend"] not in BACKENDS:
        raise CheckError("config: unexpected backend")
    expected_calls = {"kfd": 128, "hip": 64, "hsa": 192}[config["backend"]]
    checked_integer(config["issue_api_calls_per_batch"], expected_calls, expected_calls,
                    "issue_api_calls_per_batch")
    for key, length in (("run_id", 64), ("source_commit", 40), ("unique_id", 16)):
        value = config[key]
        if (
            type(value) is not str
            or re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is None
            or value == "0" * length
        ):
            raise CheckError(f"config: invalid {key}")
    measured = []
    for ordinal, batch in enumerate(records[1:-1]):
        require_fields(batch, BATCH_FIELDS, f"batch {ordinal}")
        expected_phase = "warmup" if ordinal < 10 else "sample"
        expected_index = ordinal if ordinal < 10 else ordinal - 10
        if batch["record"] != "batch" or batch["phase"] != expected_phase:
            raise CheckError(f"batch {ordinal}: unexpected record or phase")
        if checked_integer(batch["index"], 0, 29, "batch index") != expected_index:
            raise CheckError(f"batch {ordinal}: duplicate, missing, or reordered index")
        for field in ("issued_launches", "completed_launches"):
            checked_integer(batch[field], 64, 64, field)
        for field in METRICS:
            checked_integer(batch[field], 1, TIMEOUT_NS, field)
        if batch["total_batch_ns"] != batch["issue_batch_ns"] + batch["tail_wait_batch_ns"]:
            raise CheckError(f"batch {ordinal}: timing components do not sum to total")
        if batch["output_sha256"] != OUTPUT_SHA256:
            raise CheckError(f"batch {ordinal}: output hash mismatch")
        if expected_phase == "sample":
            measured.append(batch)
    complete = records[-1]
    require_fields(complete, frozenset({"record", "validated_batches"}), "complete")
    if complete["record"] != "complete":
        raise CheckError("missing completion record")
    checked_integer(complete["validated_batches"], 40, 40, "validated_batches")
    return {"config": config, "samples": measured}


def parse_log(text: str) -> dict[str, Any]:
    if len(text.encode("utf-8")) > MAX_LOG_BYTES:
        raise CheckError("log exceeds the bounded evidence size")
    if not text.endswith("\n"):
        raise CheckError("log lacks final newline")
    records = []
    try:
        for line in text.splitlines():
            records.append(json.loads(
                line, object_pairs_hook=unique_object, parse_constant=reject_constant
            ))
    except (json.JSONDecodeError, RecursionError) as error:
        raise CheckError(f"invalid JSONL: {error}") from error
    return validate_records(records)


def load_log(path: pathlib.Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            data = stream.read(MAX_LOG_BYTES + 1)
        if len(data) > MAX_LOG_BYTES:
            raise CheckError("log exceeds the bounded evidence size")
        return parse_log(data.decode("utf-8"))
    except (OSError, UnicodeError) as error:
        raise CheckError(f"cannot read {path}: {error}") from error


def percentile(values: list[int], percent: int) -> int:
    return sorted(values)[math.ceil(len(values) * percent / 100) - 1]


def compare(logs: list[dict[str, Any]]) -> dict[str, Any]:
    if len(logs) != 3:
        raise CheckError("exactly three backend logs are required")
    by_backend = {}
    matched = None
    for log in logs:
        config = log["config"]
        backend = config["backend"]
        if backend in by_backend:
            raise CheckError(f"duplicate backend {backend}")
        by_backend[backend] = log
        current = {key: value for key, value in config.items()
                   if key not in {"backend", "issue_api_calls_per_batch"}}
        if matched is not None and current != matched:
            raise CheckError("backend source, run, device, artifact, or configuration mismatch")
        matched = current
    if set(by_backend) != set(BACKENDS):
        raise CheckError("expected one kfd, one hip, and one hsa log")
    timings = {
        backend: {
            metric: {
                f"p{percent}": percentile(
                    [batch[metric] for batch in by_backend[backend]["samples"]], percent
                )
                for percent in (50, 95)
            }
            for metric in METRICS
        }
        for backend in BACKENDS
    }
    ratios = {
        baseline: {
            metric: {
                key: timings[baseline][metric][key] / timings["kfd"][metric][key]
                for key in ("p50", "p95")
            }
            for metric in METRICS
        }
        for baseline in ("hip", "hsa")
    }
    return {
        "schema": "fe2o3.r60-pipeline-comparison.v1",
        "claim_scope": "one-device-64-ordered-vecadd-host-visible-batch",
        "run_id": matched["run_id"],
        "source_commit": matched["source_commit"],
        "unique_id": matched["unique_id"],
        "hsaco_sha256": HSACO_SHA256,
        "percentile_method": "nearest-rank",
        "issue_api_calls_per_batch": {"kfd": 128, "hip": 64, "hsa": 192},
        "timings_ns": timings,
        "baseline_over_kfd": ratios,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs=3, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        result = compare([load_log(path) for path in args.logs])
    except CheckError as error:
        print(f"R60 evidence rejected: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, allow_nan=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
