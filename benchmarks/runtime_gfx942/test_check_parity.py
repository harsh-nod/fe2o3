#!/usr/bin/env python3

from __future__ import annotations

import base64
import hashlib
import importlib.util
import pathlib
import struct
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from decimal import Decimal
from io import StringIO
from unittest import mock


CHECKER_PATH = pathlib.Path(__file__).with_name("check-parity.py")
SPEC = importlib.util.spec_from_file_location("fe2o3_check_parity", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


def summary_fields(phase: str, values: list[int]) -> dict[str, str]:
    sorted_values = sorted(values)
    return {
        f"{phase}_samples_ns": ",".join(str(value) for value in values),
        f"{phase}_min_ns": str(sorted_values[0]),
        f"{phase}_mean_ns": str(sum(values) // len(values)),
        f"{phase}_max_ns": str(sorted_values[-1]),
        f"{phase}_p50_ns": str(sorted_values[(len(values) * 50 + 99) // 100 - 1]),
        f"{phase}_p95_ns": str(sorted_values[(len(values) * 95 + 99) // 100 - 1]),
    }


def valid_launch_timing_values(phase: str) -> list[int]:
    if phase == "completed_readback":
        return [0] * 30
    if phase == "completion_signal_recycle":
        return [60 + index for index in range(30)]
    if phase == "completion_detach_restore":
        return [40] * 30
    base = {
        "preparation": 250,
        "bound_snapshot": 40,
        "authority": 50,
    }.get(phase, 100)
    return [base + index for index in range(30)]


def render(prefix: str, fields: dict[str, str]) -> str:
    return prefix + " " + " ".join(f"{key}={value}" for key, value in fields.items())


def sealed_record(prefix: str, fields: dict[str, str], digest_field: str) -> str:
    payload = render(prefix, fields) + "\n"
    return render(
        prefix,
        {**fields, digest_field: hashlib.sha256(payload.encode()).hexdigest()},
    )


def valid_topology(
    slot: int, backend: str, edge: str, *, gpu_index: int = 0
) -> tuple[str, str]:
    inner = {
        "schema": CHECKER.R26_TOPOLOGY_SCHEMA,
        "placement": CHECKER.R26_FIXED_CONTEXT["placement"],
        "gpu_index": str(gpu_index),
        "pci_bdf": "0000:05:00.0",
        "unique_id": "0x0123456789abcdef",
        "numa_node": "0",
        "device_local_cpu_list": "0-47",
        "allowed_cpu_list": "0-95",
        "allowed_mem_node_list": "0-1",
        "measurement_cpu_list": "0-47",
        "observer_cpu": "48",
        "kfd_node": "2",
        "kfd_gpu_id": "28851",
    }
    sealed = sealed_record("topology", inner, "topology_sha256")
    digest = CHECKER.parse_fields(sealed, 0)["topology_sha256"]
    return (
        render(
            "topology",
            {
                "slot": str(slot),
                "phase": backend,
                "edge": edge,
                **CHECKER.parse_fields(sealed, 0),
            },
        ),
        digest,
    )


def valid_monitor(slot: int, backend: str, row: str) -> str:
    target = (row + "\n").encode()
    inner = {
        "schema": "fe2o3.r26-kfd-queue-monitor.v2",
        "status": "clean",
        "monitor": "selected-kfd-gpu-process-tree-census-v2",
        "schedule": "absolute-monotonic-raw-deadline-v1",
        "kfd_gpu_id": "28851",
        "root_pid": str(1000 + slot * 10 + ("kfd", "hsa", "hip").index(backend)),
        "process_group": str(1000 + slot * 10 + ("kfd", "hsa", "hip").index(backend)),
        "observer_cpu": "48",
        "interval_us": "2000",
        "maximum_gap_us": "10000",
        "observed_maximum_gap_us": "2500",
        "observations": "100",
        "target_selected_queue_observations": "99",
        "foreign_selected_queues": "0",
        "terminal_selected_queues": "0",
        "target_exit_code": "0",
        "target_reaped": "1",
        "process_group_absent": "1",
        "target_output_bytes": str(len(target)),
        "target_output_sha256": hashlib.sha256(target).hexdigest(),
    }
    sealed = sealed_record("monitor", inner, "monitor_sha256")
    return render(
        "monitor",
        {
            "slot": str(slot),
            "phase": backend,
            **CHECKER.parse_fields(sealed, 0),
        },
    )


def rewrite_guard_record(line: str, prefix: str, **updates: str) -> str:
    fields = CHECKER.parse_fields(line, 0)
    fields.update(updates)
    if prefix == "topology":
        outer = ("slot", "phase", "edge")
        sealed_fields = CHECKER.R26_TOPOLOGY_SEALED_FIELDS
        digest_field = "topology_sha256"
    else:
        outer = ("slot", "phase")
        sealed_fields = CHECKER.R26_MONITOR_SEALED_FIELDS
        digest_field = "monitor_sha256"
    payload = CHECKER.r26_canonical_record(prefix, fields, sealed_fields) + "\n"
    fields[digest_field] = hashlib.sha256(payload.encode()).hexdigest()
    return CHECKER.r26_canonical_record(
        prefix, fields, outer + sealed_fields + (digest_field,)
    )


def encode(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def render_system_identity(fields: dict[str, str]) -> str:
    return " ".join(
        ("context", f"schema={fields['schema']}")
        + tuple(f"{key}={fields[key]}" for key in sorted(fields.keys() - {"schema"}))
    )


def valid_system_identity(
    edge: str = "start", *, address_seed: int = 1, gpu_index: int = 0
) -> str:
    kernel_release = "6.8.0-124-generic"
    unique_id = "0123456789abcdef"
    hsa_path = "/opt/rocm-7.2.0/lib/libhsa-runtime64.so.1.18.70200"
    hip_path = "/opt/rocm-7.2.0/lib/libamdhip64.so.7.2.70200"
    hsa_observed = "/opt/rocm-7.2.0/lib/libhsa-runtime64.so.1"
    hip_observed = "/opt/rocm-7.2.0/lib/libamdhip64.so.7"
    libc_path = "/usr/lib/x86_64-linux-gnu/libc.so.6"
    hsa_loader_map = (
        f"libc.so.6={libc_path}\nlibhsa-runtime64.so.1={hsa_path}\n"
    ).encode()
    hip_loader_map = (
        f"libamdhip64.so.7={hip_path}\n"
        f"libc.so.6={libc_path}\n"
        f"libhsa-runtime64.so.1={hsa_path}\n"
    ).encode()
    kfd_loader_map = f"libc.so.6={libc_path}\n".encode()
    raw_address = address_seed * 0x1000

    def raw_ldd(entries: tuple[tuple[str, str], ...]) -> bytes:
        rows = [f"linux-vdso.so.1 (0x{raw_address:x})"]
        rows.extend(
            f"{soname} => {path} (0x{raw_address + index * 0x1000:x})"
            for index, (soname, path) in enumerate(entries, 1)
        )
        return ("\n".join(rows) + "\n").encode()

    def loader_resolution(
        observed_entries: tuple[tuple[str, str], ...],
        canonical_entries: dict[str, str],
    ) -> bytes:
        return "".join(
            f"soname={soname}\tobserved={observed}\t"
            f"resolved={canonical_entries[soname]}\n"
            for soname, observed in sorted(observed_entries)
        ).encode()

    kfd_entries = (("libc.so.6", libc_path),)
    hsa_entries = (("libc.so.6", libc_path), ("libhsa-runtime64.so.1", hsa_observed))
    hip_entries = (
        ("libamdhip64.so.7", hip_observed),
        ("libc.so.6", libc_path),
        ("libhsa-runtime64.so.1", hsa_observed),
    )
    kfd_ldd = raw_ldd(kfd_entries)
    hsa_ldd = raw_ldd(hsa_entries)
    hip_ldd = raw_ldd(hip_entries)
    kfd_loader_resolution = loader_resolution(kfd_entries, {"libc.so.6": libc_path})
    hsa_loader_resolution = loader_resolution(
        hsa_entries,
        {"libc.so.6": libc_path, "libhsa-runtime64.so.1": hsa_path},
    )
    hip_loader_resolution = loader_resolution(
        hip_entries,
        {
            "libamdhip64.so.7": hip_path,
            "libc.so.6": libc_path,
            "libhsa-runtime64.so.1": hsa_path,
        },
    )
    python_path = "/usr/bin/python3.12"
    python_entries = (("libc.so.6", libc_path),)
    python_ldd = raw_ldd(python_entries)
    python_loader_map = f"libc.so.6={libc_path}\n".encode()
    python_loader_resolution = loader_resolution(
        python_entries, {"libc.so.6": libc_path}
    )
    rocm_smi_library_path = "/opt/rocm-7.2.0/lib/librocm_smi64.so.1.0"
    rocm_smi_library_entries = (("libc.so.6", libc_path),)
    rocm_smi_library_ldd = raw_ldd(rocm_smi_library_entries)
    rocm_smi_library_loader_map = f"libc.so.6={libc_path}\n".encode()
    rocm_smi_library_loader_resolution = loader_resolution(
        rocm_smi_library_entries, {"libc.so.6": libc_path}
    )
    rocm_smi_entrypoint = b"#!/usr/bin/env python3\n# retained fixture\n"
    rocm_smi_package = (
        f"file=rocm_smi.py\tsha256={hashlib.sha256(rocm_smi_entrypoint).hexdigest()}\n"
        f"file=rsmiBindings.py\tsha256={'8' * 64}\n"
        f"file=rsmiBindingsInit.py\tsha256={'9' * 64}\n"
    ).encode()
    build_id = "ab" * 20
    build_descriptor = bytes.fromhex(build_id)
    build_note = (
        struct.pack("<III", 4, len(build_descriptor), 3) + b"GNU\x00" + build_descriptor
    )
    os_release = b'ID=ubuntu\nVERSION_ID="24.04"\nPRETTY_NAME="Ubuntu 24.04 LTS"\n'
    kernel_version = f"Linux version {kernel_release} (builder@test) #1 SMP\n".encode()
    product_snapshot = f"""\
GPU[{gpu_index}] : Unique ID: 0x{unique_id}
GPU[{gpu_index}] : Serial Number: 692424017146
GPU[{gpu_index}] : PCI Bus: 0000:05:00.0
GPU[{gpu_index}] : Card Series: AMD Instinct MI300X
GPU[{gpu_index}] : Card Model: 0x74a1
GPU[{gpu_index}] : Card Vendor: Advanced Micro Devices, Inc. [AMD/ATI]
GPU[{gpu_index}] : Card SKU: M3000100
GPU[{gpu_index}] : Subsystem ID: 0x74a1
GPU[{gpu_index}] : Device Rev: 0x00
GPU[{gpu_index}] : Node ID: 2
GPU[{gpu_index}] : GUID: 28851
GPU[{gpu_index}] : GFX Version: gfx942
""".encode()
    fields = {
        "schema": CHECKER.R26_SYSTEM_IDENTITY_SCHEMA,
        "amdgpu_build_id": build_id,
        "amdgpu_build_note_base64": encode(build_note),
        "amdgpu_build_note_sha256": hashlib.sha256(build_note).hexdigest(),
        "amdgpu_module_build_id": build_id,
        "amdgpu_module_decompressed_bytes": "123456",
        "amdgpu_module_decompressed_sha256": "d" * 64,
        "amdgpu_module_path_base64": encode(
            f"/usr/lib/modules/{kernel_release}/updates/dkms/amdgpu.ko.zst".encode()
        ),
        "amdgpu_module_sha256": "a" * 64,
        "amdgpu_srcversion": "A6F143BEC60C0AFC3263226",
        "amdgpu_taint": "OE",
        "amdgpu_vermagic_base64": encode(f"{kernel_release} SMP mod_unload".encode()),
        "amdgpu_version": "6.16.13",
        "boot_id": "12345678-1234-4234-8234-123456789abc",
        "execution_environment": CHECKER.R26_EXECUTION_ENVIRONMENT,
        "gfx_version": "gfx942",
        "gpu_guid": "28851",
        "gpu_index": str(gpu_index),
        "gpu_node_id": "2",
        "gpu_serial": "692424017146",
        "hip_binary_sha256": "4" * 64,
        "hip_ldd_base64": encode(hip_ldd),
        "hip_ldd_sha256": hashlib.sha256(hip_ldd).hexdigest(),
        "hip_library_build_id": "cd" * 20,
        "hip_library_path_base64": encode(hip_path.encode()),
        "hip_library_sha256": "b" * 64,
        "hip_library_soname": "libamdhip64.so.7",
        "hip_loader_map_base64": encode(hip_loader_map),
        "hip_loader_map_sha256": hashlib.sha256(hip_loader_map).hexdigest(),
        "hip_loader_resolution_base64": encode(hip_loader_resolution),
        "hip_loader_resolution_sha256": hashlib.sha256(
            hip_loader_resolution
        ).hexdigest(),
        "hsa_binary_sha256": "3" * 64,
        "hsa_ldd_base64": encode(hsa_ldd),
        "hsa_ldd_sha256": hashlib.sha256(hsa_ldd).hexdigest(),
        "hsa_library_build_id": "ef" * 20,
        "hsa_library_path_base64": encode(hsa_path.encode()),
        "hsa_library_sha256": "c" * 64,
        "hsa_library_soname": "libhsa-runtime64.so.1",
        "hsa_loader_map_base64": encode(hsa_loader_map),
        "hsa_loader_map_sha256": hashlib.sha256(hsa_loader_map).hexdigest(),
        "hsa_loader_resolution_base64": encode(hsa_loader_resolution),
        "hsa_loader_resolution_sha256": hashlib.sha256(
            hsa_loader_resolution
        ).hexdigest(),
        "kernel_machine": "x86_64",
        "kernel_release": kernel_release,
        "kernel_sysname": "Linux",
        "kernel_version_base64": encode(kernel_version),
        "kernel_version_sha256": hashlib.sha256(kernel_version).hexdigest(),
        "kfd_binary_sha256": "2" * 64,
        "kfd_ldd_base64": encode(kfd_ldd),
        "kfd_ldd_sha256": hashlib.sha256(kfd_ldd).hexdigest(),
        "kfd_loader_map_base64": encode(kfd_loader_map),
        "kfd_loader_map_sha256": hashlib.sha256(kfd_loader_map).hexdigest(),
        "kfd_loader_resolution_base64": encode(kfd_loader_resolution),
        "kfd_loader_resolution_sha256": hashlib.sha256(
            kfd_loader_resolution
        ).hexdigest(),
        "ld_audit": "absent",
        "ld_library_path": "absent",
        "ld_preload": "absent",
        "ldd_path_base64": encode(b"/usr/bin/ldd"),
        "ldd_sha256": "e" * 64,
        "loader_resolution": "fixed-ldd-transitive-observed-to-canonical-v1",
        "modinfo_path_base64": encode(b"/usr/sbin/modinfo"),
        "modinfo_sha256": "f" * 64,
        "observation_edge": edge,
        "os_release_base64": encode(os_release),
        "os_release_sha256": hashlib.sha256(os_release).hexdigest(),
        "pci_bdf": "0000:05:00.0",
        "pci_class": "0x120000",
        "pci_device": "0x74a1",
        "pci_driver": "amdgpu",
        "pci_numa_node": "0",
        "pci_revision": "0x00",
        "pci_serial": "692424017146",
        "pci_subsystem_device": "0x74a1",
        "pci_subsystem_vendor": "0x1002",
        "pci_unique_id": unique_id,
        "pci_vendor": "0x1002",
        "product_model": "0x74a1",
        "product_name_base64": encode(b"AMD Instinct MI300X OAM"),
        "product_number": "102-G30211-00",
        "product_series_base64": encode(b"AMD Instinct MI300X"),
        "product_sku": "M3000100",
        "readelf_path_base64": encode(b"/usr/bin/readelf"),
        "readelf_sha256": "1" * 64,
        "rocm_path_base64": encode(b"/opt/rocm-7.2.0"),
        "rocm_smi_entrypoint_path_base64": encode(
            b"/opt/rocm-7.2.0/libexec/rocm_smi/rocm_smi.py"
        ),
        "rocm_smi_entrypoint_sha256": hashlib.sha256(rocm_smi_entrypoint).hexdigest(),
        "rocm_smi_identity_base64": encode(product_snapshot),
        "rocm_smi_identity_sha256": hashlib.sha256(product_snapshot).hexdigest(),
        "rocm_smi_interpreter_build_id": "12" * 20,
        "rocm_smi_interpreter_invocation_path_base64": encode(b"/usr/bin/python3"),
        "rocm_smi_interpreter_ldd_base64": encode(python_ldd),
        "rocm_smi_interpreter_ldd_sha256": hashlib.sha256(python_ldd).hexdigest(),
        "rocm_smi_interpreter_loader_map_base64": encode(python_loader_map),
        "rocm_smi_interpreter_loader_map_sha256": hashlib.sha256(
            python_loader_map
        ).hexdigest(),
        "rocm_smi_interpreter_loader_resolution_base64": encode(
            python_loader_resolution
        ),
        "rocm_smi_interpreter_loader_resolution_sha256": hashlib.sha256(
            python_loader_resolution
        ).hexdigest(),
        "rocm_smi_interpreter_path_base64": encode(python_path.encode()),
        "rocm_smi_interpreter_sha256": "3" * 64,
        "rocm_smi_invocation_path_base64": encode(b"/opt/rocm-7.2.0/bin/rocm-smi"),
        "rocm_smi_library_build_id": "34" * 20,
        "rocm_smi_library_ldd_base64": encode(rocm_smi_library_ldd),
        "rocm_smi_library_ldd_sha256": hashlib.sha256(rocm_smi_library_ldd).hexdigest(),
        "rocm_smi_library_loader_map_base64": encode(rocm_smi_library_loader_map),
        "rocm_smi_library_loader_map_sha256": hashlib.sha256(
            rocm_smi_library_loader_map
        ).hexdigest(),
        "rocm_smi_library_loader_resolution_base64": encode(
            rocm_smi_library_loader_resolution
        ),
        "rocm_smi_library_loader_resolution_sha256": hashlib.sha256(
            rocm_smi_library_loader_resolution
        ).hexdigest(),
        "rocm_smi_library_path_base64": encode(rocm_smi_library_path.encode()),
        "rocm_smi_library_sha256": "4" * 64,
        "rocm_smi_library_soname": "librocm_smi64.so.1",
        "rocm_smi_package_manifest_base64": encode(rocm_smi_package),
        "rocm_smi_package_manifest_sha256": hashlib.sha256(
            rocm_smi_package
        ).hexdigest(),
        "rocm_smi_shebang_base64": encode(b"#!/usr/bin/env python3\n"),
        "unique_id": f"0x{unique_id}",
        "uuid": f"GPU-{unique_id}",
        "zstd_path_base64": encode(b"/usr/bin/zstd"),
        "zstd_sha256": "2" * 64,
    }
    return render_system_identity(fields)


def system_identity_index(lines: list[str], edge: str = "start") -> int:
    return next(
        index
        for index, line in enumerate(lines)
        if line.startswith(f"context schema={CHECKER.R26_SYSTEM_IDENTITY_SCHEMA} ")
        and CHECKER.parse_fields(line, index + 1).get("observation_edge") == edge
    )


def update_system_identity(
    lines: list[str], *, edge: str = "start", **updates: str
) -> None:
    index = system_identity_index(lines, edge)
    fields = CHECKER.parse_fields(lines[index], index + 1)
    fields.update(updates)
    lines[index] = render_system_identity(fields)


def backend_index(lines: list[str], backend: str | None = None) -> int:
    return max(
        index
        for index, line in enumerate(lines)
        if line.startswith("backend=")
        and (backend is None or line.startswith(f"backend={backend} "))
    )


def update_backend_phase(
    lines: list[str], backend: str, phase: str, values: list[int]
) -> None:
    index = backend_index(lines, backend)
    fields = CHECKER.parse_fields(lines[index], index + 1)
    fields.update(summary_fields(phase, values))
    lines[index] = render("", fields).strip()


def update_context(lines: list[str], **updates: str) -> None:
    fields = CHECKER.parse_fields(lines[0], 1)
    fields.update(updates)
    lines[0] = render("context", fields)


def valid_log(
    slot: int = 0,
    *,
    set_id: str = "a" * 64,
    git_commit: str = "1" * 40,
    gpu_index: int = 0,
) -> list[str]:
    order = CHECKER.R26_COUNTERBALANCE_ORDERS[slot]
    phase_values = {
        "h2d": [1000 + index for index in range(30)],
        "compute": [2000 + index for index in range(30)],
        "d2h": [3000 + index for index in range(30)],
        "e2e": [7000 + index * 3 for index in range(30)],
    }
    rows: dict[str, str] = {}
    for backend in order:
        row = {
            "backend": backend,
            "schema": CHECKER.R26_INPLACE_SCHEMA,
            "unique_id": "0123456789abcdef",
            "uuid": "GPU-0123456789abcdef",
            **CHECKER.R26_FIXED_ROW,
        }
        for phase, values in phase_values.items():
            row.update(summary_fields(phase, values))
        if backend == "kfd":
            row.update(
                {
                    "promotion": "full-h2d-to-compute-ready",
                    "data_path": "persistent-device-reused",
                    "control_path": "persistent-control-replayed",
                    "user_data_materializations": "0",
                }
            )
            row.update(
                summary_fields("promotion", [500 + index for index in range(30)])
            )
            for phase in CHECKER.R26_LAUNCH_TIMING_PHASES:
                row.update(summary_fields(phase, valid_launch_timing_values(phase)))
        else:
            row.update(
                {
                    "promotion": "n/a",
                    "data_path": "host-staged-one-buffer",
                    "control_path": "n/a",
                    "user_data_materializations": "n/a",
                    "promotion_samples_ns": "n/a",
                    "promotion_min_ns": "n/a",
                    "promotion_mean_ns": "n/a",
                    "promotion_max_ns": "n/a",
                    "promotion_p50_ns": "n/a",
                    "promotion_p95_ns": "n/a",
                }
            )
            for phase in CHECKER.R26_LAUNCH_TIMING_PHASES:
                row[f"{phase}_samples_ns"] = "n/a"
                for summary in CHECKER.R26_SUMMARIES:
                    row[f"{phase}_{summary}_ns"] = "n/a"
        rows[backend] = render("", row).strip()

    _, topology_sha256 = valid_topology(slot, order[0], "start", gpu_index=gpu_index)
    context = {
        "schema": CHECKER.R26_INPLACE_SCHEMA,
        "git_commit": git_commit,
        "target": "gfx942:xnack-",
        "gpu_index": str(gpu_index),
        "unique_id": "0x0123456789abcdef",
        "uuid": "GPU-0123456789abcdef",
        "bytes": "1048576",
        "elements": "262144",
        "workgroup": "256",
        "warmups": "10",
        "samples": "30",
        "iterations_per_sample": "10",
        "kernel": "inplace_transform",
        "max_busy_percent": "5",
        "phase_timeout_seconds": "180",
        "rocm_version": "7.2.0",
        "rustc": "rustc_1.90.0",
        "cargo": "cargo_1.90.0",
        "hipcc": "HIP_version_7.2.0",
        "cxx": "g++_14.2.1",
        "build_environment": CHECKER.R26_FIXED_CONTEXT["build_environment"],
        "hsaco_sha256": CHECKER.R26_FIXED_CONTEXT["hsaco_sha256"],
        "kernel_source_sha256": CHECKER.R26_FIXED_CONTEXT["kernel_source_sha256"],
        "kernel_policy_sha256": CHECKER.R26_FIXED_CONTEXT["kernel_policy_sha256"],
        "fixture_recipe_sha256": CHECKER.R26_FIXED_CONTEXT["fixture_recipe_sha256"],
        "fixture_producer_clang": CHECKER.R26_FIXED_CONTEXT["fixture_producer_clang"],
        "fixture_rebuild": CHECKER.R26_FIXED_CONTEXT["fixture_rebuild"],
        "kfd_binary_sha256": "2" * 64,
        "hsa_binary_sha256": "3" * 64,
        "hip_binary_sha256": "4" * 64,
        "hsa_source_sha256": "5" * 64,
        "hip_source_sha256": "6" * 64,
        "binary_reader_sha256": "b" * 64,
        "hsa_pool_policy_sha256": "c" * 64,
        "common_header_sha256": "7" * 64,
        "checker_sha256": "8" * 64,
        "runner_sha256": "9" * 64,
        "host_guard_sha256": "a" * 64,
        "system_identity_collector_sha256": "b" * 64,
        "execution_environment": CHECKER.R26_EXECUTION_ENVIRONMENT,
        "telemetry_command": "rocm-smi-showuse-showclocks-showpower",
        "placement": CHECKER.R26_FIXED_CONTEXT["placement"],
        "interference_monitor": "selected-kfd-gpu-process-tree-census-v2",
        "monitor_interval_us": "2000",
        "monitor_maximum_gap_us": "10000",
        "topology_sha256": topology_sha256,
        "counterbalance_design": CHECKER.R26_COUNTERBALANCE_DESIGN,
        "counterbalance_slots": "3",
        "counterbalance_slot": str(slot),
        "counterbalance_set_id": set_id,
        "backend_order": ",".join(order),
    }
    lines = [
        render("context", context),
        valid_system_identity("start", address_seed=1, gpu_index=gpu_index),
    ]

    snapshot = (
        f"GPU[{gpu_index}] : GPU use (%): 0\n"
        f"GPU[{gpu_index}] : sclk clock level: 4\n"
        f"GPU[{gpu_index}] : power: 51.0 W\n"
    ).encode()
    encoded = base64.b64encode(snapshot).decode()
    digest = hashlib.sha256(snapshot).hexdigest()
    for backend in order:
        start_topology, _ = valid_topology(slot, backend, "start", gpu_index=gpu_index)
        end_topology, _ = valid_topology(slot, backend, "end", gpu_index=gpu_index)
        lines.extend(
            (
                start_topology,
                render(
                    "context",
                    {
                        "phase": backend,
                        "gpu_busy_start_percent": "0",
                        "telemetry_start_sha256": digest,
                        "telemetry_start_base64": encoded,
                    },
                ),
                valid_monitor(slot, backend, rows[backend]),
                render(
                    "context",
                    {
                        "phase": backend,
                        "gpu_busy_end_percent": "0",
                        "telemetry_end_sha256": digest,
                        "telemetry_end_base64": encoded,
                    },
                ),
                end_topology,
                rows[backend],
            )
        )
    lines.append(valid_system_identity("end", address_seed=9, gpu_index=gpu_index))
    return lines


class R26CheckerTests(unittest.TestCase):
    def check(self, lines: list[str]) -> list[str]:
        return CHECKER.check_rows(
            lines, CHECKER.R26_INPLACE_SCHEMA, Decimal(1), Decimal(1)
        )

    def test_accepts_exact_raw_samples_identity_and_telemetry(self) -> None:
        output = self.check(valid_log())
        self.assertEqual(output[-1], "validation_status=pass")
        self.assertNotIn("parity", " ".join(output))
        self.assertEqual(len(output), 20)
        self.assertEqual(
            output[0],
            "schema=fe2o3.r26-inplace-benchmark.v4 "
            "comparison=kfd-over-reference reference=hsa phase=h2d statistic=p50_ns "
            "kfd_over_reference_p50_ratio=1.000000 lower_is_better=true "
            "evidence_only=true",
        )
        self.assertEqual(
            output[8],
            "schema=fe2o3.r26-inplace-benchmark.v4 "
            "comparison=promotion-authentication-share phase=promotion-over-h2d "
            "statistic=p50_ns promotion_over_h2d_p50_ratio=0.506903 "
            "lower_is_better=true evidence_only=true",
        )
        self.assertEqual(
            output[-2],
            "schema=fe2o3.r26-inplace-benchmark.v4 "
            "comparison=kfd-host-launch-timing phase=recycle_inclusive statistic=p50_ns "
            "value=114 evidence_only=true",
        )

    def test_physical_gpu_index_is_distinct_from_backend_local_index(self) -> None:
        lines = valid_log(gpu_index=2)
        self.assertEqual(self.check(lines)[-1], "validation_status=pass")
        for backend in ("kfd", "hsa", "hip"):
            row = CHECKER.parse_fields(lines[backend_index(lines, backend)], 0)
            self.assertEqual(row["device_index"], "0")

        row_index = backend_index(lines, "hsa")
        row = CHECKER.parse_fields(lines[row_index], row_index + 1)
        row["device_index"] = "2"
        lines[row_index] = render("", row).strip()
        monitor_index = next(
            index
            for index, line in enumerate(lines)
            if line.startswith("monitor ")
            and CHECKER.parse_fields(line, index + 1).get("phase") == "hsa"
        )
        target = (lines[row_index] + "\n").encode()
        lines[monitor_index] = rewrite_guard_record(
            lines[monitor_index],
            "monitor",
            target_output_bytes=str(len(target)),
            target_output_sha256=hashlib.sha256(target).hexdigest(),
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "backend hsa has invalid R26 field device_index"
        ):
            self.check(lines)

    def test_rejects_v3_r26_schema_instead_of_mixing_recycle_contracts(self) -> None:
        lines = valid_log()
        lines[0] = lines[0].replace(
            "schema=fe2o3.r26-inplace-benchmark.v4",
            "schema=fe2o3.r26-inplace-benchmark.v3",
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected R26 context line"):
            self.check(lines)

    def test_loader_evidence_revalidates_without_measurement_host_paths(self) -> None:
        with mock.patch.object(
            CHECKER.pathlib.Path,
            "resolve",
            side_effect=AssertionError("checker consulted the live filesystem"),
        ):
            self.assertEqual(self.check(valid_log())[-1], "validation_status=pass")

    def test_loader_evidence_enforces_collector_bounds(self) -> None:
        identity = CHECKER.parse_fields(valid_system_identity(), 1)
        cases = (
            (
                "kfd_ldd_base64",
                "kfd_ldd_sha256",
                b"x" * (CHECKER.R26_MAX_LDD_BYTES + 1),
                lambda fields: CHECKER.r26_parse_raw_ldd(fields, "kfd"),
            ),
            (
                "kfd_loader_map_base64",
                "kfd_loader_map_sha256",
                b"x" * (CHECKER.R26_MAX_LOADER_EVIDENCE_BYTES + 1),
                lambda fields: CHECKER.r26_parse_loader_map(fields, "kfd"),
            ),
            (
                "kfd_loader_resolution_base64",
                "kfd_loader_resolution_sha256",
                b"x" * (CHECKER.R26_MAX_LOADER_EVIDENCE_BYTES + 1),
                lambda fields: CHECKER.r26_parse_loader_resolution(fields, "kfd"),
            ),
        )
        for encoded_field, digest_field, raw, parser in cases:
            with self.subTest(field=encoded_field):
                mutated = dict(identity)
                mutated[encoded_field] = encode(raw)
                mutated[digest_field] = hashlib.sha256(raw).hexdigest()
                with self.assertRaisesRegex(CHECKER.CheckError, "retained bound"):
                    parser(mutated)

        dependency_count = CHECKER.R26_MAX_LOADER_DEPENDENCIES + 1
        raw_rows = "".join(
            f"lib{index:04d}.so => /lib/lib{index:04d}.so (0x1)\n"
            for index in range(dependency_count)
        ).encode()
        map_rows = "".join(
            f"lib{index:04d}.so=/lib/lib{index:04d}.so\n"
            for index in range(dependency_count)
        ).encode()
        resolution_rows = "".join(
            f"soname=lib{index:04d}.so\tobserved=/lib/lib{index:04d}.so\t"
            f"resolved=/lib/lib{index:04d}.so\n"
            for index in range(dependency_count)
        ).encode()
        cardinality_cases = (
            (
                "kfd_ldd_base64",
                "kfd_ldd_sha256",
                raw_rows,
                lambda fields: CHECKER.r26_parse_raw_ldd(fields, "kfd"),
            ),
            (
                "kfd_loader_map_base64",
                "kfd_loader_map_sha256",
                map_rows,
                lambda fields: CHECKER.r26_parse_loader_map(fields, "kfd"),
            ),
            (
                "kfd_loader_resolution_base64",
                "kfd_loader_resolution_sha256",
                resolution_rows,
                lambda fields: CHECKER.r26_parse_loader_resolution(fields, "kfd"),
            ),
        )
        for encoded_field, digest_field, raw, parser in cardinality_cases:
            with self.subTest(cardinality_field=encoded_field):
                mutated = dict(identity)
                mutated[encoded_field] = encode(raw)
                mutated[digest_field] = hashlib.sha256(raw).hexdigest()
                with self.assertRaisesRegex(CHECKER.CheckError, "cardinality"):
                    parser(mutated)

    def test_rejects_trimmed_raw_sample_set(self) -> None:
        lines = valid_log()
        index = backend_index(lines)
        lines[index] = lines[index].replace(
            "h2d_samples_ns=" + ",".join(str(1000 + i) for i in range(30)),
            "h2d_samples_ns=" + ",".join(str(1000 + i) for i in range(29)),
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "exactly 30"):
            self.check(lines)

    def test_rejects_summary_not_derived_from_raw_samples(self) -> None:
        lines = valid_log()
        index = backend_index(lines)
        lines[index] = lines[index].replace(
            "compute_mean_ns=2014", "compute_mean_ns=2015"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "inconsistent with raw"):
            self.check(lines)

    def test_rejects_uuid_unique_id_mismatch(self) -> None:
        lines = valid_log()
        index = backend_index(lines)
        lines[index] = lines[index].replace(
            "uuid=GPU-0123456789abcdef", "uuid=GPU-fedcba9876543210"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "exact context identity"):
            self.check(lines)

    def test_rejects_zero_context_unique_id(self) -> None:
        lines = valid_log()
        lines[0] = (
            lines[0]
            .replace("unique_id=0x0123456789abcdef", "unique_id=0x0000000000000000")
            .replace("uuid=GPU-0123456789abcdef", "uuid=GPU-0000000000000000")
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "must be nonzero"):
            self.check(lines)

    def test_rejects_missing_or_mismatched_system_identity(self) -> None:
        lines = valid_log()
        del lines[1]
        with self.assertRaisesRegex(
            CHECKER.CheckError, "exactly two system identities"
        ):
            self.check(lines)

        lines = valid_log()
        update_system_identity(lines, unique_id="0xfedcba9876543210")
        with self.assertRaisesRegex(CHECKER.CheckError, "unique ID.*benchmark context"):
            self.check(lines)

        lines = valid_log()
        update_system_identity(lines, hsa_binary_sha256="d" * 64)
        with self.assertRaisesRegex(CHECKER.CheckError, "HSA binary identity"):
            self.check(lines)

        lines = valid_log()
        update_system_identity(lines, kfd_binary_sha256="d" * 64)
        with self.assertRaisesRegex(CHECKER.CheckError, "KFD binary identity"):
            self.check(lines)

    def test_rejects_corrupt_retained_system_identity(self) -> None:
        lines = valid_log()
        update_system_identity(lines, os_release_sha256="d" * 64)
        with self.assertRaisesRegex(CHECKER.CheckError, "does not match its retained"):
            self.check(lines)

        lines = valid_log()
        index = system_identity_index(lines)
        fields = CHECKER.parse_fields(lines[index], index + 1)
        hsa_map = base64.b64decode(fields["hsa_loader_map_base64"])
        hsa_map = b"libamdhip64.so.7=/opt/rocm-7.2.0/lib/libamdhip64.so.7\n" + hsa_map
        update_system_identity(
            lines,
            hsa_loader_map_base64=encode(hsa_map),
            hsa_loader_map_sha256=hashlib.sha256(hsa_map).hexdigest(),
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "raw ldd output and loader map"
        ):
            self.check(lines)

        lines = valid_log()
        index = system_identity_index(lines)
        fields = CHECKER.parse_fields(lines[index], index + 1)
        hsa_resolution = base64.b64decode(fields["hsa_loader_resolution_base64"])
        hsa_resolution = hsa_resolution.replace(
            b"resolved=/opt/rocm-7.2.0/lib/libhsa-runtime64.so.1.18.70200",
            b"resolved=/opt/rocm-7.2.0/lib/libhsa-runtime64.so.1.18.changed",
        )
        update_system_identity(
            lines,
            hsa_loader_resolution_base64=encode(hsa_resolution),
            hsa_loader_resolution_sha256=hashlib.sha256(hsa_resolution).hexdigest(),
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "raw ldd output and loader map"
        ):
            self.check(lines)

        lines = valid_log()
        index = system_identity_index(lines)
        fields = CHECKER.parse_fields(lines[index], index + 1)
        package = base64.b64decode(fields["rocm_smi_package_manifest_base64"])
        package = package.replace(b"file=rsmiBindings.py", b"file=untracked.py")
        update_system_identity(
            lines,
            rocm_smi_package_manifest_base64=encode(package),
            rocm_smi_package_manifest_sha256=hashlib.sha256(package).hexdigest(),
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "invalid membership"):
            self.check(lines)

        lines = valid_log()
        index = system_identity_index(lines)
        fields = CHECKER.parse_fields(lines[index], index + 1)
        kfd_map = base64.b64decode(fields["kfd_loader_map_base64"])
        kfd_map += b"libhsa-runtime64.so.1=/opt/rocm-7.2.0/lib/libhsa.so\n"
        update_system_identity(
            lines,
            kfd_loader_map_base64=encode(kfd_map),
            kfd_loader_map_sha256=hashlib.sha256(kfd_map).hexdigest(),
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "raw ldd output and loader map"
        ):
            self.check(lines)

    def test_requires_exact_start_and_end_system_identity_edges(self) -> None:
        lines = valid_log()
        del lines[system_identity_index(lines, "end")]
        with self.assertRaisesRegex(
            CHECKER.CheckError, "exactly two system identities"
        ):
            self.check(lines)

        lines = valid_log()
        update_system_identity(lines, edge="end", observation_edge="start")
        with self.assertRaisesRegex(CHECKER.CheckError, "duplicate R26 start"):
            self.check(lines)

    def test_rejects_identity_change_between_observation_edges(self) -> None:
        lines = valid_log()
        update_system_identity(
            lines,
            edge="end",
            boot_id="87654321-4321-4321-8321-cba987654321",
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "mismatched field boot_id"):
            self.check(lines)

    def test_requires_identity_edges_to_enclose_measurement(self) -> None:
        lines = valid_log()
        end_index = system_identity_index(lines, "end")
        end_identity = lines.pop(end_index)
        lines.insert(2, end_identity)
        with self.assertRaisesRegex(CHECKER.CheckError, "do not enclose"):
            self.check(lines)

    def test_accepts_edge_varying_ldd_addresses_but_validates_each_blob(self) -> None:
        self.assertEqual(self.check(valid_log())[-1], "validation_status=pass")

        lines = valid_log()
        index = system_identity_index(lines, "end")
        fields = CHECKER.parse_fields(lines[index], index + 1)
        raw = base64.b64decode(fields["kfd_ldd_base64"])
        raw += b"unknown loader annotation\n"
        update_system_identity(
            lines,
            edge="end",
            kfd_ldd_base64=encode(raw),
            kfd_ldd_sha256=hashlib.sha256(raw).hexdigest(),
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "unknown row"):
            self.check(lines)

    def test_rejects_duplicate_os_release_key(self) -> None:
        lines = valid_log()
        raw = b'ID=ubuntu\nID=ubuntu\nVERSION_ID="24.04"\n'
        update_system_identity(
            lines,
            os_release_base64=encode(raw),
            os_release_sha256=hashlib.sha256(raw).hexdigest(),
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "duplicate key"):
            self.check(lines)

    def test_rejects_noncanonical_or_extended_system_identity(self) -> None:
        lines = valid_log()
        index = system_identity_index(lines)
        tokens = lines[index].split()
        lines[index] = " ".join(tokens[:3] + [tokens[4], tokens[3]] + tokens[5:])
        with self.assertRaisesRegex(CHECKER.CheckError, "noncanonical"):
            self.check(lines)

        lines = valid_log()
        update_system_identity(lines, future_identity="value")
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected fields"):
            self.check(lines)

        lines = valid_log()
        for edge in ("start", "end"):
            update_system_identity(
                lines,
                edge=edge,
                rocm_smi_library_path_base64=encode(
                    b"/opt/rocm-7.2.0/arbitrary/untrusted/librocm_smi64.so.1.evil"
                ),
            )
        with self.assertRaisesRegex(CHECKER.CheckError, "outside its retained ROCm"):
            self.check(lines)

    def test_rejects_reference_promotion_measurement(self) -> None:
        lines = valid_log()
        index = backend_index(lines)
        lines[index] = lines[index].replace("promotion=n/a", "promotion=measured")
        with self.assertRaisesRegex(CHECKER.CheckError, "field promotion"):
            self.check(lines)

    def test_rejects_kfd_user_materialization(self) -> None:
        lines = valid_log()
        kfd_index = next(
            i for i, line in enumerate(lines) if line.startswith("backend=kfd")
        )
        lines[kfd_index] = lines[kfd_index].replace(
            "user_data_materializations=0", "user_data_materializations=1"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "user_data_materializations"):
            self.check(lines)

    def test_rejects_kfd_without_persistent_control_replay(self) -> None:
        lines = valid_log()
        kfd_index = next(
            i for i, line in enumerate(lines) if line.startswith("backend=kfd")
        )
        lines[kfd_index] = lines[kfd_index].replace(
            "control_path=persistent-control-replayed",
            "control_path=persistent-control-prepared",
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "control_path"):
            self.check(lines)

    def test_rejects_e2e_sample_below_synchronized_phase_sum(self) -> None:
        lines = valid_log()
        index = backend_index(lines)
        lines[index] = lines[index].replace(
            "e2e_samples_ns=7000", "e2e_samples_ns=5000"
        )
        lines[index] = lines[index].replace("e2e_min_ns=7000", "e2e_min_ns=5000")
        lines[index] = lines[index].replace("e2e_mean_ns=7043", "e2e_mean_ns=6976")
        with self.assertRaisesRegex(CHECKER.CheckError, "below its phase sum"):
            self.check(lines)

    def test_rejects_incorrect_kfd_promotion_summary(self) -> None:
        lines = valid_log()
        kfd_index = next(
            i for i, line in enumerate(lines) if line.startswith("backend=kfd")
        )
        lines[kfd_index] = lines[kfd_index].replace(
            "promotion_p95_ns=528", "promotion_p95_ns=529"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "inconsistent with raw"):
            self.check(lines)

    def test_rejects_kfd_promotion_above_inclusive_h2d(self) -> None:
        lines = valid_log()
        kfd_index = next(
            i for i, line in enumerate(lines) if line.startswith("backend=kfd")
        )
        old_values = [500 + index for index in range(30)]
        new_values = [1500 + index for index in range(30)]
        replacement = summary_fields("promotion", new_values)
        for field, value in summary_fields("promotion", old_values).items():
            lines[kfd_index] = lines[kfd_index].replace(
                f"{field}={value}", f"{field}={replacement[field]}"
            )
        with self.assertRaisesRegex(CHECKER.CheckError, "exceeds inclusive H2D"):
            self.check(lines)

    def test_rejects_invalid_kfd_launch_timing_series(self) -> None:
        lines = valid_log()
        kfd_index = backend_index(lines, "kfd")
        lines[kfd_index] = lines[kfd_index].replace(
            "preparation_samples_ns=250", "preparation_samples_ns=0", 1
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "30 canonical ASCII positive integer samples"
        ):
            self.check(lines)

        for phase, values, expected in (
            (
                "completed_readback",
                [1] + [0] * 29,
                "completed_readback must be exactly zero",
            ),
            ("native_binding", [3000] * 30, "exceeds inclusive compute"),
        ):
            with self.subTest(phase=phase):
                lines = valid_log()
                update_backend_phase(lines, "kfd", phase, values)
                with self.assertRaisesRegex(CHECKER.CheckError, expected):
                    self.check(lines)

    def test_rejects_noncanonical_kfd_launch_timing_summary(self) -> None:
        for replacement in ("0250", "\u0662\u0665\u0660"):
            with self.subTest(replacement=replacement):
                lines = valid_log()
                kfd_index = backend_index(lines, "kfd")
                lines[kfd_index] = lines[kfd_index].replace(
                    "preparation_min_ns=250",
                    f"preparation_min_ns={replacement}",
                )
                with self.assertRaisesRegex(
                    CHECKER.CheckError, "canonical ASCII positive integer"
                ):
                    self.check(lines)

    def test_rejects_r26_summary_accumulator_overflow(self) -> None:
        lines = valid_log()
        update_backend_phase(
            lines, "kfd", "preparation", [CHECKER.R26_MAX_NANOSECONDS] * 30
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "preparation summary timing overflow"
        ):
            self.check(lines)

    def test_rejects_inconsistent_launch_timing_relationships(self) -> None:
        lines = valid_log()
        update_backend_phase(lines, "kfd", "preparation", [100] * 30)
        update_backend_phase(lines, "kfd", "bound_snapshot", [60] * 30)
        update_backend_phase(lines, "kfd", "authority", [50] * 30)
        with self.assertRaisesRegex(
            CHECKER.CheckError, "nested preparation sample 0 exceeds"
        ):
            self.check(lines)

        lines = valid_log()
        update_backend_phase(lines, "kfd", "preparation", [500] * 30)
        update_backend_phase(lines, "kfd", "bound_snapshot", [200] * 30)
        update_backend_phase(lines, "kfd", "authority", [200] * 30)
        for phase in (
            "native_binding",
            "publication",
            "publish_to_completion",
        ):
            update_backend_phase(lines, "kfd", phase, [400] * 30)
        update_backend_phase(lines, "kfd", "completion_signal_recycle", [200] * 30)
        update_backend_phase(lines, "kfd", "completion_detach_restore", [200] * 30)
        update_backend_phase(lines, "kfd", "recycle_inclusive", [400] * 30)
        with self.assertRaisesRegex(
            CHECKER.CheckError, "launch critical-path sample 0 exceeds"
        ):
            self.check(lines)

    def test_rejects_inconsistent_or_overflowing_recycle_components(self) -> None:
        lines = valid_log()
        update_backend_phase(
            lines, "kfd", "recycle_inclusive", [99 + index for index in range(30)]
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "components sample 0 do not equal inclusive recycle"
        ):
            self.check(lines)

        half = CHECKER.R26_MAX_NANOSECONDS // 2
        with self.assertRaisesRegex(
            CHECKER.CheckError, "completion recycle components timing overflow"
        ):
            CHECKER.r26_checked_sum(
                (half, half + 2), "completion recycle components"
            )

    def test_rejects_reference_kfd_launch_timing_measurement(self) -> None:
        lines = valid_log()
        hsa_index = backend_index(lines, "hsa")
        lines[hsa_index] = lines[hsa_index].replace(
            "preparation_samples_ns=n/a",
            "preparation_samples_ns=" + ",".join(["1"] * 30),
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "must be n/a"):
            self.check(lines)

    def test_rejects_telemetry_without_clock_evidence(self) -> None:
        lines = valid_log()
        phase_index = next(i for i, line in enumerate(lines) if "phase=kfd" in line)
        snapshot = b"GPU[0] : GPU use (%): 0\nGPU[0] : power: 51.0 W\n"
        encoded = base64.b64encode(snapshot).decode()
        digest = hashlib.sha256(snapshot).hexdigest()
        lines[phase_index] = render(
            "context",
            {
                "phase": "kfd",
                "gpu_busy_start_percent": "0",
                "telemetry_start_sha256": digest,
                "telemetry_start_base64": encoded,
            },
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "clock/power evidence"):
            self.check(lines)

    def test_rejects_telemetry_load_mismatch(self) -> None:
        lines = valid_log()
        phase_index = next(i for i, line in enumerate(lines) if "phase=kfd" in line)
        snapshot = (
            b"GPU[0] : GPU use (%): 4\n"
            b"GPU[0] : sclk clock level: 4\n"
            b"GPU[0] : power: 51.0 W\n"
        )
        encoded = base64.b64encode(snapshot).decode()
        digest = hashlib.sha256(snapshot).hexdigest()
        lines[phase_index] = render(
            "context",
            {
                "phase": "kfd",
                "gpu_busy_start_percent": "0",
                "telemetry_start_sha256": digest,
                "telemetry_start_base64": encoded,
            },
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "load does not match"):
            self.check(lines)

    def test_rejects_unsealed_or_identity_mismatched_topology(self) -> None:
        lines = valid_log()
        topology_index = next(
            index for index, line in enumerate(lines) if line.startswith("topology ")
        )
        lines[topology_index] = lines[topology_index].replace(
            "observer_cpu=48", "observer_cpu=49"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "topology seal"):
            self.check(lines)

        lines = valid_log()
        topology_sha256 = ""
        for index, line in enumerate(lines):
            if line.startswith("topology "):
                lines[index] = rewrite_guard_record(
                    line, "topology", pci_bdf="0000:06:00.0"
                )
                topology_sha256 = CHECKER.parse_fields(lines[index], index + 1)[
                    "topology_sha256"
                ]
        update_context(lines, topology_sha256=topology_sha256)
        with self.assertRaisesRegex(CHECKER.CheckError, "does not match pci_bdf"):
            self.check(lines)

    def test_rejects_topology_kfd_and_numa_identity_mismatches(self) -> None:
        for field, value in (
            ("numa_node", "1"),
            ("kfd_node", "3"),
            ("kfd_gpu_id", "28852"),
        ):
            with self.subTest(field=field):
                lines = valid_log()
                topology_sha256 = ""
                for index, line in enumerate(lines):
                    if line.startswith("topology "):
                        lines[index] = rewrite_guard_record(
                            line, "topology", **{field: value}
                        )
                        topology_sha256 = CHECKER.parse_fields(lines[index], index + 1)[
                            "topology_sha256"
                        ]
                update_context(lines, topology_sha256=topology_sha256)
                with self.assertRaisesRegex(
                    CHECKER.CheckError, f"does not match {field}"
                ):
                    self.check(lines)

    def test_rejects_missing_dirty_or_unbound_monitor(self) -> None:
        lines = valid_log()
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        del lines[monitor_index]
        with self.assertRaisesRegex(CHECKER.CheckError, "missing R26 monitor"):
            self.check(lines)

        lines = valid_log()
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        lines[monitor_index] = rewrite_guard_record(
            lines[monitor_index], "monitor", status="contaminated"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "monitor status"):
            self.check(lines)

        lines = valid_log()
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        lines[monitor_index] = rewrite_guard_record(
            lines[monitor_index], "monitor", target_output_sha256="b" * 64
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "digest does not match its row"
        ):
            self.check(lines)

    def test_rejects_monitor_cadence_or_row_release_order(self) -> None:
        lines = valid_log()
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        lines[monitor_index] = rewrite_guard_record(
            lines[monitor_index], "monitor", observed_maximum_gap_us="10001"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "gap exceeds"):
            self.check(lines)

        lines = valid_log()
        monitor_index = next(
            index for index, line in enumerate(lines) if line.startswith("monitor ")
        )
        lines[monitor_index] = rewrite_guard_record(
            lines[monitor_index],
            "monitor",
            target_selected_queue_observations="0",
        )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "target_selected_queue_observations"
        ):
            self.check(lines)

        lines = valid_log()
        row_index = next(
            index for index, line in enumerate(lines) if line.startswith("backend=")
        )
        lines[row_index - 1], lines[row_index] = lines[row_index], lines[row_index - 1]
        with self.assertRaisesRegex(CHECKER.CheckError, "guarded row release"):
            self.check(lines)

    def test_rejects_monitor_v2_schedule_or_terminal_state(self) -> None:
        for field, value, error in (
            ("schema", "fe2o3.r26-kfd-queue-monitor.v1", "monitor schema"),
            (
                "monitor",
                "selected-kfd-gpu-process-tree-census-v1",
                "monitor monitor",
            ),
            ("schedule", "relative-sleep-v1", "monitor schedule"),
            ("process_group", "9999", "process_group does not match root_pid"),
            ("target_reaped", "0", "monitor target_reaped"),
            ("process_group_absent", "0", "monitor process_group_absent"),
            ("terminal_selected_queues", "1", "monitor terminal_selected_queues"),
            ("observations", "2", "at least three queue observations"),
            ("observed_maximum_gap_us", "0", "must be a positive integer"),
        ):
            with self.subTest(field=field):
                lines = valid_log()
                monitor_index = next(
                    index
                    for index, line in enumerate(lines)
                    if line.startswith("monitor ")
                )
                lines[monitor_index] = rewrite_guard_record(
                    lines[monitor_index], "monitor", **{field: value}
                )
                with self.assertRaisesRegex(CHECKER.CheckError, error):
                    self.check(lines)

    def test_accepts_exact_three_slot_counterbalance_without_aggregation(self) -> None:
        logs = {slot: valid_log(slot) for slot in (0, 1, 2)}
        output = CHECKER.check_r26_counterbalance_set([logs[2], logs[0], logs[1]])
        slot_hashes = {
            slot: hashlib.sha256(("\n".join(lines) + "\n").encode()).hexdigest()
            for slot, lines in logs.items()
        }
        manifest_sha256 = hashlib.sha256(
            CHECKER.r26_manifest_payload("a" * 64, slot_hashes)
        ).hexdigest()
        expected_performance: list[str] = []
        for slot in (0, 1, 2):
            for phase in CHECKER.R26_PHASES:
                for reference in ("hsa", "hip"):
                    expected_performance.append(
                        f"schema={CHECKER.R26_INPLACE_SCHEMA} "
                        f"counterbalance_slot={slot} "
                        "comparison=kfd-over-reference "
                        f"reference={reference} phase={phase} statistic=p50_ns "
                        "kfd_over_reference_p50_ratio=1.000000 "
                        "lower_is_better=true evidence_only=true"
                    )
            expected_performance.append(
                f"schema={CHECKER.R26_INPLACE_SCHEMA} "
                f"counterbalance_slot={slot} "
                "comparison=promotion-authentication-share "
                "phase=promotion-over-h2d statistic=p50_ns "
                "promotion_over_h2d_p50_ratio=0.506903 "
                "lower_is_better=true evidence_only=true"
            )
            for phase in CHECKER.R26_LAUNCH_TIMING_PHASES:
                value = CHECKER.r26_p50(valid_launch_timing_values(phase))
                expected_performance.append(
                    f"schema={CHECKER.R26_INPLACE_SCHEMA} "
                    f"counterbalance_slot={slot} "
                    "comparison=kfd-host-launch-timing "
                    f"phase={phase} statistic=p50_ns value={value} "
                    "evidence_only=true"
                )
            expected_performance.append(
                f"schema={CHECKER.R26_INPLACE_SCHEMA} "
                f"counterbalance_slot={slot} slot_validation_status=pass"
            )
        self.assertEqual(
            output,
            expected_performance
            + [
                "schema=fe2o3.r26-inplace-benchmark.v4 "
                "counterbalance_design=cyclic-latin-square-3 "
                "counterbalance_slots=3 "
                f"counterbalance_set_id={'a' * 64} "
                f"slot_0_sha256={slot_hashes[0]} "
                f"slot_1_sha256={slot_hashes[1]} "
                f"slot_2_sha256={slot_hashes[2]} "
                f"manifest_schema={CHECKER.R26_MANIFEST_SCHEMA} "
                f"manifest_sha256={manifest_sha256} "
                "raw_samples_per_backend_per_slot=30 aggregation=none "
                "claim=evidence-only set_validation_status=pass"
            ],
        )

    def test_rejects_missing_or_duplicate_counterbalance_slots(self) -> None:
        with self.assertRaisesRegex(CHECKER.CheckError, "missing.*slot 2"):
            CHECKER.check_r26_counterbalance_set([valid_log(0), valid_log(1)])
        with self.assertRaisesRegex(CHECKER.CheckError, "duplicate.*slot 0"):
            CHECKER.check_r26_counterbalance_set(
                [valid_log(0), valid_log(0), valid_log(2)]
            )

    def test_rejects_counterbalance_context_mismatch(self) -> None:
        with self.assertRaisesRegex(
            CHECKER.CheckError, "mismatched context field git_commit"
        ):
            CHECKER.check_r26_counterbalance_set(
                [valid_log(0), valid_log(1, git_commit="2" * 40), valid_log(2)]
            )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "mismatched context field counterbalance_set_id"
        ):
            CHECKER.check_r26_counterbalance_set(
                [valid_log(0), valid_log(1, set_id="b" * 64), valid_log(2)]
            )

    def test_rejects_counterbalance_system_identity_mismatch(self) -> None:
        logs = [valid_log(0), valid_log(1), valid_log(2)]
        for edge in ("start", "end"):
            update_system_identity(
                logs[1],
                edge=edge,
                boot_id="87654321-4321-4321-8321-cba987654321",
            )
        with self.assertRaisesRegex(
            CHECKER.CheckError, "mismatched start system identity"
        ):
            CHECKER.check_r26_counterbalance_set(logs)

    def test_rejects_counterbalance_host_topology_mismatch(self) -> None:
        logs = [valid_log(0), valid_log(1), valid_log(2)]
        topology_sha256 = ""
        for index, line in enumerate(logs[1]):
            if line.startswith("topology "):
                logs[1][index] = rewrite_guard_record(
                    line, "topology", measurement_cpu_list="1-47"
                )
                topology_sha256 = CHECKER.parse_fields(logs[1][index], index + 1)[
                    "topology_sha256"
                ]
        update_context(logs[1], topology_sha256=topology_sha256)
        with self.assertRaisesRegex(CHECKER.CheckError, "mismatched host topology"):
            CHECKER.check_r26_counterbalance_set(logs)

    def test_rejects_wrong_declared_or_observed_counterbalance_order(self) -> None:
        lines = valid_log(0)
        lines[0] = lines[0].replace(
            "backend_order=kfd,hsa,hip", "backend_order=hsa,hip,kfd"
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "backend_order"):
            self.check(lines)

        lines = valid_log(0)
        row_indices = [
            index for index, line in enumerate(lines) if line.startswith("backend=")
        ]
        lines[row_indices[-2]], lines[row_indices[-1]] = (
            lines[row_indices[-1]],
            lines[row_indices[-2]],
        )
        with self.assertRaisesRegex(CHECKER.CheckError, "row order"):
            self.check(lines)

        lines = valid_log(0)
        telemetry_indices = [
            index
            for index, line in enumerate(lines)
            if line.startswith("context phase=")
        ]
        first, second = telemetry_indices[0], telemetry_indices[1]
        lines[first], lines[second] = lines[second], lines[first]
        with self.assertRaisesRegex(CHECKER.CheckError, "telemetry order|guarded row"):
            self.check(lines)

    def test_rejects_unrecognized_r26_fields_and_lines(self) -> None:
        lines = valid_log()
        lines[0] += " future_context=value"
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected fields"):
            self.check(lines)

        lines = valid_log()
        telemetry_index = next(
            index
            for index, line in enumerate(lines)
            if line.startswith("context phase=")
        )
        lines[telemetry_index] += " future_telemetry=value"
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected field"):
            self.check(lines)

        lines = valid_log()
        index = backend_index(lines)
        lines[index] += " future_result=value"
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected R26 fields"):
            self.check(lines)

        lines = valid_log()
        lines.append("message=benchmark-complete")
        with self.assertRaisesRegex(CHECKER.CheckError, "unexpected R26 evidence line"):
            self.check(lines)

        lines = valid_log()
        lines[0] += " future-token"
        with self.assertRaisesRegex(CHECKER.CheckError, "malformed R26 evidence token"):
            self.check(lines)

    def test_cli_validates_exact_counterbalance_set_without_aggregation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for slot in (2, 0, 1):
                path = pathlib.Path(directory, f"slot-{slot}.log")
                path.write_text("\n".join(valid_log(slot)) + "\n", encoding="utf-8")
                paths.append(path)
            arguments = [
                str(CHECKER_PATH),
                *(str(path) for path in paths),
                "--schema",
                CHECKER.R26_INPLACE_SCHEMA,
                "--r26-counterbalance-set",
            ]
            stdout = StringIO()
            with mock.patch.object(sys, "argv", arguments), redirect_stdout(stdout):
                self.assertEqual(CHECKER.main(), 0)
            self.assertIn("aggregation=none", stdout.getvalue())
            self.assertNotIn("parity", stdout.getvalue())

    def test_cli_requires_exactly_three_counterbalance_logs(self) -> None:
        arguments = [
            str(CHECKER_PATH),
            "slot-0.log",
            "slot-1.log",
            "--schema",
            CHECKER.R26_INPLACE_SCHEMA,
            "--r26-counterbalance-set",
        ]
        with (
            mock.patch.object(sys, "argv", arguments),
            redirect_stderr(StringIO()),
            self.assertRaisesRegex(SystemExit, "2"),
        ):
            CHECKER.main()

    def test_cli_does_not_replace_explicit_zero_threshold(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8") as input_file:
            input_file.flush()
            arguments = [
                str(CHECKER_PATH),
                input_file.name,
                "--schema",
                "fe2o3.async-copy-benchmark.v1",
                "--max-latency-ratio",
                "0",
                "--min-bandwidth-ratio",
                "1",
            ]
            with (
                mock.patch.object(sys, "argv", arguments),
                mock.patch.object(CHECKER, "check_rows", return_value=["ok"]) as check,
                redirect_stdout(StringIO()),
            ):
                self.assertEqual(CHECKER.main(), 0)
            self.assertEqual(check.call_args.args[2], Decimal(0))
            self.assertEqual(check.call_args.args[3], Decimal(1))

    def test_cli_rejects_r26_parity_thresholds(self) -> None:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8") as input_file:
            input_file.flush()
            arguments = [
                str(CHECKER_PATH),
                input_file.name,
                "--schema",
                CHECKER.R26_INPLACE_SCHEMA,
                "--max-latency-ratio",
                "1",
            ]
            with (
                mock.patch.object(sys, "argv", arguments),
                redirect_stderr(StringIO()),
                self.assertRaisesRegex(SystemExit, "2"),
            ):
                CHECKER.main()


if __name__ == "__main__":
    unittest.main()
