#!/usr/bin/env python3
"""Authenticate the signed packet and replay its control-flow proof campaign."""
import argparse
import hashlib
from pathlib import Path
import sys
import types

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

PACKET = Path('docs/evidence/dev-completion-settlement-control-2026-09-25')
SOURCE = '0885dd132c8472137e8c0a73c47ddfe46321136a'
HELPER = Path('docs/evidence/dev-mixed-acquisition-proof-2026-09-24/audit.py')
HELPER_SHA = 'b33da903896766a11dc61d3c26b4d60e1bbe983eef7be8c066c83fd66118b1f2'
CHECK = Path('crates/fe2o3-runtime-model/verus/check-completion-settlement-control.py')
CHECK_SHA = '0f7f2565d9e5c1461f67719ea9a82302a56ad6b872c44bc9b2edd1e6cfd50eb9'


def load(path, pin, name):
    if path.resolve() != path or not path.is_file():
        raise ValueError('ordinary helper required')
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != pin:
        raise ValueError('authenticate helper before import')
    module = types.ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--unpublished', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    packet = repo / PACKET
    old = load(repo / HELPER, HELPER_SHA, 'completion_packet_helpers')
    old.verify_signature(repo, SOURCE)
    for path, pin in ((HELPER, HELPER_SHA), (CHECK, CHECK_SHA)):
        data = old.ordinary(repo, path)
        old.need(old.sha(data) == pin and data == old.git(repo, 'show', SOURCE + ':' + str(path)), 'signed control')
    sealed = old.check_seal(packet)
    if not args.unpublished:
        head = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
        old.verify_signature(repo, head)
        tracked = {Path(p).relative_to(PACKET) for p in
                   old.git(repo, 'ls-tree', '-r', '--name-only', head, '--', str(PACKET)).decode().splitlines()}
        old.need(tracked == set(sealed) | {Path('SHA256SUMS')}, 'signed packet roster')
        for path in tracked:
            old.need(old.ordinary(packet, path) == old.git(repo, 'show', head + ':' + str(PACKET / path)), 'signed packet bytes')
    campaign = packet / 'raw/campaign1'
    source = old.unique_json(old.ordinary(campaign, Path('source.json')))
    old.need(source['commit'] == SOURCE, 'qualified source identity')
    before = old.unique_json(old.ordinary(packet, Path('cleanup-before.json')))
    after = old.unique_json(old.ordinary(packet, Path('cleanup-after.json')))
    paths = ['/home/harsh/.codex-tmp/fe2o3-completion-control-20260925-MZxYtw3L',
             '/dev/shm/fe2o3-completion-control-20260925-avNFWlxj']
    old.need(set(before) == {'roots', 'retained_files', 'known_terminal_groups', 'scope'}
             and [row['path'] for row in before['roots']] == paths
             and before['scope'] == 'exact-owned-paths-and-recorded-proof-groups-not-global-process-absence', 'cleanup scope')
    for row in before['roots']:
        old.need(set(row) == {'path', 'uid', 'inode', 'allocated_bytes', 'entries'}
                 and row['uid'] == 1000 and all(type(row[k]) is int and row[k] > 0
                    for k in ('uid', 'inode', 'allocated_bytes', 'entries')), 'owned cleanup metadata')
    old.need(set(after) == {'paths', 'absent'} and after['paths'] == paths and after['absent'] is True, 'recorded path absence')
    raw = {str(p): old.sha(old.ordinary(packet / 'raw', p)) for p in old.files(packet / 'raw')}
    old.need(before['retained_files'] == raw, 'complete scratch retention')
    rows = [old.unique_json(p.read_bytes()) for p in sorted(campaign.glob('*/record.json'))]
    old.need(len(rows) == 27 and before['known_terminal_groups'] == [row['process_group'] for row in rows], 'recorded proof groups')
    checker = load(repo / CHECK, CHECK_SHA, 'completion_packet_checker')
    saved = sys.argv
    try:
        sys.argv = [str(repo / CHECK), '--output', str(campaign), '--verify']
        checker.main()
    finally:
        sys.argv = saved
    old.need(old.check_seal(packet) == sealed, 'unchanged packet during replay')
    print('PASS: completion control packet ' + ('seal/replay (unpublished)' if args.unpublished else 'signed seal/replay'))


if __name__ == '__main__':
    main()
