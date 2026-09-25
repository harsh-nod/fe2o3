#!/usr/bin/env python3
"""Replay the signed completion-effects packet without promoting Context claims."""
import argparse
import hashlib
from pathlib import Path
import stat
import sys
import types

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

PACKET = Path('docs/evidence/dev-completion-journal-effects-2026-09-25')
SOURCE = '957c5ce64b9a8e7cfd43fa4a5172b58ba3aa86c2'
HELPER = Path('docs/evidence/dev-mixed-acquisition-proof-2026-09-24/audit.py')
HELPER_SHA = 'b33da903896766a11dc61d3c26b4d60e1bbe983eef7be8c066c83fd66118b1f2'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-completion-journal-effects.py')
TEST = CHECK.with_name('test-completion-journal-effects.py')
PINS = {CHECK: 'aa66c149b5f694f4604154c34bd1a63308520bd0aebcf16e6f755f80f2f2ec7d',
        TEST: 'ef5fd7c17396c08f1adc37af2603510ceea064b84fd1a199d0b87e1b53dae6d5'}
PRIVATE = {
    'development': Path('/home/harsh/.codex-tmp/fe2o3-completion-effects-20260925-a9zy6WtN'),
    'qualification': Path('/home/harsh/.codex-tmp/fe2o3-completion-qualification-20260925-q7ssMxzt'),
}
COLLECTOR = Path('docs/evidence/dev-native-producer-mi300x-2026-09-24/collect.py')
COLLECTOR_SHA = '9c9c629aa7ce73e52f565dee51f89810ae96a152ad3b2fb4c9f272f864c3f9cf'
SCOPE = 'exact-owned-paths-and-recorded-groups-not-global-process-absence'
ENV = {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}
CONTROLS = {PACKET / name for name in ('audit.py', 'collect.py', 'test_audit.py')} | {
    HELPER, CHECK, TEST, COLLECTOR, COLLECTOR.with_name('verify.py')}


def module(path, pin, name):
    if path.resolve() != path or not path.is_file():
        raise ValueError('ordinary pinned helper')
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != pin:
        raise ValueError('authenticate helper before import')
    result = types.ModuleType(name)
    result.__file__ = str(path)
    sys.modules[name] = result
    exec(compile(data, str(path), 'exec'), result.__dict__)
    return result


def load(repo):
    old = module(repo / HELPER, HELPER_SHA, 'completion_effect_packet_helpers')
    old.verify_signature(repo, SOURCE)
    for path, pin in PINS.items():
        data = old.ordinary(repo, path)
        old.need(old.sha(data) == pin and data == old.git(repo, 'show', SOURCE + ':' + str(path)), 'signed replay control')
    old.need(old.ordinary(repo, HELPER) == old.git(repo, 'show', SOURCE + ':' + str(HELPER)), 'signed packet helper')
    checker = module(repo / CHECK, PINS[CHECK], 'completion_effect_packet_checker')
    return old, checker


def campaign_identity(campaign, old):
    source = old.unique_json(old.ordinary(campaign, Path('source.json')))
    old.need(source['commit'] == SOURCE and source['recorded_output'] == str(PRIVATE['qualification'] / 'campaign1'),
             'qualified source and original campaign location')


def replay(repo, campaign, old, checker):
    campaign_identity(campaign, old)
    saved = sys.argv
    try:
        sys.argv = [str(repo / CHECK), '--output', str(campaign), '--verify']
        checker.main()
    finally:
        sys.argv = saved


def recorded_groups(packet, old):
    result = []
    for folder, count in (('raw/qualification/campaign1', 38),
                          ('raw/qualification/mutation-development', 30)):
        rows = [old.unique_json(old.ordinary(packet, p.relative_to(packet)))
                for p in sorted((packet / folder).glob('*/record.json'))]
        old.need(len(rows) == count, 'complete terminal proof command roster')
        for row in rows:
            old.need(type(row['process_group']) is int and row['process_group'] > 0
                     and type(row['status']) is int and row['status'] in (0, 1)
                     and row['group_absent'] is True, 'terminal proof command group')
            result.append(row['process_group'])
    return result


def check_cleanup(packet, old):
    repo = packet.parents[2]
    controls = {str(p): old.sha(old.ordinary(repo, p)) for p in CONTROLS}
    for name in ('controls-before.json', 'controls-after.json'):
        old.need(old.unique_json(old.ordinary(packet, Path('publication') / name)) == controls, 'collection control continuity')
    before = old.unique_json(old.ordinary(packet, Path('cleanup-before.json')))
    after = old.unique_json(old.ordinary(packet, Path('cleanup-after.json')))
    old.need(set(before) == {'scope', 'roots', 'known_terminal_groups'} and before['scope'] == SCOPE,
             'exact cleanup scope')
    old.need(type(before['roots']) is dict and set(before['roots']) == set(PRIVATE), 'closed owned roots')
    for name, source in PRIVATE.items():
        row = before['roots'][name]
        old.need(set(row) == {'path', 'snapshot', 'retained_files', 'allocated_bytes'} and row['path'] == str(source)
                 and type(row['allocated_bytes']) is int and row['allocated_bytes'] > 0, 'owned cleanup root')
        snapshot = row['snapshot']
        old.need(type(snapshot) is dict and '.' in snapshot and all(type(v) is list and len(v) == 6 for v in snapshot.values()),
                 'complete owned snapshot schema')
        old.need(snapshot['.'][1] == 1000 and snapshot['.'][5] is None, 'owned root identity')
        for path, value in snapshot.items():
            name_path = Path(path)
            mode, uid, device, inode, size, digest = value
            old.need(str(name_path) == path and not name_path.is_absolute() and '..' not in name_path.parts
                     and all(type(v) is int for v in value[:5]) and uid == 1000
                     and device == snapshot['.'][2] and inode > 0 and size >= 0
                     and (stat.S_ISDIR(mode) and digest is None or stat.S_ISREG(mode) and type(digest) is str),
                     'ordinary same-device owned snapshot member')
        raw = packet / 'raw' / name
        old.need(set(snapshot) == {'.'} | {str(p.relative_to(raw)) for p in raw.rglob('*')}, 'complete retained tree shape')
        retained = {str(p): old.sha(old.ordinary(raw, p)) for p in old.files(raw)}
        old.need(row['retained_files'] == retained and {p: v[5] for p, v in snapshot.items() if v[5] is not None} == retained,
                 'complete byte-exact scratch retention')
    old.need(set(after) == {'scope', 'paths', 'absent'} and after['absent'] is True
             and after == {'scope': SCOPE, 'paths': [str(p) for p in PRIVATE.values()], 'absent': True}, 'recorded owned-path absence')
    replay_row = old.unique_json(old.ordinary(packet, Path('publication/replay/record.json')))
    old.need(type(before['known_terminal_groups']) is list
             and all(type(group) is int and group > 0 for group in before['known_terminal_groups']), 'strict terminal group identities')
    old.need(before['known_terminal_groups'] == recorded_groups(packet, old) + [replay_row['process_group']], 'exact terminal groups')
    for name, expected_stdout in (('replay', 'PASS: completion journal effects replay\n'), ('absence', '')):
        folder = packet / 'publication' / name
        row = old.unique_json(old.ordinary(folder, Path('record.json')))
        old.need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}
                 and type(row['status']) is int and row['status'] == 0 and row['group_absent'] is True
                 and all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group'))
                 and 0 < row['finished_ns'] - row['started_ns'] <= (900 if name == 'replay' else 30) * 10**9,
                 'bounded successful publication receipt')
        original = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
        expected = ['/usr/bin/python3', '-I', '-B', str(original / CHECK), '--output',
                    str(original / PACKET / 'raw/qualification/campaign1'), '--verify'] if name == 'replay' else [
                    '/usr/bin/python3', '-I', '-B', '-c',
                    'import os,sys; assert all(not os.path.lexists(p) for p in sys.argv[1:])', *map(str, PRIVATE.values())]
        old.need(row['command'] == expected and old.ordinary(folder, Path('stdout.log')).decode() == expected_stdout,
                 'exact publication command and transcript')
        stderr = old.ordinary(folder, Path('stderr.log')).decode()
        old.need(stderr == (old.SIGNATURE_LINE * 7 if name == 'replay' else ''), 'only expected signature diagnostics')
    old.need(replay_row['finished_ns'] <= old.unique_json(old.ordinary(packet, Path('publication/absence/record.json')))['started_ns'],
             'replay before removal/absence')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--unpublished', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    packet = repo / PACKET
    old, checker = load(repo)
    sealed = old.check_seal(packet)
    if not args.unpublished:
        head = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
        old.verify_signature(repo, head)
        tracked = {Path(p).relative_to(PACKET) for p in old.git(repo, 'ls-tree', '-r', '--name-only', head, '--', str(PACKET)).decode().splitlines()}
        old.need(tracked == set(sealed) | {Path('SHA256SUMS')}, 'signed packet roster')
        for path in tracked:
            old.need(old.ordinary(packet, path) == old.git(repo, 'show', head + ':' + str(PACKET / path)), 'signed packet bytes')
    check_cleanup(packet, old)
    replay(repo, packet / 'raw/qualification/campaign1', old, checker)
    old.need(old.check_seal(packet) == sealed, 'packet unchanged during replay')
    print('PASS: completion effects packet ' + ('seal/replay (unpublished)' if args.unpublished else 'signed seal/replay'))


if __name__ == '__main__':
    main()
