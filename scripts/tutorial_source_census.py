"""Bound current tutorial inputs to same-invocation diagnostic source spans.

These joins are observations, not compiler-execution attestations, policy
receipts, or evidence that a pending SIMT/tile/mixed implementation exists.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re


HASH = re.compile(r"[0-9a-f]{64}\Z")
MAX_FUNCTIONS = 512
MAX_FILES = 128
MAX_FILE_BYTES = 4 * 1024 * 1024


class CensusError(ValueError):
    pass


def reject(message):
    raise CensusError("source census: " + message)


def exact(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        reject(label + " fields differ")
    return value


def digest(value, label):
    if not isinstance(value, str) or not HASH.fullmatch(value):
        reject(label + " is not a lowercase SHA-256")
    return value


def integer(value, limit, label):
    if type(value) is not int or not 0 <= value <= limit:
        reject(label + " exceeds its bound")
    return value


def text(value, label, limit=4096):
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > limit:
        reject(label + " is empty or exceeds its bound")
    return value


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False)


def registered_invocations(manifest):
    """Derive invocation identities; callers never select root/variant matches."""
    result = {}
    for fixture in manifest["compilerFixtures"]:
        value = {"fixtureId": fixture["fixtureId"], "target": fixture["target"],
                 "compilerInput": {key: value for key, value in fixture["compilerInput"].items()
                                   if key != "contractSha256"}}
        result[value["fixtureId"]] = (value, [
            ("fixture", fixture["fixtureId"], symbol)
            for symbol in value["compilerInput"]["kernelSymbols"]], False)
    for lesson in manifest["curriculum"]["lessons"]:
        for ordinal, tab in enumerate(lesson["codeTabs"]):
            item = tab["sourceItem"]
            if item is None:
                continue
            for index, case in enumerate(item["cases"]):
                identifier = f"source-driver-{lesson['lessonId']}-tab{ordinal}-case{index}"
                if identifier in result:
                    reject("duplicate registered invocation")
                value = {"fixtureId": identifier, "target": case["target"], "compilerInput": {
                    **item["compilerInput"], "features": case["features"],
                    "kernelSymbols": [case["kernelSymbol"]]}}
                result[identifier] = (value, [("source-driver-case", lesson["lessonId"], ordinal, index)],
                                      case["expectation"]["kind"] == "rejected")
    if len(result) > 4096:
        reject("invocation roster exceeds its bound")
    return result


def input_snapshot(parent, root, manifest, manifest_sha256, fixture):
    """Read a fresh physical closure using the existing manifest validator."""
    exact(fixture, {"fixtureId", "target", "compilerInput"}, "snapshot fixture")
    entry = registered_invocations(manifest).get(fixture["fixtureId"])
    if entry is None or canonical(entry[0]) != canonical(fixture):
        reject("snapshot invocation differs from its registered source contract")
    inputs = fixture["compilerInput"]
    cache = {}
    checked = parent.validate_compiler_input_data(
        root, inputs, "source census snapshot", cache,
        feature_scoped_includes=entry[1][0][0] == "source-driver-case")
    package = cache[inputs["packageManifest"]]
    sources = []
    for path, source in package["packageSources"]:
        payload = source.encode("utf-8")
        sha = hashlib.sha256(payload).hexdigest()
        if parent.sha256_file(path, parent.MAX_ATTRIBUTED_SOURCE_BYTES, "source census snapshot") != sha:
            reject("physical source changed during snapshot")
        sources.append({"path": path.relative_to(root).as_posix(), "sha256": sha, "bytes": len(payload)})
    return {"schema": "fe2o3-tutorial-source-input-snapshot-v1",
            "manifestSha256": manifest_sha256, "fixture": fixture,
            "enabledFeatures": checked["enabledFeatures"], "sources": sources}


def option_values(arguments, option):
    values = []
    index = 1
    while index < len(arguments):
        argument = arguments[index]
        if argument == option:
            index += 1
            if index == len(arguments):
                reject("missing compiler option value")
            values.append(arguments[index])
        elif argument.startswith(option + "="):
            values.append(argument[len(option) + 1:])
        index += 1
    return values


def check_invocation(root, fixture, snapshot, arguments, cwd):
    if (not isinstance(arguments, list) or not 1 <= len(arguments) <= 4096
            or any(not isinstance(arg, str) for arg in arguments)
            or sum(len(arg.encode("utf-8")) for arg in arguments) > 1024 * 1024
            or any(arg.startswith("@") for arg in arguments)):
        reject("compiler arguments are missing, indirect, or exceed bounds")
    inputs = fixture["compilerInput"]
    package = (root / inputs["packageManifest"]).parent.resolve()
    if cwd != str(package):
        reject("working directory differs from the selected package")
    if option_values(arguments, "--crate-name") != [inputs["cargoTarget"]["name"]]:
        reject("compiler crate selection differs")
    if option_values(arguments, "--crate-type") != ["lib"]:
        reject("compiler Cargo target kind differs")
    if option_values(arguments, "--target") != ["amdgcn-amd-amdhsa"]:
        reject("compiler target differs")
    cfg = option_values(arguments, "--cfg")
    features = []
    for value in cfg:
        match = re.fullmatch(r'feature="([^"\\]+)"', value)
        if match is None:
            reject("unrecorded non-feature compiler cfg")
        features.append(match[1])
    if len(set(features)) != len(features) or sorted(features) != sorted(snapshot["enabledFeatures"]):
        reject("compiler features differ from the validated Cargo closure")
    cpus = []
    for index, argument in enumerate(arguments):
        value = arguments[index + 1] if argument == "-C" and index + 1 < len(arguments) else argument[2:] if argument.startswith("-C") else ""
        if value.startswith("target-cpu="):
            cpus.append(value.removeprefix("target-cpu="))
    if cpus != [fixture["target"]]:
        reject("compiler target profile differs")
    # Do not parse arbitrary rustc arguments as a shell command or infer a
    # source from crate-name. Require the actual selected library argument.
    expected_source = (package / inputs["cargoTarget"]["sourcePath"]).resolve()
    sources = [argument for argument in arguments[1:] if argument.endswith(".rs")]
    if len(sources) != 1 or (package / sources[0]).resolve() != expected_source:
        reject("compiler source argument differs from Cargo target")


def observation(value, label):
    exact(value, {"status", "value"}, label)
    if value["status"] == "unavailable":
        text(value["value"], label + " reason")
        return None
    if value["status"] != "available":
        reject(label + " observation status differs")
    return value["value"]


def check_span(value, files):
    span = observation(value, "source span")
    if span is None:
        return None
    exact(span, {"expansion", "callSite", "expansionChainSha256", "expansionDepth"}, "source span")
    digest(span["expansionChainSha256"], "expansion chain")
    integer(span["expansionDepth"], 64, "expansion depth")
    for name in ("expansion", "callSite"):
        origin = exact(span[name], {"file", "coordinates"}, "source origin")
        index = integer(origin["file"], len(files) - 1, "source file index")
        coordinates = exact(origin["coordinates"], {
            "original_start", "original_end", "normalized_start", "normalized_end"}, "source coordinates")
        for prefix, limit in (("original", files[index]["originalBytes"]),
                              ("normalized", files[index]["normalizedBytes"])):
            start = integer(coordinates[prefix + "_start"], limit, "source start")
            end = integer(coordinates[prefix + "_end"], limit, "source end")
            if start > end:
                reject("source coordinates are reversed")
    return span


def check_record(root, fixture, snapshot, record, seen_runs):
    exact(record, {"schema", "diagnosticOnly", "qualified", "authenticatesCompilerExecution",
                   "runId", "arguments", "workingDirectory", "before", "after", "census", "censusSha256"}, "runner record")
    if (record["schema"] != "fe2o3-tutorial-source-census-observation-v1"
            or record["diagnosticOnly"] is not True or record["qualified"] is not False
            or record["authenticatesCompilerExecution"] is not False):
        reject("runner record is not diagnostic-only")
    run_id = digest(record["runId"], "run ID")
    if run_id in seen_runs:
        reject("replayed run ID across invocations")
    seen_runs.add(run_id)
    if canonical(record["before"]) != canonical(snapshot) or canonical(record["after"]) != canonical(snapshot):
        reject("before/after input snapshot is stale or changed")
    check_invocation(root, fixture, snapshot, record["arguments"], record["workingDirectory"])
    census = exact(record["census"], {
        "schema", "diagnosticOnly", "qualified", "authenticatesCompilerExecution", "extractionSucceeded",
        "arguments", "workingDirectory", "extractionMode", "runId", "selection"}, "compiler census")
    encoded = json.dumps(census, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False).encode("utf-8")
    if digest(record["censusSha256"], "retained census digest") != hashlib.sha256(encoded).hexdigest():
        reject("compiler census changed after runner retention")
    if (census["schema"] != "fe2o3-diagnostic-source-census-v1"
            or census["diagnosticOnly"] is not True or census["qualified"] is not False
            or census["authenticatesCompilerExecution"] is not False
            or type(census["extractionSucceeded"]) is not bool
            or census["runId"] != run_id or census["arguments"] != record["arguments"]
            or census["workingDirectory"] != record["workingDirectory"]
            or census["extractionMode"] != {"kind": "fixed-checked-output", "policy": 4}):
        reject("compiler census differs from the exact run/arguments/P4 mode")
    selection = observation(census["selection"], "selection")
    if selection is None:
        reject("runner published an incomplete compiler census")
    exact(selection, {"target", "functions", "files"}, "selection")
    if selection["target"] != fixture["target"]:
        reject("source selection target differs")
    files = selection["files"]
    functions = selection["functions"]
    if not isinstance(files, list) or not 1 <= len(files) <= MAX_FILES:
        reject("source file roster exceeds its bound")
    if not isinstance(functions, list) or not 1 <= len(functions) <= MAX_FUNCTIONS:
        reject("function roster exceeds its bound")
    identities = set()
    paths = set()
    total_bytes = 0
    for file in files:
        exact(file, {"identity", "displayPath", "compiledSourceHash", "originalSha256", "originalBytes", "normalizedBytes"}, "source file")
        identity = digest(file["identity"], "file identity")
        path = text(file["displayPath"], "source path")
        resolved = str((Path(record["workingDirectory"]) / path).resolve())
        if identity in identities or resolved in paths:
            reject("ambiguous source file identity/path")
        identities.add(identity)
        paths.add(resolved)
        text(file["compiledSourceHash"], "compiler source hash")
        digest(file["originalSha256"], "original source digest")
        total_bytes += integer(file["originalBytes"], MAX_FILE_BYTES, "original source length")
        integer(file["normalizedBytes"], file["originalBytes"], "normalized source length")
    if total_bytes > 16 * 1024 * 1024:
        reject("source closure exceeds its byte bound")
    identities = set()
    roots = {}
    for function in functions:
        exact(function, {"functionIdentity", "definitionIdentity", "monomorphizationIdentity", "role",
                         "exportName", "logicalName", "definition", "identifier"}, "function")
        identity = digest(function["functionIdentity"], "function identity")
        if identity in identities:
            reject("duplicate function identity")
        identities.add(identity)
        digest(function["definitionIdentity"], "definition identity")
        digest(function["monomorphizationIdentity"], "monomorphization identity")
        text(function["exportName"], "export name")
        if function["logicalName"] is not None:
            text(function["logicalName"], "logical name")
        if function["role"] not in ("kernel-entry", "internal-helper", "device-ffi-export"):
            reject("function role differs")
        check_span(function["definition"], files)
        check_span(function["identifier"], files)
        if function["role"] == "kernel-entry":
            name = function["exportName"]
            if name in roots:
                reject("ambiguous selected kernel root")
            roots[name] = function
    if set(roots) != set(fixture["compilerInput"]["kernelSymbols"]):
        reject("selected root roster differs from the registered invocation")
    return roots, files


def source_origin(root, cwd, function, files, selected):
    path, offset, source, sha = selected
    span = observation(function["identifier"], "identifier")
    definition = observation(function["definition"], "definition")
    if span is None or definition is None:
        return None, "compiler identifier or definition span unavailable"
    if span["expansionDepth"] or definition["expansionDepth"]:
        return None, "expanded source requires an explicit generated-body map"
    if span["expansion"] != span["callSite"] or definition["expansion"] != definition["callSite"]:
        reject("unexpanded source has conflicting call-site coordinates")
    origin = span["expansion"]
    file = files[origin["file"]]
    if (Path(cwd) / file["displayPath"]).resolve() != (root / path).resolve():
        reject("root identifier path differs from its feature-selected source")
    payload = source.encode("utf-8")
    normalized = payload.removeprefix(b"\xef\xbb\xbf").replace(b"\r\n", b"\n")
    if (file["originalSha256"] != sha or file["originalBytes"] != len(payload)
            or file["normalizedBytes"] != len(normalized)):
        reject("root source hash/length differs from current compiled source")
    coordinates = origin["coordinates"]
    start, end = coordinates["original_start"], coordinates["original_end"]
    token = function["exportName"].encode("utf-8")
    if start != offset or payload[start:end] not in (token, b"r#" + token):
        reject("root identifier is not the exact selected source token")
    defined = definition["expansion"]
    bounds = defined["coordinates"]
    if defined["file"] != origin["file"] or not bounds["original_start"] <= start < end <= bounds["original_end"]:
        reject("identifier is outside its exact definition")
    for current in (coordinates, bounds):
        for edge in ("start", "end"):
            prefix = payload[:current["original_" + edge]]
            try:
                prefix.decode("utf-8")
            except UnicodeDecodeError:
                reject("source coordinate splits a UTF-8 character")
            if len(prefix.removeprefix(b"\xef\xbb\xbf").replace(b"\r\n", b"\n")) != current["normalized_" + edge]:
                reject("normalized/original source coordinates disagree")
    return {"sourcePath": path, "sourceSha256": sha, "functionUtf8Offset": offset,
            "functionIdentity": function["functionIdentity"], "definitionIdentity": function["definitionIdentity"],
            "monomorphizationIdentity": function["monomorphizationIdentity"]}, None


def runtime_matches(runtime, displays, path, offset, source, symbol):
    tabs = {(lesson["id"], index): tab for lesson in runtime["lessons"]
            for index, tab in enumerate(lesson["codeTabs"])}
    matches = []
    physical = source.encode("utf-8")
    for row in displays:
        if row["classification"] not in ("kernel", "required-negative") or row["kernelSymbol"] != symbol:
            continue
        tab = tabs[(row["lessonId"], row["tabOrdinal"])]
        if tab["sourcePath"] != path:
            continue
        displayed = tab["displayedCode"].encode("utf-8")
        candidates = set()
        if tab["sourceDigestScope"] == "file":
            if displayed == physical:
                candidates.add(row["functionUtf8Offset"])
        else:
            for fragment in tab["sourceFragments"] or []:
                encoded = fragment.encode("utf-8")
                if not encoded or physical.count(encoded) != 1 or displayed.count(encoded) != 1:
                    reject("ambiguous physical/runtime source fragment")
                physical_start = physical.index(encoded)
                display_start = displayed.index(encoded)
                relative = row["functionUtf8Offset"] - display_start
                if 0 <= relative < len(encoded):
                    candidates.add(physical_start + relative)
        if len(candidates) > 1:
            reject("ambiguous runtime occurrence source coordinates")
        if candidates == {offset}:
            matches.append({key: row[key] for key in (
                "lessonId", "tabOrdinal", "functionUtf8Offset", "kernelSymbol", "classification", "bindingStatus")})
    return matches


def join(parent, root, manifest, manifest_sha256, runtime, projection, reports, *, max_records=4096):
    if runtime is None or not projection.get("runtimeCensusValidated"):
        reject("source joins require the validated live runtime inventory")
    invocations = registered_invocations(manifest)
    records = {}
    for corpus in reports:
        if corpus is None:
            continue
        if not isinstance(corpus, dict):
            reject("corpus must be an object")
        if corpus.get("manifest_sha256") != manifest_sha256:
            reject("corpus raw manifest digest is stale")
        if corpus.get("default_pipeline_activated") is not False or corpus.get("grants_artifact_or_launch_authority") is not False:
            reject("corpus grants authority")
        driver = corpus.get("schema") == "fe2o3-ordinary-source-policy4-source-driver-corpus-v1"
        if not driver and corpus.get("schema") != "fe2o3-ordinary-source-policy4-extraction-corpus-v1":
            reject("unsupported corpus schema")
        expected = {identifier for identifier, (_, keys, negative) in invocations.items()
                    if (keys[0][0] == "source-driver-case") == driver and not negative}
        if driver:
            exact(corpus, {"schema", "manifest_sha256", "configurations", "strict_negative_drivers_unchanged",
                           "all_checked_output_passed", "default_pipeline_activated", "grants_artifact_or_launch_authority", "cases"}, "source-driver corpus")
            if corpus["strict_negative_drivers_unchanged"] != sum(negative for _, _, negative in invocations.values()):
                reject("source-driver report omitted required-negative obligations")
        cases = corpus.get("cases")
        if not isinstance(cases, list) or len(cases) != len(expected) or corpus.get("configurations") != len(expected):
            reject("corpus must retain its complete invocation roster")
        found = set()
        for entry in cases:
            case = entry.get("result") if driver and isinstance(entry, dict) else entry
            if not isinstance(case, dict) or not isinstance(case.get("fixture"), dict):
                reject("corpus case is incomplete")
            identifier = case["fixture"].get("fixtureId")
            if identifier not in expected or identifier in found or identifier in records:
                reject("duplicate, unknown, or replayed corpus invocation")
            found.add(identifier)
            if canonical(case["fixture"]) != canonical(invocations[identifier][0]):
                reject("corpus inputs differ from the registered invocation")
            if driver:
                key = invocations[identifier][1][0]
                lesson = next(row for row in manifest["curriculum"]["lessons"] if row["lessonId"] == key[1])
                item = lesson["codeTabs"][key[2]]["sourceItem"]
                expected_case = item["cases"][key[3]]
                expected_source = {
                    "selection": {"kind": key[0], "lessonId": key[1], "tabOrdinal": key[2], "caseOrdinal": key[3]},
                    "source_item_contract_sha256": item["contractSha256"], "original_driver": item["driver"],
                    "original_driver_test": expected_case["testFunction"], "original_expectation": expected_case["expectation"],
                    "fixture": invocations[identifier][0],
                }
                if canonical(entry.get("source")) != canonical(expected_source):
                    reject("source-driver origin differs from its registered source case")
            if case.get("source_census") is not None and case.get("source_census_error") is not None:
                reject("runner retained an incomplete census as a success")
            if not isinstance(case.get("compiler_artifacts"), list) or any(not isinstance(path, str) for path in case["compiler_artifacts"]):
                reject("case artifact observations differ")
            passed = (isinstance(case.get("observation"), dict) and case.get("refusal") is None
                      and case.get("compiler_artifacts") == [])
            if (case.get("status") not in ("blocked", "checked-output-pass")
                    or (case["status"] == "checked-output-pass") != passed
                    or (case["status"] == "blocked" and not isinstance(case.get("refusal"), dict))):
                reject("case outcome contradicts its status")
            records[identifier] = case
        if found != expected:
            reject("corpus omitted registered invocations")
        if corpus.get("all_checked_output_passed") is not all(records[identifier]["status"] == "checked-output-pass" for identifier in found):
            reject("corpus outcome contradicts its cases")
    by_selection = {}
    for kernel in projection["kernelIdentities"]:
        for selection in kernel["selections"]:
            key = (("fixture", selection["fixtureId"], selection["kernelSymbol"]) if selection["kind"] == "fixture"
                   else ("source-driver-case", selection["lessonId"], selection["tabOrdinal"], selection["caseOrdinal"]))
            by_selection[key] = kernel
    result = {"schema": "fe2o3-tutorial-source-census-joins-v1", "diagnosticOnly": True,
              "qualified": False, "authenticatesCompilerExecution": False,
              "policy": {"kind": "fixed-checked-output", "version": 4, "productionMixedCompilation": False},
              "joins": [], "unresolved": []}
    seen_runs = set()
    remaining = integer(max_records, 4096, "derived record limit")

    def charge(count):
        nonlocal remaining
        if count > remaining:
            reject("derived join record bound exceeded")
        remaining -= count

    def unresolved(row):
        charge(1)
        result["unresolved"].append(row)

    def observe(selections, selected_source, unused_projection):
        for identifier, (fixture, keys, negative) in invocations.items():
            case = records.get(identifier)
            if case is None or case.get("source_census") is None:
                unresolved({"invocation": identifier, "reason":
                    "required-negative driver remains separate" if negative else
                    "same-run source/input census unavailable", "detail": None if case is None else case.get("source_census_error")})
                continue
            snapshot = input_snapshot(parent, root, manifest, manifest_sha256, fixture)
            record = case["source_census"]
            roots, files = check_record(root, fixture, snapshot, record, seen_runs)
            if case["status"] == "checked-output-pass" and record["census"]["extractionSucceeded"] is not True:
                reject("successful case retained a failed callback census")
            for key in keys:
                selection = selections[key]
                function = roots[selection["symbol"]]
                anchors = [observation(function[field], field) for field in ("identifier", "definition")]
                unavailable = any(anchor is None for anchor in anchors)
                expanded = any(anchor is not None and anchor["expansionDepth"] for anchor in anchors)
                if unavailable or expanded:
                    unresolved({"invocation": identifier, "kernelSymbol": selection["symbol"], "reason":
                        "compiler identifier or definition span unavailable" if unavailable else
                        "expanded source requires an explicit generated-body map"})
                    continue
                selected = selected_source(key)
                origin, reason = source_origin(root, record["workingDirectory"], function, files, selected)
                if reason:
                    unresolved({"invocation": identifier, "kernelSymbol": selection["symbol"], "reason": reason})
                    continue
                kernel = by_selection[key]
                matches = runtime_matches(runtime, projection["displayItems"], selected[0], selected[1], selected[2], selection["symbol"])
                variants = []
                for obligation in projection["kernelIdentities"]:
                    for variant in obligation["variants"]:
                        source = variant["source"]
                        if source and source["implementationKernelId"] == kernel["kernelId"] and source["selectionSha256"] == selection["identity"]:
                            variants.append({"kernelId": obligation["kernelId"], "kind": variant["kind"], "status": variant["status"]})
                # Charge the join, source origin, and repeated nested rows before
                # retaining them; a small input must not amplify into a product.
                charge(2 + len(matches) + len(variants))
                result["joins"].append({"invocation": identifier, "runId": record["runId"], "kernelId": kernel["kernelId"],
                    "selectionSha256": selection["identity"], "source": origin, "runtimeOccurrences": matches,
                    "registeredVariantBindings": variants, "extractionSucceeded": record["census"]["extractionSucceeded"]})
                if not matches:
                    unresolved({"invocation": identifier, "kernelId": kernel["kernelId"], "reason": "no exact current runtime source occurrence"})
        for kernel in projection["kernelIdentities"]:
            for variant in kernel["variants"]:
                if variant["status"] == "pending":
                    unresolved({"kernelId": kernel["kernelId"], "variant": variant["kind"],
                        "reason": "implementation source selection is pending; census cannot infer a variant"})

    parent.validate_kernel_inventory(manifest, runtime, repo_root=root, observe_sources=observe)
    return result
