"""Bounded diagnostic comparison against already validated fixture display bindings.

No compiler invocation, Rust parsing, execution authentication or qualification.
Paths in the census are labels; only existing package inputs supply source bytes.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
from typing import Any, Callable


MAX_DOCUMENT_BYTES = 4 * 1024 * 1024
MAX_ARGUMENT_BYTES = 1024 * 1024
MAX_ARGUMENTS = 4096
MAX_TEXT_BYTES = 4096
MAX_FILES = 128
MAX_FUNCTIONS = 512
MAX_FILE_BYTES = 4 * 1024 * 1024
MAX_SOURCE_BYTES = 16 * 1024 * 1024
INVOCATION_KEYS = {"runId", "arguments", "workingDirectory", "extractionMode"}
TARGETS = {"gfx942": "gfx942:xnack-", "gfx950": "gfx950:xnack-"}


class SourceCensusError(ValueError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise SourceCensusError(message)


def _object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    _require(type(value) is dict and value.keys() == keys, f"{label}: exact object fields required")
    return value


def _text(value: Any, label: str, maximum: int = MAX_TEXT_BYTES, *, empty: bool = False) -> str:
    _require(type(value) is str, f"{label}: string required")
    try:
        size = len(value.encode("utf-8"))
    except UnicodeError as error:
        raise SourceCensusError(f"{label}: invalid UTF-8") from error
    _require((empty or size > 0) and size <= maximum and "\0" not in value,
             f"{label}: text bound or NUL violation")
    return value


def _integer(value: Any, maximum: int, label: str) -> int:
    _require(type(value) is int and 0 <= value <= maximum, f"{label}: integer bound violation")
    return value


def _digest(value: Any, label: str) -> str:
    _require(re.fullmatch(r"[0-9a-f]{64}", _text(value, label)) is not None,
             f"{label}: lowercase SHA-256 required")
    return value


def _array(value: Any, maximum: int, label: str) -> list:
    _require(type(value) is list and len(value) <= maximum, f"{label}: array bound violation")
    return value


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result = {}
    for key, value in pairs:
        _require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _read(path: Path, maximum: int) -> bytes:
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
        with os.fdopen(descriptor, "rb") as stream:
            metadata = os.fstat(stream.fileno())
            _require(stat.S_ISREG(metadata.st_mode) and metadata.st_size <= maximum,
                     "input must be a bounded regular file")
            payload = stream.read(maximum + 1)
            _require(len(payload) == metadata.st_size and len(payload) <= maximum,
                     "input changed or exceeded its byte bound")
            return payload
    except OSError as error:
        raise SourceCensusError(f"cannot read input: {error}") from error


def load_document(path: Path) -> Any:
    def invalid_constant(value: str) -> None:
        raise SourceCensusError(f"non-finite JSON constant: {value}")

    try:
        return json.loads(_read(path, MAX_DOCUMENT_BYTES).decode("utf-8"),
                          object_pairs_hook=_pairs, parse_constant=invalid_constant)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise SourceCensusError(f"invalid census JSON: {error}") from error


def _invocation(value: dict[str, Any]) -> None:
    _digest(value["runId"], "runId")
    arguments = _array(value["arguments"], MAX_ARGUMENTS, "arguments")
    _require(bool(arguments), "arguments must include the driver")
    total = sum(len(_text(arg, "argument", MAX_ARGUMENT_BYTES, empty=True).encode("utf-8"))
                for arg in arguments)
    _require(total <= MAX_ARGUMENT_BYTES, "argument byte bound exceeded")
    cwd = _text(value["workingDirectory"], "workingDirectory")
    _require(Path(cwd).is_absolute(), "workingDirectory must be absolute")
    mode = value["extractionMode"]
    _require(type(mode) is dict, "extractionMode must be an object")
    kind = _text(mode.get("kind"), "extractionMode.kind")
    fields = {
        "semantic-mir": {"kind"}, "ranked-memory": {"kind"},
        "llvm": {"kind", "expected_target"},
        "compiler-handoff": {"kind", "version", "expected_target"},
        "simulation-bundle": {"kind", "version"},
    }
    _require(kind in fields, "unsupported extractionMode.kind")
    _object(mode, fields[kind], "extractionMode")
    if "version" in mode:
        version = _integer(mode["version"], 65535, "extractionMode.version")
        _require(version in ({1, 3} if kind == "compiler-handoff" else set(range(1, 7))),
                 "unsupported extractionMode.version")
    if mode.get("expected_target") is not None:
        _require(_text(mode["expected_target"], "expected_target") in TARGETS.values(),
                 "unsupported extractionMode.expected_target")


def _observation(value: Any, label: str) -> Any:
    _object(value, {"status", "value"}, label)
    _require(value["status"] in ("available", "unavailable"), f"{label}: invalid status")
    if value["status"] == "unavailable":
        _text(value["value"], f"{label} unavailable reason")
        return None
    _require(type(value["value"]) is dict, f"{label}: available value must be an object")
    return value["value"]


def _path(cwd: str, path: str) -> str:
    return os.path.abspath(os.path.join(cwd, path))


def _span(span: Any, files: list, source_bytes: dict[str, bytes], cwd: str,
          endpoints: dict[str, dict[int, int]]) -> None:
    _object(span, {"expansion", "callSite", "expansionChainSha256", "expansionDepth"}, "span")
    _digest(span["expansionChainSha256"], "expansionChainSha256")
    _integer(span["expansionDepth"], 64, "expansionDepth")
    for anchor in ("expansion", "callSite"):
        origin = _object(span[anchor], {"file", "coordinates"}, anchor)
        index = _integer(origin["file"], len(files) - 1, "span file")
        file = files[index]
        coordinates = _object(origin["coordinates"], {
            "normalized_start", "normalized_end", "original_start", "original_end",
        }, "coordinates")
        for unit in ("normalized", "original"):
            start = _integer(coordinates[f"{unit}_start"], file[f"{unit}Bytes"], f"{unit} start")
            end = _integer(coordinates[f"{unit}_end"], file[f"{unit}Bytes"], f"{unit} end")
            _require(start <= end, "reversed source range")
        path = _path(cwd, file["displayPath"])
        if path in source_bytes:
            observed = endpoints.setdefault(path, {})
            for endpoint in ("start", "end"):
                position = coordinates[f"original_{endpoint}"]
                normalized = coordinates[f"normalized_{endpoint}"]
                _require(observed.setdefault(position, normalized) == normalized,
                         "inconsistent normalized coordinate for original endpoint")


def _check_original_coordinates(source: bytes, endpoints: dict[int, int]) -> None:
    # Full UTF-8 and file lengths were validated before collecting endpoints.
    # Sorted, disjoint slices scan each source byte at most once, even when many
    # functions repeat anchors. Normalized offsets never select original tokens.
    previous = 3 if source.startswith(b"\xef\xbb\xbf") else 0
    normalized = 0
    for position in sorted(endpoints):
        _require(position >= previous, "source endpoint lies inside normalization")
        _require(position == len(source) or source[position] & 0xC0 != 0x80,
                 "source endpoint is not a UTF-8 boundary")
        _require(not (0 < position < len(source)
                      and source[position - 1] == 13 and source[position] == 10),
                 "source endpoint lies inside normalization")
        normalized += position - previous - source[previous:position].count(b"\r\n")
        _require(normalized == endpoints[position], "normalized/original coordinates differ")
        previous = position


def _selection(value: Any, sources: dict[str, bytes], cwd: str, target: str) -> Any:
    selection = _observation(value, "selection")
    if selection is None:
        return None
    _object(selection, {"target", "files", "functions"}, "selection")
    _require(selection["target"] == target, "selection target differs from fixture")
    files = _array(selection["files"], MAX_FILES, "files")
    seen_ids, seen_paths = set(), set()
    total = 0
    for file in files:
        _object(file, {"identity", "displayPath", "compiledSourceHash", "originalSha256",
                       "originalBytes", "normalizedBytes"}, "file")
        identity = _digest(file["identity"], "file identity")
        path = _path(cwd, _text(file["displayPath"], "displayPath"))
        _require(identity not in seen_ids and path not in seen_paths, "duplicate census file")
        seen_ids.add(identity)
        seen_paths.add(path)
        _text(file["compiledSourceHash"], "compiledSourceHash")
        _digest(file["originalSha256"], "originalSha256")
        size = _integer(file["originalBytes"], MAX_FILE_BYTES, "originalBytes")
        _integer(file["normalizedBytes"], size, "normalizedBytes")
        total += size + 1
        source = sources.get(path)
        if source is not None:
            normalized = source.decode("utf-8").removeprefix("\ufeff").replace("\r\n", "\n")
            _require(size == len(source) and file["originalSha256"] == hashlib.sha256(source).hexdigest()
                     and file["normalizedBytes"] == len(normalized.encode("utf-8")),
                     "census source hash/length differs from current fixture member")
    _require(total <= MAX_SOURCE_BYTES, "aggregate source byte bound exceeded")
    functions = _array(selection["functions"], MAX_FUNCTIONS, "functions")
    identities = set()
    endpoints: dict[str, dict[int, int]] = {}
    for function in functions:
        _object(function, {"functionIdentity", "definitionIdentity", "monomorphizationIdentity",
                           "role", "exportName", "logicalName", "definition", "identifier"}, "function")
        for field in ("functionIdentity", "definitionIdentity", "monomorphizationIdentity"):
            _digest(function[field], field)
        _require(function["functionIdentity"] not in identities, "duplicate function identity")
        identities.add(function["functionIdentity"])
        _require(function["role"] in ("kernel-entry", "internal-helper", "device-ffi-export"),
                 "invalid function role")
        _text(function["exportName"], "exportName")
        if function["logicalName"] is not None:
            _text(function["logicalName"], "logicalName")
        for field in ("definition", "identifier"):
            span = _observation(function[field], field)
            if span is not None:
                _span(span, files, sources, cwd, endpoints)
    for path, observed in endpoints.items():
        _check_original_coordinates(sources[path], observed)
    return selection


def _feature_cfgs(arguments: list[str]) -> list[str]:
    _require(all(not argument.startswith("@") for argument in arguments[1:]),
             "response-file arguments are unsupported")
    features = []
    iterator = iter(arguments[1:])
    for argument in iterator:
        if argument == "--cfg":
            cfg = next(iterator, None)
            _require(cfg is not None, "missing --cfg value")
        elif argument.startswith("--cfg="):
            cfg = argument[len("--cfg="):]
        else:
            continue
        if re.match(r"\s*feature\b", cfg):
            match = re.fullmatch(r'\s*feature\s*=\s*"([^"\\]+)"\s*', cfg)
            _require(match is not None, "unsupported feature cfg")
            features.append(match[1])
    _require(len(features) == len(set(features)), "duplicate feature cfg")
    return sorted(features)


def compare(
    root: Path, manifest: dict, fixtures: dict, inventory: dict | None,
    census_path: Path, request_path: Path, *,
    validate_compiler_input: Callable, cargo_feature_closure: Callable,
) -> dict:
    """Inputs must come from the existing validated manifest and pair projection."""
    request = _object(load_document(request_path), INVOCATION_KEYS | {
        "schema", "fixtureId", "contractSha256", "cargoIntent",
    }, "request")
    _require(request["schema"] == "fe2o3-tutorial-source-census-request-v1", "unsupported request schema")
    _invocation(request)
    fixture_id = _text(request["fixtureId"], "fixtureId")
    _require(fixture_id in fixtures, "unknown fixtureId")
    fixture = fixtures[fixture_id]
    inputs = fixture["compilerInput"]
    _require(_digest(request["contractSha256"], "contractSha256") == inputs["contractSha256"],
             "request contractSha256 differs from fixture")
    cache: dict = {}
    checked = validate_compiler_input(root, fixture, "source census fixture", cache)
    cargo = cache[inputs["packageManifest"]]["cargo"]
    intent = _object(request["cargoIntent"], {
        "packageManifest", "packageName", "cargoTarget", "features", "defaultFeatures",
    }, "caller cargoIntent")
    _text(intent["packageManifest"], "packageManifest")
    _text(intent["packageName"], "packageName")
    target = _object(intent["cargoTarget"], {"kind", "name", "sourcePath"}, "cargoTarget")
    for field in target:
        _text(target[field], f"cargoTarget.{field}")
    features = _array(intent["features"], MAX_ARGUMENTS, "features")
    for feature in features:
        _text(feature, "feature")
    _require(type(intent["defaultFeatures"]) is bool, "defaultFeatures must be boolean")
    _require(intent["packageName"] == checked["packageName"]
             and all(intent[key] == inputs[key] for key in intent if key != "packageName"),
             "caller Cargo intent differs from fixture")
    enabled = cargo_feature_closure(cargo, features, intent["defaultFeatures"], "source census intent")
    _require(_feature_cfgs(request["arguments"]) == enabled, "driver feature cfgs differ from Cargo feature closure")
    expected_target = TARGETS[fixture["target"]]
    mode_target = request["extractionMode"].get("expected_target")
    _require(mode_target is None or mode_target == expected_target, "mode expected_target differs from fixture")

    _require(inventory is not None, "existing fixture-source-contract inventory required")
    kernel_ids = {row["kernelId"] for row in inventory["kernelIdentities"]
                  if any(ref["kind"] == "fixture" and ref["fixtureId"] == fixture_id for ref in row["selections"])}
    tabs = {(lesson["lessonId"], tab["ordinal"]): tab
            for lesson in manifest["curriculum"]["lessons"] for tab in lesson["codeTabs"]}
    expected = []
    sources = {}
    for row in inventory["displayItems"]:
        if row["bindingStatus"] != "fixture-source-contract" or not kernel_ids.intersection(row["kernelIds"]):
            continue
        tab = tabs[row["lessonId"], row["tabOrdinal"]]
        path = root / tab["sourcePath"]
        if str(path) not in sources:
            sources[str(path)] = _read(path, MAX_FILE_BYTES)
            _require(len(sources) <= MAX_FILES and sum(map(len, sources.values())) <= MAX_SOURCE_BYTES,
                     "expected source byte/file bound exceeded")
        source = sources[str(path)]
        _require(len(source) == tab["displayedUtf8Bytes"]
                 and hashlib.sha256(source).hexdigest() == tab["sourceSha256"],
                 "physical display source changed after validation")
        offset = row["functionUtf8Offset"]
        token = ("r#" if source[offset:offset + 2] == b"r#" else "") + row["kernelSymbol"]
        end = offset + len(token.encode("utf-8"))
        _require(source[offset:end] == token.encode("utf-8"), "validated identifier token changed")
        expected.append({
            **{key: row[key] for key in ("lessonId", "tabOrdinal", "kernelSymbol", "functionUtf8Offset")},
            "sourcePath": tab["sourcePath"], "originalSha256": tab["sourceSha256"],
            "originalBytes": len(source), "originalStart": offset, "originalEnd": end,
            "identifierToken": token,
        })
    _require(bool(expected), "fixture has no existing fixture-source-contract display bindings")

    census = _object(load_document(census_path), INVOCATION_KEYS | {
        "schema", "diagnosticOnly", "qualified", "authenticatesCompilerExecution",
        "extractionSucceeded", "selection",
    }, "census")
    _require(census["schema"] == "fe2o3-diagnostic-source-census-v1", "unsupported census schema")
    for field, value in (("diagnosticOnly", True), ("qualified", False), ("authenticatesCompilerExecution", False)):
        _require(census[field] is value, f"{field}: invalid diagnostic marker")
    _require(type(census["extractionSucceeded"]) is bool, "extractionSucceeded must be boolean")
    _invocation(census)
    for field in sorted(INVOCATION_KEYS):
        _require(census[field] == request[field], f"census {field} differs from independent request")
    selection = _selection(census["selection"], sources, census["workingDirectory"], expected_target)
    entries = [] if selection is None else [row for row in selection["functions"] if row["role"] == "kernel-entry"]
    if selection is not None:
        _require(len(entries) == len(inputs["kernelSymbols"]), "kernel-entry roster differs from fixture")
    matched = set()
    comparisons = []
    for occurrence in expected:
        candidates = []
        for entry in entries:
            span = _observation(entry["identifier"], "identifier")
            if span is None:
                continue
            origin = span["expansion"]
            file = selection["files"][origin["file"]]
            coordinates = origin["coordinates"]
            if (_path(census["workingDirectory"], file["displayPath"]) == str(root / occurrence["sourcePath"])
                    and coordinates["original_start"] == occurrence["originalStart"]
                    and coordinates["original_end"] == occurrence["originalEnd"]):
                candidates.append(entry)
        _require(len(candidates) <= 1, "ambiguous kernel-entry identifier")
        candidate = candidates[0] if candidates else None
        if candidate is not None:
            matched.add(candidate["functionIdentity"])
        comparisons.append({
            "expected": occurrence, "status": "matched" if candidate else "unresolved",
            "functionIdentity": candidate["functionIdentity"] if candidate else None,
            "reason": None if candidate else "No exact available kernel-entry identifier; generated identifiers are not substituted.",
        })
    for entry in entries:
        if entry["identifier"]["status"] == "available":
            _require(entry["functionIdentity"] in matched, "available kernel-entry identifier differs from existing display binding")
    return {
        "schema": "fe2o3-tutorial-source-census-comparison-v1",
        "diagnosticOnly": True, "qualified": False, "authenticatesCompilerExecution": False,
        "fixtureId": fixture_id, "contractSha256": request["contractSha256"],
        "callerCargoIntent": intent, "enabledFeatures": enabled,
        **{key: census[key] for key in sorted(INVOCATION_KEYS)},
        "extractionSucceeded": census["extractionSucceeded"],
        "extractionStatus": "succeeded" if census["extractionSucceeded"] else "failed",
        "selection": census["selection"], "comparisons": comparisons,
    }
