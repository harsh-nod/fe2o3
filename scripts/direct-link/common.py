#!/usr/bin/env python3
"""Shared bounded codecs for direct-link release evidence."""

from __future__ import annotations

import hashlib
import os
import re
import stat
from pathlib import Path

MAX_RECORD_BYTES = 64 * 1024
MAX_HASHED_FILE_BYTES = 512 * 1024 * 1024
SUPPORTED_PROCESSORS = frozenset(("gfx1151", "gfx942", "gfx950"))

_DIGEST_RE = re.compile(r"[0-9a-f]{64}\Z")
_COMMIT_RE = re.compile(r"[0-9a-f]{40}\Z")
_REASON_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
_FEATURE_RE = re.compile(r"(?:sramecc|xnack)[+-]\Z")


class EvidenceError(ValueError):
    """An evidence record or its authenticated input is invalid."""


def require_digest(value: str, name: str) -> str:
    if _DIGEST_RE.fullmatch(value) is None:
        raise EvidenceError(f"{name} must be exactly 64 lowercase hexadecimal digits")
    return value


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


def metadata_snapshot(metadata: os.stat_result) -> tuple[int, int, int, int, int]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def require_bounded_text(value: str, name: str, maximum: int) -> str:
    if not 1 <= len(value) <= maximum:
        raise EvidenceError(f"{name} is empty or exceeds {maximum} bytes")
    try:
        encoded = value.encode("ascii")
    except UnicodeEncodeError as error:
        raise EvidenceError(f"{name} must contain ASCII only") from error
    if len(encoded) != len(value):
        raise EvidenceError(f"{name} must contain ASCII only")
    if value != value.strip() or "  " in value:
        raise EvidenceError(f"{name} has noncanonical whitespace")
    if any(ord(character) < 0x20 or ord(character) > 0x7E for character in value):
        raise EvidenceError(f"{name} contains a control character")
    if "\t" in value:
        raise EvidenceError(f"{name} contains a tab")
    return value


def read_regular_file(path: Path, maximum: int = MAX_RECORD_BYTES) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(
            f"cannot open regular file {path}: {error.strerror}"
        ) from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise EvidenceError(f"input is not a regular file: {path}")
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


def sha256_file(path: Path) -> str:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(
            f"cannot open regular file {path}: {error.strerror}"
        ) from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise EvidenceError(f"hashed input is not a regular file: {path}")
        if before.st_size > MAX_HASHED_FILE_BYTES:
            raise EvidenceError(
                f"hashed input exceeds the {MAX_HASHED_FILE_BYTES}-byte bound: {path}"
            )
        digest = hashlib.sha256()
        total = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_HASHED_FILE_BYTES:
                raise EvidenceError(
                    f"hashed input exceeds the {MAX_HASHED_FILE_BYTES}-byte bound: {path}"
                )
            digest.update(chunk)
        after = os.fstat(descriptor)
        if total != before.st_size or metadata_snapshot(before) != metadata_snapshot(
            after
        ):
            raise EvidenceError(f"hashed input changed while being measured: {path}")
        return digest.hexdigest()
    finally:
        os.close(descriptor)


def typed_identity(domain: str, payload: bytes) -> str:
    return f"{domain}-sha256-{hashlib.sha256(payload).hexdigest()}"
