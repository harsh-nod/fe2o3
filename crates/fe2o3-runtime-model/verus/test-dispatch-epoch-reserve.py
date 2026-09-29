#!/usr/bin/env python3
"""Fail-closed closure, exact source joins, schema and diagnostic calibration."""
import hashlib
import json
from pathlib import Path
import re
import runpy
import types

r = runpy.run_path(str(Path(__file__).with_name("check-dispatch-epoch-reserve.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_text() for path in r["FILES"]}
lexer_path = root / r["V"] / "check-negative-quality.py"
lexer_raw = lexer_path.read_bytes()
need(hashlib.sha256(lexer_raw).hexdigest() ==
     "7fadaf2f0b1b155ae8b0038e17fc01471a648319f4c2d59dc4adea25ec59cd5f", "authenticated Rust lexical helper")
lexer = types.ModuleType("epoch_reserve_rust_lexer")
lexer.__file__ = str(lexer_path)
exec(compile(lexer_raw, lexer.__file__, "exec"), lexer.__dict__)
code_only = lexer.code_only


def rejects(operation):
    try:
        operation()
    except ValueError:
        return
    raise ValueError("hostile calibration accepted")


r["audit"](inputs)
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        rejects(lambda: r["audit"]({**inputs, path: inputs[path] + suffix}))
    rejects(lambda: r["audit"]({p: text for p, text in inputs.items() if p != path}))
rejects(lambda: r["audit"]({**inputs, Path("/foreign.rs"): ""}))
for path, edges in r["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + statement, statement.replace(".rs", "-foreign.rs")):
            rejects(lambda: r["audit"]({**inputs, path: inputs[path].replace(statement, replacement)}))


def compact(text):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", text))


production = (root / "crates/fe2o3-kfd/src/queue_dispatch_binding.rs").read_text()
patterns = [
    'include!("queue_dispatch_binding/epoch_reserve_body.rs");',
    'include!("queue_dispatch_binding/epoch_cancel_body.rs");',
    'macro_rules!dispatch_rust_expr{($body:expr)=>{$body};}',
    'pubconstfnslots(self)->usize{dispatch_capacity_slots_body!(dispatch_rust_expr,self)}',
    'fnreserve(&mutself,queue:QueueKeyV1,expected_roster:CompletionDispatchRosterV1)->Result<DispatchEpochIdentityV1,Gfx942DispatchBindingErrorV1>{dispatch_reserve_epoch_body!(dispatch_rust_expr,self,queue,expected_roster)}',
    'fnpreflight_reservation(&self,queue:QueueKeyV1)->Result<(usize,u64,u64),Gfx942DispatchBindingErrorV1>{dispatch_preflight_reservation_body!(dispatch_rust_expr,self,queue)}',
    'fnensure_not_poisoned(&self)->Result<(),Gfx942DispatchBindingErrorV1>{dispatch_not_poisoned_body!(dispatch_rust_expr,self)}',
    'fncancel_epoch(&mutself,identity:DispatchEpochIdentityV1)->Result<(),Gfx942DispatchBindingErrorV1>{dispatch_cancel_epoch_body!(dispatch_rust_expr,self,identity)}',
    'fnrequire_identity(&self,identity:DispatchEpochIdentityV1,expected:DispatchEpochPhaseV1)->Result<(),Gfx942DispatchBindingErrorV1>{dispatch_require_identity_body!(dispatch_rust_expr,self,identity,expected)}',
    'fnexpected_roster(&self,identity:DispatchEpochIdentityV1)->Result<CompletionDispatchRosterV1,Gfx942DispatchBindingErrorV1>{dispatch_expected_roster_body!(dispatch_rust_expr,self,identity)}',
    'pubconstGFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1:usize=64;',
    'pub(crate)useGfx942FixedDispatchCapacityProfileV1asFixedDispatchCapacityProfileV1;',
]


def wiring(source):
    prefix = source.split("macro_rules!", 1)[0]
    need("/*" not in prefix and "*/" not in prefix, "restricted include prefix has no block comments")
    need(compact(re.sub(r"//[^\n]*", "", prefix)) ==
         '#![allow(dead_code)]include!("queue_dispatch_binding/epoch_cancel_body.rs");'
         'include!("queue_dispatch_binding/epoch_reserve_body.rs");'
         'include!("queue_dispatch_binding/cancel_binding_body.rs");', "exact active include prefix")
    source = compact(code_only(source))
    need(source.count("include!(") == 3, "only the three active dispatch includes")
    for pattern in patterns:
        if not pattern.startswith("include!"):
            need(source.count(pattern) == 1, "exact production wrapper: " + pattern)


wiring(production)
prefix, remainder = production.split("macro_rules!", 1)
canonical_production = compact(re.sub(r"//[^\n]*", "", prefix)) + "\n" + compact(code_only("macro_rules!" + remainder))
wiring(canonical_production)
for pattern in patterns:
    rejects(lambda: wiring(canonical_production.replace(pattern, "WRONG")))
    rejects(lambda: wiring(canonical_production + pattern))
    for decoy in ("/* " + pattern + " */", "// " + pattern + "\n", 'r###"' + pattern + '"###'):
        rejects(lambda: wiring(canonical_production.replace(pattern, decoy + "\nWRONG")))


def shape(source, name):
    source = code_only(source)
    source = re.sub(r"\bpub(?:\([^)]*\))?\s+", "", source)
    matches = list(re.finditer(r"\b(?:struct|enum)\s+" + name + r"(?:<[^>]*>)?\s*\{", source))
    need(len(matches) == 1, "unique shape " + name)
    start = end = matches[0].end()
    depth = 1
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    need(depth == 0, "closed shape")
    return compact(source[start:end - 1]).rstrip(",")


def variants(body):
    entries, start, depth = [], 0, 0
    for index, char in enumerate(body + ","):
        if char in "({[<":
            depth += 1
        elif char in ")}]>":
            depth -= 1
        elif char == "," and depth == 0:
            entries.append(body[start:index])
            start = index + 1
        need(depth >= 0, "balanced variant payload")
    need(depth == 0 and len(entries) == len(set(entries)) and all(entries), "complete unique variants")
    return set(entries)


def schemas(parent, model, completion):
    for name in ("DispatchEpochPhaseV1", "DispatchEpochSlotV1", "DispatchEpochIdentityV1"):
        need(shape(parent, name) == shape(model, name), "exact schema " + name)
    # The accounted table's untouched linear ownership payload is arbitrary C.
    owner = shape(parent, "DispatchGenerationOwnerV1")
    projected = shape(model, "DispatchGenerationOwnerV1").replace("slots:Vec<DispatchEpochSlotV1>,credits:C,", "slots:HostMetadataTableV1<DispatchEpochSlotV1>,")
    need(owner == projected, "accounted table payload projection")
    profile = re.sub(r"#\[[^]]*\]", "", shape(parent, "Gfx942FixedDispatchCapacityProfileV1"))
    need(profile == shape(model, "FixedDispatchCapacityProfileV1"), "capacity profile")
    for name in ("CompletionDispatchRosterV1", "CompletionBatchOccurrenceV1"):
        need(shape(completion, name) == shape(model, name), "exact completion fields " + name)
    need(variants(shape(model, "Gfx942DispatchBindingErrorV1")) <=
         variants(shape(parent, "Gfx942DispatchBindingErrorV1")), "complete real error variants")


model = inputs[r["CANCEL"]]
completion = (root / "crates/fe2o3-kfd/src/queue_completion.rs").read_text()
schemas(production, model, completion)
for field in ("recipe_occurrence: u64", "slot_generation: u64", "next_generation: u64", "credits: C", "roster_sha256: [u8; 32]"):
    need(field in model, "hostile shape site")
    rejects(lambda: schemas(production, model.replace(field, "wrong: u64"), completion))
    rejects(lambda: schemas(production, model.replace(field, "/* " + field + " */ wrong: u64"), completion))
for old, new in (("ResourcePhase", "WrongResourcePhase"), ("Poisoned", "NotPoisoned"),
                 ("DispatchEpochCapacity { maximum: usize }", "DispatchEpochCapacity { maximum: u64 }")):
    need(old in production, "enum mutation site")
    rejects(lambda: schemas(production.replace(old, new), model, completion))

cases = r["mutations"](inputs[r["BODY"]])
need(len(cases) == 28, "mutation count")
for data, focus in cases.values():
    need(data != inputs[r["BODY"]] and focus[1:] in r["METHODS"], "executable mutation")
rejects(lambda: r["mutations"](inputs[r["BODY"]] * 2))
rejects(lambda: r["mutations"](inputs[r["BODY"]].replace("$vacant = true;", "$vacant = false;")))

campaign = r["campaign"]()
classifier, verifier = campaign.inherited(), {"version": "calibration-only"}
leaf = classifier.inherited()
path = "/snapshot/dispatch_epoch_reserve_v1.rs"
result = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
         "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
for name in r["METHODS"]:
    notes = r["selection_notes"](leaf, "*" + name)
    check = lambda messages: classifier.logical_negative(notes, 1, json.dumps(result),
        "\n".join(json.dumps(item) for item in messages), verifier, {path})
    for message in notes.SELECTION_NOTES:
        note = dict(error, level="note", message=message, spans=[])
        need(check([note, error]), "exact selected logical failure")
        need(not check([dict(note, message=message + " unknown"), error]), "unknown selector refused")
    for message in notes.LOGICAL_ERRORS:
        logical = dict(error, message=message)
        need(check([logical]), "known logical failure")
        for bad in (dict(logical, level="warning"), dict(logical, message=message + " unknown"),
                    dict(logical, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check([bad]), "unauthenticated diagnostic refused")
    for bad in ("Resource limit (rlimit) exceeded", "type annotations needed", "internal error",
                "recommendation not met", "external_body/assume_specification not allowed with --no-cheating"):
        need(not check([error, dict(error, message=bad)]), "nonlogical mixed result refused")
positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "positive accepted")
need(not classifier.proof_positive(1, json.dumps(positive), "", verifier, campaign.EXPECTED, {path}), "failed positive refused")
print("PASS: dispatch epoch reservation calibration (5 groups)")
