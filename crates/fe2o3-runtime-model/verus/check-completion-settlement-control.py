#!/usr/bin/env python3
"""Qualify only the shared completion suffix's call order and Result forwarding."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import sys
import tempfile
import types

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')

V = Path('crates/fe2o3-runtime-model/verus')
CHECK = V / 'check-completion-settlement-control.py'
TEST = V / 'test-completion-settlement-control.py'
ROOT = V / 'context_completion_settlement_control_v1.rs'
CONTEXT = Path('crates/fe2o3-runtime/src/context.rs')
BODY = CONTEXT.parent / 'context/completion_settlement_body.rs'
PREVIOUS = V / 'check-owner-mixed-lifecycle.py'
PREVIOUS_SHA = 'ec0fb0c1ad835683321d8aad84dda11ad919c88c9542ff9db1ede84584531e59'
BASELINE = 'decfecb132592f1f370649a0c8476a35ef8bf1d3'
PINS = {BODY: 'c301af065bd86332164721cd7a43ee7c480e14e9aaaa275f8e295dcdff7f5277',
        ROOT: '3088ae2257b092df9afb1233ec906ee61e570573714c169f8a0b73c7e7a600a3'}
CALLS = ('release_submission_inputs_v1($submission)',
         'settle_submission_writer_v1($submission, $outcome)',
         'release_operation_dependencies_v1($submission)',
         'publish_submission_status_v1($submission, $status)')


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def once(text, before, after):
    need(text.count(before) == 1, 'unique replacement')
    return text.replace(before, after)


def load(repo):
    path = repo / PREVIOUS
    need(path.resolve() == path and path.is_file(), 'ordinary inherited controller')
    data = path.read_bytes()
    need(sha(data) == PREVIOUS_SHA, 'authenticate inherited controller before import')
    owner = types.ModuleType('completion_control_helpers')
    owner.__file__ = str(path)
    sys.modules[owner.__name__] = owner
    exec(compile(data, str(path), 'exec'), owner.__dict__)
    old, lifecycle, deps, _, _ = owner.previous(repo)
    return owner, old, lifecycle, deps


def source_gate(candidate, baseline):
    for path, pin in PINS.items():
        need(sha(candidate[path]) == pin, 'reviewed source: ' + str(path))
    text = baseline.decode()
    text = once(text, 'mod graph;\n', '''macro_rules! completion_settlement_rust_expr {
    ($body:expr) => {
        $body
    };
}
include!("context/completion_settlement_body.rs");

mod graph;
''')
    text = once(text, '    fn settle_terminal_submission_v1(\n',
        '    #[allow(clippy::question_mark)] // Explicit matches are shared with Verus.\n'
        '    fn settle_terminal_submission_v1(\n')
    text = once(text, '''        self.release_submission_inputs_v1(submission)?;
        self.settle_submission_writer_v1(submission, outcome)?;
        self.release_operation_dependencies_v1(submission)?;
        self.publish_submission_status_v1(submission, status)''', '''        completion_settlement_execution_body!(
            completion_settlement_rust_expr,
            self,
            submission,
            status,
            outcome
        )''')
    text = once(text, '    mod async_journal_tests;\n',
        '    mod async_journal_tests;\n    mod completion_settlement_tests;\n')
    need(candidate[CONTEXT] == text.encode(), 'exact production delta and unchanged prechecks')


def tool_gate(candidate, legacy):
    for path in (legacy.CLOSURE, legacy.MANIFEST):
        need(sha(candidate[path]) == legacy.PINS[path], 'frozen tool authority: ' + str(path))


def mutations(body):
    blocks = ['''            match $context.''' + call + ''' {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
''' for call in CALLS[:3]]
    tail = '            $context.' + CALLS[3] + '\n'
    cases = {}
    for i in range(2):
        cases[f'swap-{i}'] = once(body, blocks[i] + blocks[i + 1], blocks[i + 1] + blocks[i])
    cases['early-publication'] = once(body, blocks[2] + tail,
        '            let result = $context.' + CALLS[3] + ';\n' + blocks[2] + '            result\n')
    for i, block in enumerate(blocks):
        cases[f'omit-{i}'] = once(body, block, '')
        cases[f'duplicate-{i}'] = once(body, block, block + block)
        cases[f'continue-error-{i}'] = once(body, block, block.replace('Err(error) => return Err(error)', 'Err(_) => ()'))
    for i, call in enumerate(CALLS):
        cases[f'wrong-submission-{i}'] = once(body, call, call.replace('$submission', '0'))
    cases['wrong-outcome'] = once(body, CALLS[1], CALLS[1].replace('$outcome', '0'))
    cases['wrong-status'] = once(body, CALLS[3], CALLS[3].replace('$status', '0'))
    cases['omit-publication'] = once(body, tail, '            Ok($status)\n')
    cases['discard-result'] = once(body, tail, '            match $context.' + CALLS[3]
        + ' { Ok(_) => Ok($status), Err(error) => Err(error) }\n')
    cases['swallow-publication-error'] = once(body, tail, '            match $context.' + CALLS[3]
        + ' { Ok(value) => Ok(value), Err(_) => Ok($status) }\n')
    cases['duplicate-publication'] = once(body, tail, '            let _ = $context.' + CALLS[3] + ';\n' + tail)
    need(len(cases) == 22 and len(set(cases.values())) == 22 and body not in cases.values(), 'distinct mutation roster')
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--verus', type=Path)
    parser.add_argument('--verify', action='store_true')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    output = args.output.absolute()
    need(output.resolve() == output, 'ordinary output location')
    owner, old, lifecycle, deps = load(repo)
    base, scalar, legacy = (deps[key] for key in ('base', 'scalar', 'legacy'))
    paths = {CHECK, TEST, CONTEXT, BODY, ROOT, PREVIOUS, legacy.CLOSURE, legacy.MANIFEST}
    paths |= {Path(m.__file__).relative_to(repo) for m in (owner, old, lifecycle, *deps.values())}
    candidate = {p: old.ordinary(repo, p) for p in paths}
    tool_gate(candidate, legacy)
    source_gate(candidate, old.git(repo, 'show', BASELINE + ':' + str(CONTEXT)))
    inputs = {str(p): sha(data) for p, data in sorted(candidate.items())}
    head = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
    recorded = base.unique_json(old.ordinary(output, Path('source.json')).decode()) if args.verify else None
    if recorded:
        need(set(recorded) == {'commit', 'inputs', 'recorded_repo', 'recorded_output', 'recorded_verus'}, 'closed replay identity')
        need(type(recorded['commit']) is str and re.fullmatch('[0-9a-f]{40}', recorded['commit']), 'exact source commit')
        for key in ('recorded_repo', 'recorded_output', 'recorded_verus'):
            need(type(recorded[key]) is str and Path(recorded[key]).is_absolute()
                 and '..' not in Path(recorded[key]).parts, 'canonical recorded location')
        old.git(repo, 'merge-base', '--is-ancestor', recorded['commit'], head)
        commit, verus = recorded['commit'], Path(recorded['recorded_verus'])
    else:
        need(args.verus is not None, 'pinned Verus required')
        commit, verus = head, args.verus.resolve(strict=True)
    need(verus.is_absolute() and verus.name == 'verus', 'absolute pinned verifier')
    for path, data in candidate.items():
        need(data == old.git(repo, 'show', commit + ':' + str(path)), 'signed source input: ' + str(path))
    with tempfile.TemporaryDirectory(prefix='completion-control-signer-') as temporary:
        signer = Path(temporary) / 'allowed-signers'
        signer.write_text(old.SIGNER)
        for revision in (BASELINE, commit):
            old.git(repo, '-c', 'gpg.ssh.allowedSignersFile=' + str(signer), 'verify-commit', revision)
    recorded_repo = Path(recorded['recorded_repo']) if recorded else repo
    recorded_output = Path(recorded['recorded_output']) if recorded else output
    need(recorded_repo.is_absolute() and recorded_output.is_absolute(), 'absolute recorded locations')
    identity = dict(commit=commit, inputs=inputs, recorded_repo=str(recorded_repo),
                    recorded_output=str(recorded_output), recorded_verus=str(verus))
    changes = mutations(candidate[BODY].decode())
    names = ['checker-tests', 'closure-before', 'positive-before', *changes, 'positive-after', 'closure-after']
    env = dict(HOME='/home/harsh', PATH='/home/harsh/.cargo/bin:/usr/bin:/bin',
               CARGO_HOME='/home/harsh/.cargo', RUSTUP_HOME='/home/harsh/.rustup',
               VERUS_Z3_PATH=str(verus.parent / 'z3'), TMPDIR=str(output))
    tests = ['/usr/bin/python3', '-I', '-B', str(recorded_repo / TEST)]
    closure = ['/bin/sh', str(recorded_repo / legacy.CLOSURE), str(verus.parent), str(recorded_repo / legacy.MANIFEST)]
    active = {}

    def command(stage, name):
        mutation = (name, '', '', '', 'SettlementProbeV1::settle_v1', '', '', 'logical') if name in changes else None
        result = legacy.command(verus, stage / ROOT, mutation)
        result[4] = '120'
        return result

    def validate(name, row, stdout, stderr):
        owner.receipt_budget(row, 120)
        if name == 'checker-tests':
            need(row['command'] == tests and type(row['status']) is int and row['status'] == 0
                 and stdout == 'PASS: completion settlement control calibration (4 groups)\n' and not stderr, 'checker calibration')
            return None
        if name.startswith('closure-'):
            need(row['command'] == closure and type(row['status']) is int and row['status'] == 0 and not stderr
                 and stdout == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned tool closure')
            return None
        root = Path(row['command'][-1])
        need(str(root).endswith('/' + str(ROOT)), 'proof root')
        stage = Path(str(root)[:-len(str(ROOT)) - 1])
        need(stage.is_absolute() and stage.parent == recorded_output and stage.name.startswith('sources-'), 'owned source stage')
        need(row['command'] == command(stage, name), 'exact proof command')
        if stage in active:
            old.stage_gate(stage, active[stage], row, base)
        else:
            need(args.verify, 'live source continuity required')
        observed = legacy.normalized(base, stdout, stderr, stage)
        scalar.check_verifier(observed['verus'])
        if name in changes:
            lifecycle.check_negative(scalar, name, row['status'], observed)
        else:
            need(type(row['status']) is int and row['status'] == 0 and not observed['diagnostics'], 'clean positive')
            need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                'success': True, 'verified': 8, 'errors': 0, 'is-verifying-entire-crate': True}), 'whole-root count')
        return observed

    def unchanged():
        need(old.git(repo, 'rev-parse', 'HEAD').decode().strip() == head
             and candidate == {p: old.ordinary(repo, p) for p in paths}, 'unchanged signed inputs')

    lock = None if args.verify else scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    campaign = scalar.Campaign(output, identity, names, base, legacy.same, validate, args.verify)
    if args.verify:
        need(len(campaign.accepted) == len(names), 'complete campaign')
        need(base.unique_json(old.ordinary(output, Path('inputs-after.json')).decode()) == inputs, 'closing source inputs')
        need(legacy.same(base.unique_json(old.ordinary(output, Path('results.json')).decode()), campaign.proofs), 'proof summary')
        unchanged()
        print('PASS: completion settlement control replay')
        return
    for name in names:
        unchanged()
        if name == 'checker-tests' or name.startswith('closure-'):
            campaign.run(name, tests if name == 'checker-tests' else closure, 120, env)
        else:
            stage = Path(tempfile.mkdtemp(prefix='sources-', dir=output))
            owner.materialize(stage, {BODY: changes.get(name, candidate[BODY].decode()).encode(), ROOT: candidate[ROOT]})
            active[stage] = old.stage_map(stage)
            campaign.run(name, command(stage, name), 130, env)
            old.stage_gate(stage, active[stage], campaign.rows[name][0], base)
            shutil.rmtree(stage)
            need(not os.path.lexists(stage), 'owned source stage absence')
            del active[stage]
        unchanged()
        print(name, 'PASS', flush=True)
    (output / 'inputs-after.json').write_text(json.dumps(inputs, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')
    lock.close()
    print('PASS: completion settlement control campaign')


if __name__ == '__main__':
    main()
