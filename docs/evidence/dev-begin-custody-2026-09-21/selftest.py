#!/usr/bin/env python3
"""Reject adverse actual solver diagnostics and unauthenticated guard projections."""
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
    spec = importlib.util.spec_from_file_location('custody_diagnostic_selftest', path)
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
    guard = (good / checker.GUARDS).read_bytes()
    lifecycle = (good / checker.LIFECYCLE).read_bytes()
    adverse_sources = [
        (source.replace('mod begin_reader_guards;', 'mod unrelated;'), guard, lifecycle),
        (source + '\ninclude!("extra.rs");\n', guard, lifecycle),
        (source, guard.replace(b'use super::*;', b'use unrelated::*;'), lifecycle),
        (source, guard.replace(b'<= 2_097_152', b'<= 2_097_153'), lifecycle),
        (source, guard + b'\n', lifecycle),
        (source, guard, lifecycle + b'\n'),
    ]
    with tempfile.TemporaryDirectory(prefix='fe2o3-begin-guard-selftest-') as temporary:
        folder = Path(temporary) / 'case'
        shutil.copytree(good, folder)
        checker.audit_source(module, source, folder)
        for changed, guards, authority in adverse_sources:
            (folder / checker.GUARDS).write_bytes(guards)
            (folder / checker.LIFECYCLE).write_bytes(authority)
            try:
                checker.audit_source(module, changed, folder)
            except ValueError:
                pass
            else:
                raise ValueError('accepted unauthenticated guard projection')
    print(json.dumps({'adverse_solver_results_rejected': len(invalid),
                      'adverse_source_projections_rejected': len(adverse_sources),
                      'known_negative_rechecked': mutation.name}))


if __name__ == '__main__':
    main()
