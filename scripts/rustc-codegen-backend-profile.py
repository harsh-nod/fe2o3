#!/usr/bin/env python3
"""Enforce the bounded rustc-codegen backend development profile."""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = ROOT / "Cargo.toml"
PACKAGE = "rustc-codegen-fe2o3"
EXPECTED_OVERRIDE = {"strip": "debuginfo"}


class PolicyError(Exception):
    """The workspace manifest does not carry the exact reviewed override."""


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as source:
            manifest = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise PolicyError(f"cannot parse {path}: {error}") from error
    if not isinstance(manifest, dict):
        raise PolicyError(f"{path} is not a TOML table")
    return manifest


def validate_manifest(manifest: dict[str, Any]) -> None:
    try:
        override = manifest["profile"]["dev"]["package"][PACKAGE]
    except (KeyError, TypeError) as error:
        raise PolicyError(
            f"missing [profile.dev.package.{PACKAGE}] override"
        ) from error
    if override != EXPECTED_OVERRIDE:
        raise PolicyError(
            f"[profile.dev.package.{PACKAGE}] must be exactly "
            f"{EXPECTED_OVERRIDE!r}, found {override!r}"
        )


def parse_manifest(text: str) -> dict[str, Any]:
    try:
        manifest = tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        raise PolicyError(f"cannot parse test manifest: {error}") from error
    if not isinstance(manifest, dict):
        raise PolicyError("test manifest is not a TOML table")
    return manifest


def self_test() -> None:
    exact = f'''\
[workspace]
members = []

[profile.dev.package.{PACKAGE}]
strip = "debuginfo"
'''
    validate_manifest(parse_manifest(exact))

    hostile = {
        "removed": "[workspace]\nmembers = []\n",
        "flipped": exact.replace('"debuginfo"', '"none"'),
        "relocated": exact.replace("profile.dev.package", "profile.release.package"),
        "broadened": exact + "debug = 0\n",
    }
    for name, mutation in hostile.items():
        try:
            validate_manifest(parse_manifest(mutation))
        except PolicyError:
            continue
        raise PolicyError(f"hostile {name} profile mutation was accepted")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "self-test"))
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    args = parser.parse_args()

    try:
        if args.command == "self-test":
            self_test()
            print("rustc-codegen backend profile policy self-test passed")
        else:
            validate_manifest(load_manifest(args.manifest))
            print(
                "rustc-codegen backend profile policy passed: "
                f"[profile.dev.package.{PACKAGE}] strip=debuginfo"
            )
    except PolicyError as error:
        print(f"rustc-codegen backend profile policy: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
