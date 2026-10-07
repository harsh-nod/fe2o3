#!/usr/bin/env python3
"""Conditional shared retirement source binding; not native disposal authority."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
V = Path('crates/fe2o3-runtime-model/verus')
C = Path('crates/fe2o3-runtime/src/context')
CONTEXT = C.with_suffix('.rs')
GRAPH = C / 'graph.rs'
UNPUBLISHED = C / 'unpublished.rs'
REPLICAS = C / 'replicas.rs'
STORAGE = C / 'replicas/storage.rs'
BODY = C / 'graph/retirement_bodies.rs'
HOST_TABLE = Path('crates/fe2o3-resource-accounting/src/host_table.rs')
GUARD = Path('crates/fe2o3-runtime-model/src/r63_graph_reservation.rs')
MODEL_ROOT = GUARD.parent / 'lib.rs'
RUNTIME_ROOT = C.parent / 'lib.rs'
CPU_ROOT = C / 'tests/replica_tests.rs'
CPU_TEST = C / 'tests/replica_tests/retirement.rs'
PROOF = V / 'graph_reservation_retirement_v1.rs'
DEFINITIONS = V / 'graph_reservation_retirement_definitions_v1.rs'
PROOF_FILES = (PROOF, DEFINITIONS, BODY)
SOURCE_FILES = (CONTEXT, GRAPH, UNPUBLISHED, REPLICAS, STORAGE, HOST_TABLE,
                GUARD, MODEL_ROOT, RUNTIME_ROOT, CPU_ROOT, CPU_TEST)
FILES = (*PROOF_FILES, *SOURCE_FILES)
HELPERS = V / 'check-graph-version-ledger-v1.py'
HELPERS_SHA = 'e7c78fbf6aaf9f0539f4f2f4294ea79f61e5d17fdbe3ef7fa19693dfb6efc601'
MANIFEST = BASE / 'pins/GRAPH_RESERVATION_RETIREMENT_INPUTS_V1.json'
MANIFEST_PIN = BASE / 'pins/GRAPH_RESERVATION_RETIREMENT_INPUTS_V1_SHA256'


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         'ordinary canonical source: ' + str(path))
    return path.read_bytes()


def helpers():
    path = ROOT / HELPERS
    need(sha(ordinary(path)) == HELPERS_SHA, 'reviewed unchanged source parser')
    spec = importlib.util.spec_from_file_location('retirement_sources', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def unique_json(raw):
    return helpers().unique_json(raw)


def fields(text, name, helper, parser):
    body = helper.item_body(text, 'struct', name, parser)
    result, part, depth = {}, [], 0
    for token in [*body, ',']:
        if token == ',' and depth == 0:
            if not part:
                continue
            if part[0] == 'pub':
                part = part[1:]
                if part and part[0] == '(':
                    end = parser.matching_token(part, 0, '(', ')')
                    part = part[end + 1:]
            need(len(part) >= 3 and part[1] == ':' and part[0] not in result,
                 'distinct complete field declaration')
            result[part[0]] = ''.join(part[2:])
            part = []
        else:
            part.append(token)
            depth += token in ('<', '(', '[')
            depth -= token in ('>', ')', ']')
            need(depth >= 0, 'balanced field type')
    need(depth == 0 and not part, 'complete field roster')
    return result


def source_shapes(values):
    helper = helpers()
    parser = helper.lexer()
    token = lambda text: helper.tokens(text, parser)
    body = lambda path, kind, name: helper.item_body(values[path], kind, name, parser)
    native = {
        'scope_epoch': 'scope_epoch::Anchor', 'replicas': 'Option<RuntimeReplicaStorageV1>',
        'backend': 'B', 'context_generation': 'u64', 'devices': 'Vec<RuntimeDeviceV1>',
        'streams': 'HashMap<RuntimeStreamIdV1,StreamRecordV1>', 'backend_streams': 'HashSet<u64>',
        'allocations': 'HashMap<RuntimeAllocationIdV1,AllocationRecordV1>',
        'backend_allocations': 'HashSet<u64>', 'allocation_admission': 'ContextAllocationAdmissionV1',
        'versions': 'Option<ContextVersionsV1>', 'modules': 'HashMap<RuntimeModuleIdV1,ModuleRecordV1>',
        'backend_modules': 'HashSet<u64>', 'kernels': 'HashMap<u64,KernelRecordV1>',
        'events': 'HashMap<RuntimeEventIdV1,EventRecordV1>', 'backend_events': 'HashSet<u64>',
        'submissions': 'HashMap<RuntimeSubmissionIdV1,SubmissionRecordV1>',
        'backend_submissions': 'HashSet<u64>',
        'scalar_peer_copies': 'HashMap<RuntimeSubmissionIdV1,ScalarPeerCopyRootV1>',
        'producer_launches': 'HashMap<RuntimeSubmissionIdV1,ProducerLaunchRootV1>',
        'same_device_copies': 'HashMap<RuntimeSubmissionIdV1,SameDeviceCopyRootV1>',
        'segmented_peer_copies': 'HashMap<RuntimeSubmissionIdV1,SegmentedPeerCopyRootV1>',
        'generated_issues': 'HashMap<RuntimeStreamIdV1,generated_issue::GeneratedIssueV1>',
        'completion_callbacks': 'HashMap<RuntimeSubmissionIdV1,Vec<RuntimeCompletionCallbackV1>>',
        'completion_callback_count': 'usize', 'completion_callback_panic_count': 'u64',
        'next_identity': 'u64', 'terminal': 'bool',
        'graph_reservation': 'Option<ContextGraphReservationV1>',
        'native_pair_reservation': 'Option<u64>', 'graph_issue_closed': 'bool',
    }
    need(fields(values[CONTEXT], 'RuntimeContextV1', helper, parser) == native,
         'all 31 native Context fields and exact types')
    projected = dict(native)
    for name in ('scope_epoch', 'backend', 'allocation_admission'):
        projected[name] = 'O'
    projected.update(replicas='Option<RuntimeReplicaStorageV1<O>>', devices='Vec<O>',
                     streams='HashMap<K,StreamRecordV1<O>>', versions='Option<O>',
                     kernels='HashMap<u64,O>', completion_callbacks='HashMap<K,Vec<O>>')
    for name in ('allocations', 'modules', 'events', 'submissions', 'scalar_peer_copies',
                 'producer_launches', 'same_device_copies', 'segmented_peer_copies', 'generated_issues'):
        projected[name] = 'HashMap<K,O>'
    need(fields(values[DEFINITIONS], 'RuntimeContextV1', helper, parser) == projected,
         'explicit complete opaque-owner and key projection')
    frame = ' '.join('&&& self.' + name + ' == before.' + name
                     for name in native if name != 'graph_reservation')
    need(body(DEFINITIONS, 'fn', 'retained_frame') == token(frame), 'all 30 retained fields framed exactly')
    for name, path in (('ContextGraphReservationV1', GRAPH), ('RuntimeReplicaUsageV1', REPLICAS)):
        need(fields(values[path], name, helper, parser) == fields(values[DEFINITIONS], name, helper, parser),
             'exact ' + name + ' fields')
    helper.position(token(values[GRAPH]), token('#[derive(Clone, Copy, Debug, Eq, PartialEq)] '
        'pub(crate) struct ContextGraphReservationV1'))
    helper.position(token(values[DEFINITIONS]), token('#[derive(Clone, Copy, PartialEq, Eq)] '
        'struct ContextGraphReservationV1'))
    stream = fields(values[CONTEXT], 'StreamRecordV1', helper, parser)
    need(stream == dict(backend_stream='u64', device='RuntimeDeviceIdV1',
                        unpublished='Option<u64>', generated='Option<u64>'), 'complete stream record')
    need(fields(values[DEFINITIONS], 'StreamRecordV1', helper, parser) == dict(stream, device='O'),
         'stream owner framing')
    need(fields(values[STORAGE], 'RuntimeReplicaStorageV1', helper, parser)
         == {'slots': 'HostMetadataTableV1<ReplicaSlotV1>'}, 'original fixed credited table')
    need(fields(values[HOST_TABLE], 'HostMetadataTableV1', helper, parser)
         == dict(slots='Vec<T>', credits='Option<RetainedResourceCreditsV1>'), 'retained credit owner')
    need(fields(values[DEFINITIONS], 'HostMetadataTableV1', helper, parser)
         == dict(slots='Vec<T>', credits='Option<C>'), 'opaque credit owner is not erased')
    need(fields(values[STORAGE], 'ReplicaSlotV1', helper, parser)
         == dict(incarnation='u64', state='ReplicaStateV1'), 'complete slot incarnation and state')
    need(fields(values[DEFINITIONS], 'ReplicaSlotV1', helper, parser)
         == dict(incarnation='u64', state='ReplicaStateV1<O>'), 'complete slot projection')
    expected_state = token('Vacant, Pending(ReplicaPendingV1), Settled(ReplicaFactV1),')
    need(body(STORAGE, 'enum', 'ReplicaStateV1') == expected_state, 'complete native replica state')
    need(helper.trailing_commas(body(DEFINITIONS, 'enum', 'ReplicaStateV1'))
         == helper.trailing_commas(token('Vacant, Pending(O), Settled(O)')), 'opaque state payloads')
    need(body(HOST_TABLE, 'fn', 'deref') == body(DEFINITIONS, 'fn', 'deref'), 'actual table dereference')
    need(body(GUARD, 'fn', 'r63_graph_can_release_v1')
         == body(DEFINITIONS, 'fn', 'r63_graph_can_release_v1'), 'unchanged pure release guard')
    need(helper.parameters(values[GUARD], 'r63_graph_can_release_v1', parser)
         == helper.parameters(values[DEFINITIONS], 'r63_graph_can_release_v1', parser), 'guard arguments')
    wrappers = {
        (GRAPH, 'release_graph_v1'): 'self.require_graph_access(Some(token))?; '
            'use retirement_bodies::graph_retirement_runtime_expr; '
            'retirement_bodies::graph_reservation_retirement_body_v1!'
            '(graph_retirement_runtime_expr,self,token,(stream,values,found),[],[],[])',
        (UNPUBLISHED, 'has_unpublished_holds_v1'):
            'use super::graph::retirement_bodies::graph_retirement_runtime_expr; '
            'super::graph::retirement_bodies::graph_unpublished_holds_body_v1!'
            '(graph_retirement_runtime_expr,self,(record,values,found),[],[],[])',
        (REPLICAS, 'pending_replicas_v1'):
            'use super::graph::retirement_bodies::graph_retirement_runtime_expr; '
            'super::graph::retirement_bodies::graph_pending_replicas_body_v1!(graph_retirement_runtime_expr,self)',
        (STORAGE, 'usage'): 'use super::super::graph::retirement_bodies::graph_retirement_runtime_expr; '
            'super::super::graph::retirement_bodies::graph_replica_usage_body_v1!'
            '(graph_retirement_runtime_expr,self,(index,pending,settled,count),[],[])',
    }
    for (path, name), expected in wrappers.items():
        need(helper.trailing_commas(body(path, 'fn', name)) == helper.trailing_commas(token(expected)),
             'only original prefix and exact shared body: ' + name)
    helper.position(token(values[BODY]), token('macro_rules! graph_retirement_runtime_expr'))
    need(body(BODY, '!', 'graph_retirement_runtime_expr')
         == token('($expression:expr) => { $expression };'), 'native syntax callback is exact identity')
    for name in ('obeys_eq_spec', 'eq_spec'):
        expected = 'true' if name == 'obeys_eq_spec' else '*self == *other'
        need(body(DEFINITIONS, 'fn', name) == token(expected), 'derived token equality contract')
    for path, declaration in ((RUNTIME_ROOT, 'mod context;'), (CONTEXT, 'mod graph;'),
                              (CONTEXT, 'mod replicas;'), (CONTEXT, 'mod unpublished;'),
                              (GRAPH, 'pub(super) mod retirement_bodies;'),
                              (REPLICAS, 'mod storage;'), (CPU_ROOT, 'mod retirement;')):
        code = token(values[path])
        index = helper.position(code, token(declaration))
        need(code[:index].count('{') == code[:index].count('}') and
             (index == 0 or code[index - 1] != ']'), 'active unattributed module edge ' + declaration)
    helper.root_module(values[MODEL_ROOT], 'r63_graph_reservation', parser)
    for statement in ('include!("graph_reservation_retirement_definitions_v1.rs");',
                      'include!("../../fe2o3-runtime/src/context/graph/retirement_bodies.rs");'):
        helper.active_include(values[PROOF], statement, parser)
    for path in PROOF_FILES:
        code = parser.code_only(values[path])
        need(not re.search(r'\b(?:assume\w*|admit\w*|axiom|external\w*|unsafe|extern|cfg|cfg_attr)\b', code),
             'no added trusted or inactive proof: ' + str(path))
        need(not re.search(r'\b(?:struct|enum|type|trait|mod|as)\s+(?:Vec|Option|Some|None|std|vstd|HashMap|HashSet)\b', code),
             'no standard-library shadow')
        need('include_str' not in code and 'include_bytes' not in code and 'option_env' not in code,
             'no hidden proof inputs')
        if path != BODY:
            need('macro_rules' not in code, 'no executable macro shadow')


def snapshot():
    return {path: ordinary(ROOT / path).decode('ascii') for path in FILES}


def manifest(values):
    return {str(path): sha(text.encode('ascii')) for path, text in values.items()}


def audit(values):
    need(set(values) == set(FILES), 'closed native and proof source roster')
    raw = ordinary(MANIFEST)
    need(sha(raw) == ordinary(MANIFEST_PIN).decode().strip(), 'source manifest digest')
    record = helpers().unique_json(raw)
    need(set(record) == {'scope', 'files'} and record['scope'] == 'post-access shared retirement only; no native disposal',
         'exact conditional proof scope')
    need(record['files'] == manifest(values), 'exact reviewed input bytes')
    source_shapes(values)
    return record


if __name__ == '__main__':
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python3 -I -B required')
    audit(snapshot())
    print('PASS: exact graph reservation retirement source closure (source only)')
