#!/usr/bin/env python3
"""Focused publication-receipt corruption tests; these do not execute Verus."""
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

import copy
import json
from pathlib import Path
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
A = types.ModuleType('mixed_lifecycle_audit_tests')
A.__file__ = str(HERE / 'audit.py')
exec(compile((HERE / 'audit.py').read_bytes(), A.__file__, 'exec'), A.__dict__)
OLD = A.module(REPO / A.HELPER, A.PINS[A.HELPER], 'mixed_lifecycle_test_helpers')


class Publication(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='mixed-lifecycle-packet-tests-')
        self.addCleanup(self.temporary.cleanup)
        self.packet = Path(self.temporary.name)
        self.raw = self.packet / 'raw'
        self.output = self.packet / 'publication'
        self.write('raw/campaign1/source.json', {'commit': A.PRIOR_SOURCE})
        for n in range(12):
            self.write(f'raw/campaign1/stage-{n:02}/record.json',
                       {'process_group': 50 + n, 'started_ns': 1 + n, 'finished_ns': 2 + n,
                        'status': 0, 'group_absent': True})
        self.write('raw/campaign2/source.json', {'recorded_repo': str(REPO),
                   'recorded_output': str(A.PRIVATE / 'campaign2')})
        for n in range(16):
            self.write(f'raw/campaign2/stage-{n:02}/record.json',
                       {'process_group': 100 + n, 'finished_ns': 10 + n})
        controls = {str(p): OLD.sha(OLD.ordinary(REPO, p)) for p in A.control_paths()}
        self.write('publication/inputs-before.json', controls)
        self.write('publication/inputs-after.json', controls)
        commands = A.commands(REPO)
        self.write('publication/controls.json', {'source': A.SOURCE, 'environment': A.ENV,
                   'commands': commands, 'cwd': str(REPO), 'timeout_seconds': 900})
        transcripts = {'replay': ('PASS: mixed-lifecycle campaign replay\n', 4),
                       'calibration': ('PASS: mixed-lifecycle checker calibration (10 groups)\n', 20),
                       'absence': ('', 0)}
        for n, (name, command) in enumerate(commands.items()):
            root = f'publication/commands/{name}/'
            self.write(root + 'record.json', {'command': command, 'started_ns': 100 + n * 10,
                       'finished_ns': 105 + n * 10, 'process_group': 200 + n, 'status': 0, 'group_absent': True})
            stdout, count = transcripts[name]
            (self.packet / root / 'stdout.log').write_text(stdout)
            (self.packet / root / 'stderr.log').write_text(OLD.SIGNATURE_LINE * count)
        inventory = {str(p): OLD.sha(OLD.ordinary(self.raw, p)) for p in OLD.files(self.raw)}
        snapshot = {}
        for path in (self.raw, *self.raw.rglob('*')):
            info = path.lstat()
            name = str(path.relative_to(self.raw))
            snapshot[name] = [info.st_mode, info.st_uid, info.st_dev, info.st_ino,
                              info.st_size, inventory.get(name)]
        self.write('publication/cleanup-source.json', snapshot)
        row = {'path': str(A.PRIVATE), 'retained_files': inventory, 'allocated_bytes': 4096,
               'known_terminal_groups': list(range(50, 62)) + list(range(100, 116)) + [201, 200],
               'scope': 'exact-owned-path-and-recorded-groups-not-global-process-absence', 'absent': False}
        self.write('publication/cleanup-before.json', row)
        self.write('publication/cleanup-after.json', row | {'absent': True})

    def write(self, name, value):
        target = self.packet / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(value) + '\n')

    def check(self):
        A.check_publication(REPO, self.packet, OLD)

    def corrupt(self, name, mutations):
        path = self.packet / name
        original = OLD.unique_json(path.read_bytes())
        for mutation in mutations:
            value = copy.deepcopy(original)
            mutation(value)
            self.write(name, value)
            with self.assertRaises((ValueError, KeyError, TypeError)):
                self.check()
        self.write(name, original)
        self.check()

    def test_exact_fixture(self):
        self.check()

    def test_cleanup_metadata(self):
        self.corrupt('publication/cleanup-source.json', [
            lambda v: v['campaign1/source.json'].__setitem__(1, True),
            lambda v: v['campaign1/source.json'].__setitem__(2, v['.'][2] + 1),
            lambda v: v['campaign1/source.json'].__setitem__(4, 0),
            lambda v: v['campaign1/source.json'].__setitem__(0, v['.'][0]),
            lambda v: v.pop('.'),
            lambda v: v.__setitem__('../outside', v['campaign1/source.json']),
        ])
        self.corrupt('publication/cleanup-after.json', [lambda v: v.update(absent=1)])

    def test_commands_and_order(self):
        self.corrupt('publication/commands/replay/record.json', [
            lambda v: v.update(started_ns=1, finished_ns=2),
            lambda v: v.update(status=True),
            lambda v: v.update(group_absent=1),
            lambda v: v.update(finished_ns=v['started_ns'] + 911_000_000_000),
            lambda v: v['command'].append('--untrusted'),
        ])

    def test_rosters_and_transcripts(self):
        path = self.output / 'commands/replay/stderr.log'
        original = path.read_bytes()
        path.write_bytes(original + b'unexpected\n')
        with self.assertRaises(ValueError):
            self.check()
        path.write_bytes(original)
        self.write('publication/unexpected.json', {})
        with self.assertRaises(ValueError):
            self.check()
        (self.output / 'unexpected.json').unlink()
        self.check()


if __name__ == '__main__':
    unittest.main()
