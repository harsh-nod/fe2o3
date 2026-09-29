#!/usr/bin/env python3
"""Synthetic classifier and source-wiring checks, not solver evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-bound-cancel.py")))
campaign = runner["campaign"]()
need = runner["need"]
root = runner["ROOT"]
body = (root / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 23, "distinct cancellation mutation roster")
need({focus for _, focus in cases.values()} == {"*" + name for name in runner["FUNCTIONS"]},
     "all five executable helpers selected")
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text, _ in cases.values()), "no mutation bypass")
for changed in (body + body, body.replace("record.generation != $slot.generation", "false"),
                body.replace("$seen[$word] |= $bit;", "$seen[$word] |= 0;")):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("stale or ambiguous mutation site accepted")

parent = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
proof = (root / runner["PROOF"]).read_text()
need(parent.count('include!("queue_completion/bound_cancel_body.rs");') == 1, "production include")
need(proof.count('include!("../../fe2o3-kfd/src/queue_completion/bound_cancel_body.rs");') == 1,
     "proof uses production body")
for name in ("packet_count", "validate_bound", "validate_retention", "require_unpinned", "cancel_bound_retaining"):
    call = "completion_" + name + "_body!("
    need(parent.count(call) == proof.count(call) == 1, "shared macro wiring: " + name)
need(campaign.FILES == [runner["BODY"], runner["PROOF"]], "two-file relocation closure")

classifier = campaign.inherited()
leaf = classifier.inherited()
verifier = {"version": "calibration-only"}
path = "/snapshot/completion_bound_cancel_v1.rs"
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
bit_note = dict(error, level="note", message=runner["BITVECTOR_ENUMERATION_NOTE"])
positive = lambda diagnostic: classifier.proof_positive(0, json.dumps(success), json.dumps(diagnostic),
    verifier, campaign.EXPECTED, {path})
need(positive(bit_note), "exact informational bitvector enumeration accepted")
need(not positive(dict(bit_note, level="error")), "bitvector error never accepted as a note")
need(not positive(dict(bit_note, message=bit_note["message"] + " unknown")), "unknown enumeration rejected")
need(not positive(dict(bit_note, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])),
     "foreign positive enumeration rejected")
need(not positive(dict(bit_note, children=[error])), "nested error rejected")
need(not check([bit_note]), "enumeration alone is not a negative control")
need(check([bit_note, error]), "exact enumeration permits a separate authenticated logical error")
need(not check([dict(bit_note, message=bit_note["message"] + " Resource limit (rlimit) exceeded"), error]),
     "resource-bearing enumeration rejected")
print("PASS: completion bound cancellation calibration (4 groups)")
