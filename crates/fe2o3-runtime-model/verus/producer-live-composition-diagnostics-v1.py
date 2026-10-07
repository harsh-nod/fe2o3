#!/usr/bin/env python3
"""Exact new live-boundary profile over the unchanged source-span classifier."""
import hashlib
import importlib.util
import json
from pathlib import Path

BASE = Path(__file__).resolve().parent
V = Path("crates/fe2o3-runtime-model/verus")
PROOF = V / "context_producer_journal_composition_v1.rs"
DEFINITIONS = V / "context_live_validation_definitions_v1.rs"
BODY = Path("crates/fe2o3-runtime/src/context/versions/live_validation_bodies.rs")
FORWARD = Path("crates/fe2o3-runtime-model/src/context_queued_writers/query_bodies.rs")
BASE_SHA = "681cbc9b935eacbe10a24112818e5640631bfa1300d0dd4f51c0c8fa0780ec03"
DIAGNOSTIC_COUNTS = {
    "live-missing-reference-error": (1, 0),
    "live-phase-skipped": (2, 1),
    "live-provisional-accepted": (2, 1),
    "live-allocation-key-skipped": (1, 0),
    "live-device-skipped": (1, 0),
    "live-extent-skipped": (1, 0),
    "live-binding-error-coerced": (1, 0),
    "phase-lookup-skipped": (2, 1),
    "phase-predicate-skipped": (1, 0),
    "phase-slot-zero": (2, 1),
    "phase-error-coerced": (1, 0),
    "allocation-forward-constant-error": (1, 0),
    "allocation-forward-error-coerced": (1, 0),
    "allocation-forward-terminal-gate": (1, 0),
    "live-observer-allocation-generation-zero": (1, 0),
    "live-observer-allocation-local-zero": (1, 0),
    "live-observer-device-generation-zero": (1, 0),
    "live-observer-device-local-zero": (1, 0),
    "live-observer-extent-zero": (1, 0),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def load_base():
    path = BASE / "producer-journal-composition-diagnostics-v1.py"
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "unchanged concrete diagnostic classifier")
    spec = importlib.util.spec_from_file_location("live_base_diagnostics", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def negative(shared_parser, verifier, sources, expected_paths, root, case, status, stdout, stderr):
    base = load_base()
    base.COUNTS = {"leaf": 42, "conditional": 64, "concrete": 218}
    base.LENGTHS = {"leaf": 7, "conditional": 9, "concrete": 47}
    if case["boundary_label"] != "actual-live-validation-result-equality":
        return base.negative(shared_parser, verifier, sources, expected_paths, root, case, status, stdout, stderr)
    need(type(status) is int and status == 1, "only normal logical rejection")
    need(case["family"] == "concrete" and Path(case["root"]) == PROOF
         and case["capture_selector"] is None, "whole concrete root only")
    need(set(sources) == set(expected_paths) == {Path(p) for p in case["closure"]}
         and len(sources) == 47, "all 47 exact source inputs")
    need(stdout.endswith("\n") and stderr.endswith("\n"), "complete diagnostic streams")
    helper = base.parser(shared_parser)
    helper.MACROS |= {"context_validate_phase_body_v1!", "context_validate_live_body_v1!",
                      "queued_allocation_lookup_body_v1!"}
    report = base.parse(helper, stdout)
    need(type(report) is dict and set(report) == {"verus", "verification-results", "func-details"}
         and report["verus"] == verifier, "complete exact verifier report")
    expected = {"encountered-error": True, "encountered-vir-error": False, "success": False,
                "verified": 217, "errors": 1, "is-verifying-entire-crate": True}
    actual = report["verification-results"]
    need(type(actual) is dict and set(actual) == set(expected)
         and all(type(actual[k]) is type(v) and actual[k] == v for k, v in expected.items()),
         "exact whole-root 217/1 logical rejection, no frontend or resource failures")
    boundary = case["boundary"]
    targets = {
        "concrete_composition::Versions::validate_live": (DEFINITIONS, "validate_live",
            "context_validate_live_body_v1", BODY,
            "out == live_allocation_decision(self.journal, self.phases@, id, *record)"),
        "concrete_composition::Versions::validate_phase": (DEFINITIONS, "validate_phase",
            "context_validate_phase_body_v1", BODY,
            "out == live_phase_decision(self.journal, self.phases@, reference, phase)"),
        "ContextQueuedWriterJournalV1::lookup_allocation": (DEFINITIONS, "lookup_allocation",
            "queued_allocation_lookup_body_v1", FORWARD,
            "out == allocation_lookup_decision_v1(self.inner.stable.journal, allocation)"),
        "concrete_composition::Observations::observe_live": (PROOF, "observe_live", None, None,
            "out == live_allocation_decision(old(self).versions.journal,\n"
            "                old(self).versions.phases@, allocation, *record)"),
    }
    need(boundary in targets, "one reviewed live-result boundary")
    path, method, macro, macro_path, expression = targets[boundary]
    function = helper.function_interval(sources[path], method)
    ranges = {str(root / path): function}
    if macro is not None:
        lo, hi = base.unique(sources[macro_path], "macro_rules! " + macro + " {")
        ranges[str(root / macro_path)] = (lo, helper.balanced_end(sources[macro_path], hi - 1))
    details = report["func-details"]
    need(type(details) is dict and 0 < len(details) <= 1024
         and PROOF.stem + "::" + boundary in details
         and all(type(name) is str and value == {"obligation_proof_notes": [], "failed_proof_notes": []}
                 for name, value in details.items()), "exact intended function and finite note schema")
    rows = [base.parse(helper, line) for line in stderr.splitlines()]
    errors, notes, summaries = [], [], []
    mapping = {str(root / p): text for p, text in sources.items()}
    proof_path = str(root / PROOF)
    for row in rows:
        need(not summaries, "no diagnostic after terminal summary")
        need(type(row) is dict
             and set(row) == {"$message_type", "message", "code", "level", "spans", "children", "rendered"}
             and row["$message_type"] == "diagnostic" and type(row["message"]) is str
             and type(row["rendered"]) is str and row["code"] is None and row["children"] == []
             and type(row["spans"]) is list, "unchanged strict diagnostic schema")
        positions = [helper.span_source(span, mapping, proof_path) for span in row["spans"]]
        if row["level"] == "error" and row["message"] == "postcondition not satisfied":
            need(row["spans"] and all(helper.in_family(position, ranges) for position in positions),
                 "all actual error locations belong to the intended function and macro")
            errors.append(row)
        elif row["level"] == "note" and row["message"] in helper.ENUMERATION:
            need(row["spans"] and all(span["is_primary"] for span in row["spans"])
                 and all(helper.in_family(position, ranges) for position in positions),
                 "exact source-bound intended function enumeration")
            notes.append((row["message"], tuple(positions)))
        elif row["level"] == "error" and row["message"] == (
                "aborting due to " + str(len(errors)) + " previous error" + ("s" if len(errors) != 1 else "")):
            need(row["spans"] == [] and errors, "exact terminal logical error count")
            summaries.append(row)
        else:
            raise ValueError("unreviewed diagnostic in exact live profile: " + row["message"])
    need(case["name"] in DIAGNOSTIC_COUNTS
         and (len(errors), len(notes)) == DIAGNOSTIC_COUNTS[case["name"]]
         and len(notes) == len(set(notes)) and len(summaries) == 1,
         "exact calibrated error/enumeration counts and complete summary")
    expected_span = (str(root / path), *base.unique(sources[path], expression))
    for row in errors:
        primaries = [span for span in row["spans"] if span["is_primary"]]
        need(len(primaries) == 1 and primaries[0]["label"] == "failed this postcondition"
             and primaries[0]["expansion"] is None
             and helper.span_source(primaries[0], mapping, str(root / PROOF)) == expected_span,
             "exact actual live-result contract is every failed primary")
    return {"logical_diagnostic_accepted": True, "verified": 217, "errors": 1,
            "logical_messages": [row["message"] for row in errors],
            "family": "concrete", "case": case["name"], "full_root": True,
            "qualified_kill": False, "signed_campaign_qualified": False,
            "historical_capture_is_qualified_kill": False,
            "primary_expression": expression, "source_and_process_binding_required": True}
