#!/usr/bin/env python3
"""Produce V3 evidence from two clean, detached direct-link builds."""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

from common import (
    EvidenceError,
    SUPPORTED_PROCESSORS,
    decode_canonical_text,
    metadata_snapshot,
    open_directory,
    open_parent_beneath,
    open_regular_beneath,
    read_regular_file,
    require_bounded_text,
    require_commit,
    require_reason,
    require_target,
    require_typed_identity,
    typed_descriptor_identity,
    typed_file_identity,
    typed_file_identity_beneath,
    typed_identity,
)

SCHEMA_VERSION = "3"
RECORD_DOMAIN = "fe2o3-direct-link-repro-v3"
SOURCE_TREE_DOMAIN = "fe2o3-source-tree-v1"
SOURCE_SNAPSHOT_DOMAIN = "fe2o3-source-snapshot-v1"
SOURCE_FILE_DOMAIN = "fe2o3-source-file-v1"
ARGV_DOMAIN = "fe2o3-build-argv-v1"
EXECUTABLE_DOMAIN = "fe2o3-build-executable-v1"
GIT_EXECUTABLE_DOMAIN = "fe2o3-git-executable-v1"
ENVIRONMENT_DOMAIN = "fe2o3-build-environment-v1"
TOOLCHAIN_DOMAIN = "fe2o3-llvm-toolchain-v1"
WORKER_DOMAIN = "fe2o3-worker-v1"
REQUEST_DOMAIN = "fe2o3-link-request-v1"
LINKED_ARTIFACT_DOMAIN = "fe2o3-linked-artifact-v1"
FINAL_ARTIFACT_DOMAIN = "fe2o3-final-artifact-v1"
PROVENANCE = "clean-detached-pinned-executable-v2"
TRUST_LEVEL = "unauthenticated-local-observation"
RELEASE_BLOCK_REASON = "missing-authenticated-upstream-attestations"

FIELDS = (
    "schema_version",
    "provenance",
    "trust_level",
    "git_commit",
    "source_tree_identity",
    "source_snapshot_identity",
    "git_executable_identity",
    "canonical_argv",
    "canonical_argv_identity",
    "first_source_dir",
    "first_build_dir",
    "first_expanded_argv",
    "second_source_dir",
    "second_build_dir",
    "second_expanded_argv",
    "build_executable_identity",
    "environment",
    "environment_identity",
    "llvm_toolchain_identity",
    "worker_identity",
    "request_identity",
    "target",
    "first_linked_artifact_identity",
    "second_linked_artifact_identity",
    "first_final_artifact_identity",
    "second_final_artifact_identity",
    "status",
    "reason",
)
STATUSES = frozenset(("pass", "fail", "unavailable"))
MAX_LOG_BYTES = 1024 * 1024
MAX_GIT_OUTPUT_BYTES = 32 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 6 * 60 * 60
PROCESS_DRAIN_GRACE_SECONDS = 0.25


@dataclass(frozen=True)
class ProcessResult:
    returncode: int | None
    output: bytes
    timed_out: bool = False
    overflow: bool = False
    unavailable: bool = False


@dataclass(frozen=True)
class SourceMeasurement:
    identity: str
    metadata: dict[str, tuple[int, int, int, int, int, int]]


@dataclass
class PinnedExecutable:
    descriptor: int
    identity: str

    def close(self) -> None:
        os.close(self.descriptor)


@dataclass
class BuildContext:
    source_dir: Path
    build_dir: Path
    build_descriptor: int
    source_measurement: SourceMeasurement
    expanded_argv: list[str]


@dataclass(frozen=True)
class ReproducibilityResult:
    git_commit: str
    source_tree_identity: str
    source_snapshot_identity: str
    git_executable_identity: str
    canonical_argv: str
    canonical_argv_identity: str
    first_source_dir: str
    first_build_dir: str
    first_expanded_argv: str
    second_source_dir: str
    second_build_dir: str
    second_expanded_argv: str
    build_executable_identity: str
    environment: str
    environment_identity: str
    llvm_toolchain_identity: str
    worker_identity: str
    request_identity: str
    target: str
    first_linked_artifact_identity: str
    second_linked_artifact_identity: str
    first_final_artifact_identity: str
    second_final_artifact_identity: str
    status: str
    reason: str

    def values(self) -> dict[str, str]:
        return {
            "schema_version": SCHEMA_VERSION,
            "provenance": PROVENANCE,
            "trust_level": TRUST_LEVEL,
            "git_commit": self.git_commit,
            "source_tree_identity": self.source_tree_identity,
            "source_snapshot_identity": self.source_snapshot_identity,
            "git_executable_identity": self.git_executable_identity,
            "canonical_argv": self.canonical_argv,
            "canonical_argv_identity": self.canonical_argv_identity,
            "first_source_dir": self.first_source_dir,
            "first_build_dir": self.first_build_dir,
            "first_expanded_argv": self.first_expanded_argv,
            "second_source_dir": self.second_source_dir,
            "second_build_dir": self.second_build_dir,
            "second_expanded_argv": self.second_expanded_argv,
            "build_executable_identity": self.build_executable_identity,
            "environment": self.environment,
            "environment_identity": self.environment_identity,
            "llvm_toolchain_identity": self.llvm_toolchain_identity,
            "worker_identity": self.worker_identity,
            "request_identity": self.request_identity,
            "target": self.target,
            "first_linked_artifact_identity": self.first_linked_artifact_identity,
            "second_linked_artifact_identity": self.second_linked_artifact_identity,
            "first_final_artifact_identity": self.first_final_artifact_identity,
            "second_final_artifact_identity": self.second_final_artifact_identity,
            "status": self.status,
            "reason": self.reason,
        }

    def preimage(self) -> bytes:
        values = self.values()
        return (
            "\n".join(f"{field}\t{values[field]}" for field in FIELDS) + "\n"
        ).encode("ascii")

    def identity(self) -> str:
        return typed_identity(RECORD_DOMAIN, self.preimage())

    def canonical_bytes(self) -> bytes:
        return self.preimage() + f"record_identity\t{self.identity()}\n".encode("ascii")


def canonical_json(value: object, name: str) -> str:
    encoded = json.dumps(
        value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
    )
    return require_bounded_text(encoded, name, 32 * 1024)


def decode_argv(value: str) -> list[str]:
    try:
        decoded = json.loads(value)
    except json.JSONDecodeError as error:
        raise EvidenceError("canonical_argv is not valid JSON") from error
    if (
        not isinstance(decoded, list)
        or not decoded
        or any(not isinstance(argument, str) for argument in decoded)
    ):
        raise EvidenceError("canonical_argv must be a nonempty string array")
    for argument in decoded:
        require_bounded_text(argument, "build argument", 4096)
    if canonical_json(decoded, "canonical_argv") != value:
        raise EvidenceError("canonical_argv is not canonically encoded")
    return decoded


def validate_command_template(argv: list[str]) -> None:
    required = ("{source_dir}", "{build_dir}", "{target}")
    if any(token in argv[0] for token in required):
        raise EvidenceError("build executable must not contain a placeholder")
    for token in required:
        if not any(token in argument for argument in argv):
            raise EvidenceError(f"canonical_argv must bind the {token} placeholder")
    for argument in argv:
        remainder = argument
        for token in required:
            remainder = remainder.replace(token, "")
        if "{" in remainder or "}" in remainder:
            raise EvidenceError("canonical_argv contains an unknown placeholder")


def decode_environment(value: str) -> dict[str, str]:
    try:
        decoded = json.loads(value)
    except json.JSONDecodeError as error:
        raise EvidenceError("environment is not valid JSON") from error
    if not isinstance(decoded, dict) or not decoded:
        raise EvidenceError("environment must be a nonempty object")
    for name, item in decoded.items():
        if not isinstance(name, str) or not isinstance(item, str):
            raise EvidenceError("environment must map strings to strings")
        require_bounded_text(name, "environment name", 64)
        require_bounded_text(item, f"environment value {name}", 4096)
    if canonical_json(decoded, "environment") != value:
        raise EvidenceError("environment is not canonically encoded")
    return decoded


def validate_result(result: ReproducibilityResult) -> None:
    require_commit(result.git_commit)
    require_target(result.target)
    require_typed_identity(
        result.source_tree_identity, SOURCE_TREE_DOMAIN, "source_tree_identity"
    )
    require_typed_identity(
        result.source_snapshot_identity,
        SOURCE_SNAPSHOT_DOMAIN,
        "source_snapshot_identity",
    )
    require_typed_identity(
        result.git_executable_identity,
        GIT_EXECUTABLE_DOMAIN,
        "git_executable_identity",
    )
    argv = decode_argv(result.canonical_argv)
    validate_command_template(argv)
    if result.canonical_argv_identity != typed_identity(
        ARGV_DOMAIN, result.canonical_argv.encode("ascii")
    ):
        raise EvidenceError("canonical_argv_identity mismatch")
    if not Path(argv[0]).is_absolute():
        raise EvidenceError("canonical build executable must be an absolute path")
    run_paths = (
        (
            result.first_source_dir,
            result.first_build_dir,
            result.first_expanded_argv,
        ),
        (
            result.second_source_dir,
            result.second_build_dir,
            result.second_expanded_argv,
        ),
    )
    for source_dir, build_dir, encoded_expanded in run_paths:
        source_path = Path(require_bounded_text(source_dir, "source directory", 4096))
        build_path = Path(require_bounded_text(build_dir, "build directory", 4096))
        if not source_path.is_absolute() or not build_path.is_absolute():
            raise EvidenceError(
                "recorded source and build directories must be absolute"
            )
        expanded = decode_argv(encoded_expanded)
        expected = expanded_command(argv, build_path, source_path, result.target)
        if expanded != expected:
            raise EvidenceError("expanded argv does not match its recorded run paths")
    if (
        result.first_source_dir == result.second_source_dir
        or result.first_build_dir == result.second_build_dir
    ):
        raise EvidenceError("reproducibility runs must use distinct directories")
    require_typed_identity(
        result.build_executable_identity,
        EXECUTABLE_DOMAIN,
        "build_executable_identity",
    )
    decode_environment(result.environment)
    if result.environment_identity != typed_identity(
        ENVIRONMENT_DOMAIN, result.environment.encode("ascii")
    ):
        raise EvidenceError("environment_identity mismatch")
    require_typed_identity(
        result.llvm_toolchain_identity,
        TOOLCHAIN_DOMAIN,
        "llvm_toolchain_identity",
    )
    require_typed_identity(result.worker_identity, WORKER_DOMAIN, "worker_identity")
    require_typed_identity(result.request_identity, REQUEST_DOMAIN, "request_identity")
    if result.status not in STATUSES:
        raise EvidenceError("reproducibility status is unknown")

    artifacts = (
        (
            "first_linked_artifact_identity",
            result.first_linked_artifact_identity,
            LINKED_ARTIFACT_DOMAIN,
        ),
        (
            "second_linked_artifact_identity",
            result.second_linked_artifact_identity,
            LINKED_ARTIFACT_DOMAIN,
        ),
        (
            "first_final_artifact_identity",
            result.first_final_artifact_identity,
            FINAL_ARTIFACT_DOMAIN,
        ),
        (
            "second_final_artifact_identity",
            result.second_final_artifact_identity,
            FINAL_ARTIFACT_DOMAIN,
        ),
    )
    for name, identity, domain in artifacts:
        if identity != "none":
            require_typed_identity(identity, domain, name)

    if result.status == "pass":
        if result.reason != "-":
            raise EvidenceError("passing reproducibility result must use reason '-'")
        if any(identity == "none" for _, identity, _ in artifacts):
            raise EvidenceError("passing result must identify all artifacts")
        if (
            result.first_linked_artifact_identity
            != result.second_linked_artifact_identity
        ):
            raise EvidenceError("linked artifacts are not reproducible")
        if (
            result.first_final_artifact_identity
            != result.second_final_artifact_identity
        ):
            raise EvidenceError("final artifacts are not reproducible")
    else:
        require_reason(result.reason)


def parse_result(path: Path) -> ReproducibilityResult:
    data = read_regular_file(path)
    lines = decode_canonical_text(data, "reproducibility record")
    values: dict[str, str] = {}
    record_identity: str | None = None
    for line_number, line in enumerate(lines, start=1):
        fields = line.split("\t")
        if len(fields) != 2:
            raise EvidenceError(
                f"reproducibility line {line_number} must contain exactly two fields"
            )
        key, value = fields
        if key == "record_identity":
            if record_identity is not None:
                raise EvidenceError("duplicate record_identity")
            record_identity = value
        elif key in FIELDS:
            if key in values:
                raise EvidenceError(f"duplicate reproducibility field: {key}")
            values[key] = value
        else:
            raise EvidenceError(f"unknown reproducibility field: {key}")
    missing = set(FIELDS) - set(values)
    if missing:
        raise EvidenceError(f"missing reproducibility fields: {sorted(missing)}")
    if values["schema_version"] != SCHEMA_VERSION:
        raise EvidenceError("reproducibility schema_version must be exactly 3")
    if values["provenance"] != PROVENANCE:
        raise EvidenceError(
            "reproducibility provenance is not a clean pinned-executable build"
        )
    if values["trust_level"] != TRUST_LEVEL:
        raise EvidenceError("reproducibility trust_level must be unauthenticated")
    if record_identity is None:
        raise EvidenceError("reproducibility record_identity is missing")

    result = ReproducibilityResult(
        **{
            field: values[field]
            for field in FIELDS
            if field not in {"schema_version", "provenance", "trust_level"}
        }
    )
    validate_result(result)
    require_typed_identity(record_identity, RECORD_DOMAIN, "record_identity")
    if result.identity() != record_identity:
        raise EvidenceError("reproducibility record_identity mismatch")
    if data != result.canonical_bytes():
        raise EvidenceError(
            "reproducibility record uses noncanonical field order or encoding"
        )
    return result


def terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def pin_executable(path: Path) -> PinnedExecutable:
    if not hasattr(os, "memfd_create") or not hasattr(os, "MFD_ALLOW_SEALING"):
        raise EvidenceError("sealed executable pinning is unavailable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        source = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(
            f"cannot pin build executable {path}: {error.strerror}"
        ) from error
    pinned = -1
    try:
        before = os.fstat(source)
        if not stat.S_ISREG(before.st_mode) or before.st_size <= 0:
            raise EvidenceError("build executable must be a nonempty regular file")
        if before.st_mode & 0o111 == 0:
            raise EvidenceError(
                "build executable does not have an execute permission bit"
            )
        if before.st_size > 512 * 1024 * 1024:
            raise EvidenceError("build executable exceeds the measurement bound")
        pinned = os.memfd_create(
            "fe2o3-build-executable",
            os.MFD_ALLOW_SEALING | getattr(os, "MFD_CLOEXEC", 0),
        )
        remaining = before.st_size
        while remaining:
            chunk = os.read(source, min(1024 * 1024, remaining))
            if not chunk:
                raise EvidenceError("build executable was truncated while pinning")
            view = memoryview(chunk)
            while view:
                written = os.write(pinned, view)
                view = view[written:]
            remaining -= len(chunk)
        after = os.fstat(source)
        if metadata_snapshot(before) != metadata_snapshot(after):
            raise EvidenceError("build executable changed while being pinned")
        os.fchmod(pinned, 0o500)
        seals = (
            fcntl.F_SEAL_WRITE
            | fcntl.F_SEAL_GROW
            | fcntl.F_SEAL_SHRINK
            | fcntl.F_SEAL_SEAL
        )
        fcntl.fcntl(pinned, fcntl.F_ADD_SEALS, seals)
        if fcntl.fcntl(pinned, fcntl.F_GET_SEALS) & seals != seals:
            raise EvidenceError("build executable memfd was not fully sealed")
        identity = typed_descriptor_identity(
            EXECUTABLE_DOMAIN, pinned, "sealed build executable"
        )
        proc_path = Path(f"/proc/self/fd/{pinned}")
        if not proc_path.exists():
            raise EvidenceError(
                "sealed executable descriptor is not executable via procfs"
            )
        result = PinnedExecutable(pinned, identity)
        pinned = -1
        return result
    except OSError as error:
        raise EvidenceError(
            f"sealed executable pinning failed: {error.strerror}"
        ) from error
    finally:
        os.close(source)
        if pinned >= 0:
            os.close(pinned)


def run_bounded(
    argv: list[str],
    cwd: Path,
    environment: dict[str, str],
    timeout: int,
    output_limit: int = MAX_LOG_BYTES,
    executable_descriptor: int | None = None,
) -> ProcessResult:
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    output = bytearray()
    overflow = False
    timed_out = False
    cleanup_started: float | None = None
    try:
        try:
            executable = None
            pass_fds: tuple[int, ...] = ()
            if executable_descriptor is not None:
                executable = f"/proc/self/fd/{executable_descriptor}"
                pass_fds = (executable_descriptor,)
            process = subprocess.Popen(
                argv,
                cwd=cwd,
                env=environment,
                executable=executable,
                pass_fds=pass_fds,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,
            )
        except (FileNotFoundError, PermissionError, OSError):
            return ProcessResult(None, b"", unavailable=True)
        assert process.stdout is not None
        os.set_blocking(process.stdout.fileno(), False)
        selector.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + timeout

        while process.poll() is None or selector.get_map():
            now = time.monotonic()
            if process.poll() is None and now >= deadline:
                timed_out = True
                terminate_process_group(process)
                cleanup_started = now
            elif process.poll() is None and overflow and cleanup_started is None:
                terminate_process_group(process)
                cleanup_started = now
            elif process.poll() is not None and cleanup_started is None:
                terminate_process_group(process)
                cleanup_started = now

            if (
                cleanup_started is not None
                and now - cleanup_started >= PROCESS_DRAIN_GRACE_SECONDS
            ):
                for key in list(selector.get_map().values()):
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                break

            for key, _ in selector.select(timeout=0.02):
                try:
                    chunk = os.read(key.fd, 64 * 1024)
                except BlockingIOError:
                    continue
                if not chunk:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                remaining = output_limit - len(output)
                if remaining > 0:
                    output.extend(chunk[:remaining])
                if len(chunk) > remaining:
                    overflow = True

        try:
            returncode = process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            terminate_process_group(process)
            returncode = process.wait(timeout=1)
        return ProcessResult(returncode, bytes(output), timed_out, overflow)
    finally:
        selector.close()
        if process is not None:
            terminate_process_group(process)
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                pass


def sanitized_environment(
    target: str,
    source_date_epoch: int,
    path: str,
    git_commit: str,
    toolchain_identity: str,
    worker_identity: str,
    request_identity: str,
) -> dict[str, str]:
    return {
        "FE2O3_GIT_COMMIT": git_commit,
        "FE2O3_LLVM_TOOLCHAIN_IDENTITY": toolchain_identity,
        "FE2O3_REQUEST_IDENTITY": request_identity,
        "FE2O3_TARGET": target,
        "FE2O3_WORKER_IDENTITY": worker_identity,
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": path,
        "SOURCE_DATE_EPOCH": str(source_date_epoch),
        "TZ": "UTC",
    }


def validate_path_environment(value: str) -> str:
    require_bounded_text(value, "environment PATH", 4096)
    parts = value.split(":")
    if any(not part or not Path(part).is_absolute() for part in parts):
        raise argparse.ArgumentTypeError(
            "environment PATH must contain only nonempty absolute directories"
        )
    return value


def expanded_command(
    command: list[str], build_dir: Path, source_dir: Path, target: str
) -> list[str]:
    replacements = {
        "{build_dir}": str(build_dir),
        "{source_dir}": str(source_dir),
        "{target}": target,
    }
    expanded: list[str] = []
    for original in command:
        argument = original
        for token, replacement in replacements.items():
            argument = argument.replace(token, replacement)
        expanded.append(argument)
    return expanded


def git_environment(path: str) -> dict[str, str]:
    return {
        "GIT_ATTR_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_SYSTEM": "/dev/null",
        "GIT_TERMINAL_PROMPT": "0",
        "HOME": "/nonexistent-fe2o3-home",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": path,
        "TZ": "UTC",
        "XDG_CONFIG_HOME": "/nonexistent-fe2o3-xdg",
    }


def run_git(
    git: Path,
    arguments: list[str],
    cwd: Path,
    path: str,
    timeout: int = 120,
) -> bytes:
    result = run_bounded(
        [str(git), *arguments],
        cwd,
        git_environment(path),
        timeout,
        MAX_GIT_OUTPUT_BYTES,
    )
    if (
        result.unavailable
        or result.returncode != 0
        or result.timed_out
        or result.overflow
    ):
        raise EvidenceError("git snapshot command failed")
    return result.output


def source_tree_identity(git: Path, repository: Path, commit: str, path: str) -> str:
    tree = run_git(
        git,
        ["-C", str(repository), "ls-tree", "-r", "-z", "--full-tree", commit],
        repository,
        path,
    )
    if not tree:
        raise EvidenceError("source tree is empty")
    if any(entry.startswith(b"160000 ") for entry in tree.split(b"\0") if entry):
        raise EvidenceError("source tree contains an unbound submodule")
    return typed_identity(SOURCE_TREE_DOMAIN, tree)


def materialize_snapshot(
    git: Path,
    source: Path,
    destination: Path,
    commit: str,
    expected_tree: str,
    path: str,
) -> None:
    run_git(
        git,
        [
            "clone",
            "--no-hardlinks",
            "--no-checkout",
            "--quiet",
            "--",
            str(source),
            str(destination),
        ],
        destination.parent,
        path,
    )
    run_git(
        git,
        [
            "-C",
            str(destination),
            "-c",
            "advice.detachedHead=false",
            "-c",
            "core.attributesFile=/dev/null",
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--detach",
            "--quiet",
            commit,
        ],
        destination,
        path,
    )
    head = (
        run_git(git, ["-C", str(destination), "rev-parse", "HEAD"], destination, path)
        .decode("ascii")
        .strip()
    )
    if head != commit:
        raise EvidenceError("detached source snapshot has the wrong commit")
    if source_tree_identity(git, destination, commit, path) != expected_tree:
        raise EvidenceError("detached source snapshot has the wrong tree identity")
    require_clean_snapshot(git, destination, path)


def require_clean_snapshot(git: Path, snapshot: Path, path: str) -> None:
    status = run_git(
        git,
        ["-C", str(snapshot), "status", "--porcelain=v1", "--untracked-files=all"],
        snapshot,
        path,
    )
    if status:
        raise EvidenceError("detached source snapshot was mutated by the build")


def tracked_source_paths(git: Path, snapshot: Path, path: str) -> list[Path]:
    encoded = run_git(
        git,
        ["-C", str(snapshot), "ls-files", "--cached", "-z"],
        snapshot,
        path,
    )
    paths: list[Path] = []
    for raw in encoded.split(b"\0"):
        if not raw:
            continue
        try:
            name = raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise EvidenceError("tracked source path is not ASCII") from error
        pure = PurePosixPath(name)
        if (
            pure.is_absolute()
            or not pure.parts
            or any(component in ("", ".", "..") for component in pure.parts)
        ):
            raise EvidenceError("tracked source path is not canonical and relative")
        paths.append(Path(*pure.parts))
    if not paths or paths != sorted(paths, key=lambda item: item.as_posix()):
        raise EvidenceError("tracked source path list is empty or noncanonical")
    return paths


def source_metadata(value: os.stat_result) -> tuple[int, int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_size,
        value.st_mtime_ns,
        value.st_ctime_ns,
    )


def append_manifest_field(manifest: bytearray, value: bytes) -> None:
    manifest.extend(len(value).to_bytes(8, "big"))
    manifest.extend(value)


def measure_source_snapshot(snapshot: Path, paths: list[Path]) -> SourceMeasurement:
    root = open_directory(snapshot)
    metadata: dict[str, tuple[int, int, int, int, int, int]] = {
        "directory:.": source_metadata(os.fstat(root))
    }
    manifest = bytearray()
    try:
        directories = {
            Path(*tracked.parts[:index])
            for tracked in paths
            for index in range(1, len(tracked.parts))
        }
        for directory in sorted(directories, key=lambda item: item.as_posix()):
            descriptor, _ = open_parent_beneath(root, directory / ".measurement-leaf")
            try:
                metadata[f"directory:{directory.as_posix()}"] = source_metadata(
                    os.fstat(descriptor)
                )
            finally:
                os.close(descriptor)

        for tracked in paths:
            parent, name = open_parent_beneath(root, tracked)
            try:
                before = os.stat(name, dir_fd=parent, follow_symlinks=False)
                path_bytes = tracked.as_posix().encode("ascii")
                append_manifest_field(manifest, path_bytes)
                append_manifest_field(
                    manifest, (before.st_mode & 0o7777).to_bytes(4, "big")
                )
                if stat.S_ISREG(before.st_mode):
                    descriptor = open_regular_beneath(root, tracked)
                    try:
                        opened = os.fstat(descriptor)
                        if source_metadata(before) != source_metadata(opened):
                            raise EvidenceError(
                                f"tracked source changed while opening: {tracked}"
                            )
                        identity = typed_descriptor_identity(
                            SOURCE_FILE_DOMAIN, descriptor, tracked.as_posix()
                        )
                        metadata[f"file:{tracked.as_posix()}"] = source_metadata(
                            os.fstat(descriptor)
                        )
                    finally:
                        os.close(descriptor)
                    append_manifest_field(manifest, b"regular")
                    append_manifest_field(manifest, identity.encode("ascii"))
                elif stat.S_ISLNK(before.st_mode):
                    target = os.readlink(name.encode("ascii"), dir_fd=parent)
                    after = os.stat(name, dir_fd=parent, follow_symlinks=False)
                    if source_metadata(before) != source_metadata(after):
                        raise EvidenceError(
                            f"tracked symlink changed while reading: {tracked}"
                        )
                    metadata[f"symlink:{tracked.as_posix()}"] = source_metadata(after)
                    append_manifest_field(manifest, b"symlink")
                    append_manifest_field(manifest, target)
                else:
                    raise EvidenceError(
                        f"tracked source has unsupported file type: {tracked}"
                    )
            finally:
                os.close(parent)
    finally:
        os.close(root)
    return SourceMeasurement(
        typed_identity(SOURCE_SNAPSHOT_DOMAIN, bytes(manifest)), metadata
    )


def validate_artifact_path(value: str) -> Path:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or not path.parts
        or any(part in ("", ".", "..") for part in path.parts)
    ):
        raise argparse.ArgumentTypeError(
            "artifact path must be a nonempty relative path without traversal"
        )
    return Path(*path.parts)


def write_build_log(build_descriptor: int, data: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open("build.log", flags, 0o600, dir_fd=build_descriptor)
    except OSError as error:
        raise EvidenceError("cannot create parent-owned build log") from error
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            view = view[written:]
    finally:
        os.close(descriptor)


def empty_result(
    metadata: dict[str, str], status: str, reason: str, artifacts: list[str]
) -> ReproducibilityResult:
    values = [*artifacts, *("none" for _ in range(4 - len(artifacts)))]
    return ReproducibilityResult(
        **metadata,
        first_linked_artifact_identity=values[0],
        second_linked_artifact_identity=values[1],
        first_final_artifact_identity=values[2],
        second_final_artifact_identity=values[3],
        status=status,
        reason=reason,
    )


def emit(result: ReproducibilityResult) -> int:
    validate_result(result)
    sys.stdout.buffer.write(result.canonical_bytes())
    return 0 if result.status == "pass" else 1


def run_comparison(args: argparse.Namespace) -> int:
    target = require_target(args.target)
    commit = require_commit(args.commit)
    for identity, domain, name in (
        (args.llvm_toolchain_identity, TOOLCHAIN_DOMAIN, "llvm_toolchain_identity"),
        (args.worker_identity, WORKER_DOMAIN, "worker_identity"),
        (args.request_identity, REQUEST_DOMAIN, "request_identity"),
    ):
        require_typed_identity(identity, domain, name)
    try:
        source = args.source_dir.resolve(strict=True)
        work_root = args.work_root.resolve(strict=True)
    except OSError as error:
        raise EvidenceError(f"cannot resolve clean-build directory: {error}") from error
    if not source.is_dir() or not work_root.is_dir():
        raise EvidenceError("source-dir and work-root must be directories")

    command = list(args.build_command)
    if command and command[0] == "--":
        command.pop(0)
    if not command:
        raise EvidenceError("run requires a build command after '--'")
    for argument in command:
        require_bounded_text(argument, "build argument", 4096)
    validate_command_template(command)
    executable = Path(command[0])
    if not executable.is_absolute():
        raise EvidenceError("build command executable must be an absolute path")
    pinned_executable = pin_executable(executable)
    canonical_argv = canonical_json(command, "canonical_argv")
    argv_identity = typed_identity(ARGV_DOMAIN, canonical_argv.encode("ascii"))
    try:
        git_name = shutil.which("git", path=args.environment_path)
        if git_name is None:
            raise EvidenceError("git executable is unavailable in the sanitized PATH")
        git = Path(git_name).resolve(strict=True)
        git_identity = typed_file_identity(GIT_EXECUTABLE_DOMAIN, git)
        tree_identity = source_tree_identity(git, source, commit, args.environment_path)
        epoch_output = run_git(
            git,
            ["-C", str(source), "show", "-s", "--format=%ct", commit],
            source,
            args.environment_path,
        )
        try:
            source_date_epoch = int(epoch_output.decode("ascii").strip())
        except (UnicodeDecodeError, ValueError) as error:
            raise EvidenceError("commit timestamp is malformed") from error

        environment = sanitized_environment(
            target,
            source_date_epoch,
            args.environment_path,
            commit,
            args.llvm_toolchain_identity,
            args.worker_identity,
            args.request_identity,
        )
        canonical_environment = canonical_json(environment, "environment")

        with tempfile.TemporaryDirectory(
            prefix=f"fe2o3-repro-{target.split(':', 1)[0]}-", dir=work_root
        ) as temporary:
            root = Path(temporary)
            contexts: list[BuildContext] = []
            try:
                tracked_paths: list[Path] | None = None
                for label in ("first", "second"):
                    run_root = root / label
                    source_snapshot = run_root / "source"
                    build_dir = run_root / "build"
                    run_root.mkdir(mode=0o700)
                    materialize_snapshot(
                        git,
                        source,
                        source_snapshot,
                        commit,
                        tree_identity,
                        args.environment_path,
                    )
                    current_paths = tracked_source_paths(
                        git, source_snapshot, args.environment_path
                    )
                    if tracked_paths is None:
                        tracked_paths = current_paths
                    elif current_paths != tracked_paths:
                        raise EvidenceError(
                            "detached snapshots have different tracked path sets"
                        )
                    source_measurement = measure_source_snapshot(
                        source_snapshot, current_paths
                    )
                    build_dir.mkdir(mode=0o700)
                    contexts.append(
                        BuildContext(
                            source_snapshot,
                            build_dir,
                            open_directory(build_dir),
                            source_measurement,
                            expanded_command(
                                command, build_dir, source_snapshot, target
                            ),
                        )
                    )
                if (
                    contexts[0].source_measurement.identity
                    != contexts[1].source_measurement.identity
                ):
                    raise EvidenceError(
                        "detached snapshots contain different checked-out bytes"
                    )

                metadata = {
                    "git_commit": commit,
                    "source_tree_identity": tree_identity,
                    "source_snapshot_identity": contexts[0].source_measurement.identity,
                    "git_executable_identity": git_identity,
                    "canonical_argv": canonical_argv,
                    "canonical_argv_identity": argv_identity,
                    "first_source_dir": str(contexts[0].source_dir),
                    "first_build_dir": str(contexts[0].build_dir),
                    "first_expanded_argv": canonical_json(
                        contexts[0].expanded_argv, "first_expanded_argv"
                    ),
                    "second_source_dir": str(contexts[1].source_dir),
                    "second_build_dir": str(contexts[1].build_dir),
                    "second_expanded_argv": canonical_json(
                        contexts[1].expanded_argv, "second_expanded_argv"
                    ),
                    "build_executable_identity": pinned_executable.identity,
                    "environment": canonical_environment,
                    "environment_identity": typed_identity(
                        ENVIRONMENT_DOMAIN, canonical_environment.encode("ascii")
                    ),
                    "llvm_toolchain_identity": args.llvm_toolchain_identity,
                    "worker_identity": args.worker_identity,
                    "request_identity": args.request_identity,
                    "target": target,
                }

                linked: list[str] = []
                finalized: list[str] = []
                assert tracked_paths is not None
                for context in contexts:
                    result = run_bounded(
                        context.expanded_argv,
                        context.build_dir,
                        environment,
                        args.timeout,
                        executable_descriptor=pinned_executable.descriptor,
                    )
                    write_build_log(context.build_descriptor, result.output)
                    if result.unavailable:
                        return emit(
                            empty_result(
                                metadata,
                                "unavailable",
                                "pinned-executable-unavailable",
                                [],
                            )
                        )
                    if result.timed_out:
                        return emit(empty_result(metadata, "fail", "build-timeout", []))
                    if result.overflow:
                        return emit(
                            empty_result(metadata, "fail", "build-log-limit", [])
                        )
                    if result.returncode != 0:
                        return emit(
                            empty_result(metadata, "fail", "build-command-failed", [])
                        )

                    after_source = measure_source_snapshot(
                        context.source_dir, tracked_paths
                    )
                    try:
                        require_clean_snapshot(
                            git, context.source_dir, args.environment_path
                        )
                    except EvidenceError:
                        return emit(
                            empty_result(
                                metadata, "fail", "source-snapshot-mutated", []
                            )
                        )
                    if (
                        after_source.identity != context.source_measurement.identity
                        or after_source.metadata != context.source_measurement.metadata
                    ):
                        return emit(
                            empty_result(
                                metadata, "fail", "source-snapshot-mutated", []
                            )
                        )
                    try:
                        current_executable = typed_file_identity(
                            EXECUTABLE_DOMAIN, executable
                        )
                    except EvidenceError:
                        current_executable = "mutated"
                    if current_executable != pinned_executable.identity:
                        return emit(
                            empty_result(
                                metadata, "fail", "build-executable-mutated", []
                            )
                        )
                    try:
                        linked.append(
                            typed_file_identity_beneath(
                                LINKED_ARTIFACT_DOMAIN,
                                context.build_descriptor,
                                args.linked_artifact,
                            )
                        )
                        finalized.append(
                            typed_file_identity_beneath(
                                FINAL_ARTIFACT_DOMAIN,
                                context.build_descriptor,
                                args.final_artifact,
                            )
                        )
                    except EvidenceError:
                        return emit(
                            empty_result(metadata, "fail", "artifact-unmeasurable", [])
                        )

                artifact_values = [linked[0], linked[1], finalized[0], finalized[1]]
                status = (
                    "pass"
                    if linked[0] == linked[1] and finalized[0] == finalized[1]
                    else "fail"
                )
                reason = "-" if status == "pass" else "artifact-mismatch"
                return emit(empty_result(metadata, status, reason, artifact_values))
            finally:
                for context in contexts:
                    os.close(context.build_descriptor)
    finally:
        pinned_executable.close()


def inspect_command(args: argparse.Namespace) -> int:
    result = parse_result(args.record)
    print(
        f"direct-link reproducibility record is canonical: {result.identity()} "
        f"status={result.status}"
    )
    return 0


def validate_command(args: argparse.Namespace) -> int:
    result = parse_result(args.record)
    expected = {
        "git_commit": args.expect_commit,
        "source_tree_identity": args.expect_source_tree_identity,
        "source_snapshot_identity": args.expect_source_snapshot_identity,
        "canonical_argv_identity": args.expect_argv_identity,
        "build_executable_identity": typed_file_identity(
            EXECUTABLE_DOMAIN, args.build_executable
        ),
        "environment_identity": args.expect_environment_identity,
        "llvm_toolchain_identity": args.expect_llvm_toolchain_identity,
        "worker_identity": args.expect_worker_identity,
        "request_identity": args.expect_request_identity,
        "target": args.expect_target,
    }
    for field, value in expected.items():
        if getattr(result, field) != value:
            raise EvidenceError(
                f"{field} mismatch: expected {value}, found {getattr(result, field)}"
            )
    print(
        f"direct-link reproducibility release=blocked reason={RELEASE_BLOCK_REASON} "
        f"record={result.identity()} observation={result.status}"
    )
    return 1


def inspect_matrix(args: argparse.Namespace) -> int:
    records: dict[str, ReproducibilityResult] = {}
    for path in args.record:
        result = parse_result(path)
        if ":" in result.target or result.target not in SUPPORTED_PROCESSORS:
            raise EvidenceError(
                f"matrix record must use an exact base target: {result.target}"
            )
        if result.target in records:
            raise EvidenceError(f"duplicate matrix target: {result.target}")
        records[result.target] = result
    if set(records) != set(SUPPORTED_PROCESSORS):
        missing = sorted(set(SUPPORTED_PROCESSORS) - set(records))
        extra = sorted(set(records) - set(SUPPORTED_PROCESSORS))
        raise EvidenceError(
            f"wrong reproducibility target matrix; missing={missing}, extra={extra}"
        )
    commits = {result.git_commit for result in records.values()}
    toolchains = {result.llvm_toolchain_identity for result in records.values()}
    workers = {result.worker_identity for result in records.values()}
    if len(commits) != 1 or len(toolchains) != 1 or len(workers) != 1:
        raise EvidenceError("matrix records do not share commit, toolchain, and worker")
    for target in sorted(records):
        result = records[target]
        print(f"{target}\t{result.status}\t{result.reason}\t{result.identity()}")
    return 0


def bounded_timeout(value: str) -> int:
    try:
        timeout = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be an integer") from error
    if not 1 <= timeout <= MAX_TIMEOUT_SECONDS:
        raise argparse.ArgumentTypeError(
            f"timeout must be between 1 and {MAX_TIMEOUT_SECONDS} seconds"
        )
    return timeout


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    run_parser = subparsers.add_parser(
        "run", help="run two clean builds from detached checkouts"
    )
    run_parser.add_argument("--commit", required=True)
    run_parser.add_argument("--target", required=True)
    run_parser.add_argument(
        "--linked-artifact", required=True, type=validate_artifact_path
    )
    run_parser.add_argument(
        "--final-artifact", required=True, type=validate_artifact_path
    )
    run_parser.add_argument("--source-dir", required=True, type=Path)
    run_parser.add_argument("--work-root", required=True, type=Path)
    run_parser.add_argument("--llvm-toolchain-identity", required=True)
    run_parser.add_argument("--worker-identity", required=True)
    run_parser.add_argument("--request-identity", required=True)
    run_parser.add_argument(
        "--environment-path", type=validate_path_environment, default="/usr/bin:/bin"
    )
    run_parser.add_argument("--timeout", type=bounded_timeout, default=1800)
    run_parser.add_argument("build_command", nargs=argparse.REMAINDER)
    run_parser.set_defaults(handler=run_comparison)

    inspect_parser = subparsers.add_parser(
        "inspect", help="validate only the structure and integrity of one record"
    )
    inspect_parser.add_argument("record", type=Path)
    inspect_parser.set_defaults(handler=inspect_command)

    validate_parser = subparsers.add_parser(
        "validate", help="check pinned inputs and report the release gate as blocked"
    )
    validate_parser.add_argument("record", type=Path)
    validate_parser.add_argument("--expect-commit", required=True)
    validate_parser.add_argument("--expect-source-tree-identity", required=True)
    validate_parser.add_argument("--expect-source-snapshot-identity", required=True)
    validate_parser.add_argument("--expect-argv-identity", required=True)
    validate_parser.add_argument("--build-executable", required=True, type=Path)
    validate_parser.add_argument("--expect-environment-identity", required=True)
    validate_parser.add_argument("--expect-llvm-toolchain-identity", required=True)
    validate_parser.add_argument("--expect-worker-identity", required=True)
    validate_parser.add_argument("--expect-request-identity", required=True)
    validate_parser.add_argument("--expect-target", required=True)
    validate_parser.set_defaults(handler=validate_command)

    matrix_parser = subparsers.add_parser(
        "inspect-matrix",
        help="structurally inspect gfx1151/gfx942/gfx950 observations",
    )
    matrix_parser.add_argument("record", nargs="+", type=Path)
    matrix_parser.set_defaults(handler=inspect_matrix)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        return args.handler(args)
    except EvidenceError as error:
        print(f"direct-link reproducibility: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
