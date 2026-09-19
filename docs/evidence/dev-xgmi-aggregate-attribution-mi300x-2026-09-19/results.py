#!/usr/bin/env python3
"""Strict parser for depth-one KFD aggregate host-attribution trials."""

from decimal import Decimal, InvalidOperation
import re


_BYTES = 1_048_576
_RECORDS = 82
_MAX_LINE_BYTES = 8_192
_MAX_TRANSCRIPT_BYTES = (_RECORDS + 1) * _MAX_LINE_BYTES
_U64_MAX = (1 << 64) - 1
_UID_INPUT = re.compile(r"0x[0-9A-Fa-f]{1,16}", re.ASCII)
_UID_OUTPUT = re.compile(r"[0-9a-f]{16}", re.ASCII)
_DECIMAL = re.compile(r"0|[1-9][0-9]*", re.ASCII)
_POSITIVE_DECIMAL = re.compile(r"[1-9][0-9]*", re.ASCII)
_THREE_PLACE_DECIMAL = re.compile(r"(?:0|[1-9][0-9]*)\.[0-9]{3}", re.ASCII)

_SUMMARY_CONSTANTS = {
    "backend": "kfd",
    "schema": "fe2o3.xgmi-peer-aggregate-hot-only-benchmark.v1",
    "surface": "runtime-facade",
    "target": "gfx942:xnack-",
    "bytes": str(_BYTES),
    "depth": "1",
    "queue_depth": "1",
    "batch_size": "1",
    "direction": "forward-then-reverse",
    "outstanding_depth": "1",
    "engine_parallelism": "ordered-single-sdma",
    "warmups": "10",
    "samples": "30",
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
}
_METRICS = {
    "forward_p50_ns",
    "forward_p95_ns",
    "forward_p50_GBps",
    "reverse_p50_ns",
    "reverse_p95_ns",
    "reverse_p50_GBps",
}
_PHASES = (
    "admission_validation_ns",
    "preparation_ns",
    "opening_currentness_ns",
    "submission_ns",
    "wait_ns",
    "closing_currentness_ns",
    "settlement_ns",
)
_DIAGNOSTIC_FIXED = {
    "schema": "fe2o3.xgmi-aggregate-host-attribution.v1",
    "backend": "kfd",
    "authority": "none",
    "teardown": "explicit",
    "timing": "backend-aggregate-progress-host-only",
}
_DIAGNOSTIC_KEYS = {
    *_DIAGNOSTIC_FIXED,
    "ordinal",
    "backend_submission",
    "source_uid",
    "destination_uid",
    *_PHASES,
    "total_ns",
}


def _uids(unique_ids: list[str]) -> tuple[str, str]:
    if type(unique_ids) is not list or len(unique_ids) != 2:
        raise ValueError("expected exactly two unique IDs")
    parsed = []
    for value in unique_ids:
        if type(value) is not str or _UID_INPUT.fullmatch(value) is None:
            raise ValueError("unique IDs must be 0x-prefixed hexadecimal")
        number = int(value, 16)
        if number == 0:
            raise ValueError("unique IDs must be nonzero")
        parsed.append(number)
    if parsed[0] == parsed[1]:
        raise ValueError("unique IDs must be distinct")
    return f"{parsed[0]:016x}", f"{parsed[1]:016x}"


def _tokens(line: bytes) -> dict[str, str]:
    if not line or len(line) > _MAX_LINE_BYTES:
        raise ValueError("result row must be bounded and nonempty")
    try:
        text = line.decode("ascii")
    except UnicodeDecodeError as error:
        raise ValueError("result row must be ASCII") from error
    if not text or text.startswith(" ") or text.endswith(" ") or "  " in text:
        raise ValueError("result row must use strict single-space framing")
    fields = {}
    for token in text.split(" "):
        if token.count("=") != 1:
            raise ValueError("result row contains a non-field word")
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise ValueError("result row contains an empty or duplicate field")
        fields[key] = value
    return fields


def _u64(value: str, *, positive: bool = False) -> int:
    pattern = _POSITIVE_DECIMAL if positive else _DECIMAL
    if pattern.fullmatch(value) is None:
        raise ValueError("duration must be a canonical u64")
    number = int(value)
    if number > _U64_MAX:
        raise ValueError("duration exceeds u64")
    return number


def _bandwidth(value: str, p50_ns: int) -> None:
    if _THREE_PLACE_DECIMAL.fullmatch(value) is None:
        raise ValueError("bandwidth must be a finite three-place decimal")
    try:
        observed = Decimal(value)
    except InvalidOperation as error:
        raise ValueError("bandwidth is not numeric") from error
    if not observed.is_finite():
        raise ValueError("bandwidth must be finite")
    expected = Decimal(_BYTES) / Decimal(p50_ns)
    if abs(expected - observed) > Decimal("0.00050001"):
        raise ValueError("bandwidth does not agree with bytes/p50")


def _summary(line: bytes, mode: str, uids: tuple[str, str]) -> dict[str, str]:
    fields = _tokens(line)
    constants = dict(_SUMMARY_CONSTANTS)
    constants["unique_ids"] = ",".join(uids)
    if mode == "on":
        constants["diagnostic"] = "aggregate-host-attribution"
    if set(fields) != set(constants) | _METRICS:
        raise ValueError("summary field roster does not match the producer")
    if any(fields[key] != value for key, value in constants.items()):
        raise ValueError("summary controls or identity changed")
    for direction in ("forward", "reverse"):
        p50 = _u64(fields[direction + "_p50_ns"], positive=True)
        p95 = _u64(fields[direction + "_p95_ns"], positive=True)
        if p95 < p50:
            raise ValueError("p95 must not be less than p50")
        _bandwidth(fields[direction + "_p50_GBps"], p50)
    return fields


def _population(ordinal: int) -> tuple[str, int, str]:
    direction = "forward" if ordinal % 2 == 0 else "reverse"
    if ordinal < 2:
        return "prime", 0, direction
    if ordinal < 22:
        return "warmup", (ordinal - 2) // 2, direction
    return "sample", (ordinal - 22) // 2, direction


def _diagnostic(
    line: bytes, ordinal: int, uids: tuple[str, str]
) -> dict[str, object]:
    fields = _tokens(line)
    if set(fields) != _DIAGNOSTIC_KEYS:
        raise ValueError("diagnostic field roster does not match the producer")
    if any(fields[key] != value for key, value in _DIAGNOSTIC_FIXED.items()):
        raise ValueError("diagnostic constants changed")
    if fields["ordinal"] != str(ordinal):
        raise ValueError("diagnostic ordinal is not canonical")
    if fields["backend_submission"] != str(ordinal + 7):
        raise ValueError("diagnostic submission identity is not canonical")
    source, destination = (uids if ordinal % 2 == 0 else tuple(reversed(uids)))
    if (
        _UID_OUTPUT.fullmatch(fields["source_uid"]) is None
        or _UID_OUTPUT.fullmatch(fields["destination_uid"]) is None
        or fields["source_uid"] != source
        or fields["destination_uid"] != destination
    ):
        raise ValueError("diagnostic direction or endpoint identity changed")
    durations = {key: _u64(fields[key]) for key in _PHASES}
    total = _u64(fields["total_ns"])
    stages = 0
    for value in durations.values():
        stages += value
        if stages > _U64_MAX:
            raise ValueError("diagnostic stage sum exceeds u64")
    if stages > total:
        raise ValueError("diagnostic stages exceed total")
    population, round_index, direction = _population(ordinal)
    return {
        "fields": fields,
        "population": population,
        "round": round_index,
        "direction": direction,
        "durations_ns": {**durations, "total_ns": total},
    }


def parse_result(data: bytes, backend: str, unique_ids: list[str]) -> dict[str, object]:
    """Parse one exact off/on KFD transcript without making a speedup claim."""

    if backend not in ("off", "on"):
        raise ValueError("backend must be off or on")
    if (
        type(data) is not bytes
        or not data
        or len(data) > _MAX_TRANSCRIPT_BYTES
        or not data.endswith(b"\n")
        or b"\r" in data
        or b"\0" in data
    ):
        raise ValueError("result must be a complete bounded transcript")
    lines = data[:-1].split(b"\n")
    expected = 1 if backend == "off" else _RECORDS + 1
    if len(lines) != expected or any(not line for line in lines):
        raise ValueError("result transcript has the wrong row count")
    uids = _uids(unique_ids)
    diagnostics = []
    if backend == "on":
        diagnostics = [
            _diagnostic(line, ordinal, uids)
            for ordinal, line in enumerate(lines[:-1])
        ]
    summary = _summary(lines[-1], backend, uids)
    return {"mode": backend, "summary": summary, "records": diagnostics}
