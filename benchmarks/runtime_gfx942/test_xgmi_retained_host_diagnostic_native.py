#!/usr/bin/env python3
"""Synthetic lifecycle/ownership controls; never launches a child process."""

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parents[1]
sys.path.insert(0, str(HERE))
import xgmi_retained_host_diagnostic_campaign as planner
import xgmi_retained_host_diagnostic_native as runner
import test_xgmi_retained_host_diagnostic_campaign as fixture
import xgmi_peer_series_native as native

HOT, ORDINARY, OBSERVATIONS = native.load_helpers()


def cargo_rows(work, source):
    rows = []
    folder = work / 'target/release/examples'
    folder.mkdir(parents=True)
    for backend, target in runner.KFD_TARGETS.items():
        path = folder / target
        path.write_bytes(b'\x7fELFsynthetic-not-executed-' + backend.encode('ascii'))
        path.chmod(0o700)
        rows.append({'reason': 'compiler-artifact', 'manifest_path': str(source / 'crates/fe2o3-kfd/Cargo.toml'),
                     'target': {'name': target, 'kind': ['example']}, 'profile': {'test': False},
                     'executable': str(path), 'fresh': False, 'features': ['default', 'hardware-diagnostic', 'live-validation']})
    rows.append({'reason': 'build-finished', 'success': True})
    return rows


def encoded(rows):
    return ('\n'.join(json.dumps(row) for row in rows) + '\n').encode('ascii')


class ArtifactTests(unittest.TestCase):
    def test_both_kfd_features_targets_and_unchanged_rocm_commands(self):
        binaries = Path('/owned/binaries')
        specs = runner.build_specs(native, binaries)
        self.assertEqual(specs[1:], native.build_specs(binaries)[1:])
        self.assertEqual(specs[0][1][specs[0][1].index('--features') + 1], 'live-validation,hardware-diagnostic')
        self.assertEqual(specs[0][1].count('--example'), 2)
        self.assertIn('--message-format=json', specs[0][1])
        self.assertEqual(specs[0][1][specs[0][1].index('--jobs') + 1], '2')

    def test_paired_fresh_artifacts_refuse_missing_stale_foreign_and_aliases(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            rows = cargo_rows(work, SOURCE)
            result = runner.fresh_kfd_artifacts(encoded(rows), work, SOURCE, HOT)
            self.assertEqual(set(result), {'kfd', 'kfd-profiled'})
            bad = [rows[1:], [rows[0], *rows], [dict(rows[0], fresh=True), *rows[1:]],
                   [dict(rows[0], manifest_path='/foreign/Cargo.toml'), *rows[1:]],
                   [dict(rows[0], features=['live-validation']), *rows[1:]],
                   [dict(rows[0], features=['hardware-diagnostic', 'live-validation']), *rows[1:]],
                   [dict(rows[0], features=['cpu-runtime-fixtures', 'default', 'hardware-diagnostic', 'live-validation']), *rows[1:]]]
            for selected in bad:
                with self.assertRaises((RuntimeError, ValueError)):
                    runner.fresh_kfd_artifacts(encoded(selected), work, SOURCE, HOT)
            path = Path(rows[0]['executable'])
            saved = path.with_suffix('.saved')
            path.rename(saved)
            path.symlink_to(saved)
            with self.assertRaises(RuntimeError):
                runner.fresh_kfd_artifacts(encoded(rows), work, SOURCE, HOT)

    def test_fourth_elf_is_independently_passed_to_host_collector(self):
        device = fixture.existing.inputs()['admission']['endpoints'][0]
        ordinary = runner.host_command(native, Path('/owned/bin'), device, 'host-before', 'start', False)[1]
        profiled = runner.host_command(native, Path('/owned/bin'), device, 'host-profiled-before', 'start', True)[1]
        index = ordinary.index('--kfd-binary') + 1
        self.assertEqual(ordinary[index], '/owned/bin/kfd-series')
        self.assertEqual(profiled[index], '/owned/bin/kfd-retained-host-diagnostic')
        self.assertEqual(ordinary[:index] + ordinary[index + 1:], profiled[:index] + profiled[index + 1:])


class LifecycleTests(unittest.TestCase):
    def exercise(self, root, failure=None, disk_failure=False):
        output = root / 'attempt'
        calls, saves, pauses = [], {}, []
        args = types.SimpleNamespace(output=output, commit='0' * 40,
            devices=[{key: row[key] for key in ('physical_index', 'unique_id', 'pci_bdf')}
                     for row in fixture.existing.inputs()['admission']['endpoints']])
        workload = {row['name']: row for row in fixture.receipts()}
        owner = types.SimpleNamespace(MANAGED=HOT.B.MANAGED, interrupted=HOT.B.interrupted)
        old_handlers = {sig: signal.getsignal(sig) for sig in owner.MANAGED}
        def save(path, value):
            saves[path.stem] = copy.deepcopy(value)
            if disk_failure and path.stem in ('host-after', 'finished'):
                raise OSError('synthetic evidence disk failure')
            path.write_text(json.dumps(value))
        owner.write_json = save
        owner.settled_postflight = lambda observe, name, error: HOT.B.settled_postflight(
            observe, name, error, sleep=lambda delay: pauses.append(delay))
        def observe_spec(label, index, bdf, uid):
            return ['synthetic-observe', label, str(index), bdf, uid]
        def parse_endpoint(raw, index, bdf, uid):
            value = json.loads(raw)
            if value['busy'] != 0:
                raise ValueError('exact-zero idle gate')
            return value
        class Recorder:
            def __init__(self, folder, cwd, _hot):
                self.output, self.cwd = folder, cwd
                folder.mkdir()
                self.attempts = []
            def run(self, name, command, seconds, **kwargs):
                self.attempts.append(name)
                calls.append((name, command, kwargs.get('env')))
                folder = self.output / name
                folder.mkdir()
                raw = b''
                if name == 'build-kfd-pair':
                    if failure == 'build':
                        raise RuntimeError('synthetic build failure')
                    raw = encoded(cargo_rows(output / 'work', SOURCE))
                elif name in ('build-hsa', 'build-hip'):
                    (output / 'binaries' / runner.BINARIES[name[6:]]).write_bytes(b'\x7fELFsynthetic-' + name.encode('ascii'))
                elif name.startswith('host-'):
                    profiled = 'profiled' in name
                    current = {key: HOT.sha(output / 'binaries' / path) for key, path in runner.BINARIES.items()}
                    context = {'unique_id': '0x' + args.devices[0]['unique_id'], 'pci_bdf': args.devices[0]['pci_bdf'],
                               'boot_id': 'synthetic-boot'}
                    for role in ('kfd', 'hip', 'hsa'):
                        context[role + '_binary_sha256'] = current['kfd-profiled' if role == 'kfd' and profiled else role]
                    value = {'schema': 'fe2o3.xgmi-peer-series-host-observation.v1',
                             'environment': fixture.existing.environment(), 'context': context,
                             'continuity': {'kind': 'profiled' if profiled else 'ordinary', 'identity': context}}
                    if failure == 'fourth-elf' and profiled:
                        value['context']['kfd_binary_sha256'] = current['kfd']
                    raw = json.dumps(value).encode('ascii')
                elif command[0] == 'synthetic-observe':
                    _, label, index, bdf, uid = command
                    raw = json.dumps({'gpu_index': int(index), 'pci_bdf': bdf, 'unique_id': uid,
                                      'busy': 1 if failure == 'busy' and label.endswith('-before') and label.startswith('d') else 0}).encode('ascii')
                elif '--inspect-peer-pair' in command:
                    raw = b'{"synthetic":"query"}\n'
                elif name in workload:
                    raw = b'malformed successful workload\n' if failure == 'malformed' else workload[name]['stdout']
                else:
                    raise AssertionError('unexpected synthetic command: ' + name)
                (folder / 'stdout').write_bytes(raw)
                (folder / 'stderr').write_bytes(b'')
                (folder / 'receipt.json').write_text(json.dumps({'synthetic': name}))
                return folder
        def source_snapshot(_rec, label, *_args):
            if failure == 'source' and label == 'source-before':
                raise ValueError('synthetic source failure')
            return {'source': 'synthetic'}
        fake_native = types.SimpleNamespace(ROOT=SOURCE, HERE=native.HERE,
            fresh_output=native.fresh_output, FreshRecorder=Recorder, source_snapshot=source_snapshot,
            tool_snapshot=lambda *_args: {'tools': 'synthetic'}, fresh_census=lambda *_args: {'synthetic': 'fresh-only'},
            build_specs=native.build_specs, query_specs=native.query_specs, CONTROLS=native.CONTROLS,
            QUERY_SETTLE_SECONDS=native.QUERY_SETTLE_SECONDS)
        hot = types.SimpleNamespace(B=owner, need=HOT.need, sha=HOT.sha, parse_json=HOT.parse_json,
                                    environment=HOT.environment, observe_spec=observe_spec, parse_endpoint=parse_endpoint)
        observations = types.SimpleNamespace(digest=OBSERVATIONS.digest,
            join_admission=lambda *_args, **_kwargs: (copy.deepcopy(fixture.existing.inputs()['admission']), {'boot_id': 'synthetic-boot'}))
        with patch.object(runner.time, 'sleep', side_effect=lambda delay: pauses.append(delay)), \
             patch.object(runner.resource, 'setrlimit'), patch.object(runner.os, 'umask'):
            if failure or disk_failure:
                with self.assertRaises((RuntimeError, OSError)):
                    runner.run_campaign(args, fake_native, hot, ORDINARY, observations, planner)
            else:
                runner.run_campaign(args, fake_native, hot, ORDINARY, observations, planner)
        self.assertEqual({sig: signal.getsignal(sig) for sig in owner.MANAGED}, old_handlers)
        self.assertIn('fresh-census', saves)
        self.assertIn('tools-after', saves)
        self.assertIn('source-after', saves)
        return calls, saves, pauses

    def test_complete_twenty_four_roster_both_postflights_fourth_host_and_visibility(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, saves, pauses = self.exercise(Path(directory))
        plan = planner.trial_specs(**fixture.existing.inputs())
        workload = [row for row in calls if row[0] in {spec['name'] for spec in plan['trials']}]
        self.assertEqual([row[0] for row in workload], [spec['name'] for spec in plan['trials']])
        self.assertEqual(len(saves['replay']['trials']), 24)
        self.assertTrue(saves['finished']['native_execution'])
        self.assertFalse(saves['profiled-admission-boundary']['profiled_binary_query_performed'])
        self.assertIn('host-profiled-before', saves)
        self.assertIn('host-profiled-after', saves)
        self.assertEqual(pauses.count(20), 25)
        self.assertEqual(pauses.count(2), 51)
        query_settles = [value for name, value in saves.items() if name.endswith('-settle')]
        self.assertEqual(len(query_settles), 26)
        for value in query_settles:
            self.assertEqual(value['fixed_seconds'], 2)
            self.assertFalse(value['polling'])
            self.assertTrue(value['outside_timed_workloads'])
            names = [row[0] for row in calls]
            positions = [names.index(name) for name in value['closed_query_stages']]
            self.assertEqual(positions, list(range(positions[0], positions[0] + 3)))
        for spec, (_, _, environment) in zip(plan['trials'], workload, strict=True):
            for name in spec['clear_environment']:
                self.assertEqual(environment.get(name), spec['environment_overrides'].get(name))

    def test_first_malformed_success_stops_later_workloads_and_both_postflights_run(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, saves, _ = self.exercise(Path(directory), failure='malformed')
        names = [row[0] for row in calls]
        workload_names = {row['name'] for row in planner.trial_specs(**fixture.existing.inputs())['trials']}
        self.assertEqual([name for name in names if name in workload_names], ['d01-t01-kfd'])
        for suffix in ('-settled-gpu1', '-settled-gpu2', '-delayed-gpu1', '-delayed-gpu2'):
            self.assertIn('d01-t01-kfd' + suffix, names)
        self.assertFalse(saves['finished']['native_execution'])

    def test_nonzero_physical_observation_stops_before_any_workload(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, saves, _ = self.exercise(Path(directory), failure='busy')
        names = {row[0] for row in calls}
        workload_names = {row['name'] for row in planner.trial_specs(**fixture.existing.inputs())['trials']}
        self.assertFalse(names & workload_names)
        self.assertIn('d01-t01-kfd-delayed-gpu2', names)
        self.assertFalse(saves['finished']['native_execution'])

    def test_source_and_build_failures_skip_closing_missing_binary_queries(self):
        for failure in ('source', 'build'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                calls, saves, _ = self.exercise(Path(directory), failure=failure)
            self.assertFalse(any('--inspect-peer-pair' in row[1] for row in calls))
            self.assertEqual(saves['admission-after-status']['status'], 'not-performed')
            self.assertFalse(saves['admission-after-status']['native_acceptance'])
            self.assertIn('campaign-close-delayed-gpu2', {row[0] for row in calls})

    def test_fourth_elf_identity_mismatch_stops_before_workloads(self):
        with tempfile.TemporaryDirectory() as directory:
            calls, saves, _ = self.exercise(Path(directory), failure='fourth-elf')
        self.assertFalse(any(row[0].startswith('d') for row in calls))
        self.assertFalse(saves['finished']['native_execution'])

    def test_handler_restore_and_all_closing_observations_survive_save_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            _, saves, _ = self.exercise(Path(directory), disk_failure=True)
        self.assertIn('host-profiled-after', saves)
        self.assertIn('binaries-after', saves)
        self.assertIn('finished', saves)


if __name__ == '__main__':
    unittest.main(verbosity=2)
