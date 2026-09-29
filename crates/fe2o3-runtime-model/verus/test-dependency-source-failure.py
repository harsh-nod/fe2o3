#!/usr/bin/env python3
"""Fail-closed source graph, native wiring and negative-classifier calibration."""
import json
from pathlib import Path
import re
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-dependency-source-failure.py")))
need, root = runner["need"], runner["ROOT"]
inputs = {path: (root / path).read_bytes().decode("utf-8") for path in runner["FILES"]}
runner["audit"](inputs)
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        try:
            runner["audit"]({**inputs, path: inputs[path] + suffix})
        except ValueError:
            pass
        else:
            raise ValueError("additional trust/input accepted")
for path, edges in runner["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + "\n" + statement, statement.replace(".rs", "-foreign.rs")):
            try:
                runner["audit"]({**inputs, path: inputs[path].replace(statement, replacement)})
            except ValueError:
                pass
            else:
                raise ValueError("changed closure edge accepted")
for changed in ({path: text for path, text in inputs.items() if path != runner["EPOCH_BODY"]},
                {**inputs, Path("/foreign.rs"): ""}):
    try:
        runner["audit"](changed)
    except ValueError:
        pass
    else:
        raise ValueError("inexact closure accepted")

fixed = (root / "crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs").read_text()
binding = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()


def wiring(fixed, binding, proof):
    compact = lambda text: re.sub(r"\s+", "", text)
    need(fixed.count('include!("dependency_source_failure_body.rs");') == 1, "production failure include")
    need(binding.count('include!("queue_dispatch_binding/cancel_binding_body.rs");') == 1, "production binding include")
    need(compact(fixed).count("Err(failure)=>{returnErr(dependency_source_failure_body!(dependency_source_rust_expr,self,recipe,identity,failure));}") == 1,
         "exact inline failure join")
    need(compact(fixed).count("macro_rules!dependency_source_rust_expr{($body:expr)=>{$body};}") == 1, "identity syntax adapter")
    need(compact(fixed).count("native_dependency_source_cancel_body!(dependency_source_rust_expr,session,identity)") == 1, "native forwarding")
    need(compact(binding).count("dispatch_cancel_binding_body!(dispatch_rust_expr,self,identity)") == 1, "generation forwarding")
    need(compact(proof).count("dependency_source_recipe_cancel_call!($recipe,$session,$identity)") == 1, "observer calls production macro")
    need(compact(proof).count("dependency_source_failure_body!(@annotatedverus_exec_expr,session,recipe,identity,failure,observed_cancel)") == 1, "proved failure join")


proof = inputs[runner["PROOF"]]
wiring(fixed, binding, proof)
for texts in ((fixed.replace("                    identity,\n                    failure", "                    other_identity,\n                    failure"), binding, proof),
              (fixed.replace("$body\n", "Ok(())\n"), binding, proof),
              (fixed.replace("native_dependency_source_cancel_body!(dependency_source_rust_expr, session, identity)", "Ok(())"), binding, proof),
              (fixed, binding.replace("dispatch_cancel_binding_body!(dispatch_rust_expr, self, identity)", "Ok(())"), proof),
              (fixed, binding, proof.replace("dependency_source_recipe_cancel_call!($recipe, $session, $identity)", "$recipe.cancel($session, $identity)"))):
    need(texts != (fixed, binding, proof), "changed wiring control")
    try:
        wiring(*texts)
    except ValueError:
        pass
    else:
        raise ValueError("wrong production wiring accepted")

for binding_mode, count in ((False, 13), (True, 3)):
    campaign = runner["campaign"](binding_mode)
    body = inputs[campaign.BODY]
    cases = campaign.mutations(body)
    need(len(cases) == len(set(cases.values())) == count, "exact mutation roster")
    for text, _ in cases.values():
        need(text != body, "changed executable body")
        runner["audit"]({**inputs, campaign.BODY: text})
    try:
        campaign.mutations(body + body)
    except ValueError:
        pass
    else:
        raise ValueError("ambiguous mutation sites accepted")
    classifier = campaign.inherited()
    leaf = classifier.inherited()
    verifier = {"version": "calibration-only"}
    path = "/snapshot/dependency_source_failure_v1.rs"
    result = {"verus": verifier, "verification-results": {
        "encountered-error": True, "encountered-vir-error": False,
        "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
    error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
             "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
    for _, focus in cases.values():
        notes = runner["selection_notes"](leaf, focus)
        check = lambda diagnostics: classifier.logical_negative(notes, 1, json.dumps(result),
            "\n".join(json.dumps(d) for d in diagnostics), verifier, {path})
        for message in notes.SELECTION_NOTES:
            note = dict(error, level="note", message=message, spans=[])
            need(check([note, error]), "exact selected logical negative")
            need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
        need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign primary rejected")
        need(not check([dict(error, children=[error])]), "nested diagnostic rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
            need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
    check_positive = lambda status, diagnostics: classifier.proof_positive(status, json.dumps(positive), diagnostics,
        verifier, campaign.EXPECTED, {path})
    need(check_positive(0, ""), "exact positive")
    need(not check_positive(1, "") and not check_positive(0, json.dumps(error)), "invalid positive rejected")
print("PASS: dependency source failure calibration (6 groups)")
