#!/usr/bin/env python3
"""Run one manifest hardware command and emit only its authenticated archive."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
from typing import Any

import tutorial_hardware_receipt as receipt


REPO_ROOT = Path(__file__).resolve().parent.parent
PROTOCOL = "authenticated-v1"
ENVIRONMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*\Z")
MAX_PROCESS_OUTPUT = 64 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 24 * 60 * 60
REQUIRED_ENVIRONMENT = {
    "attestor identity": "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_IDENTITY",
    "attestor private key": "FE2O3_TUTORIAL_HARDWARE_ATTESTOR_PRIVATE_KEY",
    "compiler evidence root": "FE2O3_TUTORIAL_HARDWARE_EVIDENCE_ROOT",
    "pre-hardware record": "FE2O3_TUTORIAL_HARDWARE_RECORD",
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
    "driver": "FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT",
    "kir": "FE2O3_TUTORIAL_HARDWARE_KIR_INPUT",
    "llvm": "FE2O3_TUTORIAL_HARDWARE_LLVM_INPUT",
    "numericalPolicy": "FE2O3_TUTORIAL_HARDWARE_NUMERICAL_POLICY_INPUT",
    "proof": "FE2O3_TUTORIAL_HARDWARE_PROOF_INPUT",
    "proofChecker": "FE2O3_TUTORIAL_HARDWARE_PROOF_CHECKER_INPUT",
    "proofObligations": "FE2O3_TUTORIAL_HARDWARE_PROOF_OBLIGATIONS_INPUT",
    "runtime": "FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT",
    "source": "FE2O3_TUTORIAL_HARDWARE_SOURCE_INPUT",
    "target": "FE2O3_TUTORIAL_HARDWARE_TARGET_INPUT",
    "targetDecision": "FE2O3_TUTORIAL_HARDWARE_TARGET_DECISION_INPUT",
}


class RunnerError(ValueError):
    """The hardware runner did not complete the authenticated protocol."""


def fail(message: str) -> None:
    raise RunnerError(message)


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
    expected = stat.S_ISDIR(metadata.st_mode) if directory else stat.S_ISREG(metadata.st_mode)
    if path.is_symlink() or resolved != path or not expected:
        fail(f"{label} must be a real, non-symlink {'directory' if directory else 'file'}")
    return path


def repository_runner(value: str) -> tuple[Path, str]:
    supplied = Path(value)
    if not supplied.is_absolute():
        supplied = Path.cwd() / supplied
    try:
        resolved = supplied.resolve(strict=True)
    except OSError as error:
        fail(f"cannot resolve hardware runner: {error}")
    if not resolved.is_relative_to(REPO_ROOT) or not resolved.is_file() or resolved.is_symlink():
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


def observation_payload(path: Path, name: str) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        fail(f"cannot inspect typed {name} observation: {error}")
    if (
        path.is_symlink()
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
    ):
        fail(f"typed {name} observation must be a new, owned, non-linked regular file")
    return receipt._read_regular(
        path, f"{name} observation", OBSERVATION_LIMITS[name]
    )


def run_inner(
    runner: Path,
    arguments: list[str],
    timeout: int,
    environment: dict[str, str],
    stdout_path: Path,
    stderr_path: Path,
) -> None:
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        process = subprocess.Popen(
            [str(runner), *arguments],
            cwd=REPO_ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            fail(f"hardware runner exceeded its {timeout} second timeout")
    for path, label in ((stdout_path, "stdout"), (stderr_path, "stderr")):
        if path.stat().st_size > MAX_PROCESS_OUTPUT:
            fail(f"hardware runner {label} exceeded its byte bound")
    if status != 0:
        detail = stderr_path.read_bytes()[-4096:].decode("utf-8", errors="replace").strip()
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
        values["FE2O3_TUTORIAL_HARDWARE_RECORD"], "pre-hardware record"
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
    environment = {
        name: value
        for name, value in os.environ.items()
        if not name.startswith("FE2O3_TUTORIAL_HARDWARE_")
    }
    environment.update(declared_environment)
    environment["FE2O3_TUTORIAL_HARDWARE_INNER"] = "1"
    environment["FE2O3_TUTORIAL_HARDWARE_REQUEST_INPUT"] = str(request_path)
    environment["FE2O3_TUTORIAL_HARDWARE_RECORD_INPUT"] = str(record_path)
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
    for name, variable in OBSERVATION_OUTPUTS.items():
        environment[variable] = str(observations[name])
    stdout_path = scratch / "runner-stdout.bin"
    stderr_path = scratch / "runner-stderr.bin"
    archive: bytes | None = None
    prepared = False
    failure: BaseException | None = None
    try:
        run_inner(runner, arguments, timeout, environment, stdout_path, stderr_path)
        if (
            receipt._read_regular(
                request_path, "hardware transaction request", receipt.MAX_JSON_BYTES
            )
            != request_payload
            or receipt._read_regular(
                record_path, "pre-hardware transaction record", receipt.MAX_JSON_BYTES
            )
            != record_payload
        ):
            fail("hardware runner changed its transaction request or record")
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


def main(arguments: list[str] | None = None) -> int:
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
