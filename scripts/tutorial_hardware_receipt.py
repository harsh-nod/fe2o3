#!/usr/bin/env python3
"""Create and verify authenticated tutorial hardware receipt archives.

The remote protocol has two phases. ``prepare`` snapshots and signs run evidence
before runner scratch is removed. ``finalize`` is called after cleanup; it checks
that scratch is absent, removes its own spool, signs that observation, and emits
one canonical archive on stdout. Successful verification authenticates evidence
only. It grants no compiler, load, launch, or publication authority.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
from typing import Any, Callable
import zipfile


POLICY_SCHEMA = "fe2o3-tutorial-hardware-attestation-policy-v1"
CHALLENGE_SCHEMA = "fe2o3-tutorial-hardware-receipt-challenge-v1"
RUN_SCHEMA = "fe2o3-tutorial-hardware-run-receipt-v1"
CLEANUP_SCHEMA = "fe2o3-tutorial-hardware-cleanup-receipt-v1"
PRE_CLEANUP_SCHEMA = "fe2o3-tutorial-hardware-pre-cleanup-capsule-v1"
TRANSPORT_SCHEMA = "fe2o3-tutorial-hardware-archive-v1"
ISA_OBSERVATION_SCHEMA = "fe2o3-tutorial-hardware-isa-observation-v1"
RESOURCE_OBSERVATION_SCHEMA = "fe2o3-tutorial-hardware-resource-observation-v1"
RESULT_OBSERVATION_SCHEMA = "fe2o3-tutorial-hardware-result-observation-v1"
TRANSPORT_NAME = "hardware-archive-v1.zip"
INDEX_NAME = "index-v1.json"
AUTHORITY = "authenticated-observation-no-independent-authority"
OPENSSL_PATH = Path("/usr/bin/openssl")
RUN_DOMAIN = b"fe2o3-tutorial-hardware-run-receipt-v1\0"
CLEANUP_DOMAIN = b"fe2o3-tutorial-hardware-cleanup-receipt-v1\0"
TRANSPORT_DOMAIN = b"fe2o3-tutorial-hardware-archive-v1\0"
SCRATCH_DOMAIN = b"fe2o3-tutorial-hardware-scratch-v1\0"
COMMAND_DOMAIN = b"fe2o3-tutorial-semantic-command-v1\0"
SIGNATURE_CONTEXT = b"fe2o3-tutorial-hardware-receipt-signature-v1\0"
CAPABILITY_RESULT_SET_DOMAIN = b"FE2O3/INERT-CAPABILITY-RESULT-SET/V1\0"
CAPABILITY_RESULT_SET_MAGIC = b"FE2OCAPR"
OBJECT_PREFIX = PurePosixPath("objects/sha256")
MAX_JSON_BYTES = 4 * 1024 * 1024
MAX_OBJECT_BYTES = 512 * 1024 * 1024
MAX_ARCHIVE_BYTES = 1024 * 1024 * 1024
MAX_ARCHIVE_OBJECTS = 32
MAX_WORKGROUP_THREADS = 1024 * 1024
MAX_RESOURCE_VALUE = (1 << 63) - 1
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_ID = re.compile(r"[0-9a-f]{40}\Z")
IDENTITY = re.compile(r"[a-z0-9][a-z0-9._-]{0,95}\Z")
TARGET = re.compile(r"gfx[0-9a-f]{3}(?::[A-Za-z0-9_+-]+)*\Z")
COMPILER_OBSERVATION_KINDS = {
    "artifact": "artifact",
    "artifactInspection": "artifact-inspection",
    "compilerPolicy": "compiler-policy",
    "driver": "driver-identity",
    "kir": "optimized-kir-v13",
    "llvm": "llvm-module",
    "numericalPolicy": "numerical-policy",
    "proof": "proof-evidence",
    "proofChecker": "proof-checker",
    "proofObligations": "proof-obligation-set",
    "runtime": "runtime-identity",
    "source": "source-closure",
    "target": "target-identity",
    "targetDecision": "target-capability-decision",
}


class HardwareReceiptError(ValueError):
    """A hardware archive is malformed, unauthenticated, stale, or replayed."""


def _fail(message: str) -> None:
    raise HardwareReceiptError(message)


def canonical(value: Any) -> bytes:
    try:
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
    except (TypeError, ValueError, UnicodeError) as error:
        _fail(f"cannot encode canonical JSON: {error}")


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _domain_identity(domain: bytes, value: Any) -> str:
    return sha256(domain + canonical(value))


def _capability_result_set_identity(payload: bytes) -> str:
    if not 48 <= len(payload) <= 1024 * 1024 or payload[:8] != CAPABILITY_RESULT_SET_MAGIC:
        _fail("proof evidence has an invalid typed result-set envelope")
    version, flags, declared = struct.unpack_from("<HHI", payload, 8)
    terminal = payload[-32:]
    expected = bytes.fromhex(
        sha256(
            CAPABILITY_RESULT_SET_DOMAIN
            + struct.pack("<Q", len(payload) - 32)
            + payload[:-32]
        )
    )
    if version != 1 or flags != 0 or declared != len(payload) or terminal != expected:
        _fail("proof evidence typed result-set identity differs from its canonical body")
    return terminal.hex()


def _authenticated_checker_evidence_identity(payload: bytes) -> str:
    if not payload:
        _fail("authenticated proof checker evidence is empty")
    return sha256(
        b"FE2O3/AUTHENTICATED-COMPILER-CAPABILITY-EVIDENCE-IDENTITY/V5\0"
        + struct.pack("<Q", len(payload))
        + payload
    )


def _command_identity(command: Any) -> str:
    return sha256(COMMAND_DOMAIN + canonical(command)[:-1])


def _decode_unique(payload: bytes, label: str) -> Any:
    def unique(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        return json.loads(payload, object_pairs_hook=unique)
    except (UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot decode {label}: {error}")


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _exact(value: dict[str, Any], fields: set[str], label: str) -> None:
    if set(value) != fields:
        _fail(
            f"{label} fields differ: missing={sorted(fields - set(value))!r} "
            f"extra={sorted(set(value) - fields)!r}"
        )


def _identity(value: Any, label: str) -> str:
    if not isinstance(value, str) or IDENTITY.fullmatch(value) is None:
        _fail(f"{label} is not a canonical identity")
    return value


def _digest(value: Any, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256.fullmatch(value) is None
        or value == "0" * 64
    ):
        _fail(f"{label} is not a nonzero lowercase SHA-256 identity")
    return value


def _read_regular(path: Path, label: str, maximum: int) -> bytes:
    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
            _fail(f"{label} must be a regular non-symlink file")
        if metadata.st_size <= 0 or metadata.st_size > maximum:
            _fail(f"{label} has an invalid byte length")
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
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
    if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    ) or observed != metadata.st_size:
        _fail(f"{label} changed while it was read")
    return b"".join(chunks)


def _load_json_file(path: Path, label: str) -> dict[str, Any]:
    payload = _read_regular(path, label, MAX_JSON_BYTES)
    value = _object(_decode_unique(payload, label), label)
    if payload != canonical(value):
        _fail(f"{label} is not canonical JSON followed by one newline")
    return value


def object_reference(payload: bytes) -> dict[str, Any]:
    digest = sha256(payload)
    return {
        "bytes": len(payload),
        "path": str(OBJECT_PREFIX / digest[:2] / digest),
        "sha256": digest,
    }


def validate_reference(value: Any, label: str) -> dict[str, Any]:
    reference = _object(value, label)
    _exact(reference, {"bytes", "path", "sha256"}, label)
    digest = _digest(reference["sha256"], f"{label}.sha256")
    size = reference["bytes"]
    if (
        not isinstance(size, int)
        or isinstance(size, bool)
        or not 0 < size <= MAX_OBJECT_BYTES
    ):
        _fail(f"{label}.bytes is invalid")
    if reference["path"] != str(OBJECT_PREFIX / digest[:2] / digest):
        _fail(f"{label}.path is not content-addressed")
    return reference


def _candidate(value: Any, label: str) -> dict[str, Any]:
    candidate = _object(value, label)
    _exact(candidate, {"compilerCommit", "compilerTree", "worktreeClean"}, label)
    if (
        not isinstance(candidate["compilerCommit"], str)
        or GIT_ID.fullmatch(candidate["compilerCommit"]) is None
        or candidate["compilerCommit"] == "0" * 40
        or not isinstance(candidate["compilerTree"], str)
        or GIT_ID.fullmatch(candidate["compilerTree"]) is None
        or candidate["compilerTree"] == "0" * 40
        or candidate["worktreeClean"] is not True
    ):
        _fail(f"{label} is not an exact clean compiler candidate")
    return candidate


def _binding(document: dict[str, Any], field: str, domain: bytes, label: str) -> None:
    observed = _digest(document[field], f"{label}.{field}")
    subject = {key: value for key, value in document.items() if key != field}
    if observed != _domain_identity(domain, subject):
        _fail(f"{label} binding is stale")


def _measure_executable(path: Path) -> tuple[bytes, str]:
    payload = _read_regular(path, "OpenSSL verifier", 128 * 1024 * 1024)
    metadata = path.stat()
    if (
        metadata.st_mode & 0o022
        or metadata.st_uid != 0
        or metadata.st_mode & 0o111 == 0
    ):
        _fail("OpenSSL verifier is not root-owned, executable, and non-writable")
    return payload, sha256(payload)


def _openssl(
    arguments: list[str], *, input_payload: bytes | None = None
) -> subprocess.CompletedProcess[bytes]:
    _, before = _measure_executable(OPENSSL_PATH)
    process = subprocess.run(
        [str(OPENSSL_PATH), *arguments],
        input=input_payload,
        stdin=None if input_payload is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=30,
    )
    _, after = _measure_executable(OPENSSL_PATH)
    if before != after:
        _fail("OpenSSL verifier changed while it was used")
    return process


@dataclass(frozen=True)
class LaneTrust:
    attestor_identity: str
    lane: str
    public_key: bytes
    public_key_sha256: str
    reservation_identity: str
    target: str


@dataclass(frozen=True)
class TrustPolicy:
    lanes: dict[tuple[str, str], LaneTrust]
    verifier_sha256: str


def load_trust_policy(path: Path) -> TrustPolicy:
    if not path.is_absolute() or path != Path(os.path.normpath(str(path))):
        _fail("hardware trust policy path must be absolute and lexically normalized")
    metadata = path.lstat()
    if path.is_symlink() or path.resolve(strict=True) != path or not stat.S_ISREG(
        metadata.st_mode
    ):
        _fail("hardware trust policy must be an exact regular non-symlink file")
    if metadata.st_mode & 0o022:
        _fail("hardware trust policy must not be group/other writable")
    policy = _load_json_file(path, "hardware trust policy")
    _exact(policy, {"lanes", "schema", "verifier"}, "hardware trust policy")
    verifier = _object(policy["verifier"], "hardware trust policy.verifier")
    _exact(verifier, {"path", "sha256"}, "hardware trust policy.verifier")
    _, measured = _measure_executable(OPENSSL_PATH)
    if (
        verifier["path"] != str(OPENSSL_PATH)
        or _digest(verifier["sha256"], "hardware trust policy.verifier.sha256")
        != measured
    ):
        _fail("hardware trust policy does not pin the active OpenSSL verifier")
    raw_lanes = policy["lanes"]
    if not isinstance(raw_lanes, list) or not raw_lanes:
        _fail("hardware trust policy.lanes must be a non-empty array")
    lanes: dict[tuple[str, str], LaneTrust] = {}
    ordering: list[tuple[str, str]] = []
    for index, raw in enumerate(raw_lanes):
        lane = _object(raw, f"hardware trust policy.lanes[{index}]")
        _exact(
            lane,
            {
                "attestorIdentity",
                "lane",
                "publicKeyPem",
                "publicKeySha256",
                "reservationIdentity",
                "target",
            },
            f"hardware trust policy.lanes[{index}]",
        )
        lane_name = _identity(
            lane["lane"], f"hardware trust policy.lanes[{index}].lane"
        )
        attestor = _identity(
            lane["attestorIdentity"],
            f"hardware trust policy.lanes[{index}].attestorIdentity",
        )
        reservation = _identity(
            lane["reservationIdentity"],
            f"hardware trust policy.lanes[{index}].reservationIdentity",
        )
        target = lane["target"]
        if not isinstance(target, str) or TARGET.fullmatch(target) is None:
            _fail(f"hardware trust policy.lanes[{index}].target is invalid")
        try:
            public_key = lane["publicKeyPem"].encode("ascii")
        except (AttributeError, UnicodeError):
            _fail(f"hardware trust policy.lanes[{index}].publicKeyPem is invalid")
        process = _openssl(["pkey", "-pubin", "-pubout"], input_payload=public_key)
        if process.returncode != 0 or process.stdout != public_key:
            _fail(
                f"hardware trust policy.lanes[{index}] has non-canonical public key material"
            )
        der = _openssl(["pkey", "-pubin", "-outform", "DER"], input_payload=public_key)
        if (
            der.returncode != 0
            or len(der.stdout) != 44
            or not der.stdout.startswith(bytes.fromhex("302a300506032b6570032100"))
        ):
            _fail(f"hardware trust policy.lanes[{index}] key is not Ed25519")
        key_sha = _digest(
            lane["publicKeySha256"],
            f"hardware trust policy.lanes[{index}].publicKeySha256",
        )
        if key_sha != sha256(public_key):
            _fail(f"hardware trust policy.lanes[{index}] public key identity differs")
        key = (lane_name, target)
        if key in lanes:
            _fail("hardware trust policy contains a duplicate lane/target")
        lanes[key] = LaneTrust(
            attestor, lane_name, public_key, key_sha, reservation, target
        )
        ordering.append(key)
    if ordering != sorted(ordering):
        _fail("hardware trust policy lanes are not canonically ordered")
    if policy["schema"] != POLICY_SCHEMA:
        _fail("hardware trust policy schema differs")
    return TrustPolicy(lanes, measured)


def trust_policy_document(lanes: list[dict[str, str]]) -> dict[str, Any]:
    """Build policy bytes from already trusted public keys for deployment tooling."""
    _, verifier = _measure_executable(OPENSSL_PATH)
    normalized = []
    for lane in lanes:
        public_key_path = Path(lane["publicKeyPath"])
        if (
            not public_key_path.is_absolute()
            or public_key_path != Path(os.path.normpath(str(public_key_path)))
            or public_key_path.is_symlink()
            or public_key_path.resolve(strict=True) != public_key_path
        ):
            _fail("hardware attestor public key path must be exact and non-symlink")
        public_key = _read_regular(
            public_key_path, "hardware attestor public key", 1024 * 1024
        )
        process = _openssl(["pkey", "-pubin", "-pubout"], input_payload=public_key)
        if process.returncode != 0:
            _fail("cannot canonicalize hardware attestor public key")
        normalized.append(
            {
                "attestorIdentity": lane["attestorIdentity"],
                "lane": lane["lane"],
                "publicKeyPem": process.stdout.decode("ascii"),
                "publicKeySha256": sha256(process.stdout),
                "reservationIdentity": lane["reservationIdentity"],
                "target": lane["target"],
            }
        )
    normalized.sort(key=lambda item: (item["lane"], item["target"]))
    return {
        "lanes": normalized,
        "schema": POLICY_SCHEMA,
        "verifier": {"path": str(OPENSSL_PATH), "sha256": verifier},
    }


def _derive_public_key(private_key: Path) -> bytes:
    metadata = private_key.lstat()
    if (
        not private_key.is_absolute()
        or private_key != Path(os.path.normpath(str(private_key)))
        or private_key.is_symlink()
        or private_key.resolve(strict=True) != private_key
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_mode & 0o077
    ):
        _fail("hardware attestor private key must be a private regular file")
    process = _openssl(["pkey", "-in", str(private_key), "-pubout"])
    if process.returncode != 0:
        _fail("cannot derive the hardware attestor public key")
    canonical_key = _openssl(
        ["pkey", "-pubin", "-pubout"], input_payload=process.stdout
    )
    if canonical_key.returncode != 0:
        _fail("hardware attestor did not derive canonical Ed25519 public material")
    der = _openssl(
        ["pkey", "-pubin", "-outform", "DER"], input_payload=canonical_key.stdout
    )
    if (
        der.returncode != 0
        or len(der.stdout) != 44
        or not der.stdout.startswith(bytes.fromhex("302a300506032b6570032100"))
    ):
        _fail("hardware attestor key is not Ed25519")
    return canonical_key.stdout


def _sign(payload: bytes, private_key: Path) -> bytes:
    signed = SIGNATURE_CONTEXT + payload
    with tempfile.TemporaryDirectory(prefix="fe2o3-hardware-sign-") as temporary:
        input_path = Path(temporary) / "payload.bin"
        output_path = Path(temporary) / "signature.bin"
        input_path.write_bytes(signed)
        process = _openssl(
            [
                "pkeyutl",
                "-sign",
                "-rawin",
                "-inkey",
                str(private_key),
                "-in",
                str(input_path),
                "-out",
                str(output_path),
            ]
        )
        if process.returncode != 0:
            _fail("hardware attestor failed to produce an Ed25519 signature")
        signature = _read_regular(output_path, "hardware receipt signature", 64)
    if len(signature) != 64:
        _fail("hardware attestor produced a malformed Ed25519 signature")
    return signature


def _verify(payload: bytes, signature: bytes, trust: LaneTrust) -> None:
    if len(signature) != 64:
        _fail("hardware receipt has a malformed Ed25519 signature")
    with tempfile.TemporaryDirectory(prefix="fe2o3-hardware-signature-") as temporary:
        public_key = Path(temporary) / "public.pem"
        payload_path = Path(temporary) / "payload.bin"
        signature_path = Path(temporary) / "signature.bin"
        public_key.write_bytes(trust.public_key)
        payload_path.write_bytes(SIGNATURE_CONTEXT + payload)
        signature_path.write_bytes(signature)
        process = _openssl(
            [
                "pkeyutl",
                "-verify",
                "-pubin",
                "-inkey",
                str(public_key),
                "-rawin",
                "-in",
                str(payload_path),
                "-sigfile",
                str(signature_path),
            ]
        )
    if process.returncode != 0:
        _fail("hardware receipt signature authentication failed")


def _load_export_object(root: Path, reference: Any, label: str) -> bytes:
    reference = validate_reference(reference, label)
    root = root.resolve(strict=True)
    path = root
    for component in PurePosixPath(reference["path"]).parts:
        path /= component
        if path.is_symlink():
            _fail(f"{label} traverses a symlink")
    if not path.resolve(strict=True).is_relative_to(root):
        _fail(f"{label} escapes its evidence root")
    payload = _read_regular(path, label, MAX_OBJECT_BYTES)
    if len(payload) != reference["bytes"] or sha256(payload) != reference["sha256"]:
        _fail(f"{label} content identity differs")
    return payload


def _scratch_path(path: Path, *, must_exist: bool) -> tuple[Path, str]:
    if not path.is_absolute():
        _fail("hardware runner scratch path must be absolute")
    normalized = Path(os.path.normpath(str(path)))
    if str(normalized) != str(path):
        _fail("hardware runner scratch path must be lexically normalized")
    if must_exist:
        resolved = normalized.resolve(strict=True)
        if resolved != normalized or normalized.is_symlink() or not normalized.is_dir():
            _fail("hardware runner scratch must be a real, symlink-free directory")
    elif normalized.exists() or normalized.is_symlink():
        _fail("hardware runner left scratch residue after cleanup")
    return normalized, sha256(SCRATCH_DOMAIN + str(normalized).encode("utf-8"))


def _add_payload(
    objects: dict[str, bytes], payload: bytes, label: str
) -> dict[str, Any]:
    if not payload or len(payload) > MAX_OBJECT_BYTES:
        _fail(f"{label} has an invalid byte length")
    reference = object_reference(payload)
    previous = objects.setdefault(reference["sha256"], payload)
    if previous != payload:
        _fail(f"{label} collides with another content-addressed object")
    return reference


def _write_spool(spool: Path, objects: dict[str, bytes], state: dict[str, Any]) -> None:
    if spool.exists() or spool.is_symlink():
        _fail("hardware receipt spool must be a new path")
    if not spool.is_absolute() or spool != Path(os.path.normpath(str(spool))):
        _fail("hardware receipt spool must be an absolute normalized path")
    if spool.parent.resolve(strict=True) != spool.parent:
        _fail("hardware receipt spool parent must be a real directory")
    parent_fd = os.open(
        spool.parent,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        os.mkdir(spool.name, mode=0o700, dir_fd=parent_fd)
        spool_fd = os.open(
            spool.name,
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent_fd,
        )
        directories: dict[str, int] = {}
        objects_fd = _mkdir_open_at(spool_fd, "objects", 0o700)
        sha_fd = _mkdir_open_at(objects_fd, "sha256", 0o700)
        for digest, payload in sorted(objects.items()):
            prefix_fd = directories.get(digest[:2])
            if prefix_fd is None:
                prefix_fd = _mkdir_open_at(sha_fd, digest[:2], 0o700)
                directories[digest[:2]] = prefix_fd
            _write_new_at(prefix_fd, digest, payload, 0o600)
        _write_new_at(spool_fd, "state-v1.json", canonical(state), 0o600)
        for descriptor in directories.values():
            os.fsync(descriptor)
        os.fsync(sha_fd)
        os.fsync(objects_fd)
        os.fsync(spool_fd)
        os.fsync(parent_fd)
    except BaseException:
        shutil.rmtree(spool, ignore_errors=True)
        raise
    finally:
        for descriptor in locals().get("directories", {}).values():
            os.close(descriptor)
        for name in ("sha_fd", "objects_fd", "spool_fd", "parent_fd"):
            descriptor = locals().get(name)
            if descriptor is not None:
                os.close(descriptor)


def _mkdir_open_at(parent_fd: int, name: str, mode: int) -> int:
    if not name or "/" in name or name in {".", ".."}:
        _fail("descriptor-relative directory component is invalid")
    os.mkdir(name, mode=mode, dir_fd=parent_fd)
    return os.open(
        name,
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        dir_fd=parent_fd,
    )


def _write_new_at(parent_fd: int, name: str, payload: bytes, mode: int) -> None:
    if not name or "/" in name or name in {".", ".."}:
        _fail("descriptor-relative file component is invalid")
    descriptor = os.open(
        name,
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
        mode,
        dir_fd=parent_fd,
    )
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    except BaseException:
        try:
            os.unlink(name, dir_fd=parent_fd)
        except FileNotFoundError:
            pass
        raise


def _canonical_observation(payload: bytes, label: str) -> dict[str, Any]:
    document = _object(_decode_unique(payload, label), label)
    if payload != canonical(document):
        _fail(f"{label} must be canonical JSON followed by one newline")
    return document


def _kernel_symbols(value: Any, label: str) -> list[str]:
    if (
        not isinstance(value, list)
        or not value
        or len(value) > 1024
        or value != sorted(set(value))
        or any(
            not isinstance(symbol, str) or not symbol or len(symbol.encode("utf-8")) > 1024
            for symbol in value
        )
    ):
        _fail(f"{label} must be a sorted, unique, non-empty string array")
    return value


def _validate_runner_observations(
    request: dict[str, Any],
    record: dict[str, Any],
    observations: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    fixture = _object(request.get("fixture"), "transaction request.fixture")
    candidate = _candidate(request.get("candidate"), "transaction request.candidate")
    target = fixture.get("target")
    lane = _identity(
        fixture.get("hardwareLane"), "transaction request.fixture.hardwareLane"
    )
    reservation = _identity(
        fixture.get("hardwareReservation"),
        "transaction request.fixture.hardwareReservation",
    )
    symbols = _kernel_symbols(
        _object(fixture.get("compilerInput"), "transaction request.fixture.compilerInput").get(
            "kernelSymbols"
        ),
        "transaction request kernel symbols",
    )
    transaction_sha = _digest(
        _object(
            record.get("productionTransaction"), "production record.productionTransaction"
        ).get("transactionSha256"),
        "production transaction identity",
    )
    artifact_sha = observations["artifact"]["sha256"]
    llvm_sha = observations["llvm"]["sha256"]
    driver_sha = observations["driver"]["sha256"]
    runtime_sha = observations["runtime"]["sha256"]

    isa = _canonical_observation(
        _load_spooled_observation(observations, "isa"), "hardware ISA observation"
    )
    _exact(
        isa,
        {
            "artifactSha256",
            "disassembly",
            "inspectionToolSha256",
            "kernelSymbols",
            "llvmModuleSha256",
            "schema",
            "target",
        },
        "hardware ISA observation",
    )
    _digest(isa.get("inspectionToolSha256"), "hardware ISA inspection tool")
    if (
        isa.get("schema") != ISA_OBSERVATION_SCHEMA
        or isa.get("artifactSha256") != artifact_sha
        or isa.get("llvmModuleSha256") != llvm_sha
        or isa.get("kernelSymbols") != symbols
        or isa.get("target") != target
        or not isinstance(isa.get("disassembly"), str)
        or not isa["disassembly"]
    ):
        _fail("hardware ISA observation is incomplete, stale, or substituted")

    resource = _canonical_observation(
        _load_spooled_observation(observations, "resource"),
        "hardware resource observation",
    )
    _exact(
        resource,
        {
            "artifactSha256",
            "inspectionToolSha256",
            "kernelSymbols",
            "ldsBytes",
            "registersPerWorkgroup",
            "schema",
            "scratchBytes",
            "target",
            "workgroupSize",
        },
        "hardware resource observation",
    )
    _digest(resource.get("inspectionToolSha256"), "hardware resource inspection tool")
    sizes = resource.get("workgroupSize")
    if (
        resource.get("schema") != RESOURCE_OBSERVATION_SCHEMA
        or resource.get("artifactSha256") != artifact_sha
        or resource.get("kernelSymbols") != symbols
        or resource.get("target") != target
        or not isinstance(sizes, list)
        or len(sizes) != 3
        or any(
            not isinstance(value, int)
            or isinstance(value, bool)
            or value <= 0
            or value > MAX_WORKGROUP_THREADS
            for value in sizes
        )
        or sizes[0] * sizes[1] * sizes[2] > MAX_WORKGROUP_THREADS
        or any(
            not isinstance(resource.get(field), int)
            or isinstance(resource.get(field), bool)
            or resource[field] < 0
            or resource[field] > MAX_RESOURCE_VALUE
            for field in ("ldsBytes", "registersPerWorkgroup", "scratchBytes")
        )
    ):
        _fail("hardware resource observation is incomplete, stale, or substituted")

    result = _canonical_observation(
        _load_spooled_observation(observations, "result"),
        "hardware result observation",
    )
    _exact(
        result,
        {
            "artifactSha256",
            "authority",
            "candidate",
            "checks",
            "commandSha256",
            "driverIdentitySha256",
            "fixtureId",
            "kernelSymbols",
            "lane",
            "observedIdentities",
            "outcome",
            "reservationIdentity",
            "runtimeIdentitySha256",
            "schema",
            "target",
            "transactionSha256",
        },
        "hardware result observation",
    )
    checks = _object(result.get("checks"), "hardware result observation.checks")
    _exact(
        checks,
        {
            "canariesChecked",
            "completeOutputChecked",
            "inputsUnchangedChecked",
            "paddingChecked",
            "timedOut",
        },
        "hardware result observation.checks",
    )
    identities = _object(
        result.get("observedIdentities"),
        "hardware result observation.observedIdentities",
    )
    _exact(
        identities,
        {
            "canaryAfterSha256",
            "canaryBeforeSha256",
            "expectedOutputSha256",
            "inputAfterSha256",
            "inputBeforeSha256",
            "observedOutputSha256",
            "paddingAfterSha256",
            "paddingBeforeSha256",
        },
        "hardware result observation.observedIdentities",
    )
    for field, digest in identities.items():
        _digest(digest, f"hardware result observation.observedIdentities.{field}")
    if (
        result.get("schema") != RESULT_OBSERVATION_SCHEMA
        or result.get("authority") != "observation-only"
        or result.get("outcome") != "passed"
        or result.get("candidate") != candidate
        or result.get("commandSha256") != _command_identity(fixture.get("hardwareCommand"))
        or result.get("fixtureId") != fixture.get("fixtureId")
        or result.get("target") != target
        or result.get("lane") != lane
        or result.get("reservationIdentity") != reservation
        or result.get("kernelSymbols") != symbols
        or result.get("transactionSha256") != transaction_sha
        or result.get("artifactSha256") != artifact_sha
        or result.get("driverIdentitySha256") != driver_sha
        or result.get("runtimeIdentitySha256") != runtime_sha
        or any(checks[field] is not True for field in checks if field != "timedOut")
        or checks["timedOut"] is not False
        or identities["expectedOutputSha256"] != identities["observedOutputSha256"]
        or identities["inputBeforeSha256"] != identities["inputAfterSha256"]
        or identities["canaryBeforeSha256"] != identities["canaryAfterSha256"]
        or identities["paddingBeforeSha256"] != identities["paddingAfterSha256"]
    ):
        _fail("hardware result is not a complete target-matched semantic observation")
    return result


def _load_spooled_observation(
    observations: dict[str, dict[str, Any]], name: str
) -> bytes:
    payload = observations[name].get("_payload")
    if not isinstance(payload, bytes):
        _fail(f"hardware {name} observation payload is unavailable")
    return payload


def prepare_transport(
    request: dict[str, Any],
    record: dict[str, Any],
    export_root: Path,
    isa_observation: bytes,
    resource_observation: bytes,
    result_observation: bytes,
    scratch_path: Path,
    spool: Path,
    attestor_identity: str,
    private_key: Path,
) -> bytes:
    """Sign and spool the pre-cleanup receipt while runner scratch still exists."""
    scratch, scratch_sha = _scratch_path(scratch_path, must_exist=True)
    spool_parent = spool.parent.resolve(strict=True)
    if spool_parent == scratch or spool_parent.is_relative_to(scratch):
        _fail("hardware receipt spool must be outside runner scratch")
    candidate = _candidate(request.get("candidate"), "transaction request.candidate")
    challenge = _object(
        request.get("hardwareReceiptChallenge"),
        "transaction request.hardwareReceiptChallenge",
    )
    _exact(
        challenge,
        {"nonce", "reservationIdentity", "schema", "transportSchema"},
        "hardware receipt challenge",
    )
    nonce = _digest(challenge["nonce"], "hardware receipt challenge.nonce")
    if (
        challenge["schema"] != CHALLENGE_SCHEMA
        or challenge["transportSchema"] != TRANSPORT_SCHEMA
    ):
        _fail("hardware receipt challenge schema differs")
    fixture = _object(request.get("fixture"), "transaction request.fixture")
    lane = _identity(
        fixture.get("hardwareLane"), "transaction request.fixture.hardwareLane"
    )
    reservation = _identity(
        fixture.get("hardwareReservation"),
        "transaction request.fixture.hardwareReservation",
    )
    if challenge.get("reservationIdentity") != reservation:
        _fail("hardware receipt challenge reservation differs from the fixture")
    target = fixture.get("target")
    if not isinstance(target, str) or TARGET.fullmatch(target) is None:
        _fail("transaction request target is invalid")
    transaction = _object(
        record.get("productionTransaction"), "production record.productionTransaction"
    )
    transaction_sha = _digest(
        transaction.get("transactionSha256"),
        "production record.productionTransaction.transactionSha256",
    )
    files = _object(record.get("evidenceFiles"), "production record.evidenceFiles")
    objects: dict[str, bytes] = {}
    observations: dict[str, dict[str, Any]] = {}
    compiler_payloads: dict[str, bytes] = {}
    observation_payloads: dict[str, bytes] = {}
    for observation, kind in COMPILER_OBSERVATION_KINDS.items():
        reference = validate_reference(files.get(kind), f"production evidence {kind}")
        payload = _load_export_object(
            export_root, reference, f"production evidence {kind}"
        )
        observations[observation] = _add_payload(
            objects, payload, f"hardware {observation} observation"
        )
        compiler_payloads[observation] = payload
    for name, payload in (
        ("isa", isa_observation),
        ("resource", resource_observation),
        ("result", result_observation),
    ):
        observations[name] = _add_payload(objects, payload, f"hardware {name} observation")
        observation_payloads[name] = payload
    validation_observations = {
        name: {**reference, "_payload": observation_payloads[name]}
        if name in observation_payloads
        else reference
        for name, reference in observations.items()
    }
    _validate_runner_observations(request, record, validation_observations)
    production = _object(
        record.get("productionEvidence"), "production record.productionEvidence"
    )
    expected_production_joins = {
        "artifactInspectionSha256": "artifactInspection",
        "artifactSha256": "artifact",
    }
    if any(
        production.get(field) != observations[name]["sha256"]
        for field, name in expected_production_joins.items()
    ) or production.get("proofEvidenceSha256") != _capability_result_set_identity(
        compiler_payloads["proof"]
    ) or production.get("proofCheckerSha256") != _authenticated_checker_evidence_identity(
        compiler_payloads["proofChecker"]
    ):
        _fail("compiler-bound hardware input is stale or substituted")
    hardware = _object(record.get("hardware"), "production record.hardware")
    if (
        hardware.get("driverIdentitySha256") != observations["driver"]["sha256"]
        or hardware.get("runtimeIdentitySha256") != observations["runtime"]["sha256"]
    ):
        _fail("driver or runtime hardware input is stale or substituted")
    public_key = _derive_public_key(private_key)
    key_sha = sha256(public_key)
    run = {
        "artifactInspectionSha256": observations["artifactInspection"]["sha256"],
        "artifactSha256": observations["artifact"]["sha256"],
        "attestor": {
            "identity": _identity(attestor_identity, "hardware attestor identity"),
            "publicKeySha256": key_sha,
        },
        "authority": AUTHORITY,
        "candidate": candidate,
        "challengeNonce": nonce,
        "commandSha256": _command_identity(fixture.get("hardwareCommand")),
        "fixtureId": fixture.get("fixtureId"),
        "kernelSymbols": fixture.get("compilerInput", {}).get("kernelSymbols"),
        "lane": lane,
        "launchContractSha256": production.get("launchContractSha256"),
        "observations": observations,
        "outcome": "passed",
        "phase": "pre-cleanup",
        "receiptBindingSha256": "0" * 64,
        "requestBindingSha256": request.get("requestBindingSha256"),
        "reservationIdentity": reservation,
        "schema": RUN_SCHEMA,
        "scratchIdentitySha256": scratch_sha,
        "sequence": 1,
        "target": target,
        "targetIdentitySha256": production.get("targetIdentitySha256"),
        "transactionSha256": transaction_sha,
    }
    run["receiptBindingSha256"] = _domain_identity(
        RUN_DOMAIN,
        {key: value for key, value in run.items() if key != "receiptBindingSha256"},
    )
    run_payload = canonical(run)
    run_reference = _add_payload(objects, run_payload, "hardware run receipt")
    signature_reference = _add_payload(
        objects, _sign(run_payload, private_key), "hardware run receipt signature"
    )
    state = {
        "attestorIdentity": attestor_identity,
        "candidate": candidate,
        "challengeNonce": nonce,
        "fixtureId": fixture.get("fixtureId"),
        "lane": lane,
        "publicKeySha256": key_sha,
        "requestBindingSha256": request.get("requestBindingSha256"),
        "reservationIdentity": reservation,
        "runReceipt": run_reference,
        "runSignature": signature_reference,
        "schema": PRE_CLEANUP_SCHEMA,
        "scratchIdentitySha256": scratch_sha,
        "target": target,
        "transactionSha256": transaction_sha,
    }
    _write_spool(spool, objects, state)
    return _canonical_archive(state, objects)


def _load_spool(spool: Path) -> tuple[dict[str, Any], dict[str, bytes]]:
    if spool.is_symlink() or not spool.is_dir() or spool.resolve(strict=True) != spool:
        _fail("hardware receipt spool is missing or not a real directory")
    state = _load_json_file(spool / "state-v1.json", "hardware receipt spool state")
    objects: dict[str, bytes] = {}
    actual: set[str] = set()
    for current, directories, filenames in os.walk(spool, followlinks=False):
        if any((Path(current) / name).is_symlink() for name in directories + filenames):
            _fail("hardware receipt spool contains a symlink")
        for filename in filenames:
            path = Path(current) / filename
            relative = str(path.relative_to(spool))
            actual.add(relative)
            if relative == "state-v1.json":
                continue
            parts = PurePosixPath(relative).parts
            if len(parts) != 4 or parts[:2] != ("objects", "sha256"):
                _fail("hardware receipt spool contains an unexpected file")
            digest = _digest(parts[3], "hardware receipt spool object identity")
            if parts[2] != digest[:2]:
                _fail("hardware receipt spool object path is not content-addressed")
            payload = _read_regular(
                path, "hardware receipt spool object", MAX_OBJECT_BYTES
            )
            if sha256(payload) != digest:
                _fail("hardware receipt spool object identity differs")
            objects[digest] = payload
    expected = {"state-v1.json"}
    expected.update(str(OBJECT_PREFIX / digest[:2] / digest) for digest in objects)
    if actual != expected or len(objects) > MAX_ARCHIVE_OBJECTS:
        _fail("hardware receipt spool inventory differs")
    return state, objects


def _canonical_archive(index: dict[str, Any], objects: dict[str, bytes]) -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_STORED, allowZip64=True
    ) as archive:
        entries = [(INDEX_NAME, canonical(index))]
        entries.extend(
            (str(OBJECT_PREFIX / digest[:2] / digest), payload)
            for digest, payload in sorted(objects.items())
        )
        for name, payload in entries:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o600) << 16
            info.flag_bits = 0
            archive.writestr(info, payload)
    payload = output.getvalue()
    if not payload or len(payload) > MAX_ARCHIVE_BYTES:
        _fail("hardware receipt archive exceeds its byte bound")
    return payload


def _finalize_transport(
    spool: Path,
    scratch_path: Path,
    private_key: Path,
    request: dict[str, Any],
    record: dict[str, Any],
    export_root: Path,
) -> bytes:
    """Remove all remote residue and return the canonical signed archive bytes."""
    state, objects = _load_spool(spool)
    _exact(
        state,
        {
            "attestorIdentity",
            "candidate",
            "challengeNonce",
            "fixtureId",
            "lane",
            "publicKeySha256",
            "requestBindingSha256",
            "reservationIdentity",
            "runReceipt",
            "runSignature",
            "schema",
            "scratchIdentitySha256",
            "target",
            "transactionSha256",
        },
        "hardware receipt spool state",
    )
    scratch, scratch_sha = _scratch_path(scratch_path, must_exist=False)
    if scratch_sha != state["scratchIdentitySha256"]:
        _fail("cleanup checked a substituted hardware scratch path")
    public_key = _derive_public_key(private_key)
    if sha256(public_key) != state["publicKeySha256"]:
        _fail("cleanup used a different hardware attestor key")
    run_reference = validate_reference(
        state["runReceipt"], "hardware spool run receipt"
    )
    run_payload = objects.get(run_reference["sha256"])
    if run_payload is None:
        _fail("hardware spool omitted the run receipt")
    pre_cleanup_capsule_sha = sha256(_canonical_archive(state, objects))
    if (
        request.get("candidate") != state["candidate"]
        or request.get("requestBindingSha256") != state["requestBindingSha256"]
        or request.get("fixture", {}).get("fixtureId") != state["fixtureId"]
        or request.get("fixture", {}).get("hardwareLane") != state["lane"]
        or request.get("fixture", {}).get("hardwareReservation")
        != state["reservationIdentity"]
        or request.get("fixture", {}).get("target") != state["target"]
        or record.get("productionTransaction", {}).get("transactionSha256")
        != state["transactionSha256"]
    ):
        _fail("cleanup received a substituted transaction or candidate")
    run_document = _object(
        _decode_unique(run_payload, "hardware spool run receipt"),
        "hardware spool run receipt",
    )
    if run_payload != canonical(run_document):
        _fail("hardware spool run receipt is not canonical")
    run_observations = _object(
        run_document.get("observations"), "hardware spool run observations"
    )
    result_reference = validate_reference(
        run_observations.get("result"), "hardware result observation"
    )
    result_payload = objects.get(result_reference["sha256"])
    if result_payload is None:
        _fail("hardware spool omitted the result observation")
    result_document = _canonical_observation(
        result_payload, "hardware result observation"
    )
    result_checks = _object(
        result_document.get("checks"), "hardware result observation.checks"
    )
    hardware_payload = canonical(
        {
            "artifactInspectionSha256": run_document["artifactInspectionSha256"],
            "artifactSha256": run_document["artifactSha256"],
            "authority": "verification-input-no-independent-authority",
            "candidate": state["candidate"],
            "checks": {
                "canariesChecked": result_checks.get("canariesChecked"),
                "cleanupComplete": True,
                "completeOutputChecked": result_checks.get("completeOutputChecked"),
                "inputsUnchangedChecked": result_checks.get("inputsUnchangedChecked"),
                "isaInspected": True,
                "paddingChecked": result_checks.get("paddingChecked"),
                "resourceUsageInspected": True,
                "timedOut": result_checks.get("timedOut"),
            },
            "commandSha256": run_document["commandSha256"],
            "driverIdentitySha256": run_observations["driver"]["sha256"],
            "fixtureId": state["fixtureId"],
            "isaInspectionSha256": run_observations["isa"]["sha256"],
            "kernelSymbols": run_document["kernelSymbols"],
            "lane": state["lane"],
            "launchContractSha256": run_document["launchContractSha256"],
            "outcome": "passed",
            "reservationIdentity": state["reservationIdentity"],
            "resourceUsageSha256": run_observations["resource"]["sha256"],
            "resultSha256": result_reference["sha256"],
            "runtimeIdentitySha256": run_observations["runtime"]["sha256"],
            "schema": "fe2o3-tutorial-hardware-qualification-evidence-v1",
            "target": state["target"],
            "targetIdentitySha256": run_document["targetIdentitySha256"],
            "timeoutSeconds": request["fixture"]["hardwareCommand"]["timeoutSeconds"],
        }
    )
    hardware_reference = _add_payload(
        objects, hardware_payload, "hardware summary evidence"
    )
    cleanup_observation = canonical(
        {
            "checks": {
                "receiptSpoolAbsentBeforeEmission": True,
                "scratchDescendantsAbsent": True,
                "scratchRootAbsent": True,
            },
            "scratchIdentitySha256": scratch_sha,
            "schema": "fe2o3-tutorial-hardware-cleanup-observation-v1",
        }
    )
    cleanup_observation_reference = _add_payload(
        objects, cleanup_observation, "hardware cleanup observation"
    )
    cleanup = {
        "attestor": {
            "identity": state["attestorIdentity"],
            "publicKeySha256": state["publicKeySha256"],
        },
        "authority": AUTHORITY,
        "candidate": state["candidate"],
        "challengeNonce": state["challengeNonce"],
        "checks": {
            "cleanupCompleted": True,
            "receiptSpoolAbsentBeforeEmission": True,
            "scratchDescendantsAbsent": True,
            "scratchRootAbsent": True,
        },
        "cleanupObservation": cleanup_observation_reference,
        "fixtureId": state["fixtureId"],
        "hardwareEvidence": hardware_reference,
        "lane": state["lane"],
        "outcome": "passed",
        "phase": "post-cleanup",
        "preCleanupCapsuleSha256": pre_cleanup_capsule_sha,
        "receiptBindingSha256": "0" * 64,
        "requestBindingSha256": state["requestBindingSha256"],
        "reservationIdentity": state["reservationIdentity"],
        "runReceiptSha256": run_reference["sha256"],
        "schema": CLEANUP_SCHEMA,
        "scratchIdentitySha256": scratch_sha,
        "sequence": 2,
        "target": state["target"],
        "transactionSha256": state["transactionSha256"],
    }
    cleanup["receiptBindingSha256"] = _domain_identity(
        CLEANUP_DOMAIN,
        {key: value for key, value in cleanup.items() if key != "receiptBindingSha256"},
    )
    cleanup_payload = canonical(cleanup)
    cleanup_reference = _add_payload(
        objects, cleanup_payload, "hardware cleanup receipt"
    )
    cleanup_signature = _add_payload(
        objects,
        _sign(cleanup_payload, private_key),
        "hardware cleanup receipt signature",
    )
    index = {
        "candidate": state["candidate"],
        "challengeNonce": state["challengeNonce"],
        "cleanupReceipt": cleanup_reference,
        "cleanupSignature": cleanup_signature,
        "fixtureId": state["fixtureId"],
        "lane": state["lane"],
        "preCleanupCapsuleSha256": pre_cleanup_capsule_sha,
        "requestBindingSha256": state["requestBindingSha256"],
        "reservationIdentity": state["reservationIdentity"],
        "runReceipt": run_reference,
        "runSignature": state["runSignature"],
        "schema": TRANSPORT_SCHEMA,
        "target": state["target"],
        "transactionSha256": state["transactionSha256"],
        "transportBindingSha256": "0" * 64,
    }
    index["transportBindingSha256"] = _domain_identity(
        TRANSPORT_DOMAIN,
        {key: value for key, value in index.items() if key != "transportBindingSha256"},
    )
    # The signed run data is held in memory before the final residue check.
    shutil.rmtree(spool)
    if spool.exists() or spool.is_symlink() or scratch.exists() or scratch.is_symlink():
        _fail("hardware runner left scratch or receipt spool residue")
    return _canonical_archive(index, objects)


def finalize_transport(
    spool: Path,
    scratch_path: Path,
    private_key: Path,
    request: dict[str, Any],
    record: dict[str, Any],
    export_root: Path,
) -> bytes:
    try:
        return _finalize_transport(
            spool, scratch_path, private_key, request, record, export_root
        )
    finally:
        shutil.rmtree(spool, ignore_errors=True)


@dataclass(frozen=True)
class ValidatedTransport:
    archive_reference: dict[str, Any]
    cleanup_receipt_sha256: str
    isa_inspection_sha256: str
    result_observation_sha256: str
    resource_usage_sha256: str
    run_receipt_sha256: str


ObjectWriter = Callable[[bytes], dict[str, Any]]


def _archive_entries(payload: bytes) -> tuple[dict[str, Any], dict[str, bytes]]:
    if not payload or len(payload) > MAX_ARCHIVE_BYTES:
        _fail("hardware receipt archive has an invalid byte length")
    try:
        with zipfile.ZipFile(io.BytesIO(payload), "r") as archive:
            infos = archive.infolist()
            names = [info.filename for info in infos]
            if (
                not names
                or len(names) > MAX_ARCHIVE_OBJECTS + 1
                or len(names) != len(set(names))
            ):
                _fail("hardware receipt archive inventory is invalid")
            if names != sorted(names) or names[0] != INDEX_NAME:
                _fail("hardware receipt archive entries are not canonically ordered")
            entries: dict[str, bytes] = {}
            total = 0
            for info in infos:
                if (
                    info.compress_type != zipfile.ZIP_STORED
                    or info.date_time != (1980, 1, 1, 0, 0, 0)
                    or info.extra
                    or info.comment
                    or info.is_dir()
                    or info.file_size <= 0
                    or info.file_size > MAX_OBJECT_BYTES
                    or info.compress_size != info.file_size
                ):
                    _fail("hardware receipt archive has non-canonical entry metadata")
                data = archive.read(info)
                total += len(data)
                if total > MAX_ARCHIVE_BYTES:
                    _fail("hardware receipt archive expands past its byte bound")
                entries[info.filename] = data
            if archive.comment:
                _fail("hardware receipt archive has a comment")
    except (OSError, zipfile.BadZipFile, RuntimeError) as error:
        _fail(f"cannot decode hardware receipt archive: {error}")
    index_payload = entries.pop(INDEX_NAME)
    index = _object(
        _decode_unique(index_payload, "hardware archive index"),
        "hardware archive index",
    )
    if index_payload != canonical(index):
        _fail("hardware archive index is not canonical JSON")
    objects: dict[str, bytes] = {}
    for name, data in entries.items():
        parts = PurePosixPath(name).parts
        if len(parts) != 4 or parts[:2] != ("objects", "sha256"):
            _fail("hardware receipt archive has a non-object entry")
        digest = _digest(parts[3], "hardware archive object identity")
        if parts[2] != digest[:2] or sha256(data) != digest:
            _fail("hardware archive object path or identity differs")
        objects[digest] = data
    if payload != _canonical_archive(index, objects):
        _fail("hardware receipt archive bytes are not canonical")
    return index, objects


def _archive_object(
    objects: dict[str, bytes], reference: Any, label: str, *, json_object: bool = False
) -> tuple[dict[str, Any], bytes, dict[str, Any] | None]:
    reference = validate_reference(reference, label)
    payload = objects.get(reference["sha256"])
    if payload is None or len(payload) != reference["bytes"]:
        _fail(f"hardware archive omitted {label}")
    document = None
    if json_object:
        document = _object(_decode_unique(payload, label), label)
        if payload != canonical(document):
            _fail(f"{label} is not canonical JSON")
    return reference, payload, document


def validate_and_ingest(
    archive_path: Path,
    request: dict[str, Any],
    record: dict[str, Any],
    policy: TrustPolicy,
    writer: ObjectWriter,
    seen_receipts: set[str],
) -> ValidatedTransport:
    _, active_verifier = _measure_executable(OPENSSL_PATH)
    if active_verifier != policy.verifier_sha256:
        _fail("pinned OpenSSL verifier identity changed")
    archive_payload = _read_regular(
        archive_path, "hardware receipt archive", MAX_ARCHIVE_BYTES
    )
    index, objects = _archive_entries(archive_payload)
    _exact(
        index,
        {
            "candidate",
            "challengeNonce",
            "cleanupReceipt",
            "cleanupSignature",
            "fixtureId",
            "lane",
            "preCleanupCapsuleSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "runReceipt",
            "runSignature",
            "schema",
            "target",
            "transactionSha256",
            "transportBindingSha256",
        },
        "hardware archive index",
    )
    _binding(
        index, "transportBindingSha256", TRANSPORT_DOMAIN, "hardware archive index"
    )
    fixture = _object(request.get("fixture"), "transaction request.fixture")
    challenge = _object(
        request.get("hardwareReceiptChallenge"),
        "transaction request.hardwareReceiptChallenge",
    )
    _exact(
        challenge,
        {"nonce", "reservationIdentity", "schema", "transportSchema"},
        "hardware receipt challenge",
    )
    _digest(challenge.get("nonce"), "hardware receipt challenge.nonce")
    if (
        challenge.get("schema") != CHALLENGE_SCHEMA
        or challenge.get("transportSchema") != TRANSPORT_SCHEMA
        or challenge.get("reservationIdentity") != fixture.get("hardwareReservation")
    ):
        _fail("hardware receipt challenge schema differs")
    transaction = _object(
        record.get("productionTransaction"), "production record.productionTransaction"
    )
    expected = {
        "candidate": request.get("candidate"),
        "challengeNonce": challenge.get("nonce"),
        "fixtureId": fixture.get("fixtureId"),
        "lane": fixture.get("hardwareLane"),
        "requestBindingSha256": request.get("requestBindingSha256"),
        "reservationIdentity": fixture.get("hardwareReservation"),
        "schema": TRANSPORT_SCHEMA,
        "target": fixture.get("target"),
        "transactionSha256": transaction.get("transactionSha256"),
    }
    if any(index[key] != value for key, value in expected.items()):
        _fail("hardware archive is replayed, cross-target, or cross-transaction")
    trust = policy.lanes.get((index["lane"], index["target"]))
    if trust is None:
        _fail("hardware archive lane and target are not trusted")
    if index["reservationIdentity"] != trust.reservation_identity:
        _fail("hardware archive reservation is not trusted for its lane and target")
    run_ref, run_payload, run = _archive_object(
        objects, index["runReceipt"], "hardware run receipt", json_object=True
    )
    assert run is not None
    signature_ref, signature, _ = _archive_object(
        objects, index["runSignature"], "hardware run signature"
    )
    _exact(
        run,
        {
            "artifactInspectionSha256",
            "artifactSha256",
            "attestor",
            "authority",
            "candidate",
            "challengeNonce",
            "commandSha256",
            "fixtureId",
            "kernelSymbols",
            "lane",
            "launchContractSha256",
            "observations",
            "outcome",
            "phase",
            "receiptBindingSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "schema",
            "scratchIdentitySha256",
            "sequence",
            "target",
            "targetIdentitySha256",
            "transactionSha256",
        },
        "hardware run receipt",
    )
    _binding(run, "receiptBindingSha256", RUN_DOMAIN, "hardware run receipt")
    attestor = _object(run["attestor"], "hardware run receipt.attestor")
    _exact(attestor, {"identity", "publicKeySha256"}, "hardware run receipt.attestor")
    observations = _object(run["observations"], "hardware run receipt.observations")
    expected_observations = set(COMPILER_OBSERVATION_KINDS) | {
        "isa",
        "resource",
        "result",
    }
    _exact(observations, expected_observations, "hardware run receipt.observations")
    observation_references: dict[str, dict[str, Any]] = {}
    for name in sorted(expected_observations):
        reference, payload, _ = _archive_object(
            objects, observations[name], f"hardware {name} observation"
        )
        observation_references[name] = {**reference, "_payload": payload}
    runner_observation_digests = {
        observation_references[name]["sha256"] for name in ("isa", "resource", "result")
    }
    if len(runner_observation_digests) != 3:
        _fail("hardware receipt conflates distinct runner observations")
    _validate_runner_observations(request, record, observation_references)
    observation_references = {
        name: {key: value for key, value in reference.items() if key != "_payload"}
        for name, reference in observation_references.items()
    }
    compiler_input = _object(
        fixture.get("compilerInput"), "transaction request.fixture.compilerInput"
    )
    kernel_symbols = compiler_input.get("kernelSymbols")
    if (
        not isinstance(kernel_symbols, list)
        or not kernel_symbols
        or kernel_symbols != sorted(set(kernel_symbols))
        or any(not isinstance(symbol, str) or not symbol for symbol in kernel_symbols)
    ):
        _fail("transaction request kernel symbols are not sorted and unique")
    production = _object(
        record.get("productionEvidence"), "production record.productionEvidence"
    )
    run_joins = {
        "artifactInspectionSha256": observation_references["artifactInspection"][
            "sha256"
        ],
        "artifactSha256": observation_references["artifact"]["sha256"],
    }
    if (
        run.get("kernelSymbols") != kernel_symbols
        or any(run.get(key) != value for key, value in run_joins.items())
        or run.get("artifactInspectionSha256")
        != production.get("artifactInspectionSha256")
        or run.get("artifactSha256") != production.get("artifactSha256")
        or run.get("launchContractSha256") != production.get("launchContractSha256")
        or run.get("targetIdentitySha256") != production.get("targetIdentitySha256")
    ):
        _fail("hardware run receipt artifact, launch, target, or ABI join differs")
    _digest(run.get("commandSha256"), "hardware run receipt.commandSha256")
    _digest(
        run.get("launchContractSha256"),
        "hardware run receipt.launchContractSha256",
    )
    if (
        run["schema"] != RUN_SCHEMA
        or run["authority"] != AUTHORITY
        or run["outcome"] != "passed"
        or run["phase"] != "pre-cleanup"
        or run["sequence"] != 1
        or attestor
        != {
            "identity": trust.attestor_identity,
            "publicKeySha256": trust.public_key_sha256,
        }
        or run.get("reservationIdentity") != trust.reservation_identity
        or any(run[key] != value for key, value in expected.items() if key != "schema")
    ):
        _fail("hardware run receipt is stale, cross-target, or unauthenticated")
    _digest(run["scratchIdentitySha256"], "hardware run receipt.scratchIdentitySha256")
    _verify(run_payload, signature, trust)
    cleanup_ref, cleanup_payload, cleanup = _archive_object(
        objects, index["cleanupReceipt"], "hardware cleanup receipt", json_object=True
    )
    assert cleanup is not None
    _, cleanup_signature, _ = _archive_object(
        objects, index["cleanupSignature"], "hardware cleanup signature"
    )
    _exact(
        cleanup,
        {
            "attestor",
            "authority",
            "candidate",
            "challengeNonce",
            "checks",
            "cleanupObservation",
            "fixtureId",
            "hardwareEvidence",
            "lane",
            "outcome",
            "phase",
            "preCleanupCapsuleSha256",
            "receiptBindingSha256",
            "requestBindingSha256",
            "reservationIdentity",
            "runReceiptSha256",
            "schema",
            "scratchIdentitySha256",
            "sequence",
            "target",
            "transactionSha256",
        },
        "hardware cleanup receipt",
    )
    _binding(
        cleanup, "receiptBindingSha256", CLEANUP_DOMAIN, "hardware cleanup receipt"
    )
    checks = _object(cleanup["checks"], "hardware cleanup receipt.checks")
    expected_checks = {
        "cleanupCompleted",
        "receiptSpoolAbsentBeforeEmission",
        "scratchDescendantsAbsent",
        "scratchRootAbsent",
    }
    _exact(checks, expected_checks, "hardware cleanup receipt.checks")
    cleanup_expected = {
        "attestor": attestor,
        "authority": AUTHORITY,
        "candidate": request.get("candidate"),
        "challengeNonce": challenge.get("nonce"),
        "fixtureId": fixture.get("fixtureId"),
        "lane": fixture.get("hardwareLane"),
        "outcome": "passed",
        "phase": "post-cleanup",
        "preCleanupCapsuleSha256": index["preCleanupCapsuleSha256"],
        "requestBindingSha256": request.get("requestBindingSha256"),
        "reservationIdentity": fixture.get("hardwareReservation"),
        "runReceiptSha256": run_ref["sha256"],
        "schema": CLEANUP_SCHEMA,
        "scratchIdentitySha256": run["scratchIdentitySha256"],
        "sequence": 2,
        "target": fixture.get("target"),
        "transactionSha256": transaction.get("transactionSha256"),
    }
    if any(cleanup[key] != value for key, value in cleanup_expected.items()) or any(
        checks[key] is not True for key in expected_checks
    ):
        _fail("hardware cleanup receipt is incomplete, stale, or cross-transaction")
    pre_cleanup_index = {
        "attestorIdentity": attestor["identity"],
        "candidate": request.get("candidate"),
        "challengeNonce": challenge.get("nonce"),
        "fixtureId": fixture.get("fixtureId"),
        "lane": fixture.get("hardwareLane"),
        "publicKeySha256": attestor["publicKeySha256"],
        "requestBindingSha256": request.get("requestBindingSha256"),
        "reservationIdentity": fixture.get("hardwareReservation"),
        "runReceipt": run_ref,
        "runSignature": index["runSignature"],
        "schema": PRE_CLEANUP_SCHEMA,
        "scratchIdentitySha256": run["scratchIdentitySha256"],
        "target": fixture.get("target"),
        "transactionSha256": transaction.get("transactionSha256"),
    }
    pre_cleanup_digests = {
        run_ref["sha256"],
        validate_reference(index["runSignature"], "hardware run signature")["sha256"],
    }
    pre_cleanup_digests.update(
        reference["sha256"] for reference in observation_references.values()
    )
    pre_cleanup_objects = {digest: objects[digest] for digest in pre_cleanup_digests}
    if _digest(
        index["preCleanupCapsuleSha256"],
        "hardware archive index.preCleanupCapsuleSha256",
    ) != sha256(_canonical_archive(pre_cleanup_index, pre_cleanup_objects)):
        _fail("hardware pre-cleanup capsule identity is stale or substituted")
    _verify(cleanup_payload, cleanup_signature, trust)
    cleanup_observation_ref, _, cleanup_observation = _archive_object(
        objects,
        cleanup["cleanupObservation"],
        "hardware cleanup observation",
        json_object=True,
    )
    assert cleanup_observation is not None
    _exact(
        cleanup_observation,
        {"checks", "schema", "scratchIdentitySha256"},
        "hardware cleanup observation",
    )
    if (
        cleanup_observation["schema"]
        != "fe2o3-tutorial-hardware-cleanup-observation-v1"
        or cleanup_observation["scratchIdentitySha256"] != run["scratchIdentitySha256"]
        or cleanup_observation["checks"]
        != {
            "receiptSpoolAbsentBeforeEmission": True,
            "scratchDescendantsAbsent": True,
            "scratchRootAbsent": True,
        }
    ):
        _fail("hardware cleanup observation is malformed or substituted")
    files = _object(record.get("evidenceFiles"), "production record.evidenceFiles")
    joins = COMPILER_OBSERVATION_KINDS
    referenced = {
        run_ref["sha256"],
        signature_ref["sha256"],
        cleanup_ref["sha256"],
        validate_reference(index["cleanupSignature"], "hardware cleanup signature")[
            "sha256"
        ],
        cleanup_observation_ref["sha256"],
    }
    hardware_reference = validate_reference(
        cleanup["hardwareEvidence"], "hardware cleanup summary evidence"
    )
    if hardware_reference != validate_reference(
        files.get("hardware"), "production evidence hardware"
    ):
        _fail("hardware cleanup summary evidence is substituted")
    _, hardware_payload, hardware_document = _archive_object(
        objects,
        hardware_reference,
        "hardware cleanup summary evidence",
        json_object=True,
    )
    assert hardware_document is not None
    if (
        hardware_document.get("commandSha256") != run["commandSha256"]
        or hardware_document.get("artifactSha256") != run["artifactSha256"]
        or hardware_document.get("artifactInspectionSha256")
        != run["artifactInspectionSha256"]
        or hardware_document.get("launchContractSha256") != run["launchContractSha256"]
        or hardware_document.get("targetIdentitySha256") != run["targetIdentitySha256"]
        or hardware_document.get("isaInspectionSha256")
        != observation_references["isa"]["sha256"]
        or hardware_document.get("resourceUsageSha256")
        != observation_references["resource"]["sha256"]
        or hardware_document.get("driverIdentitySha256")
        != observation_references["driver"]["sha256"]
        or hardware_document.get("runtimeIdentitySha256")
        != observation_references["runtime"]["sha256"]
        or hardware_document.get("resultSha256")
        != observation_references["result"]["sha256"]
        or hardware_document.get("reservationIdentity") != trust.reservation_identity
    ):
        _fail("hardware cleanup summary does not join the signed run observations")
    referenced.add(hardware_reference["sha256"])
    for observation, kind in joins.items():
        reference = validate_reference(
            observations[observation], f"hardware {observation} observation"
        )
        if reference != validate_reference(
            files.get(kind), f"production evidence {kind}"
        ):
            _fail(f"hardware {observation} observation is substituted")
        referenced.add(reference["sha256"])
    isa = validate_reference(observations["isa"], "hardware ISA observation")
    resource = validate_reference(
        observations["resource"], "hardware resource observation"
    )
    result = validate_reference(observations["result"], "hardware result observation")
    referenced.update((isa["sha256"], resource["sha256"], result["sha256"]))
    if set(objects) != referenced:
        _fail("hardware archive contains omitted or unreferenced objects")
    if run_ref["sha256"] in seen_receipts:
        _fail("hardware run receipt replay detected")
    for payload in objects.values():
        if writer(payload) != object_reference(payload):
            _fail("hardware archive changed during local ingestion")
    archive_reference = writer(archive_payload)
    if archive_reference != object_reference(archive_payload):
        _fail("hardware transport archive changed during local ingestion")
    seen_receipts.add(run_ref["sha256"])
    return ValidatedTransport(
        archive_reference,
        cleanup_ref["sha256"],
        isa["sha256"],
        result["sha256"],
        resource["sha256"],
        run_ref["sha256"],
    )


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("measure-verifier")
    policy = subparsers.add_parser("make-policy")
    policy.add_argument(
        "--lane",
        action="append",
        nargs=5,
        metavar=("LANE", "TARGET", "RESERVATION", "ATTESTOR", "PUBLIC_KEY"),
        required=True,
    )
    prepare = subparsers.add_parser("prepare")
    prepare.add_argument("--attestor-identity", required=True)
    prepare.add_argument("--evidence-root", required=True, type=Path)
    prepare.add_argument("--isa-observation", required=True, type=Path)
    prepare.add_argument("--private-key", required=True, type=Path)
    prepare.add_argument("--record", required=True, type=Path)
    prepare.add_argument("--request", required=True, type=Path)
    prepare.add_argument("--resource-observation", required=True, type=Path)
    prepare.add_argument("--result-observation", required=True, type=Path)
    prepare.add_argument("--scratch-path", required=True, type=Path)
    prepare.add_argument("--spool", required=True, type=Path)
    finalize = subparsers.add_parser("finalize")
    finalize.add_argument("--evidence-root", required=True, type=Path)
    finalize.add_argument("--private-key", required=True, type=Path)
    finalize.add_argument("--record", required=True, type=Path)
    finalize.add_argument("--request", required=True, type=Path)
    finalize.add_argument("--scratch-path", required=True, type=Path)
    finalize.add_argument("--spool", required=True, type=Path)
    options = parser.parse_args(arguments)
    try:
        if options.command == "measure-verifier":
            _, digest = _measure_executable(OPENSSL_PATH)
            print(digest)
        elif options.command == "make-policy":
            lanes = [
                {
                    "lane": lane,
                    "target": target,
                    "reservationIdentity": reservation,
                    "attestorIdentity": attestor,
                    "publicKeyPath": key,
                }
                for lane, target, reservation, attestor, key in options.lane
            ]
            sys.stdout.buffer.write(canonical(trust_policy_document(lanes)))
        elif options.command == "prepare":
            request = _load_json_file(options.request, "hardware transaction request")
            record = _load_json_file(options.record, "pre-hardware transaction record")
            sys.stdout.buffer.write(
                prepare_transport(
                    request,
                    record,
                    options.evidence_root,
                    _read_regular(
                        options.isa_observation, "ISA observation", MAX_OBJECT_BYTES
                    ),
                    _read_regular(
                        options.resource_observation,
                        "resource observation",
                        MAX_OBJECT_BYTES,
                    ),
                    _read_regular(
                        options.result_observation,
                        "result observation",
                        MAX_OBJECT_BYTES,
                    ),
                    options.scratch_path,
                    options.spool,
                    options.attestor_identity,
                    options.private_key,
                )
            )
        else:
            request = _load_json_file(options.request, "hardware transaction request")
            record = _load_json_file(
                options.record, "completed hardware transaction record"
            )
            payload = finalize_transport(
                options.spool,
                options.scratch_path,
                options.private_key,
                request,
                record,
                options.evidence_root,
            )
            sys.stdout.buffer.write(payload)
    except (HardwareReceiptError, OSError, subprocess.SubprocessError) as error:
        print(f"tutorial hardware receipt: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
