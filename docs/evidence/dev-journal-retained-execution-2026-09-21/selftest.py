#!/usr/bin/env python3
"""Reject altered solver diagnostics, source identities and rehashed proof packets."""
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
    repo = args.repo.resolve()
    path = Path(__file__).resolve().with_name('check.py')
    spec = importlib.util.spec_from_file_location('typed_admission_selftest_checker', path)
    checker = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), 'exec'), checker.__dict__)
    module = checker.load(repo)
    base = module.inherited().BASE

    def inputs(name, mutation):
        case = args.proof / name
        source = (case / 'verus' / checker.SOURCE).read_text()
        generated = {p.relative_to(case): p.read_bytes() for p in case.rglob('*.rs')}
        record = base.unique_json((case / 'solver/record.json').read_text())
        original = Path(record['command'][-1])
        out, err = ((case / 'solver' / name).read_text() for name in ('stdout.log', 'stderr.log'))
        checker.check_result(module, record['status'], out, err, source, generated, original, mutation)
        return source, generated, original, out, err

    controls = checker.mutations()
    for item in controls:
        inputs(item.name, item)
    for name in ('positive_before', 'positive_after'):
        inputs(name, None)
    item = controls[0]
    source, generated, original, out, err = inputs(item.name, item)
    invalid = [(code, out, err) for code in (0, 2, 101, 124, -9, True)]
    report = base.unique_json(out)
    for field, value in [('verified', 1), ('verified', 0.0), ('errors', True), ('errors', 2),
                         ('is-verifying-entire-crate', True), ('encountered-vir-error', True), ('success', False)]:
        changed = copy.deepcopy(report)
        changed['verification-results'][field] = value
        invalid.append((1, json.dumps(changed), err))
    rows = [base.unique_json(line) for line in err.splitlines()]
    for index in range(3):
        for field, value in [('$message_type', 'other'), ('code', {'code': 'E0000'}),
                             ('level', 'warning'), ('children', [{'message': 'timeout'}])]:
            changed = copy.deepcopy(rows)
            changed[index][field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for primary in (False, True):
        for field, value in [('file_name', '/foreign.rs'), ('byte_start', 0), ('line_start', 1.0),
                             ('is_primary', int(primary)), ('label', 'foreign clause'), ('expansion', {}),
                             ('suggested_replacement', 'replacement'), ('suggestion_applicability', 'MachineApplicable')]:
            changed = copy.deepcopy(rows)
            target = next(s for s in changed[1]['spans'] if s['is_primary'] is primary)
            target[field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for marker in (None, 0, 1, 'true'):
        changed = copy.deepcopy(rows)
        extra = copy.deepcopy(changed[1]['spans'][0])
        extra['is_primary'] = marker
        changed[1]['spans'].append(extra)
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    # Genuine source coordinates still must identify the named body, call and definition.
    proof, body = generated[checker.EXEC].decode(), generated[checker.BODY].decode()
    for part in ('body', 'invocation', 'definition'):
        changed = copy.deepcopy(rows)
        target = next(s for s in changed[1]['spans'] if s['is_primary'] is False)
        expansion = copy.deepcopy(target['expansion'])
        if part == 'invocation':
            offset = proof.index(item.function)
            expansion['span'] = module.retained_parent.exact_span(proof, original.parent.parent / checker.EXEC,
                offset, offset + len(item.function), False)
        else:
            text = '$reference.slot'
            offset = body.index(text)
            wrong = module.retained_parent.exact_span(body, original.parent / '..' / checker.BODY,
                offset, offset + len(text), False)
            if part == 'definition':
                expansion['def_site_span'] = wrong
            else:
                wrong['label'] = target['label']
                target.clear()
                target.update(wrong)
        target['expansion'] = expansion
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    invalid.extend([(1, out + out, err), (1, out, ''), (1, out, err + '\nsolver timed out'), (1, out, err + err)])
    for status, stdout, stderr in invalid:
        try:
            checker.check_result(module, status, stdout, stderr, source, generated, original, item)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse solver result')

    snapshots = {module.HERE.parent / p: (repo / checker.CRATE / p).read_bytes() for p in checker.PINS}
    with tempfile.TemporaryDirectory(prefix='fe2o3-typed-admission-source-selftest-') as temporary:
        for path in snapshots:
            changed = dict(snapshots)
            changed[path] += b'\n'
            try:
                checker.stage(module, changed, Path(temporary) / path.name, None)
            except ValueError as error:
                checker.need(str(error).startswith('pinned typed admission source:'), 'intended source rejection')
            else:
                raise ValueError('accepted changed typed admission input')

    rejected_packets = 0
    if args.packet:
        command = [sys.executable, '-I', '-B', str(Path(__file__).resolve().with_name('audit.py')), '--repo', str(repo), '--packet']
        result = subprocess.run([*command, str(args.packet.resolve())], capture_output=True, text=True, timeout=90)
        checker.need(result.returncode == 0, 'pristine packet: ' + result.stderr)
        failures = ['exact staged nested inputs', 'exact typed postcondition', 'owned solver completion',
                    'exact verification result', 'proof roster', 'proof input identities', 'owned command completion']
        with tempfile.TemporaryDirectory(prefix='fe2o3-typed-admission-packet-selftest-') as temporary:
            for index, failure in enumerate(failures):
                packet = Path(temporary) / str(index)
                shutil.copytree(args.packet, packet)
                case = packet / 'proof' / item.name
                if index == 0:
                    path = case / checker.BODY
                    path.write_bytes(path.read_bytes() + b'\n')
                elif index == 1:
                    changed = copy.deepcopy(rows)
                    next(s for s in changed[1]['spans'] if s['is_primary'] is True)['byte_start'] = 0
                    (case / 'solver/stderr.log').write_text('\n'.join(map(json.dumps, changed)) + '\n')
                elif index == 2:
                    path = case / 'solver/record.json'
                    data = json.loads(path.read_text())
                    data['command'].insert(-1, '--verify-root')
                    path.write_text(json.dumps(data))
                elif index == 3:
                    path = packet / 'proof/positive_after/solver/stdout.log'
                    data = json.loads(path.read_text())
                    data['verification-results']['verified'] = checker.COUNT - 1
                    path.write_text(json.dumps(data))
                elif index == 4:
                    (packet / 'proof/foreign.rs').write_text('foreign\n')
                elif index == 5:
                    path = packet / 'proof/inputs-after.json'
                    data = json.loads(path.read_text())
                    data[next(iter(data))] = '0' * 64
                    path.write_text(json.dumps(data))
                else:
                    path = packet / 'cargo/test/receipt.json'
                    data = json.loads(path.read_text())
                    data['group_absent'] = False
                    path.write_text(json.dumps(data))
                manifest = [f'{checker.digest(p.read_bytes())}  {p.relative_to(packet)}\n'
                            for p in sorted(packet.rglob('*')) if p.is_file() and p.name != 'SHA256SUMS']
                (packet / 'SHA256SUMS').write_text(''.join(manifest))
                result = subprocess.run([*command, str(packet)], capture_output=True, text=True, timeout=90)
                checker.need(result.returncode != 0 and 'ValueError: ' + failure in result.stderr,
                             'packet rejection for intended reason: ' + result.stderr)
                rejected_packets += 1
    print(json.dumps({'adverse_solver_results_rejected': len(invalid), 'altered_inputs_rejected': len(snapshots),
                      'rehashed_packets_rejected': rejected_packets, 'known_negatives_rechecked': len(controls)}))


if __name__ == '__main__':
    main()
