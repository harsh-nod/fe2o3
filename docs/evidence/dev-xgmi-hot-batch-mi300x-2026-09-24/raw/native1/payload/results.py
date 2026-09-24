#!/usr/bin/env python3
"""Strict, trial-bound parsing of MI300X persistent-hot peer batch results."""

from decimal import Decimal
import re


_U64_MAX = (1 << 64) - 1
_METRIC_KEYS = {
    f"{direction}_{metric}"
    for direction in ("forward", "reverse")
    for metric in ("p50_ns", "p95_ns", "p50_GBps")
}
_COMMON = {
    "measurement": "persistent-hot",
    "prime_batches": "1",
    "direction": "forward-then-reverse",
    "canaries": "pass",
    "teardown": "explicit",
}
_NATIVE = {
    **_COMMON,
    "schema": "fe2o3.xgmi-peer-persistent-hot-benchmark.v1",
    "surface": "native-api",
    "mapping_lifetime": "process-persistent-hot",
    "engine_parallelism": "runtime-selected-unknown",
    "timing": "native-enqueue-through-observed-completion",
}
_CONSTANTS = {
    "hip": {
        **_NATIVE,
        "devices": "0,1",
        "peer_access": "enabled",
        "progress": "peer-async-then-stream-synchronize",
    },
    "hsa": {
        **_NATIVE,
        "gpu_indices": "0,1",
        "xnack": "disabled",
        "progress": "peer-async-then-signal-wait-reset",
    },
    "kfd": {
        **_COMMON,
        "schema": "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1",
        "surface": "runtime-facade",
        "target": "gfx942:xnack-",
        "engine_parallelism": "ordered-single-sdma",
        "peer_access": "topology-xgmi",
        "mapping_lifetime": "persistent-no-host-access-between-timed-rounds",
        "doorbells_per_batch": "1",
        "progress": "explicit-exact-roster-aggregate-wait",
        "aggregate_roster": "exact-round-submissions",
        "background_progress": "false",
        "forward_engine": "topology-selected",
        "reverse_engine": "topology-selected",
        "timing": "facade-enqueue-through-aggregate-close",
    },
}


def _canonical_unique_ids(unique_ids: list[str]) -> str:
    if not isinstance(unique_ids, list) or len(unique_ids) != 2:
        raise ValueError("expected exactly two unique IDs")
    parsed = []
    for value in unique_ids:
        if not isinstance(value, str) or re.fullmatch(r"0x[0-9A-Fa-f]{1,16}", value) is None:
            raise ValueError("expected unique IDs must be 0x-prefixed hexadecimal")
        number = int(value, 16)
        if number == 0:
            raise ValueError("expected unique IDs must be nonzero")
        parsed.append(number)
    if parsed[0] == parsed[1]:
        raise ValueError("expected unique IDs must be distinct")
    return f"{parsed[0]:016x},{parsed[1]:016x}"


def _tokens(data: bytes) -> dict[str, str]:
    if not isinstance(data, bytes) or not data or len(data) > 8192:
        raise ValueError("result must be bounded nonempty bytes")
    if not data.endswith(b"\n") or data.count(b"\n") != 1:
        raise ValueError("result must contain exactly one newline-terminated row")
    try:
        row = data[:-1].decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("result must be ASCII") from error
    if not row or row.startswith(" ") or row.endswith(" ") or "  " in row:
        raise ValueError("result must use strict single-space framing")
    fields = {}
    for token in row.split(" "):
        if token.count("=") != 1:
            raise ValueError("result contains a non-field word")
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise ValueError("result contains an empty or duplicate field")
        fields[key] = value
    return fields


def _positive_u64(value: str) -> int:
    if re.fullmatch(r"[1-9][0-9]*", value) is None:
        raise ValueError("timing must be a canonical positive integer")
    number = int(value)
    if number > _U64_MAX:
        raise ValueError("timing exceeds u64")
    return number


def _bandwidth(value: str, p50_ns: int, batch_bytes: int) -> None:
    if re.fullmatch(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", value) is None:
        raise ValueError("bandwidth must be a finite three-place decimal")
    expected = Decimal(batch_bytes) / Decimal(p50_ns)
    if abs(expected - Decimal(value)) > Decimal("0.00050001"):
        raise ValueError("bandwidth does not match batch bytes/p50")


def parse_result(
    raw: bytes, *, backend: str, unique_ids: list[str], copy_bytes: int,
    depth: int, warmups: int, samples: int,
) -> dict[str, str]:
    """Validate against external trial controls; timings describe entire batches."""
    if not isinstance(backend, str) or backend not in _CONSTANTS:
        raise ValueError("unsupported backend")
    for value, minimum in ((copy_bytes, 1), (depth, 1), (warmups, 0), (samples, 1)):
        if type(value) is not int or not minimum <= value <= _U64_MAX:
            raise ValueError("trial controls must be bounded integers")
    if depth not in (1, 16, 32):
        raise ValueError("unqualified matched depth")
    batch_bytes = copy_bytes * depth
    if batch_bytes > _U64_MAX or copy_bytes > _U64_MAX - 64 or warmups + samples >= _U64_MAX:
        raise ValueError("trial arithmetic overflows")
    expected_unique_ids = _canonical_unique_ids(unique_ids)
    constants = {
        **_CONSTANTS[backend], "backend": backend, "bytes": str(copy_bytes),
        "depth": str(depth), "outstanding_depth": str(depth),
        "warmups": str(warmups), "samples": str(samples),
    }
    if backend == "kfd":
        constants.update(queue_depth=str(depth), batch_size=str(depth))
    fields = _tokens(raw)
    expected_keys = set(constants) | _METRIC_KEYS | {"unique_ids"}
    if backend != "kfd":
        expected_keys.add("targets")
    if set(fields) != expected_keys:
        raise ValueError("result field roster does not match the producer")
    for key, expected in constants.items():
        if fields[key] != expected:
            raise ValueError(f"unexpected {key}")
    if fields["unique_ids"] != expected_unique_ids:
        raise ValueError("result unique IDs do not match the expected endpoints")
    if backend == "hip":
        targets = fields["targets"].split(",")
        if len(targets) != 2 or targets[0] != targets[1] or re.fullmatch(
            r"gfx942:(?:sramecc[+-]:)?xnack-", targets[0]
        ) is None:
            raise ValueError("unexpected HIP targets")
    elif backend == "hsa" and fields["targets"] != "gfx942,gfx942":
        raise ValueError("unexpected HSA targets")
    for direction in ("forward", "reverse"):
        p50 = _positive_u64(fields[f"{direction}_p50_ns"])
        p95 = _positive_u64(fields[f"{direction}_p95_ns"])
        if p95 < p50:
            raise ValueError("p95 must not be less than p50")
        _bandwidth(fields[f"{direction}_p50_GBps"], p50, batch_bytes)
    return fields
