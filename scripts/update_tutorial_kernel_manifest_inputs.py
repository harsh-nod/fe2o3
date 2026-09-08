#!/usr/bin/env python3
"""Check or refresh repository-derived tutorial compiler-input identities.

This tool updates only source inputs. It cannot create capability closures,
proof results, simulator observations, hardware evidence, or production status.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import sys
from typing import Any

import tutorial_kernel_manifest as manifest_contract


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST = REPO_ROOT / "config" / manifest_contract.MANIFEST_NAME
DIGEST = MANIFEST.with_suffix(".sha256")


class RefreshError(ValueError):
    """Repository inputs cannot be refreshed deterministically."""


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _refresh_fixture(fixture: dict[str, Any]) -> None:
    fixture_id = fixture["fixtureId"]
    label = f"fixture {fixture_id}.compilerInput"
    compiler_input = fixture["compilerInput"]
    package_manifest = compiler_input["packageManifest"]
    manifest_path, manifest_bytes = manifest_contract._repository_file(
        REPO_ROOT,
        package_manifest,
        f"{label}.packageManifest",
        maximum_bytes=manifest_contract.MAX_CARGO_MANIFEST_BYTES,
    )
    package_root = manifest_path.parent
    lock_path = manifest_contract._effective_cargo_lock(REPO_ROOT, package_root, label)
    lock_relative = lock_path.relative_to(REPO_ROOT).as_posix()
    _, lock_bytes = manifest_contract._repository_file(
        REPO_ROOT,
        lock_relative,
        f"{label}.cargoLockPath",
        maximum_bytes=manifest_contract.MAX_CARGO_LOCK_BYTES,
    )

    compiler_input["packageManifestSha256"] = _sha256(manifest_bytes)
    compiler_input["sourceClosureSha256"] = (
        manifest_contract._package_rust_source_closure(
            REPO_ROOT, package_root, label
        )
    )
    compiler_input["cargoLockPath"] = lock_relative
    compiler_input["cargoLockSha256"] = _sha256(lock_bytes)
    compiler_input["contractSha256"] = (
        manifest_contract._fixture_input_contract_sha256(fixture)
    )


def refreshed_document(document: dict[str, Any]) -> dict[str, Any]:
    refreshed = copy.deepcopy(document)
    for fixture in refreshed["compilerFixtures"]:
        _refresh_fixture(fixture)
    return refreshed


def encoded(document: dict[str, Any]) -> bytes:
    return (
        json.dumps(document, ensure_ascii=True, indent=2, allow_nan=False)
        + "\n"
    ).encode("ascii")


def _digest_record(payload: bytes) -> bytes:
    return f"{_sha256(payload)}  config/{MANIFEST.name}\n".encode("ascii")


def _replace_regular_file(path: Path, payload: bytes) -> None:
    if path.is_symlink() or not path.is_file():
        raise RefreshError(f"refusing to replace non-regular file: {path}")
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    if temporary.exists() or temporary.is_symlink():
        raise RefreshError(f"temporary path already exists: {temporary}")
    try:
        temporary.write_bytes(payload)
        temporary.chmod(path.stat().st_mode & 0o777)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="replace the manifest and its raw SHA-256 sidecar",
    )
    options = parser.parse_args(arguments)
    try:
        raw, document = manifest_contract._load_json_unique(MANIFEST)
        refreshed = refreshed_document(document)
        payload = encoded(refreshed)
        if not options.write:
            if raw != payload:
                raise RefreshError(
                    "tutorial compiler-input identities are stale; rerun with --write"
                )
            expected_digest = _digest_record(payload)
            if DIGEST.read_bytes() != expected_digest:
                raise RefreshError("tutorial manifest digest sidecar is stale")
            print("tutorial compiler-input identities are current")
            return 0

        _replace_regular_file(MANIFEST, payload)
        _replace_regular_file(DIGEST, _digest_record(payload))
        manifest_contract.validate_repository(REPO_ROOT)
    except (OSError, RefreshError, manifest_contract.ManifestError) as error:
        print(f"tutorial manifest input refresh: {error}", file=sys.stderr)
        return 1
    print("refreshed tutorial compiler-input identities")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
