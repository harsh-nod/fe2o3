#!/usr/bin/env python3
"""Exercise diagnostic, source and offline packet rejection independently of solvers."""
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
    spec = importlib.util.spec_from_file_location('representation_selftest_checker', path)
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
        out = (case / 'solver/stdout.log').read_text()
        err = (case / 'solver/stderr.log').read_text()
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
                         ('is-verifying-entire-crate', True), ('encountered-vir-error', True), ('success', 0)]:
        changed = copy.deepcopy(report)
        changed['verification-results'][field] = value
        invalid.append((1, json.dumps(changed), err))
    rows = [base.unique_json(line) for line in err.splitlines()]
    error_index = next(i for i, row in enumerate(rows) if row['message'] == 'postcondition not satisfied')
    for field, value in [('$message_type', 'other'), ('message', 'assertion failed'), ('level', 'warning'),
                         ('code', {'code': 'E0000'}), ('children', [{'message': 'timed out'}])]:
        changed = copy.deepcopy(rows)
        changed[error_index][field] = value
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for primary in (False, True):
        for field, value in [('file_name', '/foreign.rs'), ('byte_start', 0), ('line_start', 1.0),
                             ('is_primary', int(primary)), ('label', 'foreign clause'), ('expansion', {}),
                             ('suggested_replacement', 'replacement'), ('suggestion_applicability', 'MachineApplicable')]:
            changed = copy.deepcopy(rows)
            target = next(s for s in changed[error_index]['spans'] if s['is_primary'] is primary)
            target[field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    # A genuine source span is still invalid when it does not identify the exit.
    changed = copy.deepcopy(rows)
    proof = generated[Path(item.proof_file)].decode('ascii')
    offset = proof.index(item.function)
    secondary = next(s for s in changed[error_index]['spans'] if s['is_primary'] is False)
    secondary.clear()
    secondary.update(module.retained_parent.exact_span(proof, original.parent.parent / item.proof_file,
        offset, offset + len(item.function), False, 'at the end of the function body'))
    invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for marker in (None, 0, 1, 'true'):
        changed = copy.deepcopy(rows)
        extra = copy.deepcopy(changed[error_index]['spans'][0])
        extra['is_primary'] = marker
        changed[error_index]['spans'].append(extra)
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    for index in (0, len(rows) - 1):
        for field, value in [('$message_type', 'other'), ('code', {'code': 'E0000'})]:
            changed = copy.deepcopy(rows)
            changed[index][field] = value
            invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    invalid.extend([(1, out + out, err), (1, out, ''), (1, out, err + '\nsolver timed out'), (1, out, err + err)])
    for status, stdout, stderr in invalid:
        try:
            checker.check_result(module, status, stdout, stderr, source, generated, original, item)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse solver result')

    macro_item = next(control for control in controls if control.name == 'writer_comparison')
    macro_source, macro_generated, macro_original, macro_out, macro_err = inputs(macro_item.name, macro_item)
    macro_rows = [base.unique_json(line) for line in macro_err.splitlines()]
    macro_error = next(i for i, row in enumerate(macro_rows) if row['message'] == 'postcondition not satisfied')
    shared = macro_generated[Path(macro_item.path)].decode('ascii')
    shared_path = macro_original.parent / ('../' + macro_item.path)
    macro_invalid = []
    for part in ('body', 'invocation', 'definition'):
        changed = copy.deepcopy(macro_rows)
        target = next(span for span in changed[macro_error]['spans'] if span['is_primary'] is False)
        expansion = copy.deepcopy(target['expansion'])
        if part == 'invocation':
            offset = macro_source.index(macro_item.function)
            expansion['span'] = module.retained_parent.exact_span(macro_source, macro_original,
                offset, offset + len(macro_item.function), False)
        else:
            text = '$left.context_generation'
            offset = shared.index(text)
            if part == 'definition':
                expansion['def_site_span'] = module.retained_parent.exact_span(shared, shared_path,
                    offset, offset + len(text), False)
            else:
                target.clear()
                target.update(module.retained_parent.exact_span(shared, shared_path,
                    offset, offset + len(text), False, 'at the end of the function body'))
        target['expansion'] = expansion
        macro_invalid.append('\n'.join(map(json.dumps, changed)))
    for stderr in macro_invalid:
        try:
            checker.check_result(module, 1, macro_out, stderr, macro_source, macro_generated, macro_original, macro_item)
        except ValueError as error:
            checker.need(str(error) == 'exact proof postcondition and exit spans', 'macro span rejection')
        else:
            raise ValueError('accepted source-consistent wrong macro span')

    snapshots = {module.HERE.parent / p: (repo / checker.CRATE / p).read_bytes() for p in checker.PINS}
    with tempfile.TemporaryDirectory(prefix='fe2o3-representation-source-selftest-') as temporary:
        for path in snapshots:
            changed = dict(snapshots)
            changed[path] += b'\n'
            try:
                checker.stage(module, changed, Path(temporary) / path.name, None)
            except ValueError as error:
                checker.need(str(error).startswith('pinned representation source:'), 'intended source rejection')
            else:
                raise ValueError('accepted changed representation input')

    rejected_packets = 0
    if args.packet:
        command = [sys.executable, '-I', '-B', str(Path(__file__).resolve().with_name('audit.py')),
                   '--repo', str(repo), '--packet']
        pristine = subprocess.run([*command, str(args.packet.resolve())], capture_output=True, text=True, timeout=90)
        checker.need(pristine.returncode == 0, 'pristine packet: ' + pristine.stderr)
        failures = ['exact staged nested inputs', 'exact proof postcondition and exit spans',
                    'owned solver completion', 'exact verification result', 'proof roster',
                    'proof input identities', 'owned command completion']
        with tempfile.TemporaryDirectory(prefix='fe2o3-representation-packet-selftest-') as temporary:
            for index, failure in enumerate(failures):
                packet = Path(temporary) / str(index)
                shutil.copytree(args.packet, packet)
                if index == 0:
                    path = packet / 'proof/kind' / item.path
                    path.write_bytes(path.read_bytes() + b'\n')
                elif index == 1:
                    path = packet / 'proof/kind/solver/stderr.log'
                    path.write_text('\n'.join(map(json.dumps, changed_rows(rows, error_index))) + '\n')
                elif index == 2:
                    path = packet / 'proof/kind/solver/record.json'
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
    print(json.dumps({'adverse_solver_results_rejected': len(invalid) + len(macro_invalid), 'altered_inputs_rejected': len(snapshots),
                      'rehashed_packets_rejected': rejected_packets, 'known_negatives_rechecked': len(controls)}))


def changed_rows(rows, error_index):
    changed = copy.deepcopy(rows)
    changed[error_index]['spans'][0]['byte_start'] = 0
    return changed


if __name__ == '__main__':
    main()
