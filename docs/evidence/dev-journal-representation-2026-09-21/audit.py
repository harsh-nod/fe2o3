#!/usr/bin/env python3
"""Offline Git-bound journal views, scoped diagnostics, process and CPU receipt audit."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
PACKET = Path('docs/evidence/dev-journal-representation-2026-09-21')
PARENT = Path('docs/evidence/dev-begin-execution-2026-09-21/audit.py')
PARENT_SHA = '03372f38686d1ecf9dbed9a3962e51dbbaae5588c039796dd77f47ae215cf18f'
ORIGINAL = Path('/home/harsh/.codex-tmp/fe2o3-journal-representation-20260921.ZHZP6795/proof-final')
STATIC = {'README.md', 'audit.py', 'check.py', 'selftest.py', 'cargo-checks.py'}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(name, path, data):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def blob(repo, source, path):
    return subprocess.check_output(['git', 'cat-file', 'blob', f'{source}:{path}'], cwd=repo)


def proof_checks(packet, old, parent, helper, model, checker, contents, extra, temporary):
    campaign = packet / 'proof'
    inputs = {**parent.expected_inputs(contents), **{str(old.ROOT / p): sha(data) for p, data in extra.items()}}
    need(helper.unique_json(campaign / 'inputs-before.json') == inputs
         and helper.unique_json(campaign / 'inputs-after.json') == inputs, 'proof input identities')
    need(helper.unique_json(campaign / 'environment.json') == {**old.ENV, 'VERUS_Z3_PATH': str(parent.VERUS.parent / 'z3')}, 'proof environment')
    actual, expected = helper.unique_json(campaign / 'report.json'), checker.report()
    need(actual == expected and all(type(actual[k]) is type(v) for k, v in expected.items()), 'exact proof report')
    base = model.inherited().BASE
    cases = [('positive_before', None), *[(m.name, m) for m in checker.mutations(base)], ('positive_after', None)]
    need(helper.entries(campaign) == {'inputs-before.json', 'inputs-after.json', 'environment.json',
        'completed-cases.json', 'report.json', 'closure-before', 'closure-after', *(n for n, _ in cases)}, 'proof roster')
    need(all(sha(extra[old.PROOF.parent / path]) == pin for path, pin in checker.PINS.items()), 'representation source identities')
    snapshots = {temporary / p: data for p, data in {**contents, **extra}.items()}
    completed = {}
    for name, mutation in cases:
        folder, original = campaign / name, ORIGINAL / name
        source, body, generated = checker.stage(model, snapshots, temporary / 'audits' / name, mutation)
        need(helper.entries(folder) == {'verus', 'src', 'sources.json', 'solver'}, 'case root roster')
        files = {str(p.relative_to(folder)) for p in folder.rglob('*') if p.is_file()}
        need(files == {str(p) for p in generated} | {'sources.json', 'solver/record.json',
             'solver/stdout.log', 'solver/stderr.log'}, 'recursive case file roster')
        need(all(helper.read(folder / p) == data for p, data in generated.items()), 'exact staged nested inputs')
        need(helper.unique_json(folder / 'sources.json') == {str(original / p): sha(data) for p, data in generated.items()}, 'case identities')
        path = original / 'verus' / checker.SOURCE
        command = checker.solver_command(parent.VERUS, path, mutation)
        record = helper.solver_record(folder / 'solver', command, 1 if mutation else 0)
        checker.check_result(model, record['status'], helper.read(folder / 'solver/stdout.log').decode(),
            helper.read(folder / 'solver/stderr.log').decode(), source, body, path, mutation)
        completed[name] = {str(original / p.relative_to(folder)): sha(p.read_bytes()) for p in folder.rglob('*') if p.is_file()}
    need(helper.unique_json(campaign / 'completed-cases.json') == completed, 'frozen completed cases')
    for phase in ('before', 'after'):
        folder = campaign / ('closure-' + phase)
        command = ['/bin/sh', str(old.ROOT / parent.CLOSURE), str(parent.VERUS.parent), str(old.ROOT / old.PROOF / 'pins/VERUS_CLOSURE_MANIFEST')]
        helper.solver_record(folder, command, 0)
        need(helper.read(folder / 'stdout.log') == b'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n'
             and not helper.read(folder / 'stderr.log'), 'Verus closure receipts')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    repo, packet = args.repo.resolve(), args.packet.resolve()
    source = (packet / 'SOURCE').read_text().strip()
    need(re.fullmatch('[0-9a-f]{40}', source), 'source commit')
    data = blob(repo, source, PARENT)
    need(sha(data) == PARENT_SHA, 'pinned raw Begin auditor')
    old = load('journal_representation_audit_parent', PARENT, data)
    old.manifest(packet)
    need({p.name for p in packet.iterdir()} == STATIC | {'SOURCE', 'SHA256SUMS', 'proof', 'cargo'}, 'packet root roster')
    need(all((packet / name).read_bytes() == blob(repo, source, PACKET / name) for name in STATIC), 'static packet source binding')
    with tempfile.TemporaryDirectory(prefix='fe2o3-journal-representation-audit-') as scratch:
        temporary = Path(scratch)
        data = blob(repo, source, old.PARENT)
        need(sha(data) == old.PARENT_SHA, 'pinned enrollment archive helpers')
        parent = load('journal_representation_enrollment_parent', temporary / 'parent.py', data)
        contents, inherited_model = parent.signed_sources(repo, temporary)
        check_path = PACKET / 'check.py'
        checker = load('journal_representation_checked_source', temporary / check_path, blob(repo, source, check_path))
        extra = {}
        def materialize(path):
            extra[path] = blob(repo, source, path)
            target = temporary / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(extra[path])
        for path in [check_path, checker.PRIOR,
                     Path('docs/evidence/dev-shared-settlement-scratch-2026-09-21/check.py'),
                     Path('docs/evidence/dev-shared-retained-2026-09-21/check.py'),
                     Path('docs/evidence/dev-shared-settlement-storage-2026-09-21/check.py'),
                     Path('docs/evidence/dev-settlement-commit-2026-09-21/check.py'),
                     Path('docs/evidence/dev-settlement-admission-2026-09-21/check.py'),
                     Path('docs/evidence/dev-begin-custody-2026-09-21/check.py'),
                     Path('docs/evidence/dev-begin-execution-2026-09-21/check.py')]:
            materialize(path)
        model = checker.load(temporary)
        for path in checker.extra_paths(model):
            materialize(path)
        helper = inherited_model.archive_helpers
        proof_checks(packet, old, parent, helper, model, checker, contents, extra, temporary)
        commands = {'compiler': ['rustc', '-Vv'],
            'format': ['cargo', 'fmt', '--check', '-p', 'fe2o3-runtime-model'],
            'test': ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model'],
            'clippy': ['cargo', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']}
        need(helper.entries(packet / 'cargo') == {*commands, 'inputs-before.json', 'inputs-after.json'}, 'Cargo command roster')
        for name, command in commands.items():
            old.command_receipt(helper, packet / 'cargo' / name, command, old.ROOT, 300)
        summaries = [line for line in helper.read(packet / 'cargo/test/stdout').decode().splitlines() if line.startswith('test result:')]
        patterns = [r'test result: ok\. 850 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s',
                    r'test result: ok\. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s']
        need(len(summaries) == len(patterns) and all(re.fullmatch(p, s) for p, s in zip(patterns, summaries)), 'exact terminal test summaries')
        cargo_inputs = helper.unique_json(packet / 'cargo/inputs-before.json')
        need(cargo_inputs == helper.unique_json(packet / 'cargo/inputs-after.json'), 'Cargo source brackets')
        source_names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', source,
            'crates/fe2o3-runtime-model/src'], cwd=repo, text=True).splitlines()
        paths = [*source_names, 'Cargo.toml', 'Cargo.lock', 'crates/fe2o3-runtime-model/Cargo.toml',
            'crates/fe2o3-runtime/src/context.rs', 'docs/evidence/dev-journal-representation-2026-09-21/cargo-checks.py',
            'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py',
            'crates/fe2o3-runtime-model/verus/context_journal_representation_v1.rs']
        need(set(cargo_inputs) == {str(old.ROOT / p) for p in paths}, 'exact Cargo input roster')
        need(all(Path(p).is_relative_to(old.ROOT) and h == sha(blob(repo, source, Path(p).relative_to(old.ROOT)))
                 for p, h in cargo_inputs.items()), 'Cargo input Git identities')
    print('PASS: development shared declarations, exact views, scoped mutation diagnostics and CPU receipts')


if __name__ == '__main__':
    main()
