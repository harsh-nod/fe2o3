#!/usr/bin/env python3
"""Validate a precommitted matched-backend protocol, not hardware authenticity."""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import sys
from typing import Any

PLAN_SCHEMA = "fe2o3.scale3-matched-plan.v1"
CAPTURE_SCHEMA = "fe2o3.scale3-matched-capture.v1"
BACKENDS = ("kfd", "hsa", "hip")
MAX_LOG_BYTES = 2 * 1024 * 1024
MAX_RECORD_BYTES = 16384
MAX_RECORDS = 1024
MAX_INTEGER = (1 << 63) - 1
MAX_ALLOCATION_BYTES = 1 << 34
PHASES = ("reset", "submission", "transfer", "wait", "validation")
SEMANTICS = {
    "completion": "all-operations-observed-complete-before-validation",
    "reuse": "one-context-two-streams-module-buffer-set-per-block-reset-every-iteration",
    "stream_topology": "independent-compute-and-directional-copy-streams-no-cross-dependency",
    "host_memory": "pinned-host-visible-coherent-no-pageable-staging",
    "device_memory": "device-local-no-managed-or-peer-fallback",
    "geometry_units": "grid-and-workgroup-dimensions-in-work-items",
    "validation": "independent-oracle-full-requested-buffers-outside-measured-interval",
    "wall_clock": "clock-monotonic-ns-same-host-boot",
    "cpu_clock": "process-cpu-ns-including-runtime-worker-threads",
    "transfer_cost": "host-enqueue-cost-not-device-copy-duration",
    "end_to_end": "first-enqueue-through-final-completion-observation",
    "latency_summary": "nearest-rank-p50-p95-p99-measured-samples-only",
    "operation_throughput": "completed-operations-over-summed-measured-end-to-end-time",
    "copy_throughput": "payload-bytes-over-summed-end-to-end-time-not-device-bandwidth",
    "cpu_summary": "summed-measured-process-cpu-ns-per-completed-operation",
    "process_lifetime": "fresh-isolated-producer-process-per-backend-block",
    "setup_cost": "producer-internal-resource-setup-excludes-process-spawn",
    "oracle_boundary": "hip-hsa-isolated-benchmark-processes-no-production-fallback",
    "failure_policy": "abort-entire-campaign-no-retry-or-sample-replacement",
    "physical_overlap": "unmeasured",
}
PEAK_BASES = {
    "requested_allocation_bytes": "explicit-api-request-bytes",
    "process_peak_rss_bytes": "os-process-high-water",
    "resident_backing_bytes": "exact-native-backing-accounting",
    "native_published_slots": "exact-native-retained-roster",
}


class CheckError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CheckError(message)


def fields(value: Any, expected: set[str], where: str) -> None:
    require(type(value) is dict and set(value) == expected, f"{where}: missing or unexpected fields")


def integer(value: Any, low: int, high: int, where: str) -> int:
    require(type(value) is int and low <= value <= high, f"{where}: invalid bounded integer")
    return value


def digest(value: Any, where: str, length: int = 64) -> str:
    require(type(value) is str and re.fullmatch(rf"[0-9a-f]{{{length}}}", value) is not None
            and value != "0" * length, f"{where}: invalid identity")
    return value


def name(value: Any, where: str) -> None:
    require(type(value) is str and re.fullmatch(r"[A-Za-z0-9_.:+/-]{1,128}", value) is not None,
            f"{where}: invalid bounded name")


def exact(value: Any, expected: Any) -> bool:
    if type(value) is not type(expected):
        return False
    if type(expected) is dict:
        return value.keys() == expected.keys() and all(exact(value[key], expected[key]) for key in expected)
    if type(expected) is list:
        return len(value) == len(expected) and all(exact(left, right) for left, right in zip(value, expected))
    return value == expected


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def reject_constant(value: str) -> None:
    raise CheckError(f"nonfinite JSON number: {value}")


def reject_float(value: str) -> None:
    raise CheckError(f"floating-point JSON is outside the integer-only protocol: {value}")


def encoded(text: str) -> bytes:
    require(type(text) is str and len(text) <= MAX_LOG_BYTES, "capture exceeds byte bound")
    try:
        data = text.encode("utf-8")
    except UnicodeError as error:
        raise CheckError(f"invalid UTF-8: {error}") from error
    require(len(data) <= MAX_LOG_BYTES, "capture exceeds byte bound")
    return data


def parse_records(text: str) -> list[dict[str, Any]]:
    data = encoded(text)
    require(data.endswith(b"\n") and b"\r" not in data, "capture requires LF-terminated JSONL")
    require(1 <= data.count(b"\n") <= MAX_RECORDS, "capture exceeds record bound")
    records = []
    for line in data.splitlines():
        require(0 < len(line) <= MAX_RECORD_BYTES, "record exceeds byte bound")
        try:
            record = json.loads(line.decode("utf-8"), object_pairs_hook=unique_object,
                                parse_constant=reject_constant, parse_float=reject_float)
        except (ValueError, UnicodeError, RecursionError) as error:
            raise CheckError(f"invalid JSON record: {error}") from error
        require(type(record) is dict, "record must be an object")
        records.append(record)
    return records


def load_text(path: pathlib.Path) -> str:
    try:
        with path.open("rb") as stream:
            data = stream.read(MAX_LOG_BYTES + 1)
        require(len(data) <= MAX_LOG_BYTES, "capture exceeds byte bound")
        return data.decode("utf-8")
    except (OSError, UnicodeError) as error:
        raise CheckError(f"cannot read {path}: {error}") from error


def sha256(text: str) -> str:
    return hashlib.sha256(encoded(text)).hexdigest()


def object_sha256(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     allow_nan=False).encode("ascii")).hexdigest()


def validate_plan(text: str, expected_sha256: str) -> dict[str, Any]:
    digest(expected_sha256, "externally pinned plan")
    require(sha256(text) == expected_sha256, "plan differs from externally pinned bytes")
    records = parse_records(text)
    require(len(records) == 1, "plan must contain exactly one record")
    plan = records[0]
    fields(plan, {"schema", "campaign_id", "source_commit", "host_boot_id", "gpu", "workload",
                  "producers", "semantics", "policy", "resource_limits"}, "plan")
    require(plan["schema"] == PLAN_SCHEMA, "unknown protocol schema")
    digest(plan["campaign_id"], "campaign")
    digest(plan["source_commit"], "source", 40)
    require(type(plan["host_boot_id"]) is str and re.fullmatch(
        r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", plan["host_boot_id"]) is not None
        and plan["host_boot_id"] != "00000000-0000-0000-0000-000000000000", "invalid host boot identity")
    gpu = plan["gpu"]
    fields(gpu, {"unique_id", "pci_bdf", "kfd_gpu_id", "target", "topology_sha256", "numa_node", "measurement_cpus"}, "GPU")
    digest(gpu["unique_id"], "GPU unique ID", 16)
    digest(gpu["topology_sha256"], "topology")
    require(type(gpu["pci_bdf"]) is str and re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-1][0-9a-f]\.[0-7]", gpu["pci_bdf"]) is not None,
            "invalid PCI identity")
    require(gpu["target"] == "gfx942:xnack-", "unsupported target")
    integer(gpu["kfd_gpu_id"], 1, (1 << 32) - 1, "KFD GPU ID")
    integer(gpu["numa_node"], 0, 65535, "NUMA node")
    cpus = gpu["measurement_cpus"]
    require(type(cpus) is list and 1 <= len(cpus) <= 256, "invalid CPU roster")
    for cpu in cpus:
        integer(cpu, 0, 65535, "CPU")
    require(cpus == sorted(set(cpus)), "CPU roster must be sorted and unique")
    fields(plan["producers"], set(BACKENDS), "producers")
    for backend in BACKENDS:
        producer = plan["producers"][backend]
        fields(producer, {"binary_sha256", "toolchain_closure_sha256", "runtime_version"}, "producer")
        digest(producer["binary_sha256"], "producer binary")
        digest(producer["toolchain_closure_sha256"], "producer toolchain")
        name(producer["runtime_version"], "runtime version")
    require(exact(plan["semantics"], SEMANTICS), "completion, reuse, clock, or measurement semantics drift")
    workload = plan["workload"]
    fields(workload, {"name", "artifact_sha256", "abi_sha256", "effects_sha256", "oracle_sha256", "argument_template_sha256",
                      "kernel", "grid", "workgroup", "kernarg_bytes", "buffers", "kernel_bindings", "copy"}, "workload")
    for key in ("name", "kernel"):
        name(workload[key], key)
    for key in ("artifact_sha256", "abi_sha256", "effects_sha256", "oracle_sha256", "argument_template_sha256"):
        digest(workload[key], key)
    integer(workload["kernarg_bytes"], 1, 65536, "kernarg bytes")
    for key, maximum in (("grid", (1 << 32) - 1), ("workgroup", 1024)):
        dimensions = workload[key]
        require(type(dimensions) is list and len(dimensions) == 3, "invalid launch dimensions")
        product = 1
        for dimension in dimensions:
            product *= integer(dimension, 1, maximum, key)
        require(product <= maximum, "launch geometry exceeds bounded profile")
    require(all(grid % group == 0 for grid, group in zip(workload["grid"], workload["workgroup"])),
            "grid must contain whole workgroups")
    buffers = workload["buffers"]
    require(type(buffers) is list and 3 <= len(buffers) <= 16, "invalid buffer roster")
    names = set()
    for buffer in buffers:
        fields(buffer, {"name", "memory", "bytes", "initial_sha256", "expected_sha256"}, "buffer")
        name(buffer["name"], "buffer name")
        require(buffer["name"] not in names, "duplicate buffer name")
        names.add(buffer["name"])
        require(buffer["memory"] in ("host-visible", "device-local"), "unsupported buffer memory")
        integer(buffer["bytes"], 1, MAX_ALLOCATION_BYTES, "buffer bytes")
        digest(buffer["initial_sha256"], "initial full-buffer oracle")
        digest(buffer["expected_sha256"], "expected full-buffer oracle")
    bindings = workload["kernel_bindings"]
    require(type(bindings) is list and 1 <= len(bindings) <= 14, "invalid compute binding roster")
    for binding in bindings:
        integer(binding, 0, len(buffers) - 1, "kernel binding")
        require(buffers[binding]["memory"] == "device-local", "compute binding must be device-local")
    require(len(bindings) == len(set(bindings)), "aliased compute binding descriptors")
    copy = workload["copy"]
    fields(copy, {"direction", "source", "destination", "source_offset", "destination_offset", "bytes"}, "copy")
    integer(copy["bytes"], 1, MAX_ALLOCATION_BYTES, "copy bytes")
    for endpoint in ("source", "destination"):
        index = integer(copy[endpoint], 0, len(buffers) - 1, "copy endpoint")
        require(index not in bindings, "copy/compute descriptors must be whole-allocation disjoint")
        offset = integer(copy[endpoint + "_offset"], 0, MAX_ALLOCATION_BYTES, "copy offset")
        require(offset + copy["bytes"] <= buffers[index]["bytes"], "copy exceeds requested allocation")
    expected_memory = {"h2d": ("host-visible", "device-local"), "d2h": ("device-local", "host-visible")}
    require(type(copy["direction"]) is str and copy["direction"] in expected_memory, "unsupported copy direction")
    require(copy["source"] != copy["destination"] and tuple(buffers[copy[key]]["memory"] for key in ("source", "destination"))
            == expected_memory[copy["direction"]], "copy direction or endpoint mismatch")
    policy = plan["policy"]
    fields(policy, {"rounds", "warmups_per_block", "samples_per_block", "rotation", "publication_order", "phase_deadline_ns"}, "policy")
    require(integer(policy["rounds"], 3, 12, "rounds") % 3 == 0, "rotation requires complete balanced cycles")
    integer(policy["warmups_per_block"], 1, 4, "warmups")
    integer(policy["samples_per_block"], 2, 16, "samples")
    require(exact(policy["rotation"], list(BACKENDS)), "backend order must follow the precommitted rotation")
    require(policy["publication_order"] in ("compute-first", "copy-first"), "unknown publication order")
    integer(policy["phase_deadline_ns"], 1, 10_000_000_000, "phase deadline")
    limits = plan["resource_limits"]
    fields(limits, set(PEAK_BASES), "resource limits")
    for key, value in limits.items():
        if value is None:
            require(key in ("resident_backing_bytes", "native_published_slots"), "required resource ceiling missing")
        else:
            integer(value, 1, MAX_INTEGER, "resource ceiling")
    require(sum(buffer["bytes"] for buffer in buffers) <= limits["requested_allocation_bytes"], "requested payload exceeds ceiling")
    return plan


def header(record: dict[str, Any], kind: str, plan: dict[str, Any], plan_hash: str, correctness_hash: str | None) -> str:
    keys = {"record", "schema", "kind", "capture_id", "plan_sha256", "campaign_id", "source_commit", "host_boot_id", "physical_overlap"}
    if kind == "timing":
        keys.add("correctness_capture_sha256")
    fields(record, keys, "capture header")
    require(record["record"] == "capture" and record["schema"] == CAPTURE_SCHEMA and record["kind"] == kind,
            "wrong or mixed capture kind")
    for key in ("campaign_id", "source_commit", "host_boot_id"):
        require(exact(record[key], plan[key]), "capture campaign/source/host mismatch")
    require(record["plan_sha256"] == plan_hash and record["physical_overlap"] == "unmeasured", "plan or overlap claim mismatch")
    if kind == "timing":
        require(record["correctness_capture_sha256"] == correctness_hash, "timing capture references different correctness bytes")
    return digest(record["capture_id"], "capture ID")


def matched(record: dict[str, Any], backend: str, plan: dict[str, Any]) -> None:
    require(record["backend"] == backend, "missing, duplicated, or reordered backend")
    require(exact(record["producer"], plan["producers"][backend]), "producer identity mismatch")
    require(exact(record["gpu"], plan["gpu"]), "exact GPU/topology/placement mismatch")
    require(record["workload_sha256"] == object_sha256(plan["workload"]), "artifact, ABI, effects, geometry, bytes, or oracle mismatch")


def occurrence(value: Any, seen: set[str]) -> None:
    digest(value, "occurrence")
    require(value not in seen, "reused capture, invocation, or sample identity")
    seen.add(value)


def validate_correctness(text: str, plan: dict[str, Any], plan_hash: str, seen: set[str]) -> None:
    records = parse_records(text)
    require(len(records) == 5, "correctness capture requires three exact backend validations")
    occurrence(header(records[0], "correctness", plan, plan_hash, None), seen)
    for backend, record in zip(BACKENDS, records[1:-1]):
        fields(record, {"record", "backend", "producer", "gpu", "workload_sha256", "invocation_id",
                        "initial_buffer_sha256", "output_buffer_sha256", "completed_operations", "cleanup"}, "correctness")
        require(record["record"] == "correctness", "timing data cannot replace correctness capture")
        matched(record, backend, plan)
        occurrence(record["invocation_id"], seen)
        for key, expected_key in (("initial_buffer_sha256", "initial_sha256"), ("output_buffer_sha256", "expected_sha256")):
            require(exact(record[key], [buffer[expected_key] for buffer in plan["workload"]["buffers"]]), "independent full-buffer oracle mismatch")
        integer(record["completed_operations"], 2, 2, "correctness completion")
        require(record["cleanup"] == "complete", "correctness cleanup incomplete")
    require(exact(records[-1], {"record": "complete", "validated_backends": 3, "cleanup": "complete"}), "correctness capture incomplete")


def interval(value: Any, deadline: int) -> tuple[int, int, int, int]:
    fields(value, {"wall_start_ns", "wall_end_ns", "cpu_start_ns", "cpu_end_ns"}, "host interval")
    values = tuple(integer(value[key], 0, MAX_INTEGER, key) for key in
                   ("wall_start_ns", "wall_end_ns", "cpu_start_ns", "cpu_end_ns"))
    wall_start, wall_end, cpu_start, cpu_end = values
    require(0 < wall_end - wall_start <= deadline and 0 <= cpu_end - cpu_start <= deadline * 1024,
            "invalid or over-deadline wall/process CPU interval")
    return values


def validate_peaks(value: Any, plan: dict[str, Any]) -> list[str]:
    fields(value, set(PEAK_BASES), "resource peaks")
    unavailable = []
    for key, observation in value.items():
        fields(observation, {"value", "basis"}, "resource observation")
        ceiling = plan["resource_limits"][key]
        if observation["value"] is None:
            require(key in ("resident_backing_bytes", "native_published_slots") and ceiling is None
                    and observation["basis"] == "unavailable", "missing required resource observation")
            unavailable.append(key)
            continue
        peak = integer(observation["value"], 1, MAX_INTEGER, "resource peak")
        require(observation["basis"] == PEAK_BASES[key], "resource accounting basis mismatch")
        require(ceiling is None or peak <= ceiling, "resource peak exceeds precommitted ceiling")
        requested = sum(buffer["bytes"] for buffer in plan["workload"]["buffers"])
        if key == "requested_allocation_bytes":
            require(peak == requested, "requested buffer roster accounting mismatch")
        if key == "resident_backing_bytes":
            require(peak >= requested, "physical backing peak cannot omit requested payload")
    return unavailable


def validate_timing(text: str, plan: dict[str, Any], plan_hash: str, correctness_hash: str, seen: set[str]) -> dict[str, Any]:
    records = parse_records(text)
    policy = plan["policy"]
    iterations = policy["warmups_per_block"] + policy["samples_per_block"]
    blocks = policy["rounds"] * 3
    require(len(records) == 2 + blocks * (iterations + 2), "missing or invented timing records")
    occurrence(header(records[0], "timing", plan, plan_hash, correctness_hash), seen)
    cursor, sample_ordinal, wall_frontier = 1, 0, 0
    unavailable = set()
    phase_order = list(PHASES)
    if policy["publication_order"] == "copy-first":
        phase_order[1:3] = ["transfer", "submission"]
    for round_index in range(policy["rounds"]):
        order = BACKENDS[round_index % 3:] + BACKENDS[:round_index % 3]
        for position, backend in enumerate(order):
            start = records[cursor]
            cursor += 1
            fields(start, {"record", "round", "position", "backend", "producer", "gpu", "workload_sha256", "invocation_id", "setup"}, "block")
            require(start["record"] == "block", "missing block setup")
            require(exact(start["round"], round_index) and exact(start["position"], position), "rotation schedule mismatch")
            matched(start, backend, plan)
            occurrence(start["invocation_id"], seen)
            setup = interval(start["setup"], policy["phase_deadline_ns"])
            require(setup[0] >= wall_frontier, "blocks overlap or clocks are incompatible")
            frontier_wall, frontier_cpu = setup[1], setup[3]
            for ordinal in range(iterations):
                record = records[cursor]
                cursor += 1
                fields(record, {"record", "round", "backend", "invocation_id", "sample_id", "phase", "index",
                                "ordinal", "issued_operations", "completed_operations", "phases", "end_to_end_ns", "process_cpu_ns", "validation"}, "iteration")
                expected_phase = "warmup" if ordinal < policy["warmups_per_block"] else "sample"
                expected_index = ordinal if expected_phase == "warmup" else ordinal - policy["warmups_per_block"]
                require(record["record"] == "iteration" and record["phase"] == expected_phase
                        and exact(record["index"], expected_index) and exact(record["ordinal"], sample_ordinal)
                        and exact(record["round"], round_index) and record["backend"] == backend
                        and record["invocation_id"] == start["invocation_id"], "missing, invented, reordered, or relabelled sample")
                sample_ordinal += 1
                occurrence(record["sample_id"], seen)
                integer(record["issued_operations"], 2, 2, "issued operations")
                integer(record["completed_operations"], 2, 2, "completed operations")
                require(record["validation"] == "full-requested-buffers-passed-outside-interval", "timed workload not validated")
                phases = record["phases"]
                require(type(phases) is list and len(phases) == 5, "missing cost category")
                checked = {}
                for index, (phase_name, phase) in enumerate(zip(phase_order, phases)):
                    fields(phase, {"name", "interval"}, "cost category")
                    require(phase["name"] == phase_name, "cost order or category mismatch")
                    current = interval(phase["interval"], policy["phase_deadline_ns"])
                    require(current[0] >= frontier_wall and current[2] >= frontier_cpu, "sample clocks moved backwards")
                    if index > 0:
                        require(current[0] == frontier_wall and current[2] == frontier_cpu, "unaccounted cost interval")
                    frontier_wall, frontier_cpu = current[1], current[3]
                    checked[phase_name] = current
                first = checked[phase_order[1]]
                last = checked["wait"]
                require(exact(record["end_to_end_ns"], last[1] - first[0])
                        and exact(record["process_cpu_ns"], last[3] - first[2]), "invented aggregate or reset/validation contamination")
            complete = records[cursor]
            cursor += 1
            fields(complete, {"record", "round", "backend", "invocation_id", "warmups", "samples", "cleanup", "cleanup_cost", "resource_peaks"}, "block completion")
            require(complete["record"] == "block-complete" and exact(complete["round"], round_index)
                    and complete["backend"] == backend and complete["invocation_id"] == start["invocation_id"]
                    and exact(complete["warmups"], policy["warmups_per_block"])
                    and exact(complete["samples"], policy["samples_per_block"])
                    and complete["cleanup"] == "complete", "block completion or cleanup mismatch")
            cleanup = interval(complete["cleanup_cost"], policy["phase_deadline_ns"])
            require(cleanup[0] >= frontier_wall and cleanup[2] >= frontier_cpu, "cleanup precedes completed work")
            wall_frontier = cleanup[1]
            unavailable.update(validate_peaks(complete["resource_peaks"], plan))
    require(exact(records[cursor], {"record": "complete", "blocks": blocks, "warmups": blocks * policy["warmups_per_block"],
                                    "samples": blocks * policy["samples_per_block"], "cleanup": "complete"}), "timing capture incomplete")
    return {"blocks": blocks, "samples_per_backend": policy["rounds"] * policy["samples_per_block"],
            "unavailable_peak_categories": sorted(unavailable)}


def validate_campaign(plan_text: str, correctness_text: str, timing_text: str, expected_plan_sha256: str) -> dict[str, Any]:
    plan = validate_plan(plan_text, expected_plan_sha256)
    seen: set[str] = set()
    validate_correctness(correctness_text, plan, expected_plan_sha256, seen)
    validated = validate_timing(timing_text, plan, expected_plan_sha256, sha256(correctness_text), seen)
    return {"schema": "fe2o3.scale3-protocol-validation.v1", "plan_sha256": expected_plan_sha256,
            "consistency_only": True, "hardware_authenticity": "not-established", "performance_claim": False,
            "native_budget_closure": "not-established", "physical_overlap": "unmeasured", **validated}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", required=True, type=pathlib.Path)
    parser.add_argument("--plan-sha256", required=True)
    parser.add_argument("--correctness", required=True, type=pathlib.Path)
    parser.add_argument("--timing", required=True, type=pathlib.Path)
    args = parser.parse_args(argv)
    try:
        result = validate_campaign(load_text(args.plan), load_text(args.correctness), load_text(args.timing), args.plan_sha256)
    except CheckError as error:
        print(f"SCALE-3 protocol rejected: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, sort_keys=True, allow_nan=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
