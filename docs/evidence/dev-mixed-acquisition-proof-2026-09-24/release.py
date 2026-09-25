#!/usr/bin/env python3
"""Bind both post-collection attempts, then run the signed packet audit."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys
import tempfile
import types

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
AUDIT_SHA = 'b33da903896766a11dc61d3c26b4d60e1bbe983eef7be8c066c83fd66118b1f2'
data = (HERE / 'audit.py').read_bytes()
if (HERE / 'audit.py').is_symlink() or hashlib.sha256(data).hexdigest() != AUDIT_SHA:
    raise ValueError('pinned corrected auditor before import')
A = types.ModuleType('mixed_release_audit')
A.__file__ = str(HERE / 'audit.py')
exec(compile(data, A.__file__, 'exec'), A.__dict__)


def read(root, path):
    return A.unique_json(A.ordinary(root, Path(path)))


def post_attempts(repo, packet):
    source = read(packet, 'raw/campaign1/source.json')
    original = Path(source['recorded_repo']) / A.PACKET
    command = ['/usr/bin/python3', '-I', '-B', str(original / 'test_audit.py'),
               '--raw', str(original / 'raw/campaign1'), '--finalization-packet', str(original)]
    finished = read(packet, 'audit1/commands/absence/record.json')['finished_ns']
    for folder, status in (('post-collection-tests', 1), ('post-collection-tests2', 0)):
        root = packet / folder
        row = read(root, 'record.json')
        A.need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}
               and type(row['status']) is int and row['status'] == status and row['group_absent'] is True
               and row['command'] == command
               and all(type(row[k]) is int and row[k] > 0 for k in ('started_ns', 'finished_ns', 'process_group')),
               'exact terminal post-collection attempt')
        A.need(finished <= row['started_ns'] < row['finished_ns']
               and row['finished_ns'] - row['started_ns'] <= 910 * 10**9, 'ordered bounded post-collection attempt')
        finished = row['finished_ns']
        A.need(A.ordinary(root, Path('stderr.log')).decode() == A.SIGNATURE_LINE * 40, 'closed post-collection diagnostics')
        stdout = A.ordinary(root, Path('stdout.log')).decode()
        if status == 0:
            A.need(stdout == 'PASS: mixed-acquisition packet calibration (6 groups)\n', 'complete corrected calibration')
        else:
            A.need(stdout.startswith('.....E\n') and 'ERROR: test_finalization_corruptions ' in stdout
                   and 'ValueError: complete cleanup tests\n' in stdout
                   and re.search(r'\nRan 6 tests in [0-9]+\.[0-9]+s\n\nFAILED \(errors=1\)\n\n?\Z', stdout),
                   'preserved original parser failure')
    root = packet / 'post-collection-tests2'
    before, after = read(root, 'inputs-before.json'), read(root, 'inputs-after.json')
    old = read(packet, 'audit1/inputs-before.json')
    A.need(before == after and set(before) == set(old), 'closed corrected-audit control bracket')
    for name, digest in before.items():
        path = Path(name)
        if path == A.PACKET / 'README.md':
            current = A.ordinary(root, Path('README.measured.md'))
        elif path.is_relative_to(A.PACKET):
            current = A.ordinary(packet, path.relative_to(A.PACKET))
        else:
            current = A.ordinary(repo, path)
        A.need(A.sha(current) == digest, 'unchanged corrected-audit executable/control identity')


def self_test(repo, packet):
    post_attempts(repo, packet)
    changes = [
        ('post-collection-tests2/record.json', lambda v: v.update(status=True)),
        ('post-collection-tests/record.json', lambda v: v.update(status=0)),
        ('post-collection-tests2/record.json', lambda v: v.update(group_absent=False)),
        ('post-collection-tests2/record.json', lambda v: v.update(command=['substituted'])),
        ('post-collection-tests2/record.json', lambda v: v.update(finished_ns=1)),
        ('post-collection-tests2/inputs-after.json', lambda v: v.clear()),
    ]
    for path, mutate in changes:
        with tempfile.TemporaryDirectory(prefix='mixed-release-test-') as temporary:
            target = Path(temporary) / 'packet'
            shutil.copytree(packet, target)
            value = read(target, path)
            mutate(value)
            (target / path).write_text(json.dumps(value) + '\n')
            try:
                post_attempts(repo, target)
            except ValueError:
                pass
            else:
                raise ValueError('accepted corrupted publication receipt: ' + path)
    print('PASS: post-collection publication calibration (6 corruptions)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=REPO)
    parser.add_argument('--packet', type=Path, default=HERE)
    parser.add_argument('--unpublished', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    repo, packet = args.repo.resolve(), args.packet.resolve()
    if args.self_test:
        self_test(repo, packet)
        return
    post_attempts(repo, packet)
    saved = sys.argv
    try:
        sys.argv = [A.__file__, '--repo', str(repo), '--packet', str(packet)]
        if args.unpublished:
            sys.argv.append('--unpublished')
        A.main()
    finally:
        sys.argv = saved
    print('PASS: post-collection outcomes and corrected packet audit')


if __name__ == '__main__':
    main()
