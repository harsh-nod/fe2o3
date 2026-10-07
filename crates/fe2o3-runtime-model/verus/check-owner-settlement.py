#!/usr/bin/env python3
"""Record actual-owner settlement verification and frozen CPU regression tests.

This checkpoint does not claim settlement historical correspondence, custody
preservation, physical-capacity refinement, native admission, or performance.
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
RAW = CRATE / 'verus/context_owner_settlement_execution_v1.rs'
RAW_SHA256 = 'ab7179857276bfc61dff5c338a251f76b68ba877b26c9cd9c24f6b3422dead38'
BODY = CRATE / 'src/context_version_journal/settlement_wrapper_bodies.rs'
COMMIT = CRATE / 'src/context_version_journal/settlement_commit_body.rs'
FROZEN = '269bb2c3d57b1221ee5e8871bf935d61dbacdc27'
PREVIOUS = CRATE / 'verus/check-owner-writer.py'
LEGACY = Path('docs/evidence/dev-producer-stable-2026-09-22/check.py')
CPU_PARSER = Path('docs/evidence/dev-producer-stable-2026-09-22/audit.py')
POLICY_NEW = [CRATE / ('verus/context_owner_settlement_' + name + '_v1.rs')
              for name in ('decisions', 'bodies', 'witnesses')]
NEW = [RAW, BODY, *POLICY_NEW]
REUSED = [CRATE / ('src/context_version_journal/' + name + '.rs') for name in
          ('settlement_return_body', 'settlement_scratch_bodies', 'settlement_commit_body', 'settlement_storage_declarations')]
PROJECTIONS = [(BODY, 'audited-settlement-wrappers.rs', 4),
               (REUSED[0], 'audited-settlement-return.rs', 1),
               (REUSED[1], 'audited-settlement-scratch.rs', 2),
               (REUSED[2], 'audited-settlement-commit.rs', 1)]
# name, source, unique declaration, module, function, before, after, kind
MUTATIONS = [
    ('evidence', BODY, 'macro_rules! settlement_preflight_body {', '',
     'ContextVersionJournalV1::preflight_settlement_observed_v1',
     'if $evidence.slot != $writer.slot || !$same_key($evidence.key, $writer.key)', 'if false', 'execution'),
    ('unknown_header', BODY, 'macro_rules! settlement_preflight_body {', '',
     'ContextVersionJournalV1::preflight_settlement_observed_v1',
     '$header($journal, $writer, false)', '$header($journal, $writer, true)', 'execution'),
    ('chain', BODY, 'macro_rules! settlement_preflight_body {', '',
     'ContextVersionJournalV1::preflight_settlement_observed_v1',
     'if let Err(error) = $chain($journal, $writer, head, count) { return Err(error); }', '', 'execution'),
    ('writer_capacity', BODY, 'macro_rules! settlement_preflight_body {', '',
     'ContextVersionJournalV1::preflight_settlement_observed_v1',
     'writer_storage: $writer_capacity', 'writer_storage: usize::MAX', 'execution'),
    ('member_capacity', BODY, 'macro_rules! settlement_preflight_body {', '',
     'ContextVersionJournalV1::preflight_settlement_observed_v1',
     'member_storage: $member_capacity', 'member_storage: usize::MAX', 'execution'),
    ('success_selection', BODY, 'macro_rules! settlement_outcome_body {', '',
     'ContextVersionJournalV1::settle_success_observed_v1',
     '$evidence.writer, $success', '$evidence.writer, false', 'execution'),
    ('no_effect_selection', BODY, 'macro_rules! settlement_outcome_body {', '',
     'ContextVersionJournalV1::settle_no_effect_observed_v1',
     '$evidence.writer, $success', '$evidence.writer, true', 'execution'),
    ('lineage', COMMIT, 'macro_rules! settlement_commit_body {', '', 'shared_settlement_commit_v1',
     'if $success {', 'if false {', 'execution'),
    ('scratch_clear', COMMIT, 'macro_rules! settlement_commit_body {', '', 'shared_settlement_commit_v1',
     '.take()', '', 'execution'),
    ('stable_frame', BODY, 'macro_rules! settlement_owner_forward_body {', '',
     'ContextReadLeasedJournalV1::settle_success_observed_v1',
     '$owner.$field.$method($writer, $evidence $($extra)*)',
     '{ $owner.next_incarnation = 0; $owner.$field.$method($writer, $evidence $($extra)*) }', 'execution'),
    ('producer_frame', BODY, 'macro_rules! settlement_owner_forward_body {', '',
     'ContextProducerReadJournalV1::settle_no_effect_observed_v1',
     '$owner.$field.$method($writer, $evidence $($extra)*)',
     '{ $owner.next_incarnation = 0; $owner.$field.$method($writer, $evidence $($extra)*) }', 'execution'),
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


def method(source, name):
    anchors = list(re.finditer(r'^    (?:pub(?:\([^)]*\))? )?fn ' + re.escape(name) + r'\(', source, re.M))
    need(len(anchors) == 1, 'unique method: ' + name)
    start = anchors[0].start()
    return source[start:source.index('\n    }\n', start) + len('\n    }\n')]


def authenticate(repo, inherited):
    need(digest((repo / RAW).read_bytes()) == RAW_SHA256, 'exact raw include/declaration envelope')
    def old(path):
        return subprocess.check_output(['git', '-C', str(repo), 'show', FROZEN + ':' + str(path)]).decode()
    def compact(value):
        return re.sub(r'\s+', '', value)
    src = CRATE / 'src'
    changed = set()
    for owner, field in [('context_version_journal', None), ('context_read_leases', 'journal'), ('context_producer_reads', 'stable')]:
        path = src / (owner + ('/settlement.rs' if field is None else '.rs'))
        previous, current = old(path), (repo / path).read_text()
        names = ['settle_success', 'settle_no_effect'] + ([] if field else ['preflight_settlement', 'settle_retained'])
        frozen = (repo / src / owner / 'settlement_baseline.rs').read_text()
        reconstructed = previous
        pieces = []
        for name in names:
            original = method(previous, name)
            baseline = re.sub(r'^    (?:pub(?:\([^)]*\))? )?fn ', '    pub(crate) fn ', original)
            baseline = re.sub(r'\b(settle_success|settle_no_effect|preflight_settlement|settle_retained)\b',
                              lambda m: 'baseline_' + m[0] + '_v1', baseline)
            need(method(frozen, 'baseline_' + name + '_v1') == baseline, 'exact frozen chain: ' + owner + ':' + name)
            pieces.append(baseline.rstrip())
            if field:
                adapter = f'settlement_owner_forward_body!(self, {field}, {name}, writer, evidence, [])'
            elif name in ('settle_success', 'settle_no_effect'):
                outcome = 'true' if name == 'settle_success' else 'false'
                adapter = f'settlement_outcome_body!(self, settle_retained, writer, evidence, {outcome}, [])'
            elif name == 'preflight_settlement':
                adapter = ('settlement_preflight_body!(settlement_rust_expr, self, writer, evidence, '
                           'self.free.capacity(), self.member_free.capacity(), retained::shared_retained_header_v1, '
                           'retained::shared_retained_writer_key_v1, retained::shared_retained_chain_v1, '
                           'settlement_scratch::shared_settlement_scratch_scan_v1)')
            else:
                adapter = ('settlement_execute_body!(settlement_rust_expr, self, writer, evidence, success, head, count, '
                           'preflight_settlement, [], settlement_scratch::shared_settlement_scratch_stage_v1, '
                           'settlement_commit::shared_settlement_commit_v1, [], [], [])')
            expected = original.split(' {', 1)[0] + ' {\n        ' + adapter + '\n    }\n'
            need(compact(method(current, name)) == compact(expected), 'exact settlement adapter: ' + owner + ':' + name)
            reconstructed = reconstructed.replace(original, method(current, name))
        ty = {'context_version_journal': 'ContextVersionJournalV1', 'context_read_leases': 'ContextReadLeasedJournalV1',
              'context_producer_reads': 'ContextProducerReadJournalV1'}[owner]
        expected = ('// Frozen at ' + FROZEN + '; only names and visibility are redirected.\nuse super::*;\n\n'
                    'impl ' + ty + ' {\n' + '\n\n'.join(pieces) + '\n}\n')
        need(frozen == expected, 'closed frozen file: ' + owner)
        if field:
            anchor = 'mod writer_lifecycle_templates {\n    include!("context_version_journal/writer_lifecycle_bodies.rs");\n}'
            addition = ('\n\n#[allow(unused_macros)]\n#[macro_use]\nmod settlement_templates {\n'
                        '    include!("context_version_journal/settlement_wrapper_bodies.rs");\n}')
            reconstructed = reconstructed.replace(anchor, anchor + addition)
            reconstructed = reconstructed.replace('mod writer_lifecycle_baseline;',
                                                   'mod writer_lifecycle_baseline;\n\n#[cfg(test)]\nmod settlement_baseline;')
        else:
            reconstructed = reconstructed.replace('mod disposal;', 'mod disposal;\n\n#[cfg(test)]\n'
                                                   '#[path = "settlement_baseline.rs"]\nmod settlement_baseline;')
            anchor = 'mod unknown_templates {\n    include!("unknown_wrapper_bodies.rs");\n}'
            addition = ('\n\n#[allow(unused_macros)]\n#[macro_use]\nmod settlement_templates {\n'
                        '    include!("settlement_wrapper_bodies.rs");\n}\n\nmacro_rules! settlement_rust_expr {\n'
                        '    ($body:expr) => {\n        $body\n    };\n}')
            reconstructed = reconstructed.replace(anchor, anchor + addition)
            reconstructed = reconstructed.replace('    pub(super) fn preflight_settlement(',
                                                   '    #[allow(clippy::question_mark)]\n    pub(super) fn preflight_settlement(')
        need(current == reconstructed, 'only authenticated runtime adapter deltas: ' + owner)
        changed.add(path)
    exceptions = changed | {src / (owner + '/guard_test_support.rs') for owner in
                            ('context_version_journal', 'context_read_leases', 'context_producer_reads')}
    exceptions.update([src / 'context_version_journal/settlement_tests.rs', src / 'context_producer_reads/tests.rs'])
    roster = subprocess.check_output(['git', '-C', str(repo), 'ls-tree', '-rz', '--name-only', FROZEN, '--', str(src)])
    for path in [Path(p.decode()) for p in roster.split(b'\0') if p]:
        if path not in exceptions:
            need((repo / path).read_text() == old(path), 'unchanged runtime dependency: ' + str(path))
    for path in inherited:
        need((repo / path).read_text() == old(path), 'unchanged inherited proof: ' + str(path))

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
    writer = module(repo, PREVIOUS, 'settlement_writer_check')
    previous = module(repo, writer.PREVIOUS, 'settlement_enrollment_check')
    ancestor = module(repo, previous.PREVIOUS, 'settlement_unknown_check')
    legacy = module(repo, LEGACY, 'enrollment_legacy_check')
    base = module(repo, legacy.BASE, 'enrollment_process_recorder')
    policy = module(repo, legacy.POLICY, 'enrollment_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    inherited = set(legacy.SOURCES) | set(ancestor.NEW) | set(previous.NEW) | set(writer.NEW) | set(REUSED)
    authenticate(repo, inherited)
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | set(writer.CPU_INPUTS) | set(legacy.PINS) | {LEGACY, CPU_PARSER, PREVIOUS, writer.PREVIOUS, previous.PREVIOUS, Path(__file__).resolve().relative_to(repo),
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
        cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None), ('writer-regression', None)]
        for name, mutation in cases:
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    need(digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                for path in set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY}) | set(previous.POLICY_NEW) | set(writer.POLICY_NEW) | set(POLICY_NEW):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3), *previous.PROJECTIONS, *writer.PROJECTIONS, *PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                command = legacy.command(verus, stage / (writer.ROOT if name == 'writer-regression' else RAW), mutation)
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
                                    'success': True, 'verified': 845 if name == 'writer-regression' else 306,
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
        status, stdout, _ = base.run_owned(command, 600, output / name, env)
        need(status == 0, 'CPU check: ' + name)
        if name == 'test':
            parse = module(repo, CPU_PARSER, 'settlement_cpu_parser').test_results
            need(parse(stdout) == [(956, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'exact CPU test inventory')
        print(name, 'passed', flush=True)
    after = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
