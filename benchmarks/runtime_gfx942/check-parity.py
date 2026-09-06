#!/usr/bin/env python3
"""Fail-closed comparison of matched fe2o3, HSA, and HIP benchmark rows."""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import pathlib
import re
import shlex
import struct
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
    "fe2o3.d2d-copy-benchmark.v1": (
        ("d2d_p50_ns", "latency"),
        ("d2d_p95_ns", "latency"),
        ("d2d_p50_GBps", "bandwidth"),
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

R26_INPLACE_SCHEMA = "fe2o3.r26-inplace-benchmark.v4"
R26_SYSTEM_IDENTITY_SCHEMA = "fe2o3.r26-system-identity.v1"
R26_MANIFEST_SCHEMA = "fe2o3.r26-evidence-manifest.v1"
R26_TOPOLOGY_SCHEMA = "fe2o3.r26-host-topology.v1"
R26_MONITOR_SCHEMA = "fe2o3.r26-kfd-queue-monitor.v2"
R26_EXECUTION_ENVIRONMENT = "env-i-lang-c-lc-all-c-path-usr-sbin-usr-bin-sbin-bin-v1"
R26_PHASES = ("h2d", "compute", "d2h", "e2e")
R26_LAUNCH_TIMING_PHASES = (
    "preparation",
    "bound_snapshot",
    "authority",
    "native_binding",
    "publication",
    "publish_to_completion",
    "completed_readback",
    "completion_signal_recycle",
    "completion_detach_restore",
    "recycle_inclusive",
)
R26_SUMMARIES = ("min", "mean", "max", "p50", "p95")
R26_COUNTERBALANCE_DESIGN = "cyclic-latin-square-3"
R26_MAX_LDD_BYTES = 1 << 20
R26_MAX_LOADER_DEPENDENCIES = 4096
R26_MAX_LOADER_EVIDENCE_BYTES = 4 << 20
R26_MAX_NANOSECONDS = (1 << 128) - 1
R26_COUNTERBALANCE_ORDERS = {
    0: ("kfd", "hsa", "hip"),
    1: ("hsa", "hip", "kfd"),
    2: ("hip", "kfd", "hsa"),
}
R26_CONTEXT_FIELDS = (
    "git_commit",
    "target",
    "gpu_index",
    "unique_id",
    "uuid",
    "bytes",
    "elements",
    "workgroup",
    "warmups",
    "samples",
    "iterations_per_sample",
    "kernel",
    "max_busy_percent",
    "phase_timeout_seconds",
    "rocm_version",
    "rustc",
    "cargo",
    "hipcc",
    "cxx",
    "build_environment",
    "hsaco_sha256",
    "kernel_source_sha256",
    "kernel_policy_sha256",
    "fixture_recipe_sha256",
    "fixture_producer_clang",
    "fixture_rebuild",
    "kfd_binary_sha256",
    "hsa_binary_sha256",
    "hip_binary_sha256",
    "hsa_source_sha256",
    "hip_source_sha256",
    "binary_reader_sha256",
    "hsa_pool_policy_sha256",
    "common_header_sha256",
    "checker_sha256",
    "runner_sha256",
    "host_guard_sha256",
    "system_identity_collector_sha256",
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
)
R26_ROW_FIELDS = (
    "device_index",
    "unique_id",
    "uuid",
    "target",
    "xnack",
    "kernel",
    "bytes",
    "elements",
    "workgroup",
    "warmups",
    "samples",
    "iterations_per_sample",
    "sample_value",
    "recycle_inclusive_sample_value",
    "trimming",
    "input_pattern",
    "pattern_start",
    "validation",
    "validated_iterations",
    "pattern_a_iterations",
    "pattern_b_iterations",
    "timing",
    "interphase_control",
    "promotion",
    "data_path",
    "control_path",
    "user_data_materializations",
    "input_a_sha256",
    "output_a_sha256",
    "input_b_sha256",
    "output_b_sha256",
)
R26_FIXED_CONTEXT = {
    "target": "gfx942:xnack-",
    "bytes": "1048576",
    "elements": "262144",
    "workgroup": "256",
    "warmups": "10",
    "samples": "30",
    "iterations_per_sample": "10",
    "kernel": "inplace_transform",
    "hsaco_sha256": "8fe108f507def33e7717130a328ff9058067630b4fc5ee7820030cc07a3d98e9",
    "kernel_source_sha256": "1185d4cd931c1bb43d113e66714af3d98bd96f7d036f5c610a909abf34ba87d5",
    "kernel_policy_sha256": "c060c3c4a96012fc6661b0585f4ff8ffe7b7f8483eb40262e4a018133c0ea585",
    "fixture_recipe_sha256": "29c6db8ea2a86392eb980b78e42fa1c049a6f92ca8dd3dc8224f90cf66254ab5",
    "fixture_producer_clang": "AMD_clang_version_22.0.0git_(https://github.com/RadeonOpenCompute/llvm-project_roc-7.2.0_26014_7b800a19466229b8479a78de19143dc33c3ab9b5)",
    "fixture_rebuild": "not-run-on-measurement-host",
    "build_environment": (
        "env-i-explicit-home-toolchain-path-cargo-incremental-0-private-target-v1"
    ),
    "execution_environment": R26_EXECUTION_ENVIRONMENT,
    "telemetry_command": "rocm-smi-showuse-showclocks-showpower",
    "placement": "taskset-cpulist-then-numactl-physcpubind-membind-v1",
    "interference_monitor": "selected-kfd-gpu-process-tree-census-v2",
    "monitor_interval_us": "2000",
    "monitor_maximum_gap_us": "10000",
    "counterbalance_design": R26_COUNTERBALANCE_DESIGN,
    "counterbalance_slots": "3",
}
R26_FIXED_ROW = {
    "device_index": "0",
    "target": "gfx942:xnack-",
    "xnack": "disabled",
    "kernel": "inplace_transform",
    "bytes": "1048576",
    "elements": "262144",
    "workgroup": "256",
    "warmups": "10",
    "samples": "30",
    "iterations_per_sample": "10",
    "sample_value": "integer-average-ns-over-10-iterations",
    "recycle_inclusive_sample_value": "sum-of-component-integer-averages-ns",
    "trimming": "none",
    "input_pattern": "alternating-full-a-b",
    "pattern_start": "a",
    "validation": "every-element-every-iteration",
    "validated_iterations": "310",
    "pattern_a_iterations": "155",
    "pattern_b_iterations": "155",
    "timing": "host-monotonic",
    "interphase_control": "e2e-h2d-compute-d2h",
    "input_a_sha256": "ce96f8d88572648c07a6c03d7ce49af52c637af65267645eafdd2193ee6e49b7",
    "output_a_sha256": "4a42778046c60e35849ad35fe4dc4bf39a0a4d616b75c9e62d146dbdb41ec960",
    "input_b_sha256": "061cc02d1e9f513366e292544724ef6592b6ca4f59cfb2464a29bd94ff71236e",
    "output_b_sha256": "49f9da5c37cd051649cf257f528b1b573b44a1937b865b05643823267579cf62",
}
R26_SAMPLE_FIELDS = tuple(
    field
    for phase in R26_PHASES + ("promotion",) + R26_LAUNCH_TIMING_PHASES
    for field in (f"{phase}_samples_ns",)
    + tuple(f"{phase}_{summary}_ns" for summary in R26_SUMMARIES)
)
R26_EXACT_CONTEXT_FIELDS = frozenset(("schema",) + R26_CONTEXT_FIELDS)
R26_SYSTEM_IDENTITY_FIELDS = (
    "amdgpu_build_id",
    "amdgpu_build_note_base64",
    "amdgpu_build_note_sha256",
    "amdgpu_module_build_id",
    "amdgpu_module_decompressed_bytes",
    "amdgpu_module_decompressed_sha256",
    "amdgpu_module_path_base64",
    "amdgpu_module_sha256",
    "amdgpu_srcversion",
    "amdgpu_taint",
    "amdgpu_vermagic_base64",
    "amdgpu_version",
    "boot_id",
    "execution_environment",
    "gfx_version",
    "gpu_guid",
    "gpu_index",
    "gpu_node_id",
    "gpu_serial",
    "hip_binary_sha256",
    "hip_ldd_base64",
    "hip_ldd_sha256",
    "hip_library_build_id",
    "hip_library_path_base64",
    "hip_library_sha256",
    "hip_library_soname",
    "hip_loader_map_base64",
    "hip_loader_map_sha256",
    "hip_loader_resolution_base64",
    "hip_loader_resolution_sha256",
    "hsa_binary_sha256",
    "hsa_ldd_base64",
    "hsa_ldd_sha256",
    "hsa_library_build_id",
    "hsa_library_path_base64",
    "hsa_library_sha256",
    "hsa_library_soname",
    "hsa_loader_map_base64",
    "hsa_loader_map_sha256",
    "hsa_loader_resolution_base64",
    "hsa_loader_resolution_sha256",
    "kernel_machine",
    "kernel_release",
    "kernel_sysname",
    "kernel_version_base64",
    "kernel_version_sha256",
    "kfd_binary_sha256",
    "kfd_ldd_base64",
    "kfd_ldd_sha256",
    "kfd_loader_map_base64",
    "kfd_loader_map_sha256",
    "kfd_loader_resolution_base64",
    "kfd_loader_resolution_sha256",
    "ld_audit",
    "ld_library_path",
    "ld_preload",
    "ldd_path_base64",
    "ldd_sha256",
    "loader_resolution",
    "modinfo_path_base64",
    "modinfo_sha256",
    "observation_edge",
    "os_release_base64",
    "os_release_sha256",
    "pci_bdf",
    "pci_class",
    "pci_device",
    "pci_driver",
    "pci_numa_node",
    "pci_revision",
    "pci_serial",
    "pci_subsystem_device",
    "pci_subsystem_vendor",
    "pci_unique_id",
    "pci_vendor",
    "product_model",
    "product_name_base64",
    "product_number",
    "product_series_base64",
    "product_sku",
    "readelf_path_base64",
    "readelf_sha256",
    "rocm_path_base64",
    "rocm_smi_entrypoint_path_base64",
    "rocm_smi_entrypoint_sha256",
    "rocm_smi_identity_base64",
    "rocm_smi_identity_sha256",
    "rocm_smi_interpreter_build_id",
    "rocm_smi_interpreter_invocation_path_base64",
    "rocm_smi_interpreter_ldd_base64",
    "rocm_smi_interpreter_ldd_sha256",
    "rocm_smi_interpreter_loader_map_base64",
    "rocm_smi_interpreter_loader_map_sha256",
    "rocm_smi_interpreter_loader_resolution_base64",
    "rocm_smi_interpreter_loader_resolution_sha256",
    "rocm_smi_interpreter_path_base64",
    "rocm_smi_interpreter_sha256",
    "rocm_smi_invocation_path_base64",
    "rocm_smi_library_build_id",
    "rocm_smi_library_ldd_base64",
    "rocm_smi_library_ldd_sha256",
    "rocm_smi_library_loader_map_base64",
    "rocm_smi_library_loader_map_sha256",
    "rocm_smi_library_loader_resolution_base64",
    "rocm_smi_library_loader_resolution_sha256",
    "rocm_smi_library_path_base64",
    "rocm_smi_library_sha256",
    "rocm_smi_library_soname",
    "rocm_smi_package_manifest_base64",
    "rocm_smi_package_manifest_sha256",
    "rocm_smi_shebang_base64",
    "unique_id",
    "uuid",
    "zstd_path_base64",
    "zstd_sha256",
)
R26_EXACT_SYSTEM_IDENTITY_FIELDS = frozenset(("schema",) + R26_SYSTEM_IDENTITY_FIELDS)
R26_EDGE_VARIANT_SYSTEM_IDENTITY_FIELDS = frozenset(
    {
        "observation_edge",
        "hip_ldd_base64",
        "hip_ldd_sha256",
        "hsa_ldd_base64",
        "hsa_ldd_sha256",
        "kfd_ldd_base64",
        "kfd_ldd_sha256",
        "rocm_smi_interpreter_ldd_base64",
        "rocm_smi_interpreter_ldd_sha256",
        "rocm_smi_library_ldd_base64",
        "rocm_smi_library_ldd_sha256",
    }
)
R26_TOPOLOGY_SEALED_FIELDS = (
    "schema",
    "placement",
    "gpu_index",
    "pci_bdf",
    "unique_id",
    "numa_node",
    "device_local_cpu_list",
    "allowed_cpu_list",
    "allowed_mem_node_list",
    "measurement_cpu_list",
    "observer_cpu",
    "kfd_node",
    "kfd_gpu_id",
)
R26_EXACT_TOPOLOGY_FIELDS = frozenset(
    ("slot", "phase", "edge", "topology_sha256") + R26_TOPOLOGY_SEALED_FIELDS
)
R26_MONITOR_SEALED_FIELDS = (
    "schema",
    "status",
    "monitor",
    "schedule",
    "kfd_gpu_id",
    "root_pid",
    "process_group",
    "observer_cpu",
    "interval_us",
    "maximum_gap_us",
    "observed_maximum_gap_us",
    "observations",
    "target_selected_queue_observations",
    "foreign_selected_queues",
    "terminal_selected_queues",
    "target_exit_code",
    "target_reaped",
    "process_group_absent",
    "target_output_bytes",
    "target_output_sha256",
)
R26_EXACT_MONITOR_FIELDS = frozenset(
    ("slot", "phase", "monitor_sha256") + R26_MONITOR_SEALED_FIELDS
)
R26_FIXED_SYSTEM_IDENTITY = {
    "gfx_version": "gfx942",
    "execution_environment": R26_EXECUTION_ENVIRONMENT,
    "hip_library_soname": "libamdhip64.so.7",
    "hsa_library_soname": "libhsa-runtime64.so.1",
    "kernel_machine": "x86_64",
    "kernel_sysname": "Linux",
    "ld_audit": "absent",
    "ld_library_path": "absent",
    "ld_preload": "absent",
    "loader_resolution": "fixed-ldd-transitive-observed-to-canonical-v1",
    "pci_class": "0x120000",
    "pci_device": "0x74a1",
    "pci_driver": "amdgpu",
    "pci_revision": "0x00",
    "pci_subsystem_device": "0x74a1",
    "pci_subsystem_vendor": "0x1002",
    "pci_vendor": "0x1002",
    "product_model": "0x74a1",
    "product_number": "102-G30211-00",
    "product_sku": "M3000100",
}
R26_EXACT_ROW_FIELDS = frozenset(
    ("backend", "schema") + R26_ROW_FIELDS + R26_SAMPLE_FIELDS
)

SCHEMA_MATCH_FIELDS = {
    "fe2o3.async-copy-benchmark.v1": ("unique_id", "warmups", "samples"),
    "fe2o3.async-copy-multi-device-benchmark.v1": (
        "devices",
        "unique_ids",
        "warmups",
        "samples",
    ),
    "fe2o3.d2d-copy-benchmark.v1": ("unique_id", "warmups", "samples"),
    "fe2o3.xgmi-peer-benchmark.v1": ("unique_ids", "warmups", "samples"),
}

SCHEMA_CONTEXT = {
    "fe2o3.async-copy-benchmark.v1": "fe2o3.async-copy-benchmark.v1",
    "fe2o3.async-copy-multi-device-benchmark.v1": "fe2o3.async-copy-benchmark.v1",
    "fe2o3.d2d-copy-benchmark.v1": "fe2o3.d2d-copy-benchmark.v1",
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
    "fe2o3.d2d-copy-benchmark.v1": (
        "kfd_profile",
        "sdma_manifest_sha256",
        "d2d_window_manifest_sha256",
        "timing",
        "setup_validation",
        "measurement",
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
CANONICAL_BUILD_ID = re.compile(r"[0-9a-f]{16,128}")
CANONICAL_BOOT_ID = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
)
CANONICAL_PCI_BDF = re.compile(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]")
FORBIDDEN_KFD_RUNTIME = re.compile(r"lib(?:hsa-runtime64|amdhip64)\.so(?:\..*)?")
STRIPED_KFD_PROFILE = re.compile(r"striped(2|4|6|8|10|12|14|16)")
GFX942_SDMA_MAX_LINEAR_COPY_BYTES = 0x003F_FFE0
GFX942_D2D_MAX_WINDOW_PACKETS = 63
GFX942_D2D_MIN_QUALIFICATION_BYTES = (
    GFX942_SDMA_MAX_LINEAR_COPY_BYTES * GFX942_D2D_MAX_WINDOW_PACKETS + 1
)
GFX942_D2D_MAX_QUALIFICATION_BYTES = 256 * 1024 * 1024
GFX942_SDMA_MANIFEST_SHA256 = (
    "c4dc0b4d058579c0edb99be5593f22f7d0123e4680758b0d86aa18bbf146fa62"
)
GFX942_D2D_WINDOW_MANIFEST_SHA256 = (
    "93d1277fe7aa07e0773793a756f4a4797d25e1abd09b5cb7639188a08baaedc7"
)
D2D_BANDWIDTH_ROUNDING_TOLERANCE = Decimal("0.000501")


class CheckError(Exception):
    pass


def parse_fields(line: str, line_number: int) -> dict[str, str]:
    fields: dict[str, str] = {}
    for token in line.split():
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        if not key or not value or key in fields:
            raise CheckError(
                f"line {line_number}: malformed or duplicate field {key!r}"
            )
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

    if "unique_id" in row and CANONICAL_UNIQUE_ID.fullmatch(row["unique_id"]) is None:
        raise CheckError(
            "field unique_id must be exactly 16 lowercase hexadecimal digits"
        )
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


def positive_integer(
    fields: dict[str, str], field: str, *, allow_zero: bool = False
) -> int:
    try:
        value = fields[field]
    except KeyError as error:
        raise CheckError(f"missing required field {field}") from error
    if not value.isdigit() or (not allow_zero and value == "0"):
        qualifier = "nonnegative" if allow_zero else "positive"
        raise CheckError(f"field {field} must be a {qualifier} integer")
    return int(value)


def admitted_kfd_profile(profile: str) -> bool:
    return profile in {
        "generic",
        "directional",
        "engine0",
        "engine1",
        "same-device-d2d",
    } or (STRIPED_KFD_PROFILE.fullmatch(profile) is not None)


def r26_raw_samples(
    row: dict[str, str], phase: str, *, allow_zero: bool = False
) -> list[int]:
    field = f"{phase}_samples_ns"
    try:
        values = row[field].split(",")
    except KeyError as error:
        raise CheckError(
            f"backend {row.get('backend', '?')} is missing required field {field}"
        ) from error
    pattern = r"(?:0|[1-9][0-9]*)" if allow_zero else r"[1-9][0-9]*"
    if len(values) != 30 or any(re.fullmatch(pattern, value) is None for value in values):
        qualifier = "nonnegative" if allow_zero else "positive"
        raise CheckError(
            f"field {field} must contain exactly 30 canonical ASCII {qualifier} "
            "integer samples"
        )
    if any(
        len(value) > len(str(R26_MAX_NANOSECONDS))
        or (len(value) == len(str(R26_MAX_NANOSECONDS))
            and value > str(R26_MAX_NANOSECONDS))
        for value in values
    ):
        raise CheckError(f"field {field} contains a sample above the R26 u128 bound")
    return [int(value) for value in values]


def r26_summary_integer(
    row: dict[str, str], field: str, *, allow_zero: bool = False
) -> int:
    try:
        value = row[field]
    except KeyError as error:
        raise CheckError(f"missing required field {field}") from error
    pattern = r"(?:0|[1-9][0-9]*)" if allow_zero else r"[1-9][0-9]*"
    qualifier = "nonnegative" if allow_zero else "positive"
    if re.fullmatch(pattern, value) is None:
        raise CheckError(
            f"field {field} must be a canonical ASCII {qualifier} integer"
        )
    maximum = str(R26_MAX_NANOSECONDS)
    if len(value) > len(maximum) or (len(value) == len(maximum) and value > maximum):
        raise CheckError(f"field {field} exceeds the R26 u128 bound")
    return int(value)


def r26_validate_summaries(
    row: dict[str, str], phase: str, values: list[int], *, allow_zero: bool = False
) -> None:
    sorted_values = sorted(values)
    expected = {
        "min": sorted_values[0],
        "mean": r26_checked_sum(values, f"{phase} summary") // len(values),
        "max": sorted_values[-1],
        "p50": sorted_values[(len(values) * 50 + 99) // 100 - 1],
        "p95": sorted_values[(len(values) * 95 + 99) // 100 - 1],
    }
    for summary, expected_value in expected.items():
        field = f"{phase}_{summary}_ns"
        if r26_summary_integer(row, field, allow_zero=allow_zero) != expected_value:
            raise CheckError(f"field {field} is inconsistent with raw samples")


def r26_checked_sum(values: Iterable[int], description: str) -> int:
    total = 0
    for value in values:
        if value > R26_MAX_NANOSECONDS - total:
            raise CheckError(f"R26 {description} timing overflow")
        total += value
    return total


def r26_p50(values: list[int]) -> int:
    return sorted(values)[(len(values) * 50 + 99) // 100 - 1]


def r26_validate_context(context: dict[str, str]) -> tuple[str, str]:
    missing = set(R26_CONTEXT_FIELDS) - context.keys()
    if missing:
        raise CheckError(
            f"R26 benchmark context is missing fields: {','.join(sorted(missing))}"
        )
    unexpected = context.keys() - R26_EXACT_CONTEXT_FIELDS
    if unexpected:
        raise CheckError(
            f"R26 benchmark context has unexpected fields: {','.join(sorted(unexpected))}"
        )
    if CANONICAL_GIT_COMMIT.fullmatch(context["git_commit"]) is None:
        raise CheckError("R26 context git_commit must be a canonical 40-digit commit")
    for field, expected in R26_FIXED_CONTEXT.items():
        if context.get(field) != expected:
            raise CheckError(f"R26 context has invalid {field}")
    if not context["gpu_index"].isdigit():
        raise CheckError("R26 context gpu_index must be a nonnegative integer")
    if CONTEXT_UNIQUE_ID.fullmatch(context["unique_id"]) is None:
        raise CheckError("R26 context unique_id must be canonical")
    unique_id = context["unique_id"].removeprefix("0x")
    if unique_id == "0" * 16:
        raise CheckError("R26 context unique_id must be nonzero")
    uuid = f"GPU-{unique_id}"
    if context["uuid"] != uuid:
        raise CheckError("R26 context UUID does not match its unique ID")
    max_busy = positive_integer(context, "max_busy_percent", allow_zero=True)
    if max_busy > 100:
        raise CheckError("R26 context max_busy_percent must not exceed 100")
    positive_integer(context, "phase_timeout_seconds")
    for field in ("rocm_version", "rustc", "cargo", "hipcc", "cxx"):
        if context[field].lower() == "unknown":
            raise CheckError(f"R26 context {field} identity must be retained")
    for field in (
        "hsaco_sha256",
        "kernel_source_sha256",
        "kernel_policy_sha256",
        "fixture_recipe_sha256",
        "kfd_binary_sha256",
        "hsa_binary_sha256",
        "hip_binary_sha256",
        "hsa_source_sha256",
        "hip_source_sha256",
        "binary_reader_sha256",
        "hsa_pool_policy_sha256",
        "common_header_sha256",
        "checker_sha256",
        "runner_sha256",
        "host_guard_sha256",
        "system_identity_collector_sha256",
        "topology_sha256",
    ):
        if CANONICAL_SHA256.fullmatch(context[field]) is None:
            raise CheckError(f"R26 context {field} must be canonical")
    slot = positive_integer(context, "counterbalance_slot", allow_zero=True)
    if slot not in R26_COUNTERBALANCE_ORDERS:
        raise CheckError("R26 context counterbalance_slot must be 0, 1, or 2")
    if CANONICAL_SHA256.fullmatch(context["counterbalance_set_id"]) is None:
        raise CheckError("R26 context counterbalance_set_id must be canonical")
    expected_order = ",".join(R26_COUNTERBALANCE_ORDERS[slot])
    if context["backend_order"] != expected_order:
        raise CheckError(
            "R26 context backend_order does not match its counterbalance slot"
        )
    return unique_id, uuid


def r26_decode_base64(identity: dict[str, str], field: str) -> bytes:
    try:
        encoded = identity[field]
        decoded = base64.b64decode(encoded, validate=True)
    except (KeyError, binascii.Error, ValueError) as error:
        raise CheckError(
            f"R26 system identity {field} is not canonical base64"
        ) from error
    if not decoded or base64.b64encode(decoded).decode("ascii") != encoded:
        raise CheckError(f"R26 system identity {field} is not canonical base64")
    return decoded


def r26_validate_retained_blob(
    identity: dict[str, str], encoded_field: str, digest_field: str
) -> bytes:
    decoded = r26_decode_base64(identity, encoded_field)
    digest = identity[digest_field]
    if CANONICAL_SHA256.fullmatch(digest) is None:
        raise CheckError(f"R26 system identity {digest_field} must be canonical")
    if hashlib.sha256(decoded).hexdigest() != digest:
        raise CheckError(
            f"R26 system identity {digest_field} does not match its retained data"
        )
    return decoded


def r26_decode_path(identity: dict[str, str], field: str) -> pathlib.PurePosixPath:
    raw = r26_decode_base64(identity, field)
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError(f"R26 system identity {field} is not UTF-8") from error
    path = pathlib.PurePosixPath(text)
    if not path.is_absolute() or str(path) != text or ".." in path.parts:
        raise CheckError(f"R26 system identity {field} is not a canonical path")
    return path


def r26_parse_loader_map(
    identity: dict[str, str], backend: str
) -> dict[str, pathlib.PurePosixPath]:
    encoded_field = f"{backend}_loader_map_base64"
    digest_field = f"{backend}_loader_map_sha256"
    raw = r26_validate_retained_blob(identity, encoded_field, digest_field)
    if len(raw) > R26_MAX_LOADER_EVIDENCE_BYTES:
        raise CheckError(f"R26 {backend} loader map exceeds its retained bound")
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError(f"R26 {backend} loader map is not UTF-8") from error
    lines = text.splitlines()
    if len(lines) > R26_MAX_LOADER_DEPENDENCIES:
        raise CheckError(f"R26 {backend} loader map has excessive cardinality")
    if not text.endswith("\n") or not lines or lines != sorted(lines):
        raise CheckError(f"R26 {backend} loader map is not canonical")
    resolved: dict[str, pathlib.PurePosixPath] = {}
    for line in lines:
        if line.count("=") != 1:
            raise CheckError(f"R26 {backend} loader map has a malformed row")
        soname, path_text = line.split("=", 1)
        path = pathlib.PurePosixPath(path_text)
        if (
            re.fullmatch(r"[^/=\s]+", soname) is None
            or soname in resolved
            or not path.is_absolute()
            or str(path) != path_text
            or ".." in path.parts
        ):
            raise CheckError(f"R26 {backend} loader map has a malformed row")
        resolved[soname] = path
    return resolved


def r26_parse_loader_resolution(
    identity: dict[str, str], backend: str
) -> dict[str, tuple[pathlib.PurePosixPath, pathlib.PurePosixPath]]:
    raw = r26_validate_retained_blob(
        identity,
        f"{backend}_loader_resolution_base64",
        f"{backend}_loader_resolution_sha256",
    )
    if len(raw) > R26_MAX_LOADER_EVIDENCE_BYTES:
        raise CheckError(f"R26 {backend} loader resolution exceeds its retained bound")
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError(f"R26 {backend} loader resolution is not UTF-8") from error
    lines = text.splitlines()
    if len(lines) > R26_MAX_LOADER_DEPENDENCIES:
        raise CheckError(f"R26 {backend} loader resolution has excessive cardinality")
    if not text.endswith("\n") or not lines or lines != sorted(lines):
        raise CheckError(f"R26 {backend} loader resolution is not canonical")
    resolution: dict[str, tuple[pathlib.PurePosixPath, pathlib.PurePosixPath]] = {}
    pattern = re.compile(r"soname=([^/=\s]+)\tobserved=(/[^\s]*)\tresolved=(/[^\s]*)")
    for line in lines:
        match = pattern.fullmatch(line)
        if match is None:
            raise CheckError(f"R26 {backend} loader resolution has a malformed row")
        soname, observed_text, resolved_text = match.groups()
        observed = pathlib.PurePosixPath(observed_text)
        resolved = pathlib.PurePosixPath(resolved_text)
        if (
            soname in resolution
            or str(observed) != observed_text
            or str(resolved) != resolved_text
            or "=" in observed_text
            or "=" in resolved_text
            or ".." in observed.parts
            or ".." in resolved.parts
        ):
            raise CheckError(f"R26 {backend} loader resolution has a malformed row")
        resolution[soname] = (observed, resolved)
    return resolution


def r26_parse_raw_ldd(
    identity: dict[str, str], backend: str
) -> dict[str, pathlib.PurePosixPath]:
    raw = r26_validate_retained_blob(
        identity, f"{backend}_ldd_base64", f"{backend}_ldd_sha256"
    )
    if len(raw) > R26_MAX_LDD_BYTES:
        raise CheckError(f"R26 {backend} raw ldd output exceeds its retained bound")
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError(f"R26 {backend} raw ldd output is not UTF-8") from error
    lines = text.splitlines()
    if len(lines) > R26_MAX_LOADER_DEPENDENCIES:
        raise CheckError(
            f"R26 {backend} raw ldd output has excessive dependency cardinality"
        )
    resolved: dict[str, pathlib.PurePosixPath] = {}
    for line in lines:
        stripped = line.strip()
        if not stripped:
            raise CheckError(f"R26 {backend} raw ldd output has an empty row")
        if "=> not found" in stripped:
            raise CheckError(f"R26 {backend} raw ldd output is unresolved")
        mapped = re.fullmatch(r"(\S+)\s+=>\s+(/\S+)\s+\(0x[0-9a-fA-F]+\)", stripped)
        direct = re.fullmatch(r"(/\S+)\s+\(0x[0-9a-fA-F]+\)", stripped)
        virtual = re.fullmatch(
            r"linux-(?:vdso|gate)\.so\.[0-9]+\s+\(0x[0-9a-fA-F]+\)",
            stripped,
        )
        if virtual is not None or stripped == "statically linked":
            continue
        if mapped is not None:
            soname, path_text = mapped.groups()
        elif direct is not None:
            path_text = direct.group(1)
            soname = pathlib.PurePosixPath(path_text).name
        else:
            raise CheckError(f"R26 {backend} raw ldd output has an unknown row")
        path = pathlib.PurePosixPath(path_text)
        if (
            soname in resolved
            or not path.is_absolute()
            or str(path) != path_text
            or "=" in path_text
            or ".." in path.parts
        ):
            raise CheckError(f"R26 {backend} raw ldd output has a malformed row")
        resolved[soname] = path

    canonical = r26_parse_loader_map(identity, backend)
    retained_resolution = r26_parse_loader_resolution(identity, backend)
    if (
        resolved.keys() != canonical.keys()
        or resolved.keys() != retained_resolution.keys()
    ):
        raise CheckError(f"R26 {backend} raw ldd output and loader map differ")
    for soname, observed in resolved.items():
        retained_observed, retained_canonical = retained_resolution[soname]
        if observed != retained_observed or canonical[soname] != retained_canonical:
            raise CheckError(f"R26 {backend} raw ldd output and loader map differ")
    return canonical


def r26_parse_rocm_smi_package(identity: dict[str, str]) -> dict[str, str]:
    raw = r26_validate_retained_blob(
        identity,
        "rocm_smi_package_manifest_base64",
        "rocm_smi_package_manifest_sha256",
    )
    try:
        text = raw.decode("ascii", "strict")
    except UnicodeDecodeError as error:
        raise CheckError("R26 ROCm SMI package manifest is not ASCII") from error
    lines = text.splitlines()
    expected_names = ("rocm_smi.py", "rsmiBindings.py", "rsmiBindingsInit.py")
    if not text.endswith("\n") or len(lines) != len(expected_names):
        raise CheckError("R26 ROCm SMI package manifest is incomplete")
    files: dict[str, str] = {}
    for line in lines:
        match = re.fullmatch(r"file=([^/\s]+)\tsha256=([0-9a-f]{64})", line)
        if match is None or match.group(1) in files:
            raise CheckError("R26 ROCm SMI package manifest has a malformed row")
        files[match.group(1)] = match.group(2)
    if tuple(files) != expected_names:
        raise CheckError("R26 ROCm SMI package manifest has invalid membership")
    return files


def r26_parse_os_release(data: bytes) -> dict[str, str]:
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError("R26 retained os-release is not UTF-8") from error
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise CheckError("R26 retained os-release has a malformed row")
        key, encoded = line.split("=", 1)
        if re.fullmatch(r"[A-Z][A-Z0-9_]*", key) is None or key in fields:
            raise CheckError("R26 retained os-release has an invalid or duplicate key")
        try:
            decoded = shlex.split(encoded, posix=True)
        except ValueError as error:
            raise CheckError(f"R26 retained os-release has malformed {key}") from error
        if len(decoded) != 1:
            raise CheckError(f"R26 retained os-release has invalid {key}")
        fields[key] = decoded[0]
    if fields.get("ID") != "ubuntu" or fields.get("VERSION_ID") != "24.04":
        raise CheckError("R26 system identity does not retain Ubuntu 24.04")
    return fields


def r26_parse_build_note(note: bytes) -> str:
    if len(note) < 16:
        raise CheckError("R26 amdgpu GNU build-ID note is truncated")
    namesz, descsz, note_type = struct.unpack_from("<III", note)
    name_end = 12 + namesz
    desc_start = (name_end + 3) & ~3
    desc_end = desc_start + descsz
    padded_end = (desc_end + 3) & ~3
    if (
        namesz != 4
        or note_type != 3
        or note[12:name_end] != b"GNU\x00"
        or descsz < 8
        or descsz > 64
        or padded_end != len(note)
        or any(note[desc_end:padded_end])
    ):
        raise CheckError("R26 amdgpu GNU build-ID note is malformed")
    return note[desc_start:desc_end].hex()


def r26_validate_system_identity(
    identity: dict[str, str], context: dict[str, str]
) -> None:
    missing = R26_EXACT_SYSTEM_IDENTITY_FIELDS - identity.keys()
    unexpected = identity.keys() - R26_EXACT_SYSTEM_IDENTITY_FIELDS
    if missing or unexpected:
        detail = ",".join(sorted(missing or unexpected))
        qualifier = "missing" if missing else "unexpected"
        raise CheckError(f"R26 system identity has {qualifier} fields: {detail}")
    for field, expected in R26_FIXED_SYSTEM_IDENTITY.items():
        if identity[field] != expected:
            raise CheckError(f"R26 system identity has invalid {field}")
    if identity["execution_environment"] != context["execution_environment"]:
        raise CheckError(
            "R26 system identity execution environment does not match context"
        )
    if identity["observation_edge"] not in {"start", "end"}:
        raise CheckError("R26 system identity has invalid observation edge")

    if identity["gpu_index"] != context["gpu_index"]:
        raise CheckError(
            "R26 system identity GPU index does not match benchmark context"
        )
    if identity["unique_id"] != context["unique_id"]:
        raise CheckError(
            "R26 system identity unique ID does not match benchmark context"
        )
    unique_id = context["unique_id"].removeprefix("0x")
    if identity["pci_unique_id"] != unique_id:
        raise CheckError("R26 PCI sysfs unique ID does not match benchmark context")
    if identity["uuid"] != context["uuid"]:
        raise CheckError("R26 system identity UUID does not match benchmark context")
    if identity["gpu_serial"] != identity["pci_serial"]:
        raise CheckError("R26 product and PCI sysfs serial numbers differ")
    if not identity["gpu_serial"].isdigit() or identity["gpu_serial"] == "0":
        raise CheckError("R26 system identity GPU serial is not canonical")
    for field in ("gpu_guid", "gpu_node_id"):
        if not identity[field].isdigit():
            raise CheckError(f"R26 system identity {field} is not canonical")
    if identity["gpu_guid"] == "0":
        raise CheckError("R26 system identity GPU GUID is zero")
    if re.fullmatch(r"-?[0-9]+", identity["pci_numa_node"]) is None:
        raise CheckError("R26 system identity PCI NUMA node is not canonical")
    if CANONICAL_PCI_BDF.fullmatch(identity["pci_bdf"]) is None:
        raise CheckError("R26 system identity PCI BDF is not canonical")
    if CANONICAL_BOOT_ID.fullmatch(identity["boot_id"]) is None:
        raise CheckError("R26 system identity boot ID is not canonical")
    if re.fullmatch(r"[0-9][A-Za-z0-9._+-]*", identity["kernel_release"]) is None:
        raise CheckError("R26 system identity kernel release is not canonical")

    if identity["hsa_binary_sha256"] != context["hsa_binary_sha256"]:
        raise CheckError("R26 HSA binary identity does not match benchmark context")
    if identity["hip_binary_sha256"] != context["hip_binary_sha256"]:
        raise CheckError("R26 HIP binary identity does not match benchmark context")
    if identity["kfd_binary_sha256"] != context["kfd_binary_sha256"]:
        raise CheckError("R26 KFD binary identity does not match benchmark context")
    for field in sorted(field for field in identity if field.endswith("_sha256")):
        if CANONICAL_SHA256.fullmatch(identity[field]) is None:
            raise CheckError(f"R26 system identity {field} must be canonical")
    for field in (
        "amdgpu_build_id",
        "amdgpu_module_build_id",
        "hip_library_build_id",
        "hsa_library_build_id",
        "rocm_smi_interpreter_build_id",
        "rocm_smi_library_build_id",
    ):
        value = identity[field]
        if len(value) % 2 != 0 or CANONICAL_BUILD_ID.fullmatch(value) is None:
            raise CheckError(f"R26 system identity {field} must be canonical")
    if identity["amdgpu_module_build_id"] != identity["amdgpu_build_id"]:
        raise CheckError("R26 loaded and on-disk amdgpu build IDs differ")
    positive_integer(identity, "amdgpu_module_decompressed_bytes")

    product_name = r26_decode_base64(identity, "product_name_base64")
    product_series = r26_decode_base64(identity, "product_series_base64")
    if product_name != b"AMD Instinct MI300X OAM":
        raise CheckError("R26 system identity has invalid MI300X product name")
    if product_series != b"AMD Instinct MI300X":
        raise CheckError("R26 system identity has invalid MI300X product series")

    os_release = r26_validate_retained_blob(
        identity, "os_release_base64", "os_release_sha256"
    )
    r26_parse_os_release(os_release)
    kernel_version = r26_validate_retained_blob(
        identity, "kernel_version_base64", "kernel_version_sha256"
    )
    kernel_prefix = f"Linux version {identity['kernel_release']} ".encode()
    if not kernel_version.startswith(kernel_prefix):
        raise CheckError("R26 retained kernel version does not match kernel release")

    build_note = r26_validate_retained_blob(
        identity, "amdgpu_build_note_base64", "amdgpu_build_note_sha256"
    )
    if r26_parse_build_note(build_note) != identity["amdgpu_build_id"]:
        raise CheckError("R26 amdgpu build ID does not match its retained note")
    vermagic = r26_decode_base64(identity, "amdgpu_vermagic_base64")
    if not vermagic.startswith(f"{identity['kernel_release']} ".encode()):
        raise CheckError("R26 amdgpu vermagic does not match kernel release")
    if re.fullmatch(r"[0-9][A-Za-z0-9._+-]*", identity["amdgpu_version"]) is None:
        raise CheckError("R26 amdgpu version is not canonical")
    if re.fullmatch(r"[0-9A-F]+", identity["amdgpu_srcversion"]) is None:
        raise CheckError("R26 amdgpu srcversion is not canonical")
    if re.fullmatch(r"(?:none|[A-Z]+)", identity["amdgpu_taint"]) is None:
        raise CheckError("R26 amdgpu taint is not canonical")

    rocm_path = r26_decode_path(identity, "rocm_path_base64")
    hsa_path = r26_decode_path(identity, "hsa_library_path_base64")
    hip_path = r26_decode_path(identity, "hip_library_path_base64")
    module_path = r26_decode_path(identity, "amdgpu_module_path_base64")
    rocm_smi_invocation = r26_decode_path(identity, "rocm_smi_invocation_path_base64")
    rocm_smi_entrypoint = r26_decode_path(identity, "rocm_smi_entrypoint_path_base64")
    rocm_smi_interpreter_invocation = r26_decode_path(
        identity, "rocm_smi_interpreter_invocation_path_base64"
    )
    rocm_smi_interpreter = r26_decode_path(identity, "rocm_smi_interpreter_path_base64")
    rocm_smi_library = r26_decode_path(identity, "rocm_smi_library_path_base64")
    fixed_tool_paths = {
        "ldd_path_base64": pathlib.PurePosixPath("/usr/bin/ldd"),
        "modinfo_path_base64": pathlib.PurePosixPath("/usr/sbin/modinfo"),
        "readelf_path_base64": pathlib.PurePosixPath("/usr/bin/readelf"),
        "rocm_smi_interpreter_invocation_path_base64": pathlib.PurePosixPath(
            "/usr/bin/python3"
        ),
        "zstd_path_base64": pathlib.PurePosixPath("/usr/bin/zstd"),
    }
    for field, expected in fixed_tool_paths.items():
        if r26_decode_path(identity, field) != expected:
            raise CheckError(f"R26 system identity has invalid fixed tool path {field}")
    if rocm_path.name != f"rocm-{context['rocm_version']}":
        raise CheckError("R26 ROCm path does not match benchmark ROCm version")
    if rocm_smi_invocation != rocm_path / "bin" / "rocm-smi":
        raise CheckError("R26 ROCm SMI invocation path is outside its fixed location")
    if rocm_smi_entrypoint != rocm_path / "libexec" / "rocm_smi" / "rocm_smi.py":
        raise CheckError("R26 ROCm SMI entrypoint is outside its fixed package")
    if (
        rocm_smi_interpreter_invocation != pathlib.PurePosixPath("/usr/bin/python3")
        or rocm_smi_interpreter.parent != pathlib.PurePosixPath("/usr/bin")
        or re.fullmatch(r"python3(?:\.[0-9]+)?", rocm_smi_interpreter.name) is None
    ):
        raise CheckError("R26 ROCm SMI interpreter path is invalid")
    if (
        rocm_smi_library.parent != rocm_path / "lib"
        or re.fullmatch(r"librocm_smi64\.so\.1(?:\.[0-9]+)*", rocm_smi_library.name)
        is None
        or identity["rocm_smi_library_soname"] != "librocm_smi64.so.1"
    ):
        raise CheckError("R26 ROCm SMI library is outside its retained ROCm tree")
    if r26_decode_base64(identity, "rocm_smi_shebang_base64") != (
        b"#!/usr/bin/env python3\n"
    ):
        raise CheckError("R26 ROCm SMI shebang is invalid")
    package = r26_parse_rocm_smi_package(identity)
    if package["rocm_smi.py"] != identity["rocm_smi_entrypoint_sha256"]:
        raise CheckError("R26 ROCm SMI entrypoint does not match its package")
    for runtime_path in (hsa_path, hip_path):
        if runtime_path.parts[: len(rocm_path.parts)] != rocm_path.parts:
            raise CheckError("R26 runtime library is outside the retained ROCm tree")
    module_parts = module_path.parts
    if not any(
        module_parts[index : index + 2] == ("modules", identity["kernel_release"])
        for index in range(len(module_parts) - 1)
    ) or not module_path.name.startswith("amdgpu.ko"):
        raise CheckError("R26 amdgpu module path does not match kernel release")

    kfd_map = r26_parse_raw_ldd(identity, "kfd")
    hsa_map = r26_parse_raw_ldd(identity, "hsa")
    hip_map = r26_parse_raw_ldd(identity, "hip")
    rocm_smi_interpreter_map = r26_parse_raw_ldd(identity, "rocm_smi_interpreter")
    r26_parse_raw_ldd(identity, "rocm_smi_library")
    hsa_soname = identity["hsa_library_soname"]
    hip_soname = identity["hip_library_soname"]
    if any(FORBIDDEN_KFD_RUNTIME.fullmatch(soname) for soname in kfd_map):
        raise CheckError("R26 KFD loader map resolves an HSA or HIP runtime")
    if any(
        FORBIDDEN_KFD_RUNTIME.fullmatch(soname) for soname in rocm_smi_interpreter_map
    ):
        raise CheckError("R26 ROCm SMI interpreter resolves an HSA or HIP runtime")
    if hip_soname in hsa_map or hsa_map.get(hsa_soname) != hsa_path:
        raise CheckError("R26 raw HSA loader map has invalid runtime custody")
    if hip_map.get(hsa_soname) != hsa_path or hip_map.get(hip_soname) != hip_path:
        raise CheckError("R26 HIP loader map has invalid runtime custody")
    if hsa_path == hip_path:
        raise CheckError("R26 HSA and HIP runtimes resolve to the same file")

    product_snapshot = r26_validate_retained_blob(
        identity, "rocm_smi_identity_base64", "rocm_smi_identity_sha256"
    )
    try:
        product_text = product_snapshot.decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError("R26 retained product identity is not UTF-8") from error
    selected: dict[str, str] = {}
    pattern = re.compile(r"^GPU\[([0-9]+)\]\s*:\s*([^:]+):\s*(.*?)\s*$")
    for line in product_text.splitlines():
        match = pattern.fullmatch(line.strip())
        if match is None or match.group(1) != identity["gpu_index"]:
            continue
        key = " ".join(match.group(2).split())
        if key in selected:
            raise CheckError(f"R26 retained product identity duplicates {key}")
        selected[key] = match.group(3).strip()
    expected_product = {
        "Unique ID": identity["unique_id"],
        "Serial Number": identity["gpu_serial"],
        "PCI Bus": identity["pci_bdf"],
        "Card Series": "AMD Instinct MI300X",
        "Card Model": "0x74a1",
        "Card Vendor": "Advanced Micro Devices, Inc. [AMD/ATI]",
        "Card SKU": "M3000100",
        "Subsystem ID": "0x74a1",
        "Device Rev": "0x00",
        "Node ID": identity["gpu_node_id"],
        "GUID": identity["gpu_guid"],
        "GFX Version": "gfx942",
    }
    for field, expected in expected_product.items():
        actual = selected.get(field)
        if field in {
            "Unique ID",
            "PCI Bus",
            "Card Model",
            "Subsystem ID",
            "Device Rev",
        }:
            actual = actual.lower() if actual is not None else None
        if actual != expected:
            raise CheckError(f"R26 retained product identity has invalid {field}")


def r26_validate_telemetry(
    phases: list[dict[str, str]], context: dict[str, str]
) -> None:
    expected = {
        (backend, edge)
        for backend in ("kfd", "hsa", "hip")
        for edge in ("start", "end")
    }
    observed: set[tuple[str, str]] = set()
    for phase in phases:
        backend = phase.get("phase")
        if backend not in {"kfd", "hsa", "hip"}:
            raise CheckError(f"unexpected R26 phase {backend!r}")
        edges = [
            edge
            for edge in ("start", "end")
            if f"gpu_busy_{edge}_percent" in phase
            or f"telemetry_{edge}_sha256" in phase
            or f"telemetry_{edge}_base64" in phase
        ]
        if len(edges) != 1:
            raise CheckError("malformed R26 phase telemetry context")
        edge = edges[0]
        expected_fields = {
            "phase",
            f"gpu_busy_{edge}_percent",
            f"telemetry_{edge}_sha256",
            f"telemetry_{edge}_base64",
        }
        if set(phase) != expected_fields:
            unexpected = phase.keys() - expected_fields
            missing = expected_fields - phase.keys()
            detail = sorted(unexpected or missing)[0]
            qualifier = "unexpected" if unexpected else "missing"
            raise CheckError(f"R26 phase telemetry has {qualifier} field {detail}")
        key = (backend, edge)
        if key in observed:
            raise CheckError(f"duplicate R26 {backend} {edge} telemetry")
        observed.add(key)
        busy = positive_integer(phase, f"gpu_busy_{edge}_percent", allow_zero=True)
        if busy > int(context["max_busy_percent"]):
            raise CheckError(f"R26 {backend} {edge} load exceeds the context maximum")
        digest = phase.get(f"telemetry_{edge}_sha256", "")
        encoded = phase.get(f"telemetry_{edge}_base64", "")
        if CANONICAL_SHA256.fullmatch(digest) is None:
            raise CheckError("R26 telemetry digest must be canonical")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (binascii.Error, ValueError) as error:
            raise CheckError("R26 telemetry is not canonical base64") from error
        if not decoded or hashlib.sha256(decoded).hexdigest() != digest:
            raise CheckError(
                "R26 telemetry digest does not match its retained snapshot"
            )
        normalized = decoded.lower()
        gpu_marker = f"gpu[{context['gpu_index']}]".encode()
        selected_gpu = b"\n".join(
            line for line in normalized.splitlines() if gpu_marker in line
        )
        if (
            not selected_gpu
            or (b"clock" not in selected_gpu and b"sclk" not in selected_gpu)
            or b"power" not in selected_gpu
        ):
            raise CheckError(
                "R26 telemetry lacks the selected GPU clock/power evidence"
            )
        busy_values = re.findall(rb"gpu use \(%\):\s*([0-9]+)", selected_gpu)
        if len(busy_values) != 1 or int(busy_values[0]) != busy:
            raise CheckError(
                "R26 telemetry selected-GPU load does not match its phase field"
            )
    if observed != expected:
        missing = expected - observed
        extra = observed - expected
        detail = sorted(missing or extra)[0]
        qualifier = "missing" if missing else "unexpected"
        raise CheckError(f"{qualifier} R26 phase telemetry: {detail}")
    slot = int(context["counterbalance_slot"])
    expected_sequence = [
        (backend, edge)
        for backend in R26_COUNTERBALANCE_ORDERS[slot]
        for edge in ("start", "end")
    ]
    actual_sequence = []
    for phase in phases:
        edge = next(
            edge for edge in ("start", "end") if f"gpu_busy_{edge}_percent" in phase
        )
        actual_sequence.append((phase["phase"], edge))
    if actual_sequence != expected_sequence:
        raise CheckError("R26 telemetry order does not match its counterbalance slot")


def r26_canonical_record(
    prefix: str, fields: dict[str, str], ordered_fields: tuple[str, ...]
) -> str:
    return " ".join(
        (prefix,) + tuple(f"{field}={fields[field]}" for field in ordered_fields)
    )


def r26_parse_id_list(value: str, field: str) -> set[int]:
    result: set[int] = set()
    previous = -2
    for segment in value.split(","):
        bounds = segment.split("-", 1)
        if any(re.fullmatch(r"0|[1-9][0-9]*", bound) is None for bound in bounds):
            raise CheckError(f"R26 topology {field} is not a canonical ID list")
        start = int(bounds[0])
        end = int(bounds[-1])
        if start > end or end > 1_048_575 or (len(bounds) == 2 and start == end):
            raise CheckError(f"R26 topology {field} has an invalid ID range")
        if start <= previous + 1:
            raise CheckError(f"R26 topology {field} is not maximally coalesced")
        if len(result) + end - start + 1 > 65_536:
            raise CheckError(f"R26 topology {field} has excessive cardinality")
        result.update(range(start, end + 1))
        previous = end
    if not result:
        raise CheckError(f"R26 topology {field} is empty")
    return result


def r26_validate_topology_evidence(
    records: list[tuple[dict[str, str], str, int]],
    context: dict[str, str],
    system_identity: dict[str, str],
) -> str:
    slot = context["counterbalance_slot"]
    expected = {
        (backend, edge)
        for backend in ("kfd", "hsa", "hip")
        for edge in ("start", "end")
    }
    observed: set[tuple[str, str]] = set()
    identities: set[str] = set()
    for topology, line, line_number in records:
        if set(topology) != R26_EXACT_TOPOLOGY_FIELDS:
            difference = set(topology) ^ R26_EXACT_TOPOLOGY_FIELDS
            raise CheckError(
                f"line {line_number}: R26 topology has invalid field {sorted(difference)[0]}"
            )
        backend = topology["phase"]
        edge = topology["edge"]
        key = (backend, edge)
        if key not in expected:
            raise CheckError(f"line {line_number}: unexpected R26 topology boundary")
        if key in observed:
            raise CheckError(f"line {line_number}: duplicate R26 topology boundary")
        observed.add(key)
        if topology["slot"] != slot:
            raise CheckError(f"line {line_number}: R26 topology slot mismatch")
        canonical_inner = r26_canonical_record(
            "topology", topology, R26_TOPOLOGY_SEALED_FIELDS
        )
        digest = topology["topology_sha256"]
        if (
            CANONICAL_SHA256.fullmatch(digest) is None
            or hashlib.sha256((canonical_inner + "\n").encode()).hexdigest() != digest
        ):
            raise CheckError(f"line {line_number}: invalid R26 topology seal")
        expected_line = r26_canonical_record(
            "topology",
            topology,
            ("slot", "phase", "edge")
            + R26_TOPOLOGY_SEALED_FIELDS
            + ("topology_sha256",),
        )
        if line != expected_line:
            raise CheckError(f"line {line_number}: noncanonical R26 topology record")
        identities.add(f"{canonical_inner} topology_sha256={digest}")

        expected_identity = {
            "schema": R26_TOPOLOGY_SCHEMA,
            "placement": context["placement"],
            "gpu_index": context["gpu_index"],
            "pci_bdf": system_identity["pci_bdf"],
            "unique_id": context["unique_id"],
            "numa_node": system_identity["pci_numa_node"],
            "kfd_node": system_identity["gpu_node_id"],
            "kfd_gpu_id": system_identity["gpu_guid"],
            "topology_sha256": context["topology_sha256"],
        }
        for field, expected_value in expected_identity.items():
            if topology[field] != expected_value:
                raise CheckError(
                    f"line {line_number}: R26 topology does not match {field}"
                )
        kfd_gpu_id = positive_integer(topology, "kfd_gpu_id")
        if kfd_gpu_id >= 1 << 32:
            raise CheckError("R26 topology KFD GPU ID is out of range")
        positive_integer(topology, "kfd_node", allow_zero=True)
        numa_node = positive_integer(topology, "numa_node", allow_zero=True)
        observer_cpu = positive_integer(topology, "observer_cpu", allow_zero=True)
        local = r26_parse_id_list(
            topology["device_local_cpu_list"], "device_local_cpu_list"
        )
        allowed = r26_parse_id_list(topology["allowed_cpu_list"], "allowed_cpu_list")
        allowed_mem = r26_parse_id_list(
            topology["allowed_mem_node_list"], "allowed_mem_node_list"
        )
        measurement = r26_parse_id_list(
            topology["measurement_cpu_list"], "measurement_cpu_list"
        )
        if measurement - (local & allowed):
            raise CheckError("R26 topology measurement CPUs are not local and allowed")
        if observer_cpu not in allowed or observer_cpu in measurement:
            raise CheckError("R26 topology observer CPU is not disjoint and allowed")
        if numa_node not in allowed_mem:
            raise CheckError("R26 topology NUMA node is not allowed")
    if observed != expected:
        missing = sorted(expected - observed)[0]
        raise CheckError(f"missing R26 topology boundary: {missing}")
    if len(identities) != 1:
        raise CheckError("R26 topology changed within the counterbalance slot")
    return next(iter(identities))


def r26_validate_monitor_evidence(
    records: list[tuple[dict[str, str], str, int]],
    context: dict[str, str],
    topologies: list[tuple[dict[str, str], str, int]],
    row_lines: dict[str, str],
) -> None:
    slot = context["counterbalance_slot"]
    topology = topologies[0][0]
    observed: set[str] = set()
    for monitor, line, line_number in records:
        if set(monitor) != R26_EXACT_MONITOR_FIELDS:
            difference = set(monitor) ^ R26_EXACT_MONITOR_FIELDS
            raise CheckError(
                f"line {line_number}: R26 monitor has invalid field {sorted(difference)[0]}"
            )
        backend = monitor["phase"]
        if backend not in {"kfd", "hsa", "hip"}:
            raise CheckError(f"line {line_number}: unexpected R26 monitor phase")
        if backend in observed:
            raise CheckError(f"line {line_number}: duplicate R26 monitor for {backend}")
        observed.add(backend)
        if monitor["slot"] != slot:
            raise CheckError(f"line {line_number}: R26 monitor slot mismatch")
        canonical_inner = r26_canonical_record(
            "monitor", monitor, R26_MONITOR_SEALED_FIELDS
        )
        digest = monitor["monitor_sha256"]
        if (
            CANONICAL_SHA256.fullmatch(digest) is None
            or hashlib.sha256((canonical_inner + "\n").encode()).hexdigest() != digest
        ):
            raise CheckError(f"line {line_number}: invalid R26 monitor seal")
        expected_line = r26_canonical_record(
            "monitor",
            monitor,
            ("slot", "phase") + R26_MONITOR_SEALED_FIELDS + ("monitor_sha256",),
        )
        if line != expected_line:
            raise CheckError(f"line {line_number}: noncanonical R26 monitor record")
        fixed = {
            "schema": R26_MONITOR_SCHEMA,
            "status": "clean",
            "monitor": context["interference_monitor"],
            "schedule": "absolute-monotonic-raw-deadline-v1",
            "kfd_gpu_id": topology["kfd_gpu_id"],
            "observer_cpu": topology["observer_cpu"],
            "interval_us": context["monitor_interval_us"],
            "maximum_gap_us": context["monitor_maximum_gap_us"],
            "foreign_selected_queues": "0",
            "terminal_selected_queues": "0",
            "target_exit_code": "0",
            "target_reaped": "1",
            "process_group_absent": "1",
        }
        for field, expected_value in fixed.items():
            if monitor[field] != expected_value:
                raise CheckError(f"line {line_number}: invalid R26 monitor {field}")
        root_pid = positive_integer(monitor, "root_pid")
        process_group = positive_integer(monitor, "process_group")
        if process_group != root_pid:
            raise CheckError("R26 monitor process_group does not match root_pid")
        observations = positive_integer(monitor, "observations")
        if observations < 3:
            raise CheckError("R26 monitor requires at least three queue observations")
        positive_integer(monitor, "target_selected_queue_observations")
        observed_gap = positive_integer(monitor, "observed_maximum_gap_us")
        maximum_gap = positive_integer(monitor, "maximum_gap_us")
        if observed_gap > maximum_gap:
            raise CheckError("R26 monitor observation gap exceeds its maximum")
        target = (row_lines[backend] + "\n").encode()
        if positive_integer(monitor, "target_output_bytes") != len(target):
            raise CheckError("R26 monitor target byte count does not match its row")
        target_digest = monitor["target_output_sha256"]
        if (
            CANONICAL_SHA256.fullmatch(target_digest) is None
            or hashlib.sha256(target).hexdigest() != target_digest
        ):
            raise CheckError("R26 monitor target digest does not match its row")
    if observed != {"kfd", "hsa", "hip"}:
        missing = sorted({"kfd", "hsa", "hip"} - observed)[0]
        raise CheckError(f"missing R26 monitor for {missing}")


def _check_r26_inplace_rows(
    lines: Iterable[str],
) -> tuple[list[str], dict[str, str], dict[str, str], str]:
    context: dict[str, str] | None = None
    context_line_number: int | None = None
    system_identities: list[tuple[dict[str, str], str, int]] = []
    phases: list[dict[str, str]] = []
    topologies: list[tuple[dict[str, str], str, int]] = []
    monitors: list[tuple[dict[str, str], str, int]] = []
    rows: dict[str, dict[str, str]] = {}
    row_lines: dict[str, str] = {}
    row_order: list[str] = []
    evidence_order: list[tuple[str, str, str]] = []
    evidence_line_numbers: list[int] = []
    for line_number, line in enumerate(lines, 1):
        stripped = line.strip()
        if not stripped:
            raise CheckError(f"line {line_number}: empty R26 evidence line")
        tokens = stripped.split()
        first_field = 1 if tokens[0] in {"context", "topology", "monitor"} else 0
        if any("=" not in token for token in tokens[first_field:]):
            raise CheckError(f"line {line_number}: malformed R26 evidence token")
        fields = parse_fields(stripped, line_number)
        if stripped.startswith("context "):
            if "phase" in fields:
                phases.append(fields)
                evidence_line_numbers.append(line_number)
                edges = [
                    edge
                    for edge in ("start", "end")
                    if any(key.endswith(f"_{edge}_percent") for key in fields)
                ]
                evidence_order.append(
                    (
                        "telemetry",
                        fields.get("phase", ""),
                        edges[0] if len(edges) == 1 else "",
                    )
                )
            elif fields.get("schema") == R26_INPLACE_SCHEMA:
                if context is not None:
                    raise CheckError(f"line {line_number}: duplicate R26 context")
                context = fields
                context_line_number = line_number
            elif fields.get("schema") == R26_SYSTEM_IDENTITY_SCHEMA:
                canonical = " ".join(
                    ("context", f"schema={R26_SYSTEM_IDENTITY_SCHEMA}")
                    + tuple(
                        f"{key}={fields[key]}"
                        for key in sorted(fields.keys() - {"schema"})
                    )
                )
                if stripped != canonical:
                    raise CheckError(
                        f"line {line_number}: noncanonical R26 system identity"
                    )
                system_identities.append((fields, stripped, line_number))
            else:
                raise CheckError(f"line {line_number}: unexpected R26 context line")
            continue
        if stripped.startswith("topology "):
            topologies.append((fields, stripped, line_number))
            evidence_line_numbers.append(line_number)
            evidence_order.append(
                ("topology", fields.get("phase", ""), fields.get("edge", ""))
            )
            continue
        if stripped.startswith("monitor "):
            monitors.append((fields, stripped, line_number))
            evidence_line_numbers.append(line_number)
            evidence_order.append(("monitor", fields.get("phase", ""), ""))
            continue
        if fields.get("schema") != R26_INPLACE_SCHEMA or "backend" not in fields:
            raise CheckError(f"line {line_number}: unexpected R26 evidence line")
        backend = fields["backend"]
        if backend not in {"kfd", "hsa", "hip"}:
            raise CheckError(f"line {line_number}: unexpected backend {backend!r}")
        if backend in rows:
            raise CheckError(f"line {line_number}: duplicate {backend} R26 row")
        rows[backend] = fields
        row_lines[backend] = stripped
        row_order.append(backend)
        evidence_line_numbers.append(line_number)
        evidence_order.append(("row", backend, ""))

    if context is None:
        raise CheckError("missing R26 benchmark context")
    if len(system_identities) != 2:
        raise CheckError(
            "R26 evidence requires exactly two system identities (start and end)"
        )
    missing_backends = {"kfd", "hsa", "hip"} - rows.keys()
    if missing_backends:
        raise CheckError(
            f"R26 rows missing backends: {','.join(sorted(missing_backends))}"
        )
    unique_id, uuid = r26_validate_context(context)
    identities_by_edge: dict[str, dict[str, str]] = {}
    identity_lines_by_edge: dict[str, str] = {}
    for identity, identity_line, line_number in system_identities:
        r26_validate_system_identity(identity, context)
        edge = identity["observation_edge"]
        if edge in identities_by_edge:
            raise CheckError(
                f"line {line_number}: duplicate R26 {edge} system identity"
            )
        identities_by_edge[edge] = identity
        identity_lines_by_edge[edge] = identity_line
    if set(identities_by_edge) != {"start", "end"}:
        raise CheckError("R26 system identities must contain start and end edges")
    assert context_line_number is not None
    identity_line_numbers = {
        identity["observation_edge"]: line_number
        for identity, _, line_number in system_identities
    }
    if (
        not evidence_line_numbers
        or not context_line_number < identity_line_numbers["start"]
        or not identity_line_numbers["start"] < min(evidence_line_numbers)
        or not identity_line_numbers["end"] > max(evidence_line_numbers)
    ):
        raise CheckError(
            "R26 system identity edges do not enclose the measured evidence"
        )
    for field in R26_EXACT_SYSTEM_IDENTITY_FIELDS:
        if field in R26_EDGE_VARIANT_SYSTEM_IDENTITY_FIELDS:
            continue
        if identities_by_edge["start"][field] != identities_by_edge["end"][field]:
            raise CheckError(
                f"R26 start/end system identity has mismatched field {field}"
            )
    slot = int(context["counterbalance_slot"])
    if tuple(row_order) != R26_COUNTERBALANCE_ORDERS[slot]:
        raise CheckError("R26 row order does not match its counterbalance slot")

    raw_by_backend: dict[str, dict[str, list[int]]] = {}
    promotion_samples: list[int] | None = None
    for backend, row in rows.items():
        missing_fields = R26_EXACT_ROW_FIELDS - row.keys()
        if missing_fields:
            raise CheckError(
                f"backend {backend} is missing R26 fields: {','.join(sorted(missing_fields))}"
            )
        unexpected_fields = row.keys() - R26_EXACT_ROW_FIELDS
        if unexpected_fields:
            raise CheckError(
                f"backend {backend} has unexpected R26 fields: "
                f"{','.join(sorted(unexpected_fields))}"
            )
        for field, expected in R26_FIXED_ROW.items():
            if row.get(field) != expected:
                raise CheckError(f"backend {backend} has invalid R26 field {field}")
        if row["unique_id"] != unique_id or row["uuid"] != uuid:
            raise CheckError(
                f"backend {backend} does not match the exact context identity"
            )
        if backend == "kfd":
            expected_backend = {
                "promotion": "full-h2d-to-compute-ready",
                "data_path": "persistent-device-reused",
                "control_path": "persistent-control-replayed",
                "user_data_materializations": "0",
            }
        else:
            expected_backend = {
                "promotion": "n/a",
                "data_path": "host-staged-one-buffer",
                "control_path": "n/a",
                "user_data_materializations": "n/a",
            }
        for field, expected in expected_backend.items():
            if row.get(field) != expected:
                raise CheckError(f"backend {backend} has invalid R26 field {field}")

        phase_samples: dict[str, list[int]] = {}
        for phase in R26_PHASES:
            values = r26_raw_samples(row, phase)
            r26_validate_summaries(row, phase, values)
            phase_samples[phase] = values
        for index in range(30):
            phase_sum = sum(phase_samples[phase][index] for phase in R26_PHASES[:3])
            if phase_samples["e2e"][index] < phase_sum:
                raise CheckError(
                    f"backend {backend} E2E sample {index} is below its phase sum"
                )
        raw_by_backend[backend] = phase_samples
        if backend == "kfd":
            promotion_samples = r26_raw_samples(row, "promotion")
            r26_validate_summaries(row, "promotion", promotion_samples)
            for index, value in enumerate(promotion_samples):
                if value > phase_samples["h2d"][index]:
                    raise CheckError(
                        f"backend kfd promotion sample {index} exceeds inclusive H2D"
                    )
            launch_samples: dict[str, list[int]] = {}
            for phase in R26_LAUNCH_TIMING_PHASES:
                completed_readback = phase == "completed_readback"
                values = r26_raw_samples(row, phase, allow_zero=completed_readback)
                launch_samples[phase] = values
                r26_validate_summaries(
                    row, phase, values, allow_zero=completed_readback
                )
                if completed_readback and any(values):
                    raise CheckError(
                        "backend kfd completed_readback must be exactly zero "
                        "for persistent device execution"
                    )
                for index, value in enumerate(values):
                    if value > phase_samples["compute"][index]:
                        raise CheckError(
                            f"backend kfd {phase} sample {index} exceeds inclusive compute"
                        )
            for index in range(30):
                nested_preparation = r26_checked_sum(
                    (
                        launch_samples["bound_snapshot"][index],
                        launch_samples["authority"][index],
                    ),
                    "nested preparation",
                )
                if nested_preparation > launch_samples["preparation"][index]:
                    raise CheckError(
                        f"backend kfd nested preparation sample {index} exceeds "
                        "inclusive preparation"
                    )
                completion_recycle = r26_checked_sum(
                    (
                        launch_samples["completion_signal_recycle"][index],
                        launch_samples["completion_detach_restore"][index],
                    ),
                    "completion recycle components",
                )
                if completion_recycle != launch_samples["recycle_inclusive"][index]:
                    raise CheckError(
                        f"backend kfd completion recycle components sample {index} "
                        "do not equal inclusive recycle"
                    )
                critical_path = r26_checked_sum(
                    (
                        launch_samples["preparation"][index],
                        launch_samples["native_binding"][index],
                        launch_samples["publication"][index],
                        launch_samples["publish_to_completion"][index],
                        launch_samples["recycle_inclusive"][index],
                    ),
                    "launch critical-path",
                )
                if critical_path > phase_samples["compute"][index]:
                    raise CheckError(
                        f"backend kfd launch critical-path sample {index} exceeds "
                        "inclusive compute"
                    )
        else:
            unavailable_phases = ("promotion",) + R26_LAUNCH_TIMING_PHASES
            for field in tuple(
                field
                for phase in unavailable_phases
                for field in (f"{phase}_samples_ns",)
                + tuple(f"{phase}_{summary}_ns" for summary in R26_SUMMARIES)
            ):
                if row.get(field) != "n/a":
                    raise CheckError(f"backend {backend} field {field} must be n/a")

    r26_validate_telemetry(phases, context)
    topology_identity = r26_validate_topology_evidence(
        topologies, context, identities_by_edge["start"]
    )
    r26_validate_monitor_evidence(monitors, context, topologies, row_lines)
    expected_evidence_order = [
        evidence
        for backend in R26_COUNTERBALANCE_ORDERS[slot]
        for evidence in (
            ("topology", backend, "start"),
            ("telemetry", backend, "start"),
            ("monitor", backend, ""),
            ("telemetry", backend, "end"),
            ("topology", backend, "end"),
            ("row", backend, ""),
        )
    ]
    if evidence_order != expected_evidence_order:
        raise CheckError("R26 evidence order does not prove guarded row release")
    assert promotion_samples is not None
    output: list[str] = []
    for phase in R26_PHASES:
        kfd_p50 = r26_p50(raw_by_backend["kfd"][phase])
        for reference in ("hsa", "hip"):
            reference_p50 = r26_p50(raw_by_backend[reference][phase])
            ratio = Decimal(kfd_p50) / Decimal(reference_p50)
            output.append(
                " ".join(
                    (
                        f"schema={R26_INPLACE_SCHEMA}",
                        "comparison=kfd-over-reference",
                        f"reference={reference}",
                        f"phase={phase}",
                        "statistic=p50_ns",
                        f"kfd_over_reference_p50_ratio={ratio:.6f}",
                        "lower_is_better=true",
                        "evidence_only=true",
                    )
                )
            )
    authentication_share = Decimal(r26_p50(promotion_samples)) / Decimal(
        r26_p50(raw_by_backend["kfd"]["h2d"])
    )
    output.append(
        " ".join(
            (
                f"schema={R26_INPLACE_SCHEMA}",
                "comparison=promotion-authentication-share",
                "phase=promotion-over-h2d",
                "statistic=p50_ns",
                f"promotion_over_h2d_p50_ratio={authentication_share:.6f}",
                "lower_is_better=true",
                "evidence_only=true",
            )
        )
    )
    for phase in R26_LAUNCH_TIMING_PHASES:
        values = r26_raw_samples(
            rows["kfd"], phase, allow_zero=phase == "completed_readback"
        )
        output.append(
            " ".join(
                (
                    f"schema={R26_INPLACE_SCHEMA}",
                    "comparison=kfd-host-launch-timing",
                    f"phase={phase}",
                    "statistic=p50_ns",
                    f"value={r26_p50(values)}",
                    "evidence_only=true",
                )
            )
        )
    output.append("validation_status=pass")
    return output, context, identity_lines_by_edge, topology_identity


def check_r26_inplace_rows(lines: Iterable[str]) -> list[str]:
    output, _, _, _ = _check_r26_inplace_rows(lines)
    return output


def r26_materialize_log(lines: Iterable[str]) -> tuple[list[str], bytes]:
    materialized = list(lines)
    if not materialized:
        raise CheckError("empty R26 slot log")
    if any("\x00" in line or "\r" in line for line in materialized):
        raise CheckError("R26 slot logs must be NUL-free LF-terminated UTF-8 text")
    terminated = [line.endswith("\n") for line in materialized]
    if all(terminated):
        if any("\n" in line[:-1] for line in materialized):
            raise CheckError(
                "R26 slot log iterator must yield exactly one line at a time"
            )
        encoded = "".join(materialized).encode("utf-8")
    elif not any(terminated) and all("\n" not in line for line in materialized):
        encoded = ("\n".join(materialized) + "\n").encode("utf-8")
    else:
        raise CheckError("R26 slot log has mixed or malformed line termination")
    return materialized, encoded


def r26_manifest_payload(set_id: str, slot_hashes: dict[int, str]) -> bytes:
    if CANONICAL_SHA256.fullmatch(set_id) is None:
        raise CheckError("R26 manifest counterbalance set ID must be canonical")
    if set(slot_hashes) != set(R26_COUNTERBALANCE_ORDERS):
        raise CheckError("R26 manifest requires exact slot hashes 0, 1, and 2")
    if any(
        CANONICAL_SHA256.fullmatch(digest) is None for digest in slot_hashes.values()
    ):
        raise CheckError("R26 manifest slot hashes must be canonical")
    return (
        " ".join(
            (
                f"schema={R26_MANIFEST_SCHEMA}",
                f"counterbalance_set_id={set_id}",
                f"slot_0_sha256={slot_hashes[0]}",
                f"slot_1_sha256={slot_hashes[1]}",
                f"slot_2_sha256={slot_hashes[2]}",
            )
        )
        + "\n"
    ).encode("ascii")


def check_r26_counterbalance_set(logs: Iterable[Iterable[str]]) -> list[str]:
    contexts: dict[int, dict[str, str]] = {}
    system_identity_lines: dict[int, dict[str, str]] = {}
    topology_identity_lines: dict[int, str] = {}
    performance_lines: dict[int, list[str]] = {}
    slot_hashes: dict[int, str] = {}
    for lines in logs:
        materialized_lines, exact_bytes = r26_materialize_log(lines)
        slot_output, context, system_identity_line, topology_identity_line = (
            _check_r26_inplace_rows(materialized_lines)
        )
        slot = int(context["counterbalance_slot"])
        if slot in contexts:
            raise CheckError(f"duplicate R26 counterbalance slot {slot}")
        contexts[slot] = context
        system_identity_lines[slot] = system_identity_line
        topology_identity_lines[slot] = topology_identity_line
        performance_lines[slot] = slot_output
        slot_hashes[slot] = hashlib.sha256(exact_bytes).hexdigest()
    expected_slots = set(R26_COUNTERBALANCE_ORDERS)
    if set(contexts) != expected_slots:
        missing = expected_slots - contexts.keys()
        extra = contexts.keys() - expected_slots
        detail = sorted(missing or extra)[0]
        qualifier = "missing" if missing else "unexpected"
        raise CheckError(f"{qualifier} R26 counterbalance slot {detail}")

    baseline = contexts[0]
    baseline_topology_identity = topology_identity_lines[0]
    varying = {"counterbalance_slot", "backend_order", "topology_sha256"}
    for slot, context in contexts.items():
        for field in R26_CONTEXT_FIELDS:
            if field not in varying and context[field] != baseline[field]:
                raise CheckError(
                    f"R26 counterbalance slot {slot} has mismatched context field {field}"
                )
        for edge in ("start", "end"):
            if system_identity_lines[slot][edge] != system_identity_lines[0][edge]:
                raise CheckError(
                    f"R26 counterbalance slot {slot} has mismatched {edge} "
                    "system identity"
                )
        if topology_identity_lines[slot] != baseline_topology_identity:
            raise CheckError(
                f"R26 counterbalance slot {slot} has mismatched host topology"
            )
    manifest_sha256 = hashlib.sha256(
        r26_manifest_payload(baseline["counterbalance_set_id"], slot_hashes)
    ).hexdigest()
    output: list[str] = []
    for slot in sorted(performance_lines):
        for line in performance_lines[slot]:
            if line == "validation_status=pass":
                output.append(
                    f"schema={R26_INPLACE_SCHEMA} counterbalance_slot={slot} "
                    "slot_validation_status=pass"
                )
                continue
            schema, separator, remainder = line.partition(" ")
            if schema != f"schema={R26_INPLACE_SCHEMA}" or not separator:
                raise CheckError("R26 slot comparison output is noncanonical")
            output.append(f"{schema} counterbalance_slot={slot} {remainder}")
    output.append(
        " ".join(
            (
                f"schema={R26_INPLACE_SCHEMA}",
                f"counterbalance_design={R26_COUNTERBALANCE_DESIGN}",
                "counterbalance_slots=3",
                f"counterbalance_set_id={baseline['counterbalance_set_id']}",
                f"slot_0_sha256={slot_hashes[0]}",
                f"slot_1_sha256={slot_hashes[1]}",
                f"slot_2_sha256={slot_hashes[2]}",
                f"manifest_schema={R26_MANIFEST_SCHEMA}",
                f"manifest_sha256={manifest_sha256}",
                "raw_samples_per_backend_per_slot=30",
                "aggregation=none",
                "claim=evidence-only",
                "set_validation_status=pass",
            )
        )
    )
    return output


def validate_striped_kfd_copy_row(row: dict[str, str], profile: str) -> None:
    match = STRIPED_KFD_PROFILE.fullmatch(profile)
    if match is None:
        return
    depth = positive_integer(row, "depth")
    configured = positive_integer(row, "configured_queues")
    expected_configured = int(match.group(1))
    if configured != expected_configured:
        raise CheckError("KFD striped row configured_queues does not match its profile")
    concurrency = min(configured, depth)
    if positive_integer(row, "concurrency") != concurrency:
        raise CheckError(
            "KFD striped row concurrency does not match depth and queue count"
        )
    if positive_integer(row, "doorbells_per_batch") != concurrency:
        raise CheckError(
            "KFD striped row doorbell count does not match its concurrency"
        )
    if positive_integer(row, "queue_depth") != (depth + concurrency - 1) // concurrency:
        raise CheckError(
            "KFD striped row queue depth is not the balanced shard ceiling"
        )
    if positive_integer(row, "batch_size") != depth:
        raise CheckError("KFD striped row batch size does not match depth")
    if row.get("direction") != "h2d-then-d2h":
        raise CheckError("KFD striped row has an unsupported direction methodology")
    if any(field.startswith("combined_") for field in row):
        raise CheckError(
            "KFD striped row must not claim a single combined currentness envelope"
        )


def validate_d2d_copy_rows(group: dict[str, dict[str, str]]) -> None:
    for backend, row in group.items():
        if row.get("device_index") != "0":
            raise CheckError(f"D2D {backend} row must use visible device index zero")
        if not row.get("target", "").startswith("gfx942"):
            raise CheckError(f"D2D {backend} row must report a gfx942 target")
        if row.get("xnack") != "disabled":
            raise CheckError(f"D2D {backend} row must report XNACK disabled")
        if positive_integer(row, "depth") != 1:
            raise CheckError(f"D2D {backend} row must use depth one")

    kfd = group["kfd"]
    copy_bytes = positive_integer(kfd, "bytes")
    if not (
        GFX942_D2D_MIN_QUALIFICATION_BYTES
        <= copy_bytes
        <= GFX942_D2D_MAX_QUALIFICATION_BYTES
    ):
        raise CheckError("D2D row does not exercise the R23 cross-window envelope")
    packet_count = (
        copy_bytes + GFX942_SDMA_MAX_LINEAR_COPY_BYTES - 1
    ) // GFX942_SDMA_MAX_LINEAR_COPY_BYTES
    window_count = (
        packet_count + GFX942_D2D_MAX_WINDOW_PACKETS - 1
    ) // GFX942_D2D_MAX_WINDOW_PACKETS
    expected = {
        "packet_count": str(packet_count),
        "window_count": str(window_count),
        "doorbells_per_copy": str(window_count),
        "max_packets_per_window": str(GFX942_D2D_MAX_WINDOW_PACKETS),
        "validation": "full-source-and-destination-every-round",
        "teardown": "explicit",
        "progress": "explicit-flush-then-wait",
        "timing": "facade-enqueue-flush-through-observed-completion",
    }
    for field, value in expected.items():
        if kfd.get(field) != value:
            raise CheckError(f"KFD D2D row has invalid {field} methodology")

    for backend, row in group.items():
        p50 = positive_number(row, "d2d_p50_ns")
        p95 = positive_number(row, "d2d_p95_ns")
        if p95 < p50:
            raise CheckError(f"D2D {backend} p95 latency is below p50 latency")
        bandwidth = positive_number(row, "d2d_p50_GBps")
        expected_bandwidth = Decimal(copy_bytes) / p50
        if abs(bandwidth - expected_bandwidth) > D2D_BANDWIDTH_ROUNDING_TOLERANCE:
            raise CheckError(
                f"D2D {backend} bandwidth is inconsistent with bytes and p50"
            )


def validate_context(context: dict[str, str], schema: str) -> tuple[str, ...]:
    missing = (
        set(COMMON_CONTEXT_FIELDS + SCHEMA_CONTEXT_FIELDS[schema]) - context.keys()
    )
    if missing:
        raise CheckError(
            f"benchmark context is missing fields: {','.join(sorted(missing))}"
        )
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
    if (
        not depths
        or len(set(depths)) != len(depths)
        or any(not depth.isdigit() or depth == "0" for depth in depths)
    ):
        raise CheckError("context depths must contain distinct positive integers")
    if schema != "fe2o3.xgmi-peer-benchmark.v1":
        if CANONICAL_SHA256.fullmatch(context["sdma_manifest_sha256"]) is None:
            raise CheckError("context sdma_manifest_sha256 must be canonical")
        if not admitted_kfd_profile(context["kfd_profile"]):
            raise CheckError("context kfd_profile is unsupported")
        if (
            schema
            in {
                "fe2o3.async-copy-benchmark.v1",
                "fe2o3.async-copy-multi-device-benchmark.v1",
            }
            and context.get("kfd_multi_profile", LEGACY_KFD_MULTI_PROFILE)
            != "directional"
        ):
            raise CheckError("context kfd_multi_profile must be directional")
        if schema == "fe2o3.d2d-copy-benchmark.v1":
            if context["sdma_manifest_sha256"] != GFX942_SDMA_MANIFEST_SHA256:
                raise CheckError("D2D context has the wrong SDMA manifest identity")
            if (
                context["d2d_window_manifest_sha256"]
                != GFX942_D2D_WINDOW_MANIFEST_SHA256
            ):
                raise CheckError("D2D context has the wrong window manifest identity")
            if (
                context["kfd_profile"] != "same-device-d2d"
                or context["timing"] != "submit-through-observed-completion"
                or context["setup_validation"] != "outside-timing"
                or context["measurement"] != "runtime-facade-r23-d2d-window"
            ):
                raise CheckError("D2D context has an unsupported timing methodology")
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
    device_count = (
        1
        if schema
        in {
            "fe2o3.async-copy-benchmark.v1",
            "fe2o3.d2d-copy-benchmark.v1",
        }
        else 2
    )
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
        raise CheckError(
            f"unexpected phase load context: {'/'.join(sorted(unexpected)[0])}"
        )


def comparison_key(row: dict[str, str], schema: str) -> tuple[str, str]:
    try:
        byte_count = row["bytes"]
        depth = row["depth_per_device" if "multi-device" in schema else "depth"]
    except KeyError as error:
        raise CheckError(
            f"backend {row.get('backend', '?')} lacks the matched size/depth coordinates"
        ) from error
    if (
        not byte_count.isdigit()
        or not depth.isdigit()
        or byte_count == "0"
        or depth == "0"
    ):
        raise CheckError("matched size/depth coordinates must be positive integers")
    return byte_count, depth


def check_rows(
    lines: Iterable[str],
    schema: str,
    max_latency_ratio: Decimal | float | str,
    min_bandwidth_ratio: Decimal | float | str,
) -> list[str]:
    if schema == R26_INPLACE_SCHEMA:
        return check_r26_inplace_rows(lines)
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
    if schema == "fe2o3.xgmi-peer-benchmark.v1" and set(xgmi_diagnostics) != set(
        groups
    ):
        raise CheckError(
            "XGMI evidence requires one remap diagnostic per persistent-hot row"
        )

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
        if schema in {
            "fe2o3.async-copy-benchmark.v1",
            "fe2o3.d2d-copy-benchmark.v1",
        }:
            if kfd.get("profile") != context["kfd_profile"]:
                raise CheckError(
                    "KFD copy row profile does not match benchmark context"
                )
            if schema == "fe2o3.async-copy-benchmark.v1":
                validate_striped_kfd_copy_row(kfd, context["kfd_profile"])
            else:
                validate_d2d_copy_rows(group)
        elif schema == "fe2o3.async-copy-multi-device-benchmark.v1":
            if (
                context.get("kfd_multi_profile", LEGACY_KFD_MULTI_PROFILE)
                != "directional"
            ):
                raise CheckError(
                    "multi-device KFD copy requires the directional profile"
                )
        else:
            validate_xgmi_kfd_measurement(kfd, "persistent-hot")
            diagnostic = xgmi_diagnostics[key]
            validate_xgmi_kfd_measurement(diagnostic, "remap-per-round")
            if matched_methodology(diagnostic, schema) != kfd_methodology:
                raise CheckError(
                    "KFD XGMI diagnostic does not match persistent methodology"
                )
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
            if (
                row["warmups"] != context["warmups"]
                or row["samples"] != context["samples"]
            ):
                raise CheckError(f"backend {backend} does not match context statistics")
            row_ids = (
                (row["unique_id"],)
                if schema
                in {
                    "fe2o3.async-copy-benchmark.v1",
                    "fe2o3.d2d-copy-benchmark.v1",
                }
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
    parser.add_argument("input", nargs="+", type=pathlib.Path)
    parser.add_argument(
        "--schema", required=True, choices=tuple(SCHEMA_METRICS) + (R26_INPLACE_SCHEMA,)
    )
    parser.add_argument("--r26-counterbalance-set", action="store_true")
    parser.add_argument("--max-latency-ratio", type=Decimal)
    parser.add_argument("--min-bandwidth-ratio", type=Decimal)
    arguments = parser.parse_args()
    if arguments.r26_counterbalance_set:
        if arguments.schema != R26_INPLACE_SCHEMA:
            parser.error("--r26-counterbalance-set requires the R26 schema")
        if len(arguments.input) != len(R26_COUNTERBALANCE_ORDERS):
            parser.error("--r26-counterbalance-set requires exactly three slot logs")
    elif len(arguments.input) != 1:
        parser.error("exactly one input log is required without a set check")
    if arguments.schema == R26_INPLACE_SCHEMA and (
        arguments.max_latency_ratio is not None
        or arguments.min_bandwidth_ratio is not None
    ):
        parser.error("R26 is evidence-only and does not accept parity thresholds")
    if arguments.schema != R26_INPLACE_SCHEMA and (
        arguments.max_latency_ratio is None or arguments.min_bandwidth_ratio is None
    ):
        parser.error(
            "--max-latency-ratio and --min-bandwidth-ratio are required for parity schemas"
        )
    try:
        if arguments.r26_counterbalance_set:
            logs = []
            for path in arguments.input:
                data = path.read_bytes()
                if (
                    not data
                    or not data.endswith(b"\n")
                    or b"\r" in data
                    or b"\x00" in data
                ):
                    raise CheckError(
                        f"R26 slot log {path} must be NUL-free LF-terminated UTF-8 text"
                    )
                try:
                    text = data.decode("utf-8")
                except UnicodeDecodeError as error:
                    raise CheckError(f"R26 slot log {path} is not UTF-8") from error
                logs.append(text.splitlines(keepends=True))
            output = check_r26_counterbalance_set(logs)
        else:
            with arguments.input[0].open(encoding="utf-8") as input_file:
                output = check_rows(
                    input_file,
                    arguments.schema,
                    Decimal(1)
                    if arguments.max_latency_ratio is None
                    else arguments.max_latency_ratio,
                    Decimal(1)
                    if arguments.min_bandwidth_ratio is None
                    else arguments.min_bandwidth_ratio,
                )
    except (CheckError, OSError) as error:
        print(error, file=sys.stderr)
        return 1
    print("\n".join(output))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
