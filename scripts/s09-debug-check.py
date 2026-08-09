#!/usr/bin/env python3
"""Normalize and validate the closed S09 DWARF/ROCgdb evidence profiles."""

from __future__ import annotations

import argparse
import array
import fcntl
import hashlib
import os
import pathlib
import re
import stat
import string
import sys

MAX_INPUT_BYTES = 8 * 1024 * 1024
MAX_HARDWARE_SECTION_LINES = 128
PRODUCTION_POLICY_PATH = pathlib.Path("/etc/fe2o3/s09-trust-v1.tsv")
FS_IOC_GETFLAGS = 0x80086601
FS_IMMUTABLE_FL = 0x00000010
ADDRESS = re.compile(r"0x[0-9a-fA-F]+")
THREAD = re.compile(r"\bThread (?:0x[0-9a-fA-F]+|[0-9]+)", re.IGNORECASE)
PROCESS_ID = re.compile(r"\b(?:LWP|process) [0-9]+\b", re.IGNORECASE)
AMDGPU_LANE = re.compile(r"AMDGPU Lane (?!<LANE>)[^]]+\]")
AMDGPU_WAVE = re.compile(r"AMDGPU Wave \S+")
MEMORY_URI = re.compile(r"memory://[0-9]+#offset=0x[0-9a-fA-F]+&size=[0-9]+")
SPACE = re.compile(r"[ \t]+")
HEX_SHA256 = re.compile(r"[0-9a-f]{64}")
HEX_BUILD_ID = re.compile(r"[0-9a-f]{40,64}")
S09_SOURCE = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs"
S09_DIRECTORY = "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src"
PATH_ATOM_CHARACTERS = frozenset(
    string.ascii_letters + string.digits + "._-$%+~:@#?&=<>/,\\"
)
PERCENT_ESCAPE = re.compile(r"%[0-9A-Fa-f]{2}")
WAVE_COORDINATE = re.compile(r"\([0-9]+,[0-9]+,[0-9]+\)/[0-9]+")
HOST_THREAD_FRAME_PREFIX = re.compile(
    r'^[1-9][0-9]* Thread <THREAD> \(process <PID>\) "[A-Za-z0-9_]{1,15}" '
)
HOST_THREAD_FRAME_SUFFIX = re.compile(
    r" at \.\./sysdeps/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*:[1-9][0-9]*$"
)
NORMALIZED_MEMORY_URI = "memory://<PID>#offset=0x<ADDR>&size=<SIZE>"
VARIABLES = ("scale", "input_data", "input_len", "output_data", "output_len", "i")
EXPECTED_OBSERVATIONS = {
    "scale": "1.5",
    "input_data": "(*mut f32) 0x<ADDR>",
    "input_len": "1",
    "output_data": "(*mut f32) 0x<ADDR>",
    "output_len": "1",
    "i": "0",
}
HARDWARE_TEST = "gfx942_cov6_alpha_then_zeta_generated_safe_spi_with_fake_authenticator"
S09_SOURCE_SHA256 = "a02f62a73198b493258224701c4f29e25b3eca02a738bf02c03989d45b77099e"
MANIFEST_SCHEMA = "fe2o3-s09-protected-manifest-v1"
MANIFEST_FIELDS = (
    "manifest_schema",
    "trust_domain",
    "profile",
    "claim",
    "source_commit",
    "source_tree",
    "source_path",
    "source_sha256",
    "target",
    "optimization",
    "rustc_sha256",
    "llvm_link_worker_sha256",
    "lld_sha256",
    "llvm_dwarfdump_sha256",
    "llvm_readobj_sha256",
    "rocgdb_sha256",
    "checker_sha256",
    "harness_source_sha256",
    "hsaco_sha256",
    "host_executable_sha256",
    "host_executable_build_id",
    "artifact_facts_sha256",
    "hardware_facts_sha256",
    "dwarf_normalized_sha256",
    "rocgdb_normalized_sha256",
    "hardware_test",
    "execution_closure",
)
POLICY_SCHEMA = "fe2o3-s09-production-policy-v1"
POLICY_FIELDS = (
    "policy_schema",
    "manifest_path",
    "manifest_sha256",
    "profile",
    "target",
    "source_commit",
    "source_tree",
    "source_sha256",
    "rustc_sha256",
    "llvm_link_worker_sha256",
    "lld_sha256",
    "llvm_dwarfdump_sha256",
    "llvm_readobj_sha256",
    "rocgdb_sha256",
    "checker_sha256",
    "harness_source_sha256",
    "hsaco_sha256",
    "host_executable_sha256",
    "host_executable_build_id",
)
POLICY_MANIFEST_BINDINGS = tuple(
    field
    for field in POLICY_FIELDS
    if field not in {"policy_schema", "manifest_path", "manifest_sha256"}
)
ARTIFACT_FACT_FIELDS = (
    "format",
    "object_format",
    "arch",
    "target",
    "optimization",
    "source_path",
    "kernel",
    "kernel",
)
HARDWARE_FACT_FIELDS = ("format", "object_format", "sha256", "build_id")


class CheckError(Exception):
    pass


def read_bounded_bytes(path: pathlib.Path) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise CheckError(f"cannot open input safely: {path}: {error}") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
            raise CheckError(f"input must be a single-link regular file: {path}")
        if before.st_size == 0 or before.st_size > MAX_INPUT_BYTES:
            raise CheckError(f"input size must be within 1..{MAX_INPUT_BYTES} bytes: {path}")
        chunks: list[bytes] = []
        remaining = MAX_INPUT_BYTES + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
        identity = lambda value: (
            value.st_dev,
            value.st_ino,
            value.st_size,
            value.st_mtime_ns,
            value.st_ctime_ns,
        )
        if identity(before) != identity(after) or len(data) != before.st_size:
            raise CheckError(f"input changed while being read: {path}")
        return data
    except OSError as error:
        raise CheckError(f"cannot read input safely: {path}: {error}") from error
    finally:
        try:
            os.close(descriptor)
        except OSError as error:
            raise CheckError(f"cannot close input safely: {path}: {error}") from error


def read_bounded(path: pathlib.Path) -> str:
    try:
        return read_bounded_bytes(path).decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError(f"input is not UTF-8: {path}") from error


def file_sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(read_bounded_bytes(path)).hexdigest()


def descriptor_identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def validate_production_policy_metadata(
    metadata: os.stat_result, file_flags: int
) -> None:
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != 0
        or metadata.st_mode & 0o222
    ):
        raise CheckError(
            "production S09 policy must be root-owned, single-link, regular, and nonwritable"
        )
    if metadata.st_size == 0 or metadata.st_size > MAX_INPUT_BYTES:
        raise CheckError("production S09 policy size is outside the fixed bound")
    if not file_flags & FS_IMMUTABLE_FL:
        raise CheckError("production S09 policy is not filesystem-immutable")


def read_production_policy() -> bytes:
    path = PRODUCTION_POLICY_PATH
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise CheckError("cannot open fixed production S09 policy") from error
    try:
        before = os.fstat(descriptor)
        file_flags = array.array("I", [0])
        try:
            fcntl.ioctl(descriptor, FS_IOC_GETFLAGS, file_flags, True)
        except OSError as error:
            raise CheckError("cannot verify production S09 policy immutable flag") from error
        validate_production_policy_metadata(before, file_flags[0])
        chunks: list[bytes] = []
        remaining = MAX_INPUT_BYTES + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
        if (
            descriptor_identity(before) != descriptor_identity(after)
            or len(data) != before.st_size
        ):
            raise CheckError("production S09 policy changed while being read")
        return data
    except OSError as error:
        raise CheckError("cannot read fixed production S09 policy") from error
    finally:
        try:
            os.close(descriptor)
        except OSError as error:
            raise CheckError("cannot close fixed production S09 policy") from error


def parse_fixed_manifest_path(value: str) -> pathlib.Path:
    if not value.startswith("/") or "//" in value or "\\" in value:
        raise CheckError("production manifest path is not an exact POSIX absolute path")
    pure = pathlib.PurePosixPath(value)
    if str(pure) != value or any(part in {".", ".."} for part in pure.parts):
        raise CheckError("production manifest path is noncanonical")
    path = pathlib.Path(value)
    try:
        if path.resolve(strict=True) != path:
            raise CheckError("production manifest path does not resolve canonically")
    except OSError as error:
        raise CheckError("production manifest installation is absent") from error
    return path


def write_new(path: pathlib.Path, text: str) -> None:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise CheckError(f"output must be a fresh absolute path: {path}")
    if path.parent.resolve() != path.parent:
        raise CheckError(f"output parent must be canonical: {path.parent}")
    with path.open("x", encoding="utf-8", newline="\n") as output:
        output.write(text)


def path_atoms(text: str) -> list[str]:
    atoms: list[str] = []
    index = 0
    while index < len(text):
        if text[index] not in PATH_ATOM_CHARACTERS:
            index += 1
            continue
        end = index + 1
        while end < len(text) and text[end] in PATH_ATOM_CHARACTERS:
            end += 1
        atoms.append(text[index:end])
        index = end
    return atoms


def require_path_hygiene(text: str) -> None:
    if PERCENT_ESCAPE.search(text):
        raise CheckError("normalized evidence contains percent-encoded path data")
    without_wave_coordinates = WAVE_COORDINATE.sub("<WAVE_COORDINATE>", text)
    saw_source = False
    for atom in path_atoms(without_wave_coordinates):
        candidate = atom.rstrip(",;")
        if candidate == NORMALIZED_MEMORY_URI:
            continue
        if candidate == S09_DIRECTORY:
            saw_source = True
            continue
        source_match = re.fullmatch(re.escape(S09_SOURCE) + r"(?::(?:68|69|70))?", candidate)
        if source_match:
            saw_source = True
            continue
        if candidate.lower().startswith("file:"):
            raise CheckError("normalized evidence contains a file URI")
        if "/" in candidate or "\\" in candidate:
            components = re.split(r"[/\\]", candidate)
            if any(component in {".", ".."} for component in components):
                raise CheckError("normalized evidence contains a dot path component")
            raise CheckError(f"normalized evidence contains unallowlisted path atom {candidate!r}")
    if not saw_source:
        raise CheckError("evidence contains no canonical S09 source path")


def normalize_line(line: str) -> str:
    stripped = line.strip()
    if stripped.startswith("Reading symbols from ") or stripped.startswith("of file /"):
        return "HOST_EXECUTABLE_SYMBOLS_LOADED"
    if stripped.startswith('Using host libthread_db library "/'):
        return "HOST_THREAD_LIBRARY_LOADED"
    line = MEMORY_URI.sub("memory://<PID>#offset=0x<ADDR>&size=<SIZE>", line)
    line = THREAD.sub("Thread <THREAD>", line)
    line = PROCESS_ID.sub("process <PID>", line)
    line = AMDGPU_LANE.sub("AMDGPU Lane <LANE>", line)
    line = AMDGPU_WAVE.sub("AMDGPU Wave <WAVE>", line)
    line = re.sub(r"^(\*? )[0-9]+( +AMDGPU Wave)", r"\1<THREAD>\2", line)
    line = ADDRESS.sub("0x<ADDR>", line)
    if line.lstrip().startswith("Starting program: "):
        line = "Starting program: $HOST_EXECUTABLE"
    line = SPACE.sub(" ", line.strip())
    if HOST_THREAD_FRAME_PREFIX.search(line) and HOST_THREAD_FRAME_SUFFIX.search(line):
        return "HOST_THREAD_FRAME"
    return line


def normalize_dwarf(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    lines = [
        "$HSACO: file format elf64-amdgpu"
        if line.endswith(": file format elf64-amdgpu")
        else line
        for line in lines
    ]
    normalized = "\n".join(line for line in lines if line) + "\n"
    require_path_hygiene(normalized)
    return normalized


def normalize_rocgdb(text: str) -> str:
    lines = [normalize_line(line) for line in text.splitlines()]
    begin = [index for index, line in enumerate(lines) if line == "FE2O3_S09_BEGIN"]
    end = [index for index, line in enumerate(lines) if line == "FE2O3_S09_END"]
    if len(begin) != 1 or len(end) != 1 or begin[0] >= end[0]:
        raise CheckError("ROCgdb transcript must contain one ordered S09 marker interval")
    normalized = "\n".join(line for line in lines[begin[0] : end[0] + 1] if line) + "\n"
    require_path_hygiene(normalized)
    return normalized


def require_once(text: str, token: str, context: str) -> None:
    count = text.count(token)
    if count != 1:
        raise CheckError(f"{context} requires exactly one {token!r}; found {count}")


def parse_canonical_facts(
    text: str, expected_fields: tuple[str, ...], context: str
) -> list[tuple[str, str]]:
    if "\r" in text or not text.endswith("\n") or "\n\n" in text:
        raise CheckError(f"{context} is not canonical LF-delimited text")
    lines = text.removesuffix("\n").split("\n")
    if len(lines) != len(expected_fields):
        raise CheckError(f"{context} field count changed")
    facts: list[tuple[str, str]] = []
    for expected_field, line in zip(expected_fields, lines, strict=True):
        fields = line.split("=")
        if len(fields) != 2 or fields[0] != expected_field or not fields[1]:
            raise CheckError(f"{context} field {expected_field!r} is absent or out of order")
        value = fields[1]
        if value != value.strip() or any(ord(character) < 0x20 for character in value):
            raise CheckError(f"{context} field {expected_field!r} is noncanonical")
        facts.append((expected_field, value))
    return facts


def check_artifact_fact_schema(text: str) -> None:
    expected = [
        ("format", "fe2o3-s09-artifact-facts-v1"),
        ("object_format", "elf64-amdgpu"),
        ("arch", "amdgcn"),
        ("target", "gfx942:xnack-"),
        ("optimization", "O0"),
        ("source_path", S09_SOURCE),
        ("kernel", "alpha:alpha.kd"),
        ("kernel", "zeta:zeta.kd"),
    ]
    if parse_canonical_facts(
        text, ARTIFACT_FACT_FIELDS, "protected artifact facts"
    ) != expected:
        raise CheckError("protected artifact facts contain an unexpected field value")


def check_hardware_fact_schema(text: str, sha256: str, build_id: str) -> None:
    expected = [
        ("format", "fe2o3-s09-hardware-facts-v1"),
        ("object_format", "elf64-x86-64"),
        ("sha256", sha256),
        ("build_id", build_id),
    ]
    if parse_canonical_facts(
        text, HARDWARE_FACT_FIELDS, "protected hardware facts"
    ) != expected:
        raise CheckError("protected hardware facts contain an unexpected field value")


def check_dwarf(text: str) -> None:
    for token in (
        "DW_TAG_compile_unit",
        'DW_AT_producer ("fe2o3 S09 alpha gfx942 O0 v1")',
        "DW_LANG_Rust",
        "DW_TAG_subprogram",
        'DW_AT_name ("alpha")',
        f'DW_AT_comp_dir ("{S09_DIRECTORY}")',
    ):
        require_once(text, token, "DWARF")
    for token in ("DW_AT_decl_line (68)", S09_SOURCE):
        if token not in text:
            raise CheckError(f"DWARF is missing {token!r}")
    dies = re.split(r"(?=^0x<ADDR>: .*DW_TAG_)", text, flags=re.MULTILINE)
    for name in VARIABLES:
        require_once(text, f'DW_AT_name ("{name}")', "DWARF")
        matching = [
            die
            for die in dies
            if f'DW_AT_name ("{name}")' in die
            and "DW_AT_location" in die
            and (
                "DW_TAG_formal_parameter" in die
                if name != "i"
                else "DW_TAG_variable" in die
            )
        ]
        if len(matching) != 1:
            raise CheckError(f"DWARF does not contain one located DIE for {name!r}")
    if "DW_AT_decl_line (70)" not in text:
        raise CheckError("DWARF does not bind local `i` to source line 70")
    for line in (68, 69, 70):
        if not re.search(rf"(?m)^0x<ADDR> +{line} +", text):
            raise CheckError(f"DWARF line table does not contain source line {line}")
    for rejected in ("DW_AT_APPLE_optimized", "DW_AT_GNU_locviews", "DW_TAG_structure_type"):
        if rejected in text:
            raise CheckError(f"bounded S09 DWARF contains unsupported construct {rejected!r}")


def artifact_facts(metadata: str, dwarf: str) -> str:
    check_dwarf(normalize_dwarf(dwarf))
    for token in (
        "Format: elf64-amdgpu",
        "Arch: amdgcn",
        ".name:           alpha",
        ".symbol:         alpha.kd",
        ".name:           zeta",
        ".symbol:         zeta.kd",
    ):
        require_once(metadata, token, "AMDGPU artifact metadata")
    targets = re.findall(
        r"(?m)^amdhsa\.target:\s+'(amdgcn-amd-amdhsa--gfx942:xnack-)'$", metadata
    )
    if targets != ["amdgcn-amd-amdhsa--gfx942:xnack-"]:
        raise CheckError("AMDGPU artifact metadata has no exact unique gfx942:xnack- target")
    return (
        "format=fe2o3-s09-artifact-facts-v1\n"
        "object_format=elf64-amdgpu\n"
        "arch=amdgcn\n"
        "target=gfx942:xnack-\n"
        "optimization=O0\n"
        "source_path=" + S09_SOURCE + "\n"
        "kernel=alpha:alpha.kd\n"
        "kernel=zeta:zeta.kd\n"
    )


def hardware_facts(text: str, sha256: str) -> tuple[str, str]:
    if not HEX_SHA256.fullmatch(sha256):
        raise CheckError("hardware executable SHA-256 must be 64 lowercase hex digits")
    for token in ("Class:", "ELF64", "Machine:", "Advanced Micro Devices X86-64"):
        if token not in text:
            raise CheckError(f"hardware executable identity is missing {token!r}")
    build_ids = re.findall(r"(?m)^\s*Build ID: ([0-9a-f]+)$", text)
    if len(build_ids) != 1 or not HEX_BUILD_ID.fullmatch(build_ids[0]):
        raise CheckError("hardware executable must contain one bounded GNU build ID")
    build_id = build_ids[0]
    facts = (
        "format=fe2o3-s09-hardware-facts-v1\n"
        "object_format=elf64-x86-64\n"
        f"sha256={sha256}\n"
        f"build_id={build_id}\n"
    )
    return facts, build_id


def check_rocgdb(text: str, hsaco_sha256: str, hardware_sha256: str, build_id: str) -> None:
    if not HEX_SHA256.fullmatch(hsaco_sha256) or not HEX_SHA256.fullmatch(hardware_sha256):
        raise CheckError("ROCgdb binding SHA-256 values must be lowercase hex")
    if not HEX_BUILD_ID.fullmatch(build_id):
        raise CheckError("ROCgdb hardware build ID is malformed")
    markers = (
        "FE2O3_S09_BEGIN",
        "FE2O3_S09_BINDING",
        "FE2O3_S09_KERNEL_LOAD",
        "FE2O3_S09_GPU_CONTEXT",
        "FE2O3_S09_FUNCTION",
        "FE2O3_S09_BP2_ARMED",
        "FE2O3_S09_BP2_STOP",
        "FE2O3_S09_ARGUMENTS",
        "FE2O3_S09_BP3_ARMED",
        "FE2O3_S09_BP3_STOP",
        "FE2O3_S09_LOCAL",
        "FE2O3_S09_RESUME",
        "FE2O3_S09_HARDWARE_PASS",
        "FE2O3_S09_ROCGDB_EXIT_STATUS = 0",
        "FE2O3_S09_END",
    )
    positions = []
    for marker in markers:
        require_once(text, marker, "ROCgdb transcript")
        positions.append(text.index(marker))
    if positions != sorted(positions):
        raise CheckError("ROCgdb S09 markers are out of order")

    for token in (
        f"hsaco_sha256 = {hsaco_sha256}",
        f"hardware_sha256 = {hardware_sha256}",
        f"hardware_build_id = {build_id}",
        "target = gfx942:xnack-",
        "optimization = O0",
    ):
        require_once(text, token, "ROCgdb binding")
    run_nonces = re.findall(r"(?m)^run_nonce = ([0-9a-f]{64})$", text)
    if len(run_nonces) != 1:
        raise CheckError("ROCgdb binding requires one lowercase 64-hex run nonce")
    run_nonce = run_nonces[0]

    marker_position = dict(zip(markers, positions, strict=True))
    kernel_section = text[marker_position["FE2O3_S09_KERNEL_LOAD"] : marker_position["FE2O3_S09_GPU_CONTEXT"]]
    if 'Function "alpha" not defined.' not in kernel_section or "Breakpoint 1 (alpha) pending." not in kernel_section:
        raise CheckError("ROCgdb did not prove pending alpha resolved after kernel load")
    gpu_section = text[marker_position["FE2O3_S09_GPU_CONTEXT"] : marker_position["FE2O3_S09_FUNCTION"]]
    if not re.search(r"Switching to Thread <THREAD>, lane 0 \(AMDGPU Lane <LANE>\)", gpu_section):
        raise CheckError("ROCgdb did not select an AMDGPU lane")
    if not re.search(r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*' + re.escape(S09_SOURCE) + r":68$", gpu_section):
        raise CheckError("ROCgdb thread inventory does not bind alpha to an AMDGPU wave")
    if "Yes memory://<PID>#offset=0x<ADDR>&size=<SIZE>" not in gpu_section:
        raise CheckError("ROCgdb did not report a loaded in-memory AMDGPU code object")
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 1, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":68",
        gpu_section,
    ):
        raise CheckError("ROCgdb did not hit the loaded alpha kernel breakpoint")

    function_section = text[marker_position["FE2O3_S09_FUNCTION"] : marker_position["FE2O3_S09_BP2_ARMED"]]
    if f"at {S09_SOURCE}:68" not in function_section:
        raise CheckError("ROCgdb alpha frame is not bound to canonical source line 68")

    bp2_hit = text[marker_position["FE2O3_S09_BP2_ARMED"] : marker_position["FE2O3_S09_BP2_STOP"]]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 2, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":69",
        bp2_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP2 line-69 stop")
    bp2_context = text[marker_position["FE2O3_S09_BP2_STOP"] : marker_position["FE2O3_S09_ARGUMENTS"]]
    if not re.search(
        r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
        + re.escape(S09_SOURCE)
        + r":69$",
        bp2_context,
    ) or f"at {S09_SOURCE}:69" not in bp2_context:
        raise CheckError("ROCgdb BP2 observations are not bound to an AMDGPU wave at line 69")

    argument_section = text[marker_position["FE2O3_S09_ARGUMENTS"] : marker_position["FE2O3_S09_BP3_ARMED"]]
    bp3_hit = text[marker_position["FE2O3_S09_BP3_ARMED"] : marker_position["FE2O3_S09_BP3_STOP"]]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 3, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":70",
        bp3_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP3 line-70 stop")
    bp3_context = text[marker_position["FE2O3_S09_BP3_STOP"] : marker_position["FE2O3_S09_LOCAL"]]
    if not re.search(
        r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
        + re.escape(S09_SOURCE)
        + r":70$",
        bp3_context,
    ) or f"at {S09_SOURCE}:70" not in bp3_context:
        raise CheckError("ROCgdb local observation is not bound to an AMDGPU wave at line 70")
    local_section = text[marker_position["FE2O3_S09_LOCAL"] : marker_position["FE2O3_S09_RESUME"]]
    for name in VARIABLES:
        section = local_section if name == "i" else argument_section
        observations = re.findall(rf"(?m)^{re.escape(name)}\s*=\s*(\S.*)$", section)
        if len(observations) != 1:
            raise CheckError(
                f"ROCgdb requires one successful {name!r} observation; found {len(observations)}"
            )
        value = observations[0].lower()
        for rejected in (
            "<optimized out>",
            "<unavailable>",
            "could not find the frame base",
            "error reading variable",
        ):
            if rejected in value:
                raise CheckError(f"ROCgdb could not inspect {name!r}: {observations[0]}")
        if observations[0] != EXPECTED_OBSERVATIONS[name]:
            raise CheckError(f"ROCgdb observed unexpected {name!r} value: {observations[0]!r}")

    hardware_section = text[
        marker_position["FE2O3_S09_RESUME"]
        : marker_position["FE2O3_S09_HARDWARE_PASS"]
    ]
    if len(hardware_section.splitlines()) > MAX_HARDWARE_SECTION_LINES:
        raise CheckError("ROCgdb hardware result section exceeds its fixed line bound")
    result_marker = (
        "FE2O3_S09_HARNESS_RESULT_V1 "
        f"hsaco_sha256={hsaco_sha256} run_nonce={run_nonce} result=passed"
    )
    result_lines = [
        line
        for line in hardware_section.splitlines()
        if line.startswith("FE2O3_S09_HARNESS_RESULT_V1")
    ]
    if result_lines != [result_marker]:
        raise CheckError("ROCgdb transcript lacks the exact bound harness result marker")
    normal_exits = [
        match.start()
        for match in re.finditer(
            r"(?m)^\[Inferior [1-9][0-9]* \(process <PID>\) exited normally\]$",
            hardware_section,
        )
    ]
    if len(normal_exits) != 1 or hardware_section.index(result_marker) >= normal_exits[0]:
        raise CheckError("ROCgdb inferior did not exit normally after the harness result")

    for rejected in ("No symbol", "Cannot access memory", "The program is not being run"):
        if rejected.lower() in text.lower():
            raise CheckError(f"ROCgdb transcript contains failure marker {rejected!r}")


def parse_protected_manifest(data: bytes, required_domain: str) -> dict[str, str]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError("protected manifest is not UTF-8") from error
    if "\r" in text or not text.endswith("\n") or "\n\n" in text:
        raise CheckError("protected manifest is not canonical LF-delimited text")
    lines = text.removesuffix("\n").split("\n")
    if len(lines) != len(MANIFEST_FIELDS):
        raise CheckError("protected manifest field count changed")
    manifest: dict[str, str] = {}
    for expected, line in zip(MANIFEST_FIELDS, lines, strict=True):
        fields = line.split("\t")
        if len(fields) != 2 or fields[0] != expected or not fields[1]:
            raise CheckError(f"protected manifest field {expected!r} is absent or out of order")
        if fields[1] != fields[1].strip() or any(ord(character) < 0x20 for character in fields[1]):
            raise CheckError(f"protected manifest field {expected!r} is noncanonical")
        manifest[expected] = fields[1]

    expected_values = {
        "manifest_schema": MANIFEST_SCHEMA,
        "profile": "s09-alpha-gfx942-o0-v1",
        "claim": "authoritative-source-debug",
        "source_path": S09_SOURCE,
        "source_sha256": S09_SOURCE_SHA256,
        "target": "gfx942:xnack-",
        "optimization": "O0",
        "hardware_test": HARDWARE_TEST,
        "execution_closure": "protected-controller-v1",
    }
    for field, expected in expected_values.items():
        if manifest[field] != expected:
            raise CheckError(f"protected manifest field {field!r} changed")
    if manifest["trust_domain"] != required_domain:
        raise CheckError("protected manifest trust domain is not authorized")
    for field in ("source_commit", "source_tree"):
        if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", manifest[field]):
            raise CheckError(f"protected manifest {field!r} is not a canonical Git object ID")
    for field in MANIFEST_FIELDS:
        if field.endswith("_sha256") and not HEX_SHA256.fullmatch(manifest[field]):
            raise CheckError(f"protected manifest {field!r} is not a SHA-256 digest")
    if not HEX_BUILD_ID.fullmatch(manifest["host_executable_build_id"]):
        raise CheckError("protected manifest host executable build ID is malformed")
    return manifest


def parse_production_policy(data: bytes) -> dict[str, str]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError("production S09 policy is not UTF-8") from error
    if "\r" in text or not text.endswith("\n") or "\n\n" in text:
        raise CheckError("production S09 policy is not canonical LF-delimited text")
    lines = text.removesuffix("\n").split("\n")
    if len(lines) != len(POLICY_FIELDS):
        raise CheckError("production S09 policy field count changed")
    policy: dict[str, str] = {}
    for expected, line in zip(POLICY_FIELDS, lines, strict=True):
        fields = line.split("\t")
        if len(fields) != 2 or fields[0] != expected or not fields[1]:
            raise CheckError(f"production S09 policy field {expected!r} is absent or out of order")
        if fields[1] != fields[1].strip() or any(ord(character) < 0x20 for character in fields[1]):
            raise CheckError(f"production S09 policy field {expected!r} is noncanonical")
        policy[expected] = fields[1]
    if policy["policy_schema"] != POLICY_SCHEMA:
        raise CheckError("production S09 policy schema changed")
    if policy["profile"] != "s09-alpha-gfx942-o0-v1" or policy["target"] != "gfx942:xnack-":
        raise CheckError("production S09 policy profile or target changed")
    if not HEX_SHA256.fullmatch(policy["manifest_sha256"]):
        raise CheckError("production S09 policy manifest digest is malformed")
    for field in POLICY_FIELDS:
        if field.endswith("_sha256") and not HEX_SHA256.fullmatch(policy[field]):
            raise CheckError(f"production S09 policy field {field!r} is not SHA-256")
    for field in ("source_commit", "source_tree"):
        if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", policy[field]):
            raise CheckError(f"production S09 policy field {field!r} is not a Git object ID")
    if not HEX_BUILD_ID.fullmatch(policy["host_executable_build_id"]):
        raise CheckError("production S09 policy host build ID is malformed")
    parse_fixed_manifest_path(policy["manifest_path"])
    return policy


def check_evidence_bundle(
    manifest: dict[str, str],
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
) -> None:
    evidence = {
        "artifact_facts_sha256": artifact_path,
        "hardware_facts_sha256": hardware_path,
        "dwarf_normalized_sha256": dwarf_path,
        "rocgdb_normalized_sha256": rocgdb_path,
    }
    for digest_field, path in evidence.items():
        if file_sha256(path) != manifest[digest_field]:
            raise CheckError(f"evidence file does not match {digest_field!r}")
    checker_path = pathlib.Path(__file__).resolve(strict=True)
    if file_sha256(checker_path) != manifest["checker_sha256"]:
        raise CheckError("protected manifest does not bind this exact checker")

    artifact = read_bounded(artifact_path)
    hardware = read_bounded(hardware_path)
    dwarf = read_bounded(dwarf_path)
    rocgdb = read_bounded(rocgdb_path)
    if normalize_dwarf(dwarf) != dwarf or normalize_rocgdb(rocgdb) != rocgdb:
        raise CheckError("authoritative evidence inputs must already be canonical normalized files")
    check_dwarf(dwarf)
    check_rocgdb(
        rocgdb,
        manifest["hsaco_sha256"],
        manifest["host_executable_sha256"],
        manifest["host_executable_build_id"],
    )
    check_artifact_fact_schema(artifact)
    check_hardware_fact_schema(
        hardware,
        manifest["host_executable_sha256"],
        manifest["host_executable_build_id"],
    )


def check_production(
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
) -> None:
    policy = parse_production_policy(read_production_policy())
    manifest_path = parse_fixed_manifest_path(policy["manifest_path"])
    manifest_bytes = read_bounded_bytes(manifest_path)
    if hashlib.sha256(manifest_bytes).hexdigest() != policy["manifest_sha256"]:
        raise CheckError("installed production manifest does not match fixed policy")
    manifest = parse_protected_manifest(manifest_bytes, "production-v1")
    for field in POLICY_MANIFEST_BINDINGS:
        if policy[field] != manifest[field]:
            raise CheckError(f"production policy does not bind manifest field {field!r}")
    check_evidence_bundle(manifest, artifact_path, hardware_path, dwarf_path, rocgdb_path)


def check_fixture(
    manifest_path: pathlib.Path,
    expected_manifest_sha256: str,
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
) -> None:
    if not HEX_SHA256.fullmatch(expected_manifest_sha256):
        raise CheckError("fixture manifest digest must be lowercase SHA-256")
    manifest_bytes = read_bounded_bytes(manifest_path)
    if hashlib.sha256(manifest_bytes).hexdigest() != expected_manifest_sha256:
        raise CheckError("fixture manifest does not match its non-authoritative test digest")
    manifest = parse_protected_manifest(manifest_bytes, "test-fixture-v1")
    check_evidence_bundle(manifest, artifact_path, hardware_path, dwarf_path, rocgdb_path)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    def add_evidence_arguments(command: argparse.ArgumentParser) -> None:
        command.add_argument("--artifact-facts", required=True, type=pathlib.Path)
        command.add_argument("--hardware-facts", required=True, type=pathlib.Path)
        command.add_argument("--dwarf", required=True, type=pathlib.Path)
        command.add_argument("--rocgdb", required=True, type=pathlib.Path)

    for name in ("normalize-dwarf", "normalize-rocgdb"):
        command = subparsers.add_parser(name)
        command.add_argument("--input", required=True, type=pathlib.Path)
        command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-dwarf")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-rocgdb")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command.add_argument("--hsaco-sha256", required=True)
    command.add_argument("--hardware-sha256", required=True)
    command.add_argument("--hardware-build-id", required=True)
    command = subparsers.add_parser("artifact-facts")
    command.add_argument("--metadata", required=True, type=pathlib.Path)
    command.add_argument("--dwarf", required=True, type=pathlib.Path)
    command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("hardware-facts")
    command.add_argument("--input", required=True, type=pathlib.Path)
    command.add_argument("--sha256", required=True)
    command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-production")
    add_evidence_arguments(command)
    command = subparsers.add_parser("check-fixture")
    command.add_argument("--manifest", required=True, type=pathlib.Path)
    command.add_argument("--expected-manifest-sha256", required=True)
    add_evidence_arguments(command)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "normalize-dwarf":
        write_new(args.output, normalize_dwarf(read_bounded(args.input)))
    elif args.command == "normalize-rocgdb":
        write_new(args.output, normalize_rocgdb(read_bounded(args.input)))
    elif args.command == "check-dwarf":
        check_dwarf(normalize_dwarf(read_bounded(args.input)))
    elif args.command == "check-rocgdb":
        check_rocgdb(
            normalize_rocgdb(read_bounded(args.input)),
            args.hsaco_sha256,
            args.hardware_sha256,
            args.hardware_build_id,
        )
    elif args.command == "artifact-facts":
        write_new(
            args.output,
            artifact_facts(read_bounded(args.metadata), read_bounded(args.dwarf)),
        )
    elif args.command == "hardware-facts":
        facts, _ = hardware_facts(read_bounded(args.input), args.sha256)
        write_new(args.output, facts)
    elif args.command == "check-production":
        check_production(
            args.artifact_facts,
            args.hardware_facts,
            args.dwarf,
            args.rocgdb,
        )
        print("S09 production trust policy accepted protected evidence")
    elif args.command == "check-fixture":
        check_fixture(
            args.manifest,
            args.expected_manifest_sha256,
            args.artifact_facts,
            args.hardware_facts,
            args.dwarf,
            args.rocgdb,
        )
        print("S09 non-authoritative fixture checker passed")
    else:
        raise AssertionError(args.command)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CheckError as error:
        print(f"s09-debug-check: {error}", file=sys.stderr)
        raise SystemExit(1)
