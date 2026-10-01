#!/usr/bin/env python3
"""Lightweight source/classifier calibration, never compiler or solver execution."""
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


need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
path = Path(__file__).with_name("check-producer-input-fold.py").resolve()
check = types.ModuleType("producer_input_fold_controls")
check.__file__ = str(path)
sys.modules[check.__name__] = check
exec(compile(path.read_bytes(), str(path), "exec"), check.__dict__)
sources = check.snapshot()
check.audit(sources)


def hostile(path, before, after):
    need(sources[path].count(before) == 1 and before != after, "nonvacuous source control")
    changed = {**sources, path: sources[path].replace(before, after)}
    refused(lambda: check.audit(changed), "changed source accepted")


for path, before, after in (
    (check.OWNER, 'include!("producer_input_fold_body.rs");', '// omitted shared fold'),
    (check.OWNER, "producer_input_fold_body!(", "disconnected_fold_copy!("),
    (check.OWNER, "producer_input_validate_body!(", "disconnected_native_copy!("),
    (check.OWNER, ".has_expected_credit(id, device, byte_len)", ".has_expected_credit(id, device, 0)"),
    (check.PROOF, 'include!("../../fe2o3-runtime/src/context/versions/producer_input_fold_body.rs");', '// disconnected model'),
    (check.PROOF, "owner: C,", "owner: (),"),
    (check.SPEC, "result: Result<ContextProducerReadStatusV1, E>,", "result: Result<ContextProducerReadStatusV1, ()>,"),
    (check.PROOF, "self.records[index].take().unwrap()", "self.records[0].take().unwrap()"),
    (check.PROOF, "final(self).consumed == reached(old(self).original, 0),", "true,"),
    (check.PROOF, "out == fold_result(old(self).original, 0, ContextProducerReadStatusV1::Success,", "out == fold_result(old(self).original, 0, ContextProducerReadStatusV1::Unknown,"),
    (check.PROOF, "before == *old(self),", "before == *self,"),
):
    hostile(path, before, after)
for missing in (check.OWNER, check.PROOF, check.SPEC, check.BODY):
    changed = dict(sources)
    del changed[missing]
    refused(lambda: check.audit(changed), "missing required source accepted")
for extra in (check.SRC / "shadow_fold.rs", check.V / "shadow_fold.rs"):
    refused(lambda: check.audit({**sources, extra: "// unbound\n"}), "additional source accepted")

cases = check.mutations(sources[check.BODY])
expected_names = {
    "error-substitutes-invalid-reference", "error-swallowed-as-success", "unknown-stops-later-validation",
    "unknown-forgets-prior-aggregate", "unknown-demoted", "no-effect-forgets-prior-aggregate", "no-effect-demoted",
    "pending-forgets-prior-aggregate", "pending-demoted", "initial-aggregate-unknown",
    "initial-input-cursor-one", "initial-active-cursor-one", "initial-queued-cursor-one",
    "validation-family-cursors-swapped", "final-active-count-omitted", "final-queued-count-omitted",
    "final-counts-require-both-mismatches", "final-active-count-cross-wired", "final-queued-count-cross-wired",
    "final-count-error-promoted", "validation-duplicated-at-same-index", "success-stops-after-first-input",
}
need(set(cases) == expected_names and len(cases) == check.MUTANT_COUNT == 22,
     "exact proposed controller mutation roster")
need(len({body for body, _ in cases.values()}) == 22, "unique body bytes")
original = sources[check.BODY]
prefix = original[:original.index(check.NATIVE_ANCHOR)]
need(len(prefix.encode("utf-8")) == check.FOLD_PREFIX_BYTES == 2003
     and check.sha(prefix) == check.FOLD_PREFIX_SHA
     == "f38d698075a14e469478805710dd6cf1058346aafc3d1b966efe0518c6a7c991",
     "exact preserved original fold bytes before both scan helpers")
need(all(name not in prefix for name in ("producer_dependency_contains_body", "producer_source_pair_contains_body",
                                        "producer_input_validate_body")), "no helper or validator in fold mutation interval")
native_suffix = original[original.index(check.NATIVE_ANCHOR):]
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for name, (body, focus) in cases.items():
    need(body != original and body[body.index(check.NATIVE_ANCHOR):] == native_suffix,
         "only actual fold controller changed: " + name)
    need(focus == check.SELECTOR and ".clone()" not in body and "assume(" not in body and "admit(" not in body,
         "no duplicate opaque owners, invented authority, or proof-only mutation")
    refused(lambda: check.audit({**sources, check.BODY: body}), "mutant accepted as positive source")
    refused(lambda: check.selection_notes(leaf, focus), "uncalibrated extracted selector accepted")
refused(lambda: check.change(original, "not a source site", "replacement"), "absent mutation accepted")
refused(lambda: check.change(original, "return Err(E::InvalidReference);", "return Ok(status);"),
        "native validator selected as controller mutation")
for before, after in (("if &$dependencies[$index] == $dependency {", "if false {"),
                      ("let original = &$sources[$index];", "let original = &$sources[0];")):
    refused(lambda: check.change(original, before, after), "scan helper selected as fold mutation")
for focus in ("reconcile", "*", "*Observations::validate", "*NativeOwner::quarantine"):
    refused(lambda: check.selection_notes(leaf, focus), "out-of-scope function selector accepted")

base = (check.ROOT / check.BASE).read_bytes()
need(hashlib.sha256(base).hexdigest() == check.BASE_SHA
     and check.controller_source().replace('"--multiple-errors", "1"', '"--multiple-errors", "0"').encode() == base,
     "campaign controller has only the reporting-threshold adaptation")
ast.parse(check.controller_source())
need('"--no-cheating"' in check.controller_source() and '"--multiple-errors", "1"' in check.controller_source(),
     "strict proof command flags with reporting threshold one")
need(check.SELECTION_NOTES is None, "fresh selector calibration is pending")
refused(check.campaign, "uncalibrated campaign launched")
campaign = check.controller()
need(campaign.FILES == [check.PROOF, check.SPEC, check.BODY] and campaign.EXPECTED["verified"] == 13,
     "exact accepted positive closure/count")
count = check.EXPECTED_VERIFIED
try:
    for value in (None, 0, -1, True, "13", 12, 14):
        check.EXPECTED_VERIFIED = value
        refused(check.controller, "unmeasured or malformed positive count accepted")
finally:
    check.EXPECTED_VERIFIED = count

classifier = campaign.inherited()
verifier = {"fixture": "source-calibration-only"}
paths = {str(check.ROOT / path) for path in check.FILES}
notes = types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={"synthetic selection note"})
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
    need(not accepts(error={**diagnostic, **patch}), "compiler/tool/warning/path rejection counted as logical")
for key, value in (("encountered-vir-error", True), ("errors", 0), ("verified", True),
                   ("is-verifying-entire-crate", True), ("success", False)):
    need(not accepts(value={**negative, "verification-results": {**negative["verification-results"], key: value}}),
         "malformed selected proof result accepted")
for message in notes.SELECTION_NOTES:
    note = {**diagnostic, "level": "note", "message": message}
    need(classifier.logical_negative(notes, 1, json.dumps(negative),
         json.dumps(diagnostic) + "\n" + json.dumps(note), verifier, paths), "known exact selection note")
unknown_note = {**diagnostic, "level": "note", "message": "verifying unreviewed function"}
need(not classifier.logical_negative(notes, 1, json.dumps(negative),
     json.dumps(diagnostic) + "\n" + json.dumps(unknown_note), verifier, paths), "selection whitelist is not widened")
print("PASS: producer-input fold source/controller calibration (4 groups; 22 logical mutants constructed, not executed)")
