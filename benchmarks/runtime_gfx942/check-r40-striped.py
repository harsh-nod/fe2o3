#!/usr/bin/env python3
"""Fail-closed validator for the R40 matched striped-copy evidence set."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import importlib.util
import pathlib
import re
import sys
from decimal import Decimal, InvalidOperation
from typing import Iterable


ROW_SCHEMA = "fe2o3.async-copy-striped-benchmark.v2"
EVIDENCE_SCHEMA = "fe2o3.r40-striped-evidence.v1"
MANIFEST_SCHEMA = "fe2o3.r40-striped-evidence-manifest.v1"
BACKEND_ORDERS = {
    0: ("kfd", "hsa", "hip"),
    1: ("hsa", "hip", "kfd"),
    2: ("hip", "kfd", "hsa"),
}
WORKLOADS = tuple(
    (bytes_count, queue_count, "standalone" if queue_count == 16 else "combined")
    for bytes_count in (4096, 1048576)
    for queue_count in (2, 4, 8, 14, 16)
)
WORKLOAD_IDS = tuple(
    f"bytes{bytes_count}-q{queue_count}-{profile}"
    for bytes_count, queue_count, profile in WORKLOADS
)
WORKLOAD_ORDERS = {
    0: WORKLOAD_IDS,
    1: tuple(reversed(WORKLOAD_IDS)),
    2: WORKLOAD_IDS[5:] + WORKLOAD_IDS[:5],
}
DIRECTIONS = ("h2d", "d2h")
COMPONENTS = ("submit", "wait", "e2e")
SHA256 = re.compile(r"[0-9a-f]{64}")
UNIQUE_ID = re.compile(r"[0-9a-f]{16}")
GIT_COMMIT = re.compile(r"[0-9a-f]{40}")
PHASE_ID = re.compile(
    r"bytes(?:4096|1048576)-q(?:2|4|8|14|16)-(?:combined|standalone)\.(?:kfd|hsa|hip)"
)
MAX_NS = (1 << 128) - 1
EXPECTED_UNIQUE_ID = "0xd2e26fef80cf5c33"
FIXED_ROW = {
    "schema": ROW_SCHEMA,
    "depth": "112",
    "assignment": "rotating-round-robin-v1",
    "submit_order": "rotating-queue-major-v1",
    "direction": "h2d-then-d2h",
    "warmups": "10",
    "samples": "30",
    "validation": "full-buffer-every-round",
    "queue_creation_timed": "no",
    "allocation_timed": "no",
}
BASE_ROW_FIELDS = frozenset(
    {
        "backend",
        "schema",
        "workload_id",
        "unique_id",
        "bytes",
        "depth",
        "logical_queue_count",
        "per_queue_depth",
        "assignment",
        "submit_order",
        "direction",
        "warmups",
        "samples",
        "validation",
        "queue_creation_timed",
        "allocation_timed",
        "api",
        "resource_profile",
        "physical_engine_count",
    }
)
METRIC_FIELDS = frozenset(
    field
    for direction in DIRECTIONS
    for field in (
        *(f"{direction}_{component}_samples_ns" for component in COMPONENTS),
        *(
            f"{direction}_{component}_{percentile}_ns"
            for component in COMPONENTS
            for percentile in ("p50", "p95")
        ),
        f"{direction}_e2e_p50_GBps",
    )
)
KFD_FIELDS = frozenset(
    {
        "directional_queue_count",
        "striped_queue_count",
        "queue_id_sha256",
        "engine_placement_sha256",
        "directional_smoke",
        "aggregate_poll_smoke",
        "destroy",
    }
)
CONTEXT_FIELDS = frozenset(
    {
        "schema",
        "git_commit",
        "target",
        "gpu_index",
        "unique_id",
        "uuid",
        "depth",
        "warmups",
        "samples",
        "bytes_set",
        "logical_queue_counts",
        "profiles",
        "max_busy_percent",
        "phase_timeout_seconds",
        "rocm_version",
        "rustc",
        "cargo",
        "hipcc",
        "cxx",
        "kfd_binary_sha256",
        "hsa_binary_sha256",
        "hip_binary_sha256",
        "hsa_source_sha256",
        "hip_source_sha256",
        "common_header_sha256",
        "checker_sha256",
        "runner_sha256",
        "host_guard_sha256",
        "system_identity_collector_sha256",
        "build_environment",
        "execution_environment",
        "telemetry_command",
        "placement",
        "interference_monitor",
        "monitor_interval_us",
        "monitor_maximum_gap_us",
        "topology_sha256",
        "counterbalance_design",
        "counterbalance_slots",
        "counterbalance_slot",
        "counterbalance_set_id",
        "backend_order",
        "workload_order",
        "claim_scope",
    }
)


class CheckError(Exception):
    pass


def load_r26_checker() -> object:
    path = pathlib.Path(__file__).with_name("check-parity.py")
    spec = importlib.util.spec_from_file_location("fe2o3_r26_checker", path)
    if spec is None or spec.loader is None:
        raise CheckError("could not load the retained R26 guard validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def parse_fields(line: str, line_number: int) -> dict[str, str]:
    fields: dict[str, str] = {}
    tokens = line.split()
    first_field = (
        1
        if tokens
        and tokens[0] in {"context", "phase", "topology", "telemetry", "monitor"}
        else 0
    )
    if any("=" not in token for token in tokens[first_field:]):
        raise CheckError(f"line {line_number}: malformed evidence token")
    for token in tokens[first_field:]:
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise CheckError(
                f"line {line_number}: malformed or duplicate field {key!r}"
            )
        fields[key] = value
    return fields


def canonical_positive_integer(value: str, description: str) -> int:
    if re.fullmatch(r"[1-9][0-9]*", value) is None:
        raise CheckError(f"{description} must be a canonical positive integer")
    if len(value) > len(str(MAX_NS)) or (
        len(value) == len(str(MAX_NS)) and value > str(MAX_NS)
    ):
        raise CheckError(f"{description} exceeds the u128 evidence bound")
    return int(value)


def canonical_nonnegative_integer(value: str, description: str) -> int:
    if re.fullmatch(r"0|[1-9][0-9]*", value) is None:
        raise CheckError(f"{description} must be a canonical nonnegative integer")
    return int(value)


def samples(row: dict[str, str], field: str) -> list[int]:
    try:
        encoded = row[field]
    except KeyError as error:
        raise CheckError(f"row is missing {field}") from error
    values = encoded.split(",")
    if len(values) != 30:
        raise CheckError(f"{field} must contain exactly 30 raw samples")
    return [canonical_positive_integer(value, field) for value in values]


def percentile(values: list[int], numerator: int, denominator: int) -> int:
    ordered = sorted(values)
    rank = (len(ordered) * numerator + denominator - 1) // denominator
    return ordered[rank - 1]


def validate_row(row: dict[str, str]) -> dict[str, dict[str, list[int]]]:
    backend = row.get("backend", "?")
    if backend not in {"kfd", "hsa", "hip"}:
        raise CheckError(f"unexpected backend {backend!r}")
    expected_fields = BASE_ROW_FIELDS | METRIC_FIELDS
    if backend == "kfd":
        expected_fields |= KFD_FIELDS
    difference = set(row) ^ expected_fields
    if difference:
        raise CheckError(f"backend {backend} has invalid field {sorted(difference)[0]}")
    for field, expected in FIXED_ROW.items():
        if row[field] != expected:
            raise CheckError(f"backend {backend} has invalid {field}")
    if UNIQUE_ID.fullmatch(row["unique_id"]) is None or (
        row["unique_id"] != EXPECTED_UNIQUE_ID.removeprefix("0x")
    ):
        raise CheckError(f"backend {backend} has invalid unique_id")
    workload_id = row["workload_id"]
    if workload_id not in WORKLOAD_IDS:
        raise CheckError(f"backend {backend} has unadmitted workload_id")
    bytes_count, queue_count, kind = WORKLOADS[WORKLOAD_IDS.index(workload_id)]
    expected_shape = {
        "bytes": str(bytes_count),
        "logical_queue_count": str(queue_count),
        "per_queue_depth": str(112 // queue_count),
    }
    for field, expected in expected_shape.items():
        if row[field] != expected:
            raise CheckError(f"backend {backend} has inconsistent {field}")
    expected_backend = {
        "hip": {
            "api": "hip",
            "resource_profile": f"nonblocking-streams-q{queue_count}",
            "physical_engine_count": "n/a",
        },
        "hsa": {
            "api": "hsa-amd-memory-async-copy",
            "resource_profile": f"logical-dependency-width-q{queue_count}",
            "physical_engine_count": "not-observed",
        },
        "kfd": {
            "api": "native-kfd-sdma",
            "resource_profile": (
                "striped16"
                if kind == "standalone"
                else f"combined-striped{queue_count}"
            ),
            "physical_engine_count": "2",
        },
    }[backend]
    for field, expected in expected_backend.items():
        if row[field] != expected:
            raise CheckError(f"backend {backend} has invalid {field}")
    if backend == "kfd":
        expected_kfd = {
            "directional_queue_count": "0" if kind == "standalone" else "2",
            "striped_queue_count": str(queue_count),
            "directional_smoke": "pass",
            "aggregate_poll_smoke": "pass",
            "destroy": "pass",
        }
        for field, expected in expected_kfd.items():
            if row[field] != expected:
                raise CheckError(f"backend kfd has invalid {field}")
        for field in ("queue_id_sha256", "engine_placement_sha256"):
            if SHA256.fullmatch(row[field]) is None:
                raise CheckError(f"backend kfd has invalid {field}")

    raw: dict[str, dict[str, list[int]]] = {}
    transfer_bytes = bytes_count * 112
    for direction in DIRECTIONS:
        raw[direction] = {}
        for component in COMPONENTS:
            stem = f"{direction}_{component}"
            values = samples(row, f"{stem}_samples_ns")
            raw[direction][component] = values
            expected_p50 = percentile(values, 1, 2)
            expected_p95 = percentile(values, 19, 20)
            if canonical_positive_integer(row[f"{stem}_p50_ns"], stem) != expected_p50:
                raise CheckError(f"{stem}_p50_ns is inconsistent with raw samples")
            if canonical_positive_integer(row[f"{stem}_p95_ns"], stem) != expected_p95:
                raise CheckError(f"{stem}_p95_ns is inconsistent with raw samples")
        for submit_ns, wait_ns, e2e_ns in zip(
            raw[direction]["submit"],
            raw[direction]["wait"],
            raw[direction]["e2e"],
            strict=True,
        ):
            if submit_ns + wait_ns != e2e_ns:
                raise CheckError(f"{direction} e2e sample is not submit plus wait")
        try:
            observed_gbps = Decimal(row[f"{direction}_e2e_p50_GBps"])
        except (InvalidOperation, KeyError) as error:
            raise CheckError(f"{direction} throughput is not numeric") from error
        expected_gbps = Decimal(transfer_bytes) / Decimal(
            percentile(raw[direction]["e2e"], 1, 2)
        )
        if (
            not observed_gbps.is_finite()
            or observed_gbps <= 0
            or abs(observed_gbps - expected_gbps) > Decimal("0.0000000005")
        ):
            raise CheckError(f"{direction} throughput is inconsistent with E2E p50")
    return raw


def validate_performance(
    rows: dict[tuple[int, str, str], dict[str, str]], *, require_parity: bool = False
) -> tuple[list[str], bool]:
    output: list[str] = []
    ten_x = True
    bounded_parity = True
    violations: list[str] = []
    for workload_id in WORKLOAD_IDS:
        for direction in DIRECTIONS:
            kfd_values = [
                int(rows[slot, workload_id, "kfd"][f"{direction}_e2e_p50_ns"])
                for slot in BACKEND_ORDERS
            ]
            for reference in ("hsa", "hip"):
                reference_values = [
                    int(rows[slot, workload_id, reference][f"{direction}_e2e_p50_ns"])
                    for slot in BACKEND_ORDERS
                ]
                slot_ratios = [
                    Decimal(kfd) / Decimal(ref)
                    for kfd, ref in zip(kfd_values, reference_values, strict=True)
                ]
                median_ratio = sorted(slot_ratios)[1]
                bandwidth_ratios = [Decimal(1) / ratio for ratio in slot_ratios]
                median_bandwidth_ratio = sorted(bandwidth_ratios)[1]
                cell_violations: list[str] = []
                if median_ratio > Decimal("1.10"):
                    cell_violations.append("median-latency-gt-1.10")
                if any(ratio > Decimal("1.20") for ratio in slot_ratios):
                    cell_violations.append("slot-latency-gt-1.20")
                if median_bandwidth_ratio < Decimal("0.90"):
                    cell_violations.append("median-bandwidth-lt-0.90")
                if cell_violations:
                    bounded_parity = False
                    violations.extend(
                        f"{workload_id}:{direction}:{reference}:{violation}"
                        for violation in cell_violations
                    )
                if any(ratio > Decimal("0.10") for ratio in slot_ratios):
                    ten_x = False
                output.append(
                    f"schema={ROW_SCHEMA} workload_id={workload_id} "
                    f"direction={direction} reference={reference} "
                    f"median_latency_ratio={median_ratio:.6f} "
                    f"median_bandwidth_ratio={median_bandwidth_ratio:.6f} "
                    f"bounded_parity_status={'demonstrated' if not cell_violations else 'not-demonstrated'}"
                )
    output.append(
        f"schema={ROW_SCHEMA} orders_of_magnitude_status="
        f"{'demonstrated' if ten_x else 'not-demonstrated'} "
        "criterion=every-matched-slot-workload-direction-kfd-latency-le-0.10x "
        "claim_scope=single-mi300x-gpu2-striped-copy-only"
    )
    output.append(
        f"schema={ROW_SCHEMA} bounded_parity_status="
        f"{'demonstrated' if bounded_parity else 'not-demonstrated'} "
        f"violations={','.join(violations) if violations else 'none'} "
        "criterion=paired-slot-ratios-median-latency-le-1.10-"
        "median-bandwidth-ge-0.90-every-slot-latency-le-1.20"
    )
    if require_parity and not bounded_parity:
        raise CheckError("bounded parity was required but not demonstrated")
    return output, bounded_parity


def canonical_guard_record(
    prefix: str, fields: dict[str, str], ordered_fields: tuple[str, ...]
) -> str:
    return " ".join(
        (prefix,) + tuple(f"{field}={fields[field]}" for field in ordered_fields)
    )


def validate_telemetry(record: dict[str, str], context: dict[str, str]) -> None:
    edge = record.get("edge")
    if edge not in {"start", "end"}:
        raise CheckError("telemetry has invalid edge")
    exact = {
        "phase",
        "edge",
        "gpu_busy_percent",
        "telemetry_sha256",
        "telemetry_base64",
    }
    if set(record) != exact:
        raise CheckError("telemetry has an invalid field set")
    busy = record["gpu_busy_percent"]
    if not busy.isdigit() or int(busy) > int(context["max_busy_percent"]):
        raise CheckError("telemetry load exceeds the admitted maximum")
    if edge == "end" and record["phase"].endswith(".kfd") and busy != "0":
        raise CheckError("KFD post-phase GPU busy must be zero")
    try:
        retained = base64.b64decode(record["telemetry_base64"], validate=True)
    except (binascii.Error, ValueError) as error:
        raise CheckError("telemetry is not canonical base64") from error
    if (
        not retained
        or hashlib.sha256(retained).hexdigest() != record["telemetry_sha256"]
    ):
        raise CheckError("telemetry digest mismatch")
    selected = b"\n".join(
        line for line in retained.lower().splitlines() if b"gpu[2]" in line
    )
    if (
        not selected
        or b"power" not in selected
        or (b"clock" not in selected and b"sclk" not in selected)
    ):
        raise CheckError("telemetry omits selected-GPU power/clock evidence")
    observed = re.findall(rb"gpu use \(%\):\s*([0-9]+)", selected)
    if len(observed) != 1 or int(observed[0]) != int(busy):
        raise CheckError("telemetry load does not match its retained snapshot")


def validate_slot(
    lines: Iterable[str], r26: object
) -> tuple[
    dict[str, str],
    dict[tuple[int, str, str], dict[str, str]],
    str,
    dict[str, str],
    str,
]:
    materialized = list(lines)
    if not materialized or any(
        not line.endswith("\n") or "\n" in line[:-1] or "\r" in line or "\0" in line
        for line in materialized
    ):
        raise CheckError("slot log must be nonempty LF-terminated UTF-8 line records")
    exact_bytes = "".join(materialized).encode("utf-8")
    stripped = [line[:-1] for line in materialized]
    context = parse_fields(stripped[0], 1)
    if not stripped[0].startswith("context ") or set(context) != CONTEXT_FIELDS:
        raise CheckError("slot log has an invalid evidence context")
    fixed_context = {
        "schema": EVIDENCE_SCHEMA,
        "target": "gfx942:xnack-",
        "gpu_index": "2",
        "unique_id": EXPECTED_UNIQUE_ID,
        "uuid": "GPU-d2e26fef80cf5c33",
        "depth": "112",
        "warmups": "10",
        "samples": "30",
        "bytes_set": "4096,1048576",
        "logical_queue_counts": "2,4,8,14,16",
        "profiles": "combined-striped2,combined-striped4,combined-striped8,combined-striped14,striped16",
        "max_busy_percent": "5",
        "phase_timeout_seconds": "180",
        "build_environment": "env-i-explicit-home-toolchain-path-cargo-incremental-0-private-target-v1",
        "execution_environment": "env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1",
        "telemetry_command": "rocm-smi-showuse-showclocks-showpower",
        "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1",
        "interference_monitor": "selected-kfd-gpu-process-tree-census-v2",
        "monitor_interval_us": "2000",
        "monitor_maximum_gap_us": "10000",
        "counterbalance_design": "cyclic-latin-square-3-backends-workload-forward-reverse-rotate5-v1",
        "counterbalance_slots": "3",
        "claim_scope": "single-mi300x-gpu2-striped-copy-only",
    }
    for field, expected in fixed_context.items():
        if context[field] != expected:
            raise CheckError(f"context has invalid {field}")
    if GIT_COMMIT.fullmatch(context["git_commit"]) is None:
        raise CheckError("context git_commit is not canonical")
    for field in (name for name in CONTEXT_FIELDS if name.endswith("_sha256")):
        if SHA256.fullmatch(context[field]) is None:
            raise CheckError(f"context has invalid {field}")
    if context["counterbalance_slot"] not in {"0", "1", "2"}:
        raise CheckError("context has invalid counterbalance_slot")
    slot = int(context["counterbalance_slot"])
    if context["backend_order"] != ",".join(BACKEND_ORDERS[slot]):
        raise CheckError("context backend order does not match its slot")
    if context["workload_order"] != ",".join(WORKLOAD_ORDERS[slot]):
        raise CheckError("context workload order does not match its slot")
    if SHA256.fullmatch(context["counterbalance_set_id"]) is None:
        raise CheckError("context counterbalance_set_id is not canonical")

    identities: list[tuple[dict[str, str], str]] = []
    index = 1
    for edge in ("start",):
        if index >= len(stripped):
            raise CheckError("slot log omits start system identity")
        line = stripped[index]
        identity = parse_fields(line, index + 1)
        if identity.get("schema") != r26.R26_SYSTEM_IDENTITY_SCHEMA:
            raise CheckError("slot log has an invalid start system identity")
        canonical_identity = " ".join(
            ("context", f"schema={r26.R26_SYSTEM_IDENTITY_SCHEMA}")
            + tuple(
                f"{key}={identity[key]}" for key in sorted(identity.keys() - {"schema"})
            )
        )
        if line != canonical_identity:
            raise CheckError("start system identity is noncanonical")
        r26.r26_validate_system_identity(identity, context)
        if identity["observation_edge"] != edge:
            raise CheckError("system identity edge mismatch")
        identities.append((identity, line))
        index += 1

    rows: dict[tuple[int, str, str], dict[str, str]] = {}
    topology_identity: str | None = None
    phase_sequence = [
        (workload_id, backend)
        for workload_id in WORKLOAD_ORDERS[slot]
        for backend in BACKEND_ORDERS[slot]
    ]
    for sequence, (workload_id, backend) in enumerate(phase_sequence):
        phase_id = f"{workload_id}.{backend}"
        if index >= len(stripped):
            raise CheckError(f"slot log omits phase {phase_id}")
        marker = parse_fields(stripped[index], index + 1)
        expected_marker = {
            "slot": str(slot),
            "sequence": str(sequence),
            "workload_id": workload_id,
            "backend": backend,
            "phase_id": phase_id,
        }
        if not stripped[index].startswith("phase ") or marker != expected_marker:
            raise CheckError(f"slot phase marker mismatch for {phase_id}")
        index += 1
        phase_records: list[tuple[str, dict[str, str], str, int]] = []
        for kind in (
            "topology",
            "telemetry",
            "monitor",
            "telemetry",
            "topology",
            "row",
        ):
            if index >= len(stripped):
                raise CheckError(f"slot phase {phase_id} is truncated")
            line = stripped[index]
            fields = parse_fields(line, index + 1)
            observed_kind = (
                "topology"
                if line.startswith("topology ")
                else "telemetry"
                if line.startswith("telemetry ")
                else "monitor"
                if line.startswith("monitor ")
                else "row"
                if fields.get("schema") == ROW_SCHEMA
                else "unknown"
            )
            if observed_kind != kind:
                raise CheckError(f"slot phase {phase_id} has invalid evidence order")
            phase_records.append((kind, fields, line, index + 1))
            index += 1
        start_topology = phase_records[0]
        start_telemetry = phase_records[1]
        monitor_record = phase_records[2]
        end_telemetry = phase_records[3]
        end_topology = phase_records[4]
        row_record = phase_records[5]
        row = row_record[1]
        if row.get("backend") != backend or row.get("workload_id") != workload_id:
            raise CheckError(f"phase {phase_id} row binding mismatch")
        validate_row(row)
        row_line = row_record[2]
        for expected_edge, record in (
            ("start", start_telemetry),
            ("end", end_telemetry),
        ):
            if (
                record[1].get("phase") != phase_id
                or record[1].get("edge") != expected_edge
            ):
                raise CheckError(f"phase {phase_id} telemetry binding mismatch")
            validate_telemetry(record[1], context)
        for expected_edge, record in (("start", start_topology), ("end", end_topology)):
            topology, line, line_number = record[1], record[2], record[3]
            if set(topology) != r26.R26_EXACT_TOPOLOGY_FIELDS:
                raise CheckError(f"line {line_number}: topology field set mismatch")
            if (
                topology["slot"] != str(slot)
                or topology["phase"] != phase_id
                or topology["edge"] != expected_edge
            ):
                raise CheckError(f"line {line_number}: topology binding mismatch")
            canonical_inner = canonical_guard_record(
                "topology", topology, r26.R26_TOPOLOGY_SEALED_FIELDS
            )
            digest = topology["topology_sha256"]
            if hashlib.sha256((canonical_inner + "\n").encode()).hexdigest() != digest:
                raise CheckError(f"line {line_number}: topology seal mismatch")
            canonical_outer = canonical_guard_record(
                "topology",
                topology,
                ("slot", "phase", "edge")
                + r26.R26_TOPOLOGY_SEALED_FIELDS
                + ("topology_sha256",),
            )
            if line != canonical_outer:
                raise CheckError(f"line {line_number}: topology is noncanonical")
            identity = f"{canonical_inner} topology_sha256={digest}"
            if topology_identity is None:
                topology_identity = identity
            elif topology_identity != identity:
                raise CheckError("host topology changed within the evidence slot")
            if (
                topology["gpu_index"] != "2"
                or topology["unique_id"] != EXPECTED_UNIQUE_ID
                or topology["topology_sha256"] != context["topology_sha256"]
            ):
                raise CheckError("topology does not match the selected GPU context")
            expected_identity = {
                "placement": context["placement"],
                "pci_bdf": identities[0][0]["pci_bdf"],
                "numa_node": identities[0][0]["pci_numa_node"],
                "kfd_node": identities[0][0]["gpu_node_id"],
                "kfd_gpu_id": identities[0][0]["gpu_guid"],
            }
            for field, expected in expected_identity.items():
                if topology[field] != expected:
                    raise CheckError(f"topology does not match {field}")
            local = r26.r26_parse_id_list(
                topology["device_local_cpu_list"], "device_local_cpu_list"
            )
            allowed = r26.r26_parse_id_list(
                topology["allowed_cpu_list"], "allowed_cpu_list"
            )
            allowed_mem = r26.r26_parse_id_list(
                topology["allowed_mem_node_list"], "allowed_mem_node_list"
            )
            measurement = r26.r26_parse_id_list(
                topology["measurement_cpu_list"], "measurement_cpu_list"
            )
            observer = canonical_nonnegative_integer(
                topology["observer_cpu"], "observer CPU"
            )
            numa_node = canonical_nonnegative_integer(
                topology["numa_node"], "NUMA node"
            )
            if measurement - (local & allowed):
                raise CheckError("measurement CPUs are not local and allowed")
            if observer not in allowed or observer in measurement:
                raise CheckError("observer CPU is not disjoint and allowed")
            if numa_node not in allowed_mem:
                raise CheckError("NUMA node is not allowed")
        monitor, monitor_line, monitor_number = (
            monitor_record[1],
            monitor_record[2],
            monitor_record[3],
        )
        if set(monitor) != r26.R26_EXACT_MONITOR_FIELDS:
            raise CheckError(f"line {monitor_number}: monitor field set mismatch")
        if monitor["slot"] != str(slot) or monitor["phase"] != phase_id:
            raise CheckError(f"line {monitor_number}: monitor binding mismatch")
        canonical_inner = canonical_guard_record(
            "monitor", monitor, r26.R26_MONITOR_SEALED_FIELDS
        )
        if (
            hashlib.sha256((canonical_inner + "\n").encode()).hexdigest()
            != monitor["monitor_sha256"]
        ):
            raise CheckError(f"line {monitor_number}: monitor seal mismatch")
        canonical_outer = canonical_guard_record(
            "monitor",
            monitor,
            ("slot", "phase") + r26.R26_MONITOR_SEALED_FIELDS + ("monitor_sha256",),
        )
        if monitor_line != canonical_outer:
            raise CheckError(f"line {monitor_number}: monitor is noncanonical")
        fixed_monitor = {
            "schema": r26.R26_MONITOR_SCHEMA,
            "status": "clean",
            "monitor": context["interference_monitor"],
            "schedule": "absolute-monotonic-raw-deadline-v1",
            "interval_us": "2000",
            "maximum_gap_us": "10000",
            "foreign_selected_queues": "0",
            "terminal_selected_queues": "0",
            "target_exit_code": "0",
            "target_reaped": "1",
            "process_group_absent": "1",
            "target_output_bytes": str(len((row_line + "\n").encode())),
            "target_output_sha256": hashlib.sha256(
                (row_line + "\n").encode()
            ).hexdigest(),
            "kfd_gpu_id": start_topology[1]["kfd_gpu_id"],
            "observer_cpu": start_topology[1]["observer_cpu"],
        }
        for field, expected in fixed_monitor.items():
            if monitor[field] != expected:
                raise CheckError(f"line {monitor_number}: monitor has invalid {field}")
        if (
            canonical_positive_integer(monitor["observations"], "monitor observations")
            < 3
        ):
            raise CheckError("monitor has fewer than three observations")
        canonical_positive_integer(
            monitor["target_selected_queue_observations"],
            "target selected queue observations",
        )
        canonical_positive_integer(monitor["root_pid"], "monitor root PID")
        canonical_positive_integer(monitor["process_group"], "monitor process group")
        if (
            canonical_positive_integer(
                monitor["observed_maximum_gap_us"], "monitor gap"
            )
            > 10000
        ):
            raise CheckError("monitor observation gap exceeds 10ms")
        if monitor["root_pid"] != monitor["process_group"]:
            raise CheckError("monitor target process group is not isolated")
        rows[slot, workload_id, backend] = row

    if index >= len(stripped):
        raise CheckError("slot log omits end system identity")
    end_line = stripped[index]
    end_identity = parse_fields(end_line, index + 1)
    if end_identity.get("schema") != r26.R26_SYSTEM_IDENTITY_SCHEMA:
        raise CheckError("slot log has an invalid end system identity")
    canonical_end_identity = " ".join(
        ("context", f"schema={r26.R26_SYSTEM_IDENTITY_SCHEMA}")
        + tuple(
            f"{key}={end_identity[key]}"
            for key in sorted(end_identity.keys() - {"schema"})
        )
    )
    if end_line != canonical_end_identity:
        raise CheckError("end system identity is noncanonical")
    r26.r26_validate_system_identity(end_identity, context)
    if end_identity["observation_edge"] != "end":
        raise CheckError("system identity end edge mismatch")
    identities.append((end_identity, end_line))
    index += 1
    if index != len(stripped):
        raise CheckError("slot log contains trailing evidence")
    for field in r26.R26_EXACT_SYSTEM_IDENTITY_FIELDS:
        if (
            field not in r26.R26_EDGE_VARIANT_SYSTEM_IDENTITY_FIELDS
            and identities[0][0][field] != identities[1][0][field]
        ):
            raise CheckError(f"start/end system identity mismatch in {field}")
    assert topology_identity is not None
    return (
        context,
        rows,
        hashlib.sha256(exact_bytes).hexdigest(),
        {"start": identities[0][1], "end": identities[1][1]},
        topology_identity,
    )


def check_set(
    logs: Iterable[Iterable[str]], *, require_parity: bool = False
) -> list[str]:
    r26 = load_r26_checker()
    contexts: dict[int, dict[str, str]] = {}
    rows: dict[tuple[int, str, str], dict[str, str]] = {}
    hashes: dict[int, str] = {}
    identities: dict[int, dict[str, str]] = {}
    topologies: dict[int, str] = {}
    for log in logs:
        context, slot_rows, digest, slot_identities, topology = validate_slot(log, r26)
        slot = int(context["counterbalance_slot"])
        if slot in contexts:
            raise CheckError(f"duplicate counterbalance slot {slot}")
        contexts[slot] = context
        rows.update(slot_rows)
        hashes[slot] = digest
        identities[slot] = slot_identities
        topologies[slot] = topology
    if set(contexts) != {0, 1, 2}:
        raise CheckError("evidence set requires exact slots 0, 1, and 2")
    baseline = contexts[0]
    varying = {
        "counterbalance_slot",
        "backend_order",
        "workload_order",
        "topology_sha256",
    }
    for slot, context in contexts.items():
        for field in CONTEXT_FIELDS - varying:
            if context[field] != baseline[field]:
                raise CheckError(f"slot {slot} context mismatch in {field}")
        if identities[slot] != identities[0]:
            raise CheckError(f"slot {slot} system identity mismatch")
        if topologies[slot] != topologies[0]:
            raise CheckError(f"slot {slot} host topology mismatch")
    output, bounded_parity = validate_performance(rows, require_parity=require_parity)
    manifest = (
        f"schema={MANIFEST_SCHEMA} counterbalance_set_id={baseline['counterbalance_set_id']} "
        f"slot_0_sha256={hashes[0]} slot_1_sha256={hashes[1]} "
        f"slot_2_sha256={hashes[2]}\n"
    ).encode("ascii")
    output.append(
        f"schema={ROW_SCHEMA} counterbalance_set_id={baseline['counterbalance_set_id']} "
        f"slot_0_sha256={hashes[0]} slot_1_sha256={hashes[1]} "
        f"slot_2_sha256={hashes[2]} manifest_schema={MANIFEST_SCHEMA} "
        f"manifest_sha256={hashlib.sha256(manifest).hexdigest()} phases=90 "
        "raw_samples_per-phase=30 functional_status=pass "
        f"bounded_parity_status={'demonstrated' if bounded_parity else 'not-demonstrated'} "
        "set_validation_status=pass"
    )
    return output


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs=3, type=pathlib.Path)
    parser.add_argument(
        "--require-parity",
        action="store_true",
        help="reject valid evidence unless its pre-registered bounded parity test passes",
    )
    arguments = parser.parse_args(argv)
    try:
        opened = [
            path.open("r", encoding="utf-8", newline="") for path in arguments.logs
        ]
        try:
            for line in check_set(opened, require_parity=arguments.require_parity):
                print(line)
        finally:
            for stream in opened:
                stream.close()
    except (CheckError, OSError, UnicodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
