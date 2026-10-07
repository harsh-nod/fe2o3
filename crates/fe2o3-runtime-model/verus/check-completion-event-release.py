#!/usr/bin/env python3
"""Qualify exact host event release, not session-wrapper or GPU behavior."""

import hashlib
from pathlib import Path
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/event_release_body.rs")
PROOF = V / "completion_event_release_v1.rs"
FUNCTIONS = ("require_ready", "validate_active_event", "validate_live_occurrence",
             "event_release_preflight", "release_compute_event")


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique event-release mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    add("ready-accepts-poison", "$owner.phase == CompletionOwnerPhaseV1::Ready", "true", "require_ready")
    add("active-accepts-substitution", "$owner.dependency_ledger.events.get(&$event.event_id) != Some(&$event.exact)",
        "$owner.dependency_ledger.events.get(&$event.event_id).is_none()", "validate_active_event")
    add("live-ignores-queue", "$exact.queue != $owner.queue || ", "", "validate_live_occurrence")
    add("live-ignores-mapping", " || $exact.signal_mapping != $owner.signal_mapping", "", "validate_live_occurrence")
    add("live-ignores-generation", "record.generation != $exact.slot.generation", "false", "validate_live_occurrence")
    add("live-ignores-batch", "=> batch_id == $exact.batch_id,", "=> true,", "validate_live_occurrence")
    add("live-accepts-available", "CompletionSlotPhaseV1::Available => false,",
        "CompletionSlotPhaseV1::Available => true,", "validate_live_occurrence")
    add("preflight-omits-ready", "$owner.require_ready()?;", "", "event_release_preflight")
    add("preflight-omits-active", "$owner.validate_active_event($event)?;", "", "event_release_preflight")
    add("preflight-keeps-pin", ".checked_sub(1)", ".checked_sub(0)", "event_release_preflight")
    add("release-removes-other-event", ".events.remove(&$event.event_id)", ".events.remove(&0u64)", "release_compute_event")
    add("release-writes-other-slot", "$owner.slots[$event.exact.slot.index as usize].event_pins = next;",
        "$owner.slots[0].event_pins = next;", "release_compute_event")
    add("release-clears-reader-pins", "Ok(Gfx942ComputeEventReleaseObservationV1)",
        "$owner.slots[$event.exact.slot.index as usize].native_reader_pins = 0;\n"
        "            Ok(Gfx942ComputeEventReleaseObservationV1)", "release_compute_event")
    add("release-rewinds-event-id", "Ok(Gfx942ComputeEventReleaseObservationV1)",
        "$owner.dependency_ledger.next_event_id = 0;\n"
        "            Ok(Gfx942ComputeEventReleaseObservationV1)", "release_compute_event")
    add("refusal-substitutes-token", "Err(error) => return Err((error, $event)),",
        "Err(error) => return Err((error, Gfx942ComputeEventOccurrenceV1 { event_id: 0, ..$event })),",
        "release_compute_event")
    need(len(cases) == len(set(cases.values())) == 15
         and all(text != body for text, _ in cases.values()), "distinct executable mutants")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in FUNCTIONS}, "exact release mutation function")
    functions = FUNCTIONS if focus is None else (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function completion_event_release_v1::CompletionSignalArenaOwnerV1::"
          + name + " (selected functions)" for name in functions},
    })


def campaign():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("event_release_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES = [BODY, PROOF]
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=16)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-event-release.py"))
    campaign().main()
