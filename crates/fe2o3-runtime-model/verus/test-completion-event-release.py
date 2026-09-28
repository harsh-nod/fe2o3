#!/usr/bin/env python3
"""Synthetic classifier and source-wiring smoke checks, not solver evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-event-release.py")))
campaign = runner["campaign"]()
need = runner["need"]
body = (runner["ROOT"] / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 15, "distinct release mutation roster")
need({focus for _, focus in cases.values()} == {"*" + name for name in runner["FUNCTIONS"]},
     "all changed helpers selected")
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text, _ in cases.values()), "no mutation bypass")
for changed in (body + body, body.replace(".checked_sub(1)", ".checked_sub(0)"),
                body.replace("=> batch_id == $exact.batch_id,", "=> true,")):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("stale or ambiguous mutation site accepted")

root = runner["ROOT"]
parent = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
child = (root / "crates/fe2o3-kfd/src/queue_completion/dependency_event.rs").read_text()
proof = (root / runner["PROOF"]).read_text()
need(parent.count('include!("queue_completion/event_release_body.rs");') == 1, "production body include")
need(proof.count('include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");') == 1,
     "proof uses production body")
for source, name, args in (
    (parent, "require_ready", "self"),
    (child, "validate_active_event", "self, event"),
    (child, "validate_live_occurrence", "self, exact"),
    (child, "event_release_preflight", "self, event"),
    (child, "release_event", "self, event"),
):
    call = "completion_" + name + "_body!(completion_rust_expr, " + args + ")"
    need(source.count(call) == 1, "production macro wiring: " + name)
    need(proof.count(call.replace("completion_rust_expr", "verus_exec_expr")) == 1,
         "proof macro wiring: " + name)
need(campaign.FILES == [runner["BODY"], runner["PROOF"]], "two-file relocation closure")

classifier = campaign.inherited()
leaf = classifier.inherited()
verifier = {"version": "calibration-only"}
path = "/snapshot/completion_event_release_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0,
}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for _, focus in cases.values():
    notes = runner["selection_notes"](leaf, focus)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected note accepted")
        need(not check([dict(note, message=message + " unknown"), error]), "unknown note rejected")
    for message in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
        need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]),
         "foreign diagnostic rejected")
success = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, campaign.EXPECTED, {path}),
     "exact positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, campaign.EXPECTED, {path}),
     "failed positive rejected")
print("PASS: completion event release calibration (4 groups)")
