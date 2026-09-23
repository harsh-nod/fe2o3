#!/usr/bin/env python3
"""Check developer retirement records against Git objects; does not rerun Verus."""
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
SOURCE = 'adae0c1fc2eba6ad96a24bf1970af1d5bfb3e752'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-owner-retirement.py')


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
    need(set(source) == {'commit', 'probe', 'inputs'} and source['commit'] == SOURCE and source['probe'] is False, 'source-bound campaign')
    need((packet / 'SOURCE').read_text() == SOURCE + '\n', 'source anchor')
    with tempfile.TemporaryDirectory(prefix='fe2o3-retirement-audit-') as temporary:
        tree = Path(temporary)
        for path, digest in source['inputs'].items():
            need(not Path(path).is_absolute() and '..' not in Path(path).parts, 'source path')
            data = git(repo, 'show', SOURCE + ':' + path)
            need(sha(data) == digest, 'Git source identity: ' + path)
            target = tree / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        check = module(tree / CHECK, 'enrollment_record_check')
        scalar = module(tree / check.PREVIOUS, 'retirement_record_scalar')
        history = module(tree / scalar.PREVIOUS, 'retirement_record_history')
        settlement = module(tree / history.PREVIOUS, 'settlement_record_raw')
        writer = module(tree / settlement.PREVIOUS, 'settlement_record_writer')
        previous = module(tree / writer.PREVIOUS, 'settlement_record_enrollment')
        ancestor = module(tree / previous.PREVIOUS, 'settlement_record_unknown')
        legacy = module(tree / settlement.LEGACY, 'enrollment_record_legacy')
        base = module(tree / legacy.BASE, 'enrollment_record_base')
        frozen = json.loads(git(repo, 'show', check.FROZEN + ':' + str(check.FROZEN_INPUTS)), object_pairs_hook=unique)
        inherited = {Path(p) for p in frozen['inputs']} | {check.FROZEN_INPUTS}
        expected = inherited | set(check.NEW) | {CHECK}
        need(set(source['inputs']) == {str(p) for p in expected}, 'complete source roster')
        committed = {Path(p.decode()) for p in git(repo, 'ls-tree', '-rz', '--name-only', SOURCE, '--', str(check.CRATE)).split(b'\0') if p}
        need({p for p in committed if p.is_relative_to(check.CRATE / 'src')}
             == {p for p in expected if p.is_relative_to(check.CRATE / 'src')}, 'committed runtime source roster')
        need(all(p == check.CRATE / 'Cargo.toml' or p.is_relative_to(check.CRATE / 'src')
                 or p.is_relative_to(check.CRATE / 'verus') for p in committed), 'committed Cargo discovery roster')
        check.authenticate(tree, inherited, scalar, git_repo=repo)
        need(read(records / 'inputs-after.json') == source['inputs'], 'source bracket')
        for path, pin in legacy.PINS.items():
            need(sha((tree / path).read_bytes()) == pin, 'helper/closure pin')
        results = read(records / 'results.json')
        cases = [('positive-before', None), *[(m[0], m) for m in check.MUTATIONS], ('positive-after', None), ('raw-regression', None), ('scalar-regression', None)]
        need(set(results) == {name for name, _ in cases}, 'exact proof case roster')
        verus = campaign_root = None
        receipts = {}
        for name, mutation in cases:
            folder = records / name
            row = read(folder / 'record.json')
            receipts[name] = row
            root = check.RAW if name == 'raw-regression' else scalar.ROOT if name == 'scalar-regression' else check.ROOT
            staged_root = Path(row['command'][-1])
            need(staged_root.is_absolute() and str(staged_root).endswith('/' + str(root)), 'proof root')
            stage = Path(str(staged_root)[:-len(str(root)) - 1])
            need(stage.name.startswith('sources-') and (campaign_root is None or stage.parent == campaign_root), 'common owned source staging')
            campaign_root = stage.parent
            current_verus = Path(row['command'][5])
            need(current_verus.is_absolute() and (verus is None or current_verus == verus), 'same verifier')
            verus = current_verus
            terminal(row, 1 if mutation else 0, legacy.command(verus, staged_root, mutation))
            stdout, stderr = (folder / 'stdout.log').read_text(), (folder / 'stderr.log').read_text()
            observed = legacy.normalized(base, stdout, stderr, stage)
            need(legacy.same(observed, results[name]), 'record/result consistency')
            scalar.check_verifier(observed['verus'])
            if mutation:
                check.check_negative(scalar, name, row['status'], observed)
            else:
                need(not observed['diagnostics'], 'clean positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                    'success': True, 'verified': 331 if name == 'raw-regression' else 929 if name == 'scalar-regression' else check.PAIRED_VERIFIED,
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
            'recorder-test': ['python3', '-B', script_repo + '/' + str(CHECK), '--selftest'],
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
        need(legacy.same(read(records / 'accepted.json'), [
            {'name': name, 'files': {file: sha((records / name / file).read_bytes())
             for file in ('record.json', 'stdout.log', 'stderr.log')}} for name in order]), 'complete controller acceptance')
        for before, after in zip(order, order[1:]):
            need(receipts[before]['finished_ns'] <= receipts[after]['started_ns'], 'serial receipt order')
        old_audit = Path('docs/evidence/dev-producer-stable-2026-09-22/audit.py')
        target = tree / old_audit
        target.write_bytes(git(repo, 'show', SOURCE + ':' + str(old_audit)))
        parse = module(target, 'enrollment_cpu_result_parser').test_results
        need(parse((records / 'test/stdout.log').read_text()) == [(980, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'unit and doctest results')
        need((records / 'recorder-test/stdout.log').read_text() ==
             'PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
             'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
             and not (records / 'recorder-test/stderr.log').read_bytes(), 'recorder regression inventory')
        names = {name for name, _ in cases} | set(commands) | {'closure-before', 'closure-after'}
        for name in names:
            need({p.name for p in (records / name).iterdir()} == {'record.json', 'stdout.log', 'stderr.log'}, 'case entry roster')
        expected_files = {'source.json', 'accepted.json', 'inputs-after.json', 'results.json'}
        need({p.name for p in records.iterdir()} == expected_files | names, 'record root roster')
        expected_files.update(name + '/' + file for name in names for file in ('record.json', 'stdout.log', 'stderr.log'))
        need({str(p.relative_to(records)) for p in records.rglob('*') if p.is_file()} == expected_files, 'record file roster')
        need(set(files) == {'README.md', 'SOURCE', 'audit.py'} | {'records/' + p for p in expected_files}, 'packet file roster')
    if not quiet:
        print('PASS: Git source identities, scoped developer proof records, and CPU receipts')


def selftest(repo, packet):
    cases = ('probe', 'source_roster', 'positive_count', 'positive_type', 'negative_scope', 'cpu_count',
             'closure_transcript', 'closure_prefix', 'receipt_order', 'auditor_identity',
             'negative_type', 'negative_count', 'negative_roster', 'paired_root', 'model_input',
             'acceptance_missing', 'acceptance_order', 'acceptance_hash', 'verifier_metadata', 'recorder_test', 'extra_stage', 'extra_case_directory',
             'diagnostic_wrong_case', 'diagnostic_resource', 'diagnostic_frontend', 'diagnostic_abort_only')
    for case in cases:
        with tempfile.TemporaryDirectory(prefix='fe2o3-retirement-audit-test-') as temporary:
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
                path = records / 'roster_bound/stdout.log'
                row = read(path)
                field, value = {'negative_type': ('encountered-error', 1), 'negative_count': ('verified', -1),
                                'negative_roster': ('unexpected', False)}[case]
                row['verification-results'][field] = value
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results['roster_bound']['result'] = row['verification-results']
                path.write_text(json.dumps(results))
            elif case.startswith('diagnostic_'):
                name = 'roster_bound' if case == 'diagnostic_wrong_case' else 'unchecked_return'
                path = records / name / 'stderr.log'
                rows = [json.loads(line, object_pairs_hook=unique) for line in path.read_text().splitlines()]
                message = {'diagnostic_wrong_case': 'possible arithmetic underflow/overflow',
                           'diagnostic_resource': 'function body check: Resource limit (rlimit) exceeded',
                           'diagnostic_frontend': 'unexpected token',
                           'diagnostic_abort_only': 'aborting due to 1 previous error'}[case]
                if case == 'diagnostic_abort_only':
                    rows = []
                rows.append({'level': 'error', 'message': message})
                path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
                root = 'crates/fe2o3-runtime-model/verus/context_owner_retirement_paired_v1.rs'
                stage = read(records / name / 'record.json')['command'][-1][:-len(root) - 1]
                normalized = json.loads(json.dumps(rows).replace(stage, '<CASE>'), object_pairs_hook=unique)
                normalized.sort(key=lambda row: json.dumps(row, sort_keys=True))
                path = records / 'results.json'
                results = read(path)
                results[name]['diagnostics'] = normalized
                path.write_text(json.dumps(results))
            elif case == 'negative_scope':
                path = records / 'roster_bound/record.json'
                row = read(path)
                index = row['command'].index('--verify-function') + 1
                row['command'][index] = 'owner_retirement_live_witness_v1'
                path.write_text(json.dumps(row))
            elif case == 'paired_root':
                path = records / 'positive-before/record.json'
                row = read(path)
                row['command'][-1] = row['command'][-1].replace(
                    'context_owner_retirement_paired_v1.rs', 'context_owner_retirement_execution_v1.rs')
                path.write_text(json.dumps(row))
            elif case == 'model_input':
                path = records / 'source.json'
                row = read(path)
                del row['inputs']['crates/fe2o3-runtime-model/verus/context_owner_retirement_model_v1.rs']
                path.write_text(json.dumps(row))
                (records / 'inputs-after.json').write_text(json.dumps(row['inputs']))
            elif case == 'cpu_count':
                path = records / 'test/stdout.log'
                text = path.read_text()
                need(text.count('980 passed;') == 1, 'CPU selftest anchor')
                path.write_text(text.replace('980 passed;', '979 passed;'))
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
            elif case == 'acceptance_missing':
                entries = read(records / 'accepted.json')
                (records / 'accepted.json').write_text(json.dumps(entries[:-1]))
            elif case == 'acceptance_order':
                entries = read(records / 'accepted.json')
                entries.reverse()
                (records / 'accepted.json').write_text(json.dumps(entries))
            elif case == 'acceptance_hash':
                entries = read(records / 'accepted.json')
                entries[-1]['files']['record.json'] = '0' * 64
                (records / 'accepted.json').write_text(json.dumps(entries))
            elif case == 'verifier_metadata':
                path = records / 'positive-after/stdout.log'
                row = read(path)
                row['verus']['profile'] = 'other'
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results['positive-after']['verus'] = row['verus']
                path.write_text(json.dumps(results))
            elif case == 'recorder_test':
                (records / 'recorder-test/stdout.log').write_text('PASS\n')
            elif case == 'extra_stage':
                (records / 'sources-interrupted').mkdir()
            elif case == 'extra_case_directory':
                (records / 'positive-before/unexpected').mkdir()
            else:
                path = changed / 'audit.py'
                path.write_text(path.read_text() + '\n# changed packaged auditor\n')
            if not case.startswith('acceptance_'):
                entries = read(records / 'accepted.json')
                for entry in entries:
                    entry['files'] = {file: sha((records / entry['name'] / file).read_bytes())
                                      for file in ('record.json', 'stdout.log', 'stderr.log')}
                (records / 'accepted.json').write_text(json.dumps(entries))
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
