#!/usr/bin/env python3
"""Validate the shared, content-addressed tutorial kernel manifest.

The manifest enumerates release qualification. It is deliberately not an input
to compiler dispatch, optimization, legalization, or lowering.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import stat
import subprocess
import sys
import tomllib
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
CONFIG = REPO_ROOT / "config"
MANIFEST_NAME = "tutorial-kernel-manifest-v1.json"
SCHEMA_NAME = "tutorial-kernel-manifest-schema-v1.json"
EXPECTATION_SCHEMA_NAME = "tutorial-semantic-expectation-schema-v1.json"
EVIDENCE_SCHEMA_NAME = "tutorial-semantic-qualification-evidence-schema-v1.json"
CAPABILITY_QUALIFICATION_SCHEMA_NAME = (
    "tutorial-capability-qualification-batch-schema-v1.json"
)
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_ID = re.compile(r"[0-9a-f]{40}\Z")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
RUST_IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
TARGET = re.compile(r"gfx[0-9]{3}\Z")
DIAGNOSTIC = re.compile(r"FE2O3-[A-Z]+-[0-9]{3}\Z")
TOP_LEVEL_KEYS = {
    "baseline",
    "capabilityContract",
    "capabilityKernels",
    "compilerFixtures",
    "entries",
    "productionContract",
    "qualification",
    "roadmapIssue",
    "schema",
}
CAPABILITY_KERNEL_KEYS = {
    "capabilityClosure",
    "fixtureId",
    "hardwareCommand",
    "kernelSymbol",
    "lessonIds",
    "negativeFixtureCoverage",
    "productionCapabilityPath",
    "proofRequirements",
    "requiredProperties",
    "simulatorCommand",
    "targetMatrix",
}
COMPILER_FIXTURE_KEYS = {
    "compilerInput",
    "fixtureId",
    "matrix",
    "target",
    "testId",
    "testPath",
}
COMPILER_INPUT_KEYS = {
    "cargoLockPath",
    "cargoLockSha256",
    "cargoTarget",
    "contractSha256",
    "defaultFeatures",
    "features",
    "kernelSymbols",
    "packageManifest",
    "packageManifestSha256",
    "sourceClosureSha256",
    "sourcePaths",
}
ENTRY_KEYS = {
    "classification",
    "compilerFixtureIds",
    "lessonId",
    "packageManifest",
    "requiredGates",
    "siteEvidenceKind",
    "sourcePaths",
}
PROOF_REQUIREMENT_KEYS = {
    "checkerSha256",
    "evidenceSha256",
    "obligationSetSha256",
    "properties",
    "status",
}
NEGATIVE_CASE_KEYS = {
    "category",
    "diagnosticCode",
    "failureStage",
    "fixtureId",
    "testPath",
}
NEGATIVE_FAILURE_STAGES = {
    "artifact-inspection",
    "host-preparation",
    "kir-verification",
    "macro-authentication",
    "mir-admission",
    "runtime-completion",
    "sealed-verifier",
    "static-analysis",
    "target-legalization",
}
ENTRY_CLASSIFICATIONS = {
    "compiler-produced",
    "design-only",
    "external-baseline",
    "legacy-compiler-produced",
    "simulator-only",
    "unsupported",
}
ENTRY_GATES = {
    "cpu-reference",
    "hardware",
    "production-compile",
    "semantic-simulation",
}
QUALIFICATION_GATES = {"cpu-reference", "semantic-simulation"}
SITE_EVIDENCE_KINDS = {
    "compiler-checked",
    "compiler-hsaco-observed",
    "design-only",
    "gpu-observed",
    "runnable-now",
    "source-example",
    "source-model-verified",
    "source-tested",
}
CAPABILITY_REQUIREMENTS = {
    "abi.kernel-entry",
    "address-space.global",
    "address-space.private",
    "address-space.workgroup",
    "async-copy.global-to-workgroup",
    "async-wait.completion",
    "atomic.workgroup",
    "barrier.workgroup",
    "collective.subgroup",
    "collective.workgroup",
    "execution.invocation",
    "matrix.multiply-accumulate",
    "memory.global.read",
    "memory.global.write",
    "memory.private",
    "memory.workgroup",
    "numerical.bf16",
    "numerical.f16",
    "numerical.f32",
    "numerical.fp4",
    "numerical.fp8",
    "numerical.integer",
    "object.executable",
    "resource.launch",
    "subgroup.width.64",
}
REQUIRED_PROPERTIES = {
    "abi-conformance",
    "address-bounds",
    "alias-legality",
    "artifact-currentness",
    "atomic-legality",
    "barrier-convergence",
    "capability-provenance",
    "collective-participation",
    "effect-legality",
    "functional-refinement",
    "happens-before",
    "initialized-before-read",
    "launch-preconditions",
    "machine-refinement",
    "numerical-policy",
    "output-injectivity",
    "race-freedom",
    "resource-legality",
    "source-mir-kir-refinement",
    "target-capability-closure",
    "tensor-layout",
    "workgroup-memory-epochs",
}
PRODUCTION_EVIDENCE_KEYS = {
    "artifactSha256",
    "artifactInspectionSha256",
    "capabilityAnalysisSha256",
    "capabilityClosureSha256",
    "compilerCommit",
    "compilerPolicySha256",
    "compilerTree",
    "finalOptimizedKirSha256",
    "hardwareEvidenceSha256",
    "hostAdmissionSha256",
    "launchContractSha256",
    "loweringIdentitySha256",
    "machineRefinementSha256",
    "negativeFixtureSetSha256",
    "numericalPolicySha256",
    "proofCheckerSha256",
    "proofEvidenceSha256",
    "proofObligationSetSha256",
    "simulatorEvidenceSha256",
    "sourceMirIdentitySha256",
    "sourceMirToKirRefinementSha256",
    "targetCapabilityDecisionSha256",
    "targetIdentitySha256",
}
PRODUCTION_PROOF_PROPERTIES = {
    "capability-provenance",
    "functional-refinement",
    "machine-refinement",
    "source-mir-kir-refinement",
}
PRODUCTION_NEGATIVE_CATEGORIES = {
    "capability-forgery",
    "capability-substitution",
    "evidence",
    "host-invocation",
    "launch",
    "stale-output",
    "target",
    "unsupported-operation",
}
SOURCE_CLOSURE_DIGEST_DOMAIN = b"fe2o3-tutorial-package-rust-source-closure-v1\0"
FIXTURE_INPUT_DIGEST_DOMAIN = b"fe2o3-tutorial-fixture-compiler-input-v1\0"
NEGATIVE_FIXTURE_DIGEST_DOMAIN = b"fe2o3-tutorial-capability-negative-fixtures-v1\0"
MAX_CARGO_MANIFEST_BYTES = 1024 * 1024
MAX_CARGO_LOCK_BYTES = 8 * 1024 * 1024
MAX_PACKAGE_SOURCE_FILES = 4096
MAX_PACKAGE_SOURCE_BYTES = 64 * 1024 * 1024
IGNORED_PACKAGE_DIRECTORIES = {"target"}
HARDWARE_PROTOCOL_ENVIRONMENT = (
    "FE2O3_TUTORIAL_HARDWARE_PROTOCOL=authenticated-v1"
)
HARDWARE_RUNNER_PROLOGUE = (
    b"#!/usr/bin/env bash\n"
    b'if ! source "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." '
    b'&& pwd -P)/scripts/tutorial-hardware-runner.sh"; then\n'
    b"    exit 1\n"
    b"fi\n"
    b'fe2o3_tutorial_hardware_entry "$0" "$@" || exit\n'
)
UNOBSERVED_HARDWARE_REASON = "awaiting-signed-target-matched-hardware-observation"


class ManifestError(ValueError):
    """The shared manifest cannot be trusted as release metadata."""


def _fail(message: str) -> None:
    raise ManifestError(message)


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _array(value: Any, label: str, *, nonempty: bool = False) -> list[Any]:
    if not isinstance(value, list) or (nonempty and not value):
        qualifier = "non-empty " if nonempty else ""
        _fail(f"{label} must be a {qualifier}array")
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        _fail(
            f"{label} fields differ: missing={sorted(expected - actual)!r} "
            f"extra={sorted(actual - expected)!r}"
        )


def _unique_strings(
    value: Any, label: str, *, nonempty: bool = False
) -> list[str]:
    values = _array(value, label, nonempty=nonempty)
    if any(not isinstance(item, str) or not item for item in values):
        _fail(f"{label} must contain non-empty strings")
    if len(values) != len(set(values)):
        _fail(f"{label} contains duplicates")
    return values


def _sorted_unique_strings(
    value: Any, label: str, *, nonempty: bool = False
) -> list[str]:
    values = _unique_strings(value, label, nonempty=nonempty)
    if values != sorted(values):
        _fail(f"{label} must be sorted")
    return values


def _sha256(value: Any, label: str) -> str:
    value = _string(value, label)
    if SHA256.fullmatch(value) is None:
        _fail(f"{label} is not a lowercase SHA-256 digest")
    return value


def _relative_path(value: Any, label: str) -> str:
    value = _string(value, label)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or "\\" in value:
        _fail(f"{label} is not a portable repository-relative path")
    return value


def _load_json_unique(path: Path) -> tuple[bytes, dict[str, Any]]:
    try:
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or path.is_symlink():
            _fail(f"{path} must be a regular non-symlink file")
        raw = path.read_bytes()
        if not raw or len(raw) > 4 * 1024 * 1024:
            _fail(f"{path} has an invalid byte length")

        def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
            result: dict[str, Any] = {}
            for key, value in pairs:
                if key in result:
                    _fail(f"{path} contains duplicate JSON key {key!r}")
                result[key] = value
            return result

        decoded = json.loads(raw, object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot load {path}: {error}")
    return raw, _object(decoded, str(path))


def _read_content_addressed(path: Path, digest_path: Path) -> bytes:
    raw, _ = _load_json_unique(path)
    try:
        digest_metadata = digest_path.lstat()
        if not stat.S_ISREG(digest_metadata.st_mode) or digest_path.is_symlink():
            _fail(f"{digest_path} must be a regular non-symlink file")
        digest_record = digest_path.read_text(encoding="ascii")
    except (OSError, UnicodeError) as error:
        _fail(f"cannot read {digest_path}: {error}")
    expected_record = f"{hashlib.sha256(raw).hexdigest()}  config/{path.name}\n"
    if digest_record != expected_record:
        _fail(f"{digest_path} does not exactly address config/{path.name}")
    return raw


def _index(records: Any, key: str, label: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for offset, raw in enumerate(_array(records, label, nonempty=True)):
        record = _object(raw, f"{label}[{offset}]")
        identity = _string(record.get(key), f"{label}[{offset}].{key}")
        if identity in result:
            _fail(f"duplicate {label} identity {identity!r}")
        result[identity] = record
    return result


def _validate_command(record: dict[str, Any], label: str) -> None:
    _exact_keys(
        record,
        {"arguments", "environment", "executable", "timeoutSeconds", "workingDirectory"},
        label,
    )
    _relative_path(record["executable"], f"{label}.executable")
    arguments = _array(record["arguments"], f"{label}.arguments")
    if any(not isinstance(item, str) or not item for item in arguments):
        _fail(f"{label}.arguments must contain non-empty strings")
    _unique_strings(record["environment"], f"{label}.environment")
    if record["workingDirectory"] != ".":
        _fail(f"{label}.workingDirectory must be '.'")
    timeout = record["timeoutSeconds"]
    if not isinstance(timeout, int) or isinstance(timeout, bool) or timeout <= 0:
        _fail(f"{label}.timeoutSeconds must be positive")


def _canonical_json_sha256(domain: bytes, value: Any) -> str:
    payload = json.dumps(
        value,
        allow_nan=False,
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("ascii")
    return hashlib.sha256(domain + payload).hexdigest()


def _fixture_input_contract_sha256(fixture: dict[str, Any]) -> str:
    compiler_input = _object(fixture.get("compilerInput"), "fixture.compilerInput")
    return _canonical_json_sha256(
        FIXTURE_INPUT_DIGEST_DOMAIN,
        {
            "fixtureId": fixture.get("fixtureId"),
            "target": fixture.get("target"),
            "matrix": fixture.get("matrix"),
            "compilerInput": {
                key: value
                for key, value in compiler_input.items()
                if key != "contractSha256"
            },
        },
    )


def _validate_capability_command(
    raw: Any, label: str, target: str, *, complete: bool
) -> None:
    record = _object(raw, label)
    _exact_keys(
        record,
        {"command", "evidenceSha256", "reasonCode", "status", "subjectSha256", "target"},
        label,
    )
    if record["target"] != target:
        _fail(f"{label} target differs from its compiler fixture")
    if complete:
        if record["status"] != "capability-path-qualified" or record["reasonCode"] is not None:
            _fail(f"{label} is not capability-path-qualified")
        _sha256(record["subjectSha256"], f"{label}.subjectSha256")
        _sha256(record["evidenceSha256"], f"{label}.evidenceSha256")
        _validate_command(_object(record["command"], f"{label}.command"), f"{label}.command")
        return
    status = record["status"]
    if status == "unavailable":
        if (
            record["command"] is not None
            or record["evidenceSha256"] is not None
            or record["subjectSha256"] is not None
            or not isinstance(record["reasonCode"], str)
            or not record["reasonCode"]
        ):
            _fail(f"{label} unavailable state is malformed")
        return
    if status not in {"available-legacy-only", "available-authenticated-unobserved"}:
        _fail(f"{label} status is invalid")
    _validate_command(_object(record["command"], f"{label}.command"), f"{label}.command")
    if record["evidenceSha256"] is not None or record["subjectSha256"] is not None:
        _fail(f"{label} claims evidence without qualification")
    if status == "available-authenticated-unobserved":
        if record["reasonCode"] != UNOBSERVED_HARDWARE_REASON:
            _fail(f"{label} has an invalid authenticated-unobserved reason")
        if HARDWARE_PROTOCOL_ENVIRONMENT not in record["command"]["environment"]:
            _fail(f"{label} bypasses the authenticated hardware protocol")
    elif not isinstance(record["reasonCode"], str) or not record["reasonCode"]:
        _fail(f"{label} legacy state has no reason")


def _required_negative_categories(required_properties: set[str]) -> set[str]:
    categories = set(PRODUCTION_NEGATIVE_CATEGORIES)
    if "abi-conformance" in required_properties:
        categories.add("abi")
    if "address-bounds" in required_properties:
        categories.add("bounds")
    if required_properties & {"alias-legality", "output-injectivity", "race-freedom"}:
        categories.add("alias")
    if "initialized-before-read" in required_properties:
        categories.add("initialization")
    if required_properties & {
        "atomic-legality",
        "barrier-convergence",
        "collective-participation",
        "happens-before",
        "workgroup-memory-epochs",
    }:
        categories.add("synchronization")
    if "effect-legality" in required_properties:
        categories.add("raw-pointer")
    return categories


def _validate_negative_case(raw: Any, label: str) -> dict[str, Any]:
    case = _object(raw, label)
    _exact_keys(case, NEGATIVE_CASE_KEYS, label)
    fixture_id = _string(case["fixtureId"], f"{label}.fixtureId")
    if SLUG.fullmatch(fixture_id) is None:
        _fail(f"{label}.fixtureId is not a slug")
    category = _string(case["category"], f"{label}.category")
    if category not in PRODUCTION_NEGATIVE_CATEGORIES | {
        "abi",
        "alias",
        "bounds",
        "initialization",
        "raw-pointer",
        "synchronization",
    }:
        _fail(f"{label}.category is invalid")
    diagnostic = _string(case["diagnosticCode"], f"{label}.diagnosticCode")
    if DIAGNOSTIC.fullmatch(diagnostic) is None:
        _fail(f"{label}.diagnosticCode is invalid")
    if case["failureStage"] not in NEGATIVE_FAILURE_STAGES:
        _fail(f"{label}.failureStage is invalid")
    _relative_path(case["testPath"], f"{label}.testPath")
    return case


def _validate_kernel_shape(
    record: dict[str, Any], label: str, fixture: dict[str, Any]
) -> None:
    _exact_keys(record, CAPABILITY_KERNEL_KEYS, label)
    if record["fixtureId"] != fixture["fixtureId"]:
        _fail(f"{label} fixture identity differs")
    symbol = _string(record["kernelSymbol"], f"{label}.kernelSymbol")
    if RUST_IDENTIFIER.fullmatch(symbol) is None:
        _fail(f"{label}.kernelSymbol is not a Rust identifier")
    _sorted_unique_strings(record["lessonIds"], f"{label}.lessonIds", nonempty=True)
    required_properties = _sorted_unique_strings(
        record["requiredProperties"], f"{label}.requiredProperties", nonempty=True
    )
    if not set(required_properties) <= REQUIRED_PROPERTIES:
        _fail(f"{label}.requiredProperties contains an unknown property")

    closure = _object(record["capabilityClosure"], f"{label}.capabilityClosure")
    _exact_keys(closure, {"requirements", "sha256", "status"}, f"{label}.capabilityClosure")
    requirements = _sorted_unique_strings(
        closure["requirements"], f"{label}.capabilityClosure.requirements", nonempty=True
    )
    if not set(requirements) <= CAPABILITY_REQUIREMENTS:
        _fail(f"{label}.capabilityClosure.requirements contains an unknown capability")
    if closure["status"] == "complete":
        _sha256(closure["sha256"], f"{label}.capabilityClosure.sha256")
    elif closure["status"] in {"incomplete", "not-produced", "unsupported"}:
        if closure["sha256"] is not None:
            _fail(f"{label} incomplete capability closure carries an identity")
    else:
        _fail(f"{label}.capabilityClosure.status is invalid")

    path = _object(record["productionCapabilityPath"], f"{label}.productionCapabilityPath")
    _exact_keys(path, {"evidence", "path", "status"}, f"{label}.productionCapabilityPath")
    expected_path = {
        "complete": "canonical-capability",
        "incomplete": "canonical-capability",
        "legacy-only": "legacy",
        "unsupported": "none",
    }.get(path["status"])
    if expected_path is None or path["path"] != expected_path:
        _fail(f"{label}.productionCapabilityPath state is incoherent")
    if path["status"] != "complete" and path["evidence"] is not None:
        _fail(f"{label} incomplete production path carries evidence")

    proof = _object(record["proofRequirements"], f"{label}.proofRequirements")
    _exact_keys(proof, PROOF_REQUIREMENT_KEYS, f"{label}.proofRequirements")
    proof_properties = _sorted_unique_strings(
        proof["properties"], f"{label}.proofRequirements.properties", nonempty=True
    )
    if not PRODUCTION_PROOF_PROPERTIES <= set(proof_properties):
        _fail(f"{label} omits a mandatory production proof property")
    if not set(proof_properties) <= set(required_properties):
        _fail(f"{label} proof requirements exceed requiredProperties")
    proof_identities = ("checkerSha256", "evidenceSha256", "obligationSetSha256")
    if proof["status"] == "complete":
        for key in proof_identities:
            _sha256(proof[key], f"{label}.proofRequirements.{key}")
    elif proof["status"] in {"missing", "unsupported"}:
        if any(proof[key] is not None for key in proof_identities):
            _fail(f"{label} incomplete proof requirements carry identities")
    else:
        _fail(f"{label}.proofRequirements.status is invalid")

    matrix = _array(record["targetMatrix"], f"{label}.targetMatrix")
    if len(matrix) != 2:
        _fail(f"{label}.targetMatrix must contain neutral and backend records")
    neutral = _object(matrix[0], f"{label}.targetMatrix[0]")
    _exact_keys(neutral, {"kind", "requirements", "status", "target"}, f"{label}.targetMatrix[0]")
    if (
        neutral["kind"] != "neutral"
        or neutral["target"] != "target-neutral"
        or neutral["status"] not in {"not-evaluated", "requirements-derived"}
        or _sorted_unique_strings(
            neutral["requirements"], f"{label}.targetMatrix[0].requirements", nonempty=True
        )
        != requirements
    ):
        _fail(f"{label} target-neutral requirements differ from its capability closure")
    backend = _object(matrix[1], f"{label}.targetMatrix[1]")
    _exact_keys(
        backend,
        {"capabilityDecisionSha256", "kind", "status", "target", "targetIdentitySha256"},
        f"{label}.targetMatrix[1]",
    )
    if backend["kind"] != "backend" or backend["target"] != fixture["target"]:
        _fail(f"{label} backend target differs from its compiler fixture")
    target_identities = ("capabilityDecisionSha256", "targetIdentitySha256")
    if backend["status"] == "capability-complete":
        for key in target_identities:
            _sha256(backend[key], f"{label}.targetMatrix[1].{key}")
    elif backend["status"] in {"legacy-only", "not-evaluated", "unsupported"}:
        if any(backend[key] is not None for key in target_identities):
            _fail(f"{label} incomplete backend decision carries identities")
    else:
        _fail(f"{label}.targetMatrix[1].status is invalid")

    negatives = _object(
        record["negativeFixtureCoverage"], f"{label}.negativeFixtureCoverage"
    )
    _exact_keys(negatives, {"cases", "status"}, f"{label}.negativeFixtureCoverage")
    cases = _array(negatives["cases"], f"{label}.negativeFixtureCoverage.cases")
    case_ids: list[str] = []
    for offset, raw_case in enumerate(cases):
        case = _validate_negative_case(
            raw_case, f"{label}.negativeFixtureCoverage.cases[{offset}]"
        )
        case_ids.append(case["fixtureId"])
    if len(case_ids) != len(set(case_ids)):
        _fail(f"{label} negative fixture identities must be unique")
    if negatives["status"] == "complete":
        if not cases:
            _fail(f"{label} complete negative coverage has no cases")
    elif negatives["status"] in {"legacy-only", "missing"}:
        if cases:
            _fail(f"{label} incomplete negative coverage carries cases")
    else:
        _fail(f"{label}.negativeFixtureCoverage.status is invalid")


def _expected_simulator_commands(
    qualification: dict[str, Any], lesson_ids: set[str], fixture_id: str
) -> list[dict[str, Any]]:
    commands: list[dict[str, Any]] = []
    for raw_suite in _array(qualification.get("suites"), "qualification.suites", nonempty=True):
        suite = _object(raw_suite, "qualification suite")
        if suite.get("gate") != "semantic-simulation" or suite.get("availability") != "available":
            continue
        covered = any(
            _object(item, "qualification coverage").get("lessonId") in lesson_ids
            and fixture_id
            in _array(
                _object(item, "qualification coverage").get("fixtureIds"),
                "qualification coverage.fixtureIds",
            )
            for item in _array(suite.get("coverage"), "qualification suite.coverage")
        )
        if covered:
            command = _object(suite.get("command"), "qualification suite.command")
            if command not in commands:
                commands.append(command)
    return commands


def _expected_hardware_command(fixture: dict[str, Any]) -> dict[str, Any] | None:
    matrix = fixture.get("matrix")
    if matrix is None:
        return None
    matrix = _object(matrix, "compiler fixture.matrix")
    return {
        "executable": matrix.get("runnerPath"),
        "arguments": matrix.get("runnerArguments"),
        "environment": matrix.get("environment"),
        "workingDirectory": ".",
        "timeoutSeconds": 1200,
    }


def _validate_complete_kernel(
    record: dict[str, Any],
    label: str,
    fixture: dict[str, Any],
    baseline: dict[str, Any],
    qualification: dict[str, Any],
) -> None:
    target = _string(fixture.get("target"), f"{label}.target")
    fixture_id = _string(fixture.get("fixtureId"), f"{label}.fixtureId")
    lesson_ids = set(
        _unique_strings(record.get("lessonIds"), f"{label}.lessonIds", nonempty=True)
    )
    closure = _object(record.get("capabilityClosure"), f"{label}.capabilityClosure")
    if closure.get("status") != "complete":
        _fail(f"{label} has no complete capability closure")
    closure_sha = _sha256(closure.get("sha256"), f"{label}.capabilityClosure.sha256")
    closure_requirements = _unique_strings(
        closure.get("requirements"), f"{label}.capabilityClosure.requirements", nonempty=True
    )

    path = _object(record.get("productionCapabilityPath"), f"{label}.productionCapabilityPath")
    if path.get("status") != "complete" or path.get("path") != "canonical-capability":
        _fail(f"{label} does not use the sole canonical capability path")
    evidence = _object(path.get("evidence"), f"{label}.productionCapabilityPath.evidence")
    _exact_keys(evidence, PRODUCTION_EVIDENCE_KEYS, f"{label}.productionCapabilityPath.evidence")
    for key, value in evidence.items():
        if key in {"compilerCommit", "compilerTree"}:
            if not isinstance(value, str) or GIT_ID.fullmatch(value) is None:
                _fail(f"{label} evidence {key} is not a Git identity")
        else:
            _sha256(value, f"{label}.productionCapabilityPath.evidence.{key}")
    if (
        evidence["compilerCommit"] != baseline["compilerCommit"]
        or evidence["compilerTree"] != baseline["compilerTree"]
        or evidence["capabilityClosureSha256"] != closure_sha
    ):
        _fail(f"{label} production evidence is stale against the baseline or closure")

    proof = _object(record.get("proofRequirements"), f"{label}.proofRequirements")
    if proof.get("status") != "complete":
        _fail(f"{label} proof requirements are incomplete")
    required_properties = set(
        _unique_strings(record.get("requiredProperties"), f"{label}.requiredProperties", nonempty=True)
    )
    proof_property_list = _unique_strings(
        proof.get("properties"), f"{label}.proofRequirements.properties", nonempty=True
    )
    properties = set(proof_property_list)
    if not PRODUCTION_PROOF_PROPERTIES <= properties:
        _fail(f"{label} omits a mandatory production proof property")
    if not properties <= required_properties:
        _fail(f"{label} proof requirements exceed requiredProperties")
    for key in ("obligationSetSha256", "checkerSha256", "evidenceSha256"):
        _sha256(proof.get(key), f"{label}.proofRequirements.{key}")
    if (
        evidence["proofCheckerSha256"] != proof["checkerSha256"]
        or evidence["proofEvidenceSha256"] != proof["evidenceSha256"]
        or evidence["proofObligationSetSha256"] != proof["obligationSetSha256"]
    ):
        _fail(f"{label} proof identities are stale against production evidence")

    matrix = _array(record.get("targetMatrix"), f"{label}.targetMatrix")
    if len(matrix) != 2:
        _fail(f"{label}.targetMatrix must contain neutral and backend records")
    neutral = _object(matrix[0], f"{label}.targetMatrix[0]")
    backend = _object(matrix[1], f"{label}.targetMatrix[1]")
    if (
        neutral.get("kind") != "neutral"
        or neutral.get("target") != "target-neutral"
        or neutral.get("status") != "requirements-derived"
    ):
        _fail(f"{label} lacks a derived target-neutral requirement record")
    if _unique_strings(
        neutral.get("requirements"), f"{label}.targetMatrix[0].requirements", nonempty=True
    ) != closure_requirements:
        _fail(f"{label} target-neutral requirements are stale against the closure")
    if (
        backend.get("kind") != "backend"
        or backend.get("target") != target
        or backend.get("status") != "capability-complete"
    ):
        _fail(f"{label} lacks a complete backend capability decision")
    backend_decision = _sha256(
        backend.get("capabilityDecisionSha256"),
        f"{label}.targetMatrix[1].capabilityDecisionSha256",
    )
    backend_target = _sha256(
        backend.get("targetIdentitySha256"), f"{label}.targetMatrix[1].targetIdentitySha256"
    )
    if (
        evidence["targetCapabilityDecisionSha256"] != backend_decision
        or evidence["targetIdentitySha256"] != backend_target
    ):
        _fail(f"{label} target decision is stale against production evidence")

    simulator = _object(record.get("simulatorCommand"), f"{label}.simulatorCommand")
    hardware = _object(record.get("hardwareCommand"), f"{label}.hardwareCommand")
    _validate_capability_command(simulator, f"{label}.simulatorCommand", target, complete=True)
    _validate_capability_command(hardware, f"{label}.hardwareCommand", target, complete=True)
    expected_simulator = _expected_simulator_commands(
        qualification, lesson_ids, fixture_id
    )
    if len(expected_simulator) != 1 or simulator["command"] != expected_simulator[0]:
        _fail(f"{label} simulator command is stale against qualification.suites")
    if hardware["command"] != _expected_hardware_command(fixture):
        _fail(f"{label} hardware command is stale against the compiler fixture matrix")
    if (
        simulator["evidenceSha256"] != evidence["simulatorEvidenceSha256"]
        or simulator["subjectSha256"] != evidence["finalOptimizedKirSha256"]
        or hardware["evidenceSha256"] != evidence["hardwareEvidenceSha256"]
        or hardware["subjectSha256"] != evidence["artifactSha256"]
    ):
        _fail(f"{label} command evidence is stale against production evidence")

    negatives = _object(record.get("negativeFixtureCoverage"), f"{label}.negativeFixtureCoverage")
    if negatives.get("status") != "complete":
        _fail(f"{label} negative fixture coverage is incomplete")
    cases = _array(negatives.get("cases"), f"{label}.negativeFixtureCoverage.cases", nonempty=True)
    categories = {
        _string(_object(case, f"{label}.negativeCase").get("category"), f"{label}.negativeCase.category")
        for case in cases
    }
    if not _required_negative_categories(required_properties) <= categories:
        _fail(f"{label} omits a mandatory negative-fixture category")
    expected_negative_identity = _canonical_json_sha256(
        NEGATIVE_FIXTURE_DIGEST_DOMAIN,
        {"cases": cases, "fixtureId": fixture_id},
    )
    if evidence["negativeFixtureSetSha256"] != expected_negative_identity:
        _fail(f"{label} negative-fixture identity is stale against production evidence")


def _validate_qualification(
    qualification: dict[str, Any],
    entries: dict[str, dict[str, Any]],
    fixtures: dict[str, dict[str, Any]],
    *,
    qualified: bool,
) -> None:
    _exact_keys(
        qualification,
        {"evidenceSchemaPath", "hardwareTargets", "suites"},
        "qualification",
    )
    if qualification["evidenceSchemaPath"] != f"config/{EVIDENCE_SCHEMA_NAME}":
        _fail("qualification names the wrong semantic evidence schema")

    fixture_targets = {fixture["target"] for fixture in fixtures.values()}
    hardware_targets: set[str] = set()
    for offset, raw in enumerate(
        _array(qualification["hardwareTargets"], "qualification.hardwareTargets", nonempty=True)
    ):
        label = f"qualification.hardwareTargets[{offset}]"
        target = _string(_object(raw, label).get("target"), f"{label}.target")
        if target in hardware_targets:
            _fail(f"duplicate qualification hardware target {target!r}")
        hardware_targets.add(target)
        record = _object(raw, label)
        if qualified:
            _exact_keys(record, {"lane", "status", "target"}, label)
            if record["status"] != "required-qualified":
                _fail(f"{label} is not required-qualified")
            _string(record["lane"], f"{label}.lane")
        else:
            _exact_keys(
                record,
                {"deterministicSemanticRunnerAvailability", "reason", "target"},
                label,
            )
            if record["deterministicSemanticRunnerAvailability"] != "unavailable":
                _fail(f"{label} claims hardware availability without qualification")
            _string(record["reason"], f"{label}.reason")
    if hardware_targets != fixture_targets:
        _fail("qualification hardware targets differ from compiler fixture targets")

    fixture_lessons = {
        fixture_id: {
            lesson_id
            for lesson_id, entry in entries.items()
            if fixture_id in entry["compilerFixtureIds"]
        }
        for fixture_id in fixtures
    }
    covered: set[tuple[str, str, str]] = set()
    suite_ids: set[str] = set()
    for offset, raw in enumerate(
        _array(qualification["suites"], "qualification.suites", nonempty=True)
    ):
        label = f"qualification.suites[{offset}]"
        suite = _object(raw, label)
        _exact_keys(
            suite,
            {"availability", "command", "coverage", "gate", "suiteId", "unavailableReason"},
            label,
        )
        suite_id = _string(suite["suiteId"], f"{label}.suiteId")
        if SLUG.fullmatch(suite_id) is None or suite_id in suite_ids:
            _fail(f"{label}.suiteId is invalid or duplicated")
        suite_ids.add(suite_id)
        gate = _string(suite["gate"], f"{label}.gate")
        if gate not in QUALIFICATION_GATES:
            _fail(f"{label}.gate is invalid")
        if suite["availability"] == "available":
            _validate_command(_object(suite["command"], f"{label}.command"), f"{label}.command")
            if suite["unavailableReason"] is not None:
                _fail(f"{label} is available but carries an unavailable reason")
        elif suite["availability"] == "unavailable":
            if suite["command"] is not None:
                _fail(f"{label} is unavailable but carries a command")
            _string(suite["unavailableReason"], f"{label}.unavailableReason")
        else:
            _fail(f"{label}.availability is invalid")
        for coverage_offset, coverage_raw in enumerate(
            _array(suite["coverage"], f"{label}.coverage", nonempty=True)
        ):
            coverage_label = f"{label}.coverage[{coverage_offset}]"
            coverage = _object(coverage_raw, coverage_label)
            _exact_keys(coverage, {"fixtureIds", "lessonId"}, coverage_label)
            lesson_id = _string(coverage["lessonId"], f"{coverage_label}.lessonId")
            if lesson_id not in entries:
                _fail(f"{coverage_label} names unknown lesson {lesson_id!r}")
            for fixture_id in _sorted_unique_strings(
                coverage["fixtureIds"], f"{coverage_label}.fixtureIds", nonempty=True
            ):
                if fixture_id not in fixtures:
                    _fail(f"{coverage_label} names unknown fixture {fixture_id!r}")
                if lesson_id not in fixture_lessons[fixture_id]:
                    _fail(f"{coverage_label} crosses fixture lesson ownership")
                key = (gate, lesson_id, fixture_id)
                if key in covered:
                    _fail(f"duplicate qualification coverage {key!r}")
                covered.add(key)

    for lesson_id, entry in entries.items():
        if "semantic-simulation" not in entry["requiredGates"]:
            continue
        for fixture_id in entry["compilerFixtureIds"]:
            if ("semantic-simulation", lesson_id, fixture_id) not in covered:
                _fail(
                    "qualification omits semantic-simulation coverage for lesson "
                    f"{lesson_id!r} fixture {fixture_id!r}"
                )


def validate_document(
    document: dict[str, Any], *, require_qualified: bool = False
) -> dict[str, int]:
    _exact_keys(document, TOP_LEVEL_KEYS, "manifest")
    if document["schema"] != "fe2o3-tutorial-kernel-manifest-v1":
        _fail("manifest schema identity differs")
    if document["roadmapIssue"] != "https://github.com/harsh-nod/fe2o3/issues/271":
        _fail("manifest roadmap issue differs")

    contract = _object(document["productionContract"], "productionContract")
    expected_contract = {
        "pipelineEntry": "rustc-codegen-fe2o3::production_pipeline",
        "requiredPolicyVersion": 4,
        "requiresFinalOptimizedGraphVerification": True,
        "allowsPipelineSelection": False,
        "allowsFallback": False,
    }
    if contract != expected_contract:
        _fail("production contract permits drift, selection, or fallback")

    capability_contract = _object(document["capabilityContract"], "capabilityContract")
    _exact_keys(
        capability_contract,
        {
            "allowsExactProfileFallback",
            "allowsLegacyFallback",
            "completeClassification",
            "legacyClassification",
            "neutralTarget",
            "roadmapIssue",
            "schema",
            "status",
        },
        "capabilityContract",
    )
    if (
        capability_contract.get("schema") != "fe2o3-tutorial-capability-status-v1"
        or capability_contract.get("roadmapIssue") != "https://github.com/harsh-nod/fe2o3/issues/272"
        or capability_contract.get("completeClassification") != "compiler-produced"
        or capability_contract.get("legacyClassification") != "legacy-compiler-produced"
        or capability_contract.get("neutralTarget") != "target-neutral"
        or capability_contract.get("allowsLegacyFallback") is not False
        or capability_contract.get("allowsExactProfileFallback") is not False
        or capability_contract.get("status") not in {"migration", "qualified"}
    ):
        _fail("capability contract differs from issue #272")

    baseline = _object(document["baseline"], "baseline")
    _exact_keys(baseline, {"compilerCommit", "compilerTree", "status"}, "baseline")
    commit = _string(baseline.get("compilerCommit"), "baseline.compilerCommit")
    tree = _string(baseline.get("compilerTree"), "baseline.compilerTree")
    if GIT_ID.fullmatch(commit) is None or GIT_ID.fullmatch(tree) is None:
        _fail("baseline compiler commit/tree identities are malformed")
    if baseline.get("status") not in {"migration", "qualified"}:
        _fail("baseline status is invalid")

    fixtures = _index(document["compilerFixtures"], "fixtureId", "compilerFixtures")
    entries = _index(document["entries"], "lessonId", "entries")
    kernels = _index(document["capabilityKernels"], "fixtureId", "capabilityKernels")
    if set(fixtures) != set(kernels):
        _fail("compiler fixture and capability kernel identities differ")

    fixture_lessons: dict[str, set[str]] = {identity: set() for identity in fixtures}
    production_entries = 0
    for lesson_id, entry in entries.items():
        _exact_keys(entry, ENTRY_KEYS, f"entry {lesson_id}")
        if SLUG.fullmatch(lesson_id) is None:
            _fail(f"entry lessonId {lesson_id!r} is not a slug")
        classification = _string(entry.get("classification"), f"entry {lesson_id}.classification")
        if classification not in ENTRY_CLASSIFICATIONS:
            _fail(f"entry {lesson_id}.classification is invalid")
        fixture_ids = _sorted_unique_strings(
            entry.get("compilerFixtureIds"), f"entry {lesson_id}.compilerFixtureIds"
        )
        _relative_path(entry["packageManifest"], f"entry {lesson_id}.packageManifest")
        for offset, source in enumerate(
            _sorted_unique_strings(
                entry["sourcePaths"], f"entry {lesson_id}.sourcePaths", nonempty=True
            )
        ):
            _relative_path(source, f"entry {lesson_id}.sourcePaths[{offset}]")
        gates = _sorted_unique_strings(
            entry["requiredGates"], f"entry {lesson_id}.requiredGates", nonempty=True
        )
        if not set(gates) <= ENTRY_GATES:
            _fail(f"entry {lesson_id}.requiredGates contains an invalid gate")
        site_evidence = _string(
            entry["siteEvidenceKind"], f"entry {lesson_id}.siteEvidenceKind"
        )
        if site_evidence not in SITE_EVIDENCE_KINDS:
            _fail(f"entry {lesson_id}.siteEvidenceKind is invalid")
        for fixture_id in fixture_ids:
            if fixture_id not in fixtures:
                _fail(f"entry {lesson_id} names unknown fixture {fixture_id}")
            fixture_lessons[fixture_id].add(lesson_id)
        if classification == "compiler-produced":
            production_entries += 1
            if not fixture_ids or "production-compile" not in gates:
                _fail(f"compiler-produced entry {lesson_id} lacks its production gate")
        elif classification == "legacy-compiler-produced" and not fixture_ids:
            _fail(f"legacy compiler entry {lesson_id} has no fixture")

    for fixture_id, fixture in fixtures.items():
        _exact_keys(fixture, COMPILER_FIXTURE_KEYS, f"fixture {fixture_id}")
        if SLUG.fullmatch(fixture_id) is None:
            _fail(f"fixture identity {fixture_id!r} is not a slug")
        target = _string(fixture.get("target"), f"fixture {fixture_id}.target")
        if TARGET.fullmatch(target) is None:
            _fail(f"fixture {fixture_id}.target is invalid")
        _string(fixture["testId"], f"fixture {fixture_id}.testId")
        _relative_path(fixture["testPath"], f"fixture {fixture_id}.testPath")
        matrix = fixture["matrix"]
        if matrix is not None:
            matrix = _object(matrix, f"fixture {fixture_id}.matrix")
            _exact_keys(
                matrix,
                {"artifactName", "caseId", "environment", "runnerArguments", "runnerPath"},
                f"fixture {fixture_id}.matrix",
            )
            case_id = _string(matrix["caseId"], f"fixture {fixture_id}.matrix.caseId")
            if SLUG.fullmatch(case_id) is None:
                _fail(f"fixture {fixture_id}.matrix.caseId is not a slug")
            artifact = _string(
                matrix["artifactName"], f"fixture {fixture_id}.matrix.artifactName"
            )
            if "/" in artifact or not artifact.endswith(".hsaco"):
                _fail(f"fixture {fixture_id}.matrix.artifactName is invalid")
            _relative_path(matrix["runnerPath"], f"fixture {fixture_id}.matrix.runnerPath")
            _unique_strings(
                matrix["runnerArguments"], f"fixture {fixture_id}.matrix.runnerArguments"
            )
            _unique_strings(
                matrix["environment"], f"fixture {fixture_id}.matrix.environment"
            )

        compiler_input = _object(fixture.get("compilerInput"), f"fixture {fixture_id}.compilerInput")
        _exact_keys(
            compiler_input, COMPILER_INPUT_KEYS, f"fixture {fixture_id}.compilerInput"
        )
        symbols = _sorted_unique_strings(
            compiler_input.get("kernelSymbols"),
            f"fixture {fixture_id}.kernelSymbols",
            nonempty=True,
        )
        if any(RUST_IDENTIFIER.fullmatch(symbol) is None for symbol in symbols):
            _fail(f"fixture {fixture_id} contains an invalid kernel symbol")
        for key in ("packageManifest", "cargoLockPath"):
            _relative_path(compiler_input.get(key), f"fixture {fixture_id}.{key}")
        for offset, source in enumerate(
            _sorted_unique_strings(
                compiler_input["sourcePaths"],
                f"fixture {fixture_id}.sourcePaths",
                nonempty=True,
            )
        ):
            _relative_path(source, f"fixture {fixture_id}.sourcePaths[{offset}]")
        _sorted_unique_strings(compiler_input["features"], f"fixture {fixture_id}.features")
        if not isinstance(compiler_input["defaultFeatures"], bool):
            _fail(f"fixture {fixture_id}.defaultFeatures must be boolean")
        cargo_target = _object(
            compiler_input["cargoTarget"], f"fixture {fixture_id}.cargoTarget"
        )
        _exact_keys(cargo_target, {"kind", "name", "sourcePath"}, f"fixture {fixture_id}.cargoTarget")
        if cargo_target["kind"] != "lib" or RUST_IDENTIFIER.fullmatch(
            _string(cargo_target["name"], f"fixture {fixture_id}.cargoTarget.name")
        ) is None:
            _fail(f"fixture {fixture_id}.cargoTarget is invalid")
        _relative_path(
            cargo_target["sourcePath"], f"fixture {fixture_id}.cargoTarget.sourcePath"
        )
        for key in ("packageManifestSha256", "cargoLockSha256", "sourceClosureSha256", "contractSha256"):
            _sha256(compiler_input.get(key), f"fixture {fixture_id}.{key}")
        if compiler_input["contractSha256"] != _fixture_input_contract_sha256(fixture):
            _fail(f"fixture {fixture_id}.compilerInput.contractSha256 is stale")

        kernel = kernels[fixture_id]
        label = f"capability kernel {fixture_id}"
        _validate_kernel_shape(kernel, label, fixture)
        symbol = kernel["kernelSymbol"]
        if symbol not in symbols:
            _fail(f"capability kernel {fixture_id} symbol is absent from its compiler input")
        lesson_ids = set(kernel["lessonIds"])
        if lesson_ids != fixture_lessons[fixture_id]:
            _fail(f"capability kernel {fixture_id} lesson ownership differs from entries")

        classifications = {entries[lesson]["classification"] for lesson in lesson_ids}
        complete = "compiler-produced" in classifications
        if complete:
            if classifications != {"compiler-produced"}:
                _fail(f"fixture {fixture_id} mixes production and non-production lessons")
            _validate_complete_kernel(
                kernel,
                label,
                fixture,
                baseline,
                document["qualification"],
            )
        elif kernel.get("productionCapabilityPath", {}).get("status") == "complete":
            _fail(f"non-production fixture {fixture_id} carries production authority")
        else:
            simulator = _object(
                kernel.get("simulatorCommand"),
                f"capability kernel {fixture_id}.simulatorCommand",
            )
            hardware = _object(
                kernel.get("hardwareCommand"),
                f"capability kernel {fixture_id}.hardwareCommand",
            )
            _validate_capability_command(
                simulator,
                f"{label}.simulatorCommand",
                target,
                complete=False,
            )
            _validate_capability_command(
                hardware,
                f"{label}.hardwareCommand",
                target,
                complete=False,
            )
            if hardware["command"] != _expected_hardware_command(fixture):
                _fail(
                    f"capability kernel {fixture_id} hardware command is stale "
                    "against the compiler fixture matrix"
                )

    qualification = _object(document["qualification"], "qualification")
    _validate_qualification(
        qualification,
        entries,
        fixtures,
        qualified=baseline["status"] == "qualified",
    )

    if baseline["status"] == "qualified":
        if capability_contract["status"] != "qualified" or production_entries != len(entries):
            _fail("qualified baseline does not classify every entry as compiler-produced")
    elif capability_contract["status"] != "migration":
        _fail("migration baseline must retain migration capability status")
    if require_qualified and (
        baseline["status"] != "qualified"
        or capability_contract["status"] != "qualified"
        or production_entries != len(entries)
    ):
        _fail("release requires a fully qualified capability manifest")

    return {
        "entries": len(entries),
        "fixtures": len(fixtures),
        "production_entries": production_entries,
    }


def _git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        _fail(f"git {' '.join(arguments)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _repository_path(root: Path, relative: Any, label: str) -> Path:
    portable = _relative_path(relative, label)
    current = root
    parts = PurePosixPath(portable).parts
    if not parts:
        _fail(f"{label} is empty")
    for offset, part in enumerate(parts):
        current /= part
        try:
            metadata = current.lstat()
        except OSError as error:
            _fail(f"cannot inspect {label}: {error}")
        if current.is_symlink():
            _fail(f"{label} traverses a symlink")
        if offset + 1 != len(parts) and not stat.S_ISDIR(metadata.st_mode):
            _fail(f"{label} traverses a non-directory")
    return current


def _repository_file(
    root: Path,
    relative: Any,
    label: str,
    *,
    maximum_bytes: int | None = None,
    executable: bool = False,
) -> tuple[Path, bytes]:
    path = _repository_path(root, relative, label)
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        _fail(f"{label} must be a regular non-symlink file")
    if executable and metadata.st_mode & 0o111 == 0:
        _fail(f"{label} is not executable")
    if maximum_bytes is not None and metadata.st_size > maximum_bytes:
        _fail(f"{label} exceeds its byte bound")
    try:
        payload = path.read_bytes()
    except OSError as error:
        _fail(f"cannot read {label}: {error}")
    if len(payload) != metadata.st_size:
        _fail(f"{label} changed while it was read")
    return path, payload


def _package_rust_source_closure(root: Path, package_root: Path, label: str) -> str:
    digest = hashlib.sha256(SOURCE_CLOSURE_DIGEST_DOMAIN)
    file_count = 0
    total_bytes = 0
    for directory, directory_names, file_names in os.walk(package_root, followlinks=False):
        directory_path = Path(directory)
        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            candidate = directory_path / name
            if candidate.is_symlink():
                _fail(f"{label} package contains a symlink")
        directory_names[:] = [
            name for name in directory_names if name not in IGNORED_PACKAGE_DIRECTORIES
        ]
        for name in file_names:
            if not name.endswith(".rs"):
                continue
            candidate = directory_path / name
            if candidate.is_symlink() or not candidate.is_file():
                _fail(f"{label} source is not a regular non-symlink file")
            try:
                payload = candidate.read_bytes()
                payload.decode("utf-8")
            except (OSError, UnicodeError) as error:
                _fail(f"cannot read {label} source: {error}")
            file_count += 1
            total_bytes += len(payload)
            if file_count > MAX_PACKAGE_SOURCE_FILES:
                _fail(f"{label} package source closure exceeds its file bound")
            if total_bytes > MAX_PACKAGE_SOURCE_BYTES:
                _fail(f"{label} package source closure exceeds its byte bound")
            relative = candidate.relative_to(root).as_posix().encode("utf-8")
            digest.update(len(relative).to_bytes(4, "little"))
            digest.update(relative)
            digest.update(len(payload).to_bytes(8, "little"))
            digest.update(payload)
    if file_count == 0:
        _fail(f"{label} package has no Rust source files")
    return digest.hexdigest()


def _effective_cargo_lock(root: Path, package_root: Path, label: str) -> Path:
    directory = package_root
    while directory == root or root in directory.parents:
        candidate = directory / "Cargo.lock"
        if candidate.exists() or candidate.is_symlink():
            relative = candidate.relative_to(root).as_posix()
            return _repository_file(
                root,
                relative,
                f"{label}.cargoLockPath",
                maximum_bytes=MAX_CARGO_LOCK_BYTES,
            )[0]
        if directory == root:
            break
        directory = directory.parent
    _fail(f"{label} package has no effective Cargo.lock")


def _validate_qualified_repository_inputs(root: Path, document: dict[str, Any]) -> None:
    package_cache: dict[str, tuple[Path, dict[str, Any], str, Path]] = {}
    for raw_fixture in _array(document["compilerFixtures"], "compilerFixtures", nonempty=True):
        fixture = _object(raw_fixture, "compiler fixture")
        fixture_id = _string(fixture.get("fixtureId"), "compiler fixture.fixtureId")
        label = f"fixture {fixture_id}.compilerInput"
        compiler_input = _object(fixture.get("compilerInput"), label)
        package_manifest = _relative_path(
            compiler_input.get("packageManifest"), f"{label}.packageManifest"
        )
        cached = package_cache.get(package_manifest)
        if cached is None:
            manifest_path, manifest_bytes = _repository_file(
                root,
                package_manifest,
                f"{label}.packageManifest",
                maximum_bytes=MAX_CARGO_MANIFEST_BYTES,
            )
            if manifest_path.name != "Cargo.toml":
                _fail(f"{label}.packageManifest does not name Cargo.toml")
            try:
                cargo = tomllib.loads(manifest_bytes.decode("utf-8"))
            except (UnicodeError, tomllib.TOMLDecodeError) as error:
                _fail(f"cannot parse {label}.packageManifest: {error}")
            package_root = manifest_path.parent
            closure = _package_rust_source_closure(root, package_root, label)
            lock_path = _effective_cargo_lock(root, package_root, label)
            cached = (package_root, cargo, closure, lock_path)
            package_cache[package_manifest] = cached
        else:
            manifest_path, manifest_bytes = _repository_file(
                root,
                package_manifest,
                f"{label}.packageManifest",
                maximum_bytes=MAX_CARGO_MANIFEST_BYTES,
            )
        package_root, cargo, closure, lock_path = cached
        if hashlib.sha256(manifest_bytes).hexdigest() != compiler_input["packageManifestSha256"]:
            _fail(f"{label}.packageManifestSha256 is stale")
        expected_lock = lock_path.relative_to(root).as_posix()
        if compiler_input["cargoLockPath"] != expected_lock:
            _fail(f"{label}.cargoLockPath is not the effective Cargo.lock")
        _, lock_bytes = _repository_file(
            root,
            expected_lock,
            f"{label}.cargoLockPath",
            maximum_bytes=MAX_CARGO_LOCK_BYTES,
        )
        if hashlib.sha256(lock_bytes).hexdigest() != compiler_input["cargoLockSha256"]:
            _fail(f"{label}.cargoLockSha256 is stale")
        if closure != compiler_input["sourceClosureSha256"]:
            _fail(f"{label}.sourceClosureSha256 is stale")

        for offset, source in enumerate(
            _unique_strings(compiler_input.get("sourcePaths"), f"{label}.sourcePaths", nonempty=True)
        ):
            path, _ = _repository_file(root, source, f"{label}.sourcePaths[{offset}]")
            if path != package_root and package_root not in path.parents:
                _fail(f"{label}.sourcePaths[{offset}] is outside its package")

        target = _object(compiler_input.get("cargoTarget"), f"{label}.cargoTarget")
        package = _object(cargo.get("package"), f"{label} Cargo [package]")
        package_name = _string(package.get("name"), f"{label} Cargo package.name")
        library = cargo.get("lib", {})
        if not isinstance(library, dict):
            _fail(f"{label} Cargo [lib] must be a table")
        expected_name = library.get("name", package_name.replace("-", "_"))
        expected_source = library.get("path", "src/lib.rs")
        if target != {"kind": "lib", "name": expected_name, "sourcePath": expected_source}:
            _fail(f"{label}.cargoTarget differs from Cargo [lib]")
        target_path = (package_root / expected_source).relative_to(root).as_posix()
        _repository_file(root, target_path, f"{label}.cargoTarget.sourcePath")

        _repository_file(root, fixture.get("testPath"), f"fixture {fixture_id}.testPath")

    qualification = _object(document["qualification"], "qualification")
    for raw_suite in _array(qualification["suites"], "qualification.suites", nonempty=True):
        suite = _object(raw_suite, "qualification suite")
        if suite.get("availability") == "available":
            command = _object(suite.get("command"), "qualification suite.command")
            _repository_file(
                root,
                command.get("executable"),
                "qualification suite.command.executable",
                executable=True,
            )

    for raw_kernel in _array(document["capabilityKernels"], "capabilityKernels", nonempty=True):
        kernel = _object(raw_kernel, "capability kernel")
        if kernel.get("productionCapabilityPath", {}).get("status") != "complete":
            continue
        fixture_id = _string(kernel.get("fixtureId"), "capability kernel.fixtureId")
        for field in ("simulatorCommand", "hardwareCommand"):
            command = _object(kernel.get(field), f"capability kernel {fixture_id}.{field}")
            executable = _object(command.get("command"), f"capability kernel {fixture_id}.{field}.command").get("executable")
            _repository_file(
                root,
                executable,
                f"capability kernel {fixture_id}.{field}.command.executable",
                executable=True,
            )
        negatives = _object(
            kernel.get("negativeFixtureCoverage"),
            f"capability kernel {fixture_id}.negativeFixtureCoverage",
        )
        for offset, raw_case in enumerate(_array(negatives.get("cases"), "negative cases", nonempty=True)):
            case = _object(raw_case, "negative case")
            _repository_file(
                root,
                case.get("testPath"),
                f"capability kernel {fixture_id}.negativeFixtureCoverage.cases[{offset}].testPath",
            )


def _validate_hardware_runner_inputs(root: Path, document: dict[str, Any]) -> None:
    for raw_fixture in _array(
        document["compilerFixtures"], "compilerFixtures", nonempty=True
    ):
        fixture = _object(raw_fixture, "compiler fixture")
        fixture_id = _string(fixture.get("fixtureId"), "compiler fixture.fixtureId")
        matrix = fixture.get("matrix")
        if matrix is None:
            continue
        matrix = _object(matrix, f"fixture {fixture_id}.matrix")
        _, runner_payload = _repository_file(
            root,
            matrix.get("runnerPath"),
            f"fixture {fixture_id}.matrix.runnerPath",
            executable=True,
        )
        if HARDWARE_PROTOCOL_ENVIRONMENT not in _unique_strings(
            matrix.get("environment"), f"fixture {fixture_id}.matrix.environment"
        ):
            _fail(f"fixture {fixture_id} bypasses authenticated hardware transport")
        if not runner_payload.startswith(HARDWARE_RUNNER_PROLOGUE):
            _fail(
                f"fixture {fixture_id}.matrix.runnerPath does not invoke the shared "
                "authenticated hardware adapter"
            )


def validate_repository(
    root: Path = REPO_ROOT,
    site_repository: Path | None = None,
    *,
    require_qualified: bool = False,
) -> dict[str, int]:
    manifest_path = root / "config" / MANIFEST_NAME
    schema_path = root / "config" / SCHEMA_NAME
    manifest_bytes = _read_content_addressed(manifest_path, manifest_path.with_suffix(".sha256"))
    schema_bytes = _read_content_addressed(schema_path, schema_path.with_suffix(".sha256"))
    expectation_schema_path = root / "config" / EXPECTATION_SCHEMA_NAME
    expectation_schema_bytes = _read_content_addressed(
        expectation_schema_path, expectation_schema_path.with_suffix(".sha256")
    )
    evidence_schema_path = root / "config" / EVIDENCE_SCHEMA_NAME
    evidence_schema_bytes = _read_content_addressed(
        evidence_schema_path, evidence_schema_path.with_suffix(".sha256")
    )
    capability_qualification_schema_path = (
        root / "config" / CAPABILITY_QUALIFICATION_SCHEMA_NAME
    )
    capability_qualification_schema_bytes = _read_content_addressed(
        capability_qualification_schema_path,
        capability_qualification_schema_path.with_suffix(".sha256"),
    )
    _, schema = _load_json_unique(schema_path)
    if (
        schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema"
        or schema.get("$id") != "https://harsh-nod.github.io/fe2o3-kernels/schema/tutorial-kernel-manifest-v1.json"
        or schema.get("type") != "object"
        or schema.get("additionalProperties") is not False
    ):
        _fail("shared schema root is not the closed v1 contract")
    for path, schema_id in (
        (
            expectation_schema_path,
            "https://harsh-nod.github.io/fe2o3-kernels/schema/tutorial-semantic-expectation-v1.json",
        ),
        (
            evidence_schema_path,
            "https://harsh-nod.github.io/fe2o3-kernels/schema/tutorial-semantic-qualification-evidence-v1.json",
        ),
        (
            capability_qualification_schema_path,
            "https://harsh-nod.github.io/fe2o3-kernels/schema/tutorial-capability-qualification-batch-v1.json",
        ),
    ):
        _, shared_schema = _load_json_unique(path)
        if (
            shared_schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema"
            or shared_schema.get("$id") != schema_id
            or shared_schema.get("type") != "object"
            or shared_schema.get("additionalProperties") is not False
        ):
            _fail(f"{path.name} root is not the closed v1 contract")
    _, document = _load_json_unique(manifest_path)
    stats = validate_document(document, require_qualified=require_qualified)
    _validate_hardware_runner_inputs(root, document)
    if require_qualified:
        _validate_qualified_repository_inputs(root, document)

    baseline = document["baseline"]
    observed_tree = _git(root, "show", "-s", "--format=%T", baseline["compilerCommit"])
    if observed_tree != baseline["compilerTree"]:
        _fail("baseline compiler commit does not own the recorded tree")
    ancestry = subprocess.run(
        ["git", "-C", str(root), "merge-base", "--is-ancestor", baseline["compilerCommit"], "HEAD"],
        check=False,
    )
    if ancestry.returncode != 0:
        _fail("current compiler HEAD does not contain the recorded baseline")

    if site_repository is not None:
        site_metadata = site_repository.lstat()
        if not stat.S_ISDIR(site_metadata.st_mode) or site_repository.is_symlink():
            _fail("tutorial repository must be a real directory")
        site_root = site_repository.resolve(strict=True)
        for name, expected in (
            (MANIFEST_NAME, manifest_bytes),
            (SCHEMA_NAME, schema_bytes),
            (EXPECTATION_SCHEMA_NAME, expectation_schema_bytes),
            (EVIDENCE_SCHEMA_NAME, evidence_schema_bytes),
            (
                CAPABILITY_QUALIFICATION_SCHEMA_NAME,
                capability_qualification_schema_bytes,
            ),
        ):
            candidate = site_root / "config" / name
            if candidate.read_bytes() != expected:
                _fail(f"tutorial repository config/{name} differs byte-for-byte")
            digest = candidate.with_suffix(".sha256")
            compiler_digest = root / "config" / digest.name
            if digest.read_bytes() != compiler_digest.read_bytes():
                _fail(f"tutorial repository config/{digest.name} differs byte-for-byte")

    return stats


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=REPO_ROOT)
    parser.add_argument("--site-repository", type=Path)
    parser.add_argument(
        "--require-qualified",
        action="store_true",
        help="reject migration manifests; intended for release admission",
    )
    options = parser.parse_args(arguments)
    try:
        stats = validate_repository(
            options.repository.resolve(),
            options.site_repository,
            require_qualified=options.require_qualified,
        )
    except (ManifestError, OSError) as error:
        print(f"tutorial kernel manifest: {error}", file=sys.stderr)
        return 1
    print(
        "validated shared tutorial kernel manifest: "
        f"{stats['entries']} lessons, {stats['fixtures']} fixtures, "
        f"{stats['production_entries']} compiler-produced"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
