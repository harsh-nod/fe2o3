#!/usr/bin/env python3
"""Compare checked KFD identity evidence with an isolated rocminfo measurement."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import os
from pathlib import Path
import re
import stat
import sys


EXPECTED_GPU_COUNT = 8
EXPECTED_METADATA_AUDIT_ROOTS = 5
EXPECTED_PROFILE = "gfx942:xnack-"
EXPECTED_TARGET = "gfx942"
EXPECTED_WAVEFRONT = 64
EXPECTED_ISAS = (
    "amdgcn-amd-amdhsa--gfx9-4-generic:sramecc+:xnack-",
    "amdgcn-amd-amdhsa--gfx942:sramecc+:xnack-",
)
EXPECTED_PRIMARY_ISA = EXPECTED_ISAS[1]
EXPECTED_MARKETING_NAME = "AMD Instinct MI300X"
EXPECTED_PROFILE_SHA256 = (
    "e12ea33b259666e7928612403109640b03b0d637b893a2c15b87d17a4211c8de"
)
EXPECTED_KERNEL_RELEASE = "6.8.0-124-generic"
EXPECTED_AMDGPU_VERSION = "6.16.13"
EXPECTED_AMDGPU_SRCVERSION = "A6F143BEC60C0AFC3263226"
EXPECTED_ROCM_RELEASE = "7.2.4"
EXPECTED_HSA_RUNTIME = "1.18"

MAX_PURE_RUST_BYTES = 128 * 1024
MAX_ROCMINFO_BYTES = 1024 * 1024
MAX_EXECUTABLE_BYTES = 64 * 1024 * 1024
MAX_VERSION_BYTES = 256
MAX_PROVENANCE_SOURCE_BYTES = 2 * 1024 * 1024
MAX_AUDIT_REPORT_BYTES = 4096
MAX_GIT_OBSERVATION_BYTES = 256
MAX_TIME_OBSERVATION_BYTES = 64
MAX_LINES = 16_384
MAX_LINE_BYTES = 4096
MAX_AGENTS = 64
MAX_ISAS_PER_AGENT = 8

BOOT_ID_RE = re.compile(
    r"[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}"
)
UUID_RE = re.compile(r"[0-9a-f]{16}")
PCI_RE = re.compile(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]")
DECIMAL_RE = re.compile(r"(?:0|[1-9][0-9]{0,9})")
APERTURE_RE = re.compile(r"0x[0-9a-f]+\.\.=0x[0-9a-f]+")
ANSI_SGR_RE = re.compile(r"\x1b\[[0-9;]{1,16}m")
AGENT_RE = re.compile(r"Agent ([1-9][0-9]*) *")
TOP_FIELD_RE = re.compile(
    r"  (ASIC Revision|BDFID|Chip ID|Device Type|Feature|Marketing Name|Name|Node|"
    r"Profile|Uuid|Vendor Name|Wavefront Size): +(.+?) *"
)
FIRMWARE_FIELD_RE = re.compile(r"  (Packet Processor uCode|SDMA engine uCode):: +(.+?) *")
ISA_NAME_RE = re.compile(r"      Name: +(amdgcn-[!-~]+) *")
GIT_HEAD_RE = re.compile(r"[0-9a-f]{40}")
UTC_TIME_RE = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z")
METADATA_AUDIT_RE = re.compile(
    r"pure-Rust runtime audit: OK \(metadata roots=([0-9]+) packages=([0-9]+) "
    r"allowed_build_scripts=([^ ]+) sha256=([0-9a-f]{64}); "
    r"profile=fe2o3\.runtime\.pure-rust\.gfx942\.v1\)"
)
ELF_AUDIT_RE = re.compile(
    r"pure-Rust runtime audit: OK \(ELF bytes=([0-9]+) needed=([0-9]+) "
    r"dynsym=([0-9]+) sha256=([0-9a-f]{64}); "
    r"profile=fe2o3\.runtime\.pure-rust\.gfx942\.v1\)"
)

PURE_KEYS = {
    "amdgpu",
    "amdgpu_srcversion",
    "aperture_gpuvm",
    "aperture_lds",
    "aperture_scratch",
    "boot_id",
    "currentness",
    "descriptors",
    "drm",
    "firmware",
    "gpu_id",
    "kernel",
    "node",
    "partition",
    "pci",
    "profile",
    "profile_sha256",
    "target",
    "unique_id",
    "vram_lost_counter",
    "wavefront",
}


class OracleInputError(ValueError):
    """An oracle input cannot be interpreted without guessing."""


@dataclass(frozen=True, order=True)
class PureRustGpu:
    unique_id: str
    node: int
    gpu_id: int
    pci: str
    render_minor: int
    target: str
    wavefront: int
    vram_lost_counter: int
    boot_id: str


@dataclass(frozen=True, order=True)
class RocminfoGpu:
    unique_id: str
    agent: int
    node: int
    bdf_id: int
    name: str
    wavefront: int
    compute_firmware: int
    sdma_firmware: int
    primary_isa: str


@dataclass(frozen=True)
class RocminfoObservation:
    module_version: str
    runtime_version: str
    xnack_enabled: str
    gpus: tuple[RocminfoGpu, ...]


@dataclass(frozen=True)
class ProvenanceInputs:
    runner_data: bytes
    policy_data: bytes
    auditor_data: bytes
    cargo_lock_data: bytes
    metadata_audit_report_data: bytes
    elf_audit_report_data: bytes
    git_observation_data: bytes
    measurement_time_data: bytes


@dataclass(frozen=True)
class MeasurementProvenance:
    git_head: str
    measurement_utc: str
    metadata_packages: int
    metadata_allowed_build_scripts: str
    metadata_snapshot_sha256: str
    elf_bytes: int
    elf_needed: int
    elf_dynsym: int
    elf_audited_sha256: str


def _read_stable_regular(
    path: Path, maximum: int, label: str, *, require_executable: bool = False
) -> bytes:
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise OracleInputError(f"cannot open {label} {path}: {error}") from error
    try:
        try:
            before = os.fstat(descriptor)
        except OSError as error:
            raise OracleInputError(f"cannot inspect {label} {path}: {error}") from error
        if not stat.S_ISREG(before.st_mode):
            raise OracleInputError(f"{label} is not a regular file: {path}")
        if require_executable and (before.st_mode & 0o111) == 0:
            raise OracleInputError(f"{label} is not executable: {path}")
        if before.st_size < 0 or before.st_size > maximum:
            raise OracleInputError(f"{label} exceeds {maximum} bytes: {path}")
        chunks: list[bytes] = []
        total = 0
        while True:
            try:
                chunk = os.read(descriptor, min(65_536, maximum + 1 - total))
            except OSError as error:
                raise OracleInputError(f"cannot read {label} {path}: {error}") from error
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum:
                raise OracleInputError(f"{label} exceeds {maximum} bytes: {path}")
        try:
            after = os.fstat(descriptor)
        except OSError as error:
            raise OracleInputError(f"cannot re-inspect {label} {path}: {error}") from error
    finally:
        os.close(descriptor)
    identity_before = (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    data = b"".join(chunks)
    if identity_before != identity_after or len(data) != before.st_size:
        raise OracleInputError(f"{label} changed while being read: {path}")
    return data


def _strict_lines(data: bytes, label: str) -> tuple[str, ...]:
    if not data or not data.endswith(b"\n"):
        raise OracleInputError(f"{label} must be non-empty and newline terminated")
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise OracleInputError(f"{label} is not ASCII") from error
    if any(byte != 0x0A and byte != 0x1B and not 0x20 <= byte <= 0x7E for byte in data):
        raise OracleInputError(f"{label} contains an unsupported control byte")
    text = ANSI_SGR_RE.sub("", text)
    if "\x1b" in text or "\r" in text or "\0" in text:
        raise OracleInputError(f"{label} contains an unsupported control sequence")
    lines = tuple(text.splitlines())
    if len(lines) > MAX_LINES:
        raise OracleInputError(f"{label} exceeds {MAX_LINES} lines")
    if any(len(line.encode("ascii")) > MAX_LINE_BYTES for line in lines):
        raise OracleInputError(f"{label} contains a line over {MAX_LINE_BYTES} bytes")
    return lines


def _parse_decimal(value: str, label: str) -> int:
    if DECIMAL_RE.fullmatch(value) is None:
        raise OracleInputError(f"invalid {label}: {value!r}")
    return int(value)


def _single_ascii_line(data: bytes, label: str) -> str:
    lines = _strict_lines(data, label)
    if len(lines) != 1:
        raise OracleInputError(f"{label} must contain exactly one line")
    return lines[0]


def parse_provenance(
    inputs: ProvenanceInputs, pure_executable_data: bytes
) -> MeasurementProvenance:
    git_lines = _strict_lines(inputs.git_observation_data, "Git observation")
    if len(git_lines) != 2:
        raise OracleInputError("Git observation must contain exactly two lines")
    head_key, head_separator, git_head = git_lines[0].partition("=")
    worktree_key, worktree_separator, worktree = git_lines[1].partition("=")
    if (
        head_key != "head"
        or head_separator != "="
        or GIT_HEAD_RE.fullmatch(git_head) is None
        or worktree_key != "worktree"
        or worktree_separator != "="
        or worktree != "clean"
    ):
        raise OracleInputError("Git observation is not an exact clean commit")

    measurement_utc = _single_ascii_line(
        inputs.measurement_time_data, "UTC measurement-time observation"
    )
    if UTC_TIME_RE.fullmatch(measurement_utc) is None:
        raise OracleInputError("UTC measurement-time observation is not canonical")
    try:
        parsed_time = datetime.strptime(measurement_utc, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=timezone.utc
        )
    except ValueError as error:
        raise OracleInputError("UTC measurement-time observation is invalid") from error
    if parsed_time.strftime("%Y-%m-%dT%H:%M:%SZ") != measurement_utc:
        raise OracleInputError("UTC measurement-time observation is not canonical")

    metadata_report = _single_ascii_line(
        inputs.metadata_audit_report_data, "metadata audit report"
    )
    metadata_match = METADATA_AUDIT_RE.fullmatch(metadata_report)
    if metadata_match is None:
        raise OracleInputError("metadata audit report does not match the V1 success schema")
    roots = _parse_decimal(metadata_match.group(1), "metadata audit root count")
    packages = _parse_decimal(metadata_match.group(2), "metadata audit package count")
    allowed_build_scripts = metadata_match.group(3)
    if (
        roots != EXPECTED_METADATA_AUDIT_ROOTS
        or not 4 <= packages <= 65_536
        or allowed_build_scripts != "libc@0.2.189,rustix@1.1.4"
    ):
        raise OracleInputError("metadata audit report differs from the V1 policy gate")

    elf_report = _single_ascii_line(inputs.elf_audit_report_data, "ELF audit report")
    elf_match = ELF_AUDIT_RE.fullmatch(elf_report)
    if elf_match is None:
        raise OracleInputError("ELF audit report does not match the V1 success schema")
    elf_bytes = _parse_decimal(elf_match.group(1), "ELF audit byte count")
    needed = _parse_decimal(elf_match.group(2), "ELF dependency count")
    dynsym = _parse_decimal(elf_match.group(3), "ELF dynamic-symbol count")
    elf_audited_sha256 = elf_match.group(4)
    executable_sha256 = hashlib.sha256(pure_executable_data).hexdigest()
    if (
        elf_bytes != len(pure_executable_data)
        or needed > 4096
        or dynsym > 1_000_000
        or elf_audited_sha256 != executable_sha256
    ):
        raise OracleInputError("ELF audit report is not bound to the supplied executable")

    return MeasurementProvenance(
        git_head=git_head,
        measurement_utc=measurement_utc,
        metadata_packages=packages,
        metadata_allowed_build_scripts=allowed_build_scripts,
        metadata_snapshot_sha256=metadata_match.group(4),
        elf_bytes=elf_bytes,
        elf_needed=needed,
        elf_dynsym=dynsym,
        elf_audited_sha256=elf_audited_sha256,
    )


def parse_pure_rust(data: bytes) -> tuple[PureRustGpu, ...]:
    records: list[PureRustGpu] = []
    node_ids: set[int] = set()
    gpu_ids: set[int] = set()
    pci_addresses: set[str] = set()
    render_minors: set[int] = set()
    for line_number, line in enumerate(_strict_lines(data, "pure-Rust evidence"), 1):
        if not line or line != line.strip() or "  " in line:
            raise OracleInputError(f"malformed pure-Rust evidence line {line_number}")
        fields: dict[str, str] = {}
        render_minor: int | None = None
        for token in line.split(" "):
            if re.fullmatch(r"renderD(?:0|[1-9][0-9]{0,4})", token):
                if render_minor is not None:
                    raise OracleInputError(f"duplicate render node on line {line_number}")
                render_minor = int(token[7:])
                continue
            key, separator, value = token.partition("=")
            if separator != "=" or not key or not value or key in fields:
                raise OracleInputError(
                    f"malformed or duplicate token on pure-Rust line {line_number}"
                )
            fields[key] = value
        if set(fields) != PURE_KEYS or render_minor is None:
            raise OracleInputError(
                f"pure-Rust line {line_number} fields differ from schema: "
                f"missing={sorted(PURE_KEYS - set(fields))}, "
                f"unknown={sorted(set(fields) - PURE_KEYS)}"
            )
        exact = {
            "amdgpu": EXPECTED_AMDGPU_VERSION,
            "amdgpu_srcversion": EXPECTED_AMDGPU_SRCVERSION,
            "aperture_gpuvm": "0x10000..=0x7fffffffffff",
            "aperture_lds": "0x1000000000000..=0x10000ffffffff",
            "aperture_scratch": "0x2000000000000..=0x20000ffffffff",
            "currentness": "contracted-clear",
            "descriptors": "3",
            "drm": "3.64.0",
            "firmware": "192/25",
            "kernel": EXPECTED_KERNEL_RELEASE,
            "partition": "SPX/NPS1",
            "profile": EXPECTED_PROFILE,
            "profile_sha256": EXPECTED_PROFILE_SHA256,
            "target": EXPECTED_TARGET,
            "wavefront": str(EXPECTED_WAVEFRONT),
        }
        for key, expected in exact.items():
            if fields[key] != expected:
                raise OracleInputError(
                    f"pure-Rust line {line_number} {key} is {fields[key]!r}, "
                    f"expected {expected!r}"
                )
        if BOOT_ID_RE.fullmatch(fields["boot_id"]) is None:
            raise OracleInputError(f"invalid boot_id on pure-Rust line {line_number}")
        if UUID_RE.fullmatch(fields["unique_id"]) is None:
            raise OracleInputError(f"invalid unique_id on pure-Rust line {line_number}")
        if PCI_RE.fullmatch(fields["pci"]) is None:
            raise OracleInputError(f"invalid PCI address on pure-Rust line {line_number}")
        for key in ("aperture_lds", "aperture_scratch", "aperture_gpuvm"):
            if APERTURE_RE.fullmatch(fields[key]) is None:
                raise OracleInputError(f"invalid {key} on pure-Rust line {line_number}")
        node = _parse_decimal(fields["node"], "topology node")
        gpu_id = _parse_decimal(fields["gpu_id"], "KFD GPU ID")
        vram_lost_counter = _parse_decimal(
            fields["vram_lost_counter"], "VRAM-lost counter"
        )
        if not 2 <= node <= 257 or not 1 <= gpu_id <= 0xFFFF_FFFF:
            raise OracleInputError(
                f"pure-Rust line {line_number} device number is outside the V1 bounds"
            )
        if vram_lost_counter > 0xFFFF_FFFF:
            raise OracleInputError(
                f"pure-Rust line {line_number} VRAM-lost counter is outside the V1 bounds"
            )
        if not 128 <= render_minor <= 0xFFFF:
            raise OracleInputError(
                f"pure-Rust line {line_number} render minor is outside the V1 bounds"
            )
        if node in node_ids or gpu_id in gpu_ids or fields["pci"] in pci_addresses:
            raise OracleInputError("pure-Rust evidence contains a duplicate device identity")
        if render_minor in render_minors:
            raise OracleInputError("pure-Rust evidence contains a duplicate render minor")
        node_ids.add(node)
        gpu_ids.add(gpu_id)
        pci_addresses.add(fields["pci"])
        render_minors.add(render_minor)
        records.append(
            PureRustGpu(
                unique_id=fields["unique_id"],
                node=node,
                gpu_id=gpu_id,
                pci=fields["pci"],
                render_minor=render_minor,
                target=fields["target"],
                wavefront=int(fields["wavefront"]),
                vram_lost_counter=vram_lost_counter,
                boot_id=fields["boot_id"],
            )
        )
    records.sort()
    if len(records) != EXPECTED_GPU_COUNT:
        raise OracleInputError(
            f"pure-Rust evidence reports {len(records)} GPUs, expected {EXPECTED_GPU_COUNT}"
        )
    if len({record.unique_id for record in records}) != len(records):
        raise OracleInputError("pure-Rust evidence contains a duplicate unique ID")
    if len({record.boot_id for record in records}) != 1:
        raise OracleInputError("pure-Rust evidence crosses boot identities")
    return tuple(records)


def _single_header(headers: dict[str, str], key: str, value: str) -> None:
    if key in headers:
        raise OracleInputError(f"rocminfo repeats {key}")
    headers[key] = value


def parse_rocminfo(data: bytes) -> RocminfoObservation:
    headers: dict[str, str] = {}
    agents: list[tuple[int, dict[str, str], tuple[str, ...]]] = []
    agent_ids: set[int] = set()
    current_id: int | None = None
    current_fields: dict[str, str] = {}
    current_isas: list[str] = []

    def finish_agent() -> None:
        nonlocal current_id, current_fields, current_isas
        if current_id is not None:
            agents.append((current_id, current_fields, tuple(current_isas)))
        current_id = None
        current_fields = {}
        current_isas = []

    for line in _strict_lines(data, "rocminfo output"):
        stripped = line.strip()
        module = re.fullmatch(r"ROCk module version ([0-9]+\.[0-9]+\.[0-9]+) is loaded", stripped)
        if module:
            _single_header(headers, "module_version", module.group(1))
            continue
        if stripped.startswith("Runtime Version:"):
            _single_header(headers, "runtime_version", stripped.split(":", 1)[1].strip())
            continue
        if stripped.startswith("XNACK enabled:"):
            _single_header(headers, "xnack_enabled", stripped.split(":", 1)[1].strip())
            continue
        agent_match = AGENT_RE.fullmatch(line)
        if agent_match:
            finish_agent()
            current_id = int(agent_match.group(1))
            if current_id in agent_ids:
                raise OracleInputError(f"rocminfo repeats Agent {current_id}")
            agent_ids.add(current_id)
            if len(agent_ids) > MAX_AGENTS:
                raise OracleInputError(f"rocminfo exceeds {MAX_AGENTS} agents")
            continue
        if current_id is None:
            continue
        field_match = TOP_FIELD_RE.fullmatch(line)
        if field_match is None:
            field_match = FIRMWARE_FIELD_RE.fullmatch(line)
        if field_match:
            key, value = field_match.groups()
            if key in current_fields:
                raise OracleInputError(f"rocminfo Agent {current_id} repeats {key}")
            current_fields[key] = value
            continue
        isa_match = ISA_NAME_RE.fullmatch(line)
        if isa_match:
            isa = isa_match.group(1)
            if isa in current_isas:
                raise OracleInputError(f"rocminfo Agent {current_id} repeats ISA {isa}")
            current_isas.append(isa)
            if len(current_isas) > MAX_ISAS_PER_AGENT:
                raise OracleInputError(
                    f"rocminfo Agent {current_id} exceeds {MAX_ISAS_PER_AGENT} ISAs"
                )
    finish_agent()

    expected_headers = {
        "module_version": EXPECTED_AMDGPU_VERSION,
        "runtime_version": EXPECTED_HSA_RUNTIME,
        "xnack_enabled": "NO",
    }
    if headers != expected_headers:
        raise OracleInputError(
            f"rocminfo headers differ from profile: observed={headers}, "
            f"expected={expected_headers}"
        )

    gpus: list[RocminfoGpu] = []
    for agent, fields, isas in agents:
        gpu_candidate = fields.get("Device Type") == "GPU" or fields.get("Uuid", "").startswith(
            "GPU-"
        )
        if not gpu_candidate:
            continue
        required = {
            "Device Type",
            "ASIC Revision",
            "BDFID",
            "Chip ID",
            "Feature",
            "Marketing Name",
            "Name",
            "Node",
            "Packet Processor uCode",
            "Profile",
            "SDMA engine uCode",
            "Uuid",
            "Vendor Name",
            "Wavefront Size",
        }
        if not required.issubset(fields):
            raise OracleInputError(
                f"rocminfo Agent {agent} lacks GPU fields: {sorted(required - set(fields))}"
            )
        exact = {
            "Device Type": "GPU",
            "ASIC Revision": "1(0x1)",
            "Chip ID": "29857(0x74a1)",
            "Feature": "KERNEL_DISPATCH",
            "Marketing Name": EXPECTED_MARKETING_NAME,
            "Name": EXPECTED_TARGET,
            "Profile": "BASE_PROFILE",
            "Vendor Name": "AMD",
            "Wavefront Size": "64(0x40)",
        }
        for key, expected in exact.items():
            if fields[key] != expected:
                raise OracleInputError(
                    f"rocminfo Agent {agent} {key} is {fields[key]!r}, expected {expected!r}"
                )
        uuid_match = re.fullmatch(r"GPU-([0-9a-f]{16})", fields["Uuid"])
        if uuid_match is None:
            raise OracleInputError(f"rocminfo Agent {agent} has invalid GPU UUID")
        if tuple(sorted(isas)) != EXPECTED_ISAS:
            raise OracleInputError(
                f"rocminfo Agent {agent} ISA set differs from the exact profile: {sorted(isas)}"
            )
        node = _parse_decimal(fields["Node"], f"rocminfo Agent {agent} node")
        bdf_id = _parse_decimal(fields["BDFID"], f"rocminfo Agent {agent} BDFID")
        compute_firmware = _parse_decimal(
            fields["Packet Processor uCode"], f"rocminfo Agent {agent} compute firmware"
        )
        sdma_firmware = _parse_decimal(
            fields["SDMA engine uCode"], f"rocminfo Agent {agent} SDMA firmware"
        )
        if (
            not 2 <= node <= 257
            or not 0 <= bdf_id <= 0xFFFF
            or compute_firmware != 192
            or sdma_firmware != 25
        ):
            raise OracleInputError(f"rocminfo Agent {agent} identity is outside the V1 bounds")
        gpus.append(
            RocminfoGpu(
                unique_id=uuid_match.group(1),
                agent=agent,
                node=node,
                bdf_id=bdf_id,
                name=fields["Name"],
                wavefront=EXPECTED_WAVEFRONT,
                compute_firmware=compute_firmware,
                sdma_firmware=sdma_firmware,
                primary_isa=EXPECTED_PRIMARY_ISA,
            )
        )
    gpus.sort()
    if len(gpus) != EXPECTED_GPU_COUNT:
        raise OracleInputError(
            f"rocminfo reports {len(gpus)} GPUs, expected {EXPECTED_GPU_COUNT}"
        )
    if len({gpu.unique_id for gpu in gpus}) != len(gpus):
        raise OracleInputError("rocminfo contains duplicate GPU UUIDs")
    return RocminfoObservation(
        module_version=headers["module_version"],
        runtime_version=headers["runtime_version"],
        xnack_enabled=headers["xnack_enabled"],
        gpus=tuple(gpus),
    )


def compare_and_render(
    pure_data: bytes,
    rocminfo_data: bytes,
    rocm_release_data: bytes,
    pure_executable_data: bytes,
    rocminfo_executable_data: bytes,
    checker_data: bytes,
    provenance_inputs: ProvenanceInputs,
) -> str:
    pure = parse_pure_rust(pure_data)
    oracle = parse_rocminfo(rocminfo_data)
    provenance = parse_provenance(provenance_inputs, pure_executable_data)
    try:
        rocm_release = rocm_release_data.decode("ascii")
    except UnicodeDecodeError as error:
        raise OracleInputError("ROCm release is not ASCII") from error
    if rocm_release != f"{EXPECTED_ROCM_RELEASE}\n":
        raise OracleInputError(
            f"ROCm release is {rocm_release!r}, expected {EXPECTED_ROCM_RELEASE!r}"
        )
    pure_ids = tuple(record.unique_id for record in pure)
    oracle_ids = tuple(record.unique_id for record in oracle.gpus)
    if pure_ids != oracle_ids:
        missing_from_oracle = sorted(set(pure_ids) - set(oracle_ids))
        missing_from_pure = sorted(set(oracle_ids) - set(pure_ids))
        raise OracleInputError(
            "GPU UUID sets disagree: "
            f"missing_from_oracle={missing_from_oracle}, "
            f"missing_from_pure_rust={missing_from_pure}"
        )
    for checked, measured in zip(pure, oracle.gpus, strict=True):
        domain, bus, device, function = (
            int(component, 16) for component in re.split(r"[:.]", checked.pci)
        )
        pure_bdf_id = (bus << 8) | (device << 3) | function
        if (
            checked.target != measured.name
            or checked.wavefront != measured.wavefront
            or checked.node != measured.node
            or domain != 0
            or pure_bdf_id != measured.bdf_id
        ):
            raise OracleInputError(f"GPU property disagreement for {checked.unique_id}")

    lines = [
        "schema=fe2o3-r1-device-identity-oracle-measurement-v1",
        "claim_status=Measured",
        "claim_scope=device-identity-differential",
        "authority=none",
        "proof_effect=none",
        "runtime_authority_effect=none",
        "result=match",
        "differential_match_fields=uuid,node,pci-bdf,target,wavefront,firmware",
        "pure_rust_only_fields=currentness,vram_lost_counter",
        "oracle_only_fields=isa",
        "currentness_claim_status=Contracted",
        "currentness=contracted-clear",
        "currentness_source=pure-rust-only",
        "currentness_hsa_comparison=not-performed",
        "vram_lost_counter_source=pure-rust-only",
        "oracle=rocminfo",
        f"rocm_release={EXPECTED_ROCM_RELEASE}",
        f"hsa_runtime={oracle.runtime_version}",
        f"hsa_xnack={oracle.xnack_enabled}",
        f"amdgpu_module={oracle.module_version}",
        f"kernel_release={EXPECTED_KERNEL_RELEASE}",
        f"boot_id={pure[0].boot_id}",
        "device_model=AMD-Instinct-MI300X",
        f"profile={EXPECTED_PROFILE}",
        f"profile_sha256={EXPECTED_PROFILE_SHA256}",
        f"oracle_primary_isa={EXPECTED_PRIMARY_ISA}",
        f"oracle_compat_isa={EXPECTED_ISAS[0]}",
        f"gpu_count={EXPECTED_GPU_COUNT}",
        f"git_head={provenance.git_head}",
        "git_worktree=clean",
        f"measurement_utc_observed={provenance.measurement_utc}",
        "measurement_time_trust=untrusted-host-clock",
        f"runtime_oracle_runner_sha256={hashlib.sha256(provenance_inputs.runner_data).hexdigest()}",
        f"runtime_pure_rust_policy_sha256={hashlib.sha256(provenance_inputs.policy_data).hexdigest()}",
        f"runtime_pure_rust_auditor_sha256={hashlib.sha256(provenance_inputs.auditor_data).hexdigest()}",
        f"cargo_lock_sha256={hashlib.sha256(provenance_inputs.cargo_lock_data).hexdigest()}",
        "metadata_audit_status=passed",
        f"metadata_audit_roots={EXPECTED_METADATA_AUDIT_ROOTS}",
        f"metadata_audit_packages={provenance.metadata_packages}",
        f"metadata_audit_allowed_build_scripts={provenance.metadata_allowed_build_scripts}",
        f"metadata_audit_report_sha256={hashlib.sha256(provenance_inputs.metadata_audit_report_data).hexdigest()}",
        f"metadata_snapshot_sha256={provenance.metadata_snapshot_sha256}",
        "elf_audit_status=passed",
        f"elf_audit_bytes={provenance.elf_bytes}",
        f"elf_audit_needed={provenance.elf_needed}",
        f"elf_audit_dynsym={provenance.elf_dynsym}",
        f"elf_audit_report_sha256={hashlib.sha256(provenance_inputs.elf_audit_report_data).hexdigest()}",
        f"elf_audited_sha256={provenance.elf_audited_sha256}",
        f"pure_rust_output_sha256={hashlib.sha256(pure_data).hexdigest()}",
        f"rocminfo_output_sha256={hashlib.sha256(rocminfo_data).hexdigest()}",
        f"pure_rust_executable_sha256={hashlib.sha256(pure_executable_data).hexdigest()}",
        f"rocminfo_executable_sha256={hashlib.sha256(rocminfo_executable_data).hexdigest()}",
        f"comparator_sha256={hashlib.sha256(checker_data).hexdigest()}",
    ]
    for checked, measured in zip(pure, oracle.gpus, strict=True):
        lines.append(
            f"gpu unique_id={checked.unique_id} node={checked.node} gpu_id={checked.gpu_id} "
            f"pci={checked.pci} renderD{checked.render_minor} oracle_agent={measured.agent} "
            f"oracle_bdf_id={measured.bdf_id} target={checked.target} "
            f"wavefront={checked.wavefront} "
            f"firmware={measured.compute_firmware}/"
            f"{measured.sdma_firmware} isa={measured.primary_isa} differential_match=true "
            f"currentness=contracted-clear currentness_source=pure-rust-only "
            f"vram_lost_counter={checked.vram_lost_counter} "
            f"vram_lost_counter_source=pure-rust-only"
        )
    return "\n".join(lines) + "\n"


def compare_files(args: argparse.Namespace) -> str:
    checker_path = Path(__file__).resolve()
    return compare_and_render(
        _read_stable_regular(
            args.pure_rust_output, MAX_PURE_RUST_BYTES, "pure-Rust evidence"
        ),
        _read_stable_regular(args.rocminfo_output, MAX_ROCMINFO_BYTES, "rocminfo output"),
        _read_stable_regular(args.rocm_release, MAX_VERSION_BYTES, "ROCm release"),
        _read_stable_regular(
            args.pure_rust_executable,
            MAX_EXECUTABLE_BYTES,
            "pure-Rust identity executable",
            require_executable=True,
        ),
        _read_stable_regular(
            args.rocminfo_executable,
            MAX_EXECUTABLE_BYTES,
            "rocminfo executable",
            require_executable=True,
        ),
        _read_stable_regular(checker_path, MAX_EXECUTABLE_BYTES, "oracle comparator"),
        ProvenanceInputs(
            runner_data=_read_stable_regular(
                args.runner,
                MAX_PROVENANCE_SOURCE_BYTES,
                "oracle runner",
                require_executable=True,
            ),
            policy_data=_read_stable_regular(
                args.policy, MAX_PROVENANCE_SOURCE_BYTES, "pure-Rust runtime policy"
            ),
            auditor_data=_read_stable_regular(
                args.auditor,
                MAX_PROVENANCE_SOURCE_BYTES,
                "pure-Rust runtime auditor",
                require_executable=True,
            ),
            cargo_lock_data=_read_stable_regular(
                args.cargo_lock, MAX_PROVENANCE_SOURCE_BYTES, "Cargo lockfile"
            ),
            metadata_audit_report_data=_read_stable_regular(
                args.metadata_audit_report,
                MAX_AUDIT_REPORT_BYTES,
                "metadata audit report",
            ),
            elf_audit_report_data=_read_stable_regular(
                args.elf_audit_report, MAX_AUDIT_REPORT_BYTES, "ELF audit report"
            ),
            git_observation_data=_read_stable_regular(
                args.git_observation,
                MAX_GIT_OBSERVATION_BYTES,
                "Git observation",
            ),
            measurement_time_data=_read_stable_regular(
                args.measurement_time,
                MAX_TIME_OBSERVATION_BYTES,
                "UTC measurement-time observation",
            ),
        ),
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pure-rust-output", required=True, type=Path)
    parser.add_argument("--rocminfo-output", required=True, type=Path)
    parser.add_argument("--rocm-release", required=True, type=Path)
    parser.add_argument("--pure-rust-executable", required=True, type=Path)
    parser.add_argument("--rocminfo-executable", required=True, type=Path)
    parser.add_argument("--runner", required=True, type=Path)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument("--auditor", required=True, type=Path)
    parser.add_argument("--cargo-lock", required=True, type=Path)
    parser.add_argument("--metadata-audit-report", required=True, type=Path)
    parser.add_argument("--elf-audit-report", required=True, type=Path)
    parser.add_argument("--git-observation", required=True, type=Path)
    parser.add_argument("--measurement-time", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        sys.stdout.write(compare_files(args))
    except OracleInputError as error:
        print(f"runtime identity oracle: ERROR: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
