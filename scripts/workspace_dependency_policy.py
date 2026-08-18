#!/usr/bin/env python3
"""Enforce the checked-in fe2o3 workspace dependency-layer policy."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_POLICY = REPO_ROOT / "scripts" / "workspace-dependency-policy.json"


class PolicyConfigurationError(ValueError):
    """The policy or Cargo metadata cannot be interpreted safely."""


def _require_list(value: Any, context: str) -> list[Any]:
    if not isinstance(value, list):
        raise PolicyConfigurationError(f"{context} must be a list")
    return value


def _require_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise PolicyConfigurationError(f"{context} must be a non-empty string")
    return value


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PolicyConfigurationError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise PolicyConfigurationError(f"{path} must contain a JSON object")
    return value


def _parse_policy(
    raw: dict[str, Any],
) -> tuple[dict[str, str], list[tuple[str, str]], set[tuple[str, str]], int]:
    if raw.get("schema_version") != 1:
        raise PolicyConfigurationError("policy schema_version must be 1")

    package_layers: dict[str, str] = {}
    layer_names: set[str] = set()
    layers = _require_list(raw.get("layers"), "policy layers")
    for index, entry in enumerate(layers):
        if not isinstance(entry, dict):
            raise PolicyConfigurationError(f"layers[{index}] must be an object")
        layer = _require_string(entry.get("name"), f"layers[{index}].name")
        if layer in layer_names:
            raise PolicyConfigurationError(f"duplicate layer {layer!r}")
        layer_names.add(layer)
        for package in _require_list(
            entry.get("packages"), f"layer {layer!r} packages"
        ):
            package = _require_string(package, f"package in layer {layer!r}")
            previous = package_layers.get(package)
            if previous is not None:
                raise PolicyConfigurationError(
                    f"package {package!r} is assigned to both {previous!r} and {layer!r}"
                )
            package_layers[package] = layer

    path_layers: list[tuple[str, str]] = []
    for index, entry in enumerate(
        _require_list(raw.get("path_layers"), "policy path_layers")
    ):
        if not isinstance(entry, dict):
            raise PolicyConfigurationError(f"path_layers[{index}] must be an object")
        prefix = _require_string(entry.get("prefix"), f"path_layers[{index}].prefix")
        layer = _require_string(entry.get("layer"), f"path_layers[{index}].layer")
        prefix = prefix.replace(os.sep, "/")
        if prefix.startswith("/") or ".." in Path(prefix).parts:
            raise PolicyConfigurationError(
                f"path layer prefix {prefix!r} must be workspace-relative"
            )
        if not prefix.endswith("/"):
            raise PolicyConfigurationError(
                f"path layer prefix {prefix!r} must end with '/'"
            )
        if layer not in layer_names:
            raise PolicyConfigurationError(
                f"path layer prefix {prefix!r} names unknown layer {layer!r}"
            )
        path_layers.append((prefix, layer))

    forbidden: set[tuple[str, str]] = set()
    for index, entry in enumerate(
        _require_list(
            raw.get("forbidden_dependency_directions"),
            "policy forbidden_dependency_directions",
        )
    ):
        if not isinstance(entry, dict):
            raise PolicyConfigurationError(
                f"forbidden_dependency_directions[{index}] must be an object"
            )
        source = _require_string(
            entry.get("from"), f"forbidden_dependency_directions[{index}].from"
        )
        if source not in layer_names:
            raise PolicyConfigurationError(
                f"forbidden direction names unknown source layer {source!r}"
            )
        for target in _require_list(
            entry.get("to"), f"forbidden directions from {source!r}"
        ):
            target = _require_string(target, f"forbidden target from {source!r}")
            if target not in layer_names:
                raise PolicyConfigurationError(
                    f"forbidden direction names unknown target layer {target!r}"
                )
            direction = (source, target)
            if direction in forbidden:
                raise PolicyConfigurationError(
                    f"duplicate forbidden direction {source!r} -> {target!r}"
                )
            forbidden.add(direction)

    return package_layers, path_layers, forbidden, len(layer_names)


def _relative_manifest(manifest: str, workspace_root: str) -> str:
    relative = os.path.relpath(os.path.realpath(manifest), os.path.realpath(workspace_root))
    if relative == ".." or relative.startswith(f"..{os.sep}"):
        raise PolicyConfigurationError(
            f"workspace member manifest is outside the workspace: {manifest}"
        )
    return relative.replace(os.sep, "/")


def _classify(
    package: dict[str, Any],
    workspace_root: str,
    package_layers: dict[str, str],
    path_layers: list[tuple[str, str]],
) -> tuple[str | None, str]:
    name = _require_string(package.get("name"), "Cargo package name")
    manifest = _require_string(package.get("manifest_path"), f"manifest for {name!r}")
    relative_manifest = _relative_manifest(manifest, workspace_root)
    exact = package_layers.get(name)
    if exact is not None:
        return exact, relative_manifest

    matches = {
        layer
        for prefix, layer in path_layers
        if relative_manifest.startswith(prefix)
    }
    if len(matches) > 1:
        joined = ", ".join(sorted(matches))
        raise PolicyConfigurationError(
            f"workspace member {name!r} matches multiple path layers: {joined}"
        )
    return (next(iter(matches)) if matches else None), relative_manifest


def check_policy(
    metadata: dict[str, Any], policy: dict[str, Any]
) -> tuple[list[str], dict[str, int]]:
    package_layers, path_layers, forbidden, layer_count = _parse_policy(policy)
    workspace_root = _require_string(
        metadata.get("workspace_root"), "metadata workspace_root"
    )
    member_ids = set(
        _require_string(member, "metadata workspace member")
        for member in _require_list(
            metadata.get("workspace_members"), "metadata workspace_members"
        )
    )
    packages = _require_list(metadata.get("packages"), "metadata packages")
    packages_by_id: dict[str, dict[str, Any]] = {}
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            raise PolicyConfigurationError(f"metadata packages[{index}] must be an object")
        package_id = _require_string(package.get("id"), f"metadata packages[{index}].id")
        if package_id in packages_by_id:
            raise PolicyConfigurationError(f"duplicate Cargo package id {package_id!r}")
        packages_by_id[package_id] = package

    missing_ids = sorted(member_ids - packages_by_id.keys())
    if missing_ids:
        raise PolicyConfigurationError(
            "workspace member ids missing from metadata packages: " + ", ".join(missing_ids)
        )

    members = [packages_by_id[package_id] for package_id in member_ids]
    members.sort(key=lambda package: (package["name"], package["manifest_path"]))
    classified: dict[str, tuple[dict[str, Any], str, str]] = {}
    package_directories: set[str] = set()
    package_names: set[str] = set()
    violations: list[str] = []
    for package in members:
        name = _require_string(package.get("name"), "Cargo package name")
        if name in package_names:
            raise PolicyConfigurationError(f"duplicate workspace package name {name!r}")
        package_names.add(name)
        package_dir = os.path.realpath(os.path.dirname(package["manifest_path"]))
        if package_dir in package_directories:
            raise PolicyConfigurationError(
                f"multiple workspace packages use directory {package_dir!r}"
            )
        package_directories.add(package_dir)
        layer, relative_manifest = _classify(
            package, workspace_root, package_layers, path_layers
        )
        if layer is None:
            violations.append(
                f"unclassified workspace member: {name} ({relative_manifest})"
            )
            continue
        classified[package_dir] = (package, layer, relative_manifest)

    dependency_count = 0
    for package, source_layer, source_manifest in sorted(
        classified.values(), key=lambda item: (item[0]["name"], item[2])
    ):
        dependencies = _require_list(
            package.get("dependencies"), f"dependencies for {package['name']!r}"
        )
        for dependency in dependencies:
            if not isinstance(dependency, dict):
                raise PolicyConfigurationError(
                    f"dependency for {package['name']!r} must be an object"
                )
            dependency_path = dependency.get("path")
            if not isinstance(dependency_path, str):
                continue
            target = classified.get(os.path.realpath(dependency_path))
            if target is None:
                continue
            dependency_count += 1
            target_package, target_layer, _ = target
            if (source_layer, target_layer) not in forbidden:
                continue
            kind = dependency.get("kind") or "normal"
            if not isinstance(kind, str):
                kind = str(kind)
            target_condition = dependency.get("target")
            condition = f", target {target_condition}" if target_condition else ""
            violations.append(
                "forbidden dependency: "
                f"{package['name']} [{source_layer}] -> "
                f"{target_package['name']} [{target_layer}] "
                f"({kind}{condition}; {source_manifest})"
            )

    return sorted(violations), {
        "workspace_members": len(members),
        "layers": layer_count,
        "internal_dependencies": dependency_count,
    }


def _cargo_metadata() -> dict[str, Any]:
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--no-deps",
    ]
    try:
        completed = subprocess.run(
            command,
            cwd=REPO_ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", None) or str(error)
        raise PolicyConfigurationError(f"cargo metadata failed: {detail.strip()}") from error
    try:
        value = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise PolicyConfigurationError(f"cargo metadata emitted invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise PolicyConfigurationError("cargo metadata must emit a JSON object")
    return value


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument(
        "--metadata",
        type=Path,
        help="read metadata JSON from a file instead of invoking Cargo",
    )
    arguments = parser.parse_args(argv)

    try:
        policy = _load_json(arguments.policy)
        metadata = _load_json(arguments.metadata) if arguments.metadata else _cargo_metadata()
        violations, stats = check_policy(metadata, policy)
    except PolicyConfigurationError as error:
        print(f"workspace dependency policy configuration error: {error}", file=sys.stderr)
        return 2

    if violations:
        for violation in violations:
            print(violation, file=sys.stderr)
        print(
            f"workspace dependency policy: FAILED ({len(violations)} violation(s))",
            file=sys.stderr,
        )
        return 1

    print(
        "workspace dependency policy: OK "
        f"({stats['workspace_members']} members, {stats['layers']} layers, "
        f"{stats['internal_dependencies']} internal dependency declarations)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
