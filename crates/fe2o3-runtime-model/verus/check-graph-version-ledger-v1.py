#!/usr/bin/env python3
"""Source binding for actual graph ledger transactions, not native settlement."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
V = Path('crates/fe2o3-runtime-model/verus')
NATIVE = Path('crates/fe2o3-runtime/src/async_engine/graph/versions.rs')
GRAPH_ROOT = NATIVE.parent.with_suffix('.rs')
ASYNC_ROOT = GRAPH_ROOT.parent.with_suffix('.rs')
RUNTIME_ROOT = ASYNC_ROOT.parent / 'lib.rs'
BODY = NATIVE.with_suffix('') / 'transition_bodies.rs'
CPU_TEST = NATIVE.with_suffix('') / 'transition_tests.rs'
GUARDS = Path('crates/fe2o3-runtime-model/src/r65_graph_versions.rs')
MODEL_ROOT = GUARDS.parent / 'lib.rs'
NODE = Path('crates/fe2o3-completion/src/graph.rs')
ADMITTED = NATIVE.parent / 'admitted.rs'
STAGING = Path('crates/fe2o3-runtime/src/context/generated_scope/graph/staging.rs')
PROOF = V / 'graph_version_ledger_v1.rs'
DEFINITIONS = V / 'graph_version_ledger_definitions_v1.rs'
PROOF_FILES = (PROOF, DEFINITIONS, BODY)
SOURCE_FILES = (NATIVE, CPU_TEST, GUARDS, MODEL_ROOT, NODE, ADMITTED, STAGING,
                GRAPH_ROOT, ASYNC_ROOT, RUNTIME_ROOT)
FILES = (*PROOF_FILES, *SOURCE_FILES)
LEXER = V / 'check-negative-quality.py'
LEXER_SHA = '74c83c0776206e6c486ebb0bf9fbf6bcfdf207a1f86c22bed6054b5f7438c5fd'
MANIFEST = BASE / 'pins/GRAPH_VERSION_LEDGER_INPUTS_V1.json'
MANIFEST_PIN = BASE / 'pins/GRAPH_VERSION_LEDGER_INPUTS_V1_SHA256'
PREDECESSOR = '2653a4ead0ac1c659b82c5d49b56230a1c096b1f'
PREDECESSOR_BODIES = {
    'begin': '8f5ef29c4bca94bc4ab396b8d3e325cd1f07e39bdc00f8b0364fc457d0dde707',
    'commit': 'f3bfff3d6c1e61e2daa0e9fa96d33790bd0a23d574a7bd27314aaf7ef4847698',
    'fail': 'b597fea1ef6c925e3920338ae77da575e6118ff37e96f96333df113323844fb0',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def unique_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         'ordinary canonical source: ' + str(path))
    return path.read_bytes()


def lexer():
    path = ROOT / LEXER
    need(sha(ordinary(path)) == LEXER_SHA, 'pinned existing Rust lexer')
    spec = importlib.util.spec_from_file_location('graph_ledger_lexer', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def tokens(text, parser):
    return parser.TOKEN.findall(parser.code_only(text))


def trailing_commas(values):
    return [token for i, token in enumerate(values)
            if token != ',' or i + 1 == len(values) or values[i + 1] not in (')', ']')]


def position(sequence, part):
    matches = [i for i in range(len(sequence) - len(part) + 1)
               if sequence[i:i + len(part)] == part]
    need(len(matches) == 1, 'one active token sequence: ' + ''.join(part))
    return matches[0]


def item_body(text, kind, name, parser):
    values = tokens(text, parser)
    start = position(values, [kind, name])
    opening = values.index('{', start)
    closing = parser.matching_token(values, opening, '{', '}')
    return values[opening + 1:closing]


def parameters(text, name, parser):
    values = tokens(text, parser)
    start = position(values, ['fn', name])
    need(values[start + 2] == '(', 'nongeneric executable guard/transaction signature')
    closing = parser.matching_token(values, start + 2, '(', ')')
    return trailing_commas(values[start + 3:closing] + [')'])


def fields(text, name, parser):
    body = item_body(text, 'struct', name, parser)
    result, part, depth = {}, [], 0
    for token in [*body, ',']:
        if token == ',' and depth == 0:
            if not part:
                continue
            need(len(part) >= 3 and part[1] == ':' and part[0] not in result,
                 'complete distinct private field declaration')
            result[part[0]] = ''.join(part[2:])
            part = []
        else:
            part.append(token)
            if token in ('<', '(', '['):
                depth += 1
            elif token in ('>', ')', ']'):
                depth -= 1
            need(depth >= 0, 'balanced field type')
    need(depth == 0 and not part, 'complete field parse')
    return result


def active_include(text, statement, parser):
    marker = '__graph_ledger_include__'
    need(text.count(statement) == 1 and marker not in text, 'one exact include spelling')
    code = tokens(text.replace(statement, marker), parser)
    index = position(code, [marker])
    need(code[:index].count('{') == code[:index].count('}')
         and (index == 0 or code[index - 1] != ']'), 'root non-attributed include')


def root_module(text, name, parser, allowed=()):
    code = tokens(text, parser)
    index = position(code, ['mod', name, ';'])
    need(code[:index].count('{') == code[:index].count('}'), 'active root module')
    attributes = []
    while index > 0 and code[index - 1] == ']':
        depth, opening = 1, index - 2
        while opening >= 0 and depth:
            depth += (code[opening] == ']') - (code[opening] == '[')
            opening -= 1
        opening += 1
        need(depth == 0, 'balanced module attributes')
        if opening >= 2 and code[opening - 2:opening] == ['#', '!']:
            break
        need(opening >= 1 and code[opening - 1] == '#', 'outer module attribute')
        attributes.insert(0, code[opening - 1:index])
        index = opening - 1
    need(attributes == list(allowed), 'exact active module attributes, no path substitution')


def source_shapes(values):
    parser = lexer()
    native, definitions = values[NATIVE], values[DEFINITIONS]
    expected = {
        'segments': 'Vec<Region>', 'current': 'Vec<Option<usize>>', 'pending': 'Vec<Option<usize>>',
        'uses': 'Vec<Vec<Use>>', 'records': 'Vec<Version>', 'nodes': 'Vec<CompletionNodeIdV1>',
        'started': 'Vec<bool>', 'report_records': 'Vec<RuntimeGraphVersionRecordV1>',
        'report_inputs': 'Vec<RuntimeGraphInputVersionV1>',
    }
    need(fields(native, 'VersionLedger', parser) == expected, 'all nine native ledger fields')
    projected = dict(expected, segments='Vec<(A,u64,u64)>', report_records='Vec<R>', report_inputs='Vec<I>')
    need(fields(definitions, 'VersionLedger', parser) == projected, 'complete explicit opaque frame mapping')
    position(tokens(native, parser), tokens('type Region = (RuntimeAllocationIdV1, u64, u64);', parser))
    root_module(values[RUNTIME_ROOT], 'async_engine', parser)
    root_module(values[ASYNC_ROOT], 'graph', parser, (tokens('#[allow(unsafe_code)]', parser),))
    root_module(values[GRAPH_ROOT], 'versions', parser)
    root_module(values[GRAPH_ROOT], 'admitted', parser)
    for name in ('Use', 'Version'):
        need(fields(native, name, parser) == fields(definitions, name, parser), 'exact ' + name + ' fields')
    need(item_body(values[GUARDS], 'enum', 'R65GraphVersionStateV1', parser)
         == item_body(definitions, 'enum', 'RuntimeGraphVersionStateV1', parser), 'exact phase roster')
    position(tokens(values[NODE], parser), ['struct', 'CompletionNodeIdV1', '(', 'u32', ')', ';'])
    position(tokens(definitions, parser), ['struct', 'CompletionNodeIdV1', '(', 'u32', ')', ';'])
    root_tokens = tokens(values[MODEL_ROOT], parser)
    for statement in ('mod r65_graph_versions;', 'pub use r65_graph_versions::*;'):
        at = position(root_tokens, tokens(statement, parser))
        need(at == 0 or root_tokens[at - 1] != ']', 'active model guard module/export')
        need(root_tokens[:at].count('{') == root_tokens[:at].count('}'), 'root guard module/export')
    position(tokens(native, parser), tokens('use fe2o3_runtime_model::{r65_version_begin_write_v1, '
             'r65_version_commit_write_v1, r65_version_input_ready_v1,};', parser))
    position(tokens(native, parser), tokens('pub use fe2o3_runtime_model::R65GraphVersionStateV1 '
                                           'as RuntimeGraphVersionStateV1;', parser))
    position(tokens(native, parser), tokens('macro_rules! graph_version_runtime_expr { '
             '($expression:expr) => { $expression }; }', parser))
    for name in ('r65_version_input_ready_v1', 'r65_version_begin_write_v1', 'r65_version_commit_write_v1'):
        actual = item_body(values[GUARDS], 'fn', name, parser)
        actual = ['RuntimeGraphVersionStateV1' if token == 'R65GraphVersionStateV1' else token for token in actual]
        need(actual == item_body(definitions, 'fn', name, parser), 'actual per-segment executable guard: ' + name)
        params = parameters(values[GUARDS], name, parser)
        params = ['RuntimeGraphVersionStateV1' if token == 'R65GraphVersionStateV1' else token for token in params]
        need(params == parameters(definitions, name, parser), 'exact executable guard arguments: ' + name)
    active_include(native, 'include!("versions/transition_bodies.rs");', parser)
    active_include(values[PROOF], 'include!("graph_version_ledger_definitions_v1.rs");', parser)
    active_include(values[PROOF], 'include!("../../fe2o3-runtime/src/async_engine/graph/versions/transition_bodies.rs");', parser)
    for method in ('begin', 'commit'):
        need(parameters(native, method, parser) == parameters(values[PROOF], method, parser),
             'exact mutable receiver and node index arguments')
        expected_body = tokens('graph_version_' + method + '_body_v1!(graph_version_runtime_expr,self,node,'
                               '(check,write,count),[],[],[],[],[])', parser)
        actual = item_body(native, 'fn', method, parser)
        need(trailing_commas(actual) == trailing_commas(expected_body),
             'only shared transaction in native ' + method)
        code = tokens(values[PROOF], parser)
        position(code, ['graph_version_' + method + '_body_v1', '!', '(', 'verus_exec_expr', ',', 'self', ',', 'node'])
    for path in PROOF_FILES:
        source = parser.code_only(values[path])
        need(not re.search(r'\b(?:assume\w*|admit\w*|axiom|external\w*|unsafe|extern|cfg|cfg_attr)\b', source),
             'no inactive proof or new trust: ' + str(path))
        need(not re.search(r'\b(?:mod|include_str|include_bytes|env|option_env)\b', source), 'closed proof input graph')
        if path != BODY:
            need('macro_rules' not in source, 'no executable macro shadow')
        need(not re.search(r'\b(?:struct|enum|type|trait|mod|as)\s+(?:Vec|Option|Some|None|std|vstd)\b', source),
             'no library or constructor shadow')
        code = tokens(values[path], parser)
        need(code.count('use') == (1 if path == DEFINITIONS else 0), 'closed proof import roster')
        if path == DEFINITIONS:
            position(code, tokens('use vstd::prelude::*;', parser))
    need(tokens(definitions, parser).count('include') == 0, 'definitions have no additional includes')
    need(tokens(values[PROOF], parser).count('include') == 2, 'exact proof includes')
    need(tokens(values[BODY], parser).count('include') == 0, 'body has no additional includes')
    for method in ('begin', 'commit', 'fail'):
        source, name = (native, method) if method == 'fail' else (values[CPU_TEST], 'predecessor_' + method)
        body = item_body(source, 'fn', name, parser)
        if method != 'fail':
            body = ['self' if token == 'ledger' else token for token in body]
        digest = sha(json.dumps(body, separators=(',', ':')).encode())
        need(digest == PREDECESSOR_BODIES[method], 'unchanged predecessor executable body: ' + method)
    # This is an exact call-order binding only, not a proof of either native
    # settlement or CompletionAuthority's implementation.
    successor = item_body(values[ADMITTED], 'fn', 'succeed_host_staging', parser)
    commit = position(successor, ['self', '.', 'versions', '.', 'commit', '(', 'index', ')'])
    ready = position(successor, ['self', '.', 'authority', '.', 'mark_succeeded'])
    need(commit < ready and 'return' in successor[commit:ready], 'commit refusal precedes successor authority')
    staging = item_body(values[STAGING], 'fn', 'try_stage_graph_host_write_v1', parser)
    written = position(staging, ['self', '.', 'context', '.', 'write_host_visible_with_graph_access_v1'])
    committed = position(staging, ['core', '.', 'succeed_host_staging'])
    arm = tokens('Ok(()) => {', parser)
    need(written < committed and arm in [staging[i:i + len(arm)] for i in range(written, committed)],
         'actual successful host write before graph commit')


def snapshot():
    return {path: ordinary(ROOT / path).decode('ascii') for path in FILES}


def manifest(values):
    return {str(path): sha(text.encode('ascii')) for path, text in values.items()}


def audit(values):
    need(set(values) == set(FILES), 'exact source and proof roster')
    raw = ordinary(MANIFEST)
    need(sha(raw) == ordinary(MANIFEST_PIN).decode().strip(), 'exact input manifest pin')
    record = unique_json(raw)
    need(set(record) == {'scope', 'predecessor', 'files'} and record['predecessor'] == PREDECESSOR
         and record['scope'] == 'conditional shared begin/commit only; no native settlement', 'closed manifest schema')
    need(record['files'] == manifest(values), 'reviewed exact input bytes')
    source_shapes(values)
    return record


if __name__ == '__main__':
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python3 -I -B required')
    audit(snapshot())
    print('PASS: exact executable graph-version ledger source closure (source only)')
