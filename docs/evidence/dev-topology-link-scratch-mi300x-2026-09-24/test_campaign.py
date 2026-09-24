#!/usr/bin/env python3
import importlib.util
import hashlib
import io
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('scratch_native_test', HERE / 'native.py')
N = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(N)
CSPEC = importlib.util.spec_from_file_location('scratch_campaign_test', HERE / 'campaign.py')
C = importlib.util.module_from_spec(CSPEC)
CSPEC.loader.exec_module(C)
DEVICES = [[5, '0000:a6:00.0', '0xb7baafd0fb173d8e'], [6, '0000:c6:00.0', '0x10a254ce4987e716']]

class Controls(unittest.TestCase):
    def setUp(self):
        self.rows = list(N.trials(Path('/owned'), DEVICES))

    def test_exact_order_and_unique_names(self):
        self.assertEqual([(r[1], r[2]) for r in self.rows], [
            ('baseline', 'on'), ('candidate', 'on'), ('candidate', 'on'), ('baseline', 'on'),
            ('candidate', 'off'), ('hsa', 'off'), ('hip', 'off'), ('baseline', 'off'),
            ('baseline', 'off'), ('hip', 'off'), ('hsa', 'off'), ('candidate', 'off')])
        self.assertEqual(len({r[0] for r in self.rows}), 12)

    def test_kfd_uses_same_elf_for_both_modes(self):
        for cohort in ('baseline', 'candidate'):
            rows = [r for r in self.rows if r[1] == cohort]
            self.assertEqual({r[3][0] for r in rows}, {'/owned/kfd-' + cohort})
            self.assertEqual({r[2] for r in rows}, {'on', 'off'})
            for _, _, mode, command, env in rows:
                self.assertEqual(command[1:7], [d[2] for d in DEVICES] + ['1048576', '1', '10', '30'])
                self.assertEqual(command[7], '--aggregate-peer-batch-hot-currentness-diagnose' if mode == 'on' else '--aggregate-peer-batch-hot-only')
                self.assertNotIn('ROCR_VISIBLE_DEVICES', env)
                self.assertNotIn('HIP_VISIBLE_DEVICES', env)

    def test_comparator_masks_identities_and_workload(self):
        for _, backend, mode, command, env in self.rows:
            if backend not in ('hip', 'hsa'):
                continue
            self.assertEqual(mode, 'off')
            self.assertEqual(command, ['/owned/peer-' + backend, '0', '1', '1048576', '1', '10', '30',
                                       *[d[2] for d in DEVICES], '--persistent-hot'])
            self.assertEqual(env['HSA_XNACK'], '0')
            own = 'HIP_VISIBLE_DEVICES' if backend == 'hip' else 'ROCR_VISIBLE_DEVICES'
            other = 'ROCR_VISIBLE_DEVICES' if backend == 'hip' else 'HIP_VISIBLE_DEVICES'
            self.assertEqual(env[own], '5,6')
            self.assertNotIn(other, env)

    def test_reversed_endpoint_arguments_stay_bound(self):
        rows = list(N.trials(Path('/owned'), DEVICES[::-1]))
        self.assertEqual(rows[0][3][1:3], [DEVICES[1][2], DEVICES[0][2]])
        self.assertEqual(rows[5][4]['ROCR_VISIBLE_DEVICES'], '6,5')

class SourceArchives(unittest.TestCase):
    def exercise(self, mutation=None):
        with tempfile.TemporaryDirectory(prefix='archive-test-', dir=HERE) as directory:
            root = Path(directory)
            tree, archive = root / 'tree', root / 'archive'
            tree.mkdir()
            archive.mkdir()
            files = {'Cargo.toml': b'[workspace]\n', 'crates/a/lib.rs': b'pub fn a() {}\n'}
            rows = []
            for name, raw in files.items():
                oid = hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()
                rows.append(f'100644 blob {oid}\t{name}'.encode())
            (tree / 'stdout').write_bytes(b'\0'.join(rows) + b'\0')
            with tarfile.open(archive / 'stdout', 'w') as output:
                for name in ('crates/', 'crates/a/'):
                    info = tarfile.TarInfo(name)
                    info.type = tarfile.DIRTYPE
                    output.addfile(info)
                if mutation == 'foreign-directory':
                    info = tarfile.TarInfo('../escape/')
                    info.type = tarfile.DIRTYPE
                    output.addfile(info)
                for name, raw in files.items():
                    if mutation == 'missing' and name == 'Cargo.toml':
                        continue
                    if mutation == 'substitution' and name == 'Cargo.toml':
                        raw = b'changed\n'
                    info = tarfile.TarInfo(name)
                    info.size = len(raw)
                    info.mode = 0o644
                    output.addfile(info, io.BytesIO(raw))
            class Recorder:
                def run(self, name, *args, **kwargs):
                    return tree if name.endswith('-tree') else archive
            return C.source(Recorder(), 'test', C.BASELINE, root)[1]

    def test_signed_file_roster_accepts_git_style_parent_directories(self):
        self.assertEqual(set(self.exercise()), {'Cargo.toml', 'crates/a/lib.rs'})

    def test_missing_substituted_and_foreign_directory_inputs_reject(self):
        for mutation in ('missing', 'substitution', 'foreign-directory'):
            with self.subTest(mutation=mutation), self.assertRaises(RuntimeError):
                self.exercise(mutation)

class CohortBuilds(unittest.TestCase):
    def test_cold_targets_are_distinct_with_other_settings_matched(self):
        with tempfile.TemporaryDirectory(prefix='target-test-', dir=HERE) as directory:
            root = Path(directory)
            baseline = C.build_environment(root, 'baseline')
            candidate = C.build_environment(root, 'candidate')
            self.assertEqual(Path(baseline['CARGO_TARGET_DIR']), root / 'target-baseline')
            self.assertEqual(Path(candidate['CARGO_TARGET_DIR']), root / 'target-candidate')
            self.assertNotEqual(baseline['CARGO_TARGET_DIR'], candidate['CARGO_TARGET_DIR'])
            for env in (baseline, candidate):
                self.assertEqual(list(Path(env['CARGO_TARGET_DIR']).iterdir()), [])
            self.assertEqual({k: v for k, v in baseline.items() if k != 'CARGO_TARGET_DIR'},
                             {k: v for k, v in candidate.items() if k != 'CARGO_TARGET_DIR'})
            with self.assertRaises(FileExistsError):
                C.build_environment(root, 'baseline')
            with self.assertRaises(RuntimeError):
                C.build_environment(root, '../baseline')

    def test_equal_cohort_elf_bytes_reject_before_remote_execution(self):
        with tempfile.TemporaryDirectory(prefix='elf-test-', dir=HERE) as directory:
            root = Path(directory)
            (root / 'kfd-baseline').write_bytes(b'baseline fixture')
            (root / 'kfd-candidate').write_bytes(b'baseline fixture')
            with self.assertRaises(RuntimeError):
                C.distinct_cohort_binaries(root)
            (root / 'kfd-candidate').write_bytes(b'candidate fixture')
            C.distinct_cohort_binaries(root)

class BuildSequence(unittest.TestCase):
    class Recorder:
        def __init__(self, output, cwd, equal=False):
            self.output, self.cwd, self.equal = output, cwd, equal
            self.output.mkdir()
            self.calls = []

        def run(self, name, command, seconds, *, env, **kwargs):
            self.calls.append((name, command, self.cwd, dict(env)))
            if command[0] in ('ssh', 'scp'):
                raise AssertionError('a rejected local build reached remote work')
            folder = self.output / name
            folder.mkdir()
            (folder / 'stdout').write_bytes((command[0] + '\n').encode())
            (folder / 'stderr').write_bytes(b'')
            if command[:2] == ['cargo', 'build']:
                # Deliberately stale target-keyed cache; old source mtimes cannot refresh it.
                binary = Path(env['CARGO_TARGET_DIR']) / ('x86_64-unknown-linux-musl/release/examples/' + C.EXAMPLE)
                if not binary.exists():
                    binary.parent.mkdir(parents=True)
                    binary.write_bytes(b'equal fixture' if self.equal else (self.cwd / 'Cargo.toml').read_bytes())
            return folder

    @staticmethod
    def fixture_source(rec, cohort, commit, output):
        checkout = output / ('source-' + cohort)
        checkout.mkdir()
        source = checkout / 'Cargo.toml'
        source.write_bytes((cohort + '\n').encode())
        os.utime(source, (1, 1))
        return checkout, C.B.inventory(checkout)

    def test_production_build_sequence_isolates_old_mtime_source_cohorts(self):
        with tempfile.TemporaryDirectory(prefix='build-test-', dir=HERE) as directory:
            root = Path(directory)
            payload = root / 'payload'
            payload.mkdir()
            rec = self.Recorder(root / 'local', C.REPO)
            with mock.patch.object(C, 'source', side_effect=self.fixture_source):
                for cohort in ('baseline', 'candidate'):
                    source, env = C.build_cohort(rec, cohort, cohort + '-commit', root, payload)
                    self.assertEqual(source['commit'], cohort + '-commit')
                    self.assertEqual((payload / ('kfd-' + cohort)).read_bytes(), (cohort + '\n').encode())
                    calls = [row for row in rec.calls if row[0].startswith(cohort + '-') and not row[0].endswith('-signature')]
                    self.assertEqual(len(calls), 6)
                    for _, _, cwd, actual_env in calls:
                        self.assertEqual(cwd, root / ('source-' + cohort))
                        self.assertEqual(actual_env, env)
                    self.assertEqual(Path(env['CARGO_TARGET_DIR']), root / ('target-' + cohort))
            C.distinct_cohort_binaries(payload)

    def test_existing_or_aliased_target_refuses_before_build_or_source(self):
        for alias in (False, True):
            with self.subTest(alias=alias), tempfile.TemporaryDirectory(prefix='alias-test-', dir=HERE) as directory:
                root = Path(directory)
                (root / 'target-baseline').mkdir()
                if alias:
                    (root / 'target-candidate').symlink_to(root / 'target-baseline', target_is_directory=True)
                else:
                    (root / 'target-candidate').mkdir()
                rec = self.Recorder(root / 'local', C.REPO)
                with mock.patch.object(C, 'source') as source, self.assertRaises(FileExistsError):
                    C.build_cohort(rec, 'candidate', 'candidate-commit', root, root)
                source.assert_not_called()
                self.assertEqual(rec.calls, [])

    def test_main_rejects_equal_elf_before_any_remote_call(self):
        with tempfile.TemporaryDirectory(prefix='main-test-', dir=HERE) as directory:
            output = Path(directory) / 'campaign'
            rec = self.Recorder(Path(directory) / 'records', C.REPO, equal=True)
            candidate = '1e13a90cd5e604a4612b079b48f629005a0a047b'
            def git(*args):
                if args[0] == 'rev-parse':
                    return candidate.encode()
                if args[0] == 'merge-base':
                    return b''
                if args[0] == 'diff':
                    return '\n'.join(sorted(C.EXPECTED_DELTA)).encode()
                raise AssertionError(args)
            argv = ['campaign.py', '--candidate', candidate, '--output', str(output), '--cpu-packet', str(output / 'missing-cpu')]
            for device in DEVICES:
                argv += ['--device', ','.join(map(str, device))]
            with mock.patch.object(sys, 'argv', argv), mock.patch.object(C.K, 'git', side_effect=git), \
                    mock.patch.object(C.B, 'Recorder', return_value=rec), \
                    mock.patch.object(C, 'source', side_effect=self.fixture_source), \
                    mock.patch.object(C.signal, 'signal'), mock.patch.object(C, 'qualified_cpu') as cpu:
                with self.assertRaisesRegex(RuntimeError, 'must not reuse the baseline ELF'):
                    C.main()
                cpu.assert_not_called()
            self.assertFalse((output / 'owner.json').exists())
            self.assertEqual(sum(row[1][:2] == ['cargo', 'build'] for row in rec.calls), 2)
            self.assertFalse(any(row[1][0] in ('ssh', 'scp') for row in rec.calls))

if __name__ == '__main__':
    unittest.main()
