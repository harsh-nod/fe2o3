#!/usr/bin/env python3
"""Source/controller calibration; logical body mutants are constructed, not run."""
import ast
import hashlib
import json
from pathlib import Path
import sys
import types


def need(condition, message):
    if not condition:
        raise AssertionError(message)


need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
FILE = Path(__file__).resolve().with_name("check-retained-pair-owned-storage.py")
check = types.ModuleType("retained_owned_storage_test")
check.__file__ = str(FILE)
sys.modules[check.__name__] = check
exec(compile(FILE.read_bytes(), str(FILE), "exec"), check.__dict__)
sources = check.snapshot()


def refused(action, message):
    try:
        action()
    except ValueError:
        return
    raise AssertionError(message)


def changed(path, before, after):
    need(sources[path].count(before) == 1 and before != after, "unique hostile source site: " + before)
    candidate = dict(sources)
    candidate[path] = candidate[path].replace(before, after)
    refused(lambda: check.audit(candidate), "edited source accepted: " + before)


check.audit(sources)
for path, before, after in (
    (check.OWNER, 'include!("owned/declarations.rs");', '#[cfg(any())]\ninclude!("owned/declarations.rs");'),
    (check.OWNER, 'include!("owned/bodies.rs");', '#[cfg(any())]\ninclude!("owned/bodies.rs");'),
    (check.OWNER, "retained_pair_owned_parts_body!(self)", "(self.queue, self.destination, self.source)"),
    (check.OWNER, "retained_pair_owned_new_body!(context)", "Self { context: None }"),
    (check.OWNER, "retained_pair_owned_context_body!(self)", "empty_owned_context()"),
    (check.OWNER, "retained_pair_owned_context_mut_body!(self)", "empty_owned_context()"),
    (check.OWNER, "retained_pair_owned_take_body!(self)", "empty_owned_context()"),
    (check.OWNER, "retained_pair_owned_admit_post_body!(self, result)", "Ok(self)"),
    (check.OWNER, "retained_pair_owned_finish_post_body!(self, result)", "Ok(self.take())"),
    (check.OWNER, "retained_pair_owned_recover_body!(self)", "Err(self)"),
    (check.OWNER, "let result = run_operation(self.context_mut(), close);", "let result = close(self.context_mut());"),
    (check.OWNER, "std::process::abort();", "return;"),
    (check.DECLARATIONS, "struct Parts<Q, S, D> {", "#[derive(Clone, Copy)]\nstruct Parts<Q, S, D> {"),
    (check.DECLARATIONS, "context: Option<C>,", "context: Option<()>,"),
    (check.DECLARATIONS, "error: E,", "error: (),"),
    (check.PROOF, "requires false,", "requires true,"),
    (check.PROOF, "#[verifier::external_body]", "#[verifier::external]"),
    (check.PROOF, "fn terminal(&self) -> bool;", "fn terminal(&self) -> (value: bool) ensures !value;"),
    (check.PROOF, "ensures returning_owned_quarantine_effect(*old(self), *final(self));", "ensures true;"),
    (check.PROOF, "final(self).context == Some(*final(result)),", "final(self).context.is_some(),"),
    (check.PROOF, "            result == old(self).context.unwrap(),", "            true,"),
    (check.PROOF, "final(self).context.is_none(),", "true,"),
    (check.PROOF, "failure.error == error", "true"),
    (check.PROOF, "Some(context) == self.owner.context", "self.owner.context.is_some()"),
    (check.PROOF, "Err(failure) => failure == self,", "Err(failure) => true,"),
    (check.PROOF, "use vstd::prelude::verus as retained_pair_owned_declarations_v1;",
     "macro_rules! retained_pair_owned_declarations_v1 { ($($items:tt)*) => {}; }"),
):
    changed(path, before, after)
for missing in (check.OWNER, check.DECLARATIONS, check.BODY, check.PROOF):
    candidate = dict(sources)
    del candidate[missing]
    refused(lambda: check.audit(candidate), "missing source accepted")
for path in (check.SRC / "unbound_owned_shadow.rs", check.V / "unbound_owned_proof.rs"):
    candidate = dict(sources)
    candidate[path] = "// extra unbound file\n"
    refused(lambda: check.audit(candidate), "extra source accepted")

cases = check.mutations(sources[check.BODY])
need(len(cases) == check.MUTANT_COUNT == 21 and len({body for body, _ in cases.values()}) == 21,
     "distinct constructed logical controls")
expected_selectors = {"*Parts::into_parts", "*Owned::new", "*Owned::context", "*Owned::context_mut",
                      "*Owned::take", "*Owned::admit_post", "*Owned::finish_post", "*Failure::recover_unadmitted"}
need({selector for _, selector in cases.values()} == expected_selectors, "all eight storage methods covered")
leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied", "precondition not satisfied"})
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY], "actual executable mutation: " + name)
    need(".clone()" not in body and "assume(" not in body and "admit(" not in body,
         "no invented duplicate payloads or assumed mutation facts")
    candidate = dict(sources)
    candidate[check.BODY] = body
    refused(lambda: check.audit(candidate), "mutated body accepted as positive source")
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS == leaf.LOGICAL_ERRORS and len(notes.SELECTION_NOTES) == 2,
         "unchanged strict logical diagnostic class")
need("recover-ignores-terminal" not in cases and "parts-swaps-directions" not in cases,
     "do not count intentionally unproved calls or Rust type errors as logical controls")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "unbound selection accepted")
refused(lambda: check.change(sources[check.BODY], "absent", "a", "b"), "absent macro accepted")

adapted = check.controller_source()
ast.parse(adapted)
base = (check.ROOT / check.BASE).read_bytes()
need(hashlib.sha256(base).hexdigest() == check.BASE_SHA, "pinned inherited controller")
restored = adapted.replace('"--triggers-mode", "silent", "--output-json"',
                           '"--triggers-mode", "silent", "--no-cheating", "--output-json"')
restored = restored.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"')
need(restored.encode() == base, "only disclosed trust flag and first-error command changed")
need('"--no-cheating"' not in adapted, "conditional adapter is not advertised as no-cheating")
campaign = check.campaign()
need(campaign.FILES == [check.PROOF, check.DECLARATIONS, check.BODY]
     and campaign.PROOF == check.PROOF and campaign.BODY == check.BODY
     and campaign.EXPECTED["verified"] == 8, "exact proof closure and measured eight-count")

configured = check.EXPECTED_VERIFIED
try:
    for unset in (None, 0, -1, True, "8"):
        check.EXPECTED_VERIFIED = unset
        refused(check.campaign, "unmeasured or malformed proof count accepted")
finally:
    check.EXPECTED_VERIFIED = configured

classifier = campaign.inherited()
verifier = {"fixture": "source-calibration-only"}
paths = {str(check.ROOT / path) for path in check.FILES}
notes = check.selection_notes(leaf, "*Owned::take")
negative = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False, "verified": 0,
    "errors": 1, "is-verifying-entire-crate": False}}
diagnostic = {"$message_type": "diagnostic", "message": "postcondition not satisfied",
              "level": "error", "code": None, "children": [],
              "spans": [{"file_name": str(check.ROOT / check.PROOF), "is_primary": True}]}


def accepts(value=negative, error=diagnostic, status=1):
    return classifier.logical_negative(notes, status, json.dumps(value), json.dumps(error), verifier, paths)


need(accepts(), "nonvacuous synthetic logical negative")
for status in (0, 101, 124, -9):
    need(not accepts(status=status), "non-logical exit accepted")
for patch in ({"message": "mismatched types", "code": {"code": "E0308"}},
              {"message": "precondition not satisfied", "code": {"code": "E0277"}},
              {"message": "timed out"}, {"level": "warning"}, {"spans": []},
              {"spans": [{"file_name": "/tmp/unrelated.rs", "is_primary": True}]}):
    need(not accepts(error={**diagnostic, **patch}), "compiler/tool/path error accepted as logical")
for key, value in (("encountered-vir-error", True), ("errors", 0), ("verified", True),
                   ("is-verifying-entire-crate", True), ("success", False)):
    need(not accepts(value={**negative, "verification-results": {**negative["verification-results"], key: value}}),
         "malformed negative schema accepted")
print("PASS: retained-pair owned-storage source/controller calibration (5 groups; 21 logical mutants constructed, not executed)")
