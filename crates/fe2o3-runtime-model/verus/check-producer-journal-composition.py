#!/usr/bin/env python3
"""Source-only binding of actual journal/validator/fold composition.

No measured verifier count or signed qualification is inherited. The native
journal and live-allocation closures are executed; credit remains a reached-call
boundary. Root-local ghost projections introduce no executable conversions.
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
LIVE_DEFINITIONS = V / "context_live_validation_definitions_v1.rs"
LIVE_DECLARATIONS = Path("crates/fe2o3-runtime/src/context/versions/live_validation_declarations.rs")
HELPERS = {
    "check-producer-journal-observers.py": '351fa4cf312c9026151269c87c600c954f442ac48a68517804f9f823b099da53',
    "check-producer-input-validate.py": '15be2145871b2d460658271b85fe78231d0a18453a58b8e05b903e59ed151560',
    "check-producer-input-composition.py": 'bc6b993a9be346a39d1ad7950056b94930048ebccce5400a6d7349fb5487e7ad',
}
PROOF_SHA = "c9a45886ededd88a14c439bb346619f6826c11c22a610338c43cf730983307eb"
LIVE_DEFINITIONS_SHA = "60726b1adde644ad2624bb947b092d5097ec79b6bd484ce81652ec8cb0347069"
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
            composition.LOGIC, composition.SPEC, leaf.BODY, wrapper.BODY,
            leaf.LIVE_BODY, LIVE_DEFINITIONS, LIVE_DECLARATIONS, *wrapper.inherited().FILES]


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
    selected = ROOT / LIVE_DEFINITIONS
    need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
         "ordinary live-validation definitions")
    sources[LIVE_DEFINITIONS] = selected.read_text()
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
         "{credit:bool}", "only independent credit observations")
    need(normalized(block(text, "struct Source")) ==
         "{journal:ContextQueuedWriterJournalV1,phases:Seq<Option<AllocationPhaseV1>>,external:Seq<ExternalReturns>}",
         "ghost source is actual complete journal and phase storage plus per-input credits")
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
    need(observations.count("old(self).answers(index, *old(active), *old(queued))") == 4,
         "result, both cursors and trace derive from incoming cursor answers")
    need("let context = self.context;" in observations and "let root = self.root;" in observations
         and "let id = self.id;" in observations and "let consumer = self.consumer;" in observations
         and "let launch = self.launch;" in observations,
         "same native validator projections")
    need(observations.count("producer_input_validate_body!(verus_exec_expr, context, root, id,") == 1,
         "actual per-input validator executes")
    need("self.external.credit" in block(observations, "fn observe_expected_credit")
         and "self.versions.validate_live(allocation, record)" in block(observations, "fn observe_live")
         and "external.live" not in text,
         "credit callback remains explicit; live result comes from actual reached validator")
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


def live_binding(sources):
    leaf = helper("check-producer-input-validate.py")
    normalized, block = leaf.normalized, leaf.block
    versions = sources[leaf.SRC / "context/versions.rs"]
    definitions = sources[LIVE_DEFINITIONS]
    body = sources[leaf.LIVE_BODY]
    owner = normalized(block(versions, "struct ContextVersionsV1"))
    projected = normalized(block(definitions, "struct Versions<V>"))
    for field in ("journal:ContextQueuedWriterJournalV1", "phases:Vec<Option<AllocationPhaseV1>>"):
        need(field in owner and field in projected, "actual borrowed live-validator storage: " + field)
    need(projected == "{journal:ContextQueuedWriterJournalV1,phases:Vec<Option<AllocationPhaseV1>>,unread:V}",
         "unread native custody is opaque, not discarded")
    need('include!("versions/live_validation_declarations.rs");' in versions
         and 'include!("../../fe2o3-runtime/src/context/versions/live_validation_declarations.rs");'
         in definitions, "actual shared phase enum declaration")
    for name, arguments in (("validate_phase", "reference,phase"), ("validate_live", "id,record")):
        expected = "{context_" + name + "_body_v1!(self," + arguments + ")}"
        decision = "live_phase_decision" if name == "validate_phase" else "live_allocation_decision"
        proved = "{proof{reveal(" + decision + ");}" + expected[1:]
        need(normalized(block(versions, "fn " + name + "(")) == expected
             and normalized(block(definitions, "fn " + name + "(")) == proved,
             "exact shared production/live proof body: " + name)
    forward = sources[leaf.MODEL / "context_queued_writers/forward.rs"]
    need(normalized(block(forward, "fn lookup_allocation("))
         == "{queued_allocation_lookup_body_v1!(self,allocation)}",
         "actual queued allocation lookup forwarding")
    need("queued_allocation_lookup_body_v1!(self, allocation)" in block(definitions, "fn lookup_allocation("),
         "proof executes actual diagnostic lookup with no terminal pre-gate")
    need(body.count("$this.journal.lookup_allocation(") == 2
         and "$this.validate_phase(reference, AllocationPhaseV1::Live)?;" in body,
         "both production lookups and exact live phase retained")
    answers = normalized(block(sources[PROOF], "spec fn journal_answers"))
    need("live_allocation_decision(journal,phases,root.inputs@[indexasint].source.region.allocation,"
         "root.inputs@[indexasint].source.record)" in answers,
         "ghost live answer derives from the actual reached input and phase storage")
    need("requires" not in definitions
         and not re.search(r"\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external", definitions),
         "live validation admits arbitrary storage with no new trusted validity premise")


def audit(sources):
    wrapper = helper("check-producer-journal-observers.py")
    leaf = helper("check-producer-input-validate.py")
    composition = helper("check-producer-input-composition.py")
    wrapper_keys = {path for path in sources if path.is_relative_to(wrapper.RUNTIME)
                    or path.is_relative_to(wrapper.MODEL)} | set(wrapper.files())
    leaf_keys = {path for path in sources if path.is_relative_to(leaf.SRC)} | set(leaf.DECLARATIONS) | set(leaf.FILES)
    composition_keys = (leaf_keys - {leaf.PROOF}) | set(composition.FILES)
    need(set(sources) == wrapper_keys | leaf_keys | composition_keys | {PROOF, LIVE_DEFINITIONS},
         "exact full runtime, model and source proof roster")
    need(wrapper_keys <= sources.keys() and leaf_keys <= sources.keys() and composition_keys <= sources.keys(),
         "complete inherited source bindings")
    wrapper.audit({path: sources[path] for path in wrapper_keys})
    leaf.audit({path: sources[path] for path in leaf_keys})
    composition.audit({path: sources[path] for path in composition_keys})
    need(sha(sources[PROOF]) == PROOF_SHA, "exact candidate concrete composition root")
    need(sha(sources[LIVE_DEFINITIONS]) == LIVE_DEFINITIONS_SHA,
         "exact candidate actual live-validator contracts and shared invocations")
    closure = files()
    need(len(closure) == len(set(closure)) == 47, "complete 47-input concrete live/journal composition closure")
    leaf.closure(sources, PROOF, closure)
    semantics(sources)
    live_binding(sources)
    need(EXPECTED_VERIFIED is None and QUALIFIED is False, "source-only candidate, no inherited verifier result")


if __name__ == "__main__":
    sources = snapshot()
    audit(sources)
    print(json.dumps({"source_binding_passed": True, "qualified": QUALIFIED,
                      "expected_verified": EXPECTED_VERIFIED, "closure_files": len(files()),
                      "source_files": len(sources)}, sort_keys=True))
