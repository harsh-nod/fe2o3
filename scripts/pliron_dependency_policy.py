#!/usr/bin/env python3
"""Enforce the exact Wave 0 Pliron dependency closure."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
PLIRON_REVISION = "2610651306ea3ba670f68d5d8b1e1159bcd521ed"
PLIRON_VERSION = "0.17.0"
PLIRON_SOURCE = (
    "git+https://github.com/pliron-org/pliron.git"
    f"?rev={PLIRON_REVISION}#{PLIRON_REVISION}"
)
REQUIRED_PACKAGES = ("pliron", "pliron-derive")


class PolicyInputError(ValueError):
    """Cargo metadata does not have the shape required by this checker."""


def _packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise PolicyInputError("metadata packages must be a list")
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            raise PolicyInputError(f"metadata packages[{index}] must be an object")
    return packages


def check_metadata(metadata: dict[str, Any]) -> tuple[list[str], dict[str, int]]:
    """Return stable policy violations and closure statistics."""
    selected: dict[str, list[dict[str, Any]]] = {
        name: [] for name in (*REQUIRED_PACKAGES, "pliron-llvm")
    }
    unexpected_source_packages: list[str] = []
    for package in _packages(metadata):
        name = package.get("name")
        if not isinstance(name, str) or not name:
            raise PolicyInputError("every Cargo package must have a non-empty name")
        source = package.get("source")
        if name in selected:
            selected[name].append(package)
        if isinstance(source, str) and "github.com/pliron-org/pliron.git" in source:
            if name not in selected:
                unexpected_source_packages.append(name)

    violations: list[str] = []
    for name in REQUIRED_PACKAGES:
        matches = selected[name]
        if not matches:
            violations.append(f"missing required Pliron package: {name}")
        if len(matches) > 1:
            violations.append(f"duplicate Pliron package identity: {name} ({len(matches)})")

    for name, matches in selected.items():
        for package in matches:
            version = package.get("version")
            source = package.get("source")
            if version != PLIRON_VERSION:
                violations.append(
                    f"wrong Pliron version: {name} resolved {version!r}, "
                    f"expected {PLIRON_VERSION!r}"
                )
            if source != PLIRON_SOURCE:
                violations.append(
                    f"wrong Pliron source: {name} resolved {source!r}, "
                    f"expected {PLIRON_SOURCE!r}"
                )

    if selected["pliron-llvm"]:
        violations.append(
            "pliron-llvm is outside the D0 closure; add it only with the reviewed LLVM integration"
        )
    for name in sorted(set(unexpected_source_packages)):
        violations.append(f"unexpected package from the Pliron repository: {name}")

    return sorted(violations), {
        "pliron": len(selected["pliron"]),
        "pliron_derive": len(selected["pliron-derive"]),
        "pliron_llvm": len(selected["pliron-llvm"]),
    }


def _cargo_metadata() -> dict[str, Any]:
    try:
        completed = subprocess.run(
            ["cargo", "metadata", "--locked", "--format-version", "1"],
            cwd=REPO_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or str(error)
        raise PolicyInputError(f"cargo metadata failed: {detail.strip()}") from error
    try:
        metadata = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise PolicyInputError(f"cargo metadata returned invalid JSON: {error}") from error
    if not isinstance(metadata, dict):
        raise PolicyInputError("cargo metadata must return a JSON object")
    return metadata


def main() -> int:
    try:
        violations, stats = check_metadata(_cargo_metadata())
    except PolicyInputError as error:
        print(f"Pliron dependency policy configuration error: {error}", file=sys.stderr)
        return 2
    if violations:
        for violation in violations:
            print(violation, file=sys.stderr)
        return 1
    print(
        "Pliron dependency policy: OK "
        f"(pliron={stats['pliron']}, derive={stats['pliron_derive']}, "
        f"llvm={stats['pliron_llvm']}, revision={PLIRON_REVISION})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
