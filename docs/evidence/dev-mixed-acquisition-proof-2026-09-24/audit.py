#!/usr/bin/env python3
"""Audit the sealed mixed-acquisition developer packet, then replay its records."""
import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import types
from pathlib import Path

sys.dont_write_bytecode = True
PACKET = Path('docs/evidence/dev-mixed-acquisition-proof-2026-09-24')
SOURCE_COMMIT = '2d56e65642cde5752fb40396b47ea8605bc2f9e6'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-mixed-acquire.py')
TEST = CHECK.with_name('test-mixed-acquire.py')
PINS = {CHECK: '6ab16853e5f3fa9630ab740b797750ee36c52bff7d93231b69b0d84ba786073f',
        TEST: 'cfec5254ce092db01f91edaed1e8d49985cf460ea3d2800d43674818c4128276'}
SIGNER = 'harmenon@amd.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICITzoV64zd4tYeZhvOi+mnwQaxEI4rFvXeC3HxileBS\n'
SIGNATURE_LINE = 'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n'
COLLECTION_CONTROLS = {
    'audit.py': '5212f2e9697e14aec1b55a5b21468723e3d59186bfd359b57b61c836b39805be',
    'finalize.py': '46ed611b5cd1277720efc1480fdf5829bfce0788a45b8f366d98d78ef03d5d80',
    'test_audit.py': '34846bf4b8bc735944407d65e81fbeecfe139ba9152da396359f9e21325e460b',
    'test_finalize.py': '45b9eecbc5e898ede661aefe1bcb7acff79cffeed20e7f8919f088b2085978a9',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique_json(data):
    def pairs(rows):
        result = {}
        for key, value in rows:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=pairs)


def ordinary(root, path):
    need(not path.is_absolute() and '..' not in path.parts, 'relative packet/source path')
    need(not any((root / p).is_symlink() for p in (path, *path.parents)), 'no packet/source symlinks')
    need(stat.S_ISREG((root / path).lstat().st_mode), 'ordinary packet/source file')
    return (root / path).read_bytes()


def git(repo, *args):
    return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
        '-c', 'gpg.ssh.program=/usr/bin/ssh-keygen', '-C', str(repo), *args], env={
        'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'GIT_NO_REPLACE_OBJECTS': '1',
        'GIT_OPTIONAL_LOCKS': '0', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull})


def verify_signature(repo, commit):
    need(re.fullmatch('[0-9a-f]{40}', commit) is not None, 'exact signed commit')
    with tempfile.TemporaryDirectory(prefix='mixed-packet-signer-') as temporary:
        path = Path(temporary) / 'allowed-signers'
        path.write_text(SIGNER)
        git(repo, '-c', 'gpg.ssh.allowedSignersFile=' + str(path), 'verify-commit', commit)


def files(root):
    paths = list(root.rglob('*'))
    need(not root.is_symlink() and root.is_dir() and not any(p.is_symlink() for p in paths), 'ordinary packet tree')
    return {p.relative_to(root) for p in paths if not p.is_dir()}


def check_seal(root):
    sealed = {}
    for line in ordinary(root, Path('SHA256SUMS')).decode('ascii').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  ([A-Za-z0-9_./-]+)', line)
        need(match is not None, 'canonical seal line')
        path = Path(match[2])
        need(str(path) == match[2] and path not in sealed and path != Path('SHA256SUMS'), 'unique canonical sealed path')
        need(not path.is_absolute() and '..' not in path.parts, 'contained sealed path')
        sealed[path] = match[1]
    need(set(sealed) == files(root) - {Path('SHA256SUMS')}, 'closed packet seal roster')
    for path, digest in sealed.items():
        need(sha(ordinary(root, path)) == digest, 'sealed bytes: ' + str(path))
    manifest = unique_json(ordinary(root, Path('artifacts.json')))
    need(type(manifest) is dict and set(manifest) == {'schema', 'files'}
         and type(manifest['schema']) is int and manifest['schema'] == 1,
         'artifact manifest schema')
    raw = {str(p): sha(ordinary(root, p)) for p in files(root) if p.parts[0] == 'raw'}
    need(manifest['files'] == raw, 'exact raw artifact manifest')
    return sealed


def load_checker(repo):
    verify_signature(repo, SOURCE_COMMIT)
    captured = {}
    for path, digest in PINS.items():
        data = ordinary(repo, path)
        need(sha(data) == digest and data == git(repo, 'show', SOURCE_COMMIT + ':' + str(path)),
             'authenticated replay control before import: ' + str(path))
        captured[path] = data
    module = types.ModuleType('sealed_mixed_replay')
    module.__file__ = str(repo / CHECK)
    sys.modules[module.__name__] = module
    exec(compile(captured[CHECK], module.__file__, 'exec'), module.__dict__)
    return module


def replay(repo, raw):
    source = unique_json(ordinary(raw, Path('source.json')))
    need(source['commit'] == SOURCE_COMMIT, 'qualified source commit')
    module = load_checker(repo)
    saved = sys.argv
    try:
        sys.argv = [str(repo / CHECK), '--repo', str(repo), '--output', str(raw), '--verify']
        module.main()
    finally:
        sys.argv = saved


def check_visibility(report):
    need(set(report) == {'scope', 'inspected_processes', 'observed_private_references', 'uninspectable',
                        'uninspectable_count', 'vanished', 'excluded_collector_pid', 'collector_reference'}, 'closed visibility report')
    need(report['scope'] == 'same-uid-best-effort-not-global-absence'
         and type(report['observed_private_references']) is int and report['observed_private_references'] == 0
         and type(report['inspected_processes']) is int and report['inspected_processes'] >= 0
         and type(report['uninspectable']) is list and type(report['uninspectable_count']) is int
         and report['uninspectable_count'] == len(report['uninspectable'])
         and type(report['excluded_collector_pid']) is int and report['excluded_collector_pid'] > 0
         and report['collector_reference'] == 'only read-only campaign lock; source copy reads already closed', 'scoped visibility fields')
    need(type(report['vanished']) is list and all(type(p) is int and p > 0 for p in report['vanished'])
         and len(set(report['vanished'])) == len(report['vanished']), 'vanished process identities')
    observed = set(report['vanished'])
    for row in report['uninspectable']:
        need(type(row) is dict and set(row) == {'process_identity', 'field', 'errno'}, 'inaccessible field schema')
        identity = row['process_identity']
        need(type(identity) is dict and set(identity) == {'pid', 'uid', 'start_time_ticks'}
             and type(identity['pid']) is int and identity['pid'] > 0
             and type(identity['uid']) is int and identity['uid'] >= 0
             and (identity['start_time_ticks'] is None or type(identity['start_time_ticks']) is int and identity['start_time_ticks'] > 0)
             and type(row['errno']) is int and row['errno'] in (1, 13)
             and type(row['field']) is str and re.fullmatch(r'(?:stat(?:-after)?|cmdline|cwd|maps|fd(?:/[0-9]+)?)', row['field']),
             'inaccessible process field identity')
        observed.add(identity['pid'])
    need(len(observed) <= report['inspected_processes'] and report['excluded_collector_pid'] not in observed,
         'visibility process bounds')


def check_finalization(repo, packet):
    def read(root, path):
        return unique_json(ordinary(root, Path(path)))
    marker = read(packet, 'finalization.json')
    need(set(marker) == {'schema', 'audit'} and type(marker['schema']) is int and marker['schema'] == 1
         and re.fullmatch(r'audit[1-9][0-9]*', marker['audit']), 'finalization marker')
    audit = packet / marker['audit']
    source = read(packet, 'raw/campaign1/source.json')
    original_repo = Path(source['recorded_repo'])
    original_packet = original_repo / PACKET
    private = Path('/home/harsh/.codex-tmp/fe2o3-mixed-proof-20260924-65fWJbkW')
    need(Path(source['recorded_output']) == private / 'campaign1', 'exact collected source path')
    collect = Path('docs/evidence/dev-native-producer-mi300x-2026-09-24')
    visibility = Path('docs/evidence/dev-mixed-input-acquisition-cpu-2026-09-24/finalize.py')
    paths = {PACKET / name for name in ('audit.py', 'test_audit.py', 'finalize.py', 'test_finalize.py', 'README.md')}
    paths |= {collect / name for name in ('collect.py', 'verify.py', 'test_verify.py')} | {visibility}
    before, after = read(audit, 'inputs-before.json'), read(audit, 'inputs-after.json')
    need(before == after and set(before) == {str(p) for p in paths}, 'complete finalization control bracket')
    need(files(audit / 'controls-source') == paths, 'exact captured finalization controls')
    for path in paths:
        data = ordinary(audit / 'controls-source', path)
        need(sha(data) == before[str(path)], 'captured finalization control identity')
        if path.is_relative_to(PACKET):
            # Keep the actual collection controls, including the initial audit bug.
            if path.name != 'README.md':
                need(sha(data) == COLLECTION_CONTROLS[path.name], 'exact historical collection control')
                if path.name not in ('audit.py', 'finalize.py'):
                    need(data == ordinary(packet, path.relative_to(PACKET)), 'unchanged calibration control')
        else:
            need(data == git(repo, 'show', SOURCE_COMMIT + ':' + str(path)), 'signed inherited collection helper')
    python = ['/usr/bin/python3', '-I', '-B']
    commands = {
        'replay': python + [str(original_repo / CHECK), '--output', str(private / 'campaign1'), '--verify'],
        'packet-tests': python + [str(original_packet / 'test_audit.py'), '--raw', str(private / 'campaign1')],
        'finalizer-tests': python + [str(original_packet / 'test_finalize.py')],
        'cleanup-tests': python + [str(original_repo / collect / 'test_verify.py'), 'CleanupTests'],
    }
    need(read(audit, 'controls.json') == {'environment': {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'},
         'cwd': str(original_repo), 'timeout_seconds': 900, 'commands': commands}, 'exact finalization commands')
    commands['absence'] = python + ['-c', 'import os,sys; assert not os.path.lexists(sys.argv[1])', str(private)]
    need({p.name for p in (audit / 'commands').iterdir()} == set(commands), 'complete finalization stage roster')
    transcripts = {'replay': 'PASS: independent mixed-acquisition campaign replay\n',
        'packet-tests': 'PASS: mixed-acquisition packet calibration (5 groups)\n',
        'finalizer-tests': 'PASS: mixed-acquisition finalizer calibration (8 groups)\n', 'cleanup-tests': '', 'absence': ''}
    rows, finished = {}, 0
    for name, command in commands.items():
        root = audit / 'commands' / name
        need(files(root) == {Path(p) for p in ('record.json', 'stdout.log', 'stderr.log')}, 'exact finalization case files')
        row = read(root, 'record.json')
        need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}
             and type(row['status']) is int and row['status'] == 0 and row['group_absent'] is True
             and all(type(row[p]) is int and row[p] > 0 for p in ('started_ns', 'finished_ns', 'process_group')),
             'normal successful finalization receipt')
        bound = 30 if name == 'absence' else 900
        need(row['command'] == command and finished <= row['started_ns'] < row['finished_ns']
             and row['finished_ns'] - row['started_ns'] <= (bound + 10) * 10**9, 'ordered bounded finalization command')
        need(ordinary(root, Path('stdout.log')).decode() == transcripts[name], 'exact finalization transcript')
        stderr = ordinary(root, Path('stderr.log')).decode()
        if name == 'cleanup-tests':
            names = ('actual_owned_removal_and_failure_preservation',
                     'final_recheck_rejects_changed_added_and_aliased_entries', 'loader_authenticates_before_compile',
                     'missing_file_and_live_recorded_group_prevent_deletion', 'mount_refuses_owned_snapshot')
            prefix = ''.join('test_' + n + ' (__main__.CleanupTests.test_' + n + ') ... ok\n' for n in names)
            need(stderr.startswith(prefix) and re.fullmatch(r'\n-{70}\nRan 5 tests in [0-9]+\.[0-9]+s\n\nOK\n',
                 stderr[len(prefix):]), 'complete cleanup tests')
        if name in ('replay', 'packet-tests'):
            lines = stderr.splitlines(keepends=True)
            need(lines and all(line == SIGNATURE_LINE for line in lines)
                 and (name != 'replay' or len(lines) == 3), 'only expected Git signature diagnostics')
        if name in ('absence', 'finalizer-tests'):
            need(not stderr, 'clean finalization stage')
        rows[name], finished = row, row['finished_ns']
    retained = {str(p): sha(ordinary(packet / 'raw', p)) for p in files(packet / 'raw')}
    initial, terminal = read(audit, 'cleanup-before.json'), read(audit, 'cleanup-after.json')
    need(set(initial) == {'path', 'retained_files', 'excluded', 'allocated_bytes', 'known_terminal_groups', 'scope', 'absent'}
         and initial['path'] == str(private) and initial['retained_files'] == retained and initial['excluded'] == []
         and type(initial['allocated_bytes']) is int and initial['allocated_bytes'] >= 0 and initial['absent'] is False
         and terminal == initial | {'absent': True}, 'complete byte-exact retention and terminal cleanup')
    proof_rows = [read(p.parent, p.name) for p in sorted((packet / 'raw/campaign1').glob('*/record.json'))]
    groups = [r['process_group'] for r in proof_rows] + [rows[n]['process_group'] for n in sorted(rows) if n != 'absence']
    need(len(proof_rows) == 15 and initial['known_terminal_groups'] == groups, 'exact cleanup command groups')
    snapshot = read(audit, 'cleanup-source.json')
    need({p: row[5] for p, row in snapshot.items() if row[5] is not None} == retained, 'retained full scratch inventory')
    need(initial['scope'] == 'controller-lock-and-owned-command-groups-not-global-process-absence', 'bounded cleanup claim')
    for name in ('process-visibility-before.json', 'process-visibility-final.json'):
        check_visibility(read(audit, name))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--packet', type=Path)
    parser.add_argument('--unpublished', action='store_true', help='check seal and replay without an evidence-commit claim')
    args = parser.parse_args()
    repo = args.repo.resolve()
    packet = args.packet.resolve() if args.packet else repo / PACKET
    sealed = check_seal(packet)
    if not args.unpublished:
        commit = git(repo, 'rev-parse', 'HEAD').decode().strip()
        verify_signature(repo, commit)
        tracked = {Path(p).relative_to(PACKET) for p in git(repo, 'ls-tree', '-r', '--name-only', commit, '--', str(PACKET)).decode().splitlines()}
        need(tracked == set(sealed) | {Path('SHA256SUMS')}, 'exact signed evidence roster')
        for path in tracked:
            need(git(repo, 'show', commit + ':' + str(PACKET / path)) == ordinary(packet, path), 'signed evidence bytes')
    check_finalization(repo, packet)
    replay(repo, packet / 'raw/campaign1')
    need(check_seal(packet) == sealed, 'unchanged packet seal during replay')
    print('PASS: mixed-acquisition packet ' + ('seal/replay (unpublished)' if args.unpublished else 'signed seal/replay'))


if __name__ == '__main__':
    main()
