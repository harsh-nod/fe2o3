#!/usr/bin/env python3
"""Light source/strict-classifier controls; no compiler, formatter or solver."""
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
path = Path(__file__).resolve().with_name("check-request-charge.py")
check = types.ModuleType("request_charge_test")
check.__file__ = str(path)
sys.modules[check.__name__] = check
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
sources = check.snapshot()
check.audit(sources)
need(len(sources) == 16 and check.include_closure(sources) == set(check.FILES) and len(check.FILES) == 3,
     "exact selected input and executable rosters")
for selected in sources:
    changed = dict(sources)
    changed[selected] += "\n"
    refused(lambda: check.audit(changed), "drifted source accepted")
    del changed[selected]
    refused(lambda: check.audit(changed), "missing source accepted")
changed = dict(sources)
changed[check.SRC / "unbound.rs"] = "// extra\n"
refused(lambda: check.audit(changed), "extra source accepted")
for selected, before, after in (
    (check.OWNER, "LogicalPayloadBytes,\n    RequestedAllocationBytes", "RequestedAllocationBytes,\n    LogicalPayloadBytes"),
    (check.OWNER, "#[repr(usize)]", "#[repr(u8)]"),
    (check.RUNTIME, "r67_requested_allocation_charge_v1(bytes)", "r67_requested_allocation_charge_v1(0)"),
    (check.KFD, "r67_requested_allocation_charge_v1(bytes)", "r67_requested_allocation_charge_v1(1)"),
    (check.VECTOR, "usize = 19", "usize = 20"),
):
    need(sources[selected].count(before) == 1, "unique hostile source anchor")
    changed = dict(sources)
    changed[selected] = changed[selected].replace(before, after)
    original = check.SOURCE_TREE_SHA
    try:
        # Exercise schema/forwarding guards independently of the whole-input pin.
        check.SOURCE_TREE_SHA = check.tree_hash({p: t for p, t in changed.items() if p != check.PROOF})
        refused(lambda: check.audit(changed), "hostile schema/forwarding accepted despite rebound fixture hash")
    finally:
        check.SOURCE_TREE_SHA = original

cases = check.mutations(sources[check.BODY])
need(len(cases) == check.MUTANT_COUNT == 21, "twenty-one actual coordinate mutants constructed")
need({name for name in cases if name.startswith("contaminated-coordinate-")} == {
    f"contaminated-coordinate-{index:02d}" for index in range(19) if index not in (1, 18)
}, "all seventeen zero coordinates are tested")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY], "nonvacuous actual constructor mutation")
    changed = dict(sources)
    changed[check.BODY] = body
    refused(lambda: check.audit(changed), "mutant accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS is leaf.LOGICAL_ERRORS and len(notes.SELECTION_NOTES) == 2,
         "unchanged logical error set and exact constructor selector")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "foreign selector accepted")

adapted = check.controller_source()
ast.parse(adapted)
need(adapted.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode()
     == (check.ROOT / check.BASE).read_bytes(), "only first-error limit adapted")
need('"--no-cheating"' in adapted, "full no-cheating policy retained")
module = check.controller()
need(module.FILES == check.FILES and module.PROOF == check.PROOF and module.BODY == check.BODY,
     "actual three-file proof closure and mutation target")
need(check.EXPECTED_VERIFIED == 3 and check.campaign().EXPECTED["verified"] == 3,
     "measured full-root count must remain three")
original = check.EXPECTED_VERIFIED
try:
    for value in (None, 0, -1, True, "3"):
        check.EXPECTED_VERIFIED = value
        refused(check.campaign, "malformed count accepted")
finally:
    check.EXPECTED_VERIFIED = original

classifier = module.inherited()
verifier = {"fixture": "synthetic-calibration-only"}
paths = {str(check.ROOT / file) for file in check.FILES}
# Synthetic classifier control; the measured count is pinned above.
expected = dict(module.EXPECTED, verified=3)
positive = {"verus": verifier, "verification-results": expected}
need(classifier.proof_positive(0, json.dumps(positive), "", verifier, expected, paths), "strict synthetic positive")
for count in (0, 2, 4, True, "3"):
    changed = dict(positive, **{"verification-results": dict(expected, verified=count)})
    need(not classifier.proof_positive(0, json.dumps(changed), "", verifier, expected, paths), "wrong fixture count accepted")
notes = check.selection_notes(leaf, "*r67_requested_allocation_charge_v1")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "verified": 0, "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return type(status) is int and classifier.logical_negative(
        notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous logical rejection control")
for status in (0, 101, 124, -9, True):
    need(not accepts(status=status), "non-logical status accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"code": {"code": "E0277"}}, {"message": "timed out"},
              {"level": "warning"}, {"spans": []},
              {"spans": [{"file_name": "/tmp/foreign.rs", "is_primary": True}]}):
    need(not accepts(error=dict(diagnostic, **patch)), "non-logical diagnostic accepted")
for field, value in (("encountered-vir-error", True), ("errors", 0), ("is-verifying-entire-crate", True)):
    malformed = dict(negative, **{"verification-results": dict(negative["verification-results"], **{field: value})})
    need(not accepts(value=malformed), "non-logical summary accepted")
print("request charge calibration: 4 groups passed; 21 logical mutants constructed, not executed")
