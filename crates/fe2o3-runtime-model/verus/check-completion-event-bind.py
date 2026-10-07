#!/usr/bin/env python3
"""Qualify allocation-free host event binding and its post-publication join."""
import hashlib
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
BASE = V / "check-completion-bound-cancel.py"
BASE_SHA = "fcd9507c3cccffb1b2c331bbc74fddcd83198d7e5ce73337b06041377f2f74f6"
PROOF = V / "completion_event_bind_v1.rs"
SCHEMA = V / "completion_owner_schema_v1.rs"
BOUND = V / "completion_bound_cancel_execution_v1.rs"
CORE = V / "completion_event_core_v1.rs"
OCCURRENCE = V / "completion_event_occurrence_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/event_bind_body.rs")
PUBLISH = BODY.with_name("source_publish_body.rs")
BOUND_BODY = BODY.with_name("bound_cancel_body.rs")
ADAPTERS = BODY.with_name("rollback_adapters_body.rs")
SINGLE = BODY.with_name("event_release_body.rs")
FILES = [PROOF, SCHEMA, BOUND, CORE, OCCURRENCE, BODY, PUBLISH, BOUND_BODY, ADAPTERS, SINGLE]
EDGES = {
    PROOF: [('include!("completion_owner_schema_v1.rs");', SCHEMA),
            ('include!("completion_bound_cancel_execution_v1.rs");', BOUND),
            ('include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");', SINGLE),
            ('include!("completion_event_core_v1.rs");', CORE),
            ('include!("../../fe2o3-kfd/src/queue_completion/event_bind_body.rs");', BODY),
            ('include!("completion_event_occurrence_v1.rs");', OCCURRENCE),
            ('include!("../../fe2o3-kfd/src/queue_completion/source_publish_body.rs");', PUBLISH)],
    BOUND: [('include!("../../fe2o3-kfd/src/queue_completion/bound_cancel_body.rs");', BOUND_BODY),
            ('include!("../../fe2o3-kfd/src/queue_completion/rollback_adapters_body.rs");', ADAPTERS)],
}
CLOSURE_ERROR = "unable to prove post-condition of closure"
BOUNDS_ERROR = "precondition not met: index in bounds for this access"
BOUNDS_SELECTORS = {"*mark_published_retaining", "*bind_compute_event_batch_after_publication"}


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    need(set(inputs) == set(FILES), "exact ten-file closure")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source), "no added trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(target in FILES and source.count(statement) == 1, "exact edge: " + statement)
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra input: " + str(path))
    need(reached == set(FILES), "complete reachable closure")


def mutations(body):
    cases = {}

    def add(name, old, new, focus, occurrence=None):
        need(body.count(old) == (1 if occurrence is None else 2), "exact mutation site: " + name)
        offset = body.find(old) if occurrence in (None, 0) else body.find(old, body.find(old) + len(old))
        cases[name] = (body[:offset] + new + body[offset + len(old):], "*" + focus)

    published, mark = "validate_published", "mark_published_retaining"
    exact, packet = "exact_occurrence", "packet_id_at"
    bind = "bind_compute_event_batch_after_publication"
    add("published-ignores-last", "$retention.last_packet_id.is_none()", "false", published)
    add("published-accepts-bound", "\n                CompletionSlotPhaseV1::Published {", "\n                CompletionSlotPhaseV1::Bound {", published)
    add("mark-ignores-validation", "$owner.validate_bound(&$retention)", "Ok::<(), Gfx942CompletionErrorV1>(())", mark)
    add("mark-writes-neighbor", "$owner.slots[slot.index as usize].phase", "$owner.slots[0].phase", mark)
    add("mark-keeps-bound-phase", "= CompletionSlotPhaseV1::Published {", "= CompletionSlotPhaseV1::Bound {", mark)
    add("mark-forgets-last", "$retention.last_packet_id = Some($last);", "$retention.last_packet_id = None;", mark)
    add("occurrence-wrong-slot", ".get($index)", ".get(0)", exact, 0)
    add("occurrence-wrong-dispatch", ".get($index)", ".get(0)", exact, 1)
    add("occurrence-wrong-generation", "dispatch_generation: dispatch.dispatch_generation", "dispatch_generation: 0", exact)
    add("packet-ignores-index", "$index >= $n", "false", packet)
    add("packet-wrong-count", "u64::try_from($n)", "u64::try_from(0usize)", packet)
    add("packet-wrong-next", ".checked_add(1)", ".checked_add(0)", packet)
    add("packet-wrong-first", ".checked_sub(packet_count)", ".checked_sub(0)", packet)
    add("packet-wrong-offset", ".checked_add(batch_index)", ".checked_add(0)", packet)
    add("bind-ignores-ready", "$owner.require_ready()", "Ok::<(), Gfx942CompletionErrorV1>(())", bind)
    add("bind-ignores-published", "$owner.validate_published(&$batch.retention)", "Ok::<(), Gfx942CompletionErrorV1>(())", bind)
    add("bind-ignores-length", "$events.len() != $n", "false", bind)
    add("bind-accepts-bound-event", "event.exact.packet_id.is_some()", "false", bind)
    add("bind-ignores-active", "$owner.validate_active_event(event)", "Ok::<(), Gfx942CompletionErrorV1>(())", bind)
    add("bind-checks-only-prefix", "while $i < $events.len()\n                $($preflight)*", "while $i < $events.len() && $i == 0\n                $($preflight)*", bind)
    add("bind-ignores-position", "event.exact != unbound", "false", bind)
    add("bind-wrong-packet", "packet_id_at(&$batch.retention, $i)\n                            .expect", "packet_id_at(&$batch.retention, 0)\n                            .expect", bind)
    add("bind-wrong-ledger-key", ".entry($events[$i].event_id)", ".entry(0)", bind)
    write = '*retained.expect("event batch was authenticated before binding") = exact;'
    add("bind-omits-ledger-write", write, 'let _ = retained.expect("event batch was authenticated before binding");', bind)
    add("bind-omits-token-write", "$events[$i].exact = exact;", "", bind)
    add("bind-rewinds-counter", "$events[$i].exact = exact;", "$events[$i].exact = exact; $owner.dependency_ledger.next_event_id = 0;", bind)
    add("bind-clobbers-pins", "$events[$i].exact = exact;", "$events[$i].exact = exact; $owner.slots[0].event_pins = 0;", bind)
    add("forward-empty-roster", "$owner.bind_compute_event_batch_after_publication($events, $batch)",
        "$owner.bind_compute_event_batch_after_publication(Vec::new(), $batch)", "bind_dependency_event_batch_v1")
    need(len(cases) == len(set(cases.values())) == 28, "distinct binding mutants")
    return cases


def source_mutations(body):
    cases = {}

    def add(name, old, new, occurrence=None):
        need(body.count(old) == (1 if occurrence is None else 2), "exact source mutation: " + name)
        offset = body.find(old) if occurrence in (None, 0) else body.find(old, body.find(old) + len(old))
        cases[name] = (body[:offset] + new + body[offset + len(old):], "*finish_source_publication")

    mark = "($owner)\n                .mark_published_retaining($retention, $last)"
    bind = "($owner)\n                .bind_dependency_event_batch_v1($events, &$batch)"
    add("source-wrong-last", ".mark_published_retaining($retention, $last)", ".mark_published_retaining($retention, 0)")
    add("source-fabricates-publication", mark,
        "{ if true { Ok(Gfx942CompletionBatchV1 { retention: $retention }) } else { " + mark + " } }")
    add("source-binds-empty", ".bind_dependency_event_batch_v1($events, &$batch)", ".bind_dependency_event_batch_v1(Vec::new(), &$batch)")
    add("source-fabricates-binding", bind, "{ if true { Ok($events) } else { " + bind + " } }")
    for index, name in enumerate(("source-wrong-mark-error", "source-wrong-bind-error")):
        add(name, "ComputeAqlQueueSessionErrorV1::Completion($failure.0)",
            "ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::StaleBatchGeneration)", index)
    need(len(cases) == len(set(cases.values())) == 6, "distinct source mutants")
    return cases


def selection_notes(leaf, focus=None):
    free = {"exact_occurrence", "packet_id_at"}
    methods = {"validate_published", "mark_published_retaining", "bind_compute_event_batch_after_publication",
               "bind_dependency_event_batch_v1", "finish_source_publication"}
    names = free | methods
    need(focus is None or focus in {"*" + name for name in names}, "exact binding selector")
    selected = names if focus is None else [focus[1:]]
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS
        | ({BOUNDS_ERROR} if focus in BOUNDS_SELECTORS else set())
        | ({CLOSURE_ERROR} if focus == "*finish_source_publication" else set()), SELECTION_NOTES={
            "verifying root module (selected functions)",
            *{"verifying root module, function completion_event_bind_v1::"
              + ("CompletionSignalArenaOwnerV1::" if name in methods else "") + name + " (selected functions)" for name in selected},
        })


def campaign(source=False):
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated retention controller")
    base = types.ModuleType("event_bind_retention_controller")
    base.__file__ = str(ROOT / BASE)
    sys.modules[base.__name__] = base
    exec(compile(raw, base.__file__, "exec"), base.__dict__)
    module = base.campaign()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, PUBLISH if source else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=43)
    module.mutations = source_mutations if source else mutations
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-event-bind.py"))
    need(sys.argv.count("--source") <= 1, "one source selector")
    source = "--source" in sys.argv
    if source:
        sys.argv.remove("--source")
    campaign(source).main()
