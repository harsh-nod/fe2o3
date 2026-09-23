#!/usr/bin/env python3
"""Source-bound batch enrollment owner correspondence and CPU qualification.

Not a native admission, physical-allocation or performance qualification.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
ROOT = CRATE / 'verus/context_owner_enrollment_historical_v1.rs'
RAW = CRATE / 'verus/context_owner_enrollment_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/enrollment_wrapper_bodies.rs'
LEAF = CRATE / 'src/context_version_journal/enrollment_bodies.rs'
BRIDGE = CRATE / 'verus/context_owner_enrollment_correspondence_v1.rs'
FROZEN = 'ea7d4e0f2a9c4dd9635fb4e5bbf186f902d9425c'
PREVIOUS = CRATE / 'verus/check-owner-unknown.py'
LEGACY = Path('docs/evidence/dev-producer-stable-2026-09-22/check.py')
POLICY_NEW = [BRIDGE, *[CRATE / ('verus/context_owner_enrollment_' + name + '_v1.rs')
    for name in ('bodies', 'invariant', 'witnesses')],
    *[CRATE / ('verus/context_journal_enrollment_' + name + '_v1.rs') for name in (
        'ordering_bodies', 'adaptive_bodies', 'execution_bodies', 'value_views',
        'historical_bodies', 'historical_witnesses', 'custody_witness')]]
PROJECTIONS = [(BODY, 'audited-enrollment-wrappers.rs', 3), (LEAF, 'audited-enrollment-bodies.rs', 10),
    (CRATE / 'src/context_version_journal/enrollment_ordering_bodies.rs', 'audited-enrollment-ordering.rs', 8),
    (CRATE / 'src/context_version_journal/enrollment_adaptive_bodies.rs', 'audited-enrollment-adaptive.rs', 13)]
NEW = [ROOT, RAW, *POLICY_NEW, *[row[0] for row in PROJECTIONS],
    CRATE / 'src/context_version_journal/enrollment_declarations.rs',
    CRATE / 'verus/context_journal_enrollment_ordering_v1.rs',
    CRATE / 'verus/context_journal_enrollment_adaptive_v1.rs',
    CRATE / 'verus/context_journal_enrollment_execution_v1.rs']
# name, source, unique declaration, module, function, before, after, kind
MUTATIONS = [
    ('journal_noop', BODY, 'macro_rules! journal_enrollment_wrapper_body {', 'production',
     'ContextVersionJournalV1::enroll_allocations', '$execute($journal, $entries, $output)', 'Ok(())', 'execution'),
    ('stable_noop', BODY, 'macro_rules! stable_enrollment_wrapper_body {', 'production',
     'ContextReadLeasedJournalV1::enroll_allocations', '$owner.journal.enroll_allocations($entries, $output)', 'Ok(())', 'execution'),
    ('producer_noop', BODY, 'macro_rules! producer_enrollment_wrapper_body {', 'production',
     'ContextProducerReadJournalV1::enroll_allocations', '$owner.stable.enroll_allocations($entries, $output)', 'Ok(())', 'execution'),
    ('producer_frame', BODY, 'macro_rules! producer_enrollment_wrapper_body {', 'production',
     'ContextProducerReadJournalV1::enroll_allocations', '$owner.stable.enroll_allocations($entries, $output)',
     '{ $owner.next_incarnation = 0; $owner.stable.enroll_allocations($entries, $output) }', 'execution'),
    ('skip_replay', LEAF, 'macro_rules! enrollment_journal_body {', 'production', 'enrollment_journal_exec_v1',
     'if enrollment_replay_exec_v1($contents, $entries)', 'if false', 'execution'),
    ('skip_commit', LEAF, 'macro_rules! enrollment_journal_body {', 'production', 'enrollment_journal_exec_v1',
     'enrollment_commit_journal_exec_v1($contents, $entries, $output, $remaining);', '', 'execution'),
    ('skip_clear', LEAF, 'macro_rules! enrollment_journal_body {', 'production', 'enrollment_journal_exec_v1',
     'enrollment_clear_output_exec_v1($output);', '', 'execution'),
    ('skip_actual', BRIDGE, 'fn producer_enrollment_historical_exec_v1(', 'production', 'producer_enrollment_historical_exec_v1',
     'let result = actual.enroll_allocations(entries, output);', 'let result: Result<(), EnrollmentErrorV1> = Ok(());', 'execution'),
    ('skip_historical', BRIDGE, 'fn producer_enrollment_historical_exec_v1(', 'production', 'producer_enrollment_historical_exec_v1',
     'let model_result = logical::enrollment_journal_exec_v1(&mut model.stable.journal, model_entries, model_output);',
     'let model_result: Result<(), logical::EnrollmentErrorV1> = Ok(());', 'execution'),
]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def authenticate(repo):
    module(repo, PREVIOUS, 'enrollment_prior_auth').authenticate(repo)
    baseline = module(repo, Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'), 'enrollment_frozen_parser')
    for owner, path, inner, adapter in [
        ('context_producer_reads', 'context_producer_reads.rs', 'self.stable.enroll_allocations(entries, output)',
         'producer_enrollment_wrapper_body!(self, entries, output)'),
        ('context_read_leases', 'context_read_leases.rs', 'self.journal.enroll_allocations(entries, output)',
         'stable_enrollment_wrapper_body!(self, entries, output)'),
        ('context_version_journal', 'context_version_journal/allocation_lifecycle.rs',
         'enrollment::enrollment_journal_exec_v1(self, canonical, output)',
         'journal_enrollment_wrapper_body!(\n            self,\n            canonical,\n            output,\n            enrollment::enrollment_journal_exec_v1\n        )'),
    ]:
        old = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(CRATE / 'src' / path)], text=True)
        original = baseline.method(old, 'enroll_allocations')
        frozen = baseline.method((repo / CRATE / 'src' / owner / 'enrollment_baseline.rs').read_text(), 'baseline_enroll_allocations_v1')
        transformed = original.replace('pub fn enroll_allocations(', 'pub(crate) fn baseline_enroll_allocations_v1(')
        if owner != 'context_version_journal':
            transformed = transformed.replace(inner, inner.replace('.enroll_allocations(', '.baseline_enroll_allocations_v1('))
        need(frozen == transformed, 'exact frozen adapter: ' + owner)
        current = baseline.method((repo / CRATE / 'src' / path).read_text(), 'enroll_allocations')
        need(current == original.replace(inner, adapter), 'exact public adapter: ' + owner)
    unchanged = [CRATE / ('src/context_version_journal/' + name + '.rs') for name in (
        'enrollment_bodies', 'enrollment_ordering_bodies', 'enrollment_adaptive_bodies', 'enrollment_declarations', 'declarations')]
    unchanged += [CRATE / 'src/context_version_journal/allocation_lifecycle/enrollment.rs',
        CRATE / 'src/context_version_journal/allocation_lifecycle/ordering.rs',
        CRATE / 'verus/context_version_journal_enrollment_v1.rs']
    unchanged += [p.relative_to(repo) for p in (repo / CRATE / 'src/context_version_journal/allocation_lifecycle/ordering').rglob('*.rs')]
    for path in unchanged:
        old = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(path)])
        need((repo / path).read_bytes() == old, 'unchanged enrollment algorithm/type/history: ' + str(path))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--probe', action='store_true')
    args = parser.parse_args()
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    os.chdir(repo)
    previous = module(repo, PREVIOUS, 'enrollment_previous_check')
    legacy = module(repo, LEGACY, 'enrollment_legacy_check')
    base = module(repo, legacy.BASE, 'enrollment_process_recorder')
    policy = module(repo, legacy.POLICY, 'enrollment_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    authenticate(repo)
    sources = sorted(set(legacy.SOURCES) | set(previous.NEW) | set(NEW))
    inputs = set(sources) | set(legacy.PINS) | {LEGACY, PREVIOUS, Path(__file__).resolve().relative_to(repo),
        CRATE / 'verus/context_version_journal_settlement_v1.rs',
        Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'),
        Path('docs/runtime-producer-read-reservations-v1.md'),
        Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), CRATE / 'Cargo.toml'}
    inputs.update(p.relative_to(repo) for p in (repo / CRATE / 'src').rglob('*') if p.is_file())
    before = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    if not args.probe:
        for path, expected in before.items():
            data = subprocess.check_output(['git', '-C', str(repo), 'show', commit + ':' + path])
            need(digest(data) == expected, 'source commit mismatch: ' + path)
    for path, pin in legacy.PINS.items():
        need(digest((repo / path).read_bytes()) == pin, 'source pin: ' + str(path))
    output.mkdir()
    (output / 'source.json').write_text(json.dumps({'commit': commit, 'probe': args.probe, 'inputs': before}, indent=2) + '\n')
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve())}
    results = {}
    for phase in ['before', 'after']:
        status, _, _ = base.run_owned(['/bin/sh', str(repo / legacy.CLOSURE), str(verus.parent), str(repo / legacy.MANIFEST)],
                                      120, output / ('closure-' + phase), env)
        need(status == 0, 'pinned Verus distribution')
        if phase == 'after':
            break
        cases = [('raw', None), ('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None)]
        for name, mutation in cases:
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                for path in set(legacy.POLICY_SOURCES) | (set(previous.NEW) - {previous.ROOT, previous.RAW, previous.BODY}) | set(POLICY_NEW):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (previous.BODY, 'audited-unknown-wrappers.rs', 3), *PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                command = legacy.command(verus, stage / (RAW if name == 'raw' else ROOT), mutation)
                status, stdout, stderr = base.run_owned(command, 610, output / name, env)
                observed = legacy.normalized(base, stdout, stderr, stage)
                if mutation:
                    # Reuse the scoped logical-failure checks; this developer record
                    # captures diagnostics, rather than claiming precommitted replay.
                    legacy.check_result(base, status, stdout, stderr, stage, mutation, {name: observed})
                else:
                    need(status == 0 and not observed['diagnostics'], 'clean terminal positive')
                    report = observed['result']
                    need(report == {'encountered-error': False, 'encountered-vir-error': False,
                                    'success': True, 'verified': 270 if name == 'raw' else 829,
                                    'errors': 0, 'is-verifying-entire-crate': True}, 'exact whole-root positive')
                results[name] = observed
                print(name, observed['result'], flush=True)
        need(results['positive-before'] == results['positive-after'], 'matching whole-root positives')
    for name, command in [
        ('compiler', ['rustc', '+1.97.1', '-Vv']),
        ('format', ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
    ]:
        status, _, _ = base.run_owned(command, 600, output / name, env)
        need(status == 0, 'CPU check: ' + name)
        print(name, 'passed', flush=True)
    after = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
