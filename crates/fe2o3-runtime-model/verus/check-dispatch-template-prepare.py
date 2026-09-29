#!/usr/bin/env python3
"""Qualify raw immutable metadata preparation, not complete template binding."""
import hashlib
import functools
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
PROOF = V / "dispatch_template_prepare_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/template_prepare_body.rs")
FILES = [PROOF, BODY]
EDGES = {PROOF: [('include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_prepare_body.rs");', BODY)]}
METHODS = {"prepared_kernarg_layout_matches_code", "prepare_dispatch_templates_v1",
           "CompletionDispatchGenerationBindingV1::new", "CompletionPacketTemplateV1::new"}
EXPECTED_VERIFIED = 30
LEXER = V / "check-negative-quality.py"
LEXER_SHA = "7fadaf2f0b1b155ae8b0038e17fc01471a648319f4c2d59dc4adea25ec59cd5f"
BITVECTOR_ENUMERATION_NOTE = (
    "bitvector assertion not satisfied: not all errors may have been reported; "
    "rerun with a higher value for --multiple-errors to find other potential errors in this function"
)


def need(value, message):
    if not value:
        raise ValueError(message)


@functools.lru_cache(maxsize=1)
def rust_lexer():
    raw = (ROOT / LEXER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == LEXER_SHA, "authenticated Rust lexical helper")
    module = types.ModuleType("template_prepare_closure_lexer")
    module.__file__ = str(ROOT / LEXER)
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module.code_only


def audit(inputs):
    need(set(inputs) == set(FILES), "exact two-file preparation closure")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source),
             "no added trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(source.count(statement) == 1, "exact include edge")
            sentinel = "__fe2o3_template_closure_include__"
            need(sentinel not in source, "reserved closure sentinel")
            need(rust_lexer()(source.replace(statement, sentinel)).count(sentinel) == 1,
                 "active closure include")
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source),
             "no extra input")
    need(reached == set(FILES), "reachable closure")


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique preparation mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    abi = "prepared_kernarg_layout_matches_code"
    for name, old, new in (
        ("abi-rejects-unbound", "return true;", "return false;"),
        ("abi-inverts-bound", "if !$bound {", "if $bound {"),
        ("abi-nonzero-initial", "let mut $difference = 0u8;", "let mut $difference = 1u8;"),
        ("abi-drops-last-byte", "while $index < 32 $($invariants)*", "while $index < 31 $($invariants)*"),
        ("abi-forgets-prefix", "$difference |= $layout[$index] ^ $abi[$index];", "$difference = $layout[$index] ^ $abi[$index];"),
        ("abi-cancels-prefix", "$difference |= $layout[$index] ^ $abi[$index];", "$difference ^= $layout[$index] ^ $abi[$index];"),
        ("abi-inverts-identity", "$difference == 0", "$difference != 0"),
    ):
        add(name, old, new, abi)

    generation = "CompletionDispatchGenerationBindingV1::new"
    add("generation-substitutes-queue", "queue: $queue,",
        "queue: { let mut queue = $queue; queue.id.0 = 0; queue },", generation)
    add("generation-substitutes-code", "code: $code,", "code: $kernarg,", generation)
    add("generation-substitutes-kernarg", "kernarg: $kernarg,", "kernarg: $code,", generation)
    add("generation-substitutes-counter", "dispatch_generation: $generation,", "dispatch_generation: 0,", generation)

    template = "CompletionPacketTemplateV1::new"
    for name, old, new in (
        ("template-substitutes-geometry", "geometry: $geometry,",
         "geometry: { let mut geometry = $geometry; geometry.dimensions = 0; geometry },"),
        ("template-substitutes-ordering", "ordering: $ordering,", "ordering: AqlDispatchOrderingV1::Independent,"),
        ("template-substitutes-private-size", "private_segment_size: $private,", "private_segment_size: $group,"),
        ("template-substitutes-group-size", "group_segment_size: $group,", "group_segment_size: $private,"),
        ("template-substitutes-kernel-address", "kernel_object: $kernel,", "kernel_object: $kernarg,"),
        ("template-substitutes-kernarg-address", "kernarg_address: $kernarg,", "kernarg_address: $kernel,"),
        ("template-substitutes-alignment", "kernarg_alignment: $alignment,", "kernarg_alignment: 0,"),
        ("template-substitutes-generations", "generations: $generations,",
         "generations: { let mut generations = $generations; generations.dispatch_generation = 0; generations },"),
    ):
        add(name, old, new, template)

    prepare = "prepare_dispatch_templates_v1"
    for name, old, new in (
        ("lookup-accepts-end", "packet.code_index >= $codes.len()", "packet.code_index > $codes.len()"),
        ("lookup-rejects-valid", "packet.code_index >= $codes.len()", "true"),
        ("lookup-substitutes-program", "let code = &$codes[packet.code_index];", "let code = &$codes[0];"),
        ("lookup-wrong-detail", '"packet program index",', '"wrong packet program index",'),
        ("prepare-skips-abi", "if !prepared_kernarg_layout_matches_code(", "if false && !prepared_kernarg_layout_matches_code("),
        ("prepare-wrong-error-index", "packet: $index,", "packet: 0,"),
        ("prepare-wrong-abi-detail", 'detail: "prepared kernarg dispatch ABI identity",', 'detail: "wrong dispatch ABI identity",'),
        ("prepare-skips-first", "let mut $index = 0;\n            while $index < $packets.len()",
         "let mut $index = 1;\n            while $index < $packets.len()"),
        ("prepare-drops-last", "while $index < $packets.len() $($invariants)*",
         "while $index < $packets.len() && $index != $packets.len() - 1 $($invariants)*"),
        ("prepare-substitutes-geometry", "packet.geometry,", "$packets[0].geometry,"),
        ("prepare-substitutes-code-mapping", "code.mapping,", "packet.kernarg_mapping,"),
        ("prepare-substitutes-kernarg-mapping", "packet.kernarg_mapping,", "code.mapping,"),
        ("prepare-substitutes-counter", "                        $generation,", "                        0,"),
        ("prepare-rejects-empty", "let mut $templates = Vec::<CompletionPacketTemplateV1>::new();",
         'if $packets.is_empty() { return Err(Gfx942DispatchBindingErrorV1::InvalidCode("packet program index")); }\n'
         "            let mut $templates = Vec::<CompletionPacketTemplateV1>::new();"),
        ("lookup-before-earlier-abi", "let mut $templates = Vec::<CompletionPacketTemplateV1>::new();",
         "if $packets.len() > 1 && $packets[1].code_index >= $codes.len() {\n"
         '                return Err(Gfx942DispatchBindingErrorV1::InvalidCode("packet program index"));\n'
         "            }\n"
         "            let mut $templates = Vec::<CompletionPacketTemplateV1>::new();"),
    ):
        add(name, old, new, prepare)
    need(len(cases) == len(set(cases.values())) == 34 and all(text != body for text, _ in cases.values()),
         "distinct preparation mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in METHODS}, "exact preparation selector")
    names = METHODS if focus is None else [focus[1:]]
    indexed = focus == "*prepare_dispatch_templates_v1"
    return types.SimpleNamespace(
        LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({"precondition not met: index in bounds for this access",
            "possible arithmetic underflow/overflow"} if indexed else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            *{"verifying root module, function dispatch_template_prepare_v1::" + name
              + " (selected functions)" for name in names}})


def campaign():
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    need(EXPECTED_VERIFIED > 0, "measured positive proof count configured")
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("template_prepare_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    inherited = module.inherited

    def with_bitvector_enumeration():
        classifier = inherited()
        # This pinned release emits the exact informational note on successful
        # bitvector queries with --multiple-errors 0. Error levels stay rejected.
        classifier.ENUMERATION_NOTES = classifier.ENUMERATION_NOTES | {BITVECTOR_ENUMERATION_NOTE}
        return classifier

    module.inherited = with_bitvector_enumeration
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-dispatch-template-prepare.py"))
    campaign().main()
