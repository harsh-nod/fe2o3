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
path = Path(__file__).resolve().with_name("check-domain-retained-observation.py")
check = types.ModuleType("domain_retained_observation_test")
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
need(check.include_closure(sources) == set(check.FILES) and len(check.FILES) == 7,
     "seven actual transitive executable inputs")
for selected in sources:
    candidate = dict(sources)
    candidate[selected] += "\n"
    refused(lambda: check.audit(candidate), "drifted source accepted")
    del candidate[selected]
    refused(lambda: check.audit(candidate), "missing source accepted")
candidate = dict(sources)
candidate[check.SRC / "domain/unbound_observer.rs"] = "// additional source\n"
refused(lambda: check.audit(candidate), "extra accounting source accepted")
for selected, before, after in (
    (check.OWNER, "&state.nodes,", "&[],"),
    (check.OWNER, "state.max_depth,", "4,"),
    (check.OWNER, "&state.records,", "&[],"),
    (check.OWNER, "state.poisoned,", "false,"),
    (check.OWNER, "self.root.state.lock()", "self.root.lock()"),
    (check.OWNER, "    credit: Record,", "    credit: bool,"),
    (check.PROOF, "&&& accepted(nodes, profile, key)", "&&& true"),
    (check.PROOF, "&&& records[slot as int]->Some_0.leaf == key", "&&& true"),
    (check.PROOF, "expected.counts@", "records[slot as int]->Some_0.credit.charge.counts@"),
    (check.RECORD_PROOF, "owner != 0 && self.owner == owner", "true"),
    (check.ARENA_BODY, "node.key.generation == $key.generation", "true"),
):
    need(before in sources[selected], "hostile source anchor")
    candidate = dict(sources)
    candidate[selected] = candidate[selected].replace(before, after)
    refused(lambda: check.audit(candidate), "hostile source accepted")

cases = check.mutations(sources[check.BODY])
need(len(cases) == check.MUTANT_COUNT == 15, "fifteen constructed actual-body controls")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY], "actual observer changed: " + name)
    candidate = dict(sources)
    candidate[check.BODY] = body
    refused(lambda: check.audit(candidate), "mutant accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS == leaf.LOGICAL_ERRORS and len(notes.SELECTION_NOTES) == 2,
         "strict selector and inherited logical diagnostics")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "unbound selector accepted")

adapted = check.controller_source()
ast.parse(adapted)
need(adapted.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode()
     == (check.ROOT / check.BASE).read_bytes(), "only first-error limit adapted")
need('"--no-cheating"' in adapted, "no assumed/external proof-body flag relaxation")
module = check.controller()
need(module.FILES == check.FILES and module.PROOF == check.PROOF and module.BODY == check.BODY,
     "exact actual executable proof closure and mutant target")
need(type(check.EXPECTED_VERIFIED) is int and check.EXPECTED_VERIFIED == 21,
     "exact measured 21-obligation full positive")
need(check.campaign().EXPECTED["verified"] == 21, "campaign uses the measured positive count")
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
positive = {"verus": verifier, "verification-results": check.campaign().EXPECTED}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier,
                              check.campaign().EXPECTED, paths), "exact positive classifier control")
for count in (0, 20, 22, True, "21"):
    changed = dict(positive, **{"verification-results": dict(positive["verification-results"], verified=count)})
    need(not classifier.proof_positive(0, json.dumps(changed), "", verifier,
                                       check.campaign().EXPECTED, paths), "wrong positive count accepted")
notes = check.selection_notes(leaf, "*domain_retained_observation_v1")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "verified": 0, "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous logical negative control")
need(accepts(error=dict(diagnostic, message="precondition not satisfied")), "bounds logical rejection control")
for status in (0, 101, 124, -9):
    need(not accepts(status=status), "non-logical status accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"message": "postcondition not satisfied", "code": {"code": "E0277"}},
              {"message": "timed out"}, {"level": "warning"}, {"spans": []}):
    need(not accepts(error=dict(diagnostic, **patch)), "non-logical diagnostic accepted")
for field, value in (("encountered-vir-error", True), ("errors", 0)):
    malformed = dict(negative, **{"verification-results": dict(negative["verification-results"], **{field: value})})
    need(not accepts(value=malformed), "non-logical summary accepted")
print("domain retained observation calibration: 4 groups passed; 15 logical mutants constructed, not executed")
