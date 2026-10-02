#!/usr/bin/env python3
"""Source-only binding of actual journal/validator/fold composition.

No measured verifier count or signed qualification is inherited. The native
journal closure is executed; live allocation and credit remain reached-call
boundaries. Root-local ghost projections introduce no executable conversions.
"""
import hashlib
import json
from pathlib import Path
import re
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
PROOF = V / "context_producer_journal_composition_v1.rs"
ENROLLMENT = Path("crates/fe2o3-runtime-model/src/context_version_journal/enrollment_declarations.rs")
HELPERS = {
    "check-producer-journal-observers.py": "687a6b238fc2a490776ed9a047ca38cfde2bbaff8e8d1c02cc4b1036396f1d18",
    "check-producer-input-validate.py": "29aa9b38579b8dd591c900874e98edce4f4cd4565efc855821d7ea56ff32ab25",
    "check-producer-input-composition.py": "c6a7c992e4106c5c7b52cf139f2188eb633495f161f2f02e09a10f932ed0df85",
}
PROOF_SHA = "cae1577d6f5551eb6f541d245b97fc0fdf324505e79a4bb18eaf022976dd8f4e"
EXPECTED_VERIFIED = None
QUALIFIED = False


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def helper(name):
    path = ROOT / V / name
    need(path.is_file() and not path.is_symlink() and path.resolve() == path, "ordinary source guard")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == HELPERS[name], "exact reviewed dependency guard")
    module = types.ModuleType(name.replace("-", "_"))
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def files():
    wrapper = helper("check-producer-journal-observers.py")
    leaf = helper("check-producer-input-validate.py")
    composition = helper("check-producer-input-composition.py")
    return [PROOF, ENROLLMENT, leaf.RUNTIME_DECLARATIONS, leaf.OUTCOMES,
            composition.LOGIC, composition.SPEC, leaf.BODY, wrapper.BODY, *wrapper.inherited().FILES]


def snapshot():
    sources = helper("check-producer-journal-observers.py").snapshot()
    conditional = helper("check-producer-input-composition.py").snapshot()
    leaf = helper("check-producer-input-validate.py")
    conditional[leaf.PROOF] = (ROOT / leaf.PROOF).read_text()
    for path, text in conditional.items():
        need(path not in sources or sources[path] == text, "identical overlapping source bytes")
        sources[path] = text
    selected = ROOT / PROOF
    need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
         "ordinary concrete composition root")
    sources[PROOF] = selected.read_text()
    return sources


def semantics(sources):
    leaf = helper("check-producer-input-validate.py")
    composition = helper("check-producer-input-composition.py")
    text = sources[PROOF]
    marker = '    include!("producer_input_composition_logic_v1.rs");'
    need(text.count(marker) == 1, "one shared prefix proof")
    observations, fold = text.split(marker)
    normalized, block = leaf.normalized, leaf.block
    need('include!("context_queued_query_execution_v1.rs");' in text
         and 'include!("../src/context_version_journal/enrollment_declarations.rs");' in text,
         "actual complete journal and enrollment declarations")
    need(leaf.JOURNAL_DECLARATIONS not in files(), "no duplicated compared journal types in concrete closure")
    local = text + sources[leaf.RUNTIME_DECLARATIONS] + sources[leaf.OUTCOMES] + sources[composition.LOGIC]
    for name in ("WriterKind", "WriterKey", "WriterReference", "AllocationKey", "JournalDeviceKey",
                 "AllocationReference", "AllocationWrite", "AllocationRead", "ProducerRead",
                 "QueuedProducerRead", "ProducerReadReference", "QueuedProducerReadReference",
                 "ProducerReadStatus", "VersionJournalError", "AllocationEnrollment"):
        need(not re.search(r"\b(?:struct|enum)\s+Context" + name + r"V1\b", local),
             "actual declaration identity: " + name)
    need(normalized(block(text, "struct ExternalReturns")) ==
         "{credit:bool,live:Result<ContextAllocationReferenceV1,ContextVersionJournalErrorV1>}",
         "only independent live and credit observations")
    need(normalized(block(text, "struct Source")) ==
         "{journal:ContextQueuedWriterJournalV1,external:Seq<ExternalReturns>}",
         "ghost source is actual complete journal plus per-input external values")
    for part in (observations, fold):
        name = "struct Observations" if part is observations else "struct Composition"
        fields = normalized(block(part, name))
        for expected in ("context:&'aContext<C,L,P,D>,", "versions:&'aVersions<V>,", "root:&'aRoot<R>,",
                         "id:RuntimeSubmissionIdV1,", "consumer:ContextWriterKeyV1,", "launch:bool,"):
            need(expected in fields, "borrowed native owner projection")
        need(normalized(block(part, "spec fn owner_view")) == "{Owner{context:*self.context,root:*self.root}}",
             "owner adaptation is ghost-only")
        frame = normalized(block(part, "spec fn same_binding"))
        for field in ("context", "versions", "root", "id", "consumer", "launch"):
            need("self." + field + "==before." + field in frame, "entire borrowed owner frame")
    routes = {
        "active_lookup": "queued_active_lookup_decision_v1",
        "active_status": "queued_active_status_decision_v1",
        "queued_lookup": "reads::lookup_decision_v1",
        "queued_status": "reads::status_decision_v1",
    }
    for name, decision in routes.items():
        macro = "producer_observe_" + name + "_body_v1"
        call = "let result = " + macro + "!(self, reference);"
        need(observations.count(call) == 1, "actual reached forwarding macro " + name)
        need("result == " + decision + "(old(self).versions.journal, reference)," in observations,
             "call-specific actual journal result " + name)
        family = "active" if name.startswith("active") else "queued"
        references = "references" if family == "active" else "queued_references"
        need(decision + "(journal, root." + references + "@[" + family + " as int])" in observations,
             "incoming cursor and actual reference decision " + name)
    need(observations.count("old(self).answers(*old(active), *old(queued))") == 4,
         "result, both cursors and trace derive from incoming cursor answers")
    need("let context = self.context;" in observations and "let root = self.root;" in observations
         and "let id = self.id;" in observations and "let consumer = self.consumer;" in observations
         and "let launch = self.launch;" in observations,
         "same native validator projections")
    need(observations.count("producer_input_validate_body!(verus_exec_expr, context, root, id,") == 1,
         "actual per-input validator executes")
    need("self.external.credit" in block(observations, "fn observe_expected_credit")
         and "self.external.live" in block(observations, "fn observe_live"),
         "remaining reached callbacks explicit, not assumed native facts")
    need("let external = self.returns[index];" in fold and "returns: &'a [ExternalReturns]" in fold,
         "external observations independent per reached input")
    need("let result = validation.validate(index, active, queued);" in fold
         and "validation.validate(index, active, queued)?" not in fold,
         "record error receipt after actual validator before fold early exit")
    need("active_before, queued_before, *active, *queued, result, validation.calls@, external.credit" in fold,
         "receipt records actual cursor changes, errors and calls")
    for value in ("self.calls@ = self.calls@ + validation.calls@;",
                  "self.consumed@ = (index + 1) as nat;",
                  "producer_input_fold_body!(verus_exec_expr, self, invalid_reference,",
                  "final(self).consumed@ == fold::reached(old(self).original(), 0),",
                  "final(self).receipts@ == old(self).original().take(final(self).consumed@ as int),",
                  "fold_status_result(out) == fold::fold_result(old(self).original(), 0,"):
        need(fold.count(value) == 1, "actual fold and complete reached-prefix contract")
    shared = sources[composition.LOGIC]
    need("source_answers(owner, answers, index, before.active, before.queued)" in shared,
         "each prefix derives journal observations at the preceding cursors")
    need("result is Ok" not in block(shared, "spec fn receipt"), "cursor advancement is not success-dependent")
    need(not re.search(r"\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external", local),
         "no new assumptions or opaque executable conversions")


def audit(sources):
    wrapper = helper("check-producer-journal-observers.py")
    leaf = helper("check-producer-input-validate.py")
    composition = helper("check-producer-input-composition.py")
    wrapper_keys = {path for path in sources if path.is_relative_to(wrapper.RUNTIME)
                    or path.is_relative_to(wrapper.MODEL)} | set(wrapper.files())
    leaf_keys = {path for path in sources if path.is_relative_to(leaf.SRC)} | set(leaf.DECLARATIONS) | set(leaf.FILES)
    composition_keys = (leaf_keys - {leaf.PROOF}) | set(composition.FILES)
    need(set(sources) == wrapper_keys | leaf_keys | composition_keys | {PROOF},
         "exact full runtime, model and source proof roster")
    need(wrapper_keys <= sources.keys() and leaf_keys <= sources.keys() and composition_keys <= sources.keys(),
         "complete inherited source bindings")
    wrapper.audit({path: sources[path] for path in wrapper_keys})
    leaf.audit({path: sources[path] for path in leaf_keys})
    composition.audit({path: sources[path] for path in composition_keys})
    need(sha(sources[PROOF]) == PROOF_SHA, "exact candidate concrete composition root")
    closure = files()
    need(len(closure) == len(set(closure)) == 44, "complete 44-input concrete journal composition closure")
    leaf.closure(sources, PROOF, closure)
    semantics(sources)
    need(EXPECTED_VERIFIED is None and QUALIFIED is False, "source-only candidate, no inherited verifier result")


if __name__ == "__main__":
    sources = snapshot()
    audit(sources)
    print(json.dumps({"source_binding_passed": True, "qualified": QUALIFIED,
                      "expected_verified": EXPECTED_VERIFIED, "closure_files": len(files()),
                      "source_files": len(sources)}, sort_keys=True))
