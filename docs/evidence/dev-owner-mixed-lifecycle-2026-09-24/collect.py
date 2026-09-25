#!/usr/bin/env python3
"""Retain the completed campaign before removing its exact owned scratch tree."""
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

import fcntl
import hashlib
from contextlib import ExitStack
import json
import os
from pathlib import Path
import shutil
import signal
import types

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
AUDIT_SHA = '7fbc7e6edafe156be17909de0299be511061c488ac8a8674f09977569d5e169a'
AUDIT_BYTES = (HERE / 'audit.py').read_bytes()
if (HERE / 'audit.py').is_symlink() or hashlib.sha256(AUDIT_BYTES).hexdigest() != AUDIT_SHA:
    raise ValueError('authenticate frozen publication auditor before import')
A = types.ModuleType('mixed_lifecycle_collection_audit')
A.__file__ = str(HERE / 'audit.py')
sys.modules[A.__name__] = A
exec(compile(AUDIT_BYTES, A.__file__, 'exec'), A.__dict__)
COUNTS = {'checker': 5, 'history': 1, 'preservation': 1, 'release': 2,
          'settlement': 2, 'whole': 4, 'witness': 5}
ROSTER = {f'{stem}-{n}.{stream}' for stem, count in COUNTS.items()
          for n in range(1, count + 1) for stream in ('stdout', 'stderr')}
ROSTER |= {'campaign1', 'campaign1.lock', 'campaign2', 'campaign2.lock', 'atomic-event-dev-Jg2UzL2t'}
ROSTER |= {f'{stem}.{stream}' for stem in ('tool-closure', 'campaign1-initial', 'campaign1-resume',
                                        'campaign2', 'atomic-event-1', 'atomic-classification-1')
           for stream in ('stdout', 'stderr')}


def write(path, value):
    with path.open('x') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


def main():
    A.need(Path.cwd() == REPO and HERE == REPO / A.PACKET, 'exact collection working directory')
    A.need(A.PRIVATE.resolve(strict=True) == A.PRIVATE, 'exact ordinary scratch path')
    old, checker = A.load(REPO)
    cleanup = A.module(REPO / A.COLLECTOR, A.PINS[A.COLLECTOR], 'mixed_lifecycle_cleanup_helpers')
    _, _, deps, _, _ = checker.previous(REPO)
    base = deps['base']
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)

    def inputs():
        return {str(p): old.sha(old.ordinary(REPO, p)) for p in sorted(A.control_paths())}

    def inventory(root):
        return {str(p): old.sha(old.ordinary(root, p)) for p in sorted(old.files(root))}

    def snapshot():
        A.need({p.name for p in A.PRIVATE.iterdir()} == ROSTER, 'closed owned scratch roster')
        result = cleanup.owned_snapshot(A.PRIVATE)
        A.need(all((A.PRIVATE / p).lstat().st_nlink == 1 for p, row in result.items() if row[5] is not None),
               'no hard-linked scratch files')
        return result

    def groups(root, count):
        result = []
        for path in sorted(root.glob('*/record.json')):
            row = old.unique_json(old.ordinary(root, path.relative_to(root)))
            A.need(type(row['process_group']) is int and row['process_group'] > 0
                   and type(row['status']) is int and row['status'] in (0, 1)
                   and row['group_absent'] is True and not base.group_exists(row['process_group']),
                   'recorded command group is terminal and absent')
            result.append(row['process_group'])
        A.need(len(result) == count, 'exact terminal command count')
        return result

    raw, output = HERE / 'raw', HERE / 'publication'
    A.need(not os.path.lexists(raw) and not os.path.lexists(output), 'fresh collection destinations')
    before = inputs()
    A.need(before[str(A.PACKET / 'audit.py')] == AUDIT_SHA, 'same loaded publication auditor')
    commands = A.commands(REPO)
    # Hold both campaign locks through copying, replay and exact-owned removal.
    with ExitStack() as stack:
        for name in ('campaign1', 'campaign2'):
            lock = stack.enter_context((A.PRIVATE / (name + '.lock')).open('rb'))
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        original = snapshot()
        terminal = groups(A.PRIVATE / 'campaign1', 12) + groups(A.PRIVATE / 'campaign2', 16)
        expected = inventory(A.PRIVATE)
        output.mkdir()
        (output / 'commands').mkdir()
        write(output / 'inputs-before.json', before)
        write(output / 'controls.json', {'source': A.SOURCE, 'environment': A.ENV,
              'commands': commands, 'cwd': str(REPO), 'timeout_seconds': 900})
        shutil.copytree(A.PRIVATE, raw)
        A.need(inventory(raw) == expected and snapshot() == original, 'complete byte-exact retention')
        transcripts = {'replay': ('PASS: mixed-lifecycle campaign replay\n', 4),
                       'calibration': ('PASS: mixed-lifecycle checker calibration (10 groups)\n', 20)}
        for name, (expected_stdout, signatures) in transcripts.items():
            status, stdout, stderr = base.run_owned(commands[name], 900, output / 'commands' / name, A.ENV)
            A.need(status == 0 and stdout == expected_stdout and stderr == old.SIGNATURE_LINE * signatures,
                   'exact successful publication stage: ' + name)
        publication_groups = groups(output / 'commands', 2)
        terminal += publication_groups
        allocated = sum(p.lstat().st_blocks * 512 for p in (A.PRIVATE, *A.PRIVATE.rglob('*')))
        row = {'path': str(A.PRIVATE), 'retained_files': expected, 'allocated_bytes': allocated,
               'known_terminal_groups': terminal,
               'scope': 'exact-owned-path-and-recorded-groups-not-global-process-absence', 'absent': False}
        write(output / 'cleanup-source.json', original)
        write(output / 'cleanup-before.json', row)
        A.need(inputs() == before and inventory(raw) == expected and snapshot() == original,
               'unchanged controls and both retained/source inventories before removal')
        A.need(groups(A.PRIVATE / 'campaign1', 12) + groups(A.PRIVATE / 'campaign2', 16)
               + groups(output / 'commands', 2) == terminal,
               'same absent command groups immediately before removal')
        shutil.rmtree(A.PRIVATE)
        A.need(not os.path.lexists(A.PRIVATE), 'owned scratch absent after removal')
        write(output / 'cleanup-after.json', row | {'absent': True})
    status, stdout, stderr = base.run_owned(commands['absence'], 30, output / 'commands/absence', A.ENV)
    A.need(status == 0 and not stdout and not stderr, 'independent owned-path absence')
    A.need(inputs() == before and inventory(raw) == expected, 'final control and retention continuity')
    write(output / 'inputs-after.json', inputs())
    A.check_publication(REPO, HERE, old)
    print(json.dumps({'retained_files': len(expected), 'removed_allocated_bytes': allocated, 'absent': True}))


if __name__ == '__main__':
    main()
