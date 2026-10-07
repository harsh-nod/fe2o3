#!/usr/bin/env python3
"""Qualify the shared settlement body, not native receipt authentication."""

import hashlib
from pathlib import Path
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
BODY = Path("crates/fe2o3-runtime/src/kfd_backend/ordered_publication_settlement_body.rs")
PROOF = V / "ordered_publication_settlement_v1.rs"


def need(value, message):
    if not value:
        raise ValueError(message)


def mutations(body):
    cases = {}

    def add(name, old, new):
        need(body.count(old) == 1, "unique settlement mutation site: " + name)
        cases[name] = (body.replace(old, new), "*settle_returned")

    add("retry-keeps-entry", "$pipeline.withdraw_publication_v1($identity)",
        "$pipeline.entry_mut_v1($identity)")
    lookup = "let $entry = match $pipeline.entry_mut_v1($identity) {"
    add("confirm-before-classification", lookup,
        "let _ = $pipeline.confirm_publication_v1($identity);\n            " + lookup)
    add("publication-skips-confirmation", "$pipeline.confirm_publication_v1($identity).is_err()", "false")
    add("publication-discards-receipt", "let observation = OrderedPublicationObservationV1 {",
        "$active.execution = None;\n            let observation = OrderedPublicationObservationV1 {")
    add("observation-substitutes-id", "id: $id,", "id: 0,")
    add("publication-returns-no-observation", "Ok(Some(observation))", "Ok(None)")
    add("lookup-substitutes-submission", lookup,
        "let $entry = match $pipeline.entry_mut_v1(RuntimeComputePipelineIdentityV1 { submission: 0, ..$identity }) {")
    timestamp = "$fields!($entry.active).published_at = $now;"
    need(body.count(timestamp) == 1, "unique timestamp assignment")
    anchor = "$($after_confirm)*"
    need(body.count(anchor) == 1, "unique timestamp relocation anchor")
    delayed = body.replace(timestamp, "")
    delayed = delayed.replace(anchor, anchor + "\n"
        "            let $entry = $pipeline.entry_mut_v1($identity).unwrap();\n            " + timestamp)
    cases["timestamp-after-confirmation"] = (delayed, "*settle_returned")
    add("publication-omits-duration", "$fields!($entry.active).performance.publication = $elapsed;",
        "let _ = $elapsed;")
    add("missing-identity-wrong-error", "None => return Err(OrderedPublicationSettlementErrorV1::MissingIdentity),",
        "None => return Err(OrderedPublicationSettlementErrorV1::NoOutcome),")
    need(len(cases) == len(set(cases.values())) == 10 and all(text != body for text, _ in cases.values()),
         "distinct settlement mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus in (None, "*settle_returned"), "exact settlement mutation function")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        "verifying root module, function ordered_publication_settlement_v1::settle_returned (selected functions)",
    })


def campaign():
    # Keep the authenticated controller and strict failure classifier unchanged.
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("ordered_settlement_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES = [*module.FILES, BODY, PROOF]
    module.BODY = BODY
    module.PROOF = PROOF
    module.EXPECTED = dict(module.EXPECTED, verified=37)
    module.mutations = mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    campaign().main()
