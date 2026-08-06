#!/usr/bin/env python3
"""Durable local policy pin and replay observations for direct-link attestations.

This module is inert infrastructure. Its forgeable observations do not authorize
release, publication, loading, or launch.
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
    require_typed_identity,
    typed_identity,
)

SCHEMA_VERSION = 1
STATE_DOMAIN = "fe2o3-direct-link-policy-state-v1"
INTEGRITY_DOMAIN = "fe2o3-direct-link-policy-state-integrity-v1"
POLICY_DOMAIN = "fe2o3-direct-link-trust-policy-v1"
BUILD_CONTEXT_DOMAIN = "fe2o3-direct-link-build-context-v1"

STATE_FILE = "policy-state-v1.json"
LOCK_FILE = "policy-state-v1.lock"
TEMP_FILE = ".policy-state-v1.next"

MAX_STATE_BYTES = 192 * 1024
MAX_CONSUMPTIONS = 512
MAX_POLICY_EPOCH = (1 << 63) - 1
MAX_GENERATION = (1 << 63) - 1
MAX_JSON_INTEGER_DIGITS = 19

FAULT_POINTS = (
    "before-write",
    "after-write",
    "before-file-fsync",
    "after-file-fsync",
    "before-rename",
    "after-rename",
    "before-directory-fsync",
    "after-directory-fsync",
)

_CAMPAIGN_NONCE_RE = re.compile(r"[a-z0-9][a-z0-9._-]{0,63}\Z")
_TOP_LEVEL_FIELDS = frozenset(("checksum", "domain", "payload", "schema_version"))
_BODY_FIELDS = frozenset(("domain", "payload", "schema_version"))
_PAYLOAD_FIELDS = frozenset(
    ("consumptions", "generation", "policy_epoch", "policy_identity")
)
_CONSUMPTION_FIELDS = frozenset(("build_context_identity", "campaign_nonce"))

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


def _require_positive_bounded_integer(value: object, name: str, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceError(f"{name} must be an integer")
    if not 1 <= value <= maximum:
        raise EvidenceError(f"{name} is outside its supported range")
    return value


def require_policy_epoch(value: object) -> int:
    return _require_positive_bounded_integer(value, "policy_epoch", MAX_POLICY_EPOCH)


def require_campaign_nonce(value: object) -> str:
    if not isinstance(value, str) or _CAMPAIGN_NONCE_RE.fullmatch(value) is None:
        raise EvidenceError("campaign_nonce is malformed or exceeds 64 bytes")
    return value


@dataclass(frozen=True, order=True)
class ReplayConsumptionV1:
    campaign_nonce: str
    build_context_identity: str

    def __post_init__(self) -> None:
        require_campaign_nonce(self.campaign_nonce)
        require_typed_identity(
            self.build_context_identity,
            BUILD_CONTEXT_DOMAIN,
            "build_context_identity",
        )

    def to_object(self) -> dict[str, object]:
        return {
            "build_context_identity": self.build_context_identity,
            "campaign_nonce": self.campaign_nonce,
        }

    @classmethod
    def from_object(cls, value: object) -> ReplayConsumptionV1:
        if not isinstance(value, dict):
            raise EvidenceError("policy state consumption must be an object")
        _require_exact_fields(value, _CONSUMPTION_FIELDS, "policy state consumption")
        return cls(
            campaign_nonce=require_campaign_nonce(value["campaign_nonce"]),
            build_context_identity=_require_identity(
                value["build_context_identity"],
                BUILD_CONTEXT_DOMAIN,
                "build_context_identity",
            ),
        )


def _require_identity(value: object, domain: str, name: str) -> str:
    if not isinstance(value, str):
        raise EvidenceError(f"{name} must be a string")
    return require_typed_identity(value, domain, name)


@dataclass(frozen=True)
class PolicyReplayStateV1:
    policy_identity: str
    policy_epoch: int
    generation: int
    consumptions: tuple[ReplayConsumptionV1, ...]

    def __post_init__(self) -> None:
        require_typed_identity(self.policy_identity, POLICY_DOMAIN, "policy_identity")
        require_policy_epoch(self.policy_epoch)
        _require_positive_bounded_integer(self.generation, "generation", MAX_GENERATION)
        if len(self.consumptions) > MAX_CONSUMPTIONS:
            raise EvidenceError("policy state exceeds the replay cardinality bound")
        canonical = tuple(sorted(self.consumptions))
        if self.consumptions != canonical:
            raise EvidenceError("policy state consumptions are not canonically sorted")
        campaigns: set[str] = set()
        contexts: set[str] = set()
        for consumption in self.consumptions:
            if consumption.campaign_nonce in campaigns:
                raise EvidenceError("policy state reuses a campaign nonce")
            if consumption.build_context_identity in contexts:
                raise EvidenceError("policy state reuses a build context")
            campaigns.add(consumption.campaign_nonce)
            contexts.add(consumption.build_context_identity)

    def payload_object(self) -> dict[str, object]:
        return {
            "consumptions": [entry.to_object() for entry in self.consumptions],
            "generation": self.generation,
            "policy_epoch": self.policy_epoch,
            "policy_identity": self.policy_identity,
        }

    def canonical_bytes(self) -> bytes:
        body = {
            "domain": STATE_DOMAIN,
            "payload": self.payload_object(),
            "schema_version": SCHEMA_VERSION,
        }
        checksum = typed_identity(INTEGRITY_DOMAIN, canonical_json_bytes(body))
        encoded = canonical_json_bytes({**body, "checksum": checksum})
        if len(encoded) > MAX_STATE_BYTES:
            raise EvidenceError("policy state exceeds its encoded size bound")
        return encoded

    def identity(self) -> str:
        return typed_identity(STATE_DOMAIN, self.canonical_bytes())

    @classmethod
    def from_bytes(cls, data: bytes) -> PolicyReplayStateV1:
        if len(data) > MAX_STATE_BYTES:
            raise EvidenceError("policy state exceeds its encoded size bound")
        value = _parse_canonical_json(data)
        _require_exact_fields(value, _TOP_LEVEL_FIELDS, "policy state")
        if value["domain"] != STATE_DOMAIN:
            raise EvidenceError("policy state has the wrong domain")
        if value["schema_version"] != SCHEMA_VERSION:
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
        state = cls(
            policy_identity=_require_identity(
                payload["policy_identity"], POLICY_DOMAIN, "policy_identity"
            ),
            policy_epoch=require_policy_epoch(payload["policy_epoch"]),
            generation=_require_positive_bounded_integer(
                payload["generation"], "generation", MAX_GENERATION
            ),
            consumptions=tuple(
                ReplayConsumptionV1.from_object(item) for item in raw_consumptions
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


@dataclass(frozen=True)
class LocalPolicyStateObservationV1:
    """Forgeable, descriptive result of one local state operation."""

    outcome: str
    state_identity: str
    state: PolicyReplayStateV1


class LocalPolicyStateStoreV1:
    """Descriptor-pinned durable state for a cooperating local process set."""

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

    def __enter__(self) -> LocalPolicyStateStoreV1:
        if self._closed:
            raise EvidenceError("policy state store is closed")
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()

    def observe(self) -> LocalPolicyStateObservationV1 | None:
        with self._locked():
            state = self._read_state()
            if state is None:
                return None
            return _observation("observed", state)

    def pin_policy(
        self, policy_identity: str, policy_epoch: int
    ) -> LocalPolicyStateObservationV1:
        policy_identity = _require_identity(
            policy_identity, POLICY_DOMAIN, "policy_identity"
        )
        policy_epoch = require_policy_epoch(policy_epoch)
        with self._locked():
            current = self._read_state()
            if current is None:
                updated = PolicyReplayStateV1(
                    policy_identity=policy_identity,
                    policy_epoch=policy_epoch,
                    generation=1,
                    consumptions=(),
                )
                self._replace_state(updated)
                return _observation("initialized", updated)
            if policy_epoch < current.policy_epoch:
                raise EvidenceError("policy epoch rollback is forbidden")
            if policy_epoch == current.policy_epoch:
                if policy_identity != current.policy_identity:
                    raise EvidenceError(
                        "policy identity changed without an epoch advance"
                    )
                return _observation("already-pinned", current)
            if policy_identity == current.policy_identity:
                raise EvidenceError(
                    "policy epoch advanced without a new policy identity"
                )
            updated = PolicyReplayStateV1(
                policy_identity=policy_identity,
                policy_epoch=policy_epoch,
                generation=_next_generation(current.generation),
                consumptions=current.consumptions,
            )
            self._replace_state(updated)
            return _observation("advanced", updated)

    def consume_once(
        self,
        *,
        policy_identity: str,
        policy_epoch: int,
        campaign_nonce: str,
        build_context_identity: str,
    ) -> LocalPolicyStateObservationV1:
        policy_identity = _require_identity(
            policy_identity, POLICY_DOMAIN, "policy_identity"
        )
        policy_epoch = require_policy_epoch(policy_epoch)
        candidate = ReplayConsumptionV1(
            campaign_nonce=require_campaign_nonce(campaign_nonce),
            build_context_identity=_require_identity(
                build_context_identity,
                BUILD_CONTEXT_DOMAIN,
                "build_context_identity",
            ),
        )
        with self._locked():
            current = self._read_state()
            if current is None:
                raise EvidenceError(
                    "policy state must be pinned before replay consumption"
                )
            if (
                current.policy_identity != policy_identity
                or current.policy_epoch != policy_epoch
            ):
                raise EvidenceError(
                    "replay consumption does not match the pinned policy"
                )
            for existing in current.consumptions:
                if existing == candidate:
                    return _observation("already-consumed", current)
                if existing.campaign_nonce == candidate.campaign_nonce:
                    raise EvidenceError("campaign nonce was already consumed")
                if existing.build_context_identity == candidate.build_context_identity:
                    raise EvidenceError("build context was already consumed")
            if len(current.consumptions) >= MAX_CONSUMPTIONS:
                raise EvidenceError("policy replay ledger is full")
            updated = PolicyReplayStateV1(
                policy_identity=current.policy_identity,
                policy_epoch=current.policy_epoch,
                generation=_next_generation(current.generation),
                consumptions=tuple(sorted((*current.consumptions, candidate))),
            )
            self._replace_state(updated)
            return _observation("consumed", updated)

    @contextmanager
    def _locked(self) -> Iterator[None]:
        if self._closed:
            raise EvidenceError("policy state store is closed")
        _require_private_directory_descriptor(self._directory)
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

    def _read_state(self) -> PolicyReplayStateV1 | None:
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
            return PolicyReplayStateV1.from_bytes(data)
        finally:
            os.close(descriptor)

    def _fault(self, point: str) -> None:
        if self._fault_injector is not None:
            self._fault_injector(point)

    def _replace_state(self, state: PolicyReplayStateV1) -> None:
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
            self._fault("before-write")
            _write_all(descriptor, data)
            self._fault("after-write")
            self._fault("before-file-fsync")
            os.fsync(descriptor)
            self._fault("after-file-fsync")
            _verify_named_descriptor(
                self._directory, TEMP_FILE, descriptor, "policy state temp"
            )
            self._fault("before-rename")
            os.replace(
                TEMP_FILE,
                STATE_FILE,
                src_dir_fd=self._directory,
                dst_dir_fd=self._directory,
            )
            self._fault("after-rename")
            _verify_named_descriptor(
                self._directory, STATE_FILE, descriptor, "policy state"
            )
            self._fault("before-directory-fsync")
            os.fsync(self._directory)
            self._fault("after-directory-fsync")
        except OSError as error:
            raise EvidenceError("cannot durably replace policy state") from error
        finally:
            os.close(descriptor)


def _observation(
    outcome: str, state: PolicyReplayStateV1
) -> LocalPolicyStateObservationV1:
    return LocalPolicyStateObservationV1(
        outcome=outcome, state_identity=state.identity(), state=state
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
