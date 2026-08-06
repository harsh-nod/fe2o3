#!/usr/bin/env python3
"""Shared bounded codecs and typed identities for direct-link evidence."""

from __future__ import annotations

import hashlib
import os
import re
import stat
from pathlib import Path

MAX_RECORD_BYTES = 256 * 1024
MAX_HASHED_FILE_BYTES = 512 * 1024 * 1024
SUPPORTED_PROCESSORS = frozenset(("gfx1151", "gfx942", "gfx950"))

_COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
_DOMAIN_RE = re.compile(r"[a-z0-9][a-z0-9-]{0,63}-v[1-9][0-9]*\Z")
_REASON_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
_FEATURE_RE = re.compile(r"(?:sramecc|xnack)[+-]\Z")
_IDENTITY_MAGIC = b"FE2O3-TYPED-IDENTITY\x00\x01"


class EvidenceError(ValueError):
    """An evidence record or its authenticated input is invalid."""


def require_commit(value: str) -> str:
    if _COMMIT_RE.fullmatch(value) is None:
        raise EvidenceError(
            "git_commit must be exactly 40 lowercase hexadecimal digits"
        )
    return value


def require_reason(value: str) -> str:
    if _REASON_RE.fullmatch(value) is None:
        raise EvidenceError("reason must be a bounded lowercase reason code")
    return value


def require_target(value: str) -> str:
    parts = value.split(":")
    if not parts or parts[0] not in SUPPORTED_PROCESSORS:
        raise EvidenceError(
            "target processor must be one of gfx1151, gfx942, or gfx950"
        )
    features = parts[1:]
    if parts[0] == "gfx1151" and features:
        raise EvidenceError("gfx1151 does not accept sramecc or xnack modifiers")
    if any(_FEATURE_RE.fullmatch(feature) is None for feature in features):
        raise EvidenceError("target contains an unsupported or malformed feature")
    if len(set(features)) != len(features):
        raise EvidenceError("target contains a duplicate feature")
    feature_names = [feature[:-1] for feature in features]
    if len(set(feature_names)) != len(feature_names):
        raise EvidenceError("target contains conflicting feature states")
    if feature_names != sorted(feature_names, key=("sramecc", "xnack").index):
        raise EvidenceError("target features are not in canonical order")
    return value


def require_bounded_text(value: str, name: str, maximum: int) -> str:
    try:
        encoded = value.encode("ascii")
    except UnicodeEncodeError as error:
        raise EvidenceError(f"{name} must contain ASCII only") from error
    if not 1 <= len(encoded) <= maximum:
        raise EvidenceError(f"{name} is empty or exceeds {maximum} bytes")
    if any(byte < 0x20 or byte > 0x7E for byte in encoded):
        raise EvidenceError(f"{name} contains a control character")
    return value


def require_typed_identity(value: str, domain: str, name: str) -> str:
    require_domain(domain)
    expected_prefix = f"{domain}-sha256-"
    if not value.startswith(expected_prefix):
        raise EvidenceError(f"{name} must use the {domain} identity domain")
    digest = value[len(expected_prefix) :]
    if re.fullmatch(r"[0-9a-f]{64}", digest) is None:
        raise EvidenceError(f"{name} has a malformed SHA-256 digest")
    return value


def require_domain(domain: str) -> str:
    if _DOMAIN_RE.fullmatch(domain) is None:
        raise EvidenceError("identity domain is malformed")
    return domain


def identity_prefix(domain: str, payload_length: int) -> bytes:
    encoded_domain = require_domain(domain).encode("ascii")
    if payload_length < 0:
        raise EvidenceError("identity payload length must not be negative")
    return (
        _IDENTITY_MAGIC
        + len(encoded_domain).to_bytes(4, "big")
        + encoded_domain
        + payload_length.to_bytes(8, "big")
    )


def typed_identity(domain: str, payload: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(identity_prefix(domain, len(payload)))
    digest.update(payload)
    return f"{domain}-sha256-{digest.hexdigest()}"


def metadata_snapshot(metadata: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _open_regular(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(
            f"cannot open regular file {path}: {error.strerror}"
        ) from error
    metadata = os.fstat(descriptor)
    if not stat.S_ISREG(metadata.st_mode):
        os.close(descriptor)
        raise EvidenceError(f"input is not a regular file: {path}")
    return descriptor


def open_directory(path: Path) -> int:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(f"cannot pin directory {path}: {error.strerror}") from error
    if not stat.S_ISDIR(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise EvidenceError(f"input is not a directory: {path}")
    return descriptor


def relative_parts(path: Path) -> tuple[str, ...]:
    parts = path.parts
    if (
        path.is_absolute()
        or not parts
        or any(part in ("", ".", "..") for part in parts)
    ):
        raise EvidenceError("descriptor-relative path is empty, absolute, or traverses")
    return parts


def open_parent_beneath(root_descriptor: int, path: Path) -> tuple[int, str]:
    parts = relative_parts(path)
    current = os.dup(root_descriptor)
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        for component in parts[:-1]:
            try:
                next_descriptor = os.open(component, directory_flags, dir_fd=current)
            except OSError as error:
                raise EvidenceError(
                    f"path component is not a no-follow directory: {component}"
                ) from error
            os.close(current)
            current = next_descriptor
        return current, parts[-1]
    except Exception:
        os.close(current)
        raise


def open_regular_beneath(root_descriptor: int, path: Path) -> int:
    parent, name = open_parent_beneath(root_descriptor, path)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        try:
            descriptor = os.open(name, flags, dir_fd=parent)
        except OSError as error:
            raise EvidenceError(
                f"cannot open descriptor-relative regular file {path}: {error.strerror}"
            ) from error
    finally:
        os.close(parent)
    if not stat.S_ISREG(os.fstat(descriptor).st_mode):
        os.close(descriptor)
        raise EvidenceError(f"descriptor-relative input is not a regular file: {path}")
    return descriptor


def read_regular_file(path: Path, maximum: int = MAX_RECORD_BYTES) -> bytes:
    descriptor = _open_regular(path)
    try:
        before = os.fstat(descriptor)
        if before.st_size > maximum:
            raise EvidenceError(f"input exceeds the {maximum}-byte bound: {path}")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        if len(data) > maximum:
            raise EvidenceError(f"input exceeds the {maximum}-byte bound: {path}")
        after = os.fstat(descriptor)
        if len(data) != before.st_size or metadata_snapshot(
            before
        ) != metadata_snapshot(after):
            raise EvidenceError(f"input changed while being read: {path}")
        return data
    finally:
        os.close(descriptor)


def typed_file_identity(domain: str, path: Path) -> str:
    descriptor = _open_regular(path)
    try:
        return typed_descriptor_identity(domain, descriptor, str(path))
    finally:
        os.close(descriptor)


def typed_file_identity_beneath(domain: str, root_descriptor: int, path: Path) -> str:
    descriptor = open_regular_beneath(root_descriptor, path)
    try:
        return typed_descriptor_identity(domain, descriptor, str(path))
    finally:
        os.close(descriptor)


def typed_descriptor_identity(domain: str, descriptor: int, name: str) -> str:
    before = os.fstat(descriptor)
    if before.st_size > MAX_HASHED_FILE_BYTES:
        raise EvidenceError(
            f"hashed input exceeds the {MAX_HASHED_FILE_BYTES}-byte bound: {name}"
        )
    os.lseek(descriptor, 0, os.SEEK_SET)
    digest = hashlib.sha256()
    digest.update(identity_prefix(domain, before.st_size))
    total = 0
    while True:
        chunk = os.read(descriptor, 1024 * 1024)
        if not chunk:
            break
        total += len(chunk)
        if total > MAX_HASHED_FILE_BYTES:
            raise EvidenceError(
                f"hashed input exceeds the {MAX_HASHED_FILE_BYTES}-byte bound: {name}"
            )
        digest.update(chunk)
    after = os.fstat(descriptor)
    if total != before.st_size or metadata_snapshot(before) != metadata_snapshot(after):
        raise EvidenceError(f"hashed input changed while being measured: {name}")
    return f"{domain}-sha256-{digest.hexdigest()}"


def decode_canonical_text(data: bytes, name: str) -> list[str]:
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{name} must contain ASCII only") from error
    if not text:
        raise EvidenceError(f"{name} is empty")
    if not text.endswith("\n"):
        raise EvidenceError(f"{name} is truncated or lacks its final newline")
    if "\r" in text or "\0" in text:
        raise EvidenceError(f"{name} contains a forbidden byte")
    lines = text[:-1].split("\n")
    if any(not line for line in lines):
        raise EvidenceError(f"{name} contains a blank line")
    return lines
