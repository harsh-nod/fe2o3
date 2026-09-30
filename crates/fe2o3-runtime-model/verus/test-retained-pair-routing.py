#!/usr/bin/env python3
"""Source/classifier calibration only; no compiler, solver or native execution."""
import ast
import hashlib
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


need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize,
     "use python3 -I -B")
path = Path(__file__).with_name("check-retained-pair-routing.py").resolve()
check = types.ModuleType("retained_pair_routing_controls")
check.__file__ = str(path)
sys.modules[check.__name__] = check
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
sources = check.snapshot()
check.audit(sources)


def hostile(path, before, after):
    need(sources[path].count(before) == 1 and before != after, "nonvacuous source control")
    changed = {**sources, path: sources[path].replace(before, after)}
    refused(lambda: check.audit(changed), "changed source accepted")


for selected, before, after in (
    (check.OWNER, 'include!("xgmi_retained/routing_bodies.rs");', '// disconnected controller'),
    (check.OWNER, "publish_once(phase, |requests| work.submit(requests))", "publish_once(phase, |_| work.submit(Vec::new()))"),
    (check.OWNER, "                retained.deadline,", "                Instant::now(),"),
    (check.OWNER, "self.owner.wait_batch_until(tickets, deadline)", "self.owner.wait_batch_for(tickets, deadline)"),
    (check.DECLARATIONS, "Prepared(Vec<R>),", "Prepared(Vec<()>),"),
    (check.DECLARATIONS, "Waited(Result<C, E>),", "Waited(Result<(), E>),"),
    (check.PROOF, 'include!("../../fe2o3-runtime/src/kfd_backend/xgmi_retained/routing_bodies.rs");', '// disconnected proof'),
    (check.PROOF, "    owner: O,", "    owner: (),"),
    (check.PROOF, "self.submit_return.take().unwrap()", "self.submit_return.unwrap()"),
    (check.PROOF, "out == advanced(phase, old(self).submit_return, old(self).wait_return),", "true,"),
    (check.PROOF, "final(self).deadline == Some(deadline) && final(self).wait_return.is_none(),", "final(self).wait_return.is_none(),"),
):
    hostile(selected, before, after)
for missing in (check.OWNER, check.DECLARATIONS, check.BODY, check.PROOF):
    changed = dict(sources)
    del changed[missing]
    refused(lambda: check.audit(changed), "missing required source accepted")
for extra in (check.SRC / "unbound_routing.rs", check.V / "unbound_routing.rs"):
    refused(lambda: check.audit({**sources, extra: "// unbound\n"}), "additional source accepted")

cases = check.mutations(sources[check.BODY])
expected_names = {
    "submit-success-promoted-ready", "submit-error-promoted-ready", "submit-receives-empty-requests",
    "unpublished-inactive-phase-erased", "submit-failure-reopened-prepared", "pending-skips-wait",
    "pending-promoted-ready", "wait-receives-empty-tickets", "wait-result-erased",
    "unchanged-error-phase-erased", "published-phase-erased",
}
need(set(cases) == expected_names and len(cases) == check.MUTANT_COUNT == 11,
     "exact proposed routing mutation roster")
need(len({body for body, _ in cases.values()}) == 11, "distinct mutated actual body bytes")
need(sum(focus == check.SELECTORS["publish"] for _, focus in cases.values()) == 5
     and sum(focus == check.SELECTORS["advance"] for _, focus in cases.values()) == 6,
     "exact actual publish/composed-advance selector split")
for name, borrow in (("submit-receives-empty-requests", "let _ = &$requests;"),
                     ("pending-promoted-ready", "let _ = &$tickets;"),
                     ("wait-receives-empty-tickets", "let _ = &$tickets;")):
    need(cases[name][0].count(borrow) == 1,
         "mutant uses the original binding without changing its routing violation: " + name)
for name, (body, focus) in cases.items():
    need(body != sources[check.BODY] and focus in check.SELECTORS.values(), "real routing mutation: " + name)
    need(".clone()" not in body and "assume(" not in body and "admit(" not in body,
         "no duplicate opaque ownership or proof-only premise")
    refused(lambda: check.audit({**sources, check.BODY: body}), "mutant accepted as positive source")
refused(lambda: check.change(sources[check.BODY], "not-a-macro", "one", "two"),
        "absent mutation macro accepted")
refused(lambda: check.change(sources[check.BODY], "retained_pair_advance_body", "missing", "two"),
        "absent mutation anchor accepted")

measured_notes = {
    "*Observations::publish": frozenset({"verifying root module (selected functions)"}),
    "*Observations::advance": frozenset({"verifying root module (selected functions)"}),
}
need(check.EXPECTED_VERIFIED == 4 and check.SELECTION_NOTES == measured_notes,
     "measured full positive count and exact independently reviewed selection diagnostics")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for focus in check.SELECTORS.values():
    need(check.selection_notes(leaf, focus).SELECTION_NOTES == measured_notes[focus],
         "exact measured per-selector singleton allowlist")
for focus in ("*", "*NativeWork::wait", "*Observations::submit", "*Observations::advance_typo"):
    refused(lambda: check.selection_notes(leaf, focus), "unreviewed selector accepted")
try:
    for malformed in (None, {}, {"*Observations::publish": frozenset()},
                      {name: frozenset() for name in measured_notes}):
        check.SELECTION_NOTES = malformed
        refused(check.campaign, "unmeasured or empty selection allowlist accepted")
finally:
    check.SELECTION_NOTES = measured_notes
base = (check.ROOT / check.BASE).read_bytes()
need(hashlib.sha256(base).hexdigest() == check.BASE_SHA and check.controller_source().encode() == base,
     "exact unchanged managed campaign controller")
ast.parse(check.controller_source())
need('"--no-cheating"' in check.controller_source() and '"--multiple-errors", "0"' in check.controller_source(),
     "unchanged strict proof command flags")
controller = check.campaign()
need(controller.FILES == check.FILES and controller.PROOF == check.PROOF
     and controller.BODY == check.BODY and controller.EXPECTED["verified"] == 4,
     "campaign uses exact shared source closure and measured full positive count")
classifier = controller.inherited()
verifier = {"fixture": "source-calibration-only"}
paths = {str(check.ROOT / path) for path in check.FILES}
notes = check.selection_notes(leaf, "*Observations::publish")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False, "verified": 0,
    "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous synthetic logical-negative acceptance")
for status in (0, 101, 124, -9):
    need(not accepts(status=status), "non-logical exit accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"message": "precondition not satisfied", "code": {"code": "E0277"}},
              {"message": "timed out"}, {"level": "warning"}, {"spans": []},
              {"spans": [{"file_name": "/tmp/wrong.rs", "is_primary": True}]}):
    need(not accepts(error={**diagnostic, **patch}), "compiler/tool/warning/path error counted as logical")
for key, value in (("encountered-vir-error", True), ("errors", 0), ("verified", True),
                   ("is-verifying-entire-crate", True), ("success", False)):
    need(not accepts(value={**negative, "verification-results": {**negative["verification-results"], key: value}}),
         "malformed selected-proof result accepted")
unknown_note = {**diagnostic, "level": "note", "message": "verifying an unmeasured routing selection"}
measured_note = {**unknown_note, "message": "verifying root module (selected functions)", "spans": []}
need(classifier.logical_negative(notes, 1, json.dumps(negative),
     json.dumps(diagnostic) + "\n" + json.dumps(measured_note), verifier, paths),
     "nonvacuous measured selection-note classifier calibration")
need(not classifier.logical_negative(notes, 1, json.dumps(negative),
     json.dumps(diagnostic) + "\n" + json.dumps(unknown_note), verifier, paths),
     "unknown selection notes remain rejected")
print("PASS: retained-routing source/classifier calibration (4 groups; 11 mutants constructed, not executed)")
