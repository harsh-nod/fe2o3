#!/usr/bin/env python3
"""Replay retained source, build, admission, result and collection evidence; no GPU work."""
import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import shlex
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
CANDIDATE = '1e13a90cd5e604a4612b079b48f629005a0a047b'
CPU_COMMIT = '7fc77eed0c56cb94641c53e3d7365a2a170ccf43'
CPU_PREFIX = 'docs/evidence/dev-topology-link-scratch-cpu-2026-09-24/'
PROTOCOL_NAMES = {'campaign.py', 'native.py', 'test_campaign.py', 'test_cpu_binding.py', 'validate_protocol.py'}

def need(value, message):
    if not value:
        raise RuntimeError(message)

def sha(path):
    need(path.is_file() and not path.is_symlink(), 'ordinary file: ' + str(path))
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON field')
        result[key] = value
    return result

def read(path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique)

def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

def unpack(path, index, destination):
    need(sha(path) == index['archive_sha256'], 'compressed archive hash')
    destination.mkdir()
    names, total = set(), 0
    with tarfile.open(path, 'r|gz') as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not name.startswith('/') and
                 all(part not in ('', '.', '..') for part in name.split('/')), 'ordinary archive member')
            need(name in index['files'] and name not in names, 'exact archive member')
            names.add(name)
            total += member.size
            need(len(names) <= 2048 and 0 <= member.size <= 200 * 1024**2 and total <= 512 * 1024**2, 'bounded archive')
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, target.open('xb') as output:
                while data := source.read(1024 * 1024):
                    output.write(data)
            need(sha(target) == index['files'][name], 'raw archive member hash')
    need(names == index['files'].keys(), 'complete archive roster')

def receipt(folder, command=None, environment=None, cwd=None, seconds=None, input_sha=None):
    row = read(folder / 'receipt.json')
    need(type(row['exit']) is int and row['exit'] == 0 and row['error'] is None and row['group_absent'] is True, 'successful reaped receipt: ' + folder.name)
    need(row['stdin_sha256'] == input_sha, 'exact recorded stdin')
    for stream in ('stdout', 'stderr'):
        need(sha(folder / stream) == row[stream + '_sha256'], 'receipt stream hash')
    for key in ('started_ns', 'finished_ns', 'timeout_seconds', 'pid'):
        need(type(row[key]) is int and row[key] > 0, 'integer receipt field')
    need(0 <= row['finished_ns'] - row['started_ns'] <= row['timeout_seconds'] * 10**9, 'command within declared bound')
    for actual, expected, label in ((row['command'], command, 'command'), (row['environment'], environment, 'environment'),
                                    (row['cwd'], str(cwd) if cwd is not None else None, 'cwd'), (row['timeout_seconds'], seconds, 'timeout')):
        if expected is not None:
            need(actual == expected, 'exact receipt ' + label + ': ' + folder.name)
    return row

def sequence(rows):
    for previous, following in zip(rows, rows[1:]):
        need(previous['finished_ns'] <= following['started_ns'], 'serialized receipt chronology')

def control_output(value, name, marker):
    if name == 'create':
        need(value == marker, 'created exact owner marker')
    elif name == 'cleanup':
        need(value == {'removed': marker['path']}, 'removed exact owned path')
    elif name == 'absence':
        need(set(value) == {'path_absent', 'processes_absent'} and value['path_absent'] is True and value['processes_absent'] is True, 'independent path/process absence')

def control_receipt(folder, name, marker, C):
    serialized = json.dumps(marker, sort_keys=True, separators=(',', ':'))
    command = ['ssh', '-T', *C.C.SSH, 'mi300x', shlex.join(['/usr/bin/python3', '-I', '-B', '-', name, serialized])]
    row = receipt(folder, command, cwd=C.REPO, seconds=120, input_sha=hashlib.sha256(C.C.control_bytes(C.N.PREFIX)).hexdigest())
    need(row['environment'] is None, 'inherited transport environment')
    need((folder / 'stderr').read_bytes() == b'', 'empty owned control stderr')
    control_output(read(folder / 'stdout'), name, marker)
    return row

def binary_roster(value):
    need(set(value) == {'baseline', 'candidate', 'hip', 'hsa'} and
         all(isinstance(digest, str) and re.fullmatch(r'[0-9a-f]{64}', digest) for digest in value.values()), 'exact canonical binary attestations')

def protocol_roster(value):
    need(set(value) == PROTOCOL_NAMES and all(isinstance(digest, str) and re.fullmatch(r'[0-9a-f]{64}', digest) for digest in value.values()), 'complete captured protocol roster')

def source_archive(path, entries):
    files = {}
    with tarfile.open(path, 'r:') as archive:
        for member in archive:
            need(member.isfile() or member.isdir(), 'ordinary signed source entry')
            if member.isdir():
                continue
            need(member.name in entries and member.name not in files, 'exact signed source file')
            mode, oid = entries[member.name]
            raw = archive.extractfile(member).read()
            need(hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == oid, 'source blob identity')
            need(bool(member.mode & 0o111) == (mode == '100755'), 'source executable mode')
            files[member.name] = hashlib.sha256(raw).hexdigest()
    need(files.keys() == entries.keys(), 'complete signed source archive')
    return files

def verify_run(run, C):
    B, H, N, K = C.B, C.H, C.N, C.K
    binding, marker = read(run / 'binding.json'), read(run / 'owner.json')
    need(binding['commit'] == CANDIDATE and marker['commit'] == CANDIDATE and marker['binding_sha256'] == sha(run / 'binding.json'), 'candidate/owner binding')
    owned = B.owned_path(marker, exists=False)
    need(H.same_json(binding['controls'], N.CONTROLS) and H.same_json(binding['order'], N.ORDER), 'declared controls/order')
    need(binding['target'] == 'x86_64-unknown-linux-musl' and binding['profile'] == 'release default' and binding['features'] == 'default,hardware-diagnostic', 'build profile')
    need(B.inventory(run / 'payload') == binding['payload'] and set(binding['payload']) == N.PAYLOAD, 'retained payload closure')
    with tarfile.open(run / 'payload/source.tar.gz', 'r:gz') as archive:
        B.validate_members(archive.getmembers(), binding['source_files'])
        need({member.name: hashlib.sha256(archive.extractfile(member).read()).hexdigest() for member in archive.getmembers()} == binding['source_files'], 'remote source archive')
    C.distinct_cohort_binaries(run / 'payload')
    protocol = read(run / 'protocol-before.json')
    protocol_roster(protocol)
    need(protocol == read(run / 'protocol-after.json') == binding['protocol_files'] == B.inventory(run / 'protocol'), 'unchanged frozen protocol')
    need(all(sha(HERE / name) == digest for name, digest in protocol.items()), 'replay uses captured protocol')
    local, ordered = run / 'local', []
    execution = Path(read(local / 'baseline-build/receipt.json')['cwd']).parent
    for cohort, commit in (('baseline', C.BASELINE), ('candidate', CANDIDATE)):
        K.git('-c', 'gpg.ssh.allowedSignersFile=' + C.C.SIGNERS, 'verify-commit', commit)
        raw = K.git('ls-tree', '-rz', commit, '--', *K.SELECTORS)
        need(raw == (local / (cohort + '-tree/stdout')).read_bytes(), 'captured signed tree')
        entries = {}
        for row in raw.rstrip(b'\0').split(b'\0'):
            header, name = row.split(b'\t')
            mode, kind, oid = header.decode().split()
            need(kind == 'blob' and mode in ('100644', '100755'), 'ordinary source tree')
            entries[name.decode()] = (mode, oid)
        files = source_archive(local / (cohort + '-archive/stdout'), entries)
        need(files == read(run / (cohort + '-source.json')) == binding['cohorts'][cohort]['files'], 'signed source maps')
        need(binding['cohorts'][cohort]['commit'] == commit, 'cohort commit')
        env = binding['local_build_environments'][cohort]
        expected_env = {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
                        'LANG': 'C', 'LC_ALL': 'C', 'CARGO_TARGET_DIR': str(execution / ('target-' + cohort)),
                        'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never', 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-03'}
        need(env == expected_env, 'exact isolated cohort environment')
        selectors = [value for value in K.SELECTORS if any(name == value or name.startswith(value + '/') for name in entries)]
        commands = [('signature', [*K.GIT, '-c', 'gpg.ssh.allowedSignersFile=' + C.C.SIGNERS, 'verify-commit', commit], 30),
                    ('tree', [*K.GIT, 'ls-tree', '-rz', commit, '--', *K.SELECTORS], 120),
                    ('archive', [*K.GIT, 'archive', '--format=tar', commit, *selectors], 120),
                    ('rustc', ['rustc', '-vV'], 30), ('cargo', ['cargo', '-V'], 30),
                    ('build', ['cargo', 'build', '--locked', '--release', '-p', 'fe2o3-runtime', '--target', binding['target'], '--features', 'hardware-diagnostic', '--example', C.EXAMPLE], 1800),
                    ('tests', ['cargo', 'test', '--locked', '--release', '-p', 'fe2o3-runtime', '--target', binding['target'], '--features', 'hardware-diagnostic', '--example', C.EXAMPLE], 1800),
                    ('after-rustc', ['rustc', '-vV'], 30), ('after-cargo', ['cargo', '-V'], 30)]
        for label, command, seconds in commands:
            git_stage = label in ('signature', 'tree', 'archive')
            ordered.append(receipt(local / (cohort + '-' + label), command, K.GIT_ENV if git_stage else env,
                                   C.REPO if git_stage else execution / ('source-' + cohort), seconds))
        for tool in ('rustc', 'cargo'):
            for stream in ('stdout', 'stderr'):
                need((local / (cohort + '-' + tool) / stream).read_bytes() == (local / (cohort + '-after-' + tool) / stream).read_bytes() ==
                     (local / ('baseline-' + tool) / stream).read_bytes(), 'matched unchanged tool output')
        test_output = (local / (cohort + '-tests/stdout')).read_text()
        expected_tests = {'aggregate_and_ordinary_depth_bounds_are_distinct', 'aggregate_classification_includes_all_aggregate_modes',
                          'aggregate_diagnostic_call_roster_is_bounded_before_native_open', 'aggregate_hot_only_requires_depth_one',
                          'canaries_bind_the_exact_inner_copy_region', 'currentness_flag_is_feature_gated_and_exclusive_before_native_open',
                          'currentness_mode_preserves_hot_only_summary_with_distinct_diagnostic_label', 'diagnostic_submission_roster_is_bounded_before_native_open',
                          'currentness_rows_preserve_every_identity_and_distinct_nested_interval', 'existing_modes_preserve_both_phases_and_report_schemas',
                          'patterns_distinguish_round_slot_and_direction', 'percentile_uses_nearest_rank', 'progress_flags_are_explicit_and_mutually_exclusive_before_native_open'}
        names = re.findall(r'^test tests::([a-z0-9_]+) \.\.\. ok$', test_output, re.MULTILINE)
        need(len(names) == 13 and set(names) == expected_tests and 'test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;' in test_output, 'complete benchmark tests')
    need(set(K.git('diff', '--name-only', C.BASELINE, CANDIDATE, '--', *K.SELECTORS).decode().splitlines()) == C.EXPECTED_DELTA, 'exact reviewed source delta')
    K.git('-c', 'gpg.ssh.allowedSignersFile=' + C.C.SIGNERS, 'verify-commit', CPU_COMMIT)
    cpu_tar = K.git('archive', '--format=tar', CPU_COMMIT, CPU_PREFIX)
    with tarfile.open(fileobj=io.BytesIO(cpu_tar), mode='r:') as archive:
        cpu = {member.name.removeprefix(CPU_PREFIX + 'raw/cpu1/'): hashlib.sha256(archive.extractfile(member).read()).hexdigest()
               for member in archive if member.isfile() and member.name.startswith(CPU_PREFIX + 'raw/cpu1/')}
    need(cpu == binding['cpu_qualification_files'] == B.inventory(run / 'cpu-qualification'), 'published CPU qualification bytes')
    roots = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates', 'benchmarks/runtime_gfx942']
    expected_cpu = {name: digest for name, digest in binding['cohorts']['candidate']['files'].items()
                    if any(name == root or name.startswith(root + '/') for root in roots)
                    and (name.endswith(('.rs', '.toml', '.lock', '.json', '.py', '.cpp', '.hpp')) or '/fixtures/' in name)}
    cpu_before = read(run / 'cpu-qualification/inputs-before.json')
    need(cpu_before == read(run / 'cpu-qualification/inputs-after.json') and cpu_before['source'] == expected_cpu, 'complete CPU source bracket binds candidate')
    need(sha(run / 'cpu-runner.py') == binding['cpu_runner_sha256'] == 'b1e15db084398e4e19cbb0f4f7a0588cf2594b89611d49248a1426243c339b4a', 'qualified CPU runner')
    serialized = json.dumps(marker, sort_keys=True, separators=(',', ':'))
    for name in ('create', 'upload', 'native', 'inventory', 'collect', 'cleanup', 'absence'):
        if name in ('create', 'inventory', 'cleanup', 'absence'):
            row = control_receipt(local / name, name, marker, C)
        else:
            row = receipt(local / name, cwd=C.REPO, seconds=12000 if name == 'native' else 120)
        need(row['environment'] is None, 'inherited transport environment')
        if name == 'native':
            need(row['command'] == K.native_command(serialized), 'bound native invocation')
        elif name == 'upload':
            need(row['command'] == ['scp', '-q', *C.C.SSH, '--', *(str(execution / 'payload' / member) for member in sorted(N.PAYLOAD)), str(execution / 'binding.json'), 'mi300x:' + str(owned) + '/'], 'exact upload')
        elif name == 'collect':
            need(row['command'] == ['scp', '-q', '-r', *C.C.SSH, '--', 'mi300x:' + str(owned) + '/results', str(execution / 'remote')], 'exact collection')
        ordered.append(row)
    need(len(list(local.iterdir())) == len(ordered) == 25, 'complete local command roster')
    sequence(ordered)
    need(H.same_json(read(run / 'collection.json'), {'failures': [], 'owned_cleanup': True, 'exclusive_reservation': False, 'performance_acceptance': False}), 'collected and cleaned without acceptance inflation')
    remote = run / 'remote'
    need(B.inventory(remote) == read(run / 'remote-inventory.json') == read(local / 'inventory/stdout'), 'byte-exact complete remote collection')
    need(read(remote / 'source-before.json') == read(remote / 'source-after.json') == binding['source_files'], 'remote source continuity')
    binaries = read(remote / 'binaries.json')
    binary_roster(binaries)
    need(binaries == read(remote / 'binaries-after.json'), 'remote ELF continuity')
    for cohort in ('baseline', 'candidate'):
        need(binaries[cohort] == binding['payload']['kfd-' + cohort], 'retained KFD ELF binding')
    env, rows = H.environment(owned), []
    for name, command, seconds in N.IDENTITIES + N.builds(owned):
        rows.append(receipt(remote / name, command, env, owned / 'source', seconds))
    diagnostic = C.load(run / 'payload/diagnostic.py', 'replay_diagnostic', C.PARSER_DIGESTS['diagnostic.py'])
    ordinary = C.load(run / 'payload/results.py', 'replay_hot', C.PARSER_DIGESTS['results.py'])
    results = []
    for name, backend, mode, command, phase_env in N.trials(owned, binding['devices']):
        for phase in ('before', 'workload', 'settled', 'delayed'):
            if phase == 'workload':
                row = receipt(remote / name, command, phase_env, owned / 'source', 180)
                rows.append(row)
                need((remote / name / 'stderr').read_bytes() == b'', 'empty workload stderr')
                raw, uids = (remote / name / 'stdout').read_bytes(), [device[2] for device in binding['devices']]
                parsed = diagnostic.parse_result(raw, mode, uids) if mode == 'on' else ordinary.parse_result(raw, 'kfd' if backend in ('baseline', 'candidate') else backend, uids)
                results.append({'trial': name, 'backend': backend, 'mode': mode, 'result': parsed})
                continue
            previous = rows[-1]['finished_ns']
            for ordinal, (index, bdf, uid) in enumerate(binding['devices']):
                label = name + '-' + phase
                folder = remote / (label + '-gpu' + str(index))
                row = receipt(folder, H.observe_spec(label, index, bdf, uid), env, owned / 'source', 100)
                if ordinal == 0 and phase in ('settled', 'delayed'):
                    need(row['started_ns'] >= previous + (2 if phase == 'settled' else 20) * 10**9, 'postflight settling delay')
                need((folder / 'stderr').read_bytes() == b'', 'empty observer stderr')
                H.parse_endpoint((folder / 'stdout').read_bytes(), index, bdf, uid)
                rows.append(row)
    for name, command, seconds in N.IDENTITIES:
        rows.append(receipt(remote / ('after-' + name), command, env, owned / 'source', seconds))
        for stream in ('stdout', 'stderr'):
            need((remote / name / stream).read_bytes() == (remote / ('after-' + name) / stream).read_bytes(), 'remote tool continuity')
    need(len(rows) == 92 and len(list(remote.glob('*/receipt.json'))) == 92, 'complete remote command roster')
    sequence(rows)
    native = ordered[20]
    need(native['started_ns'] <= rows[0]['started_ns'] and rows[-1]['finished_ns'] <= native['finished_ns'], 'remote stages enclosed by invocation')
    need(H.same_json(results, read(remote / 'validated-results.json')), 'independent result replay')
    need(H.same_json(read(remote / 'finished.json'), {'commit': CANDIDATE, 'failures': [], 'native_execution': True, 'exclusive_reservation': False, 'performance_acceptance': False, 'formal_refinement': False}), 'bounded native completion claim')
    return results

def main():
    index = read(HERE / 'archive-index.json')
    need(set(index) == {'native1', 'native2', 'protocol8'}, 'complete archive roster')
    with tempfile.TemporaryDirectory(prefix='scratch-replay-') as directory:
        root = Path(directory)
        for name in index:
            unpack(HERE / (name + '.tar.gz'), index[name], root / name)
        captured = read(root / 'native2/protocol-before.json')
        protocol_roster(captured)
        need(all(sha(HERE / name) == digest for name, digest in captured.items()), 'captured replay imports')
        C = load(HERE / 'campaign.py', 'scratch_replay_campaign')
        need(sha(Path(C.C.SIGNERS)) == C.C.SIGNERS_SHA, 'pinned trusted signer list')
        rejected = root / 'native1'
        need(sha(rejected / 'payload/kfd-baseline') == sha(rejected / 'payload/kfd-candidate'), 'preserved rejected equal ELFs')
        need(read(rejected / 'rejection.json')['accepted_performance_evidence'] is False, 'rejected run excluded')
        rejected_collection = read(rejected / 'collection.json')
        need(rejected_collection['owned_cleanup'] is True and rejected_collection['failures'] and rejected_collection['performance_acceptance'] is False, 'rejected run collected/cleaned but failed')
        rejected_marker = read(rejected / 'owner.json')
        need(rejected_marker['binding_sha256'] == sha(rejected / 'binding.json'), 'rejected ownership binding')
        need(C.B.inventory(rejected / 'remote') == read(rejected / 'remote-inventory.json') == read(rejected / 'local/inventory/stdout'), 'rejected byte-exact collection')
        need(receipt(rejected / 'local/collect')['environment'] is None, 'rejected collection transport environment')
        for name in ('create', 'inventory', 'cleanup', 'absence'):
            control_receipt(rejected / 'local' / name, name, rejected_marker, C)
        protocol = root / 'protocol8'
        before = read(protocol / 'inputs-before.json')
        need(before == read(protocol / 'inputs-after.json'), 'protocol test inputs unchanged')
        original = Path(read(root / 'native2/local/baseline-build/receipt.json')['cwd']).parent.parent
        for name, digest in captured.items():
            need(before[str(original / name)] == digest, 'qualified captured protocol')
        tests = [(original / 'test_campaign.py', 11), (original / 'test_cpu_binding.py', 11)]
        for lane, counts in (('dev-xgmi-currentness-attribution-mi300x-2026-09-20', (9, 7)), ('dev-xgmi-peer-hot-mi300x-2026-09-19', (12, 7))):
            for name, count in zip(('test_results.py', 'test_native.py'), counts):
                tests.append((C.REPO / 'docs/evidence' / lane / name, count))
            for name in ('results.py', 'native.py', 'test_results.py', 'test_native.py'):
                path = C.REPO / 'docs/evidence' / lane / name
                need(before[str(path)] == sha(path), 'unchanged inherited parser/test input')
        test_rows = []
        need(len([path for path in protocol.iterdir() if path.is_dir()]) == len(tests), 'complete protocol test roster')
        for index, (path, count) in enumerate(tests):
            folder = protocol / (str(index) + '-' + path.stem)
            test_rows.append(receipt(folder, ['/usr/bin/python3', '-I', '-B', str(path)],
                                     {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}, C.REPO, 120))
            stderr = (folder / 'stderr').read_text()
            need(('Ran ' + str(count) + ' tests in ') in stderr and stderr.endswith('\nOK\n'), 'protocol test count/status')
        sequence(test_rows)
        results = verify_run(root / 'native2', C)
        summary = [{'trial': row['trial'], 'backend': row['backend'], **{key: value for key, value in row['result'].items() if '_p50_ns' in key or '_p95_ns' in key}}
                   for row in results if row['mode'] == 'off']
        print(json.dumps({'native_execution': True, 'performance_acceptance': False, 'off_process_summaries_ns': summary}, indent=2))

if __name__ == '__main__':
    main()
