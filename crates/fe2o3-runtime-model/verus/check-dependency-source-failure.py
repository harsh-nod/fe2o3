#!/usr/bin/env python3
"""Qualify native-recipe post-binding failure settlement, not native no-effect authority."""
import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-dispatch-epoch-cancel.py"
BASE_SHA = "486a946fa3d94fc2579b37c1c59a6e59bfa79af215ca40423a358b5be38aaf4a"
PROOF = V / "dependency_source_failure_v1.rs"
EPOCH_PROOF = V / "dispatch_epoch_cancel_v1.rs"
EPOCH_BODY = Path("crates/fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs")
BINDING = EPOCH_BODY.with_name("cancel_binding_body.rs")
BODY = Path("crates/fe2o3-kfd/src/queue_live/dependency_source_failure_body.rs")
FILES = [PROOF, EPOCH_PROOF, EPOCH_BODY, BINDING, BODY]
EDGES = {
    PROOF: [('include!("dispatch_epoch_cancel_v1.rs");', EPOCH_PROOF),
            ('include!("../../fe2o3-kfd/src/queue_dispatch_binding/cancel_binding_body.rs");', BINDING),
            ('include!("../../fe2o3-kfd/src/queue_live/dependency_source_failure_body.rs");', BODY)],
    EPOCH_PROOF: [('include!("../../fe2o3-kfd/src/queue_dispatch_binding/epoch_cancel_body.rs");', EPOCH_BODY)],
}


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    need(set(inputs) == set(FILES), "exact five-file closure")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source), "no added trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(target in FILES and source.count(statement) == 1, "exact source edge: " + statement)
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra source input: " + str(path))
    need(reached == set(FILES), "reachable complete closure")


def mutations(body):
    cases = {}

    def add(name, old, new, focus="settle_source_failure"):
        need(body.count(old) == 1, "unique failure mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    call = "$cancel!($recipe, $session, $identity)"
    test = call + ".is_err()"
    # Keep the call syntactically referenced so omission fails semantically,
    # without unrelated unused-macro diagnostics from the observation adapter.
    add("omits-cancellation", test, "{ if false { let _ = " + call + "; } false }")
    add("cancels-twice", test, "{ let _ = " + call + "; " + test + " }")
    add("inverts-cancellation-result", test, call + ".is_ok()")
    add("ignores-cancellation-refusal", test, "{ let _ = " + call + "; false }")
    add("rejects-cancellation-success", test, "{ let _ = " + call + "; true }")
    add("wrong-normalized-error", "Gfx942DispatchBindingErrorV1::StaleDispatchGeneration,", "Gfx942DispatchBindingErrorV1::Poisoned,")
    success = "                    } else {\n                        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)"
    add("success-becomes-terminal", success, success.replace("RetryableBeforeSideEffect", "Terminal"))
    nonretry = "                    FixedDispatchSubmissionFailureV1::Terminal(error)\n                }"
    add("nonretry-becomes-retryable", nonretry, nonretry.replace("Terminal", "RetryableBeforeSideEffect"))
    add("nonretry-becomes-rejected", nonretry, nonretry.replace("Terminal", "RejectedBeforeSideEffect"))
    add("cancels-nonretry", nonretry, "                    let _ = " + call + ";\n" + nonretry)
    add("recipe-call-forwards-neighbor", "$recipe.cancel($session, $identity)",
        "$recipe.cancel($session, DispatchEpochIdentityV1 { slot_index: 0, ..$identity })")
    forward = ('$session\n                .dispatch\n                .as_mut()\n'
               '                .expect("dependency source dispatch owner remains retained")\n'
               '                .cancel_binding($identity)')
    add("native-fabricates-cancellation", forward, "Ok(())", "cancel")
    add("native-forwards-neighbor", ".cancel_binding($identity)",
        ".cancel_binding(DispatchEpochIdentityV1 { slot_index: 0, ..$identity })", "cancel")
    need(len(cases) == len(set(cases.values())) == 13, "distinct failure mutants")
    return cases


def binding_mutations(body):
    call = "$owner.generation.cancel_epoch($identity)"
    need(body.count(call) == 1, "unique binding forwarding")
    cases = {
        "binding-fabricates-cancellation": body.replace(call, "Ok(())"),
        "binding-forwards-neighbor": body.replace(call, "$owner.generation.cancel_epoch(DispatchEpochIdentityV1 { slot_index: 0, ..$identity })"),
        "binding-rewinds-counter": body.replace(call, "$owner.generation.next_generation = 0; " + call),
    }
    need(len(set(cases.values())) == 3, "distinct binding mutants")
    return {name: (text, "*cancel_binding") for name, text in cases.items()}


def selection_notes(leaf, focus=None):
    names = {"settle_source_failure": "", "cancel": "NativeDependencySourceRecipeV1::",
             "cancel_binding": "DispatchResourceOwnerV1::"}
    need(focus is None or focus in {"*" + name for name in names}, "exact failure selector")
    selected = names if focus is None else [focus[1:]]
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES={
        "verifying root module (selected functions)",
        *{"verifying root module, function dependency_source_failure_v1::" + names[name] + name + " (selected functions)" for name in selected},
    })


def campaign(binding=False):
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated dispatch controller")
    base = types.ModuleType("source_failure_dispatch_controller")
    base.__file__ = str(ROOT / BASE)
    sys.modules[base.__name__] = base
    exec(compile(raw, base.__file__, "exec"), base.__dict__)
    module = base.campaign()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BINDING if binding else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=21)
    module.mutations = binding_mutations if binding else mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-dependency-source-failure.py"))
    need(sys.argv.count("--binding") <= 1, "one binding selector")
    binding = "--binding" in sys.argv
    if binding:
        sys.argv.remove("--binding")
    campaign(binding).main()
