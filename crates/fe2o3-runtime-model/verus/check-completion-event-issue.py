#!/usr/bin/env python3
"""Qualify host event issuance and lane packing with two explicit std contracts."""
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
BOUND_CONTROLLER = V / "check-completion-bound-cancel.py"
BOUND_SHA = "fcd9507c3cccffb1b2c331bbc74fddcd83198d7e5ce73337b06041377f2f74f6"
PROOF = V / "completion_event_issue_v1.rs"
CONTRACTS = V / "completion_issue_reserve_contracts_v1.rs"
CONTRACTS_SHA = "fcf520081b0857ca3082a9787f49dc9fd24c19b9d09b4126fdabf46136b09de1"
SCHEMA = V / "completion_owner_schema_v1.rs"
BOUND = V / "completion_bound_cancel_execution_v1.rs"
CORE = V / "completion_event_core_v1.rs"
OCCURRENCE = V / "completion_event_occurrence_v1.rs"
BODY = Path("crates/fe2o3-kfd/src/queue_completion/event_issue_body.rs")
BIND = BODY.with_name("event_bind_body.rs")
BOUND_BODY = BODY.with_name("bound_cancel_body.rs")
ADAPTERS = BODY.with_name("rollback_adapters_body.rs")
SINGLE = BODY.with_name("event_release_body.rs")
OUTPUT = Path("crates/fe2o3-kfd/src/queue_live/dependency_source_output_body.rs")
FILES = [PROOF, CONTRACTS, SCHEMA, BOUND, CORE, OCCURRENCE, BODY, BIND, BOUND_BODY, ADAPTERS, SINGLE, OUTPUT]
EDGES = {
    PROOF: [('#[path = "completion_issue_reserve_contracts_v1.rs"]\nmod reserve_contracts;', CONTRACTS),
            ('include!("completion_owner_schema_v1.rs");', SCHEMA),
            ('include!("completion_bound_cancel_execution_v1.rs");', BOUND),
            ('include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");', SINGLE),
            ('include!("completion_event_core_v1.rs");', CORE),
            ('include!("../../fe2o3-kfd/src/queue_completion/event_bind_body.rs");', BIND),
            ('include!("completion_event_occurrence_v1.rs");', OCCURRENCE),
            ('include!("../../fe2o3-kfd/src/queue_completion/event_issue_body.rs");', BODY),
            ('include!("../../fe2o3-kfd/src/queue_live/dependency_source_output_body.rs");', OUTPUT)],
    BOUND: [('include!("../../fe2o3-kfd/src/queue_completion/bound_cancel_body.rs");', BOUND_BODY),
            ('include!("../../fe2o3-kfd/src/queue_completion/rollback_adapters_body.rs");', ADAPTERS)],
}
BOUNDS_ERROR = "precondition not met: index in bounds for this access"
BATCH = "record_unbound_compute_event_batch"
ONE = "record_unbound_compute_event"
BOUNDS_SELECTORS = {"*" + BATCH, "*" + ONE, "*pack_source_output"}


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    need(set(inputs) == set(FILES), "exact twelve-file closure")
    need(hashlib.sha256(inputs[CONTRACTS].encode()).hexdigest() == CONTRACTS_SHA, "exact two std contracts")
    reached, pending = set(), [PROOF]
    while pending:
        path = pending.pop()
        if path in reached:
            continue
        reached.add(path)
        source = inputs[path]
        if path != CONTRACTS:
            need(not re.search(r"\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern)\b", source), "no added trust: " + str(path))
        for statement, target in EDGES.get(path, []):
            need(target in FILES and source.count(statement) == 1, "exact edge: " + statement)
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra input: " + str(path))
    need(reached == set(FILES), "complete reachable closure")


def mutations(body, single=False):
    cases = {}

    def add(name, macro, old, new, focus, occurrence=None):
        marker = "macro_rules! " + macro + " {"
        need(body.count(marker) == 1, "unique macro: " + macro)
        start = body.index(marker)
        end = body.find("\nmacro_rules!", start + len(marker))
        end = len(body) if end < 0 else end
        part = body[start:end]
        need(part.count(old) == (1 if occurrence is None else 2), "exact mutation: " + name)
        offset = part.find(old) if occurrence in (None, 0) else part.find(old, part.find(old) + len(old))
        offset += start
        cases[name] = (body[:offset] + new + body[offset + len(old):], "*" + focus)

    macro = "completion_record_single_event_body" if single else "completion_record_event_batch_body"
    focus = ONE if single else BATCH
    change = lambda name, old, new, occurrence=None: add(name, macro, old, new, focus, occurrence)
    for name, expression in (("ready", "$owner.require_ready()"), ("bound", "$owner.validate_bound($retention)"),
                             ("identity", "validate_logical_identity($session, $epoch)")):
        change("ignores-" + name, expression, "Ok::<(), Gfx942CompletionErrorV1>(())")
    if single:
        change("capacity-boundary", ">= GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1", "> GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1")
        change("wrong-index", "$retention, $index, None", "$retention, 0, None")
        change("counter-increment", "next_event_id.checked_add(1)", "next_event_id.checked_add(0)")
        change("pin-increment", "record.event_pins.checked_add(1)", "record.event_pins.checked_add(0)")
        change("wrong-pin-read", "let record = &$owner.slots[exact.slot.index as usize];", "let record = &$owner.slots[0];")
        change("ignores-reservation", "if $reservation.is_err()", "if false")
        change("fabricates-reservation-refusal", "if $reservation.is_err()", "if true")
        change("wrong-ledger-key", ".insert(event_id, exact)", ".insert(0, exact)")
        change("omits-ledger-insert", "$owner.dependency_ledger.events.insert(event_id, exact)", "None::<ExactCompletionOccurrenceV1>")
        change("omits-pin", "$owner.slots[exact.slot.index as usize].event_pins = next_event_pins;", "let _ = next_event_pins;")
        change("rewinds-counter", "$owner.dependency_ledger.next_event_id = next_event_id;", "$owner.dependency_ledger.next_event_id = 1;")
        change("wrong-returned-id", "{ event_id, exact }", "{ event_id: 0, exact }")
        expected = 15
    else:
        for field in ("next_event_id", "next_reader_lease_id"):
            add("constructor-" + field, "completion_dependency_ledger_new_body", field + ": 1,", field + ": 0,", "new")
        for value in ("session", "epoch"):
            add("logical-ignores-" + value, "completion_logical_identity_body", "if $" + value + " == 0", "if false", "validate_logical_identity")
        change("capacity-boundary", "next_len > GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1", "next_len >= GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1")
        change("wrong-count", "u64::try_from($n)", "u64::try_from(0usize)")
        change("counter-increment", "next_event_id.checked_add(count)", "next_event_id.checked_add(0)")
        change("ignores-pin-overflow", ".event_pins.checked_add(1).is_none()", ".event_pins.checked_add(0).is_none()")
        change("wrong-pin-read", "$owner.slots[$occurrence.slot.index as usize].event_pins.checked_add", "$owner.slots[0].event_pins.checked_add")
        change("checks-prefix", "while $i < $n $($preflight)*", "while $i < $n && $i == 0 $($preflight)*")
        for index, label in enumerate(("ledger", "output")):
            change(label + "-ignores-reservation", "if $reservation.is_err()", "if false", index)
            change(label + "-fabricates-refusal", "if $reservation.is_err()", "if true", index)
        change("commits-prefix", "while $i < $n $($commit)*", "while $i < $n && $i == 0 $($commit)*")
        change("wrong-commit-occurrence", "exact_occurrence($session, $epoch, $retention, $i, None)\n                    .expect", "exact_occurrence($session, $epoch, $retention, 0, None)\n                    .expect")
        change("wrong-id", "next_event_id + $i as u64", "next_event_id")
        change("omits-ledger-insert", "$owner.dependency_ledger.events.insert(event_id, $occurrence)", "None::<ExactCompletionOccurrenceV1>")
        change("omits-pin", ".event_pins += 1;", ".event_pins += 0;")
        change("doubles-pin", ".event_pins += 1;", ".event_pins += 2;")
        change("wrong-pin-write", "$owner.slots[$occurrence.slot.index as usize].event_pins +=", "$owner.slots[0].event_pins +=")
        change("wrong-token", "{ event_id, exact: $occurrence }", "{ event_id: 0, exact: $occurrence }")
        change("omits-token", "$events.push(Gfx942ComputeEventOccurrenceV1 { event_id, exact: $occurrence });", "")
        change("rewinds-counter", "$owner.dependency_ledger.next_event_id = $next;", "$owner.dependency_ledger.next_event_id = 1;")
        change("rewinds-reader-counter", "$owner.dependency_ledger.next_event_id = $next;", "$owner.dependency_ledger.next_event_id = $next; $owner.dependency_ledger.next_reader_lease_id = 1;")
        for wrapper, method in (("completion_record_dependency_batch_body", "record_dependency_event_batch_v1"),
                                ("completion_record_bound_dependency_batch_body", "record_dependency_event_batch_for_bound_v1")):
            for parameter in ("session", "epoch"):
                old = "($session, $epoch,"
                new = "(0, $epoch," if parameter == "session" else "($session, 0,"
                add(wrapper + "-wrong-" + parameter, wrapper, old, new, method)
        expected = 32
    need(len(cases) == len(set(cases.values())) == expected, "distinct issuance mutants")
    return cases


def packing_mutations(body):
    cases = {}

    def add(name, old, new, focus):
        need(body.count(old) == 1, "exact packing mutation: " + name)
        cases[name] = (body.replace(old, new), "*" + focus)

    reserve, pack = "reserve_source_output", "pack_source_output"
    add("wrong-failure-class", "::RejectedBeforeSideEffect(", "::RetryableBeforeSideEffect(", reserve)
    add("wrong-failure-detail", '"dependency source event output allocation"', '"wrong allocation"', reserve)
    add("fabricates-refusal", "Ok(()) => $finish!(Ok(output)),", 'Ok(()) => $finish!(Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1::Contract("dependency source event output allocation")))),', reserve)
    old = 'Err(_error) => $finish!(Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(\n                    ComputeAqlQueueSessionErrorV1::Contract("dependency source event output allocation"),\n                ))),'
    add("ignores-refusal", old, "Err(_error) => $finish!(Ok(output)),", reserve)
    push = "$output.push(Gfx942ComputeDependencyEventV1 { lane: $lane, event: $event });"
    add("omits-token", push, "", pack)
    add("wrong-lane", "lane: $lane, event:", "lane: ComputeAqlQueueLaneV1 { generation: 0, ..$lane }, event:", pack)
    add("reverses-order", "$output.push(Gfx942ComputeDependencyEventV1", "$output.insert(0, Gfx942ComputeDependencyEventV1", pack)
    add("drops-prefix", "let mut $pending = $events.into_iter();", "$output.clear(); let mut $pending = $events.into_iter();", pack)
    need(len(cases) == len(set(cases.values())) == 8, "distinct packing mutants")
    return cases


def selection_notes(leaf, focus=None):
    free = {"validate_logical_identity", "reserve_source_output", "pack_source_output"}
    methods = {BATCH, ONE, "record_dependency_event_batch_v1", "record_dependency_event_batch_for_bound_v1"}
    names = free | methods | {"new"}
    need(focus is None or focus in {"*" + name for name in names}, "exact issuance selector")
    selected = names if focus is None else [focus[1:]]
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({BOUNDS_ERROR} if focus in BOUNDS_SELECTORS else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            *{"verifying root module, function completion_event_issue_v1::"
              + ("CompletionSignalArenaOwnerV1::" if name in methods else "CompletionDependencyLedgerV1::" if name == "new" else "")
              + name + " (selected functions)" for name in selected}})


def campaign(mode="batch"):
    need(mode in ("batch", "single", "packing"), "exact campaign mode")
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    source = raw.decode()
    need(source.count('"--no-cheating", ') == 1, "exact explicit trust-profile adaptation")
    source = source.replace('"--no-cheating", ', '')
    module = types.ModuleType("event_issue_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(source, module.__file__, "exec"), module.__dict__)
    raw = (ROOT / BOUND_CONTROLLER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BOUND_SHA, "authenticated enumeration diagnostic")
    bound = types.ModuleType("event_issue_bound_diagnostic")
    bound.__file__ = str(ROOT / BOUND_CONTROLLER)
    exec(compile(raw, bound.__file__, "exec"), bound.__dict__)
    inherited = module.inherited

    def classifier():
        result = inherited()
        result.ENUMERATION_NOTES |= {bound.BITVECTOR_ENUMERATION_NOTE}
        return result

    module.inherited = classifier
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, OUTPUT if mode == "packing" else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=46)
    module.mutations = packing_mutations if mode == "packing" else lambda body: mutations(body, mode == "single")
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-event-issue.py"))
    modes = [value for value in sys.argv[1:] if value in ("--single", "--packing")]
    need(len(modes) <= 1, "one campaign selector")
    mode = modes[0][2:] if modes else "batch"
    if modes:
        sys.argv.remove(modes[0])
    campaign(mode).main()
