#!/usr/bin/env python3
"""Validate copy-only HIP payloads, independently of native campaign admission."""

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import re
import sys

SCHEMA = "fe2o3.hip-directional-copy-diagnostic.v1"
MAX_PAYLOAD = 4 * 1024 * 1024
MAX_INTERVAL = (1 << 63) - 1


def require(condition, message):
    if not condition:
        raise ValueError(message)


def decimal(value, maximum):
    require(
        re.fullmatch(r"0|[1-9][0-9]*", value) is not None, "canonical decimal required"
    )
    require(len(value) <= 20, "oversized decimal")
    number = int(value)
    require(number <= maximum, "decimal exceeds bound")
    return number


@dataclass(frozen=True)
class Expected:
    device_index: int
    unique_id: int
    target: str
    byte_count: int
    warmups: int
    samples: int

    def __post_init__(self):
        for value, minimum, maximum in (
            (self.device_index, 0, (1 << 31) - 1),
            (self.unique_id, 1, (1 << 64) - 1),
            (self.byte_count, 1, 256 * 1024 * 1024),
            (self.warmups, 0, 9999),
            (self.samples, 1, 10000),
        ):
            require(
                type(value) is int and minimum <= value <= maximum,
                "invalid expected workload",
            )
        require(self.warmups + self.samples <= 10000, "too many rounds")
        require(
            isinstance(self.target, str)
            and re.fullmatch(r"gfx942(?::[a-z][a-z0-9_]*[+-])+", self.target)
            is not None,
            "expected gfx942 feature target",
        )
        features = self.target.split(":")[1:]
        require(
            "xnack-" in features and "xnack+" not in features, "disabled XNACK required"
        )
        require(
            len({feature[:-1] for feature in features}) == len(features),
            "duplicate target feature",
        )


def rows(text):
    require(isinstance(text, str) and text.isascii(), "ASCII payload required")
    require(
        0 < len(text) <= MAX_PAYLOAD and text.endswith("\n"),
        "bounded complete payload required",
    )
    result = []
    for line in text[:-1].split("\n"):
        fields = line.split(" ")
        row = {}
        for field in fields:
            match = re.fullmatch(r"([a-z][a-z0-9_]*)=([A-Za-z0-9_.:+-]+)", field)
            require(match is not None, "malformed field or whitespace")
            key, value = match.groups()
            require(key not in row, "duplicate field")
            row[key] = value
        result.append(row)
        require(len(result) <= 10002, "too many rows")
    return result


def validate(stdout, stderr, exit_code, expected):
    """Return checked rounds only; callers must separately qualify host/identity evidence."""
    require(type(exit_code) is int and exit_code == 0, "native process did not succeed")
    require(stderr == "", "unexpected native stderr")
    require(
        isinstance(expected, Expected), "independent expected configuration required"
    )
    parsed = rows(stdout)
    total = expected.warmups + expected.samples
    require(len(parsed) == total + 2, "incomplete or extra row roster")
    require(
        parsed[0]
        == {
            "schema": SCHEMA,
            "record": "config",
            "device_index": str(expected.device_index),
            "unique_id": f"{expected.unique_id:016x}",
            "target": expected.target,
            "xnack": "disabled",
            "bytes": str(expected.byte_count),
            "depth": "1",
            "warmups": str(expected.warmups),
            "samples": str(expected.samples),
            "host_allocation": "hipHostMallocDefault",
            "stream": "nonblocking",
            "engine": "runtime_selected",
            "allocator_benchmark": "disabled",
        },
        "configuration does not match independently supplied workload",
    )
    intervals = []
    for index, row in enumerate(parsed[1:-1]):
        require(
            set(row)
            == {
                "schema",
                "record",
                "index",
                "phase",
                "pattern",
                "checked_bytes",
                "h2d_total_ns",
                "d2h_total_ns",
            },
            "unexpected round field roster",
        )
        expected_fields = {
            "schema": SCHEMA,
            "record": "round",
            "index": str(index),
            "phase": "warmup" if index < expected.warmups else "sample",
            "pattern": str((index * 67 + 1) % 251 + 1),
            "checked_bytes": str(expected.byte_count),
        }
        require(
            all(row[key] == value for key, value in expected_fields.items()),
            "round identity, pattern or validation mismatch",
        )
        h2d = decimal(row["h2d_total_ns"], MAX_INTERVAL)
        d2h = decimal(row["d2h_total_ns"], MAX_INTERVAL)
        require(h2d > 0 and d2h > 0, "nonpositive interval")
        intervals.append(
            {"index": index, "phase": row["phase"], "h2d_ns": h2d, "d2h_ns": d2h}
        )
    require(
        parsed[-1]
        == {
            "schema": SCHEMA,
            "record": "complete",
            "validated_rounds": str(total),
            "measured_rounds": str(expected.samples),
            "allocations_released": "3",
            "streams_destroyed": "1",
        },
        "incomplete validation or cleanup",
    )
    return {
        "schema": "fe2o3.hip-copy-payload-check.v1",
        "payload_valid": True,
        "validated_rounds": total,
        "measured_rounds": expected.samples,
        "checked_bytes_per_round": expected.byte_count,
        "rounds": intervals,
        "performance_accepted": False,
        "scope": "payload-only; host observations and artifact identity are separate",
    }


def read_bounded(path):
    with path.open("rb") as source:
        value = source.read(MAX_PAYLOAD + 1)
    require(len(value) <= MAX_PAYLOAD, "oversized input file")
    return value.decode("ascii")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stdout", type=Path, required=True)
    parser.add_argument("--stderr", type=Path, required=True)
    parser.add_argument("--exit-code", type=int, required=True)
    parser.add_argument("--device-index", type=int, required=True)
    parser.add_argument("--unique-id", type=lambda value: int(value, 0), required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--bytes", dest="byte_count", type=int, required=True)
    parser.add_argument("--warmups", type=int, required=True)
    parser.add_argument("--samples", type=int, required=True)
    args = parser.parse_args(argv)
    try:
        expected = Expected(
            args.device_index,
            args.unique_id,
            args.target,
            args.byte_count,
            args.warmups,
            args.samples,
        )
        checked = validate(
            read_bounded(args.stdout),
            read_bounded(args.stderr),
            args.exit_code,
            expected,
        )
    except (OSError, ValueError, UnicodeError) as error:
        print(f"HIP payload rejected: {error}", file=sys.stderr)
        return 1
    print(json.dumps(checked, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
