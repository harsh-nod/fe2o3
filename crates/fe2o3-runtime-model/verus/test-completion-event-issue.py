#!/usr/bin/env python3
"""Fail-closed issuer closure, wiring and diagnostic calibration."""
import json
from pathlib import Path
import re
import runpy

runner = runpy.run_path(str(Path(__file__).with_name("check-completion-event-issue.py")))
need, root = runner["need"], runner["ROOT"]
inputs = {path: (root / path).read_bytes().decode("utf-8") for path in runner["FILES"]}


def rejects(operation, message):
    try:
        operation()
    except ValueError:
        return
    raise ValueError(message)


runner["audit"](inputs)
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        rejects(lambda: runner["audit"]({**inputs, path: inputs[path] + suffix}), "additional trust/input accepted")
    rejects(lambda: runner["audit"]({p: text for p, text in inputs.items() if p != path}), "missing input accepted")
rejects(lambda: runner["audit"]({**inputs, Path("/foreign.rs"): ""}), "extra input accepted")
for path, edges in runner["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + "\n" + statement, statement.replace(".rs", "-foreign.rs")):
            rejects(lambda: runner["audit"]({**inputs, path: inputs[path].replace(statement, replacement)}), "changed edge accepted")
contract = runner["CONTRACTS"]
rejects(lambda: runner["audit"]({**inputs, contract: inputs[contract] + "\n"}), "changed trust supplement accepted")
rejects(lambda: runner["audit"]({**inputs, contract: inputs[contract].replace("\n", "\r\n")}), "CRLF trust drift accepted")

paths = ["crates/fe2o3-kfd/src/queue_completion.rs",
         "crates/fe2o3-kfd/src/queue_completion/dependency_event.rs",
         "crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs"]
compact = lambda text: re.sub(r",\)", ")", re.sub(r"\s+", "", text))
sources = [compact((root / path).read_text()) for path in paths]
patterns = [
    ['include!("queue_completion/event_issue_body.rs");',
     "macro_rules!completion_rust_expr{($body:expr)=>{$body};}",
     "completion_record_bound_dependency_batch_body!(completion_rust_expr,self,session_occurrence,source_acceptance_epoch,bound)",
     "completion_record_dependency_batch_body!(completion_rust_expr,self,session_occurrence,source_acceptance_epoch,retention)"],
    ["completion_dependency_ledger_new_body!(completion_rust_expr)",
     "completion_record_event_batch_body!(completion_rust_expr,self,session_occurrence,source_acceptance_epoch,retention,N)",
     "completion_record_single_event_body!(completion_rust_expr,self,session_occurrence,source_acceptance_epoch,retention,batch_index)",
     "completion_logical_identity_body!(completion_rust_expr,session_occurrence,acceptance_epoch)"],
    ['include!("dependency_source_output_body.rs");',
     "macro_rules!dependency_source_rust_expr{($body:expr)=>{$body};}",
     "letmutoutput=dependency_source_output_reserve_body!(dependency_source_rust_expr,N)?;",
     "events:dependency_source_output_pack_body!(dependency_source_rust_expr,output,events,lane)"]]


def wiring(sources):
    for text, group in zip(sources, patterns):
        for pattern in group:
            need(text.count(pattern) == 1, "exact production wiring: " + pattern)
    fixed = sources[2]
    start = "fnsubmit_fixed_dispatch_with_dependency_events_operation_v1<constN:usize>"
    end = "pub(super)fnsubmit_with_dependency_events_classified_v1<constN:usize>"
    need(fixed.count(start) == fixed.count(end) == 1, "exact caller boundaries")
    caller = fixed.split(start)[1].split(end)[0]
    guards = [
        "ifself.terminal_poisoned{returnErr(FixedDispatchSubmissionFailureV1::Terminal(Gfx942DispatchBindingErrorV1::Poisoned.into()));}",
        "ifself.has_any_persistent_compute_attachment_v1(){returnErr(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(Gfx942DispatchBindingErrorV1::ResourcePhase.into()));}",
        'ifN==0||N>super::completion::GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1{returnErr(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1::Contract("dependencysourcepacketcountmustbe1through8192")));}',
        patterns[2][2],
        "letacceptance=matchself.dependency_owner.reserve_acceptance_epoch()",
        "letbinding=recipe.bind(self);",
        "letcompletion=self.submit_with_dependency_events_classified_v1(",
        patterns[2][3],
    ]
    for guard in guards:
        need(caller.count(guard) == 1, "exact source caller guard/reservation")
    positions = [caller.index(guard) for guard in guards]
    need(positions == sorted(positions), "guard-reserve-identity-bind-submit-pack order")


wiring(sources)
for index, group in enumerate(patterns):
    for pattern in group:
        changed = sources.copy()
        changed[index] = changed[index].replace(pattern, "WRONG")
        rejects(lambda: wiring(changed), "missing/substituted wiring accepted")
changed = sources.copy()
reservation = patterns[2][2]
changed[2] = changed[2].replace(reservation, "").replace("letbinding=recipe.bind(self);", "letbinding=recipe.bind(self);" + reservation)
rejects(lambda: wiring(changed), "late reservation accepted")
for old, new in (("ifself.terminal_poisoned{", "iffalse{"),
                 ("ifself.has_any_persistent_compute_attachment_v1(){", "iffalse{"),
                 ("ifN==0||N>super::completion::GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1{", "iffalse{"),
                 (patterns[2][3], "events:events.into_iter().collect()")):
    changed = sources.copy()
    changed[2] = changed[2].replace(old, new)
    rejects(lambda: wiring(changed), "changed source guard/packing accepted")

# Reservation sizes/calls are wiring controls, not capacity or cost theorems.
def reservations(issue, output):
    need(issue.count("$owner.dependency_ledger.events.try_reserve($n)") == 1, "batch ledger reserve")
    need(issue.count("$events.try_reserve_exact($n)") == 1, "batch output reserve")
    need(issue.count("$owner.dependency_ledger.events.try_reserve(1)") == 1, "single ledger reserve")
    need(output.count("output.try_reserve_exact($n)") == 1, "source output reserve")


issue, output = inputs[runner["BODY"]], inputs[runner["OUTPUT"]]
reservations(issue, output)
for old in ("$owner.dependency_ledger.events.try_reserve($n)", "$events.try_reserve_exact($n)",
            "$owner.dependency_ledger.events.try_reserve(1)"):
    rejects(lambda: reservations(issue.replace(old, "Ok(())"), output), "omitted reserve accepted")
rejects(lambda: reservations(issue, output.replace("try_reserve_exact($n)", "try_reserve_exact(0)")), "zero reservation accepted")

for mode, count in (("batch", 32), ("single", 15), ("packing", 8)):
    campaign = runner["campaign"](mode)
    body = inputs[campaign.BODY]
    cases = campaign.mutations(body)
    need(len(cases) == len(set(cases.values())) == count, "exact mutation roster")
    for text, _ in cases.values():
        need(text != body, "changed executable body")
        runner["audit"]({**inputs, campaign.BODY: text})
    rejects(lambda: campaign.mutations(body + body), "ambiguous mutation accepted")
    classifier = campaign.inherited()
    leaf = classifier.inherited()
    verifier = {"version": "calibration-only"}
    path = "/snapshot/completion_event_issue_v1.rs"
    result = {"verus": verifier, "verification-results": {
        "encountered-error": True, "encountered-vir-error": False,
        "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
    error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
             "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
    for _, focus in cases.values():
        notes = runner["selection_notes"](leaf, focus)
        check = lambda diagnostics, status=1: classifier.logical_negative(notes, status, json.dumps(result),
            "\n".join(json.dumps(d) for d in diagnostics), verifier, {path})
        need(check([error]), "logical negative")
        for status in (0, 124, 137, -9):
            need(not check([error], status), "nonlogical status rejected")
        for message in notes.SELECTION_NOTES:
            note = dict(error, level="note", message=message, spans=[])
            need(check([note, error]), "exact selector")
            need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
        bounds = dict(error, message=runner["BOUNDS_ERROR"])
        need(check([bounds]) == (focus in runner["BOUNDS_SELECTORS"]), "bounds restricted to indexed bodies")
        arithmetic = dict(error, message=runner["ARITHMETIC_ERROR"])
        need(check([arithmetic]) == (focus in runner["ARITHMETIC_SELECTORS"]), "arithmetic restricted to issuers")
        for hostile in (dict(arithmetic, message=runner["ARITHMETIC_ERROR"] + " unknown"),
                        dict(arithmetic, level="warning"), dict(arithmetic, children=[error]),
                        dict(arithmetic, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check([hostile]), "hostile arithmetic diagnostic rejected")
        need(not check([arithmetic, dict(error, message="Resource limit (rlimit) exceeded")]), "mixed arithmetic/resource rejected")
        for hostile in (dict(bounds, message=runner["BOUNDS_ERROR"] + " unknown"), dict(bounds, level="warning"),
                        dict(error, children=[error]), dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check([hostile]), "hostile diagnostic rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
            need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
    check_positive = lambda status, diagnostics: classifier.proof_positive(status, json.dumps(positive), diagnostics,
        verifier, campaign.EXPECTED, {path})
    need(check_positive(0, ""), "exact positive")
    need(not check_positive(1, "") and not check_positive(0, json.dumps(error)), "invalid positive rejected")
    for message in classifier.ENUMERATION_NOTES:
        note = dict(error, level="note", message=message)
        need(check_positive(0, json.dumps(note)), "exact enumeration")
        need(not check([note]) and check([note, error]), "note alone cannot prove a negative")
        for hostile in (dict(note, message=message + " unknown"), dict(note, level="warning"),
                        dict(note, level="error"), dict(note, children=[error]),
                        dict(note, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check_positive(0, json.dumps(hostile)), "hostile positive note rejected")
print("PASS: completion event issuance calibration (6 groups)")
