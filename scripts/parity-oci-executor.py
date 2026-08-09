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
import stat
import subprocess
import sys
from dataclasses import dataclass


MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_ITEMS = 256
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
    layout_path: str
    index_digest: str
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
    required = {"HOME", "HOSTNAME", "LC_ALL", "PATH", "ROCR_VISIBLE_DEVICES"}
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
    layout_path = cursor.scalar("oci_layout_path")
    index_digest = cursor.scalar("oci_index_sha256")
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
        or not SHA256_RE.fullmatch(runtime_digest)
        or not SHA256_RE.fullmatch(runtime_version_digest)
        or not SHA256_RE.fullmatch(runtime_info_digest)
    ):
        fail("malformed runtime binding")
    if not SHA256_RE.fullmatch(index_digest):
        fail("malformed OCI index binding")
    manifest_match = OCI_DIGEST_RE.fullmatch(manifest_digest)
    config_match = OCI_DIGEST_RE.fullmatch(config_digest)
    if (
        manifest_match is None
        or config_match is None
        or not manifest_size.isdigit()
        or not config_size.isdigit()
        or IMAGE_REFERENCE_RE.fullmatch(image_reference) is None
        or not image_reference.endswith("@" + manifest_digest)
    ):
        fail("malformed OCI image identity")
    layer_count = parse_count(cursor.scalar("image_layer_count"), "image layer")
    layers: list[Layer] = []
    for index in range(layer_count):
        digest, size = cursor.record("image_layer", 4, index)[2:]
        if OCI_DIGEST_RE.fullmatch(digest) is None or not size.isdigit():
            fail("malformed OCI layer binding")
        layers.append(Layer(digest, int(size)))
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
        layout_path,
        index_digest,
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
    return value


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
    layout_marker = require_safe_oci_entry(layout, ("oci-layout",), "OCI layout marker")
    index_path = require_safe_oci_entry(layout, ("index.json",), "OCI index")
    require_safe_oci_entry(layout, ("blobs",), "OCI blob directory", directory=True)
    require_safe_oci_entry(
        layout, ("blobs", "sha256"), "OCI SHA-256 directory", directory=True
    )
    try:
        marker = json.loads(layout_marker.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        fail("invalid OCI layout marker")
    if marker != {"imageLayoutVersion": "1.0.0"}:
        fail("unsupported OCI layout version")
    if sha256_file(index_path) != profile.index_digest:
        fail("OCI index binding mismatch")
    try:
        index = json.loads(index_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError):
        fail("invalid OCI index JSON")
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


@dataclass(frozen=True)
class AuthorizedRequest:
    """Inputs authorized by protected policy, before any runtime contact."""

    policy: Policy
    profile: Profile
    request: Request
    repo: Path
    request_path: Path
    seccomp_path: Path


@dataclass(frozen=True)
class RuntimeReadyRequest:
    """Authorized inputs whose exact host, daemon, and image passed preflight."""

    authorized: AuthorizedRequest
    image_diff_ids: tuple[str, ...]


def parse_request(path: Path) -> Request:
    _, rows = read_rows(path, "OCI execution request")
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
        request_id, profile_id, source_commit, source_tree, job_id, job_path, job_digest
    )


def run_git(repo: Path, *arguments: str) -> str:
    process = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if process.returncode != 0:
        fail(f"git {' '.join(arguments)} failed")
    return process.stdout.strip()


def verify_source(repo: Path, request: Request, *, require_detached: bool) -> None:
    try:
        repo = repo.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve source checkout: {error}")
    if run_git(repo, "rev-parse", "HEAD") != request.source_commit:
        fail("source checkout commit differs from request")
    if run_git(repo, "rev-parse", "HEAD^{tree}") != request.source_tree:
        fail("source checkout tree differs from request")
    if run_git(repo, "status", "--porcelain", "--untracked-files=all"):
        fail("source checkout is not clean")
    if require_detached:
        symbolic_ref = subprocess.run(
            ["git", "-C", str(repo), "symbolic-ref", "-q", "HEAD"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if symbolic_ref.returncode == 0:
            fail("source checkout must be detached")
        if symbolic_ref.returncode != 1:
            fail("cannot establish detached source checkout")
    current = repo
    for index, component in enumerate(request.job_path.split("/")):
        current /= component
        try:
            info = current.lstat()
        except OSError:
            fail("source job is missing")
        if stat.S_ISLNK(info.st_mode):
            fail("source job path contains a symlink")
        if index + 1 < len(request.job_path.split("/")):
            if not stat.S_ISDIR(info.st_mode):
                fail("source job parent is not a directory")
        elif not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
            fail("source job is not a single-link regular file")
    job = current
    blob = subprocess.run(
        ["git", "-C", str(repo), "show", f"{request.source_commit}:{request.job_path}"],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if (
        blob.returncode
        or sha256_bytes(blob.stdout) != request.job_digest
        or sha256_file(job) != request.job_digest
    ):
        fail("source job differs from attested source tree")


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
    verify_oci_image(profile)
    repo_input = Path(args.repo)
    if not repo_input.is_absolute() or not valid_absolute(str(repo_input)):
        fail("source checkout path must be canonical absolute")
    repo = repo_input.resolve(strict=True)
    if repo != repo_input:
        fail("source checkout path contains a symlink")
    verify_source(repo, request, require_detached=args.require_detached)
    return AuthorizedRequest(
        policy,
        profile,
        request,
        repo,
        request_path.resolve(strict=True),
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
    process = subprocess.run(
        [profile.runtime_path, *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={"HOME": "/nonexistent", "LC_ALL": "C", "PATH": "/nonexistent"},
    )
    if process.returncode != 0:
        detail = process.stderr.decode("ascii", errors="replace").strip()
        if len(detail) > 240:
            detail = detail[:240] + "..."
        fail(f"OCI runtime control plane unavailable for {label}: {detail}")
    if process.stderr or not process.stdout.endswith(b"\n"):
        fail(f"OCI runtime returned non-canonical {label} identity")
    return process.stdout


def establish_runtime_ready(authorized: AuthorizedRequest) -> RuntimeReadyRequest:
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
    return RuntimeReadyRequest(authorized, verify_runtime_image(profile))


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
    repo: Path,
    request_path: Path,
    seccomp_path: Path,
) -> list[str]:
    name = f"fe2o3-evidence-{request.request_id[:24]}"
    arguments = [
        profile.runtime_path,
        "create",
        "--name",
        name,
        "--hostname",
        "fe2o3-evidence",
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
            f"type=bind,src={repo},dst={profile.source_mount},readonly,bind-recursive=readonly",
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
    arguments = docker_create_arguments(
        profile,
        request,
        authorized.repo,
        authorized.request_path,
        authorized.seccomp_path,
    )
    print("oci_execution_plan_schema_version\t1")
    print("authorization_source\tprotected-policy")
    print(f"profile_id\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print(f"argument_count\t{len(arguments)}")
    for index, argument in enumerate(arguments):
        print(f"argument\t{index:04d}\t{argument.encode('ascii').hex()}")


def command_preflight(args: argparse.Namespace) -> None:
    ready = establish_runtime_ready(authorize(args))
    profile = ready.authorized.profile
    request = ready.authorized.request
    print(f"authorized_profile\t{profile.profile_id}")
    print(f"profile_sha256\t{profile.profile_digest}")
    print(f"request_id\t{request.request_id}")
    print("runtime_preflight\tpassed")
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
    verify.add_argument("--repo", required=True)
    verify.add_argument("--request", required=True)
    verify.add_argument("--trusted-root", required=True)
    verify.add_argument(
        "--policy", required=True, help="path relative to protected root"
    )
    verify.add_argument("--require-detached", action="store_true")
    verify.set_defaults(func=command_verify)
    plan = subparsers.add_parser(
        "plan", help="render the fixed protected OCI invocation"
    )
    plan.add_argument("--repo", required=True)
    plan.add_argument("--request", required=True)
    plan.add_argument("--trusted-root", required=True)
    plan.add_argument("--policy", required=True, help="path relative to protected root")
    plan.add_argument("--require-detached", action="store_true")
    plan.set_defaults(func=command_plan)
    preflight = subparsers.add_parser(
        "preflight", help="validate host, runtime daemon, and loaded image identity"
    )
    preflight.add_argument("--repo", required=True)
    preflight.add_argument("--request", required=True)
    preflight.add_argument("--trusted-root", required=True)
    preflight.add_argument(
        "--policy", required=True, help="path relative to protected root"
    )
    preflight.add_argument("--require-detached", action="store_true")
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
