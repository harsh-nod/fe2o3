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
RETIRED_QUALIFICATION_SELECTOR = "FE2O3_QUALIFICATION_ORACLE_V1"
MAX_TEST_SOURCE_BYTES = 8 * 1024 * 1024


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
) -> dict[str, tuple[Path, frozenset[str]]]:
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
    actual: dict[str, tuple[Path, frozenset[str]]] = {}
    for index, target in enumerate(targets):
        location = f"Cargo metadata targets[{index}]"
        if not isinstance(target, dict):
            raise PolicyError(f"{location} must be an object")
        name = target.get("name")
        kinds = target.get("kind")
        source_path = target.get("src_path")
        required_features = target.get("required-features", [])
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
        if not isinstance(required_features, list) or any(
            not isinstance(feature, str) or not feature for feature in required_features
        ):
            raise PolicyError(f"{location}.required-features is malformed")
        if "test" not in kinds:
            continue
        if not TEST_TARGET.fullmatch(name):
            raise PolicyError(f"malformed Cargo test target: {name}")
        if name in actual:
            raise PolicyError(f"duplicate Cargo test target: {name}")
        source = Path(source_path)
        if not source.is_absolute():
            raise PolicyError(f"Cargo test target source path must be absolute: {name}")
        if source.is_symlink() or not source.is_file():
            raise PolicyError(f"Cargo test target source is not a regular file: {name}")
        try:
            resolved_source = source.resolve(strict=True)
            resolved_source.relative_to(expected_manifest.parent)
        except OSError as error:
            raise PolicyError(
                f"cannot resolve Cargo test target source for {name}: {error}"
            ) from error
        except ValueError as error:
            raise PolicyError(
                f"Cargo test target source escapes the package root: {name}"
            ) from error
        actual[name] = (resolved_source, frozenset(required_features))

    if not actual:
        raise PolicyError(f"no Cargo integration test targets for {PACKAGE_NAME}")
    return actual


def load_policy(
    manifest_path: Path, package_manifest: Path, metadata_path: Path | None
) -> list[tuple[str, list[str]]]:
    raw = read_json(manifest_path, "shard manifest")

    if not isinstance(raw, dict) or set(raw) != {
        "schema",
        "shards",
        "retiredQualificationTargets",
    }:
        raise PolicyError(
            "manifest must contain exactly schema, shards, and "
            "retiredQualificationTargets"
        )
    if type(raw["schema"]) is not int or raw["schema"] != 2:
        raise PolicyError("manifest schema must be integer 2")
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
    retired = raw["retiredQualificationTargets"]
    if not isinstance(retired, list) or any(
        not isinstance(target, str) for target in retired
    ):
        raise PolicyError("retiredQualificationTargets must contain only strings")
    if retired != sorted(retired):
        raise PolicyError("retired qualification targets are not sorted")
    if len(retired) != len(set(retired)):
        raise PolicyError("duplicate retired qualification target")
    for target in retired:
        if not TEST_TARGET.fullmatch(target):
            raise PolicyError(f"malformed retired qualification target: {target}")
        if target in owners:
            raise PolicyError(
                f"target is both active and retired qualification coverage: {target}"
            )

    actual = cargo_test_targets(package_manifest, metadata_path)
    assigned = set(owners)
    inventoried = assigned | set(retired)
    unknown = sorted(inventoried - actual.keys())
    missing = sorted(actual.keys() - inventoried)
    if unknown:
        raise PolicyError("unknown or renamed test targets: " + ", ".join(unknown))
    if missing:
        raise PolicyError("missing or newly unassigned test targets: " + ", ".join(missing))

    def read_source(target: str) -> str:
        path = actual[target][0]
        try:
            size = path.stat().st_size
            if size > MAX_TEST_SOURCE_BYTES:
                raise PolicyError(
                    f"test target source exceeds {MAX_TEST_SOURCE_BYTES} bytes: {target}"
                )
            return path.read_text(encoding="utf-8")
        except PolicyError:
            raise
        except (OSError, UnicodeError) as error:
            raise PolicyError(f"cannot read test target {target}: {error}") from error

    def injects_retired_selector(source: str) -> bool:
        if re.search(
            rf'\.env\s*\(\s*"{RETIRED_QUALIFICATION_SELECTOR}"', source
        ):
            return True
        aliases = re.findall(
            rf"\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"
            rf'"{RETIRED_QUALIFICATION_SELECTOR}"',
            source,
        )
        return any(
            re.search(rf"\.env\s*\(\s*{re.escape(alias)}\b", source)
            for alias in aliases
        )

    for target in sorted(assigned):
        source = read_source(target)
        if injects_retired_selector(source):
            raise PolicyError(
                "active production test target injects the retired qualification "
                f"selector: {target}"
            )
        if "qualification-oracles-test-only" in actual[target][1]:
            raise PolicyError(
                "active production test target requires the offline qualification "
                f"feature: {target}"
            )
    for target in retired:
        if not injects_retired_selector(read_source(target)):
            raise PolicyError(
                "retired qualification target no longer injects the selector and must "
                f"move to an active shard: {target}"
            )
        if "qualification-oracles-test-only" not in actual[target][1]:
            raise PolicyError(
                "retired qualification target is not feature-gated in Cargo metadata: "
                f"{target}"
            )

    support_root = package_manifest.parent / "tests" / "support"
    if support_root.is_dir():
        for support in sorted(support_root.rglob("*.rs")):
            if support.is_symlink() or not support.is_file():
                raise PolicyError(
                    f"test support source is not a regular file: {support}"
                )
            try:
                size = support.stat().st_size
                if size > MAX_TEST_SOURCE_BYTES:
                    raise PolicyError(
                        "test support source exceeds "
                        f"{MAX_TEST_SOURCE_BYTES} bytes: {support}"
                    )
                source = support.read_text(encoding="utf-8")
            except PolicyError:
                raise
            except (OSError, UnicodeError) as error:
                raise PolicyError(f"cannot read test support source {support}: {error}") from error
            if injects_retired_selector(source):
                raise PolicyError(
                    f"shared test support injects the retired qualification selector: {support}"
                )
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
    subcommands.add_parser("retired")
    tests = subcommands.add_parser("tests")
    tests.add_argument("shard")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        shards = load_policy(args.manifest, args.package_manifest, args.metadata)
        if args.command == "check":
            target_count = sum(len(tests) for _, tests in shards)
            raw = read_json(args.manifest, "shard manifest")
            assert isinstance(raw, dict)
            retired_count = len(raw["retiredQualificationTargets"])
            print(
                "rustc-codegen shard policy passed: "
                f"{len(shards)} shards, {target_count} production targets, "
                f"{retired_count} retired qualification targets"
            )
        elif args.command == "list":
            for shard_id, _ in shards:
                print(shard_id)
        elif args.command == "retired":
            raw = read_json(args.manifest, "shard manifest")
            assert isinstance(raw, dict)
            for target in raw["retiredQualificationTargets"]:
                print(target)
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
