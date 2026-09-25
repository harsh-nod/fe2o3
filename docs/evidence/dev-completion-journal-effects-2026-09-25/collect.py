#!/usr/bin/env python3
"""Copy both owned development trees, replay, then remove only exact snapshots."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import types

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
AUDIT_SHA = '9b58ee00ad6cafcbb6baaed2a1ca1f22f3284aff07f0af5272ea48d8289269fa'
path = HERE / 'audit.py'
data = path.read_bytes()
if path.resolve() != path or hashlib.sha256(data).hexdigest() != AUDIT_SHA:
    raise ValueError('authenticate publication auditor before import')
A = types.ModuleType('completion_effect_collection_audit')
A.__file__ = str(path)
sys.modules[A.__name__] = A
exec(compile(data, str(path), 'exec'), A.__dict__)


def write(path, value):
    with path.open('x') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


def main():
    old, checker = A.load(REPO)
    old.need(Path.cwd() == REPO and HERE == REPO / A.PACKET, 'exact collection location')
    for path in (A.COLLECTOR, A.COLLECTOR.with_name('verify.py')):
        old.need(old.ordinary(REPO, path) == old.git(REPO, 'show', A.SOURCE + ':' + str(path)), 'signed cleanup helper')
    cleanup = A.module(REPO / A.COLLECTOR, A.COLLECTOR_SHA, 'completion_effect_cleanup')
    deps = checker.load(REPO)[4]
    base = deps['base']
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)

    def controls():
        return {str(p): old.sha(old.ordinary(REPO, p)) for p in A.CONTROLS}

    def inventory(root):
        return {str(p): old.sha(old.ordinary(root, p)) for p in old.files(root)}

    def snapshot(root):
        result = cleanup.owned_snapshot(root)
        old.need(all((root / p).lstat().st_nlink == 1 for p, row in result.items() if row[5] is not None), 'no hard links')
        return result

    before = controls()
    raw, publication = HERE / 'raw', HERE / 'publication'
    old.need(not os.path.lexists(raw) and not os.path.lexists(publication), 'fresh retention destinations')
    lock_path = A.PRIVATE['qualification'] / 'campaign1.lock'
    with lock_path.open('rb') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        snapshots = {name: snapshot(root) for name, root in A.PRIVATE.items()}
        inventories = {name: inventory(root) for name, root in A.PRIVATE.items()}
        publication.mkdir()
        write(publication / 'controls-before.json', before)
        raw.mkdir()
        for name, source in A.PRIVATE.items():
            shutil.copytree(source, raw / name)
            old.need(inventory(raw / name) == inventories[name] and snapshot(source) == snapshots[name], 'byte-exact full retention')
        groups = A.recorded_groups(HERE, old)
        old.need(all(not base.group_exists(group) for group in groups), 'all recorded proof groups absent')
        A.campaign_identity(raw / 'qualification/campaign1', old)
        command = ['/usr/bin/python3', '-I', '-B', str(REPO / A.CHECK), '--output',
                   str(raw / 'qualification/campaign1'), '--verify']
        status, stdout, stderr = base.run_owned(command, 900, publication / 'replay', A.ENV)
        old.need(status == 0 and stdout == 'PASS: completion journal effects replay\n'
                 and stderr == old.SIGNATURE_LINE * 7, 'exact successful retained replay')
        replay_row = old.unique_json(old.ordinary(publication / 'replay', Path('record.json')))
        groups.append(replay_row['process_group'])
        roots = {}
        for name, source in A.PRIVATE.items():
            roots[name] = {'path': str(source), 'snapshot': snapshots[name], 'retained_files': inventories[name],
                           'allocated_bytes': sum(p.lstat().st_blocks * 512 for p in (source, *source.rglob('*')))}
        write(HERE / 'cleanup-before.json', {'scope': A.SCOPE, 'roots': roots, 'known_terminal_groups': groups})
        old.need(controls() == before and all(not base.group_exists(group) for group in groups), 'controls and terminal groups unchanged')
        for name, source in A.PRIVATE.items():
            old.need(inventory(raw / name) == inventories[name] and snapshot(source) == snapshots[name], 'final source/retention continuity')
        for name, source in A.PRIVATE.items():
            cleanup.remove_owned(source, snapshots[name])
            old.need(not os.path.lexists(source), 'exact owned path absent')
        write(HERE / 'cleanup-after.json', {'scope': A.SCOPE, 'paths': [str(p) for p in A.PRIVATE.values()], 'absent': True})
    command = ['/usr/bin/python3', '-I', '-B', '-c',
               'import os,sys; assert all(not os.path.lexists(p) for p in sys.argv[1:])', *map(str, A.PRIVATE.values())]
    status, stdout, stderr = base.run_owned(command, 30, publication / 'absence', A.ENV)
    old.need(status == 0 and not stdout and not stderr and controls() == before, 'independent absence and control continuity')
    write(publication / 'controls-after.json', controls())
    old.need(all(inventory(raw / name) == inventories[name] for name in A.PRIVATE), 'retention unchanged after removal')
    A.check_cleanup(HERE, old)
    print(json.dumps({'retained_files': sum(len(v) for v in inventories.values()),
                      'removed_allocated_bytes': sum(row['allocated_bytes'] for row in roots.values()), 'absent': True}))


if __name__ == '__main__':
    main()
