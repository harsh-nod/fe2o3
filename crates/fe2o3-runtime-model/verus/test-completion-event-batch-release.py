#!/usr/bin/env python3
"""Synthetic source/trust/classifier checks, not solver qualification."""

import json
from pathlib import Path
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-event-batch-release.py")))
need = runner["need"]
root = runner["ROOT"]
inputs = {path: (root / path).read_text() for path in runner["FILES"]}
runner["audit"](inputs)
campaign = runner["campaign"]()
body = inputs[runner["BODY"]]
cases = runner["mutations"](body)
need(len(cases) == len(set(cases.values())) == 24, "distinct mutations")
for text, focus in cases.values():
    need(text != body and focus == runner["FUNCTION"], "executable focused mutation")
    runner["audit"]({**inputs, runner["BODY"]: text})
for changed in (body + body, body.replace("$available.checked_sub(1)", "$available.checked_sub(0)")):
    try:
        runner["mutations"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("ambiguous or stale mutation site accepted")

for path, text in (
    (runner["CONTRACTS"], inputs[runner["CONTRACTS"]] + "\n// drift\n"),
    *((runner["PROOF"], inputs[runner["PROOF"]] + suffix) for suffix in
      ("\nassume(false);", "\nadmit();", "\n#[verifier::external_body]", "\n#[verifier::assume_termination]", "\nextern crate foreign;", "\nmod foreign;",
       '\ninclude !("/absolute/foreign.rs");', '\ninclude_str!("/absolute/foreign.rs");',
       '\ninclude_bytes!("/absolute/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");')),
    (runner["PROOF"], inputs[runner["PROOF"]].replace('queue_completion/event_release_body.rs', 'queue_completion/foreign.rs')),
    (runner["SINGLE"], inputs[runner["SINGLE"]] + '\ninclude!("foreign.rs");'),
):
    try:
        runner["audit"]({**inputs, path: text})
    except ValueError:
        pass
    else:
        raise ValueError("unapproved trust/closure change accepted")

parent = (root / "crates/fe2o3-kfd/src/queue_completion/dependency_event.rs").read_text()
need(parent.count('include!("batch_event_release_body.rs");') == 1, "production include")
need(parent.count("completion_release_event_batch_body!(completion_rust_expr, self, events)") == 1,
     "complete production method wiring")
need(inputs[runner["PROOF"]].count("completion_release_event_batch_body!(@annotated") == 1,
     "complete proof method wiring")
need(campaign.FILES == runner["FILES"], "four-file relocation closure")

classifier = campaign.inherited()
leaf = classifier.inherited()
notes = runner["selection_notes"](leaf, runner["FUNCTION"])
verifier = {"version": "calibration-only"}
path = "/snapshot/completion_event_batch_release_v1.rs"
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
    need(check([note, error]), "exact selected note accepted")
    need(not check([dict(note, message=message + " unknown"), error]), "unknown note rejected")
for message in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error",
                "external_body/assume_specification not allowed with --no-cheating"):
    need(not check([error, dict(error, message=message)]), "nonlogical/mixed failure rejected")
need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]),
     "foreign logical diagnostic rejected")
bounds = dict(error, message=runner["BOUNDS_ERROR"])
need(check([bounds]), "exact authenticated bounds precondition accepted")
need(not check([dict(bounds, message=bounds["message"] + " unknown")]), "unknown bounds diagnostic rejected")
need(not check([dict(bounds, level="warning")]), "bounds warning is not logical failure evidence")
need(not check([dict(bounds, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign bounds diagnostic rejected")
need(not check([bounds, dict(error, message="Resource limit (rlimit) exceeded")]), "bounds/resource mixture rejected")
need(not check([bounds, dict(error, message="type annotations needed")]), "bounds/frontend mixture rejected")
need(not check([dict(bounds, children=[dict(error, message="internal error")])]), "nested internal error rejected")
success = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "positive accepted")
need(not classifier.proof_positive(1, json.dumps(success), "", verifier, campaign.EXPECTED, {path}), "failed positive rejected")
need(not classifier.proof_positive(0, json.dumps(success), json.dumps(bounds), verifier, campaign.EXPECTED, {path}), "bounds failure never accepted as positive")
print("PASS: completion event batch release calibration (5 groups)")
