#!/usr/bin/env python3
"""Synthetic lifecycle controller calibration; not solver or runtime evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-compute-pipeline-chain.py")))
need = runner["need"]
campaign = runner["campaign"]()
body = (runner["ROOT"] / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 12, "distinct mutation roster")
need({focus for _, focus in cases.values()} == {"*first_epoch", "*take_frontier", "*quarantine"}, "exact selections")
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text, _ in cases.values()), "executable mutations without proof bypass")
for changed in (body + body, body.replace("$live.checked_sub(1)", "$live.checked_sub(0)"),
                body.replace("return Some($i);", "return None;"), body + "\n$epoch.checked_add(1)"):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("stale or ambiguous mutation site accepted")
need(len(campaign.FILES) == len(set(campaign.FILES)) == 4, "exact four-file closure")
for path in campaign.FILES:
    need((runner["ROOT"] / path).is_file(), "present proof input")
classifier = campaign.inherited()
leaf = classifier.inherited()
verifier = {"version": "calibration-only"}
path = "/snapshot/compute_pipeline_chain_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0,
}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for _, focus in cases.values():
    notes = runner["selection_notes"](leaf, focus)
    need(notes.SELECTION_NOTES == {"verifying root module (selected functions)",
        "verifying root module, function compute_pipeline_chain_v1::" + focus[1:] + " (selected functions)"},
        "per-mutant selection notes")
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected function accepted")
        need(not check([dict(note, message=message + " foreign"), error]), "foreign selection rejected")
    for message in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
        need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign path rejected")
success = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "exact positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "failed positive rejected")
print("PASS: pipeline chain campaign calibration (4 groups)")
