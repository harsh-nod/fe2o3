#!/usr/bin/env python3
"""Reject corrupted macro diagnostics, proof-source injections and rehashed packets."""
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
    spec = importlib.util.spec_from_file_location('shared_storage_diagnostic_selftest', path)
    checker = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), 'exec'), checker.__dict__)
    module = checker.load(args.repo.resolve())
    base = module.inherited().BASE
    mutation = checker.mutations(base)[0]
    case = args.proof / mutation.name
    source = (case / 'verus' / checker.SOURCE).read_text()
    body = (case / checker.BODY).read_text()
    record = json.loads((case / 'solver/record.json').read_text())
    original = Path(record['command'][-1])
    out = (case / 'solver/stdout.log').read_text()
    err = (case / 'solver/stderr.log').read_text()
    checker.check_result(module, 1, out, err, source, body, original, mutation)
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
    invalid.extend([(1, out + out, err), (1, out, ''), (1, out, err + '\nsolver timed out'), (1, out, err + err)])
    for status, stdout, stderr in invalid:
        try:
            checker.check_result(module, status, stdout, stderr, source, body, original, mutation)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse solver result')
    good = args.proof / 'positive_before'
    good_source = (good / 'verus' / checker.SOURCE).read_text()
    good_body = (good / checker.BODY).read_text()
    with tempfile.TemporaryDirectory(prefix='fe2o3-shared-storage-source-selftest-') as temporary:
        for index in range(8):
            folder = Path(temporary) / str(index)
            shutil.copytree(good, folder)
            changed_source, changed_body = good_source, good_body
            if index == 1:
                changed_source = changed_source.replace('../src/context_version_journal/settlement_return_body.rs', 'foreign.rs')
            elif index == 2:
                changed_source += '\ninclude!("foreign.rs");\n'
            elif index == 3:
                changed_body = changed_body.replace('($storage:ident', '($storage:expr')
            elif index == 4:
                changed_body = changed_body.replace('        Ok(())', '        assume(false);\n        Ok(())')
            elif index == 5:
                changed_body = changed_body.replace('        Ok(())', '        #[cfg(test)] let bypass = 0;\n        Ok(())')
            elif index == 6:
                changed_body += '\nmacro_rules! bypass { () => { true }; }\n'
            elif index == 7:
                path = folder / 'verus' / module.storage_parent.SOURCE
                path.write_bytes(path.read_bytes() + b'\n')
            try:
                checker.audit_sources(module, changed_source, changed_body, folder)
            except (ValueError, module.inherited().POLICY.ScanError):
                if index == 0:
                    raise
            else:
                if index != 0:
                    raise ValueError('accepted unauthenticated shared input')
    rejected_packets = 0
    if args.packet:
        audit = Path(__file__).resolve().with_name('audit.py')
        command = [sys.executable, '-I', '-B', str(audit), '--repo', str(args.repo.resolve()), '--packet']
        pristine = subprocess.run([*command, str(args.packet.resolve())], capture_output=True, text=True, timeout=90)
        checker.need(pristine.returncode == 0, 'pristine packet must pass before adverse tests: ' + pristine.stderr)
        expected_failures = ['exact staged nested inputs', 'exact macro exit, expansion, invocation and definition',
                             'owned solver completion', 'owned solver completion', 'exact whole-crate result',
                             'proof roster', 'proof input identities', 'owned command completion']
        with tempfile.TemporaryDirectory(prefix='fe2o3-shared-storage-packet-selftest-') as temporary:
            for index in range(8):
                packet = Path(temporary) / str(index)
                shutil.copytree(args.packet, packet)
                if index == 0:
                    path = packet / 'proof/writer_add' / checker.BODY
                    path.write_text(path.read_text().replace('checked_add(0)', 'checked_add(1)'))
                elif index == 1:
                    path = packet / 'proof/writer_add/solver/stderr.log'
                    rows = [json.loads(line) for line in path.read_text().splitlines()]
                    target(rows, 'expansion')['macro_decl_name'] = 'foreign!'
                    path.write_text('\n'.join(map(json.dumps, rows)) + '\n')
                elif index in (2, 3):
                    path = packet / 'proof/writer_add/solver/record.json'
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
                else:
                    path = packet / 'cargo/test/receipt.json'
                    values = json.loads(path.read_text())
                    values['group_absent'] = False
                    path.write_text(json.dumps(values))
                rows = [f'{checker.digest(p.read_bytes())}  {p.relative_to(packet)}\n'
                        for p in sorted(packet.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS']
                (packet / 'SHA256SUMS').write_text(''.join(rows))
                result = subprocess.run([*command, str(packet)], capture_output=True, text=True, timeout=90)
                checker.need(result.returncode != 0, 'rehashed adverse packet accepted')
                checker.need('ValueError: ' + expected_failures[index] in result.stderr,
                             'adverse packet failed for unrelated reason: ' + result.stderr)
                rejected_packets += 1
    print(json.dumps({'adverse_solver_results_rejected': len(invalid), 'adverse_source_inputs_rejected': 7,
                      'rehashed_adverse_packets_rejected': rejected_packets, 'known_negative_rechecked': mutation.name}))


if __name__ == '__main__':
    main()
