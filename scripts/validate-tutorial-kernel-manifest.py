#!/usr/bin/env python3
"""Validate expected ordinary-source corpus contracts, never qualification receipts.

The source scanner is the recovered donor inventory scanner, not rustc semantic
analysis. Feature reachability, observed compiler inputs, optimized-graph policy
checks, simulation and hardware remain separate qualification obligations.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import tomllib
from typing import Any


TOP_LEVEL_KEYS = {
    "schema",
    "roadmapIssue",
    "baseline",
    "recovery",
    "productionContract",
    "qualification",
    "compilerFixtures",
    "entries",
}
ENTRY_KEYS = {
    "lessonId",
    "siteEvidenceKind",
    "classification",
    "packageManifest",
    "sourcePaths",
    "compilerFixtureIds",
    "requiredGates",
}
FIXTURE_KEYS = {
    "fixtureId",
    "testId",
    "testPath",
    "target",
    "matrix",
    "compilerInput",
    "simulation",
}
SIMULATION_KEYS = {
    "requestPath",
    "requestSha256",
    "requestBytes",
    "expectationPath",
    "expectationSha256",
    "expectationBytes",
    "bundleVersion",
    "canonicalKirVersion",
    "status",
}
COMPILER_INPUT_KEYS = {
    "packageManifest",
    "packageManifestSha256",
    "cargoLockPath",
    "cargoLockSha256",
    "sourcePaths",
    "sourceClosureSha256",
    "cargoTarget",
    "defaultFeatures",
    "features",
    "kernelSymbols",
    "contractSha256",
}
CARGO_TARGET_KEYS = {"kind", "name", "sourcePath"}
MATRIX_KEYS = {
    "caseId",
    "runnerPath",
    "artifactName",
    "runnerArguments",
    "environment",
}
QUALIFICATION_KEYS = {"status", "evidenceSchemaPath", "hardwareTargets", "suites"}
HARDWARE_TARGET_KEYS = {
    "target",
    "deterministicSemanticRunnerAvailability",
    "reason",
}
SUITE_KEYS = {
    "suiteId",
    "gate",
    "availability",
    "command",
    "unavailableReason",
    "coverage",
}
SUITE_COMMAND_KEYS = {
    "executable",
    "arguments",
    "environment",
    "workingDirectory",
    "timeoutSeconds",
}
SUITE_COVERAGE_KEYS = {"lessonId", "fixtureIds"}
COMPILER_EVIDENCE = {
    "runnable-now",
    "compiler-checked",
    "compiler-hsaco-observed",
    "gpu-observed",
}
ALLOWED_EVIDENCE = COMPILER_EVIDENCE | {
    "source-example",
    "source-tested",
    "source-model-verified",
    "design-only",
}
ALLOWED_TARGETS = {"gfx942", "gfx950"}
ALLOWED_GATES = {
    "production-compile",
    "semantic-simulation",
    "cpu-reference",
    "hardware",
}
MAX_SUITE_COMMAND_ARGUMENTS = 24
MAX_SUITE_COMMAND_ENVIRONMENT = 16
MAX_ATTRIBUTED_SOURCE_BYTES = 4 * 1024 * 1024
MAX_PACKAGE_SOURCE_FILES = 4096
MAX_PACKAGE_SOURCE_BYTES = 64 * 1024 * 1024
MAX_CARGO_MANIFEST_BYTES = 1024 * 1024
MAX_CARGO_LOCK_BYTES = 8 * 1024 * 1024
MAX_SIMULATION_DOCUMENT_BYTES = 16 * 1024 * 1024
IGNORED_PACKAGE_DIRECTORIES = {"target"}
CORPUS_CONTRACT_DIGEST_DOMAIN = b"fe2o3-tutorial-kernel-corpus-contract-v1\0"
SOURCE_CLOSURE_DIGEST_DOMAIN = b"fe2o3-tutorial-package-rust-source-closure-v1\0"
FIXTURE_INPUT_DIGEST_DOMAIN = b"fe2o3-tutorial-fixture-compiler-input-v1\0"
ATTRIBUTE_START = re.compile(r"#[ \t]*\[")
MACRO_RULES_START = re.compile(r"\bmacro_rules\s*!")
SOURCE_INCLUDE_START = re.compile(r"\binclude\s*!")
RUST_DECLARATION = re.compile(
    r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:(?:const|async|unsafe)\s+)*(?:extern\s+(?:\"[^\"]*\"\s+)?)?fn\b"
)
RUST_FUNCTION_NAME = re.compile(
    r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:(?:const|async|unsafe)\s+)*(?:extern\s+(?:\"[^\"]*\"\s+)?)?"
    r"fn\s+([A-Za-z_][A-Za-z0-9_]*)\b"
)
LOWER_SHA256 = re.compile(r"[0-9a-f]{64}")
RUST_IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
TARGET_SECTIONS = ("lib", "bin", "example", "test", "bench")


def fail(message: str) -> None:
    raise SystemExit(f"tutorial kernel manifest: {message}")


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{label} must be an object")
    return value


def require_exact_keys(value: dict[str, Any], keys: set[str], label: str) -> None:
    actual = set(value)
    if actual != keys:
        fail(
            f"{label} keys differ: missing={sorted(keys - actual)}, "
            f"unknown={sorted(actual - keys)}"
        )


def require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        fail(f"{label} must be a nonempty string")
    return value


def require_target(value: Any, label: str) -> str:
    target = require_string(value, label)
    if target not in ALLOWED_TARGETS:
        fail(f"{label} is unsupported: {target}")
    return target


def require_string_list(value: Any, label: str) -> list[str]:
    if not isinstance(value, list) or not value:
        fail(f"{label} must be a nonempty array")
    result = [require_string(item, f"{label}[]") for item in value]
    if len(result) != len(set(result)):
        fail(f"{label} must be duplicate-free")
    return result


def require_string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        fail(f"{label} must be an array")
    result = [require_string(item, f"{label}[]") for item in value]
    if len(result) != len(set(result)):
        fail(f"{label} must be duplicate-free")
    return result


def checked_path(repo_root: Path, value: Any, label: str) -> str:
    text = require_string(value, label)
    relative = PurePosixPath(text)
    if relative.is_absolute() or ".." in relative.parts or str(relative) != text:
        fail(f"{label} is not a canonical repository-relative path: {text!r}")
    path = repo_root.joinpath(*relative.parts)
    if (
        any(
            repo_root.joinpath(*relative.parts[:index]).is_symlink()
            for index in range(1, len(relative.parts) + 1)
        )
        or not path.is_file()
    ):
        fail(f"{label} does not name a regular repository input: {text}")
    return text


def checked_directory(repo_root: Path, value: Any, label: str) -> str:
    text = require_string(value, label)
    relative = PurePosixPath(text)
    if relative.is_absolute() or ".." in relative.parts or str(relative) != text:
        fail(f"{label} is not a canonical repository-relative directory: {text!r}")
    path = repo_root.joinpath(*relative.parts)
    if (
        any(
            repo_root.joinpath(*relative.parts[:index]).is_symlink()
            for index in range(1, len(relative.parts) + 1)
        )
        or not path.is_dir()
    ):
        fail(f"{label} does not name a regular repository directory: {text}")
    return text


def _rust_code_without_comments_and_literals(source: str) -> str:
    """Preserve source coordinates while removing non-code Rust text."""
    output = list(source)
    index = 0
    block_depth = 0
    while index < len(source):
        if block_depth:
            if source.startswith("/*", index):
                output[index : index + 2] = "  "
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                output[index : index + 2] = "  "
                block_depth -= 1
                index += 2
            else:
                if source[index] != "\n":
                    output[index] = " "
                index += 1
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
            output[index:end] = " " * (end - index)
            index = end
            continue
        if source.startswith("/*", index):
            output[index : index + 2] = "  "
            block_depth = 1
            index += 2
            continue

        character = re.match(r"'(?:\\.|[^'\\\n])'", source[index:])
        if character is not None:
            end = index + character.end()
            output[index:end] = " " * (end - index)
            index = end
            continue

        raw = re.match(r"(?:br|cr|r)(?P<hashes>#{0,255})\"", source[index:])
        if raw is not None:
            delimiter = '"' + raw.group("hashes")
            end = source.find(delimiter, index + raw.end())
            if end < 0:
                fail("Rust source contains an unterminated raw string literal")
            end += len(delimiter)
            for offset in range(index, end):
                if source[offset] != "\n":
                    output[offset] = " "
            index = end
            continue

        prefix = 1 if source.startswith(('b"', 'c"'), index) else 0
        if source[index + prefix : index + prefix + 1] == '"':
            cursor = index + prefix + 1
            escaped = False
            closed = False
            while cursor < len(source):
                char = source[cursor]
                cursor += 1
                if char == '"' and not escaped:
                    closed = True
                    break
                escaped = char == "\\" and not escaped
                if char != "\\":
                    escaped = False
            if not closed:
                fail("Rust source contains an unterminated string literal")
            for offset in range(index, cursor):
                if source[offset] != "\n":
                    output[offset] = " "
            index = cursor
            continue
        index += 1
    if block_depth:
        fail("Rust source contains an unterminated block comment")
    return "".join(output)


def _rust_delimiters(code: str) -> dict[int, int]:
    """Return balanced delimiter pairs for code with literals/comments masked."""
    opening = {"(": ")", "[": "]", "{": "}"}
    closing = {value: key for key, value in opening.items()}
    stack: list[tuple[str, int]] = []
    pairs: dict[int, int] = {}
    for index, char in enumerate(code):
        if char in opening:
            stack.append((char, index))
        elif char in closing:
            if not stack or stack[-1][0] != closing[char]:
                fail(f"Rust source contains an unmatched delimiter at byte {index}")
            _, start = stack.pop()
            pairs[start] = index + 1
    if stack:
        fail(f"Rust source contains an unterminated {stack[-1][0]} delimiter")
    return pairs


def _rust_attribute(code: str, start: int, pairs: dict[int, int]) -> tuple[int, str]:
    opening = code.find("[", start)
    if opening < 0 or opening not in pairs:
        fail("Rust source contains an unterminated attribute")
    closing = pairs[opening]
    return closing, code[opening + 1 : closing - 1]


def _attribute_has_kernel_name(body: str) -> bool:
    name = re.match(r"\s*([A-Za-z_][A-Za-z0-9_]*)\b", body)
    if name is None:
        return False
    if name.group(1) == "kernel":
        return True
    if name.group(1) != "cfg_attr":
        return False
    # Strings and comments have already been masked. Any kernel token in a
    # cfg_attr is conservatively treated as active because features are not
    # part of this manifest's lexical contract.
    return re.search(r"\bkernel\b", body[name.end() :]) is not None


def _macro_rule_bodies(code: str, pairs: dict[int, int]) -> list[tuple[int, int]]:
    bodies = []
    for match in MACRO_RULES_START.finditer(code):
        cursor = match.end()
        while cursor < len(code) and code[cursor].isspace():
            cursor += 1
        # macro_rules! name { ... }; has one identifier before its body.
        identifier = re.match(r"[A-Za-z_][A-Za-z0-9_]*", code[cursor:])
        if identifier is not None:
            cursor += identifier.end()
        while cursor < len(code) and code[cursor].isspace():
            cursor += 1
        if cursor >= len(code) or code[cursor] not in "([{" or cursor not in pairs:
            fail("Rust source contains an unterminated macro_rules body")
        bodies.append((cursor, pairs[cursor]))
    return bodies


def source_contains_ordinary_attributed_kernel(source: str) -> bool:
    return bool(ordinary_attributed_kernel_names(source))


def ordinary_attributed_kernel_names(source: str) -> list[str]:
    code = _rust_code_without_comments_and_literals(source)
    pairs = _rust_delimiters(code)
    macro_bodies = _macro_rule_bodies(code, pairs)
    names: list[str] = []

    def in_macro_body(index: int) -> bool:
        return any(start < index < end for start, end in macro_bodies)

    for attribute in ATTRIBUTE_START.finditer(code):
        if in_macro_body(attribute.start()):
            continue
        # Inner attributes apply to a module/crate, never to a kernel item.
        if attribute.start() > 0 and code[attribute.start() - 1] == "!":
            continue
        closing, body = _rust_attribute(code, attribute.start(), pairs)
        if not _attribute_has_kernel_name(body):
            continue
        cursor = closing
        while True:
            following = re.match(r"\s*#\s*\[", code[cursor:])
            if following is None:
                break
            nested_start = cursor + following.start()
            nested_close, _ = _rust_attribute(code, nested_start, pairs)
            cursor = nested_close
        declaration = RUST_FUNCTION_NAME.match(code[cursor:])
        if declaration is not None:
            names.append(declaration.group(1))
    return names


def _package_path(package_root: Path, value: Any, label: str) -> Path:
    text = require_string(value, label)
    relative = PurePosixPath(text)
    if relative.is_absolute() or ".." in relative.parts or str(relative) != text:
        fail(f"{label} must stay within its package root: {text!r}")
    if relative.parts and relative.parts[0] in IGNORED_PACKAGE_DIRECTORIES:
        fail(f"{label} points into a generated package directory: {text!r}")
    path = package_root.joinpath(*relative.parts)
    if not path.is_file() or path.is_symlink():
        fail(f"{label} does not name a regular package file: {text}")
    return path


def _validate_package_source_path(
    repo_root: Path, package_root: Path, value: Any, label: str
) -> Path:
    text = checked_path(repo_root, value, label)
    path = repo_root.joinpath(*PurePosixPath(text).parts)
    try:
        relative = path.relative_to(package_root)
    except ValueError:
        fail(f"{label} escapes its package root: {text}")
    if relative.parts and relative.parts[0] in IGNORED_PACKAGE_DIRECTORIES:
        fail(f"{label} points into a generated package directory: {text}")
    return path


def _validate_package_targets(package_root: Path, manifest_path: Path) -> None:
    try:
        payload = manifest_path.read_bytes()
    except OSError as error:
        fail(f"cannot read package manifest: {error}")
    if len(payload) > MAX_CARGO_MANIFEST_BYTES:
        fail("package manifest exceeds the source bound")
    try:
        cargo = tomllib.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        fail(f"cannot parse package manifest: {error}")
    for section in TARGET_SECTIONS:
        targets = cargo.get(section, []) if section != "lib" else [cargo.get("lib", {})]
        if isinstance(targets, dict):
            targets = [targets]
        if not isinstance(targets, list):
            fail(f"package manifest target section {section} is malformed")
        for index, target in enumerate(targets):
            if not isinstance(target, dict) or "path" not in target:
                continue
            _package_path(
                package_root, target["path"], f"Cargo [{section}][{index}].path"
            )


def package_rust_sources(
    repo_root: Path, package_manifest: str, label: str
) -> list[tuple[Path, str]]:
    manifest_path = repo_root.joinpath(*PurePosixPath(package_manifest).parts)
    package_root = manifest_path.parent
    _validate_package_targets(package_root, manifest_path)
    sources: list[tuple[Path, str]] = []
    total_bytes = 0
    for directory, directory_names, file_names in os.walk(
        package_root, followlinks=False
    ):
        directory_path = Path(directory)
        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            path = directory_path / name
            if path.is_symlink():
                fail(
                    f"{label} package contains a symlink: {path.relative_to(repo_root)}"
                )
        directory_names[:] = [
            name for name in directory_names if name not in IGNORED_PACKAGE_DIRECTORIES
        ]
        for name in file_names:
            path = directory_path / name
            if path.is_symlink():
                fail(
                    f"{label} package contains a symlink: {path.relative_to(repo_root)}"
                )
            if path.suffix != ".rs":
                continue
            if not path.is_file():
                fail(
                    f"{label} source is not a regular file: "
                    f"{path.relative_to(repo_root)}"
                )
            try:
                size = path.stat().st_size
                if size > MAX_ATTRIBUTED_SOURCE_BYTES:
                    fail(
                        f"{label} source exceeds the source bound: "
                        f"{path.relative_to(repo_root)}"
                    )
                text = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                fail(f"cannot read {label} package source {path}: {error}")
            total_bytes += size
            if total_bytes > MAX_PACKAGE_SOURCE_BYTES:
                fail(f"{label} package source closure exceeds the byte bound")
            sources.append((path, text))
            if len(sources) > MAX_PACKAGE_SOURCE_FILES:
                fail(f"{label} package source closure exceeds the file bound")
    if not sources:
        fail(f"{label} package has no Rust source files")
    return sources


def sha256_file(path: Path, maximum_bytes: int, label: str) -> str:
    if path.is_symlink() or not path.is_file():
        fail(f"{label} is not a regular file")
    try:
        size = path.stat().st_size
        if size > maximum_bytes:
            fail(f"{label} exceeds its byte bound")
        payload = path.read_bytes()
    except OSError as error:
        fail(f"cannot read {label}: {error}")
    if len(payload) != size:
        fail(f"{label} changed while it was read")
    return hashlib.sha256(payload).hexdigest()


def package_source_closure_sha256(
    repo_root: Path, sources: list[tuple[Path, str]]
) -> str:
    digest = hashlib.sha256(SOURCE_CLOSURE_DIGEST_DOMAIN)
    for path, text in sources:
        relative = path.relative_to(repo_root).as_posix().encode("utf-8")
        payload = text.encode("utf-8")
        digest.update(len(relative).to_bytes(4, "little"))
        digest.update(relative)
        digest.update(len(payload).to_bytes(8, "little"))
        digest.update(payload)
    return digest.hexdigest()


def parse_package_identity(
    manifest_path: Path, label: str
) -> tuple[dict[str, Any], str, str]:
    try:
        payload = manifest_path.read_bytes()
        if len(payload) > MAX_CARGO_MANIFEST_BYTES:
            fail(f"{label} exceeds the manifest byte bound")
        cargo = tomllib.loads(payload.decode("utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        fail(f"cannot parse {label}: {error}")
    package = require_object(cargo.get("package"), f"{label} [package]")
    package_name = require_string(package.get("name"), f"{label} package.name")
    package_version = require_string(package.get("version"), f"{label} package.version")
    return cargo, package_name, package_version


def effective_cargo_lock(repo_root: Path, package_root: Path, label: str) -> Path:
    directory = package_root
    while directory == repo_root or repo_root in directory.parents:
        candidate = directory / "Cargo.lock"
        if candidate.exists() or candidate.is_symlink():
            relative = candidate.relative_to(repo_root).as_posix()
            checked_path(repo_root, relative, f"{label}.compilerInput.cargoLockPath")
            return candidate
        manifest = directory / "Cargo.toml"
        if manifest.is_file():
            try:
                cargo = tomllib.loads(manifest.read_text(encoding="utf-8"))
            except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
                fail(f"cannot locate {label} Cargo.lock: {error}")
            if "workspace" in cargo:
                fail(f"{label} workspace root has no Cargo.lock: {directory}")
        if directory == repo_root:
            break
        directory = directory.parent
    fail(f"{label} package has no effective Cargo.lock within the repository")


def cargo_feature_closure(
    cargo: dict[str, Any], direct: list[str], default_features: bool, label: str
) -> list[str]:
    raw_features = cargo.get("features", {})
    features = require_object(raw_features, f"{label} [features]")
    pending = list(direct)
    if default_features and "default" in features:
        pending.append("default")
    enabled: set[str] = set()
    while pending:
        feature = pending.pop()
        if feature in enabled:
            continue
        if feature not in features:
            fail(f"{label} selects unknown Cargo feature: {feature}")
        enabled.add(feature)
        members = features[feature]
        if not isinstance(members, list):
            fail(f"{label} feature {feature} must be an array")
        for member in members:
            member = require_string(member, f"{label} feature {feature}[]")
            local = member.partition("?")[0]
            if local.startswith("dep:") or "/" in local:
                continue
            if local in features:
                pending.append(local)
    return sorted(enabled)


def fixture_input_contract_sha256(fixture: dict[str, Any]) -> str:
    compiler_input = fixture["compilerInput"]
    contract = {
        "fixtureId": fixture["fixtureId"],
        "target": fixture["target"],
        "matrix": fixture["matrix"],
        "compilerInput": {
            key: value
            for key, value in compiler_input.items()
            if key != "contractSha256"
        },
    }
    payload = json.dumps(
        contract,
        allow_nan=False,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("ascii")
    return hashlib.sha256(FIXTURE_INPUT_DIGEST_DOMAIN + payload).hexdigest()


def validate_compiler_input(
    repo_root: Path,
    fixture: dict[str, Any],
    label: str,
    package_cache: dict[str, dict[str, Any]] | None = None,
) -> dict[str, Any]:
    compiler_input = require_object(fixture["compilerInput"], f"{label}.compilerInput")
    require_exact_keys(compiler_input, COMPILER_INPUT_KEYS, f"{label}.compilerInput")
    package_manifest = checked_path(
        repo_root,
        compiler_input["packageManifest"],
        f"{label}.compilerInput.packageManifest",
    )
    if not package_manifest.endswith("/Cargo.toml"):
        fail(f"{label}.compilerInput.packageManifest must name a Cargo.toml")
    cached = None if package_cache is None else package_cache.get(package_manifest)
    if cached is None:
        manifest_path = repo_root.joinpath(*PurePosixPath(package_manifest).parts)
        package_root = manifest_path.parent
        cargo, package_name, package_version = parse_package_identity(
            manifest_path, f"{label}.compilerInput.packageManifest"
        )
        package_sources = package_rust_sources(repo_root, package_manifest, label)
        for path, text in package_sources:
            validate_rust_path_attributes(text, path, package_root, label)
            validate_rust_source_includes(text, path, package_root, label)
        cached = {
            "manifestPath": manifest_path,
            "packageRoot": package_root,
            "cargo": cargo,
            "packageName": package_name,
            "packageVersion": package_version,
            "manifestSha256": sha256_file(
                manifest_path, MAX_CARGO_MANIFEST_BYTES, f"{label} package manifest"
            ),
            "cargoLockPath": effective_cargo_lock(repo_root, package_root, label),
            "packageSources": package_sources,
            "attributedNames": {},
            "sourceClosureSha256": package_source_closure_sha256(
                repo_root, package_sources
            ),
        }
        if package_cache is not None:
            package_cache[package_manifest] = cached
    manifest_path = cached["manifestPath"]
    package_root = cached["packageRoot"]
    cargo = cached["cargo"]
    package_name = cached["packageName"]
    package_version = cached["packageVersion"]
    manifest_sha256 = require_string(
        compiler_input["packageManifestSha256"],
        f"{label}.compilerInput.packageManifestSha256",
    )
    if LOWER_SHA256.fullmatch(manifest_sha256) is None:
        fail(f"{label}.compilerInput.packageManifestSha256 is not lowercase SHA-256")
    if manifest_sha256 != cached["manifestSha256"]:
        fail(f"{label}.compilerInput.packageManifestSha256 is stale")

    cargo_lock_path = checked_path(
        repo_root,
        compiler_input["cargoLockPath"],
        f"{label}.compilerInput.cargoLockPath",
    )
    expected_cargo_lock_path = cached["cargoLockPath"].relative_to(repo_root).as_posix()
    if cargo_lock_path != expected_cargo_lock_path:
        fail(f"{label}.compilerInput.cargoLockPath is not the effective Cargo.lock")
    cargo_lock_sha256 = require_string(
        compiler_input["cargoLockSha256"],
        f"{label}.compilerInput.cargoLockSha256",
    )
    if LOWER_SHA256.fullmatch(cargo_lock_sha256) is None:
        fail(f"{label}.compilerInput.cargoLockSha256 is not lowercase SHA-256")
    if cargo_lock_sha256 != sha256_file(
        cached["cargoLockPath"], MAX_CARGO_LOCK_BYTES, f"{label} effective Cargo.lock"
    ):
        fail(f"{label}.compilerInput.cargoLockSha256 is stale")

    closure_sha256 = require_string(
        compiler_input["sourceClosureSha256"],
        f"{label}.compilerInput.sourceClosureSha256",
    )
    if LOWER_SHA256.fullmatch(closure_sha256) is None:
        fail(f"{label}.compilerInput.sourceClosureSha256 is not lowercase SHA-256")
    if closure_sha256 != cached["sourceClosureSha256"]:
        fail(f"{label}.compilerInput.sourceClosureSha256 is stale")

    source_paths = require_string_list(
        compiler_input["sourcePaths"], f"{label}.compilerInput.sourcePaths"
    )
    attributed_names: set[str] = set()
    for index, source_path in enumerate(source_paths):
        path = _validate_package_source_path(
            repo_root,
            package_root,
            source_path,
            f"{label}.compilerInput.sourcePaths[{index}]",
        )
        source_key = path.relative_to(package_root).as_posix()
        names = cached["attributedNames"].get(source_key)
        if names is None:
            try:
                source = path.read_text(encoding="utf-8")
            except (OSError, UnicodeError) as error:
                fail(f"cannot read {label} compiler source: {error}")
            names = ordinary_attributed_kernel_names(source)
            cached["attributedNames"][source_key] = names
        attributed_names.update(names)

    cargo_target = require_object(
        compiler_input["cargoTarget"], f"{label}.compilerInput.cargoTarget"
    )
    require_exact_keys(
        cargo_target, CARGO_TARGET_KEYS, f"{label}.compilerInput.cargoTarget"
    )
    if cargo_target["kind"] != "lib":
        fail(f"{label}.compilerInput.cargoTarget.kind must be lib")
    lib = cargo.get("lib", {})
    if not isinstance(lib, dict):
        fail(f"{label} Cargo [lib] must be a table")
    expected_name = lib.get("name", package_name.replace("-", "_"))
    expected_source = lib.get("path", "src/lib.rs")
    if cargo_target["name"] != expected_name:
        fail(f"{label}.compilerInput.cargoTarget.name differs from Cargo [lib]")
    if cargo_target["sourcePath"] != expected_source:
        fail(f"{label}.compilerInput.cargoTarget.sourcePath differs from Cargo [lib]")
    _package_path(package_root, expected_source, f"{label} Cargo [lib].path")

    if not isinstance(compiler_input["defaultFeatures"], bool):
        fail(f"{label}.compilerInput.defaultFeatures must be boolean")
    direct_features = require_string_array(
        compiler_input["features"], f"{label}.compilerInput.features"
    )
    if direct_features != sorted(direct_features):
        fail(f"{label}.compilerInput.features must be sorted")
    enabled_features = cargo_feature_closure(
        cargo, direct_features, compiler_input["defaultFeatures"], label
    )
    kernel_symbols = require_string_list(
        compiler_input["kernelSymbols"], f"{label}.compilerInput.kernelSymbols"
    )
    if kernel_symbols != sorted(kernel_symbols):
        fail(f"{label}.compilerInput.kernelSymbols must be sorted")
    for symbol in kernel_symbols:
        if RUST_IDENTIFIER.fullmatch(symbol) is None:
            fail(f"{label}.compilerInput.kernelSymbols contains an invalid Rust symbol")
        if symbol not in attributed_names:
            fail(
                f"{label}.compilerInput.kernelSymbols is not attributed in sourcePaths: {symbol}"
            )
    contract_sha256 = require_string(
        compiler_input["contractSha256"], f"{label}.compilerInput.contractSha256"
    )
    if contract_sha256 != fixture_input_contract_sha256(fixture):
        fail(f"{label}.compilerInput.contractSha256 is stale")
    return {
        "packageName": package_name,
        "packageVersion": package_version,
        "manifestSha256": manifest_sha256,
        "cargoLockSha256": cargo_lock_sha256,
        "sourceClosureSha256": closure_sha256,
        "enabledFeatures": enabled_features,
        "primarySourceSha256": sha256_file(
            package_root / expected_source,
            MAX_ATTRIBUTED_SOURCE_BYTES,
            f"{label} primary source",
        ),
    }


def validate_rust_path_attributes(
    source: str, source_path: Path, package_root: Path, label: str
) -> None:
    code = _rust_code_without_comments_and_literals(source)
    pairs = _rust_delimiters(code)
    for attribute in ATTRIBUTE_START.finditer(code):
        closing, _ = _rust_attribute(code, attribute.start(), pairs)
        original_opening = code.find("[", attribute.start(), closing)
        body = source[original_opening + 1 : closing - 1]
        match = re.match(
            r"\s*path\s*=\s*(?:\"(?P<quoted>(?:\\.|[^\"\\])*)\"|"
            r"r(?P<hashes>#{0,255})\"(?P<raw>.*?)\"(?P=hashes))",
            body,
            re.DOTALL,
        )
        if match is None:
            continue
        value = match.group("quoted") or match.group("raw") or ""
        relative = PurePosixPath(value.replace("\\", ""))
        if relative.is_absolute():
            fail(f"{label} has a module path escaping its package root: {value!r}")
        candidate = source_path.parent.joinpath(*relative.parts)
        try:
            resolved = candidate.resolve(strict=False)
            resolved.relative_to(package_root)
        except ValueError:
            fail(f"{label} has a module path escaping its package root: {value!r}")
        package_relative = resolved.relative_to(package_root)
        if (
            package_relative.parts
            and package_relative.parts[0] in IGNORED_PACKAGE_DIRECTORIES
        ):
            fail(f"{label} references a generated package path: {value!r}")
        if not candidate.is_file() or candidate.is_symlink():
            fail(f"{label} module path does not name a regular file: {value!r}")


def validate_rust_source_includes(
    source: str, source_path: Path, package_root: Path, label: str
) -> None:
    code = _rust_code_without_comments_and_literals(source)
    pairs = _rust_delimiters(code)
    for include in SOURCE_INCLUDE_START.finditer(code):
        cursor = include.end()
        while cursor < len(code) and code[cursor].isspace():
            cursor += 1
        if cursor >= len(code) or code[cursor] != "(" or cursor not in pairs:
            fail(f"{label} has an unterminated include! invocation")
        closing = pairs[cursor]
        body = source[cursor + 1 : closing - 1]
        literal = re.fullmatch(
            r"\s*(?:\"(?P<quoted>(?:\\.|[^\"\\])*)\"|"
            r"r(?P<hashes>#{0,255})\"(?P<raw>.*?)\"(?P=hashes))\s*",
            body,
            re.DOTALL,
        )
        if literal is None:
            fail(f"{label} has a non-literal include! path")
        value = literal.group("quoted") or literal.group("raw") or ""
        value = value.replace("\\\\", "\\").replace('\\"', '"')
        relative = PurePosixPath(value)
        if relative.is_absolute():
            fail(f"{label} has an include! path escaping its package root: {value!r}")
        candidate = source_path.parent.joinpath(*relative.parts)
        try:
            resolved = candidate.resolve(strict=False)
            package_relative = resolved.relative_to(package_root)
        except ValueError:
            fail(f"{label} has an include! path escaping its package root: {value!r}")
        if (
            package_relative.parts
            and package_relative.parts[0] in IGNORED_PACKAGE_DIRECTORIES
        ):
            fail(f"{label} references a generated package path: {value!r}")
        if not candidate.is_file():
            source_relative = source_path.relative_to(package_root)
            if source_relative.parts[:2] != ("tests", "fixtures"):
                fail(f"{label} include! path is not a regular file: {value!r}")
        if candidate.is_symlink():
            fail(f"{label} include! path names a symlink: {value!r}")


def require_bounded_command_string(value: Any, label: str) -> str:
    text = require_string(value, label)
    if len(text) > 256 or "\0" in text or "\n" in text or "\r" in text:
        fail(f"{label} exceeds the command string bounds")
    if ".." in PurePosixPath(text).parts:
        fail(f"{label} contains path traversal")
    return text



def bounded_list(value: Any, label: str, maximum: int) -> list[Any]:
    if not isinstance(value, list) or not value or len(value) > maximum:
        fail(f"{label} must contain 1..{maximum} records")
    return value


def expected_path(value: Any, label: str) -> str:
    """A pending obligation can name an absent file, but not escape the repo."""
    text = require_bounded_command_string(value, label)
    path = PurePosixPath(text)
    if path.is_absolute() or str(path) != text or text == ".":
        fail(f"{label} is not a canonical repository-relative file")
    return text


def require_digest(value: Any, label: str) -> None:
    if not isinstance(value, str) or LOWER_SHA256.fullmatch(value) is None:
        fail(f"{label} must be a lowercase SHA-256")


def validate_pending_simulation(value: Any, label: str) -> None:
    simulation = require_object(value, label)
    require_exact_keys(simulation, SIMULATION_KEYS, label)
    status = simulation["status"]
    if status not in {"pending-reconciliation", "pending-design"}:
        fail(f"{label} must remain explicitly pending")
    if simulation["bundleVersion"] != 7 or simulation["canonicalKirVersion"] != 12:
        fail(f"{label} must retain the Bundle V7 / KIR V12 obligation")
    for kind in ("request", "expectation"):
        expected_path(simulation[f"{kind}Path"], f"{label}.{kind}Path")
        digest, size = simulation[f"{kind}Sha256"], simulation[f"{kind}Bytes"]
        if status == "pending-design":
            if digest is not None or size is not None:
                fail(f"{label} undesigned oracle cannot claim content pins")
        else:
            require_digest(digest, f"{label}.{kind}Sha256")
            if type(size) is not int or not 0 < size <= MAX_SIMULATION_DOCUMENT_BYTES:
                fail(f"{label}.{kind}Bytes exceeds the pending document bound")


def matrix_record(fixture: dict[str, Any]) -> str:
    matrix = fixture["matrix"]
    return "|".join(
        [
            matrix["caseId"],
            matrix["runnerPath"],
            matrix["artifactName"],
            next(iter(matrix["runnerArguments"]), ""),
            next(iter(matrix["environment"]), "").partition("=")[2],
            fixture["fixtureId"],
        ]
    )


def validate_matrix(repo_root: Path, fixture: dict[str, Any], label: str) -> None:
    matrix = require_object(fixture["matrix"], f"{label}.matrix")
    require_exact_keys(matrix, MATRIX_KEYS, f"{label}.matrix")
    for key in ("caseId", "artifactName"):
        if not isinstance(matrix[key], str) or re.fullmatch(
            r"[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}", matrix[key]
        ) is None:
            fail(f"{label}.matrix.{key} is not a bounded identifier")
    if not matrix["artifactName"].endswith(".hsaco"):
        fail(f"{label} must name an expected HSACO artifact")
    runner = checked_path(repo_root, matrix["runnerPath"], f"{label}.matrix.runnerPath")
    if not runner.startswith("examples/") or not runner.endswith(".sh"):
        fail(f"{label} must use an ordinary-source example runner")
    arguments = require_string_array(matrix["runnerArguments"], f"{label}.arguments")
    if len(arguments) > 1 or any(
        re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_-]{0,127}", arg) is None
        for arg in arguments
    ):
        fail(f"{label} runner arguments exceed the matrix record contract")
    environment = require_string_array(matrix["environment"], f"{label}.environment")
    if len(environment) > 1 or any(
        re.fullmatch(
            r"FE2O3_GFX950_SYSTEMS_ABLATION_VARIANT="
            r"(expert-serial|speculative-recompute-prefix|ngram-reverse-probe|muon-broadcast16)",
            item,
        ) is None
        for item in environment
    ):
        fail(f"{label} environment is not a supported ablation selection")
    if environment and fixture["target"] != "gfx950":
        fail(f"{label} gfx950 ablation selection used for another target")
    if "|" in runner or "\n" in runner or "\r" in runner:
        fail(f"{label} runner cannot be represented in a matrix record")


def validate_qualification(
    value: Any, entries: dict[str, Any], fixtures: dict[str, Any]
) -> None:
    qualification = require_object(value, "qualification")
    require_exact_keys(qualification, QUALIFICATION_KEYS, "qualification")
    if qualification["status"] != "pending":
        fail("source contracts cannot claim qualification")
    expected_path(qualification["evidenceSchemaPath"], "qualification.evidenceSchemaPath")
    targets = set()
    for hardware in bounded_list(qualification["hardwareTargets"], "hardwareTargets", 2):
        require_exact_keys(
            require_object(hardware, "hardware target"), HARDWARE_TARGET_KEYS, "hardware target"
        )
        target = require_target(hardware["target"], "hardware target")
        if target in targets:
            fail("duplicate hardware target")
        targets.add(target)
        if hardware["deterministicSemanticRunnerAvailability"] != "unavailable":
            fail("hardware qualification must remain unavailable")
        require_string(hardware["reason"], "hardware reason")
    if targets != ALLOWED_TARGETS:
        fail("both hardware targets must retain explicit obligations")
    suites = set()
    covered: set[tuple[str, str, str]] = set()
    for suite in bounded_list(qualification["suites"], "qualification.suites", 512):
        require_exact_keys(require_object(suite, "suite"), SUITE_KEYS, "suite")
        suite_id = require_string(suite["suiteId"], "suiteId")
        if suite_id in suites:
            fail("duplicate suiteId")
        suites.add(suite_id)
        gate = suite["gate"]
        if gate not in {"semantic-simulation", "cpu-reference"}:
            fail("unsupported pending suite gate")
        if suite["availability"] != "pending":
            fail("a retained suite command is not an available or passed adapter")
        require_string(suite["unavailableReason"], "suite pending reason")
        command = require_object(suite["command"], "suite.command")
        require_exact_keys(command, SUITE_COMMAND_KEYS, "suite.command")
        expected_path(command["executable"], "suite.command.executable")
        for key, maximum in (
            ("arguments", MAX_SUITE_COMMAND_ARGUMENTS),
            ("environment", MAX_SUITE_COMMAND_ENVIRONMENT),
        ):
            values = require_string_array(command[key], f"suite.command.{key}")
            if len(values) > maximum:
                fail(f"suite.command.{key} exceeds its bound")
            for item in values:
                require_bounded_command_string(item, f"suite.command.{key}[]")
        if command["workingDirectory"] != ".":
            expected_path(command["workingDirectory"], "suite.command.workingDirectory")
        timeout = command["timeoutSeconds"]
        if type(timeout) is not int or not 0 < timeout <= 86400:
            fail("suite.command.timeoutSeconds exceeds its bound")
        local_coverage = set()
        for coverage in bounded_list(suite["coverage"], "suite.coverage", 256):
            require_exact_keys(
                require_object(coverage, "suite coverage"), SUITE_COVERAGE_KEYS, "suite coverage"
            )
            lesson = coverage["lessonId"]
            if lesson not in entries:
                fail("suite references an unknown lesson")
            ids = require_string_list(coverage["fixtureIds"], "suite fixtureIds")
            for fixture_id in ids:
                if fixture_id not in fixtures or fixture_id not in entries[lesson]["compilerFixtureIds"]:
                    fail("suite fixture is not bound to its lesson")
                if gate not in entries[lesson]["requiredGates"]:
                    fail("suite does not correspond to a required lesson gate")
                key = (lesson, fixture_id, gate)
                if key in local_coverage:
                    fail("duplicate suite coverage")
                local_coverage.add(key)
                covered.add(key)
    for lesson, entry in entries.items():
        for fixture_id in entry["compilerFixtureIds"]:
            for gate in set(entry["requiredGates"]) & {"semantic-simulation", "cpu-reference"}:
                if (lesson, fixture_id, gate) not in covered:
                    fail(f"missing pending {gate} obligation for {lesson}/{fixture_id}")


def validate_manifest(repo_root: Path, manifest: Any) -> dict[str, Any]:
    require_exact_keys(require_object(manifest, "manifest"), TOP_LEVEL_KEYS, "manifest")
    if manifest["schema"] != "fe2o3-tutorial-kernel-source-contract-v1":
        fail("expected a source-contract schema, not a release or evidence manifest")
    if manifest["roadmapIssue"] != "https://github.com/harsh-nod/fe2o3/issues/271":
        fail("unexpected roadmap issue")
    if manifest["baseline"] != {
        "compilerCommit": None,
        "compilerTree": None,
        "status": "unqualified-source-contract",
    }:
        fail("source-content pins cannot set an accepted compiler baseline")
    recovery = require_object(manifest["recovery"], "recovery")
    require_exact_keys(
        recovery,
        {"donorCompilerCommit", "donorManifestSha256", "sourceBaseCommit", "sourceState"},
        "recovery",
    )
    for key in ("donorCompilerCommit", "sourceBaseCommit"):
        if not isinstance(recovery[key], str) or re.fullmatch(r"[0-9a-f]{40}", recovery[key]) is None:
            fail(f"recovery.{key} is not a commit name")
    require_digest(recovery["donorManifestSha256"], "recovery.donorManifestSha256")
    if recovery["sourceState"] != "dirty-proposal-not-qualified":
        fail("recovery is provenance, not a qualification receipt")
    if manifest["productionContract"] != {
        "pipelineEntry": "rustc-codegen-fe2o3::production_pipeline",
        "requiredPolicyVersion": 4,
        "requiresFinalOptimizedGraphVerification": True,
        "allowsPipelineSelection": False,
        "allowsFallback": False,
    }:
        fail("the sole production pipeline, policy and final-verification obligations are fixed")
    fixtures: dict[str, Any] = {}
    cases = set()
    cache: dict[str, dict[str, Any]] = {}
    for fixture in bounded_list(manifest["compilerFixtures"], "compilerFixtures", 256):
        require_exact_keys(require_object(fixture, "fixture"), FIXTURE_KEYS, "fixture")
        fixture_id = require_string(fixture["fixtureId"], "fixtureId")
        if re.fullmatch(r"[a-z0-9][a-z0-9-]{0,127}", fixture_id) is None or fixture_id in fixtures:
            fail("duplicate or invalid fixtureId")
        target = require_target(fixture["target"], "fixture target")
        validate_matrix(repo_root, fixture, fixture_id)
        case = (target, fixture["matrix"]["caseId"])
        if case in cases:
            fail("duplicate target/caseId")
        cases.add(case)
        if fixture["testId"] != f"kernel-compile-matrix/{target}/{case[1]}":
            fail(f"{fixture_id} testId does not identify its matrix case")
        if fixture["testPath"] != "scripts/tests/kernel-compile-matrix.sh":
            fail(f"{fixture_id} must retain the compiler-owned matrix harness")
        checked_path(repo_root, fixture["testPath"], "fixture.testPath")
        validate_compiler_input(repo_root, fixture, fixture_id, cache)
        validate_pending_simulation(fixture["simulation"], f"{fixture_id}.simulation")
        fixtures[fixture_id] = fixture
    entries: dict[str, Any] = {}
    associated = set()
    package_kernels: dict[str, bool] = {}
    for entry in bounded_list(manifest["entries"], "entries", 256):
        require_exact_keys(require_object(entry, "entry"), ENTRY_KEYS, "entry")
        lesson = require_string(entry["lessonId"], "lessonId")
        if lesson in entries:
            fail("duplicate lessonId")
        evidence = entry["siteEvidenceKind"]
        if evidence not in ALLOWED_EVIDENCE:
            fail(f"{lesson} has an unknown site evidence kind")
        classification = entry["classification"]
        if classification not in {"compiler-produced", "simulator-only", "design-only", "external-baseline"}:
            fail(f"{lesson} has an unknown classification")
        if evidence in COMPILER_EVIDENCE and classification != "compiler-produced":
            fail(f"{lesson} runnable compiler obligations cannot be downgraded")
        package = checked_path(repo_root, entry["packageManifest"], f"{lesson}.packageManifest")
        sources = require_string_list(entry["sourcePaths"], f"{lesson}.sourcePaths")
        has_kernel = False
        for source in sources:
            path = _validate_package_source_path(repo_root, (repo_root / package).parent, source, lesson)
            if path.stat().st_size > MAX_ATTRIBUTED_SOURCE_BYTES:
                fail(f"{lesson} attributed source exceeds its bound")
            has_kernel |= source_contains_ordinary_attributed_kernel(path.read_text(encoding="utf-8"))
        has_kernel |= _package_has_ordinary_kernel(
            repo_root, package, lesson, cache, package_kernels
        )
        if has_kernel and classification != "compiler-produced":
            fail(f"{lesson} ordinary kernel cannot be downgraded")
        ids = require_string_array(entry["compilerFixtureIds"], f"{lesson}.compilerFixtureIds")
        gates = require_string_list(entry["requiredGates"], f"{lesson}.requiredGates")
        if not set(gates) <= ALLOWED_GATES:
            fail(f"{lesson} has an unknown gate")
        if classification == "compiler-produced":
            if not ids or not {"production-compile", "semantic-simulation"} <= set(gates):
                fail(f"{lesson} must retain compile and simulation obligations")
            if not any(fixtures.get(i, {}).get("compilerInput", {}).get("packageManifest") == package for i in ids):
                fail(f"{lesson} has no fixture for its ordinary-source package")
        for fixture_id in ids:
            if fixture_id not in fixtures:
                fail(f"{lesson} references an unknown fixture")
            associated.add(fixture_id)
        entries[lesson] = entry
    if associated != set(fixtures):
        fail("every compiler fixture must retain a lesson/scope association")
    validate_qualification(manifest["qualification"], entries, fixtures)
    return fixtures


def _package_has_ordinary_kernel(
    repo_root: Path,
    package: str,
    label: str,
    compiler_packages: dict[str, dict[str, Any]],
    package_kernels: dict[str, bool],
) -> bool:
    # One invocation observes one package snapshot. Entry sourcePaths are still
    # independently checked and scanned; this is not atomic filesystem admission.
    if package in package_kernels:
        return package_kernels[package]
    cached = compiler_packages.get(package)
    package_root = (repo_root / package).parent
    if cached is None:
        sources = package_rust_sources(repo_root, package, label)
        attributed_names: dict[str, list[str]] = {}
    else:
        sources = cached["packageSources"]
        attributed_names = cached["attributedNames"]
    has_kernel = False
    for path, text in sources:
        key = path.relative_to(package_root).as_posix()
        names = attributed_names.get(key)
        if names is None:
            names = ordinary_attributed_kernel_names(text)
            attributed_names[key] = names
        if names:
            has_kernel = True
            break
    package_kernels[package] = has_kernel
    return has_kernel


def reject_duplicate_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_manifest(path: Path) -> Any:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_CARGO_MANIFEST_BYTES:
        fail("manifest must be a bounded regular file")
    try:
        return json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_json_object,
            parse_constant=lambda value: fail(f"non-finite JSON constant: {value}"),
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        fail(f"cannot read manifest: {error}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--emit-matrix", choices=sorted(ALLOWED_TARGETS))
    parser.add_argument("--require-qualified", action="store_true")
    arguments = parser.parse_args()
    root = arguments.repo_root.resolve()
    manifest = load_manifest(arguments.manifest or root / "config/tutorial-kernel-manifest-v1.json")
    fixtures = validate_manifest(root, manifest)
    if arguments.require_qualified:
        fail("qualification receipts and policy/final-graph evidence are not implemented by source contracts")
    if arguments.emit_matrix:
        records = [fixture for fixture in fixtures.values() if fixture["target"] == arguments.emit_matrix]
        if not records:
            fail("selected target has no compiler fixture obligations")
        for fixture in sorted(records, key=lambda fixture: fixture["matrix"]["caseId"]):
            print(matrix_record(fixture))
    else:
        print(
            f"SOURCE CONTRACT VALID fixtures={len(fixtures)} qualified=false "
            "compiler_inputs=expected semantic_oracles=pending policy_verification=pending"
        )


if __name__ == "__main__":
    main()
