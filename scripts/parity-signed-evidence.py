#!/usr/bin/env python3
"""Fail-closed signed parity evidence and MI300X queue tooling."""

from __future__ import annotations

import argparse
import base64
import binascii
import ctypes
import errno
import fcntl
import hashlib
import os
from pathlib import Path
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Iterable


MAX_BYTES = 4 * 1024 * 1024
MAX_ITEMS = 256
MAX_ARCHIVE_FILES = 4096
CLASSES = ("unit", "ui", "ir", "compile", "verus", "hardware", "debug")
CLASS_RANK = {value: index for index, value in enumerate(CLASSES)}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9._-]{0,63}$")
RESULT_ID_RE = re.compile(r"^[0-9a-f]{64}$")
TARGET_RE = re.compile(r"^(generic|gfx[0-9a-f]+(?::[A-Za-z0-9_+-]+)*)$")
LANE_RE = re.compile(r"^(-|[a-z0-9][a-z0-9._-]{0,63})$")
PATH_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/+:-]{0,511}$")
HEX_RE = re.compile(r"^(?:[0-9a-f]{2})+$")
DEFAULT_LOCK = Path("/run/lock/fe2o3/mi300x-gfx942-evidence.lock")
ARCHIVE_INDEX_RELATIVE = "archive-index-v1.tsv"
FS_IOC_GETFLAGS = 0x80086601
FS_IMMUTABLE_FL = 0x00000010
AT_FDCWD = -100
RENAME_NOREPLACE = 1
SYS_OPENAT2 = 437
RESOLVE_NO_XDEV = 0x01
RESOLVE_NO_SYMLINKS = 0x04
RESOLVE_BENEATH = 0x08
TRUST_POLICY_RELATIVE = Path("docs/parity-evidence/trust-policy-v2.tsv")
TRUST_KEYS_RELATIVE = Path("docs/parity-evidence/trusted-keys")
REQUIRED_METADATA = [
    ("exact", "docs/cuda-oxide-parity-status.tsv"),
    ("prefix", "docs/parity-evidence/archive/"),
]


class EvidenceError(Exception):
    pass


def fail(message: str) -> None:
    raise EvidenceError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def valid_row(value: str) -> bool:
    if re.fullmatch(r"[0-9]{2}", value):
        return 1 <= int(value) <= 94
    if re.fullmatch(r"S[0-9]{2}", value):
        return 1 <= int(value[1:]) <= 15
    return False


def row_rank(value: str) -> int:
    return 94 + int(value[1:]) if value.startswith("S") else int(value)


def valid_transition(from_status: str, to_status: str) -> bool:
    return (from_status == "Missing" and to_status in ("Partial", "Complete")) or (
        from_status == "Partial" and to_status == "Complete"
    )


def valid_relative(value: str) -> bool:
    if not PATH_RE.fullmatch(value) or value.startswith("/"):
        return False
    return all(part not in ("", ".", "..") for part in value.split("/"))


def valid_metadata_path(kind: str, value: str) -> bool:
    if kind == "exact":
        return valid_relative(value)
    return kind == "prefix" and value.endswith("/") and valid_relative(value[:-1])


def read_raw(path: Path) -> tuple[bytes, list[bytes], list[list[str]]]:
    try:
        raw = path.read_bytes()
    except OSError as error:
        fail(f"cannot read {path}: {error}")
    return parse_raw_bytes(raw, str(path))


def parse_raw_bytes(raw: bytes, label: str) -> tuple[bytes, list[bytes], list[list[str]]]:
    if not raw or len(raw) > MAX_BYTES:
        fail(f"invalid evidence file size: {label}")
    if b"\r" in raw or not raw.endswith(b"\n"):
        fail(f"non-canonical line endings: {label}")
    raw_lines = raw.splitlines(keepends=True)
    rows: list[list[str]] = []
    for number, raw_line in enumerate(raw_lines, 1):
        if raw_line == b"\n":
            fail(f"blank line {number}: {label}")
        try:
            line = raw_line[:-1].decode("ascii")
        except UnicodeDecodeError:
            fail(f"non-ASCII line {number}: {label}")
        rows.append(line.split("\t"))
    return raw, raw_lines, rows


def resolve_real_directory(path: Path, label: str) -> Path:
    try:
        info = path.lstat()
    except OSError:
        fail(f"{label} is missing")
    if not stat.S_ISDIR(info.st_mode) or path.is_symlink():
        fail(f"{label} must be a real directory")
    return path.resolve(strict=True)


class OpenHow(ctypes.Structure):
    _fields_ = [
        ("flags", ctypes.c_uint64),
        ("mode", ctypes.c_uint64),
        ("resolve", ctypes.c_uint64),
    ]


@dataclass(frozen=True)
class ArchiveFileIdentity:
    device: int
    inode: int
    mode: int
    links: int
    size: int
    modified_ns: int
    changed_ns: int
    digest: str


class ArchiveSnapshot:
    """One FD-anchored identity snapshot for an evidence archive."""

    def __init__(self, root: Path, *, require_immutable: bool) -> None:
        requested = root.absolute()
        flags = os.O_RDONLY | os.O_CLOEXEC | os.O_DIRECTORY | os.O_NOFOLLOW
        try:
            self.root_fd = os.open(requested, flags)
        except OSError as error:
            fail(f"evidence archive root must be a real directory: {error}")
        self.root = requested.resolve(strict=True)
        self.require_immutable = require_immutable
        self.files: dict[str, int] = {}
        self.identities: dict[str, ArchiveFileIdentity] = {}
        self.directories: set[str] = set()
        try:
            root_info = os.fstat(self.root_fd)
            if not stat.S_ISDIR(root_info.st_mode):
                fail("evidence archive root must be a real directory")
            if require_immutable and not immutable_flag_is_set_fd(self.root_fd):
                fail("production evidence archive root is not Linux immutable")
            self._scan_directory(self.root_fd, "", root_info.st_dev)
            if not self.files:
                fail("evidence archive is empty")
        except BaseException:
            self.close()
            raise

    def __enter__(self) -> ArchiveSnapshot:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        for descriptor in self.files.values():
            try:
                os.close(descriptor)
            except OSError:
                pass
        self.files.clear()
        if getattr(self, "root_fd", -1) >= 0:
            try:
                os.close(self.root_fd)
            except OSError:
                pass
            self.root_fd = -1

    @staticmethod
    def _openat2(parent_fd: int, name: str, flags: int) -> int:
        how = OpenHow(
            flags | os.O_CLOEXEC | os.O_NOFOLLOW,
            0,
            RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV,
        )
        libc = ctypes.CDLL(None, use_errno=True)
        descriptor = libc.syscall(
            SYS_OPENAT2,
            parent_fd,
            ctypes.c_char_p(os.fsencode(name)),
            ctypes.byref(how),
            ctypes.sizeof(how),
        )
        if descriptor < 0:
            error = ctypes.get_errno()
            if error == errno.ELOOP:
                fail(f"archive contains a symlink: {name}")
            fail(f"cannot securely open archive entry {name}: {os.strerror(error)}")
        return descriptor

    def _scan_directory(self, directory_fd: int, prefix: str, device: int) -> None:
        try:
            names = sorted(os.listdir(directory_fd))
        except OSError as error:
            fail(f"cannot enumerate archive directory {prefix or '.'}: {error}")
        if len(self.files) + len(self.directories) + len(names) > MAX_ARCHIVE_FILES * 4:
            fail("evidence archive exceeds the entry-count bound")
        for name in names:
            relative = f"{prefix}/{name}" if prefix else name
            if not valid_relative(relative):
                fail(f"invalid archive path: {relative}")
            descriptor = self._openat2(
                directory_fd, name, os.O_RDONLY | os.O_NONBLOCK
            )
            keep = False
            try:
                info = os.fstat(descriptor)
                if info.st_dev != device:
                    fail(f"archive entry crosses a filesystem boundary: {relative}")
                if stat.S_ISDIR(info.st_mode):
                    if self.require_immutable and not immutable_flag_is_set_fd(descriptor):
                        fail(
                            "production archive directory is not Linux immutable: "
                            f"{relative}"
                        )
                    self.directories.add(relative)
                    self._scan_directory(descriptor, relative, device)
                    continue
                if not stat.S_ISREG(info.st_mode):
                    fail(f"archive contains a non-regular entry: {relative}")
                if info.st_nlink != 1:
                    fail(f"archive file has an unsafe link count: {relative}")
                if len(self.files) >= MAX_ARCHIVE_FILES:
                    fail("evidence archive exceeds the file-count bound")
                if self.require_immutable and not immutable_flag_is_set_fd(descriptor):
                    fail(f"production archive file is not Linux immutable: {relative}")
                digest = sha256_fd(descriptor)
                after = os.fstat(descriptor)
                identity = archive_file_identity(after, digest)
                if archive_file_identity(info, digest) != identity:
                    fail(f"archive entry changed during scan: {relative}")
                self.files[relative] = descriptor
                self.identities[relative] = identity
                keep = True
            finally:
                if not keep:
                    os.close(descriptor)

    @property
    def records(self) -> dict[str, tuple[int, str]]:
        return {
            relative: (identity.size, identity.digest)
            for relative, identity in self.identities.items()
        }

    def _descriptor(self, relative: str) -> int:
        if not valid_relative(relative):
            fail(f"invalid archive path: {relative}")
        descriptor = self.files.get(relative)
        if descriptor is None:
            fail(f"archive entry is missing: {relative}")
        return descriptor

    def validate(self, relative: str) -> ArchiveFileIdentity:
        descriptor = self._descriptor(relative)
        expected = self.identities[relative]
        before = os.fstat(descriptor)
        digest = sha256_fd(descriptor)
        after = os.fstat(descriptor)
        actual = archive_file_identity(after, digest)
        if archive_file_identity(before, digest) != actual or actual != expected:
            fail(f"archive entry changed after authentication: {relative}")
        if self.require_immutable and not immutable_flag_is_set_fd(descriptor):
            fail(f"production archive file lost Linux immutable: {relative}")
        return actual

    def read(self, relative: str) -> bytes:
        descriptor = self._descriptor(relative)
        self.validate(relative)
        os.lseek(descriptor, 0, os.SEEK_SET)
        chunks: list[bytes] = []
        remaining = MAX_BYTES + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        value = b"".join(chunks)
        self.validate(relative)
        return value

    def copy_to(self, relative: str, destination: Path) -> None:
        descriptor = self._descriptor(relative)
        expected = self.validate(relative)
        destination.parent.mkdir(parents=True, exist_ok=True)
        output_descriptor = os.open(
            destination,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC,
            0o400,
        )
        try:
            os.lseek(descriptor, 0, os.SEEK_SET)
            digest = hashlib.sha256()
            written = 0
            while True:
                chunk = os.read(descriptor, 1024 * 1024)
                if not chunk:
                    break
                digest.update(chunk)
                written += len(chunk)
                view = memoryview(chunk)
                while view:
                    count = os.write(output_descriptor, view)
                    view = view[count:]
            os.fsync(output_descriptor)
            copied = os.fstat(output_descriptor)
            if (
                not stat.S_ISREG(copied.st_mode)
                or copied.st_nlink != 1
                or copied.st_size != expected.size
                or written != expected.size
                or digest.hexdigest() != expected.digest
            ):
                fail(f"copied archive file identity mismatch: {relative}")
        finally:
            os.close(output_descriptor)
        self.validate(relative)


def archive_file_identity(info: os.stat_result, digest: str) -> ArchiveFileIdentity:
    return ArchiveFileIdentity(
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
        digest,
    )


def sha256_fd(descriptor: int) -> str:
    digest = hashlib.sha256()
    os.lseek(descriptor, 0, os.SEEK_SET)
    for chunk in iter(lambda: os.read(descriptor, 1024 * 1024), b""):
        digest.update(chunk)
    return digest.hexdigest()


def immutable_flag_is_set_fd(descriptor: int) -> bool:
    encoded = bytearray(4)
    try:
        fcntl.ioctl(descriptor, FS_IOC_GETFLAGS, encoded, True)
    except OSError as error:
        fail(f"cannot establish Linux immutable flag for archive FD: {error}")
    return bool(int.from_bytes(encoded, sys.byteorder) & FS_IMMUTABLE_FL)


ArchiveRoot = Path | ArchiveSnapshot


def archive_root_path(root: ArchiveRoot) -> Path:
    return root.root if isinstance(root, ArchiveSnapshot) else root.resolve(strict=True)


def archive_read_raw(
    root: ArchiveRoot, relative: str
) -> tuple[bytes, list[bytes], list[list[str]]]:
    if isinstance(root, ArchiveSnapshot):
        return parse_raw_bytes(root.read(relative), f"{root.root}/{relative}")
    return read_raw(archive_path(root, relative))


def archive_digest(root: ArchiveRoot, relative: str) -> str:
    if isinstance(root, ArchiveSnapshot):
        return root.validate(relative).digest
    return sha256_file(archive_path(root, relative))


def archive_path(root: Path, relative: str, *, must_exist: bool = True) -> Path:
    if not valid_relative(relative):
        fail(f"invalid archive path: {relative}")
    root = root.resolve(strict=True)
    candidate = root.joinpath(relative)
    resolved = candidate.resolve(strict=False)
    if root not in resolved.parents:
        fail(f"archive path escapes root: {relative}")
    if must_exist:
        try:
            info = candidate.lstat()
        except OSError:
            fail(f"archive entry is missing: {relative}")
        if not stat.S_ISREG(info.st_mode) or candidate.is_symlink():
            fail(f"archive entry is not a regular file: {relative}")
        if root not in candidate.resolve(strict=True).parents:
            fail(f"archive entry resolves outside root: {relative}")
    return candidate


def run_git(repo: Path, *arguments: str, check: bool = True) -> str:
    process = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if check and process.returncode != 0:
        fail(f"git {' '.join(arguments)} failed")
    return process.stdout.strip()


def require_commit(repo: Path, commit: str, label: str) -> None:
    if not COMMIT_RE.fullmatch(commit):
        fail(f"malformed {label} commit")
    process = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "-e", f"{commit}^{{commit}}"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if process.returncode != 0:
        fail(f"stale {label} commit: {commit}")


def require_tree(repo: Path, commit: str, tree: str, label: str) -> None:
    require_commit(repo, commit, label)
    if not COMMIT_RE.fullmatch(tree):
        fail(f"malformed {label} source tree")
    actual = run_git(repo, "rev-parse", f"{commit}^{{tree}}")
    if actual != tree:
        fail(f"{label} source tree mismatch")


def verify_repo_source_file(
    repo: Path, relative: str, source: str, expected_digest: str
) -> Path:
    repo = repo.resolve(strict=True)
    current = repo
    parts = relative.split("/")
    for index, part in enumerate(parts):
        current = current.joinpath(part)
        try:
            info = current.lstat()
        except OSError:
            fail(f"queue script path is missing: {relative}")
        if stat.S_ISLNK(info.st_mode):
            fail(f"queue script path contains symlink: {relative}")
        if index + 1 < len(parts):
            if not stat.S_ISDIR(info.st_mode):
                fail(f"queue script parent is not a directory: {relative}")
        elif not stat.S_ISREG(info.st_mode):
            fail(f"queue script is not a regular file: {relative}")
        resolved = current.resolve(strict=True)
        if resolved != repo and repo not in resolved.parents:
            fail(f"queue script escapes attested checkout: {relative}")
    process = subprocess.run(
        ["git", "-C", str(repo), "show", f"{source}:{relative}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if process.returncode != 0 or sha256_bytes(process.stdout) != expected_digest:
        fail(f"queue script does not match attested source tree: {relative}")
    if sha256_file(current) != expected_digest:
        fail(f"queue checkout script differs from attested source: {relative}")
    return current


class Cursor:
    def __init__(self, rows: list[list[str]], label: str) -> None:
        self.rows = rows
        self.label = label
        self.index = 0

    def scalar(self, key: str) -> str:
        if self.index >= len(self.rows):
            fail(f"{self.label}: missing {key}")
        row = self.rows[self.index]
        self.index += 1
        if len(row) != 2 or row[0] != key or not row[1]:
            fail(f"{self.label}: expected canonical {key}")
        return row[1]

    def record(self, key: str, width: int, index: int) -> list[str]:
        if self.index >= len(self.rows):
            fail(f"{self.label}: missing {key} {index:04d}")
        row = self.rows[self.index]
        self.index += 1
        if len(row) != width or row[0] != key or row[1] != f"{index:04d}":
            fail(f"{self.label}: malformed {key} {index:04d}")
        return row

    def done(self) -> None:
        if self.index != len(self.rows):
            fail(f"{self.label}: unexpected trailing fields")


def parse_count(value: str, label: str, *, allow_zero: bool = True) -> int:
    if not re.fullmatch(r"0|[1-9][0-9]*", value):
        fail(f"invalid {label} count")
    count = int(value)
    if count > MAX_ITEMS or (not allow_zero and count == 0):
        fail(f"invalid {label} count")
    return count


@dataclass(frozen=True)
class TrustedKey:
    role: str
    key_id: str
    relative_path: str
    fingerprint: str
    public_key: bytes


@dataclass
class TrustPolicy:
    domain: str
    metadata_paths: list[tuple[str, str]]
    keys: dict[tuple[str, str], TrustedKey]


def canonical_ed25519_public_key(path: Path) -> bytes:
    try:
        info = path.lstat()
    except OSError:
        fail(f"public key is missing: {path}")
    if not stat.S_ISREG(info.st_mode) or path.is_symlink():
        fail(f"public key must be a regular non-symlink file: {path}")
    return canonical_ed25519_public_key_bytes(path.read_bytes(), str(path))


def canonical_ed25519_public_key_bytes(raw: bytes, label: str) -> bytes:
    process = subprocess.run(
        ["openssl", "pkey", "-pubin", "-pubout"],
        input=raw,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if process.returncode != 0:
        fail(f"public key is not public Ed25519 material: {label}")
    ed25519_fingerprint_bytes(process.stdout, label)
    return process.stdout


def require_real_path_components(root: Path, relative: Path, label: str) -> Path:
    current = root.resolve(strict=True)
    for index, part in enumerate(relative.parts):
        current = current.joinpath(part)
        try:
            info = current.lstat()
        except OSError:
            fail(f"{label} is missing: {relative}")
        if stat.S_ISLNK(info.st_mode):
            fail(f"{label} contains a symlink: {relative}")
        if index + 1 < len(relative.parts):
            if not stat.S_ISDIR(info.st_mode):
                fail(f"{label} parent is not a directory: {relative}")
        elif not stat.S_ISREG(info.st_mode):
            fail(f"{label} is not a regular file: {relative}")
    return current

def ed25519_fingerprint(path: Path) -> str:
    return ed25519_fingerprint_bytes(path.read_bytes(), path.name)


def ed25519_fingerprint_bytes(raw: bytes, label: str) -> str:
    process = subprocess.run(
        ["openssl", "pkey", "-pubin", "-outform", "DER"],
        input=raw,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    prefix = bytes.fromhex("302a300506032b6570032100")
    if (
        process.returncode != 0
        or len(process.stdout) != 44
        or not process.stdout.startswith(prefix)
    ):
        fail(f"trusted public key is not canonical Ed25519 material: {label}")
    return hashlib.sha256(process.stdout).hexdigest()



def parse_trust_policy(trusted_root: Path, policy_path: Path) -> TrustPolicy:
    trusted_root = trusted_root.resolve(strict=True)
    try:
        policy_info = policy_path.lstat()
    except OSError:
        fail("trust policy is missing")
    if not stat.S_ISREG(policy_info.st_mode) or policy_path.is_symlink():
        fail("trust policy must be a regular file under the trusted root")
    policy_path = policy_path.resolve(strict=True)
    if trusted_root not in policy_path.parents:
        fail("trust policy must be a regular file under the trusted root")
    _, _, rows = read_raw(policy_path)
    cursor = Cursor(rows, "trust policy")
    if cursor.scalar("parity_trust_policy_schema_version") != "2":
        fail("trust policy schema must be 2")
    domain = cursor.scalar("trust_domain")
    if domain not in ("production", "test"):
        fail("invalid trust domain")
    metadata_count = parse_count(cursor.scalar("metadata_path_count"), "metadata paths")
    metadata: list[tuple[str, str]] = []
    previous = ""
    for index in range(metadata_count):
        row = cursor.record("metadata_path", 4, index)
        kind, value = row[2], row[3]
        if not valid_metadata_path(kind, value):
            fail("invalid trusted metadata path")
        sort_key = f"{kind}\t{value}"
        if sort_key <= previous:
            fail("duplicate or unsorted trusted metadata path")
        previous = sort_key
        metadata.append((kind, value))
    key_count = parse_count(cursor.scalar("key_count"), "trusted keys", allow_zero=False)
    keys: dict[tuple[str, str], TrustedKey] = {}
    fingerprints: set[str] = set()
    previous = ""
    for index in range(key_count):
        row = cursor.record("key", 7, index)
        role, key_id, relative, expected = row[2], row[3], row[4], row[5]
        if row[6] != "ed25519" or not ID_RE.fullmatch(role) or not ID_RE.fullmatch(key_id):
            fail("invalid trusted key identity")
        if not SHA256_RE.fullmatch(expected):
            fail("invalid trusted key digest")
        sort_key = f"{role}\t{key_id}"
        if sort_key <= previous or (role, key_id) in keys:
            fail("duplicate or unsorted trusted key")
        previous = sort_key
        key_path = archive_path(trusted_root, relative)
        key_bytes = key_path.read_bytes()
        if sha256_bytes(key_bytes) != expected:
            fail(f"trusted public key digest mismatch: {key_id}")
        canonical = canonical_ed25519_public_key_bytes(key_bytes, key_id)
        fingerprint = ed25519_fingerprint_bytes(canonical, key_id)
        if fingerprint in fingerprints:
            fail(f"duplicate trusted public-key fingerprint: {fingerprint}")
        fingerprints.add(fingerprint)
        keys[(role, key_id)] = TrustedKey(
            role, key_id, relative, fingerprint, key_bytes
        )
    cursor.done()
    return TrustPolicy(domain, metadata, keys)


def validate_production_trust(trusted_root: Path, policy_path: Path) -> TrustPolicy:
    trusted_root = trusted_root.resolve(strict=True)
    expected_policy = require_real_path_components(
        trusted_root, TRUST_POLICY_RELATIVE, "production trust policy"
    )
    if policy_path.resolve(strict=True) != expected_policy:
        fail("production trust policy is not at its canonical repository path")
    trust = parse_trust_policy(trusted_root, expected_policy)
    if trust.domain != "production":
        fail("production trust policy must use the production domain")
    if trust.metadata_paths != REQUIRED_METADATA:
        fail("production trust policy has a non-canonical metadata allowlist")
    if len(trust.keys) != 2 or {key.role for key in trust.keys.values()} != {
        "attestor",
        "reviewer",
    }:
        fail("production trust policy requires exactly one attestor and one reviewer")
    for key in trust.keys.values():
        relative = TRUST_KEYS_RELATIVE.joinpath(f"{key.key_id}.pem")
        require_real_path_components(
            trusted_root, relative, "production public key"
        )
        if key.relative_path != relative.as_posix():
            fail(f"production public key has a non-canonical path: {key.key_id}")
        if key.public_key != canonical_ed25519_public_key_bytes(
            key.public_key, key.key_id
        ):
            fail(f"production public key is not canonical PEM: {key.key_id}")
    return trust


def bootstrap_production_trust(args: argparse.Namespace) -> None:
    if not ID_RE.fullmatch(args.attestor_key_id) or not ID_RE.fullmatch(
        args.reviewer_key_id
    ):
        fail("invalid production signing key identity")
    if args.attestor_key_id == args.reviewer_key_id:
        fail("attestor and reviewer key IDs must be distinct")
    attestor = canonical_ed25519_public_key(args.attestor_public_key)
    reviewer = canonical_ed25519_public_key(args.reviewer_public_key)
    with tempfile.TemporaryDirectory(prefix="fe2o3-bootstrap-keys-") as temp:
        attestor_path = Path(temp, "attestor.pem")
        reviewer_path = Path(temp, "reviewer.pem")
        attestor_path.write_bytes(attestor)
        reviewer_path.write_bytes(reviewer)
        if ed25519_fingerprint(attestor_path) == ed25519_fingerprint(reviewer_path):
            fail("attestor and reviewer must use distinct Ed25519 public keys")

    destination = args.output_root.absolute()
    if destination.exists() or destination.is_symlink():
        fail("production trust bootstrap output already exists")
    parent = destination.parent.resolve(strict=True)
    staging = Path(tempfile.mkdtemp(prefix=".fe2o3-trust-", dir=parent))
    try:
        keys = staging.joinpath(TRUST_KEYS_RELATIVE)
        keys.mkdir(parents=True, mode=0o700)
        material = [
            ("attestor", args.attestor_key_id, attestor),
            ("reviewer", args.reviewer_key_id, reviewer),
        ]
        records: list[tuple[str, str, str, str]] = []
        for role, key_id, public_key in material:
            relative = TRUST_KEYS_RELATIVE.joinpath(f"{key_id}.pem")
            output = staging.joinpath(relative)
            output.write_bytes(public_key)
            output.chmod(0o444)
            records.append((role, key_id, relative.as_posix(), sha256_file(output)))
        records.sort()
        lines = [
            "parity_trust_policy_schema_version\t2",
            "trust_domain\tproduction",
            "metadata_path_count\t2",
            "metadata_path\t0000\texact\tdocs/cuda-oxide-parity-status.tsv",
            "metadata_path\t0001\tprefix\tdocs/parity-evidence/archive/",
            "key_count\t2",
        ]
        for index, (role, key_id, relative, digest) in enumerate(records):
            lines.append(
                f"key\t{index:04d}\t{role}\t{key_id}\t{relative}\t{digest}\ted25519"
            )
        policy = staging.joinpath(TRUST_POLICY_RELATIVE)
        policy.write_text("\n".join(lines) + "\n", encoding="ascii")
        policy.chmod(0o444)
        validate_production_trust(staging, policy)
        os.rename(staging, destination)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True)
        raise
    print(f"production trust bootstrap written: {destination}")


def check_trust_update(args: argparse.Namespace) -> None:
    protected_root = args.protected_root.resolve(strict=True)
    candidate_root = args.candidate_root.resolve(strict=True)
    protected_present = args.protected_policy.exists() or args.protected_policy.is_symlink()
    candidate_present = args.candidate_policy.exists() or args.candidate_policy.is_symlink()
    check_row_policy_update(args.protected_row_policy, args.candidate_row_policy)
    if not protected_present:
        if not candidate_present:
            print("no active production trust policy; promotion remains fail-closed")
            return
        validate_production_trust(candidate_root, args.candidate_policy)
        print("initial production trust policy activation is monotonic")
        return

    if not candidate_present:
        fail("active trust policy cannot be removed without break-glass")
    protected = validate_production_trust(protected_root, args.protected_policy)
    candidate = validate_production_trust(candidate_root, args.candidate_policy)
    if candidate.domain != protected.domain:
        fail("trust domain cannot be changed or downgraded")
    if candidate.metadata_paths != protected.metadata_paths:
        fail("trusted metadata allowlist cannot be changed without break-glass")
    if {key.role for key in candidate.keys.values()} != {"attestor", "reviewer"}:
        fail("trust update must retain separated attestor and reviewer roles")
    for identity, key in candidate.keys.items():
        previous = protected.keys.get(identity)
        if previous is None:
            fail("trust update cannot add signing authority without break-glass")
        if previous.fingerprint != key.fingerprint:
            fail("trust update cannot replace signing authority without break-glass")
    print("production trust policy update is monotonic")


def check_protected_base(args: argparse.Namespace) -> None:
    protected_repo = args.protected_repo.resolve(strict=True)
    candidate_repo = args.candidate_repo.resolve(strict=True)
    identities = {
        "protected base": args.protected_base,
        "current default tip": args.default_tip,
        "candidate head": args.candidate_head,
    }
    for label, commit in identities.items():
        if not COMMIT_RE.fullmatch(commit) or commit == "0" * 40:
            fail(f"malformed or zero {label} commit")
    if args.protected_base != args.default_tip:
        fail("pull request base SHA is not current default tip")
    require_commit(protected_repo, args.protected_base, "protected base")
    require_commit(candidate_repo, args.candidate_head, "candidate head")
    if run_git(protected_repo, "rev-parse", "HEAD^{commit}") != args.protected_base:
        fail("protected checkout does not match event base SHA")
    if run_git(candidate_repo, "rev-parse", "HEAD^{commit}") != args.candidate_head:
        fail("candidate checkout does not match event head SHA")
    if subprocess.run(
        [
            "git",
            "-C",
            str(candidate_repo),
            "merge-base",
            "--is-ancestor",
            args.protected_base,
            args.candidate_head,
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    ).returncode:
        fail("candidate head does not contain current protected default tip")
    print("protected base and candidate head are current and ancestry-bound")


def verify_signed(
    root: ArchiveRoot, relative: str, trust: TrustPolicy, role: str
) -> tuple[list[list[str]], bytes, str]:
    raw, raw_lines, rows = archive_read_raw(root, relative)
    path = archive_root_path(root).joinpath(relative)
    if len(rows) < 7:
        fail(f"signed payload is too short: {path}")
    trailer = rows[-6:]
    if (
        trailer[0] != ["signature_schema_version", "1"]
        or trailer[1] != ["signature_domain", trust.domain]
        or trailer[2] != ["signature_role", role]
        or trailer[3] != ["signature_algorithm", "ed25519"]
        or len(trailer[4]) != 2
        or trailer[4][0] != "signing_key_id"
        or len(trailer[5]) != 2
        or trailer[5][0] != "signature_base64"
    ):
        fail(f"non-canonical signed signature context: {path}")
    key_id = trailer[4][1]
    key = trust.keys.get((role, key_id))
    if key is None:
        fail(f"untrusted {role} signing key: {key_id}")
    try:
        signature = base64.b64decode(trailer[5][1], validate=True)
    except (binascii.Error, ValueError):
        fail(f"malformed signature encoding: {path}")
    if len(signature) != 64:
        fail(f"malformed Ed25519 signature length: {path}")
    payload = b"".join(raw_lines[:-1])
    with tempfile.TemporaryDirectory(prefix="fe2o3-signature-") as temp:
        payload_path = Path(temp, "payload")
        signature_path = Path(temp, "signature")
        public_key_path = Path(temp, "public-key.pem")
        payload_path.write_bytes(payload)
        signature_path.write_bytes(signature)
        public_key_path.write_bytes(key.public_key)
        process = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                str(public_key_path),
                "-rawin",
                "-in",
                str(payload_path),
                "-sigfile",
                str(signature_path),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    if process.returncode != 0:
        fail(f"signature verification failed: {path}")
    return rows[:-6], payload, sha256_bytes(raw)


def sign_payload(
    input_path: Path,
    output_path: Path,
    private_key: Path,
    key_id: str,
    *,
    domain: str,
    role: str,
    repo: Path | None,
    test_mode: bool,
) -> None:
    if not ID_RE.fullmatch(key_id) or not ID_RE.fullmatch(role):
        fail("invalid signing key identity")
    expected_domain = "test" if test_mode else "production"
    if domain != expected_domain:
        fail(f"signing domain must be {expected_domain}")
    if not test_mode and repo is None:
        fail("--repo is required outside explicit test mode")
    raw, _, rows = read_raw(input_path)
    if any(row[0].startswith("signature_") or row[0] == "signing_key_id" for row in rows):
        fail("unsigned payload already has a signature trailer")
    if output_path.exists() or output_path.is_symlink():
        fail(f"signed output already exists: {output_path}")
    try:
        info = private_key.lstat()
    except OSError:
        fail("private key is missing")
    if not stat.S_ISREG(info.st_mode) or private_key.is_symlink():
        fail("private key must be a regular file")
    key = private_key.resolve(strict=True)
    if not test_mode:
        if repo is not None and (key == repo or repo in key.parents):
            fail("production private key must be outside the repository")
        if info.st_uid != os.geteuid() or stat.S_IMODE(info.st_mode) != 0o600:
            fail("production private key must be owned by the runner with mode 0600")
    context = (
        b"signature_schema_version\t1\n"
        + f"signature_domain\t{domain}\n".encode()
        + f"signature_role\t{role}\n".encode()
        + b"signature_algorithm\ted25519\n"
        + f"signing_key_id\t{key_id}\n".encode()
    )
    signed_bytes = raw + context
    with tempfile.TemporaryDirectory(prefix="fe2o3-sign-") as temp:
        payload_path = Path(temp, "payload")
        signature_path = Path(temp, "signature")
        payload_path.write_bytes(signed_bytes)
        process = subprocess.run(
            [
                "openssl",
                "pkeyutl",
                "-sign",
                "-inkey",
                str(key),
                "-rawin",
                "-in",
                str(payload_path),
                "-out",
                str(signature_path),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        if process.returncode != 0:
            fail(f"signing failed: {process.stderr.strip()}")
        encoded = base64.b64encode(signature_path.read_bytes()).decode("ascii")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(output_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(signed_bytes)
        stream.write(f"signature_base64\t{encoded}\n".encode())


def verify_bound_file(
    root: ArchiveRoot, path: str, size: str, digest: str, label: str
) -> Path:
    if not re.fullmatch(r"0|[1-9][0-9]*", size) or not SHA256_RE.fullmatch(digest):
        fail(f"invalid {label} size or digest")
    if isinstance(root, ArchiveSnapshot):
        identity = root.validate(path)
        if identity.size != int(size) or identity.digest != digest:
            fail(f"{label} digest mismatch: {path}")
        return root.root.joinpath(path)
    absolute = archive_path(root, path)
    if absolute.stat().st_size != int(size) or sha256_file(absolute) != digest:
        fail(f"{label} digest mismatch: {path}")
    return absolute


def verify_executor(path_text: str, size: str, digest: str) -> tuple[str, int, str]:
    if (
        not path_text.startswith("/")
        or not re.fullmatch(r"0|[1-9][0-9]*", size)
        or not SHA256_RE.fullmatch(digest)
    ):
        fail("invalid absolute executor binding")
    path = Path(path_text)
    try:
        info = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError:
        fail(f"bound executor is missing: {path_text}")
    if (
        str(resolved) != path_text
        or path.is_symlink()
        or not stat.S_ISREG(info.st_mode)
        or not info.st_mode & 0o111
        or stat.S_IMODE(info.st_mode) & 0o022
        or info.st_size != int(size)
        or sha256_file(path) != digest
    ):
        fail(f"bound executor digest or filesystem policy mismatch: {path_text}")
    return path_text, int(size), digest


def parse_executors(cursor: Cursor) -> list[tuple[str, str, int, str]]:
    count = parse_count(cursor.scalar("executor_count"), "executor", allow_zero=False)
    output: list[tuple[str, str, int, str]] = []
    previous = ""
    for index in range(count):
        row = cursor.record("executor", 6, index)
        label, path, size, digest = row[2:]
        if not ID_RE.fullmatch(label) or label <= previous:
            fail("duplicate or unsorted executor")
        path, parsed_size, digest = verify_executor(path, size, digest)
        output.append((label, path, parsed_size, digest))
        previous = label
    return output


def parse_environment(cursor: Cursor) -> list[tuple[str, str]]:
    count = parse_count(cursor.scalar("environment_count"), "environment", allow_zero=False)
    output: list[tuple[str, str]] = []
    previous = ""
    for index in range(count):
        row = cursor.record("environment", 4, index)
        name, encoded = row[2:]
        if (
            not re.fullmatch(r"[A-Z][A-Z0-9_]{0,63}", name)
            or name <= previous
            or not HEX_RE.fullmatch(encoded)
        ):
            fail("invalid or unsorted signed environment")
        try:
            value = bytes.fromhex(encoded).decode("ascii")
        except (UnicodeDecodeError, ValueError):
            fail("signed environment is not canonical ASCII")
        if "\x00" in value or "\n" in value or "\r" in value:
            fail("signed environment contains a control character")
        output.append((name, value))
        previous = name
    return output


@dataclass
class QueueJob:
    job_id: str
    result_id: str
    row: str
    from_status: str
    to_status: str
    timeout: int
    script: str
    script_sha256: str
    result_path: str
    log_path: str
    artifacts: list[tuple[str, str]]


@dataclass
class QueueRecord:
    queue_id: str
    baseline: str
    source: str
    tree: str
    target: str
    lane: str
    execution_closure: str
    archive_root: str
    executors: list[tuple[str, str, int, str]]
    environment: list[tuple[str, str]]
    mode: str
    toolchains: list[tuple[str, str, int, str]]
    jobs: list[QueueJob]
    digest: str
    relative_path: str


def parse_artifact_csv(value: str) -> list[tuple[str, str]]:
    if value == "-":
        return []
    output: list[tuple[str, str]] = []
    previous = ""
    paths: set[str] = set()
    for item in value.split(","):
        if "=" not in item:
            fail("malformed queue artifact")
        label, path = item.split("=", 1)
        if not ID_RE.fullmatch(label) or not valid_relative(path):
            fail("malformed queue artifact")
        if label <= previous or path in paths:
            fail("duplicate or unsorted queue artifact")
        previous = label
        paths.add(path)
        output.append((label, path))
    return output


def parse_queue(
    repo: Path,
    root: ArchiveRoot,
    relative: str,
    trust: TrustPolicy,
    expected_digest: str | None = None,
    *,
    enforce_execution_root: bool = False,
) -> QueueRecord:
    rows, _, digest = verify_signed(root, relative, trust, "attestor")
    if expected_digest is not None and digest != expected_digest:
        fail("signed queue manifest digest mismatch")
    cursor = Cursor(rows, "signed queue")
    if cursor.scalar("signed_queue_schema_version") != "3":
        fail("signed queue schema must be 3")
    queue_id = cursor.scalar("queue_id")
    baseline = cursor.scalar("baseline_commit")
    source = cursor.scalar("source_commit")
    tree = cursor.scalar("source_tree")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    mode = cursor.scalar("execution_mode")
    execution_closure = cursor.scalar("execution_closure")
    archive_root_text = cursor.scalar("archive_root")
    executors = parse_executors(cursor)
    environment = parse_environment(cursor)
    if not RESULT_ID_RE.fullmatch(queue_id):
        fail("invalid queue identity")
    require_tree(repo, source, tree, "queue")
    require_commit(repo, baseline, "queue baseline")
    if subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", baseline, source],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode:
        fail("queue baseline is not an ancestor of source")
    if target != "gfx942" or not re.fullmatch(r"mi300x[a-z0-9._-]*", lane):
        fail("MI300X queue target/lane mismatch")
    if mode not in ("production", "test"):
        fail("invalid queue execution mode")
    toolchain_count = parse_count(cursor.scalar("toolchain_count"), "toolchain", allow_zero=False)
    if execution_closure != "inert":
        fail("shell queue execution closure must remain inert")
    if enforce_execution_root and archive_root_text != str(archive_root_path(root)):
        fail("signed queue archive root does not match execution root")
    if not {"bash", "timeout"} <= {label for label, _, _, _ in executors}:
        fail("signed queue lacks required absolute executors")
    expected_environment = [("HOME", "/nonexistent"), ("LC_ALL", "C"), ("PATH", "/nonexistent")]
    if environment != expected_environment:
        fail("signed queue environment is not the constrained baseline")
    toolchains: list[tuple[str, str, int, str]] = []
    previous = ""
    for index in range(toolchain_count):
        row = cursor.record("toolchain", 6, index)
        label, closure, size, closure_digest = row[2], row[3], row[4], row[5]
        if not ID_RE.fullmatch(label) or label <= previous:
            fail("duplicate or unsorted queue toolchain")
        verify_bound_file(root, closure, size, closure_digest, "toolchain closure")
        toolchains.append((label, closure, int(size), closure_digest))
        previous = label
    job_count = parse_count(cursor.scalar("job_count"), "queue job", allow_zero=False)
    jobs: list[QueueJob] = []
    seen_ids: set[str] = set()
    seen_results: set[str] = set()
    seen_outputs: set[str] = set()
    for index in range(job_count):
        row = cursor.record("job", 14, index)
        (
            job_id,
            result_id,
            evidence_row,
            from_status,
            to_status,
            timeout_text,
            script,
            script_digest,
            result_path,
            log_path,
            artifacts_text,
            evidence_class,
        ) = row[2:]
        if evidence_class != "hardware":
            fail("MI300X queue jobs must be hardware evidence")
        if (
            not ID_RE.fullmatch(job_id)
            or not RESULT_ID_RE.fullmatch(result_id)
            or not valid_row(evidence_row)
            or not valid_transition(from_status, to_status)
            or not timeout_text.isdigit()
            or not 1 <= int(timeout_text) <= 14400
            or not valid_relative(script)
            or not script.startswith("scripts/")
            or not SHA256_RE.fullmatch(script_digest)
            or not valid_relative(result_path)
            or not valid_relative(log_path)
        ):
            fail(f"malformed queue job {index:04d}")
        verify_repo_source_file(repo, script, source, script_digest)
        artifacts = parse_artifact_csv(artifacts_text)
        if not artifacts:
            fail("hardware queue job needs an artifact")
        if job_id in seen_ids or result_id in seen_results:
            fail("duplicate queue job or result identity")
        outputs = [result_path, log_path, *(path for _, path in artifacts)]
        if any(value in seen_outputs for value in outputs):
            fail("duplicate queue output path")
        seen_ids.add(job_id)
        seen_results.add(result_id)
        seen_outputs.update(outputs)
        jobs.append(
            QueueJob(
                job_id,
                result_id,
                evidence_row,
                from_status,
                to_status,
                int(timeout_text),
                script,
                script_digest,
                result_path,
                log_path,
                artifacts,
            )
        )
    cursor.done()
    return QueueRecord(
        queue_id,
        baseline,
        source,
        tree,
        target,
        lane,
        execution_closure,
        archive_root_text,
        executors,
        environment,
        mode,
        toolchains,
        jobs,
        digest,
        relative,
    )


@dataclass
class ResultRecord:
    result_id: str
    row: str
    from_status: str
    to_status: str
    baseline: str
    source: str
    tree: str
    evidence_class: str
    target: str
    lane: str
    mode: str
    execution_closure: str
    executors: list[tuple[str, str, int, str]]
    environment: list[tuple[str, str]]
    queue_path: str
    queue_digest: str
    queue_id: str
    timeout: int
    toolchains: list[tuple[str, str, int, str]]
    commands: list[tuple[str, str]]
    log: tuple[str, int, str]
    artifacts: list[tuple[str, str, int, str]]
    digest: str
    relative_path: str


def parse_result(
    repo: Path,
    root: ArchiveRoot,
    relative: str,
    trust: TrustPolicy,
    expected_digest: str | None = None,
) -> ResultRecord:
    rows, _, digest = verify_signed(root, relative, trust, "attestor")
    if expected_digest is not None and digest != expected_digest:
        fail(f"signed result digest mismatch: {relative}")
    cursor = Cursor(rows, "signed result")
    schema = cursor.scalar("signed_result_schema_version")
    if schema not in ("2", "3"):
        fail("signed result schema must be 2 or 3")
    result_id = cursor.scalar("result_id")
    row_id = cursor.scalar("row_id")
    from_status = cursor.scalar("from_status")
    to_status = cursor.scalar("to_status")
    baseline = cursor.scalar("baseline_commit")
    source = cursor.scalar("source_commit")
    tree = cursor.scalar("source_tree")
    evidence_class = cursor.scalar("evidence_class")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    mode = cursor.scalar("execution_mode")
    execution_closure = "not-applicable"
    executors: list[tuple[str, str, int, str]] = []
    environment: list[tuple[str, str]] = []
    if schema == "3":
        execution_closure = cursor.scalar("execution_closure")
        executors = parse_executors(cursor)
        environment = parse_environment(cursor)
    queue_path = cursor.scalar("queue_manifest_path")
    queue_digest = cursor.scalar("queue_manifest_sha256")
    queue_id = cursor.scalar("queue_id")
    timeout_text = cursor.scalar("timeout_seconds")
    if (
        not RESULT_ID_RE.fullmatch(result_id)
        or not valid_row(row_id)
        or not valid_transition(from_status, to_status)
        or evidence_class not in CLASS_RANK
        or not TARGET_RE.fullmatch(target)
        or not LANE_RE.fullmatch(lane)
        or mode not in ("production", "test")
        or not re.fullmatch(r"0|[1-9][0-9]*", timeout_text)
    ):
        fail("malformed signed result identity")
    require_commit(repo, baseline, "result baseline")
    require_tree(repo, source, tree, "result")
    toolchain_count = parse_count(cursor.scalar("toolchain_count"), "toolchain", allow_zero=False)
    toolchains: list[tuple[str, str, int, str]] = []
    previous = ""
    for index in range(toolchain_count):
        item = cursor.record("toolchain", 6, index)
        label, closure, size, closure_digest = item[2], item[3], item[4], item[5]
        if not ID_RE.fullmatch(label) or label <= previous:
            fail("duplicate or unsorted result toolchain")
        verify_bound_file(root, closure, size, closure_digest, "toolchain closure")
        toolchains.append((label, closure, int(size), closure_digest))
        previous = label
    command_count = parse_count(cursor.scalar("command_count"), "command", allow_zero=False)
    commands: list[tuple[str, str]] = []
    for index in range(command_count):
        item = cursor.record("command", 4, index)
        if not HEX_RE.fullmatch(item[2]) or item[3] != "0":
            fail("result command did not pass")
        try:
            command_text = bytes.fromhex(item[2]).decode("ascii")
            arguments = shlex.split(command_text)
        except (UnicodeDecodeError, ValueError):
            fail("result command is not canonical ASCII")
        if not arguments or command_hex(arguments) != item[2]:
            fail("result command is not canonical")
        commands.append((item[2], item[3]))
    log_record = cursor.record("log", 5, 0)
    verify_bound_file(root, log_record[2], log_record[3], log_record[4], "result log")
    log_data = (log_record[2], int(log_record[3]), log_record[4])
    artifact_count = parse_count(cursor.scalar("artifact_count"), "artifact")
    previous = ""
    artifact_paths: set[str] = set()
    artifacts: list[tuple[str, str, int, str]] = []
    for index in range(artifact_count):
        item = cursor.record("artifact", 6, index)
        label, artifact_path, size, artifact_digest = item[2:]
        if not ID_RE.fullmatch(label) or label <= previous or artifact_path in artifact_paths:
            fail("duplicate or unsorted result artifact")
        verify_bound_file(root, artifact_path, size, artifact_digest, "result artifact")
        artifacts.append((label, artifact_path, int(size), artifact_digest))
        previous = label
        artifact_paths.add(artifact_path)
    cursor.done()
    if evidence_class in ("compile", "hardware") and artifact_count == 0:
        fail(f"{evidence_class} result needs an artifact")
    if evidence_class == "hardware":
        if (
            schema != "3"
            or execution_closure != "inert"
            or queue_path == "-"
            or not valid_relative(queue_path)
            or not SHA256_RE.fullmatch(queue_digest)
            or not RESULT_ID_RE.fullmatch(queue_id)
            or not 1 <= int(timeout_text) <= 14400
            or target == "generic"
            or lane == "-"
        ):
            fail("hardware result has no signed queue binding")
        queue = parse_queue(repo, root, queue_path, trust, queue_digest)
        jobs = [job for job in queue.jobs if job.result_id == result_id]
        if len(jobs) != 1:
            fail("hardware result identity is absent from signed queue")
        job = jobs[0]
        expected_command = (command_hex(queue_invocation(queue, job)), "0")
        artifact_identity = [(label, path) for label, path, _, _ in artifacts]
        queue_artifact_identity = list(job.artifacts)
        if (
            job.row != row_id
            or job.from_status != from_status
            or job.to_status != to_status
            or job.result_path != relative
            or queue.baseline != baseline
            or queue.source != source
            or queue.tree != tree
            or queue.target != target
            or queue.lane != lane
            or queue.mode != mode
            or queue_id != queue.queue_id
            or execution_closure != queue.execution_closure
            or executors != queue.executors
            or environment != queue_environment(queue, job)
            or int(timeout_text) != job.timeout
            or toolchains != queue.toolchains
            or commands != [expected_command]
            or log_data[0] != job.log_path
            or artifact_identity != queue_artifact_identity
            or len(artifacts) != len(job.artifacts)
        ):
            fail("hardware result does not match signed queue job")
    elif (
        schema != "2"
        or execution_closure != "not-applicable"
        or executors
        or environment
        or queue_path != "-"
        or queue_digest != "-"
        or queue_id != "-"
        or timeout_text != "0"
    ):
        fail("non-hardware result cannot claim a queue")
    return ResultRecord(
        result_id,
        row_id,
        from_status,
        to_status,
        baseline,
        source,
        tree,
        evidence_class,
        target,
        lane,
        mode,
        execution_closure,
        executors,
        environment,
        queue_path,
        queue_digest,
        queue_id,
        int(timeout_text),
        toolchains,
        commands,
        log_data,
        artifacts,
        digest,
        relative,
    )


@dataclass
class Authorization:
    authorization_id: str
    row: str
    from_status: str
    baseline: str
    source: str
    tree: str
    target: str
    lane: str
    evidence_set: str
    mode: str
    digest: str
    relative_path: str


def parse_authorization(
    repo: Path,
    root: ArchiveRoot,
    relative: str,
    expected_digest: str,
    trust: TrustPolicy,
) -> Authorization:
    rows, _, digest = verify_signed(root, relative, trust, "reviewer")
    if digest != expected_digest:
        fail("review authorization digest mismatch")
    cursor = Cursor(rows, "review authorization")
    if cursor.scalar("review_authorization_schema_version") != "1":
        fail("review authorization schema must be 1")
    authorization_id = cursor.scalar("authorization_id")
    row = cursor.scalar("row_id")
    from_status = cursor.scalar("from_status")
    baseline = cursor.scalar("baseline_commit")
    source = cursor.scalar("source_commit")
    tree = cursor.scalar("source_tree")
    if cursor.scalar("to_status") != "Complete":
        fail("review authorization must authorize Complete")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    evidence_set = cursor.scalar("evidence_set_sha256")
    reviewer = cursor.scalar("reviewer_identity")
    mode = cursor.scalar("execution_mode")
    cursor.done()
    if (
        not RESULT_ID_RE.fullmatch(authorization_id)
        or not valid_row(row)
        or not valid_transition(from_status, "Complete")
        or not SHA256_RE.fullmatch(evidence_set)
        or not ID_RE.fullmatch(reviewer)
        or mode not in ("production", "test")
    ):
        fail("malformed review authorization")
    require_commit(repo, baseline, "authorization baseline")
    require_tree(repo, source, tree, "authorization")
    return Authorization(
        authorization_id,
        row,
        from_status,
        baseline,
        source,
        tree,
        target,
        lane,
        evidence_set,
        mode,
        digest,
        relative,
    )


@dataclass
class PromotionManifest:
    baseline: str
    source: str
    tree: str
    target: str
    lane: str
    evidence_set: str
    results: list[ResultRecord]
    authorizations: list[Authorization]


def promotion_archive_closure(
    repo: Path,
    root: ArchiveRoot,
    manifest_relative: str,
    manifest: PromotionManifest,
    trust: TrustPolicy,
) -> set[str]:
    paths = {manifest_relative}
    result_by_id = {result.result_id: result for result in manifest.results}
    referenced_queues: dict[str, QueueRecord] = {}
    queue_result_ids: dict[str, set[str]] = {}
    for result in manifest.results:
        paths.add(result.relative_path)
        paths.add(result.log[0])
        paths.update(path for _, path, _, _ in result.toolchains)
        paths.update(path for _, path, _, _ in result.artifacts)
        if result.queue_path != "-":
            queue = parse_queue(repo, root, result.queue_path, trust, result.queue_digest)
            previous = referenced_queues.get(result.queue_path)
            if previous is not None and previous.digest != queue.digest:
                fail("promotion results disagree on a signed queue identity")
            referenced_queues[result.queue_path] = queue
            queue_result_ids.setdefault(result.queue_path, set()).add(result.result_id)
            paths.add(result.queue_path)
            paths.update(path for _, path, _, _ in queue.toolchains)
    for relative, queue in referenced_queues.items():
        expected_ids = {job.result_id for job in queue.jobs}
        if queue_result_ids[relative] != expected_ids:
            fail("referenced queue job/result set is not exact")
        for job in queue.jobs:
            result = result_by_id.get(job.result_id)
            if (
                result is None
                or result.evidence_class != "hardware"
                or result.queue_path != relative
                or result.relative_path != job.result_path
            ):
                fail("referenced queue job has no exact manifest result")
            paths.add(job.result_path)
            paths.add(job.log_path)
            paths.update(path for _, path in job.artifacts)
    paths.update(value.relative_path for value in manifest.authorizations)
    if ARCHIVE_INDEX_RELATIVE in paths:
        fail("promotion manifest cannot reference the reserved archive index")
    return paths


def parse_manifest(
    repo: Path,
    root: ArchiveRoot,
    relative: str,
    trust: TrustPolicy,
) -> PromotionManifest:
    _, raw_lines, rows = archive_read_raw(root, relative)
    cursor = Cursor(rows, "promotion manifest")
    if cursor.scalar("promotion_manifest_schema_version") != "2":
        fail("promotion manifest schema must be 2")
    baseline = cursor.scalar("baseline_commit")
    source = cursor.scalar("source_commit")
    tree = cursor.scalar("source_tree")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    require_commit(repo, baseline, "manifest baseline")
    require_tree(repo, source, tree, "manifest")
    if not TARGET_RE.fullmatch(target) or not LANE_RE.fullmatch(lane):
        fail("malformed manifest target/lane")
    result_count = parse_count(cursor.scalar("result_count"), "result", allow_zero=False)
    results: list[ResultRecord] = []
    seen_ids: set[str] = set()
    seen_paths: set[str] = set()
    seen_digests: set[str] = set()
    previous: tuple[int, int] = (-1, -1)
    for index in range(result_count):
        item = cursor.record("result", 9, index)
        row, from_status, to_status, evidence_class, result_path, digest, declared_id = item[2:]
        if (
            not valid_row(row)
            or not valid_transition(from_status, to_status)
            or evidence_class not in CLASS_RANK
            or not valid_relative(result_path)
            or not SHA256_RE.fullmatch(digest)
            or not RESULT_ID_RE.fullmatch(declared_id)
        ):
            fail("malformed manifest result")
        order = (row_rank(row), CLASS_RANK[evidence_class])
        if order <= previous:
            fail("duplicate or unsorted manifest result")
        result = parse_result(repo, root, result_path, trust, digest)
        if (
            result.row != row
            or result.from_status != from_status
            or result.to_status != to_status
            or result.evidence_class != evidence_class
            or result.result_id != declared_id
            or result.baseline != baseline
            or result.source != source
            or result.tree != tree
            or result.target != target
            or result.lane != lane
        ):
            fail("manifest result relabeling or source mismatch")
        if declared_id in seen_ids or result_path in seen_paths or digest in seen_digests:
            fail("duplicate result identity, path, or digest")
        seen_ids.add(declared_id)
        seen_paths.add(result_path)
        seen_digests.add(digest)
        results.append(result)
        previous = order
    evidence_end = cursor.index
    evidence_set = cursor.scalar("evidence_set_sha256")
    computed = sha256_bytes(b"".join(raw_lines[:evidence_end]))
    if not SHA256_RE.fullmatch(evidence_set) or evidence_set != computed:
        fail("promotion evidence-set digest mismatch")
    authorization_count = parse_count(cursor.scalar("authorization_count"), "authorization")
    authorizations: list[Authorization] = []
    seen_authorization_ids: set[str] = set()
    previous_row = -1
    for index in range(authorization_count):
        item = cursor.record("authorization", 5, index)
        row, authorization_path, digest = item[2:]
        if (
            not valid_row(row)
            or not valid_relative(authorization_path)
            or not SHA256_RE.fullmatch(digest)
        ):
            fail("malformed manifest authorization")
        rank = row_rank(row)
        if rank <= previous_row:
            fail("duplicate or unsorted manifest authorization")
        authorization = parse_authorization(repo, root, authorization_path, digest, trust)
        if (
            authorization.row != row
            or authorization.baseline != baseline
            or authorization.source != source
            or authorization.tree != tree
            or authorization.target != target
            or authorization.lane != lane
            or authorization.evidence_set != evidence_set
        ):
            fail("review authorization does not bind the promotion evidence set")
        if authorization.authorization_id in seen_authorization_ids:
            fail("duplicate review authorization identity")
        seen_authorization_ids.add(authorization.authorization_id)
        authorizations.append(authorization)
        previous_row = rank
    cursor.done()
    return PromotionManifest(
        baseline,
        source,
        tree,
        target,
        lane,
        evidence_set,
        results,
        authorizations,
    )


def scan_archive(root: Path, *, require_immutable: bool) -> ArchiveSnapshot:
    return ArchiveSnapshot(root, require_immutable=require_immutable)


def expected_archive_directories(paths: set[str]) -> set[str]:
    output: set[str] = set()
    for relative in paths:
        for parent in Path(relative).parents:
            if parent == Path("."):
                break
            output.add(parent.as_posix())
    return output


def actual_archive_directories(root: ArchiveRoot) -> set[str]:
    if isinstance(root, ArchiveSnapshot):
        return set(root.directories)
    root = root.resolve(strict=True)
    return {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_dir() and not path.is_symlink()
    }


def archive_index_bytes(
    manifest_relative: str,
    manifest_digest: str,
    manifest: PromotionManifest,
    records: dict[str, tuple[int, str]],
) -> bytes:
    lines = [
        "parity_archive_index_schema_version\t1",
        f"manifest_path\t{manifest_relative}",
        f"manifest_sha256\t{manifest_digest}",
        f"baseline_commit\t{manifest.baseline}",
        f"source_commit\t{manifest.source}",
        f"source_tree\t{manifest.tree}",
        f"target\t{manifest.target}",
        f"hardware_lane\t{manifest.lane}",
        f"file_count\t{len(records)}",
    ]
    for index, relative in enumerate(sorted(records)):
        size, digest = records[relative]
        lines.append(f"file\t{index:04d}\t{relative}\t{size}\t{digest}")
    return ("\n".join(lines) + "\n").encode("ascii")


def verify_archive_index(
    repo: Path,
    root: ArchiveRoot,
    manifest_relative: str,
    manifest: PromotionManifest,
    trust: TrustPolicy,
) -> None:
    if not isinstance(root, ArchiveSnapshot):
        with ArchiveSnapshot(root, require_immutable=False) as snapshot:
            verify_archive_index(
                repo, snapshot, manifest_relative, manifest, trust
            )
        return
    closure = promotion_archive_closure(
        repo, root, manifest_relative, manifest, trust
    )
    actual = root.records
    expected_paths = closure | {ARCHIVE_INDEX_RELATIVE}
    if set(actual) != expected_paths:
        missing = sorted(expected_paths - set(actual))
        extra = sorted(set(actual) - expected_paths)
        fail(f"archive index closure mismatch: missing={missing} extra={extra}")
    expected_directories = expected_archive_directories(expected_paths)
    actual_directories = actual_archive_directories(root)
    if actual_directories != expected_directories:
        fail(
            "archive directory closure mismatch: "
            f"missing={sorted(expected_directories - actual_directories)} "
            f"extra={sorted(actual_directories - expected_directories)}"
        )
    _, _, rows = archive_read_raw(root, ARCHIVE_INDEX_RELATIVE)
    cursor = Cursor(rows, "archive index")
    if cursor.scalar("parity_archive_index_schema_version") != "1":
        fail("archive index schema must be 1")
    if cursor.scalar("manifest_path") != manifest_relative:
        fail("archive index manifest path mismatch")
    manifest_digest = archive_digest(root, manifest_relative)
    if cursor.scalar("manifest_sha256") != manifest_digest:
        fail("archive index manifest digest mismatch")
    if cursor.scalar("baseline_commit") != manifest.baseline:
        fail("archive index baseline commit mismatch")
    if cursor.scalar("source_commit") != manifest.source:
        fail("archive index source commit mismatch")
    if cursor.scalar("source_tree") != manifest.tree:
        fail("archive index source tree mismatch")
    if cursor.scalar("target") != manifest.target:
        fail("archive index target mismatch")
    if cursor.scalar("hardware_lane") != manifest.lane:
        fail("archive index hardware lane mismatch")
    count_text = cursor.scalar("file_count")
    if not re.fullmatch(r"0|[1-9][0-9]*", count_text):
        fail("invalid archive index file count")
    count = int(count_text)
    if count != len(closure) or count > MAX_ARCHIVE_FILES:
        fail("archive index file count mismatch")
    indexed: dict[str, tuple[int, str]] = {}
    previous = ""
    for position in range(count):
        row = cursor.record("file", 5, position)
        relative, size_text, digest = row[2:]
        if (
            not valid_relative(relative)
            or relative <= previous
            or not re.fullmatch(r"0|[1-9][0-9]*", size_text)
            or not SHA256_RE.fullmatch(digest)
        ):
            fail("malformed or unsorted archive index entry")
        indexed[relative] = (int(size_text), digest)
        previous = relative
    cursor.done()
    expected_records = {path: actual[path] for path in closure}
    if indexed != expected_records:
        fail("archive index file identity mismatch")


def rename_noreplace(source: Path, destination: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    try:
        renameat2 = libc.renameat2
    except AttributeError:
        fail("atomic no-replace archive publication is unavailable")
    renameat2.argtypes = [
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    ]
    renameat2.restype = ctypes.c_int
    if renameat2(
        AT_FDCWD,
        os.fsencode(source),
        AT_FDCWD,
        os.fsencode(destination),
        RENAME_NOREPLACE,
    ) != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST:
            fail("evidence archive destination already exists")
        fail(f"atomic no-replace archive publication failed: {os.strerror(error)}")


def ingest_archive(args: argparse.Namespace) -> None:
    with scan_archive(
        args.source_root, require_immutable=not args.allow_test_fixtures
    ) as source_root:
        ingest_archive_snapshot(args, source_root)


def ingest_archive_snapshot(
    args: argparse.Namespace, source_root: ArchiveSnapshot
) -> None:
    repo = args.repo.resolve(strict=True)
    trust = parse_trust_policy(args.trusted_root, args.trust_policy)
    if not args.allow_test_fixtures:
        if trust.domain != "production":
            fail("production archive ingestion requires a production trust domain")
        trust = validate_production_trust(args.trusted_root, args.trust_policy)
    source_records = source_root.records
    if not SHA256_RE.fullmatch(args.expected_manifest_sha256):
        fail("malformed expected promotion manifest digest")
    if archive_digest(source_root, args.manifest) != args.expected_manifest_sha256:
        fail("promotion manifest does not match the operator-pinned digest")
    manifest = parse_manifest(repo, source_root, args.manifest, trust)
    if (
        manifest.baseline != args.expected_baseline
        or manifest.source != args.expected_source
        or manifest.tree != args.expected_tree
        or manifest.target != args.expected_target
        or manifest.lane != args.expected_lane
    ):
        fail("promotion archive does not match the operator-pinned baseline, source, or lane")
    if not args.allow_test_fixtures:
        if any(result.mode != "production" for result in manifest.results):
            fail("test result cannot enter a production evidence archive")
        if any(value.mode != "production" for value in manifest.authorizations):
            fail("test review cannot enter a production evidence archive")
    closure = promotion_archive_closure(
        repo, source_root, args.manifest, manifest, trust
    )
    if set(source_records) != closure:
        missing = sorted(closure - set(source_records))
        extra = sorted(set(source_records) - closure)
        fail(f"source archive closure mismatch: missing={missing} extra={extra}")
    expected_directories = expected_archive_directories(closure)
    actual_directories = actual_archive_directories(source_root)
    if actual_directories != expected_directories:
        fail(
            "source archive directory closure mismatch: "
            f"missing={sorted(expected_directories - actual_directories)} "
            f"extra={sorted(actual_directories - expected_directories)}"
        )

    destination = args.destination_root.absolute()
    if destination.exists() or destination.is_symlink():
        fail("evidence archive destination already exists")
    parent = destination.parent.resolve(strict=True)
    staging = Path(tempfile.mkdtemp(prefix=".fe2o3-archive-", dir=parent))
    try:
        for relative in sorted(closure):
            source_root.copy_to(relative, staging.joinpath(relative))
        index = staging.joinpath(ARCHIVE_INDEX_RELATIVE)
        index.write_bytes(
            archive_index_bytes(
                args.manifest,
                args.expected_manifest_sha256,
                manifest,
                source_records,
            )
        )
        index.chmod(0o444)
        with ArchiveSnapshot(staging, require_immutable=False) as copied:
            copied_manifest = parse_manifest(repo, copied, args.manifest, trust)
            verify_archive_index(repo, copied, args.manifest, copied_manifest, trust)
        for path in sorted(staging.rglob("*"), reverse=True):
            path.chmod(0o555 if path.is_dir() else 0o444)
        staging.chmod(0o555)
        rename_noreplace(staging, destination)
    except BaseException:
        for path in staging.rglob("*"):
            try:
                path.chmod(0o700 if path.is_dir() else 0o600)
            except OSError:
                pass
        staging.chmod(0o700)
        shutil.rmtree(staging, ignore_errors=True)
        raise
    print(f"closed signed evidence archive ingested: {destination}")


@dataclass(frozen=True)
class RowPolicy:
    target: str
    partial: tuple[str, ...]
    complete: tuple[str, ...]
    reviewer_role: str


def parse_classes(value: str, label: str) -> tuple[str, ...]:
    classes = tuple(value.split(","))
    if not classes or any(item not in CLASS_RANK for item in classes):
        fail(f"invalid {label} evidence classes")
    ranks = [CLASS_RANK[item] for item in classes]
    if ranks != sorted(set(ranks)):
        fail(f"duplicate or unsorted {label} evidence classes")
    return classes


def parse_policy(path: Path) -> dict[str, RowPolicy]:
    _, _, rows = read_raw(path)
    cursor = Cursor(rows, "row policy")
    if cursor.scalar("row_evidence_policy_schema_version") != "2":
        fail("row policy schema must be 2")
    count = parse_count(cursor.scalar("row_count"), "policy row", allow_zero=False)
    output: dict[str, RowPolicy] = {}
    previous = 0
    for index in range(count):
        item = cursor.record("row", 7, index)
        row, target, partial_csv, complete_csv, reviewer = item[2:]
        if (
            not valid_row(row)
            or not TARGET_RE.fullmatch(target)
            or not ID_RE.fullmatch(reviewer)
        ):
            fail("malformed row policy")
        rank = row_rank(row)
        if rank <= previous:
            fail("duplicate or unsorted row policy")
        partial = parse_classes(partial_csv, f"{row} Partial")
        complete = parse_classes(complete_csv, f"{row} Complete")
        if not set(partial) < set(complete):
            fail(f"Complete policy must strictly strengthen Partial for row {row}")
        output[row] = RowPolicy(target, partial, complete, reviewer)
        previous = rank
    cursor.done()
    return output


def check_row_policy_update(protected_path: Path, candidate_path: Path) -> None:
    protected = parse_policy(protected_path)
    candidate = parse_policy(candidate_path)
    if set(candidate) != set(protected):
        fail("row policy row set cannot change without break-glass")
    protected_targets = {policy.target for policy in protected.values()}
    candidate_targets = {policy.target for policy in candidate.values()}
    if candidate_targets != protected_targets:
        fail("row policy target set cannot change without break-glass")
    for row, previous in protected.items():
        current = candidate[row]
        if current.target != previous.target:
            fail(f"row policy target identity cannot change for row {row}")
        if previous.reviewer_role != "reviewer":
            fail(f"protected row policy has an invalid reviewer role for row {row}")
        if current.reviewer_role != previous.reviewer_role:
            fail(f"row policy reviewer role cannot change for row {row}")
        if not set(current.partial).issuperset(previous.partial):
            fail(f"Partial evidence requirements cannot be removed for row {row}")
        if not set(current.complete).issuperset(previous.complete):
            fail(f"Complete evidence requirements cannot be removed for row {row}")


def require_reviewer_roles(policy: dict[str, RowPolicy]) -> None:
    for row, value in policy.items():
        if value.reviewer_role != "reviewer":
            fail(f"row policy reviewer role must be reviewer for row {row}")


def parse_status(path: Path, label: str) -> tuple[str, dict[str, str]]:
    _, _, rows = read_raw(path)
    commit = ""
    output: dict[str, str] = {}
    order: list[int] = []
    for row in rows:
        if len(row) == 2 and row[0] == "fe2o3_commit":
            if commit:
                fail(f"duplicate {label} fe2o3_commit")
            commit = row[1]
        elif len(row) == 3 and row[0] in ("normative", "supplemental"):
            row_id, status_value = row[1], row[2]
            if (
                not valid_row(row_id)
                or status_value not in ("Complete", "Partial", "Missing", "N/A")
            ):
                fail(f"malformed {label} status row")
            if row_id in output:
                fail(f"duplicate {label} status row: {row_id}")
            output[row_id] = status_value
            order.append(row_rank(row_id))
    if not COMMIT_RE.fullmatch(commit):
        fail(f"malformed {label} fe2o3_commit")
    if not output or order != sorted(order):
        fail(f"{label} status rows are empty or unsorted")
    return commit, output


def metadata_delta_is_allowed(repo: Path, source: str, trust: TrustPolicy) -> None:
    head = run_git(repo, "rev-parse", "HEAD")
    process = subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", source, head],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if process.returncode != 0:
        fail("attested source is not an ancestor of candidate HEAD")
    process = subprocess.run(
        ["git", "-C", str(repo), "diff", "--name-only", "--no-renames", "-z", source, head],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if process.returncode != 0:
        fail("cannot inspect post-attestation source delta")
    changed = process.stdout.decode("utf-8").split("\0")
    for value in filter(None, changed):
        allowed = any(
            (kind == "exact" and value == path)
            or (kind == "prefix" and value.startswith(path))
            for kind, path in trust.metadata_paths
        )
        if not allowed:
            fail(f"implementation changed after attestation: {value}")


def gate(args: argparse.Namespace) -> None:
    with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
        gate_snapshot(args, root)


def gate_snapshot(args: argparse.Namespace, root: ArchiveSnapshot) -> None:
    repo = args.repo.resolve(strict=True)
    trust = parse_trust_policy(args.trusted_root, args.trust_policy)
    if not args.allow_test_fixtures:
        if trust.domain != "production":
            fail("production promotion requires a production trust domain")
        trust = validate_production_trust(args.trusted_root, args.trust_policy)
    trusted_policy_bytes = args.trusted_policy.read_bytes()
    if args.candidate_policy.read_bytes() != trusted_policy_bytes:
        fail("candidate row policy differs from protected baseline policy")
    policy = parse_policy(args.trusted_policy)
    require_reviewer_roles(policy)
    baseline_commit, baseline_status = parse_status(args.baseline_status, "baseline")
    source_commit, candidate_status = parse_status(args.candidate_status, "candidate")
    if set(baseline_status) != set(candidate_status) or set(policy) != set(candidate_status):
        fail("status and persistent policy row sets differ")
    require_commit(repo, baseline_commit, "baseline status")
    require_commit(repo, source_commit, "candidate status")
    if subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", baseline_commit, source_commit],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    ).returncode:
        fail("candidate status source is not descended from baseline")
    manifest = parse_manifest(repo, root, args.manifest, trust)
    if not args.allow_test_fixtures:
        verify_archive_index(repo, root, args.manifest, manifest, trust)
    if manifest.baseline != baseline_commit or manifest.source != source_commit:
        fail("promotion manifest status commit mismatch")
    metadata_delta_is_allowed(repo, source_commit, trust)
    by_row: dict[str, list[ResultRecord]] = {}
    for result in manifest.results:
        by_row.setdefault(result.row, []).append(result)
        if result.evidence_class == "hardware" and result.execution_closure != "verified":
            fail("hardware result execution closure is inert and cannot promote parity")
        if result.mode != trust.domain and not args.allow_test_fixtures:
            fail("test evidence cannot authorize a production promotion")
    authorization_by_row = {value.row: value for value in manifest.authorizations}
    promotions = 0
    changed_rows: set[str] = set()
    complete_rows: set[str] = set()
    for row, before in baseline_status.items():
        after = candidate_status[row]
        if before == after:
            if row in by_row or row in authorization_by_row:
                fail(f"unused evidence for unchanged row {row}")
            continue
        changed_rows.add(row)
        if not valid_transition(before, after):
            fail(f"unsupported status transition for {row}: {before} -> {after}")
        row_policy = policy[row]
        if manifest.target != row_policy.target:
            fail(f"target mismatch for row {row}")
        records = by_row.get(row, [])
        classes = tuple(record.evidence_class for record in records)
        required = row_policy.partial if after == "Partial" else row_policy.complete
        if classes != required:
            fail(f"insufficient or extra evidence for row {row}: expected {','.join(required)}")
        if any(
            record.from_status != before or record.to_status != after for record in records
        ):
            fail(f"result transition mismatch for row {row}")
        authorization = authorization_by_row.get(row)
        if after == "Complete":
            complete_rows.add(row)
            if authorization is None:
                fail(f"Complete promotion lacks reviewed authorization for row {row}")
            if authorization.from_status != before:
                fail(f"review authorization transition mismatch for row {row}")
            if authorization.mode != trust.domain and not args.allow_test_fixtures:
                fail("test review authorization cannot authorize production")
        elif authorization is not None:
            fail(f"Partial promotion cannot carry Complete authorization for row {row}")
        promotions += 1
    if promotions == 0:
        fail("promotion gate requires a supported status transition")
    if set(by_row) != changed_rows:
        fail("manifest contains evidence unused by a promotion")
    if set(authorization_by_row) != complete_rows:
        fail("manifest review authorization set is not exact")
    print(f"signed parity evidence gate passed: {promotions} promotion(s)")


def lock_path_for(args: argparse.Namespace) -> Path:
    if args.test_mode:
        if args.lock_root is None:
            fail("--test-mode requires --lock-root")
        info = args.lock_root.lstat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or args.lock_root.is_symlink()
            or info.st_uid != os.geteuid()
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail("unsafe test lock root")
        root = args.lock_root.resolve(strict=True)
        return root.joinpath(DEFAULT_LOCK.name)
    if args.lock_root is not None:
        fail("alternate lock roots require explicit test mode")
    return DEFAULT_LOCK


def acquire_lock(path: Path) -> int:
    try:
        parent_info = path.parent.lstat()
    except OSError:
        fail(f"canonical MI300X lock directory is not provisioned: {path.parent}")
    if (
        not stat.S_ISDIR(parent_info.st_mode)
        or path.parent.is_symlink()
        or parent_info.st_uid != os.geteuid()
        or stat.S_IMODE(parent_info.st_mode) & 0o022
    ):
        fail("unsafe canonical MI300X lock directory")
    try:
        before = path.lstat()
    except OSError:
        fail(f"canonical MI300X lock is not provisioned: {path}")
    if (
        not stat.S_ISREG(before.st_mode)
        or path.is_symlink()
        or before.st_uid != os.geteuid()
        or stat.S_IMODE(before.st_mode) != 0o600
        or before.st_nlink != 1
    ):
        fail("unsafe canonical MI300X lock ownership, mode, or link count")
    flags = os.O_RDWR | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    opened = os.fstat(descriptor)
    if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
        os.close(descriptor)
        fail("canonical MI300X lock changed during open")
    fcntl.flock(descriptor, fcntl.LOCK_EX)
    after = path.lstat()
    if (after.st_dev, after.st_ino) != (opened.st_dev, opened.st_ino):
        os.close(descriptor)
        fail("canonical MI300X lock changed after acquisition")
    return descriptor


def command_hex(arguments: Iterable[str]) -> str:
    return shlex.join(list(arguments)).encode("ascii").hex()


def executor_map(queue: QueueRecord) -> dict[str, str]:
    return {label: path for label, path, _, _ in queue.executors}


def queue_invocation(queue: QueueRecord, job: QueueJob) -> list[str]:
    executors = executor_map(queue)
    return [
        executors["timeout"],
        "--signal=TERM",
        "--kill-after=5s",
        str(job.timeout),
        executors["bash"],
        job.script,
    ]


def queue_environment(queue: QueueRecord, job: QueueJob) -> list[tuple[str, str]]:
    environment = dict(queue.environment)
    environment.update(
        {
            "FE2O3_EVIDENCE_ARCHIVE_ROOT": queue.archive_root,
            "FE2O3_EVIDENCE_ARTIFACTS": ",".join(
                f"{label}={path}" for label, path in job.artifacts
            ),
            "FE2O3_EVIDENCE_HARDWARE_LANE": queue.lane,
            "FE2O3_EVIDENCE_ROW": job.row,
            "FE2O3_EVIDENCE_TARGET": queue.target,
        }
    )
    sleep_path = executor_map(queue).get("sleep")
    if sleep_path is not None:
        environment["FE2O3_EVIDENCE_SLEEP"] = sleep_path
    return sorted(environment.items())


def build_result_payload(queue: QueueRecord, job: QueueJob, root: Path) -> bytes:
    lines = [
        "signed_result_schema_version\t3",
        f"result_id\t{job.result_id}",
        f"row_id\t{job.row}",
        f"from_status\t{job.from_status}",
        f"to_status\t{job.to_status}",
        f"baseline_commit\t{queue.baseline}",
        f"source_commit\t{queue.source}",
        f"source_tree\t{queue.tree}",
        "evidence_class\thardware",
        f"target\t{queue.target}",
        f"hardware_lane\t{queue.lane}",
        f"execution_mode\t{queue.mode}",
        f"execution_closure\t{queue.execution_closure}",
        f"executor_count\t{len(queue.executors)}",
    ]
    for index, (label, path, size, digest) in enumerate(queue.executors):
        lines.append(f"executor\t{index:04d}\t{label}\t{path}\t{size}\t{digest}")
    environment = queue_environment(queue, job)
    lines.append(f"environment_count\t{len(environment)}")
    for index, (name, value) in enumerate(environment):
        lines.append(f"environment\t{index:04d}\t{name}\t{value.encode('ascii').hex()}")
    lines.extend(
        [
            f"queue_manifest_path\t{queue.relative_path}",
            f"queue_manifest_sha256\t{queue.digest}",
            f"queue_id\t{queue.queue_id}",
            f"timeout_seconds\t{job.timeout}",
            f"toolchain_count\t{len(queue.toolchains)}",
        ]
    )
    for index, (label, path, size, digest) in enumerate(queue.toolchains):
        lines.append(f"toolchain\t{index:04d}\t{label}\t{path}\t{size}\t{digest}")
    invocation = queue_invocation(queue, job)
    lines.extend(
        [
            "command_count\t1",
            f"command\t0000\t{command_hex(invocation)}\t0",
        ]
    )
    log = archive_path(root, job.log_path)
    lines.append(f"log\t0000\t{job.log_path}\t{log.stat().st_size}\t{sha256_file(log)}")
    lines.append(f"artifact_count\t{len(job.artifacts)}")
    for index, (label, relative) in enumerate(job.artifacts):
        artifact = archive_path(root, relative)
        lines.append(
            f"artifact\t{index:04d}\t{label}\t{relative}\t"
            f"{artifact.stat().st_size}\t{sha256_file(artifact)}"
        )
    return ("\n".join(lines) + "\n").encode("ascii")


def run_queue(args: argparse.Namespace) -> None:
    lock = acquire_lock(lock_path_for(args))
    try:
        repo = args.repo.resolve(strict=True)
        root = resolve_real_directory(args.archive_root, "evidence archive root")
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        queue = parse_queue(
            repo,
            root,
            args.manifest,
            trust,
            enforce_execution_root=True,
        )
        if args.test_mode != (queue.mode == "test"):
            fail("queue execution mode does not match lock mode")
        if queue.mode == "production" and trust.domain != "production":
            fail("production queue requires a production trust domain")
        head = run_git(repo, "rev-parse", "HEAD")
        if head != queue.source or run_git(repo, "rev-parse", "HEAD^{tree}") != queue.tree:
            fail("queue checkout does not exactly match signed source tree")
        if run_git(repo, "symbolic-ref", "-q", "HEAD", check=False):
            fail("queue checkout must be detached")
        if run_git(repo, "status", "--porcelain=v1", "--untracked-files=all"):
            fail("queue checkout must be clean")
        for job in queue.jobs:
            for relative in [job.result_path, job.log_path, *(p for _, p in job.artifacts)]:
                output = archive_path(root, relative, must_exist=False)
                if output.exists() or output.is_symlink():
                    fail(f"queue output already exists: {relative}")
        for job in queue.jobs:
            log = archive_path(root, job.log_path, must_exist=False)
            log.parent.mkdir(parents=True, exist_ok=True)
            for _, relative in job.artifacts:
                archive_path(root, relative, must_exist=False).parent.mkdir(
                    parents=True, exist_ok=True
                )
            for _, path, size, digest in queue.executors:
                verify_executor(path, str(size), digest)
            environment = dict(queue_environment(queue, job))
            invocation = queue_invocation(queue, job)
            with log.open("xb") as stream:
                try:
                    process = subprocess.run(
                        invocation,
                        cwd=repo,
                        env=environment,
                        stdout=stream,
                        stderr=subprocess.STDOUT,
                        timeout=job.timeout + 10,
                        check=False,
                    )
                except subprocess.TimeoutExpired:
                    fail(f"queue job timed out: {job.job_id}")
            if process.returncode != 0:
                fail(f"queue job failed: {job.job_id} ({process.returncode})")
            for _, relative in job.artifacts:
                archive_path(root, relative)
            unsigned = archive_path(
                root, f"work/{job.job_id}.unsigned.tsv", must_exist=False
            )
            unsigned.parent.mkdir(parents=True, exist_ok=True)
            unsigned.write_bytes(build_result_payload(queue, job, root))
            output = archive_path(root, job.result_path, must_exist=False)
            sign_payload(
                unsigned,
                output,
                args.signing_key,
                args.key_id,
                domain=queue.mode,
                role="attestor",
                repo=repo,
                test_mode=args.test_mode,
            )
            unsigned.unlink()
            parse_result(repo, root, job.result_path, trust)
        print(f"MI300X signed evidence queue passed: {len(queue.jobs)} serialized job(s)")
    finally:
        os.close(lock)


def common_trust(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--archive-root", type=Path, required=True)
    parser.add_argument("--trusted-root", type=Path, required=True)
    parser.add_argument("--trust-policy", type=Path, required=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    signer = subparsers.add_parser("sign")
    signer.add_argument("--repo", type=Path)
    signer.add_argument("--private-key", type=Path, required=True)
    signer.add_argument("--key-id", required=True)
    signer.add_argument("--domain", choices=("production", "test"), required=True)
    signer.add_argument("--role", required=True)
    signer.add_argument("--test-mode", action="store_true")
    signer.add_argument("input", type=Path)
    signer.add_argument("output", type=Path)

    trust_update = subparsers.add_parser("check-trust-update")
    trust_update.add_argument("--protected-root", type=Path, required=True)
    trust_update.add_argument("--protected-policy", type=Path, required=True)
    trust_update.add_argument("--candidate-root", type=Path, required=True)
    trust_update.add_argument("--candidate-policy", type=Path, required=True)
    trust_update.add_argument("--protected-row-policy", type=Path, required=True)
    trust_update.add_argument("--candidate-row-policy", type=Path, required=True)

    bootstrap = subparsers.add_parser("bootstrap-production-trust")
    bootstrap.add_argument("--output-root", type=Path, required=True)
    bootstrap.add_argument("--attestor-public-key", type=Path, required=True)
    bootstrap.add_argument("--attestor-key-id", required=True)
    bootstrap.add_argument("--reviewer-public-key", type=Path, required=True)
    bootstrap.add_argument("--reviewer-key-id", required=True)

    validate_trust = subparsers.add_parser("validate-production-trust")
    validate_trust.add_argument("--trusted-root", type=Path, required=True)
    validate_trust.add_argument("--trust-policy", type=Path, required=True)

    ingest = subparsers.add_parser("ingest-archive")
    ingest.add_argument("--repo", type=Path, required=True)
    ingest.add_argument("--source-root", type=Path, required=True)
    ingest.add_argument("--destination-root", type=Path, required=True)
    ingest.add_argument("--trusted-root", type=Path, required=True)
    ingest.add_argument("--trust-policy", type=Path, required=True)
    ingest.add_argument("--manifest", required=True)
    ingest.add_argument("--expected-manifest-sha256", required=True)
    ingest.add_argument("--expected-baseline", required=True)
    ingest.add_argument("--expected-source", required=True)
    ingest.add_argument("--expected-tree", required=True)
    ingest.add_argument("--expected-target", required=True)
    ingest.add_argument("--expected-lane", required=True)
    ingest.add_argument("--allow-test-fixtures", action="store_true")

    validate_archive = subparsers.add_parser("validate-archive")
    common_trust(validate_archive)
    validate_archive.add_argument("--manifest", required=True)
    validate_archive.add_argument("--allow-test-fixtures", action="store_true")

    protected_base = subparsers.add_parser("check-protected-base")
    protected_base.add_argument("--protected-repo", type=Path, required=True)
    protected_base.add_argument("--candidate-repo", type=Path, required=True)
    protected_base.add_argument("--protected-base", required=True)
    protected_base.add_argument("--default-tip", required=True)
    protected_base.add_argument("--candidate-head", required=True)

    result = subparsers.add_parser("validate-result")
    common_trust(result)
    result.add_argument("record")

    queue = subparsers.add_parser("validate-queue")
    common_trust(queue)
    queue.add_argument("manifest")

    manifest = subparsers.add_parser("validate-manifest")
    common_trust(manifest)
    manifest.add_argument("manifest")

    shard = subparsers.add_parser("validate-shard")
    common_trust(shard)
    shard.add_argument("--manifest", required=True)
    shard.add_argument("--row", action="append", required=True)

    gate_parser = subparsers.add_parser("gate")
    common_trust(gate_parser)
    gate_parser.add_argument("--manifest", required=True)
    gate_parser.add_argument("--trusted-policy", type=Path, required=True)
    gate_parser.add_argument("--candidate-policy", type=Path, required=True)
    gate_parser.add_argument("--baseline-status", type=Path, required=True)
    gate_parser.add_argument("--candidate-status", type=Path, required=True)
    gate_parser.add_argument("--allow-test-fixtures", action="store_true")

    queue_run = subparsers.add_parser("queue-run")
    common_trust(queue_run)
    queue_run.add_argument("--manifest", required=True)
    queue_run.add_argument("--signing-key", type=Path, required=True)
    queue_run.add_argument("--key-id", required=True)
    queue_run.add_argument("--test-mode", action="store_true")
    queue_run.add_argument("--lock-root", type=Path)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    if args.command == "sign":
        sign_payload(
            args.input,
            args.output,
            args.private_key,
            args.key_id,
            domain=args.domain,
            role=args.role,
            repo=args.repo.resolve(strict=True) if args.repo else None,
            test_mode=args.test_mode,
        )
    elif args.command == "check-trust-update":
        check_trust_update(args)
    elif args.command == "bootstrap-production-trust":
        bootstrap_production_trust(args)
    elif args.command == "validate-production-trust":
        trust = validate_production_trust(args.trusted_root, args.trust_policy)
        print(f"production trust is valid: {len(trust.keys)} separated public keys")
    elif args.command == "ingest-archive":
        ingest_archive(args)
    elif args.command == "validate-archive":
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        if not args.allow_test_fixtures:
            if trust.domain != "production":
                fail("production archive validation requires a production trust domain")
            trust = validate_production_trust(args.trusted_root, args.trust_policy)
        with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
            manifest = parse_manifest(
                args.repo.resolve(strict=True), root, args.manifest, trust
            )
            verify_archive_index(
                args.repo.resolve(strict=True), root, args.manifest, manifest, trust
            )
        print(f"signed evidence archive is closed: {len(manifest.results)} result(s)")
    elif args.command == "check-protected-base":
        check_protected_base(args)
    elif args.command == "validate-result":
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
            result = parse_result(
                args.repo.resolve(strict=True), root, args.record, trust
            )
        print(f"signed result is valid: {result.result_id}")
    elif args.command == "validate-queue":
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
            queue = parse_queue(
                args.repo.resolve(strict=True),
                root,
                args.manifest,
                trust,
                enforce_execution_root=True,
            )
        print(f"signed MI300X queue is valid: {len(queue.jobs)} job(s)")
    elif args.command == "validate-manifest":
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
            manifest = parse_manifest(
                args.repo.resolve(strict=True), root, args.manifest, trust
            )
        print(f"signed promotion manifest is valid: {len(manifest.results)} result(s)")
    elif args.command == "validate-shard":
        trust = parse_trust_policy(args.trusted_root, args.trust_policy)
        with ArchiveSnapshot(args.archive_root, require_immutable=False) as root:
            manifest = parse_manifest(
                args.repo.resolve(strict=True), root, args.manifest, trust
            )
        requested = args.row
        if len(requested) != len(set(requested)) or any(
            not valid_row(row) for row in requested
        ):
            fail("duplicate or invalid shard row")
        actual = {result.row for result in manifest.results}
        if actual != set(requested):
            fail("shard manifest rows do not exactly match requested rows")
        print(f"signed evidence shard is valid: {len(actual)} row(s)")
    elif args.command == "gate":
        gate(args)
    elif args.command == "queue-run":
        run_queue(args)
    else:
        fail("unknown command")


if __name__ == "__main__":
    try:
        main()
    except EvidenceError as error:
        print(f"parity signed evidence: {error}", file=sys.stderr)
        raise SystemExit(2)
