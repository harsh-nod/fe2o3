#!/usr/bin/env python3
"""Calibrated strict diagnostics; replay alone never qualifies a mutation kill."""
import hashlib
import json
from pathlib import Path
import re

LOGICAL = frozenset(('postcondition not satisfied', 'precondition not satisfied', 'assertion failed',
    'invariant not satisfied at end of loop body', 'invariant not satisfied before loop',
    'loop invariant not satisfied'))
ROOT_NOTE = 'verifying root module (selected functions)'
ENUMERATION = frozenset(prefix + ': not all errors may have been reported; rerun with a higher value for '
    '--multiple-errors to find other potential errors in this function' for prefix in ('function body check', 'while loop'))
VSTD_SEQ_SHA = '69e792cd1c8df0f3ce87e71ae9b07b833b606315e94eb4e23d78adba2f788663'
RECOMMENDATION_CASES = {
    'header-success-cursor-reset': ('read_header', b'bytes@.subrange(end - 2, end)'),
    'u64-write-prefix-substituted': ('write_u64', b'final(bytes)@.subrange(*old(offset) as int, *final(offset) as int)'),
}


def need(value, message):
    if not value:
        raise ValueError(message)


def strict_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def decoded(value):
    return json.loads(value, object_pairs_hook=strict_object,
        parse_constant=lambda _: need(False, 'nonfinite JSON'))


def span_source(span, root, sources, depth=0, while_context=None):
    fields = {'byte_end', 'byte_start', 'column_end', 'column_start', 'expansion', 'file_name',
        'is_primary', 'label', 'line_end', 'line_start', 'suggested_replacement', 'suggestion_applicability', 'text'}
    need(type(span) is dict and set(span) == fields and depth <= 8 and type(span['is_primary']) is bool
         and span['suggested_replacement'] is None and span['suggestion_applicability'] is None
         and (span['label'] is None or type(span['label']) is str), 'exact bounded source span schema')
    need(type(span['file_name']) is str and Path(span['file_name']).is_absolute(), 'absolute source spelling')
    path = Path(span['file_name']).resolve()
    need(path.is_relative_to(root), 'source span inside this projection')
    relative = str(path.relative_to(root))
    need(relative in sources, 'source span inside exact nine-file closure')
    data = sources[relative]
    need(data.isascii(), 'ASCII byte-coordinate proof source')
    lines = data.splitlines(keepends=True)
    names = ('byte_start', 'byte_end', 'line_start', 'line_end', 'column_start', 'column_end')
    need(all(type(span[n]) is int for n in names)
         and 1 <= span['line_start'] <= span['line_end'] <= len(lines)
         and 0 <= span['byte_start'] < span['byte_end'] <= len(data), 'bounded source coordinates')
    first = lines[span['line_start'] - 1].rstrip(b'\r\n')
    last = lines[span['line_end'] - 1].rstrip(b'\r\n')
    need(1 <= span['column_start'] <= len(first) + 1 and 1 <= span['column_end'] <= len(last) + 1,
         'bounded columns')
    start = sum(map(len, lines[:span['line_start'] - 1])) + span['column_start'] - 1
    end = sum(map(len, lines[:span['line_end'] - 1])) + span['column_end'] - 1
    need((start, end) == (span['byte_start'], span['byte_end']) and type(span['text']) is list
         and len(span['text']) == span['line_end'] - span['line_start'] + 1, 'exact byte/line join')
    for number, row in enumerate(span['text'], span['line_start']):
        text = lines[number - 1].rstrip(b'\r\n').decode()
        need(type(row) is dict and set(row) == {'text', 'highlight_start', 'highlight_end'}
             and row['text'] == text and type(row['highlight_start']) is int and type(row['highlight_end']) is int
             and row['highlight_start'] == (span['column_start'] if number == span['line_start'] else 1)
             and row['highlight_end'] == (span['column_end'] if number == span['line_end'] else len(text) + 1),
             'exact source text/highlights')
    observed = [(relative, start, end)]
    if span['expansion'] is not None:
        expansion = span['expansion']
        need(type(expansion) is dict and set(expansion) == {'span', 'macro_decl_name', 'def_site_span'}
             and type(expansion['macro_decl_name']) is str, 'exact macro expansion frame')
        if expansion['macro_decl_name'] == 'desugaring of `while` loop':
            need(while_context is not None and depth == 0
                 and relative == while_context['body'] and (start, end) == while_context['header']
                 and span['is_primary'] is True and span['label'] is None,
                 'one known equality while-enumeration primary, not a logical/error placeholder')
            invocation = expansion['span']
            nested = invocation.get('expansion')
            need(type(nested) is dict and set(nested) == {'span', 'macro_decl_name', 'def_site_span'}
                 and nested['macro_decl_name'] == 'distributed_codec_bytes_equal_body_v1!',
                 'exact nested shared equality macro frame')
            for selected, expected_path, expected_interval in (
                (invocation, while_context['body'], while_context['loop']),
                (nested['span'], while_context['proof'], while_context['invocation']),
                (nested['def_site_span'], while_context['body'], while_context['definition'])):
                need(type(selected) is dict and selected.get('is_primary') is False
                     and selected.get('label') is None
                     and Path(selected.get('file_name', '')).resolve() == root / expected_path
                     and (selected.get('byte_start'), selected.get('byte_end')) == expected_interval,
                     'exact source-bound loop, selected function invocation and macro declaration')
            need(nested['span']['expansion'] is None and nested['def_site_span']['expansion'] is None,
                 'no deeper unknown macro chain')
            placeholder = dict(file_name=str(root / while_context['proof']), byte_start=0, byte_end=0,
                line_start=1, line_end=1, column_start=1, column_end=1, is_primary=False, text=[],
                label=None, suggested_replacement=None, suggestion_applicability=None, expansion=None)
            need(expansion['def_site_span'] == placeholder
                 and all(type(expansion['def_site_span'][key]) is int for key in
                         ('byte_start', 'byte_end', 'line_start', 'line_end', 'column_start', 'column_end'))
                 and type(expansion['def_site_span']['is_primary']) is bool,
                 'exact compiler-generated auxiliary while def-site sentinel only')
            observed.extend(span_source(invocation, root, sources, depth + 1))
        else:
            for name in ('span', 'def_site_span'):
                observed.extend(span_source(expansion[name], root, sources, depth + 1))
    return observed


def recommendation_spans(row, root, sources, proof, kind, case_name, region, vstd_source):
    need(case_name in RECOMMENDATION_CASES and RECOMMENDATION_CASES[case_name][0] == kind
         and len(row['spans']) == 2, 'exact two-span recommendation note in one of two named cases only')
    need(type(vstd_source) is bytes and hashlib.sha256(vstd_source).hexdigest() == VSTD_SEQ_SHA,
         'authenticated pinned vstd seq source, not an arbitrary external span')
    start, end = 32365, 32416
    expression = b'0 <= start_inclusive <= end_exclusive <= self.len()'
    lines = vstd_source.splitlines(keepends=True)
    need(vstd_source[start:end] == expression
         and sum(map(len, lines[:1029])) + 12 == start
         and sum(map(len, lines[:1029])) + 63 == end,
         'pinned vstd subrange recommends bytes and line/column join')
    auxiliary, primary = row['spans']
    expected = dict(file_name='vstd/seq.rs', byte_start=start, byte_end=end,
        line_start=1030, line_end=1030, column_start=13, column_end=64, is_primary=False,
        text=[], label='recommendation not met', suggested_replacement=None,
        suggestion_applicability=None, expansion=None)
    need(auxiliary == expected and type(auxiliary['is_primary']) is bool
         and all(type(auxiliary[key]) is int for key in
                 ('byte_start', 'byte_end', 'line_start', 'line_end', 'column_start', 'column_end')),
         'exact auxiliary nonprimary vstd recommendation, no suggestions or recursive expansion')
    need(type(primary) is dict and primary.get('is_primary') is True
         and primary.get('label') is None and primary.get('expansion') is None,
         'real recommendation primary, no role reversal or hidden expansion')
    bound = span_source(primary, root, sources)
    needle = RECOMMENDATION_CASES[case_name][1]
    data = sources[proof]
    need(data.count(needle, *region) == 1, 'one exact recommendation expression in selected function')
    location = data.index(needle, *region)
    need(bound == [(proof, location, location + len(needle))]
         and region[0] <= location < location + len(needle) <= region[1],
         'source-bound exact recommendation invocation inside selected function')
    return [[], bound]


def collect(status, stdout, stderr, verifier, root, sources, proof, body, kind, case_name, vstd_source):
    need(type(status) is int and status == 1, 'logical verifier exit1, not timeout/signal/success')
    value = decoded(stdout)
    need(type(value) is dict and json.dumps(value.get('verus'), sort_keys=True) == json.dumps(verifier, sort_keys=True),
         'pinned verifier identity')
    result = value.get('verification-results')
    need(type(result) is dict and set(result) == {'encountered-error', 'encountered-vir-error', 'verified', 'errors',
         'is-verifying-entire-crate'} and result['encountered-error'] is True
         and result['encountered-vir-error'] is False and result['is-verifying-entire-crate'] is False
         and type(result['verified']) is int and 0 <= result['verified'] <= 63
         and type(result['errors']) is int and 1 <= result['errors'] <= 16,
         'bounded selected logical result; no frontend failure or invented expected count')
    function_start = sources[proof].index(('fn ' + kind + '(').encode())
    function_end = sources[proof].index(b'\n}\n', function_start) + 3
    macro_start = sources[body].index(('macro_rules! distributed_codec_' + kind + '_body_v1 {').encode())
    next_macro = sources[body].find(b'\nmacro_rules! ', macro_start + 1)
    macro_end = len(sources[body]) if next_macro < 0 else next_macro
    regions = {proof: (function_start, function_end), body: (macro_start, macro_end)}
    while_context = None
    if kind == 'bytes_equal':
        loop_start = sources[body].index(b'while ', macro_start, macro_end)
        loop_header_end = sources[body].index(b'\n', loop_start, macro_end)
        loop_end = sources[body].index(b'\n            }\n', loop_start, macro_end) + len(b'\n            }')
        call_start = sources[proof].index(b'distributed_codec_bytes_equal_body_v1!(', function_start, function_end)
        call_end = sources[proof].index(b']);', call_start, function_end) + 2
        while_context = {'proof': proof, 'body': body, 'header': (loop_start, loop_header_end),
            'loop': (loop_start, loop_end), 'invocation': (call_start, call_end),
            'definition': (macro_start, macro_start + len(b'macro_rules! distributed_codec_bytes_equal_body_v1'))}
    rows = [decoded(line) for line in stderr.splitlines()]
    logical, roots, aborts, messages = [], 0, 0, []
    seen_enumerations, recommendations = set(), 0
    for row in rows:
        need(type(row) is dict and set(row) == {'$message_type', 'children', 'code', 'level', 'message', 'rendered', 'spans'}
             and row['$message_type'] == 'diagnostic' and row['children'] == [] and row['code'] is None
             and type(row['message']) is str and type(row['rendered']) is str and type(row['spans']) is list,
             'complete raw diagnostic envelope')
        auxiliary = (while_context if row['level'] == 'note' and row['message'] in ENUMERATION
            and row['message'].startswith('while loop:') else None)
        if auxiliary is not None:
            need(len(row['spans']) == 1, 'exactly one top-level equality while-enumeration span')
        if row['level'] == 'note' and row['message'] == 'recommendation not met':
            need(recommendations == 0, 'one unique auxiliary recommendation note')
            per_span = recommendation_spans(row, root, sources, proof, kind, case_name,
                regions[proof], vstd_source)
            recommendations += 1
        else:
            per_span = [span_source(span, root, sources, while_context=auxiliary) for span in row['spans']]
        bound = [entry for values in per_span for entry in values]
        if row['level'] == 'error' and row['message'] in LOGICAL:
            need(any(span['is_primary'] for span in row['spans'])
                 and any(span['is_primary'] and values[0][0] in regions
                         and regions[values[0][0]][0] <= values[0][1] < values[0][2] <= regions[values[0][0]][1]
                         for span, values in zip(row['spans'], per_span)),
                 'nonempty logical primary itself attributed to the selected body/proof family')
            logical.append(row)
        elif row['level'] == 'error' and row['message'] == 'aborting due to ' + str(result['errors']) + ' previous error' + ('s' if result['errors'] != 1 else ''):
            need(not row['spans'], 'spanless exact abort')
            aborts += 1
        elif row['level'] == 'note' and row['message'] == ROOT_NOTE:
            need(not row['spans'], 'spanless exact selection note')
            roots += 1
        elif row['level'] == 'note' and row['message'] in ENUMERATION:
            need(bool(bound) and row['message'] not in seen_enumerations, 'source-bound unique enumeration note')
            seen_enumerations.add(row['message'])
        elif row['level'] == 'note' and row['message'] == 'recommendation not met':
            need(bool(bound), 'recommendation note has an authenticated selected source primary')
        else:
            need(False, 'unexpected/frontend/tool diagnostic, not a collected logical failure')
        messages.append({'level': row['level'], 'message': row['message']})
    need(len(logical) == result['errors'] and roots == aborts == 1, 'all logical errors/root/abort accounted for')
    need(recommendations == int(case_name in RECOMMENDATION_CASES), 'exact observed per-case recommendation count')
    return {'observed_verified': result['verified'], 'observed_errors': result['errors'],
        'diagnostic_messages': messages, 'raw_logical_diagnostics_collected': True,
        'qualified_logical_kill': False, 'expectation_calibrated': False}


def classify(status, stdout, stderr, verifier, root, sources, proof, body, kind, case_name, calibration, vstd_source):
    need(type(calibration['source_capture_accepted_negative_classifications']) is int
         and calibration['source_capture_accepted_negative_classifications'] == 0
         and type(calibration['source_capture_qualified_kills']) is int
         and calibration['source_capture_qualified_kills'] == 0 and case_name in calibration['cases'],
         'calibration from capture-only data, known exact case')
    expected = calibration['cases'][case_name]
    need(expected['kind'] == kind and expected['selector'] == '*' + kind
         and type(expected['verified']) is int and 0 <= expected['verified'] <= 63
         and type(expected['errors']) is int and expected['errors'] == 1,
         'same selected calibrated family and exact typed expected counts')
    closure = dict(calibration['proof_inputs'], **{body: expected['body_sha256']})
    need({path: hashlib.sha256(data).hexdigest() for path, data in sources.items()} == closure,
         'exact nine proof inputs and unchanged calibrated mutant body')
    observed = collect(status, stdout, stderr, verifier, root, sources, proof, body, kind, case_name, vstd_source)
    need(observed['observed_verified'] == expected['verified'] and observed['observed_errors'] == expected['errors']
         and observed['diagnostic_messages'] == expected['diagnostic_messages'],
         'exact calibrated per-case counts and complete ordered messages')
    return {**observed, 'case': case_name, 'strict_classification_pass': True,
        'expectation_calibrated': True, 'qualified_logical_kill': False}
