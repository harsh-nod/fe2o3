#!/usr/bin/env python3
"""Synthetic mutation/wiring/classifier calibration, not solver evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-dispatch-epoch-cancel.py")))
need = runner["need"]
root = runner["ROOT"]
body = (root / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 22, "distinct mutants")
for text, focus in cases.values():
    need(text != body and focus[1:] in runner["FUNCTIONS"], "changed body and known focus")
for changed in (body + body, body.replace("if $owner.poisoned", "if false")):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("ambiguous or stale site accepted")
parent = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()
proof = (root / runner["PROOF"]).read_text()
need(parent.count('include!("queue_dispatch_binding/epoch_cancel_body.rs");') == 1, "production include")
need(proof.count('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");') == 1, "proof include")
for macro, args in (
    ("dispatch_not_poisoned_body", "self"),
    ("dispatch_require_identity_body", "self, identity, expected"),
    ("dispatch_expected_roster_body", "self, identity"),
    ("dispatch_cancel_epoch_body", "self, identity"),
):
    need(parent.count(f"{macro}!(dispatch_rust_expr, {args})") == 1, "production wiring " + macro)
    need(proof.count(f"{macro}!(verus_exec_expr, {args})") == 1, "proof wiring " + macro)

campaign = runner["campaign"]()
classifier = campaign.inherited()
leaf = classifier.inherited()
verifier = {"version": "calibration-only"}
path = "/snapshot/dispatch_epoch_cancel_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0,
}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for name in runner["FUNCTIONS"]:
    notes = runner["selection_notes"](leaf, "*" + name)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selection accepted")
        need(not check([dict(note, message=message + " unknown"), error]), "unknown selection rejected")
    bounds = dict(error, message=runner["BOUNDS_ERROR"])
    need(check([bounds]), "exact bounds failure accepted")
    for invalid in (
        dict(bounds, message=bounds["message"] + " unknown"), dict(bounds, level="warning"),
        dict(bounds, spans=[{"is_primary": True, "file_name": "/foreign.rs"}]),
        dict(bounds, children=[dict(error, message="internal error")]),
    ):
        need(not check([invalid]), "unqualified bounds failure rejected")
    for message in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error",
                    "external_body/assume_specification not allowed with --no-cheating"):
        need(not check([bounds, dict(error, message=message)]), "mixed/nonlogical failure rejected")
success = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "failed positive rejected")
need(not classifier.proof_positive(0, json.dumps(success), json.dumps(bounds), verifier, campaign.EXPECTED, {path}), "bounds never positive")
print("PASS: dispatch epoch cancellation calibration (4 groups)")
