#!/usr/bin/env python3
"""Mutation checks for the CPU-to-signed-candidate gate, using synthetic receipts."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_cpu_binding_tests', HERE / 'campaign.py')
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)

class CpuBinding(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cpu-binding-test-', dir=HERE)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.packet = self.root / 'cpu'
        self.packet.mkdir()
        shutil.copy2(HERE / 'runner.py', self.root / 'runner.py')
        self.files = {name: '1' * 64 for name in C.EXPECTED_DELTA | {'Cargo.toml', 'Cargo.lock'}}
        bracket = {'runner': C.H.sha(self.root / 'runner.py'), 'source': self.files}
        for name in ('inputs-before.json', 'inputs-after.json'):
            self.write(self.packet / name, bracket)
        packages = ['-p', 'fe2o3-kfd', '-p', 'fe2o3-runtime']
        commands = {'rustc': ['rustc', '-vV'], 'cargo': ['cargo', '-V'],
                    'after-rustc': ['rustc', '-vV'], 'after-cargo': ['cargo', '-V'],
                    'format': ['cargo', 'fmt', *packages, '--', '--check'],
                    'docs': ['cargo', '--locked', 'test', *packages, '--all-features', '--doc'],
                    'clippy': ['cargo', '--locked', 'clippy', *packages, '--all-features', '--all-targets', '--', '-D', 'warnings']}
        for label, target in [('gnu', []), ('musl', ['--target', 'x86_64-unknown-linux-musl'])]:
            for package, suffix, selected in [('fe2o3-kfd', 'topology', ['topology::']),
                                               ('fe2o3-kfd', 'currentness', ['currentness::']),
                                               ('fe2o3-runtime', 'runtime', [])]:
                commands[label + '-' + suffix] = ['cargo', '--locked', 'test', '-p', package,
                    '--all-features', '--lib', *target, *selected, '--', '--test-threads=4']
        for name, command in commands.items():
            path = self.packet / name
            path.mkdir()
            self.write(path / 'record.json', {'status': 0, 'group_absent': True, 'command': command})
            (path / 'stdout.log').write_bytes(b'fixture tool identity\n')
            (path / 'stderr.log').write_bytes(b'')

    def write(self, path, value):
        path.write_text(json.dumps(value) + '\n')

    def record(self, **fields):
        path = self.packet / 'gnu-topology/record.json'
        value = json.loads(path.read_text())
        value.update(fields)
        self.write(path, value)

    def rejected(self):
        with self.assertRaises((RuntimeError, FileNotFoundError)):
            C.qualified_cpu(self.packet, self.files)

    def test_complete_shape_passes(self):
        self.assertTrue(C.qualified_cpu(self.packet, self.files))

    def test_truncated_source_map_rejects_even_with_equal_brackets(self):
        for name in ('inputs-before.json', 'inputs-after.json'):
            path = self.packet / name
            value = json.loads(path.read_text())
            del value['source']['Cargo.lock']
            self.write(path, value)
        self.rejected()

    def test_substituted_candidate_source_rejects(self):
        self.files['Cargo.toml'] = '2' * 64
        self.rejected()

    def test_changed_runner_bytes_reject(self):
        (self.root / 'runner.py').write_text('pass\n')
        self.rejected()

    def test_narrower_or_unrelated_command_rejects(self):
        self.record(command=['true'])
        self.rejected()

    def test_failed_tool_continuity_rejects_after_successful_commands(self):
        (self.packet / 'after-rustc/stderr.log').write_text('changed\n')
        self.rejected()

    def test_boolean_status_is_not_success(self):
        self.record(status=False)
        self.rejected()

    def test_failed_command_rejects(self):
        self.record(status=1)
        self.rejected()

    def test_remaining_process_group_rejects(self):
        self.record(group_absent=False)
        self.rejected()

    def test_late_exception_rejects(self):
        self.record(exception='TimeoutExpired')
        self.rejected()

    def test_missing_stage_rejects(self):
        shutil.rmtree(self.packet / 'musl-runtime')
        self.rejected()

if __name__ == '__main__':
    unittest.main()
