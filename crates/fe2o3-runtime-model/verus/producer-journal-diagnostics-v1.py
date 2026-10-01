#!/usr/bin/env python3
"""Exact nine-case diagnostic predicate; source/process/signature qualification stays outside."""
import hashlib
import json
from pathlib import Path
import types

PROOF = Path("crates/fe2o3-runtime-model/verus/context_producer_journal_observers_v1.rs")
BODY = Path("crates/fe2o3-runtime/src/context/versions/producer_journal_observer_bodies.rs")
SPAN_PARSER_SHA = "318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4"
DECISIONS = {
    "active_lookup": "queued_active_lookup_decision_v1",
    "active_status": "queued_active_status_decision_v1",
    "queued_lookup": "reads::lookup_decision_v1",
    "queued_status": "reads::status_decision_v1",
}
CASES = {route + "-" + kind for route in DECISIONS for kind in ("constant-error", "error-coerced")} | {"active_lookup-prefetch-status"}
DIAGNOSTIC_KEYS = {"$message_type", "message", "code", "level", "spans", "children", "rendered"}
ENUMERATION = ("function body check: not all errors may have been reported; rerun with a higher value "
               "for --multiple-errors to find other potential errors in this function")


def need(value, message):
    if not value:
        raise ValueError(message)


def span_parser(path):
    need(path.is_file() and path.resolve() == path and not path.is_symlink(), "ordinary canonical shared span parser")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == SPAN_PARSER_SHA, "unchanged previously reviewed recursive span parser")
    helper = types.ModuleType("journal_private_span_parser")
    helper.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), helper.__dict__)
    # A private module instance, never mutate another campaign's parser globals.
    helper.MACROS = {"producer_observe_" + name + "_body_v1!" for name in DECISIONS}
    helper.DESUGAR = {"desugaring of operator `?`"}
    return helper


def unique_interval(text, expression):
    need(text.count(expression) == 1, "one exact source-bound expected expression")
    start = text.index(expression)
    return start, start + len(expression)


def negative(shared_span_parser, verifier, sources, expected_paths, root, case, status, stdout, stderr):
    need(type(status) is int and status == 1 and case in CASES, "one known case with integer verifier error exit")
    need(root.is_absolute() and root.resolve() == root and set(sources) == set(expected_paths)
         and len(sources) == 38 and {PROOF, BODY} <= set(sources), "complete exact 38-input source projection")
    need(all(type(text) is str and text.isascii() for text in sources.values()), "ASCII source coordinates")
    helper = span_parser(shared_span_parser)
    def parse(raw):
        def invalid(value):
            raise ValueError("nonfinite JSON: " + value)
        return json.loads(raw, object_pairs_hook=helper.strict_object, parse_constant=invalid)
    result = parse(stdout)
    need(type(result) is dict and set(result) == {"verus", "verification-results", "func-details"}
         and result["verus"] == verifier, "exact unfiltered result schema and verifier identity")
    vr = result["verification-results"]
    expected = {"encountered-error": True, "encountered-vir-error": False, "success": False,
                "verified": 164, "errors": 1, "is-verifying-entire-crate": True}
    need(type(vr) is dict and set(vr) == set(expected)
         and all(type(vr[key]) is type(value) and vr[key] == value for key, value in expected.items()),
         "measured full164/1 logical failure, never selected or front-end schema")
    route = case.split("-", 1)[0]
    function = "observe_" + route
    function_key = "context_producer_journal_observers_v1::native_observers::Observations::" + function
    need(type(result["func-details"]) is dict and result["func-details"].get(function_key)
         == {"obligation_proof_notes": [], "failed_proof_notes": []}, "intended actual wrapper appears in full function roster")
    proof, body = sources[PROOF], sources[BODY]
    function_start, function_end = helper.function_interval(proof, function)
    signature_end = proof.index("\n            ensures", function_start)
    need(proof[signature_end - 1] == ")", "named return binder closes the signature")
    # The pinned verifier's function-note span excludes that final binder token.
    signature_end -= 1
    trace_only = case == "active_lookup-prefetch-status"
    expression = ("final(self).calls@ == old(self).calls@.push(QueryCall::ActiveLookup(reference))" if trace_only
                  else "result == " + DECISIONS[route] + "(old(self).versions.journal, reference)")
    expected_primary = unique_interval(proof, expression)
    need(function_start <= expected_primary[0] < expected_primary[1] <= function_end, "expected contract belongs to intended wrapper")
    paths = {str((root / path).resolve()): text for path, text in sources.items()}
    proof_path, body_path = str(root / PROOF), str(root / BODY)
    need(stdout.endswith("\n") and stderr.endswith("\n"), "complete raw verifier streams")
    rows = [parse(line) for line in stderr.splitlines()]
    need(len(rows) == 3, "exact enumeration, one logical error, terminal summary")
    coordinates = []
    for row in rows:
        need(type(row) is dict and set(row) == DIAGNOSTIC_KEYS and row["$message_type"] == "diagnostic"
             and type(row["message"]) is str and type(row["rendered"]) is str and row["code"] is None
             and row["children"] == [] and type(row["spans"]) is list, "closed JSON diagnostic schema")
        coordinates.append([helper.span_source(span, paths, proof_path) for span in row["spans"]])
    note, error, summary = rows
    need(note["level"] == "note" and note["message"] == ENUMERATION and len(note["spans"]) == 1
         and coordinates[0] == [(proof_path, function_start, signature_end)]
         and note["spans"][0]["is_primary"] is True and note["spans"][0]["label"] is None
         and note["spans"][0]["expansion"] is None, "reporting note joins exact intended wrapper signature")
    need(error["level"] == "error" and error["message"] == "postcondition not satisfied" and len(error["spans"]) == 2,
         "one measured postcondition failure")
    exit_span, primary = error["spans"]
    need(exit_span["is_primary"] is False and primary["is_primary"] is True
         and primary["label"] == "failed this postcondition" and primary["expansion"] is None
         and coordinates[1][1] == (proof_path, *expected_primary), "exact case-specific primary contract, not merely same function")
    if trace_only:
        need(body.count("?") == 1 and coordinates[1][0] == (body_path, *unique_interval(body, "?"))
             and exit_span["label"] == "at this exit" and exit_span["expansion"] is not None
             and exit_span["expansion"]["macro_decl_name"] == "desugaring of operator `?`",
             "eager status early exit before ghost append; no result-equality inference")
        desugared = exit_span["expansion"]["span"]
        nested = desugared["expansion"]
        need(helper.span_source(desugared, paths, proof_path) == coordinates[1][0]
             and type(nested) is dict and nested["macro_decl_name"] == "producer_observe_active_lookup_body_v1!"
             and helper.span_source(nested["span"], paths, proof_path)
                 == (proof_path, *unique_interval(proof, "producer_observe_active_lookup_body_v1!(self, reference)")),
             "early exit's recursive expansion joins the actual active-lookup invocation")
    else:
        exit_path, lo, hi = coordinates[1][0]
        need(exit_path == proof_path and function_start <= lo < hi <= function_end and proof[lo:hi] == "result"
             and exit_span["label"] == "at the end of the function body" and exit_span["expansion"] is None,
             "ordinary wrapper return with result-equality failure")
    need(summary["level"] == "error" and summary["message"] == "aborting due to 1 previous error"
         and summary["spans"] == [], "exact terminal one-error summary")
    return {"logical_diagnostic_accepted": True, "verified": 164, "errors": 1, "case": case,
            "affected_wrapper": function_key,
            "boundary": "wrapper-ghost-trace-only" if trace_only else "actual-journal-result-equality",
            "primary": {"path": proof_path, "byte_start": expected_primary[0], "byte_end": expected_primary[1], "expression": expression},
            "actual_result_failure_observed": not trace_only, "inner_query_call_count_proved": False,
            "historical_capture_is_qualified_kill": False, "signed_campaign_qualified": False,
            "source_process_and_fresh_signed_campaign_binding_required": True}
