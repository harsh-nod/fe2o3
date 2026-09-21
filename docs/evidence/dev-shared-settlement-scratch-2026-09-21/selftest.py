#!/usr/bin/env python3
"""Reject altered invariant/exit diagnostics, shared inputs and rehashed packets."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--packet', type=Path)
    args = parser.parse_args()
    path = Path(__file__).resolve().with_name('check.py')
    spec = importlib.util.spec_from_file_location('shared_scratch_diagnostic_selftest', path)
    checker = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), 'exec'), checker.__dict__)
    module = checker.load(args.repo.resolve())
    base, spans_helper = module.inherited().BASE, module.retained_parent

    def case_inputs(mutation):
        case = args.proof / mutation.name
        source = (case / 'verus' / checker.SOURCE).read_text()
        body = (case / checker.BODY).read_text()
        record = json.loads((case / 'solver/record.json').read_text())
        original = Path(record['command'][-1])
        out = (case / 'solver/stdout.log').read_text()
        err = (case / 'solver/stderr.log').read_text()
        checker.check_result(module, record['status'], out, err, source, body, original, mutation)
        return source, body, original, out, err

    mutations = checker.mutations(base)
    for item in mutations:
        case_inputs(item)
    mutation = next(m for m in mutations if m.name == 'scan_error')
    source, body, original, out, err = case_inputs(mutation)
    invalid = [(code, out, err) for code in (0, 2, 101, 124, -9, True)]
    report = base.unique_json(out)
    for field, value in [('verified', checker.COUNT), ('verified', float(checker.COUNT - 1)), ('errors', True), ('errors', 2),
                         ('is-verifying-entire-crate', False), ('encountered-vir-error', True), ('success', 0)]:
        changed = copy.deepcopy(report)
        changed['verification-results'][field] = value
        invalid.append((1, json.dumps(changed), err))
    diagnostics = [base.unique_json(line) for line in err.splitlines()]
    for field, value in [('$message_type', 'other'), ('message', 'assertion failed'), ('level', 'warning'),
                         ('code', {'code': 'E0000'}), ('children', [{'message': 'timed out'}])]:
        changed = copy.deepcopy(diagnostics)
        changed[0][field] = value
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))

    def target(rows, name):
        primary = next(s for s in rows[0]['spans'] if s['is_primary'] is True)
        secondary = next(s for s in rows[0]['spans'] if s['is_primary'] is False)
        return {'primary': primary, 'exit': secondary, 'expansion': secondary['expansion'],
                'call': secondary['expansion']['span'], 'definition': secondary['expansion']['def_site_span']}[name]

    for name in ('primary', 'exit', 'call', 'definition'):
        for field, value in [('file_name', '/foreign.rs'), ('byte_start', 0), ('line_start', 1.0),
                             ('is_primary', 1), ('label', 'foreign clause'), ('expansion', {}),
                             ('suggested_replacement', 'replacement'), ('suggestion_applicability', 'MachineApplicable')]:
            changed = copy.deepcopy(diagnostics)
            target(changed, name)[field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
        for field, value in [('text', 'foreign excerpt'), ('highlight_start', True), ('highlight_end', 0)]:
            changed = copy.deepcopy(diagnostics)
            target(changed, name)['text'][0][field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for field, value in [('macro_decl_name', 'foreign!'), ('def_site_span', None), ('span', None)]:
        changed = copy.deepcopy(diagnostics)
        target(changed, 'expansion')[field] = value
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))

    def relocate_exit(rows):
        secondary = target(rows, 'exit')
        start = body.index('$count', checker.macro_bounds(body)[mutation.macro]['start'])
        forged = spans_helper.exact_span(body, Path(secondary['file_name']), start,
            start + len('$count'), False, 'at this exit', secondary['expansion'])
        secondary.clear()
        secondary.update(forged)

    changed = copy.deepcopy(diagnostics)
    relocate_exit(changed)
    relocated = '\n'.join(map(json.dumps, changed))
    try:
        checker.check_result(module, 1, out, relocated, source, body, original, mutation)
    except ValueError as error:
        checker.need(str(error) == 'exact scratch return or block exit', 'exact non-exit rejection')
    else:
        raise ValueError('accepted source-consistent non-exit span')
    invalid.append((1, out, relocated))
    changed = copy.deepcopy(diagnostics)
    secondary = target(changed, 'exit')
    start, end = module.scratch_parent.invocation(source, mutation.macro)
    secondary.clear()
    secondary.update(spans_helper.exact_span(source, original, start, end, False, 'at this exit'))
    invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    changed = copy.deepcopy(diagnostics)
    target(changed, 'exit')['label'] = 'at the end of the function body'
    invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    invalid.extend([(1, out + out, err), (1, out, ''), (1, out, err + '\nsolver timed out'), (1, out, err + err)])
    for status, stdout, stderr in invalid:
        try:
            checker.check_result(module, status, stdout, stderr, source, body, original, mutation)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse solver result')

    def relocate_invariant(rows, text, location, item):
        begin, _ = module.scratch_parent.invocation(text, item.macro)
        start = text.index('index <= count', begin)
        rows[0]['spans'] = [spans_helper.exact_span(text, location, start, start + len('index <= count'), True)]

    invariant_rejections = 0
    for item in mutations:
        if not item.invariant:
            continue
        text, shared, location, stdout, stderr = case_inputs(item)
        rows = [base.unique_json(line) for line in stderr.splitlines()]
        relocate_invariant(rows, text, location, item)
        try:
            checker.check_result(module, 1, stdout, '\n'.join(map(json.dumps, rows)), text, shared, location, item)
        except ValueError as error:
            checker.need(str(error) == 'exact scratch loop invariant span', 'exact invariant target rejection')
        else:
            raise ValueError('accepted a different genuine invariant')
        invariant_rejections += 1

    good = args.proof / 'positive_before'
    good_source = (good / 'verus' / checker.SOURCE).read_text()
    good_body = (good / checker.BODY).read_text()
    rejected_sources = 0
    with tempfile.TemporaryDirectory(prefix='fe2o3-shared-scratch-source-selftest-') as temporary:
        for index in range(10):
            folder = Path(temporary) / str(index)
            shutil.copytree(good, folder)
            changed_source, changed_body = good_source, good_body
            if index == 1:
                changed_source = changed_source.replace('../src/context_version_journal/settlement_scratch_bodies.rs', 'foreign.rs')
            elif index == 2:
                changed_source += '\ninclude!("foreign.rs");\n'
            elif index == 3:
                changed_body = changed_body.replace('$journal:ident', '$journal:expr', 1)
            elif index == 4:
                changed_body = changed_body.replace('            Ok(())', '            assume(false);\n            Ok(())')
            elif index == 5:
                changed_body = changed_body.replace('            Ok(())', '            #[cfg(test)] let bypass = 0;\n            Ok(())')
            elif index == 6:
                changed_body += '\nmacro_rules! bypass { () => { true }; }\n'
            elif index == 7:
                path = folder / 'verus' / module.scratch_parent.SOURCE
                path.write_bytes(path.read_bytes() + b'\n')
            elif index == 8:
                changed_source = changed_source.replace(checker.SCAN_INVARIANT, 'let bypass = true;')
            elif index == 9:
                changed_source = changed_source.replace('settlement_scratch_stage_body!(verus_exec_expr,', 'settlement_scratch_stage_body!(foreign_expr,')
            try:
                checker.audit_sources(module, changed_source, changed_body, folder)
            except (ValueError, module.inherited().POLICY.ScanError):
                if index == 0:
                    raise
                rejected_sources += 1
            else:
                if index != 0:
                    raise ValueError('accepted unauthenticated shared input')
    adapter = (args.repo / 'crates/fe2o3-runtime-model' / checker.ADAPTER).read_bytes()
    checker.audit_adapter(adapter)
    for changed in (b'/*\n' + adapter + b'*/\n', adapter + adapter,
                    adapter.replace(b'[]', b'[let bypass = true;]', 1)):
        checker.need(changed != adapter, 'effective adapter mutation')
        try:
            checker.audit_adapter(changed)
        except ValueError:
            rejected_sources += 1
        else:
            raise ValueError('accepted changed identity adapter')

    rejected_packets = 0
    if args.packet:
        audit = Path(__file__).resolve().with_name('audit.py')
        command = [sys.executable, '-I', '-B', str(audit), '--repo', str(args.repo.resolve()), '--packet']
        pristine = subprocess.run([*command, str(args.packet.resolve())], capture_output=True, text=True, timeout=90)
        checker.need(pristine.returncode == 0, 'pristine packet must pass before adverse tests: ' + pristine.stderr)
        expected_failures = ['exact staged nested inputs', 'exact scratch exit and expansion tree',
            'owned solver completion', 'owned solver completion', 'exact whole-crate result', 'proof roster',
            'proof input identities', 'owned command completion', 'exact scratch return or block exit',
            'exact scratch loop invariant span']
        with tempfile.TemporaryDirectory(prefix='fe2o3-shared-scratch-packet-selftest-') as temporary:
            for index, failure in enumerate(expected_failures):
                packet = Path(temporary) / str(index)
                shutil.copytree(args.packet, packet)
                if index == 0:
                    path = packet / 'proof/scan_error' / checker.BODY
                    path.write_text(path.read_text().replace('return Err(ReadErrorV1::InvalidReference);', 'return Err(ReadErrorV1::InvalidState);'))
                elif index in (1, 8):
                    path = packet / 'proof/scan_error/solver/stderr.log'
                    rows = [json.loads(line) for line in path.read_text().splitlines()]
                    if index == 1:
                        target(rows, 'expansion')['macro_decl_name'] = 'foreign!'
                    else:
                        relocate_exit(rows)
                    path.write_text('\n'.join(map(json.dumps, rows)) + '\n')
                elif index in (2, 3):
                    path = packet / 'proof/scan_error/solver/record.json'
                    receipt = json.loads(path.read_text())
                    if index == 2:
                        receipt['status'] = True
                    else:
                        receipt['command'].insert(-1, '--verify-root')
                    path.write_text(json.dumps(receipt))
                elif index == 4:
                    path = packet / 'proof/positive_after/solver/stdout.log'
                    values = json.loads(path.read_text())
                    values['verification-results']['verified'] = checker.COUNT - 1
                    path.write_text(json.dumps(values))
                elif index == 5:
                    (packet / 'proof/foreign.rs').write_text('foreign\n')
                elif index == 6:
                    path = packet / 'proof/inputs-after.json'
                    values = json.loads(path.read_text())
                    values[next(iter(values))] = '0' * 64
                    path.write_text(json.dumps(values))
                elif index == 7:
                    path = packet / 'cargo/test/receipt.json'
                    values = json.loads(path.read_text())
                    values['group_absent'] = False
                    path.write_text(json.dumps(values))
                else:
                    item = next(m for m in mutations if m.name == 'stage_prior_lineage')
                    text, _, location, _, _ = case_inputs(item)
                    path = packet / 'proof/stage_prior_lineage/solver/stderr.log'
                    rows = [json.loads(line) for line in path.read_text().splitlines()]
                    relocate_invariant(rows, text, location, item)
                    path.write_text('\n'.join(map(json.dumps, rows)) + '\n')
                rows = [f'{checker.digest(p.read_bytes())}  {p.relative_to(packet)}\n'
                        for p in sorted(packet.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS']
                (packet / 'SHA256SUMS').write_text(''.join(rows))
                result = subprocess.run([*command, str(packet)], capture_output=True, text=True, timeout=90)
                checker.need(result.returncode != 0, 'rehashed adverse packet accepted')
                checker.need('ValueError: ' + failure in result.stderr,
                             'adverse packet failed for unrelated reason: ' + result.stderr)
                rejected_packets += 1
    print(json.dumps({'adverse_solver_results_rejected': len(invalid) + invariant_rejections,
        'adverse_source_inputs_rejected': rejected_sources, 'rehashed_adverse_packets_rejected': rejected_packets,
        'known_negatives_rechecked': len(mutations)}))


if __name__ == '__main__':
    main()
