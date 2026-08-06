#!/usr/bin/env python3
"""Bounded parser and comparator for fe2o3 differential result records."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

PROTOCOL = "FE2O3_DIFF_RESULT_V1"
MAX_RESULT_BYTES = 256 * 1024
MAX_CASES = 64
MAX_ELEMENTS = 4096
ABS_TOLERANCE = 1.0e-6
REL_TOLERANCE = 1.0e-6


class ComparisonError(ValueError):
    pass


@dataclass(frozen=True)
class Case:
    kernel: str
    kind: str
    seed: int
    length: int
    left_canary: int
    right_canary: int
    values: tuple[int, ...]
    canonical: str

    @property
    def key(self) -> tuple[str, str, int, int]:
        return (self.kernel, self.kind, self.seed, self.length)


def _parse_hex32(value: str, field: str) -> int:
    if len(value) != 8 or any(char not in "0123456789abcdef" for char in value):
        raise ComparisonError(
            f"{field} must be exactly eight lowercase hexadecimal digits"
        )
    return int(value, 16)


def parse_results(data: bytes) -> dict[tuple[str, str, int, int], Case]:
    if len(data) > MAX_RESULT_BYTES:
        raise ComparisonError(
            f"result stream is {len(data)} bytes; limit is {MAX_RESULT_BYTES}"
        )
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise ComparisonError("result stream is not ASCII") from error

    cases: dict[tuple[str, str, int, int], Case] = {}
    for line_number, raw in enumerate(text.splitlines(), 1):
        if not raw.startswith(PROTOCOL + "\t"):
            continue
        fields = raw.split("\t")
        if len(fields) != 8:
            raise ComparisonError(
                f"line {line_number}: expected eight tab-separated fields"
            )
        _, kernel, kind, seed_hex, length_text, left, right, payload = fields
        if kernel not in {"fill", "vecadd", "affine"}:
            raise ComparisonError(f"line {line_number}: unknown kernel {kernel!r}")
        if kind not in {"i32", "bits32", "f32"}:
            raise ComparisonError(f"line {line_number}: unknown value kind {kind!r}")
        if len(seed_hex) != 16 or any(
            char not in "0123456789abcdef" for char in seed_hex
        ):
            raise ComparisonError(f"line {line_number}: malformed seed")
        seed = int(seed_hex, 16)
        if not length_text.isascii() or not length_text.isdecimal():
            raise ComparisonError(f"line {line_number}: malformed length")
        length = int(length_text)
        if length > MAX_ELEMENTS:
            raise ComparisonError(
                f"line {line_number}: length {length} exceeds {MAX_ELEMENTS}"
            )
        if len(payload) != length * 8:
            raise ComparisonError(
                f"line {line_number}: payload width does not match length {length}"
            )
        values = tuple(
            _parse_hex32(payload[offset : offset + 8], "payload word")
            for offset in range(0, len(payload), 8)
        )
        case = Case(
            kernel=kernel,
            kind=kind,
            seed=seed,
            length=length,
            left_canary=_parse_hex32(left, "left canary"),
            right_canary=_parse_hex32(right, "right canary"),
            values=values,
            canonical=raw,
        )
        if case.key in cases:
            raise ComparisonError(f"line {line_number}: duplicate case {case.key!r}")
        cases[case.key] = case
        if len(cases) > MAX_CASES:
            raise ComparisonError(f"case count exceeds {MAX_CASES}")
    if not cases:
        raise ComparisonError("result stream contains no differential records")
    return cases


def _f32(bits: int) -> float:
    return struct.unpack("!f", bits.to_bytes(4, "big"))[0]


def _compare_f32(reference: int, actual: int, index: int) -> None:
    expected = _f32(reference)
    observed = _f32(actual)
    if math.isnan(expected) or math.isnan(observed):
        if math.isnan(expected) and math.isnan(observed):
            return
        raise ComparisonError(
            f"element {index}: NaN policy requires both values to be NaN"
        )
    if math.isinf(expected) or math.isinf(observed):
        if expected == observed:
            return
        raise ComparisonError(
            f"element {index}: infinity sign/value mismatch: {expected} != {observed}"
        )
    tolerance = ABS_TOLERANCE + REL_TOLERANCE * abs(expected)
    if abs(expected - observed) > tolerance:
        raise ComparisonError(
            f"element {index}: {observed} differs from {expected} by more than {tolerance}"
        )


def compare_cases(reference_data: bytes, actual_data: bytes) -> list[dict[str, object]]:
    reference = parse_results(reference_data)
    actual = parse_results(actual_data)
    if set(reference) != set(actual):
        missing = sorted(set(reference) - set(actual))
        unexpected = sorted(set(actual) - set(reference))
        raise ComparisonError(
            f"case identity mismatch; missing={missing!r}; unexpected={unexpected!r}"
        )

    summaries: list[dict[str, object]] = []
    for key in sorted(reference):
        expected = reference[key]
        observed = actual[key]
        if (
            expected.left_canary != observed.left_canary
            or expected.right_canary != observed.right_canary
        ):
            raise ComparisonError(
                f"case {key!r}: canary corruption: "
                f"expected {expected.left_canary:08x}/{expected.right_canary:08x}, "
                f"observed {observed.left_canary:08x}/{observed.right_canary:08x}"
            )
        if expected.kind in {"i32", "bits32"}:
            if expected.values != observed.values:
                mismatch = next(
                    index
                    for index, pair in enumerate(zip(expected.values, observed.values))
                    if pair[0] != pair[1]
                )
                raise ComparisonError(
                    f"case {key!r}: exact {expected.kind} mismatch at element {mismatch}: "
                    f"{observed.values[mismatch]:08x} != {expected.values[mismatch]:08x}"
                )
            policy = f"exact-{expected.kind}-bits"
        else:
            for index, (reference_bits, actual_bits) in enumerate(
                zip(expected.values, observed.values)
            ):
                _compare_f32(reference_bits, actual_bits, index)
            policy = (
                f"f32-abs={ABS_TOLERANCE:g};rel={REL_TOLERANCE:g};"
                "nan=both-nan;infinity=exact-sign"
            )
        summaries.append(
            {
                "actual_sha256": hashlib.sha256(
                    observed.canonical.encode("ascii")
                ).hexdigest(),
                "kernel": expected.kernel,
                "length": expected.length,
                "policy": policy,
                "reference_sha256": hashlib.sha256(
                    expected.canonical.encode("ascii")
                ).hexdigest(),
                "seed": f"{expected.seed:016x}",
                "status": "PASS",
            }
        )
    return summaries


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("reference", type=Path)
    parser.add_argument("actual", type=Path)
    args = parser.parse_args(argv)
    try:
        summaries = compare_cases(args.reference.read_bytes(), args.actual.read_bytes())
    except (OSError, ComparisonError) as error:
        print(f"differential comparison failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summaries, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
