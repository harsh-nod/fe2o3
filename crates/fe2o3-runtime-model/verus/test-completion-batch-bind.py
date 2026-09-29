#!/usr/bin/env python3
"""Fail-closed preparation closure, production joins and classifier controls."""
import json
from pathlib import Path
import re
import runpy

r = runpy.run_path(str(Path(__file__).with_name("check-completion-batch-bind.py")))
need, root = r["need"], r["ROOT"]
inputs = {path: (root / path).read_bytes().decode("utf-8") for path in r["FILES"]}


def rejects(operation):
    try:
        operation()
    except ValueError:
        return
    raise ValueError("invalid calibration input accepted")


r["audit"](inputs)
for path in inputs:
    for suffix in ('\nassume(false);', '\n#[verifier::external_body]', '\nmod foreign;',
                   '\ninclude!("/foreign.rs");', '\ninclude_str!("/foreign.rs");',
                   '\ninclude_bytes!("/foreign.rs");', '\nenv!("FOREIGN");', '\noption_env!("FOREIGN");'):
        rejects(lambda: r["audit"]({**inputs, path: inputs[path] + suffix}))
    rejects(lambda: r["audit"]({p: text for p, text in inputs.items() if p != path}))
rejects(lambda: r["audit"]({**inputs, Path("/foreign.rs"): ""}))
for path, edges in r["EDGES"].items():
    for statement, _ in edges:
        for replacement in ("", statement + "\n" + statement, statement.replace(".rs", "-foreign.rs")):
            rejects(lambda: r["audit"]({**inputs, path: inputs[path].replace(statement, replacement)}))
contract = r["CONTRACTS"]
for text in (inputs[contract] + "\n", inputs[contract].replace("\n", "\r\n")):
    rejects(lambda: r["audit"]({**inputs, contract: text}))


def compact(text):
    return re.sub(r",([)}])", r"\1", re.sub(r"\s+", "", text))


production = [(root / path).read_text() for path in (
    "crates/fe2o3-kfd/src/queue_completion.rs", "crates/fe2o3-aql/src/lib.rs")]
patterns = [
    [
        'include!("queue_completion/batch_bind_body.rs");',
        'macro_rules!completion_rust_expr{($body:expr)=>{$body};}',
        'fnbind_fixed_batch<constN:usize>(&mutself,templates:CompletionPacketTemplatesV1<N>)->Result<BoundCompletionBatchV1<N>,Gfx942CompletionErrorV1>{completion_bind_batch_body!(completion_rust_expr,self,templates,N)}',
        'fnselect_available_slots(&self,count:usize)->Vec<CompletionSlotLeaseV1>{completion_select_slots_body!(completion_rust_expr,self,count)}',
        'fncommit_bound_batch<constN:usize>(&mutself,bound:BoundCompletionBatchV1<N>,next_batch_id:u64)->BoundCompletionBatchV1<N>{completion_commit_batch_body!(completion_rust_expr,self,bound,next_batch_id,N)}',
        'fnvalidate_dispatch_binding(&self,binding:CompletionDispatchGenerationBindingV1)->Result<(),Gfx942CompletionErrorV1>{completion_dispatch_binding_body!(completion_rust_expr,self,binding)}',
        'fnrequire_ready(&self)->Result<(),Gfx942CompletionErrorV1>{completion_require_ready_body!(completion_rust_expr,self)}',
        'fnvalidate_packet_count<constN:usize>()->Result<(),Gfx942CompletionErrorV1>{completion_packet_count_body!(completion_rust_expr,N)}',
        'constCOMPLETION_SIGNAL_CAPACITY_V1:usize=AQL_MAX_FIXED_BATCH_PACKETS_V2asusize;',
    ],
    [
        'include!("preparation_body.rs");',
        'macro_rules!aql_rust_expr{($body:expr)=>{$body};}',
        'pubconstfnnew(raw:u64)->Result<Self,AqlAddressObservationError>{aql_address_body!(aql_rust_expr,raw)}',
        'pubconstfnraw(self)->u64{aql_field_body!(aql_rust_expr,self,0)}',
        'pubconstfnrequire_alignment(self,alignment:u64)->Result<Self,AqlAddressObservationError>{aql_alignment_body!(aql_rust_expr,self,alignment)}',
        'pubconstfngrid(self)->[u32;3]{aql_field_body!(aql_rust_expr,self,grid)}',
        'pubconstfnworkgroup(self)->[u16;3]{aql_field_body!(aql_rust_expr,self,workgroup)}',
        'pubconstfndimensions(self)->u16{aql_field_body!(aql_rust_expr,self,dimensions)}',
        'pubfnnew_unpublished_with_ordering(geometry:AqlDispatchGeometryV1,private_segment_size:u32,group_segment_size:u32,kernel_object:ObservedGpuAddressV1,kernarg_address:ObservedGpuAddressV1,kernarg_alignment:u64,completion_signal:ObservedGpuAddressV1,ordering:AqlDispatchOrderingV1)->Result<AqlPreparedKernelDispatchV1,AqlDispatchPacketError>{aql_dispatch_preparation_body!(aql_rust_expr,geometry,private_segment_size,group_segment_size,kernel_object,kernarg_address,kernarg_alignment,completion_signal,ordering)}',
        'pubfntry_from_boxed_packets(packets:Box<[AqlPreparedKernelDispatchV1;N]>)->Result<Self,AqlPreparedKernelDispatchBatchErrorV1>{aql_boxed_batch_body!(aql_rust_expr,packets,N)}',
        'constAMD_SIGNAL_BYTES_V1:usize=64;', 'constAMD_SIGNAL_ALIGNMENT_V1:usize=64;',
        'constAQL_INVALID_PACKET_HEADER_V1:u16=1;', 'constAQL_MAX_FIXED_BATCH_PACKETS_V2:u32=8192;',
    ],
]


def wiring(sources):
    for source, group in zip(sources, patterns):
        source = compact(source)
        for pattern in group:
            need(source.count(pattern) == 1, "exact production join: " + pattern)


wiring(production)
for i, group in enumerate(patterns):
    for pattern in group:
        changed = list(map(compact, production))
        changed[i] = changed[i].replace(pattern, "WRONG")
        rejects(lambda: wiring(changed))


def shape(source, name):
    source = re.sub(r"//[^\n]*", "", source)
    source = re.sub(r"\bpub(?:\([^)]*\))?\s+", "", source)
    matches = list(re.finditer(r"\b(?:struct|enum)\s+" + name + r"(?:<[^>]*>)?\s*\{", source))
    need(len(matches) == 1, "unique shape: " + name)
    start = matches[0].end()
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    need(depth == 0, "closed shape")
    return compact(source[start:end - 1]).rstrip(",")


def projections(sources, model):
    groups = (
        (0, r["PROOF"], ("CompletionPacketTemplateV1", "CompletionPacketTemplatesV1", "BoundCompletionBatchV1")),
        (0, r["BOUND"], ("CompletionDispatchGenerationBindingV1", "CompletionBatchRetentionV1")),
        (0, r["SCHEMA"], ("CompletionSlotLeaseV1", "CompletionSlotRecordV1", "CompletionSlotPhaseV1", "CompletionOwnerPhaseV1")),
        (1, r["SCHEMA"], ("AqlAddressObservationError", "AqlDispatchPacketError", "AqlPreparedKernelDispatchBatchErrorV1")),
        (1, r["AQL"], ("AqlDispatchGeometryV1", "AqlKernelDispatchPacketV1", "AqlPreparedKernelDispatchV1", "AqlPreparedKernelDispatchBatchV2")),
    )
    for index, path, names in groups:
        for name in names:
            need(shape(sources[index], name) == shape(model[path], name), "exact shape: " + name)
    need('structObservedGpuAddressV1(u64);' in compact(sources[1]), "numeric address projection")
    need('structObservedGpuAddressV1(u64);' in compact(model[r["AQL"]]), "model address projection")
    # Ordering cases are retained here; header encoding/publication is not proved.
    need(shape(sources[1], "AqlDispatchOrderingV1") ==
         "Independent=AQL_SYSTEM_SCOPED_KERNEL_DISPATCH_HEADER_V1,WaitForPrior=AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1",
         "exact production ordering cases")
    need(shape(model[r["AQL"]], "AqlDispatchOrderingV1") == "Independent,WaitForPrior", "logical ordering projection")
    for pattern in patterns[1][-4:]:
        need(pattern in compact(model[r["AQL"]]), "exact model constants")
    need('constCOMPLETION_SIGNAL_CAPACITY_V1:usize=8192;' in compact(model[r["SCHEMA"]]), "exact model capacity")
    for variant in ('BatchIdentityExhausted', 'InsufficientSignals', 'WrongQueueGeneration', 'WrongVmGeneration',
                    "InvalidArena(&'staticstr)", 'PacketBinding(AqlDispatchPacketError)',
                    'BatchConstruction(AqlPreparedKernelDispatchBatchErrorV1)'):
        need(variant in shape(sources[0], 'Gfx942CompletionErrorV1')
             and variant in shape(model[r["SCHEMA"]], 'Gfx942CompletionErrorV1'), "preparation error projection")


projections(production, inputs)
for path, old, new in (
    (r["SCHEMA"], 'KernelObject(AqlAddressObservationError)', 'KernelObject(u64)'),
    (r["AQL"], 'const AMD_SIGNAL_BYTES_V1: usize = 64;', 'const AMD_SIGNAL_BYTES_V1: usize = 1;'),
    (r["PROOF"], 'kernarg_alignment: u64', 'kernarg_alignment: u32'),
):
    need(old in inputs[path], "projection calibration site")
    rejects(lambda: projections(production, {**inputs, path: inputs[path].replace(old, new)}))

for aql, count in ((False, 22), (True, 18)):
    campaign = r["campaign"](aql)
    body = inputs[campaign.BODY]
    cases = campaign.mutations(body)
    need(len(cases) == count, "exact mutation roster")
    rejects(lambda: campaign.mutations(body + body))
    for text, _ in cases.values():
        r["audit"]({**inputs, campaign.BODY: text})
    classifier = campaign.inherited()
    leaf = classifier.inherited()
    verifier = {"version": "calibration-only"}
    path = "/snapshot/completion_batch_bind_v1.rs"
    result = {"verus": verifier, "verification-results": {
        "encountered-error": True, "encountered-vir-error": False,
        "is-verifying-entire-crate": False, "errors": 1, "verified": 0}}
    error = {"$message_type": "diagnostic", "level": "error", "message": "postcondition not satisfied",
        "code": None, "children": [], "spans": [{"is_primary": True, "file_name": path}]}
    for _, focus in cases.values():
        notes = r["selection_notes"](leaf, focus)
        check = lambda diagnostics, status=1: classifier.logical_negative(notes, status, json.dumps(result),
            "\n".join(json.dumps(d) for d in diagnostics), verifier, {path})
        need(check([error]), "logical negative")
        for status in (0, 124, 137, -9):
            need(not check([error], status), "nonlogical status rejected")
        for message in notes.SELECTION_NOTES:
            note = dict(error, level="note", message=message, spans=[])
            need(check([note, error]), "exact selector")
            need(not check([dict(note, message=message + " unknown"), error]), "unknown selector rejected")
        for message in (r["BOUNDS_ERROR"], r["ARITHMETIC_ERROR"]):
            need(check([dict(error, message=message)]) == (focus in r["INDEXED"]), "indexed diagnostics scoped")
        for hostile in (dict(error, level="warning"), dict(error, children=[error]),
                        dict(error, spans=[{"is_primary": True, "file_name": "/foreign.rs"}])):
            need(not check([hostile]), "hostile diagnostic rejected")
        for message in ("recommendation not met", "Resource limit (rlimit) exceeded", "type annotations needed", "internal error"):
            need(not check([error, dict(error, message=message)]), "mixed nonlogical failure rejected")
    positive = {"verus": verifier, "verification-results": campaign.EXPECTED}
    check_positive = lambda status, diagnostics: classifier.proof_positive(status, json.dumps(positive), diagnostics,
        verifier, campaign.EXPECTED, {path})
    need(check_positive(0, ""), "exact positive")
    need(not check_positive(1, "") and not check_positive(0, json.dumps(error)), "invalid positive rejected")
print("PASS: completion batch preparation calibration (5 groups)")
