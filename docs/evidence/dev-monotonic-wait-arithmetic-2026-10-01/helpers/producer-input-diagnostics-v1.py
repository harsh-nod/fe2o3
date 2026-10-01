#!/usr/bin/env python3
"""Strict source-bound diagnostic predicates, not a signed campaign result."""
import json
from pathlib import Path
import re

V = Path("crates/fe2o3-runtime-model/verus")
BODY = Path("crates/fe2o3-runtime/src/context/versions/producer_input_fold_body.rs")
DEFINITIONS = V / "producer_input_validate_definitions_v1.rs"
SPEC = V / "producer_input_fold_spec_v1.rs"
PROOFS = {"leaf": V / "context_producer_input_validate_v1.rs",
          "fold": V / "context_producer_input_fold_v1.rs",
          "composition": V / "context_producer_input_composition_v1.rs"}
COUNTS = {"leaf": 42, "fold": 13, "composition": 64}
CLOSURES = {"leaf": {PROOFS["leaf"], DEFINITIONS, BODY},
            "fold": {PROOFS["fold"], SPEC, BODY},
            "composition": {PROOFS["composition"], DEFINITIONS, SPEC, BODY}}
SELECTED = {
    ("leaf", "*Observations::validate"): (DEFINITIONS, "validate", "producer_input_validate_body"),
    ("leaf", "*producer_dependency_contains_v1"): (DEFINITIONS, "producer_dependency_contains_v1", "producer_dependency_contains_body"),
    ("leaf", "*producer_source_pair_contains_v1"): (DEFINITIONS, "producer_source_pair_contains_v1", "producer_source_pair_contains_body"),
    ("fold", "*Observations::reconcile"): (PROOFS["fold"], "reconcile", "producer_input_fold_body"),
}
FULL_BOUNDARIES = {"Composition::validate", "Composition::active_count", "Composition::queued_count",
                   "Composition::reconcile", "Observations::observe_expected_credit"}
LOGICAL = {"postcondition not satisfied", "precondition not satisfied", "assertion failed",
           "invariant not satisfied at end of loop body", "invariant not satisfied before loop",
           "loop invariant not satisfied"}
ENUMERATION = {prefix + ": not all errors may have been reported; rerun with a higher value for "
               "--multiple-errors to find other potential errors in this function"
               for prefix in ("function body check", "while loop")}
SELECTION = "verifying root module (selected functions)"
MACROS = {"producer_input_validate_body!", "producer_input_fold_body!",
          "producer_dependency_contains_body!", "producer_source_pair_contains_body!"}
DESUGAR = {"desugaring of `while` loop", "desugaring of operator `?`"}
SPAN_KEYS = {"file_name", "byte_start", "byte_end", "line_start", "line_end", "column_start", "column_end",
             "is_primary", "text", "label", "suggested_replacement", "suggestion_applicability", "expansion"}


def need(value, message):
    if not value:
        raise ValueError(message)


def strict_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def source_map(sources, root, family):
    need(family in CLOSURES and set(sources) == CLOSURES[family], "entire exact proof source closure")
    need(root.is_absolute() and root.resolve() == root, "canonical absolute projection root")
    need(all(type(text) is str and text.isascii() for text in sources.values()), "exact ASCII source buffers")
    return {str((root / path).resolve()): text for path, text in sources.items()}


def span_source(span, paths, proof_root, depth=0, allow_zero=False):
    need(depth <= 8 and isinstance(span, dict) and set(span) == SPAN_KEYS, "bounded exact recursive source span")
    name = span["file_name"]
    need(type(name) is str and Path(name).is_absolute(), "absolute diagnostic source")
    path = str(Path(name).resolve())
    need(path in paths and type(span["is_primary"]) is bool, "typed span belongs to complete source closure")
    text = paths[path]
    numbers = ("byte_start", "byte_end", "line_start", "line_end", "column_start", "column_end")
    need(all(type(span[key]) is int for key in numbers), "integer, non-Boolean source coordinates")
    lo, hi = span["byte_start"], span["byte_end"]
    need(0 <= lo <= hi <= len(text), "source-bounded byte interval")
    line_start, line_end = text.count("\n", 0, lo) + 1, text.count("\n", 0, hi) + 1
    column_start, column_end = lo - text.rfind("\n", 0, lo), hi - text.rfind("\n", 0, hi)
    need([span[key] for key in numbers[2:]] == [line_start, line_end, column_start, column_end],
         "byte/line/column correspondence")
    lines = text.splitlines()
    if lo == hi:
        need(allow_zero and lo == 0 and path == proof_root and span["is_primary"] is False
             and span["expansion"] is None and span["label"] is None,
             "only calibrated synthetic desugaring definition can have a zero span")
        expected = []
    else:
        need(line_end <= len(lines), "full source line interval")
        expected = [{"text": lines[index - 1], "highlight_start": column_start if index == line_start else 1,
                     "highlight_end": column_end if index == line_end else len(lines[index - 1]) + 1}
                    for index in range(line_start, line_end + 1)]
    need(isinstance(span["text"], list) and all(isinstance(row, dict)
         and set(row) == {"text", "highlight_start", "highlight_end"} and type(row["text"]) is str
         and type(row["highlight_start"]) is int and type(row["highlight_end"]) is int for row in span["text"])
         and span["text"] == expected, "exact source text and integer highlight coordinates")
    need((span["label"] is None or type(span["label"]) is str) and span["suggested_replacement"] is None
         and span["suggestion_applicability"] is None, "no hidden repair or non-text span label")
    expansion = span["expansion"]
    if expansion is not None:
        need(isinstance(expansion, dict) and set(expansion) == {"span", "macro_decl_name", "def_site_span"}
             and expansion["macro_decl_name"] in MACROS | DESUGAR, "exact measured macro/desugaring expansion schema")
        macro = expansion["macro_decl_name"]
        call_path, call_lo, call_hi = span_source(expansion["span"], paths, proof_root, depth + 1)
        definition = expansion["def_site_span"]
        def_path, def_lo, def_hi = span_source(definition, paths, proof_root, depth + 1, macro in DESUGAR)
        need(expansion["span"]["is_primary"] is False and definition["is_primary"] is False,
             "nested expansion evidence is not a direct logical primary")
        call_text = paths[call_path][call_lo:call_hi]
        if macro in MACROS:
            need(call_text.startswith(macro + "(") and paths[def_path][def_lo:def_hi] == "macro_rules! " + macro[:-1]
                 and definition["expansion"] is None, "exact macro invocation and declaration correspondence")
        else:
            need(def_lo == def_hi == 0 and ((macro == "desugaring of operator `?`" and call_text == "?")
                 or (macro == "desugaring of `while` loop" and call_text.startswith("while "))),
                 "exact measured builtin desugaring correspondence")
    return path, lo, hi


def function_interval(text, name):
    anchor = "fn " + name + "("
    need(text.count(anchor) == 1, "unique intended function")
    start = text.index(anchor)
    indentation = text[text.rfind("\n", 0, start) + 1:start]
    opening = text.index("\n" + indentation + "{", start) + 1 + len(indentation)
    return start, balanced_end(text, opening)


def balanced_end(text, opening):
    need(text[opening] == "{", "exact opening delimiter")
    depth, end = 1, opening + 1
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    need(depth == 0, "balanced intended source interval")
    return end


def target_ranges(sources, root, case):
    family, selector = case["family"], case["selector"]
    if family == "composition":
        need(selector is None and case["boundary"] in FULL_BOUNDARIES, "exact unfiltered composition boundary")
        path = DEFINITIONS if case["boundary"].startswith("Observations::") else PROOFS[family]
        function = case["boundary"].split("::")[1]
        macro = "producer_input_fold_body" if function == "reconcile" else None
    else:
        need((family, selector) in SELECTED, "exact selected leaf/fold family")
        path, function, macro = SELECTED[(family, selector)]
    ranges = {str(root / path): function_interval(sources[path], function)}
    if macro is not None:
        anchor = "macro_rules! " + macro + " {"
        need(sources[BODY].count(anchor) == 1, "unique intended actual macro")
        start = sources[BODY].index(anchor)
        opening = start + len(anchor) - 1
        ranges[str(root / BODY)] = (start, balanced_end(sources[BODY], opening))
    return ranges


def in_family(item, ranges):
    path, lo, hi = item
    return path in ranges and ranges[path][0] <= lo < hi <= ranges[path][1]


def recommendation(row, sources, root, paths, proof_root):
    need(row["level"] == "note" and len(row["spans"]) == 2, "exact two-span recommendation note")
    declaration, call = row["spans"]
    dp, dlo, dhi = span_source(declaration, paths, proof_root)
    cp, clo, chi = span_source(call, paths, proof_root)
    need(dp == str(root / SPEC) and cp == str(root / PROOFS["composition"])
         and declaration["is_primary"] is False and declaration["label"] == "recommendation not met"
         and call["is_primary"] is True and call["label"] is None
         and declaration["expansion"] is None and call["expansion"] is None,
         "exact recommendation source, direct roles and labels")
    matches = []
    for family in ("active", "queued"):
        anchor = "pub(crate) open spec fn " + family + "_before<E>"
        need(sources[SPEC].count(anchor) == 1, "unique cursor declaration")
        start = sources[SPEC].index("0 <= end <= history.len()", sources[SPEC].index(anchor))
        expression = "fold::" + family + "_before(old(self).original(), index as int)"
        proof = sources[PROOFS["composition"]]
        need(proof.count(expression) == 1, "unique cursor recommendation call")
        if (dlo, dhi) == (start, start + len("0 <= end <= history.len()")) \
                and (clo, chi) == (proof.index(expression), proof.index(expression) + len(expression)):
            matches.append(family)
    need(len(matches) == 1, "exact paired cursor recommendation expression")
    return matches[0]


def negative(verifier, sources, root, case, status, stdout, stderr):
    need(type(status) is int and status == 1, "integer logical-error exit1")
    family = case["family"]
    paths = source_map(sources, root, family)
    ranges = target_ranges(sources, root, case)
    proof_root = str(root / PROOFS[family])
    result = json.loads(stdout, object_pairs_hook=strict_object)
    need(isinstance(result, dict) and result.get("verus") == verifier, "exact pinned verifier identity")
    vr = result.get("verification-results")
    keys = {"encountered-error", "encountered-vir-error", "verified", "errors", "is-verifying-entire-crate"}
    full = family == "composition"
    need(isinstance(vr, dict) and set(vr) == keys | ({"success"} if full else set())
         and (not full or vr["success"] is False), "exact distinct selected/full result schemas")
    need(vr["encountered-error"] is True and vr["encountered-vir-error"] is False
         and vr["is-verifying-entire-crate"] is full and type(vr["verified"]) is int
         and 0 <= vr["verified"] < COUNTS[family] and type(vr["errors"]) is int
         and 0 < vr["errors"] <= COUNTS[family], "bounded logical verification counts and exact mode")
    errors, summaries, selected, enumerated, recommendations, hits = [], [], [], [], [], []
    for line in stderr.splitlines():
        need(not summaries, "no records after terminal summary")
        row = json.loads(line, object_pairs_hook=strict_object)
        need(isinstance(row, dict) and set(row) == {"$message_type", "message", "code", "level", "spans", "children", "rendered"}
             and row["$message_type"] == "diagnostic" and type(row["message"]) is str
             and type(row["rendered"]) is str and row["code"] is None and row["children"] == []
             and isinstance(row["spans"], list), "strict diagnostic schema without compiler codes/children")
        spans = [(span, span_source(span, paths, proof_root)) for span in row["spans"]]
        primaries = [item for span, item in spans if span["is_primary"]]
        message = row["message"]
        if row["level"] == "error" and message in LOGICAL:
            need(primaries and all(in_family(item, ranges) for item in primaries),
                 "every logical primary belongs to the intended actual function/macro")
            errors.append(message)
            hits.extend({"path": item[0], "byte_start": item[1], "byte_end": item[2], "message": message} for item in primaries)
        elif row["level"] == "error" and re.fullmatch(r"aborting due to \d+ previous errors?", message):
            need(not spans, "span-free terminal summary")
            summaries.append(int(re.search(r"\d+", message).group()))
        elif row["level"] == "note" and message == SELECTION:
            need(not full and not spans, "only exact inert selector note on selected runs")
            selected.append(message)
        elif row["level"] == "note" and message in ENUMERATION:
            need(primaries and all(in_family(item, ranges) for item in primaries), "reporting note belongs to intended implementation")
            enumerated.append((message, tuple(primaries)))
        elif row["level"] == "note" and message == "recommendation not met":
            need(full and case["boundary"] == "Composition::validate", "only measured validator recommendation family")
            recommendations.append(recommendation(row, sources, root, paths, proof_root))
        else:
            raise ValueError("unreviewed or nonlogical diagnostic: " + message)
    need(errors and summaries == [len(errors)], "exact complete logical diagnostic summary")
    need(selected == ([] if full else [SELECTION]), "exact selected-note multiplicity")
    need(enumerated and len(enumerated) == len(set(enumerated)), "unique actual source-bound enumeration notes")
    expected_recommendations = ["active", "queued"] if full and case["boundary"] == "Composition::validate" else []
    need(sorted(recommendations) == expected_recommendations, "exact measured recommendation-note families")
    return {"logical_diagnostic_accepted": True, "verified": vr["verified"], "errors": vr["errors"],
            "logical_messages": errors, "intended_primary_spans": hits,
            "signed_campaign_qualified": False, "source_and_process_binding_required": True}
