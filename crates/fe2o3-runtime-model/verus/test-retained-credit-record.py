#!/usr/bin/env python3
"""Light source/classifier controls; logical body mutants are not executed here."""
import ast
import json
from pathlib import Path
import sys
import types


def need(value, message):
    if not value:
        raise AssertionError(message)


need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize,
     "use python3 -I -B")
path = Path(__file__).resolve().with_name("check-retained-credit-record.py")
check = types.ModuleType("retained_credit_record_test")
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
for selected, before, after in (
    (check.OWNER, 'include!("retained_charge_body.rs");',
     '#[cfg(any())]\ninclude!("retained_charge_body.rs");'),
    (check.OWNER, "retained_credit_record_matches_body!(self, owner, expected)", "true"),
    (check.OWNER, "    charge: ResourceVectorV1,", "    charge: u64,"),
    (check.PHASE, "    Quarantined,\n    Vacant,", "    Quarantined,"),
    (check.VECTOR, "DIMENSIONS_V1: usize = 19", "DIMENSIONS_V1: usize = 18"),
    (check.PROOF, "owner != 0 && self.owner == owner", "self.owner == owner"),
    (check.PROOF, "self.charge.counts@ == expected.counts@", "true"),
    (check.PROOF, "fn obeys_eq_spec() -> bool { true }", "fn obeys_eq_spec() -> bool { false }"),
):
    need(before in sources[selected], "hostile source anchor")
    candidate = dict(sources)
    candidate[selected] = candidate[selected].replace(before, after)
    refused(lambda: check.audit(candidate), "edited source accepted")
for selected in (*check.FILES, check.OWNER, check.PHASE):
    candidate = dict(sources)
    del candidate[selected]
    refused(lambda: check.audit(candidate), "missing source accepted")
candidate = dict(sources)
candidate[check.SRC / "unbound_predicate.rs"] = "// extra source\n"
refused(lambda: check.audit(candidate), "unbound source accepted")

cases = check.mutations(sources[check.BODY])
need(len(cases) == check.MUTANT_COUNT == 11, "eleven constructed logical controls")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied"})
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY], "actual predicate changed: " + name)
    candidate = dict(sources)
    candidate[check.BODY] = body
    refused(lambda: check.audit(candidate), "mutant accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS == leaf.LOGICAL_ERRORS and len(notes.SELECTION_NOTES) == 2,
         "strict inherited diagnostics")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "unbound selector accepted")

adapted = check.controller_source()
ast.parse(adapted)
need(adapted.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode()
     == (check.ROOT / check.BASE).read_bytes(), "only first-error limit adapted")
need('"--no-cheating"' in adapted, "no assumed/external proof-body flag relaxation")
module = check.controller()
need(module.FILES == [check.PROOF, check.VECTOR, check.BODY], "exact three-file closure")
need(type(check.EXPECTED_VERIFIED) is int and check.EXPECTED_VERIFIED == 7,
     "exact measured seven-obligation full positive")
need(check.campaign().EXPECTED["verified"] == 7, "campaign uses the measured positive count")
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
notes = check.selection_notes(leaf, "*Record::matches_retained_charge")
positive = {"verus": verifier, "verification-results": check.campaign().EXPECTED}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier,
                              check.campaign().EXPECTED, paths), "exact positive classifier control")
for count in (0, 6, 8, True, "7"):
    changed = dict(positive, **{"verification-results": dict(positive["verification-results"], verified=count)})
    need(not classifier.proof_positive(0, json.dumps(changed), "", verifier,
                                       check.campaign().EXPECTED, paths), "wrong positive count accepted")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "verified": 0, "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous logical negative classifier control")
for status in (0, 101, 124, -9):
    need(not accepts(status=status), "non-logical status accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"message": "postcondition not satisfied", "code": {"code": "E0277"}},
              {"message": "timed out"}, {"level": "warning"}, {"spans": []}):
    need(not accepts(error=dict(diagnostic, **patch)), "non-logical diagnostics accepted")
for field, value in (("encountered-vir-error", True), ("errors", 0)):
    malformed = dict(negative, **{"verification-results": dict(negative["verification-results"], **{field: value})})
    need(not accepts(value=malformed), "non-logical proof summary accepted")
print("retained-credit record calibration: 4 groups passed; 11 logical mutants constructed, not executed")
