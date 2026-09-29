#!/usr/bin/env python3
"""Qualify dispatch cancellation payload refinement, not native rollback authority."""

import hashlib
from pathlib import Path
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs")
PROOF = V / "dispatch_epoch_cancel_v1.rs"
FUNCTIONS = ("ensure_not_poisoned", "require_identity", "expected_roster", "cancel_epoch")
BOUNDS_ERROR = "precondition not met: index in bounds for this access"


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique dispatch mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    poison, identity, roster, cancel = FUNCTIONS
    add("ignores-poison", "if $owner.poisoned", "if false", poison)
    add("wrong-poison-error", "Err(Gfx942DispatchBindingErrorV1::Poisoned)",
        "Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)", poison)
    add("ignores-recipe", "$identity.recipe_occurrence != $owner.recipe_occurrence", "false", identity)
    add("ignores-queue", "Some($identity.queue) != $owner.recipe_queue", "false", identity)
    add("accepts-out-of-bounds", "$identity.slot_index as usize >= $owner.slots.len()", "false", identity)
    # Return a safely wrong success, so the negative is a local postcondition
    # failure rather than an unauthenticated primary span inside vstd's Vec spec.
    text, focus = cases["accepts-out-of-bounds"]
    site = "            let slot = $owner.slots[$identity.slot_index as usize];"
    need(text.count(site) == 1, "exact false-success site")
    cases["accepts-out-of-bounds"] = (text.replace(site,
        "            if $identity.slot_index as usize >= $owner.slots.len() { return Ok(()); }\n" + site), focus)
    add("ignores-slot-generation", "slot.slot_generation != $identity.slot_generation", "false", identity)
    add("ignores-phase", "slot.phase != $expected", "false", identity)
    add("ignores-dispatch-generation", "dispatch_generation == $identity.dispatch_generation", "true", roster)
    add("looks-up-neighbor", ".get($identity.slot_index as usize)", ".get(0)", roster)
    add("substitutes-roster", "=> Ok(expected_roster)",
        "=> Ok(CompletionDispatchRosterV1 { packet_count: 0, ..expected_roster })", roster)
    add("ignores-identity-refusal", "$owner.require_identity(\n", "let _ = $owner.require_identity(\n", cancel)
    # The ignored-result mutation must also remove propagation, not merely bind ().
    text, focus = cases["ignores-identity-refusal"]
    site = "            )?;\n            $owner.slots"
    need(text.count(site) == 1, "exact propagation site")
    cases["ignores-identity-refusal"] = (text.replace(site, "            );\n            $owner.slots"), focus)
    write = "$owner.slots[$identity.slot_index as usize].phase = DispatchEpochPhaseV1::Vacant;"
    add("omits-cancel", write, "", cancel)
    add("cancels-neighbor", write, "$owner.slots[0].phase = DispatchEpochPhaseV1::Vacant;", cancel)
    add("resets-slot-generation", write, write + "\n            $owner.slots[$identity.slot_index as usize].slot_generation = 0;", cancel)
    for name, field, value in (
        ("rewinds-dispatch", "next_generation", "$identity.dispatch_generation"),
        ("forgets-queue", "recipe_queue", "None"),
        ("forgets-recipe", "recipe_occurrence", "0"),
        ("forgets-recycled", "recycled_generation", "None"),
        ("forgets-predecessor", "predecessor_detached_generation", "None"),
        ("poisons-success", "poisoned", "true"),
    ):
        add(name, write, write + f"\n            $owner.{field} = {value};", cancel)
    add("rejects-valid", "$owner.require_identity(\n",
        "if !$owner.poisoned { return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration); }\n            $owner.require_identity(\n", cancel)
    add("mutates-before-refusal", "$owner.require_identity(\n",
        "$owner.next_generation = 0;\n            $owner.require_identity(\n", cancel)
    need(len(cases) == len(set(cases.values())) == 22
         and all(text != body for text, _ in cases.values()), "distinct executable mutants")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in FUNCTIONS}, "exact dispatch mutation function")
    functions = FUNCTIONS if focus is None else (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | {BOUNDS_ERROR}, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function dispatch_epoch_cancel_v1::DispatchGenerationOwnerV1::"
          + name + " (selected functions)" for name in functions},
    })


def campaign():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("dispatch_cancel_campaign")
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
    runpy.run_path(str(ROOT / V / "test-dispatch-epoch-cancel.py"))
    campaign().main()
