#!/usr/bin/env python3
"""Produce an all-or-nothing tutorial capability qualification batch.

The orchestrator owns scheduling, bounded archival, and batch publication. It
does not manufacture production records. One compiler-owned transaction export
is required for every manifest fixture, and the sealed compiler verifier must
authenticate the assembled batch before either output is published.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import secrets
import stat
import subprocess
import sys
import tempfile
from typing import Any, Callable, Iterator


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_RELATIVE = "config/tutorial-kernel-manifest-v1.json"
ROADMAP_ISSUE = "https://github.com/harsh-nod/fe2o3/issues/272"
REQUEST_SCHEMA = "fe2o3-tutorial-production-transaction-request-v1"
EXPORT_SCHEMA = "fe2o3-tutorial-production-transaction-export-v1"
HARDWARE_SCHEMA = "fe2o3-tutorial-hardware-qualification-evidence-v1"
BATCH_SCHEMA = "fe2o3-tutorial-capability-qualification-batch-v1"
TRANSACTION_EXPORT_API = (
    "rustc-codegen-fe2o3::production_pipeline::"
    "produce_tutorial_capability_qualification_transaction_v1"
)
HARDWARE_VERIFICATION_API = (
    "rustc-codegen-fe2o3::verify_tutorial_hardware_qualification_receipt_v1"
)
TRANSACTION_EXPORT_BINARY = "fe2o3-produce-tutorial-production-transaction-v1"
RESULT_NAME = "transaction-export-v1.json"
EXPECTED_FIXTURE_COUNT = 47
REQUEST_DOMAIN = b"fe2o3-tutorial-production-transaction-request-v1\0"
OBJECT_PREFIX = PurePosixPath("objects/sha256")
MAX_JSON_BYTES = 64 * 1024 * 1024
MAX_FILE_BYTES = 2 * 1024 * 1024 * 1024
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024 * 1024
MAX_ARCHIVE_FILES = 4096
MAX_PROCESS_STDOUT = 1024 * 1024
MAX_PROCESS_STDERR = 8 * 1024 * 1024
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
ENVIRONMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.+\Z")
HARDWARE_LANES = {"gfx942": "mi300x", "gfx950": "mi350"}
SCRUBBED_ENVIRONMENT = {
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUNNER",
    "FE2O3_AMDGCN_TARGET",
    "FE2O3_CODEGEN_PIPELINE",
    "FE2O3_QUALIFICATION_ORACLE",
    "RUSTC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTFLAGS",
}


class QualificationProducerError(ValueError):
    """The complete production qualification batch cannot be produced."""


def _load_sibling(module_name: str, filename: str):
    path = Path(__file__).resolve().parent / filename
    specification = importlib.util.spec_from_file_location(module_name, path)
    if specification is None or specification.loader is None:
        raise RuntimeError(f"cannot load required script {path}")
    module = importlib.util.module_from_spec(specification)
    sys.modules[module_name] = module
    specification.loader.exec_module(module)
    return module


manifest_contract = _load_sibling(
    "fe2o3_tutorial_kernel_manifest_for_qualification",
    "tutorial_kernel_manifest.py",
)
promotion_contract = _load_sibling(
    "fe2o3_tutorial_capability_promotion_for_qualification",
    "promote-tutorial-capabilities.py",
)
hardware_receipt_contract = _load_sibling(
    "fe2o3_tutorial_hardware_receipt_for_qualification",
    "tutorial_hardware_receipt.py",
)


def _fail(message: str) -> None:
    raise QualificationProducerError(message)


def _canonical(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeError) as error:
        _fail(f"cannot encode canonical JSON: {error}")


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _domain_sha256(domain: bytes, value: Any) -> str:
    return _sha256(domain + _canonical(value))


def _decode_unique(payload: bytes, label: str) -> Any:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        return json.loads(payload, object_pairs_hook=reject_duplicates)
    except (UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot decode {label}: {error}")


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        _fail(
            f"{label} fields differ: missing={sorted(keys - set(value))!r} "
            f"extra={sorted(set(value) - keys)!r}"
        )


def _sha(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} is not a nonzero lowercase SHA-256 identity")
    return value


def _read_regular(path: Path, label: str, maximum: int = MAX_JSON_BYTES) -> bytes:
    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
            _fail(f"{label} must be a regular non-symlink file")
        if metadata.st_size <= 0 or metadata.st_size > maximum:
            _fail(f"{label} has an invalid byte length")
        flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
        descriptor = os.open(path, flags)
        try:
            before = os.fstat(descriptor)
            chunks: list[bytes] = []
            observed = 0
            while True:
                chunk = os.read(descriptor, min(1024 * 1024, maximum - observed + 1))
                if not chunk:
                    break
                observed += len(chunk)
                if observed > maximum:
                    _fail(f"{label} exceeds its byte bound")
                chunks.append(chunk)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
    except OSError as error:
        _fail(f"cannot read {label}: {error}")
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) == (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    if not stable or observed != metadata.st_size:
        _fail(f"{label} changed while it was read")
    return b"".join(chunks)


def _load_canonical_json(path: Path, label: str) -> dict[str, Any]:
    payload = _read_regular(path, label)
    document = _object(_decode_unique(payload, label), label)
    if payload != _canonical(document) + b"\n":
        _fail(f"{label} is not canonical JSON followed by one newline")
    return document


def _operator_policy_path(repository: Path, path: Path) -> Path:
    """Retain the exact operator-selected policy path across the Rust boundary."""
    if not path.is_absolute() or path != Path(os.path.normpath(str(path))):
        _fail("hardware trust policy path must be absolute and lexically normalized")
    try:
        metadata = path.lstat()
        resolved = path.resolve(strict=True)
    except OSError as error:
        _fail(f"cannot resolve hardware trust policy: {error}")
    if path.is_symlink() or resolved != path or not stat.S_ISREG(metadata.st_mode):
        _fail("hardware trust policy must be an exact regular non-symlink file")
    if path.is_relative_to(repository):
        _fail("hardware trust policy must be operator-provisioned outside the worktree")
    return path


def _git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        _fail(f"git {' '.join(arguments)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def clean_candidate(repository: Path) -> dict[str, Any]:
    if _git(repository, "status", "--porcelain=v1", "--untracked-files=all"):
        _fail("tutorial qualification requires a clean compiler worktree")
    return {
        "compilerCommit": _git(repository, "rev-parse", "--verify", "HEAD"),
        "compilerTree": _git(repository, "show", "-s", "--format=%T", "HEAD"),
        "worktreeClean": True,
    }


def _manifest_identity(document: dict[str, Any], payload: bytes) -> dict[str, str]:
    return {
        "corpusContractSha256": promotion_contract.corpus_contract_sha256(document),
        "path": MANIFEST_RELATIVE,
        "rawSha256": _sha256(payload),
    }


def _object_relative(sha256: str) -> str:
    digest = _sha(sha256, "object identity")
    return str(OBJECT_PREFIX / digest[:2] / digest)


def _reference(payload: bytes) -> dict[str, Any]:
    digest = _sha256(payload)
    return {"bytes": len(payload), "path": _object_relative(digest), "sha256": digest}


def _validate_reference(value: Any, label: str) -> dict[str, Any]:
    reference = _object(value, label)
    _exact_keys(reference, {"bytes", "path", "sha256"}, label)
    size = reference["bytes"]
    digest = _sha(reference["sha256"], f"{label}.sha256")
    if (
        not isinstance(size, int)
        or isinstance(size, bool)
        or not 0 < size <= MAX_FILE_BYTES
    ):
        _fail(f"{label}.bytes is invalid")
    if reference["path"] != _object_relative(digest):
        _fail(f"{label}.path is not the canonical content-addressed object path")
    return reference


def _write_object(root: Path, payload: bytes) -> dict[str, Any]:
    if not payload or len(payload) > MAX_FILE_BYTES:
        _fail("refusing an empty or oversized evidence object")
    reference = _reference(payload)
    destination = root / reference["path"]
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if destination.exists():
        if (
            _read_regular(destination, "existing evidence object", MAX_FILE_BYTES)
            != payload
        ):
            _fail("content-addressed evidence object collision")
        return reference
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(destination, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    return reference


def _resolve_object(root: Path, reference: dict[str, Any], label: str) -> Path:
    reference = _validate_reference(reference, label)
    path = root
    for component in PurePosixPath(reference["path"]).parts:
        path /= component
        if path.is_symlink():
            _fail(f"{label} traverses a symlink")
    return path


def _copy_object(
    source_root: Path,
    destination_root: Path,
    reference: dict[str, Any],
    label: str,
) -> None:
    reference = _validate_reference(reference, label)
    source = _resolve_object(source_root, reference, label)
    payload = _read_regular(source, label, MAX_FILE_BYTES)
    if len(payload) != reference["bytes"] or _sha256(payload) != reference["sha256"]:
        _fail(f"{label} content identity differs")
    if _write_object(destination_root, payload) != reference:
        _fail(f"{label} changed during archival")


def _repository_executable(repository: Path, relative: str, label: str) -> Path:
    path = repository / relative
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        _fail(f"cannot resolve {label}: {error}")
    if not resolved.is_relative_to(repository) or path.is_symlink():
        _fail(f"{label} escapes the repository or is a symlink")
    metadata = resolved.stat()
    if not resolved.is_file() or metadata.st_mode & 0o111 == 0:
        _fail(f"{label} is not an executable regular file")
    return resolved


def _run_bounded(
    command: list[str],
    *,
    cwd: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    stdout_limit: int = MAX_PROCESS_STDOUT,
    stderr_limit: int = MAX_PROCESS_STDERR,
) -> tuple[bytes, bytes]:
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            status = process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as error:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            _fail(f"command exceeded its {timeout_seconds} second timeout: {error}")
        stdout_size, stderr_size = stdout.tell(), stderr.tell()
        if stdout_size > stdout_limit or stderr_size > stderr_limit:
            _fail("command exceeded its output bounds")
        stdout.seek(0)
        stderr.seek(0)
        output, errors = stdout.read(), stderr.read()
    if status != 0:
        detail = errors.decode("utf-8", errors="replace").strip()
        _fail(
            f"command failed with status {status}" + (f": {detail}" if detail else "")
        )
    return output, errors


def _semantic_suites(
    repository: Path,
    document: dict[str, Any],
    manifest_payload: bytes,
    candidate: dict[str, Any],
    evidence_root: Path,
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    fixtures = {item["fixtureId"] for item in document["compilerFixtures"]}
    simulation: dict[str, list[dict[str, Any]]] = {fixture: [] for fixture in fixtures}
    results: list[dict[str, Any]] = []
    for suite in document["qualification"]["suites"]:
        suite_id = suite.get("suiteId", "<unknown>")
        if suite.get("availability") != "available" or not isinstance(
            suite.get("command"), dict
        ):
            _fail(f"semantic suite {suite_id} is unavailable")
        command = suite["command"]
        manifest_contract._validate_command(
            command, f"semantic suite {suite_id}.command"
        )
        executable = _repository_executable(
            repository, command["executable"], f"semantic suite {suite_id}.command"
        )
        environment = os.environ.copy()
        for assignment in command["environment"]:
            if ENVIRONMENT.fullmatch(assignment) is None:
                _fail(
                    f"semantic suite {suite_id} has an invalid environment assignment"
                )
            name, value = assignment.split("=", 1)
            environment[name] = value
        output, errors = _run_bounded(
            [str(executable), *command["arguments"]],
            cwd=repository / command["workingDirectory"],
            environment=environment,
            timeout_seconds=command["timeoutSeconds"],
            stdout_limit=64 * 1024 * 1024,
        )
        if suite["gate"] == "semantic-simulation":
            result = _object(
                _decode_unique(output, f"semantic suite {suite_id} output"), suite_id
            )
            if (
                result.get("schema") != "fe2o3-tutorial-semantic-suite-result-v1"
                or result.get("status") != "passed"
                or result.get("authority") != "observation_only"
            ):
                _fail(
                    f"semantic suite {suite_id} did not emit an authority-free passed result"
                )
            reference = _write_object(evidence_root, output)
            covered = {
                fixture
                for coverage in suite["coverage"]
                for fixture in coverage["fixtureIds"]
            }
            for fixture in covered:
                if fixture not in simulation:
                    _fail(
                        f"semantic suite {suite_id} covers an unknown fixture {fixture}"
                    )
                simulation[fixture].append(reference)
        results.append(
            {
                "commandSha256": promotion_contract.command_sha256(command),
                "coverage": suite["coverage"],
                "exitStatus": 0,
                "gate": suite["gate"],
                "status": "passed",
                "stderrBytes": len(errors),
                "stderrSha256": _sha256(errors),
                "stdoutBytes": len(output),
                "stdoutSha256": _sha256(output),
                "suiteId": suite_id,
            }
        )
    resolved: dict[str, dict[str, Any]] = {}
    for fixture in sorted(simulation):
        if len(simulation[fixture]) != 1:
            _fail(
                f"fixture {fixture} requires exactly one Bundle V8 semantic suite result; "
                f"found {len(simulation[fixture])}"
            )
        resolved[fixture] = simulation[fixture][0]
    semantic = {
        "authority": {
            "compilerAuthority": False,
            "hardwareAuthority": False,
            "launchAuthority": False,
            "loadAuthority": False,
            "publicationAuthority": False,
        },
        "candidate": {
            "commit": candidate["compilerCommit"],
            "tree": candidate["compilerTree"],
            "worktreeClean": True,
        },
        "manifest": _manifest_identity(document, manifest_payload),
        "roadmapIssue": ROADMAP_ISSUE,
        "schema": "fe2o3-tutorial-semantic-qualification-evidence-v1",
        "suites": results,
    }
    return _write_object(evidence_root, _canonical(semantic) + b"\n"), resolved


def transaction_request(
    fixture: dict[str, Any],
    kernel: dict[str, Any],
    candidate: dict[str, Any],
    manifest: dict[str, str],
    simulator_evidence: dict[str, Any],
    hardware_reservation: str,
    hardware_challenge_nonce: str | None = None,
) -> dict[str, Any]:
    target = fixture["target"]
    lane = HARDWARE_LANES.get(target)
    hardware_command = manifest_contract._expected_hardware_command(fixture)
    if lane is None or hardware_command is None:
        _fail(
            f"fixture {fixture['fixtureId']} has no target-matched hardware lane or command"
        )
    nonce = hardware_challenge_nonce or secrets.token_hex(32)
    _sha(nonce, "hardware receipt challenge nonce")
    hardware_receipt_contract._identity(
        hardware_reservation, "hardware reservation identity"
    )
    request = {
        "candidate": candidate,
        "capabilityKernel": {
            "capabilityClosure": kernel["capabilityClosure"],
            "kernelSymbol": kernel["kernelSymbol"],
            "lessonIds": sorted(kernel["lessonIds"]),
            "requiredProperties": sorted(kernel["requiredProperties"]),
        },
        "fixture": {
            "compilerInput": fixture["compilerInput"],
            "fixtureId": fixture["fixtureId"],
            "hardwareCommand": hardware_command,
            "hardwareLane": lane,
            "hardwareReservation": hardware_reservation,
            "target": target,
        },
        "manifest": manifest,
        "hardwareReceiptChallenge": {
            "nonce": nonce,
            "reservationIdentity": hardware_reservation,
            "schema": hardware_receipt_contract.CHALLENGE_SCHEMA,
            "transportSchema": hardware_receipt_contract.TRANSPORT_SCHEMA,
        },
        "productionTransaction": {
            "allowsFallback": False,
            "allowsPipelineSelection": False,
            "pipelineEntry": promotion_contract.PIPELINE_ENTRY,
            "policyVersion": promotion_contract.POLICY_VERSION,
        },
        "requestBindingSha256": "0" * 64,
        "roadmapIssue": ROADMAP_ISSUE,
        "schema": REQUEST_SCHEMA,
        "simulatorEvidence": simulator_evidence,
    }
    request["requestBindingSha256"] = request_binding_sha256(request)
    return request


def request_binding_sha256(request: dict[str, Any]) -> str:
    subject = {
        key: value for key, value in request.items() if key != "requestBindingSha256"
    }
    return _domain_sha256(REQUEST_DOMAIN, subject)


def _validate_hardware_evidence(
    evidence_root: Path,
    record: dict[str, Any],
    request: dict[str, Any],
    transport: hardware_receipt_contract.ValidatedTransport,
) -> None:
    reference = record["evidenceFiles"]["hardware"]
    path = _resolve_object(evidence_root, reference, "hardware evidence")
    evidence = _load_canonical_json(path, "hardware evidence")
    fields = {
        "artifactInspectionSha256",
        "artifactSha256",
        "authority",
        "candidate",
        "checks",
        "commandSha256",
        "driverIdentitySha256",
        "fixtureId",
        "isaInspectionSha256",
        "kernelSymbols",
        "lane",
        "launchContractSha256",
        "outcome",
        "resourceUsageSha256",
        "reservationIdentity",
        "resultSha256",
        "runtimeIdentitySha256",
        "schema",
        "target",
        "targetIdentitySha256",
        "timeoutSeconds",
    }
    _exact_keys(evidence, fields, "hardware evidence")
    fixture = request["fixture"]
    compiler_input = fixture["compilerInput"]
    command_sha = promotion_contract.command_sha256(fixture["hardwareCommand"])
    expected_joins = {
        "artifactInspectionSha256": record["productionEvidence"][
            "artifactInspectionSha256"
        ],
        "artifactSha256": record["productionEvidence"]["artifactSha256"],
        "driverIdentitySha256": record["hardware"]["driverIdentitySha256"],
        "launchContractSha256": record["productionEvidence"]["launchContractSha256"],
        "runtimeIdentitySha256": record["hardware"]["runtimeIdentitySha256"],
        "targetIdentitySha256": record["productionEvidence"]["targetIdentitySha256"],
    }
    for key, expected in expected_joins.items():
        if _sha(evidence[key], f"hardware evidence.{key}") != expected:
            _fail(f"hardware evidence {key} is substituted")
    checks = _object(evidence["checks"], "hardware evidence.checks")
    expected_checks = {
        "canariesChecked",
        "cleanupComplete",
        "completeOutputChecked",
        "inputsUnchangedChecked",
        "isaInspected",
        "paddingChecked",
        "resourceUsageInspected",
        "timedOut",
    }
    _exact_keys(checks, expected_checks, "hardware evidence.checks")
    if any(checks[key] is not True for key in expected_checks - {"timedOut"}):
        _fail("hardware evidence is partial or cleanup is incomplete")
    if checks["timedOut"] is not False:
        _fail("hardware evidence records a timeout")
    if (
        _sha(evidence["isaInspectionSha256"], "hardware evidence.isaInspectionSha256")
        != transport.isa_inspection_sha256
        or _sha(
            evidence["resourceUsageSha256"],
            "hardware evidence.resourceUsageSha256",
        )
        != transport.resource_usage_sha256
        or _sha(evidence["resultSha256"], "hardware evidence.resultSha256")
        != transport.result_observation_sha256
    ):
        _fail("hardware evidence ISA, resource, or result observation is substituted")
    if (
        evidence["schema"] != HARDWARE_SCHEMA
        or evidence["authority"] != "verification-input-no-independent-authority"
        or evidence["outcome"] != "passed"
        or evidence["candidate"] != request["candidate"]
        or evidence["fixtureId"] != fixture["fixtureId"]
        or evidence["target"] != fixture["target"]
        or evidence["lane"] != fixture["hardwareLane"]
        or evidence["reservationIdentity"] != fixture["hardwareReservation"]
        or evidence["kernelSymbols"] != compiler_input["kernelSymbols"]
        or evidence["commandSha256"] != command_sha
        or evidence["timeoutSeconds"] != fixture["hardwareCommand"]["timeoutSeconds"]
    ):
        _fail("hardware evidence is stale, cross-target, cross-launch, or incomplete")


def validate_transaction_export(
    export_root: Path,
    request: dict[str, Any],
    destination_root: Path,
    hardware_policy: hardware_receipt_contract.TrustPolicy,
    seen_hardware_receipts: set[str],
) -> dict[str, Any]:
    if request.get("requestBindingSha256") != request_binding_sha256(request):
        _fail("production transaction request binding is stale")
    result_path = export_root / RESULT_NAME
    envelope = _load_canonical_json(result_path, "production transaction export")
    _exact_keys(
        envelope,
        {"candidate", "fixtureId", "record", "requestBindingSha256", "schema"},
        "production transaction export",
    )
    fixture = request["fixture"]
    if (
        envelope["schema"] != EXPORT_SCHEMA
        or envelope["candidate"] != request["candidate"]
        or envelope["fixtureId"] != fixture["fixtureId"]
        or envelope["requestBindingSha256"] != request["requestBindingSha256"]
    ):
        _fail("production transaction export is stale or substituted")
    record = _object(envelope["record"], "production transaction export.record")
    expected_record_fields = {
        "capabilityClosure",
        "compilerInput",
        "evidenceFiles",
        "fixtureId",
        "graph",
        "hardware",
        "kernelSymbol",
        "lessonIds",
        "negativeFixtures",
        "productionEvidence",
        "productionTransaction",
        "proof",
        "recordBindingSha256",
        "simulator",
        "target",
        "targetDecision",
    }
    _exact_keys(record, expected_record_fields, "production transaction export.record")
    kernel = request["capabilityKernel"]
    if (
        record["fixtureId"] != fixture["fixtureId"]
        or record["target"] != fixture["target"]
        or record["kernelSymbol"] != kernel["kernelSymbol"]
        or record["lessonIds"] != kernel["lessonIds"]
        or record["compilerInput"]
        != {
            key: fixture["compilerInput"][key]
            for key in (
                "cargoLockSha256",
                "contractSha256",
                "packageManifestSha256",
                "sourceClosureSha256",
            )
        }
    ):
        _fail("production transaction record is stale, reordered, or cross-target")
    if record["recordBindingSha256"] != promotion_contract.record_binding_sha256(
        record
    ):
        _fail("production transaction record binding is stale")
    files = _object(
        record["evidenceFiles"], "production transaction record.evidenceFiles"
    )
    _exact_keys(
        files, promotion_contract.ARCHIVE_KINDS, "production transaction evidence"
    )
    simulator = _validate_reference(request["simulatorEvidence"], "simulator evidence")
    if files["simulator"] != simulator:
        _fail("production transaction substituted the semantic simulator evidence")
    for kind in sorted(files):
        reference = _validate_reference(files[kind], f"production evidence {kind}")
        source = _resolve_object(export_root, reference, f"production evidence {kind}")
        if not source.exists():
            destination = _resolve_object(
                destination_root, reference, f"existing production evidence {kind}"
            )
            if kind != "simulator" or not destination.exists():
                _fail(f"production transaction omitted evidence object {kind}")
            continue
        _copy_object(
            export_root, destination_root, reference, f"production evidence {kind}"
        )
    try:
        transport = hardware_receipt_contract.validate_and_ingest(
            export_root / hardware_receipt_contract.TRANSPORT_NAME,
            request,
            record,
            hardware_policy,
            lambda payload: _write_object(destination_root, payload),
            seen_hardware_receipts,
        )
    except hardware_receipt_contract.HardwareReceiptError as error:
        _fail(f"authenticated hardware archive rejected: {error}")
    _validate_hardware_evidence(destination_root, record, request, transport)
    actual_files: set[str] = set()
    for current, directories, filenames in os.walk(export_root, followlinks=False):
        if any((Path(current) / name).is_symlink() for name in directories + filenames):
            _fail("production transaction export contains a symlink")
        for name in filenames:
            relative = str((Path(current) / name).relative_to(export_root))
            actual_files.add(relative)
            if len(actual_files) > MAX_ARCHIVE_FILES:
                _fail("production transaction export has too many files")
    expected_files = {RESULT_NAME, hardware_receipt_contract.TRANSPORT_NAME}
    expected_files.update(
        reference["path"]
        for reference in files.values()
        if (export_root / reference["path"]).exists()
    )
    if actual_files != expected_files:
        _fail("production transaction export contains omitted or unexpected files")
    return record


def _require_exact_record_order(
    records: list[dict[str, Any]], fixture_ids: list[str]
) -> None:
    observed = [record.get("fixtureId") for record in records]
    if observed != fixture_ids:
        _fail(
            "qualification records are omitted, duplicated, substituted, or reordered"
        )


def assemble_batch(
    records: list[dict[str, Any]],
    candidate: dict[str, Any],
    manifest: dict[str, str],
    semantic_reference: dict[str, Any],
    fixture_ids: list[str],
) -> dict[str, Any]:
    _require_exact_record_order(records, fixture_ids)
    batch = {
        "batchBindingSha256": "0" * 64,
        "candidate": candidate,
        "manifest": manifest,
        "records": records,
        "roadmapIssue": ROADMAP_ISSUE,
        "schema": BATCH_SCHEMA,
        "semanticQualification": semantic_reference,
    }
    batch["batchBindingSha256"] = promotion_contract.batch_binding_sha256(batch)
    return batch


def production_transaction_command(
    repository: Path, cargo_fe2o3: Path | None = None
) -> list[str]:
    source = (
        repository
        / "crates"
        / "rustc-codegen-fe2o3"
        / "src"
        / "bin"
        / f"{TRANSACTION_EXPORT_BINARY}.rs"
    )
    try:
        metadata = source.lstat()
    except OSError:
        _fail(
            f"production compiler does not export required API `{TRANSACTION_EXPORT_API}`; "
            f"expected compiler-owned binary source {source}. The sealed verifier must also "
            f"implement typed hardware receipt API `{HARDWARE_VERIFICATION_API}`"
        )
    if source.is_symlink() or not stat.S_ISREG(metadata.st_mode):
        _fail(
            "production transaction exporter source must be a regular non-symlink file"
        )
    if cargo_fe2o3 is None:
        _fail("an explicit cargo-fe2o3 production executable is required")
    if not cargo_fe2o3.is_absolute() or cargo_fe2o3 != Path(
        os.path.normpath(str(cargo_fe2o3))
    ):
        _fail("cargo-fe2o3 production executable must be absolute and normalized")
    try:
        cargo_fe2o3_metadata = cargo_fe2o3.lstat()
        resolved_cargo_fe2o3 = cargo_fe2o3.resolve(strict=True)
    except OSError as error:
        _fail(f"cannot resolve cargo-fe2o3 production executable: {error}")
    if (
        cargo_fe2o3.is_symlink()
        or resolved_cargo_fe2o3 != cargo_fe2o3
        or not stat.S_ISREG(cargo_fe2o3_metadata.st_mode)
        or cargo_fe2o3_metadata.st_mode & 0o111 == 0
    ):
        _fail(
            "cargo-fe2o3 production executable must be an exact executable regular file"
        )
    cargo = shutil.which("cargo")
    if cargo is None:
        _fail("cargo is unavailable for the compiler-owned transaction exporter")
    return [
        cargo,
        "run",
        "--quiet",
        "--locked",
        "--package",
        "rustc-codegen-fe2o3",
        "--bin",
        TRANSACTION_EXPORT_BINARY,
        "--",
        "--cargo-fe2o3",
        str(cargo_fe2o3),
    ]


def _transaction_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in SCRUBBED_ENVIRONMENT:
        environment.pop(name, None)
    environment["CARGO_TERM_COLOR"] = "never"
    return environment


def invoke_transaction(
    producer_command: list[str],
    repository: Path,
    request_path: Path,
    output_directory: Path,
    timeout_seconds: int,
    hardware_trust_policy: Path,
) -> None:
    if output_directory.exists():
        _fail("production transaction output directory already exists")
    output, _ = _run_bounded(
        [
            *producer_command,
            "--hardware-trust-policy",
            str(hardware_trust_policy),
            "--request",
            str(request_path),
            "--output-directory",
            str(output_directory),
        ],
        cwd=repository,
        environment=_transaction_environment(),
        timeout_seconds=timeout_seconds,
    )
    if output:
        _fail(
            "production transaction producer must publish records, not stdout authority"
        )


@contextmanager
def qualification_workspace(parent: Path) -> Iterator[Path]:
    parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    workspace = Path(
        tempfile.mkdtemp(prefix=".fe2o3-tutorial-qualification-", dir=parent)
    )
    try:
        os.chmod(workspace, 0o700)
        yield workspace
    finally:
        shutil.rmtree(workspace, ignore_errors=True)


def _publish_new_file(path: Path, payload: bytes) -> None:
    if path.name in {"", ".", ".."}:
        _fail("publication path has no regular basename")
    parent = path.parent
    try:
        metadata = parent.lstat()
        resolved = parent.resolve(strict=True)
    except OSError as error:
        _fail(f"cannot open publication parent: {error}")
    if parent.is_symlink() or resolved != parent or not stat.S_ISDIR(metadata.st_mode):
        _fail("publication parent must be an exact regular directory")
    directory = os.open(
        parent,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path.name, flags, 0o600, dir_fd=directory)
        try:
            with os.fdopen(descriptor, "wb") as output:
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
            os.fsync(directory)
        except BaseException:
            try:
                os.unlink(path.name, dir_fd=directory)
            except FileNotFoundError:
                pass
            raise
    except BaseException:
        raise
    finally:
        os.close(directory)


def _publish_new_directory(source: Path, destination: Path) -> None:
    """Atomically publish one staged directory using stable parent descriptors."""
    if destination.exists() or destination.is_symlink():
        _fail("evidence output must be a new path")
    source_parent = source.parent.resolve(strict=True)
    destination_parent = destination.parent.resolve(strict=True)
    if source_parent != source.parent or destination_parent != destination.parent:
        _fail("directory publication parents must be exact non-symlink paths")
    source_fd = os.open(
        source_parent,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    destination_fd = os.open(
        destination_parent,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        os.rename(
            source.name,
            destination.name,
            src_dir_fd=source_fd,
            dst_dir_fd=destination_fd,
        )
        os.fsync(destination_fd)
    finally:
        os.close(destination_fd)
        os.close(source_fd)


def _archive_inventory(root: Path) -> tuple[int, int]:
    files = 0
    total = 0
    for current, directories, filenames in os.walk(root, followlinks=False):
        if any((Path(current) / name).is_symlink() for name in directories + filenames):
            _fail("qualification evidence archive contains a symlink")
        for name in filenames:
            path = Path(current) / name
            metadata = path.lstat()
            if not stat.S_ISREG(metadata.st_mode):
                _fail("qualification evidence archive contains a non-regular file")
            files += 1
            total += metadata.st_size
            if files > MAX_ARCHIVE_FILES or total > MAX_ARCHIVE_BYTES:
                _fail("qualification evidence archive exceeds its aggregate bounds")
    return files, total


Verifier = Callable[[Path, Path, Path, int], dict[str, Any]]


def produce(
    repository: Path,
    batch_output: Path,
    evidence_output: Path,
    transaction_timeout_seconds: int,
    verifier_timeout_seconds: int,
    hardware_trust_policy: Path,
    cargo_fe2o3: Path,
    verifier: Verifier = promotion_contract.run_producer_verifier,
) -> dict[str, int]:
    repository = repository.resolve(strict=True)
    if batch_output.exists() or evidence_output.exists():
        _fail("batch and evidence outputs must both be new")
    if batch_output.parent.resolve(strict=True).is_relative_to(
        repository
    ) or evidence_output.parent.resolve(strict=True).is_relative_to(repository):
        _fail("qualification outputs must be outside the compiler worktree")
    manifest_contract.validate_repository(repository)
    manifest_payload = _read_regular(
        repository / MANIFEST_RELATIVE, "tutorial manifest"
    )
    document = _object(
        _decode_unique(manifest_payload, "tutorial manifest"), "tutorial manifest"
    )
    fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    fixture_ids = sorted(fixtures)
    if (
        len(fixture_ids) != EXPECTED_FIXTURE_COUNT
        or set(fixtures) != set(kernels)
        or [item["fixtureId"] for item in document["compilerFixtures"]] != fixture_ids
    ):
        _fail(
            "tutorial qualification requires the exact sorted 47-fixture manifest roster"
        )
    candidate = clean_candidate(repository)
    hardware_trust_policy = _operator_policy_path(repository, hardware_trust_policy)
    hardware_policy = hardware_receipt_contract.load_trust_policy(hardware_trust_policy)
    producer_command = production_transaction_command(repository, cargo_fe2o3)
    manifest_identity = _manifest_identity(document, manifest_payload)
    work_parent = evidence_output.parent.resolve(strict=True)
    with qualification_workspace(work_parent) as workspace:
        archive = workspace / "evidence"
        archive.mkdir(mode=0o700)
        semantic_reference, simulator_references = _semantic_suites(
            repository, document, manifest_payload, candidate, archive
        )
        records: list[dict[str, Any]] = []
        seen_hardware_receipts: set[str] = set()
        transactions = workspace / "transactions"
        transactions.mkdir(mode=0o700)
        for fixture_id in fixture_ids:
            unit = transactions / fixture_id
            unit.mkdir(mode=0o700)
            fixture = fixtures[fixture_id]
            lane = HARDWARE_LANES.get(fixture["target"])
            trust = hardware_policy.lanes.get((lane, fixture["target"]))
            if trust is None:
                _fail(
                    f"fixture {fixture_id} has no exact trusted lane/target reservation"
                )
            request = transaction_request(
                fixture,
                kernels[fixture_id],
                candidate,
                manifest_identity,
                simulator_references[fixture_id],
                trust.reservation_identity,
            )
            request_path = unit / "request-v1.json"
            request_payload = _canonical(request) + b"\n"
            _publish_new_file(request_path, request_payload)
            export_root = unit / "export"
            invoke_transaction(
                producer_command,
                repository,
                request_path,
                export_root,
                transaction_timeout_seconds,
                hardware_trust_policy,
            )
            if _read_regular(request_path, "transaction request") != request_payload:
                _fail(f"production transaction changed request {fixture_id}")
            records.append(
                validate_transaction_export(
                    export_root,
                    request,
                    archive,
                    hardware_policy,
                    seen_hardware_receipts,
                )
            )
        batch = assemble_batch(
            records,
            candidate,
            manifest_identity,
            semantic_reference,
            fixture_ids,
        )
        batch_payload = _canonical(batch) + b"\n"
        staged_batch = workspace / "qualification-batch-v1.json"
        _publish_new_file(staged_batch, batch_payload)
        evidence_root = archive.resolve(strict=True)
        promotion_contract.validate_batch(
            document,
            manifest_payload,
            batch,
            candidate,
            repository,
            evidence_root,
        )
        report = verifier(
            repository, staged_batch, evidence_root, verifier_timeout_seconds
        )
        promotion_contract.validate_verification_report(report, batch)
        if clean_candidate(repository) != candidate:
            _fail("compiler candidate changed during qualification")
        archive_files, total = _archive_inventory(archive)
        _publish_new_directory(archive, evidence_output)
        try:
            _publish_new_file(batch_output, batch_payload)
        except BaseException:
            shutil.rmtree(evidence_output, ignore_errors=True)
            raise
    return {
        "bytes": total,
        "fixtures": len(records),
        "objects": archive_files,
    }


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--batch-output", required=True, type=Path)
    parser.add_argument("--evidence-output", required=True, type=Path)
    parser.add_argument("--hardware-trust-policy", required=True, type=Path)
    parser.add_argument("--cargo-fe2o3", required=True, type=Path)
    parser.add_argument("--repository", default=REPO_ROOT, type=Path)
    parser.add_argument("--transaction-timeout-seconds", default=3600, type=int)
    parser.add_argument("--verifier-timeout-seconds", default=3600, type=int)
    options = parser.parse_args(arguments)
    if (
        options.transaction_timeout_seconds <= 0
        or options.verifier_timeout_seconds <= 0
    ):
        print(
            "tutorial qualification producer: timeouts must be positive",
            file=sys.stderr,
        )
        return 1
    try:
        stats = produce(
            options.repository,
            options.batch_output.resolve(strict=False),
            options.evidence_output.resolve(strict=False),
            options.transaction_timeout_seconds,
            options.verifier_timeout_seconds,
            options.hardware_trust_policy,
            options.cargo_fe2o3,
        )
    except (
        OSError,
        QualificationProducerError,
        hardware_receipt_contract.HardwareReceiptError,
        manifest_contract.ManifestError,
        promotion_contract.PromotionError,
    ) as error:
        print(f"tutorial qualification producer: {error}", file=sys.stderr)
        return 1
    print(
        f"published {stats['fixtures']} sealed tutorial qualification records "
        f"({stats['bytes']} evidence bytes)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
