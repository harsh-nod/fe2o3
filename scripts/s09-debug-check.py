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
import resource
import signal
import stat
import string
import struct
import subprocess
import sys
import tempfile

SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

from s09_pinned_snapshot import (  # noqa: E402
    SealedSnapshot,
    SnapshotError,
    sealed_snapshots,
)

MAX_INPUT_BYTES = 16 * 1024 * 1024
MAX_HARDWARE_SECTION_LINES = 128
MAX_IDENTITY_HANDOFF_BYTES = 64 * 1024
MAX_IDENTITY_RECORD_BYTES = 16 * 1024
MAX_IDENTITY_FIELD_NAME_BYTES = 64
MAX_IDENTITY_FIELD_VALUE_BYTES = 4096
MAX_ELF_SECTIONS = 4096
MAX_ELF_STRING_TABLE_BYTES = 1024 * 1024
PRODUCTION_POLICY_PATH = pathlib.Path("/etc/fe2o3/s09-trust-v2.tsv")
LLVM_DWARFDUMP = pathlib.Path("/opt/rocm/llvm/bin/llvm-dwarfdump")
LLVM_READOBJ = pathlib.Path("/opt/rocm/llvm/bin/llvm-readobj")
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
    r" at (?:\.\./sysdeps/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*|native/runtime\.c):[1-9][0-9]*$"
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
S09_SOURCE_SHA256 = "73c1ff5e2f29d245c8071bdb6c1a38af1c9ee1573b78d47a987633483b37e084"
S09_SOURCE_LENGTH = "3359"
MANIFEST_SCHEMA = "fe2o3-s09-protected-manifest-v2"
SEMANTIC_CLAIM_SCHEMA = "fe2o3-s09-semantic-identity-claim-v2"
BUILD_CLAIM_SCHEMA = "fe2o3-s09-build-identity-claim-v2"
S09_IDENTITY_SECTION = ".fe2o3.s09.identity.v2"
S09_IDENTITY_HANDOFF_DOMAIN = b"FE2O3/S09-IDENTITY-HANDOFF/V2\0"
SEMANTIC_CLAIM_FIELDS = (
    "schema",
    "crate",
    "module",
    "logical_name",
    "export_name",
    "profile",
    "source_path",
    "source_sha256",
    "source_bytes",
    "target",
    "target_capabilities",
    "code_object_version",
    "rustc_opt_level",
    "rustc_debug_info",
    "injected_debug_policy",
    "abi_sha256",
    "launch_sha256",
    "portable_mir_sha256",
)
BUILD_CLAIM_FIELDS = (
    "schema",
    "semantic_claim_sha256",
    "cargo_metadata_sha256",
    "crate_binding",
    "kernel_binding",
    "observed_def_path",
    "observed_symbol",
    "rustc_mir_capture_sha256",
    "prepared_rustc_command_sha256",
    "rustc_executable_sha256",
    "cargo_fe2o3_executable_sha256",
    "declared_cargo_executable_sha256",
    "pinned_cargo_image_sha256",
    "observed_parent_pid",
    "observed_parent_start_time_ticks",
    "codegen_backend_sha256",
    "worker_config_sha256",
    "worker_executable_sha256",
    "worker_build_identity_sha256",
    "llvm_build_identity_sha256",
)
IDENTITY_MANIFEST_FIELDS = (
    "identity_section",
    "semantic_claim_sha256",
    *(f"semantic_{field}" for field in SEMANTIC_CLAIM_FIELDS),
    "build_claim_sha256",
    *(f"build_{field}" for field in BUILD_CLAIM_FIELDS),
)
EVIDENCE_MANIFEST_FIELDS = (
    "source_commit",
    "source_tree",
    "hsaco_sha256",
    "host_executable_sha256",
    "host_executable_build_id",
    "debug_archive_manifest_sha256",
    "artifact_facts_sha256",
    "hardware_facts_sha256",
    "dwarf_normalized_sha256",
    "rocgdb_normalized_sha256",
)
MANIFEST_FIELDS = (
    "manifest_schema",
    "trust_domain",
    "claim",
    *IDENTITY_MANIFEST_FIELDS,
    *EVIDENCE_MANIFEST_FIELDS,
)
MANIFEST_FIELD_COUNT = 3 + len(IDENTITY_MANIFEST_FIELDS) + len(EVIDENCE_MANIFEST_FIELDS)
POLICY_SCHEMA = "fe2o3-s09-production-policy-v2"
POLICY_HEADER_FIELDS = (
    "policy_schema",
    "manifest_path",
    "manifest_sha256",
)
POLICY_MANIFEST_BINDINGS = MANIFEST_FIELDS
POLICY_FIELDS = POLICY_HEADER_FIELDS + POLICY_MANIFEST_BINDINGS
ARTIFACT_FACT_FIELDS = (
    "format",
    "object_format",
    "arch",
    "target",
    "optimization",
    "source_path",
    "kernel",
)
HARDWARE_FACT_FIELDS = ("format", "object_format", "sha256", "build_id")
DEBUG_ARCHIVE_MANIFEST_FIELDS = (
    "format",
    "profile",
    "result",
    "target",
    "optimization",
    "rocgdb",
    "llvm_dwarfdump",
    "llvm_readobj",
    "llvm_readelf",
    "hsaco_sha256",
    "hardware_test_sha256",
    "hardware_test_build_id",
    "run_nonce",
    "checker_sha256",
    "artifact_facts_sha256",
    "hardware_facts_sha256",
    "dwarf_normalized_sha256",
    "rocgdb_normalized_sha256",
    "rocgdb_raw_sha256",
    "rocgdb_raw_retained",
    "dwarf_verify_status",
    "dwarf_dump_status",
    "dwarf_normalize_status",
    "dwarf_check_status",
    "artifact_read_status",
    "artifact_check_status",
    "hardware_read_status",
    "hardware_check_status",
    "rocgdb_status",
    "rocgdb_normalize_status",
    "rocgdb_check_status",
)
DEBUG_ARCHIVE_STATUS_FIELDS = DEBUG_ARCHIVE_MANIFEST_FIELDS[-11:]


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
            raise CheckError(
                f"input size must be within 1..{MAX_INPUT_BYTES} bytes: {path}"
            )
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

        def identity(value: os.stat_result) -> tuple[int, int, int, int, int]:
            return (
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


def checked_slice(data: bytes, offset: int, size: int, label: str) -> bytes:
    if offset < 0 or size < 0 or offset > len(data) or size > len(data) - offset:
        raise CheckError(f"{label} is truncated or outside its bounded input")
    return data[offset : offset + size]


def unpack_from(data: bytes, offset: int, encoding: str, label: str) -> int:
    size = struct.calcsize(encoding)
    return int(struct.unpack(encoding, checked_slice(data, offset, size, label))[0])


def elf_string(strings: bytes, offset: int) -> bytes:
    if offset >= len(strings):
        raise CheckError("ELF section name offset is outside the bounded string table")
    end = strings.find(b"\0", offset)
    if end < 0:
        raise CheckError("ELF section name is unterminated")
    return strings[offset:end]


def identity_section_v2(hsaco: bytes) -> bytes:
    if not 1 <= len(hsaco) <= MAX_INPUT_BYTES:
        raise CheckError(f"HSACO must contain 1 through {MAX_INPUT_BYTES} bytes")
    if (
        len(hsaco) < 64
        or hsaco[:4] != b"\x7fELF"
        or hsaco[4] != 2
        or hsaco[5] != 1
        or hsaco[6] != 1
    ):
        raise CheckError("HSACO is not a supported ELF64 little-endian object")

    section_offset = unpack_from(hsaco, 40, "<Q", "ELF section table offset")
    section_entry_size = unpack_from(hsaco, 58, "<H", "ELF section entry size")
    section_count = unpack_from(hsaco, 60, "<H", "ELF section count")
    string_index = unpack_from(hsaco, 62, "<H", "ELF string-table index")
    if section_entry_size != 64:
        raise CheckError("ELF section entry size is not canonical ELF64")
    if not 1 <= section_count <= MAX_ELF_SECTIONS:
        raise CheckError(f"ELF section count must be 1 through {MAX_ELF_SECTIONS}")
    if string_index == 0 or string_index >= section_count or string_index == 0xFFFF:
        raise CheckError("ELF section-name string-table index is invalid or extended")
    checked_slice(
        hsaco,
        section_offset,
        section_entry_size * section_count,
        "ELF section table",
    )

    def section_header(index: int) -> bytes:
        return checked_slice(
            hsaco,
            section_offset + index * section_entry_size,
            section_entry_size,
            "ELF section header",
        )

    string_header = section_header(string_index)
    if unpack_from(string_header, 4, "<I", "ELF string-table type") != 3:
        raise CheckError("ELF section-name table is not SHT_STRTAB")
    string_offset = unpack_from(string_header, 24, "<Q", "ELF string-table offset")
    string_size = unpack_from(string_header, 32, "<Q", "ELF string-table size")
    if not 1 <= string_size <= MAX_ELF_STRING_TABLE_BYTES:
        raise CheckError(
            "ELF section-name table must contain 1 through "
            f"{MAX_ELF_STRING_TABLE_BYTES} bytes"
        )
    strings = checked_slice(
        hsaco, string_offset, string_size, "ELF section-name string table"
    )
    if strings[0] != 0:
        raise CheckError("ELF section-name table has no leading NUL")

    found: bytes | None = None
    for index in range(section_count):
        header = section_header(index)
        name_offset = unpack_from(header, 0, "<I", "ELF section name offset")
        if elf_string(strings, name_offset) != S09_IDENTITY_SECTION.encode("ascii"):
            continue
        if found is not None:
            raise CheckError("HSACO contains duplicate S09 identity sections")
        if unpack_from(header, 4, "<I", "S09 identity section type") != 1:
            raise CheckError("S09 identity section is not SHT_PROGBITS")
        offset = unpack_from(header, 24, "<Q", "S09 identity section offset")
        size = unpack_from(header, 32, "<Q", "S09 identity section size")
        if not 1 <= size <= MAX_IDENTITY_HANDOFF_BYTES:
            raise CheckError(
                "S09 identity section must contain 1 through "
                f"{MAX_IDENTITY_HANDOFF_BYTES} bytes"
            )
        found = checked_slice(hsaco, offset, size, "S09 identity section")
    if found is None:
        raise CheckError("HSACO has no S09 identity section")
    return found


def decode_identity_digest(value: str, field: str) -> None:
    if not HEX_SHA256.fullmatch(value) or value == "0" * 64:
        raise CheckError(f"{field} must be a nonzero lowercase SHA-256 digest")


def decode_identity_decimal(
    value: str, field: str, maximum: int, allow_zero: bool
) -> None:
    if (
        not value.isascii()
        or not value.isdecimal()
        or (len(value) > 1 and value.startswith("0"))
    ):
        raise CheckError(f"{field} is not a canonical decimal")
    decoded = int(value)
    if decoded > maximum:
        raise CheckError(f"{field} exceeds its codec bound")
    if decoded == 0 and not allow_zero:
        raise CheckError(f"{field} must not be zero")


def decode_identity_record(
    record: bytes, expected: tuple[str, ...], label: str
) -> dict[str, str]:
    if not 1 <= len(record) <= MAX_IDENTITY_RECORD_BYTES:
        raise CheckError(
            f"{label} record must contain 1 through {MAX_IDENTITY_RECORD_BYTES} bytes"
        )
    if not record.endswith(b"\n"):
        raise CheckError(f"{label} record is truncated or has trailing data")
    lines = record[:-1].split(b"\n")
    if len(lines) != len(expected):
        raise CheckError(
            f"{label} record has {len(lines)} fields; expected exactly {len(expected)}"
        )
    values: dict[str, str] = {}
    for index, (wanted, line) in enumerate(zip(expected, lines, strict=True)):
        columns = line.split(b"\t")
        if len(columns) != 2:
            raise CheckError(f"{label} field has a missing or duplicate separator")
        name_bytes, value_bytes = columns
        try:
            name = name_bytes.decode("utf-8")
            value = value_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise CheckError(f"{label} field is not UTF-8") from error
        if not 1 <= len(name_bytes) <= MAX_IDENTITY_FIELD_NAME_BYTES or any(
            not (
                byte == ord("_")
                or ord("0") <= byte <= ord("9")
                or ord("a") <= byte <= ord("z")
            )
            for byte in name_bytes
        ):
            raise CheckError(f"{label} field name is noncanonical")
        if not 1 <= len(value_bytes) <= MAX_IDENTITY_FIELD_VALUE_BYTES or any(
            byte < 0x21 or byte > 0x7E for byte in value_bytes
        ):
            raise CheckError(f"{label} field {name!r} is noncanonical")
        if name != wanted:
            raise CheckError(
                f"{label} field {index} must be {wanted}; found unknown, duplicate, "
                f"missing, or reordered field {name}"
            )
        values[name] = value
    return values


def take_identity_record(data: bytes, offset: int, label: str) -> tuple[bytes, int]:
    length = unpack_from(data, offset, "<I", f"{label} length")
    offset += 4
    if not 1 <= length <= MAX_IDENTITY_RECORD_BYTES:
        raise CheckError(
            f"{label} length must be 1 through {MAX_IDENTITY_RECORD_BYTES}"
        )
    return checked_slice(data, offset, length, label), offset + length


def take_identity_digest(data: bytes, offset: int, label: str) -> tuple[str, int]:
    digest = checked_slice(data, offset, 32, label)
    if digest == b"\0" * 32:
        raise CheckError(f"{label} must not be zero")
    return digest.hex(), offset + 32


def decode_hsaco_identity_v2(
    hsaco: bytes,
) -> tuple[bytes, dict[str, str], bytes, dict[str, str]]:
    handoff = identity_section_v2(hsaco)
    if not 1 <= len(handoff) <= MAX_IDENTITY_HANDOFF_BYTES:
        raise CheckError(
            "identity handoff must contain 1 through "
            f"{MAX_IDENTITY_HANDOFF_BYTES} bytes"
        )
    if not handoff.startswith(S09_IDENTITY_HANDOFF_DOMAIN):
        raise CheckError("identity handoff has a missing or unknown domain")
    offset = len(S09_IDENTITY_HANDOFF_DOMAIN)
    semantic_claim_digest, offset = take_identity_digest(
        handoff, offset, "semantic_claim_sha256"
    )
    build_claim_digest, offset = take_identity_digest(
        handoff, offset, "build_claim_sha256"
    )
    semantic_record, offset = take_identity_record(
        handoff, offset, "semantic identity claim"
    )
    build_record, offset = take_identity_record(handoff, offset, "build identity claim")
    if offset != len(handoff):
        raise CheckError("identity handoff has trailing bytes or records")

    semantic = decode_identity_record(
        semantic_record, SEMANTIC_CLAIM_FIELDS, "semantic identity claim"
    )
    build = decode_identity_record(
        build_record, BUILD_CLAIM_FIELDS, "build identity claim"
    )
    if semantic["schema"] != SEMANTIC_CLAIM_SCHEMA:
        raise CheckError("semantic schema is missing or unknown")
    if build["schema"] != BUILD_CLAIM_SCHEMA:
        raise CheckError("build schema is missing or unknown")
    for field in (
        "source_sha256",
        "abi_sha256",
        "launch_sha256",
        "portable_mir_sha256",
    ):
        decode_identity_digest(semantic[field], f"semantic identity claim {field}")
    for field in (
        "semantic_claim_sha256",
        "cargo_metadata_sha256",
        "crate_binding",
        "kernel_binding",
        "rustc_mir_capture_sha256",
        "prepared_rustc_command_sha256",
        "rustc_executable_sha256",
        "cargo_fe2o3_executable_sha256",
        "declared_cargo_executable_sha256",
        "pinned_cargo_image_sha256",
        "codegen_backend_sha256",
        "worker_config_sha256",
        "worker_executable_sha256",
        "worker_build_identity_sha256",
        "llvm_build_identity_sha256",
    ):
        decode_identity_digest(build[field], f"build identity claim {field}")
    decode_identity_decimal(semantic["source_bytes"], "source_bytes", 2**64 - 1, False)
    decode_identity_decimal(
        semantic["code_object_version"], "code_object_version", 2**16 - 1, False
    )
    decode_identity_decimal(semantic["rustc_opt_level"], "rustc_opt_level", 255, True)
    decode_identity_decimal(
        build["observed_parent_pid"], "observed_parent_pid", 2**64 - 1, False
    )
    decode_identity_decimal(
        build["observed_parent_start_time_ticks"],
        "observed_parent_start_time_ticks",
        2**64 - 1,
        False,
    )
    semantic_digest = hashlib.sha256(semantic_record).hexdigest()
    build_digest = hashlib.sha256(build_record).hexdigest()
    if semantic_claim_digest != semantic_digest or build_claim_digest != build_digest:
        raise CheckError(
            "identity handoff manifest does not bind the exact claim records"
        )
    if build["semantic_claim_sha256"] != semantic_digest:
        raise CheckError(
            "build identity claim does not bind the semantic identity claim"
        )
    return semantic_record, semantic, build_record, build


def identity_manifest_values(hsaco: bytes) -> dict[str, str]:
    semantic_record, semantic, build_record, build = decode_hsaco_identity_v2(hsaco)
    values = {
        "identity_section": S09_IDENTITY_SECTION,
        "semantic_claim_sha256": hashlib.sha256(semantic_record).hexdigest(),
        **{f"semantic_{field}": semantic[field] for field in SEMANTIC_CLAIM_FIELDS},
        "build_claim_sha256": hashlib.sha256(build_record).hexdigest(),
        **{f"build_{field}": build[field] for field in BUILD_CLAIM_FIELDS},
    }
    if tuple(values) != IDENTITY_MANIFEST_FIELDS:
        raise AssertionError("identity manifest field construction changed")
    return values


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
            raise CheckError(
                "cannot verify production S09 policy immutable flag"
            ) from error
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
        source_match = re.fullmatch(
            re.escape(S09_SOURCE) + r"(?::(?:68|69|70))?", candidate
        )
        if source_match:
            saw_source = True
            continue
        if candidate.lower().startswith("file:"):
            raise CheckError("normalized evidence contains a file URI")
        if "/" in candidate or "\\" in candidate:
            components = re.split(r"[/\\]", candidate)
            if any(component in {".", ".."} for component in components):
                raise CheckError("normalized evidence contains a dot path component")
            raise CheckError(
                f"normalized evidence contains unallowlisted path atom {candidate!r}"
            )
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
        raise CheckError(
            "ROCgdb transcript must contain one ordered S09 marker interval"
        )
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
        field, separator, value = line.partition("=")
        if not separator or field != expected_field or not value:
            raise CheckError(
                f"{context} field {expected_field!r} is absent or out of order"
            )
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
    ]
    if (
        parse_canonical_facts(text, ARTIFACT_FACT_FIELDS, "protected artifact facts")
        != expected
    ):
        raise CheckError("protected artifact facts contain an unexpected field value")


def check_hardware_fact_schema(text: str, sha256: str, build_id: str) -> None:
    expected = [
        ("format", "fe2o3-s09-hardware-facts-v1"),
        ("object_format", "elf64-x86-64"),
        ("sha256", sha256),
        ("build_id", build_id),
    ]
    if (
        parse_canonical_facts(text, HARDWARE_FACT_FIELDS, "protected hardware facts")
        != expected
    ):
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
    for rejected in (
        "DW_AT_APPLE_optimized",
        "DW_AT_GNU_locviews",
        "DW_TAG_structure_type",
    ):
        if rejected in text:
            raise CheckError(
                f"bounded S09 DWARF contains unsupported construct {rejected!r}"
            )


def artifact_facts(metadata: str, dwarf: str) -> str:
    check_dwarf(normalize_dwarf(dwarf))
    for token in (
        "Format: elf64-amdgpu",
        "Arch: amdgcn",
        "OS/ABI: AMDGPU_HSA (0x40)",
        "ABIVersion: 4",
        "Machine: EM_AMDGPU (0xE0)",
        ".kernarg_segment_align: 8",
        ".kernarg_segment_size: 296",
        ".name:           alpha",
        ".symbol:         alpha.kd",
    ):
        require_once(metadata, token, "AMDGPU artifact metadata")
    kernel_names = re.findall(r"(?m)^(?: {2}- | {4})\.name:\s+(\S+)\s*$", metadata)
    kernel_symbols = re.findall(r"(?m)^\s+\.symbol:\s+(\S+)\s*$", metadata)
    if kernel_names != ["alpha"] or kernel_symbols != ["alpha.kd"]:
        raise CheckError(
            "AMDGPU artifact metadata is not the exact alpha-only kernel set"
        )
    targets = re.findall(
        r"(?m)^amdhsa\.target:\s+'(amdgcn-amd-amdhsa--gfx942:xnack-)'$", metadata
    )
    if targets != ["amdgcn-amd-amdhsa--gfx942:xnack-"]:
        raise CheckError(
            "AMDGPU artifact metadata has no exact unique gfx942:xnack- target"
        )
    return (
        "format=fe2o3-s09-artifact-facts-v1\n"
        "object_format=elf64-amdgpu\n"
        "arch=amdgcn\n"
        "target=gfx942:xnack-\n"
        "optimization=O0\n"
        "source_path=" + S09_SOURCE + "\n"
        "kernel=alpha:alpha.kd\n"
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


def check_rocgdb(
    text: str, hsaco_sha256: str, hardware_sha256: str, build_id: str
) -> str:
    if not HEX_SHA256.fullmatch(hsaco_sha256) or not HEX_SHA256.fullmatch(
        hardware_sha256
    ):
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
    kernel_section = text[
        marker_position["FE2O3_S09_KERNEL_LOAD"] : marker_position[
            "FE2O3_S09_GPU_CONTEXT"
        ]
    ]
    if (
        'Function "alpha" not defined.' not in kernel_section
        or "Breakpoint 1 (alpha) pending." not in kernel_section
    ):
        raise CheckError(
            "ROCgdb did not prove pending alpha resolved after kernel load"
        )
    gpu_section = text[
        marker_position["FE2O3_S09_GPU_CONTEXT"] : marker_position["FE2O3_S09_FUNCTION"]
    ]
    if not re.search(
        r"Switching to Thread <THREAD>, lane 0 \(AMDGPU Lane <LANE>\)", gpu_section
    ):
        raise CheckError("ROCgdb did not select an AMDGPU lane")
    if not re.search(
        r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
        + re.escape(S09_SOURCE)
        + r":68$",
        gpu_section,
    ):
        raise CheckError(
            "ROCgdb thread inventory does not bind alpha to an AMDGPU wave"
        )
    if "Yes memory://<PID>#offset=0x<ADDR>&size=<SIZE>" not in gpu_section:
        raise CheckError("ROCgdb did not report a loaded in-memory AMDGPU code object")
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 1, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":68",
        gpu_section,
    ):
        raise CheckError("ROCgdb did not hit the loaded alpha kernel breakpoint")

    function_section = text[
        marker_position["FE2O3_S09_FUNCTION"] : marker_position["FE2O3_S09_BP2_ARMED"]
    ]
    if f"at {S09_SOURCE}:68" not in function_section:
        raise CheckError("ROCgdb alpha frame is not bound to canonical source line 68")

    bp2_hit = text[
        marker_position["FE2O3_S09_BP2_ARMED"] : marker_position["FE2O3_S09_BP2_STOP"]
    ]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 2, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":69",
        bp2_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP2 line-69 stop")
    bp2_context = text[
        marker_position["FE2O3_S09_BP2_STOP"] : marker_position["FE2O3_S09_ARGUMENTS"]
    ]
    if (
        not re.search(
            r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
            + re.escape(S09_SOURCE)
            + r":69$",
            bp2_context,
        )
        or f"at {S09_SOURCE}:69" not in bp2_context
    ):
        raise CheckError(
            "ROCgdb BP2 observations are not bound to an AMDGPU wave at line 69"
        )

    argument_section = text[
        marker_position["FE2O3_S09_ARGUMENTS"] : marker_position["FE2O3_S09_BP3_ARMED"]
    ]
    bp3_hit = text[
        marker_position["FE2O3_S09_BP3_ARMED"] : marker_position["FE2O3_S09_BP3_STOP"]
    ]
    if not re.search(
        r'Thread <THREAD> "alpha" hit Breakpoint 3, with lanes \[0-63\], alpha .* at '
        + re.escape(S09_SOURCE)
        + r":70",
        bp3_hit,
    ):
        raise CheckError("ROCgdb did not prove the exact BP3 line-70 stop")
    bp3_context = text[
        marker_position["FE2O3_S09_BP3_STOP"] : marker_position["FE2O3_S09_LOCAL"]
    ]
    if (
        not re.search(
            r'(?m)^\* <THREAD> AMDGPU Wave <WAVE> .*"alpha".*'
            + re.escape(S09_SOURCE)
            + r":70$",
            bp3_context,
        )
        or f"at {S09_SOURCE}:70" not in bp3_context
    ):
        raise CheckError(
            "ROCgdb local observation is not bound to an AMDGPU wave at line 70"
        )
    local_section = text[
        marker_position["FE2O3_S09_LOCAL"] : marker_position["FE2O3_S09_RESUME"]
    ]
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
                raise CheckError(
                    f"ROCgdb could not inspect {name!r}: {observations[0]}"
                )
        if observations[0] != EXPECTED_OBSERVATIONS[name]:
            raise CheckError(
                f"ROCgdb observed unexpected {name!r} value: {observations[0]!r}"
            )

    hardware_section = text[
        marker_position["FE2O3_S09_RESUME"] : marker_position["FE2O3_S09_HARDWARE_PASS"]
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
        raise CheckError("ROCgdb transcript lacks the exact bound runner result marker")
    normal_exits = [
        match.start()
        for match in re.finditer(
            r"(?m)^\[Inferior [1-9][0-9]* \(process <PID>\) exited normally\]$",
            hardware_section,
        )
    ]
    if len(normal_exits) != 1 or normal_exits[0] >= hardware_section.index(
        result_marker
    ):
        raise CheckError(
            "ROCgdb runner result was not conditional on a normal inferior exit"
        )

    for rejected in (
        "No symbol",
        "Cannot access memory",
        "The program is not being run",
    ):
        if rejected.lower() in text.lower():
            raise CheckError(f"ROCgdb transcript contains failure marker {rejected!r}")
    return run_nonce


def parse_debug_archive_manifest(
    data: bytes,
    protected_manifest: dict[str, str],
    observed_digests: dict[str, str],
) -> dict[str, str]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError("debug archive manifest is not UTF-8") from error
    values = dict(
        parse_canonical_facts(
            text, DEBUG_ARCHIVE_MANIFEST_FIELDS, "debug archive manifest"
        )
    )
    expected_values = {
        "format": "fe2o3-s09-debug-archive-v2",
        "profile": "s09-alpha-gfx942-o0-v1",
        "result": "passed",
        "target": "gfx942:xnack-",
        "optimization": "O0",
        "rocgdb": "/opt/rocm/bin/rocgdb-py_3.12",
        "llvm_dwarfdump": "/opt/rocm/llvm/bin/llvm-dwarfdump",
        "llvm_readobj": "/opt/rocm/llvm/bin/llvm-readobj",
        "llvm_readelf": (
            "/opt/rocm/llvm/bin/llvm-readobj --elf-output-style=GNU"
        ),
        "hsaco_sha256": observed_digests["hsaco_sha256"],
        "hardware_test_sha256": observed_digests["host_executable_sha256"],
        "hardware_test_build_id": protected_manifest["host_executable_build_id"],
        "artifact_facts_sha256": observed_digests["artifact_facts_sha256"],
        "hardware_facts_sha256": observed_digests["hardware_facts_sha256"],
        "dwarf_normalized_sha256": observed_digests["dwarf_normalized_sha256"],
        "rocgdb_normalized_sha256": observed_digests[
            "rocgdb_normalized_sha256"
        ],
        "rocgdb_raw_retained": "false",
    }
    for field, expected in expected_values.items():
        if values[field] != expected:
            raise CheckError(f"debug archive manifest field {field!r} changed")
    for field in DEBUG_ARCHIVE_STATUS_FIELDS:
        if values[field] != "0":
            raise CheckError(f"debug archive manifest status {field!r} did not pass")
    for field in ("run_nonce", "checker_sha256", "rocgdb_raw_sha256"):
        if not HEX_SHA256.fullmatch(values[field]) or values[field] == "0" * 64:
            raise CheckError(
                f"debug archive manifest field {field!r} is not a nonzero SHA-256 value"
            )
    checker_path = pathlib.Path(__file__).resolve(strict=True)
    if values["checker_sha256"] != file_sha256(checker_path):
        raise CheckError("debug archive manifest does not bind this checker image")
    return values


def serialize_ordered_fields(
    values: dict[str, str], fields: tuple[str, ...], label: str
) -> bytes:
    missing = [field for field in fields if field not in values]
    extra = sorted(set(values).difference(fields))
    if missing or extra:
        raise CheckError(f"{label} fields changed: missing={missing!r} extra={extra!r}")
    lines: list[str] = []
    for field in fields:
        value = values[field]
        if (
            not isinstance(value, str)
            or not value
            or value != value.strip()
            or "\t" in value
            or "\n" in value
            or "\r" in value
            or any(
                ord(character) < 0x20 or ord(character) == 0x7F for character in value
            )
        ):
            raise CheckError(f"{label} field {field!r} is noncanonical")
        lines.append(f"{field}\t{value}\n")
    return "".join(lines).encode("utf-8")


def parse_ordered_fields(
    data: bytes, fields: tuple[str, ...], label: str
) -> dict[str, str]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError(f"{label} is not UTF-8") from error
    if "\r" in text or not text.endswith("\n") or "\n\n" in text:
        raise CheckError(f"{label} is not canonical LF-delimited text")
    lines = text.removesuffix("\n").split("\n")
    if len(lines) != len(fields):
        raise CheckError(f"{label} field count changed")
    values: dict[str, str] = {}
    for expected, line in zip(fields, lines, strict=True):
        columns = line.split("\t")
        if len(columns) != 2 or columns[0] != expected or not columns[1]:
            raise CheckError(f"{label} field {expected!r} is absent or out of order")
        if expected in values:
            raise CheckError(f"{label} field {expected!r} is duplicated")
        values[expected] = columns[1]
    if serialize_ordered_fields(values, fields, label) != data:
        raise CheckError(f"{label} serialization is not canonical")
    return values


def require_nonzero_sha256(
    values: dict[str, str], fields: tuple[str, ...], label: str
) -> None:
    for field in fields:
        if field.endswith("_sha256"):
            value = values[field]
            if not HEX_SHA256.fullmatch(value) or value == "0" * 64:
                raise CheckError(
                    f"{label} field {field!r} is not a nonzero SHA-256 digest"
                )


def validate_manifest_values(manifest: dict[str, str], required_domain: str) -> None:
    if required_domain not in {
        "production-v2",
        "test-fixture-v2",
        "local-capability-v2",
    }:
        raise CheckError("protected manifest trust domain is unsupported")

    expected_values = {
        "manifest_schema": MANIFEST_SCHEMA,
        "claim": "source-debug-evidence-v2",
        "identity_section": S09_IDENTITY_SECTION,
        "semantic_schema": SEMANTIC_CLAIM_SCHEMA,
        "semantic_crate": "fe2o3_typed_alias_spoof",
        "semantic_module": "general_genuine",
        "semantic_logical_name": "alpha",
        "semantic_export_name": "alpha",
        "semantic_profile": "general-scalar-slice-rustc-layout-v3",
        "semantic_source_path": S09_SOURCE,
        "semantic_source_sha256": S09_SOURCE_SHA256,
        "semantic_source_bytes": S09_SOURCE_LENGTH,
        "semantic_target": "gfx942:xnack-",
        "semantic_target_capabilities": "atomics,amd-wave",
        "semantic_code_object_version": "6",
        "semantic_rustc_opt_level": "0",
        "semantic_rustc_debug_info": "full",
        "semantic_injected_debug_policy": "dwarf-v5-full",
        "build_schema": BUILD_CLAIM_SCHEMA,
    }
    for field, expected in expected_values.items():
        if manifest[field] != expected:
            raise CheckError(f"protected manifest field {field!r} changed")
    if manifest["trust_domain"] != required_domain:
        raise CheckError("protected manifest trust domain is not authorized")
    require_nonzero_sha256(manifest, MANIFEST_FIELDS, "protected manifest")
    if manifest["build_semantic_claim_sha256"] != manifest["semantic_claim_sha256"]:
        raise CheckError(
            "protected manifest build claim does not bind the semantic claim"
        )
    for field in ("source_commit", "source_tree"):
        if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", manifest[field]) or set(
            manifest[field]
        ) == {"0"}:
            raise CheckError(
                f"protected manifest {field!r} is not a canonical Git object ID"
            )
    for field in ("build_crate_binding", "build_kernel_binding"):
        if not HEX_SHA256.fullmatch(manifest[field]) or manifest[field] == "0" * 64:
            raise CheckError(
                f"protected manifest {field!r} is not a nonzero binding ID"
            )
    for field in ("build_observed_def_path", "build_observed_symbol"):
        value = manifest[field]
        if not 1 <= len(value.encode("utf-8")) <= MAX_IDENTITY_FIELD_VALUE_BYTES or any(
            ord(character) < 0x21 or ord(character) > 0x7E for character in value
        ):
            raise CheckError(
                f"protected manifest {field!r} is not a canonical observation"
            )
    for field in (
        "build_observed_parent_pid",
        "build_observed_parent_start_time_ticks",
    ):
        decode_identity_decimal(manifest[field], field, 2**64 - 1, False)
    if not HEX_BUILD_ID.fullmatch(manifest["host_executable_build_id"]) or set(
        manifest["host_executable_build_id"]
    ) == {"0"}:
        raise CheckError("protected manifest host executable build ID is malformed")


def parse_protected_manifest(data: bytes, required_domain: str) -> dict[str, str]:
    manifest = parse_ordered_fields(data, MANIFEST_FIELDS, "protected manifest")
    validate_manifest_values(manifest, required_domain)
    return manifest


def parse_production_policy(data: bytes) -> dict[str, str]:
    policy = parse_ordered_fields(data, POLICY_FIELDS, "production S09 policy")
    if policy["policy_schema"] != POLICY_SCHEMA:
        raise CheckError("production S09 policy schema changed")
    require_nonzero_sha256(policy, POLICY_FIELDS, "production S09 policy")
    validate_manifest_values(
        {field: policy[field] for field in MANIFEST_FIELDS}, "production-v2"
    )
    parse_fixed_manifest_path(policy["manifest_path"])
    return policy


def validate_policy_manifest_binding(
    policy: dict[str, str], manifest: dict[str, str]
) -> None:
    for field in POLICY_MANIFEST_BINDINGS:
        if policy[field] != manifest[field]:
            raise CheckError(
                f"production policy does not bind manifest field {field!r}"
            )


def validate_manifest_identity_binding(manifest: dict[str, str], hsaco: bytes) -> None:
    observed_identity = identity_manifest_values(hsaco)
    for field in IDENTITY_MANIFEST_FIELDS:
        if manifest[field] != observed_identity[field]:
            raise CheckError(
                f"protected manifest does not match HSACO identity field {field!r}"
            )


def snapshot_bytes(snapshot: SealedSnapshot) -> bytes:
    if not 1 <= snapshot.size <= MAX_INPUT_BYTES:
        raise CheckError(
            f"sealed evidence input {snapshot.name!r} exceeds the checker bound"
        )
    try:
        data = os.pread(snapshot.descriptor, snapshot.size + 1, 0)
    except OSError as error:
        raise CheckError(
            f"cannot read sealed evidence input {snapshot.name!r}: {error}"
        ) from error
    if len(data) != snapshot.size:
        raise CheckError(f"sealed evidence input {snapshot.name!r} is truncated")
    return data


def snapshot_text(snapshot: SealedSnapshot) -> str:
    try:
        return snapshot_bytes(snapshot).decode("utf-8")
    except UnicodeDecodeError as error:
        raise CheckError(
            f"sealed evidence input {snapshot.name!r} is not UTF-8"
        ) from error


def run_bounded_tool(
    tool: pathlib.Path, arguments: list[str], snapshots: tuple[SealedSnapshot, ...]
) -> bytes:
    descriptors = tuple(snapshot.descriptor for snapshot in snapshots)

    def limit_output_files() -> None:
        resource.setrlimit(
            resource.RLIMIT_FSIZE, (MAX_INPUT_BYTES, MAX_INPUT_BYTES)
        )

    try:
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            process = subprocess.Popen(
                [os.fspath(tool), *arguments],
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                pass_fds=descriptors,
                start_new_session=True,
                preexec_fn=limit_output_files,
            )
            try:
                returncode = process.wait(timeout=60)
            except subprocess.TimeoutExpired as error:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                raise CheckError(
                    f"direct evidence tool exceeded timeout: {tool.name}"
                ) from error
            stdout.seek(0)
            output = stdout.read(MAX_INPUT_BYTES + 1)
            stderr.seek(0)
            error_output = stderr.read(4097)
    except (OSError, subprocess.SubprocessError) as error:
        raise CheckError(
            f"direct evidence tool failed to execute: {tool}: {error}"
        ) from error
    if returncode != 0:
        detail = error_output[:4096].decode("utf-8", "replace").strip()
        raise CheckError(
            f"direct evidence tool failed: {tool.name}: {returncode}: {detail}"
        )
    if len(output) > MAX_INPUT_BYTES:
        raise CheckError(
            f"direct evidence tool output exceeds checker bound: {tool.name}"
        )
    return output


def directly_inspect_objects(
    hsaco: SealedSnapshot,
    host: SealedSnapshot,
    llvm_dwarfdump: pathlib.Path,
    llvm_readobj: pathlib.Path,
) -> tuple[str, str, str]:
    try:
        run_bounded_tool(llvm_dwarfdump, ["--verify", hsaco.proc_path], (hsaco,))
        dwarf_raw = run_bounded_tool(
            llvm_dwarfdump,
            ["--debug-info", "--debug-line", hsaco.proc_path],
            (hsaco,),
        ).decode("utf-8", "strict")
        artifact_raw = run_bounded_tool(
            llvm_readobj,
            ["--file-headers", "--notes", hsaco.proc_path],
            (hsaco,),
        ).decode("utf-8", "strict")
        hardware_raw = run_bounded_tool(
            llvm_readobj,
            ["--elf-output-style=GNU", "--file-header", "--notes", host.proc_path],
            (host,),
        ).decode("utf-8", "strict")
    except UnicodeDecodeError as error:
        raise CheckError("direct evidence tool output is not UTF-8") from error
    derived_artifact = artifact_facts(artifact_raw, dwarf_raw)
    derived_dwarf = normalize_dwarf(dwarf_raw)
    derived_hardware, _ = hardware_facts(hardware_raw, host.sha256)
    return derived_artifact, derived_dwarf, derived_hardware


def check_evidence_bundle(
    manifest: dict[str, str],
    hsaco_path: pathlib.Path,
    host_executable_path: pathlib.Path,
    debug_archive_manifest_path: pathlib.Path,
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
    llvm_dwarfdump: pathlib.Path = LLVM_DWARFDUMP,
    llvm_readobj: pathlib.Path = LLVM_READOBJ,
) -> None:
    inputs = (
        ("hsaco", hsaco_path, False),
        ("host", host_executable_path, False),
        ("debug_archive_manifest", debug_archive_manifest_path, False),
        ("artifact_facts", artifact_path, False),
        ("hardware_facts", hardware_path, False),
        ("dwarf", dwarf_path, False),
        ("rocgdb", rocgdb_path, False),
    )
    try:
        with sealed_snapshots(inputs) as snapshots:
            digest_bindings = {
                "hsaco_sha256": snapshots["hsaco"].sha256,
                "host_executable_sha256": snapshots["host"].sha256,
                "debug_archive_manifest_sha256": snapshots[
                    "debug_archive_manifest"
                ].sha256,
                "artifact_facts_sha256": snapshots["artifact_facts"].sha256,
                "hardware_facts_sha256": snapshots["hardware_facts"].sha256,
                "dwarf_normalized_sha256": snapshots["dwarf"].sha256,
                "rocgdb_normalized_sha256": snapshots["rocgdb"].sha256,
            }
            for digest_field, observed in digest_bindings.items():
                if observed != manifest[digest_field]:
                    raise CheckError(f"evidence object does not match {digest_field!r}")
            hsaco = snapshot_bytes(snapshots["hsaco"])
            debug_archive = parse_debug_archive_manifest(
                snapshot_bytes(snapshots["debug_archive_manifest"]),
                manifest,
                digest_bindings,
            )
            artifact = snapshot_text(snapshots["artifact_facts"])
            hardware = snapshot_text(snapshots["hardware_facts"])
            dwarf = snapshot_text(snapshots["dwarf"])
            rocgdb = snapshot_text(snapshots["rocgdb"])
            validate_manifest_identity_binding(manifest, hsaco)
            if normalize_dwarf(dwarf) != dwarf or normalize_rocgdb(rocgdb) != rocgdb:
                raise CheckError(
                    "authoritative evidence inputs must already be canonical normalized files"
                )
            check_dwarf(dwarf)
            observed_run_nonce = check_rocgdb(
                rocgdb,
                manifest["hsaco_sha256"],
                manifest["host_executable_sha256"],
                manifest["host_executable_build_id"],
            )
            if observed_run_nonce != debug_archive["run_nonce"]:
                raise CheckError(
                    "debug archive run nonce does not bind the ROCgdb transcript"
                )
            check_artifact_fact_schema(artifact)
            check_hardware_fact_schema(
                hardware,
                manifest["host_executable_sha256"],
                manifest["host_executable_build_id"],
            )
            direct_artifact, direct_dwarf, direct_hardware = directly_inspect_objects(
                snapshots["hsaco"],
                snapshots["host"],
                llvm_dwarfdump,
                llvm_readobj,
            )
            if artifact != direct_artifact:
                raise CheckError("artifact facts do not derive from the supplied HSACO")
            if dwarf != direct_dwarf:
                raise CheckError(
                    "normalized DWARF does not derive from the supplied HSACO"
                )
            if hardware != direct_hardware:
                raise CheckError(
                    "hardware facts do not derive from the supplied host executable"
                )
    except SnapshotError as error:
        raise CheckError(f"cannot pin evidence object: {error}") from error


def check_production(
    hsaco_path: pathlib.Path,
    host_executable_path: pathlib.Path,
    debug_archive_manifest_path: pathlib.Path,
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
    manifest = parse_protected_manifest(manifest_bytes, "production-v2")
    validate_policy_manifest_binding(policy, manifest)
    check_evidence_bundle(
        manifest,
        hsaco_path,
        host_executable_path,
        debug_archive_manifest_path,
        artifact_path,
        hardware_path,
        dwarf_path,
        rocgdb_path,
    )


def check_fixture(
    manifest_path: pathlib.Path,
    expected_manifest_sha256: str,
    hsaco_path: pathlib.Path,
    host_executable_path: pathlib.Path,
    debug_archive_manifest_path: pathlib.Path,
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
    llvm_dwarfdump: pathlib.Path,
    llvm_readobj: pathlib.Path,
) -> None:
    if not HEX_SHA256.fullmatch(expected_manifest_sha256):
        raise CheckError("fixture manifest digest must be lowercase SHA-256")
    manifest_bytes = read_bounded_bytes(manifest_path)
    if hashlib.sha256(manifest_bytes).hexdigest() != expected_manifest_sha256:
        raise CheckError(
            "fixture manifest does not match its non-authoritative test digest"
        )
    manifest = parse_protected_manifest(manifest_bytes, "test-fixture-v2")
    check_evidence_bundle(
        manifest,
        hsaco_path,
        host_executable_path,
        debug_archive_manifest_path,
        artifact_path,
        hardware_path,
        dwarf_path,
        rocgdb_path,
        llvm_dwarfdump,
        llvm_readobj,
    )


def check_capability(
    manifest_path: pathlib.Path,
    expected_manifest_sha256: str,
    hsaco_path: pathlib.Path,
    host_executable_path: pathlib.Path,
    debug_archive_manifest_path: pathlib.Path,
    artifact_path: pathlib.Path,
    hardware_path: pathlib.Path,
    dwarf_path: pathlib.Path,
    rocgdb_path: pathlib.Path,
    llvm_dwarfdump: pathlib.Path,
    llvm_readobj: pathlib.Path,
) -> None:
    if not HEX_SHA256.fullmatch(expected_manifest_sha256):
        raise CheckError("capability manifest digest must be lowercase SHA-256")
    manifest_bytes = read_bounded_bytes(manifest_path)
    if hashlib.sha256(manifest_bytes).hexdigest() != expected_manifest_sha256:
        raise CheckError("capability manifest does not match its measured digest")
    manifest = parse_protected_manifest(manifest_bytes, "local-capability-v2")
    check_evidence_bundle(
        manifest,
        hsaco_path,
        host_executable_path,
        debug_archive_manifest_path,
        artifact_path,
        hardware_path,
        dwarf_path,
        rocgdb_path,
        llvm_dwarfdump,
        llvm_readobj,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    def add_evidence_arguments(command: argparse.ArgumentParser) -> None:
        command.add_argument("--hsaco", required=True, type=pathlib.Path)
        command.add_argument("--host-executable", required=True, type=pathlib.Path)
        command.add_argument(
            "--debug-archive-manifest", required=True, type=pathlib.Path
        )
        command.add_argument("--artifact-facts", required=True, type=pathlib.Path)
        command.add_argument("--hardware-facts", required=True, type=pathlib.Path)
        command.add_argument("--dwarf", required=True, type=pathlib.Path)
        command.add_argument("--rocgdb", required=True, type=pathlib.Path)

    def add_nonproduction_tool_arguments(command: argparse.ArgumentParser) -> None:
        command.add_argument(
            "--fixture-llvm-dwarfdump", type=pathlib.Path, default=LLVM_DWARFDUMP
        )
        command.add_argument(
            "--fixture-llvm-readobj", type=pathlib.Path, default=LLVM_READOBJ
        )

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
    command = subparsers.add_parser("identity-fields")
    command.add_argument("--hsaco", required=True, type=pathlib.Path)
    command.add_argument("--output", required=True, type=pathlib.Path)
    command = subparsers.add_parser("check-production")
    add_evidence_arguments(command)
    command = subparsers.add_parser("check-fixture")
    command.add_argument("--manifest", required=True, type=pathlib.Path)
    command.add_argument("--expected-manifest-sha256", required=True)
    add_evidence_arguments(command)
    add_nonproduction_tool_arguments(command)
    command = subparsers.add_parser("check-capability")
    command.add_argument("--manifest", required=True, type=pathlib.Path)
    command.add_argument("--expected-manifest-sha256", required=True)
    add_evidence_arguments(command)
    add_nonproduction_tool_arguments(command)
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
    elif args.command == "identity-fields":
        try:
            with sealed_snapshots((("hsaco", args.hsaco, False),)) as snapshots:
                values = identity_manifest_values(snapshot_bytes(snapshots["hsaco"]))
        except SnapshotError as error:
            raise CheckError(f"cannot pin HSACO identity object: {error}") from error
        write_new(
            args.output,
            serialize_ordered_fields(
                values, IDENTITY_MANIFEST_FIELDS, "S09 identity manifest fields"
            ).decode("ascii"),
        )
    elif args.command == "check-production":
        check_production(
            args.hsaco,
            args.host_executable,
            args.debug_archive_manifest,
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
            args.hsaco,
            args.host_executable,
            args.debug_archive_manifest,
            args.artifact_facts,
            args.hardware_facts,
            args.dwarf,
            args.rocgdb,
            args.fixture_llvm_dwarfdump,
            args.fixture_llvm_readobj,
        )
        print("S09 non-authoritative fixture checker passed")
    elif args.command == "check-capability":
        check_capability(
            args.manifest,
            args.expected_manifest_sha256,
            args.hsaco,
            args.host_executable,
            args.debug_archive_manifest,
            args.artifact_facts,
            args.hardware_facts,
            args.dwarf,
            args.rocgdb,
            args.fixture_llvm_dwarfdump,
            args.fixture_llvm_readobj,
        )
        print("S09 non-authoritative capability manifest V2 accepted")
    else:
        raise AssertionError(args.command)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except CheckError as error:
        print(f"s09-debug-check: {error}", file=sys.stderr)
        raise SystemExit(1)
