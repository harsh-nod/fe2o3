#!/usr/bin/env python3

"""Validate and query the checked-in rustc-codegen integration-test shards."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = ROOT / "scripts" / "rustc-codegen-shards.json"
DEFAULT_PACKAGE_MANIFEST = (
    ROOT / "crates" / "rustc-codegen-fe2o3" / "Cargo.toml"
)
PACKAGE_NAME = "rustc-codegen-fe2o3"
SHARD_ID = re.compile(r"^[a-z0-9][a-z0-9-]*$")
TEST_TARGET = re.compile(r"^[a-z][a-z0-9_]*$")


class PolicyError(Exception):
    pass


def read_json(path: Path, description: str) -> object:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise PolicyError(f"invalid JSON in {description} {path}: {error.msg}") from error
    except OSError as error:
        raise PolicyError(f"cannot read {description} {path}: {error}") from error


def cargo_test_targets(
    package_manifest: Path, metadata_path: Path | None
) -> set[str]:
    try:
        expected_manifest = package_manifest.resolve(strict=True)
    except OSError as error:
        raise PolicyError(
            f"cannot resolve package manifest {package_manifest}: {error}"
        ) from error
    if expected_manifest.name != "Cargo.toml" or not expected_manifest.is_file():
        raise PolicyError(f"package manifest is not a Cargo.toml file: {package_manifest}")
    production_package = expected_manifest == DEFAULT_PACKAGE_MANIFEST.resolve()
    if metadata_path is not None and production_package:
        raise PolicyError(
            "fixture metadata cannot replace production Cargo metadata"
        )

    if metadata_path is None:
        command = [
            "cargo",
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
        ]
        working_directory = ROOT if production_package else expected_manifest.parent
        try:
            result = subprocess.run(
                command,
                cwd=working_directory,
                check=False,
                capture_output=True,
            )
        except OSError as error:
            raise PolicyError(f"cannot execute cargo metadata: {error}") from error
        if result.returncode != 0:
            diagnostic = result.stderr.decode("utf-8", errors="replace").strip()
            suffix = f": {diagnostic}" if diagnostic else ""
            raise PolicyError(
                f"cargo metadata failed with status {result.returncode}{suffix}"
            )
        try:
            metadata = json.loads(result.stdout.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise PolicyError(f"invalid Cargo metadata JSON: {error}") from error
    else:
        metadata = read_json(metadata_path, "Cargo metadata file")

    if not isinstance(metadata, dict):
        raise PolicyError("Cargo metadata must be an object")
    if type(metadata.get("version")) is not int or metadata["version"] != 1:
        raise PolicyError("Cargo metadata version must be integer 1")
    packages = metadata.get("packages")
    if not isinstance(packages, list):
        raise PolicyError("Cargo metadata packages must be an array")

    matching_packages: list[dict[str, object]] = []
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            raise PolicyError(f"Cargo metadata packages[{index}] must be an object")
        name = package.get("name")
        manifest_path = package.get("manifest_path")
        if not isinstance(name, str) or not name:
            raise PolicyError(f"Cargo metadata packages[{index}].name is malformed")
        if not isinstance(manifest_path, str) or not manifest_path:
            raise PolicyError(
                f"Cargo metadata packages[{index}].manifest_path is malformed"
            )
        if name == PACKAGE_NAME:
            matching_packages.append(package)

    if not matching_packages:
        raise PolicyError(f"missing {PACKAGE_NAME} package in Cargo metadata")
    if len(matching_packages) != 1:
        raise PolicyError(f"duplicate {PACKAGE_NAME} packages in Cargo metadata")

    package = matching_packages[0]
    cargo_manifest = Path(str(package["manifest_path"]))
    if not cargo_manifest.is_absolute():
        raise PolicyError("Cargo metadata package manifest_path must be absolute")
    if cargo_manifest.resolve() != expected_manifest:
        raise PolicyError(
            "Cargo metadata package manifest does not match "
            f"{expected_manifest}: {cargo_manifest}"
        )

    targets = package.get("targets")
    if not isinstance(targets, list):
        raise PolicyError("Cargo metadata package targets must be an array")
    actual: set[str] = set()
    for index, target in enumerate(targets):
        location = f"Cargo metadata targets[{index}]"
        if not isinstance(target, dict):
            raise PolicyError(f"{location} must be an object")
        name = target.get("name")
        kinds = target.get("kind")
        source_path = target.get("src_path")
        if not isinstance(name, str) or not name:
            raise PolicyError(f"{location}.name is malformed")
        if (
            not isinstance(kinds, list)
            or not kinds
            or any(not isinstance(kind, str) or not kind for kind in kinds)
        ):
            raise PolicyError(f"{location}.kind is malformed")
        if not isinstance(source_path, str) or not source_path:
            raise PolicyError(f"{location}.src_path is malformed")
        if "test" not in kinds:
            continue
        if not TEST_TARGET.fullmatch(name):
            raise PolicyError(f"malformed Cargo test target: {name}")
        if name in actual:
            raise PolicyError(f"duplicate Cargo test target: {name}")
        actual.add(name)

    if not actual:
        raise PolicyError(f"no Cargo integration test targets for {PACKAGE_NAME}")
    return actual


def load_policy(
    manifest_path: Path, package_manifest: Path, metadata_path: Path | None
) -> list[tuple[str, list[str]]]:
    raw = read_json(manifest_path, "shard manifest")

    if not isinstance(raw, dict) or set(raw) != {"schema", "shards"}:
        raise PolicyError("manifest must contain exactly schema and shards")
    if type(raw["schema"]) is not int or raw["schema"] != 1:
        raise PolicyError("manifest schema must be integer 1")
    if not isinstance(raw["shards"], list) or not raw["shards"]:
        raise PolicyError("manifest shards must be a non-empty array")

    parsed: list[tuple[str, list[str]]] = []
    seen_shards: set[str] = set()
    owners: dict[str, str] = {}
    for index, shard in enumerate(raw["shards"]):
        location = f"shards[{index}]"
        if not isinstance(shard, dict) or set(shard) != {"id", "tests"}:
            raise PolicyError(f"{location} must contain exactly id and tests")
        shard_id = shard["id"]
        tests = shard["tests"]
        if not isinstance(shard_id, str) or not SHARD_ID.fullmatch(shard_id):
            raise PolicyError(f"{location}.id is malformed")
        if shard_id in seen_shards:
            raise PolicyError(f"duplicate shard id: {shard_id}")
        if not isinstance(tests, list) or not tests:
            raise PolicyError(f"empty shard: {shard_id}")
        if any(not isinstance(target, str) for target in tests):
            raise PolicyError(f"{location}.tests must contain only strings")
        if tests != sorted(tests):
            raise PolicyError(f"targets in shard {shard_id} are not sorted")
        for target in tests:
            if not TEST_TARGET.fullmatch(target):
                raise PolicyError(f"malformed test target in {shard_id}: {target}")
            if target in owners:
                raise PolicyError(
                    f"duplicate test target: {target} ({owners[target]}, {shard_id})"
                )
            owners[target] = shard_id
        seen_shards.add(shard_id)
        parsed.append((shard_id, tests))

    shard_ids = [shard_id for shard_id, _ in parsed]
    if shard_ids != sorted(shard_ids):
        raise PolicyError("shard ids are not sorted")
    actual = cargo_test_targets(package_manifest, metadata_path)
    assigned = set(owners)
    unknown = sorted(assigned - actual)
    missing = sorted(actual - assigned)
    if unknown:
        raise PolicyError("unknown or renamed test targets: " + ", ".join(unknown))
    if missing:
        raise PolicyError("missing or newly unassigned test targets: " + ", ".join(missing))
    return parsed


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument(
        "--package-manifest", type=Path, default=DEFAULT_PACKAGE_MANIFEST
    )
    parser.add_argument(
        "--metadata",
        type=Path,
        help="read Cargo metadata from a fixture instead of executing cargo",
    )
    subcommands = parser.add_subparsers(dest="command", required=True)
    subcommands.add_parser("check")
    subcommands.add_parser("list")
    tests = subcommands.add_parser("tests")
    tests.add_argument("shard")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        shards = load_policy(args.manifest, args.package_manifest, args.metadata)
        if args.command == "check":
            target_count = sum(len(tests) for _, tests in shards)
            print(f"rustc-codegen shard policy passed: {len(shards)} shards, {target_count} targets")
        elif args.command == "list":
            for shard_id, _ in shards:
                print(shard_id)
        else:
            selected = next((tests for shard_id, tests in shards if shard_id == args.shard), None)
            if selected is None:
                raise PolicyError(f"unknown shard id: {args.shard}")
            for target in selected:
                print(target)
    except PolicyError as error:
        print(f"rustc-codegen shard policy: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
