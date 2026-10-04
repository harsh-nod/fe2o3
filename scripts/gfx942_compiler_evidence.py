#!/usr/bin/python3.12
"""Reproducible gfx942 compiler-evidence controller.

This is a measurement controller, not a compiler-causality authenticator.  Its
tool configuration is repository-pinned and host-specific by design.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import resource
import shutil
import stat
import struct
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from compiler_evidence_hardening import (
    CommandLimits,
    HardeningError,
    RetainedClosure,
    RetainedFile,
    SealedExecutable,
    SnapshotClosure,
    Supervisor,
    adversarial_self_test,
    capture_snapshot,
    capture_retained_closure,
    compare_labeled_manifests,
)


SCHEMA = "fe2o3-gfx942-compiler-tool-manifest-v1"
GOLDEN_SCHEMA = "fe2o3-gfx942-alpha-zeta-compiler-evidence-v1"
EXPECTED_PLATFORM = "mi300x-rocm-7.2.4-ubuntu-24.04-x86_64"
EXPECTED_RUST_TOOLCHAIN = "nightly-2026-04-03"
EXPECTED_CARGO_RELEASE = "1.96.0-nightly"
EXPECTED_CARGO_COMMIT = "888f675344eb1cf2308fd53183e667bdd2c58e51"
EXPECTED_RUSTC_RELEASE = "1.96.0-nightly"
EXPECTED_RUSTC_COMMIT = "55e86c996809902e8bbad512cfb4d2c18be446d9"
EXPECTED_LLVM_BUILD = "7.2.4"
EXPECTED_LLVM_PACKAGE = "22.0.0git"
EXPECTED_TRANSITION_PUBLIC_KEY = bytes.fromhex(
    "3acecd80720befbedbff8292a6adcac6a7b160e79d05ae4e8599dc1c1dcf2b01"
)
TOOL_RUNTIME_FIXTURE = (
    "tests/fixtures/compiler-evidence/gfx942-mi300x-tool-runtime-v1.json"
)
MAX_CONFIG_BYTES = 256 * 1024
MAX_TOOL_BYTES = 256 * 1024 * 1024
MAX_VERSION_BYTES = 64 * 1024
MAX_RUNTIME_FILES = 256
MAX_RUNTIME_BYTES = 512 * 1024 * 1024
MAX_CAPTURE_BYTES = 16 * 1024 * 1024
MAX_CGROUP_SAMPLES = 4096
WORKER_V2_REQUEST_DOMAIN = b"FE2O3/DIRECT-LLVM-WORKER-REQUEST/V2\0"
WORKER_V2_RESPONSE_DOMAIN = b"FE2O3/WORKER-V2-SEALED-RESPONSE/V1\0"
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
TOOL_NAMES = (
    "shell",
    "python",
    "git",
    "cmake",
    "ctest",
    "ninja",
    "cxx",
    "lld",
    "llvm_ar",
    "cargo",
    "rustc",
    "ldd",
    "getconf",
    "basename",
    "cat",
    "chmod",
    "cp",
    "dirname",
    "env",
    "ln",
    "mkdir",
    "mv",
    "pwd",
    "rm",
    "sha256sum",
    "stat",
    "touch",
    "uname",
)
EXPECTED_ALIASES = {
    "ar": "llvm_ar",
    "bash": "shell",
    "basename": "basename",
    "c++": "cxx",
    "cargo": "cargo",
    "cat": "cat",
    "cc": "cxx",
    "chmod": "chmod",
    "clang": "cxx",
    "clang++": "cxx",
    "cmake": "cmake",
    "cp": "cp",
    "ctest": "ctest",
    "dirname": "dirname",
    "env": "env",
    "git": "git",
    "getconf": "getconf",
    "ld": "lld",
    "ld.lld": "lld",
    "ln": "ln",
    "lld": "lld",
    "llvm-ar": "llvm_ar",
    "llvm-ranlib": "llvm_ar",
    "mkdir": "mkdir",
    "mv": "mv",
    "ninja": "ninja",
    "python3": "python",
    "pwd": "pwd",
    "ranlib": "llvm_ar",
    "rm": "rm",
    "rustc": "rustc",
    "sh": "shell",
    "sha256sum": "sha256sum",
    "stat": "stat",
    "touch": "touch",
    "uname": "uname",
}


class EvidenceError(RuntimeError):
    pass


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def document_record(data: bytes) -> dict[str, Any]:
    if len(data) > MAX_CAPTURE_BYTES:
        raise EvidenceError("evidence document exceeds its bound")
    return {"bytes": len(data), "sha256": sha256_bytes(data)}


def verify_bound_document(path: Path, record: dict[str, Any]) -> bytes:
    if set(record) != {"bytes", "sha256"}:
        raise EvidenceError("bound document record fields are not exact")
    try:
        data = path.read_bytes()
    except FileNotFoundError as error:
        raise EvidenceError(f"bound evidence document is missing: {path}") from error
    if document_record(data) != record:
        raise EvidenceError(f"bound evidence document changed: {path}")
    return data


def _bounded_hex(record: dict[str, Any], prefix: str) -> bytes:
    encoded = record.get(f"{prefix}_hex")
    declared_bytes = record.get(f"{prefix}_bytes")
    declared_sha256 = record.get(f"{prefix}_sha256")
    if (
        not isinstance(encoded, str)
        or not isinstance(declared_bytes, int)
        or declared_bytes < 1
        or declared_bytes > MAX_CAPTURE_BYTES
        or not isinstance(declared_sha256, str)
        or not SHA256_RE.fullmatch(declared_sha256)
        or len(encoded) != declared_bytes * 2
    ):
        raise EvidenceError(f"{prefix} capture metadata is invalid")
    try:
        data = bytes.fromhex(encoded)
    except ValueError as error:
        raise EvidenceError(f"{prefix} capture is not hexadecimal") from error
    if sha256_bytes(data) != declared_sha256:
        raise EvidenceError(f"{prefix} capture digest changed")
    return data


def verify_transaction_capture(record: dict[str, Any]) -> None:
    if record.get("schema") != "fe2o3-non-production-compiler-reproduction-record-v2":
        raise EvidenceError("compiler transaction capture schema changed")
    request = _bounded_hex(record, "canonical_request")
    response = _bounded_hex(record, "canonical_response")
    raw_output = _bounded_hex(record, "raw_output")
    if len(request) < 46 or request[:8] != b"F3LREQ02":
        raise EvidenceError("canonical Worker V2 request framing is invalid")
    tag, size = struct.unpack("<HI", request[-38:-32])
    if (tag, size) != (15, 32):
        raise EvidenceError("canonical Worker V2 request identity field is invalid")
    request_identity = hashlib.sha256(
        WORKER_V2_REQUEST_DOMAIN
        + len(request[:-38]).to_bytes(8, "little")
        + request[:-38]
    ).hexdigest()
    response_identity = hashlib.sha256(
        WORKER_V2_RESPONSE_DOMAIN
        + len(response).to_bytes(8, "little")
        + response
    ).hexdigest()
    if (
        request_identity != request[-32:].hex()
        or request_identity != record.get("worker_v2_request_identity")
        or response_identity != record.get("sealed_worker_v2_response_identity")
        or sha256_bytes(raw_output) != record.get("raw_output_identity")
    ):
        raise EvidenceError("compiler transaction raw bytes do not recompute their identities")


def reject_repository_python_bytecode(repo: Path) -> None:
    for root, directories, files in os.walk(repo):
        if "__pycache__" in directories:
            raise EvidenceError(f"repository contains Python bytecode cache: {root}")
        if any(name.endswith(".pyc") for name in files):
            raise EvidenceError(f"repository contains compiled Python bytecode: {root}")


def sha256_file(file: Any, size: int) -> str:
    if size < 1 or size > MAX_TOOL_BYTES:
        raise EvidenceError("tool size is outside the configured bound")
    digest = hashlib.sha256()
    offset = 0
    while offset < size:
        chunk = os.pread(file.fileno(), min(1024 * 1024, size - offset), offset)
        if not chunk:
            raise EvidenceError("tool became truncated while hashing")
        digest.update(chunk)
        offset += len(chunk)
    return digest.hexdigest()


def exact_stat(value: os.stat_result) -> tuple[int, int, int, int, int, int, int]:
    return (
        value.st_dev,
        value.st_ino,
        value.st_mode,
        value.st_uid,
        value.st_gid,
        value.st_size,
        value.st_ctime_ns,
    )


def stat_record(value: os.stat_result) -> dict[str, int]:
    return {
        "device": value.st_dev,
        "inode": value.st_ino,
        "mode": value.st_mode & 0o7777,
        "uid": value.st_uid,
        "gid": value.st_gid,
        "bytes": value.st_size,
        "mtime_ns": value.st_mtime_ns,
        "ctime_ns": value.st_ctime_ns,
    }


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise EvidenceError(f"{label} fields are not exact")


def read_bounded_json(path: Path) -> tuple[dict[str, Any], bytes]:
    if path.resolve(strict=True) != path or path.is_symlink():
        raise EvidenceError(f"configuration path is not canonical: {path}")
    data = path.read_bytes()
    if not data or len(data) > MAX_CONFIG_BYTES:
        raise EvidenceError(f"configuration size is invalid: {path}")
    try:
        value = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError(f"configuration is not canonical JSON: {path}") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"configuration root is not an object: {path}")
    return value, data


def retain_configured_tool_runtime(
    repo: Path, manifest: dict[str, Any]
) -> tuple[list[dict[str, Any]], RetainedClosure]:
    path = repo / TOOL_RUNTIME_FIXTURE
    if path.resolve(strict=True) != path or path.is_symlink():
        raise EvidenceError("configured tool-runtime fixture path is not canonical")
    data = path.read_bytes()
    if not data or len(data) > MAX_CONFIG_BYTES:
        raise EvidenceError("configured tool-runtime fixture size is invalid")
    if sha256_bytes(data) != manifest["runtime_manifest_sha256"]:
        raise EvidenceError("configured tool-runtime fixture digest changed")
    try:
        records = json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EvidenceError("configured tool-runtime fixture is invalid JSON") from error
    if (
        not isinstance(records, list)
        or not records
        or len(records) > MAX_RUNTIME_FILES
        or canonical_json(records) != data
    ):
        raise EvidenceError("configured tool-runtime fixture is not a bounded canonical list")
    expected_fields = {
        "path",
        "sha256",
        "bytes",
        "uid",
        "gid",
        "mode",
        "device",
        "inode",
        "mtime_ns",
        "ctime_ns",
    }
    paths: list[Path] = []
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise EvidenceError(f"tool-runtime record {index} is not an object")
        require_exact_keys(record, expected_fields, f"tool-runtime record {index}")
        runtime_path = Path(str(record["path"]))
        if (
            not runtime_path.is_absolute()
            or runtime_path.is_symlink()
            or runtime_path.resolve(strict=True) != runtime_path
            or not SHA256_RE.fullmatch(str(record["sha256"]))
        ):
            raise EvidenceError(f"tool-runtime record {index} path or digest is invalid")
        named = os.stat(runtime_path, follow_symlinks=False)
        observed = stat_record(named)
        for field in (
            "bytes",
            "uid",
            "gid",
            "mode",
            "device",
            "inode",
            "mtime_ns",
            "ctime_ns",
        ):
            if observed[field] != record[field]:
                raise EvidenceError(
                    f"configured tool-runtime identity changed: {runtime_path}: {field}"
                )
        paths.append(runtime_path)
    if paths != sorted(set(paths), key=os.fspath):
        raise EvidenceError("configured tool-runtime paths are not sorted and unique")
    closure = capture_retained_closure(
        "configured-tool-runtime",
        [(f"runtime:{runtime_path.as_posix()}", runtime_path) for runtime_path in paths],
        {"runtime_manifest_sha256": manifest["runtime_manifest_sha256"]},
    )
    try:
        for record, retained in zip(records, closure.files, strict=True):
            if retained.sha256 != record["sha256"]:
                raise EvidenceError(
                    f"configured tool-runtime content changed: {record['path']}"
                )
        closure.revalidate()
        return records, closure
    except BaseException:
        closure.close()
        raise


def validate_manifest_document(manifest: dict[str, Any]) -> None:
    require_exact_keys(
        manifest,
        {
            "schema",
            "platform",
            "rust_toolchain",
            "cargo_release",
            "cargo_commit",
            "rustc_release",
            "rustc_commit",
            "llvm_package_version",
            "llvm_build_identity",
            "registry_source",
            "runtime_manifest_sha256",
            "tools",
            "path_aliases",
        },
        "tool manifest",
    )
    expected_scalars = {
        "schema": SCHEMA,
        "platform": EXPECTED_PLATFORM,
        "rust_toolchain": EXPECTED_RUST_TOOLCHAIN,
        "cargo_release": EXPECTED_CARGO_RELEASE,
        "cargo_commit": EXPECTED_CARGO_COMMIT,
        "rustc_release": EXPECTED_RUSTC_RELEASE,
        "rustc_commit": EXPECTED_RUSTC_COMMIT,
        "llvm_package_version": EXPECTED_LLVM_PACKAGE,
        "llvm_build_identity": EXPECTED_LLVM_BUILD,
    }
    for field, expected in expected_scalars.items():
        if manifest.get(field) != expected:
            raise EvidenceError(f"tool manifest {field} is not the required identity")
    if not SHA256_RE.fullmatch(str(manifest["runtime_manifest_sha256"])):
        raise EvidenceError("runtime closure identity is not SHA-256")
    registry = Path(str(manifest["registry_source"]))
    if not registry.is_absolute() or os.fspath(registry) != os.fspath(registry.resolve(strict=False)):
        raise EvidenceError("registry source path is not absolute and canonical")
    if manifest["path_aliases"] != EXPECTED_ALIASES:
        raise EvidenceError("allowlisted PATH aliases changed")
    tools = manifest["tools"]
    if not isinstance(tools, list) or [tool.get("name") for tool in tools] != list(TOOL_NAMES):
        raise EvidenceError("tool ordering or closure changed")
    for tool in tools:
        if not isinstance(tool, dict):
            raise EvidenceError("tool entry is not an object")
        require_exact_keys(
            tool,
            {
                "name",
                "path",
                "sha256",
                "bytes",
                "uid",
                "gid",
                "mode",
                "version_args",
                "version_output_sha256",
            },
            f"tool {tool.get('name')}",
        )
        path = Path(str(tool["path"]))
        if not path.is_absolute() or os.fspath(path) != os.fspath(path.resolve(strict=False)):
            raise EvidenceError(f"tool {tool['name']} path is not absolute and canonical")
        if not SHA256_RE.fullmatch(str(tool["sha256"])) or not SHA256_RE.fullmatch(
            str(tool["version_output_sha256"])
        ):
            raise EvidenceError(f"tool {tool['name']} identity is not SHA-256")
        if (
            not isinstance(tool["bytes"], int)
            or not 1 <= tool["bytes"] <= MAX_TOOL_BYTES
            or not isinstance(tool["uid"], int)
            or not isinstance(tool["gid"], int)
            or tool["mode"] != 0o755
            or not isinstance(tool["version_args"], list)
            or not tool["version_args"]
            or not all(isinstance(arg, str) and arg and "\0" not in arg for arg in tool["version_args"])
        ):
            raise EvidenceError(f"tool {tool['name']} metadata is invalid")


def validate_golden(golden: dict[str, Any], manifest_path: Path, manifest_bytes: bytes) -> None:
    if golden.get("schema") != GOLDEN_SCHEMA:
        raise EvidenceError("compiler-evidence golden schema changed")
    expected_relative = "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    if golden.get("tool_manifest_path") != expected_relative:
        raise EvidenceError("compiler-evidence tool manifest path changed")
    if golden.get("tool_manifest_sha256") != sha256_bytes(manifest_bytes):
        raise EvidenceError("compiler-evidence tool manifest was substituted")
    if manifest_path.as_posix().split("/tests/fixtures/compiler-evidence/")[-1] != "gfx942-mi300x-tools.json":
        raise EvidenceError("unexpected tool manifest location")
    expected_transition = {
        "transition_path":
            "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.json",
        "transition_signature_path":
            "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.sig",
        "transition_public_key_path":
            "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6-transition-v1.pub",
        "transition_signature_algorithm": "ed25519-sha512",
    }
    for field, expected in expected_transition.items():
        if golden.get(field) != expected:
            raise EvidenceError(f"compiler-evidence {field} changed")
    if not SHA256_RE.fullmatch(str(golden.get("transition_sha256"))):
        raise EvidenceError("compiler-evidence transition digest is invalid")


ED25519_Q = 2**255 - 19
ED25519_L = 2**252 + 27742317777372353535851937790883648493
ED25519_D = (-121665 * pow(121666, ED25519_Q - 2, ED25519_Q)) % ED25519_Q
ED25519_I = pow(2, (ED25519_Q - 1) // 4, ED25519_Q)


def ed25519_xrecover(y: int) -> int:
    xx = (y * y - 1) * pow(ED25519_D * y * y + 1, ED25519_Q - 2, ED25519_Q)
    x = pow(xx, (ED25519_Q + 3) // 8, ED25519_Q)
    if (x * x - xx) % ED25519_Q:
        x = x * ED25519_I % ED25519_Q
    return ED25519_Q - x if x & 1 else x


ED25519_BASE_Y = 4 * pow(5, ED25519_Q - 2, ED25519_Q) % ED25519_Q
ED25519_BASE = (ed25519_xrecover(ED25519_BASE_Y), ED25519_BASE_Y)
ED25519_IDENTITY = (0, 1)


def ed25519_add(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    x1, y1 = left
    x2, y2 = right
    common = ED25519_D * x1 * x2 * y1 * y2
    return (
        (x1 * y2 + x2 * y1) * pow(1 + common, ED25519_Q - 2, ED25519_Q)
        % ED25519_Q,
        (y1 * y2 + x1 * x2) * pow(1 - common, ED25519_Q - 2, ED25519_Q)
        % ED25519_Q,
    )


def ed25519_scalar(point: tuple[int, int], value: int) -> tuple[int, int]:
    result = ED25519_IDENTITY
    while value:
        if value & 1:
            result = ed25519_add(result, point)
        point = ed25519_add(point, point)
        value >>= 1
    return result


def ed25519_decode(encoded: bytes) -> tuple[int, int] | None:
    if len(encoded) != 32:
        return None
    value = int.from_bytes(encoded, "little")
    y = value & ((1 << 255) - 1)
    if y >= ED25519_Q:
        return None
    x = ed25519_xrecover(y)
    if (x & 1) != (value >> 255):
        x = ED25519_Q - x
    point = (x, y)
    if (
        (-x * x + y * y - 1 - ED25519_D * x * x * y * y) % ED25519_Q
        or ed25519_scalar(point, ED25519_L) != ED25519_IDENTITY
        or point == ED25519_IDENTITY
    ):
        return None
    return point


def verify_ed25519(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(signature) != 64:
        return False
    public = ed25519_decode(public_key)
    encoded_r = signature[:32]
    point_r = ed25519_decode(encoded_r)
    scalar_s = int.from_bytes(signature[32:], "little")
    if public is None or point_r is None or scalar_s >= ED25519_L:
        return False
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % ED25519_L
    return ed25519_scalar(ED25519_BASE, scalar_s) == ed25519_add(
        point_r, ed25519_scalar(public, challenge)
    )


def read_transition_fixture(source_root: Path, relative: str, limit: int) -> bytes:
    path = source_root / relative
    if path.resolve(strict=True) != path or path.is_symlink():
        raise EvidenceError(f"transition fixture path is not canonical: {relative}")
    value = path.read_bytes()
    if not value or len(value) > limit:
        raise EvidenceError(f"transition fixture size is invalid: {relative}")
    return value


def validate_signed_transition(
    source_root: Path,
    golden: dict[str, Any],
    manifest_bytes: bytes,
) -> dict[str, Any]:
    transition_bytes = read_transition_fixture(
        source_root, golden["transition_path"], MAX_CONFIG_BYTES
    )
    signature_text = read_transition_fixture(
        source_root, golden["transition_signature_path"], 256
    )
    public_text = read_transition_fixture(
        source_root, golden["transition_public_key_path"], 128
    )
    try:
        signature = bytes.fromhex(signature_text.decode("ascii").strip())
        public_key = bytes.fromhex(public_text.decode("ascii").strip())
        transition = json.loads(transition_bytes)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        raise EvidenceError("signed transition fixture encoding is invalid") from error
    if public_key != EXPECTED_TRANSITION_PUBLIC_KEY:
        raise EvidenceError("signed transition reviewer key changed")
    if sha256_bytes(transition_bytes) != golden["transition_sha256"]:
        raise EvidenceError("signed transition digest changed")
    if canonical_json(transition) != transition_bytes:
        raise EvidenceError("signed transition is not canonical JSON")
    if not verify_ed25519(public_key, transition_bytes, signature):
        raise EvidenceError("signed transition review signature is invalid")
    require_exact_keys(
        transition,
        {
            "schema",
            "authority",
            "claim",
            "source_commit",
            "source_tree",
            "tool_manifest_sha256",
            "runtime_manifest_sha256",
            "summary_sha256",
            "old",
            "new",
            "reproductions",
            "review",
        },
        "signed compiler transition",
    )
    if (
        transition["schema"] != "fe2o3-non-production-gfx942-compiler-transition-v1"
        or transition["authority"] != "none"
        or transition["claim"] != "non-production-exact-artifact-observation-only"
        or transition["source_commit"]
        != "2e9cbd9533032009a56154568d21bcfb61e52141"
        or transition["source_tree"]
        != "c057c7c38b557cb859cb27c420841b03f395d540"
        or transition["tool_manifest_sha256"] != sha256_bytes(manifest_bytes)
        or transition["runtime_manifest_sha256"]
        != "4bc10d21035d0088fcf5af4c3a09f33709eff250e83471ee05a1c572f6dc3ee7"
        or transition["summary_sha256"]
        != "76b1e0ebbd4822894296cfc5b29f014b4e98c4cf724663d4197ab99c14123829"
        or transition["review"]
        != {
            "decision": "accept-fixture-transition",
            "reviewer": "fe2o3-non-production-independent-review-fixture-v1",
            "signature_algorithm": "ed25519-sha512",
        }
    ):
        raise EvidenceError("signed compiler transition scope or review changed")
    expected_old = {
        "worker_build_identity":
            "fe2o3-worker-v1-sha256-234d22f9fb347c86495e7156e53ef8eab55e939d6514973a6df373aee12f77a9",
        "worker_executable_sha256":
            "764c7309af90b7c11b9a8ca14a84d449ab9f0a7f5eaf39b82b2d316ad4f3235a",
        "final_hsaco_sha256":
            "f5bc17f1950921e5bb8e7f64b576b7477cd82b4adffd1b6cfae3f6036c85844d",
        "final_hsaco_bytes": 9392,
        "golden_sha256":
            "fca08759e5b5dd44f53436149ee8241a06b63a77f816f5ca62c1f6a4318190ff",
    }
    if transition.get("old") != expected_old:
        raise EvidenceError("signed compiler transition does not bind the previous golden")
    new = transition.get("new")
    expected_new = {
        "worker_build_identity": golden["worker_build_identity"],
        "worker_executable_sha256": golden["worker_executable_sha256"],
        "final_hsaco_sha256": golden["hsaco_sha256"],
        "final_hsaco_bytes": golden["hsaco_bytes"],
        "raw_output_identity":
            "917e86272857301f1689ea7a0dfe91ea2f836981267fdddb69b433494bea53f1",
        "response_identity":
            "36bb783716dec69f765de11fe8286b2e82290b251a1bbc952e1375b084c28439",
    }
    if new != expected_new:
        raise EvidenceError("signed compiler transition does not bind the accepted golden")
    expected_reproductions = [
        {
            "run": 1,
            "manifest_sha256":
                "6f119a38cd1c6782df3dcbc215819dfb6182ff38e44be9a6ca241e7f53a57dfb",
            "compiler_transaction_sha256":
                "5dc27484c6d8b6b2c40c97c6c302b0d307e1e600d9fe1f9859c6c659cb5ac33f",
            "request_identity":
                "0e80e8034dea9555f31c028f8901dc7ff5dde2668226c374fd2a54e42dbbbadb",
            "finalization_identity":
                "899fd406ccb2492dd0f5a2b4df8348613dc45b56cd4f8d4b6d86081271a77e5a",
            "publication_identity":
                "bd5b8469081bda3cc131b735eb7b345a866ae1ec9509dae1ad948f17911ede6b",
        },
        {
            "run": 2,
            "manifest_sha256":
                "aad9d31e5d2ded8173bf129d9013658cd6d16ba5cc476a2ab2bd71b2aba56bf9",
            "compiler_transaction_sha256":
                "c42d51a91936d9ba763164204ae202aac4a5aedfb2ce724ed4b89a932b0d904e",
            "request_identity":
                "a378c1dbe461a967b0e1e1da5e00f8db8006ee7d8737a97a17d7003fd398b478",
            "finalization_identity":
                "d489a71ff350036fff84a784e2aee6249b6e74174f57f85d9b5caa70d1fc6bd4",
            "publication_identity":
                "92b76905756ffdf6345a7ac58690edeafe449fa8a1202c3473c00951dad19de9",
        },
    ]
    if transition.get("reproductions") != expected_reproductions:
        raise EvidenceError("signed transition does not bind two independent reproductions")
    return transition


@dataclass
class PinnedTool:
    record: dict[str, Any]
    file: Any
    identity: tuple[int, int, int, int, int, int, int]
    retained: RetainedFile
    executable: SealedExecutable

    @property
    def name(self) -> str:
        return str(self.record["name"])

    @property
    def path(self) -> Path:
        return Path(str(self.record["path"]))

    def revalidate(self) -> None:
        named = os.stat(self.path, follow_symlinks=False)
        opened = os.fstat(self.file.fileno())
        if exact_stat(named) != self.identity or exact_stat(opened) != self.identity:
            raise EvidenceError(f"pinned tool changed: {self.name}")
        if sha256_file(self.file, opened.st_size) != self.record["sha256"]:
            raise EvidenceError(f"pinned tool bytes changed: {self.name}")
        try:
            self.executable.revalidate()
        except HardeningError as error:
            raise EvidenceError(str(error)) from error

    def revalidate_identity(self) -> None:
        named = os.stat(self.path, follow_symlinks=False)
        opened = os.fstat(self.file.fileno())
        if exact_stat(named) != self.identity or exact_stat(opened) != self.identity:
            raise EvidenceError(f"pinned tool changed: {self.name}")
        try:
            self.executable.revalidate_identity()
        except HardeningError as error:
            raise EvidenceError(str(error)) from error

    def close(self) -> None:
        self.executable.close()
        self.retained.close()
        self.file.close()


def pin_tools(manifest: dict[str, Any]) -> dict[str, PinnedTool]:
    pinned: dict[str, PinnedTool] = {}
    try:
        for record in manifest["tools"]:
            path = Path(record["path"])
            if path.resolve(strict=True) != path or path.is_symlink():
                raise EvidenceError(f"tool path is not exact and canonical: {path}")
            named = os.stat(path, follow_symlinks=False)
            if not stat.S_ISREG(named.st_mode) or named.st_mode & 0o7777 != record["mode"]:
                raise EvidenceError(f"tool type or mode changed: {record['name']}")
            if (named.st_uid, named.st_gid, named.st_size) != (
                record["uid"],
                record["gid"],
                record["bytes"],
            ):
                raise EvidenceError(f"tool owner or size changed: {record['name']}")
            retained = RetainedFile.open(
                f"configured-tool:{record['name']}", path, require_executable=True
            )
            executable = SealedExecutable.from_retained(retained)
            file = open(path, "rb", buffering=0)
            opened = os.fstat(file.fileno())
            if exact_stat(named) != exact_stat(opened):
                executable.close()
                retained.close()
                file.close()
                raise EvidenceError(f"tool changed while opening: {record['name']}")
            tool = PinnedTool(record, file, exact_stat(opened), retained, executable)
            tool.revalidate()
            pinned[tool.name] = tool
    except BaseException:
        for tool in pinned.values():
            tool.close()
        raise
    return pinned


def create_allowlisted_path(directory: Path, manifest: dict[str, Any], tools: dict[str, PinnedTool]) -> None:
    directory.mkdir(mode=0o700)
    for alias, name in manifest["path_aliases"].items():
        target = tools[name].executable.proc_path
        os.symlink(target, directory / alias)
    actual = {entry.name: os.readlink(entry) for entry in directory.iterdir()}
    expected = {
        alias: tools[name].executable.proc_path
        for alias, name in manifest["path_aliases"].items()
    }
    if actual != expected:
        raise EvidenceError("allowlisted PATH materialization changed")


def run_environment(run: Path, path_dir: Path) -> dict[str, str]:
    return {
        "AR": os.fspath(path_dir / "ar"),
        "CARGO_HOME": os.fspath(run / "cargo-home"),
        "CARGO_BUILD_JOBS": "8",
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TARGET_DIR": os.fspath(run / "cargo-target"),
        "CC": os.fspath(path_dir / "cc"),
        "CXX": os.fspath(path_dir / "c++"),
        "HOME": os.fspath(run / "home"),
        "LANG": "C",
        "LC_ALL": "C",
        "LD": os.fspath(path_dir / "ld.lld"),
        "LD_LIBRARY_PATH": os.fspath(run / "rust-sysroot/lib"),
        "PATH": os.fspath(path_dir),
        "RANLIB": os.fspath(path_dir / "ranlib"),
        "RUSTC": os.fspath(path_dir / "rustc"),
        "RUSTFLAGS": f"--sysroot={run / 'rust-sysroot'}",
        "RUSTUP_TOOLCHAIN": EXPECTED_RUST_TOOLCHAIN,
        "SOURCE_DATE_EPOCH": "0",
        "TERM": "dumb",
        "TMPDIR": os.fspath(run / "tmp"),
        "TZ": "UTC",
    }


def clean_environment(run: Path, path_dir: Path, manifest: dict[str, Any]) -> dict[str, str]:
    home = run / "home"
    temp = run / "tmp"
    home.mkdir(mode=0o700)
    temp.mkdir(mode=0o700)
    target = run / "cargo-target"
    target.mkdir(mode=0o700)
    if any(target.iterdir()):
        raise EvidenceError("CARGO_TARGET_DIR was not empty at run start")
    return run_environment(run, path_dir)


def run_command(
    arguments: list[str],
    cwd: Path,
    environment: dict[str, str],
    tools: dict[str, PinnedTool],
    supervisor: Supervisor,
    *,
    capture: bool = False,
    executable: SealedExecutable | None = None,
    extra_inherited_fds: tuple[int, ...] = (),
    limits: CommandLimits = CommandLimits(),
    writable_paths: tuple[Path, ...] = (),
    readable_paths: tuple[Path, ...] | None = None,
    readable_roots: tuple[Path, ...] = (),
) -> Any:
    for tool in tools.values():
        tool.revalidate_identity()
    selected = executable
    command = list(arguments)
    if selected is None:
        first = Path(arguments[0])
        matched = next(
            (
                tool
                for tool in tools.values()
                if first == tool.path
                or os.fspath(first) == tool.executable.proc_path
                or os.fspath(first) == f"/proc/self/fd/{tool.executable.fd}"
            ),
            None,
        )
        if matched is None:
            raise EvidenceError(f"command executable is not retained: {arguments[0]}")
        if matched.name == "ldd":
            selected = tools["shell"].executable
            command = ["bash", matched.executable.proc_path, *arguments[1:]]
        else:
            selected = matched.executable
    inherited = [tool.executable.fd for tool in tools.values()]
    inherited.extend(extra_inherited_fds)
    try:
        completed = supervisor.run(
            selected,
            command,
            cwd,
            environment,
            limits=limits,
            inherited_fds=inherited,
            writable_paths=writable_paths,
            readable_paths=readable_paths,
            readable_roots=readable_roots,
        )
    except HardeningError as error:
        raise EvidenceError(str(error)) from error
    for tool in tools.values():
        tool.revalidate_identity()
    if completed.returncode != 0:
        detail = completed.stdout + completed.stderr
        raise EvidenceError(
            f"command failed ({completed.returncode}): {' '.join(arguments)}\n"
            + detail[-8192:].decode("utf-8", "replace")
        )
    return completed


def version_and_runtime_manifest(
    manifest: dict[str, Any],
    tools: dict[str, PinnedTool],
    path_dir: Path,
    rust_library_path: Path,
    supervisor: Supervisor,
    configured_runtime: list[dict[str, Any]],
) -> dict[str, Any]:
    environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "LD_LIBRARY_PATH": os.fspath(rust_library_path),
        "PATH": os.fspath(path_dir),
        "TZ": "UTC",
    }
    closure_environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.fspath(path_dir),
        "TZ": "UTC",
    }
    observed_tools: list[dict[str, Any]] = []
    runtime_paths: set[Path] = set()
    ldd = tools["ldd"].path
    for name in TOOL_NAMES:
        tool = tools[name]
        completed = run_command(
            [os.fspath(tool.path), *tool.record["version_args"]],
            Path("/"),
            environment,
            tools,
            supervisor,
            capture=True,
        )
        output = (completed.stdout or b"") + (completed.stderr or b"")
        if not output or len(output) > MAX_VERSION_BYTES:
            raise EvidenceError(f"version output is invalid: {name}")
        output_digest = sha256_bytes(output)
        if output_digest != tool.record["version_output_sha256"]:
            raise EvidenceError(
                f"version output changed: {name}: expected "
                f"{tool.record['version_output_sha256']}, observed {output_digest}; "
                f"output={output.decode('utf-8', 'replace')!r}"
            )
        observed_tools.append(
            {
                "name": name,
                "path": os.fspath(tool.path),
                "sha256": tool.record["sha256"],
                "bytes": tool.record["bytes"],
                "uid": tool.record["uid"],
                "gid": tool.record["gid"],
                "mode": tool.record["mode"],
                "version_args": tool.record["version_args"],
                "version_output": output.decode("utf-8", "strict"),
                "version_output_sha256": output_digest,
                "stat": stat_record(os.fstat(tool.file.fileno())),
            }
        )
        data = os.pread(tool.file.fileno(), 128, 0)
        if data.startswith(b"\x7fELF"):
            closure = run_command(
                [os.fspath(ldd), os.fspath(tool.path)],
                Path("/"),
                closure_environment,
                tools,
                supervisor,
                capture=True,
            )
            for line in (closure.stdout or b"").decode("utf-8", "strict").splitlines():
                match = re.search(r"=>\s+(/\S+)", line)
                direct = re.match(r"\s*(/\S+)\s+\(", line)
                candidate = match.group(1) if match else direct.group(1) if direct else None
                if candidate is not None:
                    runtime_paths.add(Path(candidate).resolve(strict=True))
        elif data.startswith(b"#!"):
            interpreter = data.splitlines()[0][2:].decode("ascii").split()[0]
            runtime_paths.add(Path(interpreter).resolve(strict=True))
        else:
            raise EvidenceError(f"tool is neither ELF nor a script: {name}")
    if not runtime_paths or len(runtime_paths) > MAX_RUNTIME_FILES:
        raise EvidenceError("runtime closure file count is invalid")
    runtime: list[dict[str, Any]] = []
    total = 0
    for path in sorted(runtime_paths, key=os.fspath):
        if path.is_symlink() or path.resolve(strict=True) != path:
            raise EvidenceError(f"runtime path is not canonical: {path}")
        metadata = os.stat(path, follow_symlinks=False)
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size < 1:
            raise EvidenceError(f"runtime closure member is invalid: {path}")
        total += metadata.st_size
        if total > MAX_RUNTIME_BYTES:
            raise EvidenceError("runtime closure exceeds byte bound")
        with open(path, "rb", buffering=0) as file:
            digest = sha256_file(file, metadata.st_size)
        runtime.append(
            {
                "path": os.fspath(path),
                "sha256": digest,
                "bytes": metadata.st_size,
                "uid": metadata.st_uid,
                "gid": metadata.st_gid,
                "mode": metadata.st_mode & 0o7777,
                "device": metadata.st_dev,
                "inode": metadata.st_ino,
                "mtime_ns": metadata.st_mtime_ns,
                "ctime_ns": metadata.st_ctime_ns,
            }
        )
    runtime_digest = sha256_bytes(canonical_json(runtime))
    if runtime_digest != manifest["runtime_manifest_sha256"]:
        raise EvidenceError(
            "pinned runtime closure changed: "
            f"expected {manifest['runtime_manifest_sha256']}, observed {runtime_digest}"
        )
    if runtime != configured_runtime:
        raise EvidenceError("discovered tool loader/DSO closure differs from retained fixture")
    observed = {
        "schema": "fe2o3-observed-gfx942-tool-runtime-manifest-v1",
        "configured_tool_manifest_sha256": sha256_bytes(canonical_json(manifest)),
        "observed_tool_runtime_closure_sha256": runtime_digest,
        "tools": observed_tools,
        "runtime": runtime,
    }
    return observed


def require_absent_output(path: Path, label: str) -> None:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise EvidenceError(f"{label} must be an absent absolute path")
    parent = path.parent.resolve(strict=True)
    if parent / path.name != path:
        raise EvidenceError(f"{label} parent or spelling is not canonical")


def git_clean(
    repo: Path,
    environment: dict[str, str],
    tools: dict[str, PinnedTool],
    supervisor: Supervisor,
) -> tuple[str, str, list[str]]:
    git = os.fspath(tools["git"].path)
    status = run_command(
        [git, "-c", "core.hooksPath=/dev/null", "status", "--porcelain=v1", "--untracked-files=all"],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    ).stdout
    if status:
        raise EvidenceError("compiler evidence requires a clean committed tree")
    commit = run_command(
        [git, "rev-parse", "HEAD"], repo, environment, tools, supervisor, capture=True
    ).stdout.decode("ascii").strip()
    tree = run_command(
        [git, "rev-parse", "HEAD^{tree}"], repo, environment, tools, supervisor, capture=True
    ).stdout.decode("ascii").strip()
    listed = run_command(
        [git, "ls-files", "-z"], repo, environment, tools, supervisor, capture=True
    ).stdout
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9a-f]{40}", tree):
        raise EvidenceError("source commit identity is invalid")
    paths = [item.decode("utf-8", "strict") for item in listed.split(b"\0") if item]
    if not paths or len(paths) != len(set(paths)):
        raise EvidenceError("tracked source list is empty or duplicated")
    return commit, tree, paths


def measure_generated_executable(path: Path, parent: Path, label: str) -> tuple[Any, dict[str, Any]]:
    if path.is_symlink() or path.resolve(strict=True) != path:
        raise EvidenceError(f"{label} path is not absolute and canonical")
    if parent.resolve(strict=True) not in path.parents:
        raise EvidenceError(f"{label} escaped its isolated run root")
    named = os.stat(path, follow_symlinks=False)
    if not stat.S_ISREG(named.st_mode) or not named.st_mode & 0o111:
        raise EvidenceError(f"{label} is not an executable regular file")
    file = open(path, "rb", buffering=0)
    opened = os.fstat(file.fileno())
    if exact_stat(named) != exact_stat(opened):
        file.close()
        raise EvidenceError(f"{label} changed while it was opened")
    return file, {
        "label": label,
        "path": os.fspath(path),
        "sha256": sha256_file(file, opened.st_size),
        "stat": stat_record(opened),
    }


def revalidate_generated_executable(file: Any, record: dict[str, Any]) -> None:
    path = Path(record["path"])
    named = os.stat(path, follow_symlinks=False)
    opened = os.fstat(file.fileno())
    if stat_record(named) != record["stat"] or stat_record(opened) != record["stat"]:
        raise EvidenceError(f"generated executable stat identity changed: {record['label']}")
    if sha256_file(file, opened.st_size) != record["sha256"]:
        raise EvidenceError(f"generated executable content changed: {record['label']}")


def reject_cross_run_reuse(first: dict[str, Any], second: dict[str, Any]) -> None:
    first_records = first.get("executables")
    second_records = second.get("executables")
    if (
        not isinstance(first_records, list)
        or not isinstance(second_records, list)
        or len(first_records) != len(second_records)
        or not first_records
    ):
        raise EvidenceError("cross-run executable closures are not comparable")
    def keyed(records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
        labels = [record.get("label") for record in records]
        if (
            not all(isinstance(label, str) for label in labels)
            or labels != sorted(labels)
            or len(labels) != len(set(labels))
        ):
            raise EvidenceError("cross-run executable labels are reordered or duplicated")
        return {record["label"]: record for record in records}

    first_keyed = keyed(first_records)
    second_keyed = keyed(second_records)
    if set(first_keyed) != set(second_keyed):
        raise EvidenceError("cross-run executable roles differ")
    for label in first_keyed:
        left = first_keyed[label]
        right = second_keyed[label]
        left_stat = left["stat"]
        right_stat = right["stat"]
        if left["path"] == right["path"] or (
            left_stat["device"] == right_stat["device"]
            and left_stat["inode"] == right_stat["inode"]
        ):
            raise EvidenceError("run B reused an executable from run A")
        if (left_stat["bytes"], left_stat["mode"]) != (
            right_stat["bytes"],
            right_stat["mode"],
        ):
            raise EvidenceError(f"cross-run executable shape differs: {label}")
        if label != "Rust integration test" and left["sha256"] != right["sha256"]:
            raise EvidenceError(f"cross-run executable content differs: {label}")


def compare_generated_build_snapshots(
    first: SnapshotClosure,
    second: SnapshotClosure,
    *,
    cargo_labels: bool,
) -> None:
    cargo_hash = re.compile(r"(?<=-)[0-9a-f]{16}(?=/|\.|$)")

    def keyed(
        closure: SnapshotClosure,
    ) -> tuple[dict[str, list[tuple[int, int]]], set[tuple[int, int]]]:
        records = closure.manifest.get("files")
        if not isinstance(records, list) or len(records) != len(closure.source_files):
            raise EvidenceError("generated build manifest is incomplete")
        labels = [record.get("label") for record in records]
        if labels != sorted(labels) or len(labels) != len(set(labels)):
            raise EvidenceError("generated build manifest labels are reordered or duplicated")
        shape: dict[str, list[tuple[int, int]]] = {}
        origins: set[tuple[int, int]] = set()
        for record, retained in zip(records, closure.source_files, strict=True):
            label = record["label"]
            semantic = cargo_hash.sub("<cargo-hash>", label) if cargo_labels else label
            info = record["stat"]
            shape.setdefault(semantic, []).append((info["bytes"], info["mode"]))
            origin = os.fstat(retained.fd)
            identity = (origin.st_dev, origin.st_ino)
            origins.add(identity)
        for values in shape.values():
            values.sort()
        return shape, origins

    first_shape, first_origins = keyed(first)
    second_shape, second_origins = keyed(second)
    if first_shape != second_shape:
        raise EvidenceError("independent generated build closures differ by label or shape")
    if first_origins & second_origins:
        raise EvidenceError("run B reused a generated build artifact from run A")


def registry_paths(lock_path: Path, registry_root: Path) -> list[str]:
    lock = tomllib.loads(lock_path.read_text("utf-8"))
    packages = lock.get("package")
    if not isinstance(packages, list):
        raise EvidenceError("Cargo.lock has no package closure")
    paths: list[str] = []
    for package in packages:
        source = package.get("source")
        if not isinstance(source, str) or not source.startswith("registry+"):
            continue
        name = package.get("name")
        version = package.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            raise EvidenceError("Cargo.lock registry package identity is invalid")
        package_root = registry_root / f"{name}-{version}"
        if not package_root.exists():
            continue
        if package_root.resolve(strict=True) != package_root or package_root.is_symlink():
            raise EvidenceError(f"registry package is not canonical: {name}-{version}")
        members = sorted(path for path in package_root.rglob("*") if path.is_file())
        if not members:
            raise EvidenceError(f"registry package closure is incomplete: {name}-{version}")
        for member in members:
            if member.is_symlink() or member.resolve(strict=True) != member:
                raise EvidenceError(f"registry package member is not canonical: {member}")
            paths.append(member.relative_to(registry_root).as_posix())
    if not paths:
        raise EvidenceError("Cargo.lock selected no registry package sources")
    return sorted(set(paths))


def materialize_vendor_checksums(lock_path: Path, vendor: Path, index: int) -> RetainedClosure:
    packages = tomllib.loads(lock_path.read_text("utf-8")).get("package")
    if not isinstance(packages, list):
        raise EvidenceError("Cargo.lock has no package closure")
    generated: list[tuple[str, Path]] = []
    for package in packages:
        source = package.get("source")
        if not isinstance(source, str) or not source.startswith("registry+"):
            continue
        name = package.get("name")
        version = package.get("version")
        checksum = package.get("checksum")
        if (
            not isinstance(name, str)
            or not isinstance(version, str)
            or not isinstance(checksum, str)
            or not SHA256_RE.fullmatch(checksum)
        ):
            raise EvidenceError("Cargo.lock registry checksum identity is invalid")
        package_root = vendor / f"{name}-{version}"
        if not package_root.exists():
            continue
        files: dict[str, str] = {}
        for member in sorted(path for path in package_root.rglob("*") if path.is_file()):
            relative = member.relative_to(package_root).as_posix()
            if relative == ".cargo-checksum.json":
                raise EvidenceError("vendor checksum input was preexisting")
            files[relative] = sha256_bytes(member.read_bytes())
        package_root.chmod(0o755)
        checksum_path = package_root / ".cargo-checksum.json"
        checksum_path.write_bytes(canonical_json({"files": files, "package": checksum}))
        checksum_path.chmod(0o444)
        package_root.chmod(0o555)
        generated.append((f"vendor-checksum:{name}-{version}", checksum_path))
    if not generated:
        raise EvidenceError("no vendor checksum manifests were generated")
    return capture_retained_closure(
        f"run-{index}-vendor-generated-inputs",
        generated,
        {"cargo_lock_sha256": sha256_bytes(lock_path.read_bytes())},
    )


def prepare_run_closures(
    index: int,
    repo: Path,
    run_root: Path,
    evidence_root: Path,
    tracked_paths: list[str],
    commit: str,
    tree: str,
    manifest: dict[str, Any],
) -> tuple[
    Path,
    SnapshotClosure,
    SnapshotClosure,
    SnapshotClosure,
    RetainedClosure,
    RetainedFile,
]:
    run = run_root / f"run-{index}"
    if run.exists() or run.is_symlink():
        raise EvidenceError(f"run {index} work root was not absent")
    run.mkdir(mode=0o700)
    source = capture_snapshot(
        f"run-{index}-repository",
        repo,
        run / "source",
        tracked_paths,
        {"git_commit": commit, "git_tree": tree},
    )
    cargo_home = run / "cargo-home"
    cargo_home.mkdir(mode=0o700)
    registry_root = Path(str(manifest["registry_source"]))
    registry = capture_snapshot(
        f"run-{index}-registry",
        registry_root,
        cargo_home / "vendor",
        registry_paths(source.root / "Cargo.lock", registry_root),
        {
            "cargo_lock_sha256": sha256_bytes((source.root / "Cargo.lock").read_bytes()),
            "registry_source": os.fspath(registry_root),
        },
    )
    vendor_generated = materialize_vendor_checksums(
        source.root / "Cargo.lock", cargo_home / "vendor", index
    )
    rust_root = Path(
        "/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu"
    )
    if rust_root.resolve(strict=True) != rust_root:
        raise EvidenceError("nightly-2026-04-03 sysroot is not canonical")
    rust_paths = sorted(
        member.relative_to(rust_root).as_posix()
        for member in rust_root.rglob("*")
        if member.is_file()
    )
    rust = capture_snapshot(
        f"run-{index}-rust-sysroot",
        rust_root,
        run / "rust-sysroot",
        rust_paths,
        {
            "rust_toolchain": EXPECTED_RUST_TOOLCHAIN,
            "rustc_sha256": next(
                tool["sha256"] for tool in manifest["tools"] if tool["name"] == "rustc"
            ),
        },
    )
    cargo_config = cargo_home / "config.toml"
    cargo_config.write_text(
        "[net]\noffline = true\n\n"
        "[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n"
        "[source.vendored-sources]\n"
        f'directory = "{cargo_home / "vendor"}"\n',
        encoding="ascii",
    )
    cargo_config.chmod(0o444)
    config = RetainedFile.open(
        f"run-{index}-cargo-config", cargo_config, require_read_only=True
    )
    output_dir = evidence_root / f"run-{index}"
    output_dir.mkdir(mode=0o700)
    (output_dir / "repository-source-manifest.json").write_bytes(
        canonical_json(source.manifest)
    )
    (output_dir / "cargo-registry-manifest.json").write_bytes(
        canonical_json(registry.manifest)
    )
    (output_dir / "rust-sysroot-manifest.json").write_bytes(
        canonical_json(rust.manifest)
    )
    (output_dir / "cargo-vendor-generated-manifest.json").write_bytes(
        canonical_json(vendor_generated.manifest)
    )
    return run, source, registry, rust, vendor_generated, config


def build_provider_members() -> tuple[list[tuple[str, Path]], dict[str, Any]]:
    roots = (
        ("llvm-include", Path("/opt/rocm-7.2.4/lib/llvm/include")),
        ("llvm-cmake", Path("/opt/rocm-7.2.4/lib/llvm/lib/cmake")),
        ("clang-resource", Path("/opt/rocm-7.2.4/lib/llvm/lib/clang/22/include")),
        (
            "device-bitcode",
            Path("/opt/rocm-7.2.4/lib/llvm/lib/clang/22/lib/amdgcn/bitcode"),
        ),
        ("rocm-info", Path("/opt/rocm-7.2.4/.info")),
        ("rocm-include", Path("/opt/rocm-7.2.4/include")),
        ("gcc-cxx", Path("/usr/include/c++/13")),
        (
            "gcc-cxx-target",
            Path("/usr/include/x86_64-linux-gnu/c++/13"),
        ),
        ("system-local-include", Path("/usr/local/include")),
        ("system-target-include", Path("/usr/include/x86_64-linux-gnu")),
        ("system-include", Path("/usr/include")),
    )
    members: dict[Path, str] = {}
    selected_roots = []
    for prefix, root in roots:
        if root.resolve(strict=True) != root or root.is_symlink():
            raise EvidenceError(f"build-provider root is not canonical: {root}")
        selected_roots.append(
            {
                "label_prefix": prefix,
                "path": os.fspath(root),
                "reason": (
                    "exact clang++ include-search root"
                    if prefix.startswith(("gcc-cxx", "system-"))
                    else "LLVM/LLD Worker compile or link input root"
                ),
            }
        )
        for member in sorted(root.rglob("*")):
            if member.is_file():
                retained_path = member.resolve(strict=True)
                if retained_path.is_symlink() or not retained_path.is_file():
                    raise EvidenceError(
                        f"build-provider member did not resolve to a file: {member}"
                    )
                members.setdefault(
                    retained_path,
                    f"{prefix}:{member.relative_to(root).as_posix()}",
                )
    library_root = Path("/opt/rocm-7.2.4/lib/llvm/lib")
    for member in sorted(library_root.iterdir()):
        if member.is_file() and not member.is_symlink():
            members.setdefault(member, f"llvm-library:{member.name}")
    excluded_large_dsos = []
    rocm_library_root = Path("/opt/rocm-7.2.4/lib")
    for member in sorted(rocm_library_root.rglob("*")):
        if (
            member.is_file()
            and not member.is_symlink()
            and member.stat().st_size > 512 * 1024 * 1024
            and (member.name.endswith(".so") or ".so." in member.name)
        ):
            excluded_large_dsos.append(
                {
                    "path": os.fspath(member),
                    "bytes": member.stat().st_size,
                    "reason": (
                        "not a compiler input; executable ELF/DSO and dlopen "
                        "closures are captured separately and fail closed"
                    ),
                }
            )
    policy = {
        "schema": "fe2o3-gfx942-build-provider-selection-v1",
        "selected_roots": selected_roots,
        "llvm_library_selection": {
            "path": os.fspath(library_root),
            "scope": "all direct regular files",
            "reason": "CMake-imported LLVM/LLD link inputs",
        },
        "excluded_large_rocm_dsos": excluded_large_dsos,
        "runtime_separation": (
            "non-compiler ROCm DSOs are accepted only through the separately "
            "retained executable runtime and dlopen closure"
        ),
    }
    return (
        sorted(
            ((label, path) for path, label in members.items()),
            key=lambda item: item[0],
        ),
        policy,
    )


def generated_cargo_paths(target: Path) -> list[str]:
    debug = target / "debug"
    build = debug / "build"
    deps = debug / "deps"
    if not build.is_dir() or not deps.is_dir():
        raise EvidenceError("Cargo generated-artifact roots are absent")
    selected: set[Path] = set()
    for member in build.rglob("*"):
        if member.is_file() and not member.is_symlink():
            selected.add(member)
    for member in deps.glob("*.so"):
        if member.is_file() and not member.is_symlink():
            selected.add(member)
    relative = sorted(member.relative_to(target).as_posix() for member in selected)
    if (
        not relative
        or not any("/build-script-build" in name for name in relative)
        or not any(name.endswith(".so") for name in relative)
    ):
        raise EvidenceError("Cargo build-script/proc-macro closure is incomplete")
    return relative


def capture_generated_build_closures(
    index: int,
    run: Path,
    worker_build: Path,
    evidence_root: Path,
) -> tuple[SnapshotClosure, SnapshotClosure]:
    worker_paths = sorted(
        member.relative_to(worker_build).as_posix()
        for member in worker_build.rglob("*")
        if (
            member.is_file()
            and not member.is_symlink()
            and "Testing/Temporary" not in member.relative_to(worker_build).as_posix()
            and member.relative_to(worker_build).as_posix() != ".ninja_log"
        )
    )
    if not worker_paths or not any(name.endswith(".o") for name in worker_paths):
        raise EvidenceError("generated Worker build closure is incomplete")
    worker = capture_snapshot(
        f"run-{index}-generated-worker-build",
        worker_build,
        run / "captured-worker-build",
        worker_paths,
        {
            "kind": "generated-cmake-worker-build",
            "source_root": os.fspath(worker_build),
            "excluded": [
                {
                    "path_prefix": "Testing/Temporary/",
                    "reason": "transient CTest timing/log output, not a build input",
                },
                {
                    "path": ".ninja_log",
                    "reason": "transient Ninja timing log, not a build input",
                },
            ],
        },
        allow_source_hardlinks=True,
    )
    cargo_target = run / "cargo-target"
    try:
        cargo = capture_snapshot(
            f"run-{index}-generated-cargo-build",
            cargo_target,
            run / "captured-cargo-build",
            generated_cargo_paths(cargo_target),
            {
                "kind": "generated-cargo-build-script-and-proc-macro-artifacts",
                "source_root": os.fspath(cargo_target),
            },
            allow_source_hardlinks=True,
        )
    except BaseException:
        worker.close()
        raise
    output = evidence_root / f"run-{index}"
    (output / "generated-worker-build-manifest.json").write_bytes(
        canonical_json(worker.manifest)
    )
    (output / "generated-cargo-build-manifest.json").write_bytes(
        canonical_json(cargo.manifest)
    )
    return worker, cargo


def build_and_generate(
    index: int,
    repo: Path,
    run: Path,
    evidence_root: Path,
    manifest: dict[str, Any],
    tools: dict[str, PinnedTool],
    golden: dict[str, Any],
    supervisor: Supervisor,
    *,
    observe_candidate: bool,
) -> tuple[
    Path,
    dict[str, Any],
    RetainedFile,
    RetainedFile,
    SnapshotClosure,
    SnapshotClosure,
]:
    if run != run.parent / f"run-{index}" or not run.is_dir():
        raise EvidenceError("run root does not match its independent index")
    path_dir = run / "tool-path"
    create_allowlisted_path(path_dir, manifest, tools)
    environment = clean_environment(run, path_dir, manifest)
    environment["RUSTC"] = f"/proc/self/fd/{tools['rustc'].executable.fd}"
    worker_build = run / "worker-build"
    if worker_build.exists():
        raise EvidenceError("Worker build directory was not absent at run start")
    cmake = os.fspath(tools["cmake"].path)
    compiler_launcher = ";".join(
        (
            tools["python"].executable.proc_path,
            os.fspath(repo / "scripts/retained_tool_launcher.py"),
            "--expected",
            os.fspath(tools["cxx"].path),
            "--sha256",
            tools["cxx"].record["sha256"],
        )
    )
    run_command(
        [
            cmake,
            "-S",
            os.fspath(repo / "tools/fe2o3-llvm-link-worker"),
            "-B",
            os.fspath(worker_build),
            "-G",
            "Ninja",
            "-DLLVM_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/llvm",
            "-DLLD_DIR=/opt/rocm-7.2.4/lib/llvm/lib/cmake/lld",
            f"-DCMAKE_MAKE_PROGRAM={path_dir / 'ninja'}",
            f"-DCMAKE_CXX_COMPILER={tools['cxx'].path}",
            "-DCMAKE_CXX_COMPILER_ARG1=--driver-mode=g++",
            f"-DCMAKE_CXX_COMPILER_LAUNCHER={compiler_launcher}",
            f"-DCMAKE_CXX_LINKER_LAUNCHER={compiler_launcher}",
            f"-DCMAKE_LINKER={path_dir / 'ld.lld'}",
            f"-DCMAKE_AR={path_dir / 'ar'}",
            f"-DCMAKE_RANLIB={path_dir / 'ranlib'}",
            "-DFE2O3_PINNED_LLVM_VERSION=22.0.0git",
            "-DFE2O3_LLVM_BUILD_ID_FILE=/opt/rocm-7.2.4/.info/version",
            "-DFE2O3_EXPECTED_LLVM_BUILD_ID=7.2.4",
            "-DFE2O3_GFX942_DEVICE_LIB_DIR="
            "/opt/rocm-7.2.4/lib/llvm/lib/clang/22/lib/amdgcn/bitcode",
            "-DBUILD_TESTING=ON",
            "-DCMAKE_BUILD_TYPE=Release",
        ],
        repo,
        environment,
        tools,
        supervisor,
    )
    run_command(
        [cmake, "--build", os.fspath(worker_build), "--parallel", "8"],
        repo,
        environment,
        tools,
        supervisor,
    )
    worker = worker_build / "fe2o3-llvm-link-worker"
    generated: list[tuple[Any, dict[str, Any]]] = []
    worker_file, worker_record = measure_generated_executable(
        worker, worker_build, "Worker"
    )
    generated.append((worker_file, worker_record))
    if not observe_candidate and worker_record["sha256"] != golden["worker_executable_sha256"]:
        raise EvidenceError(f"run {index} Worker executable digest changed")
    ctest_listing = run_command(
        [os.fspath(tools["ctest"].path), "--test-dir", os.fspath(worker_build), "--show-only=json-v1"],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    )
    listing = json.loads((ctest_listing.stdout or b"").decode("utf-8"))
    tests = listing.get("tests", [])
    expected_native_tests = {
        "fe2o3-worker-codec-tests",
        "fe2o3-worker-pipeline-tests",
        "fe2o3-worker-device-library-policy-tests",
    }
    if not isinstance(tests, list) or {test.get("name") for test in tests} != expected_native_tests:
        raise EvidenceError("CTest did not list the exact three native Worker tests")
    native_tests: list[tuple[Path, RetainedFile, SealedExecutable, list[str]]] = []
    native_results = []
    for test in tests:
        command = test.get("command", [])
        if not command:
            raise EvidenceError("CTest listed a test without an executable")
        executable = Path(command[0])
        file, record = measure_generated_executable(
            executable, worker_build, f"native test {test.get('name')}"
        )
        generated.append((file, record))
        retained = RetainedFile.open(record["label"], executable, require_executable=True)
        sealed = SealedExecutable.from_retained(retained)
        direct = run_command(
            [os.fspath(executable), *command[1:]],
            worker_build,
            environment,
            tools,
            supervisor,
            executable=sealed,
        )
        native_results.append(
            {
                "name": test["name"],
                "direct_returncode": direct.returncode,
                "executable_sha256": sealed.sha256,
            }
        )
        native_tests.append((executable, retained, sealed, command))
    ctest_file = worker_build / "CTestTestfile.cmake"
    ctest_original = ctest_file.read_bytes()
    if not ctest_original or len(ctest_original) > 1024 * 1024:
        raise EvidenceError("generated CTest command file is empty or unbounded")
    ctest_fd = os.open(ctest_file, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    ctest_stat = os.fstat(ctest_fd)
    ctest_identity = (
        ctest_stat.st_dev,
        ctest_stat.st_ino,
        ctest_stat.st_mode,
        ctest_stat.st_nlink,
        ctest_stat.st_uid,
        ctest_stat.st_gid,
    )
    ctest_rewritten = ctest_original
    for executable, _, sealed, _ in native_tests:
        original = os.fsencode(executable)
        if ctest_rewritten.count(original) != 1:
            raise EvidenceError("CTest command did not contain one exact native executable path")
        ctest_rewritten = ctest_rewritten.replace(original, os.fsencode(sealed.proc_path))
    ctest_file.write_bytes(ctest_rewritten)
    try:
        ctest = run_command(
            [
                os.fspath(tools["ctest"].path),
                "--test-dir",
                os.fspath(worker_build),
                "--output-on-failure",
            ],
            repo,
            environment,
            tools,
            supervisor,
            capture=True,
            extra_inherited_fds=tuple(item[2].fd for item in native_tests),
        )
        ctest_output = (ctest.stdout + ctest.stderr).decode("utf-8", "replace")
        if "100% tests passed, 0 tests failed out of 3" not in ctest_output:
            raise EvidenceError("CTest did not report an exact 3/3 pass")
    finally:
        ctest_file.write_bytes(ctest_original)
        if ctest_file.read_bytes() != ctest_original:
            raise EvidenceError("generated CTest command file was not exactly restored")
        restored = os.stat(ctest_file, follow_symlinks=False)
        if (
            restored.st_dev,
            restored.st_ino,
            restored.st_mode,
            restored.st_nlink,
            restored.st_uid,
            restored.st_gid,
        ) != ctest_identity or os.pread(ctest_fd, restored.st_size, 0) != ctest_original:
            raise EvidenceError("generated CTest command file identity changed")
        os.close(ctest_fd)
        for _, retained, sealed, _ in reversed(native_tests):
            sealed.revalidate()
            retained.revalidate()
            sealed.close()
            retained.close()
    for file, record in generated:
        revalidate_generated_executable(file, record)
    build_identity = (worker_build / "fe2o3-worker-build-id.txt").read_text("ascii").strip()
    if not observe_candidate and build_identity != golden["worker_build_identity"]:
        raise EvidenceError(f"run {index} Worker build identity changed")
    output_dir = evidence_root / f"run-{index}"
    if not output_dir.is_dir():
        raise EvidenceError("run evidence root was not prepared independently")
    output = output_dir / "alpha-zeta-cov6.hsaco"
    transaction_output = output_dir / "compiler-transaction-observation.json"
    generation_environment = dict(environment)
    generation_environment.update(
        {
            "CFLAGS": "-resource-dir=/opt/rocm-7.2.4/lib/llvm/lib/clang/22",
            "CXXFLAGS": "-resource-dir=/opt/rocm-7.2.4/lib/llvm/lib/clang/22",
            "FE2O3_GFX942_ALPHA_ZETA_OUTPUT": os.fspath(output),
            "FE2O3_LLVM_BUILD_ID": EXPECTED_LLVM_BUILD,
            "FE2O3_LLVM_LINK_WORKER": os.fspath(worker),
            "FE2O3_LLVM_LINK_WORKER_BUILD_ID": build_identity,
            "FE2O3_NON_PRODUCTION_COMPILER_REPRODUCTION_RECORD_V1": os.fspath(
                transaction_output
            ),
            "FE2O3_NON_PRODUCTION_COMPILER_EVIDENCE_SCOPE_V1":
                "exact-artifact-observation-only",
            "FE2O3_NON_PRODUCTION_COMPILER_REPRODUCTION_V1":
                "gfx942-alpha-zeta-cov6-v1",
            "ROCM_PATH": "/opt/rocm-7.2.4",
        }
    )
    if observe_candidate:
        generation_environment[
            "FE2O3_NON_PRODUCTION_COMPILER_TRANSITION_OBSERVATION_V1"
        ] = "observe-without-golden-acceptance"
    build_test = run_command(
        [
            os.fspath(tools["cargo"].path),
            "test",
            "--offline",
            "--locked",
            "-p",
            "rustc-codegen-fe2o3",
            "--test",
            "kernel_ir_codegen",
            "--no-run",
            "--message-format=json",
        ],
        repo,
        environment,
        tools,
        supervisor,
        capture=True,
    )
    test_executables: list[Path] = []
    for line in (build_test.stdout or b"").splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        executable = message.get("executable")
        if target.get("name") == "kernel_ir_codegen" and target.get("kind") == ["test"] and executable:
            test_executables.append(Path(executable))
    if len(test_executables) != 1:
        raise EvidenceError(f"run {index} did not produce exactly one integration-test executable")
    test_file, test_record = measure_generated_executable(
        test_executables[0], run / "cargo-target", "Rust integration test"
    )
    generated.append((test_file, test_record))
    retained_worker = RetainedFile.open(
        f"run-{index}-worker-exec", worker, require_executable=True
    )
    sealed_worker = SealedExecutable.from_retained(retained_worker)
    retained_test = RetainedFile.open(
        f"run-{index}-rust-integration-test",
        test_executables[0],
        require_executable=True,
    )
    sealed_test = SealedExecutable.from_retained(retained_test)
    generation_environment["FE2O3_LLVM_LINK_WORKER"] = (
        f"/proc/self/fd/{sealed_worker.fd}"
    )
    try:
        run_command(
            [
                os.fspath(test_executables[0]),
                golden["generator_test"],
                "--ignored",
                "--exact",
                "--nocapture",
            ],
            repo,
            generation_environment,
            tools,
            supervisor,
            executable=sealed_test,
            extra_inherited_fds=(sealed_worker.fd,),
        )
        retained_worker.revalidate()
        retained_test.revalidate()
    finally:
        sealed_test.close()
        retained_test.close()
        sealed_worker.close()
        retained_worker.close()
    for file, record in generated:
        revalidate_generated_executable(file, record)
    output.chmod(0o444)
    retained_hsaco = RetainedFile.open(
        f"run-{index}-final-hsaco", output, require_read_only=True
    )
    data = os.pread(retained_hsaco.fd, os.fstat(retained_hsaco.fd).st_size, 0)
    if not data or len(data) > golden["max_hsaco_bytes"]:
        retained_hsaco.close()
        raise EvidenceError(f"run {index} HSACO is empty or exceeds its size bound")
    if not observe_candidate and (
        len(data) != golden["hsaco_bytes"] or sha256_bytes(data) != golden["hsaco_sha256"]
    ):
        retained_hsaco.close()
        raise EvidenceError(f"run {index} HSACO identity changed")
    retained_transaction = RetainedFile.open(
        f"run-{index}-compiler-transaction-observation",
        transaction_output,
        require_read_only=True,
    )
    transaction = json.loads(
        os.pread(
            retained_transaction.fd,
            os.fstat(retained_transaction.fd).st_size,
            0,
        )
    )
    if (
        transaction.get("authority") != "none"
        or transaction.get("final_hsaco_sha256") != sha256_bytes(data)
        or transaction.get("final_hsaco_bytes") != len(data)
    ):
        retained_transaction.close()
        retained_hsaco.close()
        raise EvidenceError("compiler transaction observation does not bind the final HSACO")
    executable_manifest = {
        "schema": "fe2o3-gfx942-run-executable-manifest-v1",
        "run": index,
        "worker_build_identity": build_identity,
        "worker_sha256": worker_record["sha256"],
        "hsaco": retained_hsaco.record(),
        "compiler_transaction": retained_transaction.record(),
        "ctest": {
            "listed": sorted(expected_native_tests),
            "passed": 3,
            "total": 3,
            "sealed_direct_results": native_results,
        },
        "executables": sorted(
            (record for _, record in generated), key=lambda record: record["label"]
        ),
    }
    (output_dir / "executables.json").write_bytes(canonical_json(executable_manifest))
    worker_generated, cargo_generated = capture_generated_build_closures(
        index, run, worker_build, evidence_root
    )
    for file, _ in generated:
        file.close()
    return (
        output,
        executable_manifest,
        retained_hsaco,
        retained_transaction,
        worker_generated,
        cargo_generated,
    )


def build_from_canonical_snapshot(
    index: int,
    source: SnapshotClosure,
    run: Path,
    execution_root: Path,
    evidence_root: Path,
    manifest: dict[str, Any],
    tools: dict[str, PinnedTool],
    golden: dict[str, Any],
    supervisor: Supervisor,
    *,
    observe_candidate: bool,
) -> tuple[
    Path,
    dict[str, Any],
    RetainedFile,
    RetainedFile,
    SnapshotClosure,
    SnapshotClosure,
]:
    archived_root = source.root
    if archived_root != run / "source":
        raise EvidenceError("independent source snapshot has an unexpected archive path")
    source.relocate(execution_root)
    try:
        return build_and_generate(
            index,
            source.root,
            run,
            evidence_root,
            manifest,
            tools,
            golden,
            supervisor,
            observe_candidate=observe_candidate,
        )
    finally:
        source.relocate(archived_root)


def controller(run_root: Path, evidence_root: Path, *, observe_candidate: bool) -> None:
    require_absent_output(run_root, "run root")
    require_absent_output(evidence_root, "evidence root")
    script = Path(__file__)
    if script.resolve(strict=True) != script or script.is_symlink():
        raise EvidenceError("controller must execute by its canonical repository path")
    repo = script.parent.parent
    manifest_path = repo / "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    golden_path = repo / "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json"
    manifest, manifest_bytes = read_bounded_json(manifest_path)
    golden, _ = read_bounded_json(golden_path)
    validate_manifest_document(manifest)
    validate_golden(golden, manifest_path, manifest_bytes)
    soft_files, hard_files = resource.getrlimit(resource.RLIMIT_NOFILE)
    if hard_files < 262144:
        raise EvidenceError("compiler evidence requires an RLIMIT_NOFILE hard limit of 262144")
    if soft_files < 262144:
        resource.setrlimit(resource.RLIMIT_NOFILE, (262144, hard_files))
    tools = pin_tools(manifest)
    closures: list[SnapshotClosure] = []
    retained_closures: list[RetainedClosure] = []
    generated_build_closures: list[SnapshotClosure] = []
    generated_inputs: list[RetainedFile] = []
    retained_artifacts: list[RetainedFile] = []
    try:
        supervisor = Supervisor()
        configured_runtime, runtime_closure = retain_configured_tool_runtime(
            repo, manifest
        )
        retained_closures.append(runtime_closure)
        supervisor.guards.extend(runtime_closure.files)
        run_root.mkdir(mode=0o700)
        evidence_root.mkdir(mode=0o700)
        supervisor.set_writable_roots(
            (run_root, evidence_root, Path("/tmp"), Path("/dev/null"))
        )
        bootstrap_path = run_root / "bootstrap-tool-path"
        create_allowlisted_path(bootstrap_path, manifest, tools)
        bootstrap_environment = {
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": os.fspath(bootstrap_path),
            "TZ": "UTC",
        }
        commit, tree, tracked_paths = git_clean(
            repo, bootstrap_environment, tools, supervisor
        )
        prepared = []
        for index in (1, 2):
            run, source, registry, rust, vendor_generated, cargo_config = prepare_run_closures(
                index,
                repo,
                run_root,
                evidence_root,
                tracked_paths,
                commit,
                tree,
                manifest,
            )
            prepared.append((run, source))
            closures.extend((source, registry, rust))
            retained_closures.append(vendor_generated)
            supervisor.guards.extend(vendor_generated.files)
            generated_inputs.append(cargo_config)
        compare_labeled_manifests(
            closures[0].manifest, closures[3].manifest, reject_reuse=True
        )
        compare_labeled_manifests(
            closures[1].manifest, closures[4].manifest, reject_reuse=True
        )
        compare_labeled_manifests(
            closures[2].manifest, closures[5].manifest, reject_reuse=True
        )
        compare_labeled_manifests(
            retained_closures[1].manifest,
            retained_closures[2].manifest,
            reject_reuse=True,
        )
        first_transition = validate_signed_transition(
            prepared[0][1].root, golden, manifest_bytes
        )
        second_transition = validate_signed_transition(
            prepared[1][1].root, golden, manifest_bytes
        )
        if first_transition != second_transition:
            raise EvidenceError("independent source snapshots contain different transitions")
        provider_members, provider_policy = build_provider_members()
        for index in (1, 2):
            provider = capture_retained_closure(
                f"run-{index}-llvm-rocm-provider",
                provider_members,
                {
                    "llvm_package_version": EXPECTED_LLVM_PACKAGE,
                    "llvm_build_identity": EXPECTED_LLVM_BUILD,
                    "rocm_root": "/opt/rocm-7.2.4",
                    "selection_policy": provider_policy,
                },
            )
            retained_closures.append(provider)
            supervisor.guards.extend(provider.files)
            (evidence_root / f"run-{index}/llvm-rocm-provider-manifest.json").write_bytes(
                canonical_json(provider.manifest)
            )
        compare_labeled_manifests(
            retained_closures[3].manifest, retained_closures[4].manifest
        )
        for closure in closures:
            closure.revalidate()
        observed = version_and_runtime_manifest(
            manifest,
            tools,
            bootstrap_path,
            run_root / "run-1/rust-sysroot/lib",
            supervisor,
            configured_runtime,
        )
        observed_tool_runtime_bytes = canonical_json(observed)
        (evidence_root / "tool-runtime-manifest.json").write_bytes(
            observed_tool_runtime_bytes
        )
        observed_tool_runtime_record = document_record(observed_tool_runtime_bytes)
        canonical_source = run_root / "canonical-execution-source"
        (
            first,
            first_executables,
            first_hsaco,
            first_transaction,
            first_worker_generated,
            first_cargo_generated,
        ) = build_from_canonical_snapshot(
            1,
            prepared[0][1],
            prepared[0][0],
            canonical_source,
            evidence_root,
            manifest,
            tools,
            golden,
            supervisor,
            observe_candidate=observe_candidate,
        )
        generated_build_closures.extend(
            (first_worker_generated, first_cargo_generated)
        )
        supervisor.guards.extend(
            first_worker_generated.source_files
            + first_worker_generated.snapshot_files
            + first_cargo_generated.source_files
            + first_cargo_generated.snapshot_files
        )
        retained_artifacts.extend((first_hsaco, first_transaction))
        for closure in closures:
            closure.revalidate()
        git_clean(repo, bootstrap_environment, tools, supervisor)
        (
            second,
            second_executables,
            second_hsaco,
            second_transaction,
            second_worker_generated,
            second_cargo_generated,
        ) = build_from_canonical_snapshot(
            2,
            prepared[1][1],
            prepared[1][0],
            canonical_source,
            evidence_root,
            manifest,
            tools,
            golden,
            supervisor,
            observe_candidate=observe_candidate,
        )
        generated_build_closures.extend(
            (second_worker_generated, second_cargo_generated)
        )
        supervisor.guards.extend(
            second_worker_generated.source_files
            + second_worker_generated.snapshot_files
            + second_cargo_generated.source_files
            + second_cargo_generated.snapshot_files
        )
        compare_generated_build_snapshots(
            first_worker_generated,
            second_worker_generated,
            cargo_labels=False,
        )
        compare_generated_build_snapshots(
            first_cargo_generated,
            second_cargo_generated,
            cargo_labels=True,
        )
        retained_artifacts.extend((second_hsaco, second_transaction))
        for closure in closures:
            closure.revalidate()
        git_clean(repo, bootstrap_environment, tools, supervisor)
        first_hsaco.revalidate()
        second_hsaco.revalidate()
        first_bytes = os.pread(first_hsaco.fd, os.fstat(first_hsaco.fd).st_size, 0)
        second_bytes = os.pread(second_hsaco.fd, os.fstat(second_hsaco.fd).st_size, 0)
        if first_bytes != second_bytes:
            raise EvidenceError("independent Worker/build/target runs were not byte-identical")
        first_transaction_bytes = os.pread(
            first_transaction.fd, os.fstat(first_transaction.fd).st_size, 0
        )
        second_transaction_bytes = os.pread(
            second_transaction.fd, os.fstat(second_transaction.fd).st_size, 0
        )
        first_transaction_record = json.loads(first_transaction_bytes)
        second_transaction_record = json.loads(second_transaction_bytes)
        verify_transaction_capture(first_transaction_record)
        verify_transaction_capture(second_transaction_record)
        for field in (
            "worker_identity",
            "response_identity",
            "raw_output_identity",
            "finalized_output_identity",
            "final_hsaco_sha256",
            "final_hsaco_bytes",
        ):
            if first_transaction_record.get(field) != second_transaction_record.get(field):
                raise EvidenceError(
                    f"independent compiler transaction stable field differed: {field}"
                )
        reject_cross_run_reuse(first_executables, second_executables)
        first_hsaco.revalidate()
        second_hsaco.revalidate()
        git_clean(repo, bootstrap_environment, tools, supervisor)
        for closure in closures:
            closure.revalidate()
        for closure in retained_closures:
            closure.revalidate()
        for tool in tools.values():
            tool.revalidate()
        reproduction_manifest_sha256 = []
        for index in (1, 2):
            output_dir = evidence_root / f"run-{index}"
            documents = {}
            document_names = [
                "repository-source-manifest.json",
                "cargo-registry-manifest.json",
                "cargo-vendor-generated-manifest.json",
                "rust-sysroot-manifest.json",
                "llvm-rocm-provider-manifest.json",
                "executables.json",
                "compiler-transaction-observation.json",
                "generated-worker-build-manifest.json",
                "generated-cargo-build-manifest.json",
            ]
            for name in document_names:
                document = output_dir / name
                value = document.read_bytes()
                documents[name] = {"bytes": len(value), "sha256": sha256_bytes(value)}
            reproduction = {
                "schema": "fe2o3-gfx942-run-reproduction-manifest-v1",
                "run": index,
                "source_commit": commit,
                "source_tree": tree,
                "work_root": os.fspath(run_root / f"run-{index}"),
                "documents": documents,
                "hsaco_bytes": len(first_bytes),
                "hsaco_sha256": sha256_bytes(first_bytes),
                "claim": "non-production-exact-artifact-observation-only",
                "authority": "none",
            }
            reproduction_bytes = canonical_json(reproduction)
            (output_dir / "reproduction-manifest.json").write_bytes(reproduction_bytes)
            reproduction_manifest_sha256.append(sha256_bytes(reproduction_bytes))
        summary = {
            "schema": "fe2o3-gfx942-two-run-compiler-evidence-summary-v1",
            "source_commit": commit,
            "source_tree": tree,
            "tool_manifest_sha256": sha256_bytes(manifest_bytes),
            "configured_tool_runtime_fixture_sha256": manifest[
                "runtime_manifest_sha256"
            ],
            "observed_tool_runtime_manifest": observed_tool_runtime_record,
            "run_1": {
                "worker_build": os.fspath(run_root / "run-1/worker-build"),
                "cargo_target": os.fspath(run_root / "run-1/cargo-target"),
                "worker_build_identity": first_executables["worker_build_identity"],
                "worker_sha256": first_executables["worker_sha256"],
                "hsaco_bytes": len(first_bytes),
                "hsaco_sha256": sha256_bytes(first_bytes),
            },
            "run_2": {
                "worker_build": os.fspath(run_root / "run-2/worker-build"),
                "cargo_target": os.fspath(run_root / "run-2/cargo-target"),
                "worker_build_identity": second_executables["worker_build_identity"],
                "worker_sha256": second_executables["worker_sha256"],
                "hsaco_bytes": len(second_bytes),
                "hsaco_sha256": sha256_bytes(second_bytes),
            },
            "claim": "exact-artifact-observation-only",
            "compiler_causality_authenticated": False,
            "compiler_receipt_issued": False,
            "transition_candidate_observation": observe_candidate,
            "reproduction_manifest_sha256": reproduction_manifest_sha256,
            "run_scoped_transaction_identities": [
                {
                    field: first_transaction_record[field]
                    for field in (
                        "request_identity",
                        "finalization_identity",
                        "publication_identity",
                    )
                },
                {
                    field: second_transaction_record[field]
                    for field in (
                        "request_identity",
                        "finalization_identity",
                        "publication_identity",
                    )
                },
            ],
        }
        (evidence_root / "summary.json").write_bytes(canonical_json(summary))
        verify_bound_document(
            evidence_root / "tool-runtime-manifest.json",
            summary["observed_tool_runtime_manifest"],
        )
        for transaction in (first_transaction_record, second_transaction_record):
            verify_transaction_capture(transaction)
        print(f"source commit: {commit}")
        print(f"artifact SHA-256: {sha256_bytes(first_bytes)}")
        print("independent pinned-tool Worker V2 compiler evidence: PASS")
    finally:
        for generated in generated_inputs:
            generated.close()
        for artifact in retained_artifacts:
            artifact.close()
        for closure in closures:
            closure.close()
        for closure in generated_build_closures:
            closure.close()
        for closure in retained_closures:
            closure.close()
        for tool in tools.values():
            tool.close()


def self_test(repo: Path) -> None:
    reject_repository_python_bytecode(repo)
    manifest_path = repo / "tests/fixtures/compiler-evidence/gfx942-mi300x-tools.json"
    golden_path = repo / "tests/fixtures/compiler-evidence/gfx942-alpha-zeta-cov6.json"
    manifest, manifest_bytes = read_bounded_json(manifest_path)
    golden, _ = read_bounded_json(golden_path)
    validate_manifest_document(manifest)
    validate_golden(golden, manifest_path, manifest_bytes)
    configured_runtime, runtime_closure = retain_configured_tool_runtime(repo, manifest)
    try:
        if sha256_bytes(canonical_json(configured_runtime)) != manifest["runtime_manifest_sha256"]:
            raise AssertionError("retained tool-runtime fixture digest changed")
    finally:
        runtime_closure.close()
    transition = validate_signed_transition(repo, golden, manifest_bytes)
    transition_bytes = (repo / golden["transition_path"]).read_bytes()
    signature = bytes.fromhex(
        (repo / golden["transition_signature_path"]).read_text("ascii").strip()
    )
    for label, changed_message, changed_signature, changed_key in (
        ("transition content", transition_bytes[:-2] + b"x\n", signature, EXPECTED_TRANSITION_PUBLIC_KEY),
        ("transition signature", transition_bytes, bytes([signature[0] ^ 1]) + signature[1:], EXPECTED_TRANSITION_PUBLIC_KEY),
        ("transition key", transition_bytes, signature, bytes([EXPECTED_TRANSITION_PUBLIC_KEY[0] ^ 1]) + EXPECTED_TRANSITION_PUBLIC_KEY[1:]),
    ):
        if verify_ed25519(changed_key, changed_message, changed_signature):
            raise AssertionError(f"{label} substitution was accepted")
    if transition["authority"] != "none":
        raise AssertionError("transition fixture unexpectedly grants authority")
    for label, mutate in (
        ("cargo path", lambda value: value["tools"][9].__setitem__("path", value["tools"][10]["path"])),
        ("rustc digest", lambda value: value["tools"][10].__setitem__("sha256", "0" * 64)),
        ("cargo version", lambda value: value.__setitem__("cargo_commit", "0" * 40)),
        ("runtime closure", lambda value: value.__setitem__("runtime_manifest_sha256", "0" * 64)),
        ("PATH alias", lambda value: value["path_aliases"].__setitem__("cargo", "rustc")),
    ):
        changed = copy.deepcopy(manifest)
        mutate(changed)
        try:
            validate_manifest_document(changed)
            # Shape-valid content substitutions are rejected by the golden digest.
            validate_golden(golden, manifest_path, canonical_json(changed))
        except EvidenceError:
            pass
        else:
            raise AssertionError(f"{label} substitution was accepted")
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        bound = root / "bound.bin"
        bound.write_bytes(b"bound evidence")
        bound_record = document_record(bound.read_bytes())
        verify_bound_document(bound, bound_record)
        bound.write_bytes(b"mutated evidence")
        try:
            verify_bound_document(bound, bound_record)
        except EvidenceError:
            pass
        else:
            raise AssertionError("bound document mutation was accepted")
        bound.unlink()
        try:
            verify_bound_document(bound, bound_record)
        except EvidenceError:
            pass
        else:
            raise AssertionError("bound document omission was accepted")
        try:
            document_record(b"x" * (MAX_CAPTURE_BYTES + 1))
        except EvidenceError:
            pass
        else:
            raise AssertionError("oversize evidence document was accepted")

        request_prefix = b"F3LREQ02" + b"fixture-request"
        request_identity = hashlib.sha256(
            WORKER_V2_REQUEST_DOMAIN
            + len(request_prefix).to_bytes(8, "little")
            + request_prefix
        ).digest()
        request = request_prefix + struct.pack("<HI", 15, 32) + request_identity
        response = b"F3LRSP02fixture-response"
        response_identity = hashlib.sha256(
            WORKER_V2_RESPONSE_DOMAIN
            + len(response).to_bytes(8, "little")
            + response
        ).hexdigest()
        raw = b"raw-output"
        transaction_fixture = {
            "schema": "fe2o3-non-production-compiler-reproduction-record-v2",
            "canonical_request_bytes": len(request),
            "canonical_request_hex": request.hex(),
            "canonical_request_sha256": sha256_bytes(request),
            "canonical_response_bytes": len(response),
            "canonical_response_hex": response.hex(),
            "canonical_response_sha256": sha256_bytes(response),
            "raw_output_bytes": len(raw),
            "raw_output_hex": raw.hex(),
            "raw_output_sha256": sha256_bytes(raw),
            "raw_output_identity": sha256_bytes(raw),
            "worker_v2_request_identity": request_identity.hex(),
            "sealed_worker_v2_response_identity": response_identity,
        }
        verify_transaction_capture(transaction_fixture)
        for label, mutate in (
            ("omitted request", lambda value: value.pop("canonical_request_hex")),
            (
                "mutated response",
                lambda value: value.__setitem__("canonical_response_hex", "00" + value["canonical_response_hex"][2:]),
            ),
            (
                "oversize raw output",
                lambda value: value.__setitem__("raw_output_bytes", MAX_CAPTURE_BYTES + 1),
            ),
        ):
            changed = copy.deepcopy(transaction_fixture)
            mutate(changed)
            try:
                verify_transaction_capture(changed)
            except EvidenceError:
                pass
            else:
                raise AssertionError(f"transaction {label} was accepted")
        for name in ("run", "evidence"):
            path = root / name
            path.mkdir()
            try:
                require_absent_output(path, name)
            except EvidenceError:
                pass
            else:
                raise AssertionError(f"preexisting {name} directory was accepted")
        run_a = root / "fresh-a"
        run_b = root / "fresh-b"
        run_a.mkdir()
        run_b.mkdir()
        (run_a / "cargo-target").mkdir()
        (run_b / "cargo-target").mkdir()
        (run_a / "worker-build").mkdir()
        (run_b / "worker-build").mkdir()
        if (
            (run_a / "cargo-target").resolve() == (run_b / "cargo-target").resolve()
            or (run_a / "worker-build").resolve() == (run_b / "worker-build").resolve()
        ):
            raise AssertionError("cross-run build-root reuse was accepted")
        reused = {
            "executables": [
                {
                    "label": "test",
                    "path": "/run-a/test",
                    "sha256": "1" * 64,
                    "stat": {
                        "device": 1,
                        "inode": 2,
                        "bytes": 10,
                        "mode": 0o555,
                    },
                }
            ]
        }
        substituted = {
            "executables": [
                {
                    "label": "test",
                    "path": "/run-b/test",
                    "sha256": "1" * 64,
                    "stat": {
                        "device": 1,
                        "inode": 2,
                        "bytes": 10,
                        "mode": 0o555,
                    },
                }
            ]
        }
        try:
            reject_cross_run_reuse(reused, substituted)
        except EvidenceError:
            pass
        else:
            raise AssertionError("cross-run executable reuse was accepted")
        reordered = copy.deepcopy(substituted)
        reordered["executables"] = [
            {
                **reordered["executables"][0],
                "label": "z-test",
                "path": "/run-b/z-test",
                "stat": {
                    **reordered["executables"][0]["stat"],
                    "inode": 3,
                },
            },
            {
                **reordered["executables"][0],
                "label": "a-test",
                "path": "/run-b/a-test",
                "stat": {
                    **reordered["executables"][0]["stat"],
                    "inode": 4,
                },
            },
        ]
        try:
            reject_cross_run_reuse(reordered, reordered)
        except EvidenceError:
            pass
        else:
            raise AssertionError("reordered executable manifest was accepted")
        tools = pin_tools(manifest)
        try:
            allowlist = root / "allowlist"
            shadow = root / "shadow"
            shadow.mkdir()
            (shadow / "cargo").write_text("substituted", encoding="ascii")
            create_allowlisted_path(allowlist, manifest, tools)
            selected = shutil.which("cargo", path=os.fspath(allowlist))
            if selected is None or os.readlink(selected) != tools["cargo"].executable.proc_path:
                raise AssertionError("allowlisted PATH resolved a shadow Cargo")
            if str(shadow) in str(allowlist):
                raise AssertionError("ambient PATH entered the allowlist")
        finally:
            for tool in tools.values():
                tool.close()
    adversarial_self_test()
    reject_repository_python_bytecode(repo)
    print("gfx942 compiler-evidence controller mutation tests: PASS")
    print("gfx942 compiler-evidence clean launcher bytecode test: PASS")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--observe-transition-candidate", action="store_true")
    parser.add_argument("run_root", nargs="?")
    parser.add_argument("evidence_root", nargs="?")
    args = parser.parse_args()
    script = Path(__file__).resolve(strict=True)
    repo = script.parent.parent
    try:
        if args.self_test:
            if (
                args.run_root is not None
                or args.evidence_root is not None
                or args.observe_transition_candidate
            ):
                raise EvidenceError("--self-test accepts no output paths")
            self_test(repo)
        else:
            raise EvidenceError(
                "gfx942 Worker V2 compiler capture is retired; use the production "
                "Worker V3 compiler path before defining replacement evidence"
            )
    except (EvidenceError, HardeningError, OSError, UnicodeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
