#!/usr/bin/env python3
"""Audit source-bound developer lifecycle records; does not rerun Verus."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
SOURCE = '0da85d666e17988d65cce7a30c7ee67319487e45'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py')
DOCUMENT = Path('docs/runtime-producer-read-reservations-v1.md')
DOCUMENT_SHA256 = '8382d19dc9a23e6399e723275c8bd454d89745f2593f078b0ab5980fc8ab9f3a'
SOURCE_COUNT = 449
NEGATIVE_COUNT = 49
COMMAND_COUNT = 62
RECORD_COUNT = 190
PAIRED_VERIFIED = 1240
RAW_VERIFIED = 384
INSPECTION_VERIFIED = 1179
CONSTRUCTOR_VERIFIED = 1156
WHOLE_TIMEOUT_SECONDS = 900
SCOPED_TIMEOUT_SECONDS = 600
CPU_TEST_RESULTS = [(1021, 0, 18, 0, 0), (27, 0, 0, 0, 0)]
RECORDER_TRANSCRIPT = (
    'PASS: lifecycle bootstrap; 18 helper controls, 5 ambient Git controls, replacement-ref isolation\n'
    'PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
    'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
    'PASS: inspection wall-time budgets; 3 whole-root and 25 scoped command shapes checked; '
    '5 malformed legacy prefixes rejected\n'
    'PASS: inspection diagnostic, independence, and source-auth checks; 36 malformed cases rejected; '
    '25 scoped mutation anchors authenticated\n'
    'PASS: frozen inspection recorder; exact signed-parent sources and transcript; no process-group escape\n'
    'PASS: lifecycle inherited proof edits; 15 rehashed changes rejected; original contracts unchanged\n'
    'PASS: lifecycle source-policy controls; 19 rehashed changes rejected\n'
    'PASS: lifecycle inherited/root/checker/roster authentication; 6 changes rejected\n'
    'PASS: lifecycle logical mutation anchors; 49 scoped controls authenticated\n'
)

SELFTEST_CASES = (
    'probe', 'source_roster', 'positive_count', 'positive_type', 'negative_scope', 'cpu_count',
    'closure_transcript', 'closure_prefix', 'receipt_order', 'auditor_identity',
    'negative_type', 'negative_count', 'negative_roster', 'paired_root', 'model_input',
    'acceptance_missing', 'acceptance_order', 'acceptance_hash', 'verifier_metadata', 'recorder_test',
    'extra_stage', 'extra_case_directory', 'diagnostic_arithmetic', 'diagnostic_resource',
    'diagnostic_frontend', 'diagnostic_abort_only', 'document_hash', 'diagnostic_initial_typo',
    'raw_count', 'constructor_count', 'negative_terminal', 'incomplete_group', 'extra_packet_directory',
    'whole_timeout', 'inspection_count', 'verifier_basename',
    'raw_root', 'inspection_root', 'constructor_root',
)


def need(condition, message):
    if not condition:
        raise ValueError(message)


def configuration():
    need(re.fullmatch(r'[0-9a-f]{40}', SOURCE) is not None, 'final source commit required')
    need(all(type(count) is int and count > 0 for count in
             (PAIRED_VERIFIED, RAW_VERIFIED, INSPECTION_VERIFIED, CONSTRUCTOR_VERIFIED, NEGATIVE_COUNT, COMMAND_COUNT, RECORD_COUNT)), 'final measured proof inventories required')


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


def git_environment():
    return {'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'GIT_NO_REPLACE_OBJECTS': '1',
            'GIT_OPTIONAL_LOCKS': '0', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null'}


def git(repo, *args):
    return subprocess.check_output(['/usr/bin/git', '-C', str(repo), *args], env=git_environment())


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


def inherited_roster(check, inherited):
    need(len(inherited) == check.INHERITED_INPUT_COUNT == 437, 'exact inherited inspection source roster')
    need({path for path in inherited if path.suffix == '.py'} == set(check.HELPER_PINS),
         'exact inherited Python helper roster')


def source_tree(repo, source, tree):
    configuration()
    need(set(source) == {'commit', 'probe', 'inputs'} and source['commit'] == SOURCE
         and source['probe'] is False, 'source-bound campaign')
    need(len(source['inputs']) == SOURCE_COUNT, 'exact source input count')
    for path, digest in source['inputs'].items():
        need(not Path(path).is_absolute() and '..' not in Path(path).parts, 'source path')
        data = git(repo, 'show', SOURCE + ':' + path)
        need(sha(data) == digest, 'Git source identity: ' + path)
        target = tree / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    check = module(tree / CHECK, 'lifecycle_record_check')
    deps = check.dependencies(tree)
    scalar, legacy, base = (deps[name] for name in ('scalar', 'legacy', 'base'))
    inspection = check.INSPECTION
    frozen_inputs = git(repo, 'show', inspection.FROZEN + ':' + str(inspection.FROZEN_INPUTS)).decode()
    inherited = {Path(p) for p in base.unique_json(frozen_inputs)['inputs']}
    inherited |= {inspection.FROZEN_INPUTS, check.PREVIOUS} | set(inspection.NEW)
    inherited_roster(check, inherited)
    expected = inherited | set(check.NEW) | {CHECK}
    need(set(source['inputs']) == {str(p) for p in expected}, 'complete source roster')
    committed = {Path(p.decode()) for p in git(repo, 'ls-tree', '-rz', '--name-only', SOURCE, '--', str(check.CRATE)).split(b'\0') if p}
    need({p for p in committed if p.is_relative_to(check.CRATE / 'src')}
         == {p for p in expected if p.is_relative_to(check.CRATE / 'src')}, 'committed runtime source roster')
    need(all(p == check.CRATE / 'Cargo.toml' or p.is_relative_to(check.CRATE / 'src')
             or p.is_relative_to(check.CRATE / 'verus') for p in committed), 'committed Cargo discovery roster')
    check.configuration()
    need((check.PAIRED_VERIFIED, check.RAW_VERIFIED, check.INSPECTION_VERIFIED, check.CONSTRUCTOR_VERIFIED)
         == (PAIRED_VERIFIED, RAW_VERIFIED, INSPECTION_VERIFIED, CONSTRUCTOR_VERIFIED), 'pinned proof inventories')
    need(check.CPU_TEST_RESULTS == CPU_TEST_RESULTS, 'pinned CPU inventory')
    need((check.WHOLE_TIMEOUT_SECONDS, check.SCOPED_TIMEOUT_SECONDS)
         == (WHOLE_TIMEOUT_SECONDS, SCOPED_TIMEOUT_SECONDS), 'pinned proof wall-time budgets')
    need(inspection.DOCUMENT_PINS == {DOCUMENT: DOCUMENT_SHA256}, 'pinned reviewed document roster')
    need(source['inputs'][str(DOCUMENT)] == DOCUMENT_SHA256, 'reviewed document source identity')
    need(check.recorder_transcript(NEGATIVE_COUNT) == RECORDER_TRANSCRIPT, 'pinned recorder selftest inventory')
    check.authenticate(tree, inherited, deps, git_repo=repo)
    for path, pin in legacy.PINS.items():
        need(sha((tree / path).read_bytes()) == pin, 'helper/closure pin')
    check.scan_sources(tree, deps)
    return check, deps, inherited


def audit(repo, packet, quiet=False):
    configuration()
    need(not any(p.is_symlink() for p in packet.rglob('*')), 'no artifact symlinks')
    need({p.name for p in packet.iterdir()} == {'README.md', 'SOURCE', 'audit.py', 'SHA256SUMS', 'records'}, 'packet root roster')
    need((packet / 'audit.py').read_bytes() == Path(__file__).read_bytes(), 'running auditor identity')
    manifest = {}
    for line in (packet / 'SHA256SUMS').read_text().splitlines():
        digest, path = line.split('  ', 1)
        need(path not in manifest and not Path(path).is_absolute() and '..' not in Path(path).parts, 'manifest path')
        manifest[path] = digest
    files = {str(p.relative_to(packet)): p for p in packet.rglob('*') if p.is_file() and p != packet / 'SHA256SUMS'}
    need(len(files) == RECORD_COUNT + 3 and set(manifest) == set(files), 'manifest roster')
    need(all(sha(files[p].read_bytes()) == h for p, h in manifest.items()), 'artifact hashes')
    records = packet / 'records'
    source = read(records / 'source.json')
    need((packet / 'SOURCE').read_text() == SOURCE + '\n', 'source anchor')
    with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-audit-') as temporary:
        tree = Path(temporary)
        check, deps, _ = source_tree(repo, source, tree)
        scalar, legacy, base, constructor = (deps[name] for name in ('scalar', 'legacy', 'base', 'constructor'))
        need(read(records / 'inputs-after.json') == source['inputs'], 'source bracket')
        results = read(records / 'results.json')
        mutations = check.mutations(tree, deps['policy'])
        need(len(mutations) == NEGATIVE_COUNT
             and len({change[0] for change in mutations}) == NEGATIVE_COUNT, 'exact negative control inventory')
        cases = [('positive-before', None), *[(m[0], m) for m in mutations], ('positive-after', None),
                 ('raw-regression', None), ('inspection-regression', None), ('constructor-regression', None)]
        need(len(cases) == NEGATIVE_COUNT + 5 and set(results) == {name for name, _ in cases}, 'exact proof case roster')
        verus = campaign_root = None
        receipts = {}
        for name, change in cases:
            folder = records / name
            row = read(folder / 'record.json')
            receipts[name] = row
            roots = {'raw-regression': check.INSPECTION.RAW, 'inspection-regression': check.INSPECTION.ROOT,
                     'constructor-regression': constructor.ROOT}
            root = roots.get(name, check.ROOT)
            staged_root = Path(row['command'][-1])
            need(staged_root.is_absolute() and str(staged_root).endswith('/' + str(root)), 'proof root')
            stage = Path(str(staged_root)[:-len(str(root)) - 1])
            need(stage.name.startswith('sources-') and (campaign_root is None or stage.parent == campaign_root), 'common owned source staging')
            campaign_root = stage.parent
            current_verus = Path(row['command'][5])
            need(current_verus.name == 'verus', 'named pinned verifier executable')
            need(current_verus.is_absolute() and (verus is None or current_verus == verus), 'same verifier')
            verus = current_verus
            terminal(row, 1 if change else 0, check.command(legacy, verus, staged_root, change))
            stdout, stderr = (folder / 'stdout.log').read_text(), (folder / 'stderr.log').read_text()
            observed = legacy.normalized(base, stdout, stderr, stage)
            need(legacy.same(observed, results[name]), 'record/result consistency')
            scalar.check_verifier(observed['verus'])
            if change:
                check.check_negative(scalar, name, row['status'], observed)
            else:
                count = {'raw-regression': RAW_VERIFIED, 'inspection-regression': INSPECTION_VERIFIED,
                         'constructor-regression': CONSTRUCTOR_VERIFIED}.get(name, PAIRED_VERIFIED)
                need(not observed['diagnostics'], 'clean positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                    'success': True, 'verified': count, 'errors': 0, 'is-verifying-entire-crate': True}), 'whole-root proof result')
        need(legacy.same(results['positive-before'], results['positive-after']), 'matching whole-root positives')
        closure_command = None
        for phase in ('before', 'after'):
            row = read(records / ('closure-' + phase) / 'record.json')
            receipts['closure-' + phase] = row
            command = row['command']
            need(len(command) == 4 and command[0] == '/bin/sh'
                 and command[1].endswith('/' + str(legacy.CLOSURE))
                 and command[2] == str(verus.parent) and command[3].endswith('/' + str(legacy.MANIFEST)), 'closure command')
            script, manifest_path = Path(command[1]), Path(command[3])
            need(script.is_absolute() and manifest_path.is_absolute(), 'absolute closure paths')
            script_repo = command[1][:-len(str(legacy.CLOSURE)) - 1]
            manifest_repo = command[3][:-len(str(legacy.MANIFEST)) - 1]
            need(script_repo == manifest_repo and script_repo, 'common closure repository')
            need(closure_command is None or command == closure_command, 'identical closure bracket commands')
            closure_command = command
            terminal(row, 0, command)
            folder = records / ('closure-' + phase)
            need((folder / 'stdout.log').read_text() ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'closure transcript')
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
        need(len(order) == COMMAND_COUNT and len(receipts) == COMMAND_COUNT, 'exact completed command inventory')
        need(legacy.same(read(records / 'accepted.json'), [
            {'name': name, 'files': {file: sha((records / name / file).read_bytes())
             for file in ('record.json', 'stdout.log', 'stderr.log')}} for name in order]), 'complete controller acceptance')
        for before, after in zip(order, order[1:]):
            need(receipts[before]['finished_ns'] <= receipts[after]['started_ns'], 'serial receipt order')
        parser_path = deps['settlement'].CPU_PARSER
        need(str(parser_path) in source['inputs'], 'CPU parser is source bound')
        parse = module(tree / parser_path, 'lifecycle_cpu_result_parser').test_results
        need(parse((records / 'test/stdout.log').read_text()) == CPU_TEST_RESULTS, 'unit and doctest results')
        need((records / 'recorder-test/stdout.log').read_text() == RECORDER_TRANSCRIPT
             and not (records / 'recorder-test/stderr.log').read_bytes(), 'recorder regression inventory')
        names = {name for name, _ in cases} | set(commands) | {'closure-before', 'closure-after'}
        for name in names:
            need({p.name for p in (records / name).iterdir()} == {'record.json', 'stdout.log', 'stderr.log'}, 'case entry roster')
        expected_files = {'source.json', 'accepted.json', 'inputs-after.json', 'results.json'}
        need({p.name for p in records.iterdir()} == expected_files | names, 'record root roster')
        expected_files.update(name + '/' + file for name in names for file in ('record.json', 'stdout.log', 'stderr.log'))
        need(len(expected_files) == RECORD_COUNT
             and {str(p.relative_to(records)) for p in records.rglob('*') if p.is_file()} == expected_files, 'record file roster')
        need(set(files) == {'README.md', 'SOURCE', 'audit.py'} | {'records/' + p for p in expected_files}, 'packet file roster')
    if not quiet:
        print(f'PASS: {SOURCE_COUNT} Git source identities, {COMMAND_COUNT} terminal commands, {NEGATIVE_COUNT} scoped lifecycle controls, and CPU receipts')


def source_policy_selftest(repo, source):
    with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-document-test-') as temporary:
        tree = Path(temporary)
        check, deps, inherited = source_tree(repo, source, tree)
        target = tree / DOCUMENT
        original = target.read_bytes()
        target.write_bytes(original + b'\n')
        try:
            check.authenticate(tree, inherited, deps, git_repo=repo)
        except ValueError as error:
            need(str(error) == 'unchanged inspection input: ' + str(DOCUMENT), 'document pin is the rejecting boundary')
        else:
            raise ValueError('accepted changed source-bound document')
        target.write_bytes(original)
        check.authenticate(tree, inherited, deps, git_repo=repo)
        changed = inherited - {check.PREVIOUS}
        replacement = Path('LICENSE-MIT')
        need(replacement not in inherited, 'helper roster replacement fixture')
        changed.add(replacement)
        try:
            inherited_roster(check, changed)
        except ValueError as error:
            need(str(error) == 'exact inherited Python helper roster', 'helper roster rejecting boundary')
        else:
            raise ValueError('accepted inherited Python helper substitution')


def bare_repository_selftest(repo, packet):
    with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-bare-test-') as temporary:
        bare = Path(temporary) / 'source.git'
        subprocess.run(['/usr/bin/git', 'clone', '--quiet', '--bare', '--shared', str(repo), str(bare)],
                       env=git_environment(), check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        need(git(bare, 'rev-parse', '--is-bare-repository') == b'true\n', 'bare repository selftest fixture')
        audit(bare, packet, quiet=True)


def selftest(repo, packet):
    for case in SELFTEST_CASES:
        with tempfile.TemporaryDirectory(prefix='fe2o3-lifecycle-audit-test-') as temporary:
            changed = Path(temporary) / 'packet'
            shutil.copytree(packet, changed)
            records = changed / 'records'
            if case in ('probe', 'source_roster', 'model_input', 'document_hash'):
                path = records / 'source.json'
                row = read(path)
                if case == 'probe':
                    row['probe'] = True
                elif case == 'document_hash':
                    row['inputs'][str(DOCUMENT)] = '0' * 64
                else:
                    removed = str(DOCUMENT) if case == 'source_roster' else 'crates/fe2o3-runtime-model/verus/context_owner_lifecycle_model_v1.rs'
                    need('LICENSE-MIT' not in row['inputs'], 'source roster selftest replacement anchor')
                    del row['inputs'][removed]
                    row['inputs']['LICENSE-MIT'] = sha(git(repo, 'show', SOURCE + ':LICENSE-MIT'))
                path.write_text(json.dumps(row))
                (records / 'inputs-after.json').write_text(json.dumps(row['inputs']))
            elif case in ('positive_count', 'positive_type', 'raw_count', 'inspection_count', 'constructor_count'):
                name = {'raw_count': 'raw-regression', 'inspection_count': 'inspection-regression',
                        'constructor_count': 'constructor-regression'}.get(case, 'positive-before')
                path = records / name / 'stdout.log'
                row = read(path)
                row['verification-results']['success' if case == 'positive_type' else 'verified'] = 1 if case == 'positive_type' else 0
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results[name]['result'] = row['verification-results']
                path.write_text(json.dumps(results))
            elif case in ('negative_type', 'negative_count', 'negative_roster'):
                path = records / 'actual_enrollscalar/stdout.log'
                row = read(path)
                field, value = {'negative_type': ('encountered-error', 1), 'negative_count': ('verified', -1),
                                'negative_roster': ('unexpected', False)}[case]
                row['verification-results'][field] = value
                path.write_text(json.dumps(row))
                path = records / 'results.json'
                results = read(path)
                results['actual_enrollscalar']['result'] = row['verification-results']
                path.write_text(json.dumps(results))
            elif case.startswith('diagnostic_'):
                name = 'actual_enrollscalar'
                path = records / name / 'stderr.log'
                rows = [json.loads(line, object_pairs_hook=unique) for line in path.read_text().splitlines()]
                message = {'diagnostic_arithmetic': 'possible arithmetic underflow/overflow',
                           'diagnostic_resource': 'function body check: Resource limit (rlimit) exceeded',
                           'diagnostic_frontend': 'unexpected token',
                           'diagnostic_abort_only': 'aborting due to 1 previous error',
                           'diagnostic_initial_typo': 'invariant not satisfied before loop body'}[case]
                if case == 'diagnostic_abort_only':
                    rows = []
                rows.append({'level': 'error', 'message': message})
                path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
                root = 'crates/fe2o3-runtime-model/verus/context_owner_lifecycle_paired_v1.rs'
                stage = read(records / name / 'record.json')['command'][-1][:-len(root) - 1]
                normalized = json.loads(json.dumps(rows).replace(stage, '<CASE>'), object_pairs_hook=unique)
                normalized.sort(key=lambda row: json.dumps(row, sort_keys=True))
                path = records / 'results.json'
                results = read(path)
                results[name]['diagnostics'] = normalized
                path.write_text(json.dumps(results))
            elif case in ('negative_scope', 'negative_terminal', 'incomplete_group'):
                path = records / 'actual_enrollscalar/record.json'
                row = read(path)
                if case == 'negative_scope':
                    row['command'][row['command'].index('--verify-function') + 1] = 'lifecycle_query_correspondence_v1'
                elif case == 'negative_terminal':
                    row['status'] = 124
                else:
                    row['group_absent'] = False
                path.write_text(json.dumps(row))
            elif case == 'paired_root':
                path = records / 'positive-before/record.json'
                row = read(path)
                row['command'][-1] = row['command'][-1].replace(
                    'context_owner_lifecycle_paired_v1.rs', 'context_owner_lifecycle_correspondence_v1.rs')
                path.write_text(json.dumps(row))
            elif case in ('raw_root', 'inspection_root', 'constructor_root'):
                name = {'raw_root': 'raw-regression', 'inspection_root': 'inspection-regression',
                        'constructor_root': 'constructor-regression'}[case]
                path = records / name / 'record.json'
                row = read(path)
                changed_root = str(Path(row['command'][-1]).with_name('context_owner_lifecycle_paired_v1.rs'))
                need(changed_root != row['command'][-1], 'effective regression root mutation')
                row['command'][-1] = changed_root
                path.write_text(json.dumps(row))
            elif case == 'verifier_basename':
                for name in read(records / 'results.json'):
                    path = records / name / 'record.json'
                    row = read(path)
                    row['command'][5] = '/unreviewed/not-verus'
                    path.write_text(json.dumps(row))
                for phase in ('before', 'after'):
                    path = records / ('closure-' + phase) / 'record.json'
                    row = read(path)
                    row['command'][2] = '/unreviewed'
                    path.write_text(json.dumps(row))
            elif case == 'whole_timeout':
                path = records / 'positive-before/record.json'
                row = read(path)
                need(row['command'][4] == str(WHOLE_TIMEOUT_SECONDS), 'whole timeout selftest anchor')
                row['command'][4] = str(SCOPED_TIMEOUT_SECONDS)
                path.write_text(json.dumps(row))
            elif case == 'cpu_count':
                path = records / 'test/stdout.log'
                text = path.read_text()
                need(text.count('1021 passed;') == 1, 'CPU selftest anchor')
                path.write_text(text.replace('1021 passed;', '1020 passed;'))
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
            elif case.startswith('acceptance_'):
                entries = read(records / 'accepted.json')
                if case == 'acceptance_missing':
                    entries = entries[:-1]
                elif case == 'acceptance_order':
                    entries.reverse()
                else:
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
            elif case == 'extra_packet_directory':
                (changed / 'unexpected').mkdir()
            elif case == 'auditor_identity':
                path = changed / 'audit.py'
                path.write_text(path.read_text() + '\n# changed packaged auditor\n')
            else:
                raise ValueError('unknown auditor selftest case: ' + case)
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
            except ValueError as error:
                if case in ('source_roster', 'model_input'):
                    need(str(error) == 'complete source roster', 'exact source roster is the rejecting boundary: ' + case)
                if case.startswith('diagnostic_'):
                    need(str(error) == 'logical failure, not arithmetic, syntax, timeout, or solver resource exhaustion',
                         'diagnostic policy is the rejecting boundary: ' + case)
                if case == 'verifier_basename':
                    need(str(error) == 'named pinned verifier executable', 'verifier basename rejecting boundary')
                if case in ('raw_root', 'inspection_root', 'constructor_root'):
                    need(str(error) == 'proof root', 'regression root rejecting boundary: ' + case)
                continue
            raise ValueError('accepted rehashed mutation: ' + case)
    source_policy_selftest(repo, read(packet / 'records/source.json'))
    bare_repository_selftest(repo, packet)
    print(f'PASS: {len(SELFTEST_CASES)} rehashed malformed record sets and 2 source-policy changes rejected; bare Git audit passed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).parent)
    parser.add_argument('--selftest', action='store_true')
    args = parser.parse_args()
    audit(args.repo.resolve(), args.packet.resolve())
    if args.selftest:
        selftest(args.repo.resolve(), args.packet.resolve())
