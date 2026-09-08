#!/usr/bin/env python3
"""Atomically promote the tutorial manifest from sealed production records.

This collector never manufactures production identities. It validates an exact,
content-addressed 47-fixture batch, asks the production compiler's sealed
verifier to authenticate that batch, and only then emits a qualified manifest
generation. The emitted generation grants no runtime authority.
"""

from __future__ import annotations

import argparse
from copy import deepcopy
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import stat
import struct
import subprocess
import sys
import tempfile
from typing import Any, Callable

import tutorial_kernel_manifest as manifest_contract
import tutorial_negative_fixtures as negative_contract


REPO_ROOT = Path(__file__).resolve().parent.parent
BATCH_SCHEMA = "fe2o3-tutorial-capability-qualification-batch-v1"
VERIFICATION_SCHEMA = "fe2o3-tutorial-capability-producer-verification-v1"
PROMOTION_RECEIPT_SCHEMA = "fe2o3-tutorial-capability-promotion-receipt-v1"
SEMANTIC_QUALIFICATION_SCHEMA = "fe2o3-tutorial-semantic-qualification-evidence-v1"
ROADMAP_ISSUE = "https://github.com/harsh-nod/fe2o3/issues/272"
MANIFEST_PATH = "config/tutorial-kernel-manifest-v1.json"
PIPELINE_ENTRY = "rustc-codegen-fe2o3::production_pipeline"
POLICY_VERSION = 4
RECORD_BINDING_DOMAIN = b"fe2o3-tutorial-capability-record-v1\0"
BATCH_BINDING_DOMAIN = b"fe2o3-tutorial-capability-batch-v1\0"
COMMAND_DOMAIN = b"fe2o3-tutorial-semantic-command-v1\0"
CORPUS_DOMAIN = b"fe2o3-tutorial-kernel-corpus-contract-v1\0"
PROMOTION_RECEIPT_DOMAIN = b"fe2o3-tutorial-capability-promotion-receipt-v1\0"
V8_CONTENT_DOMAIN = b"FE2O3/SIMULATION-BUNDLE-CONTENT/V8\0"
V8_SOURCE_MAP_DOMAIN = b"FE2O3/SIMULATION-SOURCE-MAP/V8\0"
V8_SEMANTIC_MIR_DOMAIN = b"FE2O3/SIMULATION-SEMANTIC-MIR/V8\0"
V8_STORAGE_MAP_DOMAIN = b"FE2O3/SIMULATION-STORAGE-MAP/V8\0"
V8_AGGREGATE_MAP_DOMAIN = b"FE2O3/SIMULATION-AGGREGATE-STORAGE-MAP/V8\0"
LOWERING_RECEIPT_DOMAIN = b"FE2O3/INERT-LINEAGE-CONTENT/AMDGPU-LOWERING/V3\0"
SOURCE_REFINEMENT_RECEIPT_DOMAIN = (
    b"FE2O3/CAPABILITY/SOURCE-MIR-TO-KIR-REFINEMENT-RECEIPT/V1\0"
)
MACHINE_REFINEMENT_RECEIPT_DOMAIN = (
    b"FE2O3/CAPABILITY/MACHINE-REFINEMENT-RECEIPT/V1\0"
)
AUTHENTICATED_CHECKER_EVIDENCE_DOMAIN = (
    b"FE2O3/AUTHENTICATED-COMPILER-CAPABILITY-EVIDENCE-IDENTITY/V5\0"
)
CAPABILITY_OBLIGATION_SET_DOMAIN = b"FE2O3/INERT-CAPABILITY-OBLIGATION-SET/V1\0"
CAPABILITY_RESULT_SET_DOMAIN = b"FE2O3/INERT-CAPABILITY-RESULT-SET/V1\0"
CAPABILITY_OBLIGATION_SET_MAGIC = b"FE2OCAPO"
CAPABILITY_RESULT_SET_MAGIC = b"FE2OCAPR"
V8_MAGIC = b"F2SIMB08"
V8_HEADER_BYTES = 416
V8_BUNDLE_VERSION = 8
V8_KIR_VERSION = 13
MAX_BATCH_BYTES = 64 * 1024 * 1024
MAX_VERIFIER_OUTPUT_BYTES = 16 * 1024 * 1024
MAX_VERIFIER_ERROR_BYTES = 8 * 1024 * 1024
MAX_EVIDENCE_FILE_BYTES = 2 * 1024 * 1024 * 1024
MAX_EVIDENCE_TOTAL_BYTES = 64 * 1024 * 1024 * 1024
MAX_V8_BYTES = 512 * 1024 * 1024
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_ID = re.compile(r"[0-9a-f]{40}\Z")
TARGET = re.compile(r"gfx[0-9]{3}\Z")
DIAGNOSTIC = re.compile(r"FE2O3-[A-Z]+-[0-9]{3}\Z")
HARDWARE_LANES = {"gfx942": "mi300x", "gfx950": "mi350"}
NEGATIVE_CATEGORIES = {
    "abi",
    "alias",
    "bounds",
    "capability-forgery",
    "capability-substitution",
    "evidence",
    "host-invocation",
    "initialization",
    "launch",
    "raw-pointer",
    "stale-output",
    "synchronization",
    "target",
    "unsupported-operation",
}
NEGATIVE_FAILURE_STAGES = {
    "artifact-inspection",
    "host-preparation",
    "kir-verification",
    "macro-authentication",
    "mir-admission",
    "runtime-completion",
    "sealed-verifier",
    "static-analysis",
    "target-legalization",
}

ARCHIVE_KINDS = {
    "artifact",
    "artifact-inspection",
    "capability-analysis",
    "capability-closure",
    "compiler-input",
    "compiler-policy",
    "driver-identity",
    "hardware",
    "host-admission",
    "launch-contract",
    "llvm-module",
    "lowering",
    "machine-refinement",
    "negative-fixture-set",
    "numerical-policy",
    "optimized-kir-v13",
    "proof-checker",
    "proof-evidence",
    "proof-obligation-set",
    "runtime-identity",
    "sealed-production-receipt",
    "semantic-mir",
    "simulation-bundle-v8",
    "simulator",
    "source-closure",
    "source-mir-to-kir-refinement",
    "target-capability-decision",
    "target-identity",
}


class PromotionError(ValueError):
    """A batch cannot cross the tutorial release boundary."""


@dataclass(frozen=True)
class EvidenceSnapshot:
    path: Path
    size: int
    sha256: str


@dataclass(frozen=True)
class V8Metadata:
    canonical_kir: bytes
    canonical_kir_sha256: str
    content_identity_sha256: str
    final_graph_epoch: int
    kernel_abi_identity_sha256: str
    kernel_count: int
    semantic_mir: bytes
    semantic_mir_identity_sha256: str
    source_inventory_receipt_sha256: str
    source_preflight_receipt_sha256: str
    subject_identity_sha256: str
    target: str


@dataclass(frozen=True)
class ValidatedBatch:
    document: dict[str, Any]
    records: dict[str, dict[str, Any]]
    snapshots: tuple[EvidenceSnapshot, ...]


Verifier = Callable[[Path, Path, Path, int], dict[str, Any]]


def _fail(message: str) -> None:
    raise PromotionError(message)


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
        _fail(f"value is not canonical JSON: {error}")


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _domain_sha256(domain: bytes, payload: bytes) -> str:
    return hashlib.sha256(domain + payload).hexdigest()


def _length_delimited_identity(domain: bytes, payload: bytes) -> str:
    return hashlib.sha256(domain + struct.pack("<Q", len(payload)) + payload).hexdigest()


def _capability_set_identity(
    payload: bytes, magic: bytes, domain: bytes, maximum: int, label: str
) -> str:
    if not 48 <= len(payload) <= maximum:
        _fail(f"{label} has an invalid canonical byte length")
    if payload[:8] != magic:
        _fail(f"{label} has the wrong typed-set magic")
    version, flags, declared = struct.unpack_from("<HHI", payload, 8)
    if version != 1 or flags != 0 or declared != len(payload):
        _fail(f"{label} has a noncanonical typed-set header")
    terminal = payload[-32:]
    expected = bytes.fromhex(_length_delimited_identity(domain, payload[:-32]))
    if terminal == bytes(32) or terminal != expected:
        _fail(f"{label} typed-set identity differs from its canonical body")
    return terminal.hex()


def corpus_contract_sha256(document: dict[str, Any]) -> str:
    contract = {key: value for key, value in document.items() if key != "baseline"}
    return _domain_sha256(CORPUS_DOMAIN, _canonical(contract))


def command_sha256(command: dict[str, Any]) -> str:
    return _domain_sha256(COMMAND_DOMAIN, _canonical(command))


def record_binding_sha256(record: dict[str, Any]) -> str:
    subject = {key: value for key, value in record.items() if key != "recordBindingSha256"}
    return _domain_sha256(RECORD_BINDING_DOMAIN, _canonical(subject))


def batch_binding_sha256(batch: dict[str, Any]) -> str:
    subject = {key: value for key, value in batch.items() if key != "batchBindingSha256"}
    return _domain_sha256(BATCH_BINDING_DOMAIN, _canonical(subject))


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _array(value: Any, label: str, *, nonempty: bool = False) -> list[Any]:
    if not isinstance(value, list) or (nonempty and not value):
        _fail(f"{label} must be a {'non-empty ' if nonempty else ''}array")
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        _fail(
            f"{label} fields differ: missing={sorted(expected - actual)!r} "
            f"extra={sorted(actual - expected)!r}"
        )


def _sha(value: Any, label: str) -> str:
    value = _string(value, label)
    if SHA256.fullmatch(value) is None or value == "0" * 64:
        _fail(f"{label} is not a nonzero lowercase SHA-256 identity")
    return value


def _git_id(value: Any, label: str) -> str:
    value = _string(value, label)
    if GIT_ID.fullmatch(value) is None or value == "0" * 40:
        _fail(f"{label} is not a nonzero lowercase Git identity")
    return value


def _sorted_unique_strings(value: Any, label: str, *, nonempty: bool = True) -> list[str]:
    values = _array(value, label, nonempty=nonempty)
    if any(not isinstance(item, str) or not item for item in values):
        _fail(f"{label} must contain non-empty strings")
    if values != sorted(set(values)):
        _fail(f"{label} must be sorted and unique")
    return values


def _relative_path(value: Any, label: str) -> str:
    value = _string(value, label)
    path = PurePosixPath(value)
    if path.is_absolute() or value in {".", ".."} or ".." in path.parts or "\\" in value:
        _fail(f"{label} is not a portable relative path")
    return value


def _load_json_unique(path: Path, maximum: int = MAX_BATCH_BYTES) -> tuple[bytes, dict[str, Any]]:
    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
            _fail(f"{path} must be a regular non-symlink file")
        if metadata.st_size <= 0 or metadata.st_size > maximum:
            _fail(f"{path} has an invalid byte length")
        raw = path.read_bytes()

        document = _decode_json_unique(raw, str(path))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot load {path}: {error}")
    if len(raw) != metadata.st_size:
        _fail(f"{path} changed while it was read")
    return raw, _object(document, str(path))


def _decode_json_unique(raw: bytes, label: str) -> Any:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        return json.loads(raw, object_pairs_hook=reject_duplicates)
    except (UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot decode {label}: {error}")


def _git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        _fail(f"git {' '.join(arguments)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def clean_candidate(repository: Path) -> dict[str, Any]:
    if _git(repository, "status", "--porcelain=v1", "--untracked-files=all"):
        _fail("capability promotion requires a clean compiler worktree")
    return {
        "compilerCommit": _git_id(
            _git(repository, "rev-parse", "--verify", "HEAD"), "compiler HEAD"
        ),
        "compilerTree": _git_id(
            _git(repository, "show", "-s", "--format=%T", "HEAD"), "compiler tree"
        ),
        "worktreeClean": True,
    }


def _evidence_root(path: Path) -> Path:
    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISDIR(metadata.st_mode):
            _fail("evidence root must be a real directory")
        return path.resolve(strict=True)
    except OSError as error:
        _fail(f"cannot inspect evidence root: {error}")


def _evidence_path(root: Path, relative: Any, label: str) -> Path:
    relative = _relative_path(relative, label)
    current = root
    for offset, part in enumerate(PurePosixPath(relative).parts):
        current /= part
        try:
            metadata = current.lstat()
        except OSError as error:
            _fail(f"cannot inspect {label}: {error}")
        if current.is_symlink():
            _fail(f"{label} traverses a symlink")
        if offset + 1 != len(PurePosixPath(relative).parts) and not stat.S_ISDIR(
            metadata.st_mode
        ):
            _fail(f"{label} traverses a non-directory")
    return current


def _snapshot_file(root: Path, raw: Any, label: str) -> EvidenceSnapshot:
    reference = _object(raw, label)
    _exact_keys(reference, {"bytes", "path", "sha256"}, label)
    expected_size = reference["bytes"]
    if (
        not isinstance(expected_size, int)
        or isinstance(expected_size, bool)
        or expected_size <= 0
        or expected_size > MAX_EVIDENCE_FILE_BYTES
    ):
        _fail(f"{label}.bytes is invalid")
    expected_sha = _sha(reference["sha256"], f"{label}.sha256")
    path = _evidence_path(root, reference["path"], f"{label}.path")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
        try:
            before = os.fstat(descriptor)
            if not stat.S_ISREG(before.st_mode) or before.st_size != expected_size:
                _fail(f"{label} size or file type differs")
            digest = hashlib.sha256()
            observed = 0
            while True:
                chunk = os.read(descriptor, 1024 * 1024)
                if not chunk:
                    break
                observed += len(chunk)
                digest.update(chunk)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
    except OSError as error:
        _fail(f"cannot read {label}: {error}")
    stable = (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    ) == (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    )
    if not stable or observed != expected_size:
        _fail(f"{label} changed while it was read")
    if digest.hexdigest() != expected_sha:
        _fail(f"{label} content digest differs")
    return EvidenceSnapshot(path, expected_size, expected_sha)


def _read_snapshot(snapshot: EvidenceSnapshot, label: str, maximum: int) -> bytes:
    if snapshot.size > maximum:
        _fail(f"{label} exceeds its byte bound")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(snapshot.path, flags)
        try:
            metadata = os.fstat(descriptor)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_size != snapshot.size:
                _fail(f"{label} size or file type differs")
            payload = bytearray()
            while True:
                chunk = os.read(descriptor, min(1024 * 1024, snapshot.size - len(payload)))
                if not chunk:
                    break
                payload.extend(chunk)
        finally:
            os.close(descriptor)
    except OSError as error:
        _fail(f"cannot read {label}: {error}")
    if len(payload) != snapshot.size or _sha256(payload) != snapshot.sha256:
        _fail(f"{label} changed after evidence validation")
    return bytes(payload)


def _hex(raw: bytes) -> str:
    return raw.hex()


def _parse_v8(payload: bytes) -> V8Metadata:
    if len(payload) < V8_HEADER_BYTES or len(payload) > MAX_V8_BYTES:
        _fail("simulation Bundle V8 has an invalid byte length")
    if payload[:8] != V8_MAGIC:
        _fail("simulation evidence is not Bundle V8")
    bundle_version, reserved, production_version, canonical_version = struct.unpack_from(
        "<HHHH", payload, 8
    )
    kernel_count = struct.unpack_from("<I", payload, 16)[0]
    target_length = struct.unpack_from("<H", payload, 20)[0]
    if (
        bundle_version != V8_BUNDLE_VERSION
        or reserved != 0
        or production_version != V8_KIR_VERSION
        or canonical_version != V8_KIR_VERSION
        or kernel_count == 0
        or target_length == 0
        or payload[22:28] != bytes(6)
    ):
        _fail("simulation Bundle V8 header is invalid")
    kir_length = struct.unpack_from("<Q", payload, 28)[0]
    source_length = struct.unpack_from("<I", payload, 36)[0]
    semantic_length = struct.unpack_from("<Q", payload, 40)[0]
    storage_length = struct.unpack_from("<I", payload, 48)[0]
    aggregate_length = struct.unpack_from("<I", payload, 52)[0]
    epoch = struct.unpack_from("<Q", payload, 56)[0]
    inventory = _hex(payload[64:96])
    inventory_bytes = struct.unpack_from("<Q", payload, 96)[0]
    preflight = _hex(payload[104:136])
    preflight_bytes = struct.unpack_from("<Q", payload, 136)[0]
    production_kir = _hex(payload[144:176])
    production_kir_bytes = struct.unpack_from("<Q", payload, 176)[0]
    canonical_kir = _hex(payload[184:216])
    canonical_kir_bytes = struct.unpack_from("<Q", payload, 216)[0]
    abi = _hex(payload[224:256])
    source_identity = _hex(payload[256:288])
    semantic_identity = _hex(payload[288:320])
    storage_identity = _hex(payload[320:352])
    aggregate_identity = _hex(payload[352:384])
    subject = _hex(payload[384:416])
    lengths = (
        target_length,
        kir_length,
        source_length,
        semantic_length,
        storage_length,
        aggregate_length,
    )
    if (
        any(length <= 0 for length in lengths)
        or epoch == 0
        or inventory_bytes == 0
        or preflight_bytes == 0
        or production_kir_bytes != kir_length
        or canonical_kir_bytes != kir_length
        or production_kir != canonical_kir
    ):
        _fail("simulation Bundle V8 contains an invalid production coordinate")
    for identity in (
        inventory,
        preflight,
        production_kir,
        canonical_kir,
        abi,
        source_identity,
        semantic_identity,
        storage_identity,
        aggregate_identity,
        subject,
    ):
        _sha(identity, "simulation Bundle V8 identity")
    end = V8_HEADER_BYTES + sum(lengths)
    if end != len(payload):
        _fail("simulation Bundle V8 has trailing or missing bytes")
    cursor = V8_HEADER_BYTES
    target_bytes = payload[cursor : cursor + target_length]
    cursor += target_length
    try:
        target = target_bytes.decode("utf-8")
    except UnicodeError as error:
        _fail(f"simulation Bundle V8 target is invalid UTF-8: {error}")
    if TARGET.fullmatch(target) is None:
        _fail("simulation Bundle V8 target is invalid")
    kir = payload[cursor : cursor + kir_length]
    cursor += kir_length
    source = payload[cursor : cursor + source_length]
    cursor += source_length
    semantic = payload[cursor : cursor + semantic_length]
    cursor += semantic_length
    storage = payload[cursor : cursor + storage_length]
    cursor += storage_length
    aggregate = payload[cursor : cursor + aggregate_length]
    for expected, domain, section, label in (
        (source_identity, V8_SOURCE_MAP_DOMAIN, source, "source map"),
        (semantic_identity, V8_SEMANTIC_MIR_DOMAIN, semantic, "semantic MIR"),
        (storage_identity, V8_STORAGE_MAP_DOMAIN, storage, "storage map"),
        (aggregate_identity, V8_AGGREGATE_MAP_DOMAIN, aggregate, "aggregate map"),
    ):
        if _domain_sha256(domain, section) != expected:
            _fail(f"simulation Bundle V8 {label} identity differs")
    return V8Metadata(
        canonical_kir=kir,
        canonical_kir_sha256=canonical_kir,
        content_identity_sha256=_domain_sha256(V8_CONTENT_DOMAIN, payload),
        final_graph_epoch=epoch,
        kernel_abi_identity_sha256=abi,
        kernel_count=kernel_count,
        semantic_mir=semantic,
        semantic_mir_identity_sha256=semantic_identity,
        source_inventory_receipt_sha256=inventory,
        source_preflight_receipt_sha256=preflight,
        subject_identity_sha256=subject,
        target=target,
    )


def _require_v8_command(command: dict[str, Any], label: str) -> None:
    arguments = command.get("arguments")
    if not isinstance(arguments, list):
        _fail(f"{label} arguments are invalid")
    positions = [index for index, item in enumerate(arguments) if item == "--bundle-version"]
    if len(positions) != 1 or positions[0] + 1 >= len(arguments) or arguments[positions[0] + 1] != "8":
        _fail(f"{label} must request production simulation Bundle V8 exactly once")


def _validate_negative_cases(
    raw: Any,
    fixture_id: str,
    required_properties: set[str],
    repository: Path,
) -> tuple[dict[str, Any], list[EvidenceSnapshot]]:
    record = _object(raw, f"record {fixture_id}.negativeFixtures")
    _exact_keys(record, {"cases", "setSha256", "status"}, f"record {fixture_id}.negativeFixtures")
    if record["status"] != "passed":
        _fail(f"record {fixture_id} negative fixtures did not pass")
    cases = _array(record["cases"], f"record {fixture_id}.negativeFixtures.cases", nonempty=True)
    manifest_cases: list[dict[str, Any]] = []
    snapshots: list[EvidenceSnapshot] = []
    previous = ""
    categories: set[str] = set()
    for offset, raw_case in enumerate(cases):
        label = f"record {fixture_id}.negativeFixtures.cases[{offset}]"
        case = _object(raw_case, label)
        _exact_keys(
            case,
            {
                "category",
                "diagnosticCode",
                "evidenceSha256",
                "failureStage",
                "fixtureId",
                "testPath",
                "testSha256",
            },
            label,
        )
        case_id = _string(case["fixtureId"], f"{label}.fixtureId")
        if case_id <= previous:
            _fail(f"record {fixture_id} negative fixture identities are not sorted and unique")
        previous = case_id
        category = _string(case["category"], f"{label}.category")
        if category not in NEGATIVE_CATEGORIES:
            _fail(f"{label}.category is invalid")
        diagnostic = _string(case["diagnosticCode"], f"{label}.diagnosticCode")
        if DIAGNOSTIC.fullmatch(diagnostic) is None:
            _fail(f"{label}.diagnosticCode is invalid")
        failure_stage = _string(case["failureStage"], f"{label}.failureStage")
        if failure_stage not in NEGATIVE_FAILURE_STAGES:
            _fail(f"{label}.failureStage is invalid")
        categories.add(category)
        _sha(case["evidenceSha256"], f"{label}.evidenceSha256")
        test_path = manifest_contract._repository_file(
            repository, case["testPath"], f"{label}.testPath"
        )
        path, payload = test_path
        test_sha = _sha(case["testSha256"], f"{label}.testSha256")
        if _sha256(payload) != test_sha:
            _fail(f"{label} test source is stale")
        snapshots.append(EvidenceSnapshot(path, len(payload), test_sha))
        manifest_cases.append(
            {
                "category": category,
                "diagnosticCode": case["diagnosticCode"],
                "failureStage": case["failureStage"],
                "fixtureId": case_id,
                "testPath": case["testPath"],
            }
        )
    required = manifest_contract._required_negative_categories(required_properties)
    if not required <= categories:
        _fail(f"record {fixture_id} omits mandatory negative categories")
    expected = manifest_contract._canonical_json_sha256(
        manifest_contract.NEGATIVE_FIXTURE_DIGEST_DOMAIN,
        {"cases": manifest_cases, "fixtureId": fixture_id},
    )
    if _sha(record["setSha256"], f"record {fixture_id}.negativeFixtures.setSha256") != expected:
        _fail(f"record {fixture_id} negative fixture set is stale")
    return {"cases": manifest_cases, "status": "complete"}, snapshots


def _validate_semantic_qualification(
    raw: Any,
    document: dict[str, Any],
    manifest_bytes: bytes,
    candidate: dict[str, Any],
    evidence_root: Path,
) -> tuple[dict[str, tuple[str, int]], EvidenceSnapshot]:
    snapshot = _snapshot_file(
        evidence_root, raw, "qualification batch.semanticQualification"
    )
    payload = _read_snapshot(snapshot, "semantic qualification evidence", MAX_BATCH_BYTES)
    evidence = _object(
        _decode_json_unique(payload, "semantic qualification evidence"),
        "semantic qualification evidence",
    )
    if payload != _canonical(evidence) + b"\n":
        _fail("semantic qualification evidence is not canonical JSON")
    _exact_keys(
        evidence,
        {"authority", "candidate", "manifest", "roadmapIssue", "schema", "suites"},
        "semantic qualification evidence",
    )
    if (
        evidence["schema"] != SEMANTIC_QUALIFICATION_SCHEMA
        or evidence["roadmapIssue"] != ROADMAP_ISSUE
    ):
        _fail("semantic qualification evidence schema or roadmap identity differs")
    authority = _object(evidence["authority"], "semantic qualification evidence.authority")
    expected_authority = {
        "compilerAuthority": False,
        "hardwareAuthority": False,
        "launchAuthority": False,
        "loadAuthority": False,
        "publicationAuthority": False,
    }
    if authority != expected_authority:
        _fail("semantic qualification evidence must remain authority-free")
    expected_candidate = {
        "commit": candidate["compilerCommit"],
        "tree": candidate["compilerTree"],
        "worktreeClean": True,
    }
    if evidence["candidate"] != expected_candidate:
        _fail("semantic qualification evidence is stale against the compiler candidate")
    expected_manifest = {
        "corpusContractSha256": corpus_contract_sha256(document),
        "path": MANIFEST_PATH,
        "rawSha256": _sha256(manifest_bytes),
    }
    if evidence["manifest"] != expected_manifest:
        _fail("semantic qualification evidence is stale against the manifest")

    declared = document["qualification"]["suites"]
    if any(suite.get("availability") != "available" for suite in declared):
        _fail("promotion requires every declared semantic suite to be available")
    results = _array(
        evidence["suites"], "semantic qualification evidence.suites", nonempty=True
    )
    if len(results) != len(declared):
        _fail("semantic qualification evidence does not cover every declared suite")
    fixture_results: dict[str, list[tuple[str, int]]] = {
        fixture["fixtureId"]: [] for fixture in document["compilerFixtures"]
    }
    for offset, (suite, raw_result) in enumerate(zip(declared, results, strict=True)):
        label = f"semantic qualification evidence.suites[{offset}]"
        result = _object(raw_result, label)
        _exact_keys(
            result,
            {
                "commandSha256",
                "coverage",
                "exitStatus",
                "gate",
                "stderrBytes",
                "stderrSha256",
                "stdoutBytes",
                "stdoutSha256",
                "status",
                "suiteId",
            },
            label,
        )
        if (
            result["suiteId"] != suite["suiteId"]
            or result["gate"] != suite["gate"]
            or result["coverage"] != suite["coverage"]
            or result["status"] != "passed"
            or result["exitStatus"] != 0
            or _sha(result["commandSha256"], f"{label}.commandSha256")
            != command_sha256(suite["command"])
        ):
            _fail(f"{label} is stale, reordered, or did not pass")
        for field in ("stderrBytes", "stdoutBytes"):
            value = result[field]
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                _fail(f"{label}.{field} is invalid")
        _sha(result["stderrSha256"], f"{label}.stderrSha256")
        stdout_sha = _sha(result["stdoutSha256"], f"{label}.stdoutSha256")
        if suite["gate"] == "semantic-simulation":
            covered_fixtures: set[str] = set()
            for coverage in suite["coverage"]:
                covered_fixtures.update(coverage["fixtureIds"])
            for fixture_id in covered_fixtures:
                fixture_results[fixture_id].append(
                    (stdout_sha, result["stdoutBytes"])
                )
    resolved: dict[str, tuple[str, int]] = {}
    for fixture_id, identities in fixture_results.items():
        if len(identities) != 1:
            _fail(
                f"fixture {fixture_id} requires exactly one semantic simulator result, "
                f"found {len(identities)}"
            )
        resolved[fixture_id] = identities[0]
    return resolved, snapshot


def _validate_record(
    record: dict[str, Any],
    fixture: dict[str, Any],
    kernel: dict[str, Any],
    document: dict[str, Any],
    candidate: dict[str, Any],
    repository: Path,
    evidence_root: Path,
    semantic_evidence: tuple[str, int],
) -> tuple[dict[str, Any], list[EvidenceSnapshot]]:
    fixture_id = fixture["fixtureId"]
    label = f"record {fixture_id}"
    _exact_keys(
        record,
        {
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
        },
        label,
    )
    if record["fixtureId"] != fixture_id:
        _fail(f"{label} fixture identity differs from its manifest fixture")
    if record["kernelSymbol"] != kernel["kernelSymbol"]:
        _fail(f"{label} kernel symbol differs from its manifest kernel")
    lessons = _sorted_unique_strings(record["lessonIds"], f"{label}.lessonIds")
    if lessons != sorted(kernel["lessonIds"]):
        _fail(f"{label} lesson ownership differs from its manifest kernel")
    target = _string(record["target"], f"{label}.target")
    if target != fixture["target"] or TARGET.fullmatch(target) is None:
        _fail(f"{label} target differs from its manifest fixture")
    binding = _sha(record["recordBindingSha256"], f"{label}.recordBindingSha256")
    if binding != record_binding_sha256(record):
        _fail(f"{label} binding is stale")

    compiler_input = _object(record["compilerInput"], f"{label}.compilerInput")
    compiler_keys = {
        "cargoLockSha256",
        "contractSha256",
        "packageManifestSha256",
        "sourceClosureSha256",
    }
    _exact_keys(compiler_input, compiler_keys, f"{label}.compilerInput")
    expected_input = fixture["compilerInput"]
    for key in compiler_keys:
        if _sha(compiler_input[key], f"{label}.compilerInput.{key}") != expected_input[key]:
            _fail(f"{label} compiler input {key} is stale")

    transaction = _object(record["productionTransaction"], f"{label}.productionTransaction")
    _exact_keys(
        transaction,
        {
            "allowsFallback",
            "allowsPipelineSelection",
            "pipelineEntry",
            "policyVersion",
            "status",
            "transactionSha256",
        },
        f"{label}.productionTransaction",
    )
    if transaction != {
        **transaction,
        "allowsFallback": False,
        "allowsPipelineSelection": False,
        "pipelineEntry": PIPELINE_ENTRY,
        "policyVersion": POLICY_VERSION,
        "status": "sealed-production-complete",
    }:
        _fail(f"{label} did not use the sole production transaction")
    _sha(transaction["transactionSha256"], f"{label}.productionTransaction.transactionSha256")

    evidence = _object(record["productionEvidence"], f"{label}.productionEvidence")
    _exact_keys(evidence, manifest_contract.PRODUCTION_EVIDENCE_KEYS, f"{label}.productionEvidence")
    for key, value in evidence.items():
        if key in {"compilerCommit", "compilerTree"}:
            _git_id(value, f"{label}.productionEvidence.{key}")
        else:
            _sha(value, f"{label}.productionEvidence.{key}")
    if (
        evidence["compilerCommit"] != candidate["compilerCommit"]
        or evidence["compilerTree"] != candidate["compilerTree"]
    ):
        _fail(f"{label} compiler candidate is stale")

    closure = _object(record["capabilityClosure"], f"{label}.capabilityClosure")
    _exact_keys(closure, {"requirements", "sha256", "status"}, f"{label}.capabilityClosure")
    requirements = _sorted_unique_strings(closure["requirements"], f"{label}.capabilityClosure.requirements")
    if (
        closure["status"] != "complete"
        or requirements != sorted(kernel["capabilityClosure"]["requirements"])
        or _sha(closure["sha256"], f"{label}.capabilityClosure.sha256")
        != evidence["capabilityClosureSha256"]
    ):
        _fail(f"{label} capability closure is stale or incomplete")

    proof = _object(record["proof"], f"{label}.proof")
    _exact_keys(
        proof,
        {"checkerSha256", "evidenceSha256", "obligationSetSha256", "properties", "status"},
        f"{label}.proof",
    )
    properties = _sorted_unique_strings(proof["properties"], f"{label}.proof.properties")
    required_properties = set(kernel["requiredProperties"])
    if (
        proof["status"] != "complete"
        or not manifest_contract.PRODUCTION_PROOF_PROPERTIES <= set(properties)
        or not set(properties) <= required_properties
    ):
        _fail(f"{label} proof coverage is incomplete")
    proof_joins = {
        "checkerSha256": "proofCheckerSha256",
        "evidenceSha256": "proofEvidenceSha256",
        "obligationSetSha256": "proofObligationSetSha256",
    }
    for proof_key, evidence_key in proof_joins.items():
        if _sha(proof[proof_key], f"{label}.proof.{proof_key}") != evidence[evidence_key]:
            _fail(f"{label} proof identity {proof_key} is substituted")

    target_decision = _object(record["targetDecision"], f"{label}.targetDecision")
    _exact_keys(
        target_decision,
        {"capabilityDecisionSha256", "status", "targetIdentitySha256"},
        f"{label}.targetDecision",
    )
    if (
        target_decision["status"] != "capability-complete"
        or _sha(
            target_decision["capabilityDecisionSha256"],
            f"{label}.targetDecision.capabilityDecisionSha256",
        )
        != evidence["targetCapabilityDecisionSha256"]
        or _sha(
            target_decision["targetIdentitySha256"],
            f"{label}.targetDecision.targetIdentitySha256",
        )
        != evidence["targetIdentitySha256"]
    ):
        _fail(f"{label} target capability decision is substituted")

    files = _object(record["evidenceFiles"], f"{label}.evidenceFiles")
    _exact_keys(files, ARCHIVE_KINDS, f"{label}.evidenceFiles")
    snapshots_by_kind: dict[str, EvidenceSnapshot] = {}
    for kind in sorted(ARCHIVE_KINDS):
        snapshots_by_kind[kind] = _snapshot_file(
            evidence_root, files[kind], f"{label}.evidenceFiles.{kind}"
        )
    if snapshots_by_kind["artifact"].sha256 != evidence["artifactSha256"]:
        _fail(f"{label} artifact digest is substituted")
    for kind, evidence_key in (
        ("artifact-inspection", "artifactInspectionSha256"),
        ("hardware", "hardwareEvidenceSha256"),
        ("simulator", "simulatorEvidenceSha256"),
    ):
        if snapshots_by_kind[kind].sha256 != evidence[evidence_key]:
            _fail(f"{label} {kind.replace('-', ' ')} evidence digest is substituted")

    typed_identity_claims = {
        "loweringIdentitySha256": _length_delimited_identity(
            LOWERING_RECEIPT_DOMAIN,
            _read_snapshot(
                snapshots_by_kind["lowering"], f"{label} lowering receipt", 4 * 1024 * 1024
            ),
        ),
        "sourceMirToKirRefinementSha256": _length_delimited_identity(
            SOURCE_REFINEMENT_RECEIPT_DOMAIN,
            _read_snapshot(
                snapshots_by_kind["source-mir-to-kir-refinement"],
                f"{label} source MIR-to-KIR refinement receipt",
                4 * 1024 * 1024,
            ),
        ),
        "machineRefinementSha256": _length_delimited_identity(
            MACHINE_REFINEMENT_RECEIPT_DOMAIN,
            _read_snapshot(
                snapshots_by_kind["machine-refinement"],
                f"{label} machine refinement receipt",
                4 * 1024 * 1024,
            ),
        ),
        "proofCheckerSha256": _length_delimited_identity(
            AUTHENTICATED_CHECKER_EVIDENCE_DOMAIN,
            _read_snapshot(
                snapshots_by_kind["proof-checker"],
                f"{label} authenticated proof checker evidence",
                4 * 1024 * 1024,
            ),
        ),
        "proofObligationSetSha256": _capability_set_identity(
            _read_snapshot(
                snapshots_by_kind["proof-obligation-set"],
                f"{label} capability obligation set",
                64 * 1024,
            ),
            CAPABILITY_OBLIGATION_SET_MAGIC,
            CAPABILITY_OBLIGATION_SET_DOMAIN,
            64 * 1024,
            f"{label} capability obligation set",
        ),
        "proofEvidenceSha256": _capability_set_identity(
            _read_snapshot(
                snapshots_by_kind["proof-evidence"],
                f"{label} capability result set",
                1024 * 1024,
            ),
            CAPABILITY_RESULT_SET_MAGIC,
            CAPABILITY_RESULT_SET_DOMAIN,
            1024 * 1024,
            f"{label} capability result set",
        ),
    }
    for claim, expected in typed_identity_claims.items():
        if evidence[claim] != expected:
            _fail(f"{label} typed production identity {claim} is substituted")

    v8_payload = _read_snapshot(
        snapshots_by_kind["simulation-bundle-v8"], f"{label} simulation Bundle V8", MAX_V8_BYTES
    )
    v8 = _parse_v8(v8_payload)
    graph = _object(record["graph"], f"{label}.graph")
    _exact_keys(
        graph,
        {
            "bundleContentIdentitySha256",
            "bundleSubjectIdentitySha256",
            "canonicalKirBytes",
            "canonicalKirVersion",
            "finalGraphEpoch",
            "kernelAbiIdentitySha256",
            "kernelCount",
            "productionKirIdentitySha256",
            "semanticMirIdentitySha256",
            "sourceInventoryReceiptSha256",
            "sourcePreflightReceiptSha256",
        },
        f"{label}.graph",
    )
    expected_graph = {
        "bundleContentIdentitySha256": v8.content_identity_sha256,
        "bundleSubjectIdentitySha256": v8.subject_identity_sha256,
        "canonicalKirBytes": len(v8.canonical_kir),
        "canonicalKirVersion": V8_KIR_VERSION,
        "finalGraphEpoch": v8.final_graph_epoch,
        "kernelAbiIdentitySha256": v8.kernel_abi_identity_sha256,
        "kernelCount": v8.kernel_count,
        "productionKirIdentitySha256": v8.canonical_kir_sha256,
        "semanticMirIdentitySha256": v8.semantic_mir_identity_sha256,
        "sourceInventoryReceiptSha256": v8.source_inventory_receipt_sha256,
        "sourcePreflightReceiptSha256": v8.source_preflight_receipt_sha256,
    }
    if graph != expected_graph or v8.target != target:
        _fail(f"{label} V8 graph coordinates are stale or substituted")
    if (
        evidence["finalOptimizedKirSha256"] != v8.canonical_kir_sha256
        or evidence["sourceMirIdentitySha256"] != v8.semantic_mir_identity_sha256
    ):
        _fail(f"{label} V8 source/MIR or final graph identity is substituted")
    archived_kir = _read_snapshot(
        snapshots_by_kind["optimized-kir-v13"], f"{label} optimized KIR V13", MAX_V8_BYTES
    )
    archived_mir = _read_snapshot(
        snapshots_by_kind["semantic-mir"], f"{label} semantic MIR", MAX_V8_BYTES
    )
    if archived_kir != v8.canonical_kir or archived_mir != v8.semantic_mir:
        _fail(f"{label} archived MIR or KIR differs from Bundle V8")

    simulator = _object(record["simulator"], f"{label}.simulator")
    hardware = _object(record["hardware"], f"{label}.hardware")
    simulator_keys = {"commandSha256", "evidenceSha256", "status", "subjectSha256"}
    hardware_keys = {
        "artifactInspectionSha256",
        "artifactSha256",
        "canariesChecked",
        "commandSha256",
        "driverIdentitySha256",
        "evidenceSha256",
        "fullOutputChecked",
        "inputsUnchangedChecked",
        "lane",
        "launchContractSha256",
        "paddingChecked",
        "runtimeIdentitySha256",
        "status",
        "subjectSha256",
        "target",
        "targetIdentitySha256",
        "timeoutSeconds",
    }
    _exact_keys(simulator, simulator_keys, f"{label}.simulator")
    _exact_keys(hardware, hardware_keys, f"{label}.hardware")
    expected_simulators = manifest_contract._expected_simulator_commands(
        document["qualification"], set(lessons), fixture_id
    )
    if len(expected_simulators) != 1:
        _fail(f"{label} does not have exactly one declared simulator command")
    simulator_command = expected_simulators[0]
    _require_v8_command(simulator_command, f"{label} simulator command")
    hardware_command = manifest_contract._expected_hardware_command(fixture)
    if hardware_command is None:
        _fail(f"{label} has no target-matched hardware command")
    if (
        simulator["status"] != "passed"
        or _sha(simulator["commandSha256"], f"{label}.simulator.commandSha256")
        != command_sha256(simulator_command)
        or _sha(simulator["subjectSha256"], f"{label}.simulator.subjectSha256")
        != evidence["finalOptimizedKirSha256"]
        or _sha(simulator["evidenceSha256"], f"{label}.simulator.evidenceSha256")
        != evidence["simulatorEvidenceSha256"]
        or simulator["evidenceSha256"] != semantic_evidence[0]
        or snapshots_by_kind["simulator"].size != semantic_evidence[1]
    ):
        _fail(f"{label} simulator evidence is stale or substituted")
    expected_lane = HARDWARE_LANES.get(target)
    if expected_lane is None:
        _fail(f"{label} target has no authenticated hardware lane")
    hardware_identity_joins = {
        "artifactInspectionSha256": "artifactInspectionSha256",
        "artifactSha256": "artifactSha256",
        "launchContractSha256": "launchContractSha256",
        "targetIdentitySha256": "targetIdentitySha256",
    }
    for hardware_key, evidence_key in hardware_identity_joins.items():
        if _sha(hardware[hardware_key], f"{label}.hardware.{hardware_key}") != evidence[
            evidence_key
        ]:
            _fail(f"{label} hardware {hardware_key} is substituted")
    for hardware_key, archive_kind in (
        ("driverIdentitySha256", "driver-identity"),
        ("runtimeIdentitySha256", "runtime-identity"),
    ):
        if (
            _sha(hardware[hardware_key], f"{label}.hardware.{hardware_key}")
            != snapshots_by_kind[archive_kind].sha256
        ):
            _fail(f"{label} hardware {hardware_key} is substituted")
    if (
        hardware["status"] != "passed"
        or hardware["target"] != target
        or hardware["lane"] != expected_lane
        or hardware["timeoutSeconds"] != hardware_command["timeoutSeconds"]
        or any(
            hardware[key] is not True
            for key in (
                "canariesChecked",
                "fullOutputChecked",
                "inputsUnchangedChecked",
                "paddingChecked",
            )
        )
        or _sha(hardware["commandSha256"], f"{label}.hardware.commandSha256")
        != command_sha256(hardware_command)
        or _sha(hardware["subjectSha256"], f"{label}.hardware.subjectSha256")
        != evidence["artifactSha256"]
        or _sha(hardware["evidenceSha256"], f"{label}.hardware.evidenceSha256")
        != evidence["hardwareEvidenceSha256"]
    ):
        _fail(f"{label} hardware evidence is stale, incomplete, or not target-matched")

    negative_manifest, negative_snapshots = _validate_negative_cases(
        record["negativeFixtures"], fixture_id, required_properties, repository
    )
    if evidence["negativeFixtureSetSha256"] != record["negativeFixtures"]["setSha256"]:
        _fail(f"{label} negative fixture identity is substituted")
    promoted = {
        "capabilityClosure": {
            "requirements": requirements,
            "sha256": closure["sha256"],
            "status": "complete",
        },
        "hardwareCommand": {
            "command": hardware_command,
            "evidenceSha256": hardware["evidenceSha256"],
            "reasonCode": None,
            "status": "capability-path-qualified",
            "subjectSha256": hardware["subjectSha256"],
            "target": target,
        },
        "negativeFixtureCoverage": negative_manifest,
        "productionCapabilityPath": {
            "evidence": evidence,
            "path": "canonical-capability",
            "status": "complete",
        },
        "proofRequirements": {
            "checkerSha256": proof["checkerSha256"],
            "evidenceSha256": proof["evidenceSha256"],
            "obligationSetSha256": proof["obligationSetSha256"],
            "properties": properties,
            "status": "complete",
        },
        "simulatorCommand": {
            "command": simulator_command,
            "evidenceSha256": simulator["evidenceSha256"],
            "reasonCode": None,
            "status": "capability-path-qualified",
            "subjectSha256": simulator["subjectSha256"],
            "target": target,
        },
        "targetMatrix": [
            {
                "kind": "neutral",
                "requirements": requirements,
                "status": "requirements-derived",
                "target": "target-neutral",
            },
            {
                "capabilityDecisionSha256": target_decision["capabilityDecisionSha256"],
                "kind": "backend",
                "status": "capability-complete",
                "target": target,
                "targetIdentitySha256": target_decision["targetIdentitySha256"],
            },
        ],
    }
    snapshots = list(snapshots_by_kind.values()) + negative_snapshots
    return promoted, snapshots


def validate_batch(
    document: dict[str, Any],
    manifest_bytes: bytes,
    batch: dict[str, Any],
    candidate: dict[str, Any],
    repository: Path,
    evidence_root: Path,
) -> ValidatedBatch:
    _exact_keys(
        batch,
        {
            "batchBindingSha256",
            "candidate",
            "manifest",
            "records",
            "roadmapIssue",
            "schema",
            "semanticQualification",
        },
        "qualification batch",
    )
    if batch["schema"] != BATCH_SCHEMA or batch["roadmapIssue"] != ROADMAP_ISSUE:
        _fail("qualification batch schema or roadmap identity differs")
    manifest = _object(batch["manifest"], "qualification batch.manifest")
    _exact_keys(
        manifest,
        {"corpusContractSha256", "path", "rawSha256"},
        "qualification batch.manifest",
    )
    if (
        manifest["path"] != MANIFEST_PATH
        or _sha(manifest["rawSha256"], "qualification batch.manifest.rawSha256")
        != _sha256(manifest_bytes)
        or _sha(
            manifest["corpusContractSha256"],
            "qualification batch.manifest.corpusContractSha256",
        )
        != corpus_contract_sha256(document)
    ):
        _fail("qualification batch is stale against the input manifest")
    batch_candidate = _object(batch["candidate"], "qualification batch.candidate")
    _exact_keys(
        batch_candidate,
        {"compilerCommit", "compilerTree", "worktreeClean"},
        "qualification batch.candidate",
    )
    if batch_candidate != candidate:
        _fail("qualification batch is stale against the clean compiler candidate")
    if (
        _sha(batch["batchBindingSha256"], "qualification batch.batchBindingSha256")
        != batch_binding_sha256(batch)
    ):
        _fail("qualification batch binding is stale")

    semantic_results, semantic_snapshot = _validate_semantic_qualification(
        batch["semanticQualification"],
        document,
        manifest_bytes,
        candidate,
        evidence_root,
    )

    fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    raw_records = _array(batch["records"], "qualification batch.records", nonempty=True)
    record_ids = [
        _string(_object(item, "qualification record").get("fixtureId"), "record.fixtureId")
        for item in raw_records
    ]
    if record_ids != sorted(fixtures) or len(record_ids) != len(set(record_ids)):
        _fail("qualification batch must cover every manifest fixture exactly once in order")
    records: dict[str, dict[str, Any]] = {}
    snapshots: list[EvidenceSnapshot] = []
    transaction_ids: set[str] = set()
    total_bytes = 0
    for raw_record in raw_records:
        record = _object(raw_record, "qualification record")
        fixture_id = record["fixtureId"]
        promoted, record_snapshots = _validate_record(
            record,
            fixtures[fixture_id],
            kernels[fixture_id],
            document,
            candidate,
            repository,
            evidence_root,
            semantic_results[fixture_id],
        )
        transaction = record["productionTransaction"]["transactionSha256"]
        if transaction in transaction_ids:
            _fail("production transaction identity is reused across fixtures")
        transaction_ids.add(transaction)
        records[fixture_id] = promoted
        snapshots.extend(record_snapshots)
        total_bytes += sum(snapshot.size for snapshot in record_snapshots)
        if total_bytes > MAX_EVIDENCE_TOTAL_BYTES:
            _fail("qualification evidence exceeds the aggregate byte bound")
    snapshots.append(semantic_snapshot)
    return ValidatedBatch(batch, records, tuple(snapshots))


def expected_verification_report(
    batch: dict[str, Any], verifier_identity_sha256: str
) -> dict[str, Any]:
    return {
        "authority": "verification-only-no-runtime-authority",
        "batchBindingSha256": batch["batchBindingSha256"],
        "candidate": batch["candidate"],
        "manifestRawSha256": batch["manifest"]["rawSha256"],
        "records": [
            {
                "fixtureId": record["fixtureId"],
                "productionTransactionSha256": record["productionTransaction"][
                    "transactionSha256"
                ],
                "recordBindingSha256": record["recordBindingSha256"],
                "sealedProductionReceiptSha256": record["evidenceFiles"][
                    "sealed-production-receipt"
                ]["sha256"],
                "simulationBundleV8Sha256": record["evidenceFiles"][
                    "simulation-bundle-v8"
                ]["sha256"],
                "status": "accepted",
            }
            for record in batch["records"]
        ],
        "schema": VERIFICATION_SCHEMA,
        "status": "accepted",
        "verifierIdentitySha256": _sha(
            verifier_identity_sha256, "producer verifier identity"
        ),
    }


def validate_verification_report(report: dict[str, Any], batch: dict[str, Any]) -> None:
    _exact_keys(
        report,
        {
            "authority",
            "batchBindingSha256",
            "candidate",
            "manifestRawSha256",
            "records",
            "schema",
            "status",
            "verifierIdentitySha256",
        },
        "producer verification report",
    )
    if (
        report["schema"] != VERIFICATION_SCHEMA
        or report["status"] != "accepted"
        or report["authority"] != "verification-only-no-runtime-authority"
    ):
        _fail("producer verification report did not accept the sealed production batch")
    verifier = _sha(
        report["verifierIdentitySha256"],
        "producer verification report.verifierIdentitySha256",
    )
    expected = expected_verification_report(batch, verifier)
    if report != expected:
        _fail("producer verification report is stale, incomplete, reordered, or substituted")


def run_producer_verifier(
    repository: Path, batch_path: Path, evidence_root: Path, timeout_seconds: int
) -> dict[str, Any]:
    cargo = shutil.which("cargo")
    if cargo is None:
        _fail("cargo is unavailable for the sealed production verifier")
    command = [
        cargo,
        "run",
        "--quiet",
        "--locked",
        "--package",
        "rustc-codegen-fe2o3",
        "--bin",
        "fe2o3-verify-tutorial-production-batch-v1",
        "--",
        "--repository",
        str(repository),
        "--batch",
        str(batch_path),
        "--evidence-root",
        str(evidence_root),
    ]
    environment = os.environ.copy()
    for name in (
        "CARGO_BUILD_RUSTC",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTFLAGS",
    ):
        environment.pop(name, None)
    environment["CARGO_TERM_COLOR"] = "never"
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(
            command,
            cwd=repository,
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
            _fail(f"sealed production verifier exceeded {timeout_seconds} seconds: {error}")
        stdout_size = stdout.tell()
        stderr_size = stderr.tell()
        if stdout_size > MAX_VERIFIER_OUTPUT_BYTES or stderr_size > MAX_VERIFIER_ERROR_BYTES:
            _fail("sealed production verifier exceeded its output bound")
        stdout.seek(0)
        stderr.seek(0)
        output = stdout.read()
        errors = stderr.read()
    if status != 0:
        detail = errors.decode("utf-8", errors="replace").strip()
        _fail(
            "sealed production verifier rejected the batch or is not implemented"
            + (f": {detail}" if detail else "")
        )
    report = _decode_json_unique(output, "sealed production verifier output")
    if output != _canonical(report) + b"\n":
        _fail("sealed production verifier output is not canonical JSON")
    return _object(report, "sealed production verifier output")


def build_promoted_manifest(
    document: dict[str, Any], candidate: dict[str, Any], records: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    promoted = deepcopy(document)
    promoted["baseline"] = {
        "compilerCommit": candidate["compilerCommit"],
        "compilerTree": candidate["compilerTree"],
        "status": "qualified",
    }
    promoted["capabilityContract"]["status"] = "qualified"
    promoted["qualification"]["hardwareTargets"] = [
        {
            "lane": lane,
            "status": "required-qualified",
            "target": target,
        }
        for target, lane in sorted(HARDWARE_LANES.items())
    ]
    for entry in promoted["entries"]:
        entry["classification"] = "compiler-produced"
        entry["requiredGates"] = sorted(set(entry["requiredGates"]) | {"hardware"})
    for kernel in promoted["capabilityKernels"]:
        fixture_id = kernel["fixtureId"]
        replacement = records.get(fixture_id)
        if replacement is None:
            _fail(f"no authenticated production record for fixture {fixture_id}")
        for key, value in replacement.items():
            kernel[key] = deepcopy(value)
    try:
        manifest_contract.validate_document(promoted, require_qualified=True)
    except manifest_contract.ManifestError as error:
        _fail(f"promoted manifest violates the release contract: {error}")
    return promoted


def encoded_manifest(document: dict[str, Any]) -> bytes:
    return (
        json.dumps(document, allow_nan=False, ensure_ascii=True, indent=2) + "\n"
    ).encode("ascii")


def _revalidate_snapshots(snapshots: tuple[EvidenceSnapshot, ...]) -> None:
    observed: set[tuple[Path, int, str]] = set()
    for snapshot in snapshots:
        identity = (snapshot.path, snapshot.size, snapshot.sha256)
        if identity in observed:
            continue
        observed.add(identity)
        _read_snapshot(snapshot, str(snapshot.path), MAX_EVIDENCE_FILE_BYTES)


def _promotion_receipt(
    manifest_bytes: bytes, batch: dict[str, Any], verification: dict[str, Any]
) -> bytes:
    body = {
        "authority": "release-metadata-only",
        "batchBindingSha256": batch["batchBindingSha256"],
        "compilerCommit": batch["candidate"]["compilerCommit"],
        "compilerTree": batch["candidate"]["compilerTree"],
        "manifestSha256": _sha256(manifest_bytes),
        "producerVerificationSha256": _sha256(_canonical(verification) + b"\n"),
        "schema": PROMOTION_RECEIPT_SCHEMA,
    }
    body["receiptSha256"] = _domain_sha256(PROMOTION_RECEIPT_DOMAIN, _canonical(body))
    return _canonical(body) + b"\n"


def _write_new_file(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        0o600,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def publish_generation(
    output: Path,
    manifest_bytes: bytes,
    batch: dict[str, Any],
    verification: dict[str, Any],
) -> None:
    if output.exists() or output.is_symlink():
        _fail(f"refusing to replace existing output generation: {output}")
    parent = output.parent.resolve(strict=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{output.name}.tmp-", dir=parent))
    try:
        config = temporary / "config"
        config.mkdir(mode=0o700)
        manifest_path = config / manifest_contract.MANIFEST_NAME
        digest_path = config / manifest_contract.MANIFEST_NAME.replace(".json", ".sha256")
        receipt_path = temporary / "promotion-receipt-v1.json"
        _write_new_file(manifest_path, manifest_bytes)
        _write_new_file(
            digest_path,
            f"{_sha256(manifest_bytes)}  {MANIFEST_PATH}\n".encode("ascii"),
        )
        _write_new_file(receipt_path, _promotion_receipt(manifest_bytes, batch, verification))
        for directory in (config, temporary):
            descriptor = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
        os.rename(temporary, output)
        descriptor = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)


def promote(
    repository: Path,
    batch_path: Path,
    evidence_root: Path,
    output: Path,
    timeout_seconds: int,
    verifier: Verifier = run_producer_verifier,
) -> dict[str, int]:
    repository = repository.resolve(strict=True)
    evidence_root = _evidence_root(evidence_root)
    manifest_contract.validate_repository(repository)
    manifest_path = repository / MANIFEST_PATH
    manifest_bytes, document = manifest_contract._load_json_unique(manifest_path)
    candidate = clean_candidate(repository)
    batch_bytes, batch = _load_json_unique(batch_path)
    validated = validate_batch(
        document,
        manifest_bytes,
        batch,
        candidate,
        repository,
        evidence_root,
    )
    negative_evidence = negative_contract.execute(repository, document)
    report = verifier(repository, batch_path, evidence_root, timeout_seconds)
    validate_verification_report(report, batch)
    negative_contract.validate_promotion_records(
        repository, document, batch["records"], negative_evidence
    )
    promoted = build_promoted_manifest(document, candidate, validated.records)
    manifest_contract._validate_qualified_repository_inputs(repository, promoted)
    _revalidate_snapshots(validated.snapshots)
    batch_snapshot = EvidenceSnapshot(batch_path, len(batch_bytes), _sha256(batch_bytes))
    if _read_snapshot(batch_snapshot, "qualification batch", MAX_BATCH_BYTES) != batch_bytes:
        _fail("qualification batch changed before publication")
    if clean_candidate(repository) != candidate:
        _fail("compiler candidate changed before publication")
    payload = encoded_manifest(promoted)
    publish_generation(output, payload, batch, report)
    return {
        "entries": len(promoted["entries"]),
        "fixtures": len(promoted["compilerFixtures"]),
    }


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--batch", required=True, type=Path)
    parser.add_argument("--evidence-root", required=True, type=Path)
    parser.add_argument("--output-directory", required=True, type=Path)
    parser.add_argument("--repository", default=REPO_ROOT, type=Path)
    parser.add_argument("--verifier-timeout-seconds", default=3600, type=int)
    options = parser.parse_args(arguments)
    if options.verifier_timeout_seconds <= 0:
        print("tutorial capability promotion: verifier timeout must be positive", file=sys.stderr)
        return 1
    try:
        stats = promote(
            options.repository,
            options.batch.resolve(strict=True),
            options.evidence_root,
            options.output_directory.resolve(strict=False),
            options.verifier_timeout_seconds,
        )
    except (
        OSError,
        PromotionError,
        manifest_contract.ManifestError,
        negative_contract.NegativeFixtureError,
    ) as error:
        print(f"tutorial capability promotion: {error}", file=sys.stderr)
        return 1
    print(
        "published qualified tutorial manifest generation: "
        f"{stats['entries']} lessons, {stats['fixtures']} fixtures"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
