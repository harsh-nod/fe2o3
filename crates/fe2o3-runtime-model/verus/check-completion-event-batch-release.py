#!/usr/bin/env python3
"""Qualify full host-ledger batch release with two explicit trusted std contracts."""

import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/batch_event_release_body.rs")
SINGLE = BODY.with_name("event_release_body.rs")
PROOF = V / "completion_event_batch_release_v1.rs"
CONTRACTS = V / "completion_hash_reserve_contracts_v1.rs"
CONTRACTS_SHA = "a3c6d3bd3f022470323da7bf44e4148097e9691fb055634a85f161cf12199a49"
FILES = [BODY, SINGLE, PROOF, CONTRACTS]
FUNCTION = "*release_compute_event_batch"
BOUNDS_ERROR = "precondition not met: index in bounds for this access"


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    # A fail-closed source guardrail, not equivalent to Verus --no-cheating.
    need(set(inputs) == set(FILES), "exact batch proof closure")
    need(hashlib.sha256(inputs[CONTRACTS].encode()).hexdigest() == CONTRACTS_SHA,
         "exact two trusted standard-library declarations")
    forbidden = r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b"
    for path in FILES:
        if path != CONTRACTS:
            need(not re.search(forbidden, inputs[path]), "additional trust construct: " + str(path))
    proof = inputs[PROOF]
    for statement in (
        'include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");',
        'include!("../../fe2o3-kfd/src/queue_completion/batch_event_release_body.rs");',
        '#[path = "completion_hash_reserve_contracts_v1.rs"]\nmod reserve_contracts;',
    ):
        need(proof.count(statement) == 1, "exact proof input: " + statement)
        proof = proof.replace(statement, "")
    source_inputs = r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b"
    need(not re.search(source_inputs, proof), "no extra proof source input")
    for path in (BODY, SINGLE, CONTRACTS):
        need(not re.search(source_inputs, inputs[path]),
             "no transitive source input: " + str(path))


def mutations(body):
    cases = {}

    def add(name, old, new, occurrence=None):
        need(body.count(old) == (1 if occurrence is None else 2), "exact mutation site: " + name)
        offset = body.find(old) if occurrence in (None, 0) else body.find(old, body.find(old) + len(old))
        cases[name] = (body[:offset] + new + body[offset + len(old):], FUNCTION)

    add("ignores-ready", "if let Err(error) = $owner.require_ready() {", "if let Err(error) = Ok::<(), Gfx942CompletionErrorV1>(()) {")
    for position, label in enumerate(("early", "late")):
        add(label + "-ignores-reservation-failure", "if $reservation.is_err()", "if false", position)
        add(label + "-fabricates-allocation-failure", "if $reservation.is_err()", "if true", position)
    add("accepts-duplicate", "if !$ids.insert(event.event_id)", "if false")
    add("ignores-active", "$owner.validate_active_event(event)", "Ok::<(), Gfx942CompletionErrorV1>(())")
    add("ignores-live", "$owner.validate_live_occurrence(event.exact)", "Ok::<(), Gfx942CompletionErrorV1>(())")
    add("accepts-zero-pin", "$owner.slots[event.exact.slot.index as usize].event_pins == 0", "false")
    add("validates-only-first", "while $i < $events.len()", "while $i < $events.len() && $i == 0")
    add("omits-budget-check", "if $events.len() > 1", "if false")
    add("reserves-single-budget", "if $events.len() > 1", "if $events.len() > 0")
    add("budgets-only-first", "while $j < $events.len()", "while $j < $events.len() && $j == 0")
    add("aliases-budget-keys", "let $index = $events[$j].exact.slot.index;", "let $index = 0u32;")
    add("reads-neighbor-budget", "let pins = $owner.slots[$index as usize].event_pins;", "let pins = $owner.slots[0].event_pins;")
    add("omits-budget-debit", "$available.checked_sub(1)", "$available.checked_sub(0)")
    add("doubles-budget-debit", "$available.checked_sub(1)", "$available.checked_sub(2)")
    add("omits-budget-write", "*$available = next;", "let _ = next;")
    add("omits-ledger-removal", "$owner.dependency_ledger.events.remove(&$event.event_id)", "Some($event.exact)")
    add("removes-wrong-event", "$owner.dependency_ledger.events.remove(&$event.event_id)", "$owner.dependency_ledger.events.remove(&0)")
    add("omits-pin-decrement", "$owner.slots[$event.exact.slot.index as usize].event_pins -= 1;", "$owner.slots[$event.exact.slot.index as usize].event_pins -= 0;")
    add("returns-wrong-count", "$finish!(Ok($released))", "$finish!(Ok(0))")
    add("wrong-duplicate-error", "Gfx942CompletionErrorV1::DuplicateDependency", "Gfx942CompletionErrorV1::StaleEventOccurrence")
    add("substitutes-refusal-roster", "Gfx942CompletionErrorV1::DuplicateDependency, $events", "Gfx942CompletionErrorV1::DuplicateDependency, Vec::new()")
    need(len(cases) == len(set(cases.values())) == 24, "distinct executable mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus in (None, FUNCTION), "exact batch-release selector")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | {BOUNDS_ERROR}, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function completion_event_batch_release_v1::CompletionSignalArenaOwnerV1::release_compute_event_batch (selected functions)",
    })


def campaign():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    audit({path: (ROOT / path).read_text() for path in FILES})
    # This campaign deliberately uses a different trust profile. Verus rejects
    # the two std contracts under --no-cheating, including imported contracts.
    source = raw.decode()
    need(source.count('"--no-cheating", ') == 1, "exact verifier-option adaptation")
    source = source.replace('"--no-cheating", ', '')
    module = types.ModuleType("event_batch_release_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(source, module.__file__, "exec"), module.__dict__)
    module.FILES = FILES
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=28)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-event-batch-release.py"))
    campaign().main()
