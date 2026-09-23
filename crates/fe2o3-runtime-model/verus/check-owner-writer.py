#!/usr/bin/env python3
"""Source-bound writer lifecycle correspondence and CPU qualification.

Not a native admission, physical-allocation or performance qualification.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
ROOT = CRATE / 'verus/context_owner_writer_historical_v1.rs'
RAW = CRATE / 'verus/context_owner_writer_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/writer_lifecycle_bodies.rs'
BRIDGE = CRATE / 'verus/context_owner_writer_correspondence_v1.rs'
FROZEN = '9b55f0c15895c0eb4d825410a2732542334e8225'
PREVIOUS = CRATE / 'verus/check-owner-enrollment.py'
LEGACY = Path('docs/evidence/dev-producer-stable-2026-09-22/check.py')
POLICY_NEW = [BRIDGE, *[CRATE / ('verus/context_owner_writer_' + name + '_v1.rs')
    for name in ('bodies', 'witnesses')]]
PROJECTIONS = [(BODY, 'audited-writer-lifecycle.rs', 4)]
NEW = [ROOT, RAW, BODY, *POLICY_NEW]
# name, source, unique declaration, module, function, before, after, kind
MUTATIONS = [
    ('register_id', BODY, 'macro_rules! writer_register_body {', 'production',
     'ContextVersionJournalV1::register_writer', 'if !$issuable($key.local)', 'if false', 'execution'),
    ('register_replay', BODY, 'macro_rules! writer_register_body {', 'production',
     'ContextVersionJournalV1::register_writer', 'if $key.local <= $journal.registration_watermark', 'if false', 'execution'),
    ('register_store', BODY, 'macro_rules! writer_register_body {', 'production',
     'ContextVersionJournalV1::register_writer', '$journal.writers[slot] = Some(WriterEntryV1::Reserved($key));', '', 'execution'),
    ('register_watermark', BODY, 'macro_rules! writer_register_body {', 'production',
     'ContextVersionJournalV1::register_writer', '$journal.registration_watermark = $key.local;', '', 'execution'),
    ('lookup_noop', BODY, 'macro_rules! writer_reserved_lookup_body {', 'production',
     'ContextVersionJournalV1::lookup_reserved', '$lookup($journal, $reference)', 'Ok($reference.key)', 'execution'),
    ('abort_capacity', BODY, 'macro_rules! writer_abort_body {', 'production',
     'ContextVersionJournalV1::abort_reserved_observed_v1',
     'if $journal.free.len() >= $journal.writer_capacity || $journal.free.len() >= $capacity', 'if false', 'execution'),
    ('abort_watermark', BODY, 'macro_rules! writer_abort_body {', 'production',
     'ContextVersionJournalV1::abort_reserved_observed_v1', '$journal.reserved_count = reserved_count;',
     '$journal.reserved_count = reserved_count; $journal.registration_watermark = 0;', 'execution'),
    ('abort_physical_capacity', BODY, 'macro_rules! writer_abort_body {', 'production',
     'ContextVersionJournalV1::abort_reserved_observed_v1',
     ' || $journal.free.len() >= $capacity', '', 'execution'),
    ('stable_frame', BODY, 'macro_rules! writer_owner_forward_body {', 'production',
     'ContextReadLeasedJournalV1::register_writer', '$owner.$field.$method($argument $($extra)*)',
     '{ $owner.next_incarnation = 0; $owner.$field.$method($argument $($extra)*) }', 'execution'),
    ('producer_frame', BODY, 'macro_rules! writer_owner_forward_body {', 'production',
     'ContextProducerReadJournalV1::abort_reserved_observed_v1', '$owner.$field.$method($argument $($extra)*)',
     '{ $owner.next_incarnation = 0; $owner.$field.$method($argument $($extra)*) }', 'execution'),
    ('skip_actual_register', BRIDGE, 'fn owner_register_historical_exec_v1(', 'production', 'owner_register_historical_exec_v1',
     'let result = actual.register_writer(key);',
     'let result: Result<WriterReferenceV1, ReadErrorV1> = Err(ReadErrorV1::WriterReplay);', 'execution'),
    ('skip_historical_register', BRIDGE, 'fn owner_register_historical_exec_v1(', 'production', 'owner_register_historical_exec_v1',
     'let model_result = logical::register_contents_exec_v1(&mut model.stable.journal, model_key);',
     'let model_result: Result<logical::WriterReferenceV1, logical::JournalErrorV1> = Err(logical::JournalErrorV1::WriterReplay);', 'execution'),
    ('skip_actual_abort', BRIDGE, 'fn owner_abort_historical_exec_v1(', 'production', 'owner_abort_historical_exec_v1',
     'let result = actual.abort_reserved_observed_v1(reference, observed_free_capacity);',
     'let result: Result<(), ReadErrorV1> = Ok(());', 'execution'),
    ('skip_historical_abort', BRIDGE, 'fn owner_abort_historical_exec_v1(', 'production', 'owner_abort_historical_exec_v1',
     'let model_result = logical::abort_contents_exec_v1(&mut model.stable.journal, observed_free_capacity, model_reference);',
     'let model_result: Result<(), logical::JournalErrorV1> = Ok(());', 'execution'),
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
    baseline = module(repo, Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'), 'writer_frozen_parser')
    def old(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(path)], text=True)
    def compact(text):
        return re.sub(r'\s+', '', text)
    for owner, field in [('context_version_journal', None), ('context_read_leases', 'journal'), ('context_producer_reads', 'stable')]:
        path = CRATE / 'src' / (owner + '.rs')
        previous, current = old(path), (repo / path).read_text()
        frozen = (repo / CRATE / 'src' / owner / 'writer_lifecycle_baseline.rs').read_text()
        names = ['register_writer', 'abort_reserved'] + ([] if field else ['lookup_reserved'])
        for name in names:
            original = baseline.method(previous, name)
            transformed = original.replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(')
            if field:
                transformed = transformed.replace('.' + name + '(', '.baseline_' + name + '_v1(')
            elif name == 'abort_reserved':
                transformed = transformed.replace('self.lookup_reserved(', 'self.baseline_lookup_reserved_v1(')
            need(baseline.method(frozen, 'baseline_' + name + '_v1') == transformed, 'exact frozen method: ' + owner + ':' + name)
            if field:
                argument = 'key' if name == 'register_writer' else 'writer'
                adapter = f'writer_owner_forward_body!(self, {field}, {name}, {argument}, [])'
            else:
                adapter = {
                    'register_writer': 'writer_register_body!(writer_rust_expr, self, key, issuable_context_id, Self::count_indexed_access)',
                    'lookup_reserved': 'writer_reserved_lookup_body!(self, reference, begin::begin_reserved_exec_v1)',
                    'abort_reserved': 'writer_abort_body!(writer_rust_expr, self, reference, self.free.capacity(), Self::count_indexed_access)',
                }[name]
            expected = original.split(' {', 1)[0] + ' {\n        ' + adapter + '\n    }\n'
            need(compact(baseline.method(current, name)) == compact(expected), 'exact shared adapter: ' + owner + ':' + name)
        if field is None:
            for name in ('read_slot', 'store_slot', 'next_free', 'pop_free', 'push_free', 'count_indexed_access'):
                need(baseline.method(previous, name) == baseline.method(current, name), 'unchanged instrumentation: ' + name)
            identity = 'fn issuable_context_id(value: u64) -> bool {\n    value != 0 && value != u64::MAX\n}'
            need(previous.count(identity) == current.count(identity) == 1, 'unchanged ID predicate')
    path = CRATE / 'src/context_version_journal/begin.rs'
    need((repo / path).read_text() == old(path).replace('\nfn begin_reserved_exec_v1(', '\npub(super) fn begin_reserved_exec_v1('),
         'Begin adapter changed only visibility')
    for path in [
        *[CRATE / ('src/' + owner + '/declarations.rs') for owner in ('context_version_journal', 'context_read_leases', 'context_producer_reads')],
        CRATE / 'src/context_version_journal/retained.rs',
        CRATE / 'src/context_version_journal/retained_bodies.rs',
        CRATE / 'src/context_version_journal/begin_bodies.rs',
        CRATE / 'verus/context_version_journal_issuance_v1.rs',
        CRATE / 'verus/context_producer_journal_issuance_v1.rs',
    ]:
        need((repo / path).read_text() == old(path), 'unchanged declarations/lookup/history: ' + str(path))


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
    previous = module(repo, PREVIOUS, 'writer_previous_check')
    ancestor = module(repo, previous.PREVIOUS, 'writer_ancestor_check')
    legacy = module(repo, LEGACY, 'enrollment_legacy_check')
    base = module(repo, legacy.BASE, 'enrollment_process_recorder')
    policy = module(repo, legacy.POLICY, 'enrollment_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    authenticate(repo)
    sources = sorted(set(legacy.SOURCES) | set(ancestor.NEW) | set(previous.NEW) | set(NEW))
    inputs = set(sources) | set(legacy.PINS) | {LEGACY, PREVIOUS, previous.PREVIOUS, Path(__file__).resolve().relative_to(repo),
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
                    need(digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                for path in set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY}) | set(previous.POLICY_NEW) | set(POLICY_NEW):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3), *previous.PROJECTIONS, *PROJECTIONS]:
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
                                    'success': True, 'verified': 278 if name == 'raw' else 845,
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
