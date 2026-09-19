#!/usr/bin/env python3
"""Strict parser for the matched depth-one XGMI persistent-hot results."""

from decimal import Decimal, InvalidOperation
import re


_BYTES = 1_048_576
_MAX_RECORD_BYTES = 8_192
_U64_MAX = (1 << 64) - 1
_EXPECTED_ORDINALS = "0,1"
_EXPECTED_WORKLOAD = {
    "bytes": str(_BYTES),
    "depth": "1",
    "warmups": "10",
    "samples": "30",
}
_METRIC_KEYS = {
    "forward_p50_ns",
    "forward_p95_ns",
    "forward_p50_GBps",
    "reverse_p50_ns",
    "reverse_p95_ns",
    "reverse_p50_GBps",
}
_POSITIVE_DECIMAL = re.compile(r"[1-9][0-9]*", re.ASCII)
_THREE_PLACE_DECIMAL = re.compile(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", re.ASCII)
_EXPECTED_UID = re.compile(r"0x[0-9A-Fa-f]{1,16}", re.ASCII)
_OUTPUT_UID_PAIR = re.compile(r"[0-9a-f]{16},[0-9a-f]{16}", re.ASCII)
_HIP_TARGET = re.compile(r"gfx942:(?:sramecc[+-]:)?xnack-", re.ASCII)


_COMMON_NATIVE = {
    "schema": "fe2o3.xgmi-peer-persistent-hot-benchmark.v1",
    "surface": "native-api",
    "measurement": "persistent-hot",
    "mapping_lifetime": "process-persistent-hot",
    "prime_batches": "1",
    "direction": "forward-then-reverse",
    "outstanding_depth": "1",
    "engine_parallelism": "runtime-selected-unknown",
    "timing": "native-enqueue-through-observed-completion",
    "canaries": "pass",
    "teardown": "explicit",
}

_CONSTANTS = {
    "hip": {
        **_COMMON_NATIVE,
        **_EXPECTED_WORKLOAD,
        "backend": "hip",
        "surface": "native-api",
        "devices": _EXPECTED_ORDINALS,
        "peer_access": "enabled",
        "progress": "peer-async-then-stream-synchronize",
    },
    "hsa": {
        **_COMMON_NATIVE,
        **_EXPECTED_WORKLOAD,
        "backend": "hsa",
        "surface": "native-api",
        "gpu_indices": _EXPECTED_ORDINALS,
        "xnack": "disabled",
        "progress": "peer-async-then-signal-wait-reset",
    },
    "kfd": {
        **_EXPECTED_WORKLOAD,
        "backend": "kfd",
        "schema": "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1",
        "surface": "runtime-facade",
        "target": "gfx942:xnack-",
        "queue_depth": "1",
        "batch_size": "1",
        "direction": "forward-then-reverse",
        "outstanding_depth": "1",
        "engine_parallelism": "ordered-single-sdma",
        "measurement": "persistent-hot",
        "peer_access": "topology-xgmi",
        "mapping_lifetime": "persistent-no-host-access-between-timed-rounds",
        "prime_batches": "1",
        "doorbells_per_batch": "1",
        "progress": "explicit-exact-roster-aggregate-wait",
        "aggregate_roster": "exact-round-submissions",
        "background_progress": "false",
        "forward_engine": "topology-selected",
        "reverse_engine": "topology-selected",
        "canaries": "pass",
        "teardown": "explicit",
        "timing": "facade-enqueue-through-aggregate-close",
    },
}

_BACKEND_FIELDS = {
    "hip": {"devices", "unique_ids", "targets", "peer_access"},
    "hsa": {"gpu_indices", "unique_ids", "targets", "xnack"},
    "kfd": {"unique_ids", "target"},
}


def _canonical_unique_ids(unique_ids: list[str]) -> str:
    if not isinstance(unique_ids, list) or len(unique_ids) != 2:
        raise ValueError("expected exactly two unique IDs")
    parsed = []
    for value in unique_ids:
        if not isinstance(value, str) or _EXPECTED_UID.fullmatch(value) is None:
            raise ValueError("expected unique IDs must be 0x-prefixed hexadecimal")
        number = int(value, 16)
        if number == 0:
            raise ValueError("expected unique IDs must be nonzero")
        parsed.append(number)
    if parsed[0] == parsed[1]:
        raise ValueError("expected unique IDs must be distinct")
    return f"{parsed[0]:016x},{parsed[1]:016x}"


def _tokens(data: bytes) -> dict[str, str]:
    if not isinstance(data, bytes) or not data or len(data) > _MAX_RECORD_BYTES:
        raise ValueError("result must be bounded nonempty bytes")
    if not data.endswith(b"\n") or data.count(b"\n") != 1:
        raise ValueError("result must contain exactly one newline-terminated row")
    try:
        row = data[:-1].decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("result must be ASCII") from error
    if not row or row.startswith(" ") or row.endswith(" ") or "  " in row:
        raise ValueError("result must use strict single-space framing")

    result = {}
    for token in row.split(" "):
        if token.count("=") != 1:
            raise ValueError("result contains a non-field word")
        key, value = token.split("=", 1)
        if not key or not value or key in result:
            raise ValueError("result contains an empty or duplicate field")
        result[key] = value
    return result


def _positive_u64(fields: dict[str, str], key: str) -> int:
    value = fields[key]
    if _POSITIVE_DECIMAL.fullmatch(value) is None:
        raise ValueError(f"{key} must be a canonical positive integer")
    number = int(value)
    if number > _U64_MAX:
        raise ValueError(f"{key} exceeds u64")
    return number


def _bandwidth(fields: dict[str, str], key: str, p50_ns: int) -> None:
    value = fields[key]
    if _THREE_PLACE_DECIMAL.fullmatch(value) is None:
        raise ValueError(f"{key} must be a finite three-place decimal")
    try:
        observed = Decimal(value)
    except InvalidOperation as error:
        raise ValueError(f"{key} is not numeric") from error
    if not observed.is_finite():
        raise ValueError(f"{key} must be finite")
    expected = Decimal(_BYTES) / Decimal(p50_ns)
    if abs(expected - observed) > Decimal("0.00050001"):
        raise ValueError(f"{key} does not match bytes/p50")


def parse_result(data: bytes, backend: str, unique_ids: list[str]) -> dict[str, str]:
    """Parse and validate one producer's predeclared persistent-hot result row."""

    if backend not in _CONSTANTS:
        raise ValueError("unsupported backend")
    expected_unique_ids = _canonical_unique_ids(unique_ids)
    fields = _tokens(data)
    constants = _CONSTANTS[backend]
    expected_keys = set(constants) | _BACKEND_FIELDS[backend] | _METRIC_KEYS
    if set(fields) != expected_keys:
        raise ValueError("result field roster does not match the producer")
    for key, expected in constants.items():
        if fields[key] != expected:
            raise ValueError(f"unexpected {key}")

    if _OUTPUT_UID_PAIR.fullmatch(fields["unique_ids"]) is None:
        raise ValueError("result unique IDs are not canonical")
    if fields["unique_ids"] != expected_unique_ids:
        raise ValueError("result unique IDs do not match the expected endpoints")

    if backend == "hip":
        targets = fields["targets"].split(",")
        if (
            len(targets) != 2
            or targets[0] != targets[1]
            or _HIP_TARGET.fullmatch(targets[0]) is None
        ):
            raise ValueError("unexpected HIP targets")
    elif backend == "hsa" and fields["targets"] != "gfx942,gfx942":
        raise ValueError("unexpected HSA targets")

    forward_p50 = _positive_u64(fields, "forward_p50_ns")
    forward_p95 = _positive_u64(fields, "forward_p95_ns")
    reverse_p50 = _positive_u64(fields, "reverse_p50_ns")
    reverse_p95 = _positive_u64(fields, "reverse_p95_ns")
    if forward_p95 < forward_p50 or reverse_p95 < reverse_p50:
        raise ValueError("p95 must not be less than p50")
    _bandwidth(fields, "forward_p50_GBps", forward_p50)
    _bandwidth(fields, "reverse_p50_GBps", reverse_p50)
    return fields
