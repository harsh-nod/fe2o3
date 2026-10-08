#!/usr/bin/env python3
"""Source binding and calibrated diagnostics; fresh signed qualification is separate."""
import hashlib
import json
from pathlib import Path
import re
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
RUNTIME = Path("crates/fe2o3-runtime/src")
MODEL = Path("crates/fe2o3-runtime-model/src")
PRIOR = V / "check-queued-query.py"
PRIOR_SHA = 'd5fa810db6df207bc51a2c86be6f4a7772fa5a39f1fa1e38aa4644b3ff762647'
PROOF = V / "context_producer_journal_observers_v1.rs"
BODY = RUNTIME / "context/versions/producer_journal_observer_bodies.rs"
OWNER = RUNTIME / "context/versions/producer_readers.rs"
PREPARATION = RUNTIME / "context/versions/producer_readers/preparation.rs"
VERSIONS = RUNTIME / "context/versions.rs"
RUNTIME_FILES = 635
RUNTIME_SHA = 'c25e709ce0f60e713b2b030f8ee15611de37995f868d77b3f724ae23f44d0734'
PROOF_SHA = "78621d8cb181436d0b214896cae86684fc7ab2ac218546eac5ea4c3e40f9a785"
BODY_SHA = "5fb7f1572c41a6f2c6dffa74f040e870bb4ae874133c127ee5ae87d52c3fce59"
NATIVE_PREEXTRACTION_SHA = "f470e1a37f07d8b27e29d2bfed0bf4714e62de9fd109446b887bd51401e95d94"
EXPECTED_VERIFIED = 165
QUALIFIED = False
DIAGNOSTICS = V / "producer-journal-diagnostics-v1.py"
DIAGNOSTICS_SHA = "00be0604b67bff75b4f99e136ea674b7466283ad2b8dfe1f2a3ff92bda9e2418"
MUTATION_ROSTER_SHA = "ca49f2f42cd625627caed8c471fa580518dfcaafaf2cac9a127cfacf2f767b86"
ROUTES = {
    "active_lookup": ("ContextProducerReadReferenceV1", "ContextProducerReadV1", "lookup_producer_read"),
    "active_status": ("ContextProducerReadReferenceV1", "ContextProducerReadStatusV1", "producer_read_status"),
    "queued_lookup": ("ContextQueuedProducerReadReferenceV1", "ContextQueuedProducerReadV1", "lookup_queued_producer_read"),
    "queued_status": ("ContextQueuedProducerReadReferenceV1", "ContextProducerReadStatusV1", "queued_producer_read_status"),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def tree_hash(sources):
    return sha(json.dumps({str(path): sha(text) for path, text in sources.items()},
                          sort_keys=True, separators=(",", ":")))


def inherited():
    path = ROOT / PRIOR
    need(path.resolve() == path and not path.is_symlink(), "ordinary inherited source guard")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == PRIOR_SHA, "reviewed complete journal query guard")
    module = types.ModuleType("native_producer_journal_query")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def files():
    return [PROOF, BODY, *inherited().FILES]


def snapshot():
    result = inherited().snapshot()
    paths = {PROOF, BODY} | {path.relative_to(ROOT) for path in (ROOT / RUNTIME).rglob("*.rs")}
    for path in paths:
        file = ROOT / path
        need(file.is_file() and file.resolve() == file and not file.is_symlink(), "ordinary native closure input")
        result[path] = file.read_bytes().decode()
    return result


def normalized(text):
    return re.sub(r"\s+", "", re.sub(r"//[^\n]*", "", text))


def block(text, anchor):
    need(text.count(anchor) == 1, "unique source anchor " + anchor)
    start = text.index("{", text.index(anchor))
    end, depth = start + 1, 1
    while depth and end < len(text):
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    need(depth == 0, "complete source block")
    return text[start:end]


def reconstruct_preparation(sources):
    """Reverse the private preparation-module move before the historical check."""
    owner = sources[OWNER]
    child = sources.get(PREPARATION)
    need(isinstance(child, str), "retained native preparation module")
    declaration = "\nmod preparation;\n"
    implementation = "impl<B: RuntimeBackendV1> RuntimeContextV1<B> {\n"
    prefix = "use super::*;\n\n" + implementation
    need(owner.count(declaration) == 1 and owner.endswith(declaration),
         "one ordinary private preparation module at the original owner")
    need(owner.count(implementation) == 1, "one original Context implementation")
    need(child.startswith(prefix) and child.endswith("}\n")
         and child.count(implementation) == 1,
         "one preparation implementation with the original parent imports")
    moved = child[len(prefix):-2]
    methods = {
        "prepare_pending_input_v1": ("pub(super) ", ""),
        "prepare_producer_batch_v1": ("pub(super) ", ""),
        "prepare_producer_read_v1": ("pub(in crate::context::versions) ", "pub(super) "),
        "prepare_launch_inputs_v1": ("pub(in crate::context::versions) ", "pub(super) "),
    }
    need(len(re.findall(r"\bfn\s+", moved)) == len(methods),
         "exact four moved preparation methods")
    for name, (current_visibility, original_visibility) in methods.items():
        current = current_visibility + "fn " + name + "("
        need(moved.count(current) == 1 and "fn " + name + "(" not in owner,
             "one original-scope preparation method " + name)
        moved = moved.replace(current, original_visibility + "fn " + name + "(")
    return owner[:-len(declaration)].replace(implementation, implementation + moved + "\n", 1)


def forwarders(sources):
    owner, body, proof = (sources[path] for path in (OWNER, BODY, PROOF))
    need(owner.count('include!("producer_journal_observer_bodies.rs");') == 1, "one production shared-body include")
    declaration = """{
        context: &'a RuntimeContextV1<B>, versions: &'a ContextVersionsV1,
        root: &'a RetainedProducerReadV1, id: RuntimeSubmissionIdV1,
        consumer: ContextWriterKeyV1, launch: bool,
    }"""
    need(normalized(block(owner, "struct ProducerInputObservationsV1<'a, B: RuntimeBackendV1>"))
         == normalized(declaration), "exact native observer fields and borrowed projection types")
    need(normalized(block(sources[VERSIONS], "struct ContextVersionsV1")).count(
        "journal:ContextQueuedWriterJournalV1,") == 1, "actual complete journal field type")
    reconstructed = reconstruct_preparation(sources).replace(
        'include!("producer_journal_observer_bodies.rs");\n', "")
    for name, (reference, result, query) in ROUTES.items():
        method = "observe_" + name
        macro = "producer_" + method + "_body_v1"
        anchor = "fn " + method + "("
        begin = owner.index(anchor)
        signature = owner[begin:owner.index("{", begin)]
        need(normalized(signature) == normalized("fn " + method + "(&mut self, reference: " + reference
             + ",) -> Result<" + result + ", ContextVersionJournalErrorV1>"), "exact native forwarding signature " + name)
        need(normalized(block(owner, anchor)) == "{" + macro + "!(self,reference)}", "one reached native macro " + name)
        expected = "{($observer:expr,$reference:expr)=>{$observer.versions.journal." + query + "($reference)};}"
        need(normalized(block(body, "macro_rules! " + macro + " {")) == expected, "one unmodified reached journal call " + name)
        need(proof.count("let result = " + macro + "!(self, reference);") == 1, "same body executed by proof " + name)
        reconstructed = reconstructed.replace(macro + "!(self, reference)", "self.versions.journal." + query + "(reference)")
    need(sha(reconstructed) == NATIVE_PREEXTRACTION_SHA, "exact reversible four-call native extraction")
    need(len(re.findall(r"macro_rules!", body)) == 4, "no extra adapter macros")
    need(not re.search(r"\b(?:struct|enum)\s+Context\w+", proof), "actual Context declarations, never duplicated schemas")
    need("Returns" not in proof, "no supplied executable query answers")


def closure(sources):
    expected, reached, pending = set(files()), set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        need(path in expected and path in sources, "declared proof closure member")
        reached.add(path)
        text = sources[path]
        includes = re.findall(r'include!\("([^"\n]+)"\);', text)
        need(text.count("include!(") == len(includes) and "#[path" not in text
             and not re.search(r"\bmod\s+\w+\s*;", text), "literal closed include and module routes")
        for name in includes:
            target = (ROOT / path.parent / name).resolve()
            need(target.is_relative_to(ROOT), "contained proof include")
            pending.append(target.relative_to(ROOT))
    need(reached == expected and len(expected) == 38, "complete 36-file journal closure plus native body and root")


def audit(sources):
    prior = inherited()
    model_sources = {path: text for path, text in sources.items() if path in prior.FILES or path.is_relative_to(MODEL)}
    prior.audit(model_sources)
    runtime = {path: text for path, text in sources.items() if path.is_relative_to(RUNTIME)}
    need(len(runtime) == RUNTIME_FILES and tree_hash(runtime) == RUNTIME_SHA, "exact entire native runtime, no method or macro shadowing")
    need(set(sources) == set(model_sources) | set(runtime) | {PROOF}, "exact whole-source binding roster")
    need(sha(sources[PROOF]) == PROOF_SHA and sha(sources[BODY]) == BODY_SHA, "reviewed wrapper contracts and actual bodies")
    forwarders(sources)
    closure(sources)
    need(not re.search(r"\b(?:requires|assume|admit|assume_specification|uninterp)\b|verifier::external", sources[PROOF]),
         "no added admission premise or trusted query shortcut")


def mutations(body):
    rows = {}
    for name, (_reference, _result, query) in ROUTES.items():
        macro = "producer_observe_" + name + "_body_v1"
        original = block(body, "macro_rules! " + macro + " {")
        call = "$observer.versions.journal." + query + "($reference)"
        need(original.count(call) == 1, "unique changed native forwarding expression")
        replacements = {
            "constant-error": "Err(ContextVersionJournalErrorV1::InvalidReference)",
            "error-coerced": "match " + call + " { Ok(value) => Ok(value), Err(_) => Err(ContextVersionJournalErrorV1::InvalidReference) }",
        }
        if name == "active_lookup":
            replacements["prefetch-status"] = "{ $observer.versions.journal.producer_read_status($reference)?; " + call + " }"
        for suffix, replacement in replacements.items():
            changed = original.replace(call, replacement)
            need(body.count(original) == 1 and changed != original, "unique actual-body negative control")
            rows[name + "-" + suffix] = (body.replace(original, changed), "native_observers::Observations::observe_" + name)
    need(len(rows) == 9, "all proposed wrapper controls, never qualified by construction")
    roster = {name: {"body_sha256": sha(text), "affected_method": method,
                     "boundary": "wrapper-ghost-trace-only" if name == "active_lookup-prefetch-status"
                                 else "actual-journal-result-equality"}
              for name, (text, method) in rows.items()}
    need(sha(json.dumps(roster, sort_keys=True, separators=(",", ":"))) == MUTATION_ROSTER_SHA,
         "all nine calibrated actual-body mutations and exact distinct primary boundaries")
    return rows


def diagnostic_classifier():
    path = ROOT / DIAGNOSTICS
    need(path.resolve() == path and not path.is_symlink(), "ordinary source-bound diagnostic classifier")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == DIAGNOSTICS_SHA, "reviewed calibrated classifier bytes")
    module = types.ModuleType("producer_journal_diagnostic_classifier")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


if __name__ == "__main__":
    values = snapshot()
    audit(values)
    print(json.dumps({"source_binding_passed": True, "qualified": QUALIFIED,
                      "expected_verified": EXPECTED_VERIFIED, "closure_files": len(files()),
                      "proposed_negative_cases": len(mutations(values[BODY]))}, sort_keys=True))
