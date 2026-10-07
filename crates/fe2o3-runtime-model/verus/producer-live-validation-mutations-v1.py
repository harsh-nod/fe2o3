#!/usr/bin/env python3
"""Actual-body live-validation controls; constructing a case proves nothing."""
from pathlib import Path
import hashlib

V = Path("crates/fe2o3-runtime-model/verus")
PROOF = V / "context_producer_journal_composition_v1.rs"
DEFINITIONS = V / "context_live_validation_definitions_v1.rs"
BODY = Path("crates/fe2o3-runtime/src/context/versions/live_validation_bodies.rs")
FORWARD = Path("crates/fe2o3-runtime-model/src/context_queued_writers/query_bodies.rs")
NAMES = (
    "live-missing-reference-error", "live-phase-skipped", "live-provisional-accepted",
    "live-allocation-key-skipped", "live-device-skipped", "live-extent-skipped",
    "live-binding-error-coerced", "phase-lookup-skipped", "phase-predicate-skipped",
    "phase-slot-zero", "phase-error-coerced", "allocation-forward-constant-error",
    "allocation-forward-error-coerced", "allocation-forward-terminal-gate",
    "live-observer-allocation-generation-zero", "live-observer-allocation-local-zero",
    "live-observer-device-generation-zero", "live-observer-device-local-zero",
    "live-observer-extent-zero",
)


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def interval(text, anchor):
    need(text.count(anchor) == 1, "one intended executable body")
    start = text.index("{", text.index(anchor))
    depth, end = 1, start + 1
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    need(depth == 0, "closed intended executable body")
    return start, end


def construct(sources, closure):
    need(len(closure) == len(set(closure)) == 47 and set(closure) <= sources.keys(),
         "complete current concrete closure, never a selected callee")
    rows = {}

    def add(name, path, anchor, before, after, boundary):
        text = sources[path]
        start, end = interval(text, anchor)
        selected = text[start:end]
        need(name not in rows and before != after and selected.count(before) == 1,
             "one nonvacuous implementation-only substitution")
        altered = selected.replace(before, after)
        changed = text[:start] + altered + text[end:]
        need(interval(changed, anchor) == (start, start + len(altered)),
             "mutation stays inside executable body with unchanged contracts")
        rows[name] = {
            "family": "concrete", "name": name, "path": path, "text": changed,
            "root": PROOF, "closure": tuple(closure), "capture_selector": None,
            "origin_selector": None,
            "boundary": boundary if boundary.startswith("ContextQueuedWriterJournalV1::")
            else "concrete_composition::" + boundary,
            "boundary_label": "actual-live-validation-result-equality",
            "original_sha256": sha(text), "before": before, "after": after,
            "implementation_scope": "", "implementation_anchor": anchor,
            "implementation_span": (start, end),
        }

    live = "macro_rules! context_validate_live_body_v1 {"
    for name, before, after in (
        (NAMES[0], ".ok_or(ContextVersionJournalErrorV1::InvalidState)?",
         ".ok_or(ContextVersionJournalErrorV1::InvalidReference)?"),
        (NAMES[1], "$this.validate_phase(reference, AllocationPhaseV1::Live)?;", "let _ = reference;"),
        (NAMES[2], "AllocationPhaseV1::Live", "AllocationPhaseV1::Provisional"),
        (NAMES[3], "if reference.key != expected.key", "if false && reference.key != expected.key"),
        (NAMES[4], "|| actual.device != expected.device", "|| false && actual.device != expected.device"),
        (NAMES[5], "|| actual.byte_extent != expected.byte_extent", "|| false && actual.byte_extent != expected.byte_extent"),
        (NAMES[6], "Err(ContextVersionJournalErrorV1::InvalidAllocationReference)",
         "Err(ContextVersionJournalErrorV1::InvalidState)"),
    ):
        add(name, BODY, live, before, after, "Versions::validate_live")
    phase = "macro_rules! context_validate_phase_body_v1 {"
    for name, before, after in (
        (NAMES[7], "$this.journal.lookup_allocation($reference)?;", "let _ = $reference;"),
        (NAMES[8], "if $this.phases.get", "if false && $this.phases.get"),
        (NAMES[9], ".get($reference.slot)", ".get(0)"),
        (NAMES[10], "Err(ContextVersionJournalErrorV1::InvalidState)",
         "Err(ContextVersionJournalErrorV1::InvalidReference)"),
    ):
        add(name, BODY, phase, before, after, "Versions::validate_phase")
    call = "$owner.inner.lookup_allocation($allocation)"
    for name, after in (
        (NAMES[11], "Err(ContextVersionJournalErrorV1::InvalidState)"),
        (NAMES[12], "match " + call + " { Ok(value) => Ok(value), Err(_) => Err(ContextVersionJournalErrorV1::InvalidReference) }"),
        (NAMES[13], "{ $owner.ensure_usable()?; " + call + " }"),
    ):
        add(name, FORWARD, "macro_rules! queued_allocation_lookup_body_v1 {", call, after,
            "ContextQueuedWriterJournalV1::lookup_allocation")
    call = "self.versions.validate_live(allocation, record)"
    for name, changed in (
        (NAMES[14], "RuntimeAllocationIdV1 { context_generation: 0, local: allocation.local }"),
        (NAMES[15], "RuntimeAllocationIdV1 { context_generation: allocation.context_generation, local: 0 }"),
    ):
        add(name, PROOF, "fn observe_live(", call, call.replace("(allocation,", "(" + changed + ","),
            "Observations::observe_live")
    for name, device, byte_len in (
        (NAMES[16], "RuntimeDeviceIdV1 { context_generation: 0, local: record.device.local }", "record.byte_len"),
        (NAMES[17], "RuntimeDeviceIdV1 { context_generation: record.device.context_generation, local: 0 }", "record.byte_len"),
        (NAMES[18], "record.device", "0"),
    ):
        changed = "self.versions.validate_live(allocation, &AllocationRecordV1 { " \
            "backend_allocation: record.backend_allocation, device: " + device + ", kind: record.kind, " \
            "byte_len: " + byte_len + ", journal: record.journal })"
        add(name, PROOF, "fn observe_live(", call, changed, "Observations::observe_live")
    need(tuple(rows) == NAMES and len({(r["path"], r["text"]) for r in rows.values()}) == len(NAMES),
         "exact complete distinct 19-case live roster")
    return rows
