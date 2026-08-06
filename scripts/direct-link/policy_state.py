#!/usr/bin/env python3
"""Durable inert policy pin and aggregate release-context replay state.

The ordinary Python values returned here are forgeable observations. This module
does not authorize release, publication, loading, or launch.
"""

from __future__ import annotations

import errno
import fcntl
import json
import os
import re
import stat
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Iterator, Mapping

from common import (
    EvidenceError,
    metadata_snapshot,
    require_commit,
    require_target,
    require_typed_identity,
    typed_identity,
)

STATE_SCHEMA_VERSION = 2
RELEASE_CONTEXT_SCHEMA_VERSION = 1
OPERATION_ATTEMPT_SCHEMA_VERSION = 1

STATE_DOMAIN = "fe2o3-direct-link-policy-state-v2"
INTEGRITY_DOMAIN = "fe2o3-direct-link-policy-state-integrity-v2"
RELEASE_CONTEXT_DOMAIN = "fe2o3-direct-link-release-context-v1"
OPERATION_ATTEMPT_DOMAIN = "fe2o3-direct-link-operation-attempt-v1"
POLICY_DOMAIN = "fe2o3-direct-link-trust-policy-v1"
BUILD_CONTEXT_DOMAIN = "fe2o3-direct-link-build-context-v1"
ATTESTATION_DOMAIN = "fe2o3-direct-link-attestation-v1"

ATTESTATION_ROLES = (
    "g2-worker",
    "g5-publication",
    "g6-bundle",
    "g7-hardware-runner",
    "g7-static-runner",
)
REQUIRED_RELEASE_ROLES = tuple(sorted(ATTESTATION_ROLES))

STATE_FILE = "policy-state-v2.json"
LOCK_FILE = "policy-state-v2.lock"
TEMP_FILE = ".policy-state-v2.next"
LEGACY_STATE_FILES = (
    "policy-state-v1.json",
    "policy-state-v1.lock",
    ".policy-state-v1.next",
)

MAX_STATE_BYTES = 256 * 1024
MAX_CONSUMPTIONS = 512
MAX_POLICY_EPOCH = (1 << 63) - 1
MAX_GENERATION = (1 << 63) - 1
MAX_JSON_INTEGER_DIGITS = 19
MAX_SIGNER_IDENTITY_BYTES = 64
MAX_NONCE_BYTES = 64

DURABILITY_BOUNDARIES = (
    "before-write",
    "after-write",
    "before-file-fsync",
    "after-file-fsync",
    "before-rename",
    "after-rename",
    "before-directory-fsync",
    "after-directory-fsync",
)

_SIGNER_RE = re.compile(r"[a-z0-9][a-z0-9._@-]{0,63}\Z")
_NONCE_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
_TOP_LEVEL_FIELDS = frozenset(("checksum", "domain", "payload", "schema_version"))
_BODY_FIELDS = frozenset(("domain", "payload", "schema_version"))
_PAYLOAD_FIELDS = frozenset(
    (
        "consumptions",
        "generation",
        "pending_consumption",
        "policy_epoch",
        "policy_identity",
    )
)
_CONSUMPTION_FIELDS = frozenset(
    ("campaign_nonce", "operation_attempt_identity", "release_context_identity")
)
_ATTESTATION_BINDING_FIELDS = frozenset(
    (
        "attestation_identity",
        "build_context_identity",
        "role",
        "signer_identity",
    )
)

FaultInjector = Callable[[str], None]


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
            raise EvidenceError("policy state JSON contains a duplicate object key")
        result[key] = value
    return result


def _bounded_json_integer(value: str) -> int:
    digits = value[1:] if value.startswith("-") else value
    if len(digits) > MAX_JSON_INTEGER_DIGITS:
        raise EvidenceError("policy state JSON integer exceeds its bound")
    return int(value)


def _reject_json_number(_value: str) -> float:
    raise EvidenceError("policy state JSON does not permit non-integer numbers")


def _reject_json_constant(_value: str) -> None:
    raise EvidenceError("policy state JSON does not permit non-finite numbers")


def canonical_json_bytes(value: object) -> bytes:
    try:
        encoded = json.dumps(
            value, ensure_ascii=True, separators=(",", ":"), sort_keys=True
        )
        return (encoded + "\n").encode("ascii")
    except (
        OverflowError,
        RecursionError,
        TypeError,
        UnicodeEncodeError,
        ValueError,
    ) as error:
        raise EvidenceError("policy state contains an unsupported value") from error


def _parse_canonical_json(data: bytes) -> dict[str, Any]:
    try:
        text = data.decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError("policy state must contain ASCII only") from error
    if not text.endswith("\n") or "\r" in text or "\0" in text:
        raise EvidenceError(
            "policy state is not canonical newline-terminated ASCII JSON"
        )
    try:
        value = json.loads(
            text,
            object_pairs_hook=_unique_object,
            parse_int=_bounded_json_integer,
            parse_float=_reject_json_number,
            parse_constant=_reject_json_constant,
        )
    except EvidenceError:
        raise
    except (json.JSONDecodeError, RecursionError, ValueError) as error:
        raise EvidenceError("policy state is not valid bounded JSON") from error
    if not isinstance(value, dict):
        raise EvidenceError("policy state must be a JSON object")
    if canonical_json_bytes(value) != data:
        raise EvidenceError("policy state is not canonically encoded")
    return value


def _require_positive_integer(value: object, name: str, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceError(f"{name} must be an integer")
    if not 1 <= value <= maximum:
        raise EvidenceError(f"{name} is outside its supported range")
    return value


def require_policy_epoch(value: object) -> int:
    return _require_positive_integer(value, "policy_epoch", MAX_POLICY_EPOCH)


def _require_bounded_nonce(value: object, name: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) > MAX_NONCE_BYTES
        or _NONCE_RE.fullmatch(value) is None
    ):
        raise EvidenceError(f"{name} is malformed or exceeds {MAX_NONCE_BYTES} bytes")
    return value


def require_campaign_nonce(value: object) -> str:
    return _require_bounded_nonce(value, "campaign_nonce")


def require_signer_identity(value: object) -> str:
    if (
        not isinstance(value, str)
        or len(value) > MAX_SIGNER_IDENTITY_BYTES
        or _SIGNER_RE.fullmatch(value) is None
    ):
        raise EvidenceError("signer_identity is malformed or exceeds 64 bytes")
    return value


def _require_identity(value: object, domain: str, name: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{name} must be a string")
    if len(value) != len(domain) + len("-sha256-") + 64:
        raise EvidenceError(f"{name} must use one bounded {domain} identity")
    return require_typed_identity(value, domain, name)


def _require_source_commit(value: object) -> str:
    if not isinstance(value, str):
        raise EvidenceError("source_commit must be a string")
    return require_commit(value)


def _require_target(value: object) -> str:
    if not isinstance(value, str):
        raise EvidenceError("target must be a string")
    return require_target(value)


@dataclass(frozen=True, order=True)
class ReleaseAttestationBindingV1:
    role: str
    build_context_identity: str
    signer_identity: str
    attestation_identity: str

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        if self.role not in REQUIRED_RELEASE_ROLES:
            raise EvidenceError("release attestation role is unsupported")
        _require_identity(
            self.build_context_identity,
            BUILD_CONTEXT_DOMAIN,
            "build_context_identity",
        )
        require_signer_identity(self.signer_identity)
        _require_identity(
            self.attestation_identity,
            ATTESTATION_DOMAIN,
            "attestation_identity",
        )

    def to_object(self) -> dict[str, object]:
        return {
            "attestation_identity": self.attestation_identity,
            "build_context_identity": self.build_context_identity,
            "role": self.role,
            "signer_identity": self.signer_identity,
        }

    @classmethod
    def from_object(cls, value: object) -> ReleaseAttestationBindingV1:
        if not isinstance(value, dict):
            raise EvidenceError("release attestation binding must be an object")
        _require_exact_fields(
            value, _ATTESTATION_BINDING_FIELDS, "release attestation binding"
        )
        role = value["role"]
        if not isinstance(role, str):
            raise EvidenceError("release attestation role must be a string")
        return cls(
            role=role,
            build_context_identity=_require_identity(
                value["build_context_identity"],
                BUILD_CONTEXT_DOMAIN,
                "build_context_identity",
            ),
            signer_identity=require_signer_identity(value["signer_identity"]),
            attestation_identity=_require_identity(
                value["attestation_identity"],
                ATTESTATION_DOMAIN,
                "attestation_identity",
            ),
        )


@dataclass(frozen=True)
class ReleaseContextIdentityV1:
    source_commit: str
    target: str
    campaign_nonce: str
    policy_identity: str
    policy_epoch: int
    attestations: tuple[ReleaseAttestationBindingV1, ...]

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        _require_source_commit(self.source_commit)
        _require_target(self.target)
        require_campaign_nonce(self.campaign_nonce)
        _require_identity(self.policy_identity, POLICY_DOMAIN, "policy_identity")
        require_policy_epoch(self.policy_epoch)
        if len(self.attestations) != len(REQUIRED_RELEASE_ROLES):
            raise EvidenceError("release context omits a required attestation role")
        for binding in self.attestations:
            if not isinstance(binding, ReleaseAttestationBindingV1):
                raise EvidenceError(
                    "release context contains a non-attestation binding"
                )
            binding._validate()
        if self.attestations != tuple(sorted(self.attestations)):
            raise EvidenceError("release attestations are not canonically sorted")
        roles = tuple(binding.role for binding in self.attestations)
        if roles != REQUIRED_RELEASE_ROLES:
            raise EvidenceError(
                "release attestations must contain each required role exactly once"
            )

    def canonical_preimage(self) -> bytes:
        self._validate()
        return canonical_json_bytes(
            {
                "attestations": [binding.to_object() for binding in self.attestations],
                "campaign_nonce": self.campaign_nonce,
                "domain": RELEASE_CONTEXT_DOMAIN,
                "policy_epoch": self.policy_epoch,
                "policy_identity": self.policy_identity,
                "required_roles": list(REQUIRED_RELEASE_ROLES),
                "schema_version": RELEASE_CONTEXT_SCHEMA_VERSION,
                "source_commit": self.source_commit,
                "target": self.target,
            }
        )

    def identity(self) -> str:
        return typed_identity(RELEASE_CONTEXT_DOMAIN, self.canonical_preimage())


@dataclass(frozen=True)
class OperationAttemptIdentityV1:
    release_context_identity: str
    attempt_nonce: str

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        _require_identity(
            self.release_context_identity,
            RELEASE_CONTEXT_DOMAIN,
            "release_context_identity",
        )
        _require_bounded_nonce(self.attempt_nonce, "attempt_nonce")

    def canonical_preimage(self) -> bytes:
        self._validate()
        return canonical_json_bytes(
            {
                "attempt_nonce": self.attempt_nonce,
                "domain": OPERATION_ATTEMPT_DOMAIN,
                "release_context_identity": self.release_context_identity,
                "schema_version": OPERATION_ATTEMPT_SCHEMA_VERSION,
            }
        )

    def identity(self) -> str:
        return typed_identity(OPERATION_ATTEMPT_DOMAIN, self.canonical_preimage())


def derive_operation_attempt_identity(
    *, release_context_identity: str, attempt_nonce: str
) -> OperationAttemptIdentityV1:
    return OperationAttemptIdentityV1(
        release_context_identity=release_context_identity,
        attempt_nonce=attempt_nonce,
    )


@dataclass(frozen=True, order=True)
class ReleaseConsumptionV1:
    campaign_nonce: str
    release_context_identity: str
    operation_attempt_identity: str

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        require_campaign_nonce(self.campaign_nonce)
        _require_identity(
            self.release_context_identity,
            RELEASE_CONTEXT_DOMAIN,
            "release_context_identity",
        )
        _require_identity(
            self.operation_attempt_identity,
            OPERATION_ATTEMPT_DOMAIN,
            "operation_attempt_identity",
        )

    def to_object(self) -> dict[str, object]:
        return {
            "campaign_nonce": self.campaign_nonce,
            "operation_attempt_identity": self.operation_attempt_identity,
            "release_context_identity": self.release_context_identity,
        }

    @classmethod
    def from_object(cls, value: object) -> ReleaseConsumptionV1:
        if not isinstance(value, dict):
            raise EvidenceError("release consumption must be an object")
        _require_exact_fields(value, _CONSUMPTION_FIELDS, "release consumption")
        return cls(
            campaign_nonce=require_campaign_nonce(value["campaign_nonce"]),
            release_context_identity=_require_identity(
                value["release_context_identity"],
                RELEASE_CONTEXT_DOMAIN,
                "release_context_identity",
            ),
            operation_attempt_identity=_require_identity(
                value["operation_attempt_identity"],
                OPERATION_ATTEMPT_DOMAIN,
                "operation_attempt_identity",
            ),
        )


@dataclass(frozen=True)
class PolicyReplayStateV2:
    policy_identity: str
    policy_epoch: int
    generation: int
    consumptions: tuple[ReleaseConsumptionV1, ...]
    pending_consumption: ReleaseConsumptionV1 | None

    def __post_init__(self) -> None:
        _require_identity(self.policy_identity, POLICY_DOMAIN, "policy_identity")
        require_policy_epoch(self.policy_epoch)
        _require_positive_integer(self.generation, "generation", MAX_GENERATION)
        if len(self.consumptions) > MAX_CONSUMPTIONS:
            raise EvidenceError("policy state exceeds the replay cardinality bound")
        for consumption in self.consumptions:
            if not isinstance(consumption, ReleaseConsumptionV1):
                raise EvidenceError("policy state contains a non-consumption entry")
            consumption._validate()
        if self.pending_consumption is not None:
            if not isinstance(self.pending_consumption, ReleaseConsumptionV1):
                raise EvidenceError("pending_consumption has the wrong type")
            self.pending_consumption._validate()
        if (
            self.pending_consumption is not None
            and len(self.consumptions) >= MAX_CONSUMPTIONS
        ):
            raise EvidenceError(
                "a full replay ledger cannot have a pending consumption"
            )
        if self.consumptions != tuple(sorted(self.consumptions)):
            raise EvidenceError("release consumptions are not canonically sorted")
        minimum_generation = 1 + 2 * len(self.consumptions)
        if self.pending_consumption is not None:
            minimum_generation += 1
        if self.generation < minimum_generation:
            raise EvidenceError(
                "policy state generation is inconsistent with consumption transitions"
            )
        campaigns: set[str] = set()
        contexts: set[str] = set()
        attempts: set[str] = set()
        for consumption in self.consumptions:
            _insert_unique_consumption(consumption, campaigns, contexts, attempts)
        if self.pending_consumption is not None:
            _insert_unique_consumption(
                self.pending_consumption, campaigns, contexts, attempts
            )

    def payload_object(self) -> dict[str, object]:
        return {
            "consumptions": [entry.to_object() for entry in self.consumptions],
            "generation": self.generation,
            "pending_consumption": (
                None
                if self.pending_consumption is None
                else self.pending_consumption.to_object()
            ),
            "policy_epoch": self.policy_epoch,
            "policy_identity": self.policy_identity,
        }

    def canonical_bytes(self) -> bytes:
        body = {
            "domain": STATE_DOMAIN,
            "payload": self.payload_object(),
            "schema_version": STATE_SCHEMA_VERSION,
        }
        checksum = typed_identity(INTEGRITY_DOMAIN, canonical_json_bytes(body))
        encoded = canonical_json_bytes({**body, "checksum": checksum})
        if len(encoded) > MAX_STATE_BYTES:
            raise EvidenceError("policy state exceeds its encoded size bound")
        return encoded

    def identity(self) -> str:
        return typed_identity(STATE_DOMAIN, self.canonical_bytes())

    @classmethod
    def from_bytes(cls, data: bytes) -> PolicyReplayStateV2:
        if len(data) > MAX_STATE_BYTES:
            raise EvidenceError("policy state exceeds its encoded size bound")
        value = _parse_canonical_json(data)
        _require_exact_fields(value, _TOP_LEVEL_FIELDS, "policy state")
        if value["domain"] != STATE_DOMAIN:
            raise EvidenceError("policy state has the wrong domain")
        if value["schema_version"] != STATE_SCHEMA_VERSION:
            raise EvidenceError("policy state has an unsupported schema version")
        payload = value["payload"]
        if not isinstance(payload, dict):
            raise EvidenceError("policy state payload must be an object")
        _require_exact_fields(payload, _PAYLOAD_FIELDS, "policy state payload")
        raw_consumptions = payload["consumptions"]
        if not isinstance(raw_consumptions, list):
            raise EvidenceError("policy state consumptions must be an array")
        if len(raw_consumptions) > MAX_CONSUMPTIONS:
            raise EvidenceError("policy state exceeds the replay cardinality bound")
        raw_pending = payload["pending_consumption"]
        if raw_pending is not None and not isinstance(raw_pending, dict):
            raise EvidenceError("pending_consumption must be an object or null")
        state = cls(
            policy_identity=_require_identity(
                payload["policy_identity"], POLICY_DOMAIN, "policy_identity"
            ),
            policy_epoch=require_policy_epoch(payload["policy_epoch"]),
            generation=_require_positive_integer(
                payload["generation"], "generation", MAX_GENERATION
            ),
            consumptions=tuple(
                ReleaseConsumptionV1.from_object(item) for item in raw_consumptions
            ),
            pending_consumption=(
                None
                if raw_pending is None
                else ReleaseConsumptionV1.from_object(raw_pending)
            ),
        )
        body = {field: value[field] for field in _BODY_FIELDS}
        expected_checksum = typed_identity(INTEGRITY_DOMAIN, canonical_json_bytes(body))
        checksum = _require_identity(value["checksum"], INTEGRITY_DOMAIN, "checksum")
        if checksum != expected_checksum:
            raise EvidenceError("policy state integrity checksum does not match")
        if state.canonical_bytes() != data:
            raise EvidenceError("policy state is not canonically encoded")
        return state


def _insert_unique_consumption(
    consumption: ReleaseConsumptionV1,
    campaigns: set[str],
    contexts: set[str],
    attempts: set[str],
) -> None:
    if consumption.campaign_nonce in campaigns:
        raise EvidenceError("policy state reuses a campaign nonce")
    if consumption.release_context_identity in contexts:
        raise EvidenceError("policy state reuses a release context")
    if consumption.operation_attempt_identity in attempts:
        raise EvidenceError("policy state reuses an operation attempt")
    campaigns.add(consumption.campaign_nonce)
    contexts.add(consumption.release_context_identity)
    attempts.add(consumption.operation_attempt_identity)


@dataclass(frozen=True)
class LocalPolicyStateObservationV2:
    """Forgeable descriptive policy-state observation."""

    outcome: str
    state_identity: str
    state: PolicyReplayStateV2


@dataclass(frozen=True)
class FreshConsumptionObservationV1:
    """Forgeable result returned only by a newly committed consumption."""

    state_identity: str
    release_context_identity: str
    operation_attempt_identity: str


@dataclass(frozen=True)
class ConsumptionRecoveryObservationV1:
    """Forgeable recovery-only observation for one exact durable attempt."""

    outcome: str
    state_identity: str
    release_context_identity: str
    operation_attempt_identity: str


class LocalPolicyStateStoreV2:
    """Descriptor-pinned V2 state for a cooperating local process set."""

    def __init__(
        self,
        state_directory: Path,
        *,
        fault_injector: FaultInjector | None = None,
    ) -> None:
        self._directory = _open_private_directory(state_directory)
        self._fault_injector = fault_injector
        self._closed = False

    def close(self) -> None:
        if not self._closed:
            os.close(self._directory)
            self._closed = True

    def __enter__(self) -> LocalPolicyStateStoreV2:
        if self._closed:
            raise EvidenceError("policy state store is closed")
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def observe(self) -> LocalPolicyStateObservationV2 | None:
        with self._locked():
            state = self._read_state()
            if state is None:
                return None
            return _state_observation("observed", state)

    def pin_policy(
        self, policy_identity: str, policy_epoch: int
    ) -> LocalPolicyStateObservationV2:
        policy_identity = _require_identity(
            policy_identity, POLICY_DOMAIN, "policy_identity"
        )
        policy_epoch = require_policy_epoch(policy_epoch)
        with self._locked():
            current = self._read_state()
            if current is None:
                updated = PolicyReplayStateV2(
                    policy_identity=policy_identity,
                    policy_epoch=policy_epoch,
                    generation=1,
                    consumptions=(),
                    pending_consumption=None,
                )
                self._replace_state(
                    None, updated, transition="initialize", phase="pin-policy"
                )
                return _state_observation("initialized", updated)
            if current.pending_consumption is not None:
                raise EvidenceError(
                    "policy cannot change while a consumption attempt is pending"
                )
            if policy_epoch < current.policy_epoch:
                raise EvidenceError("policy epoch rollback is forbidden")
            if policy_epoch == current.policy_epoch:
                if policy_identity != current.policy_identity:
                    raise EvidenceError(
                        "policy identity changed without an epoch advance"
                    )
                return _state_observation("already-pinned", current)
            if policy_identity == current.policy_identity:
                raise EvidenceError(
                    "policy epoch advanced without a new policy identity"
                )
            updated = PolicyReplayStateV2(
                policy_identity=policy_identity,
                policy_epoch=policy_epoch,
                generation=_next_generation(current.generation),
                consumptions=current.consumptions,
                pending_consumption=None,
            )
            self._replace_state(
                current,
                updated,
                transition="advance-policy",
                phase="advance-policy",
            )
            return _state_observation("advanced", updated)

    def consume_once(
        self,
        *,
        release_context: ReleaseContextIdentityV1,
        operation_attempt: OperationAttemptIdentityV1,
    ) -> FreshConsumptionObservationV1:
        candidate = _consumption_from_inputs(release_context, operation_attempt)
        with self._locked():
            current = self._require_matching_policy(release_context)
            _require_fresh_consumption(current, candidate)
            if len(current.consumptions) >= MAX_CONSUMPTIONS:
                raise EvidenceError("policy replay ledger is full")
            planned = PolicyReplayStateV2(
                policy_identity=current.policy_identity,
                policy_epoch=current.policy_epoch,
                generation=_next_generation(current.generation),
                consumptions=current.consumptions,
                pending_consumption=candidate,
            )
            self._replace_state(
                current,
                planned,
                transition="plan-consumption",
                phase="plan-consumption",
            )
            completed = _complete_pending_state(planned)
            self._replace_state(
                planned,
                completed,
                transition="complete-consumption",
                phase="complete-consumption",
            )
            return FreshConsumptionObservationV1(
                state_identity=completed.identity(),
                release_context_identity=candidate.release_context_identity,
                operation_attempt_identity=candidate.operation_attempt_identity,
            )

    def resume_consumption(
        self,
        *,
        release_context: ReleaseContextIdentityV1,
        operation_attempt: OperationAttemptIdentityV1,
    ) -> ConsumptionRecoveryObservationV1:
        candidate = _consumption_from_inputs(release_context, operation_attempt)
        with self._locked():
            current = self._require_matching_policy(release_context)
            for completed in current.consumptions:
                if completed == candidate:
                    return _recovery_observation(
                        "completion-observed", current, candidate
                    )
                if _consumptions_overlap(completed, candidate):
                    raise EvidenceError(
                        "recovery attempt does not exactly match the durable completion"
                    )
            pending = current.pending_consumption
            if pending is None:
                raise EvidenceError(
                    "operation attempt is not durably registered for recovery"
                )
            if pending != candidate:
                raise EvidenceError(
                    "recovery attempt does not exactly match the durable pending operation"
                )
            completed_state = _complete_pending_state(current)
            self._replace_state(
                current,
                completed_state,
                transition="complete-consumption",
                phase="resume-consumption",
            )
            return _recovery_observation(
                "resumed-and-completed", completed_state, candidate
            )

    def _require_matching_policy(
        self, release_context: ReleaseContextIdentityV1
    ) -> PolicyReplayStateV2:
        release_context._validate()
        current = self._read_state()
        if current is None:
            raise EvidenceError("policy state must be pinned before consumption")
        if (
            current.policy_identity != release_context.policy_identity
            or current.policy_epoch != release_context.policy_epoch
        ):
            raise EvidenceError("release context does not match the pinned policy")
        return current

    @contextmanager
    def _locked(self) -> Iterator[None]:
        if self._closed:
            raise EvidenceError("policy state store is closed")
        _require_private_directory_descriptor(self._directory)
        _reject_legacy_state(self._directory)
        descriptor = _open_lock(self._directory)
        try:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as error:
                raise EvidenceError(
                    "policy state is locked by another process"
                ) from error
            _verify_named_descriptor(self._directory, LOCK_FILE, descriptor, "lock")
            self._recover_temp()
            os.fsync(self._directory)
            yield
        finally:
            os.close(descriptor)

    def _recover_temp(self) -> None:
        try:
            metadata = os.stat(TEMP_FILE, dir_fd=self._directory, follow_symlinks=False)
        except FileNotFoundError:
            return
        _require_private_regular_metadata(metadata, "recovery temp")
        try:
            os.unlink(TEMP_FILE, dir_fd=self._directory)
            os.fsync(self._directory)
        except OSError as error:
            raise EvidenceError(
                "cannot remove interrupted policy state temp"
            ) from error

    def _read_state(self) -> PolicyReplayStateV2 | None:
        flags = (
            os.O_RDONLY
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_NONBLOCK", 0)
        )
        try:
            descriptor = os.open(STATE_FILE, flags, dir_fd=self._directory)
        except FileNotFoundError:
            return None
        except OSError as error:
            raise EvidenceError("cannot open policy state file") from error
        try:
            before = os.fstat(descriptor)
            _require_private_regular_metadata(before, "policy state")
            if before.st_size > MAX_STATE_BYTES:
                raise EvidenceError("policy state exceeds its encoded size bound")
            chunks: list[bytes] = []
            remaining = MAX_STATE_BYTES + 1
            while remaining:
                chunk = os.read(descriptor, min(64 * 1024, remaining))
                if not chunk:
                    break
                chunks.append(chunk)
                remaining -= len(chunk)
            data = b"".join(chunks)
            if len(data) > MAX_STATE_BYTES:
                raise EvidenceError("policy state exceeds its encoded size bound")
            after = os.fstat(descriptor)
            if len(data) != before.st_size or metadata_snapshot(
                before
            ) != metadata_snapshot(after):
                raise EvidenceError("policy state changed while being read")
            _verify_named_descriptor(
                self._directory, STATE_FILE, descriptor, "policy state"
            )
            return PolicyReplayStateV2.from_bytes(data)
        finally:
            os.close(descriptor)

    def _fault(self, phase: str, point: str) -> None:
        if self._fault_injector is not None:
            self._fault_injector(f"{phase}:{point}")

    def _replace_state(
        self,
        previous: PolicyReplayStateV2 | None,
        state: PolicyReplayStateV2,
        *,
        transition: str,
        phase: str,
    ) -> None:
        _require_legal_transition(previous, state, transition)
        data = state.canonical_bytes()
        flags = (
            os.O_WRONLY
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0)
            | getattr(os, "O_NONBLOCK", 0)
        )
        try:
            descriptor = os.open(TEMP_FILE, flags, 0o600, dir_fd=self._directory)
        except OSError as error:
            raise EvidenceError("cannot create policy state temp") from error
        try:
            _require_private_regular_metadata(os.fstat(descriptor), "policy state temp")
            self._fault(phase, "before-write")
            _write_all(descriptor, data)
            self._fault(phase, "after-write")
            self._fault(phase, "before-file-fsync")
            os.fsync(descriptor)
            self._fault(phase, "after-file-fsync")
            _verify_named_descriptor(
                self._directory, TEMP_FILE, descriptor, "policy state temp"
            )
            self._fault(phase, "before-rename")
            os.replace(
                TEMP_FILE,
                STATE_FILE,
                src_dir_fd=self._directory,
                dst_dir_fd=self._directory,
            )
            self._fault(phase, "after-rename")
            _verify_named_descriptor(
                self._directory, STATE_FILE, descriptor, "policy state"
            )
            self._fault(phase, "before-directory-fsync")
            os.fsync(self._directory)
            self._fault(phase, "after-directory-fsync")
        except OSError as error:
            raise EvidenceError("cannot durably replace policy state") from error
        finally:
            os.close(descriptor)


def _consumption_from_inputs(
    release_context: ReleaseContextIdentityV1,
    operation_attempt: OperationAttemptIdentityV1,
) -> ReleaseConsumptionV1:
    if not isinstance(release_context, ReleaseContextIdentityV1):
        raise EvidenceError("release_context must be ReleaseContextIdentityV1")
    release_context._validate()
    if not isinstance(operation_attempt, OperationAttemptIdentityV1):
        raise EvidenceError("operation_attempt must be OperationAttemptIdentityV1")
    operation_attempt._validate()
    release_context_identity = release_context.identity()
    if operation_attempt.release_context_identity != release_context_identity:
        raise EvidenceError("operation attempt does not bind the release context")
    return ReleaseConsumptionV1(
        campaign_nonce=release_context.campaign_nonce,
        release_context_identity=release_context_identity,
        operation_attempt_identity=operation_attempt.identity(),
    )


def _require_fresh_consumption(
    state: PolicyReplayStateV2, candidate: ReleaseConsumptionV1
) -> None:
    if state.pending_consumption is not None:
        if _consumptions_overlap(state.pending_consumption, candidate):
            raise EvidenceError(
                "release context already has a durable pending attempt; use resume"
            )
        raise EvidenceError("another durable consumption attempt is pending")
    for completed in state.consumptions:
        if completed.release_context_identity == candidate.release_context_identity:
            raise EvidenceError(
                "release context replay is forbidden for fresh consumption"
            )
        if completed.campaign_nonce == candidate.campaign_nonce:
            raise EvidenceError("campaign nonce was already consumed")
        if completed.operation_attempt_identity == candidate.operation_attempt_identity:
            raise EvidenceError("operation attempt identity was already consumed")


def _consumptions_overlap(
    left: ReleaseConsumptionV1, right: ReleaseConsumptionV1
) -> bool:
    return (
        left.release_context_identity == right.release_context_identity
        or left.campaign_nonce == right.campaign_nonce
        or left.operation_attempt_identity == right.operation_attempt_identity
    )


def _complete_pending_state(state: PolicyReplayStateV2) -> PolicyReplayStateV2:
    pending = state.pending_consumption
    if pending is None:
        raise EvidenceError("policy state has no pending consumption to complete")
    return PolicyReplayStateV2(
        policy_identity=state.policy_identity,
        policy_epoch=state.policy_epoch,
        generation=_next_generation(state.generation),
        consumptions=tuple(sorted((*state.consumptions, pending))),
        pending_consumption=None,
    )


def _require_legal_transition(
    previous: PolicyReplayStateV2 | None,
    state: PolicyReplayStateV2,
    transition: str,
) -> None:
    if transition == "initialize":
        if previous is not None or state != PolicyReplayStateV2(
            policy_identity=state.policy_identity,
            policy_epoch=state.policy_epoch,
            generation=1,
            consumptions=(),
            pending_consumption=None,
        ):
            raise EvidenceError("illegal policy state initialization transition")
        return
    if previous is None:
        raise EvidenceError("non-initial policy transition requires previous state")
    if state.generation != _next_generation(previous.generation):
        raise EvidenceError(
            "policy state transition must increment generation exactly once"
        )
    if transition == "advance-policy":
        legal = (
            previous.pending_consumption is None
            and state.pending_consumption is None
            and state.consumptions == previous.consumptions
            and state.policy_epoch > previous.policy_epoch
            and state.policy_identity != previous.policy_identity
        )
    elif transition == "plan-consumption":
        legal = (
            _same_policy(previous, state)
            and previous.pending_consumption is None
            and state.pending_consumption is not None
            and state.consumptions == previous.consumptions
        )
    elif transition == "complete-consumption":
        pending = previous.pending_consumption
        legal = (
            _same_policy(previous, state)
            and pending is not None
            and state.pending_consumption is None
            and state.consumptions == tuple(sorted((*previous.consumptions, pending)))
        )
    else:
        raise EvidenceError("unknown policy state transition")
    if not legal:
        raise EvidenceError(f"illegal {transition} policy state transition")


def _same_policy(left: PolicyReplayStateV2, right: PolicyReplayStateV2) -> bool:
    return (
        left.policy_identity == right.policy_identity
        and left.policy_epoch == right.policy_epoch
    )


def _state_observation(
    outcome: str, state: PolicyReplayStateV2
) -> LocalPolicyStateObservationV2:
    return LocalPolicyStateObservationV2(
        outcome=outcome, state_identity=state.identity(), state=state
    )


def _recovery_observation(
    outcome: str,
    state: PolicyReplayStateV2,
    consumption: ReleaseConsumptionV1,
) -> ConsumptionRecoveryObservationV1:
    return ConsumptionRecoveryObservationV1(
        outcome=outcome,
        state_identity=state.identity(),
        release_context_identity=consumption.release_context_identity,
        operation_attempt_identity=consumption.operation_attempt_identity,
    )


def _next_generation(generation: int) -> int:
    if generation >= MAX_GENERATION:
        raise EvidenceError("policy state generation is exhausted")
    return generation + 1


def _write_all(descriptor: int, data: bytes) -> None:
    written = 0
    while written < len(data):
        count = os.write(descriptor, data[written:])
        if count <= 0:
            raise EvidenceError("policy state write made no progress")
        written += count


def _open_private_directory(path: Path) -> int:
    if not isinstance(path, Path) or not path.is_absolute():
        raise EvidenceError("policy state directory must be an absolute path")
    parts = path.parts[1:]
    if not parts or any(part in ("", ".", "..") for part in parts):
        raise EvidenceError("policy state directory path is malformed")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open("/", flags)
    try:
        for part in parts:
            try:
                next_descriptor = os.open(part, flags, dir_fd=descriptor)
            except OSError as error:
                raise EvidenceError(
                    "policy state directory traversal rejected a path component"
                ) from error
            os.close(descriptor)
            descriptor = next_descriptor
        _require_private_directory_descriptor(descriptor)
        return descriptor
    except Exception:
        os.close(descriptor)
        raise


def _require_private_directory_descriptor(descriptor: int) -> None:
    metadata = os.fstat(descriptor)
    if not stat.S_ISDIR(metadata.st_mode):
        raise EvidenceError("policy state path is not a directory")
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) & 0o077:
        raise EvidenceError("policy state directory must be private and owned by euid")


def _require_private_regular_metadata(metadata: os.stat_result, name: str) -> None:
    if not stat.S_ISREG(metadata.st_mode):
        raise EvidenceError(f"{name} is not a regular file")
    if metadata.st_uid != os.geteuid():
        raise EvidenceError(f"{name} is not owned by euid")
    if stat.S_IMODE(metadata.st_mode) != 0o600:
        raise EvidenceError(f"{name} must have mode 0600")
    if metadata.st_nlink != 1:
        raise EvidenceError(f"{name} must have exactly one hard link")


def _verify_named_descriptor(
    directory: int, name: str, descriptor: int, description: str
) -> None:
    opened = os.fstat(descriptor)
    _require_private_regular_metadata(opened, description)
    try:
        named = os.stat(name, dir_fd=directory, follow_symlinks=False)
    except OSError as error:
        raise EvidenceError(f"{description} pathname was replaced") from error
    _require_private_regular_metadata(named, description)
    if (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino):
        raise EvidenceError(f"{description} pathname was substituted")


def _open_lock(directory: int) -> int:
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_NONBLOCK", 0)
    )
    try:
        descriptor = os.open(LOCK_FILE, flags, 0o600, dir_fd=directory)
    except OSError as error:
        if error.errno in (errno.ELOOP, errno.ENXIO, errno.ENODEV):
            raise EvidenceError(
                "policy state lock is not a safe regular file"
            ) from error
        raise EvidenceError("cannot open policy state lock") from error
    try:
        _require_private_regular_metadata(os.fstat(descriptor), "policy state lock")
        return descriptor
    except Exception:
        os.close(descriptor)
        raise


def _reject_legacy_state(directory: int) -> None:
    for name in LEGACY_STATE_FILES:
        try:
            os.stat(name, dir_fd=directory, follow_symlinks=False)
        except FileNotFoundError:
            continue
        except OSError as error:
            raise EvidenceError("cannot inspect legacy policy state") from error
        raise EvidenceError(
            "legacy policy state requires explicit trusted migration before V2 use"
        )
