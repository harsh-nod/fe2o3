#!/usr/bin/env python3
"""Run one manifest fixture through exact production Bundle V8 simulation.

This command produces observation-only evidence. It never promotes extraction
custody into compiler, proof, artifact, launch, or hardware authority.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import signal
import stat
import subprocess
import sys
import tempfile
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST = REPO_ROOT / "config" / "tutorial-kernel-manifest-v1.json"
FIXTURE_ROOT = REPO_ROOT / "scripts" / "tutorial-semantic-fixtures-v1"
FIXTURE_SCHEMA = "fe2o3-tutorial-capability-simulation-fixture-v1"
ABI_SCHEMA = "fe2o3-physical-kernel-abi-v1"
REQUEST_SCHEMA = "fe2o3-simulation-request-v1"
SIMULATOR_RESULT_SCHEMA = "fe2o3-simulation-result-v1"
SUITE_RESULT_SCHEMA = "fe2o3-tutorial-semantic-suite-result-v1"
SCHEDULE_SCHEMA = "fe2o3-simulation-schedule-v1"
COMMAND_DOMAIN = b"fe2o3-tutorial-semantic-command-v1\0"
POLICY_DOMAIN = b"fe2o3-tutorial-numerical-policy-v1\0"
LAUNCH_DOMAIN = b"fe2o3-tutorial-simulation-launch-v1\0"
OBSERVATION_DOMAIN = b"fe2o3-tutorial-capability-simulation-observation-v1\0"
DIAGNOSTIC_IDENTITY_DOMAIN = b"fe2o3-tutorial-production-export-diagnostic-v1\0"
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")
MAX_JSON_BYTES = 64 * 1024 * 1024
MAX_PROCESS_STDOUT = 64 * 1024 * 1024
MAX_PROCESS_STDERR = 8 * 1024 * 1024
MAX_ARGUMENTS = 256
MAX_CHECK_RANGES = 4096
TIMEOUT_SECONDS = 3600
PRODUCTION_EXPORT_IDENTITY_CONTRACT = {
    "rawContentField": "evidenceFiles[optimized-kir-v13].sha256",
    "rawContentIdentity": "SHA-256(raw optimized KIR V13 bytes)",
    "verifiedCanonicalKernelIrV13Fields": [
        "productionEvidence.finalOptimizedKirSha256",
        "graph.productionKirIdentitySha256",
        "simulator.subjectSha256",
    ],
    "verifiedCanonicalKernelIrV13Identity": (
        "domain-separated VerifiedCanonicalKernelIrV13 identity"
    ),
}
PRODUCER_ERROR = re.compile(r"(FE2O3-TUTORIAL-(?:PROBE|TXN)-[0-9]{3}): .+\Z")
PRODUCER_STAGES = {
    f"FE2O3-TUTORIAL-TXN-{number:03d}": stage
    for number, stage in enumerate(
        (
            "request-io",
            "request-decode",
            "request-schema",
            "request-binding",
            "source-preflight",
            "target-preflight",
            "protected-completion",
            "production-result-validation",
            "artifact-validation",
            "evidence-presence",
            "evidence-validation",
            "hardware-receipt",
            "output-preflight",
            "publication",
            "qualification-schema",
        ),
        start=1,
    )
}
PRODUCER_STAGES.update(
    {
        "FE2O3-TUTORIAL-PROBE-001": "exporter-build",
        "FE2O3-TUTORIAL-PROBE-002": "exporter-build",
    }
)


class SimulationQualificationError(ValueError):
    """A fixture cannot produce capability-path simulation evidence."""


def _fail(message: str) -> None:
    raise SimulationQualificationError(message)


def _single_role_argument(role: str, constructor: str) -> bool:
    prefix = f"{constructor}<"
    if not role.startswith(prefix) or not role.endswith(">"):
        return False
    argument = role[len(prefix) : -1].strip()
    if not argument:
        return False
    depth = 0
    for character in argument:
        if character in "<([":
            depth += 1
        elif character in ">)]":
            depth -= 1
            if depth < 0:
                return False
        elif character == "," and depth == 0:
            return False
    return depth == 0


def _source_buffer_abi(source_type: str) -> tuple[str, str] | None:
    source_global = re.fullmatch(
        r"Global<'_,\s*(u8|u16|u32|u64|i32|f32),\s*(.+)>", source_type
    )
    if source_global is not None:
        element, role = source_global.groups()
        if role == "ReadOnly":
            return element, "read_only"
        if _single_role_argument(role, "DisjointWrite"):
            return element, "write_only"
        if role == "ExclusiveReadWrite":
            return element, "read_write"
        if _single_role_argument(role, "AtomicReadWrite"):
            return element, "read_write"
        _fail(f"unsupported Global capability role in physical ABI: {role}")

    source_write_only = re.fullmatch(
        r"WriteOnlyDisjointSlice<(u8|u16|u32|u64|i32|f32),\s*.+>", source_type
    )
    if source_write_only is not None:
        return source_write_only.group(1), "write_only"
    if source_type.startswith(("Global<", "WriteOnlyDisjointSlice<")):
        _fail(f"malformed capability buffer type in physical ABI: {source_type}")
    return None


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


def _decode_unique(payload: bytes, label: str) -> Any:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} contains duplicate JSON key {key!r}")
            result[key] = value
        return result

    try:
        return json.loads(payload, object_pairs_hook=reject_duplicates)
    except (UnicodeError, json.JSONDecodeError) as error:
        _fail(f"cannot decode {label}: {error}")


def _read_regular(path: Path, label: str, maximum: int = MAX_JSON_BYTES) -> bytes:
    try:
        metadata = path.lstat()
        if path.is_symlink() or not stat.S_ISREG(metadata.st_mode):
            _fail(f"{label} must be a regular non-symlink file")
        if not 0 < metadata.st_size <= maximum:
            _fail(f"{label} has an invalid byte length")
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            before = os.fstat(descriptor)
            chunks: list[bytes] = []
            observed = 0
            while True:
                chunk = os.read(descriptor, min(1024 * 1024, maximum - observed + 1))
                if not chunk:
                    break
                observed += len(chunk)
                if observed > maximum:
                    _fail(f"{label} exceeds its byte bound")
                chunks.append(chunk)
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
    except OSError as error:
        _fail(f"cannot read {label}: {error}")
    stable = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) == (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
    )
    if not stable or observed != metadata.st_size:
        _fail(f"{label} changed while it was read")
    return b"".join(chunks)


def _load_json(path: Path, label: str, maximum: int = MAX_JSON_BYTES) -> tuple[bytes, Any]:
    payload = _read_regular(path, label, maximum)
    return payload, _decode_unique(payload, label)


def _object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        _fail(f"{label} must be an object")
    return value


def _array(value: Any, label: str, *, nonempty: bool = False) -> list[Any]:
    if not isinstance(value, list) or (nonempty and not value):
        qualifier = "nonempty " if nonempty else ""
        _fail(f"{label} must be a {qualifier}array")
    return value


def _exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        _fail(
            f"{label} fields differ: missing={sorted(expected - set(value))!r} "
            f"extra={sorted(set(value) - expected)!r}"
        )


def _hex_bytes(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or not value.startswith("0x") or len(value) % 2:
        _fail(f"{label} is not an even-length 0x-prefixed byte string")
    try:
        return bytes.fromhex(value[2:])
    except ValueError as error:
        _fail(f"{label} is not hexadecimal: {error}")


def _repo_file(relative: Any, label: str, *, executable: bool = False) -> Path:
    if not isinstance(relative, str) or not relative:
        _fail(f"{label} must be a nonempty repository-relative path")
    portable = PurePosixPath(relative)
    if portable.is_absolute() or ".." in portable.parts or "\\" in relative:
        _fail(f"{label} escapes the repository")
    current = REPO_ROOT
    for component in portable.parts:
        current /= component
        try:
            metadata = current.lstat()
        except OSError as error:
            _fail(f"cannot inspect {label}: {error}")
        if current.is_symlink():
            _fail(f"{label} traverses a symlink")
    if not stat.S_ISREG(metadata.st_mode) or (executable and metadata.st_mode & 0o111 == 0):
        qualifier = " executable" if executable else ""
        _fail(f"{label} must be a regular{qualifier} file")
    return current


def _index(records: Any, key: str, label: str) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for offset, raw in enumerate(_array(records, label, nonempty=True)):
        record = _object(raw, f"{label}[{offset}]")
        identity = record.get(key)
        if not isinstance(identity, str) or not identity or identity in result:
            _fail(f"{label}[{offset}].{key} is missing or duplicated")
        result[identity] = record
    return result


def manifest_command(fixture_id: str) -> dict[str, Any]:
    if SLUG.fullmatch(fixture_id) is None:
        _fail("fixture identity is not a slug")
    return {
        "executable": "scripts/run-tutorial-semantic-simulation.py",
        "arguments": ["--fixture", fixture_id],
        "environment": [],
        "workingDirectory": ".",
        "timeoutSeconds": TIMEOUT_SECONDS,
    }


def command_sha256(command: dict[str, Any]) -> str:
    return _domain_sha256(COMMAND_DOMAIN, command)


def manifest_simulation_roster(document: dict[str, Any]) -> dict[str, dict[str, Any]]:
    fixtures = _index(document.get("compilerFixtures"), "fixtureId", "compilerFixtures")
    entries = _index(document.get("entries"), "lessonId", "entries")
    owners: dict[str, list[str]] = {fixture: [] for fixture in fixtures}
    for lesson_id, entry in entries.items():
        for fixture_id in _array(entry.get("compilerFixtureIds"), f"entry {lesson_id}.compilerFixtureIds"):
            if fixture_id not in owners:
                _fail(f"entry {lesson_id} names unknown fixture {fixture_id}")
            owners[fixture_id].append(lesson_id)
    if any(not lessons for lessons in owners.values()):
        _fail("every compiler fixture must belong to at least one lesson")
    return {
        fixture_id: {
            "command": manifest_command(fixture_id),
            "coverage": [
                {"fixtureIds": [fixture_id], "lessonId": lesson_id}
                for lesson_id in sorted(lessons)
            ],
            "suiteId": f"semantic-simulation-{fixture_id}",
        }
        for fixture_id, lessons in sorted(owners.items())
    }


def _cpu_suite(document: dict[str, Any], fixture_id: str) -> dict[str, Any]:
    matches: list[dict[str, Any]] = []
    qualification = _object(document.get("qualification"), "qualification")
    for raw in _array(qualification.get("suites"), "qualification.suites"):
        suite = _object(raw, "qualification suite")
        if suite.get("gate") != "cpu-reference" or suite.get("availability") != "available":
            continue
        coverage = map(
            lambda value: _object(value, "qualification coverage"),
            _array(suite.get("coverage"), "qualification suite.coverage"),
        )
        if any(
            fixture_id
            in _array(item.get("fixtureIds"), "qualification coverage.fixtureIds")
            for item in coverage
        ):
            matches.append(suite)
    unique = {suite["suiteId"]: suite for suite in matches}
    if len(unique) != 1:
        _fail(
            f"fixture {fixture_id} requires exactly one available declared CPU-reference oracle; "
            f"found {len(unique)}"
        )
    return next(iter(unique.values()))


def _fixture_path(fixture_id: str) -> Path:
    return FIXTURE_ROOT / f"{fixture_id}.json"


def load_fixture(document: dict[str, Any], fixture_id: str) -> tuple[dict[str, Any], bytes, dict[str, Any]]:
    fixtures = _index(document.get("compilerFixtures"), "fixtureId", "compilerFixtures")
    kernels = _index(document.get("capabilityKernels"), "fixtureId", "capabilityKernels")
    if fixture_id not in fixtures or fixture_id not in kernels:
        _fail(f"unknown manifest fixture {fixture_id!r}")
    path = _fixture_path(fixture_id)
    if not path.exists():
        try:
            display_path = path.relative_to(REPO_ROOT)
        except ValueError:
            display_path = path
        _fail(
            f"fixture {fixture_id} has no genuine typed capability simulation input at "
            f"{display_path}"
        )
    payload, decoded = _load_json(path, f"simulation fixture {fixture_id}")
    record = _object(decoded, f"simulation fixture {fixture_id}")
    _exact_keys(
        record,
        {
            "argumentRoles",
            "canaries",
            "checkApplicability",
            "compilerInputContractSha256",
            "expectedArguments",
            "fixtureId",
            "kernelSymbol",
            "numericalPolicy",
            "oracle",
            "oracleSuiteId",
            "padding",
            "physicalAbi",
            "productionExport",
            "request",
            "schema",
            "sourceClosureSha256",
            "target",
        },
        f"simulation fixture {fixture_id}",
    )
    manifest_fixture = fixtures[fixture_id]
    manifest_kernel = kernels[fixture_id]
    expected_coordinates = {
        "compilerInputContractSha256": manifest_fixture["compilerInput"]["contractSha256"],
        "fixtureId": fixture_id,
        "kernelSymbol": manifest_kernel["kernelSymbol"],
        "sourceClosureSha256": manifest_fixture["compilerInput"]["sourceClosureSha256"],
        "target": manifest_fixture["target"],
    }
    for key, expected in expected_coordinates.items():
        if record.get(key) != expected:
            _fail(f"simulation fixture {fixture_id}.{key} is stale or substituted")
    if record.get("schema") != FIXTURE_SCHEMA:
        _fail(f"simulation fixture {fixture_id} has an unsupported schema")
    request = _object(record.get("request"), f"simulation fixture {fixture_id}.request")
    if request.get("schema") != REQUEST_SCHEMA or request.get("kernel") != record["kernelSymbol"]:
        _fail(f"simulation fixture {fixture_id} request targets a different kernel")
    for field in ("grid", "workgroup"):
        dimensions = request.get(field)
        if (
            not isinstance(dimensions, list)
            or len(dimensions) != 3
            or any(not isinstance(value, int) or isinstance(value, bool) or value <= 0 for value in dimensions)
        ):
            _fail(f"simulation fixture {fixture_id} request has invalid {field}")
    arguments = _array(request.get("arguments"), f"simulation fixture {fixture_id}.request.arguments")
    expected_arguments = _array(record.get("expectedArguments"), f"simulation fixture {fixture_id}.expectedArguments")
    roles = _array(record.get("argumentRoles"), f"simulation fixture {fixture_id}.argumentRoles")
    if not len(arguments) == len(expected_arguments) == len(roles) or len(arguments) > MAX_ARGUMENTS:
        _fail(f"simulation fixture {fixture_id} argument rosters differ or exceed their bound")
    if any(role not in {"input", "output", "inout", "scalar"} for role in roles):
        _fail(f"simulation fixture {fixture_id} has an invalid argument role")
    _validate_physical_abi(
        record.get("physicalAbi"), arguments, roles, expected_arguments,
        manifest_fixture, fixture_id, record["kernelSymbol"],
    )
    cpu = _cpu_suite(document, fixture_id)
    if record.get("oracleSuiteId") != cpu.get("suiteId"):
        _fail(f"simulation fixture {fixture_id} names a stale CPU oracle")
    _validate_numerical_policy(record.get("numericalPolicy"), fixture_id)
    _validate_ranges(record.get("canaries"), arguments, fixture_id, "canaries")
    _validate_ranges(record.get("padding"), arguments, fixture_id, "padding")
    _validate_complete_checks(record, arguments, expected_arguments, roles, fixture_id)
    oracle = _object(record.get("oracle"), f"simulation fixture {fixture_id}.oracle")
    _exact_keys(oracle, {"function", "inputSha256", "kind"}, f"simulation fixture {fixture_id}.oracle")
    if oracle.get("kind") != "existing-rust-cpu-reference-v1":
        _fail(f"simulation fixture {fixture_id} does not name an existing Rust CPU reference")
    if not isinstance(oracle.get("function"), str) or not oracle["function"]:
        _fail(f"simulation fixture {fixture_id} has no CPU reference function")
    oracle_input = {
        "arguments": request["arguments"],
        "grid": request["grid"],
        "kernel": request["kernel"],
        "workgroup": request["workgroup"],
    }
    if oracle.get("inputSha256") != _sha256(_canonical(oracle_input)):
        _fail(f"simulation fixture {fixture_id} CPU oracle input identity is stale")
    export = _object(record.get("productionExport"), f"simulation fixture {fixture_id}.productionExport")
    _exact_keys(
        export,
        {
            "coordinates",
            "diagnostic",
            "diagnosticCode",
            "diagnosticIdentitySha256",
            "identityContract",
            "stage",
            "status",
        },
        f"simulation fixture {fixture_id}.productionExport",
    )
    if export.get("identityContract") != PRODUCTION_EXPORT_IDENTITY_CONTRACT:
        _fail(f"simulation fixture {fixture_id} conflates canonical KIR and raw content identities")
    if export.get("status") not in {"blocked", "available"}:
        _fail(f"simulation fixture {fixture_id} has invalid production export status")
    if export.get("status") == "blocked":
        diagnostic = export.get("diagnostic")
        code = export.get("diagnosticCode")
        match = PRODUCER_ERROR.fullmatch(diagnostic) if isinstance(diagnostic, str) else None
        if (
            export.get("coordinates") is not None
            or match is None
            or not isinstance(code, str)
            or match.group(1) != code
            or PRODUCER_STAGES.get(code) != export.get("stage")
        ):
            _fail(f"simulation fixture {fixture_id} omits its exact production export blocker")
        diagnostic_subject = {
            "compilerInputContractSha256": manifest_fixture["compilerInput"]["contractSha256"],
            "diagnostic": diagnostic,
            "diagnosticCode": code,
            "fixtureId": fixture_id,
            "identityContract": export["identityContract"],
            "kernelSymbol": manifest_kernel["kernelSymbol"],
            "sourceClosureSha256": manifest_fixture["compilerInput"]["sourceClosureSha256"],
            "stage": export["stage"],
            "status": export["status"],
            "target": manifest_fixture["target"],
        }
        if export.get("diagnosticIdentitySha256") != _sha256(
            DIAGNOSTIC_IDENTITY_DOMAIN + _canonical(diagnostic_subject)
        ):
            _fail(f"simulation fixture {fixture_id} production diagnostic identity is stale")
    else:
        if (
            export.get("diagnostic") is not None
            or export.get("diagnosticCode") is not None
            or export.get("diagnosticIdentitySha256") is not None
            or export.get("stage") != "production-transaction"
        ):
            _fail(f"simulation fixture {fixture_id} has stale success diagnostics")
        coordinates = _object(
            export.get("coordinates"),
            f"simulation fixture {fixture_id}.productionExport.coordinates",
        )
        _exact_keys(
            coordinates,
            {
                "bundleContentIdentitySha256",
                "bundleSubjectIdentitySha256",
                "bundleVersion",
                "finalOptimizedKirSha256",
                "kirVersion",
                "optimizedKirV13ContentSha256",
                "productionKirIdentitySha256",
                "simulationBundleV8ContentSha256",
                "simulatorSubjectSha256",
                "target",
            },
            f"simulation fixture {fixture_id}.productionExport.coordinates",
        )
        for field in (
            "bundleContentIdentitySha256",
            "bundleSubjectIdentitySha256",
            "finalOptimizedKirSha256",
            "optimizedKirV13ContentSha256",
            "productionKirIdentitySha256",
            "simulationBundleV8ContentSha256",
            "simulatorSubjectSha256",
        ):
            if not isinstance(coordinates[field], str) or SHA256.fullmatch(coordinates[field]) is None:
                _fail(f"simulation fixture {fixture_id} has a malformed {field}")
        verified = {
            coordinates["finalOptimizedKirSha256"],
            coordinates["productionKirIdentitySha256"],
            coordinates["simulatorSubjectSha256"],
        }
        if len(verified) != 1:
            _fail(f"simulation fixture {fixture_id} has inconsistent VerifiedCanonicalKernelIrV13 identities")
        if coordinates["optimizedKirV13ContentSha256"] in verified:
            _fail(f"simulation fixture {fixture_id} conflates canonical KIR and raw content identities")
        if (
            coordinates["bundleVersion"] != 8
            or coordinates["kirVersion"] != 13
            or coordinates["target"] != record["target"]
        ):
            _fail(f"simulation fixture {fixture_id} has stale bundle coordinates")
    return record, payload, cpu


def _split_parameters(parameters: str) -> list[str]:
    result: list[str] = []
    start = 0
    depth = 0
    for index, character in enumerate(parameters):
        if character in "<([":
            depth += 1
        elif character in ">)]":
            depth -= 1
        elif character == "," and depth == 0:
            value = parameters[start:index].strip()
            if value:
                result.append(value)
            start = index + 1
    value = parameters[start:].strip()
    if value:
        result.append(value)
    return result


def _source_signature(source: str, symbol: str, fixture_id: str) -> tuple[str, list[str]]:
    marker = f"pub fn {symbol}("
    start = source.find(marker)
    if start < 0:
        _fail(f"simulation fixture {fixture_id} ABI source omits {symbol}")
    start += len(marker)
    depth = 1
    cursor = start
    while cursor < len(source) and depth:
        if source[cursor] == "(":
            depth += 1
        elif source[cursor] == ")":
            depth -= 1
        cursor += 1
    if depth:
        _fail(f"simulation fixture {fixture_id} ABI source has an unterminated signature")
    parameters = _split_parameters(source[start : cursor - 1])
    normalized = f"pub fn {symbol}({','.join(' '.join(item.split()) for item in parameters)})"
    return normalized, parameters


def _validate_physical_abi(
    raw: Any,
    arguments: list[Any],
    roles: list[Any],
    expected_arguments: list[Any],
    manifest_fixture: dict[str, Any],
    fixture_id: str,
    symbol: str,
) -> None:
    abi = _object(raw, f"simulation fixture {fixture_id}.physicalAbi")
    _exact_keys(abi, {"arguments", "schema", "signatureSha256", "sourcePath"}, f"simulation fixture {fixture_id}.physicalAbi")
    if abi.get("schema") != ABI_SCHEMA:
        _fail(f"simulation fixture {fixture_id} has an unsupported physical ABI schema")
    source_path = abi.get("sourcePath")
    compiler_input = _object(manifest_fixture.get("compilerInput"), "compilerInput")
    if source_path not in _array(compiler_input.get("sourcePaths"), "compilerInput.sourcePaths"):
        _fail(f"simulation fixture {fixture_id} ABI source is outside its source closure")
    source = _repo_file(source_path, f"simulation fixture {fixture_id} ABI source")
    try:
        source_text = source.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        _fail(f"cannot decode simulation fixture {fixture_id} ABI source: {error}")
    normalized, parameters = _source_signature(source_text, symbol, fixture_id)
    if abi.get("signatureSha256") != _sha256(normalized.encode("utf-8")):
        _fail(f"simulation fixture {fixture_id} physical ABI signature is stale")
    physical = _array(abi.get("arguments"), f"simulation fixture {fixture_id}.physicalAbi.arguments")
    source_physical = [item for item in parameters if "KernelContext<'_>" not in item]
    if len(physical) != len(source_physical) or len(physical) != len(arguments):
        _fail(f"simulation fixture {fixture_id} physical ABI argument count differs")
    seen: set[str] = set()
    for index, (specification_raw, argument_raw, wanted_raw, role) in enumerate(
        zip(physical, arguments, expected_arguments, roles)
    ):
        specification = _object(specification_raw, f"physical ABI argument {index}")
        argument = _object(argument_raw, f"request argument {index}")
        wanted = _object(wanted_raw, f"expected argument {index}")
        source_parameter = re.sub(r"^mut\s+", "", source_physical[index].strip())
        source_name, separator, source_type = source_parameter.partition(":")
        if not separator:
            _fail(f"simulation fixture {fixture_id} cannot parse physical ABI argument {index}")
        source_name = source_name.strip()
        source_type = " ".join(source_type.split())
        name = specification.get("name")
        if not isinstance(name, str) or not name or name in seen or name != source_name:
            _fail(f"simulation fixture {fixture_id} physical ABI argument names are invalid")
        seen.add(name)
        if specification.get("kind") == "scalar":
            _exact_keys(specification, {"kind", "name", "type"}, f"physical ABI argument {index}")
            _exact_keys(argument, {"bits", "kind", "type"}, f"request argument {index}")
            _exact_keys(wanted, {"bits", "kind", "type"}, f"expected argument {index}")
            if (
                role != "scalar"
                or argument.get("type") != specification.get("type")
                or source_type != specification.get("type")
            ):
                _fail(f"simulation fixture {fixture_id} scalar argument {index} differs from its ABI")
        elif specification.get("kind") == "buffer":
            _exact_keys(specification, {"access", "element", "kind", "name"}, f"physical ABI argument {index}")
            _exact_keys(argument, {"access", "alignment", "bytes", "element", "kind"}, f"request argument {index}")
            _exact_keys(wanted, {"bytes", "element", "kind"}, f"expected argument {index}")
            source_buffer_abi = _source_buffer_abi(source_type)
            source_element, source_access = (
                source_buffer_abi if source_buffer_abi is not None else (None, None)
            )
            if (
                source_element is None
                or source_element != specification.get("element")
                or source_access != specification.get("access")
                or argument.get("element") != specification.get("element")
                or argument.get("access") != specification.get("access")
            ):
                _fail(f"simulation fixture {fixture_id} buffer argument {index} differs from its ABI")
            expected_role = "input" if specification.get("access") == "read_only" else "output"
            if role != expected_role:
                _fail(f"simulation fixture {fixture_id} argument {index} role differs from its ABI")
        else:
            _fail(f"simulation fixture {fixture_id} physical ABI argument {index} has invalid kind")
        request_kind, request_payload = argument["kind"], _argument_bytes(argument, f"request argument {index}")
        expected_kind, expected_payload = _expected_argument_payload(wanted, f"expected argument {index}")
        if request_kind != expected_kind or len(request_payload) != len(expected_payload):
            _fail(f"simulation fixture {fixture_id} argument {index} expected extent is incomplete")
        if role in {"input", "scalar"} and request_payload != expected_payload:
            _fail(f"simulation fixture {fixture_id} immutable argument {index} differs from its expected bytes")


def _validate_complete_checks(
    record: dict[str, Any],
    arguments: list[Any],
    expected_arguments: list[Any],
    roles: list[Any],
    fixture_id: str,
) -> None:
    applicability = _object(record.get("checkApplicability"), f"simulation fixture {fixture_id}.checkApplicability")
    _exact_keys(applicability, {"canaries", "padding"}, f"simulation fixture {fixture_id}.checkApplicability")
    for label in ("canaries", "padding"):
        policy = _object(applicability.get(label), f"simulation fixture {fixture_id}.checkApplicability.{label}")
        _exact_keys(policy, {"reason", "status"}, f"simulation fixture {fixture_id}.checkApplicability.{label}")
        ranges = record[label]
        if policy.get("status") == "checked":
            if not ranges or policy.get("reason") is not None:
                _fail(f"simulation fixture {fixture_id} {label} check is incomplete")
        elif policy.get("status") == "not-applicable":
            if ranges or not isinstance(policy.get("reason"), str) or not policy["reason"]:
                _fail(f"simulation fixture {fixture_id} {label} applicability is unexplained")
        else:
            _fail(f"simulation fixture {fixture_id} has invalid {label} applicability")
        for check in ranges:
            if roles[check["argument"]] not in {"output", "inout"}:
                _fail(f"simulation fixture {fixture_id} {label} checks a non-output argument")
            _expected_argument_payload(expected_arguments[check["argument"]], f"expected {label} argument")


def _validate_numerical_policy(raw: Any, fixture_id: str) -> None:
    policy = _object(raw, f"simulation fixture {fixture_id}.numericalPolicy")
    mode = policy.get("mode")
    if mode == "exact-bits":
        _exact_keys(policy, {"mode"}, f"simulation fixture {fixture_id}.numericalPolicy")
        return
    _exact_keys(
        policy,
        {"absoluteTolerance", "allowNaN", "mode", "relativeTolerance"},
        f"simulation fixture {fixture_id}.numericalPolicy",
    )
    if mode != "f32-tolerance":
        _fail(f"simulation fixture {fixture_id} has an unsupported numerical policy")
    for field in ("absoluteTolerance", "relativeTolerance"):
        value = policy[field]
        if not isinstance(value, (int, float)) or isinstance(value, bool) or value < 0:
            _fail(f"simulation fixture {fixture_id} has invalid {field}")
    if not isinstance(policy["allowNaN"], bool):
        _fail(f"simulation fixture {fixture_id} has invalid allowNaN")


def _validate_ranges(raw: Any, arguments: list[Any], fixture_id: str, label: str) -> None:
    ranges = _array(raw, f"simulation fixture {fixture_id}.{label}")
    if len(ranges) > MAX_CHECK_RANGES:
        _fail(f"simulation fixture {fixture_id}.{label} exceeds its bound")
    previous: tuple[int, int] | None = None
    for offset, value in enumerate(ranges):
        item = _object(value, f"simulation fixture {fixture_id}.{label}[{offset}]")
        _exact_keys(item, {"argument", "bytes", "offset"}, f"simulation fixture {fixture_id}.{label}[{offset}]")
        argument = item["argument"]
        start = item["offset"]
        expected = _hex_bytes(item["bytes"], f"simulation fixture {fixture_id}.{label}[{offset}].bytes")
        if (
            not isinstance(argument, int)
            or isinstance(argument, bool)
            or not 0 <= argument < len(arguments)
            or not isinstance(start, int)
            or isinstance(start, bool)
            or start < 0
        ):
            _fail(f"simulation fixture {fixture_id}.{label}[{offset}] has an invalid range")
        argument_bytes = _argument_bytes(arguments[argument], f"request argument {argument}")
        if start + len(expected) > len(argument_bytes):
            _fail(f"simulation fixture {fixture_id}.{label}[{offset}] exceeds its argument")
        coordinate = (argument, start)
        if previous is not None and coordinate <= previous:
            _fail(f"simulation fixture {fixture_id}.{label} is not strictly ordered")
        previous = coordinate


def _argument_bytes(argument: Any, label: str) -> bytes:
    argument = _object(argument, label)
    kind = argument.get("kind")
    if kind == "scalar":
        return _hex_bytes(argument.get("bits"), f"{label}.bits")
    if kind == "buffer":
        return _hex_bytes(argument.get("bytes"), f"{label}.bytes")
    _fail(f"{label} is not a scalar or standalone buffer")


def _run_bounded(
    command: list[str],
    *,
    cwd: Path,
    timeout: int,
    environment: dict[str, str] | None = None,
) -> tuple[bytes, bytes]:
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            _fail(f"command exceeded its {timeout} second bound: {error}")
        if stdout.tell() > MAX_PROCESS_STDOUT or stderr.tell() > MAX_PROCESS_STDERR:
            _fail("command output exceeded its byte bound")
        stdout.seek(0)
        stderr.seek(0)
        output, diagnostic = stdout.read(), stderr.read()
    if status != 0:
        detail = diagnostic.decode("utf-8", errors="replace").strip()
        _fail(f"command exited with status {status}" + (f": {detail}" if detail else ""))
    return output, diagnostic


def _cargo_selection(compiler_input: dict[str, Any]) -> list[str]:
    result = ["--manifest-path", compiler_input["packageManifest"]]
    target = _object(compiler_input.get("cargoTarget"), "compilerInput.cargoTarget")
    if target.get("kind") != "lib":
        _fail("capability simulation currently admits only manifest lib targets")
    result.append("--lib")
    if compiler_input.get("defaultFeatures") is False:
        result.append("--no-default-features")
    features = _array(compiler_input.get("features"), "compilerInput.features")
    if features:
        result.extend(["--features", ",".join(features)])
    return result


def _command_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in (
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "FE2O3_AMDGCN_TARGET",
        "FE2O3_CODEGEN_PIPELINE",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V8",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
    ):
        environment.pop(name, None)
    environment.update({"FE2O3_HIP_SYS_DISABLE": "1", "FE2O3_HSA_RUNTIME_DISABLE": "1"})
    return environment


def execute_fixture(
    document: dict[str, Any],
    fixture_id: str,
    fixture: dict[str, Any],
    cpu_suite: dict[str, Any],
    temporary: Path,
    run: Callable[..., tuple[bytes, bytes]] = _run_bounded,
) -> tuple[bytes, bytes, bytes, bytes, bytes]:
    compiler_fixture = _index(document["compilerFixtures"], "fixtureId", "compilerFixtures")[fixture_id]
    compiler_input = _object(compiler_fixture["compilerInput"], "compilerInput")
    request = temporary / "request.json"
    request.write_bytes(_canonical(fixture["request"]) + b"\n")
    bundle = temporary / "production.bundle-v8"
    schedule = temporary / "schedule.json"
    target_dir = temporary / "cargo-target"
    cargo = os.environ.get("CARGO", "cargo")
    environment = _command_environment()
    run(
        [
            cargo,
            "build",
            "--locked",
            "--offline",
            "--quiet",
            "-p",
            "rustc-codegen-fe2o3",
            "--bin",
            "fe2o3-rustc-extract",
            "--bin",
            "fe2o3-export-sim",
            "--target-dir",
            str(target_dir),
        ],
        cwd=REPO_ROOT,
        timeout=TIMEOUT_SECONDS,
        environment=environment,
    )
    exporter = target_dir / "debug" / "fe2o3-export-sim"
    run(
        [
            str(exporter),
            "--crate",
            compiler_input["cargoTarget"]["name"],
            "--output",
            str(bundle),
            "--target",
            compiler_fixture["target"],
            "--bundle-version",
            "8",
            "--target-dir",
            str(target_dir / "export"),
            "--",
            *_cargo_selection(compiler_input),
        ],
        cwd=REPO_ROOT,
        timeout=TIMEOUT_SECONDS,
        environment=environment,
    )
    simulator_stdout, simulator_stderr = run(
        [
            cargo,
            "run",
            "--locked",
            "--offline",
            "--quiet",
            "-p",
            "fe2o3-kir-sim-cli",
            "--bin",
            "fe2o3-kir-sim",
            "--",
            "--bundle-v8",
            str(bundle),
            "--request",
            str(request),
            "--record-canonical-schedule",
            str(schedule),
            "--schedule-max-decisions",
            "1048576",
        ],
        cwd=REPO_ROOT,
        timeout=TIMEOUT_SECONDS,
        environment=environment,
    )
    command = _object(cpu_suite["command"], "CPU-reference command")
    oracle_environment = _command_environment()
    for assignment in command["environment"]:
        if not isinstance(assignment, str) or "=" not in assignment:
            _fail("CPU-reference environment contains an invalid assignment")
        name, value = assignment.split("=", 1)
        oracle_environment[name] = value
    oracle_stdout, oracle_stderr = run(
        [
            str(_repo_file(command["executable"], "CPU-reference executable", executable=True)),
            *command["arguments"],
        ],
        cwd=REPO_ROOT,
        timeout=command["timeoutSeconds"],
        environment=oracle_environment,
    )
    return (
        _read_regular(bundle, "production Bundle V8"),
        _read_regular(schedule, "canonical simulator schedule"),
        simulator_stdout,
        simulator_stderr,
        oracle_stdout + oracle_stderr,
    )


def _result_argument_payload(argument: Any, label: str) -> tuple[str, bytes]:
    argument = _object(argument, label)
    kind = argument.get("kind")
    if kind == "scalar":
        return "scalar", _hex_bytes(argument.get("bits"), f"{label}.bits")
    if kind == "buffer":
        value = _object(argument.get("value"), f"{label}.value")
        return "buffer", _hex_bytes(value.get("bytes"), f"{label}.value.bytes")
    _fail(f"{label} has an unsupported result kind")


def _expected_argument_payload(argument: Any, label: str) -> tuple[str, bytes]:
    argument = _object(argument, label)
    kind = argument.get("kind")
    if kind == "scalar":
        return "scalar", _hex_bytes(argument.get("bits"), f"{label}.bits")
    if kind == "buffer":
        return "buffer", _hex_bytes(argument.get("bytes"), f"{label}.bytes")
    _fail(f"{label} has an unsupported expected kind")


def _compare_payload(expected: bytes, observed: bytes, element: str | None, policy: dict[str, Any], label: str) -> None:
    if policy["mode"] == "exact-bits" or element != "f32":
        if observed != expected:
            _fail(f"{label} differs bit-for-bit from the CPU oracle")
        return
    if len(expected) != len(observed) or len(expected) % 4:
        _fail(f"{label} has an invalid f32 byte extent")
    import math
    import struct

    absolute = float(policy["absoluteTolerance"])
    relative = float(policy["relativeTolerance"])
    for offset, (wanted, actual) in enumerate(
        zip(struct.iter_unpack("<f", expected), struct.iter_unpack("<f", observed))
    ):
        wanted_value, actual_value = wanted[0], actual[0]
        if math.isnan(wanted_value) or math.isnan(actual_value):
            if not policy["allowNaN"] or not (math.isnan(wanted_value) and math.isnan(actual_value)):
                _fail(f"{label} differs at f32 element {offset}")
            continue
        tolerance = max(absolute, relative * max(abs(wanted_value), abs(actual_value)))
        if abs(wanted_value - actual_value) > tolerance:
            _fail(f"{label} differs at f32 element {offset}")


def validate_observation(
    document: dict[str, Any],
    fixture: dict[str, Any],
    fixture_bytes: bytes,
    cpu_suite: dict[str, Any],
    bundle_bytes: bytes,
    schedule_bytes: bytes,
    simulator_stdout: bytes,
    simulator_stderr: bytes,
    oracle_transcript: bytes,
) -> dict[str, Any]:
    fixture_id = fixture["fixtureId"]
    result = _object(_decode_unique(simulator_stdout, "simulator result"), "simulator result")
    required_result = {
        "schema": SIMULATOR_RESULT_SCHEMA,
        "status": "ok",
        "authority": "observation_only",
        "simulated": True,
        "hardware_observed": False,
        "hardware_validation": False,
        "performance_prediction": False,
    }
    for key, expected in required_result.items():
        if result.get(key) != expected:
            _fail(f"simulator result has invalid {key}")
    coverage = result.get("schedule", {}).get("coverage", {})
    if coverage.get("complete") is not True:
        _fail("simulator schedule coverage is incomplete")
    if result.get("conflict_assessment", {}).get("status") != "no_conflicts_observed":
        _fail("simulator observed or could not exclude a memory conflict")
    schedule = _object(_decode_unique(schedule_bytes, "canonical simulator schedule"), "canonical simulator schedule")
    if schedule.get("schema") != SCHEDULE_SCHEMA:
        _fail("simulator schedule has an unsupported schema")
    artifact = _object(schedule.get("artifact"), "simulator schedule.artifact")
    if artifact.get("kind") != "simulation_bundle_v8":
        _fail("simulator schedule did not consume Bundle V8")
    bundle_sha = _sha256(bundle_bytes)
    request_bytes = _canonical(fixture["request"]) + b"\n"
    if artifact.get("bundle_sha256") != bundle_sha:
        _fail("simulator schedule is stale against the exact Bundle V8")
    if artifact.get("kir_sha256") != result.get("kir", {}).get("sha256"):
        _fail("simulator result and schedule name different optimized KIR")
    export = fixture["productionExport"]
    if export["status"] == "available":
        coordinates = export["coordinates"]
        if (
            bundle_sha != coordinates["simulationBundleV8ContentSha256"]
            or artifact.get("kir_sha256") != coordinates["optimizedKirV13ContentSha256"]
            or artifact.get("subject_sha256") != coordinates["simulatorSubjectSha256"]
        ):
            _fail("simulator execution differs from exact production export coordinates")
    request_record = _object(schedule.get("request"), "simulator schedule.request")
    if request_record.get("sha256") != _sha256(request_bytes) or request_record.get("bytes") != len(request_bytes):
        _fail("simulator schedule is stale against the typed fixture request")
    target = _object(schedule.get("target"), "simulator schedule.target")
    if target.get("identity") != "amdgpu_64_little_endian_v1":
        _fail("simulator schedule has a cross-target execution model")
    expected = fixture["expectedArguments"]
    observed = _array(result.get("arguments"), "simulator result.arguments")
    if len(observed) != len(expected):
        _fail("simulator omitted or added an argument observation")
    policy = fixture["numericalPolicy"]
    request_arguments = fixture["request"]["arguments"]
    for index, (wanted, actual, role) in enumerate(zip(expected, observed, fixture["argumentRoles"])):
        wanted_kind, wanted_payload = _expected_argument_payload(wanted, f"expected argument {index}")
        actual_kind, actual_payload = _result_argument_payload(actual, f"result argument {index}")
        if wanted_kind != actual_kind:
            _fail(f"result argument {index} changed kind")
        element = wanted.get("element") if isinstance(wanted, dict) else None
        _compare_payload(wanted_payload, actual_payload, element, policy, f"result argument {index}")
        if role in {"input", "scalar"}:
            request_payload = _argument_bytes(request_arguments[index], f"request argument {index}")
            if actual_payload != request_payload:
                _fail(f"immutable input argument {index} changed")
    for label in ("canaries", "padding"):
        for check in fixture[label]:
            _, actual = _result_argument_payload(
                observed[check["argument"]], f"result argument {check['argument']}"
            )
            wanted = _hex_bytes(check["bytes"], f"{label} bytes")
            start = check["offset"]
            if actual[start : start + len(wanted)] != wanted:
                _fail(f"{label} check changed at argument {check['argument']} offset {start}")
    roster = manifest_simulation_roster(document)
    command = roster[fixture_id]["command"]
    source = {
        "compilerInputContractSha256": fixture["compilerInputContractSha256"],
        "sourceClosureSha256": fixture["sourceClosureSha256"],
    }
    launch = {field: fixture["request"][field] for field in ("grid", "kernel", "workgroup")}
    binding = {
        "bundleV8Sha256": bundle_sha,
        "commandSha256": command_sha256(command),
        "finalGraphEpoch": artifact.get("final_graph_epoch"),
        "fixtureInputSha256": _sha256(fixture_bytes),
        "fixtureId": fixture_id,
        "kernelSymbol": fixture["kernelSymbol"],
        "launchSha256": _domain_sha256(LAUNCH_DOMAIN, launch),
        "manifestSha256": _sha256(_canonical(document) + b"\n"),
        "numericalPolicySha256": _domain_sha256(POLICY_DOMAIN, policy),
        "optimizedKirV13Sha256": artifact.get("kir_sha256"),
        "oracleCommandSha256": command_sha256(cpu_suite["command"]),
        "oracleSuiteId": cpu_suite["suiteId"],
        "requestSha256": _sha256(request_bytes),
        "source": source,
        "subjectSha256": artifact.get("subject_sha256"),
        "target": fixture["target"],
    }
    for field in ("optimizedKirV13Sha256", "subjectSha256"):
        if not isinstance(binding[field], str) or SHA256.fullmatch(binding[field]) is None:
            _fail(f"simulator schedule omitted {field}")
    epoch = binding["finalGraphEpoch"]
    if not isinstance(epoch, int) or isinstance(epoch, bool) or epoch <= 0:
        _fail("simulator schedule omitted a valid final graph epoch")
    observation = {
        "authority": "observation_only",
        "bindings": binding,
        "checks": {
            "canariesChecked": True,
            "completeOutputsChecked": True,
            "inputsUnchangedChecked": True,
            "paddingChecked": True,
        },
        "fixtureId": fixture_id,
        "observations": {
            "oracleTranscriptSha256": _sha256(oracle_transcript),
            "scheduleSha256": _sha256(schedule_bytes),
            "simulatorStderrSha256": _sha256(simulator_stderr),
            "simulatorStdoutSha256": _sha256(simulator_stdout),
        },
        "schema": SUITE_RESULT_SCHEMA,
        "status": "passed",
    }
    observation["observationSha256"] = _domain_sha256(OBSERVATION_DOMAIN, observation)
    return observation


def main(arguments: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True)
    options = parser.parse_args(arguments)
    try:
        manifest_bytes, raw_document = _load_json(MANIFEST, "tutorial kernel manifest")
        document = _object(raw_document, "tutorial kernel manifest")
        fixture, fixture_bytes, cpu_suite = load_fixture(document, options.fixture)
        with tempfile.TemporaryDirectory(prefix="fe2o3-tutorial-sim-") as temporary:
            outputs = execute_fixture(
                document,
                options.fixture,
                fixture,
                cpu_suite,
                Path(temporary),
            )
        evidence = validate_observation(
            document,
            fixture,
            fixture_bytes,
            cpu_suite,
            *outputs,
        )
        if evidence["bindings"]["manifestSha256"] != _sha256(manifest_bytes):
            _fail("manifest changed or is not canonical during simulation")
    except (OSError, SimulationQualificationError) as error:
        print(f"tutorial semantic simulation: {error}", file=sys.stderr)
        return 1
    sys.stdout.buffer.write(_canonical(evidence) + b"\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
