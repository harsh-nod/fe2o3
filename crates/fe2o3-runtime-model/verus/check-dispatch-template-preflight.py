#!/usr/bin/env python3
"""Qualify retained-fact preflight, not live resource authority or binding."""
import functools
import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-compute-pipeline-publication.py"
BASE_SHA = "1d4264a646983906fff5e54a2279865f5eba55413c1313698bee57064dfdfd8e"
PROOF = V / "dispatch_template_preflight_v1.rs"
RESERVE = V / "dispatch_epoch_reserve_v1.rs"
CANCEL = V / "dispatch_epoch_cancel_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/template_preflight_body.rs")
RESERVE_BODY = BODY.with_name("epoch_reserve_body.rs")
CANCEL_BODY = BODY.with_name("epoch_cancel_body.rs")
FILES = [PROOF, RESERVE, CANCEL, BODY, RESERVE_BODY, CANCEL_BODY]
EDGES = {
    PROOF: [('include!("dispatch_epoch_reserve_v1.rs");', RESERVE),
            ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_preflight_body.rs");', BODY)],
    RESERVE: [('include!("dispatch_epoch_cancel_v1.rs");', CANCEL),
              ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_reserve_body.rs");', RESERVE_BODY)],
    CANCEL: [('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");', CANCEL_BODY)],
}
METHODS = {"validate_packet_count": "validate_packet_count",
           "preflight_templates": "DispatchResourceOwnerV1::preflight_templates"}
EXPECTED_VERIFIED = 46
LEXER = V / "check-negative-quality.py"
LEXER_SHA = "7fadaf2f0b1b155ae8b0038e17fc01471a648319f4c2d59dc4adea25ec59cd5f"


def need(value, message):
    if not value:
        raise ValueError(message)


@functools.lru_cache(maxsize=1)
def rust_lexer():
    raw = (ROOT / LEXER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == LEXER_SHA, "authenticated Rust lexical helper")
    module = types.ModuleType("template_preflight_closure_lexer")
    module.__file__ = str(ROOT / LEXER)
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    return module.code_only


def audit(inputs):
    need(set(inputs) == set(FILES), "exact six-file preflight closure")
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
            marker = "__fe2o3_preflight_closure_include__"
            need(marker not in source, "reserved closure marker")
            need(rust_lexer()(source.replace(statement, marker)).count(marker) == 1, "active closure include")
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra input")
    need(reached == set(FILES), "reachable closure")


def mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "unique preflight mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    for name, old, new in (
        ("count-accepts-zero", "if $n == 0 {", "if false {"),
        ("count-rejects-maximum", "if $n > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize", "if $n >= AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize"),
        ("count-accepts-oversize", "if $n > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize", "if false"),
        ("count-substitutes-requested", "requested: $n,", "requested: 0,"),
        ("count-substitutes-maximum", "maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize,", "maximum: 0,"),
    ):
        add(name, old, new, "validate_packet_count")
    for name, old, new in (
        ("preflight-ignores-poison", "$owner.generation.ensure_not_poisoned()?;", ""),
        ("preflight-ignores-count", "validate_packet_count::<$n>()?;", ""),
        ("preflight-ignores-packet-cardinality", "$owner.packets.len() != $n", "false"),
        ("preflight-ignores-code-cardinality", "$owner.code_identity.len() != $owner.code.len()", "false"),
        ("preflight-ignores-data-cardinality", "$owner.data.len() != $owner.data_premises.len()", "false"),
        ("preflight-ignores-code-vm", "$owner.code_identity[$index].mapping.allocation.vm != $queue.vm", "false"),
        ("preflight-ignores-kernarg-vm", "$owner.kernarg.facts().mapping().allocation.vm != $queue.vm", "false"),
        ("preflight-ignores-data-vm", "$owner.data[$index].vm() != $queue.vm", "false"),
        ("preflight-skips-first-code", "let mut $index = 0;\n            while $index < $owner.code_identity.len()",
         "let mut $index = 1;\n            while $index < $owner.code_identity.len()"),
        ("preflight-drops-last-data", "while $index < $owner.data.len() $($data_invariants)*",
         "while $index < $owner.data.len() && $index != $owner.data.len() - 1 $($data_invariants)*"),
        ("preflight-ignores-late-ordering", "while $index < $owner.packets.len() $($order_invariants)*",
         "while $index < $owner.packets.len() && $index == 0 $($order_invariants)*"),
        ("preflight-inverts-ordering", "$owner.packets[$index].ordering != AqlDispatchOrderingV1::WaitForPrior",
         "$owner.packets[$index].ordering == AqlDispatchOrderingV1::WaitForPrior"),
        ("preflight-substitutes-error-index", "packet: $index,", "packet: 0,"),
        ("preflight-substitutes-error-detail", 'detail: "multi-inflight recipe requires wait-for-prior ordering",',
         'detail: "wrong ordering refusal",'),
        ("preflight-epoch-before-metadata", "$owner.generation.ensure_not_poisoned()?;",
         "let _early = $owner.generation.preflight_reservation($queue)?;\n            $owner.generation.ensure_not_poisoned()?;"),
        ("preflight-skips-epoch", "let (_, generation, _) = $owner.generation.preflight_reservation($queue)?;",
         "let generation = $owner.generation.next_generation;"),
        ("preflight-substitutes-generation", "Ok(generation)", "Ok(0)"),
    ):
        add(name, old, new, "preflight_templates")
    need(len(cases) == len(set(cases.values())) == 22 and all(text != body for text, _ in cases.values()),
         "distinct preflight mutation roster")
    return cases


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in METHODS}, "exact preflight selector")
    names = METHODS if focus is None else [focus[1:]]
    indexed = focus == "*preflight_templates"
    return types.SimpleNamespace(
        LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({"precondition not met: index in bounds for this access",
            "possible arithmetic underflow/overflow"} if indexed else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            *{"verifying root module, function dispatch_template_preflight_v1::" + METHODS[name]
              + " (selected functions)" for name in names}})


def campaign():
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    module = types.ModuleType("template_preflight_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(raw, module.__file__, "exec"), module.__dict__)
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-dispatch-template-preflight.py"))
    need(EXPECTED_VERIFIED > 0, "measured positive proof count configured")
    campaign().main()
