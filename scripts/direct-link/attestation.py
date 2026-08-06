#!/usr/bin/env python3
"""Verify bounded, canonical direct-link attestations with OpenSSH signatures.

Successful verification authenticates an observation. It does not authorize artifact
publication, module loading, or kernel launch.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import fcntl
import json
import os
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from common import (
    EvidenceError,
    metadata_snapshot,
    read_regular_file,
    require_bounded_text,
    require_commit,
    require_domain,
    require_target,
    require_typed_identity,
    typed_descriptor_identity,
    typed_identity,
)

SCHEMA_VERSION = 1
ATTESTATION_DOMAIN = "fe2o3-direct-link-attestation-v1"
POLICY_DOMAIN = "fe2o3-direct-link-trust-policy-v1"
VERIFIER_DOMAIN = "fe2o3-ssh-keygen-executable-v1"
SIGNATURE_NAMESPACE = ATTESTATION_DOMAIN
SSH_KEYGEN_PATH = Path("/usr/bin/ssh-keygen")

MAX_ATTESTATION_BYTES = 64 * 1024
MAX_POLICY_BYTES = 64 * 1024
MAX_SIGNATURE_BYTES = 16 * 1024
MAX_VERIFIER_BYTES = 64 * 1024 * 1024
MAX_VERIFIER_OUTPUT_BYTES = 16 * 1024
MAX_SIGNERS = 64
MAX_SUBJECTS = 16
MAX_ATTESTATION_LIFETIME_SECONDS = 7 * 24 * 60 * 60
MAX_CLOCK_SKEW_SECONDS = 5 * 60
MAX_VERIFIER_TIMEOUT_SECONDS = 30
MAX_UNIX_TIMESTAMP = 253402300799

ROLES = (
    "g2-worker",
    "g5-publication",
    "g6-bundle",
    "g7-hardware-runner",
    "g7-static-runner",
)
ROLE_SUBJECTS = {
    "g2-worker": (
        "llvm_toolchain",
        "request",
        "worker",
        "worker_executable",
    ),
    "g5-publication": ("linked_artifact", "publication", "request"),
    "g6-bundle": ("bundle", "ffi_closure", "final_artifact", "publication"),
    "g7-hardware-runner": (
        "argv",
        "bundle",
        "driver",
        "final_artifact",
        "hardware_run",
        "observed_gpu",
        "oracle",
        "test_executable",
    ),
    "g7-static-runner": (
        "argv",
        "bundle",
        "final_artifact",
        "ruleset",
        "runner_executable",
        "static_run",
    ),
}

_SIGNER_RE = re.compile(r"[a-z0-9][a-z0-9._@-]{0,63}\Z")
_SUBJECT_NAME_RE = re.compile(r"[a-z][a-z0-9_]{0,31}\Z")
_TYPED_IDENTITY_RE = re.compile(
    r"(?P<domain>[a-z0-9][a-z0-9-]{0,63}-v[1-9][0-9]*)-sha256-"
    r"(?P<digest>[0-9a-f]{64})\Z"
)
_POLICY_FIELDS = frozenset(
    ("domain", "schema_version", "signers", "verifier_identity", "verifier_path")
)
_SIGNER_FIELDS = frozenset(("public_key", "role", "signer_identity"))
_ATTESTATION_FIELDS = frozenset(
    (
        "build_identity",
        "domain",
        "expires_at",
        "issued_at",
        "role",
        "schema_version",
        "signer_identity",
        "source_commit",
        "subjects",
        "target",
    )
)
_SUBJECT_FIELDS = frozenset(("identity", "name"))


def _require_exact_fields(
    value: Mapping[str, Any], fields: frozenset[str], name: str
) -> None:
    actual = set(value)
    if actual != fields:
        missing = sorted(fields - actual)
        extra = sorted(actual - fields)
        raise EvidenceError(
            f"{name} has wrong fields; missing={missing}, extra={extra}"
        )


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError(f"JSON contains duplicate key: {key}")
        result[key] = value
    return result


def canonical_json_bytes(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
            + "\n"
        ).encode("ascii")
    except (TypeError, UnicodeEncodeError) as error:
        raise EvidenceError("canonical JSON contains an unsupported value") from error


def parse_canonical_json(data: bytes, name: str) -> dict[str, Any]:
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{name} must contain ASCII only") from error
    if not text.endswith("\n") or "\r" in text or "\0" in text:
        raise EvidenceError(f"{name} is not canonical newline-terminated ASCII JSON")
    try:
        value = json.loads(text, object_pairs_hook=_unique_object)
    except EvidenceError:
        raise
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{name} is not valid JSON") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"{name} must be a JSON object")
    if canonical_json_bytes(value) != data:
        raise EvidenceError(f"{name} is not canonically encoded")
    return value


def require_role(value: object) -> str:
    if not isinstance(value, str) or value not in ROLE_SUBJECTS:
        raise EvidenceError(f"role must be exactly one of {', '.join(ROLES)}")
    return value


def require_string(value: object, name: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{name} must be a string")
    return value


def require_signer_identity(value: object) -> str:
    if not isinstance(value, str) or _SIGNER_RE.fullmatch(value) is None:
        raise EvidenceError("signer_identity is malformed or exceeds 64 bytes")
    return value


def require_unix_timestamp(value: object, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceError(f"{name} must be an integer Unix timestamp")
    if not 0 <= value <= MAX_UNIX_TIMESTAMP:
        raise EvidenceError(f"{name} is outside the supported timestamp range")
    return value


def require_generic_typed_identity(value: object, name: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{name} must be a typed identity")
    match = _TYPED_IDENTITY_RE.fullmatch(value)
    if match is None:
        raise EvidenceError(f"{name} is not a canonical typed SHA-256 identity")
    domain = require_domain(match.group("domain"))
    return require_typed_identity(value, domain, name)


def _read_u32(data: bytes, offset: int) -> tuple[int, int]:
    if offset + 4 > len(data):
        raise EvidenceError("OpenSSH Ed25519 public key blob is truncated")
    return int.from_bytes(data[offset : offset + 4], "big"), offset + 4


def _read_ssh_string(data: bytes, offset: int) -> tuple[bytes, int]:
    length, offset = _read_u32(data, offset)
    if offset + length > len(data):
        raise EvidenceError("OpenSSH Ed25519 public key blob is truncated")
    return data[offset : offset + length], offset + length


def require_ed25519_public_key(value: object) -> str:
    if not isinstance(value, str):
        raise EvidenceError("public_key must be a canonical OpenSSH key")
    require_bounded_text(value, "public_key", 256)
    parts = value.split(" ")
    if len(parts) != 2 or parts[0] != "ssh-ed25519" or not parts[1]:
        raise EvidenceError("public_key must contain only an ssh-ed25519 key and blob")
    try:
        blob = base64.b64decode(parts[1], validate=True)
    except (binascii.Error, ValueError) as error:
        raise EvidenceError("public_key contains malformed base64") from error
    if base64.b64encode(blob).decode("ascii") != parts[1]:
        raise EvidenceError("public_key base64 is not canonical")
    algorithm, offset = _read_ssh_string(blob, 0)
    key, offset = _read_ssh_string(blob, offset)
    if algorithm != b"ssh-ed25519" or len(key) != 32 or offset != len(blob):
        raise EvidenceError("public_key is not a canonical Ed25519 OpenSSH blob")
    return value


@dataclass(frozen=True)
class SubjectIdentity:
    name: str
    identity: str

    @classmethod
    def from_object(cls, value: object) -> SubjectIdentity:
        if not isinstance(value, dict):
            raise EvidenceError("subject entry must be an object")
        _require_exact_fields(value, _SUBJECT_FIELDS, "subject entry")
        name = value["name"]
        if not isinstance(name, str) or _SUBJECT_NAME_RE.fullmatch(name) is None:
            raise EvidenceError("subject name is malformed")
        return cls(name, require_generic_typed_identity(value["identity"], name))

    def as_object(self) -> dict[str, object]:
        return {"identity": self.identity, "name": self.name}


@dataclass(frozen=True)
class AttestationPayloadV1:
    role: str
    signer_identity: str
    source_commit: str
    target: str
    issued_at: int
    expires_at: int
    build_identity: str
    subjects: tuple[SubjectIdentity, ...]

    @classmethod
    def from_bytes(cls, data: bytes) -> AttestationPayloadV1:
        value = parse_canonical_json(data, "attestation payload")
        _require_exact_fields(value, _ATTESTATION_FIELDS, "attestation payload")
        if value["domain"] != ATTESTATION_DOMAIN:
            raise EvidenceError("attestation domain is wrong")
        if (
            isinstance(value["schema_version"], bool)
            or value["schema_version"] != SCHEMA_VERSION
        ):
            raise EvidenceError("attestation schema_version must be exactly 1")
        role = require_role(value["role"])
        raw_subjects = value["subjects"]
        if (
            not isinstance(raw_subjects, list)
            or not 1 <= len(raw_subjects) <= MAX_SUBJECTS
        ):
            raise EvidenceError("subjects must be a nonempty bounded array")
        subjects = tuple(SubjectIdentity.from_object(item) for item in raw_subjects)
        names = tuple(subject.name for subject in subjects)
        if names != tuple(sorted(names)) or len(names) != len(set(names)):
            raise EvidenceError("subjects must have unique names in canonical order")
        if names != ROLE_SUBJECTS[role]:
            raise EvidenceError(f"subjects do not exactly match the {role} role schema")
        issued_at = require_unix_timestamp(value["issued_at"], "issued_at")
        expires_at = require_unix_timestamp(value["expires_at"], "expires_at")
        if expires_at <= issued_at:
            raise EvidenceError("expires_at must be later than issued_at")
        if expires_at - issued_at > MAX_ATTESTATION_LIFETIME_SECONDS:
            raise EvidenceError("attestation lifetime exceeds seven days")
        result = cls(
            role=role,
            signer_identity=require_signer_identity(value["signer_identity"]),
            source_commit=require_commit(
                require_string(value["source_commit"], "source_commit")
            ),
            target=require_target(require_string(value["target"], "target")),
            issued_at=issued_at,
            expires_at=expires_at,
            build_identity=require_generic_typed_identity(
                value["build_identity"], "build_identity"
            ),
            subjects=subjects,
        )
        if result.canonical_bytes() != data:
            raise EvidenceError(
                "attestation payload does not match its canonical model"
            )
        return result

    def as_object(self) -> dict[str, object]:
        return {
            "build_identity": self.build_identity,
            "domain": ATTESTATION_DOMAIN,
            "expires_at": self.expires_at,
            "issued_at": self.issued_at,
            "role": self.role,
            "schema_version": SCHEMA_VERSION,
            "signer_identity": self.signer_identity,
            "source_commit": self.source_commit,
            "subjects": [subject.as_object() for subject in self.subjects],
            "target": self.target,
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.as_object())

    def identity(self) -> str:
        return typed_identity(ATTESTATION_DOMAIN, self.canonical_bytes())


@dataclass(frozen=True)
class SignerBindingV1:
    role: str
    signer_identity: str
    public_key: str

    @classmethod
    def from_object(cls, value: object) -> SignerBindingV1:
        if not isinstance(value, dict):
            raise EvidenceError("signer binding must be an object")
        _require_exact_fields(value, _SIGNER_FIELDS, "signer binding")
        return cls(
            require_role(value["role"]),
            require_signer_identity(value["signer_identity"]),
            require_ed25519_public_key(value["public_key"]),
        )

    def as_object(self) -> dict[str, object]:
        return {
            "public_key": self.public_key,
            "role": self.role,
            "signer_identity": self.signer_identity,
        }


@dataclass(frozen=True)
class TrustPolicyV1:
    verifier_identity: str
    signers: tuple[SignerBindingV1, ...]

    @classmethod
    def from_bytes(cls, data: bytes) -> TrustPolicyV1:
        value = parse_canonical_json(data, "trust policy")
        _require_exact_fields(value, _POLICY_FIELDS, "trust policy")
        if value["domain"] != POLICY_DOMAIN:
            raise EvidenceError("trust policy domain is wrong")
        if (
            isinstance(value["schema_version"], bool)
            or value["schema_version"] != SCHEMA_VERSION
        ):
            raise EvidenceError("trust policy schema_version must be exactly 1")
        if value["verifier_path"] != str(SSH_KEYGEN_PATH):
            raise EvidenceError("trust policy must pin /usr/bin/ssh-keygen")
        verifier_identity = require_string(
            value["verifier_identity"], "verifier_identity"
        )
        require_typed_identity(verifier_identity, VERIFIER_DOMAIN, "verifier_identity")
        raw_signers = value["signers"]
        if (
            not isinstance(raw_signers, list)
            or not 1 <= len(raw_signers) <= MAX_SIGNERS
        ):
            raise EvidenceError("signers must be a nonempty bounded array")
        signers = tuple(SignerBindingV1.from_object(item) for item in raw_signers)
        keys = tuple((item.role, item.signer_identity) for item in signers)
        if keys != tuple(sorted(keys)) or len(keys) != len(set(keys)):
            raise EvidenceError("signer bindings must be unique and in canonical order")
        result = cls(verifier_identity, signers)
        if result.canonical_bytes() != data:
            raise EvidenceError("trust policy does not match its canonical model")
        return result

    def as_object(self) -> dict[str, object]:
        return {
            "domain": POLICY_DOMAIN,
            "schema_version": SCHEMA_VERSION,
            "signers": [signer.as_object() for signer in self.signers],
            "verifier_identity": self.verifier_identity,
            "verifier_path": str(SSH_KEYGEN_PATH),
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.as_object())

    def identity(self) -> str:
        return typed_identity(POLICY_DOMAIN, self.canonical_bytes())

    def binding(self, role: str, signer_identity: str) -> SignerBindingV1:
        matches = tuple(
            binding
            for binding in self.signers
            if binding.role == role and binding.signer_identity == signer_identity
        )
        if len(matches) != 1:
            raise EvidenceError("trust policy has no exact role/signer binding")
        return matches[0]


@dataclass(frozen=True)
class AuthenticatedObservationV1:
    """A signature result with no publication, load, or launch authority."""

    attestation_identity: str
    policy_identity: str
    verifier_identity: str
    payload: AttestationPayloadV1


@dataclass
class _PinnedVerifier:
    descriptor: int
    identity: str

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


def _write_all(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise EvidenceError("short write while pinning verifier input")
        view = view[written:]


def _pin_verifier(expected_identity: str) -> _PinnedVerifier:
    if not hasattr(os, "memfd_create") or not hasattr(os, "MFD_ALLOW_SEALING"):
        raise EvidenceError("sealed verifier pinning is unavailable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        source = os.open(SSH_KEYGEN_PATH, flags)
    except OSError as error:
        raise EvidenceError(
            f"cannot open pinned ssh-keygen: {error.strerror}"
        ) from error
    pinned = -1
    try:
        before = os.fstat(source)
        if (
            not stat.S_ISREG(before.st_mode)
            or not 0 < before.st_size <= MAX_VERIFIER_BYTES
        ):
            raise EvidenceError("ssh-keygen must be a nonempty bounded regular file")
        if before.st_uid != 0 or before.st_mode & 0o022:
            raise EvidenceError(
                "ssh-keygen must be root-owned and not group/other writable"
            )
        if before.st_mode & 0o111 == 0:
            raise EvidenceError("ssh-keygen is not executable")
        pinned = os.memfd_create(
            "fe2o3-ssh-keygen",
            os.MFD_ALLOW_SEALING | getattr(os, "MFD_CLOEXEC", 0),
        )
        remaining = before.st_size
        while remaining:
            chunk = os.read(source, min(1024 * 1024, remaining))
            if not chunk:
                raise EvidenceError("ssh-keygen was truncated while being pinned")
            _write_all(pinned, chunk)
            remaining -= len(chunk)
        after = os.fstat(source)
        if metadata_snapshot(before) != metadata_snapshot(after):
            raise EvidenceError("ssh-keygen changed while being pinned")
        os.fchmod(pinned, 0o500)
        seals = (
            fcntl.F_SEAL_WRITE
            | fcntl.F_SEAL_GROW
            | fcntl.F_SEAL_SHRINK
            | fcntl.F_SEAL_SEAL
        )
        fcntl.fcntl(pinned, fcntl.F_ADD_SEALS, seals)
        if fcntl.fcntl(pinned, fcntl.F_GET_SEALS) & seals != seals:
            raise EvidenceError("ssh-keygen memfd was not fully sealed")
        identity = typed_descriptor_identity(
            VERIFIER_DOMAIN, pinned, "sealed ssh-keygen"
        )
        if identity != expected_identity:
            raise EvidenceError("ssh-keygen identity does not match the pinned policy")
        if not Path(f"/proc/self/fd/{pinned}").exists():
            raise EvidenceError("pinned ssh-keygen is not executable through procfs")
        result = _PinnedVerifier(pinned, identity)
        pinned = -1
        return result
    except OSError as error:
        raise EvidenceError(f"cannot pin ssh-keygen: {error.strerror}") from error
    finally:
        os.close(source)
        if pinned >= 0:
            os.close(pinned)


def measure_verifier_identity() -> str:
    descriptor = os.open(
        SSH_KEYGEN_PATH,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or not 0 < metadata.st_size <= MAX_VERIFIER_BYTES
            or metadata.st_uid != 0
            or metadata.st_mode & 0o022
            or metadata.st_mode & 0o111 == 0
        ):
            raise EvidenceError("ssh-keygen does not satisfy the executable policy")
        return typed_descriptor_identity(
            VERIFIER_DOMAIN, descriptor, str(SSH_KEYGEN_PATH)
        )
    finally:
        os.close(descriptor)


def _signature_bytes(path: Path) -> bytes:
    data = read_regular_file(path, MAX_SIGNATURE_BYTES)
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError("OpenSSH signature must contain ASCII only") from error
    if (
        not text.endswith("\n")
        or "\r" in text
        or "\0" in text
        or not text.startswith("-----BEGIN SSH SIGNATURE-----\n")
        or not text.endswith("-----END SSH SIGNATURE-----\n")
    ):
        raise EvidenceError("OpenSSH signature envelope is malformed")
    return data


def _terminate_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def _invoke_verifier(
    verifier: _PinnedVerifier,
    binding: SignerBindingV1,
    payload: bytes,
    signature: bytes,
    timeout: int,
) -> None:
    with tempfile.TemporaryDirectory(prefix="fe2o3-attestation-") as directory_name:
        directory = Path(directory_name)
        allowed_signers = directory / "allowed_signers"
        signature_path = directory / "attestation.sig"
        for path, data in (
            (
                allowed_signers,
                f"{binding.signer_identity} {binding.public_key}\n".encode("ascii"),
            ),
            (signature_path, signature),
        ):
            descriptor = os.open(
                path,
                os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0),
                0o600,
            )
            try:
                _write_all(descriptor, data)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)

        environment = {
            "HOME": "/nonexistent-fe2o3-home",
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "TZ": "UTC",
        }
        process: subprocess.Popen[bytes] | None = None
        try:
            try:
                process = subprocess.Popen(
                    [
                        str(SSH_KEYGEN_PATH),
                        "-Y",
                        "verify",
                        "-f",
                        str(allowed_signers),
                        "-I",
                        binding.signer_identity,
                        "-n",
                        SIGNATURE_NAMESPACE,
                        "-s",
                        str(signature_path),
                    ],
                    cwd=directory,
                    env=environment,
                    executable=f"/proc/self/fd/{verifier.descriptor}",
                    pass_fds=(verifier.descriptor,),
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
            except (FileNotFoundError, PermissionError, OSError) as error:
                raise EvidenceError("OpenSSH verifier could not be executed") from error
            try:
                output, _ = process.communicate(payload, timeout=timeout)
            except subprocess.TimeoutExpired as error:
                _terminate_process_group(process)
                process.communicate()
                raise EvidenceError("OpenSSH verifier timed out") from error
            if len(output) > MAX_VERIFIER_OUTPUT_BYTES:
                raise EvidenceError("OpenSSH verifier output exceeded its bound")
            if process.returncode != 0:
                raise EvidenceError("OpenSSH signature verification failed")
        finally:
            if process is not None and process.poll() is None:
                _terminate_process_group(process)
                try:
                    process.wait(timeout=1)
                except subprocess.TimeoutExpired as error:
                    raise EvidenceError(
                        "OpenSSH verifier could not be terminated"
                    ) from error


def verify_signed_attestation(
    *,
    policy_path: Path,
    expected_policy_identity: str,
    payload_path: Path,
    signature_path: Path,
    expected_role: str,
    expected_signer_identity: str,
    expected_source_commit: str,
    expected_target: str,
    expected_build_identity: str,
    expected_subjects: Mapping[str, str],
    now: int | None = None,
    timeout: int = 10,
) -> AuthenticatedObservationV1:
    require_typed_identity(
        expected_policy_identity, POLICY_DOMAIN, "expected_policy_identity"
    )
    role = require_role(expected_role)
    signer_identity = require_signer_identity(expected_signer_identity)
    source_commit = require_commit(expected_source_commit)
    target = require_target(expected_target)
    build_identity = require_generic_typed_identity(
        expected_build_identity, "expected_build_identity"
    )
    if isinstance(timeout, bool) or not 1 <= timeout <= MAX_VERIFIER_TIMEOUT_SECONDS:
        raise EvidenceError("verifier timeout must be between 1 and 30 seconds")
    current_time = (
        int(time.time()) if now is None else require_unix_timestamp(now, "now")
    )

    policy = TrustPolicyV1.from_bytes(read_regular_file(policy_path, MAX_POLICY_BYTES))
    if policy.identity() != expected_policy_identity:
        raise EvidenceError("trust policy identity does not match the out-of-band pin")
    payload_bytes = read_regular_file(payload_path, MAX_ATTESTATION_BYTES)
    payload = AttestationPayloadV1.from_bytes(payload_bytes)
    if payload.role != role:
        raise EvidenceError("attestation role does not match the expected role")
    if payload.signer_identity != signer_identity:
        raise EvidenceError("attestation signer does not match the expected signer")
    if payload.source_commit != source_commit:
        raise EvidenceError("attestation source commit does not match")
    if payload.target != target:
        raise EvidenceError("attestation target does not match")
    if payload.build_identity != build_identity:
        raise EvidenceError(
            "attestation build identity does not match the replay bound"
        )
    if current_time + MAX_CLOCK_SKEW_SECONDS < payload.issued_at:
        raise EvidenceError("attestation was issued too far in the future")
    if current_time >= payload.expires_at:
        raise EvidenceError("attestation has expired")

    expected_names = tuple(sorted(expected_subjects))
    if expected_names != ROLE_SUBJECTS[role]:
        raise EvidenceError("expected subjects do not exactly match the role schema")
    canonical_expected = tuple(
        SubjectIdentity(
            name, require_generic_typed_identity(expected_subjects[name], name)
        )
        for name in expected_names
    )
    if payload.subjects != canonical_expected:
        raise EvidenceError("attestation subjects do not match the expected identities")

    binding = policy.binding(role, signer_identity)
    signature = _signature_bytes(signature_path)
    verifier = _pin_verifier(policy.verifier_identity)
    try:
        _invoke_verifier(verifier, binding, payload_bytes, signature, timeout)
    finally:
        verifier.close()
    return AuthenticatedObservationV1(
        attestation_identity=payload.identity(),
        policy_identity=policy.identity(),
        verifier_identity=policy.verifier_identity,
        payload=payload,
    )


def _subject_argument(value: str) -> tuple[str, str]:
    if "=" not in value:
        raise argparse.ArgumentTypeError("subject must use name=typed-identity")
    name, identity = value.split("=", 1)
    if _SUBJECT_NAME_RE.fullmatch(name) is None:
        raise argparse.ArgumentTypeError("subject name is malformed")
    try:
        require_generic_typed_identity(identity, name)
    except EvidenceError as error:
        raise argparse.ArgumentTypeError(str(error)) from error
    return name, identity


def _timeout_argument(value: str) -> int:
    try:
        result = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("timeout must be an integer") from error
    if not 1 <= result <= MAX_VERIFIER_TIMEOUT_SECONDS:
        raise argparse.ArgumentTypeError("timeout must be between 1 and 30 seconds")
    return result


def _verify_command(args: argparse.Namespace) -> int:
    subjects: dict[str, str] = {}
    for name, identity in args.expect_subject:
        if name in subjects:
            raise EvidenceError(f"duplicate expected subject: {name}")
        subjects[name] = identity
    observation = verify_signed_attestation(
        policy_path=args.policy,
        expected_policy_identity=args.expect_policy_identity,
        payload_path=args.payload,
        signature_path=args.signature,
        expected_role=args.expect_role,
        expected_signer_identity=args.expect_signer,
        expected_source_commit=args.expect_source_commit,
        expected_target=args.expect_target,
        expected_build_identity=args.expect_build_identity,
        expected_subjects=subjects,
        now=args.now,
        timeout=args.timeout,
    )
    print(
        "authenticated direct-link observation: "
        f"{observation.attestation_identity} role={observation.payload.role}"
    )
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    measure = subparsers.add_parser(
        "measure-verifier", help="measure the strict system ssh-keygen executable"
    )
    measure.set_defaults(handler=lambda _args: print(measure_verifier_identity()) or 0)

    policy_identity = subparsers.add_parser(
        "policy-identity", help="validate and identify a canonical trust policy"
    )
    policy_identity.add_argument("policy", type=Path)

    def identify_policy(args: argparse.Namespace) -> int:
        policy = TrustPolicyV1.from_bytes(
            read_regular_file(args.policy, MAX_POLICY_BYTES)
        )
        print(policy.identity())
        return 0

    policy_identity.set_defaults(handler=identify_policy)

    verify = subparsers.add_parser(
        "verify", help="verify a signed observation against explicit expected bindings"
    )
    verify.add_argument("--policy", required=True, type=Path)
    verify.add_argument("--expect-policy-identity", required=True)
    verify.add_argument("--payload", required=True, type=Path)
    verify.add_argument("--signature", required=True, type=Path)
    verify.add_argument("--expect-role", required=True, choices=ROLES)
    verify.add_argument("--expect-signer", required=True)
    verify.add_argument("--expect-source-commit", required=True)
    verify.add_argument("--expect-target", required=True)
    verify.add_argument("--expect-build-identity", required=True)
    verify.add_argument(
        "--expect-subject", required=True, action="append", type=_subject_argument
    )
    verify.add_argument("--now", type=int)
    verify.add_argument("--timeout", type=_timeout_argument, default=10)
    verify.set_defaults(handler=_verify_command)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        return args.handler(args)
    except EvidenceError as error:
        print(f"direct-link attestation: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
