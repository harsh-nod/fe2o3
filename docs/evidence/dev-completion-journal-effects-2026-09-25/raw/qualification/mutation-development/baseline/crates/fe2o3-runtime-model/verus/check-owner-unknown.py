#!/usr/bin/env python3
"""Source-bound developer verification of the three mark_unknown adapters.

This records executable correspondence, negative controls and CPU regression
checks. It is not the native admission gate or a performance qualification.
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
ROOT = CRATE / 'verus/context_owner_unknown_historical_v1.rs'
RAW = CRATE / 'verus/context_owner_unknown_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/unknown_wrapper_bodies.rs'
RETAINED = CRATE / 'src/context_version_journal/retained_bodies.rs'
BRIDGE = CRATE / 'verus/context_owner_unknown_correspondence_v1.rs'
FROZEN = 'e3ea84c1f3140600e637e9d60e16d95a7be9fd6d'
LEGACY = Path('docs/evidence/dev-producer-stable-2026-09-22/check.py')
NEW = [ROOT, RAW, BODY, BRIDGE, *[CRATE / ('verus/context_owner_unknown_' + name + '_v1.rs')
       for name in ('decisions', 'bodies', 'historical_bodies', 'witnesses')]]
# name, source, unique declaration, module, function, before, after, kind
MUTATIONS = [
    ('journal_noop', BODY, 'macro_rules! journal_unknown_wrapper_body {', 'production', 'ContextVersionJournalV1::mark_unknown',
     '$execute($journal, $writer)', 'Ok(())', 'execution'),
    ('stable_noop', BODY, 'macro_rules! stable_unknown_wrapper_body {', 'production', 'ContextReadLeasedJournalV1::mark_unknown',
     '$owner.journal.mark_unknown($writer)', 'Ok(())', 'execution'),
    ('producer_noop', BODY, 'macro_rules! producer_unknown_wrapper_body {', 'production', 'ContextProducerReadJournalV1::mark_unknown',
     '$owner.stable.mark_unknown($writer)', 'Ok(())', 'execution'),
    ('producer_frame', BODY, 'macro_rules! producer_unknown_wrapper_body {', 'production', 'ContextProducerReadJournalV1::mark_unknown',
     '$owner.stable.mark_unknown($writer)', '{ $owner.next_incarnation = 0; $owner.stable.mark_unknown($writer) }', 'execution'),
    ('skip_chain', RETAINED, 'macro_rules! retained_unknown_body {', 'production', 'shared_retained_unknown_v1',
     'let result = shared_retained_chain_v1($journal, $writer, head, count);',
     'let result: Result<(), ReadErrorV1> = Ok(());', 'execution'),
    ('skip_mutation', RETAINED, 'macro_rules! retained_unknown_body {', 'production', 'shared_retained_unknown_v1',
     'if !unknown {', 'if false {', 'execution'),
    ('unknown_shortcut', RETAINED, 'macro_rules! retained_unknown_body {', 'production', 'shared_retained_unknown_v1',
     'let result = shared_retained_chain_v1($journal, $writer, head, count);',
     'if unknown { return Ok(()); }\n        let result = shared_retained_chain_v1($journal, $writer, head, count);', 'execution'),
    ('skip_actual', BRIDGE, 'fn producer_unknown_historical_exec_v1(', 'production', 'producer_unknown_historical_exec_v1',
     'let result = actual.mark_unknown(writer);', 'let result: Result<(), ReadErrorV1> = Ok(());', 'execution'),
    ('skip_historical', BRIDGE, 'fn producer_unknown_historical_exec_v1(', 'production', 'producer_unknown_historical_exec_v1',
     'let model_result = logical::unknown_exec_v1(&mut model.stable.journal, model_writer);',
     'let model_result: Result<(), logical::ReadErrorV1> = Ok(());', 'execution'),
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
    historical = (repo / CRATE / 'verus/context_version_journal_settlement_v1.rs').read_text()
    names = ['retained_header_decision_v1', 'retained_header_exec_v1', 'retained_member_decision_v1',
             'retained_member_exec_v1', 'retained_scan_v1', 'retained_chain_decision_v1', 'retained_chain_exec_v1',
             'unknown_decision_v1', 'unknown_execution_relation_v1', 'unknown_exec_v1']
    declarations = list(re.finditer(r'^pub (?:(?:open spec|proof) )?fn (\w+)', historical, re.M))
    pieces = []
    for name in names:
        selected = [i for i, match in enumerate(declarations) if match[1] == name]
        need(len(selected) == 1, 'unique historical declaration: ' + name)
        index = selected[0]
        pieces.append(historical[declarations[index].start():declarations[index + 1].start()].rstrip())
    expected = ('// Exact retained and Unknown declarations from the historical settlement, in the importing type universe.\n'
                'use super::*;\n\nverus! {\n\n' + '\n\n'.join(pieces) + '\n\n}\n')
    need((repo / CRATE / 'verus/context_owner_unknown_historical_bodies_v1.rs').read_text() == expected,
         'unchanged historical projection')
    baseline = module(repo, Path('docs/evidence/dev-producer-stable-2026-09-22/performance.py'), 'unknown_frozen_parser')
    for owner, path, inner in [
        ('context_producer_reads', 'context_producer_reads.rs', 'self.stable.mark_unknown(writer)'),
        ('context_read_leases', 'context_read_leases.rs', 'self.journal.mark_unknown(writer)'),
        ('context_version_journal', 'context_version_journal/settlement.rs', 'retained::shared_retained_unknown_v1(self, writer)'),
    ]:
        old = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(CRATE / 'src' / path)], text=True)
        original = baseline.method(old, 'mark_unknown')
        frozen = (repo / CRATE / 'src' / owner / 'unknown_baseline.rs').read_text()
        frozen = baseline.method(frozen, 'baseline_mark_unknown_v1')
        transformed = original.replace('pub fn mark_unknown(', 'pub(crate) fn baseline_mark_unknown_v1(')
        if owner != 'context_version_journal':
            transformed = transformed.replace(inner, inner.replace('.mark_unknown(', '.baseline_mark_unknown_v1('))
        need(frozen == transformed, 'exact frozen adapter: ' + owner)
        adapter = {
            'context_producer_reads': 'producer_unknown_wrapper_body!(self, writer)',
            'context_read_leases': 'stable_unknown_wrapper_body!(self, writer)',
            'context_version_journal': 'journal_unknown_wrapper_body!(self, writer, retained::shared_retained_unknown_v1)',
        }[owner]
        current = baseline.method((repo / CRATE / 'src' / path).read_text(), 'mark_unknown')
        need(current == original.replace(inner, adapter), 'exact public adapter: ' + owner)
    for path in [RETAINED, CRATE / 'src/context_version_journal/retained.rs',
                 CRATE / 'src/context_version_journal/declarations.rs', CRATE / 'src/context_version_journal.rs',
                 CRATE / 'verus/context_version_journal_settlement_v1.rs']:
        old = subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(path)])
        need((repo / path).read_bytes() == old, 'unchanged retained leaf/type/instrumentation: ' + str(path))


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
    legacy = module(repo, LEGACY, 'unknown_legacy_check')
    base = module(repo, legacy.BASE, 'unknown_process_recorder')
    policy = module(repo, legacy.POLICY, 'unknown_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    authenticate(repo)
    sources = sorted(set(legacy.SOURCES) | set(NEW))
    inputs = set(sources) | set(legacy.PINS) | {LEGACY, Path(__file__).resolve().relative_to(repo),
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
                for path in set(legacy.POLICY_SOURCES) | (set(NEW) - {ROOT, RAW, BODY}):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (BODY, 'audited-unknown-wrappers.rs', 3)]:
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
                                    'success': True, 'verified': 193 if name == 'raw' else 728,
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
