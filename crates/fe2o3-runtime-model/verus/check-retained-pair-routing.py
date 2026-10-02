#!/usr/bin/env python3
"""Conditional actual routing refinement; native callbacks remain receipt boundaries.

This development draft refuses campaigns until both the full positive count and
actual selected-function diagnostics are measured. No native/timeout/Drop claim.
"""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
SRC = Path("crates/fe2o3-runtime/src")
OWNER = SRC / "kfd_backend/xgmi_retained.rs"
DECLARATIONS = SRC / "kfd_backend/xgmi_retained/routing_declarations.rs"
BODY = SRC / "kfd_backend/xgmi_retained/routing_bodies.rs"
PROOF = V / "retained_pair_routing_v1.rs"
FILES = [PROOF, DECLARATIONS, BODY]
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
SOURCE_FILES = 365
SOURCE_TREE_SHA = "b99359fed635d0670941683c8dc4eb374156d57311f9a766cc06a117f865a648"
PROOF_SHA = "c5138489a6b5e2773fb510a9a502aeeb9cd21ef88ff7d9863dacd94d7e797341"
EXPECTED_VERIFIED = 4
SELECTION_NOTES = {
    "*Observations::publish": frozenset({"verifying root module (selected functions)"}),
    "*Observations::advance": frozenset({"verifying root module (selected functions)"}),
}
MUTANT_COUNT = 11
SELECTORS = {"publish": "*Observations::publish", "advance": "*Observations::advance"}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def tree_hash(sources):
    leaves = {str(path): sha(text) for path, text in sources.items()}
    return hashlib.sha256(json.dumps(leaves, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def snapshot():
    files = {PROOF} | {path.relative_to(ROOT) for path in (ROOT / SRC).rglob("*.rs")}
    result = {}
    for path in files:
        selected = ROOT / path
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             "ordinary exact retained-routing source path")
        result[path] = selected.read_bytes().decode("utf-8")
    return result


def audit(sources):
    implementation = {path: text for path, text in sources.items() if path.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF}, "complete runtime Rust roster plus routing proof")
    need(len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         "reviewed runtime roster and bytes including native callback adapters")
    need(sha(sources[PROOF]) == PROOF_SHA, "exact conditional receipt/argument/result/frame contracts")
    owner = sources[OWNER]
    proof = sources[PROOF]
    for name in ("declarations", "bodies"):
        need(owner.count('include!("xgmi_retained/routing_' + name + '.rs");') == 1,
             "actual production shared " + name + " include")
        need(proof.count('include!("../../fe2o3-runtime/src/kfd_backend/xgmi_retained/routing_'
                         + name + '.rs");') == 1, "actual proof shared " + name + " include")
    need(len(re.findall(r"\binclude!\(", proof)) == 2
         and not re.search(r"\binclude!\(", sources[DECLARATIONS] + sources[BODY]),
         "exact three-file executable proof closure")
    compact = re.sub(r"\s+", "", owner)
    compact_proof = re.sub(r"\s+", "", proof)
    need(compact.count("retained_pair_routing_declarations!(retained_pair_routing_rust_items,pub(super));") == 1
         and compact_proof.count("retained_pair_routing_declarations!(verus,pub);") == 1,
         "shared non-Copy declarations with only enclosing-module visibility adapted")
    for macro in ("retained_pair_publish_once_body!", "retained_pair_advance_body!"):
        need(owner.count(macro) == proof.count(macro) == 1, "each actual routing body is expanded once")
    need("retained_pair_publish_once_body!(retained_pair_routing_rust_expr,phase,requests,publish(requests))" in compact,
         "actual FnOnce boundary receives original requests")
    need("retained_pair_advance_body!(retained_pair_routing_rust_expr,publish_once(phase,|requests|work.submit(requests)),work,deadline,tickets,)" in compact,
         "actual composed controller invokes the actual publish wrapper and Work")
    need("advance(phase,retained.deadline,&mutNativeWork{" in compact,
         "native retained wait invokes this controller with its stored deadline")
    need("self.owner.submit_batch(requests).inspect(|tickets|{" in compact
         and "self.backend.install_batch_tickets(" in compact
         and "self.owner.wait_batch_until(tickets,deadline)" in compact,
         "reviewed concrete callbacks remain the native authority boundary")
    need(not re.search(r"\b(?:assume|admit|assume_specification)\b|verifier::external|\buninterp\b", proof),
         "no invented callback contracts or trusted proof body")
    need("#[allow(" not in proof and "#![allow(" not in proof, "no proof diagnostic suppressions")
    need("#[derive(" not in sources[DECLARATIONS] and "Clone" not in sources[DECLARATIONS]
         and "Copy" not in sources[DECLARATIONS], "no fabricated copyable phase ownership")


def change(body, macro, before, after):
    anchor = "macro_rules! " + macro + " {"
    need(body.count(anchor) == 1, "unique shared routing macro")
    start = body.index(anchor)
    end = body.find("\nmacro_rules! ", start + len(anchor))
    end = len(body) if end < 0 else end
    selected = body[start:end]
    need(selected.count(before) == 1 and before != after, "one meaningful actual routing mutation")
    return body[:start] + selected.replace(before, after) + body[end:]


def mutations(body):
    rows = [
        ("submit-success-promoted-ready", "publish", "Ok(tickets) => BatchPhase::Pending(tickets)",
         "Ok(_tickets) => BatchPhase::Ready"),
        ("submit-error-promoted-ready", "publish", "Err(failure) => BatchPhase::SubmitFailure(failure)",
         "Err(_failure) => BatchPhase::Ready"),
        ("submit-receives-empty-requests", "publish", "match $submit {",
         "match { let _ = &$requests; let $requests = Vec::new(); $submit } {"),
        ("unpublished-inactive-phase-erased", "publish", "other => other,",
         "other => { let _ = other; BatchPhase::Ready },"),
        ("submit-failure-reopened-prepared", "publish", "Err(failure) => BatchPhase::SubmitFailure(failure)",
         "Err(_failure) => BatchPhase::Prepared(Vec::new())"),
        ("pending-skips-wait", "advance", "Advanced::Waited($work.wait($tickets, $deadline))",
         "Advanced::Unchanged(BatchPhase::Pending($tickets))"),
        ("pending-promoted-ready", "advance", "Advanced::Waited($work.wait($tickets, $deadline))",
         "{ let _ = &$tickets; Advanced::Unchanged(BatchPhase::Ready) }"),
        ("wait-receives-empty-tickets", "advance", "$work.wait($tickets, $deadline)",
         "$work.wait({ let _ = &$tickets; Vec::new() }, $deadline)"),
        ("wait-result-erased", "advance", "Advanced::Waited($work.wait($tickets, $deadline))",
         "{ let _ = $work.wait($tickets, $deadline); Advanced::Unchanged(BatchPhase::Ready) }"),
        ("unchanged-error-phase-erased", "advance", "other => Advanced::Unchanged(other),",
         "other => { let _ = other; Advanced::Unchanged(BatchPhase::Ready) },"),
        ("published-phase-erased", "advance", "let phase = $published;",
         "let phase = { let _ = $published; BatchPhase::Ready };"),
    ]
    result = {name: (change(body, "retained_pair_" + ("publish_once" if kind == "publish" else "advance")
                                  + "_body", before, after), SELECTORS[kind])
              for name, kind, before, after in rows}
    need(len(result) == len(rows) == MUTANT_COUNT and len(set(result.values())) == MUTANT_COUNT,
         "distinct proposed actual routing mutants; not yet logical outcomes")
    return result


def selection_notes(leaf, focus):
    need(focus in SELECTORS.values(), "exact routing function selector")
    need(isinstance(SELECTION_NOTES, dict) and set(SELECTION_NOTES) == set(SELECTORS.values())
         and all(isinstance(value, frozenset) and value and all(type(note) is str for note in value)
                 for value in SELECTION_NOTES.values()), "actual selection diagnostics not yet measured")
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=SELECTION_NOTES[focus])


def controller_source():
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated unchanged owned-process campaign")
    return raw.decode("utf-8")


def inherited_controller():
    module = types.ModuleType("retained_pair_routing_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(controller_source(), module.__file__, "exec"), module.__dict__)
    return module


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0,
         "full positive count has not been measured and pinned")
    for focus in SELECTORS.values():
        selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus)
    module = inherited_controller()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-retained-pair-routing.py"))
    campaign().main()
