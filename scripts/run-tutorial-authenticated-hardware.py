#!/usr/bin/env python3
"""Run one manifest hardware command and emit only its authenticated archive."""

from __future__ import annotations

import argparse
from copy import deepcopy
import hashlib
import io
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
from typing import Any

import tutorial_hardware_receipt as receipt


REPO_ROOT = Path(__file__).resolve().parent.parent
PROTOCOL = "authenticated-v1"
ENVIRONMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*\Z")
MAX_PROCESS_OUTPUT = 64 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 24 * 60 * 60
MAX_SSH_STDERR = 8 * 1024 * 1024
MAX_SOURCE_ARCHIVE = 1024 * 1024 * 1024
MAX_SOURCE_ENTRIES = 100_000
SSH_HOSTS = {"gfx942": "mi300x", "gfx950": "mi350"}
REQUIRED_ENVIRONMENT = {
    "attestor identity": "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY",
    "attestor private key": "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY",
    "compiler evidence root": "FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT",
    "pre-hardware record": "FE2O3_TUTORIAL_HARDWARE_PRE_RECORD",
    "reservation identity": "FE2O3_TUTORIAL_HARDWARE_RESERVATION",
    "transaction request": "FE2O3_TUTORIAL_HARDWARE_REQUEST",
}
OBSERVATION_OUTPUTS = {
    "isa": "FE2O3_TUTORIAL_HARDWARE_ISA_OBSERVATION_OUTPUT",
    "resource": "FE2O3_TUTORIAL_HARDWARE_RESOURCE_OBSERVATION_OUTPUT",
    "result": "FE2O3_TUTORIAL_HARDWARE_RESULT_OBSERVATION_OUTPUT",
}
OBSERVATION_LIMITS = {
    "isa": 64 * 1024 * 1024,
    "resource": 1024 * 1024,
    "result": 4 * 1024 * 1024,
}
COMPILER_INPUT_ENVIRONMENT = {
    "artifact": "FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT",
    "artifactInspection": "FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INSPECTION_INPUT",
    "compilerPolicy": "FE2O3_TUTORIAL_HARDWARE_COMPILER_POLICY_INPUT",
    "kir": "FE2O3_TUTORIAL_HARDWARE_KIR_INPUT",
    "llvm": "FE2O3_TUTORIAL_HARDWARE_LLVM_INPUT",
    "numericalPolicy": "FE2O3_TUTORIAL_HARDWARE_NUMERICAL_POLICY_INPUT",
    "proof": "FE2O3_TUTORIAL_HARDWARE_PROOF_INPUT",
    "proofChecker": "FE2O3_TUTORIAL_HARDWARE_PROOF_CHECKER_INPUT",
    "proofObligations": "FE2O3_TUTORIAL_HARDWARE_PROOF_OBLIGATIONS_INPUT",
    "source": "FE2O3_TUTORIAL_HARDWARE_SOURCE_INPUT",
    "target": "FE2O3_TUTORIAL_HARDWARE_TARGET_INPUT",
    "targetDecision": "FE2O3_TUTORIAL_HARDWARE_TARGET_DECISION_INPUT",
}
PLATFORM_INPUT_ENVIRONMENT = {
    "driver": "FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT",
    "runtime": "FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT",
}
ROCM_COMPONENTS = {
    "hipRuntime": ("lib/libamdhip64.so", 256 * 1024 * 1024),
    "hsaRuntime": ("lib/libhsa-runtime64.so.1", 256 * 1024 * 1024),
    "rocminfo": ("bin/rocminfo", 256 * 1024 * 1024),
    "rocmVersion": (".info/version", 64 * 1024),
}
TARGET_GFX_VERSIONS = {"gfx942": 90402, "gfx950": 90500}


class RunnerError(ValueError):
    """The hardware runner did not complete the authenticated protocol."""


def fail(message: str) -> None:
    raise RunnerError(message)


def canonical(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("ascii")
        + b"\n"
    )


def required_environment() -> dict[str, str]:
    values: dict[str, str] = {}
    for label, name in REQUIRED_ENVIRONMENT.items():
        value = os.environ.get(name)
        if not value:
            fail(f"missing {label}: {name}")
        values[name] = value
    if os.environ.get("FE2O3_TUTORIAL_HARDWARE_PROTOCOL") != PROTOCOL:
        fail("authenticated runner was entered without the exact protocol marker")
    return values


def regular_path(value: str, label: str, *, directory: bool = False) -> Path:
    path = Path(value)
    if not path.is_absolute() or path != Path(os.path.normpath(value)):
        fail(f"{label} must be an absolute, lexically normalized path")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve {label}: {error}")
    expected = (
        stat.S_ISDIR(metadata.st_mode) if directory else stat.S_ISREG(metadata.st_mode)
    )
    if path.is_symlink() or resolved != path or not expected:
        fail(
            f"{label} must be a real, non-symlink {'directory' if directory else 'file'}"
        )
    return path


def repository_runner(value: str) -> tuple[Path, str]:
    supplied = Path(value)
    if not supplied.is_absolute():
        supplied = Path.cwd() / supplied
    try:
        resolved = supplied.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve hardware runner: {error}")
    if (
        not resolved.is_relative_to(REPO_ROOT)
        or not resolved.is_file()
        or resolved.is_symlink()
    ):
        fail("hardware runner must be a regular file in this repository")
    if resolved.stat().st_mode & 0o111 == 0:
        fail("hardware runner is not executable")
    return resolved, resolved.relative_to(REPO_ROOT).as_posix()


def command_contract(
    request: dict[str, Any], runner_relative: str, arguments: list[str]
) -> tuple[int, dict[str, str]]:
    fixture = request.get("fixture")
    if not isinstance(fixture, dict):
        fail("transaction request fixture is malformed")
    command = fixture.get("hardwareCommand")
    if not isinstance(command, dict) or set(command) != {
        "arguments",
        "environment",
        "executable",
        "timeoutSeconds",
        "workingDirectory",
    }:
        fail("transaction request hardware command is malformed")
    if (
        command["executable"] != runner_relative
        or command["arguments"] != arguments
        or command["workingDirectory"] != "."
    ):
        fail("hardware runner path or arguments differ from the compiler request")
    timeout = command["timeoutSeconds"]
    if (
        not isinstance(timeout, int)
        or isinstance(timeout, bool)
        or timeout <= 0
        or timeout > MAX_TIMEOUT_SECONDS
    ):
        fail("hardware runner timeout is invalid")
    assignments = command["environment"]
    if not isinstance(assignments, list) or len(assignments) != len(set(assignments)):
        fail("hardware runner environment is malformed")
    declared: dict[str, str] = {}
    for assignment in assignments:
        if not isinstance(assignment, str) or ENVIRONMENT.fullmatch(assignment) is None:
            fail("hardware runner environment assignment is malformed")
        name, expected = assignment.split("=", 1)
        if name in declared:
            fail(f"hardware runner environment repeats {name}")
        if name.startswith("FE2O3_TUTORIAL_HARDWARE_") and (
            name != "FE2O3_TUTORIAL_HARDWARE_PROTOCOL" or expected != PROTOCOL
        ):
            fail(f"hardware runner command declares reserved protocol input {name}")
        if os.environ.get(name) != expected:
            fail(f"hardware runner environment differs for {name}")
        declared[name] = expected
    if declared.get("FE2O3_TUTORIAL_HARDWARE_PROTOCOL") != PROTOCOL:
        fail("legacy hardware command omitted the authenticated protocol marker")
    return timeout, declared


def object_path(root: Path, reference: Any, label: str) -> Path:
    validated = receipt.validate_reference(reference, label)
    path = root
    for component in PurePosixPath(validated["path"]).parts:
        path /= component
        if path.is_symlink():
            fail(f"{label} traverses a symlink")
    payload = receipt._load_export_object(root, validated, label)
    if receipt.sha256(payload) != validated["sha256"]:
        fail(f"{label} changed while it was exposed")
    return path.resolve(strict=True)


def compiler_inputs(record: dict[str, Any], root: Path) -> dict[str, Path]:
    files = record.get("evidenceFiles")
    if not isinstance(files, dict):
        fail("pre-hardware record evidenceFiles is malformed")
    paths: dict[str, Path] = {}
    for name, kind in receipt.COMPILER_OBSERVATION_KINDS.items():
        paths[name] = object_path(root, files.get(kind), f"compiler-bound {kind}")
    return paths


def _read_virtual_file(path: Path, label: str, maximum: int) -> bytes:
    """Read a root-owned procfs/sysfs file without trusting its reported size."""
    try:
        resolved = path.resolve(strict=True)
        descriptor = os.open(
            resolved,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode) or before.st_uid != 0:
                fail(f"{label} is not a root-owned regular virtual file")
            chunks: list[bytes] = []
            observed = 0
            while True:
                chunk = os.read(descriptor, min(64 * 1024, maximum - observed + 1))
                if not chunk:
                    break
                observed += len(chunk)
                if observed > maximum:
                    fail(f"{label} exceeds its byte bound")
                chunks.append(chunk)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
    except OSError as error:
        fail(f"cannot read {label}: {error}")
    stable_fields = ("st_dev", "st_ino", "st_mode", "st_uid", "st_gid")
    if any(getattr(before, field) != getattr(after, field) for field in stable_fields):
        fail(f"{label} changed while it was read")
    payload = b"".join(chunks)
    if not payload or b"\0" in payload:
        fail(f"{label} is empty or contains NUL bytes")
    return payload


def _decimal_properties(payload: bytes, label: str) -> dict[str, int]:
    try:
        lines = payload.decode("ascii").splitlines()
    except UnicodeDecodeError as error:
        fail(f"{label} is not ASCII: {error}")
    properties: dict[str, int] = {}
    for line in lines:
        fields = line.split()
        if len(fields) != 2 or not fields[1].isdigit() or fields[0] in properties:
            fail(f"{label} contains malformed or duplicate properties")
        properties[fields[0]] = int(fields[1], 10)
    return properties


def _driver_observation(
    request: dict[str, Any], record: dict[str, Any]
) -> bytes:
    fixture = request.get("fixture")
    challenge = request.get("hardwareReceiptChallenge")
    if not isinstance(fixture, dict) or not isinstance(challenge, dict):
        fail("hardware driver measurement received a malformed request")
    target = fixture.get("target")
    target_base = target.split(":", 1)[0] if isinstance(target, str) else ""
    expected_gfx_version = TARGET_GFX_VERSIONS.get(target_base)
    if expected_gfx_version is None:
        fail(f"hardware driver measurement does not support target {target!r}")

    device_path = Path("/dev/kfd")
    try:
        before = device_path.lstat()
        resolved_device = device_path.resolve(strict=True)
        after = device_path.lstat()
    except OSError as error:
        fail(f"cannot inspect /dev/kfd: {error}")
    if (
        resolved_device != device_path
        or not stat.S_ISCHR(before.st_mode)
        or receipt._file_identity(before) != receipt._file_identity(after)
    ):
        fail("/dev/kfd is not a stable, non-symlink character device")

    modules = _read_virtual_file(Path("/proc/modules"), "loaded kernel modules", 4 * 1024 * 1024)
    module_fields: list[str] | None = None
    for line in modules.decode("ascii").splitlines():
        fields = line.split()
        if fields and fields[0] == "amdgpu":
            if module_fields is not None or len(fields) < 6:
                fail("loaded amdgpu module inventory is malformed")
            module_fields = fields
    if (
        module_fields is None
        or not module_fields[1].isdigit()
        or not module_fields[2].isdigit()
        or module_fields[4] != "Live"
    ):
        fail("the amdgpu kernel module is not loaded and live")

    try:
        topology_root = Path("/sys/class/kfd/kfd/topology/nodes").resolve(strict=True)
        topology_metadata = topology_root.stat()
        entries = sorted(
            (entry for entry in topology_root.iterdir() if entry.name.isdigit()),
            key=lambda entry: int(entry.name),
        )
    except OSError as error:
        fail(f"cannot inspect KFD topology: {error}")
    if (
        not stat.S_ISDIR(topology_metadata.st_mode)
        or topology_metadata.st_uid != 0
        or topology_metadata.st_mode & 0o022
    ):
        fail("KFD topology is not a root-owned, non-writable directory")
    devices: list[dict[str, int]] = []
    for entry in entries:
        properties = _decimal_properties(
            _read_virtual_file(
                entry / "properties", f"KFD node {entry.name} properties", 256 * 1024
            ),
            f"KFD node {entry.name} properties",
        )
        if (
            properties.get("gfx_target_version") != expected_gfx_version
            or properties.get("vendor_id") != 0x1002
            or properties.get("simd_count", 0) <= 0
        ):
            continue
        device = {
            "deviceId": properties.get("device_id", 0),
            "gfxTargetVersion": properties["gfx_target_version"],
            "nodeId": int(entry.name),
            "simdCount": properties["simd_count"],
            "vendorId": properties["vendor_id"],
            "wavefrontSize": properties.get("wave_front_size", 0),
        }
        if device["deviceId"] <= 0 or device["wavefrontSize"] != 64:
            fail(f"KFD node {entry.name} has an incomplete GPU identity")
        devices.append(device)
    if not devices:
        fail(f"KFD topology contains no {target_base} device")

    kernel = os.uname()
    return canonical(
        {
            "authority": receipt.AUTHORITY,
            "challengeNonce": challenge.get("nonce"),
            "deviceNode": {
                "major": os.major(before.st_rdev),
                "minor": os.minor(before.st_rdev),
                "path": str(device_path),
            },
            "devices": devices,
            "kernel": {
                "machine": kernel.machine,
                "release": kernel.release,
                "system": kernel.sysname,
            },
            "module": {
                "name": "amdgpu",
                "refCount": int(module_fields[2], 10),
                "sizeBytes": int(module_fields[1], 10),
                "state": module_fields[4],
            },
            "preHardwareRecordSha256": receipt.pre_hardware_record_identity(record),
            "schema": receipt.DRIVER_OBSERVATION_SCHEMA,
            "target": target,
        }
    )


def _runtime_component(root: Path, relative: str, maximum: int) -> dict[str, Any]:
    logical = root / relative
    try:
        resolved = logical.resolve(strict=True)
        root_resolved = root.resolve(strict=True)
        metadata = resolved.stat()
    except OSError as error:
        fail(f"cannot resolve ROCm component {logical}: {error}")
    if (
        not resolved.is_relative_to(root_resolved)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_mode & 0o022
    ):
        fail(f"ROCm component {logical} escapes or is not root-controlled")
    try:
        payload = receipt._read_regular(
            resolved, f"ROCm component {logical}", maximum, expected_owner=0
        )
    except receipt.HardwareReceiptError as error:
        fail(str(error))
    return {
        "bytes": len(payload),
        "path": str(logical),
        "resolvedPath": str(resolved),
        "sha256": receipt.sha256(payload),
    }


def _runtime_observation(
    request: dict[str, Any], record: dict[str, Any], execution_environment: dict[str, str]
) -> bytes:
    fixture = request.get("fixture")
    challenge = request.get("hardwareReceiptChallenge")
    if not isinstance(fixture, dict) or not isinstance(challenge, dict):
        fail("hardware runtime measurement received a malformed request")
    root_value = execution_environment.get("ROCM_PATH", "/opt/rocm")
    root = Path(root_value)
    if not root.is_absolute() or root != Path(os.path.normpath(root_value)):
        fail("ROCM_PATH must be absolute and lexically normalized")
    try:
        root_resolved = root.resolve(strict=True)
        metadata = root_resolved.stat()
    except OSError as error:
        fail(f"cannot resolve ROCm root: {error}")
    if (
        not root_resolved.is_relative_to(Path("/opt"))
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_mode & 0o022
    ):
        fail("ROCm root is not a root-owned, non-writable /opt directory")
    components = {
        name: _runtime_component(root, relative, maximum)
        for name, (relative, maximum) in ROCM_COMPONENTS.items()
    }
    version_path = Path(components["rocmVersion"]["resolvedPath"])
    try:
        release = receipt._read_regular(
            version_path, "ROCm release", ROCM_COMPONENTS["rocmVersion"][1], expected_owner=0
        ).decode("ascii").strip()
    except (receipt.HardwareReceiptError, UnicodeDecodeError) as error:
        fail(f"cannot read ROCm release: {error}")
    if not release or len(release) > 128 or any(character.isspace() for character in release):
        fail("ROCm release is malformed")
    return canonical(
        {
            "authority": receipt.AUTHORITY,
            "challengeNonce": challenge.get("nonce"),
            "components": components,
            "preHardwareRecordSha256": receipt.pre_hardware_record_identity(record),
            "rocmRelease": release,
            "rocmRoot": {"path": str(root), "resolvedPath": str(root_resolved)},
            "schema": receipt.RUNTIME_OBSERVATION_SCHEMA,
            "target": fixture.get("target"),
        }
    )


def measure_platform_observations(
    request: dict[str, Any], record: dict[str, Any], execution_environment: dict[str, str]
) -> dict[str, bytes]:
    try:
        receipt._validate_pre_hardware_record(request, record)
        observations = {
            "driver": _driver_observation(request, record),
            "runtime": _runtime_observation(request, record, execution_environment),
        }
        references = {
            name: {**receipt.object_reference(payload), "_payload": payload}
            for name, payload in observations.items()
        }
        receipt._validate_platform_observations(request, record, references)
        return observations
    except receipt.HardwareReceiptError as error:
        fail(str(error))


def _write_private(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
            os.fchmod(output.fileno(), 0o400)
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def observation_payload(path: Path, name: str) -> bytes:
    try:
        return receipt._read_regular(
            path,
            f"typed {name} observation",
            OBSERVATION_LIMITS[name],
            expected_owner=os.geteuid(),
            expected_links=1,
        )
    except receipt.HardwareReceiptError as error:
        fail(str(error))


def _kill_process_group(process: subprocess.Popen[Any]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def _reject_live_descendants(process: subprocess.Popen[Any], label: str) -> None:
    try:
        os.killpg(process.pid, 0)
    except ProcessLookupError:
        return
    except PermissionError:
        pass
    _kill_process_group(process)
    fail(f"{label} left a live descendant process")


def run_inner(
    runner: Path,
    arguments: list[str],
    timeout: int,
    environment: dict[str, str],
    stdout_path: Path,
    stderr_path: Path,
) -> None:
    with stdout_path.open("x+b") as stdout, stderr_path.open("x+b") as stderr:
        process = subprocess.Popen(
            [str(runner), *arguments],
            cwd=REPO_ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout
        reason: str | None = None
        while process.poll() is None:
            if os.fstat(stdout.fileno()).st_size > MAX_PROCESS_OUTPUT:
                reason = "hardware runner stdout exceeded its byte bound"
                break
            if os.fstat(stderr.fileno()).st_size > MAX_PROCESS_OUTPUT:
                reason = "hardware runner stderr exceeded its byte bound"
                break
            if time.monotonic() >= deadline:
                reason = f"hardware runner exceeded its {timeout} second timeout"
                break
            time.sleep(0.05)
        if reason is not None:
            _kill_process_group(process)
            fail(reason)
        status = process.wait()
        if os.fstat(stdout.fileno()).st_size > MAX_PROCESS_OUTPUT:
            fail("hardware runner stdout exceeded its byte bound")
        if os.fstat(stderr.fileno()).st_size > MAX_PROCESS_OUTPUT:
            fail("hardware runner stderr exceeded its byte bound")
        _reject_live_descendants(process, "hardware runner")
        if status != 0:
            stderr.seek(max(0, os.fstat(stderr.fileno()).st_size - 4096))
            detail = stderr.read().decode("utf-8", errors="replace").strip()
            fail(
                f"hardware runner failed with status {status}"
                + (f": {detail}" if detail else "")
            )


def execute(runner_argument: str, arguments: list[str]) -> bytes:
    values = required_environment()
    runner, runner_relative = repository_runner(runner_argument)
    request_path = regular_path(
        values["FE2O3_TUTORIAL_HARDWARE_REQUEST"], "transaction request"
    )
    record_path = regular_path(
        values["FE2O3_TUTORIAL_HARDWARE_PRE_RECORD"], "pre-hardware record"
    )
    evidence_root = regular_path(
        values["FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT"],
        "compiler evidence root",
        directory=True,
    )
    private_key = regular_path(
        values["FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY"],
        "attestor private key",
    )
    request_payload = receipt._read_regular(
        request_path, "hardware transaction request", receipt.MAX_JSON_BYTES
    )
    record_payload = receipt._read_regular(
        record_path, "pre-hardware transaction record", receipt.MAX_JSON_BYTES
    )
    request = receipt._load_json_file(request_path, "hardware transaction request")
    record = receipt._load_json_file(record_path, "pre-hardware transaction record")
    timeout, declared_environment = command_contract(
        request, runner_relative, arguments
    )
    fixture = request.get("fixture", {})
    reservation = values["FE2O3_TUTORIAL_HARDWARE_RESERVATION"]
    if fixture.get("hardwareReservation") != reservation:
        fail("explicit hardware reservation differs from the compiler request")
    exposed_inputs = compiler_inputs(record, evidence_root)
    environment = {
        name: value
        for name, value in os.environ.items()
        if not name.startswith("FE2O3_TUTORIAL_HARDWARE_")
    }
    environment.update(declared_environment)
    for name in ("HIP_PATH", "HSA_PATH", "LD_LIBRARY_PATH", "LD_PRELOAD", "ROCM_PATH"):
        if name not in declared_environment:
            environment.pop(name, None)
    environment.setdefault("ROCM_PATH", "/opt/rocm")
    platform_payloads = measure_platform_observations(
        request, record, environment
    )
    runner_record = deepcopy(record)
    runner_record.pop("preHardwareBindingSha256")
    runner_record["preHardwareRecordSha256"] = receipt.pre_hardware_record_identity(
        record
    )
    runner_record["schema"] = "fe2o3-tutorial-hardware-runner-record-view-v1"
    runner_record["hardware"].update(
        {
            "driverIdentitySha256": receipt.sha256(platform_payloads["driver"]),
            "runtimeIdentitySha256": receipt.sha256(platform_payloads["runtime"]),
        }
    )
    runner_record_payload = canonical(runner_record)

    temporary_parent_value = os.environ.get("FE2O3_TUTORIAL_HARDWARE_TEMP_ROOT")
    temporary_parent = (
        regular_path(temporary_parent_value, "hardware temporary root", directory=True)
        if temporary_parent_value
        else Path(tempfile.gettempdir()).resolve(strict=True)
    )
    if temporary_parent.is_relative_to(REPO_ROOT):
        fail("hardware temporary root must be outside the repository")
    session = Path(tempfile.mkdtemp(prefix="fe2o3-hardware-", dir=temporary_parent))
    session.chmod(0o700)
    scratch = session / "scratch"
    spool = session / "spool"
    scratch.mkdir(mode=0o700)
    observations = {
        name: scratch / f"{name}-observation-v1.json" for name in OBSERVATION_OUTPUTS
    }
    platform_paths = {
        name: scratch / f"{name}-observation-v1.json"
        for name in PLATFORM_INPUT_ENVIRONMENT
    }
    runner_record_path = scratch / "runner-record-view-v1.json"
    environment["FE2O3_TUTORIAL_HARDWARE_INNER"] = "1"
    environment["FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT"] = str(request_path)
    environment["FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT"] = str(runner_record_path)
    environment["FE2O3_TUTORIAL_HARDWARE_SCRATCH"] = str(scratch)
    environment["FE2O3_RUN_SCRATCH_ROOT"] = str(scratch)
    environment["FE2O3_OUTPUT_DIR"] = str(scratch / "artifacts")
    environment["FE2O3_GFX950_ADVANCED_OUTPUT_DIR"] = str(scratch / "artifacts")
    for name in (
        "FE2O3_GFX950_FP4_OUTPUT_DIR",
        "FE2O3_GFX950_FP8_OUTPUT_DIR",
    ):
        environment[name] = str(scratch / "artifacts")
    for name, variable in COMPILER_INPUT_ENVIRONMENT.items():
        environment[variable] = str(exposed_inputs[name])
    for name, variable in PLATFORM_INPUT_ENVIRONMENT.items():
        environment[variable] = str(platform_paths[name])
    for name, variable in OBSERVATION_OUTPUTS.items():
        environment[variable] = str(observations[name])
    stdout_path = scratch / "runner-stdout.bin"
    stderr_path = scratch / "runner-stderr.bin"
    archive: bytes | None = None
    prepared = False
    failure: BaseException | None = None
    try:
        _write_private(runner_record_path, runner_record_payload)
        for name, path in platform_paths.items():
            _write_private(path, platform_payloads[name])
        run_inner(runner, arguments, timeout, environment, stdout_path, stderr_path)
        if (
            receipt._read_regular(
                request_path, "hardware transaction request", receipt.MAX_JSON_BYTES
            )
            != request_payload
            or receipt._read_regular(
                record_path, "pre-hardware transaction record", receipt.MAX_JSON_BYTES
            ) != record_payload
        ):
            fail("hardware runner changed its transaction request or record")
        if receipt._read_regular(
            runner_record_path,
            "hardware runner record view",
            receipt.MAX_JSON_BYTES,
            expected_owner=os.geteuid(),
            expected_links=1,
        ) != runner_record_payload:
            fail("hardware runner changed its record view")
        for name, path in platform_paths.items():
            if receipt._read_regular(
                path,
                f"fresh {name} observation",
                receipt.MAX_JSON_BYTES,
                expected_owner=os.geteuid(),
                expected_links=1,
            ) != platform_payloads[name]:
                fail(f"hardware runner changed its fresh {name} observation")
        if compiler_inputs(record, evidence_root) != exposed_inputs:
            fail("hardware runner substituted compiler-bound evidence")
        for name, path in observations.items():
            if not path.exists() or path.is_symlink() or not path.is_file():
                fail(
                    f"hardware runner omitted typed {name} observation; "
                    "process success alone is not qualification"
                )
        observation_payloads = {
            name: observation_payload(path, name) for name, path in observations.items()
        }
        receipt.prepare_transport(
            request,
            record,
            evidence_root,
            observation_payloads["isa"],
            observation_payloads["resource"],
            observation_payloads["result"],
            scratch,
            spool,
            values["FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY"],
            private_key,
            driver_observation=platform_payloads["driver"],
            runtime_observation=platform_payloads["runtime"],
        )
        prepared = True
    except BaseException as error:
        failure = error
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
        if prepared:
            try:
                archive = receipt.finalize_transport(
                    spool, scratch, private_key, request, record, evidence_root
                )
            except BaseException as error:
                failure = failure or error
        shutil.rmtree(spool, ignore_errors=True)
        shutil.rmtree(session, ignore_errors=True)
    if session.exists() or spool.exists() or scratch.exists():
        fail("hardware adapter left remote scratch or spool residue")
    if failure is not None:
        raise failure
    if archive is None:
        fail("hardware adapter did not finalize an authenticated archive")
    return archive


def _bounded_process(
    command: list[str],
    *,
    cwd: Path,
    stdout_path: Path,
    stderr_path: Path,
    timeout: int,
    stdout_limit: int,
) -> None:
    with stdout_path.open("x+b") as stdout, stderr_path.open("x+b") as stderr:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout
        reason = None
        while process.poll() is None:
            if os.fstat(stdout.fileno()).st_size > stdout_limit:
                reason = "stdout exceeded its byte bound"
                break
            if os.fstat(stderr.fileno()).st_size > MAX_SSH_STDERR:
                reason = "stderr exceeded its byte bound"
                break
            if time.monotonic() >= deadline:
                reason = f"process exceeded its {timeout} second timeout"
                break
            time.sleep(0.05)
        if reason is not None:
            _kill_process_group(process)
            fail(reason)
        status = process.wait()
        if os.fstat(stdout.fileno()).st_size > stdout_limit:
            fail("stdout exceeded its byte bound")
        if os.fstat(stderr.fileno()).st_size > MAX_SSH_STDERR:
            fail("stderr exceeded its byte bound")
        _reject_live_descendants(process, "remote hardware command")
        if status != 0:
            stderr.seek(max(0, os.fstat(stderr.fileno()).st_size - 4096))
            detail = stderr.read().decode("utf-8", errors="replace").strip()
            fail(
                f"remote hardware command failed with status {status}"
                + (f": {detail}" if detail else "")
            )


def _remote_path(value: str, label: str) -> str:
    path = PurePosixPath(value)
    if (
        not path.is_absolute()
        or value != str(path)
        or ".." in path.parts
        or any(character in value for character in ("\0", "\n", "\r"))
    ):
        fail(f"{label} must be an absolute normalized remote path")
    return value


def _ssh_command(host: str, script: str) -> list[str]:
    if host not in SSH_HOSTS.values():
        fail("SSH host is not a configured hardware lane")
    return [
        "ssh",
        "-oBatchMode=yes",
        "-oConnectTimeout=20",
        "-oServerAliveInterval=15",
        "-oServerAliveCountMax=2",
        "-oLogLevel=ERROR",
        host,
        f"sh -c {shlex.quote(script)}",
    ]


class RemoteHardwareSession:
    """Reuse one exact candidate deployment while isolating every hardware attempt."""

    def __init__(
        self,
        repository: Path,
        workspace: Path,
        candidate: dict[str, Any],
        remote_private_keys: dict[str, str],
    ) -> None:
        self.repository = repository.resolve(strict=True)
        self.workspace = workspace.resolve(strict=True)
        self.candidate = candidate
        self.remote_private_keys = {
            host: _remote_path(path, f"{host} attestor private key")
            for host, path in remote_private_keys.items()
        }
        self.source_archive: Path | None = None
        self.source_sha256: str | None = None
        self.roots: dict[str, str] = {}

    def __enter__(self) -> RemoteHardwareSession:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def _run_remote(
        self,
        host: str,
        script: str,
        label: str,
        *,
        timeout: int = 120,
        stdout_limit: int = 1024 * 1024,
    ) -> bytes:
        token = hashlib.sha256(
            f"{host}:{label}:{time.monotonic_ns()}".encode()
        ).hexdigest()[:16]
        stdout = self.workspace / f".{token}.stdout"
        stderr = self.workspace / f".{token}.stderr"
        try:
            _bounded_process(
                _ssh_command(host, script),
                cwd=self.repository,
                stdout_path=stdout,
                stderr_path=stderr,
                timeout=timeout,
                stdout_limit=stdout_limit,
            )
            return stdout.read_bytes()
        finally:
            stdout.unlink(missing_ok=True)
            stderr.unlink(missing_ok=True)

    def _copy_remote(self, source: Path, host: str, destination: str) -> None:
        if host not in SSH_HOSTS.values():
            fail("SCP host is not a configured hardware lane")
        destination = _remote_path(destination, "SCP destination")
        if re.fullmatch(r"/[A-Za-z0-9_./-]+", destination) is None:
            fail("SCP destination contains shell-significant characters")
        payload = receipt._read_regular(source, "SCP source", MAX_SOURCE_ARCHIVE)
        digest = hashlib.sha256(payload).hexdigest()
        stdout = self.workspace / f".{source.name}.scp.stdout"
        stderr = self.workspace / f".{source.name}.scp.stderr"
        try:
            _bounded_process(
                [
                    "scp",
                    "-q",
                    "-oBatchMode=yes",
                    "-oConnectTimeout=20",
                    "-oLogLevel=ERROR",
                    str(source),
                    f"{host}:{destination}",
                ],
                cwd=self.repository,
                stdout_path=stdout,
                stderr_path=stderr,
                timeout=600,
                stdout_limit=1024,
            )
            self._run_remote(
                host,
                "set -eu; "
                f"test -f {shlex.quote(destination)}; test ! -L {shlex.quote(destination)}; "
                f"printf '%s  %s\\n' {shlex.quote(digest)} {shlex.quote(destination)} "
                "| sha256sum -c - >/dev/null",
                "verify-transfer",
            )
        finally:
            stdout.unlink(missing_ok=True)
            stderr.unlink(missing_ok=True)

    def _build_source_archive(self) -> tuple[Path, str]:
        if self.source_archive is not None and self.source_sha256 is not None:
            return self.source_archive, self.source_sha256
        commit = self.candidate.get("compilerCommit")
        tree = self.candidate.get("compilerTree")
        for arguments, expected, label in (
            (("rev-parse", "--verify", "HEAD"), commit, "commit"),
            (("show", "-s", "--format=%T", "HEAD"), tree, "tree"),
        ):
            result = subprocess.run(
                ["git", "-C", str(self.repository), *arguments],
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            if result.returncode != 0 or result.stdout.strip() != expected:
                fail(f"candidate {label} changed before hardware deployment")
        modes = subprocess.run(
            ["git", "-C", str(self.repository), "ls-files", "-s"],
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if modes.returncode != 0 or any(
            line.startswith(("120000 ", "160000 "))
            for line in modes.stdout.splitlines()
        ):
            fail("candidate source contains an unsupported symlink or submodule")
        archive = self.workspace / "candidate-source.tar"
        stderr = self.workspace / ".candidate-source.stderr"
        with archive.open("xb") as output, stderr.open("xb") as errors:
            result = subprocess.run(
                [
                    "git",
                    "-C",
                    str(self.repository),
                    "archive",
                    "--format=tar",
                    "--prefix=source/",
                    str(commit),
                ],
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=output,
                stderr=errors,
            )
        if (
            result.returncode != 0
            or not 0 < archive.stat().st_size <= MAX_SOURCE_ARCHIVE
        ):
            detail = stderr.read_text(errors="replace")[-4096:].strip()
            fail(
                "cannot archive exact candidate source"
                + (f": {detail}" if detail else "")
            )
        stderr.unlink(missing_ok=True)
        with tarfile.open(archive, "r:") as source:
            members = source.getmembers()
            names = [member.name for member in members]
            if len(members) > MAX_SOURCE_ENTRIES or len(names) != len(set(names)):
                fail("candidate source archive inventory is invalid")
            for member in members:
                path = PurePosixPath(member.name)
                if (
                    not (member.isfile() or member.isdir())
                    or path.is_absolute()
                    or ".." in path.parts
                    or not path.parts
                    or path.parts[0] != "source"
                ):
                    fail("candidate source archive contains an unsafe entry")
        with archive.open("rb") as payload:
            digest = hashlib.file_digest(payload, "sha256").hexdigest()
        self.source_archive = archive
        self.source_sha256 = digest
        return archive, digest

    def _ensure_host(self, host: str) -> str:
        if host in self.roots:
            return self.roots[host]
        if host not in SSH_HOSTS.values() or host not in self.remote_private_keys:
            fail(f"no operator private-key path was supplied for hardware host {host}")
        archive, digest = self._build_source_archive()
        root = f"/tmp/fe2o3-tutorial-{os.getuid()}-{hashlib.sha256(os.urandom(32)).hexdigest()[:24]}"
        remote_archive = f"{root}/candidate-source.tar"
        try:
            self._run_remote(
                host,
                f"umask 077; test ! -e {shlex.quote(root)}; mkdir -- {shlex.quote(root)}",
                "create-session",
            )
            self._copy_remote(archive, host, remote_archive)
            self._run_remote(
                host,
                "set -eu; "
                f"cd -- {shlex.quote(root)}; "
                f"printf '%s  %s\\n' {shlex.quote(digest)} candidate-source.tar | sha256sum -c - >/dev/null; "
                "tar --no-same-owner --no-same-permissions -xf candidate-source.tar; "
                "rm -f -- candidate-source.tar; test -d source",
                "deploy-candidate",
                timeout=600,
            )
        except BaseException:
            try:
                self._remove(host, root)
            finally:
                raise
        self.roots[host] = root
        return root

    def _payload_archive(
        self,
        request: dict[str, Any],
        record: dict[str, Any],
        evidence_root: Path,
        name: str,
    ) -> Path:
        entries: dict[str, bytes] = {
            "request-v1.json": canonical(request),
            "pre-hardware-record-v1.json": canonical(record),
        }
        files = record.get("evidenceFiles")
        if not isinstance(files, dict):
            fail("pre-hardware record evidenceFiles is malformed")
        for kind in receipt.COMPILER_OBSERVATION_KINDS.values():
            reference = receipt.validate_reference(
                files.get(kind), f"compiler-bound {kind}"
            )
            payload = receipt._load_export_object(
                evidence_root, reference, f"compiler-bound {kind}"
            )
            entries[f"evidence/{reference['path']}"] = payload
        archive = self.workspace / name
        directories = {
            str(parent)
            for path in entries
            for parent in PurePosixPath(path).parents
            if str(parent) != "."
        }
        with tarfile.open(archive, "x:") as output:
            for directory in sorted(
                directories, key=lambda value: (value.count("/"), value)
            ):
                info = tarfile.TarInfo(directory)
                info.type = tarfile.DIRTYPE
                info.mode = 0o700
                info.mtime = 0
                output.addfile(info)
            for path, payload in sorted(entries.items()):
                info = tarfile.TarInfo(path)
                info.size = len(payload)
                info.mode = 0o600
                info.mtime = 0
                output.addfile(info, io.BytesIO(payload))
        if not 0 < archive.stat().st_size <= MAX_SOURCE_ARCHIVE:
            archive.unlink(missing_ok=True)
            fail("hardware payload archive exceeds its byte bound")
        return archive

    def _remove(self, host: str, path: str) -> None:
        self._run_remote(
            host,
            f"rm -rf -- {shlex.quote(path)}; test ! -e {shlex.quote(path)}; test ! -L {shlex.quote(path)}",
            "terminal-cleanup",
        )

    def run(
        self,
        request: dict[str, Any],
        record: dict[str, Any],
        evidence_root: Path,
        attestor_identity: str,
    ) -> bytes:
        fixture = request.get("fixture")
        if not isinstance(fixture, dict):
            fail("hardware request fixture is malformed")
        target = fixture.get("target")
        host = SSH_HOSTS.get(target)
        if host is None or fixture.get("hardwareLane") != host:
            fail("hardware target does not map to its required SSH host")
        root = self._ensure_host(host)
        fixture_id = receipt._identity(
            fixture.get("fixtureId"), "hardware fixture identity"
        )
        attempt_name = f"{fixture_id}-{hashlib.sha256(os.urandom(32)).hexdigest()[:20]}"
        attempt = f"{root}/attempts/{attempt_name}"
        remote_payload = f"{root}/{attempt_name}.tar"
        payload = self._payload_archive(
            request, record, evidence_root, f"{attempt_name}.tar"
        )
        timeout = fixture.get("hardwareCommand", {}).get("timeoutSeconds")
        if (
            not isinstance(timeout, int)
            or isinstance(timeout, bool)
            or not 0 < timeout <= MAX_TIMEOUT_SECONDS
        ):
            fail("hardware request timeout is invalid")
        command = fixture["hardwareCommand"]
        runner = PurePosixPath(command["executable"])
        if runner.is_absolute() or ".." in runner.parts:
            fail("hardware runner path is not a safe repository-relative path")
        environment = {
            assignment.split("=", 1)[0]: assignment.split("=", 1)[1]
            for assignment in command["environment"]
        }
        environment.update(
            {
                "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY": attestor_identity,
                "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY": self.remote_private_keys[
                    host
                ],
                "FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT": f"{attempt}/evidence",
                "FE2O3_TUTORIAL_HARDWARE_PRE_RECORD": f"{attempt}/pre-hardware-record-v1.json",
                "FE2O3_TUTORIAL_HARDWARE_PROTOCOL": PROTOCOL,
                "FE2O3_TUTORIAL_HARDWARE_REQUEST": f"{attempt}/request-v1.json",
                "FE2O3_TUTORIAL_HARDWARE_RESERVATION": fixture["hardwareReservation"],
                "FE2O3_TUTORIAL_HARDWARE_TEMP_ROOT": attempt,
            }
        )
        invocation = [
            "env",
            *(f"{key}={value}" for key, value in sorted(environment.items())),
            "python3",
            f"{root}/source/scripts/run-tutorial-authenticated-hardware.py",
            "--runner",
            f"{root}/source/{runner}",
            "--",
            *command["arguments"],
        ]
        shell_invocation = " ".join(shlex.quote(argument) for argument in invocation)
        script = (
            "set -eu; umask 077; "
            f"attempt={shlex.quote(attempt)}; payload={shlex.quote(remote_payload)}; "
            'cleanup() { rm -rf -- "$attempt"; rm -f -- "$payload"; }; '
            "trap cleanup EXIT HUP INT TERM; "
            'test ! -e "$attempt"; mkdir -p -- "$(dirname -- "$attempt")"; '
            'mkdir -- "$attempt"; tar --no-same-owner --no-same-permissions -xf "$payload" -C "$attempt"; '
            'rm -f -- "$payload"; '
            f"cd -- {shlex.quote(root + '/source')}; ulimit -f 2097152; {shell_invocation}"
        )
        archive_path = self.workspace / f"{attempt_name}.zip"
        stderr_path = self.workspace / f".{attempt_name}.stderr"
        try:
            self._copy_remote(payload, host, remote_payload)
            _bounded_process(
                _ssh_command(host, script),
                cwd=self.repository,
                stdout_path=archive_path,
                stderr_path=stderr_path,
                timeout=min(MAX_TIMEOUT_SECONDS, timeout + 120),
                stdout_limit=receipt.MAX_ARCHIVE_BYTES,
            )
            return receipt._read_regular(
                archive_path, "remote hardware archive", receipt.MAX_ARCHIVE_BYTES
            )
        finally:
            cleanup_failures: list[str] = []
            for path in (attempt, remote_payload):
                try:
                    self._remove(host, path)
                except BaseException as error:
                    cleanup_failures.append(f"{path}: {error}")
            for path in (payload, archive_path, stderr_path):
                try:
                    path.unlink(missing_ok=True)
                except OSError as error:
                    cleanup_failures.append(f"{path}: {error}")
            if cleanup_failures:
                fail(
                    "remote hardware attempt cleanup failed: "
                    + "; ".join(cleanup_failures)
                )

    def close(self) -> None:
        failures: list[str] = []
        for host, root in list(self.roots.items()):
            try:
                self._remove(host, root)
            except BaseException as error:
                failures.append(f"{host}: {error}")
        self.roots.clear()
        if self.source_archive is not None:
            self.source_archive.unlink(missing_ok=True)
        if failures:
            fail("remote session cleanup failed: " + "; ".join(failures))


def dispatch_main(arguments: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Dispatch one compiler-only record to its target-matched SSH lane."
    )
    parser.add_argument("--attestor-identity", required=True)
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--pre-hardware-record", required=True, type=Path)
    parser.add_argument("--remote-private-key", required=True)
    parser.add_argument("--repository", default=REPO_ROOT, type=Path)
    parser.add_argument("--request", required=True, type=Path)
    parser.add_argument("--ssh-host", required=True, choices=sorted(SSH_HOSTS.values()))
    options = parser.parse_args(arguments)
    try:
        if options.output.exists():
            fail("hardware archive output must be a new path")
        request = receipt._load_json_file(
            options.request, "hardware transaction request"
        )
        record = receipt._load_json_file(
            options.pre_hardware_record, "pre-hardware transaction record"
        )
        target = request.get("fixture", {}).get("target")
        if SSH_HOSTS.get(target) != options.ssh_host:
            fail("requested SSH host is not the target-matched hardware lane")
        candidate = request.get("candidate")
        if not isinstance(candidate, dict):
            fail("hardware request candidate is malformed")
        with tempfile.TemporaryDirectory(
            prefix=".fe2o3-hardware-dispatch-", dir=options.output.parent
        ) as temporary:
            with RemoteHardwareSession(
                options.repository,
                Path(temporary),
                candidate,
                {options.ssh_host: options.remote_private_key},
            ) as session:
                archive = session.run(
                    request,
                    record,
                    options.evidence_root.resolve(strict=True),
                    options.attestor_identity,
                )
            with options.output.open("xb") as output:
                output.write(archive)
                output.flush()
                os.fsync(output.fileno())
    except (
        OSError,
        RunnerError,
        receipt.HardwareReceiptError,
        subprocess.SubprocessError,
    ) as error:
        print(f"tutorial authenticated hardware dispatch: {error}", file=sys.stderr)
        return 1
    return 0


def main(arguments: list[str] | None = None) -> int:
    arguments = list(sys.argv[1:] if arguments is None else arguments)
    if arguments[:1] == ["dispatch"]:
        return dispatch_main(arguments[1:])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runner", required=True)
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args(arguments)
    runner_arguments = options.arguments
    if runner_arguments[:1] == ["--"]:
        runner_arguments = runner_arguments[1:]
    try:
        archive = execute(options.runner, runner_arguments)
    except (
        OSError,
        RunnerError,
        receipt.HardwareReceiptError,
        subprocess.SubprocessError,
    ) as error:
        print(f"tutorial authenticated hardware runner: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
