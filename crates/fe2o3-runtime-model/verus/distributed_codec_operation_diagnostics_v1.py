#!/usr/bin/env python3
"""Source-bound full-root diagnostics; replay never qualifies a mutation kill."""
import copy
import hashlib
import json
from pathlib import Path
import types

FIELD_PATH = Path(__file__).resolve().with_name('distributed_codec_fields_diagnostics_v1.py')
FIELD_SHA = 'c9b364de00897bcfade87face6257f02aed50f0f58a015ae9085ab4e66738f7d'
FIELD_BYTES = FIELD_PATH.read_bytes()
if FIELD_PATH.is_symlink() or FIELD_PATH.resolve() != FIELD_PATH or hashlib.sha256(FIELD_BYTES).hexdigest() != FIELD_SHA:
    raise ValueError('exact reviewed field source-span helper')
FIELD = types.ModuleType('operation_field_span_source')
FIELD.__file__ = str(FIELD_PATH)
exec(compile(FIELD_BYTES, str(FIELD_PATH), 'exec'), FIELD.__dict__)
need, decoded = FIELD.need, FIELD.decoded
ENUMERATION = 'function body check: not all errors may have been reported; rerun with a higher value for --multiple-errors to find other potential errors in this function'
SELECTORS = {'write_digest': 'write_digest', 'read_digest': 'read_digest',
             'encode_operation': 'encode', 'decode_operation': 'decode'}
LOGICAL = frozenset(('postcondition not satisfied', 'precondition not satisfied', 'assertion failed'))


def context(sources, proof, body, family):
    need(family in SELECTORS and len(sources) == 11 and proof in sources and body in sources,
         'known operation family and full eleven-input closure')
    data = sources[proof]
    function = ('fn ' + SELECTORS[family] + '(').encode()
    need(data.count(function) == 1, 'unique actual selected function')
    start = data.index(function)
    end = data.index(b'\n}\n', start) + 3
    contract = min(at for at in (data.find(b'\n    requires', start, end),
                                data.find(b'\n    ensures', start, end)) if at >= 0)
    header_end = contract
    if b'-> (result: ' in data[start:contract]:
        need(data[contract - 1:contract] == b')', 'exact named-return binder ending')
        header_end -= 1
    macro = ('distributed_codec_' + family + '_body_v1').encode()
    declaration = b'macro_rules! ' + macro
    need(sources[body].count(declaration + b' {') == 1, 'unique actual shared macro')
    macro_start = sources[body].index(declaration + b' {')
    next_macro = sources[body].find(b'\nmacro_rules! ', macro_start + 1)
    macro_end = len(sources[body]) if next_macro < 0 else next_macro
    return {'proof': proof, 'body': body, 'family': family, 'macro': macro.decode() + '!',
            'regions': {proof: (start, end), body: (macro_start, macro_end)},
            'header': (start, header_end), 'declaration': (macro_start, macro_start + len(declaration))}


def location(span, root):
    path = Path(span['file_name']).resolve()
    need(path.is_relative_to(root), 'span remains inside this exact projection')
    return str(path.relative_to(root)), span['byte_start'], span['byte_end']


def inside(span, root, regions):
    path, start, end = location(span, root)
    return path in regions and regions[path][0] <= start < end <= regions[path][1]


def macro_frame(span, root, sources, ctx):
    expansion = span['expansion']
    if expansion is None:
        return
    need(expansion['macro_decl_name'] == ctx['macro'], 'exact selected native shared macro frame')
    invocation, declaration = expansion['span'], expansion['def_site_span']
    for child in (invocation, declaration):
        need(child['is_primary'] is False and child['label'] is None and child['expansion'] is None,
             'ordinary nonprimary macro invocation/declaration without deeper frames')
    path, start, end = location(invocation, root)
    need(path == ctx['proof'] and inside(invocation, root, ctx['regions'])
         and sources[path][start:end].startswith((ctx['macro'] + '(').encode())
         and sources[path][start:end].endswith(b')'), 'exact actual invocation in the selected proof function')
    need(location(declaration, root) == (ctx['body'], *ctx['declaration']),
         'exact selected shared macro declaration')


def question_exit(span, root, sources, ctx, case_name, message):
    need(case_name == 'decode_extra_length' and ctx['family'] == 'decode_operation'
         and message == 'postcondition not satisfied' and span['is_primary'] is False
         and span['label'] == 'at this exit', 'one known nonprimary question-mark exit only')
    start, end = ctx['regions'][ctx['body']]
    line = b'read_header($bytes, &mut offset, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1)?;'
    at = sources[ctx['body']].index(line, start, end) + len(line) - 2
    need(location(span, root) == (ctx['body'], at, at + 1), 'actual header question-mark source byte')
    expansion = span['expansion']
    need(type(expansion) is dict and set(expansion) == {'span', 'macro_decl_name', 'def_site_span'}
         and expansion['macro_decl_name'] == 'desugaring of operator `?`', 'exact compiler question-mark frame')
    invocation = expansion['span']
    need(type(invocation) is dict, 'ordinary source invocation span')
    expected_invocation = dict(span, label=None, expansion=invocation.get('expansion'))
    need(invocation == expected_invocation, 'same source question mark, null auxiliary label')
    placeholder = dict(file_name=str(root / ctx['proof']), byte_start=0, byte_end=0,
        line_start=1, line_end=1, column_start=1, column_end=1, is_primary=False, text=[],
        label=None, suggested_replacement=None, suggestion_applicability=None, expansion=None)
    need(expansion['def_site_span'] == placeholder
         and all(type(expansion['def_site_span'][key]) is int for key in
                 ('byte_start', 'byte_end', 'line_start', 'line_end', 'column_start', 'column_end'))
         and type(expansion['def_site_span']['is_primary']) is bool,
         'exact auxiliary compiler def-site sentinel, never a primary or invocation')
    bare = dict(span, expansion=None)
    observed = FIELD.span_source(bare, root, sources)
    observed.extend(FIELD.span_source(invocation, root, sources))
    need(invocation['expansion'] is not None, 'actual native decode macro ancestry')
    macro_frame(invocation, root, sources, ctx)
    return observed


def source_span(span, root, sources, ctx, case_name, message):
    if type(span) is dict and type(span.get('expansion')) is dict \
            and span['expansion'].get('macro_decl_name') == 'desugaring of operator `?`':
        return question_exit(span, root, sources, ctx, case_name, message)
    observed = FIELD.span_source(span, root, sources)
    macro_frame(span, root, sources, ctx)
    return observed


def normalized_span(span, root):
    result = copy.deepcopy(span)
    result['file_name'] = str(Path(span['file_name']).resolve().relative_to(root))
    if span['expansion'] is not None:
        for name in ('span', 'def_site_span'):
            result['expansion'][name] = normalized_span(span['expansion'][name], root)
    return result


def recommendation(row, root, sources, ctx, case_name):
    need(case_name == 'digest_read_cursor' and ctx['family'] == 'read_digest' and len(row['spans']) == 2,
         'one exact two-span digest-read-cursor recommendation')
    secondary, primary = row['spans']
    need(secondary['is_primary'] is False and secondary['label'] == 'recommendation not met'
         and primary['is_primary'] is True and primary['label'] is None
         and secondary['expansion'] is primary['expansion'] is None, 'exact recommendation roles')
    data = sources[ctx['proof']]
    definition = data.index(b'spec fn digest_at(')
    condition = b'0 <= start && start + 32 <= bytes.len()'
    begin = data.index(condition, definition)
    prefix = b'assert(value.0@ == '
    call = b'digest_at(bytes@, start)'
    start, end = ctx['regions'][ctx['proof']]
    use = data.index(prefix + call, start, end) + len(prefix)
    need(location(secondary, root) == (ctx['proof'], begin, begin + len(condition))
         and location(primary, root) == (ctx['proof'], use, use + len(call)),
         'exact current digest recommendation and selected proof use')


def collect(status, stdout, stderr, verifier, root, sources, proof, body, family, case_name):
    need(type(status) is int and status == 1, 'logical verifier exit1, never timeout/signal/success')
    need(root == root.resolve() and not root.is_symlink(), 'canonical exact projection')
    value = decoded(stdout)
    need(type(value) is dict and set(value) == {'verus', 'verification-results', 'func-details'}
         and type(value['func-details']) is dict
         and json.dumps(value['verus'], sort_keys=True) == json.dumps(verifier, sort_keys=True),
         'complete pinned full-root verifier envelope')
    result = value['verification-results']
    need(type(result) is dict and set(result) == {'encountered-error', 'encountered-vir-error',
         'verified', 'errors', 'is-verifying-entire-crate', 'success'}
         and result['encountered-error'] is True and result['encountered-vir-error'] is False
         and result['is-verifying-entire-crate'] is True and result['success'] is False
         and type(result['verified']) is int and 0 <= result['verified'] <= 256
         and type(result['errors']) is int and 1 <= result['errors'] <= 32,
         'bounded full-root logical report, no frontend or selected-root result')
    ctx = context(sources, proof, body, family)
    rows = [decoded(line) for line in stderr.splitlines()]
    logical = enumerations = recommendations = questions = aborts = 0
    normalized = []
    for row in rows:
        need(not aborts, 'abort is the final diagnostic')
        need(type(row) is dict and set(row) == {'$message_type', 'children', 'code', 'level', 'message', 'rendered', 'spans'}
             and row['$message_type'] == 'diagnostic' and row['children'] == [] and row['code'] is None
             and type(row['message']) is str and type(row['rendered']) is str and type(row['spans']) is list,
             'complete raw diagnostic envelope')
        for span in row['spans']:
            source_span(span, root, sources, ctx, case_name, row['message'])
            question = span['expansion'] is not None and span['expansion']['macro_decl_name'] == 'desugaring of operator `?`'
            if question:
                need(len(row['spans']) == 2 and span is row['spans'][0], 'one first nonprimary exit span beside the real logical primary')
            questions += int(question)
        if row['level'] == 'error' and row['message'] in LOGICAL:
            primary = [span for span in row['spans'] if span['is_primary']]
            need(len(primary) == 1 and inside(primary[0], root, ctx['regions']),
                 'one nonempty logical primary in the actual selected function or shared macro')
            need(len(row['spans']) == (1 if row['message'] == 'assertion failed' else 2),
                 'observed exact logical span cardinality')
            logical += 1
        elif row['level'] == 'note' and row['message'] == ENUMERATION:
            need(enumerations == 0 and not normalized and len(row['spans']) == 1,
                 'one leading full-function enumeration note')
            span = row['spans'][0]
            need(span['is_primary'] is True and span['label'] is None and span['expansion'] is None
                 and location(span, root) == (proof, *ctx['header']), 'exact selected function signature')
            enumerations += 1
        elif row['level'] == 'note' and row['message'] == 'recommendation not met':
            need(recommendations == 0, 'unique auxiliary recommendation')
            recommendation(row, root, sources, ctx, case_name)
            recommendations += 1
        elif row['level'] == 'error' and row['message'] == 'aborting due to ' + str(logical) + ' previous error' + ('s' if logical != 1 else ''):
            need(not row['spans'] and logical > 0, 'spanless terminal abort counts emitted logical diagnostics')
            aborts += 1
        else:
            need(False, 'unexpected/frontend/resource/tool diagnostic remains fatal')
        normalized.append({'level': row['level'], 'message': row['message'],
                           'spans': [normalized_span(span, root) for span in row['spans']]})
    need(enumerations == aborts == 1 and result['errors'] <= logical <= 128
         and recommendations == int(case_name == 'digest_read_cursor')
         and questions == int(case_name == 'decode_extra_length'), 'complete exact observed diagnostic families')
    return {'verified': result['verified'], 'failed_function_count': result['errors'],
            'logical_diagnostic_count': logical, 'normalized_diagnostics': normalized,
            'source_spans_validated': True, 'qualified_logical_kill': False}


def classify(status, stdout, stderr, verifier, root, sources, proof, body, family, case_name, calibration):
    need(type(calibration['source_capture_accepted_negative_classifications']) is int
         and calibration['source_capture_accepted_negative_classifications'] == 0
         and type(calibration['source_capture_qualified_kills']) is int
         and calibration['source_capture_qualified_kills'] == 0
         and case_name in calibration['cases'], 'known exact case calibrated from unqualified capture only')
    expected = calibration['cases'][case_name]
    need(expected['family'] == family and expected['selector'] == SELECTORS[family], 'same actual selected family')
    closure = dict(calibration['proof_inputs'], **{body: expected['body_sha256']})
    need({path: hashlib.sha256(data).hexdigest() for path, data in sources.items()} == closure,
         'exact full eleven-input mutant closure and unchanged contracts')
    observed = collect(status, stdout, stderr, verifier, root, sources, proof, body, family, case_name)
    need(observed == expected['observed'], 'exact calibrated counts and entire ordered source-bound span trees')
    return {**observed, 'case': case_name, 'strict_classification_pass': True,
            'expectation_calibrated': True, 'qualified_logical_kill': False}
