#!/usr/bin/env python3
"""Light source/controller controls only; no compiler or solver is launched."""
import ast
import json
from pathlib import Path
import sys
import types


def need(value, message):
    if not value:
        raise AssertionError(message)


def refused(action, message):
    try:
        action()
    except ValueError:
        return
    raise AssertionError(message)


need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
path = Path(__file__).resolve().with_name("check-retained-credit-dispatch.py")
check = types.ModuleType("retained_dispatch_test")
check.__file__ = str(path)
sys.modules[check.__name__] = check
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
sources = check.snapshot()
check.audit(sources)
need(len(check.FILES) == 14 and check.include_closure(sources) == set(check.FILES), "exact executable closure")
for selected in set(check.FILES) | set(check.OWNERS.values()):
    changed = dict(sources)
    changed[selected] += "\n"
    refused(lambda: check.audit(changed), "drifted source accepted")
    del changed[selected]
    refused(lambda: check.audit(changed), "missing source accepted")
changed = dict(sources)
changed[check.RUNTIME / "unbound.rs"] = "// extra\n"
refused(lambda: check.audit(changed), "extra source accepted")
for name, invocation in check.INVOCATIONS.items():
    changed = dict(sources)
    owner = check.OWNERS[name]
    # A rebound source pin must not conceal a detached native entry point.
    changed[owner] = check.compact(changed[owner]).replace(invocation, "false")
    original = check.SOURCE_TREE_SHA
    try:
        check.SOURCE_TREE_SHA = check.tree_hash({p: t for p, t in changed.items() if p != check.PROOF})
        refused(lambda: check.audit(changed), "native shared entry bypass accepted")
    finally:
        check.SOURCE_TREE_SHA = original
changed = dict(sources)
changed[check.PROOF] = changed[check.PROOF].replace("// The unchanged arena include", "// A changed arena include", 1)
original = check.PROOF_SHA
try:
    check.PROOF_SHA = check.sha(changed[check.PROOF])
    refused(lambda: check.audit(changed), "domain prefix drift accepted despite rebound fixture pin")
finally:
    check.PROOF_SHA = original

cases = check.mutations(sources)
need(len(cases) == check.MUTANT_COUNT == 25, "exact twenty-five actual-body mutants constructed")
need({path for path, _, _ in cases.values()} == set(check.BODIES.values()), "all five actual gates covered")
device_path, device_body, device_selector = cases["device-ignored"]
need(device_path == check.BODIES["runtime"] and device_selector == check.SELECTORS["runtime"]
     and device_body == sources[device_path].replace("$this.device == $device", "({ let _ = $device; true })")
     and "({let_=$device;true})&&match" in check.compact(device_body),
     "ignored-device Boolean block is parenthesized before the unchanged conjunction")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for name, (selected, body, selector) in cases.items():
    need(selected in check.FILES and body != sources[selected], "nonvacuous included body mutation")
    changed = dict(sources)
    changed[selected] = body
    refused(lambda: check.audit(changed), "mutant accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS is leaf.LOGICAL_ERRORS
         and notes.SELECTION_NOTES == {"verifying root module (selected functions)"},
         "unchanged logical errors and measured singleton for exact method selector")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "foreign selector accepted")
original_notes = check.SELECTION_NOTES
try:
    for selector in check.SELECTORS.values():
        for policy in (None, {}, {k: v for k, v in original_notes.items() if k != selector},
                       dict(original_notes, **{"*unknown": frozenset({"verifying root module (selected functions)"})}),
                       *(dict(original_notes, **{selector: value}) for value in (
                           None, frozenset(), {"verifying root module (selected functions)"},
                           frozenset({"unknown selector note"}),
                           frozenset({"verifying root module, function retained_credit_dispatch_v1::" + selector[1:] + " (selected functions)"})))):
            check.SELECTION_NOTES = policy
            refused(lambda: check.selection_notes(leaf, selector), "unmeasured or incomplete policy accepted")
finally:
    check.SELECTION_NOTES = original_notes

adapted = check.controller_source()
ast.parse(adapted)
need(adapted.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode()
     == (check.ROOT / check.BASE).read_bytes(), "only first-error limit adapted")
need('"--no-cheating"' in adapted, "no-cheating retained")
module = check.controller()
need(module.FILES == check.FILES and module.PROOF == check.PROOF, "exact proof/controller join")
need(module.BODY is None and module.mutations is check.mutations,
     "no inherited single-body mutation target")
refused(module.main, "inherited pipeline execution entrypoint remained reachable")
check.calibration_arguments(["--calibrate"])
for arguments in ([], ["--verus", "/tmp/verus"], ["--output", "/tmp/proof"],
                  ["--calibrate", "--output", "/tmp/proof"], ["--unknown"]):
    refused(lambda: check.calibration_arguments(arguments), "execution or unknown checker arguments accepted")
need(type(check.EXPECTED_VERIFIED) is int and check.EXPECTED_VERIFIED == 41, "measured full discovery count")
refused(check.campaign().main, "measured count enabled inherited single-body execution")
original = check.EXPECTED_VERIFIED
try:
    for value in (None, 0, -1, True, "41"):
        check.EXPECTED_VERIFIED = value
        refused(check.campaign, "invalid count accepted")
finally:
    check.EXPECTED_VERIFIED = original

classifier = module.inherited()
verifier = {"fixture": "synthetic-only-not-a-discovery"}
paths = {str(check.ROOT / file) for file in check.FILES}
expected = dict(module.EXPECTED, verified=987)
positive = {"verus": verifier, "verification-results": expected}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, expected, paths), "synthetic positive")
for count in (0, 986, 988, True, "987"):
    changed = dict(positive, **{"verification-results": dict(expected, verified=count)})
    need(not classifier.proof_positive(0, json.dumps(changed), "", verifier, expected, paths), "wrong synthetic count accepted")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "verified": 0, "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.BODIES["context"]), "is_primary": True}]}


def accepts(notes, value=negative, error=diagnostic, status=1):
    return type(status) is int and classifier.logical_negative(
        notes, status, json.dumps(value), json.dumps(error), verifier, paths)


for selector in check.SELECTORS.values():
    notes = check.selection_notes(leaf, selector)
    need(accepts(notes), "nonvacuous strict logical rejection")
    note = {"$message_type": "diagnostic", "level": "note", "code": None, "children": [], "spans": []}
    for message, expected_note in (
            ("verifying root module (selected functions)", True),
            ("verifying root module, function retained_credit_dispatch_v1::" + selector[1:] + " (selected functions)", False),
            ("unknown selector note", False)):
        stderr = json.dumps(diagnostic) + "\n" + json.dumps(dict(note, message=message))
        need(classifier.logical_negative(notes, 1, json.dumps(negative), stderr, verifier, paths) is expected_note,
             "only actually observed selector note accepted")
    for status in (0, 101, 124, -9, True):
        need(not accepts(notes, status=status), "non-logical status accepted")
    for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
                  {"code": {"code": "E0277"}}, {"message": "timed out"},
                  {"level": "warning"}, {"spans": []},
                  {"spans": [{"file_name": "/tmp/foreign.rs", "is_primary": True}]}):
        need(not accepts(notes, error=dict(diagnostic, **patch)), "non-logical diagnostic accepted")
    for field, value in (("encountered-vir-error", True), ("errors", 0), ("is-verifying-entire-crate", True)):
        malformed = dict(negative, **{"verification-results": dict(negative["verification-results"], **{field: value})})
        need(not accepts(notes, value=malformed), "non-logical summary accepted")
print("retained dispatch calibration: 4 groups passed; 25 logical mutants constructed, not executed")
