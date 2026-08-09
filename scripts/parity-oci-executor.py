#!/usr/bin/env python3
"""Protected-policy OCI executor for promotable MI300X evidence.

The candidate supplies an execution request.  A protected policy selects the
only admissible executor profile; the request cannot assert its own closure
state or substitute image, runtime, device, or isolation settings.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import threading
import time
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
                process.stdin.write(input_data)
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
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
    for reader in readers:
        reader.join(timeout=5)
    if writer is not None:
        writer.join(timeout=5)
    if any(reader.is_alive() for reader in readers) or (writer and writer.is_alive()):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        for reader in readers:
            reader.join(timeout=1)
        if writer is not None:
            writer.join(timeout=1)
        fail(f"bounded subprocess pipe did not close for {label}")
    if overflow:
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


def read_rows(path: Path, label: str) -> tuple[bytes, list[list[str]]]:
    try:
        raw = path.read_bytes()
    except OSError as error:
        fail(f"cannot read {label}: {error}")
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
    return raw, rows


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


def protected_path(root: Path, relative: str, label: str) -> Path:
    if not valid_relative(relative):
        fail(f"invalid protected {label} path")
    try:
        root = root.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve protected root: {error}")
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
    if root not in current.resolve(strict=True).parents:
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
            parent_info = current.lstat()
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


def parse_policy(root: Path, relative: str) -> Policy:
    path = protected_path(root, relative, "OCI executor policy")
    _, rows = read_rows(path, "OCI executor policy")
    cursor = Cursor(rows, "OCI executor policy")
    if cursor.scalar("oci_executor_policy_schema_version") != "1":
        fail("OCI executor policy schema must be 1")
    domain = cursor.scalar("trust_domain")
    if domain not in ("production", "test"):
        fail("invalid OCI executor policy trust domain")
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
    return Policy(domain, profiles)


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
    git_path: str
    git_size: int
    git_digest: str
    git_version_digest: str
    git_objects_path: str
    source_staging_root: str
    output_staging_root: str
    artifact_stream_protocol: str
    source_file_limit: int
    source_byte_limit: int
    source_index_limit: int
    source_export_timeout: int
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


def load_profile(root: Path, policy: Policy, profile_id: str) -> Profile:
    entry = policy.profiles.get(profile_id)
    if entry is None:
        fail("executor profile is not authorized by protected policy")
    path = protected_path(root, entry.relative_path, "OCI executor profile")
    verify_regular(
        path, str(entry.size), entry.digest, "protected OCI executor profile"
    )
    raw, rows = read_rows(path, "OCI executor profile")
    if sha256_bytes(raw) != entry.digest:
        fail("protected OCI executor profile digest mismatch")
    cursor = Cursor(rows, "OCI executor profile")
    if cursor.scalar("oci_executor_profile_schema_version") != "1":
        fail("OCI executor profile schema must be 1")
    actual_id = cursor.scalar("profile_id")
    mode = cursor.scalar("execution_mode")
    target = cursor.scalar("target")
    lane = cursor.scalar("hardware_lane")
    runtime_path = cursor.scalar("runtime_path")
    runtime_size = cursor.scalar("runtime_size")
    runtime_digest = cursor.scalar("runtime_sha256")
    runtime_version_digest = cursor.scalar("runtime_version_sha256")
    runtime_info_digest = cursor.scalar("runtime_info_sha256")
    git_path = cursor.scalar("git_path")
    git_size = cursor.scalar("git_size")
    git_digest = cursor.scalar("git_sha256")
    git_version_digest = cursor.scalar("git_version_sha256")
    git_objects_path = cursor.scalar("git_objects_path")
    source_staging_root = cursor.scalar("source_staging_root")
    output_staging_root = cursor.scalar("output_staging_root")
    artifact_stream_protocol = cursor.scalar("artifact_stream_protocol")
    source_file_limit = cursor.scalar("source_file_limit")
    source_byte_limit = cursor.scalar("source_byte_limit")
    source_index_limit = cursor.scalar("source_index_limit")
    source_export_timeout = cursor.scalar("source_export_timeout_seconds")
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
        not valid_absolute(git_path)
        or not git_size.isdigit()
        or not 1 <= int(git_size) <= 1024**3
        or not SHA256_RE.fullmatch(git_digest)
        or not SHA256_RE.fullmatch(git_version_digest)
        or not valid_absolute(git_objects_path)
        or not valid_absolute(source_staging_root)
        or not valid_absolute(output_staging_root)
        or artifact_stream_protocol != "fe2o3-artifact-stream-v1"
        or not source_file_limit.isdigit()
        or not 1 <= int(source_file_limit) <= 16384
        or not source_byte_limit.isdigit()
        or not 1 <= int(source_byte_limit) <= 512 * 1024**2
        or not source_index_limit.isdigit()
        or not 1 <= int(source_index_limit) <= 64 * 1024**2
        or not source_export_timeout.isdigit()
        or not 1 <= int(source_export_timeout) <= 900
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
        git_path,
        int(git_size),
        git_digest,
        git_version_digest,
        git_objects_path,
        source_staging_root,
        output_staging_root,
        artifact_stream_protocol,
        int(source_file_limit),
        int(source_byte_limit),
        int(source_index_limit),
        int(source_export_timeout),
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
    if layout not in current.resolve(strict=True).parents:
        fail(f"{label} escapes OCI layout")
    return current


def read_json_bound(path: Path, binding: Layer, label: str) -> dict[str, object]:
    verify_regular(
        path, str(binding.size), binding.digest.removeprefix("sha256:"), label
    )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        fail(f"invalid {label} JSON")
    if not isinstance(value, dict):
        fail(f"invalid {label} JSON object")
    validate_json_shape(value, label)
    return value


def read_json_file(
    path: Path,
    label: str,
    *,
    maximum_bytes: int,
    expected_size: int | None = None,
    expected_digest: str | None = None,
) -> dict[str, object]:
    try:
        info = path.lstat()
        raw = path.read_bytes()
    except OSError:
        fail(f"cannot read {label}")
    if (
        path.is_symlink()
        or not stat.S_ISREG(info.st_mode)
        or info.st_nlink != 1
        or not 1 <= info.st_size <= maximum_bytes
        or len(raw) != info.st_size
        or expected_size is not None
        and info.st_size != expected_size
        or expected_digest is not None
        and sha256_bytes(raw) != expected_digest
    ):
        fail(f"{label} binding or size is invalid")
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError):
        fail(f"invalid {label} JSON")
    if not isinstance(value, dict):
        fail(f"invalid {label} JSON object")
    validate_json_shape(value, label)
    return value


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
        elif item is not None and not isinstance(item, (bool, int, float)):
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
        or not isinstance(size, int)
        or size < 0
        or size > MAX_OCI_LAYER_BYTES
    ):
        fail(f"invalid OCI {label} descriptor")
    return Layer(digest, size)


def verify_oci_image(profile: Profile) -> tuple[str, ...]:
    layout = Path(profile.layout_path)
    if (
        not layout.is_absolute()
        or layout.is_symlink()
        or not layout.is_dir()
        or layout.resolve(strict=True) != layout
    ):
        fail("OCI layout is unavailable or unsafe")
    verify_operator_owned(layout, profile, "OCI layout", directory=True)
    layout_marker = require_safe_oci_entry(layout, ("oci-layout",), "OCI layout marker")
    index_path = require_safe_oci_entry(layout, ("index.json",), "OCI index")
    require_safe_oci_entry(layout, ("blobs",), "OCI blob directory", directory=True)
    require_safe_oci_entry(
        layout, ("blobs", "sha256"), "OCI SHA-256 directory", directory=True
    )
    verify_operator_owned(layout_marker, profile, "OCI layout marker", directory=False)
    verify_operator_owned(index_path, profile, "OCI index", directory=False)
    marker = read_json_file(layout_marker, "OCI layout marker", maximum_bytes=1024)
    if marker != {"imageLayoutVersion": "1.0.0"}:
        fail("unsupported OCI layout version")
    index = read_json_file(
        index_path,
        "OCI index",
        maximum_bytes=MAX_OCI_METADATA_BYTES,
        expected_size=profile.index_size,
        expected_digest=profile.index_digest,
    )
    manifests = index.get("manifests") if isinstance(index, dict) else None
    if not isinstance(manifests, list):
        fail("invalid OCI index manifests")
    selected = [descriptor(value, "manifest") for value in manifests]
    if selected.count(profile.manifest) != 1:
        fail("protected OCI manifest is absent or ambiguous")
    manifest_path = require_safe_oci_entry(
        layout,
        ("blobs", "sha256", profile.manifest.digest.removeprefix("sha256:")),
        "OCI manifest",
    )
    verify_operator_owned(manifest_path, profile, "OCI manifest", directory=False)
    manifest = read_json_bound(manifest_path, profile.manifest, "OCI manifest")
    if descriptor(manifest.get("config"), "config") != profile.config:
        fail("OCI config differs from protected profile")
    layer_values = manifest.get("layers")
    if (
        not isinstance(layer_values, list)
        or tuple(descriptor(value, "layer") for value in layer_values) != profile.layers
    ):
        fail("OCI layers differ from protected profile")
    config_path = require_safe_oci_entry(
        layout,
        ("blobs", "sha256", profile.config.digest.removeprefix("sha256:")),
        "OCI config",
    )
    verify_operator_owned(config_path, profile, "OCI config", directory=False)
    config = read_json_bound(config_path, profile.config, "OCI config")
    rootfs = config.get("rootfs")
    diff_ids = rootfs.get("diff_ids") if isinstance(rootfs, dict) else None
    if (
        rootfs is None
        or rootfs.get("type") != "layers"
        or not isinstance(diff_ids, list)
    ):
        fail("OCI config lacks a layer rootfs")
    if any(
        not isinstance(item, str) or OCI_DIGEST_RE.fullmatch(item) is None
        for item in diff_ids
    ):
        fail("OCI config has malformed rootfs diff IDs")
    if len(diff_ids) != len(profile.layers):
        fail("OCI config/layer count mismatch")
    if config.get("architecture") != "amd64" or config.get("os") != "linux":
        fail("OCI image platform must be linux/amd64")
    image_config = config.get("config")
    if not isinstance(image_config, dict) or image_config.get("Env") not in (None, []):
        fail("OCI image must not supply inherited environment")
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
    return tuple(diff_ids)


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


@dataclass(frozen=True)
class ObservedRuntimeRequest:
    """Authorized inputs whose bounded host/runtime observations matched policy."""

    authorized: AuthorizedRequest
    image_diff_ids: tuple[str, ...]


def parse_request(path: Path) -> Request:
    raw, rows = read_rows(path, "OCI execution request")
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


@dataclass
class SourceSnapshot:
    path: Path
    manifest_path: Path
    request_path: Path
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

    def close(self) -> None:
        os.close(self.request_fd)
        os.close(self.directory_fd)
        os.close(self.root_fd)


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

    def close(self) -> None:
        os.close(self.log_fd)
        os.close(self.artifact_fd)
        os.close(self.directory_fd)
        os.close(self.root_fd)


def git_environment(profile: Profile, control: Path) -> dict[str, str]:
    return {
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_OBJECT_DIRECTORY": profile.git_objects_path,
        "GIT_ALTERNATE_OBJECT_DIRECTORIES": "",
        "GIT_DIR": str(control),
        "GIT_WORK_TREE": "/nonexistent",
        "HOME": "/nonexistent",
        "LC_ALL": "C",
        "PATH": "/nonexistent",
        "XDG_CONFIG_HOME": "/nonexistent",
    }


def git_object_command(
    profile: Profile,
    control: Path,
    arguments: list[str],
    *,
    label: str,
    stdout_limit: int,
    input_data: bytes | None = None,
) -> bytes:
    result = run_bounded(
        [
            profile.git_path,
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            *arguments,
        ],
        label=label,
        environment=git_environment(profile, control),
        timeout_seconds=profile.source_export_timeout,
        stdout_limit=stdout_limit,
        stderr_limit=64 * 1024,
        input_data=input_data,
    )
    if result.returncode or result.stderr:
        fail(f"{label} failed")
    return result.stdout


def parse_tree_index(profile: Profile, raw: bytes) -> list[tuple[str, str, str]]:
    records = raw.split(b"\0")
    if records[-1] != b"":
        fail("Git tree index is not NUL terminated")
    records.pop()
    if not 1 <= len(records) <= profile.source_file_limit:
        fail("Git tree file count exceeds protected limit")
    output: list[tuple[str, str, str]] = []
    previous = ""
    for record in records:
        try:
            identity, raw_path = record.split(b"\t", 1)
            mode, object_type, object_id = identity.decode("ascii").split(" ")
            path = raw_path.decode("ascii")
        except (ValueError, UnicodeDecodeError):
            fail("Git tree index contains a malformed entry")
        if (
            mode not in ("100644", "100755")
            or object_type != "blob"
            or not COMMIT_RE.fullmatch(object_id)
            or not valid_relative(path)
            or any(component == ".git" for component in path.split("/"))
            or path <= previous
        ):
            fail("Git tree contains an unsupported or unsorted entry")
        output.append((path, mode, object_id))
        previous = path
    return output


def parse_blob_batch(
    profile: Profile, entries: list[tuple[str, str, str]], raw: bytes
) -> list[bytes]:
    position = 0
    total = 0
    output: list[bytes] = []
    for _, _, expected_object in entries:
        end = raw.find(b"\n", position, min(len(raw), position + 256))
        if end < 0:
            fail("Git blob batch has a malformed header")
        try:
            object_id, object_type, size_text = (
                raw[position:end].decode("ascii").split(" ")
            )
        except (ValueError, UnicodeDecodeError):
            fail("Git blob batch has a malformed header")
        if (
            object_id != expected_object
            or object_type != "blob"
            or not size_text.isdigit()
            or int(size_text) > profile.source_byte_limit
        ):
            fail("Git blob batch identity differs from tree index")
        size = int(size_text)
        position = end + 1
        blob_end = position + size
        if blob_end >= len(raw) or raw[blob_end : blob_end + 1] != b"\n":
            fail("Git blob batch content is truncated")
        output.append(raw[position:blob_end])
        total += size
        if total > profile.source_byte_limit:
            fail("Git source bytes exceed protected limit")
        position = blob_end + 1
    if position != len(raw):
        fail("Git blob batch has trailing output")
    return output


def open_child_directory(parent_fd: int, component: str) -> int:
    try:
        os.mkdir(component, 0o700, dir_fd=parent_fd)
    except FileExistsError:
        pass
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
    while view:
        written = os.write(file_fd, view)
        if written <= 0:
            fail("staging write made no progress")
        view = view[written:]


def write_snapshot_file(
    snapshot_fd: int, path: str, mode: str, content: bytes
) -> tuple[int, str]:
    components = path.split("/")
    parent_fd = os.dup(snapshot_fd)
    try:
        for component in components[:-1]:
            next_fd = open_child_directory(parent_fd, component)
            os.close(parent_fd)
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
            os.fsync(file_fd)
            os.fchmod(file_fd, 0o555 if mode == "100755" else 0o444)
        finally:
            os.close(file_fd)
    finally:
        os.close(parent_fd)
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
        != (os.fstat(stage.artifact_fd).st_dev, os.fstat(stage.artifact_fd).st_ino)
        or (log.st_dev, log.st_ino)
        != (os.fstat(stage.log_fd).st_dev, os.fstat(stage.log_fd).st_ino)
    ):
        fail("retained output staging path was replaced")


def stage_output(profile: Profile, request: Request) -> OutputStage:
    root = Path(profile.output_staging_root)
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    name = f"execution-{request.request_id}"
    try:
        os.mkdir(name, 0o700, dir_fd=root_fd)
    except FileExistsError:
        os.close(root_fd)
        fail("output staging identity already exists")
    directory_fd = os.open(
        name,
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        dir_fd=root_fd,
    )
    artifact_fd = -1
    log_fd = -1
    try:
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
        os.fsync(directory_fd)
        identity = os.fstat(directory_fd)
        stage = OutputStage(
            root / name,
            root / name / "artifacts.stream",
            root / name / "stderr.log",
            root_fd,
            directory_fd,
            artifact_fd,
            log_fd,
            identity.st_dev,
            identity.st_ino,
        )
        verify_retained_output(stage)
        return stage
    except Exception:
        if log_fd >= 0:
            os.close(log_fd)
        if artifact_fd >= 0:
            os.close(artifact_fd)
        os.close(directory_fd)
        os.close(root_fd)
        raise


def stage_source(profile: Profile, request: Request) -> SourceSnapshot:
    verify_regular(
        Path(profile.git_path),
        str(profile.git_size),
        profile.git_digest,
        "Git executable",
    )
    version = run_bounded(
        [profile.git_path, "--version"],
        label="Git version",
        environment={"HOME": "/nonexistent", "LC_ALL": "C", "PATH": "/nonexistent"},
        timeout_seconds=15,
        stdout_limit=4096,
        stderr_limit=4096,
    )
    if (
        version.returncode
        or version.stderr
        or sha256_bytes(version.stdout) != profile.git_version_digest
    ):
        fail("Git executable version differs from protected profile")
    objects = Path(profile.git_objects_path)
    staging_root = Path(profile.source_staging_root)
    if (
        objects.is_symlink()
        or not objects.is_dir()
        or staging_root.is_symlink()
        or not staging_root.is_dir()
    ):
        fail("immutable source object or staging root is unsafe")
    root_fd = os.open(
        staging_root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    )
    snapshot_name = f"source-{request.request_id}"
    manifest_name = f"source-{request.request_id}.manifest.tsv"
    request_name = f"request-{request.request_id}.tsv"
    try:
        os.mkdir(snapshot_name, 0o700, dir_fd=root_fd)
    except FileExistsError:
        os.close(root_fd)
        fail("source snapshot identity already exists")
    snapshot_fd = os.open(
        snapshot_name,
        os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        dir_fd=root_fd,
    )
    request_fd = -1
    snapshot_path = staging_root / snapshot_name
    control = Path(tempfile.mkdtemp(prefix="git-control-", dir=staging_root))
    try:
        (control / "objects" / "info").mkdir(mode=0o700, parents=True)
        (control / "objects" / "pack").mkdir(mode=0o700)
        (control / "refs" / "heads").mkdir(mode=0o700, parents=True)
        (control / "refs" / "tags").mkdir(mode=0o700)
        (control / "HEAD").write_text("ref: refs/heads/invalid\n", encoding="ascii")
        (control / "config").write_text(
            "[core]\n\trepositoryformatversion = 0\n\tbare = true\n",
            encoding="ascii",
        )
        commit = git_object_command(
            profile,
            control,
            ["cat-file", "commit", request.source_commit],
            label="Git commit read",
            stdout_limit=MAX_FILE_BYTES,
        )
        first_line = commit.split(b"\n", 1)[0]
        if first_line != f"tree {request.source_tree}".encode("ascii"):
            fail("source commit/tree binding differs from immutable object store")
        raw_index = git_object_command(
            profile,
            control,
            ["ls-tree", "-rz", "--full-tree", "-r", request.source_tree],
            label="Git tree index",
            stdout_limit=profile.source_index_limit,
        )
        entries = parse_tree_index(profile, raw_index)
        batch_input = b"".join(
            f"{object_id}\n".encode("ascii") for _, _, object_id in entries
        )
        raw_blobs = git_object_command(
            profile,
            control,
            ["cat-file", "--batch"],
            label="Git blob batch",
            stdout_limit=profile.source_byte_limit + len(entries) * 128,
            input_data=batch_input,
        )
        blobs = parse_blob_batch(profile, entries, raw_blobs)
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
        for index, ((path, mode, object_id), content) in enumerate(
            zip(entries, blobs, strict=True)
        ):
            file_size, digest = write_snapshot_file(snapshot_fd, path, mode, content)
            total += file_size
            parent = Path(path).parent
            while str(parent) != ".":
                directories.add(str(parent))
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
            os.fsync(manifest_fd)
            os.fchmod(manifest_fd, 0o444)
        finally:
            os.close(manifest_fd)
        request_write_fd = os.open(
            request_name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
            0o600,
            dir_fd=root_fd,
        )
        try:
            write_all(request_write_fd, request.raw)
            os.fsync(request_write_fd)
            os.fchmod(request_write_fd, 0o444)
        finally:
            os.close(request_write_fd)
        request_fd = os.open(
            request_name,
            os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC,
            dir_fd=root_fd,
        )
        for directory in sorted(
            directories, key=lambda item: item.count("/"), reverse=True
        ):
            os.chmod(directory, 0o555, dir_fd=snapshot_fd, follow_symlinks=False)
        os.fchmod(snapshot_fd, 0o555)
        os.fsync(snapshot_fd)
        os.fsync(root_fd)
        identity = os.fstat(snapshot_fd)
        request_identity = os.fstat(request_fd)
        snapshot = SourceSnapshot(
            snapshot_path,
            staging_root / manifest_name,
            staging_root / request_name,
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
        )
        verify_retained_snapshot(snapshot)
        return snapshot
    except Exception:
        if request_fd >= 0:
            os.close(request_fd)
        os.close(snapshot_fd)
        os.close(root_fd)
        raise
    finally:
        shutil.rmtree(control, ignore_errors=True)


def authorize(args: argparse.Namespace) -> AuthorizedRequest:
    root = Path(args.trusted_root)
    policy = parse_policy(root, args.policy)
    request_path = Path(args.request)
    if not request_path.is_absolute() or not valid_absolute(str(request_path)):
        fail("OCI execution request path must be canonical absolute")
    try:
        request_info = request_path.lstat()
    except OSError:
        fail("OCI execution request is missing")
    if (
        not stat.S_ISREG(request_info.st_mode)
        or request_path.is_symlink()
        or request_info.st_nlink != 1
    ):
        fail("OCI execution request is not a single-link regular file")
    request = parse_request(request_path)
    profile = load_profile(root, policy, request.profile_id)
    root = root.resolve(strict=True)
    verify_operator_owned(root, profile, "protected root", directory=True)
    verify_operator_owned(
        Path(profile.runtime_path),
        profile,
        "OCI runtime",
        directory=False,
        allow_root=True,
    )
    verify_operator_owned(
        Path(profile.git_path),
        profile,
        "Git executable",
        directory=False,
        allow_root=True,
    )
    verify_operator_owned(
        Path(profile.git_objects_path), profile, "Git objects", directory=True
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
    seccomp = protected_path(root, profile.seccomp_path, "seccomp profile")
    verify_regular(
        seccomp,
        str(profile.seccomp_size),
        profile.seccomp_digest,
        "protected seccomp profile",
    )
    verify_operator_owned(seccomp, profile, "seccomp profile", directory=False)
    verify_oci_image(profile)
    return AuthorizedRequest(
        policy,
        profile,
        request,
        seccomp.resolve(strict=True),
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
    try:
        inspected = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError):
        fail("OCI runtime returned invalid image identity")
    if not isinstance(inspected, dict):
        fail("OCI runtime returned invalid image object")
    validate_json_shape(inspected, "OCI runtime image")
    rootfs = inspected.get("RootFS")
    config = inspected.get("Config")
    if (
        inspected.get("Id") != profile.config.digest
        or profile.image_reference not in inspected.get("RepoDigests", [])
        or inspected.get("Os") != "linux"
        or inspected.get("Architecture") != "amd64"
        or not isinstance(rootfs, dict)
        or tuple(rootfs.get("Layers", [])) != diff_ids
        or not isinstance(config, dict)
        or config.get("Env") not in (None, [])
    ):
        fail("runtime image differs from protected OCI image")
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


def command_plan(args: argparse.Namespace) -> None:
    authorized = authorize(args)
    profile = authorized.profile
    request = authorized.request
    snapshot = stage_source(profile, request)
    try:
        output = stage_output(profile, request)
        try:
            arguments = docker_create_arguments(
                profile,
                request,
                snapshot.path,
                snapshot.request_path,
                authorized.seccomp_path,
            )
            verify_retained_snapshot(snapshot)
            verify_retained_output(output)
            print("oci_execution_plan_schema_version\t1")
            print("authorization_source\tprotected-policy")
            print(f"profile_id\t{profile.profile_id}")
            print(f"profile_sha256\t{profile.profile_digest}")
            print(f"request_id\t{request.request_id}")
            print(f"container_name\tfe2o3-evidence-{request.request_id}")
            print(f"source_snapshot\t{snapshot.path}")
            print(f"source_manifest\t{snapshot.manifest_path}")
            print(f"source_manifest_sha256\t{snapshot.manifest_digest}")
            print(f"request_snapshot\t{snapshot.request_path}")
            print(f"request_sha256\t{request.digest}")
            print(f"source_file_count\t{snapshot.file_count}")
            print(f"source_bytes\t{snapshot.byte_count}")
            print(f"artifact_stream_protocol\t{profile.artifact_stream_protocol}")
            print(f"artifact_stream_path\t{output.artifact_path}")
            print(f"artifact_stream_limit\t{profile.output_limit}")
            print(f"stderr_stream_path\t{output.log_path}")
            print(f"stderr_stream_limit\t{profile.log_limit}")
            print(f"argument_count\t{len(arguments)}")
            for index, argument in enumerate(arguments):
                print(f"argument\t{index:04d}\t{argument.encode('ascii').hex()}")
        finally:
            output.close()
    finally:
        snapshot.close()


def command_preflight(args: argparse.Namespace) -> None:
    observed = observe_runtime(authorize(args))
    profile = observed.authorized.profile
    request = observed.authorized.request
    print(f"authorized_profile\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print("observational_preflight\tpassed")
    print("execution_receipt\tnot-issued")


def command_verify(args: argparse.Namespace) -> None:
    authorized = authorize(args)
    profile = authorized.profile
    request = authorized.request
    print(f"authorized_profile\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print(f"source_tree\t{request.source_tree}")
    print("authorization_source\tprotected-policy")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify = subparsers.add_parser(
        "verify", help="verify protected authorization and immutable inputs"
    )
    verify.add_argument("--request", required=True)
    verify.add_argument("--trusted-root", required=True)
    verify.add_argument(
        "--policy", required=True, help="path relative to protected root"
    )
    verify.set_defaults(func=command_verify)
    plan = subparsers.add_parser(
        "plan", help="render the fixed protected OCI invocation"
    )
    plan.add_argument("--request", required=True)
    plan.add_argument("--trusted-root", required=True)
    plan.add_argument("--policy", required=True, help="path relative to protected root")
    plan.set_defaults(func=command_plan)
    preflight = subparsers.add_parser(
        "preflight", help="validate host, runtime daemon, and loaded image identity"
    )
    preflight.add_argument("--request", required=True)
    preflight.add_argument("--trusted-root", required=True)
    preflight.add_argument(
        "--policy", required=True, help="path relative to protected root"
    )
    preflight.set_defaults(func=command_preflight)
    return parser


def main() -> int:
    parser = build_parser()
    try:
        args = parser.parse_args()
        args.func(args)
        return 0
    except ExecutorError as error:
        print(f"parity-oci-executor: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
