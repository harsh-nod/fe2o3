"""Bounded identity reconciliation for an already validated tutorial manifest.

This module owns no source inventory, execution evidence, or file I/O. The caller
validates the existing manifest/runtime projection first and supplies its bounded
Rust scanner and lexer. Restricted fixture selection uses those same coordinates.
References retain those input contracts; a lexical occurrence is never compiler
or qualification evidence.
"""

from __future__ import annotations

import hashlib
import json
import re
from typing import Any, Callable


SCHEMA = "fe2o3-tutorial-kernel-identities-v1"
MAX_TEXT_BYTES = 4 * 1024 * 1024
MAX_RUNTIME_BYTES = 16 * 1024 * 1024
MAX_IDENTITY_BYTES = 16 * 1024 * 1024
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
KERNEL_ID = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}")
ISSUE_URL = re.compile(r"https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/issues/[1-9][0-9]*")
INPUT_FIELDS = {
    "packageManifest", "packageManifestSha256", "cargoLockPath", "cargoLockSha256",
    "sourcePaths", "sourceClosureSha256", "cargoTarget", "defaultFeatures", "features",
}
DISPLAY_FIELDS = {
    "lessonId", "tabOrdinal", "functionUtf8Offset", "kernelSymbol", "classification",
    "kernelIds", "negativeCases", "bindingStatus", "reason",
}


class KernelInventoryError(ValueError):
    """An identity contract is malformed, stale, or incompletely partitioned."""


def _fail(message: str) -> None:
    raise KernelInventoryError(f"kernel inventory: {message}")


def _object(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        _fail(f"{label} has unexpected or missing fields")
    return value


def _text(value: Any, label: str, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        _fail(f"{label} must be bounded nonblank text")
    return value


def _integer(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        _fail(f"{label} must be a nonnegative integer")
    return value


def _symbol(value: Any) -> str:
    if not isinstance(value, str) or len(value) > 256 or IDENTIFIER.fullmatch(value) is None:
        _fail("kernelSymbol must be an ordinary Rust identifier")
    return value


def _display_symbol(value: Any) -> str:
    if not isinstance(value, str) or len(value) > 256 or not value.isidentifier():
        _fail("display kernelSymbol must be a Rust name without its raw prefix")
    return value


def _name_token_matches(encoded: bytes, offset: int, symbol: str) -> bool:
    token = symbol.encode("utf-8")
    return encoded[offset:offset + len(token)] == token or encoded[offset:offset + len(token) + 2] == b"r#" + token


class _Budget:
    def __init__(self, maximum: int):
        if type(maximum) is not int or maximum < 0:
            _fail("record limit must be a nonnegative integer")
        self.maximum = maximum
        self.used = 0
        self.identity_bytes = 0
        self.source_bytes = 0

    def rows(self, value: Any, label: str) -> list[Any]:
        if not isinstance(value, list) or len(value) > self.maximum - self.used:
            _fail(f"{label} exceeds the record bound or is not an array")
        self.used += len(value)
        return value


def _reference(value: Any) -> tuple[tuple[Any, ...], dict[str, Any]]:
    if not isinstance(value, dict):
        _fail("selection reference must be an object")
    kind = value.get("kind")
    if kind == "fixture":
        _object(value, {"kind", "fixtureId", "kernelSymbol"}, "fixture reference")
        fixture = _text(value["fixtureId"], "fixtureId")
        symbol = _symbol(value["kernelSymbol"])
        return (kind, fixture, symbol), {"kind": kind, "fixtureId": fixture, "kernelSymbol": symbol}
    if kind == "source-driver-case":
        _object(value, {"kind", "lessonId", "tabOrdinal", "caseOrdinal"}, "source case reference")
        lesson = _text(value["lessonId"], "lessonId")
        tab = _integer(value["tabOrdinal"], "tabOrdinal")
        case = _integer(value["caseOrdinal"], "caseOrdinal")
        return (kind, lesson, tab, case), {
            "kind": kind, "lessonId": lesson, "tabOrdinal": tab, "caseOrdinal": case,
        }
    _fail("unknown selection reference kind")


def _selection_identity(inputs: dict[str, Any], symbol: str, budget: _Budget) -> str:
    # Only the GPU target and reference location may differ in one identity.
    # Source revisions, closure/lock hashes, feature selections and Cargo target
    # remain significant even when two symbols or rendered bodies are equal.
    try:
        payload = {key: inputs[key] for key in INPUT_FIELDS}
        payload["kernelSymbol"] = symbol
        digest = hashlib.sha256(b"fe2o3-tutorial-kernel-selection-v1\0")
        encoder = json.JSONEncoder(sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)
        for chunk in encoder.iterencode(payload):
            budget.identity_bytes += len(chunk)
            if budget.identity_bytes > MAX_IDENTITY_BYTES:
                _fail("selection identity bytes exceed their aggregate bound")
            digest.update(chunk.encode("ascii"))
        return digest.hexdigest()
    except KernelInventoryError:
        raise
    except (KeyError, TypeError, ValueError):
        _fail("selection lacks a valid existing compiler input contract")


def _utf8(value: Any, label: str) -> bytes:
    if not isinstance(value, str) or len(value) > MAX_TEXT_BYTES:
        _fail(f"{label} is missing or exceeds its byte bound")
    try:
        encoded = value.encode("utf-8")
    except UnicodeError:
        _fail(f"{label} is not valid UTF-8")
    if len(encoded) > MAX_TEXT_BYTES:
        _fail(f"{label} exceeds its byte bound")
    return encoded


def _fragment_intervals(tab: dict[str, Any], runtime: dict[str, Any], encoded: bytes) -> list[tuple[int, int]]:
    if tab["sourceDigestScope"] == "file":
        if runtime.get("sourceFragments") is not None or tab["sourceFragmentsSha256"] is not None:
            _fail("whole-file source case has fragment metadata")
        if tab["sourceSha256"] != hashlib.sha256(encoded).hexdigest():
            _fail("whole-file source case digest differs")
        return [(0, len(encoded))]
    fragments = runtime.get("sourceFragments")
    digests = tab["sourceFragmentsSha256"]
    if not isinstance(fragments, list) or not isinstance(digests, list) or not 0 < len(fragments) == len(digests) <= 64:
        _fail("source case fragment coverage differs")
    parts = [_utf8(part, "source fragment") for part in fragments]
    if sum(map(len, parts)) + 2 * (len(parts) - 1) > MAX_TEXT_BYTES:
        _fail("source case fragment bytes exceed their bound")
    if b"\n\n".join(parts) != encoded:
        _fail("source case fragments do not reconstruct the displayed bytes")
    intervals = []
    offset = 0
    for part, digest in zip(parts, digests, strict=True):
        if hashlib.sha256(part).hexdigest() != digest:
            _fail("source case fragment digest differs")
        intervals.append((offset, offset + len(part)))
        offset += len(part) + 2
    return intervals


def _fixture_cfg(body: str, features: set[str]) -> bool:
    """Evaluate only literal feature/AMDGPU cfg expressions, without expansion."""
    if len(body) > 8192 or len(body.encode("utf-8")) > 8192:
        _fail("fixture cfg exceeds its byte bound")
    tokens = re.findall(r'\s+|[A-Za-z_][A-Za-z0-9_]*|"[A-Za-z0-9_-]+"|[(),=]', body)
    if "".join(tokens) != body:
        _fail("unsupported fixture cfg syntax")
    tokens = [token for token in tokens if not token.isspace()]
    if len(tokens) > 256:
        _fail("fixture cfg exceeds its token bound")
    cursor = 0

    def take() -> str:
        nonlocal cursor
        if cursor == len(tokens):
            _fail("incomplete fixture cfg")
        token = tokens[cursor]
        cursor += 1
        return token

    def expression(depth: int) -> bool:
        if depth > 32:
            _fail("fixture cfg exceeds its nesting bound")
        name = take()
        if name in {"all", "any", "not"}:
            if take() != "(":
                _fail("unsupported fixture cfg syntax")
            values = []
            while cursor < len(tokens) and tokens[cursor] != ")":
                values.append(expression(depth + 1))
                if cursor < len(tokens) and tokens[cursor] == ",":
                    take()
                elif cursor == len(tokens) or tokens[cursor] != ")":
                    _fail("unsupported fixture cfg syntax")
            if take() != ")" or (name == "not" and len(values) != 1):
                _fail("unsupported fixture cfg arity")
            return all(values) if name == "all" else any(values) if name == "any" else not values[0]
        if name not in {"feature", "target_arch"} or take() != "=":
            _fail("unsupported fixture cfg predicate")
        value = take()
        if not value.startswith('"'):
            _fail("fixture cfg requires a literal value")
        value = value[1:-1]
        return value in features if name == "feature" else value == "amdgpu"

    result = expression(0)
    if cursor != len(tokens):
        _fail("trailing fixture cfg syntax")
    return result


def _fixture_declarations(
    source: str, features: set[str], scan_functions: Callable,
    rust_syntax: Callable, budget: _Budget,
) -> tuple[list[dict[str, Any]], list[str]]:
    """Select top-level functions and ordinary external modules only."""
    _utf8(source, "fixture source")
    functions = budget.rows(scan_functions(source), "fixture function items")
    code, pairs = rust_syntax(source)
    cursor = 0
    function_index = previous_character = previous_byte = 0
    selected = []
    modules = []
    attribute = re.compile(r"#\s*(!?)\s*\[")
    while cursor < len(code):
        if code[cursor].isspace() or code[cursor] == ";":
            cursor += 1
            continue
        budget.rows([None], "fixture source items")
        enabled = True
        while match := attribute.match(code, cursor):
            opening = match.end() - 1
            end = pairs[opening]
            body = source[opening + 1:end - 1].strip()
            masked = code[opening + 1:end - 1].strip()
            cfg = re.fullmatch(r"cfg\s*\((.*)\)", body, re.DOTALL)
            if cfg is not None:
                enabled = _fixture_cfg(cfg[1], features) and enabled
            elif re.fullmatch(r'cfg_attr\s*\(\s*target_arch\s*=\s*"amdgpu"\s*,\s*no_std\s*\)', body) and match[1]:
                pass
            else:
                benign = re.fullmatch(r"(allow|deny|forbid|warn|doc|inline|kernel)(?:\s*(\(.*\)))?", masked, re.DOTALL)
                if benign is None or (benign[2] is None and benign[1] not in {"inline", "kernel"}):
                    _fail("unsupported fixture selection attribute")
                if benign[2] is not None:
                    arguments = code.find("(", opening + 1, end - 1)
                    if code[pairs[arguments]:end - 1].strip():
                        _fail("unsupported fixture selection attribute")
                    if benign[1] == "inline" and re.fullmatch(r"inline\s*\(\s*(?:always|never)\s*\)", masked) is None:
                        _fail("unsupported fixture selection attribute")
            if match[1] and not enabled:
                _fail("conditional fixture crate/module is unsupported")
            cursor = end
            while cursor < len(code) and code[cursor].isspace():
                cursor += 1
        if cursor == len(code):
            break
        start = cursor
        while cursor < len(code) and code[cursor] not in "{;":
            if code[cursor] in "([":
                cursor = pairs[cursor]
            else:
                cursor += 1
        if cursor == len(code):
            _fail("unsupported fixture item boundary")
        head = code[start:cursor]
        boundary = cursor
        cursor = pairs[cursor] if code[cursor] == "{" else cursor + 1
        first_byte = previous_byte + len(source[previous_character:start].encode("utf-8"))
        last_byte = first_byte + len(source[start:boundary].encode("utf-8"))
        end_byte = last_byte + len(source[boundary:cursor].encode("utf-8"))
        previous_character, previous_byte = cursor, end_byte
        item_functions = []
        while function_index < len(functions) and functions[function_index]["functionUtf8Offset"] < end_byte:
            function = functions[function_index]
            function_index += 1
            if enabled and function["attributedKernel"]:
                if not first_byte <= function["functionUtf8Offset"] < last_byte:
                    _fail("nested fixture kernel declaration is unsupported")
                item_functions.append(function)
        if not enabled:
            continue
        module = re.fullmatch(r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*", head)
        if module is not None:
            if code[boundary] != ";":
                _fail("inline fixture modules are unsupported")
            modules.append(module[1])
            continue
        if (re.search(r"\b(?:macro_rules|include)\b|!", head)
                or re.match(r"\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:use|const|static|type|fn)\b", head) is None):
            _fail("unsupported fixture item or module selection")
        selected.extend(item_functions)
    return selected, modules


def _fixture_selection(fixture: dict[str, Any], load_sources: Callable, scan_functions: Callable,
                       rust_syntax: Callable, budget: _Budget) -> dict[str, tuple[str, int, str]]:
    library, sources, enabled = load_sources(fixture)
    # The loader authenticates the current physical package closure and Cargo
    # inputs. Traversal establishes only this bounded source selection, not rustc
    # acceptance or any executable outcome.
    pending = [library]
    visited = set()
    selected = {}
    while pending:
        path = pending.pop()
        if path in visited or path not in sources:
            _fail("ambiguous or missing fixture module selection")
        visited.add(path)
        source = sources[path]
        budget.source_bytes += len(_utf8(source, "fixture source"))
        if budget.source_bytes > MAX_RUNTIME_BYTES:
            _fail("selected fixture source exceeds its aggregate byte bound")
        functions, modules = _fixture_declarations(source, set(enabled), scan_functions, rust_syntax, budget)
        for function in functions:
            symbol = function["kernelSymbol"]
            if symbol in selected:
                _fail("ambiguous feature-selected fixture kernel")
            selected[symbol] = (path, function["functionUtf8Offset"], source)
        # Deliberately bounded to ordinary sibling modules of a library root.
        # Nested, inline, #[path] and macro-selected modules need a separate
        # reviewed extension, not a guess about Rust's module resolution.
        if modules and path != library:
            _fail("nested fixture modules are unsupported")
        for module in modules:
            parent = path.rsplit("/", 1)[0]
            candidates = [candidate for candidate in (f"{parent}/{module}.rs", f"{parent}/{module}/mod.rs")
                          if candidate in sources]
            if len(candidates) != 1:
                _fail("ambiguous or missing fixture module selection")
            pending.append(candidates[0])
    if set(selected) != set(fixture["compilerInput"]["kernelSymbols"]):
        _fail("feature-selected fixture kernel roster differs")
    return selected


def validate_kernel_inventory(
    manifest: dict[str, Any], runtime_inventory: dict[str, Any] | None,
    scan_functions: Callable[[str], list[dict[str, Any]]], *, max_records: int = 4096,
    load_fixture_sources: Callable | None = None, rust_syntax: Callable | None = None,
) -> dict[str, Any]:
    """Validate the optional sibling and return only bounded validated fields.

    The input manifest and optional runtime projection must already pass their
    existing validators. With no runtime, selection coverage is checked but
    occurrence coverage and source/display joins remain explicitly unvalidated.
    """
    budget = _Budget(max_records)
    if (not isinstance(manifest, dict) or not isinstance(manifest.get("curriculum"), dict)
            or manifest["curriculum"].get("schema") != "fe2o3-tutorial-curriculum-obligations-v2"):
        _fail("the identity extension requires the existing V2 curriculum")
    inventory = _object(manifest.get("kernelInventory"),
                        {"schema", "kernels", "negativeCases", "displayItems"}, "root")
    if inventory["schema"] != SCHEMA:
        _fail("unsupported schema")
    tabs: dict[tuple[str, int], tuple[dict[str, Any], str]] = {}
    selections: dict[tuple[Any, ...], dict[str, Any]] = {}
    expected_negative = set()

    def add_selection(key: tuple[Any, ...], value: dict[str, Any]) -> None:
        if key in selections:
            _fail("duplicate existing source selection")
        selections[key] = value

    for fixture in budget.rows(manifest["compilerFixtures"], "fixtures"):
        inputs = fixture["compilerInput"]
        for symbol in budget.rows(inputs["kernelSymbols"], "fixture symbols"):
            symbol = _symbol(symbol)
            key = ("fixture", fixture["fixtureId"], symbol)
            add_selection(key, {"identity": _selection_identity(inputs, symbol, budget), "symbol": symbol,
                                "fixture": fixture})
    for lesson in budget.rows(manifest["curriculum"]["lessons"], "lessons"):
        lesson_id = lesson["lessonId"]
        for ordinal, tab in enumerate(budget.rows(lesson["codeTabs"], "tabs")):
            tabs[(lesson_id, ordinal)] = (tab, lesson["role"])
            item = tab["sourceItem"]
            if item is None:
                continue
            for index, case in enumerate(budget.rows(item["cases"], "source cases")):
                key = ("source-driver-case", lesson_id, ordinal, index)
                inputs = {**item["compilerInput"], "features": case["features"]}
                add_selection(key, {"identity": _selection_identity(inputs, case["kernelSymbol"], budget),
                                    "symbol": case["kernelSymbol"], "case": case})
                if case["expectation"]["kind"] == "rejected":
                    expected_negative.add(key)
    expected_positive = selections.keys() - expected_negative
    consumed = set()
    kernels = []
    by_id: dict[str, set[tuple[Any, ...]]] = {}
    identity_ids = {}
    for kernel in budget.rows(inventory["kernels"], "kernels"):
        _object(kernel, {"kernelId", "selections", "variants"}, "kernel")
        kernel_id = kernel["kernelId"]
        if not isinstance(kernel_id, str) or KERNEL_ID.fullmatch(kernel_id) is None or kernel_id in by_id:
            _fail("duplicate or invalid kernelId")
        refs = []
        keys = set()
        identity = None
        for value in budget.rows(kernel["selections"], "kernel selections"):
            key, ref = _reference(value)
            if key not in expected_positive or key in consumed:
                _fail("unknown, negative, or duplicate positive selection")
            current = selections[key]["identity"]
            if identity is not None and identity != current:
                _fail("one kernelId cannot merge different source selections")
            identity = current
            consumed.add(key)
            keys.add(key)
            refs.append(ref)
        if not refs:
            _fail("kernel identity requires an existing positive source selection")
        if identity in identity_ids:
            _fail("identical source selections must share one kernelId across targets")
        identity_ids[identity] = kernel_id
        variants = []
        for variant in budget.rows(kernel["variants"], "variants"):
            _object(variant, {"kind", "status", "source", "blocker"}, "variant")
            if variant["status"] != "pending" or variant["source"] is not None:
                _fail("variants remain pending, not source implementations or qualification")
            blocker = _object(variant["blocker"], {"owner", "issue", "reason"}, "blocker")
            owner = _text(blocker["owner"], "blocker owner")
            issue = _text(blocker["issue"], "blocker issue", 1024)
            reason = _text(blocker["reason"], "blocker reason", 2048)
            if ISSUE_URL.fullmatch(issue) is None:
                _fail("blocker issue must be an exact GitHub issue URL")
            variants.append({"kind": variant["kind"], "status": "pending", "source": None,
                             "blocker": {"owner": owner, "issue": issue, "reason": reason}})
        if [row["kind"] for row in variants] not in (["simt", "tile"], ["simt", "tile", "mixed"]):
            _fail("kernel must retain ordered SIMT/tile and optional mixed variants")
        by_id[kernel_id] = keys
        kernels.append({"kernelId": kernel_id, "selections": refs, "variants": variants})
    if consumed != expected_positive:
        _fail("missing positive source selection")

    negative_refs = []
    negative_keys = set()
    for value in budget.rows(inventory["negativeCases"], "negative cases"):
        key, ref = _reference(value)
        if key not in expected_negative or key in negative_keys:
            _fail("unknown, positive, or duplicate required-negative case")
        negative_keys.add(key)
        negative_refs.append(ref)
    if negative_keys != expected_negative:
        _fail("missing required-negative case")

    runtime_tabs = {}
    candidates = {}
    case_intervals = {}
    if runtime_inventory is not None:
        total_bytes = 0
        for lesson in budget.rows(runtime_inventory["lessons"], "runtime lessons"):
            for ordinal, live in enumerate(budget.rows(lesson["codeTabs"], "runtime tabs")):
                location = (lesson["id"], ordinal)
                if location not in tabs or location in runtime_tabs:
                    _fail("runtime tab coverage differs")
                runtime_tabs[location] = live
                tab, _ = tabs[location]
                if tab["language"] != "rust":
                    continue
                encoded = _utf8(live.get("displayedCode"), "displayed code")
                total_bytes += len(encoded)
                if total_bytes > MAX_RUNTIME_BYTES:
                    _fail("runtime Rust source exceeds its aggregate byte bound")
                if (type(tab["displayedUtf8Bytes"]) is not int
                        or len(encoded) != tab["displayedUtf8Bytes"]
                        or hashlib.sha256(encoded).hexdigest() != tab["displayedSha256"]):
                    _fail("displayed bytes differ from the retained tab")
                items = budget.rows(scan_functions(live["displayedCode"]), "scanned function items")
                offsets = set()
                for function in items:
                    _object(function, {"kernelSymbol", "functionUtf8Offset", "attributedKernel"}, "scanned item")
                    symbol = _display_symbol(function["kernelSymbol"])
                    offset = _integer(function["functionUtf8Offset"], "functionUtf8Offset")
                    attributed = function["attributedKernel"]
                    if type(attributed) is not bool or offset in offsets or not _name_token_matches(encoded, offset, symbol):
                        _fail("scanner returned a duplicate or invalid function coordinate")
                    offsets.add(offset)
                    if attributed or tab["kind"] == "kernel":
                        candidates[(*location, offset, symbol)] = attributed
                if tab["sourceItem"] is not None:
                    case_intervals[location] = _fragment_intervals(tab, live, encoded)
        if runtime_tabs.keys() != tabs.keys():
            _fail("runtime tab coverage differs")

    displays = []
    seen = set()
    negative_display_keys = set()
    bound_positive = set()
    unresolved = []
    fixture_sources = {}
    for row in budget.rows(inventory["displayItems"], "display items"):
        _object(row, DISPLAY_FIELDS, "display item")
        lesson = _text(row["lessonId"], "lessonId")
        ordinal = _integer(row["tabOrdinal"], "tabOrdinal")
        offset = _integer(row["functionUtf8Offset"], "functionUtf8Offset")
        symbol = _display_symbol(row["kernelSymbol"])
        location = (lesson, ordinal)
        coordinate = (*location, offset, symbol)
        if location not in tabs or coordinate in seen:
            _fail("unknown or duplicate display occurrence")
        seen.add(coordinate)
        tab, role = tabs[location]
        if tab["language"] != "rust" or offset + len(symbol.encode("utf-8")) > tab["displayedUtf8Bytes"]:
            _fail("display occurrence is outside a Rust tab")
        if runtime_inventory is not None and coordinate not in candidates:
            _fail("display occurrence is missing from the live function census")
        classification = row["classification"]
        status = row["bindingStatus"]
        if not isinstance(classification, str) or not isinstance(status, str):
            _fail("display classification and binding status must be strings")
        reason = _text(row["reason"], "display reason", 2048)
        ids = budget.rows(row["kernelIds"], "display kernel IDs")
        if any(not isinstance(value, str) or value not in by_id for value in ids) or len(set(ids)) != len(ids):
            _fail("display has duplicate or unknown kernel IDs")
        refs = []
        refs_keys = set()
        for value in budget.rows(row["negativeCases"], "display negative cases"):
            key, ref = _reference(value)
            if key not in expected_negative or key in refs_keys:
                _fail("display has duplicate or unknown negative cases")
            refs_keys.add(key)
            refs.append(ref)

        matching_cases = {key for key, value in selections.items()
                          if key[:3] == ("source-driver-case", lesson, ordinal) and value["symbol"] == symbol}
        if runtime_inventory is not None:
            intervals = case_intervals.get(location, [])
            matching_cases = {key for key in matching_cases
                              if type(selections[key]["case"]["displayedFragmentOrdinal"]) is int
                              and 0 <= selections[key]["case"]["displayedFragmentOrdinal"] < len(intervals)
                              and intervals[selections[key]["case"]["displayedFragmentOrdinal"]][0] <= offset
                              < intervals[selections[key]["case"]["displayedFragmentOrdinal"]][1]}
        if classification in {"conceptual", "helper"}:
            if ids or refs or status != "not-applicable" or (runtime_inventory is not None and matching_cases):
                _fail("non-kernel classification cannot detach a source case or identity")
            if classification == "conceptual" and role != "conceptual":
                _fail("executable lesson cannot be downgraded to conceptual")
            if classification == "helper" and (tab["kind"] != "kernel" or candidates.get(coordinate, False)):
                _fail("attributed kernel cannot be classified as a helper")
        elif classification in {"kernel", "required-negative"}:
            if role != "executable" or status not in {"pending", "source-driver-contract", "fixture-source-contract"}:
                _fail("kernel display has an invalid role or binding status")
            if classification == "kernel" and (refs or (runtime_inventory is not None and matching_cases & expected_negative)):
                _fail("required negative cannot be converted into a positive kernel")
            if classification == "required-negative":
                if (ids or not refs or not refs_keys <= matching_cases or not refs_keys <= expected_negative
                        or (runtime_inventory is not None and refs_keys != matching_cases)):
                    _fail("required-negative display must retain its exact source cases")
                if negative_display_keys & refs_keys:
                    _fail("required-negative case has duplicate display bindings")
                negative_display_keys.update(refs_keys)
            if status == "fixture-source-contract":
                if (classification != "kernel" or not ids or refs or matching_cases
                        or tab["kind"] != "kernel" or tab["sourceItem"] is not None
                        or tab["sourceDigestScope"] != "file" or tab["sourceFragmentsSha256"] is not None):
                    _fail("fixture display requires an exclusive whole-file positive binding")
                if load_fixture_sources is None or rust_syntax is None:
                    _fail("fixture display requires current physical source validation")
                for kernel_id in ids:
                    keys = by_id[kernel_id]
                    if any(key[0] != "fixture" or selections[key]["symbol"] != symbol for key in keys):
                        _fail("fixture display identity has a different selection")
                    for key in sorted(keys):
                        fixture = selections[key]["fixture"]
                        if tab["sourcePath"] not in fixture["compilerInput"]["sourcePaths"]:
                            _fail("fixture display path is not an exact selected source")
                        if key[1] not in fixture_sources:
                            fixture_sources[key[1]] = _fixture_selection(
                                fixture, load_fixture_sources, scan_functions, rust_syntax, budget)
                        path, source_offset, source = fixture_sources[key[1]][symbol]
                        encoded = _utf8(source, "fixture source")
                        digest = hashlib.sha256(encoded).hexdigest()
                        if (path != tab["sourcePath"] or source_offset != offset
                                or len(encoded) != tab["displayedUtf8Bytes"]
                                or digest != tab["sourceSha256"] or digest != tab["displayedSha256"]):
                            _fail("fixture display differs from the exact current source occurrence")
                        if runtime_inventory is not None:
                            live = runtime_tabs[location]
                            if live.get("sourceFragments") is not None or live["displayedCode"] != source or not candidates[coordinate]:
                                _fail("fixture display differs from the live whole-file occurrence")
                            bound_positive.add(key)
            elif status == "pending":
                unresolved.append({"lessonId": lesson, "tabOrdinal": ordinal,
                                   "functionUtf8Offset": offset, "kernelSymbol": symbol, "reason": reason})
            else:
                bound_keys = refs_keys if classification == "required-negative" else set()
                if classification == "kernel":
                    if not ids:
                        _fail("source-bound display requires a kernel identity")
                    for kernel_id in ids:
                        joined = by_id[kernel_id] & matching_cases
                        if not joined:
                            _fail("display kernel identity has no exact same-tab source case")
                        bound_keys.update(joined)
                if runtime_inventory is not None:
                    for key in bound_keys:
                        case = selections[key]["case"]
                        fragments = case_intervals.get(location, [])
                        fragment = case["displayedFragmentOrdinal"]
                        if type(fragment) is not int or not 0 <= fragment < len(fragments):
                            _fail("source case fragment ordinal differs")
                        start, end = fragments[fragment]
                        if not candidates[coordinate] or not start <= offset < end:
                            _fail("source case does not bind this exact displayed function")
                        matches = [key for key in candidates if key[:2] == location and key[3] == symbol and start <= key[2] < end]
                        if len(matches) != 1:
                            _fail("source case displayed function is ambiguous")
                    if classification == "kernel":
                        for kernel_id in ids:
                            bound_positive.update(key for key in by_id[kernel_id]
                                                  if key[0] == "fixture" or key in bound_keys)
        else:
            _fail("unknown display classification")
        displays.append({"lessonId": lesson, "tabOrdinal": ordinal,
                         "functionUtf8Offset": offset, "kernelSymbol": symbol,
                         "classification": classification, "kernelIds": list(ids),
                         "negativeCases": refs, "bindingStatus": status, "reason": reason})
    if negative_display_keys != expected_negative:
        _fail("required-negative cases need complete direct display bindings")
    if runtime_inventory is not None and seen != candidates.keys():
        _fail("missing live function census occurrences")
    pending_displays = len(unresolved)
    for key in sorted(expected_positive - bound_positive):
        if key[0] == "fixture":
            reference = {"kind": key[0], "fixtureId": key[1], "kernelSymbol": key[2]}
        else:
            reference = {"kind": key[0], "lessonId": key[1], "tabOrdinal": key[2], "caseOrdinal": key[3]}
        unresolved.append({"selection": reference, "reason": "No exact live displayed source binding for this positive selection."})
    if runtime_inventory is None:
        unresolved.append({"reason": "The live runtime function census and source/display joins have not been validated."})
    complete = runtime_inventory is not None and not unresolved
    return {
        "inventoryComplete": complete, "requiredPairCount": len(kernels) if complete else None,
        "knownKernelIdentityCount": len(kernels), "displayItemCount": len(displays),
        "pendingDisplayItemCount": pending_displays, "negativeCaseCount": len(negative_refs),
        "runtimeCensusValidated": runtime_inventory is not None,
        "unresolvedBindings": unresolved, "kernelIdentities": kernels,
        "negativeCases": negative_refs, "displayItems": displays,
    }
