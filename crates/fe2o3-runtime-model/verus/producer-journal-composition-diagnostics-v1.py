#!/usr/bin/env python3
"""Draft full-root diagnostic predicate; no capture is a qualified kill.

Source/process/signature custody is supplied by the campaign owner. This draft
must be calibrated against all 89 source-bound cases before qualification:
the unchanged 38/21 old leaf/conditional fixtures and 30 fresh concrete captures.
"""
import hashlib
import json
from pathlib import Path
import types

V = Path("crates/fe2o3-runtime-model/verus")
BODY = Path("crates/fe2o3-runtime/src/context/versions/producer_input_fold_body.rs")
JOURNAL_BODY = Path("crates/fe2o3-runtime/src/context/versions/producer_journal_observer_bodies.rs")
DEFINITIONS = V / "producer_input_validate_definitions_v1.rs"
OUTCOMES = V / "producer_input_outcome_spec_v1.rs"
PROOFS = {"leaf": V / "context_producer_input_validate_v1.rs",
          "conditional": V / "context_producer_input_composition_v1.rs",
          "concrete": V / "context_producer_journal_composition_v1.rs"}
COUNTS = {"leaf": 42, "conditional": 64, "concrete": 214}
LENGTHS = {"leaf": 6, "conditional": 8, "concrete": 44}
PARSER_SHA = "318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4"
DECISIONS = {"active_lookup": "queued_active_lookup_decision_v1", "active_status": "queued_active_status_decision_v1",
             "queued_lookup": "reads::lookup_decision_v1", "queued_status": "reads::status_decision_v1"}


def need(value, message):
    if not value:
        raise ValueError(message)


def parser(path):
    need(path.is_file() and path.resolve() == path and not path.is_symlink(), "ordinary shared span parser")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == PARSER_SHA, "exact reviewed recursive source-span parser")
    helper = types.ModuleType("private_concrete_diagnostic_parser")
    helper.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), helper.__dict__)
    helper.MACROS |= {"producer_observe_" + route + "_body_v1!" for route in DECISIONS}
    return helper


def parse(helper, text):
    def invalid(value):
        raise ValueError("nonfinite JSON: " + value)
    return json.loads(text, object_pairs_hook=helper.strict_object, parse_constant=invalid)


def unique(text, expression):
    need(text.count(expression) == 1, "one exact intended source expression")
    start = text.index(expression)
    return start, start + len(expression)


def target(helper, sources, root, case):
    family, boundary = case["family"], case["boundary"]
    proof = PROOFS[family]
    method = boundary.split("::")[-1]
    if family == "leaf":
        need(boundary in {"Observations::validate", "producer_dependency_contains_v1", "producer_source_pair_contains_v1"},
             "known complete leaf boundary")
        path = DEFINITIONS if method == "validate" else OUTCOMES
        offset = 0
    elif family == "conditional":
        need(boundary in helper.FULL_BOUNDARIES, "known conditional implementation boundary")
        path = DEFINITIONS if boundary.startswith("Observations::") else proof
        offset = 0
    else:
        need(boundary.startswith("concrete_composition::"), "concrete module boundary")
        owner = boundary.removeprefix("concrete_composition::")
        need(owner in helper.FULL_BOUNDARIES | {"Observations::observe_" + route for route in DECISIONS},
             "known concrete implementation boundary")
        path = proof
        offset = sources[proof].index("    struct Composition<'a,") if owner.startswith("Composition::") else 0
    start, end = helper.function_interval(sources[path][offset:], method)
    function = (start + offset, end + offset)
    ranges = {str(root / path): function}
    macro = None
    if family == "leaf":
        macro = {"validate": "producer_input_validate_body", "producer_dependency_contains_v1": "producer_dependency_contains_body",
                 "producer_source_pair_contains_v1": "producer_source_pair_contains_body"}[method]
        macro_path = BODY
    elif method == "reconcile":
        macro, macro_path = "producer_input_fold_body", BODY
    elif method.startswith("observe_") and method != "observe_expected_credit":
        macro, macro_path = "producer_" + method + "_body_v1", JOURNAL_BODY
    if macro:
        anchor = "macro_rules! " + macro + " {"
        lo, hi = unique(sources[macro_path], anchor)
        ranges[str(root / macro_path)] = (lo, helper.balanced_end(sources[macro_path], hi - 1))
    return ranges, path, function, method


def forwarding(helper, sources, root, case, rows, path, interval):
    name = case["name"]
    route, suffix = name.split("-", 1)
    need(route in DECISIONS and suffix in {"constant-error", "error-coerced", "prefetch-status"}
         and (suffix != "prefetch-status" or route == "active_lookup"), "known actual forwarding fault")
    trace_only = name == "active_lookup-prefetch-status"
    expected_label = "wrapper-ghost-trace-only" if trace_only else "actual-journal-result-equality"
    need(case["boundary_label"] == expected_label, "eight result-equality versus one trace-only case")
    proof, body = sources[path], sources[JOURNAL_BODY]
    expression = ("final(self).calls@ == old(self).calls@.push(Call::ActiveLookup(reference))" if trace_only
                  else "result == " + DECISIONS[route] + "(old(self).versions.journal, reference)")
    primary_interval = unique(proof, expression)
    need(interval[0] <= primary_interval[0] < primary_interval[1] <= interval[1], "case-specific wrapper contract")
    paths = {str(root / selected): text for selected, text in sources.items()}
    proof_path = str(root / path)
    need(len(rows) == 3, "exact wrapper enumeration, postcondition, summary")
    note, error, summary = rows
    signature_end = proof.index("\n            ensures", interval[0])
    need(proof[signature_end - 1] == ")", "named wrapper return binder")
    coordinates = [[helper.span_source(span, paths, proof_path) for span in row["spans"]] for row in rows]
    need(note["level"] == "note" and note["message"].startswith("function body check:")
         and coordinates[0] == [(proof_path, interval[0], signature_end - 1)]
         and note["spans"][0]["is_primary"] is True and note["spans"][0]["label"] is None
         and note["spans"][0]["expansion"] is None, "exact wrapper signature enumeration join")
    need(error["level"] == "error" and error["message"] == "postcondition not satisfied" and len(error["spans"]) == 2,
         "one actual wrapper postcondition diagnostic")
    exit_span, primary = error["spans"]
    need(exit_span["is_primary"] is False and primary["is_primary"] is True
         and primary["label"] == "failed this postcondition" and primary["expansion"] is None
         and coordinates[1][1] == (proof_path, *primary_interval), "exact result/ghost primary, not just a nearby function")
    if trace_only:
        body_path = str(root / JOURNAL_BODY)
        need(body.count("?") == 1 and coordinates[1][0] == (body_path, *unique(body, "?"))
             and exit_span["label"] == "at this exit" and exit_span["expansion"] is not None
             and exit_span["expansion"]["macro_decl_name"] == "desugaring of operator `?`", "eager-status early exit")
        desugared = exit_span["expansion"]["span"]
        nested = desugared["expansion"]
        need(helper.span_source(desugared, paths, proof_path) == coordinates[1][0] and type(nested) is dict
             and nested["macro_decl_name"] == "producer_observe_active_lookup_body_v1!"
             and helper.span_source(nested["span"], paths, proof_path)
                 == (proof_path, *unique(proof, "producer_observe_active_lookup_body_v1!(self, reference)")),
             "recursive early-exit expansion joins intended active lookup")
    else:
        exit_path, lo, hi = coordinates[1][0]
        need(exit_path == proof_path and interval[0] <= lo < hi <= interval[1] and proof[lo:hi] == "result"
             and exit_span["label"] == "at the end of the function body" and exit_span["expansion"] is None,
             "ordinary result-return equality failure")
    need(summary["message"] == "aborting due to 1 previous error" and summary["spans"] == [], "exact wrapper summary")
    return {"boundary": expected_label, "actual_result_failure_observed": not trace_only,
            "inner_query_call_count_proved": False, "primary_expression": expression}


def negative(shared_parser, verifier, sources, expected_paths, root, case, status, stdout, stderr):
    helper = parser(shared_parser)
    family = case["family"]
    need(family in PROOFS and case["capture_selector"] is None and Path(case["root"]) == PROOFS[family],
         "exact full-root family, never original selected provenance")
    need(set(sources) == set(expected_paths) == {Path(path) for path in case["closure"]}
         and len(sources) == LENGTHS[family] and PROOFS[family] in sources,
         "exact complete family projection")
    need(stdout.endswith("\n") and stderr.endswith("\n"), "complete raw streams")
    result = parse(helper, stdout)
    need(type(result) is dict and set(result) == {"verus", "verification-results", "func-details"}
         and result["verus"] == verifier, "full result schema and exact verifier identity")
    expected = {"encountered-error": True, "encountered-vir-error": False, "success": False,
                "verified": COUNTS[family] - 1, "errors": 1, "is-verifying-entire-crate": True}
    vr = result["verification-results"]
    need(type(vr) is dict and set(vr) == set(expected)
         and all(type(vr[key]) is type(value) and vr[key] == value for key, value in expected.items()),
         "one failed function in the measured full root; distinct from diagnostic multiplicity")
    details = result["func-details"]
    function_key = PROOFS[family].stem + "::" + case["boundary"]
    need(type(details) is dict and 0 < len(details) <= 1024 and function_key in details
         and all(type(name) is str and value == {"obligation_proof_notes": [], "failed_proof_notes": []}
                 for name, value in details.items()), "complete finite function-note schema and intended function join")
    ranges, path, interval, method = target(helper, sources, root, case)
    # Reuse the reviewed logical/span predicate privately in its full-root mode.
    helper.PROOFS["composition"] = PROOFS[family]
    helper.CLOSURES["composition"] = set(expected_paths)
    helper.COUNTS["composition"] = COUNTS[family]
    helper.target_ranges = lambda *_: ranges
    boundary = case["boundary"].removeprefix("concrete_composition::")
    normalized_case = {"family": "composition", "selector": None, "boundary": boundary}
    classified = helper.negative(verifier, sources, root, normalized_case, status, stdout, stderr)
    rows = [parse(helper, line) for line in stderr.splitlines()]
    if family == "concrete" and method in {"observe_" + route for route in DECISIONS}:
        classified["forwarding"] = forwarding(helper, sources, root, case, rows, path, interval)
    classified.update(family=family, case=case["name"], full_root=True, qualified_kill=False,
                      historical_capture_is_qualified_kill=False, signed_campaign_qualified=False)
    return classified
