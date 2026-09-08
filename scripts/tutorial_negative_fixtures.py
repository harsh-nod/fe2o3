#!/usr/bin/env python3
"""Validate and execute fixture-bound tutorial negative mutations."""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
from typing import Any

import tutorial_kernel_manifest as manifest_contract


DECLARATION_SCHEMA = "fe2o3-tutorial-negative-fixture-declarations-v2"
EVIDENCE_SCHEMA = "fe2o3-tutorial-negative-fixture-evidence-v2"
ROADMAP_ISSUE = "https://github.com/harsh-nod/fe2o3/issues/272"
DECLARATION_PATH = Path("config/tutorial-negative-fixtures-v1.json")
DECLARATION_DIGEST_PATH = Path("config/tutorial-negative-fixtures-v1.sha256")
SEMANTIC_FIXTURE_ROOT = Path("scripts/tutorial-semantic-fixtures-v1")
CONTRACT_DOMAIN = b"fe2o3-tutorial-negative-fixture-contract-v2\0"
COMMAND_DOMAIN = b"fe2o3-tutorial-negative-fixture-command-v2\0"
FIXTURE_BINDING_DOMAIN = b"fe2o3-tutorial-negative-fixture-binding-v2\0"
BASE_PAYLOAD_DOMAIN = b"fe2o3-tutorial-negative-base-payload-v2\0"
MUTATED_PAYLOAD_DOMAIN = b"fe2o3-tutorial-negative-mutated-payload-v2\0"
CASE_EXECUTION_DOMAIN = b"fe2o3-tutorial-negative-case-execution-v2\0"
CASE_EVIDENCE_DOMAIN = b"fe2o3-tutorial-negative-case-evidence-v2\0"
MAX_OUTPUT_BYTES = 8 * 1024 * 1024
MAX_SHARED_PREREQUISITES = 32
MAX_CASES = 2048
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
ENVIRONMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.+\Z")
FAILURE_STAGES = {
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
SUBJECT_KINDS = {"compiler-input", "evidence", "optimized-kir-v13", "receipt"}
UNAVAILABLE_RESULT = {
    "diagnosticCode": "FE2O3-NEG-PRODUCTION-LIFECYCLE-UNAVAILABLE",
    "reason": (
        "the exact production mutation lifecycle did not return a compiler-produced "
        "rejection; mutation construction is observation-only"
    ),
    "status": "unavailable",
}


class NegativeFixtureError(ValueError):
    """A negative declaration, observation, or production join is invalid."""


@dataclass(frozen=True)
class ValidatedDeclarations:
    raw_sha256: str
    shared_prerequisites: dict[str, dict[str, Any]]
    fixtures: dict[str, dict[str, Any]]


def _fail(message: str) -> None:
    raise NegativeFixtureError(message)


def _canonical(value: Any) -> bytes:
    try:
        return json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeError) as error:
        _fail(f"cannot encode canonical JSON: {error}")


def _sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _domain_sha256(domain: bytes, value: Any) -> str:
    return _sha256(domain + _canonical(value))


def _load_unique(path: Path, label: str) -> tuple[bytes, dict[str, Any]]:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
            _fail(f"{label} must be a regular non-symlink file")
        payload = path.read_bytes()
        value = json.loads(payload, object_pairs_hook=reject_duplicates)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot read {label}: {error}")
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return payload, value


def _exact(value: dict[str, Any], keys: set[str], label: str) -> None:
    if set(value) != keys:
        _fail(
            f"{label} fields differ: missing={sorted(keys - set(value))!r} "
            f"extra={sorted(set(value) - keys)!r}"
        )


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _array(value: Any, label: str, *, nonempty: bool = False) -> list[Any]:
    if not isinstance(value, list) or (nonempty and not value):
        _fail(f"{label} must be a {'non-empty ' if nonempty else ''}array")
    return value


def _string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value:
        _fail(f"{label} must be a non-empty string")
    return value


def _sha(value: Any, label: str) -> str:
    value = _string(value, label)
    if SHA256.fullmatch(value) is None:
        _fail(f"{label} must be a lowercase SHA-256 identity")
    return value


def _repository_file(
    repository: Path, relative: Any, label: str, *, executable: bool = False
) -> tuple[Path, bytes]:
    relative = _string(relative, label)
    candidate = repository / relative
    try:
        resolved = candidate.resolve(strict=True)
        metadata = candidate.lstat()
    except OSError as error:
        _fail(f"cannot resolve {label}: {error}")
    if (
        candidate.is_symlink()
        or not stat.S_ISREG(metadata.st_mode)
        or not resolved.is_relative_to(repository)
    ):
        _fail(f"{label} must be a regular repository file")
    if executable and metadata.st_mode & 0o111 == 0:
        _fail(f"{label} must be executable")
    return resolved, resolved.read_bytes()


def fixture_contract_sha256(document: dict[str, Any]) -> str:
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    subject = [
        {
            "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
            "fixtureId": fixture["fixtureId"],
            "kernelSymbol": kernels[fixture["fixtureId"]]["kernelSymbol"],
            "requiredProperties": kernels[fixture["fixtureId"]]["requiredProperties"],
            "target": fixture["target"],
        }
        for fixture in sorted(document["compilerFixtures"], key=lambda item: item["fixtureId"])
    ]
    return _domain_sha256(CONTRACT_DOMAIN, {"fixtures": subject})


def fixture_binding(fixture: dict[str, Any], kernel: dict[str, Any]) -> dict[str, Any]:
    return {
        "compilerInputContractSha256": fixture["compilerInput"]["contractSha256"],
        "fixtureId": fixture["fixtureId"],
        "kernelSymbol": kernel["kernelSymbol"],
        "requiredProperties": sorted(kernel["requiredProperties"]),
        "sourceClosureSha256": fixture["compilerInput"]["sourceClosureSha256"],
        "target": fixture["target"],
    }


def fixture_binding_sha256(fixture: dict[str, Any], kernel: dict[str, Any]) -> str:
    return _domain_sha256(FIXTURE_BINDING_DOMAIN, fixture_binding(fixture, kernel))


def _semantic_fixture(repository: Path, fixture_id: str) -> tuple[bytes, dict[str, Any]]:
    return _load_unique(
        repository / SEMANTIC_FIXTURE_ROOT / f"{fixture_id}.json",
        f"semantic fixture {fixture_id}",
    )


def base_payload(
    repository: Path,
    fixture: dict[str, Any],
    kernel: dict[str, Any],
    required_property: str,
) -> dict[str, Any]:
    raw, semantic = _semantic_fixture(repository, fixture["fixtureId"])
    if (
        semantic.get("fixtureId") != fixture["fixtureId"]
        or semantic.get("kernelSymbol") != kernel["kernelSymbol"]
        or semantic.get("target") != fixture["target"]
    ):
        _fail(f"semantic fixture {fixture['fixtureId']} is cross-fixture or cross-target")
    request = _object(semantic.get("request"), "semantic fixture request")
    export = _object(semantic.get("productionExport"), "semantic fixture production export")
    coordinates = export.get("coordinates") if export.get("status") == "available" else None
    return {
        "compilerInput": deepcopy(fixture["compilerInput"]),
        "evidence": {
            "expectedOutputSha256": _sha256(_canonical(semantic.get("expectedArguments"))),
            "numericalPolicySha256": _sha256(_canonical(semantic.get("numericalPolicy"))),
            "semanticFixtureRawSha256": _sha256(raw),
        },
        "fixtureId": fixture["fixtureId"],
        "kernelSymbol": kernel["kernelSymbol"],
        "kirV13": {
            "identity": coordinates.get("productionKirIdentitySha256") if coordinates else None,
            "status": "available" if coordinates else "unavailable",
        },
        "launch": {
            "grid": deepcopy(request.get("grid")),
            "kernel": request.get("kernel"),
            "workgroup": deepcopy(request.get("workgroup")),
        },
        "receipt": {
            "hostInvocation": "canonical",
            "identity": coordinates.get("bundleSubjectIdentitySha256") if coordinates else None,
            "status": "available" if coordinates else "unavailable",
        },
        "requiredProperty": required_property,
        "semanticContract": {
            "alias": "canonical",
            "bounds": "canonical",
            "capability": "canonical",
            "initialization": "canonical",
            "operation": "canonical",
            "order": "canonical",
            "recurrence": "canonical",
        },
        "target": fixture["target"],
    }


def apply_mutation(base: dict[str, Any], mutation: dict[str, Any]) -> dict[str, Any]:
    path = _array(mutation.get("path"), "mutation.path", nonempty=True)
    mutated = deepcopy(base)
    cursor: Any = mutated
    for offset, component in enumerate(path[:-1]):
        if isinstance(component, str) and isinstance(cursor, dict) and component in cursor:
            cursor = cursor[component]
        elif isinstance(component, int) and isinstance(cursor, list) and 0 <= component < len(cursor):
            cursor = cursor[component]
        else:
            _fail(f"mutation path is invalid at component {offset}")
    leaf = path[-1]
    if isinstance(leaf, str) and isinstance(cursor, dict) and leaf in cursor:
        cursor[leaf] = deepcopy(mutation["replacement"])
    elif isinstance(leaf, int) and isinstance(cursor, list) and 0 <= leaf < len(cursor):
        cursor[leaf] = deepcopy(mutation["replacement"])
    else:
        _fail("mutation path has no exact existing leaf")
    if mutated == base:
        _fail("negative mutation does not change its exact fixture payload")
    return mutated


def derive_payloads(
    repository: Path,
    document: dict[str, Any],
    case: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, Any]]:
    fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    fixture_id = case["fixtureId"]
    base = base_payload(
        repository,
        fixtures[fixture_id],
        kernels[fixture_id],
        case["requiredProperty"],
    )
    return base, apply_mutation(base, case["mutation"])


def _validate_command(repository: Path, raw: Any, label: str) -> dict[str, Any]:
    command = _object(raw, label)
    try:
        manifest_contract._validate_command(command, label)
    except manifest_contract.ManifestError as error:
        _fail(str(error))
    _repository_file(repository, command["executable"], f"{label}.executable", executable=True)
    if any(ENVIRONMENT.fullmatch(item) is None for item in command["environment"]):
        _fail(f"{label}.environment contains an invalid assignment")
    return command


def validate_declarations(
    repository: Path,
    document: dict[str, Any],
    declaration: dict[str, Any],
    raw_sha256: str,
) -> ValidatedDeclarations:
    _exact(
        declaration,
        {
            "fixtureContractSha256",
            "fixtures",
            "roadmapIssue",
            "schema",
            "sharedPrerequisites",
        },
        "negative declaration",
    )
    if declaration["schema"] != DECLARATION_SCHEMA or declaration["roadmapIssue"] != ROADMAP_ISSUE:
        _fail("negative declaration schema or roadmap identity differs")
    if declaration["fixtureContractSha256"] != fixture_contract_sha256(document):
        _fail("negative declaration is stale against the fixture contract")

    prerequisites: dict[str, dict[str, Any]] = {}
    previous = ""
    for offset, raw in enumerate(
        _array(declaration["sharedPrerequisites"], "negative declaration.sharedPrerequisites")
    ):
        label = f"negative declaration.sharedPrerequisites[{offset}]"
        prerequisite = _object(raw, label)
        _exact(
            prerequisite,
            {"command", "expectedObservation", "harnessPath", "harnessSha256", "prerequisiteId"},
            label,
        )
        identity = _string(prerequisite["prerequisiteId"], f"{label}.prerequisiteId")
        if SLUG.fullmatch(identity) is None or identity <= previous:
            _fail("shared prerequisite identities must be sorted unique slugs")
        previous = identity
        _validate_command(repository, prerequisite["command"], f"{label}.command")
        _, harness = _repository_file(repository, prerequisite["harnessPath"], f"{label}.harnessPath")
        if prerequisite["harnessSha256"] != _sha256(harness):
            _fail(f"shared prerequisite {identity} harness is stale")
        observation = _object(prerequisite["expectedObservation"], f"{label}.expectedObservation")
        _exact(
            observation,
            {"exitStatus", "stderrBytes", "stderrSha256", "stdoutBytes", "stdoutSha256"},
            f"{label}.expectedObservation",
        )
        if observation["exitStatus"] != 0:
            _fail(f"shared prerequisite {identity} must require a passing hostile test")
        for field in ("stderrSha256", "stdoutSha256"):
            _sha(observation[field], f"{label}.expectedObservation.{field}")
        prerequisites[identity] = prerequisite
    if len(prerequisites) > MAX_SHARED_PREREQUISITES:
        _fail("shared prerequisite roster exceeds its bound")

    manifest_fixtures = {item["fixtureId"]: item for item in document["compilerFixtures"]}
    kernels = {item["fixtureId"]: item for item in document["capabilityKernels"]}
    fixtures: dict[str, dict[str, Any]] = {}
    case_ids: set[str] = set()
    mutation_ids: set[str] = set()
    total_cases = 0
    previous_fixture = ""
    for offset, raw in enumerate(_array(declaration["fixtures"], "negative declaration.fixtures", nonempty=True)):
        label = f"negative declaration.fixtures[{offset}]"
        fixture = _object(raw, label)
        _exact(
            fixture,
            {
                "bindingSha256",
                "cases",
                "compilerInputContractSha256",
                "fixtureId",
                "kernelSymbol",
                "sourceClosureSha256",
                "target",
            },
            label,
        )
        fixture_id = _string(fixture["fixtureId"], f"{label}.fixtureId")
        if fixture_id <= previous_fixture or fixture_id not in manifest_fixtures:
            _fail("negative fixture identities must exactly match sorted manifest fixtures")
        previous_fixture = fixture_id
        manifest_fixture, kernel = manifest_fixtures[fixture_id], kernels[fixture_id]
        binding = fixture_binding(manifest_fixture, kernel)
        if any(
            fixture[key] != binding[key]
            for key in (
                "compilerInputContractSha256",
                "fixtureId",
                "kernelSymbol",
                "sourceClosureSha256",
                "target",
            )
        ) or fixture["bindingSha256"] != fixture_binding_sha256(manifest_fixture, kernel):
            _fail(f"negative fixture {fixture_id} is stale, cross-fixture, or cross-target")
        required_categories = manifest_contract._required_negative_categories(
            set(kernel["requiredProperties"])
        )
        observed_categories: set[str] = set()
        previous_case = ""
        for case_offset, raw_case in enumerate(_array(fixture["cases"], f"{label}.cases", nonempty=True)):
            case_label = f"{label}.cases[{case_offset}]"
            case = _object(raw_case, case_label)
            _exact(
                case,
                {
                    "basePayloadSha256",
                    "caseId",
                    "category",
                    "diagnosticCode",
                    "expectedResult",
                    "failureStage",
                    "fixtureBindingSha256",
                    "fixtureId",
                    "mutatedPayloadSha256",
                    "mutation",
                    "productionBoundary",
                    "requiredProperty",
                    "target",
                    "testPath",
                    "testSha256",
                },
                case_label,
            )
            case_id = _string(case["caseId"], f"{case_label}.caseId")
            category = _string(case["category"], f"{case_label}.category")
            if case_id <= previous_case or case_id in case_ids:
                _fail("negative case identities are duplicated or unsorted")
            previous_case = case_id
            case_ids.add(case_id)
            observed_categories.add(category)
            if (
                not case_id.startswith(f"{fixture_id}--")
                or case["fixtureId"] != fixture_id
                or case["target"] != fixture["target"]
                or case["fixtureBindingSha256"] != fixture["bindingSha256"]
            ):
                _fail(f"negative case {case_id} is cross-fixture or cross-target")
            if category not in required_categories:
                _fail(f"negative case {case_id} has an unrequired category")
            if case["requiredProperty"] not in kernel["requiredProperties"]:
                _fail(f"negative case {case_id} is not bound to a required property")
            if case["failureStage"] not in FAILURE_STAGES or case["productionBoundary"] != case["failureStage"]:
                _fail(f"negative case {case_id} has an invalid production boundary")
            if re.fullmatch(r"FE2O3-NEG-[0-9]{4}", str(case["diagnosticCode"])) is None:
                _fail(f"negative case {case_id} has an invalid diagnostic code")
            mutation = _object(case["mutation"], f"{case_label}.mutation")
            _exact(mutation, {"class", "path", "replacement", "subjectKind"}, f"{case_label}.mutation")
            if mutation["subjectKind"] not in SUBJECT_KINDS:
                _fail(f"negative case {case_id} has an invalid mutation subject")
            _, test = _repository_file(repository, case["testPath"], f"{case_label}.testPath")
            if case["testSha256"] != _sha256(test):
                _fail(f"negative case {case_id} test source is stale")
            if case["expectedResult"] != UNAVAILABLE_RESULT:
                _fail(f"negative case {case_id} invents a production result")
            base, mutated = derive_payloads(repository, document, case)
            expected_base = _domain_sha256(BASE_PAYLOAD_DOMAIN, base)
            expected_mutated = _domain_sha256(MUTATED_PAYLOAD_DOMAIN, mutated)
            if case["basePayloadSha256"] != expected_base or case["mutatedPayloadSha256"] != expected_mutated:
                _fail(f"negative case {case_id} mutation payload is stale or substituted")
            if expected_mutated in mutation_ids:
                _fail(f"negative case {case_id} reuses another fixture mutation payload")
            mutation_ids.add(expected_mutated)
        if not required_categories.issubset(observed_categories):
            _fail(
                f"negative fixture {fixture_id} category coverage differs: "
                f"missing={sorted(required_categories - observed_categories)!r}"
            )
        total_cases += len(fixture["cases"])
        fixtures[fixture_id] = fixture
    if set(fixtures) != set(manifest_fixtures):
        _fail("negative declarations must cover all 47 manifest fixtures exactly once")
    if total_cases > MAX_CASES:
        _fail("negative case roster exceeds its bound")
    return ValidatedDeclarations(raw_sha256, prerequisites, fixtures)


def load_declarations(
    repository: Path, document: dict[str, Any]
) -> tuple[dict[str, Any], ValidatedDeclarations]:
    repository = repository.resolve(strict=True)
    raw, declaration = _load_unique(repository / DECLARATION_PATH, "negative declaration")
    expected = f"{_sha256(raw)}  {DECLARATION_PATH.as_posix()}\n"
    if (repository / DECLARATION_DIGEST_PATH).read_text(encoding="ascii") != expected:
        _fail("negative declaration digest sidecar is stale")
    return declaration, validate_declarations(repository, document, declaration, _sha256(raw))


def command_sha256(command: dict[str, Any]) -> str:
    return _domain_sha256(COMMAND_DOMAIN, command)


def _run_prerequisite(repository: Path, prerequisite: dict[str, Any]) -> dict[str, Any]:
    command = prerequisite["command"]
    executable, _ = _repository_file(
        repository, command["executable"], "shared prerequisite executable", executable=True
    )
    environment = os.environ.copy()
    for assignment in command["environment"]:
        key, value = assignment.split("=", 1)
        environment[key] = value
    process = subprocess.Popen(
        [str(executable), *command["arguments"]],
        cwd=repository,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=command["timeoutSeconds"])
    except subprocess.TimeoutExpired as error:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        _fail(f"shared prerequisite {prerequisite['prerequisiteId']} timed out: {error}")
    if len(stdout) > MAX_OUTPUT_BYTES or len(stderr) > MAX_OUTPUT_BYTES:
        _fail(f"shared prerequisite {prerequisite['prerequisiteId']} exceeded its output bound")
    result = {
        "commandSha256": command_sha256(command),
        "exitStatus": process.returncode,
        "prerequisiteId": prerequisite["prerequisiteId"],
        "stderrBytes": len(stderr),
        "stderrSha256": _sha256(stderr),
        "stdoutBytes": len(stdout),
        "stdoutSha256": _sha256(stdout),
        "status": "passed",
    }
    expected = prerequisite["expectedObservation"]
    if any(result[field] != expected[field] for field in expected):
        detail = stderr.decode("utf-8", errors="replace").strip()
        _fail(
            f"shared prerequisite {prerequisite['prerequisiteId']} diagnostic changed"
            + (f": {detail}" if detail else "")
        )
    return result


def execute_fixture_case(
    repository: Path, document: dict[str, Any], case: dict[str, Any]
) -> dict[str, Any]:
    base, mutated = derive_payloads(repository, document, case)
    base_identity = _domain_sha256(BASE_PAYLOAD_DOMAIN, base)
    mutation_identity = _domain_sha256(MUTATED_PAYLOAD_DOMAIN, mutated)
    if base_identity != case["basePayloadSha256"] or mutation_identity != case["mutatedPayloadSha256"]:
        _fail(f"negative case {case['caseId']} changed after declaration validation")
    subject = {
        "basePayloadSha256": base_identity,
        "caseId": case["caseId"],
        "fixtureBindingSha256": case["fixtureBindingSha256"],
        "mutatedPayloadSha256": mutation_identity,
        "productionBoundary": case["productionBoundary"],
        "result": case["expectedResult"],
    }
    return {
        "basePayloadSha256": base_identity,
        "caseExecutionSha256": _domain_sha256(CASE_EXECUTION_DOMAIN, subject),
        "caseId": case["caseId"],
        "diagnosticCode": UNAVAILABLE_RESULT["diagnosticCode"],
        "fixtureBindingSha256": case["fixtureBindingSha256"],
        "fixtureId": case["fixtureId"],
        "mutatedPayloadSha256": mutation_identity,
        "mutationApplied": True,
        "productionBoundary": case["productionBoundary"],
        "productionBoundaryReached": False,
        "reason": UNAVAILABLE_RESULT["reason"],
        "status": "unavailable",
    }


def execute_fixture_cases(
    repository: Path,
    document: dict[str, Any],
    validated: ValidatedDeclarations,
) -> list[dict[str, Any]]:
    return [
        execute_fixture_case(repository, document, case)
        for fixture_id in sorted(validated.fixtures)
        for case in validated.fixtures[fixture_id]["cases"]
    ]


def build_evidence(
    declaration: dict[str, Any],
    validated: ValidatedDeclarations,
    prerequisite_results: list[dict[str, Any]],
    fixture_results: list[dict[str, Any]],
) -> dict[str, Any]:
    by_case = {result["caseId"]: result for result in fixture_results}
    if len(by_case) != len(fixture_results):
        _fail("negative fixture results are duplicated")
    cases = []
    for fixture_id in sorted(validated.fixtures):
        for case in validated.fixtures[fixture_id]["cases"]:
            if case["caseId"] not in by_case:
                _fail("negative fixture execution coverage is missing")
            result = by_case[case["caseId"]]
            subject = {
                "case": case,
                "declarationSha256": validated.raw_sha256,
                "result": result,
            }
            cases.append(
                {
                    "caseEvidenceSha256": _domain_sha256(CASE_EVIDENCE_DOMAIN, subject),
                    "caseExecutionSha256": result["caseExecutionSha256"],
                    "caseId": case["caseId"],
                    "category": case["category"],
                    "failureStage": case["failureStage"],
                    "fixtureId": fixture_id,
                    "mutatedPayloadSha256": case["mutatedPayloadSha256"],
                    "status": result["status"],
                    "target": case["target"],
                }
            )
    return {
        "authority": "observation-only-no-production-authority",
        "caseCount": len(cases),
        "cases": cases,
        "declarationSha256": validated.raw_sha256,
        "fixtureContractSha256": declaration["fixtureContractSha256"],
        "fixtureCount": len(validated.fixtures),
        "fixtureResults": fixture_results,
        "schema": EVIDENCE_SCHEMA,
        "sharedPrerequisiteResults": prerequisite_results,
        "status": "unavailable",
    }


def validate_evidence(
    declaration: dict[str, Any],
    validated: ValidatedDeclarations,
    evidence: dict[str, Any],
) -> None:
    _exact(
        evidence,
        {
            "authority",
            "caseCount",
            "cases",
            "declarationSha256",
            "fixtureContractSha256",
            "fixtureCount",
            "fixtureResults",
            "schema",
            "sharedPrerequisiteResults",
            "status",
        },
        "negative evidence",
    )
    if (
        evidence["schema"] != EVIDENCE_SCHEMA
        or evidence["authority"] != "observation-only-no-production-authority"
        or evidence["status"] != "unavailable"
        or evidence["declarationSha256"] != validated.raw_sha256
        or evidence["fixtureContractSha256"] != declaration["fixtureContractSha256"]
    ):
        _fail("negative evidence has stale status, authority, or declaration binding")
    prerequisite_results = _array(
        evidence["sharedPrerequisiteResults"], "negative evidence.sharedPrerequisiteResults"
    )
    prerequisite_ids = [item.get("prerequisiteId") for item in prerequisite_results]
    if prerequisite_ids != sorted(validated.shared_prerequisites):
        _fail("negative evidence shared prerequisite coverage is missing or duplicated")
    for result in prerequisite_results:
        prerequisite = validated.shared_prerequisites[result["prerequisiteId"]]
        expected = prerequisite["expectedObservation"]
        if (
            result.get("status") != "passed"
            or result.get("commandSha256") != command_sha256(prerequisite["command"])
            or any(result.get(field) != expected[field] for field in expected)
        ):
            _fail("negative evidence shared prerequisite transcript is stale")
    fixture_results = _array(evidence["fixtureResults"], "negative evidence.fixtureResults", nonempty=True)
    expected_cases = [
        case
        for fixture_id in sorted(validated.fixtures)
        for case in validated.fixtures[fixture_id]["cases"]
    ]
    if [item.get("caseId") for item in fixture_results] != [case["caseId"] for case in expected_cases]:
        _fail("negative evidence fixture execution coverage is missing, duplicated, or cross-fixture")
    execution_ids: set[str] = set()
    for result, case in zip(fixture_results, expected_cases):
        if (
            result.get("status") != "unavailable"
            or result.get("mutationApplied") is not True
            or result.get("productionBoundaryReached") is not False
            or result.get("fixtureId") != case["fixtureId"]
            or result.get("fixtureBindingSha256") != case["fixtureBindingSha256"]
            or result.get("basePayloadSha256") != case["basePayloadSha256"]
            or result.get("mutatedPayloadSha256") != case["mutatedPayloadSha256"]
            or result.get("productionBoundary") != case["productionBoundary"]
            or result.get("diagnosticCode") != UNAVAILABLE_RESULT["diagnosticCode"]
            or result.get("reason") != UNAVAILABLE_RESULT["reason"]
        ):
            _fail(f"negative evidence fixture result {case['caseId']} is stale or substituted")
        execution_identity = _sha(
            result.get("caseExecutionSha256"),
            f"fixture result {case['caseId']}.caseExecutionSha256",
        )
        subject = {
            "basePayloadSha256": case["basePayloadSha256"],
            "caseId": case["caseId"],
            "fixtureBindingSha256": case["fixtureBindingSha256"],
            "mutatedPayloadSha256": case["mutatedPayloadSha256"],
            "productionBoundary": case["productionBoundary"],
            "result": case["expectedResult"],
        }
        if execution_identity != _domain_sha256(CASE_EXECUTION_DOMAIN, subject):
            _fail(f"negative evidence fixture result {case['caseId']} has a stale execution identity")
        if execution_identity in execution_ids:
            _fail("negative evidence reuses one execution result across fixture cases")
        execution_ids.add(execution_identity)
    expected = build_evidence(declaration, validated, prerequisite_results, fixture_results)
    if evidence != expected:
        _fail("negative evidence case binding is stale")


def execute(repository: Path, document: dict[str, Any]) -> dict[str, Any]:
    declaration, validated = load_declarations(repository, document)
    prerequisites = [
        _run_prerequisite(repository, validated.shared_prerequisites[identity])
        for identity in sorted(validated.shared_prerequisites)
    ]
    fixture_results = execute_fixture_cases(repository, document, validated)
    evidence = build_evidence(declaration, validated, prerequisites, fixture_results)
    validate_evidence(declaration, validated, evidence)
    return evidence


def expected_record_cases(
    fixture_id: str,
    validated: ValidatedDeclarations,
    evidence: dict[str, Any],
) -> list[dict[str, Any]]:
    evidence_by_id = {case["caseId"]: case for case in evidence["cases"]}
    return [
        {
            "category": case["category"],
            "diagnosticCode": case["diagnosticCode"],
            "evidenceSha256": evidence_by_id[case["caseId"]]["caseEvidenceSha256"],
            "failureStage": case["failureStage"],
            "fixtureId": case["caseId"],
            "testPath": case["testPath"],
            "testSha256": case["testSha256"],
        }
        for case in validated.fixtures[fixture_id]["cases"]
    ]


def validate_promotion_records(
    repository: Path,
    document: dict[str, Any],
    records: list[dict[str, Any]],
    evidence: dict[str, Any],
) -> None:
    declaration, validated = load_declarations(repository, document)
    validate_evidence(declaration, validated, evidence)
    if evidence["status"] != "passed" or any(case["status"] != "passed" for case in evidence["cases"]):
        _fail("negative fixture production lifecycle is unavailable")
    record_ids = [record.get("fixtureId") for record in records if isinstance(record, dict)]
    if record_ids != sorted(validated.fixtures):
        _fail("negative promotion records are missing, duplicated, or cross-fixture")
