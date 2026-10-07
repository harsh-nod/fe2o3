#!/usr/bin/env python3
"""Source calibration for private byte primitives, not complete wire refinement."""
import hashlib
import json
from pathlib import Path
import re
import runpy
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path('crates/fe2o3-runtime-model/verus')
SRC = Path('crates/fe2o3-runtime-model/src')
OWNER = SRC / 'distributed_publication_contract.rs'
NATIVE = SRC / 'distributed_publication_contract/codec_primitives.rs'
BODY = SRC / 'distributed_publication_contract/codec_primitives_body.rs'
TESTS = SRC / 'distributed_publication_contract/codec_tests.rs'
FIELD_NATIVE = SRC / 'distributed_publication_contract/codec_fields.rs'
FIELD_BODY = SRC / 'distributed_publication_contract/codec_fields_body.rs'
PROOF = V / 'distributed_codec_primitives_v1.rs'
CONSTRUCTION = V / 'distributed_publication_construction_v1.rs'
CLASSIFIER = V / 'distributed_publication_contract_v1.rs'
DECLARATIONS = SRC / 'distributed_publication_contract/declarations.rs'
CONSTRUCTION_BODY = SRC / 'distributed_publication_contract/construction_body.rs'
CLASSIFIER_BODY = SRC / 'distributed_publication_contract/classifier_body.rs'
IDENTITY = SRC / 'identity.rs'
BRIDGE = V / 'check-distributed-publication-contract.py'
BRIDGE_SHA = 'e29345c409f6cc4e47a85a9e52e8581fefb5b4575feaea71e6726834c182dc48'
FILES = [PROOF, CONSTRUCTION, CLASSIFIER, DECLARATIONS, CONSTRUCTION_BODY, CLASSIFIER_BODY, BODY]
SOURCE_FILES = 308
SOURCE_TREE_SHA = '641e9e3c94274ba46410d9312e3259bde6999387c9eca3f4963394f1b31e7d73'
PROOF_SHA = '2fac9dd32338e9e613bcd918571f893f1a1cc18228dc39aca14bad8bf92ef9f6'
BODY_SHA = '222b9a154402bfad72a745ee6900fe29aead0b8f3e00db9a62d01e7498188f5c'
FIXTURES = V / 'fixtures/distributed_codec_primitives_v1_diagnostics.json'
FIXTURES_SHA = '76b5f83c062142a5e5f4f887dc1dd188dad5bc88b2ec11acfefb2bd49de5bf40'
UNCHANGED = {
    CONSTRUCTION: 'b0955242b7c9e8868ba917be3145451d8e26af0ddf3a49ddf7379b5155d24790',
    CLASSIFIER: '57edf7747e4c86c6e73b0311e59e06b7fb2dbf276fba5988e526dfd4fcacd853',
    DECLARATIONS: '39a5e23849a75df9af224b2a4193cd037b5bb39cbddc85c3de07ada299015363',
    CONSTRUCTION_BODY: '0057a0902bd3f8731affed1f51f8ea6296135a38588828515d5fe57149ea437a',
    CLASSIFIER_BODY: 'a9523b717e47851e50163e41a9cc4ff2ec31b10e814a90521644aaaa0158f18d',
    IDENTITY: '4df48a80f5bff5ff82481358ee320819de5afc38eee8e7520c50732326fc3564',
}
# Measured whole-crate discovery includes inherited units and fixed-array generics.
EXPECTED_VERIFIED = 54
MULTIPLE_ERRORS = 1
KINDS = ('take', 'fixed', 'put', 'finish', 'u16_le', 'u16_from_le', 'u64_le', 'u64_from_le')
SELECTORS = {kind: '*' + kind for kind in KINDS}
MACROS = {kind: 'distributed_codec_' + kind + '_body_v1' for kind in KINDS}
MUTANT_COUNT = 31
ROOT_NOTE = 'verifying root module (selected functions)'
ENUM_NOTE = ('function body check: not all errors may have been reported; rerun with a higher value for '
             '--multiple-errors to find other potential errors in this function')
RANGE_NOTE = ('recommendation not met: value may be out of range of the target type '
              '(use `#[verifier::truncate]` on the cast to silence this warning)')
POSTCONDITION = 'postcondition not satisfied'
SELECTION_NOTES = {focus: frozenset((ROOT_NOTE,)) for focus in SELECTORS.values()}
NEGATIVE_RESULT = {'encountered-error': True, 'encountered-vir-error': False, 'errors': 1,
                   'is-verifying-entire-crate': False, 'verified': 0}
SIGNATURE_SPANS = {'take': (659, 757), 'fixed': (1306, 1395), 'put': (1867, 1925),
                   'finish': (2524, 2588), 'u16_le': (3432, 3473), 'u16_from_le': (3583, 3629),
                   'u64_le': (3743, 3784), 'u64_from_le': (3894, 3940)}


def need(value, message):
    if not value:
        raise ValueError(message)


def invocation_guard(flags=None):
    flags = sys.flags if flags is None else flags
    need(getattr(flags, 'isolated', 0) and getattr(flags, 'dont_write_bytecode', 0)
         and not getattr(flags, 'optimize', 1), 'use isolated python3 -I -B without optimization')


invocation_guard()


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def tree_hash(sources):
    return hashlib.sha256(json.dumps({str(p): sha(text) for p, text in sources.items()},
                                     sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def bridge():
    data = (ROOT / BRIDGE).read_bytes()
    need(hashlib.sha256(data).hexdigest() == BRIDGE_SHA, 'unchanged reviewed controller helper bytes')
    module = types.ModuleType('distributed_codec_helpers')
    module.__file__ = str(ROOT / BRIDGE)
    sys.modules[module.__name__] = module
    exec(compile(data, module.__file__, 'exec'), module.__dict__)
    return module


def snapshot():
    names = {PROOF, CONSTRUCTION, CLASSIFIER} | {p.relative_to(ROOT) for p in (ROOT / SRC).rglob('*.rs')}
    result = {}
    for p in names:
        selected = ROOT / p
        need(selected.is_file() and not selected.is_symlink() and selected.resolve() == selected,
             'ordinary exact source file')
        result[p] = selected.read_bytes().decode()
    return result


def field_forwarding(sources):
    owner, native, body = (re.sub(r'\s+', '', sources[path])
                           for path in (OWNER, FIELD_NATIVE, FIELD_BODY))
    forwarders = (
        'fnheader(&mutself,domain:&[u8]){codec_fields::write_header(self.bytes,&mutself.offset,domain);}',
        'fnu64(&mutself,value:u64){codec_fields::write_u64(self.bytes,&mutself.offset,value);}',
        'fnheader(&mutself,domain:&[u8])->Result<(),DistributedPublicationContractErrorV1>{codec_fields::read_header(self.bytes,&mutself.offset,domain)}',
        'fnu64(&mutself)->Result<u64,DistributedPublicationContractErrorV1>{codec_fields::read_u64(self.bytes,&mutself.offset)}',
    )
    need(owner.count('modcodec_fields;') == 1 and all(owner.count(call) == 1 for call in forwarders),
         'actual Reader/Writer field methods forward to the private field helpers')
    need(native.count('usesuper::codec_primitives::{fixed,put,take,u16_from_le,u16_le,u64_from_le,u64_le};') == 1
         and native.count('include!("codec_fields_body.rs");') == 1,
         'field helpers use the real primitive functions and shared field bodies')
    calls = {
        'read_header': ('bytes,offset,domain', 'u16_from_le(schema)'),
        'write_header': ('bytes,offset,domain', 'put($bytes,$offset,&u16_le(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1),);'),
        'read_u64': ('bytes,offset', 'Ok(u64_from_le(value))'),
        'write_u64': ('bytes,offset,value', 'put($bytes,$offset,&u64_le($value));'),
    }
    signatures = {
        'read_header': 'pub(super)fnread_header(bytes:&[u8],offset:&mutusize,domain:&[u8],)->Result<(),DistributedPublicationContractErrorV1>',
        'write_header': 'pub(super)fnwrite_header(bytes:&mut[u8],offset:&mutusize,domain:&[u8])',
        'read_u64': 'pub(super)fnread_u64(bytes:&[u8],offset:&mutusize,)->Result<u64,DistributedPublicationContractErrorV1>',
        'write_u64': 'pub(super)fnwrite_u64(bytes:&mut[u8],offset:&mutusize,value:u64)',
    }
    for kind, (arguments, primitive) in calls.items():
        macro = 'distributed_codec_' + kind + '_body_v1'
        invocation = macro + '!(distributed_codec_fields_expr_v1,' + arguments + ')'
        marker = 'macro_rules!' + macro + '{'
        need(native.count(signatures[kind] + '{' + invocation + '}') == 1
             and native.count(invocation) == 1
             and body.count(marker) == 1, 'one actual shared field body per native helper')
        start = body.index(marker)
        end = body.find('macro_rules!', start + len(marker))
        region = body[start:] if end < 0 else body[start:end]
        need(region.count(primitive) == 1, 'endian primitive remains in its matching field operation')


def audit(sources):
    implementation = {p: text for p, text in sources.items() if p.is_relative_to(SRC)}
    need(set(sources) == set(implementation) | {PROOF, CONSTRUCTION, CLASSIFIER}
         and len(implementation) == SOURCE_FILES and tree_hash(implementation) == SOURCE_TREE_SHA,
         'reviewed complete model source roster')
    need(sha(sources[PROOF]) == PROOF_SHA
         and all(sha(sources[p]) == digest for p, digest in UNCHANGED.items()),
         'exact candidate proof and unchanged earlier executable closure')
    owner, native, body, proof = (sources[p] for p in (OWNER, NATIVE, BODY, PROOF))
    compact = re.sub(r'\s+', '', owner)
    need(owner.count('mod codec_primitives;') == 1 and owner.count('mod codec_tests;') == 1,
         'actual private helper and native regression modules')
    forwarders = (
        'codec_primitives::put(self.bytes,&mutself.offset,value);',
        'codec_primitives::take(self.bytes,&mutself.offset,count)',
        'codec_primitives::fixed(self.bytes,&mutself.offset)',
        'codec_primitives::finish(self.bytes,self.offset)',
    )
    need(all(compact.count(value) == 1 for value in forwarders), 'real Reader/Writer primitive forwarding')
    field_forwarding(sources)
    need(native.count('include!("codec_primitives_body.rs");') == 1
         and proof.count('include!("distributed_publication_construction_v1.rs");') == 1
         and proof.count('include!("../src/distributed_publication_contract/codec_primitives_body.rs");') == 1
         and len(re.findall(r'\binclude!\(', proof)) == 2
         and not re.search(r'\binclude!\(', body), 'closed seven-file source-shared proof input')
    for macro in MACROS.values():
        need(native.count(macro + '!') == proof.count(macro + '!') == 1
             and body.count('macro_rules! ' + macro + ' {') == 1, 'same eight actual primitive bodies')
    stripped = re.sub(r'//[^\n]*', '', proof)
    need(not re.search(r'\b(?:assume|admit|assume_specification|uninterp)\b|verifier::external|#\!?\[allow', stripped),
         'no new trusted bodies, assumptions or diagnostic allowances')
    requirement = '''requires
        *old(offset) <= old(bytes)@.len(),
        value@.len() <= old(bytes)@.len() - *old(offset) as int,'''
    need(len(re.findall(r'\brequires\b', stripped)) == 1 and proof.count(requirement) == 1
         and proof.index('fn put(') < proof.index(requirement) < proof.index('fn finish('),
         'only sufficient Writer capacity is a premise; Reader remains total')
    need(not re.search(r'\b(?:Vec|Box|unsafe|alloc)\b|std::', body + native),
         'fixed storage, safe Rust and no_std primitive implementation')
    need(sources[TESTS].count('#[test]') == 9, 'direct boundary/frame/panic/endian/header CPU tests')
    need(hashlib.sha256(ordinary_bytes(ROOT / FIXTURES)).hexdigest() == FIXTURES_SHA,
         'exact portable replay fixtures, never fresh solver evidence')
    bridge()


def mutations(body):
    inherited = bridge()
    rows = [
        ('take-cursor-omitted', 'take', '*$offset = end;', 'let _ = end;'),
        ('take-full-range-refused', 'take', 'if end > $bytes.len() {', 'if end >= $bytes.len() {'),
        ('take-prefix-substituted', 'take', 'let value = &$bytes[start..end];', 'let value = &$bytes[0..$count];'),
        ('take-failure-cursor-reset', 'take',
         'if end > $bytes.len() {\n                return Err',
         'if end > $bytes.len() {\n                *$offset = 0;\n                return Err'),
        ('take-overflow-cursor-reset', 'take',
         'None => return Err(DistributedPublicationContractErrorV1::WrongLength),',
         'None => { *$offset = 0; return Err(DistributedPublicationContractErrorV1::WrongLength); },'),
        ('take-overflow-error-substituted', 'take',
         'None => return Err(DistributedPublicationContractErrorV1::WrongLength),',
         'None => return Err(DistributedPublicationContractErrorV1::WrongSchema),'),
        ('fixed-payload-omitted', 'fixed',
         'let mut result = [0u8; $size];\n            result.copy_from_slice(value);',
         'let result = [0u8; $size];\n            let _ = value;'),
        ('put-cursor-omitted', 'put', '*$offset = end;', 'let _ = end;'),
        ('put-frame-prefix-substituted', 'put',
         'let (_, tail) = $bytes.split_at_mut(start);', 'let (_, tail) = $bytes.split_at_mut(0);'),
        ('finish-prefix-accepted', 'finish', 'if $offset == $bytes.len() {', 'if $offset <= $bytes.len() {'),
        ('finish-end-refused', 'finish', 'if $offset == $bytes.len() {', 'if $offset < $bytes.len() {'),
        ('u16-encode-low-substituted', 'u16_le', '[$value as u8,', '[($value >> 8) as u8,'),
        ('u16-encode-high-substituted', 'u16_le', '($value >> 8) as u8]', '($value >> 0) as u8]'),
    ]
    for index in range(2):
        rows.append((f'u16-decode-byte-{index}-substituted', 'u16_from_le',
                     f'$bytes[{index}]', f'$bytes[{1 - index}]'))
    for index in range(8):
        before = '$value as u8,' if index == 0 else f'($value >> {8 * index}) as u8,'
        after = '($value >> 8) as u8,' if index == 0 else f'($value >> {8 * (index - 1)}) as u8,'
        rows.append((f'u64-encode-byte-{index}-substituted', 'u64_le', before, after))
        rows.append((f'u64-decode-byte-{index}-substituted', 'u64_from_le',
                     f'$bytes[{index}]', f'$bytes[{(index + 1) % 8}]'))
    result = {name: (inherited.change(body, MACROS[kind], before, after), SELECTORS[kind])
              for name, kind, before, after in rows}
    need(len(result) == len(rows) == len(set(result.values())) == MUTANT_COUNT
         and all(value != body for value, _ in result.values()), '31 distinct actual-body mutations')
    return result


def selection_notes(leaf, focus=None):
    need(focus in SELECTORS.values() and isinstance(SELECTION_NOTES, dict)
         and set(SELECTION_NOTES) == set(SELECTORS.values()), 'exact measured selector inventory required')
    notes = SELECTION_NOTES[focus]
    need(isinstance(notes, frozenset) and notes == frozenset(('verifying root module (selected functions)',)),
         'exact measured singleton selection note required')
    return types.SimpleNamespace(LOGICAL_ERRORS=leaf.LOGICAL_ERRORS, SELECTION_NOTES=notes, CODEC_SELECTOR=focus)


def ordinary_bytes(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         'ordinary canonical diagnostic source')
    return path.read_bytes()


def item_region(data, pattern):
    starts = list(re.finditer(pattern, data, re.MULTILINE))
    need(data.isascii() and len(starts) == 1, 'one exact ASCII source-family item')
    return starts[0].start(), data.index(b'\n}\n', starts[0].end()) + 3


def negative_context(root, focus):
    need(root.is_absolute() and root.resolve() == root and root.is_dir() and not root.is_symlink()
         and focus in SELECTORS.values(), 'canonical mutant root and primitive selector')
    paths = list(root.rglob('*'))
    need(all(not p.is_symlink() and p.resolve() == p for p in paths)
         and {p.relative_to(root) for p in paths if p.is_file()} == set(FILES), 'exact seven-file mutant tree')
    trusted = {PROOF: PROOF_SHA, BODY: BODY_SHA, **{p: UNCHANGED[p] for p in FILES if p in UNCHANGED}}
    original = {p: ordinary_bytes(ROOT / p) for p in FILES}
    need(set(trusted) == set(FILES)
         and all(hashlib.sha256(data).hexdigest() == trusted[p] for p, data in original.items()),
         'unchanged reviewed proof closure and actual original body')
    observed = {p: ordinary_bytes(root / p) for p in FILES}
    need(all(observed[p] == original[p] for p in FILES if p != BODY), 'only the actual shared body may differ')
    matches = [name for name, (data, selector) in mutations(original[BODY].decode()).items()
               if data.encode() == observed[BODY] and selector == focus]
    need(len(matches) == 1, 'one known non-equivalent actual-body mutation in the selected family')
    kind = next(kind for kind in KINDS if SELECTORS[kind] == focus)
    proof, body = observed[PROOF], observed[BODY]
    function = item_region(proof, rb'^fn ' + kind.encode() + rb'[<(]')
    macro = item_region(body, rb'^macro_rules! ' + MACROS[kind].encode() + rb' \{')
    body_start = proof.index(b'\n{\n', function[0]) + 1
    ensures = proof.index(b'\n    ensures', function[0]) + len(b'\n    ensures')
    headers = [proof.find(marker, function[0], body_start) for marker in (b'\n    requires', b'\n    ensures')]
    signature_end = min(index for index in headers if index >= 0)
    # The measured Verus span excludes a named return binder's closing parenthesis.
    if b'-> (result:' in proof[function[0]:signature_end]:
        need(proof[signature_end - 1:signature_end] == b')', 'exact named return binder')
        signature_end -= 1
    need(SIGNATURE_SPANS[kind] == (function[0], signature_end), 'exact measured declaration span')
    call = (MACROS[kind] + '!(').encode()
    need(proof[function[0]:function[1]].count(call) == 1, 'one exact selected proof invocation')
    invocation_start = proof.index(call, function[0], function[1])
    invocation_end = proof.index(b'\n', invocation_start)
    argument = None
    if kind in ('u16_le', 'u64_le'):
        need(proof[invocation_start:invocation_end] == call + b'verus_exec_expr, value)',
             'exact selected encoder invocation with final value argument')
        argument = (invocation_end - 1 - len(b'value'), invocation_end - 1)
        need(proof[argument[0]:argument[1]] == b'value', 'exact final encoder argument interval')
    syntax = body[macro[0]:macro[1]]
    need(syntax.count(b'$syntax!({') == syntax.count(b'})') == 1, 'one shared executable expression frame')
    expression = (macro[0] + syntax.index(b'$syntax!({') + len(b'$syntax!('),
                  macro[0] + syntax.index(b'})') + 1)
    need(function[0] < ensures < body_start < function[1], 'actual postcondition and body regions')
    return {'root': root, 'focus': focus, 'kind': kind, 'case': matches[0], 'source': observed,
            'function': function, 'contract': (ensures, body_start), 'body': (body_start, function[1]),
            'signature': (function[0], signature_end), 'macro': macro, 'expression': expression,
            'definition': (macro[0], macro[0] + len(('macro_rules! ' + MACROS[kind]).encode())),
            'invocation': (invocation_start, invocation_end), 'argument': argument}


def contained(inner, outer):
    return outer[0] <= inner[0] < inner[1] <= outer[1]


def source_span(span, context):
    fields = {'byte_end', 'byte_start', 'column_end', 'column_start', 'expansion', 'file_name',
              'is_primary', 'label', 'line_end', 'line_start', 'suggested_replacement',
              'suggestion_applicability', 'text'}
    need(type(span) is dict and set(span) == fields and type(span['is_primary']) is bool
         and (span['label'] is None or type(span['label']) is str)
         and span['suggested_replacement'] is None and span['suggestion_applicability'] is None,
         'exact measured source-span schema')
    root = context['root']
    names = {str(root / PROOF): PROOF,
             str(root / PROOF.parent / '../src/distributed_publication_contract/codec_primitives_body.rs'): BODY}
    need(type(span['file_name']) is str and span['file_name'] in names
         and Path(span['file_name']).resolve() == root / names[span['file_name']], 'exact measured source spelling')
    path = names[span['file_name']]
    data = context['source'][path]
    need(data.isascii(), 'ASCII source-coordinate calibration')
    lines = data.splitlines(keepends=True)
    coordinates = ('line_start', 'line_end', 'column_start', 'column_end', 'byte_start', 'byte_end')
    need(all(type(span[name]) is int for name in coordinates)
         and 1 <= span['line_start'] <= span['line_end'] <= len(lines)
         and 0 <= span['byte_start'] < span['byte_end'] <= len(data), 'bounded integer source coordinates')
    start_line = lines[span['line_start'] - 1].rstrip(b'\r\n')
    end_line = lines[span['line_end'] - 1].rstrip(b'\r\n')
    need(1 <= span['column_start'] <= len(start_line) + 1
         and 1 <= span['column_end'] <= len(end_line) + 1, 'bounded source columns')
    start = sum(map(len, lines[:span['line_start'] - 1])) + span['column_start'] - 1
    end = sum(map(len, lines[:span['line_end'] - 1])) + span['column_end'] - 1
    need((start, end) == (span['byte_start'], span['byte_end'])
         and type(span['text']) is list and len(span['text']) == span['line_end'] - span['line_start'] + 1,
         'exact byte/line/column correspondence and complete text')
    for index, row in enumerate(span['text'], span['line_start']):
        text = lines[index - 1].rstrip(b'\r\n').decode()
        highlight = (span['column_start'] if index == span['line_start'] else 1,
                     span['column_end'] if index == span['line_end'] else len(text) + 1)
        need(type(row) is dict and set(row) == {'text', 'highlight_start', 'highlight_end'}
             and row['text'] == text and type(row['highlight_start']) is int and type(row['highlight_end']) is int
             and (row['highlight_start'], row['highlight_end']) == highlight,
             'exact authenticated source text and highlighted region')
    return path, (start, end)


def family_span(span, context, proof_region=None):
    path, region = source_span(span, context)
    if path == PROOF:
        need(span['expansion'] is None and contained(region, context['function'] if proof_region is None else proof_region),
             'proof span belongs to the selected function region')
    else:
        need(proof_region is None and contained(region, context['expression']), 'shared span belongs to the selected expression')
        expansion = span['expansion']
        need(type(expansion) is dict and set(expansion) == {'span', 'macro_decl_name', 'def_site_span'}
             and expansion['macro_decl_name'] == MACROS[context['kind']] + '!', 'exact one-level selected expansion')
        for name, expected_path, expected_region in (
            ('span', PROOF, context['invocation']), ('def_site_span', BODY, context['definition'])):
            nested = expansion[name]
            nested_path, nested_region = source_span(nested, context)
            need(nested_path == expected_path and nested_region == expected_region
                 and nested['expansion'] is None and nested['is_primary'] is False and nested['label'] is None,
                 'exact selected macro definition and proof invocation; no deeper expansion')
    return path, region


def bounded_negative(classifier, root, focus, status, stdout, stderr, verifier):
    try:
        need(type(status) is int and status == 1, 'actual failing verifier exit, not signal/timeout/Boolean')
        context = negative_context(root, focus)

        def nonfinite(_value):
            raise ValueError('nonfinite JSON')

        def decoded(text):
            return json.loads(text, object_pairs_hook=classifier.strict_object, parse_constant=nonfinite)

        result = decoded(stdout)
        need(type(result) is dict
             and json.dumps(result.get('verus'), sort_keys=True) == json.dumps(verifier, sort_keys=True)
             and json.dumps(result.get('verification-results'), sort_keys=True) == json.dumps(NEGATIVE_RESULT, sort_keys=True),
             'exact pinned verifier and selected zero/one result')
        rows = [decoded(line) for line in stderr.splitlines()]
        for row in rows:
            need(type(row) is dict and set(row) == {'$message_type', 'children', 'code', 'level', 'message', 'rendered', 'spans'}
                 and row['$message_type'] == 'diagnostic' and row['children'] == [] and row['code'] is None
                 and row['level'] in ('note', 'error') and type(row['message']) is str
                 and type(row['rendered']) is str and type(row['spans']) is list, 'exact raw diagnostic schema')
        logical = [row for row in rows if row['level'] == 'error' and row['message'] == POSTCONDITION]
        aborts = [row for row in rows if row['level'] == 'error' and row['message'] == 'aborting due to 1 previous error']
        roots = [row for row in rows if row['level'] == 'note' and row['message'] == ROOT_NOTE]
        enumerations = [row for row in rows if row['level'] == 'note' and row['message'] == ENUM_NOTE]
        ranges = [row for row in rows if row['level'] == 'note' and row['message'] == RANGE_NOTE]
        range_count = 2 if context['kind'] in ('u16_le', 'u64_le') else 0
        need(len(logical) == len(aborts) == len(roots) == len(enumerations) == 1
             and len(ranges) == range_count and len(rows) == 4 + range_count
             and aborts[0]['spans'] == roots[0]['spans'] == [], 'exact measured diagnostic classes and multiplicities')
        spans = logical[0]['spans']
        need(len(spans) == 2 and all(type(span) is dict for span in spans),
             'one genuine logical primary and one source-bound exit span')
        primary = [span for span in spans if span.get('is_primary') is True]
        secondary = [span for span in spans if span.get('is_primary') is False]
        need(len(primary) == len(secondary) == 1 and primary[0]['label'] == 'failed this postcondition',
             'actual postcondition primary required; auxiliary notes are never failures')
        family_span(primary[0], context, context['contract'])
        path, region = family_span(secondary[0], context)
        need(path == BODY or contained(region, context['body']), 'secondary exit span stays in selected executable body')
        enumeration = enumerations[0]['spans']
        need(len(enumeration) == 1 and enumeration[0]['is_primary'] is True and enumeration[0]['label'] is None,
             'one actual enumeration signature span')
        path, region = family_span(enumeration[0], context, context['signature'])
        need(path == PROOF and region == context['signature'], 'exact selected function signature')
        observed_ranges = []
        for row in ranges:
            need(len(row['spans']) == 1 and row['spans'][0]['is_primary'] is True and row['spans'][0]['label'] is None,
                 'one primary per bounded encoding recommendation')
            path, region = family_span(row['spans'][0], context)
            need(path == BODY or (path == PROOF and region == context['argument']),
                 'recommendation belongs to selected encoding macro or its exact final value argument')
            observed_ranges.append((path, region))
        need(len(observed_ranges) == len(set(observed_ranges)), 'distinct recommendation spans, not duplicated notes')
        notes = types.SimpleNamespace(LOGICAL_ERRORS={POSTCONDITION},
            SELECTION_NOTES={ROOT_NOTE} | ({RANGE_NOTE} if range_count else set()))
        return classifier.logical_negative(notes, status, stdout, stderr, verifier,
                                           {str(root / path) for path in FILES})
    except (ValueError, TypeError, KeyError, IndexError, OSError, UnicodeError):
        return False


def primitive_classifier(classifier):
    proxy = types.ModuleType('distributed_codec_bounded_classifier')
    proxy.__dict__.update(vars(classifier))

    def negative(notes, status, stdout, stderr, verifier, paths):
        try:
            matches = [Path(path) for path in paths if path.endswith('/' + str(PROOF))]
            need(len(matches) == 1, 'one selected proof input')
            root = matches[0]
            for _ in PROOF.parts:
                root = root.parent
            need(paths == {str(root / path) for path in FILES}, 'exact selected proof input set')
            return bounded_negative(classifier, root, notes.CODEC_SELECTOR, status, stdout, stderr, verifier)
        except (ValueError, TypeError, AttributeError):
            return False

    def positive(status, stdout, stderr, verifier, expected, paths):
        return type(status) is int and status == 0 and not stderr and classifier.proof_positive(
            status, stdout, stderr, verifier, expected, paths)

    proxy.logical_negative, proxy.proof_positive = negative, positive
    return proxy


def reporting_source(data):
    inherited = bridge()
    need(hashlib.sha256(data).hexdigest() == inherited.BASE_SHA,
         'exact unchanged inherited campaign source before reporting adaptation')
    need(type(MULTIPLE_ERRORS) is int and MULTIPLE_ERRORS == 1,
         'exact measured error-enumeration reporting level')
    before = b'"--multiple-errors", "0",'
    after = b'"--multiple-errors", "1",'
    need(data.count(before) == 1 and data.count(after) == 0,
         'single inherited proof argv reporting site')
    return data.replace(before, after)


def campaign_controller():
    inherited = bridge()
    path = ROOT / inherited.BASE
    need(path.is_file() and path.resolve() == path and not path.is_symlink(),
         'ordinary inherited campaign source')
    module = types.ModuleType('distributed_codec_primitive_campaign')
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(reporting_source(path.read_bytes()), str(path), 'exec'), module.__dict__)
    original_loader = module.inherited
    module.inherited = lambda: primitive_classifier(original_loader())
    return module


def campaign():
    audit(snapshot())
    need(type(EXPECTED_VERIFIED) is int and EXPECTED_VERIFIED == 54,
         'exact measured whole-crate positive count')
    for focus in SELECTORS.values():
        selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), focus)
    module = campaign_controller()
    module.FILES, module.PROOF, module.BODY = FILES, PROOF, BODY
    module.EXPECTED = dict(module.EXPECTED, verified=EXPECTED_VERIFIED)
    module.mutations, module.selection_notes = mutations, selection_notes
    return module


if __name__ == '__main__':
    runpy.run_path(str(ROOT / V / 'test-distributed-codec-primitives.py'))
    campaign().main()
