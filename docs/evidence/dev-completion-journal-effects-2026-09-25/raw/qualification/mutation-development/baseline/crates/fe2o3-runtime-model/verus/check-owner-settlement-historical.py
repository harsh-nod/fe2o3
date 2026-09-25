#!/usr/bin/env python3
"""Record independently executed settlement correspondence and custody proofs.

Not physical-capacity, constructor, native-admission, or performance qualification.
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
ROOT = CRATE / 'verus/context_owner_settlement_historical_v1.rs'
BRIDGE = CRATE / 'verus/context_owner_settlement_correspondence_v1.rs'
HISTORY = CRATE / 'verus/context_owner_settlement_historical_bodies_v1.rs'
WITNESS = CRATE / 'verus/context_owner_settlement_historical_witnesses_v1.rs'
PREVIOUS = CRATE / 'verus/check-owner-settlement.py'
FROZEN = '4700057ca993062dc58cdfbb15008c00f25cc65b'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-settlement-2026-09-22/records/source.json')
ORIGINALS = [CRATE / ('verus/context_version_journal_settlement' + suffix + '_v1.rs')
             for suffix in ('', '_commit', '_custody')]
NEW = [ROOT, BRIDGE, HISTORY, WITNESS]
POLICY_NEW = [BRIDGE, HISTORY, WITNESS]
HISTORICAL_VERIFIED = 916
EXTRA_MUTATIONS = [
    ('skip_actual', BRIDGE, 'fn owner_settlement_historical_exec_v1(', 'production', 'owner_settlement_historical_exec_v1',
     'let result = if success {\n'
     '        actual.settle_success_observed_v1(writer, &ContextWriterSuccessEvidenceV1 { writer: evidence }, free_storage, member_free_storage)\n'
     '    } else {\n'
     '        actual.settle_no_effect_observed_v1(writer, &ContextWriterNoEffectEvidenceV1 { writer: evidence }, free_storage, member_free_storage)\n'
     '    };', 'let result: Result<(), ReadErrorV1> = Ok(());', 'execution'),
    ('skip_historical', BRIDGE, 'fn owner_settlement_historical_exec_v1(', 'production', 'owner_settlement_historical_exec_v1',
     'let model_result = logical::settlement_exec_v1(&mut model.stable.journal, model_writer, model_evidence,\n'
     '        free_storage, member_free_storage, success);',
     'let model_result: Result<(), logical::ReadErrorV1> = Ok(());', 'execution'),
    ('historical_outcome', BRIDGE, 'fn owner_settlement_historical_exec_v1(', 'production', 'owner_settlement_historical_exec_v1',
     'free_storage, member_free_storage, success);', 'free_storage, member_free_storage, !success);', 'execution'),
    ('historical_capacity', BRIDGE, 'fn owner_settlement_historical_exec_v1(', 'production', 'owner_settlement_historical_exec_v1',
     'free_storage, member_free_storage, success);', 'free_storage, usize::MAX, success);', 'execution'),
    ('resolved_status', BRIDGE, 'spec fn owner_settlement_status_transition_v1(', 'production', 'owner_settlement_preservation_v1',
     'if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect }',
     'if success { logical::ProducerStatusV1::NoEffect } else { logical::ProducerStatusV1::Success }', 'execution'),
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


def mutations(previous):
    return [(*m[:3], 'production', *m[4:]) for m in previous.MUTATIONS] + EXTRA_MUTATIONS


def source_roster(repo, inherited):
    expected = {p for p in inherited if p.is_relative_to(CRATE / 'src')}
    paths = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no runtime source symlinks')
    need({p.relative_to(repo) for p in paths if p.is_file()} == expected, 'closed runtime source roster')
    need({p.name for p in (repo / CRATE).iterdir()} == {'Cargo.toml', 'src', 'verus'}, 'closed Cargo discovery roster')


def authenticate(repo, inherited, writer, git_repo=None):
    def old(path):
        return subprocess.check_output(['git', '-C', str(git_repo or repo), 'show', FROZEN + ':' + str(path)]).decode()
    for path in inherited:
        need((repo / path).read_bytes() == old(path).encode(), 'unchanged inherited input: ' + str(path))
    source_roster(repo, inherited)
    settlement, commit, custody = [old(path) for path in ORIGINALS]
    expected = '// Exact historical settlement declarations in the importing owner universe.\nuse super::*;\n\nverus! {\n\n'
    expected += settlement[settlement.index('pub open spec fn settlement_scratch_scan_v1'):
                           settlement.index('pub open spec fn unknown_decision_v1')].rstrip() + '\n\n'
    expected += commit.split('verus! {\n\n', 1)[1].rsplit('\n}', 1)[0].rstrip() + '\n\n'
    expected += custody[custody.index('pub proof fn settlement_cursor_canonical_v1'):
                        custody.index('pub open spec fn settlement_issued_execution_v1')].rstrip() + '\n\n'
    expected += settlement[settlement.index('pub proof fn settle_preserves_issued_custody_v1'):
                           settlement.index('pub proof fn unknown_preserves_issued_custody_v1')].rstrip() + '\n\n}\n'
    need((repo / HISTORY).read_bytes() == expected.encode(), 'exact 37 historical declarations and envelope')
    expected = old(writer.ROOT).replace('mod production {',
        '#[path = "context_owner_settlement_historical_bodies_v1.rs"]\n'
        'mod owner_settlement_historical_bodies;\nuse owner_settlement_historical_bodies::*;\n\nmod production {')
    expected = expected.replace('include!("context_owner_writer_execution_v1.rs");',
                                'include!("context_owner_settlement_execution_v1.rs");')
    expected = expected.replace('    include!("context_owner_writer_witnesses_v1.rs");',
        '    include!("context_owner_writer_witnesses_v1.rs");\n'
        '    include!("context_owner_settlement_correspondence_v1.rs");\n'
        '    include!("context_owner_settlement_historical_witnesses_v1.rs");')
    need((repo / ROOT).read_bytes() == expected.encode(), 'exact historical root additions')


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
    settlement = module(repo, PREVIOUS, 'historical_settlement_raw_check')
    writer = module(repo, settlement.PREVIOUS, 'historical_settlement_writer_check')
    previous = module(repo, writer.PREVIOUS, 'historical_settlement_enrollment_check')
    ancestor = module(repo, previous.PREVIOUS, 'historical_settlement_unknown_check')
    legacy = module(repo, settlement.LEGACY, 'historical_settlement_legacy_check')
    base = module(repo, legacy.BASE, 'historical_settlement_process_recorder')
    policy = module(repo, legacy.POLICY, 'historical_settlement_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    inherited = {Path(p) for p in json.loads((repo / FROZEN_INPUTS).read_text())['inputs']}
    inherited.update([FROZEN_INPUTS, *ORIGINALS])
    authenticate(repo, inherited, writer)
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | {Path(__file__).resolve().relative_to(repo)}
    MUTATIONS = mutations(settlement)
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
        cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None), ('raw-regression', None)]
        for name, mutation in cases:
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    need(digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                for path in set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY}) | set(previous.POLICY_NEW) | set(writer.POLICY_NEW) | set(settlement.POLICY_NEW) | set(POLICY_NEW):
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3), *previous.PROJECTIONS, *writer.PROJECTIONS, *settlement.PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                command = legacy.command(verus, stage / (settlement.RAW if name == 'raw-regression' else ROOT), mutation)
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
                                    'success': True, 'verified': 306 if name == 'raw-regression' else HISTORICAL_VERIFIED,
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
            parse = module(repo, settlement.CPU_PARSER, 'settlement_cpu_parser').test_results
            need(parse(stdout) == [(956, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'exact CPU test inventory')
        print(name, 'passed', flush=True)
    after = {str(p): digest((repo / p).read_bytes()) for p in sorted(inputs)}
    source_roster(repo, inherited)
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
