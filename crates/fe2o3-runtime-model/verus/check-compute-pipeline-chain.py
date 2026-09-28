#!/usr/bin/env python3
"""Qualify shared pipeline lifecycle bodies and metadata composition, not native authority."""

import hashlib
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-runtime/src/kfd_backend/compute_pipeline_lifecycle_body.rs")
PROOF = V / "compute_pipeline_chain_v1.rs"


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique lifecycle mutation site: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    add("scan-wrong-epoch", "entry.identity.logical_epoch == $epoch", "entry.identity.logical_epoch >= $epoch", "first_epoch")
    add("scan-wrong-index", "return Some($i);", "return Some(0);", "first_epoch")
    add("scan-skips-last", "if let Some(entry) = $slots[$i].entry.as_ref() {",
        "if $i == $slots.len() - 1 { return None; }\n                if let Some(entry) = $slots[$i].entry.as_ref() {", "first_epoch")
    add("promotion-ignores-stage", "if $staged.is_some() { return None; }",
        "if $staged.is_some() && *$live == 0 { return None; }", "take_frontier")
    add("promotion-omits-live", "$live.checked_sub(1)", "$live.checked_sub(0)", "take_frontier")
    add("promotion-reuses-frontier", "$epoch.checked_add(1)", "Some($epoch)", "take_frontier")
    add("promotion-substitutes-phase", "Some(($entry.phase, $entry.active))",
        "Some((RuntimeComputePipelinePhaseV1::Published, $entry.active))", "take_frontier")
    add("promotion-substitutes-owner", "Some(($entry.phase, $entry.active))",
        "Some(($entry.phase, ActiveSubmissionV1 { id: 0, ..$entry.active }))", "take_frontier")
    add("promotion-resets-generation", "let $entry = $slots[$index].entry.take()",
        "$slots[$index].generation = 0;\n            let $entry = $slots[$index].entry.take()", "take_frontier")
    add("quarantine-wrong-phase", "entry.phase = RuntimeComputePipelinePhaseV1::Quarantined;",
        "entry.phase = RuntimeComputePipelinePhaseV1::Published;", "quarantine")
    add("quarantine-skips-last", "if let Some(entry) = $slots[$i].entry.as_mut() {",
        "if $i == $slots.len() - 1 { break; }\n                if let Some(entry) = $slots[$i].entry.as_mut() {", "quarantine")
    add("quarantine-substitutes-owner", "entry.phase = RuntimeComputePipelinePhaseV1::Quarantined;",
        "entry.phase = RuntimeComputePipelinePhaseV1::Quarantined; entry.active.id = 0;", "quarantine")
    need(len(cases) == len(set(cases.values())) == 12 and all(text != body for text, _ in cases.values()),
         "distinct lifecycle mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    functions = ("first_epoch", "take_frontier", "quarantine")
    if focus is not None:
        need(focus in {"*" + name for name in functions}, "exact lifecycle mutation function")
        functions = (focus[1:],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function compute_pipeline_chain_v1::" + name + " (selected functions)"
          for name in functions},
    })


def campaign():
    # Reuse the signed-source, relocation, tool-closure and strict diagnostic
    # controller unchanged. The original publication campaign stays reproducible.
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("pipeline_chain_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES = [*module.FILES, BODY, PROOF]
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=44)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    campaign().main()
