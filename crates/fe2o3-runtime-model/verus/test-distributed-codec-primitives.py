#!/usr/bin/env python3
"""Light source/mutation calibration only; never launches Rust or Verus."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types

if not (sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize):
    raise ValueError('use isolated python3 -I -B without optimization')

PATH = Path(__file__).with_name('check-distributed-codec-primitives.py')
spec = importlib.util.spec_from_file_location('distributed_codec_checks', PATH)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


def refused(call):
    try:
        call()
    except (ValueError, AssertionError):
        return
    raise AssertionError('expected strict refusal')


def materialized(value, root, placeholder):
    if isinstance(value, str):
        return value.replace(placeholder, str(root))
    if isinstance(value, list):
        return [materialized(item, root, placeholder) for item in value]
    if isinstance(value, dict):
        return {key: materialized(item, root, placeholder) for key, item in value.items()}
    return value


def interval_span(template, data, start, end):
    assert data.isascii() and 0 <= start < end <= len(data)
    span = copy.deepcopy(template)
    first, last = data[:start].count(b'\n') + 1, data[:end].count(b'\n') + 1
    begin, finish = start - data.rfind(b'\n', 0, start), end - data.rfind(b'\n', 0, end)
    lines = data.splitlines()
    span.update(byte_start=start, byte_end=end, line_start=first, line_end=last,
                column_start=begin, column_end=finish)
    span['text'] = [{'text': lines[index - 1].decode(),
                     'highlight_start': begin if index == first else 1,
                     'highlight_end': finish if index == last else len(lines[index - 1]) + 1}
                    for index in range(first, last + 1)]
    return span


def replay_controls(source, mutations):
    fixture_bytes = (c.ROOT / c.FIXTURES).read_bytes()
    assert hashlib.sha256(fixture_bytes).hexdigest() == c.FIXTURES_SHA
    fixture = json.loads(fixture_bytes)
    assert fixture['schema'] == 1 and len(fixture['cases']) == 11
    assert {row['family'] for row in fixture['cases'].values()} == set(c.KINDS)
    assert fixture['proof_inputs'] == {str(path): c.sha(source[path]) for path in c.FILES}
    assert b'/home/' not in fixture_bytes and b'codex-tmp' not in fixture_bytes
    classifier = c.bridge().inherited_controller().inherited()
    proxy = c.primitive_classifier(classifier)
    placeholder = fixture['source_root_placeholder']
    with tempfile.TemporaryDirectory(prefix='fe2o3-codec-diagnostics-') as directory:
        temporary = Path(directory).resolve()
        roots, streams = {}, {}
        for name, record in fixture['cases'].items():
            body, focus = mutations[name]
            assert record['selector'] == focus and record['mutated_body_sha256'] == c.sha(body)
            assert record['family'] in c.KINDS and c.SELECTORS[record['family']] == focus
            root = temporary / name
            roots[name] = root
            for path in c.FILES:
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((body if path == c.BODY else source[path]).encode())
            stdout = json.dumps(record['stdout_projection'])
            rows = materialized(record['diagnostics'], root, placeholder)
            stderr = '\n'.join(json.dumps(row) for row in rows) + '\n'
            verifier = record['stdout_projection']['verus']
            paths = {str(root / path) for path in c.FILES}
            streams[name] = (stdout, rows, verifier)
            assert c.bounded_negative(classifier, root, focus, 1, stdout, stderr, verifier), name
            assert proxy.logical_negative(c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS={c.POSTCONDITION}), focus),
                                          1, stdout, stderr, verifier, paths)
            assert c.bounded_negative(classifier, root, focus, 1, stdout,
                                      '\n'.join(json.dumps(row) for row in reversed(rows)), verifier)

        for name, record in fixture['cases'].items():
            root, focus = roots[name], record['selector']
            stdout, rows, verifier = streams[name]

            def accepts(changed_rows=rows, changed_output=None, status=1, selector=focus, selected_root=root):
                stderr = '\n'.join(json.dumps(row) for row in changed_rows)
                return c.bounded_negative(classifier, selected_root, selector, status,
                    stdout if changed_output is None else changed_output, stderr, verifier)

            for status in (0, 2, True, 124, -15):
                assert not accepts(status=status)
            for field, value in [('errors', 0), ('errors', 2), ('errors', True), ('verified', True),
                                 ('verified', 1), ('encountered-error', False), ('encountered-vir-error', True),
                                 ('is-verifying-entire-crate', True), ('success', False)]:
                output = copy.deepcopy(record['stdout_projection'])
                output['verification-results'][field] = value
                assert not accepts(changed_output=json.dumps(output))
            for value in (float('nan'), float('inf'), -float('inf')):
                output = copy.deepcopy(record['stdout_projection'])
                output['verification-results']['verified'] = value
                assert not accepts(changed_output=json.dumps(output))
            duplicate = '{"verus":' + json.dumps(verifier) + ',"verus":' + json.dumps(verifier)
            duplicate += ',"verification-results":' + json.dumps(c.NEGATIVE_RESULT) + '}'
            assert not accepts(changed_output=duplicate)
            assert not accepts([])
            assert not accepts([row for row in rows if row['message'] != c.POSTCONDITION])
            assert not accepts([row for row in rows if row['message'].startswith('aborting')])
            error_index = next(i for i, row in enumerate(rows) if row['message'] == c.POSTCONDITION)
            primary_index = next(i for i, span in enumerate(rows[error_index]['spans']) if span['is_primary'])
            for value in ([], [rows[error_index]['spans'][primary_index]], [None, None]):
                changed = copy.deepcopy(rows)
                changed[error_index]['spans'] = value
                assert not accepts(changed)
            for field, value in [('byte_start', True), ('byte_end', 0), ('line_start', 99999),
                                 ('column_start', 0), ('is_primary', False), ('label', None),
                                 ('file_name', str(root / 'unrelated.rs')), ('expansion', {})]:
                changed = copy.deepcopy(rows)
                changed[error_index]['spans'][primary_index][field] = value
                assert not accepts(changed)
            for field, value in [('text', 'fabricated source'), ('highlight_start', 0), ('highlight_end', True)]:
                changed = copy.deepcopy(rows)
                changed[error_index]['spans'][primary_index]['text'][0][field] = value
                assert not accepts(changed)
            other = next(row for row in fixture['cases'].values() if row['family'] != record['family'])
            other_error = next(row for row in materialized(other['diagnostics'], root, placeholder)
                               if row['message'] == c.POSTCONDITION)
            other_primary = next(span for span in other_error['spans'] if span['is_primary'])
            changed = copy.deepcopy(rows)
            changed[error_index]['spans'][primary_index] = other_primary
            assert not accepts(changed)
            for field, value in [('message', 'unexpected frontend failure'), ('level', 'warning'),
                                 ('code', {'code': 'E0000'}), ('children', [{}])]:
                changed = copy.deepcopy(rows)
                changed[error_index][field] = value
                assert not accepts(changed)

            for index, row in enumerate(rows):
                if row['level'] == 'note':
                    assert not accepts(rows[:index] + rows[index + 1:])
                    assert not accepts(rows + [copy.deepcopy(row)])
            changed = copy.deepcopy(rows)
            changed.append(dict(changed[0], message='unknown diagnostic'))
            assert not accepts(changed)
            root_index = next(i for i, row in enumerate(rows) if row['message'] == c.ROOT_NOTE)
            changed = copy.deepcopy(rows)
            changed[root_index]['spans'] = [copy.deepcopy(rows[error_index]['spans'][primary_index])]
            assert not accepts(changed)
            enum_index = next(i for i, row in enumerate(rows) if row['message'] == c.ENUM_NOTE)
            changed = copy.deepcopy(rows)
            changed[enum_index]['spans'][0]['byte_end'] -= 1
            assert not accepts(changed)
            range_indices = [i for i, row in enumerate(rows) if row['message'] == c.RANGE_NOTE]
            if range_indices:
                assert len(range_indices) == 2
                changed = copy.deepcopy(rows)
                changed[range_indices[1]] = copy.deepcopy(changed[range_indices[0]])
                assert not accepts(changed)
                for index in range_indices:
                    if rows[index]['spans'][0]['expansion'] is None:
                        context = c.negative_context(root, focus)
                        argument = rows[index]['spans'][0]
                        assert c.family_span(argument, context) == (c.PROOF, context['argument'])
                        proof = source[c.PROOF].encode()
                        start, end = context['argument']
                        parameter = proof.index(b'value', *context['signature'])
                        contract = proof.index(b'value', *context['contract'])
                        other_argument = proof.index(b'verus_exec_expr', *context['invocation'])
                        other_kind = 'u64_le' if record['family'] == 'u16_le' else 'u16_le'
                        other_call = proof.index((c.MACROS[other_kind] + '!(').encode())
                        other_end = proof.index(b'\n', other_call)
                        invalid_intervals = [context['invocation'], (start - 1, end), (start, end + 1),
                            (start + 1, end), (parameter, parameter + len(b'value')),
                            (contract, contract + len(b'value')),
                            (other_argument, other_argument + len(b'verus_exec_expr')),
                            (other_end - 1 - len(b'value'), other_end - 1)]
                        for left, right in invalid_intervals:
                            changed = copy.deepcopy(rows)
                            span = interval_span(argument, proof, left, right)
                            assert c.source_span(span, context) == (c.PROOF, (left, right))
                            changed[index]['spans'] = [span]
                            assert not accepts(changed)
                        for field, value in (('expansion', {}), ('is_primary', False), ('label', ''),
                                             ('suggested_replacement', 'value')):
                            changed = copy.deepcopy(rows)
                            changed[index]['spans'][0][field] = value
                            assert not accepts(changed)
                        changed = copy.deepcopy(rows)
                        changed[index]['spans'][0]['text'][0]['text'] += ' '
                        assert not accepts(changed)
                        changed = copy.deepcopy(rows)
                        other_index = next(i for i in range_indices if i != index)
                        changed[other_index] = copy.deepcopy(changed[index])
                        assert not accepts(changed)
                        continue
                    for edit in ('macro', 'definition', 'invocation', 'nested', 'alias', 'nonprimary', 'false_text'):
                        changed = copy.deepcopy(rows)
                        span = changed[index]['spans'][0]
                        if edit == 'macro':
                            span['expansion']['macro_decl_name'] = 'unrelated!'
                        elif edit == 'definition':
                            span['expansion']['def_site_span']['byte_start'] += 1
                        elif edit == 'invocation':
                            span['expansion']['span'] = copy.deepcopy(other_primary)
                            span['expansion']['span']['is_primary'] = False
                            span['expansion']['span']['label'] = None
                        elif edit == 'nested':
                            span['expansion']['span']['expansion'] = copy.deepcopy(span['expansion'])
                        elif edit == 'alias':
                            span['file_name'] = str(root / c.BODY)
                        elif edit == 'nonprimary':
                            span['is_primary'] = False
                        else:
                            span['text'][0]['text'] += ' '
                        assert not accepts(changed)
            else:
                encoder = next(row for row in fixture['cases'].values() if row['family'] == 'u16_le')
                extra = [row for row in materialized(encoder['diagnostics'], root, placeholder) if row['message'] == c.RANGE_NOTE]
                assert not accepts(rows + extra)

            for field in ('macro_decl_name', 'span', 'def_site_span'):
                secondary = next(span for span in rows[error_index]['spans'] if not span['is_primary'])
                if secondary['expansion'] is not None:
                    changed = copy.deepcopy(rows)
                    span = next(span for span in changed[error_index]['spans'] if not span['is_primary'])
                    span['expansion'][field] = None
                    assert not accepts(changed)
            wrong_focus = next(value for value in c.SELECTORS.values() if value != focus)
            assert not accepts(selector=wrong_focus)
            extra = root / 'unreviewed.txt'
            extra.write_bytes(b'not a proof input')
            assert not accepts()
            extra.unlink()
            body_path = root / c.BODY
            original_body = body_path.read_bytes()
            body_path.write_bytes(original_body + b'\n')
            assert not accepts()
            body_path.write_bytes(original_body)
            proof_path = root / c.PROOF
            proof_path.write_bytes(source[c.PROOF].encode() + b'\n')
            assert not accepts()
            proof_path.write_bytes(source[c.PROOF].encode())
            alias = temporary / (name + '-alias')
            alias.symlink_to(root, target_is_directory=True)
            assert not accepts(selected_root=alias)
            alias.unlink()
            assert accepts()

        verifier = next(iter(streams.values()))[2]
        expected = dict(c.campaign().EXPECTED)
        positive = json.dumps({'verus': verifier, 'verification-results': expected})
        paths = {str(c.ROOT / path) for path in c.FILES}
        assert proxy.proof_positive(0, positive, '', verifier, expected, paths)
        assert not proxy.proof_positive(False, positive, '', verifier, expected, paths)
        for _stdout, rows, _verifier in streams.values():
            for row in rows:
                if row['level'] == 'note':
                    assert not proxy.proof_positive(0, positive, json.dumps(row), verifier, expected, paths)


def main():
    valid = dict(isolated=1, dont_write_bytecode=1, optimize=0)
    c.invocation_guard(types.SimpleNamespace(**valid))
    for key, value in [('isolated', 0), ('dont_write_bytecode', 0), ('optimize', 1), ('optimize', 2)]:
        refused(lambda field=key, setting=value: c.invocation_guard(
            types.SimpleNamespace(**dict(valid, **{field: setting}))))
    refused(lambda: c.invocation_guard(types.SimpleNamespace()))

    source = c.snapshot()
    c.audit(source)
    for path in (c.OWNER, c.NATIVE, c.BODY, c.TESTS, c.PROOF, *c.UNCHANGED):
        changed = dict(source)
        changed[path] += '\n'
        refused(lambda value=changed: c.audit(value))
    missing = dict(source)
    del missing[c.BODY]
    refused(lambda: c.audit(missing))
    extra = dict(source)
    extra[c.SRC / 'unreviewed.rs'] = ''
    refused(lambda: c.audit(extra))

    body = source[c.BODY]
    cases = c.mutations(body)
    assert len(cases) == len(set(cases.values())) == 31
    assert {focus for _, focus in cases.values()} == set(c.SELECTORS.values())
    for kind, count in [('take', 6), ('fixed', 1), ('put', 2), ('finish', 2),
                        ('u16_le', 2), ('u16_from_le', 2), ('u64_le', 8), ('u64_from_le', 8)]:
        assert sum(focus == c.SELECTORS[kind] for _, focus in cases.values()) == count
    for value, _ in cases.values():
        assert value != body and value.count('macro_rules! ') == 8
    refused(lambda: c.mutations(body.replace('if end > $bytes.len() {', 'if end >= $bytes.len() {')))
    assert '*$offset = 0;' in cases['take-overflow-cursor-reset'][0]
    assert '$bytes.split_at_mut(0)' in cases['put-frame-prefix-substituted'][0]

    # Actual observations calibrate parsing; fresh signed mutation acceptance is pending.
    assert type(c.EXPECTED_VERIFIED) is int and c.EXPECTED_VERIFIED == 54
    assert type(c.MULTIPLE_ERRORS) is int and c.MULTIPLE_ERRORS == 1
    assert set(c.SELECTION_NOTES) == set(c.SELECTORS.values())
    for focus in c.SELECTORS.values():
        notes = c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS={c.POSTCONDITION}), focus)
        assert notes.CODEC_SELECTOR == focus and notes.SELECTION_NOTES == frozenset((c.ROOT_NOTE,))
    assert c.campaign().EXPECTED['verified'] == 54
    for focus in ('*not_a_primitive', None):
        refused(lambda value=focus: c.selection_notes(types.SimpleNamespace(LOGICAL_ERRORS=set()), value))
    saved_notes = c.SELECTION_NOTES
    try:
        for value in (None, {}, {focus: frozenset(('unknown',)) for focus in c.SELECTORS.values()},
                      {focus: [c.ROOT_NOTE, c.ROOT_NOTE] for focus in c.SELECTORS.values()}):
            c.SELECTION_NOTES = value
            refused(c.campaign)
    finally:
        c.SELECTION_NOTES = saved_notes

    inherited = c.bridge()
    before = (c.ROOT / inherited.BASE).read_bytes()
    after = c.reporting_source(before)
    assert after == before.replace(b'"--multiple-errors", "0",', b'"--multiple-errors", "1",')
    for changed in (before + b'\n', after,
                    before.replace(b'"--multiple-errors", "0",', b'"--multiple-errors", "2",'),
                    before.replace(b'"--no-cheating",', b'')):
        refused(lambda value=changed: c.reporting_source(value))
    original_bridge = c.bridge
    anchor = b'"--multiple-errors", "0",'
    try:
        for changed in (before.replace(anchor, b''), before + anchor,
                        before.replace(anchor, b'"--multiple-errors", "2",')):
            # Test anchor guards independently of the unchanged source digest.
            c.bridge = lambda value=changed: types.SimpleNamespace(
                BASE_SHA=hashlib.sha256(value).hexdigest())
            refused(lambda value=changed: c.reporting_source(value))
    finally:
        c.bridge = original_bridge
    for value in (0, 2, True, '1', None):
        c.MULTIPLE_ERRORS = value
        refused(lambda: c.reporting_source(before))
    c.MULTIPLE_ERRORS = 1
    module = c.campaign_controller()
    assert module.FILES != c.FILES and module.EXPECTED['verified'] == 26
    assert callable(module.main)
    parsed = ast.parse(after)
    mains = [node for node in parsed.body if isinstance(node, ast.FunctionDef) and node.name == 'main']
    assert len(mains) == 1
    builders = [node for node in mains[0].body if isinstance(node, ast.FunctionDef) and node.name == 'proof']
    assert len(builders) == 1
    # Execute only the actual nested argv builder, never the campaign entrypoint.
    namespace = {'args': types.SimpleNamespace(verus=Path('/fixture/verus')), 'PROOF': c.PROOF}
    exec(compile(ast.Module(body=builders, type_ignores=[]), '<actual-proof-argv-control>', 'exec'), namespace)
    root = Path('/fixture/source')
    prefix = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '120', '/fixture/verus',
              '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
              '--error-format=json', '--no-report-long-running', '--num-threads', '4', '--multiple-errors', '1']
    assert namespace['proof'](root) == [*prefix, str(root / c.PROOF)]
    for focus in c.SELECTORS.values():
        assert namespace['proof'](root, focus) == [*prefix, '--verify-function', focus, '--verify-root', str(root / c.PROOF)]
    for value in (None, True, 53, 55, '54'):
        c.EXPECTED_VERIFIED = value
        refused(c.campaign)
    c.EXPECTED_VERIFIED = 54
    replay_controls(source, cases)
    print('PASS: codec primitive source calibration (13 groups; 11 actual diagnostic fixtures replayed; 31 mutants constructed; no verifier execution)')


main()
