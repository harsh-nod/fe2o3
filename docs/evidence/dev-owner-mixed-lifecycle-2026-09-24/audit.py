#!/usr/bin/env python3
"""Authenticate and replay the bounded mixed-reader lifecycle proof packet."""
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

import argparse
import hashlib
from pathlib import Path
import stat
import types

PACKET = Path('docs/evidence/dev-owner-mixed-lifecycle-2026-09-24')
SOURCE = '33b4606f4134045757ce6f6d340e829ba79e2314'
PRIOR_SOURCE = '145c19485eb5dce46ceea6df7aebc8e363b2a933'
PRIVATE = Path('/home/harsh/.codex-tmp/fe2o3-mixed-lifecycle-20260924-rzYF3c6z')
CHECK = Path('crates/fe2o3-runtime-model/verus/check-owner-mixed-lifecycle.py')
TEST = CHECK.with_name('test-owner-mixed-lifecycle.py')
HELPER = Path('docs/evidence/dev-mixed-acquisition-proof-2026-09-24/audit.py')
COLLECTOR = Path('docs/evidence/dev-native-producer-mi300x-2026-09-24/collect.py')
PINS = {
    CHECK: 'ec0fb0c1ad835683321d8aad84dda11ad919c88c9542ff9db1ede84584531e59',
    TEST: 'a066630935071cf9bc30404d27c70655f3f298292d49c051c1bb1f9eabbf8f21',
    HELPER: 'b33da903896766a11dc61d3c26b4d60e1bbe983eef7be8c066c83fd66118b1f2',
    COLLECTOR: '9c9c629aa7ce73e52f565dee51f89810ae96a152ad3b2fb4c9f272f864c3f9cf',
}
ENV = {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}
ABSENCE = 'import os,sys; assert not os.path.lexists(sys.argv[1])'


def need(value, message):
    if not value:
        raise ValueError(message)


def module(path, pin, name):
    need(path.resolve() == path and stat.S_ISREG(path.lstat().st_mode), 'ordinary resolved helper')
    data = path.read_bytes()
    need(hashlib.sha256(data).hexdigest() == pin, 'authenticate helper before import')
    result = types.ModuleType(name)
    result.__file__ = str(path)
    sys.modules[name] = result
    exec(compile(data, str(path), 'exec'), result.__dict__)
    return result


def load(repo):
    old = module(repo / HELPER, PINS[HELPER], 'mixed_lifecycle_packet_helpers')
    old.verify_signature(repo, SOURCE)
    for path, pin in PINS.items():
        data = old.ordinary(repo, path)
        need(old.sha(data) == pin and data == old.git(repo, 'show', SOURCE + ':' + str(path)),
             'signed inherited control: ' + str(path))
    checker = module(repo / CHECK, PINS[CHECK], 'mixed_lifecycle_packet_checker')
    return old, checker


def replay(repo, raw, old, checker):
    need(old.unique_json(old.ordinary(raw, Path('source.json')))['commit'] == SOURCE, 'qualified source commit')
    saved = sys.argv
    try:
        sys.argv = [str(repo / CHECK), '--repo', str(repo), '--output', str(raw), '--verify']
        checker.main()
    finally:
        sys.argv = saved


def control_paths():
    return set(PINS) | {PACKET / p for p in ('audit.py', 'collect.py', 'test_audit.py')}


def commands(repo):
    python = ['/usr/bin/python3', '-I', '-B']
    raw = str(repo / PACKET / 'raw/campaign2')
    return {
        'replay': python + [str(repo / CHECK), '--output', raw, '--verify'],
        'calibration': python + [str(repo / TEST), '--campaign', raw],
        'absence': python + ['-c', ABSENCE, str(PRIVATE)],
    }


def check_publication(repo, packet, old):
    def read(path):
        return old.unique_json(old.ordinary(packet, Path('publication') / path))
    source = old.unique_json(old.ordinary(packet, Path('raw/campaign2/source.json')))
    original_repo = Path(source['recorded_repo'])
    need(Path(source['recorded_output']) == PRIVATE / 'campaign2', 'exact owned campaign location')
    expected = commands(original_repo)
    publication_files = {Path(p) for p in ('controls.json', 'inputs-before.json', 'inputs-after.json',
                         'cleanup-before.json', 'cleanup-after.json', 'cleanup-source.json')}
    publication_files |= {Path('commands') / name / filename for name in expected
                          for filename in ('record.json', 'stdout.log', 'stderr.log')}
    need(old.files(packet / 'publication') == publication_files, 'closed publication file roster')
    need(read(Path('controls.json')) == {'source': SOURCE, 'environment': ENV, 'commands': expected,
         'cwd': str(original_repo), 'timeout_seconds': 900}, 'closed publication controls')
    before, after = read(Path('inputs-before.json')), read(Path('inputs-after.json'))
    need(before == after and set(before) == {str(p) for p in control_paths()}, 'publication control bracket')
    for path in control_paths():
        need(old.sha(old.ordinary(repo, path)) == before[str(path)], 'unchanged publication control')
    need({p.name for p in (packet / 'publication/commands').iterdir()} == set(expected), 'publication command roster')
    transcripts = {'replay': 'PASS: mixed-lifecycle campaign replay\n',
                  'calibration': 'PASS: mixed-lifecycle checker calibration (10 groups)\n', 'absence': ''}
    rows, end = {}, 0
    for name, command in expected.items():
        root = Path('commands') / name
        need(old.files(packet / 'publication' / root) == {Path(p) for p in ('record.json', 'stdout.log', 'stderr.log')},
             'closed publication receipt files')
        row = read(root / 'record.json')
        need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}
             and type(row['status']) is int and row['status'] == 0 and row['group_absent'] is True
             and all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group')),
             'successful terminal publication receipt')
        bound = 30 if name == 'absence' else 900
        need(row['command'] == command and end <= row['started_ns'] < row['finished_ns']
             and row['finished_ns'] - row['started_ns'] <= (bound + 10) * 10**9, 'ordered bounded publication command')
        need(old.ordinary(packet, Path('publication') / root / 'stdout.log').decode() == transcripts[name],
             'exact publication transcript')
        signatures = 4 if name == 'replay' else 20 if name == 'calibration' else 0
        need(old.ordinary(packet, Path('publication') / root / 'stderr.log').decode() == old.SIGNATURE_LINE * signatures,
             'only expected signature diagnostics')
        rows[name], end = row, row['finished_ns']
    raw = {str(p): old.sha(old.ordinary(packet / 'raw', p)) for p in old.files(packet / 'raw')}
    initial, terminal = read(Path('cleanup-before.json')), read(Path('cleanup-after.json'))
    need(set(initial) == {'path', 'retained_files', 'allocated_bytes', 'known_terminal_groups', 'scope', 'absent'}
         and initial['path'] == str(PRIVATE) and initial['retained_files'] == raw
         and type(initial['allocated_bytes']) is int and initial['allocated_bytes'] >= 0
         and initial['scope'] == 'exact-owned-path-and-recorded-groups-not-global-process-absence'
         and initial['absent'] is False and terminal['absent'] is True
         and terminal == initial | {'absent': True}, 'exact retention and cleanup records')
    snapshot = read(Path('cleanup-source.json'))
    directories = {str(p.relative_to(packet / 'raw')) for p in (packet / 'raw').rglob('*') if p.is_dir()} | {'.'}
    need(set(snapshot) == set(raw) | directories, 'exact retained snapshot roster')
    root = snapshot['.']
    for path, row in snapshot.items():
        need(type(row) is list and len(row) == 6
             and all(type(v) is int and v >= 0 for v in row[:5])
             and row[1:3] == root[1:3] and row[3] > 0, 'ordinary snapshot metadata')
        if path in raw:
            need(stat.S_ISREG(row[0]) and row[4] == len(old.ordinary(packet / 'raw', Path(path)))
                 and row[5] == raw[path], 'exact retained regular-file snapshot')
        else:
            need(stat.S_ISDIR(row[0]) and row[5] is None, 'ordinary retained directory snapshot')
    need({p: row[5] for p, row in snapshot.items() if row[5] is not None} == raw, 'retained source snapshot')
    prior = old.unique_json(old.ordinary(packet, Path('raw/campaign1/source.json')))
    need(prior['commit'] == PRIOR_SOURCE, 'retained rejected source identity')
    prior_rows = [old.unique_json(p.read_bytes()) for p in sorted((packet / 'raw/campaign1').glob('*/record.json'))]
    proof_rows = [old.unique_json(p.read_bytes()) for p in sorted((packet / 'raw/campaign2').glob('*/record.json'))]
    for row in prior_rows:
        need(type(row['status']) is int and row['status'] in (0, 1) and row['group_absent'] is True
             and all(type(row[k]) is int and row[k] > 0 for k in ('process_group', 'started_ns', 'finished_ns'))
             and row['started_ns'] <= row['finished_ns'], 'terminal retained rejected-campaign command')
    groups = [r['process_group'] for r in prior_rows + proof_rows]
    groups += [rows[name]['process_group'] for name in ('calibration', 'replay')]
    need(len(prior_rows) == 12 and len(proof_rows) == 16 and initial['known_terminal_groups'] == groups
         and all(type(group) is int and group > 0 for group in initial['known_terminal_groups']), 'exact recorded cleanup groups')
    need(max(r['finished_ns'] for r in prior_rows + proof_rows) <= rows['replay']['started_ns'], 'publication follows completed campaigns')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--packet', type=Path)
    parser.add_argument('--unpublished', action='store_true')
    args = parser.parse_args()
    repo = args.repo.resolve()
    packet = args.packet.resolve() if args.packet else repo / PACKET
    old, checker = load(repo)
    sealed = old.check_seal(packet)
    if not args.unpublished:
        commit = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
        old.verify_signature(repo, commit)
        tracked = {Path(p).relative_to(PACKET) for p in
                   old.git(repo, 'ls-tree', '-r', '--name-only', commit, '--', str(PACKET)).decode().splitlines()}
        need(tracked == set(sealed) | {Path('SHA256SUMS')}, 'exact signed evidence roster')
        for path in tracked:
            need(old.git(repo, 'show', commit + ':' + str(PACKET / path)) == old.ordinary(packet, path), 'signed evidence bytes')
    check_publication(repo, packet, old)
    replay(repo, packet / 'raw/campaign2', old, checker)
    need(old.check_seal(packet) == sealed, 'packet unchanged during replay')
    print('PASS: mixed-lifecycle packet ' + ('seal/replay (unpublished)' if args.unpublished else 'signed seal/replay'))


if __name__ == '__main__':
    main()
