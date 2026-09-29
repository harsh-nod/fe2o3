#!/usr/bin/env python3
"""Qualify composed normal-return cleanup; no native/outer-terminal authority."""

import hashlib
from pathlib import Path
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-completion-event-batch-release.py"
BASE_SHA = "7e7fb8fb8300bd6b538d3ea6b8ad64b5696167fda870701bcf30dc0d5cce8a4d"
PROOF = V / "completion_source_rollback_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/source_rollback_body.rs")
ADAPTERS = BODY.with_name("rollback_adapters_body.rs")
BOUND_BODY = BODY.with_name("bound_cancel_body.rs")
BOUND_EXECUTION = V / "completion_bound_cancel_execution_v1.rs"
CLOSURE_POSTCONDITION_ERROR = "unable to prove post-condition of closure"
BITVECTOR_ENUMERATION_NOTE = (
    "bitvector assertion not satisfied: not all errors may have been reported; "
    "rerun with a higher value for --multiple-errors to find other potential errors in this function"
)


def need(value, message):
    if not value:
        raise ValueError(message)


def inherited():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated batch controller")
    module = types.ModuleType("rollback_batch_controller")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module


def closure(base):
    files = [PROOF, BODY, ADAPTERS, BOUND_BODY, BOUND_EXECUTION,
             base.SCHEMA, base.EXECUTION, base.BODY, base.SINGLE, base.CONTRACTS, base.CORE]
    edges = {
        PROOF: [
            ('#[path = "completion_hash_reserve_contracts_v1.rs"]\nmod reserve_contracts;', base.CONTRACTS),
            ('include!("completion_owner_schema_v1.rs");', base.SCHEMA),
            ('include!("completion_bound_cancel_execution_v1.rs");', BOUND_EXECUTION),
            ('include!("completion_event_batch_release_execution_v1.rs");', base.EXECUTION),
            ('include!("../../fe2o3-kfd/src/queue_completion/source_rollback_body.rs");', BODY),
        ],
        BOUND_EXECUTION: [
            ('include!("../../fe2o3-kfd/src/queue_completion/bound_cancel_body.rs");', BOUND_BODY),
            ('include!("../../fe2o3-kfd/src/queue_completion/rollback_adapters_body.rs");', ADAPTERS),
        ],
        base.EXECUTION: base.ALLOWED_INPUTS[base.EXECUTION],
    }
    return files, edges


def mutations(body):
    cases = {}

    def add(name, old, new):
        need(body.count(old) == 1, "unique rollback mutation: " + name)
        cases[name] = (body.replace(old, new), "*rollback_source")

    release = "$release!($owner, $events).is_err()"
    cancel = "$cancel!($owner, $retention).is_err()"
    condition = release + " || " + cancel
    add("eager-cancellation", condition,
        "{ let release_failed = " + release + "; let cancel_failed = " + cancel
        + "; release_failed || cancel_failed }")
    add("wrong-conjunction", condition, release + " && " + cancel)
    add("reversed-cleanup", condition, cancel + " || " + release)
    add("omits-release", condition, "false || " + cancel)
    add("omits-cancellation", condition, release + " || false")
    add("fabricates-refusal", condition, "true")
    add("failure-is-success", "Err(Gfx942CompletionErrorV1::StaleEventOccurrence)", "Ok(())")
    add("success-is-failure", "Ok(())", "Err(Gfx942CompletionErrorV1::StaleEventOccurrence)")
    add("wrong-normalized-error", "Gfx942CompletionErrorV1::StaleEventOccurrence",
        "Gfx942CompletionErrorV1::StaleBatchGeneration")
    add("call-forwards-empty-roster", "($owner).release_dependency_event_batch_v1($events)",
        "($owner).release_dependency_event_batch_v1(Vec::new())")
    need(len(cases) == len(set(cases.values())) == 10, "distinct rollback mutations")
    return cases


def adapter_mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique adapter mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    add("adapter-substitutes-error", "$failure.0", "Gfx942CompletionErrorV1::StaleEventOccurrence", "cancel_bound")
    call = "$owner.cancel_bound_retaining($retention)"
    add("adapter-fabricates-cancellation", call, "{ if true { Ok(()) } else { " + call + " } }", "cancel_bound")
    add("adapter-forwards-empty-roster", "$owner.release_compute_event_batch($events)",
        "$owner.release_compute_event_batch(Vec::new())", "release_dependency_event_batch_v1")
    need(len(cases) == len(set(cases.values())) == 3, "distinct adapter mutations")
    return cases


def selection_notes(leaf, focus=None):
    functions = ("rollback_source", "cancel_bound", "release_dependency_event_batch_v1")
    need(focus is None or focus in {"*" + name for name in functions}, "exact rollback selector")
    selected = functions if focus is None else (focus[1:],)
    errors = leaf.LOGICAL_ERRORS
    if focus == "*cancel_bound":
        errors = errors | {CLOSURE_POSTCONDITION_ERROR}
    return types.SimpleNamespace(LOGICAL_ERRORS=errors, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function completion_source_rollback_v1::CompletionSignalArenaOwnerV1::"
          + name + " (selected functions)" for name in selected},
    })


def campaign(adapters=False):
    base = inherited()
    files, edges = closure(base)
    base.audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in files}, files, edges, PROOF)
    module = base.campaign()
    module.FILES = files
    module.PROOF = PROOF
    module.BODY = ADAPTERS if adapters else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=51)
    module.mutations = adapter_mutations if adapters else mutations
    module.selection_notes = selection_notes
    original = module.inherited

    def with_bitvector_enumeration():
        classifier = original()
        classifier.ENUMERATION_NOTES = classifier.ENUMERATION_NOTES | {BITVECTOR_ENUMERATION_NOTE}
        return classifier

    module.inherited = with_bitvector_enumeration
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-source-rollback.py"))
    need(sys.argv.count("--adapters") <= 1, "one adapter selector")
    adapters = "--adapters" in sys.argv
    if adapters:
        sys.argv.remove("--adapters")
    campaign(adapters).main()
