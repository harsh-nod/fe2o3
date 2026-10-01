#!/usr/bin/env python3
"""Synthetic joins and bounded Python relocation; no SSH, build, or GPU calls."""

import base64
from contextlib import contextmanager
import copy
import json
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parents[1]
sys.path.insert(0, str(HERE))
import xgmi_retained_wait_cadence_campaign as planner
import xgmi_retained_wait_cadence_native as experiment
import xgmi_retained_wait_cadence_transport as extension
import test_xgmi_retained_wait_cadence_campaign as fixtures
import xgmi_peer_series_native as native
import xgmi_peer_series_transport as transport

HOT, ORDINARY, _ = native.load_helpers()
MARKER = {'path': transport.PREFIX + '0123456789abcdef', 'commit': '1' * 40, 'binding_sha256': '2' * 64}


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + '\n')


def readback_fixture(root):
    inputs = fixtures.existing.inputs()
    binding = {'devices': [{key: row[key] for key in ('physical_index', 'unique_id', 'pci_bdf')}
                           for row in inputs['admission']['endpoints']],
               'selected_objects': {'Cargo.toml': '4' * 40},
               'files': {'source/Cargo.toml': {'sha256': '5' * 64}}}
    remote = Path(MARKER['path'])
    campaign = root / 'campaign-1'
    campaign.mkdir()
    binaries = campaign / 'binaries'
    binaries.mkdir()
    for backend, name in experiment.BINARIES.items():
        (binaries / name).write_bytes(b'\x7fELFsynthetic-not-executed-' + backend.encode('ascii'))
    binary_pins = {key: transport.sha(binaries / name) for key, name in experiment.BINARIES.items()}
    save(campaign / 'binaries-before.json', binary_pins)
    save(campaign / 'binaries-after.json', binary_pins)
    for role in ('source', 'tools'):
        for edge in ('before', 'after'):
            save(campaign / f'{role}-{edge}.json', {'Cargo.toml': '5' * 64} if role == 'source' else {'synthetic': role})
    census = {'namespace': 'pid:[synthetic]', 'raw_sha256': {}}
    def command(name, argv, stdout, environment=None):
        folder = campaign / 'commands' / name
        folder.mkdir(parents=True)
        (folder / 'stdout').write_bytes(stdout)
        (folder / 'stderr').write_bytes(b'')
        receipt = {'command': argv, 'cwd': str(remote / 'source'), 'exit': 0, 'error': None,
                   'group_absent': True, 'environment': environment or {},
                   'pid': 1000 + len(census['raw_sha256']),
                   'stdout_sha256': transport.sha(folder / 'stdout'), 'stderr_sha256': transport.sha(folder / 'stderr')}
        save(folder / 'receipt.json', receipt)
        census['raw_sha256'][name] = {stream: transport.sha(folder / stream) for stream in ('stdout', 'stderr', 'receipt.json')}
        return census['raw_sha256'][name]['receipt.json']
    records = fixtures.receipts()
    for row in records:
        row['execution_receipt_sha256'] = command(row['name'],
            [str(remote / 'campaign-1/binaries' / row['binary']), *row['arguments']], row['stdout'], row['environment_overrides'])
    save(campaign / 'records.json', [{**{key: value for key, value in row.items() if key not in ('stdout', 'stderr')},
        **{key + '_base64': base64.b64encode(row[key]).decode('ascii') for key in ('stdout', 'stderr')}} for row in records])
    save(campaign / 'plan.json', planner.trial_specs(**inputs))
    save(campaign / 'admission.json', inputs['admission'])
    save(campaign / 'replay.json', planner.replay_campaign(records, environment_after=fixtures.existing.environment(), **inputs))
    remote_native = types.SimpleNamespace(HERE=remote / 'source/benchmarks/runtime_gfx942')
    for cadence, labels in ((False, ('host-before', 'host-after')), (True, ('host-cadence-before', 'host-cadence-after'))):
        for label, edge in zip(labels, ('start', 'end'), strict=True):
            context = {'unique_id': '0x' + binding['devices'][0]['unique_id'], 'pci_bdf': binding['devices'][0]['pci_bdf']}
            for role in ('kfd', 'hsa', 'hip'):
                context[role + '_binary_sha256'] = binary_pins['kfd-cadence' if role == 'kfd' and cadence else role]
            value = {'schema': 'fe2o3.xgmi-peer-series-host-observation.v1', 'environment': fixtures.existing.environment(),
                     'context': context, 'continuity': context}
            save(campaign / (label + '.json'), value)
            argv = experiment.host_command(remote_native, remote / 'campaign-1/binaries', binding['devices'][0], label, edge, cadence)[1]
            command(label, argv, (campaign / (label + '.json')).read_bytes())
    save(campaign / 'cadence-admission-boundary.json', {'query_binary': str(remote / 'campaign-1/binaries/kfd-series'),
        'query_binary_sha256': binary_pins['kfd'], 'cadence_binary_sha256': binary_pins['kfd-cadence'],
        'cadence_binary_query_performed': False, 'cadence_uid_gpu_engine_join': 'strict-cadence-output-versus-current-query-inputs',
        'query_only_mode_added': False, 'performance_acceptance': False})
    artifacts, cargo = {}, []
    for backend, target in experiment.KFD_TARGETS.items():
        path = str(remote / 'campaign-1/work/target/release/examples' / target)
        row = {'reason': 'compiler-artifact', 'target': {'name': target, 'kind': ['example']},
               'profile': {'test': False}, 'fresh': False, 'executable': path,
               'features': ['default', 'hardware-diagnostic', 'live-validation'],
               'manifest_path': str(remote / 'source/crates/fe2o3-kfd/Cargo.toml')}
        artifacts[backend] = {'path': path, 'sha256': binary_pins[backend], 'cargo_record': row}
        cargo.append(row)
    cargo.append({'reason': 'build-finished', 'success': True})
    command('build-kfd-pair', experiment.kfd_build_spec()[1],
            ('\n'.join(json.dumps(row) for row in cargo) + '\n').encode('ascii'), HOT.environment(remote / 'campaign-1/work'))
    save(campaign / 'fresh-kfd-artifacts.json', artifacts)
    save(campaign / 'finished.json', {'native_execution': True, 'failures': [], 'commit': MARKER['commit'],
        'diagnostic_authority': 'none', 'instrumentation_perturbs_timing_and_readiness': True,
        'exclusive_reservation': False, 'performance_acceptance': False, 'formal_refinement': False, 'engine_matching': False})
    save(campaign / 'fresh-census.json', census)
    save(root / 'launch.json', {'marker': MARKER, 'fresh_closure': True, 'namespace': census['namespace'],
        'fresh_census_sha256': transport.sha(campaign / 'fresh-census.json'), 'failures': [],
        'public_argv': extension.public_command(remote, MARKER, binding), 'experiment': 'retained-wait-cadence-36'})
    save(root / 'monitor/receipt.json', {'group_absent': True, 'namespace': census['namespace']})
    return binding


class ReplayTests(unittest.TestCase):
    def replay(self, root, binding):
        return extension.independent_replay(root, MARKER, binding, transport, HOT, ORDINARY, experiment, planner)

    def test_existing_replay_api_accepts_exact_thirty_six_join_roster_and_four_elf_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = readback_fixture(root)
            result = self.replay(root, binding)
        self.assertTrue(result['accepted'])
        self.assertEqual((result['trials'], result['ordinary_trials'], result['profiled_trials']), (36, 24, 12))
        self.assertTrue(result['four_elf_dependency_identity'])
        self.assertFalse(result['cadence_binary_query_performed'])

    def test_fourth_binary_or_saved_host_binding_tampering_is_rejected(self):
        for mutation in ('binary', 'host', 'query-boundary', 'fresh-artifact', 'source-closing', 'entrypoint'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                binding = readback_fixture(root)
                campaign = root / 'campaign-1'
                if mutation == 'binary':
                    (campaign / 'binaries/kfd-retained-cadence').write_bytes(b'changed')
                else:
                    name, key, value = {
                        'host': ('host-cadence-before', 'context', {}),
                        'query-boundary': ('cadence-admission-boundary', 'cadence_binary_query_performed', True),
                        'fresh-artifact': ('fresh-kfd-artifacts', 'kfd-cadence', {}),
                        'source-closing': ('source-after', 'synthetic', 'changed'),
                        'entrypoint': ('../launch', 'public_argv', ['old-runner']),
                    }[mutation]
                    path = campaign / (name + '.json')
                    body = json.loads(path.read_text())
                    body[key] = value
                    save(path, body)
                with self.assertRaises((ValueError, RuntimeError, KeyError)):
                    self.replay(root, binding)

    def test_rejected_native_history_remains_rejected_not_reinterpreted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = readback_fixture(root)
            save(root / 'campaign-1/finished.json', {'native_execution': False, 'failures': ['original rejection']})
            self.assertEqual(self.replay(root, binding), {'accepted': False, 'failures': ['original rejection']})

    def test_agreeing_substituted_source_inventories_cannot_replace_signed_binding(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = readback_fixture(root)
            for edge in ('before', 'after'):
                save(root / f'campaign-1/source-{edge}.json', {'Cargo.toml': '6' * 64})
            with self.assertRaisesRegex(ValueError, 'signed selected objects'):
                self.replay(root, binding)

    def test_consistent_artifact_and_raw_default_omission_still_fails_profile_join(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binding = readback_fixture(root)
            campaign = root / 'campaign-1'
            artifacts = json.loads((campaign / 'fresh-kfd-artifacts.json').read_text())
            artifacts['kfd-cadence']['cargo_record']['features'].remove('default')
            save(campaign / 'fresh-kfd-artifacts.json', artifacts)
            command = campaign / 'commands/build-kfd-pair'
            rows = [json.loads(line) for line in (command / 'stdout').read_bytes().splitlines()]
            rows[1]['features'].remove('default')
            (command / 'stdout').write_text('\n'.join(json.dumps(row) for row in rows) + '\n')
            receipt = json.loads((command / 'receipt.json').read_text())
            receipt['stdout_sha256'] = transport.sha(command / 'stdout')
            save(command / 'receipt.json', receipt)
            census = json.loads((campaign / 'fresh-census.json').read_text())
            census['raw_sha256']['build-kfd-pair'] = {stream: transport.sha(command / stream)
                for stream in ('stdout', 'stderr', 'receipt.json')}
            save(campaign / 'fresh-census.json', census)
            launch = json.loads((root / 'launch.json').read_text())
            launch['fresh_census_sha256'] = transport.sha(campaign / 'fresh-census.json')
            save(root / 'launch.json', launch)
            with self.assertRaisesRegex(ValueError, 'fourth ELF Cargo'):
                self.replay(root, binding)


class RelocationTests(unittest.TestCase):
    def test_fresh_isolated_python_loads_relocated_integrated_helper_closure(self):
        with tempfile.TemporaryDirectory(prefix='fe2o3-native36-relocation-') as directory:
            root = Path(directory)
            destination = root / 'benchmarks/runtime_gfx942'
            destination.mkdir(parents=True)
            for path in HERE.glob('*.py'):
                shutil.copy2(path, destination / path.name)
            for relative in ('docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py',
                             'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py',
                             'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/.gitattributes'):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(SOURCE / relative, path)
            code = (
                'import runpy,sys; from pathlib import Path; '
                'root=Path(sys.argv[1]); '
                'value=runpy.run_path(str(root/"benchmarks/runtime_gfx942/xgmi_retained_wait_cadence_transport.py")); '
                'transport,native,hot,ordinary,experiment,planner=value["helpers"](); '
                'assert native.ROOT == root; '
                'assert len(experiment.BINARIES) == 4; '
                'assert experiment.KFD_TARGETS["kfd-cadence"] == "kfd-xgmi-retained-cadence-experiment"; '
                'assert len(planner.ORDER) == 12; print("relocated native36 helper closure accepted")'
            )
            result = subprocess.run([sys.executable, '-I', '-B', '-c', code, str(root)],
                                    capture_output=True, timeout=30, check=False)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual(result.stdout, b'relocated native36 helper closure accepted\n')
            self.assertEqual(result.stderr, b'')


class LifecycleTests(unittest.TestCase):
    def exercise(self, root, outcome='accepted', disk_failure=False):
        prepared = root / 'prepared'
        prepared.mkdir()
        script = prepared / 'source' / transport.TRANSPORT_RELATIVE
        script.parent.mkdir(parents=True)
        script.write_bytes(b'# synthetic bootstrap, never executed\n')
        save(prepared / 'owner.json', MARKER)
        save(prepared / 'binding.json', {'synthetic': True})
        (prepared / 'source.tar.gz').write_bytes(b'synthetic payload')
        binding = {'payload_sha256': transport.sha(prepared / 'source.tar.gz')}
        calls, steps = [], []
        archive = b'synthetic archive'
        collection = {'marker': MARKER, 'archive_sha256': hashlib_sha(archive), 'archive_bytes': len(archive), 'files': {}}
        previous = {sig: signal.getsignal(sig) for sig in HOT.B.MANAGED}
        class Recorder:
            def __init__(self, folder, _cwd, _hot):
                self.output = folder
                folder.mkdir()
            def run(self, name, command, seconds, **kwargs):
                calls.append(name)
                steps.append(name)
                folder = self.output / name
                folder.mkdir()
                raw = json.dumps(collection).encode('ascii') if name == 'collect' else archive if name == 'pull' else b''
                (folder / 'stdout').write_bytes(raw)
                (folder / 'stderr').write_bytes(b'')
                save(folder / 'receipt.json', {'exit': 255 if outcome == 'uncertain' and name == 'remote-run' else 0,
                    'error': None, 'group_absent': True})
                return folder
        def write(path, value):
            if disk_failure and path.name == 'transport-census.json':
                raise OSError('synthetic receipt-save failure')
            save(path, value)
        def replay(*_args):
            steps.append('replay')
            if outcome == 'bad-readback':
                raise ValueError('readback mismatch')
            return {'accepted': outcome != 'rejected'}
        fake_transport = types.SimpleNamespace(need=transport.need, parse_json=transport.parse_json,
            read_binding=lambda *_args: binding, sha=transport.sha, encoded=transport.encoded,
            TRANSPORT_RELATIVE=transport.TRANSPORT_RELATIVE, SSH=['synthetic-ssh-never-executed'],
            bootstrap_command=lambda _script, mode, _marker, *_extra: ['synthetic-bootstrap', mode],
            validate_archive=lambda *_args: steps.append('archive-readback'), save=write)
        fake_native = types.SimpleNamespace(FreshRecorder=Recorder, fresh_census=lambda *_args: {'synthetic': 'fresh-only'})
        with patch.object(extension, 'SCRIPT', prepared / 'source' / extension.RELATIVE), \
             patch.object(extension, 'extended_binding', side_effect=lambda *_args: steps.append('source-binding')), \
             patch.object(extension, 'independent_replay', side_effect=replay), patch.object(extension.os, 'umask'):
            if outcome != 'accepted' or disk_failure:
                with self.assertRaises((ValueError, OSError)):
                    extension.execute(types.SimpleNamespace(prepared=prepared), fake_transport, fake_native, HOT, ORDINARY, experiment, planner)
            else:
                extension.execute(types.SimpleNamespace(prepared=prepared), fake_transport, fake_native, HOT, ORDINARY, experiment, planner)
        self.assertEqual({sig: signal.getsignal(sig) for sig in HOT.B.MANAGED}, previous)
        return calls, steps

    def test_cleanup_only_after_known_terminal_archive_readback_and_independent_replay(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, steps = self.exercise(Path(directory))
        self.assertEqual(calls, ['receive', 'remote-run', 'collect', 'pull', 'cleanup', 'absence'])
        self.assertLess(steps.index('archive-readback'), steps.index('replay'))
        self.assertLess(steps.index('replay'), steps.index('cleanup'))

    def test_uncertain_original_terminal_never_collects_or_cleans(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, _ = self.exercise(Path(directory), outcome='uncertain')
        self.assertEqual(calls, ['receive', 'remote-run'])

    def test_bad_readback_retains_owned_remote_and_rejected_complete_readback_can_clean(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, _ = self.exercise(Path(directory), outcome='bad-readback')
        self.assertEqual(calls, ['receive', 'remote-run', 'collect', 'pull'])
        with tempfile.TemporaryDirectory() as directory:
            calls, _ = self.exercise(Path(directory), outcome='rejected')
        self.assertEqual(calls[-2:], ['cleanup', 'absence'])

    def test_local_handler_restore_even_when_final_receipt_save_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            self.exercise(Path(directory), disk_failure=True)

    def test_remote_lock_monitor_and_original_argv_restore_on_save_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            owned = Path(directory)
            events = []
            binding = {'devices': []}
            @contextmanager
            def lock():
                events.append('lock-enter')
                try:
                    yield {'synthetic': 'original-lock-handle'}
                finally:
                    events.append('lock-exit')
            @contextmanager
            def guard(*_args):
                events.append('monitor-enter')
                try:
                    yield {'exit': 0, 'error': None, 'group_absent': True}
                finally:
                    events.append('monitor-exit')
            def write(path, value):
                if path.name == 'launch.json':
                    raise OSError('synthetic remote save failure')
            fake_transport = types.SimpleNamespace(owned_path=lambda _marker: owned, read_binding=lambda *_args: binding,
                need=transport.need, save=write, shared_lock=lock, resource_guard=guard,
                resource_observation=lambda _owned: {'synthetic': 'resources'}, replay_fresh_closure=lambda *_args: '0' * 64)
            previous = {sig: signal.getsignal(sig) for sig in HOT.B.MANAGED}
            argv = sys.argv
            with patch.object(extension, 'SCRIPT', owned / 'source' / extension.RELATIVE), \
                 patch.object(extension, 'extended_binding'), patch.object(extension.os, 'umask'), \
                 patch.object(extension.runpy, 'run_path', side_effect=lambda *_args, **_kwargs: events.append('public-entrypoint')):
                with self.assertRaises(OSError):
                    extension.remote_run(MARKER, fake_transport, native, HOT)
            self.assertIs(sys.argv, argv)
            self.assertEqual({sig: signal.getsignal(sig) for sig in HOT.B.MANAGED}, previous)
            self.assertEqual(events, ['lock-enter', 'monitor-enter', 'public-entrypoint', 'monitor-exit', 'lock-exit'])


def hashlib_sha(raw):
    import hashlib
    return hashlib.sha256(raw).hexdigest()


if __name__ == '__main__':
    unittest.main(verbosity=2)
