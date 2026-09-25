#!/usr/bin/env python3
"""Replay, retain every local attempt, and remove only the owned proof scratch."""
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import types

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PRIVATE = Path('/home/harsh/.codex-tmp/fe2o3-mixed-proof-20260924-65fWJbkW')
COLLECT = REPO / 'docs/evidence/dev-native-producer-mi300x-2026-09-24'
VISIBILITY = REPO / 'docs/evidence/dev-mixed-input-acquisition-cpu-2026-09-24/finalize.py'
ENV = {'HOME': '/home/harsh', 'PATH': '/usr/bin:/bin', 'LC_ALL': 'C'}
DEVELOPMENT = {f'{stem}{number}.{stream}' for stem, count in (('dev', 9), ('checker-test', 4))
               for number in range(1, count + 1) for stream in ('stdout', 'stderr')}
ROSTER = DEVELOPMENT | {'campaign1', 'campaign1.stdout', 'campaign1.stderr', 'campaign1.lock'}


def need(value, message):
    if not value:
        raise ValueError(message)


def module(path, pin, name):
    need(path.resolve() == path and stat.S_ISREG(path.lstat().st_mode), 'ordinary pinned helper')
    data = path.read_bytes()
    need(hashlib.sha256(data).hexdigest() == pin, 'pinned helper identity')
    result = types.ModuleType(name)
    result.__file__ = str(path)
    sys.modules[name] = result
    exec(compile(data, str(path), 'exec'), result.__dict__)
    return result


A = module(HERE / 'audit.py', '5212f2e9697e14aec1b55a5b21468723e3d59186bfd359b57b61c836b39805be', 'mixed_final_audit')
C = module(COLLECT / 'collect.py', '9c9c629aa7ce73e52f565dee51f89810ae96a152ad3b2fb4c9f272f864c3f9cf', 'mixed_final_cleanup')
S = module(VISIBILITY, 'bf983ff06018137b73f02820e67ac03746a9a64ed71c4abb63f4be517e89875d', 'mixed_final_visibility')


def write(path, value):
    with path.open('x') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2) + '\n')


def inventory(root):
    return {str(p): A.sha(A.ordinary(root, p)) for p in sorted(A.files(root))}


def snapshot(root, roster):
    need({p.name for p in root.iterdir()} == roster, 'exact owned scratch roster')
    result = C.owned_snapshot(root)
    need(all(p.lstat().st_nlink == 1 for p in root.rglob('*') if p.is_file()), 'no hard-linked scratch files')
    return result


def terminal_groups(root, base):
    groups = []
    for path in sorted(root.glob('*/record.json')):
        row = A.unique_json(path.read_bytes())
        need(set(row) == {'command', 'started_ns', 'finished_ns', 'process_group', 'status', 'group_absent'}
             and type(row['status']) is int and row['status'] in (0, 1)
             and type(row['process_group']) is int and row['process_group'] > 0
             and row['group_absent'] is True, 'normal terminal process receipt')
        need(not base.group_exists(row['process_group']), 'owned command group remains live')
        groups.append(row['process_group'])
    need(groups, 'nonempty terminal command roster')
    return groups


def inputs():
    paths = [HERE / name for name in ('audit.py', 'test_audit.py', 'finalize.py', 'test_finalize.py', 'README.md')]
    paths += [COLLECT / 'collect.py', COLLECT / 'verify.py', COLLECT / 'test_verify.py', VISIBILITY]
    return {str(p.relative_to(REPO)): A.sha(A.ordinary(REPO, p.relative_to(REPO))) for p in paths}


def visibility(private):
    class OtherProcesses:
        def iterdir(self):
            return (entry for entry in Path('/proc').iterdir() if entry.name != str(os.getpid()))
    report = S.private_users_absent(proc=OtherProcesses(), private=private)
    report['uninspectable_count'] = len(report['uninspectable'])
    report['excluded_collector_pid'] = os.getpid()
    report['collector_reference'] = 'only read-only campaign lock; source copy reads already closed'
    return report


def retain_and_remove(source, destination, output, roster, base, current):
    with (source / 'campaign1.lock').open('rb') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        original = snapshot(source, roster)
        groups = terminal_groups(source / 'campaign1', base) + terminal_groups(output / 'commands', base)
        need(len(groups) == 19, 'fifteen proof stages and four audit stages')
        write(output / 'process-visibility-before.json', visibility(source))
        expected = inventory(source)
        shutil.copytree(source, destination)
        need(inventory(destination) == expected, 'byte-exact complete scratch retention')
        allocated = sum(p.lstat().st_blocks * 512 for p in (source, *source.rglob('*')))
        write(output / 'cleanup-source.json', original)
        row = {'path': str(source), 'retained_files': expected, 'excluded': [],
               'allocated_bytes': allocated, 'known_terminal_groups': groups,
               'scope': 'controller-lock-and-owned-command-groups-not-global-process-absence', 'absent': False}
        write(output / 'cleanup-before.json', row)
        write(output / 'process-visibility-final.json', visibility(source))
        need(current(), 'unchanged controls before owned removal')
        need(snapshot(source, roster) == original, 'unchanged tree before owned removal')
        need(terminal_groups(source / 'campaign1', base) + terminal_groups(output / 'commands', base) == groups,
             'exact terminal group roster before deletion')
        # This packet uses record.json, not the imported collector's receipt layout.
        shutil.rmtree(source)
        need(not os.path.lexists(source), 'owned scratch absent')
        write(output / 'cleanup-after.json', row | {'absent': True})
    return expected, allocated


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.absolute()
    need(output.resolve() == output and output.parent == HERE
         and re.fullmatch(r'audit[1-9][0-9]*', output.name) is not None, 'fresh contained audit directory')
    need(PRIVATE.resolve() == PRIVATE and PRIVATE.is_dir(), 'exact owned scratch path')
    need(not os.path.lexists(HERE / 'raw'), 'fresh retention destination')
    before = inputs()
    need(before[str((COLLECT / 'test_verify.py').relative_to(REPO))] ==
         '61971db898f5db1e3c5d56e7339ae1d6425dea265a06eccde6083ca93523feea', 'pinned cleanup calibration')
    checker = A.load_checker(REPO)
    _, deps, _ = checker.inherited(REPO)
    base = deps['base']
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    output.mkdir()
    write(output / 'inputs-before.json', before)
    for name, digest in before.items():
        data = A.ordinary(REPO, Path(name))
        need(A.sha(data) == digest, 'captured finalization control')
        target = output / 'controls-source' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (output / 'commands').mkdir()
    commands = [
        ('replay', ['/usr/bin/python3', '-I', '-B', str(REPO / A.CHECK), '--output', str(PRIVATE / 'campaign1'), '--verify'],
         'PASS: independent mixed-acquisition campaign replay\n'),
        ('packet-tests', ['/usr/bin/python3', '-I', '-B', str(HERE / 'test_audit.py'), '--raw', str(PRIVATE / 'campaign1')],
         'PASS: mixed-acquisition packet calibration (5 groups)\n'),
        ('finalizer-tests', ['/usr/bin/python3', '-I', '-B', str(HERE / 'test_finalize.py')],
         'PASS: mixed-acquisition finalizer calibration (8 groups)\n'),
        ('cleanup-tests', ['/usr/bin/python3', '-I', '-B', str(COLLECT / 'test_verify.py'), 'CleanupTests'], ''),
    ]
    write(output / 'controls.json', {'environment': ENV, 'cwd': os.getcwd(), 'timeout_seconds': 900,
          'commands': {name: command for name, command, _ in commands}})
    for name, command, expected in commands:
        status, stdout, stderr = base.run_owned(command, 900, output / 'commands' / name, ENV)
        need(status == 0 and stdout == expected, 'successful exact finalization stage: ' + name)
        if name == 'cleanup-tests':
            need(re.findall(r'^Ran (\d+) tests? in [0-9.]+s$', stderr, re.MULTILINE) == ['5']
                 and stderr.endswith('\nOK\n') and 'skipped' not in stderr, 'complete cleanup calibration')
    need(inputs() == before, 'unchanged finalization controls')
    raw = HERE / 'raw'
    expected, allocated = retain_and_remove(PRIVATE, raw, output, ROSTER, base, lambda: inputs() == before)
    absent = ['/usr/bin/python3', '-I', '-B', '-c',
              'import os,sys; assert not os.path.lexists(sys.argv[1])', str(PRIVATE)]
    status, stdout, stderr = base.run_owned(absent, 30, output / 'commands/absence', ENV)
    need(status == 0 and not stdout and not stderr, 'independent owned-path absence')
    need(inventory(raw) == expected and inputs() == before, 'retention and control continuity')
    write(output / 'inputs-after.json', inputs())
    write(HERE / 'finalization.json', {'schema': 1, 'audit': output.name})
    print(json.dumps({'retained_files': len(expected), 'removed_allocated_bytes': allocated, 'absent': True}))


if __name__ == '__main__':
    main()
