#!/usr/bin/env python3
"""Qualify exact host epoch reservation, not template/native-resource authority."""
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
PROOF = V / "dispatch_epoch_reserve_v1.rs"
CANCEL = V / "dispatch_epoch_cancel_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/epoch_reserve_body.rs")
CANCEL_BODY = BODY.with_name("epoch_cancel_body.rs")
FILES = [PROOF, CANCEL, BODY, CANCEL_BODY]
EDGES = {
    PROOF: [('include!("dispatch_epoch_cancel_v1.rs");', CANCEL),
            ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_reserve_body.rs");', BODY)],
    CANCEL: [('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");', CANCEL_BODY)],
}
METHODS = {"slots": "FixedDispatchCapacityProfileV1", "preflight_reservation": "DispatchGenerationOwnerV1",
           "reserve": "DispatchGenerationOwnerV1"}


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    need(set(inputs) == set(FILES), "exact four-file epoch closure")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source), "no added trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(source.count(statement) == 1, "exact include edge")
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra input")
    need(reached == set(FILES), "reachable closure")


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique reserve mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    add("default-capacity", "Self::Default64 => GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1", "Self::Default64 => 63", "slots")
    add("scaled-capacity", "Self::Qualification1024 => 1024", "Self::Qualification1024 => 1023", "slots")
    preflight = "preflight_reservation"
    for name, old, new in (
        ("ignores-poison", "$owner.ensure_not_poisoned()?;", ""),
        ("ignores-queue", "if bound != $queue", "if false"),
        ("skips-first-slot", "let mut $index = 0;", "let mut $index = 1;"),
        ("accepts-occupied", "slot.phase == DispatchEpochPhaseV1::Vacant", "true"),
        ("forgets-vacancy", "$vacant = true;", "$vacant = false;"),
        ("reuses-slot-generation", "slot.slot_generation.checked_add(1)", "slot.slot_generation.checked_add(0)"),
        ("reuses-exhausted-slot", "slot.slot_generation.checked_add(1)", "Some(1u64)"),
        ("returns-neighbor", "return Ok(($index, dispatch_generation, slot_generation));", "return Ok((0, dispatch_generation, slot_generation));"),
        ("substitutes-dispatch-generation", "return Ok(($index, dispatch_generation, slot_generation));", "return Ok(($index, 0, slot_generation));"),
        ("ignores-dispatch-overflow", "dispatch_generation.checked_add(1)", "dispatch_generation.checked_add(0)"),
        ("generation-before-capacity", "let mut $vacant = false;", "if $owner.next_generation == u64::MAX { return Err(Gfx942DispatchBindingErrorV1::GenerationExhausted); }\n            let mut $vacant = false;"),
    ):
        add(name, old, new, preflight)
    for name, old, new in (
        ("ignores-roster-queue", "$roster.queue != $queue", "false"),
        ("ignores-roster-generation", "$roster.dispatch_generation != dispatch_generation", "false"),
        ("accepts-empty-roster", "$roster.packet_count == 0", "false"),
        ("accepts-scaled-batch", "&& $roster.packet_count != 1", "&& false"),
        ("rejects-full-width-index", "slot_index > u16::MAX as usize", "slot_index >= u16::MAX as usize"),
        ("truncates-index", "slot_index > u16::MAX as usize", "false"),
        ("rewinds-generation", "$owner.next_generation = next_generation;", "$owner.next_generation = dispatch_generation;"),
        ("forgets-queue-binding", "$owner.recipe_queue = Some($queue);", ""),
        ("writes-neighbor", "$owner.slots[slot_index] = DispatchEpochSlotV1", "$owner.slots[0] = DispatchEpochSlotV1"),
        ("substitutes-roster", "expected_roster: $roster,", "expected_roster: CompletionDispatchRosterV1 { packet_count: 0, ..$roster },"),
        ("substitutes-recipe", "recipe_occurrence: $owner.recipe_occurrence,", "recipe_occurrence: 0,"),
        ("premature-mutation", "let (slot_index, dispatch_generation, slot_generation) =", "$owner.next_generation = 0;\n            let (slot_index, dispatch_generation, slot_generation) ="),
        ("poisons-success", "$owner.next_generation = next_generation;", "$owner.next_generation = next_generation; $owner.poisoned = true;"),
        ("forgets-recycled", "$owner.next_generation = next_generation;", "$owner.next_generation = next_generation; $owner.recycled_generation = None;"),
        ("forgets-predecessor", "$owner.next_generation = next_generation;", "$owner.next_generation = next_generation; $owner.predecessor_detached_generation = None;"),
    ):
        add(name, old, new, "reserve")
    need(len(cases) == len(set(cases.values())) == 28, "distinct epoch mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in METHODS}, "exact epoch selector")
    names = METHODS if focus is None else [focus[1:]]
    indexed = focus in {"*preflight_reservation", "*reserve"}
    return types.SimpleNamespace(
        LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({"precondition not met: index in bounds for this access", "possible arithmetic underflow/overflow"} if indexed else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            *{"verifying root module, function dispatch_epoch_reserve_v1::" + METHODS[name] + "::" + name
              + " (selected functions)" for name in names}})


def campaign():
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("epoch_reserve_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=23)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-dispatch-epoch-reserve.py"))
    campaign().main()
