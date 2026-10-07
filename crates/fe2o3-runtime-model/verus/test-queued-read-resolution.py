#!/usr/bin/env python3
"""Synthetic controller calibration, not solver or native-execution evidence."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-queued-read-resolution.py")))
need = runner["need"]
body = (runner["ROOT"] / runner["BODY"]).read_text()
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 16, "distinct mutation roster")
need({focus for _, focus in cases.values()} == {"*chain_resolution_witness*"}, "exact selected functions")
need(all(text != body and not any(token in text for token in ("assume(", "admit(", "external_body"))
         for text, _ in cases.values()), "executable mutations without proof bypass")
for changed in (body + body, body.replace("$entry.status = $status;", "$entry.status = ContextProducerReadStatusV1::Unknown;"),
                body.replace("$entry.next = None;", "$entry.next = $entry.next;"),
                body.replace("$retained.read_count = 0;", "$retained.read_count = $root.read_count;"),
                body + "\n$entry.previous = None;"):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("stale or ambiguous mutation site accepted")
need(len(runner["FILES"]) == 2 and runner["BODY"] != runner["PROOF"], "two-file proof closure")
for path in runner["FILES"]:
    need((runner["ROOT"] / path).is_file(), "present proof input")
classifier = runner["inherited"]()
notes = runner["selection_notes"](classifier.inherited())
verifier = {"version": "calibration-only"}
path = "/snapshot/context_queued_read_resolution_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0,
}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
    "\n".join(json.dumps(item) for item in messages), verifier, {path})
for message in notes.SELECTION_NOTES:
    note = dict(error, level="note", message=message, spans=[])
    need(check([note, error]), "exact queued-resolution selection note accepted")
    need(not check([dict(note, message=message + " unknown"), error]), "unrecognized selection note rejected")
need(not check([error, dict(error, message="Resource limit (rlimit) exceeded")]), "mixed resource failure rejected")
need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign source rejected")
success = {"verus": verifier, "verification-results": runner["EXPECTED"]}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, runner["EXPECTED"], {path}), "exact positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, runner["EXPECTED"], {path}), "failed positive rejected")
print("PASS: queued-read resolution campaign calibration (4 groups)")
