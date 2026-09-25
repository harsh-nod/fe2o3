#!/usr/bin/env python3
"""Calibrate the packet's closed retention and publication-receipt checks."""
import json
from pathlib import Path
import runpy
import shutil
import sys
import tempfile

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
A = runpy.run_path(str(HERE / 'audit.py'))
OLD = A['module'](REPO / A['HELPER'], A['HELPER_SHA'], 'completion_packet_test_helpers')


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, sort_keys=True, indent=2) + '\n')


def reject(function, *args):
    try:
        function(*args)
    except ValueError:
        return
    raise AssertionError('accepted corrupt completion packet')


def fixture(repo):
    packet = repo / A['PACKET']
    for path in A['CONTROLS']:
        destination = repo / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / path, destination)
    controls = {str(p): OLD.sha(OLD.ordinary(repo, p)) for p in A['CONTROLS']}
    for name in ('controls-before.json', 'controls-after.json'):
        write(packet / 'publication' / name, controls)
    (packet / 'raw/development').mkdir(parents=True)
    (packet / 'raw/development/result.txt').write_text('diagnostic only\n')
    for folder, count in (('campaign1', 38), ('mutation-development', 30)):
        for i in range(count):
            write(packet / 'raw/qualification' / folder / f'case-{i:02}' / 'record.json',
                  {'process_group': 1000 + i + (0 if folder == 'campaign1' else 100),
                   'status': 0 if folder == 'campaign1' else 1, 'group_absent': True})
    groups = A['recorded_groups'](packet, OLD)
    original = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
    replay = ['/usr/bin/python3', '-I', '-B', str(original / A['CHECK']), '--output',
              str(original / A['PACKET'] / 'raw/qualification/campaign1'), '--verify']
    absence = ['/usr/bin/python3', '-I', '-B', '-c',
               'import os,sys; assert all(not os.path.lexists(p) for p in sys.argv[1:])', *map(str, A['PRIVATE'].values())]
    for i, (name, command, stdout, stderr) in enumerate((
        ('replay', replay, 'PASS: completion journal effects replay\n', OLD.SIGNATURE_LINE * 7),
        ('absence', absence, '', ''),
    )):
        folder = packet / 'publication' / name
        write(folder / 'record.json', {'command': command, 'started_ns': 10 + i * 10,
              'finished_ns': 15 + i * 10, 'process_group': 2000 + i, 'status': 0, 'group_absent': True})
        (folder / 'stdout.log').write_text(stdout)
        (folder / 'stderr.log').write_text(stderr)
    roots = {}
    for name, source in A['PRIVATE'].items():
        raw = packet / 'raw' / name
        files = {str(p): OLD.sha(OLD.ordinary(raw, p)) for p in OLD.files(raw)}
        snapshot = {'.': [0o40700, 1000, 1, 1, 0, None]}
        for i, path in enumerate(sorted(raw.rglob('*')), 2):
            relative = str(path.relative_to(raw))
            snapshot[relative] = [0o40700 if path.is_dir() else 0o100600, 1000, 1, i,
                                  path.stat().st_size, files.get(relative)]
        roots[name] = {'path': str(source), 'snapshot': snapshot, 'retained_files': files, 'allocated_bytes': 4096}
    write(packet / 'cleanup-before.json', {'scope': A['SCOPE'], 'roots': roots, 'known_terminal_groups': groups + [2000]})
    write(packet / 'cleanup-after.json', {'scope': A['SCOPE'], 'paths': list(map(str, A['PRIVATE'].values())), 'absent': True})
    return packet


def main():
    with tempfile.TemporaryDirectory(prefix='completion-packet-calibration-') as temporary:
        repo = Path(temporary)
        packet = fixture(repo)
        check = A['check_cleanup']
        check(packet, OLD)

        # 1. Authenticate executable helpers before importing them.
        payload = repo / 'untrusted.py'
        payload.write_text('raise AssertionError("executed untrusted helper")\n')
        reject(A['module'], payload, '0' * 64, 'bad_helper')
        linked = repo / 'linked.py'
        linked.symlink_to(payload)
        reject(A['module'], linked, OLD.sha(payload.read_bytes()), 'bad_link')

        def corrupt(path, edit):
            original = (packet / path).read_bytes()
            value = json.loads(original)
            edit(value)
            write(packet / path, value)
            reject(check, packet, OLD)
            (packet / path).write_bytes(original)

        # 2. Every root, owner, field and complete retained tree is bound.
        for edit in (
            lambda v: v.update(scope='global-absence'),
            lambda v: v['roots']['development'].update(path='/tmp/foreign'),
            lambda v: v['roots']['development'].update(allocated_bytes=True),
            lambda v: v['roots']['development']['snapshot']['.'].__setitem__(1, 0),
            lambda v: v['roots']['development']['snapshot']['result.txt'].__setitem__(1, True),
            lambda v: v['roots']['development']['snapshot']['result.txt'].__setitem__(2, 2),
            lambda v: v['roots']['development']['snapshot'].__setitem__('extra', [0o40700, 1000, 1, 9, 0, None]),
            lambda v: v['roots']['development']['retained_files'].update({'result.txt': '0' * 64}),
        ):
            corrupt(Path('cleanup-before.json'), edit)
        extra = packet / 'raw/development/unretained'
        extra.write_text('extra\n')
        reject(check, packet, OLD)
        extra.unlink()

        # 3. Exact proof-group counts and terminal classifications are required.
        receipt = Path('raw/qualification/campaign1/case-00/record.json')
        for key, value in (('process_group', True), ('status', True), ('status', 124), ('group_absent', False)):
            corrupt(receipt, lambda row, k=key, v=value: row.update({k: v}))
        corrupt(Path('cleanup-before.json'), lambda row: row['known_terminal_groups'].pop())
        corrupt(Path('cleanup-before.json'), lambda row: row['known_terminal_groups'].__setitem__(0, float(row['known_terminal_groups'][0])))

        # 4. Absence and publication receipts cannot be substituted or reordered.
        for edit in (lambda row: row.update(absent=False), lambda row: row.update(absent=1), lambda row: row['paths'].reverse()):
            corrupt(Path('cleanup-after.json'), edit)
        for name in ('replay', 'absence'):
            for edit in (lambda row: row.update(status=True), lambda row: row.update(group_absent=False),
                         lambda row: row.update(finished_ns=10**15), lambda row: row.update(started_ns=0),
                         lambda row: row['command'].append('--skip-checks')):
                corrupt(Path('publication') / name / 'record.json', edit)
        corrupt(Path('publication/absence/record.json'), lambda row: row.update(started_ns=1))
        stderr = packet / 'publication/replay/stderr.log'
        original = stderr.read_bytes()
        stderr.write_text('unexpected diagnostic\n')
        reject(check, packet, OLD)
        stderr.write_bytes(original)

        # 5. Recomputed data hashes cannot replace the actual collection controls.
        corrupt(Path('publication/controls-after.json'), lambda row: row.update({str(A['CHECK']): '0' * 64}))
        control_path = repo / A['COLLECTOR']
        original = control_path.read_bytes()
        control_path.write_bytes(original + b'\n')
        reject(check, packet, OLD)
        control_path.write_bytes(original)
        check(packet, OLD)
        campaign = repo / 'identity-test'
        identity = {'commit': A['SOURCE'], 'recorded_output': str(A['PRIVATE']['qualification'] / 'campaign1')}
        write(campaign / 'source.json', identity)
        A['campaign_identity'](campaign, OLD)
        for key, value in (('commit', '0' * 40), ('recorded_output', '/tmp/foreign/campaign1')):
            write(campaign / 'source.json', identity | {key: value})
            reject(A['campaign_identity'], campaign, OLD)

        # 6. The packet seal includes every raw artifact and every control file.
        write(packet / 'artifacts.json', {'schema': 1, 'files': {
            str(p): OLD.sha(OLD.ordinary(packet, p)) for p in OLD.files(packet) if p.parts[0] == 'raw'}})
        seal = ''.join(OLD.sha(OLD.ordinary(packet, p)) + '  ' + str(p) + '\n' for p in sorted(OLD.files(packet)))
        (packet / 'SHA256SUMS').write_text(seal)
        OLD.check_seal(packet)
        extra = packet / 'unsigned.txt'
        extra.write_text('unsealed\n')
        reject(OLD.check_seal, packet)
        extra.unlink()
        (packet / 'SHA256SUMS').write_text(seal + seal.splitlines()[0] + '\n')
        reject(OLD.check_seal, packet)
    print('PASS: completion effects packet calibration (6 groups)')


if __name__ == '__main__':
    main()
