#!/usr/bin/env python3
"""Fail-closed binding graph, production wiring and diagnostic calibration."""
import json
from pathlib import Path
import re
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-event-bind.py")))
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
                raise ValueError("changed edge accepted")
for path in inputs:
    try:
        runner["audit"]({p: text for p, text in inputs.items() if p != path})
    except ValueError:
        pass
    else:
        raise ValueError("missing source accepted")
try:
    runner["audit"]({**inputs, Path("/foreign.rs"): ""})
except ValueError:
    pass
else:
    raise ValueError("extra source accepted")

paths = ["crates/fe2o3-kfd/src/queue_completion.rs",
         "crates/fe2o3-kfd/src/queue_completion/dependency_event.rs",
         "crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs"]
sources = [(root / path).read_text() for path in paths]


def wiring(sources):
    owner, event, fixed = sources
    compact = lambda text: re.sub(r",\)", ")", re.sub(r"\s+", "", text))
    need(owner.count('include!("queue_completion/event_bind_body.rs");') == 1, "binding production include")
    need(fixed.count('include!("../queue_completion/source_publish_body.rs");') == 1, "publication production include")
    owner, event, fixed = map(compact, sources)
    for pattern in (
        "completion_validate_published_body!(completion_rust_expr,self,retention)",
        "completion_mark_published_retaining_body!(completion_rust_expr,self,retention,last_packet_id,N)",
        "completion_bind_dependency_event_batch_body!(completion_rust_expr,self,events,batch)",
        "macro_rules!completion_rust_expr{($body:expr)=>{$body};}",
    ):
        need(owner.count(pattern) == 1, "exact completion wiring: " + pattern)
    for pattern in (
        "completion_bind_event_batch_body!(completion_rust_expr,self,events,batch,N)",
        "completion_exact_occurrence_body!(completion_rust_expr,session_occurrence,source_acceptance_epoch,retention,batch_index,packet_id)",
        "completion_packet_id_at_body!(completion_rust_expr,retention,batch_index,N)",
    ):
        need(event.count(pattern) == 1, "exact event wiring: " + pattern)
    need(fixed.count("Ok(last_packet_id)=>{completion_source_publish_body!(dependency_source_rust_expr,self.completion_owner,retention,last_packet_id,events)}") == 1,
         "exact inline post-native publication join")
    need(fixed.count("macro_rules!dependency_source_rust_expr{($body:expr)=>{$body};}") == 1, "source identity syntax")
    need(owner.count("implFrom<Gfx942CompletionErrorV1>") == 0, "conversion belongs to live owner")
    live = compact((root / "crates/fe2o3-kfd/src/queue_live.rs").read_text())
    need(live.count("implFrom<Gfx942CompletionErrorV1>forComputeAqlQueueSessionErrorV1{fnfrom(value:Gfx942CompletionErrorV1)->Self{Self::Completion(value)}}") == 1,
         "exact prior error conversion")


wiring(sources)
for index, old, new in (
    (0, "completion_validate_published_body!(completion_rust_expr, self, retention)", "Ok(())"),
    (0, "completion_bind_dependency_event_batch_body!(completion_rust_expr, self, events, batch)", "Ok(events)"),
    (1, "completion_bind_event_batch_body!(completion_rust_expr, self, events, batch, N)", "Ok(events)"),
    (1, "completion_packet_id_at_body!(completion_rust_expr, retention, batch_index, N)", "Ok(0)"),
    (2, "self.completion_owner,\n                    retention,", "self.other_owner,\n                    retention,"),
):
    changed = sources.copy()
    need(old in changed[index], "changed wiring control")
    changed[index] = changed[index].replace(old, new)
    try:
        wiring(changed)
    except ValueError:
        pass
    else:
        raise ValueError("wrong wiring accepted")

for source_mode, count in ((False, 28), (True, 6)):
    campaign = runner["campaign"](source_mode)
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
    path = "/snapshot/completion_event_bind_v1.rs"
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
            need(check([note, error]), "exact logical negative")
            need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
        need(check([dict(error, message=runner["CLOSURE_ERROR"])]) == (focus == "*finish_source_publication"), "closure error limited to source join")
        bounds = dict(error, message=runner["BOUNDS_ERROR"])
        need(check([bounds]) == (focus in runner["BOUNDS_SELECTORS"]), "bounds error limited to indexed bodies")
        for hostile in (
            dict(bounds, message=runner["BOUNDS_ERROR"] + " unknown"),
            dict(bounds, level="warning"),
            dict(bounds, spans=[{"is_primary": True, "file_name": "/foreign.rs"}]),
            dict(bounds, children=[error]),
        ):
            need(not check([hostile]), "hostile bounds diagnostic rejected")
        need(not check([bounds, dict(error, message="Resource limit (rlimit) exceeded")]), "mixed bounds/resource rejected")
        need(not check([dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])]), "foreign primary rejected")
        need(not check([dict(error, children=[error])]), "nested diagnostic rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error", runner["CLOSURE_ERROR"] + " unknown"):
            need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
    check_positive = lambda status, diagnostics: classifier.proof_positive(status, json.dumps(positive), diagnostics,
        verifier, campaign.EXPECTED, {path})
    need(check_positive(0, ""), "exact positive")
    need(not check_positive(1, "") and not check_positive(0, json.dumps(error)), "invalid positive rejected")
    for message in classifier.ENUMERATION_NOTES:
        note = dict(error, level="note", message=message)
        need(check_positive(0, json.dumps(note)), "exact inherited enumeration")
        need(not check([note]) and check([note, error]), "enumeration alone is not a logical negative")
        for hostile in (
            dict(note, message=message + " unknown"), dict(note, level="warning"),
            dict(note, level="error"), dict(note, children=[error]),
            dict(note, spans=[{"is_primary": True, "file_name": "/foreign.rs"}]),
        ):
            need(not check_positive(0, json.dumps(hostile)), "hostile positive enumeration rejected")
        need(not check([dict(note, message=message + " Resource limit (rlimit) exceeded"), error]), "mixed enumeration rejected")
print("PASS: completion event binding calibration (6 groups)")
