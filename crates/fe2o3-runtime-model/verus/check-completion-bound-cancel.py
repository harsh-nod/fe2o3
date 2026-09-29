#!/usr/bin/env python3
"""Qualify exact bound cancellation, not source rollback or native currentness."""

import hashlib
from pathlib import Path
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/bound_cancel_body.rs")
PROOF = V / "completion_bound_cancel_v1.rs"
FUNCTIONS = ("validate_packet_count", "validate_bound", "validate_retention",
             "require_unpinned", "cancel_bound_retaining")


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique cancellation mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    count, bound, retention, pins, cancel = FUNCTIONS
    add("count-accepts-zero", "if $n == 0", "if false", count)
    add("count-accepts-overflow", "if $n > COMPLETION_SIGNAL_CAPACITY_V1", "if false", count)
    add("bound-ignores-packet", "$retention.last_packet_id.is_some()", "false", bound)
    add("retention-ignores-queue", "$retention.queue != $owner.queue || ", "", retention)
    add("retention-ignores-mapping", " || $retention.signal_mapping != $owner.signal_mapping", "", retention)
    add("retention-ignores-generation", "record.generation != $slot.generation", "false", retention)
    add("retention-ignores-phase", "record.phase != $expected", "false", retention)
    add("retention-ignores-dispatch-queue", "$retention.dispatches[$i].queue != $retention.queue", "false", retention)
    add("retention-accepts-zero-dispatch", "$retention.dispatches[$i].dispatch_generation == 0", "false", retention)
    add("retention-ignores-code-vm", "$retention.dispatches[$i].code.allocation.vm != $retention.queue.vm", "false", retention)
    add("retention-ignores-kernarg-vm", "$retention.dispatches[$i].kernarg.allocation.vm != $retention.queue.vm", "false", retention)
    add("retention-accepts-duplicate", "|| $seen[$word] & $bit != 0", "|| false", retention)
    add("retention-omits-mark", "$seen[$word] |= $bit;", "$seen[$word] |= 0;", retention)
    add("retention-collapses-words", "let $word = $slot.index as usize / 64;", "let $word = 0usize;", retention)
    add("retention-skips-last", "$($initial)*\n            while $i < $n", "$($initial)*\n            while $i + 1 < $n", retention)
    add("unpinned-ignores-events", "record.event_pins != 0", "false", pins)
    add("unpinned-ignores-readers", "record.native_reader_pins != 0", "false", pins)
    add("unpinned-wrong-slot", "slot: slot.index,", "slot: 0,", pins)
    bound_check = "if let Err(error) = $owner.validate_bound(&$retention) {\n                return Err((error, $retention));\n            }"
    pin_check = "if let Err(error) = $owner.require_unpinned(&$retention.slots) {\n                return Err((error, $retention));\n            }"
    add("cancel-omits-validation", bound_check, "", cancel)
    add("cancel-omits-pins", pin_check, "", cancel)
    write = "$owner.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Available;"
    add("cancel-writes-neighbor", write, "$owner.slots[0].phase = CompletionSlotPhaseV1::Available;", cancel)
    add("cancel-resets-generation", write, write + "\n                $owner.slots[slot.index as usize].generation = 0;", cancel)
    add("cancel-substitutes-retention", bound_check,
        "if let Err(error) = $owner.validate_bound(&$retention) {\n"
        "                let mut returned = $retention; returned.last_packet_id = Some(1);\n"
        "                return Err((error, returned));\n            }", cancel)
    need(len(cases) == len(set(cases.values())) == 23
         and all(text != body for text, _ in cases.values()), "distinct executable mutants")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in FUNCTIONS}, "exact cancellation mutation function")
    functions = FUNCTIONS if focus is None else (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function completion_bound_cancel_v1::"
          + ("" if name == "validate_packet_count" else "CompletionSignalArenaOwnerV1::")
          + name + " (selected functions)" for name in functions},
    })


def campaign():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("bound_cancel_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES = [BODY, PROOF]
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=25)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-bound-cancel.py"))
    campaign().main()
