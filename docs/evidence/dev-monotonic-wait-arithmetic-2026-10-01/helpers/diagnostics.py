#!/usr/bin/env python3
"""Strict wait-helper diagnostic calibration, never fresh proof qualification.

The campaign owner still binds verifier identity, commands and process custody.
This predicate authenticates the complete two-input projection and the intended
numeric contract diagnostic. It proves no std/Instant adapter or whole cursor.
"""
import hashlib
import json
from pathlib import Path
import types

PARSER_SHA = '318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4'
MUTATIONS_SHA = 'c111410b8fa3fff49dc60b40d276575326ec9ff99fb1f0ec782724ba5308c9c2'
MACRO = 'monotonic_wait_arithmetic_body_v1'
FUNCTIONS = {
    'increment_wait_attempts_v1', 'wait_prefix_v1', 'wait_sleep_uses_remaining_v1',
    'wait_backoff_pair_v1', 'min_nat_v1', 'canonical_duration_pair_v1',
    'duration_pair_ns_v1', 'selected_sleep_pair_v1', 'WAIT_NANOS_PER_SECOND_V1',
    'SPIN_ATTEMPTS_V1', 'YIELD_ATTEMPTS_V1',
}
CEILING_CASES = {
    'backoff-ceiling-not-applied', 'backoff-reversed-ceiling-seconds',
    'backoff-reversed-ceiling-nanos',
}
PRIMARY = {
    'increment': 'result as nat == if attempts == u32::MAX {\n'
                 '        u32::MAX as nat\n    } else { attempts as nat + 1 }',
    # Verus locates these lowered match postconditions at the first pattern.
    'prefix': 'WaitPrefixV1::Spin',
    'sleep': 'Some(value)',
    'backoff': 'duration_pair_ns_v1(result) == min_nat_v1(\n'
               '            min_nat_v1(duration_pair_ns_v1(next) * 2,\n'
               '                u64::MAX as nat * 1_000_000_000 + 999_999_999),\n'
               '            duration_pair_ns_v1(ceiling))',
    'ceiling': 'duration_pair_ns_v1(result) <= duration_pair_ns_v1(ceiling)',
}
ARGS = {'increment': 'increment, attempts', 'prefix': 'prefix, attempts',
        'sleep': 'sleep, next, remaining', 'backoff': 'backoff, next, ceiling'}


def need(value, message):
    if not value:
        raise ValueError(message)


def load(path, digest, name):
    need(path.is_absolute() and path.resolve() == path and path.is_file()
         and not path.is_symlink(), 'canonical ordinary helper')
    data = path.read_bytes()
    need(hashlib.sha256(data).hexdigest() == digest, 'exact reviewed helper')
    module = types.ModuleType(name)
    module.__file__ = str(path)
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def parse(helper, text):
    def invalid(value):
        raise ValueError('nonfinite JSON: ' + value)
    return json.loads(text, object_pairs_hook=helper.strict_object, parse_constant=invalid)


def unique(text, expression, lo=0, hi=None):
    hi = len(text) if hi is None else hi
    need(text[lo:hi].count(expression) == 1, 'one intended source expression')
    start = text.index(expression, lo, hi)
    return start, start + len(expression)


def negative(parser_path, mutations_path, verifier, original, sources, root,
             case, status, stdout, stderr):
    helper = load(parser_path, PARSER_SHA, 'wait_recursive_diagnostics')
    mutations = load(mutations_path, MUTATIONS_SHA, 'wait_mutation_roster')
    need(type(status) is int and status == 1, 'logical exit1, not success/signal/timeout')
    matches = [row for row in mutations.CASES if row[0] == case.get('name')]
    need(len(matches) == 1, 'exact seventeen-case roster, never equivalent control')
    expected_case, expected_sources = mutations.project(original, matches[0])
    need(type(case) is dict and set(case) == set(expected_case)
         and all(type(case[key]) is type(value) and case[key] == value
                 for key, value in expected_case.items()) and sources == expected_sources,
         'exact case metadata and complete authenticated actual-body projection')
    need(all(type(data) is bytes for data in sources.values()), 'literal full source buffers')
    sources = {path: data.decode('ascii') for path, data in sources.items()}
    body, proof = sources[mutations.BODY], sources[mutations.PROOF]
    arm, function = case['arm'], case['expected_boundary']
    interval = helper.function_interval(proof, function)
    ensures = proof.index('    ensures', interval[0], interval[1])
    body_open = proof.index('\n{', ensures, interval[1]) + 1
    signature_end = proof.index(')\n    ', interval[0], ensures + 4)
    need(proof[signature_end + 6:].startswith(('requires', 'ensures')),
         'typed return terminator before the first contract clause')
    kind = 'ceiling' if case['name'] in CEILING_CASES else arm
    primary_end = body_open
    if arm == 'sleep':
        match_open = proof.index('match remaining {', ensures, body_open) + len('match remaining ')
        primary_end = helper.balanced_end(proof, match_open)
    primary = unique(proof, PRIMARY[kind], ensures, primary_end)
    call = unique(proof, MACRO + '!(' + ARGS[arm] + ')', body_open, interval[1])
    declaration = unique(body, 'macro_rules! ' + MACRO)
    arm_start = body.index('    (' + arm + ', ')
    macro_open = body.index('=> {{', arm_start) + len('=> {')
    macro_end = helper.balanced_end(body, macro_open)
    helper.MACROS.add(MACRO + '!')
    helper.PROOFS['composition'] = mutations.PROOF
    helper.CLOSURES['composition'] = set(sources)
    helper.COUNTS['composition'] = 7
    helper.target_ranges = lambda *_: {
        str(root / mutations.PROOF): interval,
        str(root / mutations.BODY): (arm_start, macro_end),
    }
    need(type(stdout) is str and type(stderr) is str and stdout.endswith('\n')
         and stderr.endswith('\n') and len(stdout.encode()) <= 256 * 1024
         and len(stderr.encode()) <= 256 * 1024, 'complete bounded raw streams')
    result = parse(helper, stdout)
    need(type(result) is dict and set(result) == {'verus', 'verification-results', 'func-details'}
         and result['verus'] == verifier, 'exact full report and expected verifier')
    expected = {'encountered-error': True, 'encountered-vir-error': False, 'success': False,
                'verified': 6, 'errors': 1, 'is-verifying-entire-crate': True}
    actual = result['verification-results']
    need(type(actual) is dict and set(actual) == set(expected)
         and all(type(actual[key]) is type(value) and actual[key] == value
                 for key, value in expected.items()), 'exact full7-root logical6/1 result')
    names = {mutations.PROOF.stem + '::' + name for name in FUNCTIONS}
    names |= {'vstd::function::axiom_fn_mut_call_requires', 'vstd::function::axiom_fn_mut_call_ensures'}
    details = result['func-details']
    need(type(details) is dict and set(details) == names
         and all(value == {'obligation_proof_notes': [], 'failed_proof_notes': []}
                 for value in details.values()), 'complete exact function-note roster')
    classified = helper.negative(verifier, sources, root,
        {'family': 'composition', 'selector': None, 'boundary': function}, status, stdout, stderr)
    rows = [parse(helper, line) for line in stderr.splitlines()]
    need(len(rows) == 3, 'one enumeration, one contract failure, one terminal footer')
    note, logical, summary = rows
    paths = helper.source_map(sources, root, 'composition')
    proof_path, body_path = str(root / mutations.PROOF), str(root / mutations.BODY)

    def span_at(span, path, limits, primary_role, label):
        need(helper.span_source(span, paths, proof_path) == (path, *limits)
             and span['is_primary'] is primary_role and span['label'] == label,
             'exact intended source span, role and label')

    need(note['message'] == next(value for value in helper.ENUMERATION
                                if value.startswith('function body check:'))
         and note['level'] == 'note' and len(note['spans']) == 1, 'exact function enumeration')
    span_at(note['spans'][0], proof_path, (interval[0], signature_end), True, None)
    need(note['spans'][0]['expansion'] is None, 'direct exact function signature')
    need(logical['level'] == 'error' and logical['message'] == 'postcondition not satisfied'
         and len(logical['spans']) == 2, 'exact numeric contract failure')
    exit_span, primary_span = logical['spans']
    span_at(primary_span, proof_path, primary, True, 'failed this postcondition')
    need(primary_span['expansion'] is None, 'direct numeric postcondition primary')
    span_at(exit_span, body_path, (macro_open, macro_end), False, 'at the end of the function body')
    expansion = exit_span['expansion']
    need(expansion is not None and expansion['macro_decl_name'] == MACRO + '!',
         'actual executable macro expansion required')
    span_at(expansion['span'], proof_path, call, False, None)
    span_at(expansion['def_site_span'], body_path, declaration, False, None)
    need(expansion['span']['expansion'] is None and expansion['def_site_span']['expansion'] is None,
         'exact direct wrapper-to-shared-body expansion')
    need(summary['level'] == 'error' and summary['message'] == 'aborting due to 1 previous error'
         and summary['spans'] == [], 'exact complete logical footer')
    classified.update(case=case['name'], boundary=function, proof_inputs=2,
        calibrated_logical_observation=True, logical_diagnostic_records=1,
        contract_kind=kind, primary_expression=PRIMARY[kind],
        lowered_match_pattern_primary=arm in ('prefix', 'sleep'),
        helper_only_unreachable_adapter_case=case['name'] == 'sleep-absent-remaining-selected',
        qualified_kill=False, historical_capture_is_qualified_kill=False,
        std_duration_or_instant_adapter_proved=False, whole_cursor_proved=False)
    return classified
