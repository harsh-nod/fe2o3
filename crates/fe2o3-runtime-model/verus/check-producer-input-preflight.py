#!/usr/bin/env python3
"""Successor Context root-preflight refinement, conditional on std/key contracts."""
import functools
import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
PROOF = V / "context_producer_input_preflight_v1.rs"
BODY = Path("crates/fe2o3-runtime/src/context/versions/producer_input_preflight_body.rs")
SOURCE = BODY.with_name("producer_readers.rs")
FILES = [PROOF, BODY]
LEXER = V / "check-negative-quality.py"
LEXER_SHA = "7fadaf2f0b1b155ae8b0038e17fc01471a648319f4c2d59dc4adea25ec59cd5f"
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
EXPECTED_VERIFIED = 15
# This is a new exact-source packet, not an update to historical journal guards.
PINS = {
    SOURCE: "dc06bfc4551aad7be4aa87dbf8ffcae92658ed4ef04bd32de4fc54155e1559a5",
    BODY: "974633a83f19817bcf47c50bcffcf47a07681d7e709f4ff14d1117309e4b6c8d",
    Path("crates/fe2o3-runtime/src/context.rs"): "6b276d4850975aa2825fd7626cfb5c231b8fa03c561c62bfdcd3361ca1e04374",
    Path("crates/fe2o3-runtime/src/context/versions.rs"): "5a6746b7233db8a5d64cbaac21dbcf380562ed4c890ac4c9ac538484b7303233",
    BODY.with_name("submissions.rs"): "41d2af87e8385fe46791ef2eea47d2d22505f630b019f1b74b5645baeafc9f40",
    BODY.with_name("readers.rs"): "21320e6bac1b0ab438e253d8180a746fa47589d537ae76ab1973b8e002af8afa",
    Path("crates/fe2o3-runtime-model/src/context_producer_reads/declarations.rs"): "e2d9c46736b06c7eddf97a911d4d98287b3d52074f106698b5fb53973dffb801",
    Path("crates/fe2o3-runtime-model/src/context_version_journal/declarations.rs"): "de362edd368eda151aa2a0a112cf113af91d42a2fb03259707db77c0581e5c16",
    Path("crates/fe2o3-runtime-model/src/context_queued_writers/reads.rs"): "f5b25ce4081da579a5e8dee89e322229bdbb3a796c15a14b5201d2f15399f5f4",
}


def need(value, message):
    if not value:
        raise ValueError(message)


@functools.lru_cache(maxsize=1)
def lexer():
    raw = (ROOT / LEXER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == LEXER_SHA, "authenticated lexical helper")
    module = types.ModuleType("input_preflight_lexer")
    module.__file__ = str(ROOT / LEXER)
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module.code_only


def compact(source):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", lexer()(source)))


def block(source, start):
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    need(depth == 0, "complete declaration")
    return source[start:end - 1]


def fields(source, name):
    code = compact(source)
    matches = list(re.finditer(r"struct" + name + r"(?:<[^{};]+>)?\{", code))
    need(len(matches) == 1, "unique schema " + name)
    body = block(code, matches[0].end())
    parts, start, depth = [], 0, 0
    for index, char in enumerate(body):
        depth += (char in "(<[{") - (char in ")>]}")
        if char == "," and depth == 0:
            parts.append(body[start:index])
            start = index + 1
    parts.append(body[start:])
    result = {}
    for part in parts:
        part = re.sub(r"#\[cfg\(test\)\]", "", part)
        part = re.sub(r"^pub(?:\([^)]*\))?", "", part)
        name, kind = part.split(":", 1)
        need(name not in result, "distinct schema fields")
        result[name] = kind
    return result


def audit(inputs):
    need(set(inputs) == set(FILES), "exact two-file preflight proof closure")
    include = 'include!("../../fe2o3-runtime/src/context/versions/producer_input_preflight_body.rs");'
    source = inputs[PROOF]
    need(source.count(include) == 1, "exact shared body include")
    marker = "__preflight_include__"
    need(marker not in source, "reserved include marker")
    active = compact(source.replace(include, marker))
    need(active.count(marker) == 1, "active shared include")
    prefix = active[:active.index(marker)]
    need(prefix.count("{") == prefix.count("}") and not prefix.endswith("]"), "root non-attributed include")
    for path, text in inputs.items():
        need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern|cfg|cfg_attr)\b", text),
             "no undeclared trust or inactive proof: " + str(path))
        stripped = text.replace(include, "") if path == PROOF else text
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", stripped),
             "closed proof inputs")
    need("macro_rules!producer_input_" not in compact(source), "no shared-body shadow")
    code = compact(source)
    required = "vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),"
    preconditions = re.findall(r"requires(.*?)ensures", code)
    need(preconditions == [required], "exact exported full-key law premise without hidden validity")
    map_import = "usestd::collections::HashMap;"
    need(code.count(map_import) == 1, "actual std HashMap import")
    prefix = code[:code.index(map_import)]
    need(prefix.count("{") == prefix.count("}") and not prefix.endswith("]"), "root active std HashMap import")
    need(not re.search(r"(?:struct|enum|union|type|trait|mod|macro_rules!)(?:HashMap|std)\b|as(?:HashMap|std)\b", code),
         "no std HashMap namespace shadow")
    need(re.findall(r"use[^;]*HashMap[^;]*;", code) == [map_import], "no alternative HashMap import")
    need("#[derive(Clone,Copy,PartialEq,Eq,Hash)]structRuntimeSubmissionIdV1{context_generation:u64,local:u64}"
         in compact(source), "full derived submission identity")
    need(compact(source).count("producer_input_preflight_body!(verus_exec_expr,self,id)") == 1,
         "one actual preflight proof invocation")
    need(compact(source).count("producer_input_first_reference_body!(verus_exec_expr,self)") == 1,
         "one actual first-reference proof invocation")


def source_gate(sources, proof):
    need(set(sources) == set(PINS), "exact successor production source roster")
    for path, digest in PINS.items():
        need(hashlib.sha256(sources[path].encode()).hexdigest() == digest, "exact successor source: " + str(path))
    source = compact(sources[SOURCE])
    need(source.count("producer_input_preflight_body!(completion_journal_rust_syntax,self,id)") == 1,
         "actual Context selection wrapper")
    need(source.count("producer_input_first_reference_body!(completion_journal_rust_syntax,self)") == 1,
         "actual retained-root first-reference wrapper")
    need(source.count("self.producer_input_root_v1(id)?") == 1, "selection dominates unchanged journal loop")
    context = sources[Path("crates/fe2o3-runtime/src/context.rs")]
    code = compact(context)
    need("#[derive(Clone,Copy,Debug,Eq,Hash,Ord,PartialEq,PartialOrd)]pubstruct$name{context_generation:u64,local:u64}"
         in code and code.count("runtime_id!(RuntimeSubmissionIdV1);") == 1
         and code.count("runtime_id!(RuntimeStreamIdV1);") == 1,
         "actual full derived submission key")
    need(fields(proof, "RuntimeStreamIdV1") == {"context_generation": "u64", "local": "u64"},
         "complete stream identity projection")
    for name, expected in {
        "ContextWriterKindV1": "Synchronous,Submission",
        "ProducerReadDomainV1": "DirectedPeer,Launch",
        "ProducerReadReferenceV1": "Active(ContextProducerReadReferenceV1),Queued(ContextQueuedProducerReadReferenceV1)",
        "ProducerReadRequestV1": "Active(A),Queued(Q)",
        "SubmissionWriterDomainV1": "Ordinary,Generated{stream:RuntimeStreamIdV1,hold:u64,shell_key:u64}",
        "ContextVersionJournalErrorV1": "InvalidReference",
    }.items():
        matches = list(re.finditer(r"enum" + name + r"(?:<[^{};]+>)?\{", compact(proof)))
        need(len(matches) == 1 and block(compact(proof), matches[0].end()) == expected,
             "complete enum projection: " + name)
    # Each generic payload packs all and only these unread production fields.
    packings = (
        (context, "RuntimeContextV1", {"versions", "submissions"},
         {"backend", "context_generation", "devices", "streams", "backend_streams", "allocations",
          "backend_allocations", "allocation_admission", "modules", "backend_modules", "kernels", "events",
          "backend_events", "backend_submissions", "scalar_peer_copies", "producer_launches", "generated_issues",
          "completion_callbacks", "completion_callback_count", "completion_callback_panic_count", "next_identity",
          "terminal", "graph_reservation", "graph_issue_closed"}),
        (context, "SubmissionRecordV1", {"producer_launch", "directed_peer_copy", "journal_read", "journal_producer_read", "journal_writer"},
         {"backend_submission", "stream", "device", "quiescent", "status", "scalar_peer_copy", "dependency_retains"}),
        (sources[BODY.with_name("submissions.rs")], "RetainedSubmissionWriterV1", {"writer", "domain"},
         {"allocations", "members", "queued", "disposal_quiescent", "disposal_group", "disposal_started", "disposed_count", "journal_disposed"}),
        (sources[Path("crates/fe2o3-runtime/src/context/versions.rs")], "ContextVersionsV1",
         {"producer_readers", "submission_readers", "submission_writers"},
         {"journal", "phases", "disposal_groups", "disposal_allocations", "mixed_input_fault", "completion_fault"}),
        (sources[SOURCE], "ProducerInputV1", {"request"}, {"source", "dependency"}),
    )
    payloads = {"RuntimeContextV1": "C", "SubmissionRecordV1": "S", "RetainedSubmissionWriterV1": "W",
                "ContextVersionsV1": "V", "ProducerInputV1": "I"}
    for text, name, observed, packed in packings:
        need(set(fields(text, name)) == observed | packed, "complete production field packing: " + name)
        need(set(fields(proof, name)) == observed | {"custody"}, "opaque non-Copy field packing: " + name)
        need(fields(proof, name)["custody"] == payloads[name], "unrestricted non-Copy custody payload: " + name)
    need("impl<A,Q,I,W,R,V,S,T,C>RuntimeContextV1<A,Q,I,W,R,V,S,T,C>{" in compact(proof),
         "unrestricted owner payload parameters")
    need(fields(proof, "RuntimeContextV1")["submissions"] == "HashMap<RuntimeSubmissionIdV1,SubmissionRecordV1<S,T>>",
         "actual full-key submission HashMap")
    need(fields(proof, "ContextVersionsV1") == {
        "producer_readers": "HashMap<RuntimeSubmissionIdV1,RetainedProducerReadV1<A,Q,I>>",
        "submission_writers": "HashMap<RuntimeSubmissionIdV1,RetainedSubmissionWriterV1<W>>",
        "submission_readers": "HashMap<RuntimeSubmissionIdV1,R>", "custody": "V",
    }, "actual full-key retained HashMaps")
    for name in ("FirstProducerReadV1", "SubmissionProducerReaderMarkerV1"):
        need(fields(sources[SOURCE], name) == fields(proof, name), "exact typed marker schema")
    for path, names in (
        (Path("crates/fe2o3-runtime-model/src/context_version_journal/declarations.rs"),
         ("ContextWriterKeyV1", "ContextWriterReferenceV1")),
        (Path("crates/fe2o3-runtime-model/src/context_producer_reads/declarations.rs"),
         ("ContextProducerReadReferenceV1",)),
        (Path("crates/fe2o3-runtime-model/src/context_queued_writers/reads.rs"),
         ("ContextQueuedProducerReadReferenceV1",)),
    ):
        for name in names:
            need(fields(sources[path], name) == fields(proof, name), "full nested reference identity: " + name)
    actual, model = fields(sources[SOURCE], "RetainedProducerReadV1"), fields(proof, "RetainedProducerReadV1")
    for name, old, new in (("inputs", "Vec<ProducerInputV1>", "Vec<ProducerInputV1<A,Q,I>>"),
                           ("requests", "Vec<ContextProducerReadV1>", "Vec<A>"),
                           ("queued_requests", "Vec<ContextQueuedProducerReadV1>", "Vec<Q>")):
        need(actual[name] == old and model[name] == new, "opaque request field projection")
        actual.pop(name)
        model.pop(name)
    need(actual == model, "all retained-root fields preserved")


def mutations(body):
    cases = {}
    def add(name, old, new, focus="producer_input_root_v1"):
        tokens = re.findall(r"\w+|::|=>|->|!=|==|&&|\|\||[^\s]", old)
        pattern = r"\s*".join((r",?\s*" if token == "}" else "") + re.escape(token) for token in tokens)
        matches = list(re.finditer(pattern, body))
        need(len(matches) == 1, "unique token-exact mutation site: " + name)
        match = matches[0]
        cases[name] = (body[:match.start()] + new + body[match.end():], "*" + focus)
    add("foreign-record-generation", "$context.submissions.get(&$id)",
        "$context.submissions.get(&RuntimeSubmissionIdV1 { context_generation: 0, local: $id.local })")
    add("foreign-root-generation", "versions.producer_readers.get(&$id)",
        "versions.producer_readers.get(&RuntimeSubmissionIdV1 { context_generation: 0, local: $id.local })")
    add("missing-marker-treated-absent", "if expected.is_some()", "if false")
    add("construction-record-required", "let expected = match record",
        "if record.is_none() { return Err(ContextVersionJournalErrorV1::InvalidReference); }\n            let expected = match record")
    add("reject-nonzero-count", "marker.count == 0", "marker.count != 0")
    for name, old in (
        ("input-count", "marker.count != root.inputs.len()"),
        ("active-request-count", "root.references.len() != root.requests.len()"),
        ("queued-request-count", "root.queued_references.len() != root.queued_requests.len()"),
        ("first-reference", "Some(marker.first) != root.first_reference()"),
        ("active-marker", "marker.active != match root.references.first() { Some(first) => Some((*first, root.references.len())), None => None }"),
        ("queued-marker", "marker.queued != match root.queued_references.first() { Some(first) => Some((*first, root.queued_references.len())), None => None }"),
        ("peer-queued", "!launch && !root.queued_requests.is_empty()"),
        ("peer-stable", "!launch && versions.submission_readers.contains_key(&$id)"),
        ("record-marker", "expected != Some(marker)"),
        ("writer-domain", "root.domain != SubmissionWriterDomainV1::Ordinary"),
        ("peer-writer", "!launch && !versions.submission_writers.contains_key(&$id)"),
    ):
        add("omit-" + name, old, "false")
    add("wrap-count", "root.references.len().checked_add(root.queued_references.len())",
        "Some(root.references.len().wrapping_add(root.queued_references.len()))")
    add("wrong-consumer-generation", "context_generation: $id.context_generation", "context_generation: 0")
    add("wrong-consumer-local", "local: $id.local,", "local: 0,")
    add("wrong-consumer-kind", "kind: ContextWriterKindV1::Submission", "kind: ContextWriterKindV1::Synchronous")
    add("first-becomes-last", "$root.inputs.first()?.request", "$root.inputs.last()?.request", "first_reference")
    add("wrong-returned-domain", "{ versions, root, consumer, launch }", "{ versions, root, consumer, launch: !launch }")
    need(len(cases) == len(set(cases.values())) == 22, "distinct planned logical mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    names = {"producer_input_root_v1": "RuntimeContextV1::producer_input_root_v1",
             "first_reference": "RetainedProducerReadV1::first_reference"}
    need(focus is None or focus[1:] in names and focus.startswith("*"), "exact proof selector")
    selected = names.values() if focus is None else (names[focus[1:]],)
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS,
        SELECTION_NOTES={"verifying root module (selected functions)", *{
            "verifying root module, function context_producer_input_preflight_v1::" + name + " (selected functions)"
            for name in selected}})


def campaign():
    audit({path: (ROOT / path).read_text() for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated controller")
    module = types.ModuleType("producer_input_preflight_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-producer-input-preflight.py"))
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED > 0, "positive proof count remains unmeasured")
    campaign().main()
