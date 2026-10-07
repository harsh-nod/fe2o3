#!/usr/bin/env python3
"""Qualify shared host preparation with two explicit std conversion contracts."""
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
PROOF = V / "completion_batch_bind_v1.rs"
SCHEMA = V / "completion_owner_schema_v1.rs"
BOUND = V / "completion_bound_cancel_execution_v1.rs"
CONTRACTS = V / "completion_box_contracts_v1.rs"
CONTRACTS_SHA = "5822ed2870b5c5572d0a97045bcb78b0a8b8966f123ee4267a121fca6a8eceea"
AQL = V / "completion_aql_preparation_v1.rs"
AQL_BODY = Path("crates/fe2o3-aql/src/preparation_body.rs")
BODY = Path("crates/fe2o3-kfd/src/queue_completion/batch_bind_body.rs")
BOUND_BODY = BODY.with_name("bound_cancel_body.rs")
ADAPTERS = BODY.with_name("rollback_adapters_body.rs")
READY = BODY.with_name("event_release_body.rs")
FILES = [PROOF, SCHEMA, BOUND, CONTRACTS, AQL, AQL_BODY, BODY, BOUND_BODY, ADAPTERS, READY]
EDGES = {
    PROOF: [('include!("' + path.name + '");', path) for path in (SCHEMA, BOUND, CONTRACTS, AQL)]
        + [('include!("../../fe2o3-kfd/src/queue_completion/' + path.name + '");', path) for path in (READY, BODY)],
    BOUND: [('include!("../../fe2o3-kfd/src/queue_completion/' + path.name + '");', path) for path in (BOUND_BODY, ADAPTERS)],
    AQL: [('include!("../../fe2o3-aql/src/preparation_body.rs");', AQL_BODY)],
}


def need(value, message):
    if not value:
        raise ValueError(message)


def audit(inputs):
    need(set(inputs) == set(FILES), "exact ten-file preparation closure")
    need(hashlib.sha256(inputs[CONTRACTS].encode()).hexdigest() == CONTRACTS_SHA, "exact std conversion contracts")
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
            need(source.count(statement) == 1, "exact edge: " + statement)
            source = source.replace(statement, "")
            pending.append(target)
        need(not re.search(r"\b(?:mod|path|include|include_str|include_bytes|env|option_env)\b", source), "no extra input: " + str(path))
    need(reached == set(FILES), "complete reachable closure")


def mutations(body, aql=False):
    cases = {}

    def add(name, macro, old, new, focus):
        marker = "macro_rules! " + macro + " {"
        need(body.count(marker) == 1, "unique macro: " + macro)
        start = body.index(marker)
        end = body.find("\nmacro_rules!", start + len(marker))
        end = len(body) if end < 0 else end
        part = body[start:end]
        need(part.count(old) == 1, "unique mutation: " + name)
        cases[name] = (body[:start] + part.replace(old, new) + body[end:], "*" + focus)

    if aql:
        add("address-zero", "aql_address_body", "if $raw == 0", "if $raw == 1", "new")
        for name, old, new in (
            ("alignment-bound", "$alignment > 4096", "$alignment >= 4096"),
            ("alignment-power", "$alignment & ($alignment - 1) != 0", "false"),
            ("alignment-address", "$value.0 & ($alignment - 1) != 0", "false"),
        ):
            add(name, "aql_alignment_body", old, new, "require_alignment")
        for name, old, new in (
            ("kernel-check", "$kernel.require_alignment(64)", "$kernel.require_alignment(1)"),
            ("kernarg-check", "$kernarg.require_alignment($alignment)", "$kernarg.require_alignment(1)"),
            ("signal-check", "$signal.require_alignment(AMD_SIGNAL_ALIGNMENT_V1 as u64)", "$signal.require_alignment(1)"),
            ("kernel-error", "AqlDispatchPacketError::KernelObject(error)", "AqlDispatchPacketError::Kernarg(error)"),
            ("grid-field", "grid_size_x: grid[0]", "grid_size_x: grid[1]"),
            ("workgroup-field", "workgroup_size_y: workgroup[1]", "workgroup_size_y: workgroup[0]"),
            ("private-field", "private_segment_size: $private", "private_segment_size: $group"),
            ("reserved-field", "reserved2: 0", "reserved2: 1"),
            ("signal-field", "completion_signal: $signal.raw()", "completion_signal: $kernarg.raw()"),
            ("ordering-field", "ordering: $ordering", "ordering: AqlDispatchOrderingV1::Independent"),
            ("setup-field", "$geometry.dimensions()", "0u16"),
        ):
            add(name, "aql_dispatch_preparation_body", old, new, "new_unpublished_with_ordering")
        add("batch-bound", "aql_boxed_batch_body", "$n > AQL_MAX_FIXED_BATCH_PACKETS_V2", "$n >= AQL_MAX_FIXED_BATCH_PACKETS_V2", "try_from_boxed_packets")
        add("batch-zero", "aql_boxed_batch_body", "if $n == 0", "if $n == 1", "try_from_boxed_packets")
        kernel = """            let $kernel = match $kernel.require_alignment(64) {
                Ok(value) => value,
                Err(error) => return Err(AqlDispatchPacketError::KernelObject(error)),
            };
"""
        kernarg = """            let $kernarg = match $kernarg.require_alignment($alignment) {
                Ok(value) => value,
                Err(error) => return Err(AqlDispatchPacketError::Kernarg(error)),
            };
"""
        add("alignment-error-order", "aql_dispatch_preparation_body", kernel + kernarg, kernarg + kernel, "new_unpublished_with_ordering")
        expected = 18
    else:
        for name, old, new in (
            ("selection-start", "let mut $index = 0;", "let mut $index = 1;"),
            ("selection-generation", "generation: record.generation", "generation: 0"),
            ("selection-phase", "record.phase == CompletionSlotPhaseV1::Available", "true"),
        ):
            add(name, "completion_select_slots_body", old, new, "select_available_slots")
        for name, old, new in (
            ("queue-check", "$binding.queue != $owner.queue", "false"),
            ("dispatch-check", "$binding.dispatch_generation == 0", "false"),
            ("code-vm-check", "$binding.code.allocation.vm != $owner.queue.vm", "false"),
            ("kernarg-vm-check", "$binding.kernarg.allocation.vm != $owner.queue.vm", "false"),
        ):
            add(name, "completion_dispatch_binding_body", old, new, "validate_dispatch_binding")
        for name, old, new in (
            ("commit-tag", "batch_id: $owner.next_batch_id", "batch_id: $next"),
            ("commit-counter", "$owner.next_batch_id = $next;", "$owner.next_batch_id = 0;"),
            ("commit-pins", "$($step)*\n                $index += 1;", "$owner.slots[slot.index as usize].event_pins = 0;\n                $($step)*\n                $index += 1;"),
            ("commit-phase", "$owner.next_batch_id = $next;", "$owner.next_batch_id = $next; $owner.phase = CompletionOwnerPhaseV1::Poisoned;"),
        ):
            add(name, "completion_commit_batch_body", old, new, "commit_bound_batch")
        for name, old, new in (
            ("ready-count-order", "$owner.require_ready()?;\n            validate_packet_count::<$n>()?;", "validate_packet_count::<$n>()?;\n            $owner.require_ready()?;"),
            ("batch-id-increment", "next_batch_id.checked_add(1)", "next_batch_id.checked_add(0)"),
            ("packet-template", "let template = &$templates.values[$index];", "let template = &$templates.values[0];"),
            ("packet-slot", "let slot = &$slots[$index];", "let slot = &$slots[0];"),
            ("short-packet-roster", "$prepared.push(packet);", "if $index != 0 { $prepared.push(packet); }"),
            ("premature-slot-commit", "$prepared.push(packet);", "$prepared.push(packet); $owner.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Bound { batch_id: $owner.next_batch_id };"),
            ("dispatch-roster", "$dispatches.push($templates.values[$index].generations);", "$dispatches.push($templates.values[0].generations);"),
            ("retention-last", "last_packet_id: None", "last_packet_id: Some(0)"),
            ("retention-id", "batch_id: $owner.next_batch_id, queue:", "batch_id: $next, queue:"),
            ("offset-scale", "checked_mul(AMD_SIGNAL_BYTES_V1 as u64)", "checked_mul(1)"),
            ("address-base", "$owner.gpu_base.checked_add(offset)", "0u64.checked_add(offset)"),
        ):
            add(name, "completion_bind_batch_body", old, new, "bind_fixed_batch")
        expected = 22
    need(len(cases) == len(set(cases.values())) == expected, "distinct mutation roster")
    return cases


METHODS = {
    "bind_fixed_batch": "CompletionSignalArenaOwnerV1", "select_available_slots": "CompletionSignalArenaOwnerV1",
    "validate_dispatch_binding": "CompletionSignalArenaOwnerV1", "commit_bound_batch": "CompletionSignalArenaOwnerV1",
    "new": "ObservedGpuAddressV1", "require_alignment": "ObservedGpuAddressV1",
    "new_unpublished_with_ordering": "AqlKernelDispatchPacketV1", "try_from_boxed_packets": "AqlPreparedKernelDispatchBatchV2",
}
BOUNDS_ERROR = "precondition not met: index in bounds for this access"
ARITHMETIC_ERROR = "possible arithmetic underflow/overflow"
INDEXED = {"*bind_fixed_batch", "*select_available_slots", "*commit_bound_batch"}


def selection_notes(leaf, focus=None):
    need(focus is None or focus in {"*" + name for name in METHODS}, "exact preparation selector")
    names = METHODS if focus is None else [focus[1:]]
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS | ({BOUNDS_ERROR, ARITHMETIC_ERROR} if focus in INDEXED else set()),
        SELECTION_NOTES={"verifying root module (selected functions)",
            *{"verifying root module, function completion_batch_bind_v1::" + METHODS[name] + "::" + name
                + " (selected functions)" for name in names}})


def campaign(aql=False):
    audit({path: (ROOT / path).read_bytes().decode("utf-8") for path in FILES})
    raw = (ROOT / BASE).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BASE_SHA, "authenticated publication controller")
    source = raw.decode()
    need(source.count('"--no-cheating", ') == 1, "explicit conversion trust profile")
    module = types.ModuleType("batch_bind_campaign")
    module.__file__ = str(ROOT / BASE)
    sys.modules[module.__name__] = module
    exec(compile(source.replace('"--no-cheating", ', ''), module.__file__, "exec"), module.__dict__)
    raw = (ROOT / BOUND_CONTROLLER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == BOUND_SHA, "authenticated enumeration diagnostic")
    bound = types.ModuleType("batch_bind_bound_diagnostic")
    bound.__file__ = str(ROOT / BOUND_CONTROLLER)
    exec(compile(raw, bound.__file__, "exec"), bound.__dict__)
    inherited = module.inherited

    def classifier():
        result = inherited()
        result.ENUMERATION_NOTES |= {bound.BITVECTOR_ENUMERATION_NOTE}
        return result

    module.inherited = classifier
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, AQL_BODY if aql else BODY
    module.EXPECTED = dict(module.EXPECTED, verified=55)
    module.mutations = lambda body: mutations(body, aql)
    module.selection_notes = selection_notes
    return module


if __name__ == "__main__":
    runpy.run_path(str(ROOT / V / "test-completion-batch-bind.py"))
    aql = "--aql" in sys.argv
    if aql:
        sys.argv.remove("--aql")
    campaign(aql).main()
