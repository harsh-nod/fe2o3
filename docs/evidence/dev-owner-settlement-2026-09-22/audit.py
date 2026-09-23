#!/usr/bin/env python3
"""Check developer record integrity against Git objects; does not rerun Verus."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
SOURCE = 'ba643eec26c8f8cc1598af56aff03bc5388e59b5'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-owner-settlement.py')


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def read(path):
    return json.loads(path.read_text(), object_pairs_hook=unique)


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args])


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def terminal(row, expected, command):
    need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}, 'receipt fields')
    need(type(row['status']) is int and row['status'] == expected and row['group_absent'] is True, 'normal terminal receipt')
    need(row['command'] == command, 'exact command')
    need(all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group')), 'receipt integers')
    need(row['started_ns'] <= row['finished_ns'], 'receipt time order')


def audit(repo, packet, quiet=False):
    need(not any(p.is_symlink() for p in packet.rglob('*')), 'no artifact symlinks')
    need((packet / 'audit.py').read_bytes() == Path(__file__).read_bytes(), 'running auditor identity')
    manifest = {}
    for line in (packet / 'SHA256SUMS').read_text().splitlines():
        digest, path = line.split('  ', 1)
        need(path not in manifest and not Path(path).is_absolute() and '..' not in Path(path).parts, 'manifest path')
        manifest[path] = digest
    files = {str(p.relative_to(packet)): p for p in packet.rglob('*') if p.is_file() and p != packet / 'SHA256SUMS'}
    need(set(manifest) == set(files), 'manifest roster')
    need(all(sha(files[p].read_bytes()) == h for p, h in manifest.items()), 'artifact hashes')
    records = packet / 'records'
    source = read(records / 'source.json')
    need(set(source) == {'commit', 'probe', 'inputs'} and source['commit'] == SOURCE and source['probe'] is False, 'signed-source campaign')
    need((packet / 'SOURCE').read_text() == SOURCE + '\n', 'source anchor')
    with tempfile.TemporaryDirectory(prefix='fe2o3-settlement-audit-') as temporary:
        tree = Path(temporary)
        for path, digest in source['inputs'].items():
            need(not Path(path).is_absolute() and '..' not in Path(path).parts, 'source path')
            data = git(repo, 'show', SOURCE + ':' + path)
            need(sha(data) == digest, 'Git source identity: ' + path)
            target = tree / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        check = module(tree / CHECK, 'enrollment_record_check')
        writer = module(tree / check.PREVIOUS, 'settlement_record_writer')
        previous = module(tree / writer.PREVIOUS, 'settlement_record_enrollment')
        ancestor = module(tree / previous.PREVIOUS, 'settlement_record_unknown')
        legacy = module(tree / check.LEGACY, 'enrollment_record_legacy')
        base = module(tree / legacy.BASE, 'enrollment_record_base')
        expected = set(legacy.SOURCES) | set(ancestor.NEW) | set(previous.NEW) | set(writer.NEW) | set(check.REUSED) | set(check.NEW) | set(writer.CPU_INPUTS) | set(legacy.PINS) | {
            check.LEGACY, check.PREVIOUS, writer.PREVIOUS, previous.PREVIOUS, check.CPU_PARSER, CHECK, check.CRATE / 'verus/context_version_journal_settlement_v1.rs',
            Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'),
            Path('docs/runtime-producer-read-reservations-v1.md'), Path('Cargo.toml'), Path('Cargo.lock'),
            Path('rust-toolchain.toml'), check.CRATE / 'Cargo.toml'}
        src = git(repo, 'ls-tree', '-rz', '--name-only', SOURCE, '--', str(check.CRATE / 'src'))
        expected.update(Path(p.decode()) for p in src.split(b'\0') if p)
        need(set(source['inputs']) == {str(p) for p in expected}, 'complete source roster')
        need(read(records / 'inputs-after.json') == source['inputs'], 'source bracket')
        for path, pin in legacy.PINS.items():
            need(sha((tree / path).read_bytes()) == pin, 'helper/closure pin')
        results = read(records / 'results.json')
        cases = [('positive-before', None), *[(m[0], m) for m in check.MUTATIONS], ('positive-after', None), ('writer-regression', None)]
        need(set(results) == {name for name, _ in cases}, 'exact proof case roster')
        verus = None
        receipts = {}
        for name, mutation in cases:
            folder = records / name
            row = read(folder / 'record.json')
            receipts[name] = row
            root = writer.ROOT if name == 'writer-regression' else check.RAW
            staged_root = Path(row['command'][-1])
            need(staged_root.is_absolute() and str(staged_root).endswith('/' + str(root)), 'proof root')
            stage = Path(str(staged_root)[:-len(str(root)) - 1])
            current_verus = Path(row['command'][5])
            need(current_verus.is_absolute() and (verus is None or current_verus == verus), 'same verifier')
            verus = current_verus
            terminal(row, 1 if mutation else 0, legacy.command(verus, staged_root, mutation))
            stdout, stderr = (folder / 'stdout.log').read_text(), (folder / 'stderr.log').read_text()
            observed = legacy.normalized(base, stdout, stderr, stage)
            need(legacy.same(observed, results[name]), 'record/result consistency')
            need(observed['verus']['version'] == '0.2026.08.09.92f466f'
                 and observed['verus']['toolchain'] == '1.97.1-x86_64-unknown-linux-gnu', 'verifier version')
            if mutation:
                result = observed['result']
                need(set(result) == {'encountered-error', 'encountered-vir-error', 'verified', 'errors',
                                     'is-verifying-entire-crate'}, 'closed negative result schema')
                need(result['encountered-error'] is True and result['encountered-vir-error'] is False
                     and result['is-verifying-entire-crate'] is False, 'typed negative result flags')
                need(type(result['verified']) is int and result['verified'] >= 0
                     and type(result['errors']) is int and result['errors'] > 0, 'nonnegative typed negative counts')
                legacy.check_result(base, row['status'], stdout, stderr, stage, mutation, results)
            else:
                need(not observed['diagnostics'], 'clean positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                    'success': True, 'verified': 845 if name == 'writer-regression' else 306,
                    'errors': 0, 'is-verifying-entire-crate': True}), 'whole-root proof result')
        need(legacy.same(results['positive-before'], results['positive-after']), 'matching whole-root positives')
        closure_command = None
        for phase in ('before', 'after'):
            row = read(records / ('closure-' + phase) / 'record.json')
            receipts['closure-' + phase] = row
            command = row['command']
            need(len(command) == 4 and command[0] == '/bin/sh'
                 and command[1].endswith('/' + str(legacy.CLOSURE))
                 and command[2] == str(verus.parent) and command[3].endswith('/' + str(legacy.MANIFEST)), 'closure command')
            script, manifest = Path(command[1]), Path(command[3])
            need(script.is_absolute() and manifest.is_absolute(), 'absolute closure paths')
            script_repo = command[1][:-len(str(legacy.CLOSURE)) - 1]
            manifest_repo = command[3][:-len(str(legacy.MANIFEST)) - 1]
            need(script_repo == manifest_repo and script_repo, 'common closure repository')
            need(closure_command is None or command == closure_command, 'identical closure bracket commands')
            closure_command = command
            terminal(row, 0, command)
            folder = records / ('closure-' + phase)
            need((folder / 'stdout.log').read_text() == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'closure transcript')
            need(not (folder / 'stderr.log').read_bytes(), 'clean closure diagnostics')
        commands = {
            'compiler': ['rustc', '+1.97.1', '-Vv'],
            'format': ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model'],
            'test': ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model'],
            'clippy': ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings'],
            'release-build': ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run'],
        }
        for name, command in commands.items():
            row = read(records / name / 'record.json')
            receipts[name] = row
            terminal(row, 0, command)
        order = ['closure-before', *[name for name, _ in cases], 'closure-after', *commands]
        for before, after in zip(order, order[1:]):
            need(receipts[before]['finished_ns'] <= receipts[after]['started_ns'], 'serial receipt order')
        old_audit = Path('docs/evidence/dev-producer-stable-2026-09-22/audit.py')
        target = tree / old_audit
        target.write_bytes(git(repo, 'show', SOURCE + ':' + str(old_audit)))
        parse = module(target, 'enrollment_cpu_result_parser').test_results
        need(parse((records / 'test/stdout.log').read_text()) == [(956, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'unit and doctest results')
        names = {name for name, _ in cases} | set(commands) | {'closure-before', 'closure-after'}
        expected_files = {'source.json', 'inputs-after.json', 'results.json'}
        expected_files.update(name + '/' + file for name in names for file in ('record.json', 'stdout.log', 'stderr.log'))
        need({str(p.relative_to(records)) for p in records.rglob('*') if p.is_file()} == expected_files, 'record file roster')
        need(set(files) == {'README.md', 'SOURCE', 'audit.py'} | {'records/' + p for p in expected_files}, 'packet file roster')
    if not quiet:
        print('PASS: Git source identities, scoped developer proof records, and CPU receipts')


def selftest(repo, packet):
    cases = ('probe', 'source_roster', 'positive_count', 'positive_type', 'negative_scope', 'cpu_count',
             'closure_transcript', 'closure_prefix', 'receipt_order', 'auditor_identity',
             'negative_type', 'negative_count', 'negative_roster')
    for case in cases:
        with tempfile.TemporaryDirectory(prefix='fe2o3-settlement-audit-test-') as temporary:
            changed = Path(temporary) / 'packet'
            shutil.copytree(packet, changed)
            records = changed / 'records'
            if case in ('probe', 'source_roster'):
                path = records / 'source.json'
                row = read(path)
                if case == 'probe':
                    row['probe'] = True
                else:
                    del row['inputs']['docs/runtime-producer-read-reservations-v1.md']
                    (records / 'inputs-after.json').write_text(json.dumps(row['inputs']))
                path.write_text(json.dumps(row))
            elif case in ('positive_count', 'positive_type'):
                path = records / 'positive-before/stdout.log'
                row = read(path)
                row['verification-results']['verified' if case == 'positive_count' else 'success'] = 0 if case == 'positive_count' else 1
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results['positive-before']['result'] = row['verification-results']
                path.write_text(json.dumps(results))
            elif case in ('negative_type', 'negative_count', 'negative_roster'):
                path = records / 'evidence/stdout.log'
                row = read(path)
                field, value = {'negative_type': ('encountered-error', 1), 'negative_count': ('verified', -1),
                                'negative_roster': ('unexpected', False)}[case]
                row['verification-results'][field] = value
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results['evidence']['result'] = row['verification-results']
                path.write_text(json.dumps(results))
            elif case == 'negative_scope':
                path = records / 'evidence/record.json'
                row = read(path)
                index = row['command'].index('--verify-function') + 1
                row['command'][index] = 'owner_settlement_nonempty_raw_witness_v1'
                path.write_text(json.dumps(row))
            elif case == 'cpu_count':
                path = records / 'test/stdout.log'
                text = path.read_text()
                need(text.count('956 passed;') == 1, 'CPU selftest anchor')
                path.write_text(text.replace('956 passed;', '955 passed;'))
            elif case == 'closure_transcript':
                (records / 'closure-before/stdout.log').write_text('')
            elif case == 'closure_prefix':
                path = records / 'closure-after/record.json'
                row = read(path)
                row['command'][3] = '/different-repository/' + row['command'][3].lstrip('/')
                path.write_text(json.dumps(row))
            elif case == 'receipt_order':
                path = records / 'positive-after/record.json'
                row = read(path)
                row['started_ns'] = read(records / 'positive-before/record.json')['started_ns']
                path.write_text(json.dumps(row))
            else:
                path = changed / 'audit.py'
                path.write_text(path.read_text() + '\n# changed packaged auditor\n')
            paths = sorted(p for p in changed.rglob('*') if p.is_file() and p != changed / 'SHA256SUMS')
            (changed / 'SHA256SUMS').write_text(''.join(sha(p.read_bytes()) + '  ' + str(p.relative_to(changed)) + '\n' for p in paths))
            try:
                audit(repo, changed, quiet=True)
            except ValueError:
                continue
            raise ValueError('accepted rehashed mutation: ' + case)
    print(f'PASS: {len(cases)} rehashed malformed record sets rejected')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).parent)
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args()
    audit(args.repo.resolve(), args.packet.resolve())
    if args.selftest:
        selftest(args.repo.resolve(), args.packet.resolve())
