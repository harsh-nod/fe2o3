#!/usr/bin/env python3
"""Audit a runtime Cargo closure or ELF against the pure-Rust runtime policy."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_POLICY = REPO_ROOT / "scripts" / "runtime-pure-rust-policy.json"
MAX_INPUT_BYTES = 512 * 1024 * 1024
MAX_POLICY_ENTRIES = 4096
MAX_METADATA_PACKAGES = 65536
MAX_DEPENDENCIES_PER_PACKAGE = 65536
MAX_ROOT_PACKAGES = 32
MAX_ELF_PROGRAM_HEADERS = 4096
MAX_ELF_SECTION_HEADERS = 65535
MAX_DYNAMIC_ENTRIES = 65536
MAX_DYNAMIC_STRING_BYTES = 64 * 1024 * 1024
MAX_DYNAMIC_SYMBOLS = 1_000_000

PT_LOAD = 1
PT_DYNAMIC = 2
SHT_STRTAB = 3
SHT_NOBITS = 8
SHT_DYNSYM = 11
DT_NULL = 0
DT_NEEDED = 1
DT_STRTAB = 5
DT_STRSZ = 10


class AuditInputError(ValueError):
    """The policy or audited input cannot be interpreted without guessing."""


def _require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise AuditInputError(f"{context} must be an object")
    return value


def _require_list(value: Any, context: str) -> list[Any]:
    if not isinstance(value, list):
        raise AuditInputError(f"{context} must be a list")
    return value


def _require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise AuditInputError(f"{context} must be a non-empty string")
    return value


def _require_string_list(value: Any, context: str) -> tuple[str, ...]:
    entries = tuple(
        _require_string(entry, f"entry in {context}")
        for entry in _require_list(value, context)
    )
    if len(set(entries)) != len(entries):
        raise AuditInputError(f"{context} contains duplicate entries")
    if len(entries) > MAX_POLICY_ENTRIES:
        raise AuditInputError(f"{context} exceeds {MAX_POLICY_ENTRIES} entries")
    if tuple(sorted(entries)) != entries:
        raise AuditInputError(f"{context} must be sorted")
    return entries


def _load_json(path: Path, context: str) -> dict[str, Any]:
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise AuditInputError(f"cannot read {context} {path}: {error}") from error
    if len(raw) > MAX_INPUT_BYTES:
        raise AuditInputError(f"{context} exceeds {MAX_INPUT_BYTES} bytes")
    try:
        return _require_object(json.loads(raw), context)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise AuditInputError(f"cannot parse {context} {path}: {error}") from error


def load_policy(path: Path) -> dict[str, Any]:
    policy = _load_json(path, "policy")
    expected = {
        "schema_version",
        "profile",
        "allowed_dynamic_dependencies",
        "forbidden_package_substrings",
        "forbidden_dynamic_dependency_substrings",
        "forbidden_dynamic_symbol_prefixes",
        "forbidden_binary_literals",
        "reject_cargo_links",
        "reject_cargo_build_scripts",
    }
    if set(policy) != expected:
        missing = sorted(expected - set(policy))
        unknown = sorted(set(policy) - expected)
        raise AuditInputError(
            f"policy keys differ from schema: missing={missing}, unknown={unknown}"
        )
    if policy["schema_version"] != 1:
        raise AuditInputError("policy schema_version must be 1")
    _require_string(policy["profile"], "policy profile")
    for key in (
        "allowed_dynamic_dependencies",
        "forbidden_package_substrings",
        "forbidden_dynamic_dependency_substrings",
        "forbidden_dynamic_symbol_prefixes",
        "forbidden_binary_literals",
    ):
        values = _require_string_list(policy[key], f"policy {key}")
        if key != "allowed_dynamic_dependencies" and any(
            value.lower() != value for value in values
        ):
            raise AuditInputError(f"policy {key} entries must be lowercase")
        policy[key] = values
    for key in ("reject_cargo_links", "reject_cargo_build_scripts"):
        if policy[key] is not True:
            raise AuditInputError(f"policy {key} must be true for schema version 1")
    return policy


def _production_dependency_ids(node: dict[str, Any]) -> tuple[str, ...]:
    result: set[str] = set()
    dependencies = _require_list(node.get("deps"), "Cargo resolve node deps")
    if len(dependencies) > MAX_DEPENDENCIES_PER_PACKAGE:
        raise AuditInputError(
            "Cargo resolve node exceeds the dependency-count bound"
        )
    for index, raw_dependency in enumerate(dependencies):
        dependency = _require_object(raw_dependency, f"resolve dependency {index}")
        package_id = _require_string(dependency.get("pkg"), "resolved package id")
        dependency_kinds = _require_list(
            dependency.get("dep_kinds"), f"dependency kinds for {package_id}"
        )
        if not dependency_kinds:
            result.add(package_id)
            continue
        for raw_kind in dependency_kinds:
            kind = _require_object(raw_kind, f"dependency kind for {package_id}").get(
                "kind"
            )
            if kind not in (None, "build", "dev"):
                raise AuditInputError(
                    f"unknown Cargo dependency kind {kind!r} for {package_id}"
                )
            if kind != "dev":
                result.add(package_id)
    return tuple(sorted(result))


def audit_metadata(
    metadata: dict[str, Any], roots: tuple[str, ...], policy: dict[str, Any]
) -> tuple[list[str], dict[str, int]]:
    if metadata.get("version") != 1:
        raise AuditInputError("Cargo metadata format version must be 1")
    if not roots:
        raise AuditInputError("at least one --root package is required")
    if len(set(roots)) != len(roots):
        raise AuditInputError("duplicate --root package")
    if len(roots) > MAX_ROOT_PACKAGES:
        raise AuditInputError(f"more than {MAX_ROOT_PACKAGES} root packages")

    packages_by_id: dict[str, dict[str, Any]] = {}
    package_ids_by_name: dict[str, list[str]] = {}
    packages = _require_list(metadata.get("packages"), "Cargo metadata packages")
    if len(packages) > MAX_METADATA_PACKAGES:
        raise AuditInputError("Cargo metadata exceeds the package-count bound")
    for index, raw_package in enumerate(packages):
        package = _require_object(raw_package, f"Cargo package {index}")
        package_id = _require_string(package.get("id"), f"package {index} id")
        name = _require_string(package.get("name"), f"package {package_id} name")
        if package_id in packages_by_id:
            raise AuditInputError(f"duplicate Cargo package id {package_id}")
        packages_by_id[package_id] = package
        package_ids_by_name.setdefault(name, []).append(package_id)

    resolve = _require_object(metadata.get("resolve"), "Cargo metadata resolve")
    nodes_by_id: dict[str, dict[str, Any]] = {}
    nodes = _require_list(resolve.get("nodes"), "Cargo resolve nodes")
    if len(nodes) > MAX_METADATA_PACKAGES:
        raise AuditInputError("Cargo resolve graph exceeds the node-count bound")
    for index, raw_node in enumerate(nodes):
        node = _require_object(raw_node, f"Cargo resolve node {index}")
        package_id = _require_string(node.get("id"), f"resolve node {index} id")
        if package_id in nodes_by_id:
            raise AuditInputError(f"duplicate Cargo resolve node {package_id}")
        if package_id not in packages_by_id:
            raise AuditInputError(f"resolve node has no package record: {package_id}")
        nodes_by_id[package_id] = node

    root_ids: list[str] = []
    for root in roots:
        matches = package_ids_by_name.get(root, [])
        if len(matches) != 1:
            raise AuditInputError(
                f"root package {root!r} must identify exactly one package; found {len(matches)}"
            )
        root_ids.append(matches[0])

    closure: set[str] = set()
    pending = list(reversed(root_ids))
    while pending:
        package_id = pending.pop()
        if package_id in closure:
            continue
        package = packages_by_id.get(package_id)
        node = nodes_by_id.get(package_id)
        if package is None or node is None:
            raise AuditInputError(
                f"production closure package lacks package or resolve data: {package_id}"
            )
        closure.add(package_id)
        for dependency_id in reversed(_production_dependency_ids(node)):
            if dependency_id not in packages_by_id:
                raise AuditInputError(
                    f"dependency has no Cargo package record: {dependency_id}"
                )
            pending.append(dependency_id)

    violations: set[str] = set()
    forbidden_names = policy["forbidden_package_substrings"]
    for package_id in sorted(closure):
        package = packages_by_id[package_id]
        name = _require_string(package.get("name"), f"package {package_id} name")
        normalized_name = name.lower().replace("_", "-")
        for fragment in forbidden_names:
            if fragment in normalized_name:
                violations.add(
                    f"prohibited package in production closure: {name} ({fragment})"
                )
        links = package.get("links")
        if links is not None:
            links = _require_string(links, f"package {name} links")
            violations.add(f"Cargo links package in production closure: {name} ({links})")
        targets = _require_list(package.get("targets"), f"package {name} targets")
        for target_index, raw_target in enumerate(targets):
            target = _require_object(raw_target, f"target {target_index} for {name}")
            kinds = _require_list(target.get("kind"), f"target kinds for {name}")
            if "custom-build" in kinds:
                violations.add(f"Cargo build script in production closure: {name}")

    return sorted(violations), {
        "packages": len(closure),
        "roots": len(root_ids),
    }


def _checked_slice(data: bytes, offset: int, size: int, context: str) -> bytes:
    if offset < 0 or size < 0 or offset > len(data) or size > len(data) - offset:
        raise AuditInputError(f"{context} is outside the ELF file")
    return data[offset : offset + size]


def _unpack_from(fmt: str, data: bytes, offset: int, context: str) -> tuple[Any, ...]:
    size = struct.calcsize(fmt)
    raw = _checked_slice(data, offset, size, context)
    try:
        return struct.unpack(fmt, raw)
    except struct.error as error:
        raise AuditInputError(f"cannot decode {context}: {error}") from error


def _cstring(table: bytes, offset: int, context: str) -> str:
    if offset < 0 or offset >= len(table):
        raise AuditInputError(f"{context} string offset is outside its string table")
    end = table.find(b"\0", offset)
    if end < 0:
        raise AuditInputError(f"{context} string is not NUL terminated")
    try:
        return table[offset:end].decode("utf-8")
    except UnicodeDecodeError as error:
        raise AuditInputError(f"{context} string is not UTF-8") from error


def _parse_elf(data: bytes) -> tuple[tuple[str, ...], tuple[str, ...]]:
    if len(data) < 16 or data[:4] != b"\x7fELF":
        raise AuditInputError("input is not an ELF file")
    elf_class = data[4]
    encoding = data[5]
    if elf_class != 2:
        raise AuditInputError("profile requires an ELF64 host binary")
    if encoding != 1:
        raise AuditInputError("profile requires a little-endian host binary")
    if data[6] != 1:
        raise AuditInputError(f"unsupported ELF identification version {data[6]}")
    endian = "<"
    header_format = endian + "HHIQQQIHHHHHH"
    program_format = endian + "IIQQQQQQ"
    section_format = endian + "IIQQQQIIQQ"
    dynamic_format = endian + "qQ"
    symbol_size = 24
    header_size = 16 + struct.calcsize(header_format)
    if len(data) < header_size:
        raise AuditInputError("ELF header is truncated")
    header = _unpack_from(header_format, data, 16, "ELF header")
    (
        _file_type,
        _machine,
        version,
        _entry,
        program_offset,
        section_offset,
        _flags,
        declared_header_size,
        program_entry_size,
        program_count,
        section_entry_size,
        section_count,
        _section_names,
    ) = header
    if version != 1 or declared_header_size != header_size:
        raise AuditInputError("ELF header version or size is unsupported")
    if _machine != 62:
        raise AuditInputError("profile requires an x86-64 host binary")
    if program_count == 0xFFFF or section_count == 0:
        if program_count == 0xFFFF or section_offset != 0:
            raise AuditInputError("extended ELF header numbering is unsupported")
    expected_program_size = struct.calcsize(program_format)
    if program_count and program_entry_size != expected_program_size:
        raise AuditInputError("ELF program-header entry size is unsupported")
    if program_count > MAX_ELF_PROGRAM_HEADERS:
        raise AuditInputError("ELF exceeds the program-header count bound")
    if section_count > MAX_ELF_SECTION_HEADERS:
        raise AuditInputError("ELF exceeds the section-header count bound")

    programs: list[tuple[int, int, int, int, int]] = []
    for index in range(program_count):
        raw = _unpack_from(
            program_format,
            data,
            program_offset + index * program_entry_size,
            f"ELF program header {index}",
        )
        (
            p_type,
            _p_flags,
            p_offset,
            p_vaddr,
            _p_paddr,
            p_filesz,
            p_memsz,
            _align,
        ) = raw
        _checked_slice(data, p_offset, p_filesz, f"ELF segment {index}")
        if p_filesz > p_memsz:
            raise AuditInputError(f"ELF segment {index} file size exceeds memory size")
        programs.append((p_type, p_offset, p_vaddr, p_filesz, p_memsz))

    dynamic_segments = [entry for entry in programs if entry[0] == PT_DYNAMIC]
    if not dynamic_segments:
        return (), ()
    if len(dynamic_segments) != 1:
        raise AuditInputError("ELF must contain at most one PT_DYNAMIC segment")
    if not section_offset or not section_count:
        raise AuditInputError("dynamic ELF lacks the section table required for symbol audit")

    dynamic = dynamic_segments[0]
    dynamic_entry_size = struct.calcsize(dynamic_format)
    if dynamic[3] % dynamic_entry_size != 0:
        raise AuditInputError("PT_DYNAMIC size is not an integral number of entries")
    dynamic_count = dynamic[3] // dynamic_entry_size
    if dynamic_count > MAX_DYNAMIC_ENTRIES:
        raise AuditInputError("PT_DYNAMIC exceeds the entry-count bound")
    dynamic_values: dict[int, list[int]] = {}
    terminated = False
    for index in range(dynamic_count):
        tag, value = _unpack_from(
            dynamic_format,
            data,
            dynamic[1] + index * dynamic_entry_size,
            f"dynamic entry {index}",
        )
        if tag == DT_NULL:
            terminated = True
            break
        dynamic_values.setdefault(tag, []).append(value)
    if not terminated:
        raise AuditInputError("PT_DYNAMIC has no terminating DT_NULL")
    for tag in (DT_STRTAB, DT_STRSZ):
        if len(dynamic_values.get(tag, [])) != 1:
            raise AuditInputError(f"dynamic ELF must contain exactly one tag {tag}")
    string_address = dynamic_values[DT_STRTAB][0]
    string_size = dynamic_values[DT_STRSZ][0]
    if string_size > MAX_DYNAMIC_STRING_BYTES:
        raise AuditInputError("dynamic string table exceeds the size bound")
    string_offset: int | None = None
    for p_type, p_offset, p_vaddr, p_filesz, _p_memsz in programs:
        if p_type != PT_LOAD or string_address < p_vaddr:
            continue
        relative = string_address - p_vaddr
        if relative <= p_filesz and string_size <= p_filesz - relative:
            candidate = p_offset + relative
            if string_offset is not None and candidate != string_offset:
                raise AuditInputError("dynamic string table maps through multiple segments")
            string_offset = candidate
    if string_offset is None:
        raise AuditInputError("dynamic string table is not file-backed by PT_LOAD")
    dynamic_strings = _checked_slice(
        data, string_offset, string_size, "dynamic string table"
    )
    needed = tuple(
        _cstring(dynamic_strings, offset, "DT_NEEDED")
        for offset in dynamic_values.get(DT_NEEDED, [])
    )

    expected_section_size = struct.calcsize(section_format)
    if section_entry_size != expected_section_size:
        raise AuditInputError("ELF section-header entry size is unsupported")
    sections: list[tuple[int, int, int, int]] = []
    for index in range(section_count):
        raw = _unpack_from(
            section_format,
            data,
            section_offset + index * section_entry_size,
            f"ELF section header {index}",
        )
        (
            _name,
            sh_type,
            _flags,
            _addr,
            sh_offset,
            sh_size,
            sh_link,
            _info,
            _align,
            sh_entsize,
        ) = raw
        if sh_type != SHT_NOBITS:
            _checked_slice(data, sh_offset, sh_size, f"ELF section {index}")
        sections.append(
            (
                sh_type,
                sh_offset,
                sh_size,
                sh_link if sh_type == SHT_DYNSYM else sh_entsize,
            )
        )

    dynamic_symbols: list[str] = []
    dynsym_count = 0
    for index, section in enumerate(sections):
        sh_type, sh_offset, sh_size, sh_link = section
        if sh_type != SHT_DYNSYM:
            continue
        dynsym_count += 1
        raw_header = _unpack_from(
            section_format,
            data,
            section_offset + index * section_entry_size,
            f"ELF section header {index}",
        )
        sh_entsize = raw_header[-1]
        if sh_entsize < symbol_size or sh_size % sh_entsize != 0:
            raise AuditInputError(f"dynamic symbol section {index} has invalid entries")
        symbol_count = sh_size // sh_entsize
        if symbol_count > MAX_DYNAMIC_SYMBOLS:
            raise AuditInputError(
                f"dynamic symbol section {index} exceeds the entry-count bound"
            )
        if sh_link >= len(sections):
            raise AuditInputError(f"dynamic symbol section {index} has invalid string-table link")
        linked = sections[sh_link]
        if linked[0] != SHT_STRTAB:
            raise AuditInputError(f"dynamic symbol section {index} does not link a string table")
        symbol_strings = _checked_slice(data, linked[1], linked[2], "symbol string table")
        for symbol_index in range(symbol_count):
            entry_offset = sh_offset + symbol_index * sh_entsize
            name_offset = _unpack_from(
                endian + "I", data, entry_offset, f"dynamic symbol {symbol_index}"
            )[0]
            if name_offset:
                dynamic_symbols.append(
                    _cstring(symbol_strings, name_offset, "dynamic symbol")
                )
    if dynsym_count != 1:
        raise AuditInputError("dynamic ELF must contain exactly one SHT_DYNSYM section")
    return tuple(needed), tuple(dynamic_symbols)


def _read_stable_regular_file(path: Path) -> tuple[bytes, str]:
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise AuditInputError(f"cannot open ELF input {path}: {error}") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise AuditInputError("ELF input is not a regular file")
        if before.st_size > MAX_INPUT_BYTES:
            raise AuditInputError(f"ELF input exceeds {MAX_INPUT_BYTES} bytes")
        chunks: list[bytes] = []
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1024 * 1024))
            if not chunk:
                raise AuditInputError("ELF input was truncated while being read")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise AuditInputError("ELF input grew while being read")
        after = os.fstat(descriptor)
        identity_before = (
            before.st_dev,
            before.st_ino,
            before.st_mode,
            before.st_size,
            before.st_mtime_ns,
            before.st_ctime_ns,
        )
        identity_after = (
            after.st_dev,
            after.st_ino,
            after.st_mode,
            after.st_size,
            after.st_mtime_ns,
            after.st_ctime_ns,
        )
        if identity_before != identity_after:
            raise AuditInputError("ELF input changed while being read")
        data = b"".join(chunks)
        return data, hashlib.sha256(data).hexdigest()
    finally:
        os.close(descriptor)


def audit_elf(path: Path, policy: dict[str, Any]) -> tuple[list[str], dict[str, Any]]:
    data, digest = _read_stable_regular_file(path)
    needed, symbols = _parse_elf(data)
    violations: set[str] = set()
    allowed = set(policy["allowed_dynamic_dependencies"])
    forbidden_needed = policy["forbidden_dynamic_dependency_substrings"]
    for dependency in needed:
        lowercase = dependency.lower()
        matched = [fragment for fragment in forbidden_needed if fragment in lowercase]
        if matched:
            violations.add(
                f"prohibited dynamic dependency: {dependency} ({matched[0]})"
            )
        elif dependency not in allowed:
            violations.add(f"unapproved dynamic dependency: {dependency}")
    prefixes = policy["forbidden_dynamic_symbol_prefixes"]
    for symbol in symbols:
        normalized = symbol.lstrip("_").lower()
        for prefix in prefixes:
            if normalized.startswith(prefix):
                violations.add(f"prohibited dynamic symbol: {symbol} ({prefix})")
                break
    for literal in policy["forbidden_binary_literals"]:
        if literal.encode("ascii") in data.lower():
            violations.add(f"prohibited dynamic-loader literal: {literal}")
    return sorted(violations), {
        "bytes": len(data),
        "dynamic_dependencies": len(needed),
        "dynamic_symbols": len(symbols),
        "sha256": digest,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    subparsers = parser.add_subparsers(dest="mode", required=True)
    metadata_parser = subparsers.add_parser("metadata", help="audit Cargo metadata JSON")
    metadata_parser.add_argument("--input", type=Path, required=True)
    metadata_parser.add_argument("--root", action="append", default=[])
    elf_parser = subparsers.add_parser("elf", help="audit one host ELF file")
    elf_parser.add_argument("--input", type=Path, required=True)
    arguments = parser.parse_args(argv)

    try:
        policy = load_policy(arguments.policy)
        if arguments.mode == "metadata":
            metadata_bytes = arguments.input.read_bytes()
            if len(metadata_bytes) > MAX_INPUT_BYTES:
                raise AuditInputError(f"Cargo metadata exceeds {MAX_INPUT_BYTES} bytes")
            try:
                metadata = _require_object(
                    json.loads(metadata_bytes), "Cargo metadata"
                )
            except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
                raise AuditInputError(
                    f"cannot parse Cargo metadata {arguments.input}: {error}"
                ) from error
            violations, stats = audit_metadata(
                metadata, tuple(arguments.root), policy
            )
            subject = "metadata roots={} packages={} sha256={}".format(
                stats["roots"],
                stats["packages"],
                hashlib.sha256(metadata_bytes).hexdigest(),
            )
        else:
            violations, stats = audit_elf(arguments.input, policy)
            subject = "ELF bytes={} needed={} dynsym={} sha256={}".format(
                stats["bytes"],
                stats["dynamic_dependencies"],
                stats["dynamic_symbols"],
                stats["sha256"],
            )
    except (AuditInputError, OSError) as error:
        print(f"pure-Rust runtime audit input error: {error}", file=sys.stderr)
        return 2

    if violations:
        for violation in violations:
            print(violation, file=sys.stderr)
        print(
            f"pure-Rust runtime audit: FAILED ({len(violations)} violation(s); {subject})",
            file=sys.stderr,
        )
        return 1
    print(
        "pure-Rust runtime audit: OK ({}; profile={})".format(
            subject, policy["profile"]
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
