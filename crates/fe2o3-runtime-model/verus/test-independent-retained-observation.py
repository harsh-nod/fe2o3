#!/usr/bin/env python3
"""Light source and strict-classifier controls; no compiler or solver execution."""
import ast
import json
from pathlib import Path
import sys
import types


def need(value, message):
    if not value:
        raise AssertionError(message)


need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
path = Path(__file__).resolve().with_name("check-independent-retained-observation.py")
check = types.ModuleType("independent_retained_observation_test")
check.__file__ = str(path)
sys.modules[check.__name__] = check
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
sources = check.snapshot()


def refused(action, message):
    try:
        action()
    except ValueError:
        return
    raise AssertionError(message)


check.audit(sources)
check.audit_dispatch_forwarding(sources)
need(check.include_closure(sources) == set(check.FILES) and len(check.FILES) == 5,
     "five actual transitive executable inputs")
for selected in sources:
    candidate = dict(sources)
    candidate[selected] += "\n"
    refused(lambda: check.audit(candidate), "drifted source accepted")
    del candidate[selected]
    refused(lambda: check.audit(candidate), "missing source accepted")
candidate = dict(sources)
candidate[check.SRC / "unbound_observer.rs"] = "// additional source\n"
refused(lambda: check.audit(candidate), "extra accounting source accepted")
for selected, before, after in (
    (check.DISPATCH, "let Some(token) = &$credits.token", "let Some(token) = &$other.token"),
    (check.DISPATCH, "if Arc::ptr_eq(account, actual)", "if true"),
    (check.DISPATCH, "account.state.lock()", "account.lock()"),
    (check.DISPATCH, "&state.records,", "&[],"),
    (check.DISPATCH, "state.poisoned,", "false,"),
    (check.DISPATCH, "token.slot,", "0,"),
    (check.DISPATCH, "token.owner,", "0,"),
    (check.OWNER, "    owner: u64,", "    owner: u32,"),
    (check.PROOF, "&&& !poisoned", "&&& true"),
    (check.PROOF, "&&& records[slot as int].is_some()", "&&& true"),
    (check.PROOF, "expected.counts@", "records[slot as int]->Some_0.charge.counts@"),
    (check.RECORD_PROOF, "owner != 0 && self.owner == owner", "true"),
):
    need(before in sources[selected], "hostile source anchor")
    candidate = dict(sources)
    candidate[selected] = candidate[selected].replace(before, after)
    if selected == check.DISPATCH:
        refused(lambda: check.audit_dispatch_forwarding(candidate), "hostile macro forwarding accepted")
    refused(lambda: check.audit(candidate), "hostile source accepted")

for before, after in (
    ('include!("retained_dispatch_body.rs");', 'include!("other.rs");'),
    ('resource_retained_credit_dispatch_body_v1!(', 'resource_retained_credit_dispatch_body_v1_other!('),
):
    need(sources[check.OWNER].count(before) == 1, "one native dispatch calibration anchor")
    candidate = dict(sources)
    candidate[check.OWNER] = candidate[check.OWNER].replace(before, after)
    refused(lambda: check.audit_dispatch_forwarding(candidate), "foreign dispatch route accepted")
    refused(lambda: check.audit(candidate), "foreign dispatch source accepted")

cases = check.mutations(sources[check.BODY])
need(len(cases) == check.MUTANT_COUNT == 10, "ten constructed actual-body controls")
need("Some(record) => { let _ = record; true }," in cases["ignore-credit-record"][0],
     "predicate bypass still uses its borrowed record; no unused-variable warning substitute")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
bounds_error = "precondition not met: index in bounds for this access"
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY], "actual observer changed: " + name)
    candidate = dict(sources)
    candidate[check.BODY] = body
    refused(lambda: check.audit(candidate), "mutant accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS == leaf.LOGICAL_ERRORS | {bounds_error} and len(notes.SELECTION_NOTES) == 2,
         "strict selector and exact scoped logical bounds diagnostic")
need(leaf.LOGICAL_ERRORS == {"postcondition not satisfied", "precondition not satisfied"},
     "inherited logical diagnostics unchanged")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "unbound selector accepted")

adapted = check.controller_source()
ast.parse(adapted)
need(adapted.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode()
     == (check.ROOT / check.BASE).read_bytes(), "only first-error limit adapted")
need('"--no-cheating"' in adapted, "no assumed/external proof-body flag relaxation")
module = check.controller()
need(module.FILES == check.FILES and module.PROOF == check.PROOF and module.BODY == check.BODY,
     "exact actual executable proof closure and mutant target")
need(type(check.EXPECTED_VERIFIED) is int and check.EXPECTED_VERIFIED == 8,
     "exact measured eight-obligation full positive")
need(check.campaign().EXPECTED["verified"] == 8, "campaign uses measured positive count")
original = check.EXPECTED_VERIFIED
try:
    for value in (None, 0, -1, True, "1"):
        check.EXPECTED_VERIFIED = value
        refused(check.campaign, "unmeasured/malformed count accepted")
finally:
    check.EXPECTED_VERIFIED = original

classifier = module.inherited()
verifier = {"fixture": "synthetic-calibration-only"}
paths = {str(check.ROOT / file) for file in check.FILES}
expected = check.campaign().EXPECTED
positive = {"verus": verifier, "verification-results": expected}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, expected, paths),
     "exact measured positive classifier control")
for count in (0, 7, 9, True, "8"):
    changed = dict(positive, **{"verification-results": dict(expected, verified=count)})
    need(not classifier.proof_positive(0, json.dumps(changed), "", verifier, expected, paths),
         "wrong positive count accepted")
notes = check.selection_notes(leaf, "*independent_retained_observation_v1")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "verified": 0, "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous logical negative control")
need(accepts(error=dict(diagnostic, message="precondition not satisfied")), "logical precondition rejection")
bounds_diagnostic = dict(diagnostic, message=bounds_error, spans=[{
    "file_name": str(check.ROOT / check.BODY), "is_primary": True,
}])
need(accepts(error=bounds_diagnostic), "exact logical bounds diagnostic at actual shared body")
for status in (0, 101, 124, -9):
    need(not accepts(error=bounds_diagnostic, status=status), "bounds diagnostic with invalid status accepted")
for patch in ({"code": {"code": "E0308"}}, {"code": {"code": "E0277"}}, {"code": "E0308"},
              {"level": "warning"}, {"level": "note"}, {"spans": []},
              {"spans": [{"file_name": "/tmp/foreign.rs", "is_primary": True}]},
              {"spans": [{"file_name": str(check.ROOT / check.BODY), "is_primary": False}]},
              {"message": "precondition not met"}, {"message": bounds_error + " extra"},
              {"message": "precondition not met: index out of bounds"}):
    need(not accepts(error=dict(bounds_diagnostic, **patch)), "non-exact logical bounds diagnostic accepted")
for status in (0, 101, 124, -9):
    need(not accepts(status=status), "non-logical status accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"message": "postcondition not satisfied", "code": {"code": "E0277"}},
              {"message": "timed out"}, {"level": "warning"}, {"spans": []}):
    need(not accepts(error=dict(diagnostic, **patch)), "non-logical diagnostic accepted")
for field, value in (("encountered-vir-error", True), ("errors", 0)):
    malformed = dict(negative, **{"verification-results": dict(negative["verification-results"], **{field: value})})
    need(not accepts(value=malformed), "non-logical summary accepted")
print("independent retained observation calibration: 4 groups passed; 10 logical mutants constructed, not executed")
