#!/usr/bin/env python3
"""Fail-closed comparison of matched fe2o3, HSA, and HIP benchmark rows."""

from __future__ import annotations

import argparse
import math
import pathlib
import re
import sys
from collections.abc import Iterable


SCHEMA_METRICS = {
    "fe2o3.async-copy-benchmark.v1": (
        ("h2d_p50_ns", "latency"),
        ("h2d_p95_ns", "latency"),
        ("h2d_p50_GBps", "bandwidth"),
        ("d2h_p50_ns", "latency"),
        ("d2h_p95_ns", "latency"),
        ("d2h_p50_GBps", "bandwidth"),
    ),
    "fe2o3.async-copy-multi-device-benchmark.v1": (
        ("h2d_p50_ns", "latency"),
        ("h2d_p95_ns", "latency"),
        ("h2d_aggregate_p50_GBps", "bandwidth"),
        ("d2h_p50_ns", "latency"),
        ("d2h_p95_ns", "latency"),
        ("d2h_aggregate_p50_GBps", "bandwidth"),
    ),
    "fe2o3.xgmi-peer-benchmark.v1": (
        ("forward_p50_ns", "latency"),
        ("forward_p95_ns", "latency"),
        ("forward_p50_GBps", "bandwidth"),
        ("reverse_p50_ns", "latency"),
        ("reverse_p95_ns", "latency"),
        ("reverse_p50_GBps", "bandwidth"),
    ),
}

SCHEMA_MATCH_FIELDS = {
    "fe2o3.async-copy-benchmark.v1": ("unique_id", "warmups", "samples"),
    "fe2o3.async-copy-multi-device-benchmark.v1": (
        "devices",
        "unique_ids",
        "warmups",
        "samples",
    ),
    "fe2o3.xgmi-peer-benchmark.v1": ("unique_ids", "warmups", "samples"),
}

CANONICAL_UNIQUE_ID = re.compile(r"[0-9a-f]{16}")


class CheckError(Exception):
    pass


def parse_row(line: str, line_number: int) -> dict[str, str] | None:
    fields: dict[str, str] = {}
    for token in line.split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise CheckError(f"line {line_number}: malformed or duplicate field {key!r}")
        fields[key] = value
    if "backend" not in fields:
        return None
    return fields


def positive_number(row: dict[str, str], field: str) -> float:
    try:
        value = float(row[field])
    except KeyError as error:
        raise CheckError(
            f"backend {row.get('backend', '?')} is missing required field {field}"
        ) from error
    except ValueError as error:
        raise CheckError(f"field {field} is not numeric: {row[field]!r}") from error
    if not math.isfinite(value) or value <= 0:
        raise CheckError(f"field {field} must be finite and positive")
    return value


def matched_methodology(row: dict[str, str], schema: str) -> tuple[str, ...]:
    backend = row.get("backend", "?")
    try:
        values = tuple(row[field] for field in SCHEMA_MATCH_FIELDS[schema])
    except KeyError as error:
        raise CheckError(
            f"backend {backend} is missing required match field {error.args[0]}"
        ) from error

    integer_fields = ("warmups", "samples")
    if "devices" in SCHEMA_MATCH_FIELDS[schema]:
        integer_fields += ("devices",)
    for field in integer_fields:
        value = row[field]
        if not value.isdigit() or value == "0":
            raise CheckError(f"field {field} must be a positive integer")

    if (
        "unique_id" in row
        and CANONICAL_UNIQUE_ID.fullmatch(row["unique_id"]) is None
    ):
        raise CheckError("field unique_id must be exactly 16 lowercase hexadecimal digits")
    if "unique_ids" in row:
        unique_ids = row["unique_ids"].split(",")
        if (
            len(unique_ids) != 2
            or len(set(unique_ids)) != 2
            or any(CANONICAL_UNIQUE_ID.fullmatch(value) is None for value in unique_ids)
        ):
            raise CheckError(
                "field unique_ids must contain two distinct canonical unique IDs"
            )
        if "devices" in row and int(row["devices"]) != len(unique_ids):
            raise CheckError("field devices must equal the number of unique_ids")
    return values


def comparison_key(row: dict[str, str], schema: str) -> tuple[str, str]:
    try:
        byte_count = row["bytes"]
        depth = row["depth_per_device" if "multi-device" in schema else "depth"]
    except KeyError as error:
        raise CheckError(
            f"backend {row.get('backend', '?')} lacks the matched size/depth coordinates"
        ) from error
    if not byte_count.isdigit() or not depth.isdigit() or byte_count == "0" or depth == "0":
        raise CheckError("matched size/depth coordinates must be positive integers")
    return byte_count, depth


def check_rows(
    lines: Iterable[str],
    schema: str,
    max_latency_ratio: float,
    min_bandwidth_ratio: float,
) -> list[str]:
    if schema not in SCHEMA_METRICS:
        raise CheckError(f"unsupported schema: {schema}")
    if not math.isfinite(max_latency_ratio) or max_latency_ratio <= 0:
        raise CheckError("maximum latency ratio must be finite and positive")
    if not math.isfinite(min_bandwidth_ratio) or min_bandwidth_ratio <= 0:
        raise CheckError("minimum bandwidth ratio must be finite and positive")

    groups: dict[tuple[str, str], dict[str, dict[str, str]]] = {}
    for line_number, line in enumerate(lines, 1):
        row = parse_row(line.strip(), line_number)
        if row is None or row.get("schema") != schema:
            continue
        backend = row["backend"]
        if backend not in {"kfd", "hsa", "hip"}:
            raise CheckError(f"line {line_number}: unexpected backend {backend!r}")
        key = comparison_key(row, schema)
        group = groups.setdefault(key, {})
        if backend in group:
            raise CheckError(
                f"duplicate {backend} row for bytes={key[0]} depth={key[1]}"
            )
        group[backend] = row

    if not groups:
        raise CheckError(f"no rows found for schema {schema}")

    output: list[str] = []
    failed = False
    for key in sorted(groups, key=lambda value: (int(value[0]), int(value[1]))):
        group = groups[key]
        missing = {"kfd", "hsa", "hip"} - group.keys()
        if missing:
            raise CheckError(
                f"bytes={key[0]} depth={key[1]} missing backends: {','.join(sorted(missing))}"
            )
        kfd = group["kfd"]
        kfd_methodology = matched_methodology(kfd, schema)
        for reference_name in ("hsa", "hip"):
            reference_methodology = matched_methodology(group[reference_name], schema)
            if reference_methodology != kfd_methodology:
                fields = ",".join(SCHEMA_MATCH_FIELDS[schema])
                raise CheckError(
                    f"bytes={key[0]} depth={key[1]} has mismatched {fields} "
                    f"between kfd and {reference_name}"
                )
        for metric, kind in SCHEMA_METRICS[schema]:
            kfd_value = positive_number(kfd, metric)
            for reference_name in ("hsa", "hip"):
                reference_value = positive_number(group[reference_name], metric)
                if kind == "latency":
                    ratio = kfd_value / reference_value
                    passed = ratio <= max_latency_ratio
                    limit = max_latency_ratio
                    relation = "max"
                else:
                    ratio = kfd_value / reference_value
                    passed = ratio >= min_bandwidth_ratio
                    limit = min_bandwidth_ratio
                    relation = "min"
                failed |= not passed
                output.append(
                    " ".join(
                        (
                            f"schema={schema}",
                            f"bytes={key[0]}",
                            f"depth={key[1]}",
                            f"reference={reference_name}",
                            f"metric={metric}",
                            f"ratio={ratio:.6f}",
                            f"{relation}_ratio={limit:.6f}",
                            f"status={'pass' if passed else 'fail'}",
                        )
                    )
                )
    if failed:
        output.append("parity_status=fail")
        raise CheckError("\n".join(output))
    output.append("parity_status=pass")
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=pathlib.Path)
    parser.add_argument("--schema", required=True, choices=tuple(SCHEMA_METRICS))
    parser.add_argument("--max-latency-ratio", required=True, type=float)
    parser.add_argument("--min-bandwidth-ratio", required=True, type=float)
    arguments = parser.parse_args()
    try:
        with arguments.input.open(encoding="utf-8") as input_file:
            output = check_rows(
                input_file,
                arguments.schema,
                arguments.max_latency_ratio,
                arguments.min_bandwidth_ratio,
            )
    except (CheckError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    print("\n".join(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
