#!/usr/bin/env python3
"""Fail-closed comparison of matched fe2o3, HSA, and HIP benchmark rows."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from collections.abc import Iterable
from decimal import Decimal, InvalidOperation


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

SCHEMA_CONTEXT = {
    "fe2o3.async-copy-benchmark.v1": "fe2o3.async-copy-benchmark.v1",
    "fe2o3.async-copy-multi-device-benchmark.v1": "fe2o3.async-copy-benchmark.v1",
    "fe2o3.xgmi-peer-benchmark.v1": "fe2o3.xgmi-peer-benchmark.v1",
}

COMMON_CONTEXT_FIELDS = (
    "git_commit",
    "target",
    "gpu_indices",
    "unique_ids",
    "bytes",
    "depths",
    "warmups",
    "samples",
    "max_busy_percent",
    "phase_timeout_seconds",
    "rocm_version",
    "rustc",
)

SCHEMA_CONTEXT_FIELDS = {
    "fe2o3.async-copy-benchmark.v1": ("kfd_profile", "sdma_manifest_sha256"),
    "fe2o3.async-copy-multi-device-benchmark.v1": (
        "kfd_profile",
        "sdma_manifest_sha256",
    ),
    "fe2o3.xgmi-peer-benchmark.v1": (
        "kfd_surface",
        "timing",
        "setup_validation",
        "measurement",
        "mapping_lifetime",
    ),
}

# Historical schema-V1 async-copy logs predate this explicit field. Those runners fixed the
# multi-device KFD lane to the directional profile, so absence has that one legacy meaning.
LEGACY_KFD_MULTI_PROFILE = "directional"

CANONICAL_UNIQUE_ID = re.compile(r"[0-9a-f]{16}")
CONTEXT_UNIQUE_ID = re.compile(r"0x[0-9a-f]{16}")
CANONICAL_SHA256 = re.compile(r"[0-9a-f]{64}")
CANONICAL_GIT_COMMIT = re.compile(r"[0-9a-f]{40}")


class CheckError(Exception):
    pass


def parse_fields(line: str, line_number: int) -> dict[str, str]:
    fields: dict[str, str] = {}
    for token in line.split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise CheckError(f"line {line_number}: malformed or duplicate field {key!r}")
        fields[key] = value
    return fields


def positive_number(row: dict[str, str], field: str) -> Decimal:
    try:
        value = Decimal(row[field])
    except KeyError as error:
        raise CheckError(
            f"backend {row.get('backend', '?')} is missing required field {field}"
        ) from error
    except InvalidOperation as error:
        raise CheckError(f"field {field} is not numeric: {row[field]!r}") from error
    if not value.is_finite() or value <= 0:
        raise CheckError(f"field {field} must be finite and positive")
    return value


def positive_decimal(value: Decimal | float | str, description: str) -> Decimal:
    try:
        parsed = value if isinstance(value, Decimal) else Decimal(str(value))
    except InvalidOperation as error:
        raise CheckError(f"{description} must be numeric") from error
    if not parsed.is_finite() or parsed <= 0:
        raise CheckError(f"{description} must be finite and positive")
    return parsed


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


def positive_integer(fields: dict[str, str], field: str, *, allow_zero: bool = False) -> int:
    try:
        value = fields[field]
    except KeyError as error:
        raise CheckError(f"missing required field {field}") from error
    if not value.isdigit() or (not allow_zero and value == "0"):
        qualifier = "nonnegative" if allow_zero else "positive"
        raise CheckError(f"field {field} must be a {qualifier} integer")
    return int(value)


def validate_context(context: dict[str, str], schema: str) -> tuple[str, ...]:
    missing = set(COMMON_CONTEXT_FIELDS + SCHEMA_CONTEXT_FIELDS[schema]) - context.keys()
    if missing:
        raise CheckError(f"benchmark context is missing fields: {','.join(sorted(missing))}")
    if CANONICAL_GIT_COMMIT.fullmatch(context["git_commit"]) is None:
        raise CheckError("context git_commit must be a canonical 40-digit commit")
    if context["target"] != "gfx942:xnack-":
        raise CheckError("context target must be exactly gfx942:xnack-")

    gpu_indices = context["gpu_indices"].split(",")
    if (
        len(gpu_indices) != 2
        or len(set(gpu_indices)) != 2
        or any(not index.isdigit() for index in gpu_indices)
    ):
        raise CheckError("context gpu_indices must contain two distinct indices")
    context_ids = context["unique_ids"].split(",")
    if (
        len(context_ids) != 2
        or len(set(context_ids)) != 2
        or any(CONTEXT_UNIQUE_ID.fullmatch(value) is None for value in context_ids)
    ):
        raise CheckError("context unique_ids must contain two distinct canonical IDs")

    positive_integer(context, "bytes")
    positive_integer(context, "warmups")
    positive_integer(context, "samples")
    max_busy = positive_integer(context, "max_busy_percent", allow_zero=True)
    if max_busy > 100:
        raise CheckError("context max_busy_percent must not exceed 100")
    positive_integer(context, "phase_timeout_seconds")
    depths = context["depths"].split(",")
    if not depths or len(set(depths)) != len(depths) or any(
        not depth.isdigit() or depth == "0" for depth in depths
    ):
        raise CheckError("context depths must contain distinct positive integers")
    if schema != "fe2o3.xgmi-peer-benchmark.v1":
        if CANONICAL_SHA256.fullmatch(context["sdma_manifest_sha256"]) is None:
            raise CheckError("context sdma_manifest_sha256 must be canonical")
        if context["kfd_profile"] not in {"generic", "directional", "engine0", "engine1"}:
            raise CheckError("context kfd_profile is unsupported")
        if context.get("kfd_multi_profile", LEGACY_KFD_MULTI_PROFILE) != "directional":
            raise CheckError("context kfd_multi_profile must be directional")
    elif (
        context["kfd_surface"] != "runtime-facade"
        or context["timing"] != "submit-through-observed-completion"
        or context["setup_validation"] != "outside-timing"
        or context["measurement"] != "persistent-hot"
        or context["mapping_lifetime"]
        != "persistent-no-host-access-between-timed-rounds"
    ):
        raise CheckError("XGMI context has an unsupported timing methodology")
    return tuple(value.removeprefix("0x") for value in context_ids)


def validate_xgmi_kfd_measurement(row: dict[str, str], measurement: str) -> None:
    depth = row.get("depth")
    expected = {
        "surface": "runtime-facade",
        "target": "gfx942:xnack-",
        "queue_depth": depth,
        "batch_size": depth,
        "direction": "forward-then-reverse",
        "outstanding_depth": depth,
        "engine_parallelism": "ordered-single-sdma",
        "measurement": measurement,
        "peer_access": "topology-xgmi",
        "mapping_lifetime": (
            "persistent-no-host-access-between-timed-rounds"
            if measurement == "persistent-hot"
            else "host-access-between-rounds"
        ),
        "prime_batches": "1" if measurement == "persistent-hot" else "0",
        "doorbells_per_batch": "1",
        "progress": "explicit-flush-then-wait",
        "background_progress": "false",
        "forward_engine": "topology-selected",
        "reverse_engine": "topology-selected",
        "canaries": "pass",
        "teardown": "explicit",
        "timing": "facade-enqueue-flush-through-observed-completion",
    }
    for field, value in expected.items():
        if value is None or row.get(field) != value:
            raise CheckError(
                f"KFD XGMI {measurement} row has invalid {field} methodology"
            )


def validate_phase_evidence(
    phases: list[dict[str, str]],
    groups: dict[tuple[str, str], dict[str, dict[str, str]]],
    context: dict[str, str],
    schema: str,
) -> None:
    max_busy = int(context["max_busy_percent"])
    device_count = 1 if schema == "fe2o3.async-copy-benchmark.v1" else 2
    expected: set[tuple[str, str, str]] = set()
    for key, group in groups.items():
        for backend in group:
            phase = (
                f"{backend}-multi"
                if schema == "fe2o3.async-copy-multi-device-benchmark.v1"
                else backend
            )
            expected.add((phase, key[1], "start"))
            expected.add((phase, key[1], "end"))
    expected_names = {entry[0] for entry in expected}

    observed: set[tuple[str, str, str]] = set()
    for phase in phases:
        name = phase.get("phase")
        if name not in expected_names:
            continue
        depth = phase.get("depth_per_device" if "-multi" in str(name) else "depth")
        load_fields = [
            (edge, phase.get(f"gpu_busy_{edge}_percent"))
            for edge in ("start", "end")
            if f"gpu_busy_{edge}_percent" in phase
        ]
        if name is None or depth is None or len(load_fields) != 1:
            raise CheckError("malformed phase load context")
        edge, load_text = load_fields[0]
        assert load_text is not None
        loads = load_text.split(",")
        if len(loads) != device_count or any(not load.isdigit() for load in loads):
            raise CheckError("phase load context has an invalid device roster")
        if any(int(load) > max_busy for load in loads):
            raise CheckError("phase load exceeds the context maximum")
        key = (name, depth, edge)
        if key in observed:
            raise CheckError(f"duplicate phase load context for {'/'.join(key)}")
        observed.add(key)

    missing = expected - observed
    unexpected = observed - expected
    if missing:
        raise CheckError(f"missing phase load context: {'/'.join(sorted(missing)[0])}")
    if unexpected:
        raise CheckError(f"unexpected phase load context: {'/'.join(sorted(unexpected)[0])}")


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
    max_latency_ratio: Decimal | float | str,
    min_bandwidth_ratio: Decimal | float | str,
) -> list[str]:
    if schema not in SCHEMA_METRICS:
        raise CheckError(f"unsupported schema: {schema}")
    maximum_latency = positive_decimal(max_latency_ratio, "maximum latency ratio")
    minimum_bandwidth = positive_decimal(min_bandwidth_ratio, "minimum bandwidth ratio")

    groups: dict[tuple[str, str], dict[str, dict[str, str]]] = {}
    xgmi_diagnostics: dict[tuple[str, str], dict[str, str]] = {}
    context: dict[str, str] | None = None
    phases: list[dict[str, str]] = []
    for line_number, line in enumerate(lines, 1):
        stripped = line.strip()
        fields = parse_fields(stripped, line_number)
        if stripped.startswith("context "):
            if "phase" in fields:
                phases.append(fields)
            elif fields.get("schema") == SCHEMA_CONTEXT[schema]:
                if context is not None:
                    raise CheckError(f"line {line_number}: duplicate benchmark context")
                context = fields
            continue
        row = fields if "backend" in fields else None
        if row is None or row.get("schema") != schema:
            continue
        backend = row["backend"]
        if backend not in {"kfd", "hsa", "hip"}:
            raise CheckError(f"line {line_number}: unexpected backend {backend!r}")
        key = comparison_key(row, schema)
        if schema == "fe2o3.xgmi-peer-benchmark.v1" and backend == "kfd":
            measurement = row.get("measurement")
            if measurement == "remap-per-round":
                if key in xgmi_diagnostics:
                    raise CheckError(
                        f"duplicate KFD XGMI diagnostic for bytes={key[0]} depth={key[1]}"
                    )
                xgmi_diagnostics[key] = row
                continue
            if measurement != "persistent-hot":
                raise CheckError("KFD XGMI row has an unsupported measurement")
        group = groups.setdefault(key, {})
        if backend in group:
            raise CheckError(
                f"duplicate {backend} row for bytes={key[0]} depth={key[1]}"
            )
        group[backend] = row

    if not groups:
        raise CheckError(f"no rows found for schema {schema}")
    if context is None:
        raise CheckError(f"missing benchmark context for schema {schema}")
    context_ids = validate_context(context, schema)
    context_depths = set(context["depths"].split(","))
    observed_depths = {key[1] for key in groups}
    if observed_depths != context_depths:
        missing_depths = context_depths - observed_depths
        extra_depths = observed_depths - context_depths
        detail = (
            f"missing={','.join(sorted(missing_depths, key=int)) or '-'} "
            f"extra={','.join(sorted(extra_depths, key=int)) or '-'}"
        )
        raise CheckError(f"benchmark rows do not cover declared depths: {detail}")
    if schema == "fe2o3.xgmi-peer-benchmark.v1" and set(xgmi_diagnostics) != set(groups):
        raise CheckError("XGMI evidence requires one remap diagnostic per persistent-hot row")

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
        if schema == "fe2o3.async-copy-benchmark.v1":
            if kfd.get("profile") != context["kfd_profile"]:
                raise CheckError("KFD copy row profile does not match benchmark context")
        elif schema == "fe2o3.async-copy-multi-device-benchmark.v1":
            if context.get("kfd_multi_profile", LEGACY_KFD_MULTI_PROFILE) != "directional":
                raise CheckError("multi-device KFD copy requires the directional profile")
        else:
            validate_xgmi_kfd_measurement(kfd, "persistent-hot")
            diagnostic = xgmi_diagnostics[key]
            validate_xgmi_kfd_measurement(diagnostic, "remap-per-round")
            if matched_methodology(diagnostic, schema) != kfd_methodology:
                raise CheckError("KFD XGMI diagnostic does not match persistent methodology")
            for metric, _ in SCHEMA_METRICS[schema]:
                positive_number(diagnostic, metric)
        if key[0] != context["bytes"] or key[1] not in context_depths:
            raise CheckError(
                f"bytes={key[0]} depth={key[1]} is absent from the benchmark context"
            )
        for reference_name in ("hsa", "hip"):
            reference_methodology = matched_methodology(group[reference_name], schema)
            if reference_methodology != kfd_methodology:
                fields = ",".join(SCHEMA_MATCH_FIELDS[schema])
                raise CheckError(
                    f"bytes={key[0]} depth={key[1]} has mismatched {fields} "
                    f"between kfd and {reference_name}"
                )
        for backend, row in group.items():
            if row["warmups"] != context["warmups"] or row["samples"] != context["samples"]:
                raise CheckError(f"backend {backend} does not match context statistics")
            row_ids = (
                (row["unique_id"],)
                if schema == "fe2o3.async-copy-benchmark.v1"
                else tuple(row["unique_ids"].split(","))
            )
            expected_ids = context_ids[:1] if len(row_ids) == 1 else context_ids
            if row_ids != expected_ids:
                raise CheckError(f"backend {backend} does not match context device IDs")
        for metric, kind in SCHEMA_METRICS[schema]:
            kfd_value = positive_number(kfd, metric)
            for reference_name in ("hsa", "hip"):
                reference_value = positive_number(group[reference_name], metric)
                if kind == "latency":
                    ratio = kfd_value / reference_value
                    passed = ratio <= maximum_latency
                    limit = maximum_latency
                    relation = "max"
                else:
                    ratio = kfd_value / reference_value
                    passed = ratio >= minimum_bandwidth
                    limit = minimum_bandwidth
                    relation = "min"
                if not ratio.is_finite() or ratio <= 0:
                    raise CheckError(f"metric {metric} produced a non-finite ratio")
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
    validate_phase_evidence(phases, groups, context, schema)
    if failed:
        output.append("parity_status=fail")
        raise CheckError("\n".join(output))
    output.append("parity_status=pass")
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=pathlib.Path)
    parser.add_argument("--schema", required=True, choices=tuple(SCHEMA_METRICS))
    parser.add_argument("--max-latency-ratio", required=True, type=Decimal)
    parser.add_argument("--min-bandwidth-ratio", required=True, type=Decimal)
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
