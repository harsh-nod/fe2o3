#!/usr/bin/env python3
"""Protected-policy OCI executor for promotable MI300X evidence.

The candidate supplies an execution request.  A protected policy selects the
only admissible executor profile; the request cannot assert its own closure
state or substitute image, runtime, device, or isolation settings.
"""

from __future__ import annotations

import argparse
import configparser
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import re
import secrets
import signal
import stat
import struct
import subprocess
import sys
import threading
import time
import zlib
from dataclasses import dataclass
from typing import BinaryIO


MAX_FILE_BYTES = 1024 * 1024
MAX_ITEMS = 256
MAX_JSON_DEPTH = 32
MAX_JSON_NODES = 65536
MAX_JSON_STRING_BYTES = 1024 * 1024
MAX_OCI_METADATA_BYTES = 4 * 1024 * 1024
MAX_OCI_LAYER_BYTES = 64 * 1024**3
MAX_OCI_IMAGE_BYTES = 256 * 1024**3
MAX_STAGING_ROOT_ENTRIES = 64
MAX_SOURCE_DIRECTORIES = 16384
MAX_GIT_ROOT_ENTRIES = 258
MAX_GIT_CONFIG_BYTES = 1024 * 1024
MAX_CLEANUP_ENTRIES = 40000
PROCESS_REAP_GRACE_SECONDS = 5.0
PROCESS_PIPE_JOIN_SECONDS = 5.0
PROCESS_FINAL_JOIN_SECONDS = 1.0
FS_IOC_GETFLAGS = 0x80086601
FS_IMMUTABLE_FL = 0x00000010
OPERATOR_CONFIG_DIRECTORY = Path("/etc/fe2o3/oci-executor")
OPERATOR_CONFIG_NAME = "operator-v1.tsv"
OPERATOR_CONFIG_DIGEST_NAME = "operator-v1.sha256"
OPERATOR_LAUNCHER_PATH = Path("/usr/libexec/fe2o3-oci-operator")
OPERATOR_PYTHON_ROOT = Path("/usr/libexec/fe2o3-python")
OPERATOR_INTERPRETER_PATH = OPERATOR_PYTHON_ROOT / "bin/python3"
OPERATOR_EXECUTOR_PATH = Path("/usr/libexec/fe2o3-oci-executor.py")
OPERATOR_ENVIRONMENT = {
    "HOME": "/nonexistent",
    "LC_ALL": "C",
    "PATH": "/usr/bin:/bin",
    "TZ": "UTC",
}
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9._-]{0,63}$")
RESULT_ID_RE = re.compile(r"^[0-9a-f]{64}$")
RELATIVE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/+:-]{0,511}$")
ABSOLUTE_RE = re.compile(r"^/[A-Za-z0-9][A-Za-z0-9._/+:-]{0,511}$")
OCI_DIGEST_RE = re.compile(r"^sha256:([0-9a-f]{64})$")
IMAGE_REFERENCE_RE = re.compile(
    r"^[a-z0-9]+(?:[._-][a-z0-9]+)*(?::[0-9]+)?"
    r"(?:/[a-z0-9]+(?:[._-][a-z0-9]+)*)+@sha256:[0-9a-f]{64}$"
)
ENV_NAME_RE = re.compile(r"^[A-Z][A-Z0-9_]{0,63}$")
RENDER_RE = re.compile(r"^/dev/dri/renderD[0-9]+$")


class ExecutorError(Exception):
    pass


def fail(message: str) -> None:
    raise ExecutorError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        fail(f"cannot hash {path}: {error}")
    return digest.hexdigest()


@dataclass(frozen=True)
class ProcessOutput:
    stdout: bytes
    stderr: bytes
    returncode: int


def poll_process_exit(process: subprocess.Popen[bytes], timeout_seconds: float) -> bool:
    deadline = time.monotonic() + timeout_seconds
    while process.poll() is None:
        if time.monotonic() >= deadline:
            return False
        time.sleep(0.01)
    return True


def join_threads_bound(threads: list[threading.Thread], timeout_seconds: float) -> None:
    deadline = time.monotonic() + timeout_seconds
    for thread in threads:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return
        thread.join(timeout=remaining)


def run_bounded(
    arguments: list[str],
    *,
    label: str,
    environment: dict[str, str],
    timeout_seconds: int,
    stdout_limit: int,
    stderr_limit: int,
    input_data: bytes | None = None,
) -> ProcessOutput:
    if not arguments or timeout_seconds < 1 or stdout_limit < 0 or stderr_limit < 0:
        fail(f"invalid bounded subprocess contract for {label}")
    if input_data is not None and len(input_data) > MAX_FILE_BYTES:
        fail(f"bounded subprocess input exceeds limit for {label}")
    try:
        process = subprocess.Popen(
            arguments,
            stdin=subprocess.PIPE if input_data is not None else subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=environment,
            close_fds=True,
            start_new_session=True,
        )
    except OSError as error:
        fail(f"cannot start {label}: {error}")
    assert process.stdout is not None
    assert process.stderr is not None
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    overflow: list[str] = []
    lock = threading.Lock()

    def read_stream(name: str, stream: BinaryIO, limit: int) -> None:
        while True:
            chunk = stream.read(65536)
            if not chunk:
                return
            with lock:
                if len(buffers[name]) + len(chunk) > limit:
                    overflow.append(name)
                    return
                buffers[name].extend(chunk)

    readers = [
        threading.Thread(
            target=read_stream,
            args=("stdout", process.stdout, stdout_limit),
            daemon=True,
        ),
        threading.Thread(
            target=read_stream,
            args=("stderr", process.stderr, stderr_limit),
            daemon=True,
        ),
    ]
    for reader in readers:
        reader.start()

    writer: threading.Thread | None = None
    if input_data is not None:
        assert process.stdin is not None

        def write_input() -> None:
            try:
                write_all(process.stdin.fileno(), input_data)
                process.stdin.close()
            except (BrokenPipeError, OSError):
                pass

        writer = threading.Thread(target=write_input, daemon=True)
        writer.start()

    deadline = time.monotonic() + timeout_seconds
    timed_out = False
    while process.poll() is None:
        if overflow or time.monotonic() >= deadline:
            timed_out = not overflow
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            break
        time.sleep(0.01)
    reaped = process.returncode is not None
    if not reaped:
        reaped = poll_process_exit(process, PROCESS_REAP_GRACE_SECONDS)
    if not reaped:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        reaped = poll_process_exit(process, PROCESS_REAP_GRACE_SECONDS)
    threads = [*readers, *([writer] if writer is not None else [])]
    join_threads_bound(threads, PROCESS_PIPE_JOIN_SECONDS)
    if any(thread.is_alive() for thread in threads):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        if not reaped:
            reaped = poll_process_exit(process, PROCESS_FINAL_JOIN_SECONDS)
        join_threads_bound(threads, PROCESS_FINAL_JOIN_SECONDS)
        fail(f"bounded subprocess pipe did not close for {label}")
    if not reaped or process.returncode is None:
        fail(
            f"{label} did not become waitable after bounded SIGKILL grace; "
            "reap is deferred"
        )
    if overflow:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        fail(f"{label} {overflow[0]} exceeds protected limit")
    if timed_out:
        fail(f"{label} exceeded protected timeout")
    return ProcessOutput(
        bytes(buffers["stdout"]), bytes(buffers["stderr"]), process.returncode
    )


def valid_relative(value: str) -> bool:
    return bool(
        RELATIVE_RE.fullmatch(value)
        and not value.startswith("/")
        and all(part not in ("", ".", "..") for part in value.split("/"))
    )


def valid_absolute(value: str) -> bool:
    return bool(
        ABSOLUTE_RE.fullmatch(value)
        and all(part not in ("", ".", "..") for part in value.split("/")[1:])
    )


def parse_rows(raw: bytes, label: str) -> list[list[str]]:
    if not raw or len(raw) > MAX_FILE_BYTES:
        fail(f"invalid {label} size")
    if b"\r" in raw or not raw.endswith(b"\n"):
        fail(f"non-canonical {label} line endings")
    rows: list[list[str]] = []
    for number, raw_line in enumerate(raw.splitlines(), 1):
        if not raw_line:
            fail(f"blank {label} line {number}")
        try:
            rows.append(raw_line.decode("ascii").split("\t"))
        except UnicodeDecodeError:
            fail(f"non-ASCII {label} line {number}")
    return rows


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
            fail(f"{self.label}: unexpected trailing field")


def parse_count(value: str, label: str, *, allow_zero: bool = False) -> int:
    if not re.fullmatch(r"0|[1-9][0-9]*", value):
        fail(f"invalid {label} count")
    result = int(value)
    if result > MAX_ITEMS or (result == 0 and not allow_zero):
        fail(f"invalid {label} count")
    return result


@dataclass(frozen=True)
class TrustAnchor:
    identity: str
    policy_size: int
    policy_digest: str
    owner_uid: int
    owner_gid: int
    file_contract: str


@dataclass(frozen=True)
class OperatorConfig:
    config_id: str
    trusted_root: str
    policy_path: str
    policy_identity: str
    policy_size: int
    policy_digest: str
    trusted_owner_uid: int
    trusted_owner_gid: int
    trust_file_contract: str
    inbox_root: str
    inbox_owner_uid: int
    inbox_owner_gid: int
    request_owner_uid: int
    request_owner_gid: int
    queue_trust_digest: str
    config_digest: str


def close_descriptors(descriptors: tuple[tuple[int, str], ...]) -> None:
    primary = sys.exception()
    failures: list[str] = []
    for file_fd, label in descriptors:
        if file_fd < 0:
            continue
        try:
            os.close(file_fd)
        except OSError as close_error:
            failures.append(f"cannot close {label}: {close_error}")
    if failures:
        detail = "; ".join(failures)
        if primary is not None:
            raise ExecutorError(f"{primary}; additionally {detail}") from primary
        fail(detail)


def close_descriptor(file_fd: int, label: str = "file descriptor") -> None:
    close_descriptors(((file_fd, label),))


def normalized_error(error: BaseException, context: str) -> ExecutorError:
    if isinstance(error, ExecutorError):
        return error
    return ExecutorError(f"{context}: {error}")


def append_error(primary: ExecutorError | None, error: BaseException) -> ExecutorError:
    secondary = str(error)
    if primary is None:
        return normalized_error(error, "filesystem operation failed")
    if secondary.startswith(str(primary)):
        return normalized_error(error, "filesystem operation failed")
    return ExecutorError(f"{primary}; additionally {secondary}")


def resolve_path(path: Path, label: str) -> Path:
    try:
        return path.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve {label}: {error}")


@dataclass
class TrustedRoot:
    path: Path
    fd: int
    owner_uid: int
    owner_gid: int

    def close(self) -> None:
        file_fd = self.fd
        self.fd = -1
        close_descriptor(file_fd, "protected root")


def stable_file_identity(info: os.stat_result) -> tuple[int, ...]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_uid,
        info.st_gid,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def verify_trusted_metadata(
    info: os.stat_result,
    anchor: TrustAnchor,
    label: str,
    *,
    directory: bool,
) -> None:
    expected_kind = (
        stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    )
    if (
        not expected_kind
        or (info.st_uid, info.st_gid) != (anchor.owner_uid, anchor.owner_gid)
        or info.st_mode & 0o022
        or not directory
        and info.st_nlink != 1
    ):
        fail(
            f"{label} ownership, mode, type, or link contract is unsafe "
            f"(uid={info.st_uid}, gid={info.st_gid}, "
            f"mode={stat.S_IMODE(info.st_mode):04o}, links={info.st_nlink}, "
            f"expected={anchor.owner_uid}:{anchor.owner_gid})"
        )


def verify_descriptor_immutable(file_fd: int, label: str, required: bool) -> None:
    if not required:
        return
    try:
        packed = fcntl.ioctl(file_fd, FS_IOC_GETFLAGS, struct.pack("=I", 0))
        flags = struct.unpack("=I", packed)[0]
    except (OSError, struct.error) as error:
        fail(f"cannot establish immutable flag for {label}: {error}")
    if not flags & FS_IMMUTABLE_FL:
        fail(f"{label} does not satisfy the external immutable-file contract")


def verify_immutable_flag(file_fd: int, anchor: TrustAnchor, label: str) -> None:
    if anchor.file_contract not in ("descriptor-stable", "linux-immutable"):
        fail("invalid external trust-anchor file contract")
    verify_descriptor_immutable(
        file_fd, label, anchor.file_contract == "linux-immutable"
    )


def open_owned_directory_tree(
    path: Path, expected_uid: int, expected_gid: int, label: str
) -> int:
    if not path.is_absolute() or not valid_absolute(str(path)):
        fail(f"{label} path must be canonical absolute")
    current_fd = -1
    try:
        current_fd = os.open(
            "/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
        )
        for component in path.parts[1:]:
            next_fd = os.open(
                component,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=current_fd,
            )
            close_descriptor(current_fd, f"{label} parent directory")
            current_fd = next_fd
            info = os.fstat(current_fd)
            if (
                not stat.S_ISDIR(info.st_mode)
                or (info.st_uid, info.st_gid) != (expected_uid, expected_gid)
                or info.st_mode & 0o022
            ):
                fail(f"{label} directory ownership or mode is unsafe")
        return current_fd
    except ExecutorError:
        if current_fd >= 0:
            close_descriptor(current_fd, f"{label} directory after validation failure")
        raise
    except OSError as error:
        if current_fd >= 0:
            close_descriptor(current_fd, f"{label} directory after open failure")
        fail(f"cannot open fixed {label} directory: {error}")


def read_owned_descriptor_file(
    directory_fd: int,
    name: str,
    label: str,
    *,
    maximum_bytes: int,
    expected_uid: int,
    expected_gid: int,
    require_immutable: bool,
    expected_digest: str | None = None,
) -> bytes:
    if "/" in name or not name or maximum_bytes < 1:
        fail(f"invalid fixed {label} name or bound")
    file_fd = -1
    try:
        file_fd = os.open(
            name,
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=directory_fd,
        )
        before = os.fstat(file_fd)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or (before.st_uid, before.st_gid) != (expected_uid, expected_gid)
            or before.st_mode & 0o022
            or not 1 <= before.st_size <= maximum_bytes
        ):
            fail(f"fixed {label} metadata contract is unsafe")
        verify_descriptor_immutable(file_fd, f"fixed {label}", require_immutable)
        raw = read_descriptor_bound(file_fd, maximum_bytes, f"fixed {label}")
        after = os.fstat(file_fd)
        if (
            stable_file_identity(before) != stable_file_identity(after)
            or len(raw) != before.st_size
            or expected_digest is not None
            and sha256_bytes(raw) != expected_digest
        ):
            fail(f"fixed {label} changed or differs from its provisioned digest")
        return raw
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot open fixed {label}: {error}")
    finally:
        if file_fd >= 0:
            close_descriptor(file_fd, f"fixed {label}")


def load_operator_config(
    directory: Path = OPERATOR_CONFIG_DIRECTORY,
    *,
    provision_uid: int = 0,
    provision_gid: int = 0,
    require_immutable: bool = True,
) -> OperatorConfig:
    if require_immutable:
        directory_fd = open_owned_directory_tree(
            directory, provision_uid, provision_gid, "operator configuration"
        )
    else:
        directory_fd = -1
        try:
            directory_fd = os.open(
                directory,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            )
            info = os.fstat(directory_fd)
        except OSError as error:
            failure = normalized_error(
                error, "cannot open test operator configuration directory"
            )
            try:
                close_descriptor(
                    directory_fd, "unavailable test operator configuration directory"
                )
            except ExecutorError as close_error:
                failure = append_error(failure, close_error)
            raise failure
        if (
            not stat.S_ISDIR(info.st_mode)
            or (info.st_uid, info.st_gid) != (provision_uid, provision_gid)
            or info.st_mode & 0o022
        ):
            close_descriptor(
                directory_fd, "unsafe test operator configuration directory"
            )
            fail("test operator configuration directory ownership or mode is unsafe")
    try:
        digest_raw = read_owned_descriptor_file(
            directory_fd,
            OPERATOR_CONFIG_DIGEST_NAME,
            "operator configuration digest provision",
            maximum_bytes=65,
            expected_uid=provision_uid,
            expected_gid=provision_gid,
            require_immutable=require_immutable,
        )
        if (
            len(digest_raw) != 65
            or not digest_raw.endswith(b"\n")
            or SHA256_RE.fullmatch(digest_raw[:-1].decode("ascii", errors="ignore"))
            is None
        ):
            fail("fixed operator configuration digest provision is malformed")
        config_digest = digest_raw[:-1].decode("ascii")
        raw = read_owned_descriptor_file(
            directory_fd,
            OPERATOR_CONFIG_NAME,
            "operator configuration",
            maximum_bytes=MAX_FILE_BYTES,
            expected_uid=provision_uid,
            expected_gid=provision_gid,
            require_immutable=require_immutable,
            expected_digest=config_digest,
        )
    finally:
        close_descriptor(directory_fd, "operator configuration directory")
    cursor = Cursor(parse_rows(raw, "operator configuration"), "operator configuration")
    if cursor.scalar("oci_operator_config_schema_version") != "1":
        fail("operator configuration schema must be 1")
    config_id = cursor.scalar("config_id")
    trusted_root = cursor.scalar("trusted_root")
    policy_path = cursor.scalar("policy_path")
    policy_identity = cursor.scalar("policy_identity")
    policy_size = cursor.scalar("policy_size")
    policy_digest = cursor.scalar("policy_sha256")
    trusted_uid = cursor.scalar("trusted_owner_uid")
    trusted_gid = cursor.scalar("trusted_owner_gid")
    file_contract = cursor.scalar("trust_file_contract")
    inbox_root = cursor.scalar("inbox_root")
    inbox_uid = cursor.scalar("inbox_owner_uid")
    inbox_gid = cursor.scalar("inbox_owner_gid")
    request_uid = cursor.scalar("request_owner_uid")
    request_gid = cursor.scalar("request_owner_gid")
    queue_digest = cursor.scalar("queue_trust_sha256")
    cursor.done()
    numeric = (
        policy_size,
        trusted_uid,
        trusted_gid,
        inbox_uid,
        inbox_gid,
        request_uid,
        request_gid,
    )
    if (
        not ID_RE.fullmatch(config_id)
        or not valid_absolute(trusted_root)
        or not valid_relative(policy_path)
        or not ID_RE.fullmatch(policy_identity)
        or any(not value.isdigit() or int(value) > 2**31 - 1 for value in numeric)
        or not 1 <= int(policy_size) <= MAX_FILE_BYTES
        or not SHA256_RE.fullmatch(policy_digest)
        or trusted_uid != "0"
        or trusted_gid != "0"
        or file_contract != "linux-immutable"
        or not valid_absolute(inbox_root)
        or inbox_uid != "0"
        or inbox_gid != "0"
        or not SHA256_RE.fullmatch(queue_digest)
    ):
        fail("operator configuration contains an invalid production binding")
    return OperatorConfig(
        config_id,
        trusted_root,
        policy_path,
        policy_identity,
        int(policy_size),
        policy_digest,
        int(trusted_uid),
        int(trusted_gid),
        file_contract,
        inbox_root,
        int(inbox_uid),
        int(inbox_gid),
        int(request_uid),
        int(request_gid),
        queue_digest,
        config_digest,
    )


def open_trusted_root(path: Path, anchor: TrustAnchor) -> TrustedRoot:
    if not path.is_absolute() or not valid_absolute(str(path)):
        fail("protected root path must be canonical absolute")
    try:
        root_fd = os.open(
            path,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        info = os.fstat(root_fd)
        verify_trusted_metadata(info, anchor, "protected root", directory=True)
    except ExecutorError:
        if "root_fd" in locals():
            close_descriptor(root_fd, "invalid protected root")
        raise
    except OSError as error:
        if "root_fd" in locals():
            close_descriptor(root_fd, "unavailable protected root")
        fail(f"cannot open protected root without following links: {error}")
    return TrustedRoot(path, root_fd, anchor.owner_uid, anchor.owner_gid)


def read_descriptor_bound(file_fd: int, maximum: int, label: str) -> bytes:
    output = bytearray()
    try:
        while len(output) <= maximum:
            chunk = os.read(file_fd, min(65536, maximum + 1 - len(output)))
            if not chunk:
                break
            output.extend(chunk)
    except OSError as error:
        fail(f"cannot read {label}: {error}")
    if not output or len(output) > maximum:
        fail(f"invalid {label} size")
    return bytes(output)


def read_trusted_file(
    root: TrustedRoot,
    relative: str,
    anchor: TrustAnchor,
    *,
    expected_size: int,
    expected_digest: str,
    label: str,
) -> bytes:
    if (
        not valid_relative(relative)
        or not 1 <= expected_size <= MAX_FILE_BYTES
        or not SHA256_RE.fullmatch(expected_digest)
    ):
        fail(f"malformed {label} external binding")
    try:
        current_fd = os.dup(root.fd)
    except OSError as error:
        fail(f"cannot retain protected root for {label}: {error}")
    file_fd = -1
    try:
        components = relative.split("/")
        for component in components[:-1]:
            next_fd = os.open(
                component,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=current_fd,
            )
            close_descriptor(current_fd, f"protected {label} parent")
            current_fd = next_fd
            verify_trusted_metadata(
                os.fstat(current_fd), anchor, f"{label} parent", directory=True
            )
        file_fd = os.open(
            components[-1],
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=current_fd,
        )
        before = os.fstat(file_fd)
        verify_trusted_metadata(before, anchor, label, directory=False)
        verify_immutable_flag(file_fd, anchor, label)
        if before.st_size != expected_size:
            fail(f"{label} size differs from its external binding")
        raw = read_descriptor_bound(file_fd, MAX_FILE_BYTES, label)
        after = os.fstat(file_fd)
        if (
            stable_file_identity(before) != stable_file_identity(after)
            or len(raw) != expected_size
            or sha256_bytes(raw) != expected_digest
        ):
            fail(f"{label} changed or differs from its external binding")
        return raw
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot open protected {label} without following links: {error}")
    finally:
        close_descriptors(
            (
                (file_fd, f"protected {label}"),
                (current_fd, f"protected {label} parent"),
            )
        )


def protected_path(root: Path, relative: str, label: str) -> Path:
    if not valid_relative(relative):
        fail(f"invalid protected {label} path")
    root = resolve_path(root, "protected root")
    current = root
    for index, component in enumerate(relative.split("/")):
        current = current / component
        try:
            info = current.lstat()
        except OSError:
            fail(f"protected {label} is missing")
        if stat.S_ISLNK(info.st_mode):
            fail(f"protected {label} path contains a symlink")
        if index + 1 < len(relative.split("/")):
            if not stat.S_ISDIR(info.st_mode):
                fail(f"protected {label} parent is not a directory")
        elif not stat.S_ISREG(info.st_mode):
            fail(f"protected {label} is not a regular file")
    if root not in resolve_path(current, f"protected {label}").parents:
        fail(f"protected {label} escapes protected root")
    return current


def verify_regular(path: Path, size: str, digest: str, label: str) -> None:
    if not size.isdigit() or not SHA256_RE.fullmatch(digest):
        fail(f"malformed {label} binding")
    try:
        info = path.lstat()
    except OSError:
        fail(f"{label} is missing")
    if not stat.S_ISREG(info.st_mode) or path.is_symlink() or info.st_nlink != 1:
        fail(f"{label} is not a single-link regular file")
    if info.st_size != int(size) or sha256_file(path) != digest:
        fail(f"{label} binding mismatch")


def verify_operator_owned(
    path: Path,
    profile: Profile,
    label: str,
    *,
    directory: bool,
    allow_root: bool = False,
) -> None:
    try:
        info = path.lstat()
    except OSError:
        fail(f"operator {label} is missing")
    expected_kind = (
        stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    )
    if (
        path.is_symlink()
        or not expected_kind
        or (info.st_uid, info.st_gid) != (profile.operator_uid, profile.operator_gid)
        and not (allow_root and (info.st_uid, info.st_gid) == (0, 0))
        or info.st_mode & 0o022
    ):
        fail(
            f"operator {label} ownership or mode is unsafe "
            f"(uid={info.st_uid}, gid={info.st_gid}, mode={stat.S_IMODE(info.st_mode):04o}, "
            f"expected={profile.operator_uid}:{profile.operator_gid})"
        )
    if profile.mode == "production":
        current = path.parent
        while current != current.parent:
            try:
                parent_info = current.lstat()
            except OSError:
                fail(f"operator {label} parent is unavailable")
            if (
                current.is_symlink()
                or not stat.S_ISDIR(parent_info.st_mode)
                or parent_info.st_uid != 0
                or parent_info.st_gid != 0
                or parent_info.st_mode & 0o022
            ):
                fail(f"operator {label} has a writable or unowned parent")
            current = current.parent


@dataclass(frozen=True)
class PolicyEntry:
    profile_id: str
    relative_path: str
    size: int
    digest: str


@dataclass(frozen=True)
class Policy:
    domain: str
    profiles: dict[str, PolicyEntry]
    digest: str


def parse_policy(root: TrustedRoot, relative: str, anchor: TrustAnchor) -> Policy:
    raw = read_trusted_file(
        root,
        relative,
        anchor,
        expected_size=anchor.policy_size,
        expected_digest=anchor.policy_digest,
        label="OCI executor policy",
    )
    rows = parse_rows(raw, "OCI executor policy")
    cursor = Cursor(rows, "OCI executor policy")
    if cursor.scalar("oci_executor_policy_schema_version") != "1":
        fail("OCI executor policy schema must be 1")
    domain = cursor.scalar("trust_domain")
    if domain not in ("production", "test"):
        fail("invalid OCI executor policy trust domain")
    if domain == "production" and anchor.file_contract != "linux-immutable":
        fail("production policy requires an external Linux immutable-file contract")
    count = parse_count(cursor.scalar("profile_count"), "profile")
    profiles: dict[str, PolicyEntry] = {}
    previous = ""
    for index in range(count):
        row = cursor.record("profile", 6, index)
        profile_id, profile_path, size, digest = row[2:]
        if (
            not ID_RE.fullmatch(profile_id)
            or profile_id <= previous
            or not valid_relative(profile_path)
            or not size.isdigit()
            or not 1 <= int(size) <= MAX_FILE_BYTES
            or not SHA256_RE.fullmatch(digest)
        ):
            fail("invalid or unsorted protected executor profile")
        if profile_id in profiles:
            fail("duplicate protected executor profile")
        profiles[profile_id] = PolicyEntry(profile_id, profile_path, int(size), digest)
        previous = profile_id
    cursor.done()
    return Policy(domain, profiles, anchor.policy_digest)


@dataclass(frozen=True)
class Layer:
    digest: str
    size: int


@dataclass(frozen=True)
class Device:
    path: str
    major: int
    minor: int
    access: str


@dataclass(frozen=True)
class Profile:
    profile_id: str
    mode: str
    target: str
    lane: str
    runtime_path: str
    runtime_size: int
    runtime_digest: str
    runtime_version_digest: str
    runtime_info_digest: str
    git_objects_path: str
    git_object_format: str
    git_object_limit: int
    git_object_bytes_limit: int
    git_tree_depth_limit: int
    source_staging_root: str
    output_staging_root: str
    artifact_stream_protocol: str
    source_file_limit: int
    source_byte_limit: int
    source_index_limit: int
    operator_uid: int
    operator_gid: int
    layout_path: str
    index_digest: str
    index_size: int
    image_reference: str
    manifest: Layer
    config: Layer
    layers: tuple[Layer, ...]
    entrypoint: tuple[str, ...]
    command: tuple[str, ...]
    environment: tuple[tuple[str, str], ...]
    source_mount: str
    request_mount: str
    output_mount: str
    tmp_mount: str
    output_limit: int
    tmp_limit: int
    shm_limit: int
    log_limit: int
    memory_limit: int
    pids_limit: int
    cpu_limit_milli: int
    uid: int
    gid: int
    supplemental_gid: int
    seccomp_path: str
    seccomp_size: int
    seccomp_digest: str
    devices: tuple[Device, ...]
    machine_id_digest: str
    kernel_release: str
    kernel_notes_digest: str
    driver_path: str
    driver_digest: str
    gpu_pci_slot: str
    gpu_pci_id: str
    gpu_unique_id: str
    profile_digest: str


def parse_vector(cursor: Cursor, key: str) -> tuple[str, ...]:
    count = parse_count(cursor.scalar(f"{key}_count"), key)
    result: list[str] = []
    for index in range(count):
        value = cursor.record(key, 3, index)[2]
        if not value or "\x00" in value or "\n" in value or "\r" in value:
            fail(f"invalid {key} value")
        result.append(value)
    return tuple(result)


def parse_environment(cursor: Cursor) -> tuple[tuple[str, str], ...]:
    count = parse_count(cursor.scalar("environment_count"), "environment")
    result: list[tuple[str, str]] = []
    previous = ""
    for index in range(count):
        name, encoded = cursor.record("environment", 4, index)[2:]
        if (
            not ENV_NAME_RE.fullmatch(name)
            or name <= previous
            or not re.fullmatch(r"(?:[0-9a-f]{2})*", encoded)
        ):
            fail("invalid or unsorted profile environment")
        try:
            value = bytes.fromhex(encoded).decode("ascii")
        except (ValueError, UnicodeDecodeError):
            fail("profile environment is not canonical ASCII")
        if any(character in value for character in ("\x00", "\r", "\n")):
            fail("profile environment contains a control character")
        result.append((name, value))
        previous = name
    values = dict(result)
    required = {
        "HIP_VISIBLE_DEVICES",
        "HOME",
        "HOSTNAME",
        "LC_ALL",
        "PATH",
        "ROCR_VISIBLE_DEVICES",
    }
    if not required <= values.keys() or values["HOSTNAME"] != "fe2o3-evidence":
        fail("profile environment lacks the clean GPU baseline")
    return tuple(result)


def parse_positive(cursor: Cursor, key: str, minimum: int, maximum: int) -> int:
    value = cursor.scalar(key)
    if not value.isdigit() or not minimum <= int(value) <= maximum:
        fail(f"invalid {key}")
    return int(value)


def load_profile(
    root: TrustedRoot, policy: Policy, profile_id: str, anchor: TrustAnchor
) -> Profile:
    entry = policy.profiles.get(profile_id)
    if entry is None:
        fail("executor profile is not authorized by protected policy")
    raw = read_trusted_file(
        root,
        entry.relative_path,
        anchor,
        expected_size=entry.size,
        expected_digest=entry.digest,
        label="OCI executor profile",
    )
    rows = parse_rows(raw, "OCI executor profile")
    cursor = Cursor(rows, "OCI executor profile")
    if cursor.scalar("oci_executor_profile_schema_version") != "2":
        fail("OCI executor profile schema must be 2")
    actual_id = cursor.scalar("profile_id")
    mode = cursor.scalar("execution_mode")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    runtime_path = cursor.scalar("runtime_path")
    runtime_size = cursor.scalar("runtime_size")
    runtime_digest = cursor.scalar("runtime_sha256")
    runtime_version_digest = cursor.scalar("runtime_version_sha256")
    runtime_info_digest = cursor.scalar("runtime_info_sha256")
    git_objects_path = cursor.scalar("git_objects_path")
    git_object_format = cursor.scalar("git_object_format")
    git_object_limit = cursor.scalar("git_object_limit")
    git_object_bytes_limit = cursor.scalar("git_object_bytes_limit")
    git_tree_depth_limit = cursor.scalar("git_tree_depth_limit")
    source_staging_root = cursor.scalar("source_staging_root")
    output_staging_root = cursor.scalar("output_staging_root")
    artifact_stream_protocol = cursor.scalar("artifact_stream_protocol")
    source_file_limit = cursor.scalar("source_file_limit")
    source_byte_limit = cursor.scalar("source_byte_limit")
    source_index_limit = cursor.scalar("source_index_limit")
    operator_uid = cursor.scalar("operator_uid")
    operator_gid = cursor.scalar("operator_gid")
    layout_path = cursor.scalar("oci_layout_path")
    index_digest = cursor.scalar("oci_index_sha256")
    index_size = cursor.scalar("oci_index_size")
    image_reference = cursor.scalar("image_reference")
    manifest_digest = cursor.scalar("image_manifest_digest")
    manifest_size = cursor.scalar("image_manifest_size")
    config_digest = cursor.scalar("image_config_digest")
    config_size = cursor.scalar("image_config_size")
    if actual_id != profile_id or not ID_RE.fullmatch(actual_id):
        fail("protected executor profile identity mismatch")
    if mode != policy.domain:
        fail("executor profile domain differs from protected policy")
    if target != "gfx942" or not re.fullmatch(r"mi300x[a-z0-9._-]{0,55}", lane):
        fail("executor profile target/lane mismatch")
    if not valid_absolute(runtime_path) or not valid_absolute(layout_path):
        fail("executor profile host paths must be canonical absolute paths")
    if (
        not runtime_size.isdigit()
        or not 1 <= int(runtime_size) <= 1024**3
        or not SHA256_RE.fullmatch(runtime_digest)
        or not SHA256_RE.fullmatch(runtime_version_digest)
        or not SHA256_RE.fullmatch(runtime_info_digest)
    ):
        fail("malformed runtime binding")
    if (
        not valid_absolute(git_objects_path)
        or git_object_format != "sha1-loose"
        or not git_object_limit.isdigit()
        or not 1 <= int(git_object_limit) <= 65536
        or not git_object_bytes_limit.isdigit()
        or not 1 <= int(git_object_bytes_limit) <= 1024**3
        or not git_tree_depth_limit.isdigit()
        or not 1 <= int(git_tree_depth_limit) <= 128
        or not valid_absolute(source_staging_root)
        or not valid_absolute(output_staging_root)
        or artifact_stream_protocol != "fe2o3-artifact-stream-v1"
        or not source_file_limit.isdigit()
        or not 1 <= int(source_file_limit) <= 16384
        or not source_byte_limit.isdigit()
        or not 1 <= int(source_byte_limit) <= 512 * 1024**2
        or not source_index_limit.isdigit()
        or not 1 <= int(source_index_limit) <= 64 * 1024**2
        or not operator_uid.isdigit()
        or not operator_gid.isdigit()
        or not 0 <= int(operator_uid) <= 2**31 - 1
        or not 0 <= int(operator_gid) <= 2**31 - 1
    ):
        fail("malformed immutable source export contract")
    if mode == "production" and (operator_uid != "0" or operator_gid != "0"):
        fail("production OCI executor paths must be owned by root")
    if (
        not SHA256_RE.fullmatch(index_digest)
        or not index_size.isdigit()
        or not 1 <= int(index_size) <= MAX_OCI_METADATA_BYTES
    ):
        fail("malformed OCI index binding")
    manifest_match = OCI_DIGEST_RE.fullmatch(manifest_digest)
    config_match = OCI_DIGEST_RE.fullmatch(config_digest)
    if (
        manifest_match is None
        or config_match is None
        or not manifest_size.isdigit()
        or not config_size.isdigit()
        or not 1 <= int(manifest_size) <= MAX_OCI_METADATA_BYTES
        or not 1 <= int(config_size) <= MAX_OCI_METADATA_BYTES
        or IMAGE_REFERENCE_RE.fullmatch(image_reference) is None
        or not image_reference.endswith("@" + manifest_digest)
    ):
        fail("malformed OCI image identity")
    layer_count = parse_count(cursor.scalar("image_layer_count"), "image layer")
    layers: list[Layer] = []
    layer_bytes = 0
    for index in range(layer_count):
        digest, size = cursor.record("image_layer", 4, index)[2:]
        if (
            OCI_DIGEST_RE.fullmatch(digest) is None
            or not size.isdigit()
            or not 1 <= int(size) <= MAX_OCI_LAYER_BYTES
        ):
            fail("malformed OCI layer binding")
        layers.append(Layer(digest, int(size)))
        layer_bytes += int(size)
        if layer_bytes > MAX_OCI_IMAGE_BYTES:
            fail("OCI image layers exceed protected limit")
    entrypoint = parse_vector(cursor, "entrypoint")
    command = parse_vector(cursor, "command")
    if len(entrypoint) != 1 or not valid_absolute(entrypoint[0]):
        fail("profile entrypoint must be one absolute executable")
    environment = parse_environment(cursor)
    source_mount = cursor.scalar("source_mount")
    request_mount = cursor.scalar("request_mount")
    output_mount = cursor.scalar("output_mount")
    tmp_mount = cursor.scalar("tmp_mount")
    mounts = (source_mount, request_mount, output_mount, tmp_mount)
    overlaps = any(
        left == right
        or left.startswith(right.rstrip("/") + "/")
        or right.startswith(left.rstrip("/") + "/")
        for index, left in enumerate(mounts)
        for right in mounts[index + 1 :]
    )
    if any(not valid_absolute(item) for item in mounts) or overlaps:
        fail("invalid or duplicate executor mount")
    output_limit = parse_positive(cursor, "output_limit_bytes", 1, 16 * 1024**3)
    tmp_limit = parse_positive(cursor, "tmp_limit_bytes", 1, 16 * 1024**3)
    shm_limit = parse_positive(cursor, "shm_limit_bytes", 1024**2, 16 * 1024**3)
    log_limit = parse_positive(cursor, "log_limit_bytes", 1, 1024**3)
    memory_limit = parse_positive(cursor, "memory_limit_bytes", 64 * 1024**2, 1024**4)
    pids_limit = parse_positive(cursor, "pids_limit", 1, 65536)
    cpu_limit_milli = parse_positive(cursor, "cpu_limit_milli", 1, 1024 * 1000)
    uid = parse_positive(cursor, "container_uid", 1, 2**31 - 1)
    gid = parse_positive(cursor, "container_gid", 1, 2**31 - 1)
    supplemental_gid = parse_positive(cursor, "supplemental_gid", 1, 2**31 - 1)
    if cursor.scalar("network_mode") != "none":
        fail("OCI executor network must be disabled")
    if cursor.scalar("read_only_root") != "true":
        fail("OCI executor root must be read-only")
    if cursor.scalar("cap_drop") != "ALL":
        fail("OCI executor must drop every capability")
    if cursor.scalar("no_new_privileges") != "true":
        fail("OCI executor must prevent new privileges")
    seccomp_path = cursor.scalar("seccomp_profile_path")
    seccomp_size = cursor.scalar("seccomp_profile_size")
    seccomp_digest = cursor.scalar("seccomp_profile_sha256")
    if (
        not valid_relative(seccomp_path)
        or not seccomp_size.isdigit()
        or not 1 <= int(seccomp_size) <= MAX_FILE_BYTES
        or not SHA256_RE.fullmatch(seccomp_digest)
    ):
        fail("malformed seccomp profile binding")
    device_count = parse_count(cursor.scalar("device_count"), "device")
    devices: list[Device] = []
    previous = ""
    for index in range(device_count):
        device_path, major, minor, access = cursor.record("device", 6, index)[2:]
        if (
            device_path <= previous
            or device_path not in ("/dev/kfd",)
            and RENDER_RE.fullmatch(device_path) is None
            or not major.isdigit()
            or not minor.isdigit()
            or int(major) > 2**20
            or int(minor) > 2**20
            or access != "rwm"
        ):
            fail("invalid or unsorted device policy")
        devices.append(Device(device_path, int(major), int(minor), access))
        previous = device_path
    if len(devices) != 2 or {item.path == "/dev/kfd" for item in devices} != {
        False,
        True,
    }:
        fail("executor must expose exactly /dev/kfd and one render node")
    machine_id_digest = cursor.scalar("host_machine_id_sha256")
    kernel_release = cursor.scalar("host_kernel_release")
    kernel_notes_digest = cursor.scalar("host_kernel_notes_sha256")
    driver_path = cursor.scalar("amdgpu_module_path")
    driver_digest = cursor.scalar("amdgpu_module_sha256")
    gpu_pci_slot = cursor.scalar("gpu_pci_slot")
    gpu_pci_id = cursor.scalar("gpu_pci_id")
    gpu_unique_id = cursor.scalar("gpu_unique_id")
    if (
        not SHA256_RE.fullmatch(machine_id_digest)
        or not re.fullmatch(r"[A-Za-z0-9._+-]{1,128}", kernel_release)
        or not SHA256_RE.fullmatch(kernel_notes_digest)
        or not valid_absolute(driver_path)
        or not SHA256_RE.fullmatch(driver_digest)
        or not re.fullmatch(r"[0-9a-f]{4}:[0-9a-f]{2}:[0-9a-f]{2}\.[0-7]", gpu_pci_slot)
        or not re.fullmatch(r"[0-9A-F]{4}:[0-9A-F]{4}", gpu_pci_id)
        or not re.fullmatch(r"[0-9a-f]{16,64}", gpu_unique_id)
    ):
        fail("malformed protected host/GPU identity")
    environment_values = dict(environment)
    if (
        environment_values["HIP_VISIBLE_DEVICES"] != gpu_unique_id
        or environment_values["ROCR_VISIBLE_DEVICES"] != gpu_unique_id
    ):
        fail("GPU visibility does not match protected GPU identity")
    cursor.done()
    return Profile(
        actual_id,
        mode,
        target,
        lane,
        runtime_path,
        int(runtime_size),
        runtime_digest,
        runtime_version_digest,
        runtime_info_digest,
        git_objects_path,
        git_object_format,
        int(git_object_limit),
        int(git_object_bytes_limit),
        int(git_tree_depth_limit),
        source_staging_root,
        output_staging_root,
        artifact_stream_protocol,
        int(source_file_limit),
        int(source_byte_limit),
        int(source_index_limit),
        int(operator_uid),
        int(operator_gid),
        layout_path,
        index_digest,
        int(index_size),
        image_reference,
        Layer(manifest_digest, int(manifest_size)),
        Layer(config_digest, int(config_size)),
        tuple(layers),
        entrypoint,
        command,
        environment,
        source_mount,
        request_mount,
        output_mount,
        tmp_mount,
        output_limit,
        tmp_limit,
        shm_limit,
        log_limit,
        memory_limit,
        pids_limit,
        cpu_limit_milli,
        uid,
        gid,
        supplemental_gid,
        seccomp_path,
        int(seccomp_size),
        seccomp_digest,
        tuple(devices),
        machine_id_digest,
        kernel_release,
        kernel_notes_digest,
        driver_path,
        driver_digest,
        gpu_pci_slot,
        gpu_pci_id,
        gpu_unique_id,
        entry.digest,
    )


def require_safe_oci_entry(
    layout: Path, relative: tuple[str, ...], label: str, *, directory: bool = False
) -> Path:
    current = layout
    for index, component in enumerate(relative):
        current /= component
        try:
            info = current.lstat()
        except OSError:
            fail(f"{label} is missing")
        if stat.S_ISLNK(info.st_mode):
            fail(f"{label} path contains a symlink")
        is_last = index + 1 == len(relative)
        if not is_last or directory:
            if not stat.S_ISDIR(info.st_mode):
                fail(f"{label} path contains a non-directory")
        elif not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            fail(f"{label} is not a single-link regular file")
    if layout not in resolve_path(current, label).parents:
        fail(f"{label} escapes OCI layout")
    return current


def strict_json_object(raw: bytes, label: str) -> dict[str, object]:
    def object_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                fail(f"{label} JSON contains a duplicate key")
            result[key] = value
        return result

    def reject_constant(value: str) -> object:
        raise ValueError(value)

    try:
        value = json.loads(
            raw,
            object_pairs_hook=object_pairs,
            parse_constant=reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError, ValueError):
        fail(f"invalid {label} JSON")
    if not isinstance(value, dict):
        fail(f"invalid {label} JSON object")
    try:
        validate_json_shape(value, label)
    except RecursionError:
        fail(f"{label} JSON exceeds structural limits")
    return value


def read_oci_json(
    layout_fd: int,
    relative: tuple[str, ...],
    label: str,
    *,
    maximum_bytes: int,
    expected_uid: int,
    expected_gid: int,
    expected_size: int | None = None,
    expected_digest: str | None = None,
) -> dict[str, object]:
    if (
        not relative
        or any(
            not component or "/" in component or component in (".", "..")
            for component in relative
        )
        or not 1 <= maximum_bytes <= MAX_OCI_METADATA_BYTES
        or expected_size is not None
        and not 1 <= expected_size <= maximum_bytes
        or expected_digest is not None
        and SHA256_RE.fullmatch(expected_digest) is None
    ):
        fail(f"invalid {label} descriptor read contract")
    current_fd = -1
    file_fd = -1
    try:
        current_fd = os.dup(layout_fd)
        for component in relative[:-1]:
            next_fd = os.open(
                component,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=current_fd,
            )
            close_descriptor(current_fd, f"{label} JSON parent")
            current_fd = next_fd
            parent = os.fstat(current_fd)
            if (
                not stat.S_ISDIR(parent.st_mode)
                or (parent.st_uid, parent.st_gid) != (expected_uid, expected_gid)
                or parent.st_mode & 0o022
            ):
                fail(f"{label} parent ownership or mode is unsafe")
        file_fd = os.open(
            relative[-1],
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=current_fd,
        )
        before = os.fstat(file_fd)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or (before.st_uid, before.st_gid) != (expected_uid, expected_gid)
            or before.st_mode & 0o022
            or not 1 <= before.st_size <= maximum_bytes
            or expected_size is not None
            and before.st_size != expected_size
        ):
            fail(f"{label} descriptor metadata or size is invalid")
        raw = read_descriptor_bound(file_fd, maximum_bytes, label)
        after = os.fstat(file_fd)
        if (
            stable_file_identity(before) != stable_file_identity(after)
            or len(raw) != before.st_size
            or expected_digest is not None
            and sha256_bytes(raw) != expected_digest
        ):
            fail(f"{label} changed or differs from its protected binding")
        return strict_json_object(raw, label)
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot open or read {label} by descriptor: {error}")
    finally:
        close_descriptors(
            (
                (file_fd, f"{label} JSON"),
                (current_fd, f"{label} JSON parent"),
            )
        )


def validate_json_shape(value: object, label: str) -> None:
    nodes = 0

    def visit(item: object, depth: int) -> None:
        nonlocal nodes
        nodes += 1
        if nodes > MAX_JSON_NODES or depth > MAX_JSON_DEPTH:
            fail(f"{label} JSON exceeds structural limits")
        if isinstance(item, str):
            if len(item.encode("utf-8")) > MAX_JSON_STRING_BYTES:
                fail(f"{label} JSON string exceeds limit")
        elif isinstance(item, list):
            if len(item) > MAX_JSON_NODES:
                fail(f"{label} JSON array exceeds limit")
            for child in item:
                visit(child, depth + 1)
        elif isinstance(item, dict):
            if len(item) > MAX_JSON_NODES:
                fail(f"{label} JSON object exceeds limit")
            for key, child in item.items():
                if not isinstance(key, str):
                    fail(f"{label} JSON has a non-string key")
                visit(key, depth + 1)
                visit(child, depth + 1)
        elif isinstance(item, float):
            if not math.isfinite(item):
                fail(f"{label} JSON contains a non-finite number")
        elif item is not None and not isinstance(item, (bool, int)):
            fail(f"{label} JSON contains an unsupported value")

    visit(value, 0)


def descriptor(value: object, label: str) -> Layer:
    if not isinstance(value, dict):
        fail(f"invalid OCI {label} descriptor")
    digest = value.get("digest")
    size = value.get("size")
    if (
        not isinstance(digest, str)
        or OCI_DIGEST_RE.fullmatch(digest) is None
        or type(size) is not int
        or size < 0
        or size > MAX_OCI_LAYER_BYTES
    ):
        fail(f"invalid OCI {label} descriptor")
    return Layer(digest, size)


def validate_image_config(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict or value.get("Env") not in (None, []):
        fail(f"{label} must not supply inherited environment")
    if "Volumes" in value:
        fail(f"{label} must not declare volumes")
    if "Healthcheck" in value:
        fail(f"{label} must not declare a healthcheck")
    return value


def validate_oci_rootfs(value: object, layer_count: int) -> tuple[str, ...]:
    if type(value) is not dict:
        fail("OCI config lacks a layer rootfs")
    diff_ids = value.get("diff_ids")
    if value.get("type") != "layers" or type(diff_ids) is not list:
        fail("OCI config lacks a layer rootfs")
    if any(
        type(item) is not str or OCI_DIGEST_RE.fullmatch(item) is None
        for item in diff_ids
    ):
        fail("OCI config has malformed rootfs diff IDs")
    if len(diff_ids) != layer_count:
        fail("OCI config/layer count mismatch")
    return tuple(diff_ids)


def validate_runtime_rootfs(value: object, expected: tuple[str, ...]) -> None:
    if type(value) is not dict or value.get("Type") != "layers":
        fail("runtime image has malformed RootFS")
    layers = value.get("Layers")
    if type(layers) is not list or any(type(item) is not str for item in layers):
        fail("runtime image has malformed RootFS.Layers")
    if tuple(layers) != expected:
        fail("runtime image layers differ from protected OCI image")


def verify_oci_image(profile: Profile) -> tuple[str, ...]:
    layout = Path(profile.layout_path)
    if not layout.is_absolute() or not valid_absolute(str(layout)):
        fail("OCI layout is unavailable or unsafe")
    layout_fd = -1
    try:
        layout_fd = os.open(
            layout,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        layout_info = os.fstat(layout_fd)
        if (
            not stat.S_ISDIR(layout_info.st_mode)
            or (layout_info.st_uid, layout_info.st_gid)
            != (profile.operator_uid, profile.operator_gid)
            or layout_info.st_mode & 0o022
        ):
            fail("OCI layout descriptor ownership or mode is unsafe")
        marker = read_oci_json(
            layout_fd,
            ("oci-layout",),
            "OCI layout marker",
            maximum_bytes=1024,
            expected_uid=profile.operator_uid,
            expected_gid=profile.operator_gid,
        )
        if marker != {"imageLayoutVersion": "1.0.0"}:
            fail("unsupported OCI layout version")
        index = read_oci_json(
            layout_fd,
            ("index.json",),
            "OCI index",
            maximum_bytes=MAX_OCI_METADATA_BYTES,
            expected_uid=profile.operator_uid,
            expected_gid=profile.operator_gid,
            expected_size=profile.index_size,
            expected_digest=profile.index_digest,
        )
        manifests = index.get("manifests") if isinstance(index, dict) else None
        if not isinstance(manifests, list):
            fail("invalid OCI index manifests")
        selected = [descriptor(value, "manifest") for value in manifests]
        if selected.count(profile.manifest) != 1:
            fail("protected OCI manifest is absent or ambiguous")
        manifest = read_oci_json(
            layout_fd,
            (
                "blobs",
                "sha256",
                profile.manifest.digest.removeprefix("sha256:"),
            ),
            "OCI manifest",
            maximum_bytes=MAX_OCI_METADATA_BYTES,
            expected_uid=profile.operator_uid,
            expected_gid=profile.operator_gid,
            expected_size=profile.manifest.size,
            expected_digest=profile.manifest.digest.removeprefix("sha256:"),
        )
        if descriptor(manifest.get("config"), "config") != profile.config:
            fail("OCI config differs from protected profile")
        layer_values = manifest.get("layers")
        if (
            not isinstance(layer_values, list)
            or tuple(descriptor(value, "layer") for value in layer_values)
            != profile.layers
        ):
            fail("OCI layers differ from protected profile")
        config = read_oci_json(
            layout_fd,
            (
                "blobs",
                "sha256",
                profile.config.digest.removeprefix("sha256:"),
            ),
            "OCI config",
            maximum_bytes=MAX_OCI_METADATA_BYTES,
            expected_uid=profile.operator_uid,
            expected_gid=profile.operator_gid,
            expected_size=profile.config.size,
            expected_digest=profile.config.digest.removeprefix("sha256:"),
        )
        diff_ids = validate_oci_rootfs(config.get("rootfs"), len(profile.layers))
        if config.get("architecture") != "amd64" or config.get("os") != "linux":
            fail("OCI image platform must be linux/amd64")
        validate_image_config(config.get("config"), "OCI image config")
        for layer in profile.layers:
            layer_path = require_safe_oci_entry(
                layout,
                ("blobs", "sha256", layer.digest.removeprefix("sha256:")),
                "OCI layer",
            )
            verify_operator_owned(layer_path, profile, "OCI layer", directory=False)
            verify_regular(
                layer_path,
                str(layer.size),
                layer.digest.removeprefix("sha256:"),
                "OCI layer",
            )
        return diff_ids
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot open OCI layout by descriptor: {error}")
    finally:
        if layout_fd >= 0:
            close_descriptor(layout_fd, "OCI layout")


@dataclass(frozen=True)
class Request:
    request_id: str
    profile_id: str
    source_commit: str
    source_tree: str
    job_id: str
    job_path: str
    job_digest: str
    raw: bytes
    digest: str


@dataclass(frozen=True)
class AuthorizedRequest:
    """Inputs authorized by protected policy, before any runtime contact."""

    policy: Policy
    profile: Profile
    request: Request
    seccomp_path: Path
    authorization_digest: str


@dataclass(frozen=True)
class ObservedRuntimeRequest:
    """Authorized inputs whose bounded host/runtime observations matched policy."""

    authorized: AuthorizedRequest
    image_diff_ids: tuple[str, ...]


def parse_request(raw: bytes) -> Request:
    rows = parse_rows(raw, "OCI execution request")
    cursor = Cursor(rows, "OCI execution request")
    if cursor.scalar("oci_execution_request_schema_version") != "1":
        fail("OCI execution request schema must be 1")
    request_id = cursor.scalar("request_id")
    profile_id = cursor.scalar("profile_id")
    source_commit = cursor.scalar("source_commit")
    source_tree = cursor.scalar("source_tree")
    job_id = cursor.scalar("job_id")
    job_path = cursor.scalar("job_path")
    job_digest = cursor.scalar("job_sha256")
    cursor.done()
    if (
        not RESULT_ID_RE.fullmatch(request_id)
        or not ID_RE.fullmatch(profile_id)
        or not COMMIT_RE.fullmatch(source_commit)
        or not COMMIT_RE.fullmatch(source_tree)
        or not ID_RE.fullmatch(job_id)
        or not valid_relative(job_path)
        or not job_path.startswith("scripts/evidence/jobs/")
        or not SHA256_RE.fullmatch(job_digest)
    ):
        fail("malformed OCI execution request")
    return Request(
        request_id,
        profile_id,
        source_commit,
        source_tree,
        job_id,
        job_path,
        job_digest,
        raw,
        sha256_bytes(raw),
    )


def read_request_file(path: Path, expected_uid: int, expected_gid: int) -> Request:
    if (
        not path.is_absolute()
        or not valid_absolute(str(path))
        or not 0 <= expected_uid <= 2**31 - 1
        or not 0 <= expected_gid <= 2**31 - 1
    ):
        fail("OCI execution request path or external owner binding is invalid")
    request_fd = -1
    try:
        request_fd = os.open(
            path,
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        before = os.fstat(request_fd)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or (before.st_uid, before.st_gid) != (expected_uid, expected_gid)
            or before.st_mode & 0o022
            or not 1 <= before.st_size <= MAX_FILE_BYTES
        ):
            fail(
                "OCI execution request violates its external "
                "owner/mode/type/link/size contract"
            )
        raw = read_descriptor_bound(request_fd, MAX_FILE_BYTES, "OCI execution request")
        after = os.fstat(request_fd)
        if (
            stable_file_identity(before) != stable_file_identity(after)
            or len(raw) != before.st_size
        ):
            fail("OCI execution request changed while being read")
        return parse_request(raw)
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot open OCI execution request without following links: {error}")
    finally:
        if request_fd >= 0:
            close_descriptor(request_fd, "OCI execution request")


def read_operator_request(config: OperatorConfig, request_id: str) -> Request:
    if not RESULT_ID_RE.fullmatch(request_id):
        fail("operator request identity is malformed")
    inbox_fd = open_owned_directory_tree(
        Path(config.inbox_root),
        config.inbox_owner_uid,
        config.inbox_owner_gid,
        "operator request inbox",
    )
    try:
        raw = read_owned_descriptor_file(
            inbox_fd,
            f"{request_id}.tsv",
            "operator request",
            maximum_bytes=MAX_FILE_BYTES,
            expected_uid=config.request_owner_uid,
            expected_gid=config.request_owner_gid,
            require_immutable=False,
        )
    finally:
        close_descriptor(inbox_fd, "operator request inbox")
    request = parse_request(raw)
    if request.request_id != request_id:
        fail("operator request file identity differs from selected request")
    return request


def staging_lease_name(authorization_digest: str) -> str:
    if not SHA256_RE.fullmatch(authorization_digest):
        fail("invalid staging authorization identity")
    return f"plan-{authorization_digest}-{secrets.token_hex(32)}"


def open_staging_root(profile: Profile, path: Path, label: str) -> int:
    root_fd = -1
    try:
        root_fd = os.open(
            path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
        )
        info = os.fstat(root_fd)
        if (
            not stat.S_ISDIR(info.st_mode)
            or (info.st_uid, info.st_gid)
            != (profile.operator_uid, profile.operator_gid)
            or info.st_mode & 0o022
        ):
            fail(f"{label} ownership or mode changed before staging")
        fcntl.flock(root_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        count = 0
        with os.scandir(root_fd) as entries:
            for _ in entries:
                count += 1
                if count > MAX_STAGING_ROOT_ENTRIES:
                    fail(f"{label} exceeds the protected stale-entry quota")
        return root_fd
    except BlockingIOError:
        close_descriptor(root_fd, f"busy {label}")
        fail(f"{label} is busy with another staging lease")
    except ExecutorError:
        close_descriptor(root_fd, f"invalid {label}")
        raise
    except OSError as error:
        close_descriptor(root_fd, f"unavailable {label}")
        fail(f"cannot open or lock {label}: {error}")


def remove_directory_contents(directory_fd: int, budget: list[int], label: str) -> None:
    try:
        os.fchmod(directory_fd, 0o700)
        with os.scandir(directory_fd) as iterator:
            names = []
            for entry in iterator:
                budget[0] += 1
                if budget[0] > MAX_CLEANUP_ENTRIES:
                    fail(f"{label} cleanup exceeds the protected entry quota")
                names.append(entry.name)
        for name in sorted(names):
            info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                child_fd = os.open(
                    name,
                    os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                    dir_fd=directory_fd,
                )
                try:
                    remove_directory_contents(child_fd, budget, label)
                finally:
                    close_descriptor(child_fd, f"{label} child during cleanup")
                os.rmdir(name, dir_fd=directory_fd)
            else:
                os.unlink(name, dir_fd=directory_fd)
            os.fsync(directory_fd)
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot durably clean {label}: {error}")


def cleanup_staging_lease(
    root_fd: int,
    name: str,
    expected_device: int,
    expected_inode: int,
    label: str,
) -> None:
    lease_fd = -1
    try:
        lease_fd = os.open(
            name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=root_fd,
        )
        info = os.fstat(lease_fd)
        if (info.st_dev, info.st_ino) != (expected_device, expected_inode):
            fail(f"{label} lease identity changed before cleanup")
        remove_directory_contents(lease_fd, [0], label)
        closing_fd = lease_fd
        lease_fd = -1
        close_descriptor(closing_fd, f"{label} lease before removal")
        os.rmdir(name, dir_fd=root_fd)
        os.fsync(root_fd)
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot durably remove {label}: {error}")
    finally:
        close_descriptor(lease_fd, f"{label} lease")


@dataclass
class SourceSnapshot:
    path: Path
    manifest_path: Path
    request_path: Path
    staging_root_fd: int
    root_fd: int
    directory_fd: int
    request_fd: int
    device: int
    inode: int
    request_device: int
    request_inode: int
    file_count: int
    byte_count: int
    manifest_digest: str
    lease_name: str
    lease_device: int
    lease_inode: int

    def close(self) -> None:
        fields = ("request_fd", "directory_fd", "root_fd", "staging_root_fd")
        descriptors = tuple(
            (getattr(self, field), f"source snapshot {field}") for field in fields
        )
        for field in fields:
            setattr(self, field, -1)
        close_descriptors(descriptors)

    def cleanup(self) -> None:
        descriptors = (
            (self.request_fd, "source request snapshot"),
            (self.directory_fd, "source snapshot directory"),
            (self.root_fd, "source staging lease"),
        )
        self.request_fd = -1
        self.directory_fd = -1
        self.root_fd = -1
        failure: ExecutorError | None = None
        try:
            close_descriptors(descriptors)
        except ExecutorError as error:
            failure = error
        try:
            cleanup_staging_lease(
                self.staging_root_fd,
                self.lease_name,
                self.lease_device,
                self.lease_inode,
                "source staging",
            )
        except (ExecutorError, OSError) as error:
            failure = append_error(failure, error)
        finally:
            try:
                close_descriptor(self.staging_root_fd, "source staging root")
            except ExecutorError as error:
                failure = append_error(failure, error)
            self.staging_root_fd = -1
        if failure is not None:
            raise failure


@dataclass
class OutputStage:
    path: Path
    artifact_path: Path
    log_path: Path
    root_fd: int
    directory_fd: int
    artifact_fd: int
    log_fd: int
    device: int
    inode: int
    lease_name: str

    def close(self) -> None:
        fields = ("log_fd", "artifact_fd", "directory_fd", "root_fd")
        descriptors = tuple(
            (getattr(self, field), f"output stage {field}") for field in fields
        )
        for field in fields:
            setattr(self, field, -1)
        close_descriptors(descriptors)

    def cleanup(self) -> None:
        descriptors = (
            (self.log_fd, "output stderr stream"),
            (self.artifact_fd, "output artifact stream"),
            (self.directory_fd, "output staging directory"),
        )
        self.log_fd = -1
        self.artifact_fd = -1
        self.directory_fd = -1
        failure: ExecutorError | None = None
        try:
            close_descriptors(descriptors)
        except ExecutorError as error:
            failure = error
        try:
            cleanup_staging_lease(
                self.root_fd,
                self.lease_name,
                self.device,
                self.inode,
                "output staging",
            )
        except (ExecutorError, OSError) as error:
            failure = append_error(failure, error)
        finally:
            try:
                close_descriptor(self.root_fd, "output staging root")
            except ExecutorError as error:
                failure = append_error(failure, error)
            self.root_fd = -1
        if failure is not None:
            raise failure


def verify_git_store_metadata(
    info: os.stat_result, profile: Profile, label: str, *, directory: bool
) -> None:
    expected_kind = (
        stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    )
    if (
        not expected_kind
        or (info.st_uid, info.st_gid) != (profile.operator_uid, profile.operator_gid)
        or info.st_mode & 0o022
        or not directory
        and info.st_nlink != 1
    ):
        fail(f"{label} ownership, mode, type, or link contract is unsafe")


def open_git_store_directory(
    parent_fd: int, name: str, profile: Profile, label: str
) -> int:
    file_fd = -1
    try:
        file_fd = os.open(
            name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=parent_fd,
        )
        verify_git_store_metadata(os.fstat(file_fd), profile, label, directory=True)
        return file_fd
    except ExecutorError:
        close_descriptor(file_fd, label)
        raise
    except OSError as error:
        close_descriptor(file_fd, label)
        fail(f"cannot open {label} without following links: {error}")


def scan_git_directory(file_fd: int, maximum: int, label: str) -> list[str]:
    names: list[str] = []
    try:
        with os.scandir(file_fd) as entries:
            for entry in entries:
                names.append(entry.name)
                if len(names) > maximum:
                    fail(f"{label} exceeds protected entry limit")
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot scan {label}: {error}")
    return sorted(names)


def git_path_exists(parent_fd: int, name: str, label: str) -> bool:
    try:
        os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return False
    except OSError as error:
        fail(f"cannot inspect {label}: {error}")
    return True


def read_git_control_file(
    parent_fd: int, name: str, profile: Profile, maximum: int, label: str
) -> bytes:
    file_fd = -1
    try:
        file_fd = os.open(
            name,
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=parent_fd,
        )
        before = os.fstat(file_fd)
        verify_git_store_metadata(before, profile, label, directory=False)
        if not 1 <= before.st_size <= maximum:
            fail(f"{label} exceeds protected size limit")
        raw = read_descriptor_bound(file_fd, maximum, label)
        after = os.fstat(file_fd)
        if stable_file_identity(before) != stable_file_identity(after):
            fail(f"{label} changed while being read")
        return raw
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot read {label} without following links: {error}")
    finally:
        close_descriptor(file_fd, label)


@dataclass(frozen=True)
class GitTreeEntry:
    name: str
    mode: str
    object_id: str
    directory: bool


@dataclass
class GitLooseObjectStore:
    profile: Profile
    git_directory_fd: int
    objects_fd: int
    cache: dict[str, tuple[str, bytes]]
    object_count: int = 0
    compressed_bytes: int = 0
    expanded_bytes: int = 0
    tree_bytes: int = 0

    @classmethod
    def open(cls, profile: Profile) -> GitLooseObjectStore:
        forbidden_environment = (
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_OBJECT_DIRECTORY",
            "GIT_REPLACE_REF_BASE",
            "GIT_GRAFT_FILE",
        )
        if any(name in os.environ for name in forbidden_environment):
            fail("Git object-store indirection environment is forbidden")
        objects_path = Path(profile.git_objects_path)
        if objects_path.name != "objects":
            fail("Git object store must be an exact repository objects directory")
        git_fd = -1
        objects_fd = -1
        try:
            git_fd = os.open(
                objects_path.parent,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            )
            verify_git_store_metadata(
                os.fstat(git_fd), profile, "Git repository directory", directory=True
            )
            objects_fd = open_git_store_directory(
                git_fd, "objects", profile, "Git object store"
            )
            store = cls(profile, git_fd, objects_fd, {})
            store.reject_repository_indirection()
            return store
        except (ExecutorError, OSError) as error:
            close_descriptors(
                (
                    (objects_fd, "Git object store"),
                    (git_fd, "Git repository directory"),
                )
            )
            if isinstance(error, ExecutorError):
                raise
            fail(f"cannot open Git object store by descriptor: {error}")

    def close(self) -> None:
        descriptors = (
            (self.objects_fd, "Git object store"),
            (self.git_directory_fd, "Git repository directory"),
        )
        self.objects_fd = -1
        self.git_directory_fd = -1
        close_descriptors(descriptors)

    def reject_repository_indirection(self) -> None:
        names = scan_git_directory(
            self.objects_fd, MAX_GIT_ROOT_ENTRIES, "Git object store root"
        )
        for name in names:
            if name in ("info", "pack"):
                directory_fd = open_git_store_directory(
                    self.objects_fd, name, self.profile, f"Git objects/{name}"
                )
                try:
                    if scan_git_directory(
                        directory_fd,
                        self.profile.git_object_limit,
                        f"Git objects/{name}",
                    ):
                        fail(
                            "Git alternates, packed objects, indexes, commit graphs, "
                            "and promisor metadata are forbidden"
                        )
                finally:
                    close_descriptor(directory_fd, f"Git objects/{name}")
            elif re.fullmatch(r"[0-9a-f]{2}", name):
                directory_fd = open_git_store_directory(
                    self.objects_fd, name, self.profile, f"Git object fanout {name}"
                )
                close_descriptor(directory_fd, f"Git object fanout {name}")
            else:
                fail("Git object store contains an unsupported root entry")

        if git_path_exists(
            self.git_directory_fd, "info", "Git repository info directory"
        ):
            info_fd = open_git_store_directory(
                self.git_directory_fd,
                "info",
                self.profile,
                "Git repository info directory",
            )
            try:
                if "grafts" in scan_git_directory(
                    info_fd,
                    self.profile.git_object_limit,
                    "Git repository info directory",
                ):
                    fail("Git grafts are forbidden")
            finally:
                close_descriptor(info_fd, "Git repository info directory")

        if git_path_exists(self.git_directory_fd, "refs", "Git refs directory"):
            refs_fd = open_git_store_directory(
                self.git_directory_fd,
                "refs",
                self.profile,
                "Git refs directory",
            )
            try:
                if git_path_exists(refs_fd, "replace", "Git replace refs"):
                    fail("Git replace refs are forbidden")
            finally:
                close_descriptor(refs_fd, "Git refs directory")

        if git_path_exists(
            self.git_directory_fd, "config", "Git repository configuration"
        ):
            raw = read_git_control_file(
                self.git_directory_fd,
                "config",
                self.profile,
                MAX_GIT_CONFIG_BYTES,
                "Git repository configuration",
            )
            try:
                text = raw.decode("utf-8")
                parser = configparser.ConfigParser(
                    interpolation=None, strict=True, default_section="__forbidden__"
                )
                parser.read_string(text)
            except (UnicodeDecodeError, configparser.Error):
                fail("Git repository configuration is malformed")
            for section in parser.sections():
                lowered = section.lower()
                options = {name.lower() for name in parser.options(section)}
                if lowered == "extensions" and "partialclone" in options:
                    fail("Git partial-clone configuration is forbidden")
                if lowered.startswith('remote "') and (
                    "promisor" in options or "partialclonefilter" in options
                ):
                    fail("Git promisor configuration is forbidden")

    def read_object(self, object_id: str, expected_kind: str) -> bytes:
        if COMMIT_RE.fullmatch(object_id) is None:
            fail("Git object ID is malformed")
        cached = self.cache.get(object_id)
        if cached is not None:
            kind, payload = cached
            if kind != expected_kind:
                fail("Git object kind differs from tree closure")
            return payload
        if self.object_count >= self.profile.git_object_limit:
            fail("Git object count exceeds protected limit")
        payload_limits = {
            "blob": self.profile.source_byte_limit,
            "tree": self.profile.source_index_limit,
            "commit": MAX_FILE_BYTES,
        }
        payload_limit = payload_limits.get(expected_kind)
        if payload_limit is None:
            fail("Git object kind is not supported")
        compressed_remaining = (
            self.profile.git_object_bytes_limit - self.compressed_bytes
        )
        compressed_limit = min(compressed_remaining, payload_limit + 64 * 1024)
        if compressed_limit < 1:
            fail("Git compressed object bytes exceed protected limit")
        fanout_fd = open_git_store_directory(
            self.objects_fd,
            object_id[:2],
            self.profile,
            f"Git object fanout {object_id[:2]}",
        )
        object_fd = -1
        try:
            object_fd = os.open(
                object_id[2:],
                os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=fanout_fd,
            )
            before = os.fstat(object_fd)
            verify_git_store_metadata(
                before, self.profile, f"Git object {object_id}", directory=False
            )
            if not 1 <= before.st_size <= compressed_limit:
                fail("Git compressed object bytes exceed protected limit")
            compressed = read_descriptor_bound(
                object_fd, compressed_limit, f"Git object {object_id}"
            )
            after = os.fstat(object_fd)
            if stable_file_identity(before) != stable_file_identity(after):
                fail(f"Git object {object_id} changed while being read")
        except ExecutorError:
            raise
        except OSError as error:
            fail(f"cannot read Git object {object_id} by descriptor: {error}")
        finally:
            close_descriptors(
                (
                    (object_fd, f"Git object {object_id}"),
                    (fanout_fd, f"Git object fanout {object_id[:2]}"),
                )
            )

        expanded_remaining = self.profile.git_object_bytes_limit - self.expanded_bytes
        expanded_limit = min(expanded_remaining, payload_limit + 128)
        if expanded_limit < 1:
            fail("Git expanded object bytes exceed protected limit")
        try:
            decompressor = zlib.decompressobj()
            expanded = decompressor.decompress(compressed, expanded_limit + 1)
            if decompressor.unconsumed_tail or len(expanded) > expanded_limit:
                fail("Git expanded object bytes exceed protected limit")
            expanded += decompressor.flush(expanded_limit + 1 - len(expanded))
        except zlib.error:
            fail(f"Git object {object_id} has invalid zlib framing")
        if (
            len(expanded) > expanded_limit
            or not decompressor.eof
            or decompressor.unused_data
            or decompressor.unconsumed_tail
        ):
            fail(f"Git object {object_id} has invalid or oversized zlib framing")
        try:
            header, payload = expanded.split(b"\0", 1)
            kind_raw, size_raw = header.split(b" ", 1)
            kind = kind_raw.decode("ascii")
            size_text = size_raw.decode("ascii")
        except (ValueError, UnicodeDecodeError):
            fail(f"Git object {object_id} has malformed canonical framing")
        if (
            kind not in ("blob", "tree", "commit")
            or kind != expected_kind
            or not size_text.isdigit()
            or len(size_text) > 20
            or len(size_text) > 1
            and size_text.startswith("0")
            or int(size_text) != len(payload)
            or hashlib.sha1(expanded).hexdigest() != object_id
        ):
            fail(f"Git object ID, kind, or size mismatch for {object_id}")
        if len(payload) > payload_limit:
            labels = {
                "blob": "Git source bytes exceed protected limit",
                "tree": "Git tree bytes exceed protected index limit",
                "commit": "Git commit bytes exceed protected limit",
            }
            fail(labels[expected_kind])
        self.object_count += 1
        self.compressed_bytes += len(compressed)
        self.expanded_bytes += len(expanded)
        self.cache[object_id] = (kind, payload)
        return payload

    def commit_tree(self, commit_id: str) -> str:
        payload = self.read_object(commit_id, "commit")
        header_block = payload.split(b"\n\n", 1)[0]
        lines = header_block.split(b"\n")
        trees = [line[5:] for line in lines if line.startswith(b"tree ")]
        if len(trees) != 1 or not lines or lines[0] != b"tree " + trees[0]:
            fail("Git commit has malformed or ambiguous tree binding")
        try:
            tree_id = trees[0].decode("ascii")
        except UnicodeDecodeError:
            fail("Git commit tree ID is not ASCII")
        if COMMIT_RE.fullmatch(tree_id) is None:
            fail("Git commit tree ID is malformed")
        return tree_id

    def parse_tree(self, tree_id: str) -> list[GitTreeEntry]:
        payload = self.read_object(tree_id, "tree")
        self.tree_bytes += len(payload)
        if self.tree_bytes > self.profile.source_index_limit:
            fail("Git tree bytes exceed protected index limit")
        output: list[GitTreeEntry] = []
        position = 0
        previous_key: bytes | None = None
        seen_names: set[bytes] = set()
        while position < len(payload):
            space = payload.find(b" ", position, min(len(payload), position + 8))
            nul = payload.find(b"\0", space + 1, min(len(payload), space + 258))
            if space < 0 or nul < 0 or nul + 21 > len(payload):
                fail("Git tree object is structurally malformed")
            mode_raw = payload[position:space]
            name_raw = payload[space + 1 : nul]
            object_id = payload[nul + 1 : nul + 21].hex()
            position = nul + 21
            directory = mode_raw == b"40000"
            if mode_raw not in (b"40000", b"100644", b"100755"):
                fail("Git tree contains a symlink, submodule, or unsupported mode")
            if (
                not name_raw
                or len(name_raw) > 255
                or name_raw in (b".", b"..", b".git")
                or b"/" in name_raw
                or b"\n" in name_raw
                or b"\r" in name_raw
                or name_raw in seen_names
            ):
                fail("Git tree contains an unsafe or duplicate name")
            try:
                name = name_raw.decode("ascii")
            except UnicodeDecodeError:
                fail("Git tree path is not canonical ASCII")
            if re.fullmatch(r"[A-Za-z0-9._+:-]{1,255}", name) is None:
                fail("Git tree path contains unsupported characters")
            ordering_key = name_raw + (b"/" if directory else b"")
            if previous_key is not None and ordering_key <= previous_key:
                fail("Git tree entries are not in canonical order")
            previous_key = ordering_key
            seen_names.add(name_raw)
            output.append(
                GitTreeEntry(name, mode_raw.decode("ascii"), object_id, directory)
            )
        return output

    def export_tree(self, tree_id: str) -> list[tuple[str, str, str, bytes]]:
        output: list[tuple[str, str, str, bytes]] = []
        active: set[str] = set()
        directory_count = 0

        def visit(object_id: str, prefix: str, depth: int) -> None:
            nonlocal directory_count
            if depth > self.profile.git_tree_depth_limit:
                fail("Git tree depth exceeds protected limit")
            if object_id in active:
                fail("Git tree closure contains a cycle")
            active.add(object_id)
            try:
                for entry in self.parse_tree(object_id):
                    path = entry.name if not prefix else f"{prefix}/{entry.name}"
                    if len(path) > 512:
                        fail("Git source path exceeds protected limit")
                    if entry.directory:
                        directory_count += 1
                        if directory_count > MAX_SOURCE_DIRECTORIES:
                            fail("Git source directory count exceeds protected limit")
                        visit(entry.object_id, path, depth + 1)
                    else:
                        if len(output) >= self.profile.source_file_limit:
                            fail("Git source file count exceeds protected limit")
                        payload = self.read_object(entry.object_id, "blob")
                        output.append((path, entry.mode, entry.object_id, payload))
            finally:
                active.remove(object_id)

        visit(tree_id, "", 0)
        output.sort(key=lambda item: item[0])
        if not output or len({item[0] for item in output}) != len(output):
            fail("Git source closure is empty or contains duplicate paths")
        return output


def open_child_directory(parent_fd: int, component: str) -> int:
    try:
        os.mkdir(component, 0o700, dir_fd=parent_fd)
    except FileExistsError:
        pass
    except OSError as error:
        fail(f"cannot create source staging directory: {error}")
    try:
        return os.open(
            component,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=parent_fd,
        )
    except OSError:
        fail("source staging path changed during export")


def write_all(file_fd: int, content: bytes) -> None:
    view = memoryview(content)
    try:
        while view:
            written = os.write(file_fd, view)
            if written <= 0:
                fail("staging write made no progress")
            view = view[written:]
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot write bounded staging content: {error}")


def finalize_source_file(file_fd: int, mode: int, label: str) -> None:
    try:
        os.fchmod(file_fd, mode)
        os.fsync(file_fd)
    except OSError as error:
        fail(f"cannot durably finalize {label}: {error}")


def open_snapshot_directory(snapshot_fd: int, relative: str) -> int:
    try:
        current_fd = os.dup(snapshot_fd)
    except OSError as error:
        fail(f"cannot retain source staging directory: {error}")
    try:
        for component in relative.split("/"):
            next_fd = os.open(
                component,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
                dir_fd=current_fd,
            )
            close_descriptor(current_fd, "source staging directory parent")
            current_fd = next_fd
        return current_fd
    except OSError as error:
        close_descriptor(current_fd, "changed source staging directory")
        fail(f"source staging directory changed during finalization: {error}")


def finalize_source_directories(
    snapshot_fd: int, directories: set[str], root_fd: int
) -> None:
    ordered = sorted(
        directories,
        key=lambda relative: (-len(relative.split("/")), relative),
    )
    for relative in ordered:
        directory_fd = open_snapshot_directory(snapshot_fd, relative)
        try:
            os.fchmod(directory_fd, 0o555)
            os.fsync(directory_fd)
        except OSError as error:
            fail(f"cannot durably finalize source directory {relative}: {error}")
        finally:
            close_descriptor(directory_fd, f"source directory {relative}")
    try:
        os.fchmod(snapshot_fd, 0o555)
        os.fsync(snapshot_fd)
        os.fsync(root_fd)
    except OSError as error:
        fail(f"cannot durably finalize source staging parents: {error}")


def write_snapshot_file(
    snapshot_fd: int, path: str, mode: str, content: bytes
) -> tuple[int, str]:
    components = path.split("/")
    try:
        parent_fd = os.dup(snapshot_fd)
    except OSError as error:
        fail(f"cannot retain source staging parent for {path}: {error}")
    try:
        for component in components[:-1]:
            next_fd = open_child_directory(parent_fd, component)
            close_descriptor(parent_fd, f"source file {path} parent")
            parent_fd = next_fd
        try:
            file_fd = os.open(
                components[-1],
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                0o600,
                dir_fd=parent_fd,
            )
        except OSError:
            fail("source staging file changed during export")
        try:
            write_all(file_fd, content)
            finalize_source_file(
                file_fd,
                0o555 if mode == "100755" else 0o444,
                f"source file {path}",
            )
        finally:
            close_descriptor(file_fd, f"source file {path}")
    finally:
        close_descriptor(parent_fd, f"source file {path} parent")
    return len(content), sha256_bytes(content)


def verify_retained_snapshot(snapshot: SourceSnapshot) -> None:
    try:
        by_fd = os.fstat(snapshot.directory_fd)
        by_name = os.stat(
            snapshot.path.name, dir_fd=snapshot.root_fd, follow_symlinks=False
        )
        path_info = snapshot.path.lstat()
    except OSError:
        fail("retained source snapshot path is unavailable")
    identities = {(item.st_dev, item.st_ino) for item in (by_fd, by_name, path_info)}
    if (
        len(identities) != 1
        or identities.pop() != (snapshot.device, snapshot.inode)
        or not stat.S_ISDIR(by_name.st_mode)
        or stat.S_ISLNK(path_info.st_mode)
    ):
        fail("retained source snapshot path was replaced")
    try:
        request_by_fd = os.fstat(snapshot.request_fd)
        request_by_name = os.stat(
            snapshot.request_path.name,
            dir_fd=snapshot.root_fd,
            follow_symlinks=False,
        )
        request_path_info = snapshot.request_path.lstat()
    except OSError:
        fail("retained request snapshot path is unavailable")
    request_identities = {
        (item.st_dev, item.st_ino)
        for item in (request_by_fd, request_by_name, request_path_info)
    }
    if (
        len(request_identities) != 1
        or request_identities.pop() != (snapshot.request_device, snapshot.request_inode)
        or not stat.S_ISREG(request_by_name.st_mode)
        or stat.S_ISLNK(request_path_info.st_mode)
    ):
        fail("retained request snapshot path was replaced")


def verify_retained_output(stage: OutputStage) -> None:
    try:
        by_fd = os.fstat(stage.directory_fd)
        by_name = os.stat(stage.path.name, dir_fd=stage.root_fd, follow_symlinks=False)
        by_path = stage.path.lstat()
        artifact = stage.artifact_path.lstat()
        log = stage.log_path.lstat()
        artifact_by_fd = os.fstat(stage.artifact_fd)
        log_by_fd = os.fstat(stage.log_fd)
    except OSError:
        fail("retained output staging path is unavailable")
    if (
        {(item.st_dev, item.st_ino) for item in (by_fd, by_name, by_path)}
        != {(stage.device, stage.inode)}
        or not stat.S_ISDIR(by_name.st_mode)
        or stat.S_ISLNK(by_path.st_mode)
        or not stat.S_ISREG(artifact.st_mode)
        or not stat.S_ISREG(log.st_mode)
        or (artifact.st_dev, artifact.st_ino)
        != (artifact_by_fd.st_dev, artifact_by_fd.st_ino)
        or (log.st_dev, log.st_ino) != (log_by_fd.st_dev, log_by_fd.st_ino)
    ):
        fail("retained output staging path was replaced")


def fsync_output_stage(
    artifact_fd: int, log_fd: int, directory_fd: int, root_fd: int
) -> None:
    try:
        for file_fd in (artifact_fd, log_fd):
            os.fsync(file_fd)
        os.fsync(directory_fd)
        os.fsync(root_fd)
    except OSError as error:
        fail(f"cannot durably initialize output staging: {error}")


def stage_output(profile: Profile, lease_name: str) -> OutputStage:
    root = Path(profile.output_staging_root)
    root_fd = open_staging_root(profile, root, "output staging root")
    directory_fd = -1
    artifact_fd = -1
    log_fd = -1
    created = False
    identity: os.stat_result | None = None
    try:
        os.mkdir(lease_name, 0o700, dir_fd=root_fd)
        created = True
        os.fsync(root_fd)
        directory_fd = os.open(
            lease_name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=root_fd,
        )
        identity = os.fstat(directory_fd)
        artifact_fd = os.open(
            "artifacts.stream",
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=directory_fd,
        )
        log_fd = os.open(
            "stderr.log",
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=directory_fd,
        )
        fsync_output_stage(artifact_fd, log_fd, directory_fd, root_fd)
        stage = OutputStage(
            root / lease_name,
            root / lease_name / "artifacts.stream",
            root / lease_name / "stderr.log",
            root_fd,
            directory_fd,
            artifact_fd,
            log_fd,
            identity.st_dev,
            identity.st_ino,
            lease_name,
        )
        verify_retained_output(stage)
        return stage
    except (ExecutorError, OSError) as error:
        failure = normalized_error(error, "cannot initialize output staging")
        try:
            close_descriptors(
                (
                    (log_fd, "partial output stderr stream"),
                    (artifact_fd, "partial output artifact stream"),
                    (directory_fd, "partial output staging directory"),
                )
            )
        except ExecutorError as close_error:
            failure = append_error(failure, close_error)
        if created:
            try:
                if identity is None:
                    identity = os.stat(
                        lease_name, dir_fd=root_fd, follow_symlinks=False
                    )
                cleanup_staging_lease(
                    root_fd,
                    lease_name,
                    identity.st_dev,
                    identity.st_ino,
                    "partial output staging",
                )
            except (ExecutorError, OSError) as cleanup_error:
                failure = append_error(failure, cleanup_error)
        try:
            close_descriptor(root_fd, "partial output staging root")
        except ExecutorError as close_error:
            failure = append_error(failure, close_error)
        raise failure


def stage_source(profile: Profile, request: Request, lease_name: str) -> SourceSnapshot:
    staging_root = Path(profile.source_staging_root)
    if staging_root.is_symlink() or not staging_root.is_dir():
        fail("source staging root is unsafe")
    store = GitLooseObjectStore.open(profile)
    try:
        commit_tree = store.commit_tree(request.source_commit)
        if commit_tree != request.source_tree:
            fail("source commit/tree binding differs from authenticated object store")
        entries = store.export_tree(request.source_tree)
    finally:
        store.close()
    source_total = sum(len(content) for _, _, _, content in entries)
    if source_total > profile.source_byte_limit:
        fail("Git source bytes exceed protected limit")
    staging_root_fd = open_staging_root(profile, staging_root, "source staging root")
    root_fd = -1
    snapshot_fd = -1
    request_fd = -1
    lease_created = False
    lease_identity: os.stat_result | None = None
    snapshot_name = "source"
    manifest_name = "source.manifest.tsv"
    request_name = "request.tsv"
    lease_path = staging_root / lease_name
    snapshot_path = lease_path / snapshot_name
    try:
        os.mkdir(lease_name, 0o700, dir_fd=staging_root_fd)
        lease_created = True
        os.fsync(staging_root_fd)
        root_fd = os.open(
            lease_name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=staging_root_fd,
        )
        lease_identity = os.fstat(root_fd)
        os.mkdir(snapshot_name, 0o700, dir_fd=root_fd)
        snapshot_fd = os.open(
            snapshot_name,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=root_fd,
        )
        manifest_lines = [
            "source_snapshot_manifest_schema_version\t1",
            f"request_id\t{request.request_id}",
            f"source_commit\t{request.source_commit}",
            f"source_tree\t{request.source_tree}",
            f"file_count\t{len(entries)}",
        ]
        total = 0
        job_found = False
        directories: set[str] = set()
        for index, (path, mode, object_id, content) in enumerate(entries):
            file_size, digest = write_snapshot_file(snapshot_fd, path, mode, content)
            total += file_size
            parent = Path(path).parent
            while str(parent) != ".":
                directories.add(str(parent))
                if len(directories) > MAX_SOURCE_DIRECTORIES:
                    fail("Git source directory count exceeds protected limit")
                parent = parent.parent
            if path == request.job_path:
                if digest != request.job_digest:
                    fail("source job digest differs from immutable source tree")
                job_found = True
            manifest_lines.append(
                f"file\t{index:04d}\t{mode}\t{object_id}\t{path}\t{file_size}\t{digest}"
            )
        if not job_found:
            fail("source job is absent from immutable source tree")
        manifest_lines.append(f"source_bytes\t{total}")
        manifest_raw = ("\n".join(manifest_lines) + "\n").encode("ascii")
        manifest_fd = os.open(
            manifest_name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=root_fd,
        )
        try:
            write_all(manifest_fd, manifest_raw)
            finalize_source_file(manifest_fd, 0o444, "source manifest")
        finally:
            close_descriptor(manifest_fd, "source manifest")
        request_write_fd = os.open(
            request_name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=root_fd,
        )
        try:
            write_all(request_write_fd, request.raw)
            finalize_source_file(request_write_fd, 0o444, "request snapshot")
        finally:
            close_descriptor(request_write_fd, "request snapshot writer")
        request_fd = os.open(
            request_name,
            os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=root_fd,
        )
        finalize_source_directories(snapshot_fd, directories, root_fd)
        identity = os.fstat(snapshot_fd)
        request_identity = os.fstat(request_fd)
        snapshot = SourceSnapshot(
            snapshot_path,
            lease_path / manifest_name,
            lease_path / request_name,
            staging_root_fd,
            root_fd,
            snapshot_fd,
            request_fd,
            identity.st_dev,
            identity.st_ino,
            request_identity.st_dev,
            request_identity.st_ino,
            len(entries),
            total,
            sha256_bytes(manifest_raw),
            lease_name,
            lease_identity.st_dev,
            lease_identity.st_ino,
        )
        verify_retained_snapshot(snapshot)
        return snapshot
    except (ExecutorError, OSError) as error:
        failure = normalized_error(error, "cannot initialize source staging")
        try:
            close_descriptors(
                (
                    (request_fd, "partial request snapshot"),
                    (snapshot_fd, "partial source snapshot"),
                    (root_fd, "partial source staging lease"),
                )
            )
        except ExecutorError as close_error:
            failure = append_error(failure, close_error)
        if lease_created:
            try:
                if lease_identity is None:
                    lease_identity = os.stat(
                        lease_name,
                        dir_fd=staging_root_fd,
                        follow_symlinks=False,
                    )
                cleanup_staging_lease(
                    staging_root_fd,
                    lease_name,
                    lease_identity.st_dev,
                    lease_identity.st_ino,
                    "partial source staging",
                )
            except (ExecutorError, OSError) as cleanup_error:
                failure = append_error(failure, cleanup_error)
        try:
            close_descriptor(staging_root_fd, "partial source staging root")
        except ExecutorError as close_error:
            failure = append_error(failure, close_error)
        raise failure


def external_trust_anchor(args: argparse.Namespace) -> TrustAnchor:
    if (
        not ID_RE.fullmatch(args.policy_identity)
        or not 1 <= args.policy_size <= MAX_FILE_BYTES
        or not SHA256_RE.fullmatch(args.policy_sha256)
        or not 0 <= args.trusted_owner_uid <= 2**31 - 1
        or not 0 <= args.trusted_owner_gid <= 2**31 - 1
    ):
        fail("external policy trust anchor is malformed")
    return TrustAnchor(
        args.policy_identity,
        args.policy_size,
        args.policy_sha256,
        args.trusted_owner_uid,
        args.trusted_owner_gid,
        args.trust_file_contract,
    )


def authorize(args: argparse.Namespace) -> AuthorizedRequest:
    anchor = external_trust_anchor(args)
    root = open_trusted_root(Path(args.trusted_root), anchor)
    try:
        policy = parse_policy(root, args.policy, anchor)
        if args.invocation_mode == "production-operator":
            request = read_operator_request(args.operator_config, args.request_id)
        elif args.invocation_mode == "test":
            request = read_request_file(
                Path(args.request), args.request_owner_uid, args.request_owner_gid
            )
        else:
            fail("unknown OCI executor invocation mode")
        profile = load_profile(root, policy, request.profile_id, anchor)
    finally:
        root.close()
    expected_domain = (
        "production" if args.invocation_mode == "production-operator" else "test"
    )
    if policy.domain != expected_domain or profile.mode != expected_domain:
        fail(f"{args.invocation_mode} cannot authorize {policy.domain} policy data")
    verify_operator_owned(
        Path(profile.runtime_path),
        profile,
        "OCI runtime",
        directory=False,
        allow_root=True,
    )
    verify_operator_owned(
        Path(profile.git_objects_path), profile, "Git object store", directory=True
    )
    verify_operator_owned(
        Path(profile.source_staging_root),
        profile,
        "source staging root",
        directory=True,
    )
    verify_operator_owned(
        Path(profile.output_staging_root),
        profile,
        "output staging root",
        directory=True,
    )
    verify_regular(
        Path(profile.runtime_path),
        str(profile.runtime_size),
        profile.runtime_digest,
        "OCI runtime",
    )
    seccomp = protected_path(root.path, profile.seccomp_path, "seccomp profile")
    verify_regular(
        seccomp,
        str(profile.seccomp_size),
        profile.seccomp_digest,
        "protected seccomp profile",
    )
    verify_operator_owned(seccomp, profile, "seccomp profile", directory=False)
    verify_oci_image(profile)
    queue_trust_digest = (
        args.operator_config.queue_trust_digest
        if args.invocation_mode == "production-operator"
        else args.queue_authorization_sha256
    )
    if not SHA256_RE.fullmatch(queue_trust_digest):
        fail("queue authorization trust digest is malformed")
    authorization_digest = sha256_bytes(
        b"fe2o3-oci-authorization-v1\0"
        + anchor.identity.encode("ascii")
        + b"\0"
        + args.policy.encode("ascii")
        + b"\0"
        + str(anchor.owner_uid).encode("ascii")
        + b":"
        + str(anchor.owner_gid).encode("ascii")
        + b"\0"
        + anchor.file_contract.encode("ascii")
        + b"\0"
        + bytes.fromhex(policy.digest)
        + bytes.fromhex(profile.profile_digest)
        + bytes.fromhex(request.digest)
        + bytes.fromhex(queue_trust_digest)
    )
    return AuthorizedRequest(
        policy,
        profile,
        request,
        resolve_path(seccomp, "protected seccomp profile"),
        authorization_digest,
    )


RUNTIME_VERSION_FORMAT = (
    "client={{.Client.Version}}/{{.Client.ApiVersion}}/{{.Client.GitCommit}}/"
    "{{.Client.Os}}/{{.Client.Arch}} server={{.Server.Version}}/"
    "{{.Server.ApiVersion}}/{{.Server.GitCommit}}/{{.Server.Os}}/{{.Server.Arch}}"
)
RUNTIME_INFO_FORMAT = (
    "id={{.ID}} name={{.Name}} driver={{.Driver}} cgroup={{.CgroupDriver}}/"
    "{{.CgroupVersion}} kernel={{.KernelVersion}} os={{.OperatingSystem}} "
    "type={{.OSType}} arch={{.Architecture}} root={{.DockerRootDir}} "
    "security={{json .SecurityOptions}} version={{.ServerVersion}}"
)


def runtime_output(profile: Profile, arguments: list[str], label: str) -> bytes:
    process = run_bounded(
        [profile.runtime_path, *arguments],
        label=f"OCI runtime {label}",
        environment={"HOME": "/nonexistent", "LC_ALL": "C", "PATH": "/nonexistent"},
        timeout_seconds=15,
        stdout_limit=64 * 1024,
        stderr_limit=64 * 1024,
    )
    if process.returncode != 0:
        detail = process.stderr.decode("ascii", errors="replace").strip()
        if len(detail) > 240:
            detail = detail[:240] + "..."
        fail(f"OCI runtime control plane unavailable for {label}: {detail}")
    if process.stderr or not process.stdout.endswith(b"\n"):
        fail(f"OCI runtime returned non-canonical {label} identity")
    return process.stdout


def observe_runtime(authorized: AuthorizedRequest) -> ObservedRuntimeRequest:
    profile = authorized.profile
    version = runtime_output(
        profile, ["version", "--format", RUNTIME_VERSION_FORMAT], "version"
    )
    info = runtime_output(profile, ["info", "--format", RUNTIME_INFO_FORMAT], "daemon")
    if sha256_bytes(version) != profile.runtime_version_digest:
        fail("OCI runtime version differs from protected profile")
    if sha256_bytes(info) != profile.runtime_info_digest:
        fail("OCI runtime daemon identity differs from protected profile")
    machine_id = Path("/etc/machine-id")
    kernel_notes = Path("/sys/kernel/notes")
    driver = Path(profile.driver_path)
    if sha256_file(machine_id) != profile.machine_id_digest:
        fail("host machine identity differs from protected profile")
    if os.uname().release != profile.kernel_release:
        fail("host kernel release differs from protected profile")
    if sha256_file(kernel_notes) != profile.kernel_notes_digest:
        fail("host kernel build identity differs from protected profile")
    try:
        driver_size = driver.stat().st_size
    except OSError:
        fail("AMDGPU module is missing")
    verify_regular(driver, str(driver_size), profile.driver_digest, "AMDGPU module")
    for device in profile.devices:
        path = Path(device.path)
        try:
            info = path.lstat()
        except OSError:
            fail(f"required device is missing: {device.path}")
        if (
            path.is_symlink()
            or not stat.S_ISCHR(info.st_mode)
            or os.major(info.st_rdev) != device.major
            or os.minor(info.st_rdev) != device.minor
            or info.st_gid != profile.supplemental_gid
        ):
            fail(f"device identity differs from protected profile: {device.path}")
    render = next(device for device in profile.devices if device.path != "/dev/kfd")
    render_name = Path(render.path).name
    uevent_path = Path("/sys/class/drm") / render_name / "device" / "uevent"
    unique_id_path = Path("/sys/class/drm") / render_name / "device" / "unique_id"
    try:
        uevent = dict(
            line.split("=", 1)
            for line in uevent_path.read_text(encoding="ascii").splitlines()
            if "=" in line
        )
        unique_id = unique_id_path.read_text(encoding="ascii").strip()
    except (OSError, UnicodeDecodeError, ValueError):
        fail("cannot establish exact GPU sysfs identity")
    if (
        uevent.get("DRIVER") != "amdgpu"
        or uevent.get("PCI_SLOT_NAME", "").lower() != profile.gpu_pci_slot
        or uevent.get("PCI_ID") != profile.gpu_pci_id
        or unique_id != profile.gpu_unique_id
    ):
        fail("GPU identity differs from protected profile")
    return ObservedRuntimeRequest(authorized, verify_runtime_image(profile))


def verify_runtime_image(profile: Profile) -> tuple[str, ...]:
    diff_ids = verify_oci_image(profile)
    raw = runtime_output(
        profile,
        ["image", "inspect", "--format", "{{json .}}", profile.image_reference],
        "image",
    )
    inspected = strict_json_object(raw, "OCI runtime image")
    rootfs = inspected.get("RootFS")
    config = inspected.get("Config")
    repo_digests = inspected.get("RepoDigests")
    if (
        inspected.get("Id") != profile.config.digest
        or type(repo_digests) is not list
        or any(type(value) is not str for value in repo_digests)
        or profile.image_reference not in repo_digests
        or inspected.get("Os") != "linux"
        or inspected.get("Architecture") != "amd64"
    ):
        fail("runtime image differs from protected OCI image")
    validate_runtime_rootfs(rootfs, diff_ids)
    validate_image_config(config, "runtime image config")
    return diff_ids


def docker_create_arguments(
    profile: Profile,
    request: Request,
    source_snapshot: Path,
    request_path: Path,
    seccomp_path: Path,
) -> list[str]:
    name = f"fe2o3-evidence-{request.request_id}"
    arguments = [
        profile.runtime_path,
        "create",
        "--pull=never",
        "--name",
        name,
        "--hostname",
        "fe2o3-evidence",
        "--label",
        f"org.fe2o3.evidence.request-id={request.request_id}",
        "--label",
        f"org.fe2o3.evidence.profile-sha256={profile.profile_digest}",
        "--label",
        f"org.fe2o3.evidence.source-tree={request.source_tree}",
        "--network",
        "none",
        "--no-healthcheck",
        "--cgroupns=private",
        "--read-only",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges=true",
        "--security-opt",
        f"seccomp={seccomp_path}",
        "--ipc",
        "private",
        "--pid",
        "private",
        "--uts",
        "private",
        "--log-driver",
        "none",
        "--pids-limit",
        str(profile.pids_limit),
        "--memory",
        str(profile.memory_limit),
        "--shm-size",
        str(profile.shm_limit),
        "--cpus",
        f"{profile.cpu_limit_milli / 1000:.3f}",
        "--user",
        f"{profile.uid}:{profile.gid}",
        "--group-add",
        str(profile.supplemental_gid),
        "--workdir",
        profile.source_mount,
    ]
    for name, value in profile.environment:
        arguments.extend(("--env", f"{name}={value}"))
    arguments.extend(
        (
            "--mount",
            f"type=bind,src={source_snapshot},dst={profile.source_mount},readonly,bind-recursive=readonly",
            "--mount",
            f"type=bind,src={request_path},dst={profile.request_mount},readonly",
            "--tmpfs",
            f"{profile.output_mount}:rw,nosuid,nodev,size={profile.output_limit}",
            "--tmpfs",
            f"{profile.tmp_mount}:rw,nosuid,nodev,noexec,size={profile.tmp_limit}",
        )
    )
    for device in profile.devices:
        arguments.extend(("--device", f"{device.path}:{device.path}:{device.access}"))
    arguments.extend(("--entrypoint", profile.entrypoint[0], profile.image_reference))
    arguments.extend(profile.command)
    return arguments


def invocation_digest(arguments: list[str]) -> str:
    digest = hashlib.sha256(b"fe2o3-oci-create-arguments-v1\0")
    for argument in arguments:
        try:
            encoded = argument.encode("ascii")
        except UnicodeEncodeError:
            fail("OCI invocation argument is not ASCII")
        digest.update(struct.pack(">Q", len(encoded)))
        digest.update(encoded)
    return digest.hexdigest()


def require_lease_absent(root: Path, lease_name: str, label: str) -> None:
    root_fd = -1
    try:
        root_fd = os.open(
            root,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        try:
            os.stat(lease_name, dir_fd=root_fd, follow_symlinks=False)
        except FileNotFoundError:
            return
        fail(f"{label} lease still exists before plan output")
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot establish {label} lease cleanup before plan output: {error}")
    finally:
        if root_fd >= 0:
            close_descriptor(root_fd, f"{label} plan-output check")


def emit_plan_output(
    lines: list[str],
    lease_contracts: tuple[tuple[Path, str, str], ...],
) -> None:
    if not lines or any("\n" in line or "\r" in line for line in lines):
        fail("invalid bounded plan output")
    for root, lease_name, label in lease_contracts:
        require_lease_absent(root, lease_name, label)
    raw = ("\n".join(lines) + "\n").encode("ascii")
    if len(raw) > MAX_FILE_BYTES:
        fail("plan output exceeds protected limit")
    try:
        sys.stdout.write(raw.decode("ascii"))
        sys.stdout.flush()
    except OSError as error:
        fail(f"cannot emit cleaned OCI plan: {error}")


def command_plan(args: argparse.Namespace) -> None:
    authorized = authorize(args)
    profile = authorized.profile
    request = authorized.request
    lease_name = staging_lease_name(authorized.authorization_digest)
    snapshot = stage_source(profile, request, lease_name)
    output: OutputStage | None = None
    plan_lines: list[str] | None = None
    try:
        output = stage_output(profile, lease_name)
        arguments = docker_create_arguments(
            profile,
            request,
            snapshot.path,
            snapshot.request_path,
            authorized.seccomp_path,
        )
        verify_retained_snapshot(snapshot)
        verify_retained_output(output)
        plan_lines = [
            "oci_execution_plan_schema_version\t2",
            "authorization_state\t"
            + (
                "operator-policy-matched"
                if args.invocation_mode == "production-operator"
                else "test-non-authoritative"
            ),
            f"policy_identity\t{args.policy_identity}",
            f"authorization_sha256\t{authorized.authorization_digest}",
            f"profile_id\t{profile.profile_id}",
            f"profile_sha256\t{profile.profile_digest}",
            f"request_id\t{request.request_id}",
            f"request_sha256\t{request.digest}",
            f"source_commit\t{request.source_commit}",
            f"source_tree\t{request.source_tree}",
            f"source_manifest_sha256\t{snapshot.manifest_digest}",
            f"source_file_count\t{snapshot.file_count}",
            f"source_bytes\t{snapshot.byte_count}",
            f"artifact_stream_protocol\t{profile.artifact_stream_protocol}",
            f"artifact_stream_limit\t{profile.output_limit}",
            f"stderr_stream_limit\t{profile.log_limit}",
            f"argument_count\t{len(arguments)}",
            f"invocation_sha256\t{invocation_digest(arguments)}",
        ]
    finally:
        cleanup_failures: list[str] = []
        if output is not None:
            try:
                output.cleanup()
            except ExecutorError as error:
                cleanup_failures.append(str(error))
        try:
            snapshot.cleanup()
        except ExecutorError as error:
            cleanup_failures.append(str(error))
        if cleanup_failures:
            fail("plan staging cleanup failed: " + "; ".join(cleanup_failures))
    assert plan_lines is not None
    emit_plan_output(
        plan_lines,
        (
            (Path(profile.source_staging_root), lease_name, "source staging"),
            (Path(profile.output_staging_root), lease_name, "output staging"),
        ),
    )


def command_preflight(args: argparse.Namespace) -> None:
    observed = observe_runtime(authorize(args))
    profile = observed.authorized.profile
    request = observed.authorized.request
    print(f"matched_profile\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print("observational_preflight\tpassed")
    print(
        "authorization_state\t"
        + (
            "operator-policy-matched"
            if args.invocation_mode == "production-operator"
            else "test-non-authoritative"
        )
    )
    print("execution_receipt\tnot-issued")


def command_verify(args: argparse.Namespace) -> None:
    authorized = authorize(args)
    profile = authorized.profile
    request = authorized.request
    print(f"matched_profile\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print(f"source_tree\t{request.source_tree}")
    print(
        "authorization_state\t"
        + (
            "operator-policy-matched"
            if args.invocation_mode == "production-operator"
            else "test-non-authoritative"
        )
    )


def add_test_authorization_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--test-request", dest="request", required=True)
    parser.add_argument(
        "--test-request-owner-uid", dest="request_owner_uid", required=True, type=int
    )
    parser.add_argument(
        "--test-request-owner-gid", dest="request_owner_gid", required=True, type=int
    )
    parser.add_argument(
        "--test-queue-trust-sha256",
        dest="queue_authorization_sha256",
        required=True,
    )
    parser.add_argument("--test-trusted-root", dest="trusted_root", required=True)
    parser.add_argument(
        "--test-policy",
        dest="policy",
        required=True,
        help="test-only relative policy path",
    )
    parser.add_argument("--test-policy-identity", dest="policy_identity", required=True)
    parser.add_argument(
        "--test-policy-size", dest="policy_size", required=True, type=int
    )
    parser.add_argument("--test-policy-sha256", dest="policy_sha256", required=True)
    parser.add_argument(
        "--test-trusted-owner-uid",
        dest="trusted_owner_uid",
        required=True,
        type=int,
    )
    parser.add_argument(
        "--test-trusted-owner-gid",
        dest="trusted_owner_gid",
        required=True,
        type=int,
    )
    parser.add_argument(
        "--test-trust-file-contract",
        dest="trust_file_contract",
        required=True,
        choices=("descriptor-stable",),
    )
    parser.set_defaults(invocation_mode="test")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify = subparsers.add_parser(
        "test-verify", help="perform non-authoritative test input validation"
    )
    add_test_authorization_arguments(verify)
    verify.set_defaults(func=command_verify)
    plan = subparsers.add_parser(
        "test-plan", help="render a non-authoritative ephemeral test plan"
    )
    add_test_authorization_arguments(plan)
    plan.set_defaults(func=command_plan)
    preflight = subparsers.add_parser(
        "test-preflight", help="perform non-authoritative runtime observations"
    )
    add_test_authorization_arguments(preflight)
    preflight.set_defaults(func=command_preflight)
    return parser


def verify_installed_operator_entrypoint() -> None:
    if (
        Path(os.path.abspath(sys.argv[0])) != OPERATOR_EXECUTOR_PATH
        or Path(os.path.abspath(__file__)) != OPERATOR_EXECUTOR_PATH
        or Path(os.path.abspath(sys.executable)) != OPERATOR_INTERPRETER_PATH
        or os.getcwd() != "/"
        or dict(os.environ) != OPERATOR_ENVIRONMENT
        or not sys.flags.isolated
        or not sys.flags.no_site
        or not sys.flags.ignore_environment
        or not sys.flags.no_user_site
        or any(
            not entry
            or not Path(entry).is_absolute()
            or OPERATOR_PYTHON_ROOT not in Path(entry).parents
            for entry in sys.path
        )
    ):
        fail("production operator startup state is not the fixed isolated contract")
    for path, label in (
        (OPERATOR_LAUNCHER_PATH, "operator launcher"),
        (OPERATOR_INTERPRETER_PATH, "operator interpreter"),
        (OPERATOR_EXECUTOR_PATH, "operator executor"),
    ):
        file_fd = -1
        try:
            file_fd = os.open(
                path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC
            )
            info = os.fstat(file_fd)
            if (
                not stat.S_ISREG(info.st_mode)
                or info.st_nlink != 1
                or (info.st_uid, info.st_gid) != (0, 0)
                or info.st_mode & 0o022
            ):
                fail(f"fixed {label} ownership, mode, or link contract is unsafe")
            verify_descriptor_immutable(file_fd, f"fixed {label}", True)
        except ExecutorError:
            raise
        except OSError as error:
            fail(f"cannot establish fixed {label}: {error}")
        finally:
            if file_fd >= 0:
                close_descriptor(file_fd, f"fixed {label}")
    launcher_fd = -1
    parent_fd = -1
    try:
        launcher_fd = os.open(
            OPERATOR_LAUNCHER_PATH,
            os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        parent_fd = os.open(
            f"/proc/{os.getppid()}/exe", os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC
        )
        launcher_info = os.fstat(launcher_fd)
        parent_info = os.fstat(parent_fd)
        if (launcher_info.st_dev, launcher_info.st_ino) != (
            parent_info.st_dev,
            parent_info.st_ino,
        ):
            fail("production operator parent is not the fixed native launcher")
    except ExecutorError:
        raise
    except OSError as error:
        fail(f"cannot establish fixed native launcher parent: {error}")
    finally:
        close_descriptors(
            (
                (parent_fd, "native launcher parent"),
                (launcher_fd, "fixed operator launcher"),
            )
        )


def operator_namespace(
    config: OperatorConfig,
    request_id: str,
    func: object,
) -> argparse.Namespace:
    return argparse.Namespace(
        invocation_mode="production-operator",
        operator_config=config,
        request_id=request_id,
        trusted_root=config.trusted_root,
        policy=config.policy_path,
        policy_identity=config.policy_identity,
        policy_size=config.policy_size,
        policy_sha256=config.policy_digest,
        trusted_owner_uid=config.trusted_owner_uid,
        trusted_owner_gid=config.trusted_owner_gid,
        trust_file_contract=config.trust_file_contract,
        func=func,
    )


def report_controlled_error(prefix: str, error: BaseException) -> int:
    detail = " ".join(str(error).splitlines())
    if len(detail) > 2048:
        detail = detail[:2048] + "..."
    raw = f"{prefix}: {detail}\n"
    try:
        sys.stderr.write(raw)
        sys.stderr.flush()
    except OSError:
        try:
            os.write(2, raw.encode("ascii", errors="replace"))
        except OSError:
            pass
    return 2


def operator_main(argv: list[str] | None = None) -> int:
    try:
        verify_installed_operator_entrypoint()
        parser = argparse.ArgumentParser(
            description="Fixed production OCI evidence operator entrypoint"
        )
        parser.add_argument("command", choices=("verify", "plan", "preflight"))
        parser.add_argument("--request-id", required=True)
        selected = parser.parse_args(argv)
        config = load_operator_config()
        function = {
            "verify": command_verify,
            "plan": command_plan,
            "preflight": command_preflight,
        }[selected.command]
        args = operator_namespace(config, selected.request_id, function)
        function(args)
        return 0
    except (ExecutorError, OSError) as error:
        return report_controlled_error("fe2o3-oci-operator", error)


def main() -> int:
    try:
        parser = build_parser()
        args = parser.parse_args()
        args.func(args)
        return 0
    except (ExecutorError, OSError) as error:
        return report_controlled_error("parity-oci-executor", error)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--operator-internal"]:
        raise SystemExit(operator_main(sys.argv[2:]))
    raise SystemExit(main())
