#!/usr/bin/env python3
"""Synthetic settlement controller calibration; not solver or runtime evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-ordered-publication-settlement.py")))
need = runner["need"]
campaign = runner["campaign"]()
body = (runner["ROOT"] / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 10, "distinct mutation roster")
need({focus for _, focus in cases.values()} == {"*settle_returned"}, "exact selection")
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text, _ in cases.values()), "executable mutations without proof bypass")
for changed in (body + body, body.replace("id: $id,", "id: 0,"),
                body.replace("$fields!($entry.active).published_at = $now;", ""),
                body.replace("$($after_confirm)*", ""), body + "\n$($after_confirm)*",
                body + "\nOk(Some(observation))"):
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
path = "/snapshot/ordered_publication_settlement_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0,
}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
notes = runner["selection_notes"](leaf, "*settle_returned")
need(notes.SELECTION_NOTES == {"verifying root module (selected functions)",
    "verifying root module, function ordered_publication_settlement_v1::settle_returned (selected functions)"},
    "per-mutant selection notes")
check = lambda status, stdout, messages: classifier.logical_negative(notes, status, stdout,
    "\n".join(json.dumps(item) for item in messages), verifier, {path})
for message in notes.SELECTION_NOTES:
    note = dict(error, level="note", message=message, spans=[])
    need(check(1, json.dumps(result), [note, error]), "exact selected function accepted")
    need(not check(1, json.dumps(result), [dict(note, message=message + " foreign"), error]), "foreign selection rejected")
for message in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
    need(not check(1, json.dumps(result), [error, dict(error, message=message)]), "mixed nonlogical failure rejected")
for foreign in ("/foreign.rs", "relative.rs"):
    need(not check(1, json.dumps(result), [dict(error, spans=[{"is_primary": True, "file_name": foreign}])]),
         "unauthenticated path rejected")
for status in (0, 2, 124, -9):
    need(not check(status, json.dumps(result), [error]), "wrong exit status rejected")
for key, value in (("success", False), ("errors", True), ("verified", -1), ("encountered-vir-error", True)):
    changed = dict(result, **{"verification-results": dict(result["verification-results"], **{key: value})})
    need(not check(1, json.dumps(changed), [error]), "wrong verification schema rejected")
success = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "exact positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "failed positive rejected")
print("PASS: ordered settlement campaign calibration (4 groups)")
