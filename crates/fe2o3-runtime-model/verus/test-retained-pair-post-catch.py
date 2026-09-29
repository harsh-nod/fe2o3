#!/usr/bin/env python3
"""Source/controller calibration only; this does NOT execute logical mutants."""
import ast
import hashlib
from pathlib import Path
import sys
import types

def need(condition, message):
    if not condition:
        raise AssertionError(message)


need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
FILE = Path(__file__).resolve().with_name("check-retained-pair-post-catch.py")
check = types.ModuleType("retained_post_catch_test")
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
    need(sources[path].count(before) == 1, "unique hostile source site: " + before)
    candidate = dict(sources)
    candidate[path] = candidate[path].replace(before, after)
    refused(lambda: check.audit(candidate), "edited source accepted: " + before)


check.audit(sources)
retained = check.SRC / "sdma/retained_pair.rs"
sdma = check.SRC / "sdma.rs"
memory = check.SRC / "shared_memory.rs"
for path, before, after in (
    (retained, "use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};",
     "use crate::other::{AssertUnwindSafe, catch_unwind, resume_unwind};"),
    (retained, "fn settle_operation<C: Custody, R: TerminalOutcome, P>(",
     "#[cfg(any())]\nfn settle_operation<C: Custody, R: TerminalOutcome, P>("),
    (retained, "retained_pair_terminal_body!(self)", "false"),
    (retained, "retained_pair_settle_body!(context, outcome)", "loop {}"),
    (retained, "retained_pair_unit_outcome_body!(self)", "self"),
    (retained, "retained_pair_tickets_outcome_body!(self)", "self"),
    (retained, "retained_pair_completed_outcome_body!(self)", "self"),
    (retained, "retained_pair_close_body!(self, close)", "close(&mut self.context)"),
    (retained, "retained_pair_finish_terminal_body!(self)", "self.finished = true"),
    (retained, "retained_pair_drop_body!(self)", "let _ = self"),
    (retained, "retained XGMI success has terminal custody", "different terminal error"),
    (sdma, "pub enum Gfx942SdmaErrorV1 {", "#[cfg(any())]\npub enum Gfx942SdmaErrorV1 {"),
    (sdma, "pub enum Gfx942XgmiBatchSubmissionFailureV1 {", "#[cfg(any())]\npub enum Gfx942XgmiBatchSubmissionFailureV1 {"),
    (sdma, "pub enum Gfx942XgmiBatchWaitFailureV1 {", "#[cfg(any())]\npub enum Gfx942XgmiBatchWaitFailureV1 {"),
    (sdma, "if self.destroyed || self.poisoned {", "if self.destroyed {"),
    (memory, "if let Some(usage) = self\n            .composed_request_account", "if let Some(usage) = self\n            .host_backing_account"),
    (memory, "pub enum SharedMemorySessionPhaseV1 {", "#[cfg(any())]\npub enum SharedMemorySessionPhaseV1 {"),
    (check.BODY, "Settled::Resume(payload) => resume_unwind(payload)",
     "Settled::Resume(payload) => resume_unwind(Box::new(()))"),
    (check.BODY, "let outcome = catch_unwind(AssertUnwindSafe(|| $operation($context)));",
     "let outcome = Ok($operation($context));"),
    (check.BODY, "match settle_operation($context, outcome)", "match settle_operation($context, Ok(outcome))"),
    (check.BODY, "retained_pair_close_post_body!($scope, result)", "result"),
    (check.BODY, "Settled::Resume(payload)\n", "{ std::mem::forget(payload); loop {} }\n"),
    (check.PROOF, "storage: C,", "storage: (),"),
    (check.PROOF, "returning_quarantine_effect(before.storage, after.storage)", "true"),
    (check.PROOF, "*final(context) == *old(context)", "final(context).quarantine_calls == old(context).quarantine_calls"),
    (check.PROOF, "result == settled_value(outcome, terminal_observation(*old(context)))", "true"),
    (check.PROOF, "#![allow(unused_macros)]", "#![allow(warnings)]"),
    (check.PROOF, "pub fn settle_given_returning_quarantine_contract", "#[verifier::external_body]\npub fn settle_given_returning_quarantine_contract"),
    (check.PROOF, "pub struct PostCallbackCustody<C> {", "#[derive(Clone, Copy)]\npub struct PostCallbackCustody<C> {"),
    (check.PROOF, "include!(\"../../fe2o3-kfd/src/sdma/retained_pair_operation_body.rs\");",
     "#[cfg(any())]\ninclude!(\"../../fe2o3-kfd/src/sdma/retained_pair_operation_body.rs\");"),
):
    changed(path, before, after)
extra = dict(sources)
extra[check.SRC / "unbound_inherent_shadow.rs"] = "impl Pair<'_> { fn terminal(&self) -> bool { false } }\n"
refused(lambda: check.audit(extra), "additional implementation file accepted")
missing = dict(sources)
del missing[sdma]
refused(lambda: check.audit(missing), "missing enum/getter source accepted")

cases = check.mutations(sources[check.BODY])
need(len(cases) == 29 and len({source for source, _ in cases.values()}) == 29, "distinct constructed logical controls")
for name, (body, selector) in cases.items():
    need(body != sources[check.BODY] and selector.startswith("*"), "actual executable mutation: " + name)
    candidate = dict(sources)
    candidate[check.BODY] = body
    refused(lambda: check.audit(candidate), "logical body accidentally accepted as positive source")
    leaf = types.SimpleNamespace(LOGICAL_ERRORS={"postcondition not satisfied"})
    notes = check.selection_notes(leaf, selector)
    need(notes.LOGICAL_ERRORS == leaf.LOGICAL_ERRORS and len(notes.SELECTION_NOTES) == 2,
         "unchanged strict logical diagnostics")
refused(lambda: check.selection_notes(leaf, "*unrelated"), "unbound selection accepted")

adapted = check.controller_source()
ast.parse(adapted)
base = (check.ROOT / check.BASE).read_bytes()
need(hashlib.sha256(base).hexdigest() == check.BASE_SHA, "pinned inherited controller")
restored = adapted.replace('"--triggers-mode", "silent", "--output-json"',
                           '"--triggers-mode", "silent", "--no-cheating", "--output-json"')
restored = restored.replace('"--multiple-errors", "1"', '"--multiple-errors", "0"')
need(restored.encode() == base, "only explicit trust flag and error-budget command changed")
need('"--no-cheating"' not in adapted and '"--multiple-errors", "1"' in adapted,
     "trusted returning adapter is not advertised as no-cheating")

configured = check.EXPECTED_VERIFIED
try:
    for unset in (None, 0, -1, True, "29"):
        check.EXPECTED_VERIFIED = unset
        refused(check.campaign, "unmeasured or malformed proof count accepted")
finally:
    check.EXPECTED_VERIFIED = configured
print("PASS: retained-pair post-catch source/controller calibration (4 groups; 29 logical mutants constructed, not executed)")
