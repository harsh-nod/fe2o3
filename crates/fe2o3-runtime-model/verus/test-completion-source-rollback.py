#!/usr/bin/env python3
"""Synthetic composition closure, mutation and diagnostic calibration."""

import json
from pathlib import Path
import re
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-source-rollback.py")))
need = runner["need"]
root = runner["ROOT"]
base = runner["inherited"]()
files, edges = runner["closure"](base)
inputs = {path: (root / path).read_bytes().decode("utf-8") for path in files}
audit = lambda values: base.audit(values, files, edges, runner["PROOF"])
audit(inputs)
need(len(files) == 11, "exact eleven-file closure")
for path in files:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;', '\ninclude!("foreign.rs");', '\nenv!("FOREIGN");'):
        try:
            audit({**inputs, path: inputs[path] + suffix})
        except ValueError:
            pass
        else:
            raise ValueError("extra trust/input accepted")
for path, statements in edges.items():
    for statement, _ in statements:
        for replacement in ("", statement + "\n" + statement, statement.replace(".rs", "-foreign.rs")):
            try:
                audit({**inputs, path: inputs[path].replace(statement, replacement)})
            except ValueError:
                pass
            else:
                raise ValueError("changed source edge accepted")

proof = inputs[runner["PROOF"]]
parent = (root / "crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs").read_text()
completion = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
need(parent.count('include!("../queue_completion/source_rollback_body.rs");') == 1, "inline production include")
need(parent.count("completion_source_rollback_body!(") == 1, "inline production wiring")


def production_wiring(dispatch, owner):
    compact = lambda text: re.sub(r"\s+", "", text)
    need(compact(dispatch).count("completion_source_rollback_body!(dependency_source_rust_expr,"
         "self.completion_owner,events,retention)") == 1, "exact production rollback arguments")
    need(compact(dispatch).count("macro_rules!dependency_source_rust_expr{($body:expr)=>{$body};}") == 1,
         "identity-only production syntax adapter")
    need(compact(owner).count("completion_release_dependency_event_batch_body!(completion_rust_expr,self,events)") == 1,
         "exact production event forwarding adapter")


production_wiring(parent, completion)
for dispatch, owner in (
    (parent.replace("self.completion_owner,\n                    events,", "self.other_owner,\n                    events,"), completion),
    (parent.replace("events,\n                    retention\n", "other_events,\n                    retention\n"), completion),
    (parent.replace("$body\n", "Ok(())\n"), completion),
    (parent, completion.replace("completion_release_dependency_event_batch_body!(completion_rust_expr, self, events)",
                               "completion_release_dependency_event_batch_body!(completion_rust_expr, self, other_events)")),
):
    need((dispatch, owner) != (parent, completion), "changed production wiring control")
    try:
        production_wiring(dispatch, owner)
    except ValueError:
        pass
    else:
        raise ValueError("wrong production wiring accepted")
need(proof.count("completion_source_rollback_body!(@annotated") == 1, "proved rollback wiring")
for call in ("completion_source_release_call!($owner, $events)", "completion_source_cancel_call!($owner, $retention)"):
    need(proof.count(call) == 1, "observation uses actual production call macro")

for adapters, count in ((False, 10), (True, 3)):
    campaign = runner["campaign"](adapters)
    body = inputs[campaign.BODY]
    cases = campaign.mutations(body)
    need(len(cases) == len(set(cases.values())) == count, "exact mutation count")
    for text, _ in cases.values():
        need(text != body, "changed body")
        audit({**inputs, campaign.BODY: text})
    try:
        campaign.mutations(body + body)
    except ValueError:
        pass
    else:
        raise ValueError("ambiguous mutation sites accepted")
    classifier = campaign.inherited()
    leaf = classifier.inherited()
    verifier = {"version": "calibration-only"}
    path = "/snapshot/completion_source_rollback_v1.rs"
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
            need(check([note, error]), "exact selected logical failure")
            need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
            need(not check([error, dict(error, message=message)]), "mixed failure rejected")
        closure_error = dict(error, message=runner["CLOSURE_POSTCONDITION_ERROR"])
        need(check([closure_error]) == (focus == "*cancel_bound"), "closure failure limited to exact adapter selector")
        need(not check([dict(closure_error, message=closure_error["message"] + " unknown")]), "unknown closure failure rejected")
        need(not check([dict(closure_error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign closure failure rejected")
        need(not check([dict(closure_error, children=[error])]), "nested closure diagnostic rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
            need(not check([closure_error, dict(error, message=message)]), "mixed closure failure rejected")
        need(not check([dict(error, spans=[{"is_primary": True, "file_name": "vstd/std_specs/vec.rs"}])]), "foreign primary rejected")
    positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
    need(classifier.proof_positive(0, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "positive accepted")
    need(not classifier.proof_positive(1, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "failed positive rejected")
    need(not classifier.proof_positive(0, json.dumps(positive), json.dumps(error), verifier, campaign.EXPECTED, {path}), "logical error never positive")
print("PASS: completion source rollback calibration (7 groups)")
