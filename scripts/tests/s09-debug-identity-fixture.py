#!/usr/bin/env python3
"""Create a minimal canonical ELF carrying the S09 identity V2 test records."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import pathlib
import struct
import sys


CHECKER_PATH = pathlib.Path(__file__).parents[1] / "s09-debug-check.py"
SPEC = importlib.util.spec_from_file_location("s09_debug_check", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
sys.dont_write_bytecode = True
SPEC.loader.exec_module(CHECKER)


def digest(character: str) -> str:
    return character * 64


def identity_handoff() -> bytes:
    semantic = {
        "schema": CHECKER.SEMANTIC_CLAIM_SCHEMA,
        "crate": "fe2o3_typed_alias_spoof",
        "module": "general_genuine",
        "logical_name": "alpha",
        "export_name": "alpha",
        "profile": "general-scalar-slice-rustc-layout-v3",
        "source_path": CHECKER.S09_SOURCE,
        "source_sha256": CHECKER.S09_SOURCE_SHA256,
        "source_bytes": CHECKER.S09_SOURCE_LENGTH,
        "target": "gfx942:xnack-",
        "target_capabilities": "atomics,amd-wave",
        "code_object_version": "6",
        "rustc_opt_level": "0",
        "rustc_debug_info": "full",
        "injected_debug_policy": "dwarf-v5-full",
        "abi_sha256": digest("1"),
        "launch_sha256": digest("2"),
        "portable_mir_sha256": digest("3"),
    }
    semantic_record = CHECKER.serialize_ordered_fields(
        semantic, CHECKER.SEMANTIC_CLAIM_FIELDS, "semantic identity claim"
    )
    build = {
        "schema": CHECKER.BUILD_CLAIM_SCHEMA,
        "semantic_claim_sha256": hashlib.sha256(semantic_record).hexdigest(),
        "cargo_metadata_sha256": digest("4"),
        "crate_binding": digest("5"),
        "kernel_binding": digest("6"),
        "observed_def_path": "metadata_a::kernel_a",
        "observed_symbol": "metadata_a_kernel_a",
        "rustc_mir_capture_sha256": digest("7"),
        "prepared_rustc_command_sha256": digest("8"),
        "rustc_executable_sha256": digest("9"),
        "cargo_fe2o3_executable_sha256": digest("a"),
        "declared_cargo_executable_sha256": digest("b"),
        "pinned_cargo_image_sha256": digest("c"),
        "observed_parent_pid": "4242",
        "observed_parent_start_time_ticks": "9001",
        "codegen_backend_sha256": digest("d"),
        "worker_config_sha256": digest("e"),
        "worker_executable_sha256": digest("f"),
        "worker_build_identity_sha256": digest("1"),
        "llvm_build_identity_sha256": digest("2"),
    }
    build_record = CHECKER.serialize_ordered_fields(
        build, CHECKER.BUILD_CLAIM_FIELDS, "build identity claim"
    )
    semantic_digest = hashlib.sha256(semantic_record).digest()
    build_digest = hashlib.sha256(build_record).digest()
    return (
        CHECKER.S09_IDENTITY_HANDOFF_DOMAIN
        + semantic_digest
        + build_digest
        + struct.pack("<I", len(semantic_record))
        + semantic_record
        + struct.pack("<I", len(build_record))
        + build_record
    )


def elf(identity: bytes) -> bytes:
    names = b"\0.shstrtab\0" + CHECKER.S09_IDENTITY_SECTION.encode("ascii") + b"\0"
    name_offset = len(b"\0.shstrtab\0")
    section_offset = 64 + len(names) + len(identity)
    image = bytearray(section_offset + 3 * 64)
    image[:4] = b"\x7fELF"
    image[4:7] = b"\x02\x01\x01"
    struct.pack_into("<Q", image, 40, section_offset)
    struct.pack_into("<H", image, 58, 64)
    struct.pack_into("<H", image, 60, 3)
    struct.pack_into("<H", image, 62, 1)
    image[64 : 64 + len(names)] = names
    image[64 + len(names) : section_offset] = identity

    string_header = section_offset + 64
    struct.pack_into("<I", image, string_header, 1)
    struct.pack_into("<I", image, string_header + 4, 3)
    struct.pack_into("<Q", image, string_header + 24, 64)
    struct.pack_into("<Q", image, string_header + 32, len(names))
    identity_header = section_offset + 128
    struct.pack_into("<I", image, identity_header, name_offset)
    struct.pack_into("<I", image, identity_header + 4, 1)
    struct.pack_into("<Q", image, identity_header + 24, 64 + len(names))
    struct.pack_into("<Q", image, identity_header + 32, len(identity))
    return bytes(image)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=pathlib.Path)
    args = parser.parse_args()
    if not args.output.is_absolute() or args.output.exists() or args.output.is_symlink():
        raise SystemExit("output must be a fresh absolute path")
    if args.output.parent.resolve() != args.output.parent:
        raise SystemExit("output parent must be canonical")
    args.output.write_bytes(elf(identity_handoff()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
