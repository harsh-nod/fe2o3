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
import selectors
import signal
import stat
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Mapping

from common import (
    EvidenceError,
    metadata_snapshot,
    require_bounded_text,
    require_commit,
    require_target,
    require_typed_identity,
    typed_descriptor_identity,
    typed_identity,
)

SCHEMA_VERSION = 1
ATTESTATION_DOMAIN = "fe2o3-direct-link-attestation-v1"
POLICY_DOMAIN = "fe2o3-direct-link-trust-policy-v1"
VERIFIER_DOMAIN = "fe2o3-ssh-keygen-executable-v1"
PUBLIC_KEY_DOMAIN = "fe2o3-attestation-public-key-v1"
BUILD_CONTEXT_DOMAIN = "fe2o3-direct-link-build-context-v1"
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
MAX_POLICY_EPOCH = (1 << 63) - 1
PROCESS_DRAIN_GRACE_SECONDS = 0.25

ROLES = (
    "g2-worker",
    "g5-publication",
    "g6-bundle",
    "g7-hardware-runner",
    "g7-static-runner",
)
SUBJECT_IDENTITY_DOMAINS = {
    "g2-worker": {
        "llvm_toolchain": "fe2o3-llvm-toolchain-v1",
        "request": "fe2o3-link-request-v1",
        "worker": "fe2o3-worker-v1",
        "worker_executable": "fe2o3-worker-executable-v1",
    },
    "g5-publication": {
        "linked_artifact": "fe2o3-linked-artifact-v1",
        "publication": "fe2o3-link-publication-v1",
        "request": "fe2o3-link-request-v1",
    },
    "g6-bundle": {
        "bundle": "fe2o3-bundle-index-v1",
        "ffi_closure": "fe2o3-ffi-closure-v1",
        "final_artifact": "fe2o3-final-artifact-v1",
        "publication": "fe2o3-link-publication-v1",
    },
    "g7-hardware-runner": {
        "argv": "fe2o3-hardware-argv-v1",
        "bundle": "fe2o3-bundle-index-v1",
        "driver": "fe2o3-gpu-driver-v1",
        "final_artifact": "fe2o3-final-artifact-v1",
        "hardware_run": "fe2o3-hardware-run-v1",
        "observed_gpu": "fe2o3-observed-gpu-v1",
        "oracle": "fe2o3-hardware-oracle-v1",
        "test_executable": "fe2o3-test-executable-v1",
    },
    "g7-static-runner": {
        "argv": "fe2o3-static-argv-v1",
        "bundle": "fe2o3-bundle-index-v1",
        "final_artifact": "fe2o3-final-artifact-v1",
        "ruleset": "fe2o3-static-ruleset-v1",
        "runner_executable": "fe2o3-static-runner-executable-v1",
        "static_run": "fe2o3-static-run-v1",
    },
}
ROLE_SUBJECTS = {
    role: tuple(sorted(domains)) for role, domains in SUBJECT_IDENTITY_DOMAINS.items()
}

_SIGNER_RE = re.compile(r"[a-z0-9][a-z0-9._@-]{0,63}\Z")
_SUBJECT_NAME_RE = re.compile(r"[a-z][a-z0-9_]{0,31}\Z")
_CAMPAIGN_NONCE_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
_TYPED_IDENTITY_RE = re.compile(
    r"(?P<domain>[a-z0-9][a-z0-9-]{0,63}-v[1-9][0-9]{0,5})-sha256-"
    r"(?P<digest>[0-9a-f]{64})\Z"
)
_POLICY_FIELDS = frozenset(
    (
        "domain",
        "policy_epoch",
        "schema_version",
        "signers",
        "verifier_identity",
        "verifier_path",
    )
)
_SIGNER_FIELDS = frozenset(
    (
        "key_identity",
        "public_key",
        "role",
        "signer_identity",
        "valid_from",
        "valid_until",
    )
)
_ATTESTATION_FIELDS = frozenset(
    (
        "build_context_identity",
        "campaign_nonce",
        "domain",
        "expires_at",
        "issued_at",
        "policy_epoch",
        "policy_identity",
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
        raise EvidenceError(
            f"{name} has wrong fields; missing={len(fields - actual)}, "
            f"extra={len(actual - fields)}"
        )


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError("JSON contains a duplicate object key")
        result[key] = value
    return result


def canonical_json_bytes(value: object) -> bytes:
    try:
        return (
            json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)
            + "\n"
        ).encode("ascii")
    except (
        OverflowError,
        RecursionError,
        TypeError,
        UnicodeEncodeError,
        ValueError,
    ) as error:
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
    except (json.JSONDecodeError, RecursionError, ValueError) as error:
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


def require_policy_epoch(value: object, name: str = "policy_epoch") -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceError(f"{name} must be an integer")
    if not 1 <= value <= MAX_POLICY_EPOCH:
        raise EvidenceError(f"{name} is outside the supported range")
    return value


def require_campaign_nonce(value: object) -> str:
    if not isinstance(value, str) or _CAMPAIGN_NONCE_RE.fullmatch(value) is None:
        raise EvidenceError("campaign_nonce is malformed or exceeds 64 bytes")
    return value


def require_generic_typed_identity(value: object, name: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{name} must be a typed identity")
    match = _TYPED_IDENTITY_RE.fullmatch(value)
    if match is None:
        raise EvidenceError(f"{name} is not a canonical typed SHA-256 identity")
    domain = match.group("domain")
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
    def from_object(cls, value: object, expected_domain: str) -> SubjectIdentity:
        if not isinstance(value, dict):
            raise EvidenceError("subject entry must be an object")
        _require_exact_fields(value, _SUBJECT_FIELDS, "subject entry")
        name = value["name"]
        if not isinstance(name, str) or _SUBJECT_NAME_RE.fullmatch(name) is None:
            raise EvidenceError("subject name is malformed")
        identity = require_string(value["identity"], name)
        require_typed_identity(identity, expected_domain, name)
        return cls(name, identity)

    def as_object(self) -> dict[str, object]:
        return {"identity": self.identity, "name": self.name}


def validate_subjects(
    role: str, subjects: tuple[SubjectIdentity, ...]
) -> tuple[SubjectIdentity, ...]:
    if not 1 <= len(subjects) <= MAX_SUBJECTS:
        raise EvidenceError("subjects must be a nonempty bounded array")
    names = tuple(subject.name for subject in subjects)
    if names != tuple(sorted(names)) or len(names) != len(set(names)):
        raise EvidenceError("subjects must have unique names in canonical order")
    if names != ROLE_SUBJECTS[role]:
        raise EvidenceError(f"subjects do not exactly match the {role} role schema")
    for subject in subjects:
        require_typed_identity(
            subject.identity,
            SUBJECT_IDENTITY_DOMAINS[role][subject.name],
            subject.name,
        )
    return subjects


def derive_build_context_identity(
    *,
    source_commit: str,
    target: str,
    role: str,
    policy_identity: str,
    policy_epoch: int,
    campaign_nonce: str,
    subjects: tuple[SubjectIdentity, ...],
) -> str:
    role = require_role(role)
    source_commit = require_commit(source_commit)
    target = require_target(target)
    require_typed_identity(policy_identity, POLICY_DOMAIN, "policy_identity")
    policy_epoch = require_policy_epoch(policy_epoch)
    campaign_nonce = require_campaign_nonce(campaign_nonce)
    subjects = validate_subjects(role, subjects)
    preimage = canonical_json_bytes(
        {
            "campaign_nonce": campaign_nonce,
            "domain": BUILD_CONTEXT_DOMAIN,
            "policy_epoch": policy_epoch,
            "policy_identity": policy_identity,
            "role": role,
            "schema_version": SCHEMA_VERSION,
            "source_commit": source_commit,
            "subjects": [subject.as_object() for subject in subjects],
            "target": target,
        }
    )
    return typed_identity(BUILD_CONTEXT_DOMAIN, preimage)


@dataclass(frozen=True)
class AttestationPayloadV1:
    role: str
    signer_identity: str
    source_commit: str
    target: str
    issued_at: int
    expires_at: int
    policy_identity: str
    policy_epoch: int
    campaign_nonce: str
    build_context_identity: str
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
        if len(raw_subjects) != len(ROLE_SUBJECTS[role]):
            raise EvidenceError(f"subjects do not exactly match the {role} role schema")
        subjects = tuple(
            SubjectIdentity.from_object(
                item, SUBJECT_IDENTITY_DOMAINS[role][expected_name]
            )
            for item, expected_name in zip(
                raw_subjects, ROLE_SUBJECTS[role], strict=True
            )
        )
        validate_subjects(role, subjects)
        issued_at = require_unix_timestamp(value["issued_at"], "issued_at")
        expires_at = require_unix_timestamp(value["expires_at"], "expires_at")
        if expires_at <= issued_at:
            raise EvidenceError("expires_at must be later than issued_at")
        if expires_at - issued_at > MAX_ATTESTATION_LIFETIME_SECONDS:
            raise EvidenceError("attestation lifetime exceeds seven days")
        policy_identity = require_string(value["policy_identity"], "policy_identity")
        require_typed_identity(policy_identity, POLICY_DOMAIN, "policy_identity")
        policy_epoch = require_policy_epoch(value["policy_epoch"])
        campaign_nonce = require_campaign_nonce(value["campaign_nonce"])
        build_context_identity = require_string(
            value["build_context_identity"], "build_context_identity"
        )
        require_typed_identity(
            build_context_identity,
            BUILD_CONTEXT_DOMAIN,
            "build_context_identity",
        )
        source_commit = require_commit(
            require_string(value["source_commit"], "source_commit")
        )
        target = require_target(require_string(value["target"], "target"))
        expected_context = derive_build_context_identity(
            source_commit=source_commit,
            target=target,
            role=role,
            policy_identity=policy_identity,
            policy_epoch=policy_epoch,
            campaign_nonce=campaign_nonce,
            subjects=subjects,
        )
        if build_context_identity != expected_context:
            raise EvidenceError(
                "build_context_identity does not match its canonical inputs"
            )
        result = cls(
            role=role,
            signer_identity=require_signer_identity(value["signer_identity"]),
            source_commit=source_commit,
            target=target,
            issued_at=issued_at,
            expires_at=expires_at,
            policy_identity=policy_identity,
            policy_epoch=policy_epoch,
            campaign_nonce=campaign_nonce,
            build_context_identity=build_context_identity,
            subjects=subjects,
        )
        if result.canonical_bytes() != data:
            raise EvidenceError(
                "attestation payload does not match its canonical model"
            )
        return result

    def as_object(self) -> dict[str, object]:
        return {
            "build_context_identity": self.build_context_identity,
            "campaign_nonce": self.campaign_nonce,
            "domain": ATTESTATION_DOMAIN,
            "expires_at": self.expires_at,
            "issued_at": self.issued_at,
            "policy_epoch": self.policy_epoch,
            "policy_identity": self.policy_identity,
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
    key_identity: str
    valid_from: int
    valid_until: int

    @classmethod
    def from_object(cls, value: object) -> SignerBindingV1:
        if not isinstance(value, dict):
            raise EvidenceError("signer binding must be an object")
        _require_exact_fields(value, _SIGNER_FIELDS, "signer binding")
        role = require_role(value["role"])
        signer_identity = require_signer_identity(value["signer_identity"])
        public_key = require_ed25519_public_key(value["public_key"])
        key_identity = require_string(value["key_identity"], "key_identity")
        require_typed_identity(key_identity, PUBLIC_KEY_DOMAIN, "key_identity")
        if key_identity != public_key_identity(public_key):
            raise EvidenceError("key_identity does not match the public key")
        valid_from = require_unix_timestamp(value["valid_from"], "valid_from")
        valid_until = require_unix_timestamp(value["valid_until"], "valid_until")
        if valid_until <= valid_from:
            raise EvidenceError("signer key validity interval is empty")
        return cls(
            role,
            signer_identity,
            public_key,
            key_identity,
            valid_from,
            valid_until,
        )

    def as_object(self) -> dict[str, object]:
        return {
            "key_identity": self.key_identity,
            "public_key": self.public_key,
            "role": self.role,
            "signer_identity": self.signer_identity,
            "valid_from": self.valid_from,
            "valid_until": self.valid_until,
        }


def public_key_identity(public_key: str) -> str:
    public_key = require_ed25519_public_key(public_key)
    return typed_identity(PUBLIC_KEY_DOMAIN, f"{public_key}\n".encode("ascii"))


@dataclass(frozen=True)
class TrustPolicyV1:
    verifier_identity: str
    policy_epoch: int
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
        policy_epoch = require_policy_epoch(value["policy_epoch"])
        raw_signers = value["signers"]
        if (
            not isinstance(raw_signers, list)
            or not 1 <= len(raw_signers) <= MAX_SIGNERS
        ):
            raise EvidenceError("signers must be a nonempty bounded array")
        signers = tuple(SignerBindingV1.from_object(item) for item in raw_signers)
        keys = tuple(
            (item.role, item.signer_identity, item.valid_from, item.key_identity)
            for item in signers
        )
        if keys != tuple(sorted(keys)) or len(keys) != len(set(keys)):
            raise EvidenceError("signer bindings must be unique and in canonical order")
        previous: dict[tuple[str, str], SignerBindingV1] = {}
        for signer in signers:
            signer_key = (signer.role, signer.signer_identity)
            prior = previous.get(signer_key)
            if prior is not None and signer.valid_from < prior.valid_until:
                raise EvidenceError("signer key validity intervals overlap")
            previous[signer_key] = signer
        result = cls(verifier_identity, policy_epoch, signers)
        if result.canonical_bytes() != data:
            raise EvidenceError("trust policy does not match its canonical model")
        return result

    def as_object(self) -> dict[str, object]:
        return {
            "domain": POLICY_DOMAIN,
            "policy_epoch": self.policy_epoch,
            "schema_version": SCHEMA_VERSION,
            "signers": [signer.as_object() for signer in self.signers],
            "verifier_identity": self.verifier_identity,
            "verifier_path": str(SSH_KEYGEN_PATH),
        }

    def canonical_bytes(self) -> bytes:
        return canonical_json_bytes(self.as_object())

    def identity(self) -> str:
        return typed_identity(POLICY_DOMAIN, self.canonical_bytes())

    def binding(
        self, role: str, signer_identity: str, issued_at: int
    ) -> SignerBindingV1:
        matches = tuple(
            binding
            for binding in self.signers
            if binding.role == role
            and binding.signer_identity == signer_identity
            and binding.valid_from <= issued_at < binding.valid_until
        )
        if len(matches) != 1:
            raise EvidenceError(
                "trust policy has no exact role/signer/key validity binding"
            )
        return matches[0]


@dataclass(frozen=True)
class VerifiedAttestationObservationV1:
    """Forgeable descriptive data returned after verification, never authority."""

    attestation_identity: str
    policy_identity: str
    verifier_identity: str
    key_identity: str
    payload: AttestationPayloadV1


@dataclass
class _PinnedVerifier:
    descriptor: int
    identity: str

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


@dataclass
class _SealedInput:
    descriptor: int

    def close(self) -> None:
        if self.descriptor >= 0:
            os.close(self.descriptor)
            self.descriptor = -1


@dataclass(frozen=True)
class _ProcessResult:
    returncode: int | None
    output: bytes
    timed_out: bool = False
    overflow: bool = False
    unavailable: bool = False


def _write_all(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        written = os.write(descriptor, view)
        if written <= 0:
            raise EvidenceError("short write while pinning verifier input")
        view = view[written:]


def _read_bounded_regular(path: Path, maximum: int, name: str) -> bytes:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise EvidenceError(f"cannot open {name} as a regular file") from error
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise EvidenceError(f"{name} is not a regular file")
        if not 0 < before.st_size <= maximum:
            raise EvidenceError(f"{name} is empty or exceeds the {maximum}-byte bound")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            try:
                chunk = os.read(descriptor, min(64 * 1024, remaining))
            except BlockingIOError as error:
                raise EvidenceError(f"{name} would block while being read") from error
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        after = os.fstat(descriptor)
        if len(data) != before.st_size or metadata_snapshot(
            before
        ) != metadata_snapshot(after):
            raise EvidenceError(f"{name} changed while being read")
        if len(data) > maximum:
            raise EvidenceError(f"{name} exceeds the {maximum}-byte bound")
        return data
    except OSError as error:
        raise EvidenceError(f"cannot read {name} as a regular file") from error
    finally:
        os.close(descriptor)


def _seal_bytes(name: str, data: bytes, mode: int = 0o400) -> _SealedInput:
    if not hasattr(os, "memfd_create") or not hasattr(os, "MFD_ALLOW_SEALING"):
        raise EvidenceError("sealed verifier inputs are unavailable")
    descriptor = -1
    try:
        descriptor = os.memfd_create(
            name, os.MFD_ALLOW_SEALING | getattr(os, "MFD_CLOEXEC", 0)
        )
        _write_all(descriptor, data)
        os.fchmod(descriptor, mode)
        seals = (
            fcntl.F_SEAL_WRITE
            | fcntl.F_SEAL_GROW
            | fcntl.F_SEAL_SHRINK
            | fcntl.F_SEAL_SEAL
        )
        fcntl.fcntl(descriptor, fcntl.F_ADD_SEALS, seals)
        if fcntl.fcntl(descriptor, fcntl.F_GET_SEALS) & seals != seals:
            raise EvidenceError("verifier input memfd was not fully sealed")
        os.lseek(descriptor, 0, os.SEEK_SET)
        result = _SealedInput(descriptor)
        descriptor = -1
        return result
    except OSError as error:
        raise EvidenceError("cannot create sealed verifier input") from error
    finally:
        if descriptor >= 0:
            os.close(descriptor)


def _pin_verifier(expected_identity: str) -> _PinnedVerifier:
    if not hasattr(os, "memfd_create") or not hasattr(os, "MFD_ALLOW_SEALING"):
        raise EvidenceError("sealed verifier pinning is unavailable")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
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
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NONBLOCK", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(SSH_KEYGEN_PATH, flags)
    except OSError as error:
        raise EvidenceError("cannot open the strict ssh-keygen path") from error
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
    data = _read_bounded_regular(path, MAX_SIGNATURE_BYTES, "OpenSSH signature")
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


def _run_bounded_process(
    *,
    executable_descriptor: int,
    argv: list[str],
    pass_descriptors: tuple[int, ...],
    stdin_descriptor: int,
    environment: dict[str, str],
    timeout: int,
    output_limit: int,
) -> _ProcessResult:
    process: subprocess.Popen[bytes] | None = None
    selector = selectors.DefaultSelector()
    output = bytearray()
    timed_out = False
    overflow = False
    cleanup_started: float | None = None
    try:
        try:
            process = subprocess.Popen(
                argv,
                cwd=Path("/"),
                env=environment,
                executable=f"/proc/self/fd/{executable_descriptor}",
                pass_fds=pass_descriptors,
                stdin=stdin_descriptor,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                start_new_session=True,
            )
        except (FileNotFoundError, PermissionError, OSError):
            return _ProcessResult(None, b"", unavailable=True)
        assert process.stdout is not None
        assert process.stderr is not None
        for stream in (process.stdout, process.stderr):
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ)
        deadline = time.monotonic() + timeout

        while process.poll() is None or selector.get_map():
            now = time.monotonic()
            if process.poll() is None and cleanup_started is None and now >= deadline:
                timed_out = True
                _terminate_process_group(process)
                cleanup_started = now
            elif process.poll() is None and overflow and cleanup_started is None:
                _terminate_process_group(process)
                cleanup_started = now
            elif process.poll() is not None and cleanup_started is None:
                _terminate_process_group(process)
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
                    if cleanup_started is None:
                        _terminate_process_group(process)
                        cleanup_started = time.monotonic()

        try:
            returncode = process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            _terminate_process_group(process)
            try:
                returncode = process.wait(timeout=1)
            except subprocess.TimeoutExpired as error:
                raise EvidenceError(
                    "verifier process group could not be terminated"
                ) from error
        return _ProcessResult(returncode, bytes(output), timed_out, overflow)
    finally:
        for key in list(selector.get_map().values()):
            selector.unregister(key.fileobj)
            key.fileobj.close()
        selector.close()
        if process is not None and process.poll() is None:
            _terminate_process_group(process)
            try:
                process.wait(timeout=1)
            except subprocess.TimeoutExpired as error:
                raise EvidenceError("verifier process group escaped cleanup") from error


def _invoke_verifier(
    verifier: _PinnedVerifier,
    binding: SignerBindingV1,
    payload: bytes,
    signature: bytes,
    timeout: int,
) -> None:
    sealed_inputs: list[_SealedInput] = []
    try:
        allowed = _seal_bytes(
            "fe2o3-allowed-signers",
            f"{binding.signer_identity} {binding.public_key}\n".encode("ascii"),
        )
        sealed_inputs.append(allowed)
        sealed_signature = _seal_bytes("fe2o3-attestation-signature", signature)
        sealed_inputs.append(sealed_signature)
        sealed_payload = _seal_bytes("fe2o3-attestation-payload", payload)
        sealed_inputs.append(sealed_payload)
        descriptors = (
            verifier.descriptor,
            allowed.descriptor,
            sealed_signature.descriptor,
            sealed_payload.descriptor,
        )
        result = _run_bounded_process(
            executable_descriptor=verifier.descriptor,
            argv=[
                str(SSH_KEYGEN_PATH),
                "-Y",
                "verify",
                "-f",
                f"/proc/self/fd/{allowed.descriptor}",
                "-I",
                binding.signer_identity,
                "-n",
                SIGNATURE_NAMESPACE,
                "-s",
                f"/proc/self/fd/{sealed_signature.descriptor}",
            ],
            pass_descriptors=descriptors,
            stdin_descriptor=sealed_payload.descriptor,
            environment={
                "HOME": "/nonexistent-fe2o3-home",
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "TZ": "UTC",
            },
            timeout=timeout,
            output_limit=MAX_VERIFIER_OUTPUT_BYTES,
        )
        if result.unavailable:
            raise EvidenceError("OpenSSH verifier could not be executed")
        if result.timed_out:
            raise EvidenceError("OpenSSH verifier timed out")
        if result.overflow:
            raise EvidenceError("OpenSSH verifier output exceeded its bound")
        if result.returncode != 0:
            raise EvidenceError("OpenSSH signature verification failed")
    finally:
        for sealed in reversed(sealed_inputs):
            sealed.close()


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
    expected_policy_epoch: int,
    expected_campaign_nonce: str,
    expected_subjects: Mapping[str, str],
    now: int | None = None,
    timeout: int = 10,
) -> VerifiedAttestationObservationV1:
    expected_policy_identity = require_string(
        expected_policy_identity, "expected_policy_identity"
    )
    require_typed_identity(
        expected_policy_identity, POLICY_DOMAIN, "expected_policy_identity"
    )
    role = require_role(expected_role)
    signer_identity = require_signer_identity(expected_signer_identity)
    source_commit = require_commit(
        require_string(expected_source_commit, "expected_source_commit")
    )
    target = require_target(require_string(expected_target, "expected_target"))
    policy_epoch = require_policy_epoch(expected_policy_epoch, "expected_policy_epoch")
    campaign_nonce = require_campaign_nonce(expected_campaign_nonce)
    if isinstance(timeout, bool) or not 1 <= timeout <= MAX_VERIFIER_TIMEOUT_SECONDS:
        raise EvidenceError("verifier timeout must be between 1 and 30 seconds")
    current_time = (
        int(time.time()) if now is None else require_unix_timestamp(now, "now")
    )

    if not isinstance(expected_subjects, Mapping):
        raise EvidenceError("expected subjects must be a bounded mapping")
    if len(expected_subjects) > MAX_SUBJECTS:
        raise EvidenceError("expected subjects exceed the cardinality bound")
    if len(expected_subjects) != len(ROLE_SUBJECTS[role]):
        raise EvidenceError("expected subjects do not exactly match the role schema")
    for name in expected_subjects:
        if not isinstance(name, str) or _SUBJECT_NAME_RE.fullmatch(name) is None:
            raise EvidenceError("expected subject name is malformed")
    expected_names = tuple(sorted(expected_subjects))
    if expected_names != ROLE_SUBJECTS[role]:
        raise EvidenceError("expected subjects do not exactly match the role schema")
    canonical_expected: tuple[SubjectIdentity, ...] = tuple(
        SubjectIdentity(
            name,
            require_string(expected_subjects[name], f"expected subject {name}"),
        )
        for name in expected_names
    )
    validate_subjects(role, canonical_expected)

    policy = TrustPolicyV1.from_bytes(
        _read_bounded_regular(policy_path, MAX_POLICY_BYTES, "trust policy")
    )
    if policy.identity() != expected_policy_identity:
        raise EvidenceError("trust policy identity does not match the out-of-band pin")
    if policy.policy_epoch != policy_epoch:
        raise EvidenceError("trust policy epoch does not match the expected epoch")
    payload_bytes = _read_bounded_regular(
        payload_path, MAX_ATTESTATION_BYTES, "attestation payload"
    )
    payload = AttestationPayloadV1.from_bytes(payload_bytes)
    if payload.role != role:
        raise EvidenceError("attestation role does not match the expected role")
    if payload.signer_identity != signer_identity:
        raise EvidenceError("attestation signer does not match the expected signer")
    if payload.source_commit != source_commit:
        raise EvidenceError("attestation source commit does not match")
    if payload.target != target:
        raise EvidenceError("attestation target does not match")
    if payload.policy_identity != expected_policy_identity:
        raise EvidenceError("attestation does not bind the expected trust policy")
    if payload.policy_epoch != policy_epoch:
        raise EvidenceError("attestation does not bind the expected policy epoch")
    if payload.campaign_nonce != campaign_nonce:
        raise EvidenceError("attestation does not bind the expected campaign nonce")
    if current_time + MAX_CLOCK_SKEW_SECONDS < payload.issued_at:
        raise EvidenceError("attestation was issued too far in the future")
    if current_time >= payload.expires_at:
        raise EvidenceError("attestation has expired")
    if payload.subjects != canonical_expected:
        raise EvidenceError("attestation subjects do not match the expected identities")
    expected_context = derive_build_context_identity(
        source_commit=source_commit,
        target=target,
        role=role,
        policy_identity=expected_policy_identity,
        policy_epoch=policy_epoch,
        campaign_nonce=campaign_nonce,
        subjects=canonical_expected,
    )
    if payload.build_context_identity != expected_context:
        raise EvidenceError("attestation build context does not match expected inputs")

    binding = policy.binding(role, signer_identity, payload.issued_at)
    signature = _signature_bytes(signature_path)
    verifier = _pin_verifier(policy.verifier_identity)
    try:
        _invoke_verifier(verifier, binding, payload_bytes, signature, timeout)
    finally:
        verifier.close()
    return VerifiedAttestationObservationV1(
        attestation_identity=payload.identity(),
        policy_identity=policy.identity(),
        verifier_identity=policy.verifier_identity,
        key_identity=binding.key_identity,
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
        expected_policy_epoch=args.expect_policy_epoch,
        expected_campaign_nonce=args.expect_campaign_nonce,
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
            _read_bounded_regular(args.policy, MAX_POLICY_BYTES, "trust policy")
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
    verify.add_argument("--expect-policy-epoch", required=True, type=int)
    verify.add_argument("--expect-campaign-nonce", required=True)
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
