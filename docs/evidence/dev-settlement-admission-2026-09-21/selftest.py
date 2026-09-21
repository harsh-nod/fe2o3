#!/usr/bin/env python3
"""Reject adverse actual solver diagnostics and unauthenticated settlement inputs."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    args = parser.parse_args()
    path = Path(__file__).resolve().with_name('check.py')
    spec = importlib.util.spec_from_file_location('settlement_diagnostic_selftest', path)
    checker = importlib.util.module_from_spec(spec)
    exec(compile(path.read_bytes(), str(path), 'exec'), checker.__dict__)
    module = checker.load(args.repo.resolve())
    base = module.inherited().BASE
    mutation = checker.mutations(base)[0]
    case = args.proof / mutation.name
    source = (case / checker.SOURCE).read_text()
    record = json.loads((case / 'solver/record.json').read_text())
    original = Path(record['command'][-1])
    out = (case / 'solver/stdout.log').read_text()
    err = (case / 'solver/stderr.log').read_text()
    checker.check_result(module, 1, out, err, source, original, mutation)
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
    for field, value in [('file_name', '/foreign.rs'), ('byte_start', 0), ('line_start', 1),
                         ('is_primary', 1), ('label', 'different clause'), ('expansion', {})]:
        changed = copy.deepcopy(diagnostics)
        next(span for span in changed[0]['spans'] if span['is_primary'])[field] = value
        invalid.append((1, out, '\n'.join(map(json.dumps, changed))))
    invalid.extend([(1, out + out, err), (1, out, ''), (1, out, err + '\nsolver timed out'), (1, out, err + err)])
    for status, stdout, stderr in invalid:
        try:
            checker.check_result(module, status, stdout, stderr, source, original, mutation)
        except (ValueError, KeyError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse solver result')
    good = args.proof / 'positive_before'
    source = (good / checker.SOURCE).read_text()
    prior = module.custody_checker
    with tempfile.TemporaryDirectory(prefix='fe2o3-settlement-source-selftest-') as temporary:
        for index in range(5):
            folder = Path(temporary) / str(index)
            shutil.copytree(good, folder)
            changed = source
            if index == 1:
                changed = source.replace('include!("context_version_journal_begin_custody_v1.rs");', 'include!("foreign.rs");')
            elif index == 2:
                changed = source + '\ninclude!("foreign.rs");\n'
            elif index == 3:
                target = folder / prior.SOURCE
                target.write_bytes(target.read_bytes() + b'\n')
            elif index == 4:
                target = folder / prior.GUARDS
                target.write_bytes(target.read_bytes().replace(b'<= 2_097_152', b'<= 2_097_153'))
            try:
                checker.audit_source(module, changed, folder)
            except ValueError:
                if index == 0:
                    raise
            else:
                if index != 0:
                    raise ValueError('accepted unauthenticated settlement input')
    print(json.dumps({'adverse_solver_results_rejected': len(invalid), 'adverse_source_inputs_rejected': 4,
                      'known_negative_rechecked': mutation.name}))


if __name__ == '__main__':
    main()
