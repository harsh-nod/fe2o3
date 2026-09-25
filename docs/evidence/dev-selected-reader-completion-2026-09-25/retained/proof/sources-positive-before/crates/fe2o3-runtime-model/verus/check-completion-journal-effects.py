#!/usr/bin/env python3
"""Qualify shared completion control and journal effects, not Context refinement."""
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
C = Path('crates/fe2o3-runtime/src/context')
CHECK = V / 'check-completion-journal-effects.py'
TEST = V / 'test-completion-journal-effects.py'
PREVIOUS = V / 'check-completion-settlement-control.py'
PREVIOUS_SHA = '0f7f2565d9e5c1461f67719ea9a82302a56ad6b872c44bc9b2edd1e6cfd50eb9'
OWNER_BASELINE = 'decfecb132592f1f370649a0c8476a35ef8bf1d3'
CONTROL_BASELINE = '70fe5a46bb02a046eed0ec1f53666a255b76c464'
IMPLEMENTATION = '024f2f78fbacdb1ccfb5169bb9bb316f203fdd38'
ROOT = V / 'context_completion_journal_paired_v1.rs'
EFFECTS = V / 'context_completion_journal_effects_v1.rs'
WITNESSES = V / 'context_completion_journal_witnesses_v1.rs'
CONSTRUCTOR = V / 'context_owner_constructor_witnesses_v1.rs'
PREFIX = C / 'completion_journal_prefix_body.rs'
BODY = C / 'completion_settlement_body.rs'
SETTLEMENT = C / 'versions/settlement_bodies.rs'
ADDITIONS = {ROOT, EFFECTS, WITNESSES, PREFIX, SETTLEMENT, C.parent / 'context/versions.rs',
             C / 'versions/producer_readers.rs', C / 'versions/submissions.rs',
             C / 'tests/producer_launch_tests.rs', CHECK, TEST}
VERIFIED = 1282
CONTROL_VERIFIED = 8
CONTROL_SECONDS = 120


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
    need(sha(data) == PREVIOUS_SHA, 'authenticate controller before import')
    control = types.ModuleType('completion_effect_control')
    control.__file__ = str(path)
    sys.modules[control.__name__] = control
    exec(compile(data, str(path), 'exec'), control.__dict__)
    owner, old, lifecycle, deps = control.load(repo)
    _, _, _, old_paths, prior_paths = owner.previous(repo)
    owner_paths = prior_paths | {owner.WITNESS, owner.CHECK, owner.TEST}
    legacy = deps['legacy']
    control_paths = {control.CHECK, control.TEST, control.CONTEXT, control.BODY, control.ROOT,
                     control.PREVIOUS, legacy.CLOSURE, legacy.MANIFEST}
    control_paths |= {Path(m.__file__).relative_to(repo) for m in (owner, old, lifecycle, *deps.values())}
    paths = owner_paths | control_paths | ADDITIONS
    need(len(owner_paths) == 462 and len(control_paths) == 23 and len(paths) == 477,
         'closed inherited and candidate inventories')
    return control, owner, old, lifecycle, deps, old_paths, prior_paths, owner_paths, control_paths, paths


def historical(repo, loaded, parent):
    control, owner, old, lifecycle, deps, old_paths, prior_paths, owner_paths, control_paths, _ = loaded
    prior = {p: old.git(repo, 'show', owner.BASELINE + ':' + str(p)) for p in prior_paths}
    owner_data = {p: old.git(repo, 'show', OWNER_BASELINE + ':' + str(p)) for p in owner_paths}
    owner.authenticate_baseline(repo, old, lifecycle, deps, old_paths, prior, parent)
    owner.check_delta(owner_data, prior)
    owner.contracts(owner_data, prior, old, lifecycle, deps)
    with tempfile.TemporaryDirectory(prefix='completion-effect-history-', dir=parent) as temporary:
        stage = Path(temporary)
        owner.materialize(stage, owner_data)
        owner.scan(stage, deps)
    control_data = {p: old.git(repo, 'show', CONTROL_BASELINE + ':' + str(p)) for p in control_paths}
    control.source_gate(control_data, owner_data[control.CONTEXT])
    control.tool_gate(control_data, deps['legacy'])
    need({p for p in owner_paths & control_paths if owner_data[p] != control_data[p]}
         == {control.CONTEXT}, 'only historical Context input differs')
    return owner_data, control_data


def source_gate(candidate, implementation, owner_data, control_data, loaded):
    control, owner, _, lifecycle, deps, _, _, owner_paths, control_paths, paths = loaded
    need(set(candidate) == paths and set(implementation) == paths - {CHECK, TEST}, 'exact source membership')
    for path in implementation:
        expected = implementation[path]
        if path == CONSTRUCTOR:
            expected = once(expected.decode(), '''fn owner_constructor_unknown_disposal_witness_v1() -> (result: bool)
    ensures result,
{
''', '''fn owner_constructor_unknown_disposal_witness_v1() -> (result: bool)
    ensures result,
{
    hide(logical::producer_invariant_v1);
''').encode()
        need(candidate[path] == expected, 'reviewed implementation input: ' + str(path))
    need({p for p in owner_paths if candidate[p] != owner_data[p]} == {control.CONTEXT, CONSTRUCTOR},
         'only explicit inherited deltas')
    need({p for p in control_paths if candidate[p] != control_data[p]} == {BODY}, 'only nested control-body delta')
    name = 'owner_constructor_unknown_disposal_witness_v1'
    need(lifecycle.contract(candidate[CONSTRUCTOR].decode(), name, deps['policy'])
         == lifecycle.contract(owner_data[CONSTRUCTOR].decode(), name, deps['policy']), 'unchanged disposal contract')
    anchor = '    include!("context_owner_lifecycle_mixed_witnesses_v1.rs");\n'
    extension = '''    mod completion {
        use super::*;
        include!("context_completion_journal_effects_v1.rs");
        include!("context_completion_journal_witnesses_v1.rs");
    }
'''
    need(candidate[ROOT] == once(owner_data[owner.ROOT].decode(), anchor, anchor + extension).encode(),
         'exact inherited root plus completion child')
    for name in ('completion_mixed_prefix_witness_v1', 'completion_writer_only_witness_v1'):
        need(not re.search(r'\brequires\b', lifecycle.contract(candidate[WITNESSES].decode(), name, deps['policy'])),
             'constructor witness without admission premise')
    need(flat_control(candidate) == control_data[BODY].decode(), 'exact flattened historical control body')


def flat_control(data):
    prefix = data[PREFIX].decode()
    start, end = '        $syntax!({\n', '        })\n    };\n}\n'
    need(prefix.count(start) == 1 and prefix.endswith(end), 'closed prefix expression')
    core = prefix.split(start)[1][:-len(end)]
    core = core.replace('            $($inputs_done)*\n', '')
    pattern = (r'            let result = (\$context\.settle_submission_writer_v1\([^\n;]+\));\n'
               r'            \$\(\$writer_done\)\*\n            match result \{')
    core = re.sub(pattern, r'            match \1 {', core)
    body = once(data[BODY].decode(), 'include!("completion_journal_prefix_body.rs");\n\n', '')
    return once(body, '            completion_journal_prefix_body!($syntax, $context, $submission, $outcome, [], []);\n', core)


def control_mutations(candidate, historical_body, control):
    prefix, body = candidate[PREFIX].decode(), candidate[BODY].decode()
    blocks = ['''            match $context.''' + call + ''' {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
''' for call in control.CALLS[:3]]
    inputs = blocks[0] + '            $($inputs_done)*\n'
    writer = '''            let result = $context.''' + control.CALLS[1] + ''';
            $($writer_done)*
            match result {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
'''
    tail = '            $context.' + control.CALLS[3] + '\n'
    cases = {'swap-0': {PREFIX: once(prefix, inputs + writer, writer + inputs)},
             'swap-1': {PREFIX: once(prefix, writer, blocks[2] + writer), BODY: once(body, blocks[2], '')},
             'early-publication': {BODY: once(body, blocks[2] + tail,
                 '            let result = $context.' + control.CALLS[3] + ';\n' + blocks[2] + '            result\n')}}
    for i, (path, block) in enumerate(((PREFIX, inputs), (PREFIX, writer), (BODY, blocks[2]))):
        text = candidate[path].decode()
        for name, after in (('omit', ''), ('duplicate', block + block),
                            ('continue-error', block.replace('Err(error) => return Err(error)', 'Err(_) => ()'))):
            cases[f'{name}-{i}'] = {path: once(text, block, after)}
    for i, call in enumerate(control.CALLS):
        path = PREFIX if i < 2 else BODY
        cases[f'wrong-submission-{i}'] = {path: once(candidate[path].decode(), call, call.replace('$submission', '0'))}
    cases['wrong-outcome'] = {PREFIX: once(prefix, control.CALLS[1], control.CALLS[1].replace('$outcome', '0'))}
    cases['wrong-status'] = {BODY: once(body, control.CALLS[3], control.CALLS[3].replace('$status', '0'))}
    cases['omit-publication'] = {BODY: once(body, tail, '            Ok($status)\n')}
    cases['discard-result'] = {BODY: once(body, tail, '            match $context.' + control.CALLS[3]
        + ' { Ok(_) => Ok($status), Err(error) => Err(error) }\n')}
    cases['swallow-publication-error'] = {BODY: once(body, tail, '            match $context.' + control.CALLS[3]
        + ' { Ok(value) => Ok(value), Err(_) => Ok($status) }\n')}
    cases['duplicate-publication'] = {BODY: once(body, tail, '            let _ = $context.' + control.CALLS[3] + ';\n' + tail)}
    previous = control.mutations(historical_body.decode())
    need(list(cases) == list(previous), 'all historical mutation identities and ordering')
    result = {}
    for name, edits in cases.items():
        payload = {p: text.encode() for p, text in edits.items()}
        need(flat_control(candidate | payload) == previous[name], 'same historical defect: ' + name)
        result['control-' + name] = (payload, '', 'SettlementProbeV1::settle_v1')
    return result


def effect_mutations(candidate):
    body, prefix = candidate[SETTLEMENT].decode(), candidate[PREFIX].decode()
    stable = '''            match $context.release_submission_readers_v1($id) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
'''
    input_fn = 'CompletionJournalPairV1::release_submission_inputs_v1'
    writer_fn = 'completion_writer_paired_exec_v1'
    rows = [
        ('skip-stable', SETTLEMENT, stable, '', input_fn),
        ('continue-stable-error', SETTLEMENT, stable, stable.replace('Err(error) => return Err(error)', 'Err(_) => ()'), input_fn),
        ('skip-producer', SETTLEMENT, 'if !$producer {', 'if true {', input_fn),
        ('invert-producer-presence', SETTLEMENT, 'if !$producer {', 'if $producer {', input_fn),
        ('swallow-producer-error', SETTLEMENT, '            result\n', '            Ok(())\n', input_fn),
        ('swap-success-no-effect', SETTLEMENT, '''                SubmissionWriterOutcomeV1::Success => ($journal).$success(
                    $writer, &ContextWriterSuccessEvidenceV1 { writer: $writer } $($storage)*),
                SubmissionWriterOutcomeV1::NoEffect => ($journal).$no_effect(
                    $writer, &ContextWriterNoEffectEvidenceV1 { writer: $writer } $($storage)*),''',
         '''                SubmissionWriterOutcomeV1::Success => ($journal).$no_effect(
                    $writer, &ContextWriterNoEffectEvidenceV1 { writer: $writer } $($storage)*),
                SubmissionWriterOutcomeV1::NoEffect => ($journal).$success(
                    $writer, &ContextWriterSuccessEvidenceV1 { writer: $writer } $($storage)*),''', writer_fn),
        ('settle-unknown', SETTLEMENT, 'SubmissionWriterOutcomeV1::Unknown => ($journal).mark_unknown($writer)',
         'SubmissionWriterOutcomeV1::Unknown => ($journal).$success(\n'
         '                    $writer, &ContextWriterSuccessEvidenceV1 { writer: $writer } $($storage)*)', writer_fn),
        ('discard-writer-error', PREFIX, '''            match result {
                Ok(()) => (),
                Err(error) => return Err(error),
            }''', '''            match result {
                Ok(()) => (),
                Err(_) => (),
            }''', 'CompletionJournalPairV1::complete_journal_prefix_v1'),
    ]
    result = {}
    for name, path, before, after, function in rows:
        text = body if path == SETTLEMENT else prefix
        result['effect-' + name] = ({path: once(text, before, after).encode()}, 'production::completion', function)
    need(len(result) == 8, 'closed effect mutation roster')
    return result


def scan(stage, deps):
    projections = {
        EFFECTS: ['include!("../../fe2o3-runtime/src/context/versions/settlement_bodies.rs");\n',
                  'include!("../../fe2o3-runtime/src/context/completion_journal_prefix_body.rs");\n'],
        BODY: ['include!("completion_journal_prefix_body.rs");\n'],
        PREFIX: [], SETTLEMENT: [], WITNESSES: [], CONSTRUCTOR: [],
    }
    for path, includes in projections.items():
        text = (stage / path).read_text()
        for include in includes:
            text = once(text, include, '')
        count = 3 if path == SETTLEMENT else 1 if path in (BODY, PREFIX) else 0
        need(text.count('macro_rules!') == count, 'closed shared macro roster')
        target = stage / ('audited-completion-' + path.name)
        target.write_text(text.replace('macro_rules!', ''))
        deps['policy'].scan(target)


def proof_command(loaded, verus, stage, name, changes):
    control, owner, old, _, deps, *_ = loaded
    root = control.ROOT if name.startswith('control-') else owner.ROOT if name == 'owner-regression' else ROOT
    change = None
    if name in changes:
        _, module, function = changes[name]
        change = (name, '', '', '', module, function)
    command = old.command(deps['legacy'], verus, stage / root, change)
    if name.startswith('control-'):
        command[4] = str(CONTROL_SECONDS)
    return command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verus', type=Path)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--resume', action='store_true')
    modes.add_argument('--verify', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    repo, output = Path(__file__).resolve().parents[3], args.output.absolute()
    need(output.resolve() == output, 'ordinary output location')
    loaded = load(repo)
    control, owner, old, lifecycle, deps, *_, paths = loaded
    base, scalar, legacy = (deps[key] for key in ('base', 'scalar', 'legacy'))
    parent = None if args.verify else output.parent
    owner_data, control_data = historical(repo, loaded, parent)
    implementation = {p: old.git(repo, 'show', IMPLEMENTATION + ':' + str(p)) for p in paths - {CHECK, TEST}}
    candidate = {p: old.ordinary(repo, p) for p in paths}
    source_gate(candidate, implementation, owner_data, control_data, loaded)
    changes = control_mutations(candidate, control_data[BODY], control) | effect_mutations(candidate)
    names = ['checker-tests', 'closure-before', 'effects-positive-before', 'control-positive-before',
             *changes, 'effects-positive-after', 'control-positive-after', 'owner-regression', 'closure-after']
    need(args.stop_after is None or args.stop_after in names, 'known stop checkpoint')
    inputs = {str(p): sha(data) for p, data in sorted(candidate.items())}
    head = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
    recorded = base.unique_json(old.ordinary(output, Path('source.json')).decode()) if args.verify or args.resume else None
    if recorded is not None:
        need(set(recorded) == {'commit', 'inputs', 'recorded_repo', 'recorded_output', 'recorded_verus'}, 'closed replay identity')
        need(type(recorded['commit']) is str and re.fullmatch('[0-9a-f]{40}', recorded['commit']), 'source commit')
        for key in ('recorded_repo', 'recorded_output', 'recorded_verus'):
            need(type(recorded[key]) is str and Path(recorded[key]).is_absolute()
                 and '..' not in Path(recorded[key]).parts, 'absolute recorded location')
        old.git(repo, 'merge-base', '--is-ancestor', recorded['commit'], head)
        commit, verus = recorded['commit'], Path(recorded['recorded_verus'])
        if args.resume:
            need(recorded['recorded_repo'] == str(repo) and recorded['recorded_output'] == str(output), 'resume original locations')
    else:
        need(args.verus is not None, 'pinned Verus required')
        commit, verus = head, args.verus.resolve(strict=True)
    need(verus.is_absolute() and verus.name == 'verus', 'absolute pinned verifier')
    for path, data in candidate.items():
        need(data == old.git(repo, 'show', commit + ':' + str(path)), 'signed source input: ' + str(path))
    with tempfile.TemporaryDirectory(prefix='completion-effect-signer-', dir=parent) as temporary:
        signer = Path(temporary) / 'allowed-signers'
        signer.write_text(old.SIGNER)
        for revision in (old.HISTORICAL, old.BASELINE, owner.BASELINE, OWNER_BASELINE, CONTROL_BASELINE, IMPLEMENTATION, commit):
            old.git(repo, '-c', 'gpg.ssh.allowedSignersFile=' + str(signer), 'verify-commit', revision)
    recorded_repo = Path(recorded['recorded_repo']) if recorded else repo
    recorded_output = Path(recorded['recorded_output']) if recorded else output
    identity = dict(commit=commit, inputs=inputs, recorded_repo=str(recorded_repo),
                    recorded_output=str(recorded_output), recorded_verus=str(verus))
    tests = ['/usr/bin/python3', '-I', '-B', str(recorded_repo / TEST)]
    closure = ['/bin/sh', str(recorded_repo / legacy.CLOSURE), str(verus.parent), str(recorded_repo / legacy.MANIFEST)]
    env = dict(HOME='/home/harsh', PATH='/home/harsh/.cargo/bin:/usr/bin:/bin',
               CARGO_HOME='/home/harsh/.cargo', RUSTUP_HOME='/home/harsh/.rustup',
               VERUS_Z3_PATH=str(verus.parent / 'z3'), TMPDIR=str(output))
    active = {}

    def validate(name, row, stdout, stderr):
        budget = 120 if name == 'checker-tests' or name.startswith('closure-') or name.startswith('control-') else (
            old.SCOPED_SECONDS if name in changes else old.WHOLE_SECONDS)
        owner.receipt_budget(row, budget)
        if name == 'checker-tests':
            need(row['command'] == tests and type(row['status']) is int and row['status'] == 0 and not stderr
                 and stdout == 'PASS: completion journal effects calibration (8 groups)\n', 'checker calibration')
            return None
        if name.startswith('closure-'):
            need(row['command'] == closure and type(row['status']) is int and row['status'] == 0 and not stderr
                 and stdout == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'tool closure')
            return None
        root = control.ROOT if name.startswith('control-') else owner.ROOT if name == 'owner-regression' else ROOT
        recorded_root = Path(row['command'][-1])
        need(str(recorded_root).endswith('/' + str(root)), 'exact proof root')
        stage = Path(str(recorded_root)[:-len(str(root)) - 1])
        need(stage.is_absolute() and stage.parent == recorded_output and stage.name.startswith('sources-'), 'owned stage')
        need(row['command'] == proof_command(loaded, verus, stage, name, changes), 'exact proof command and selection')
        if stage in active:
            old.stage_gate(stage, active[stage], row, base)
        else:
            need(args.verify or args.resume, 'live source continuity required')
        observed = legacy.normalized(base, stdout, stderr, stage)
        scalar.check_verifier(observed['verus'])
        if name in changes:
            lifecycle.check_negative(scalar, name, row['status'], observed)
        else:
            count = CONTROL_VERIFIED if name.startswith('control-') else owner.VERIFIED if name == 'owner-regression' else VERIFIED
            need(type(row['status']) is int and row['status'] == 0 and not observed['diagnostics'], 'clean positive')
            need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                'success': True, 'verified': count, 'errors': 0, 'is-verifying-entire-crate': True}), 'whole-root count')
        return observed

    def unchanged():
        need(old.git(repo, 'rev-parse', 'HEAD').decode().strip() == head
             and candidate == {p: old.ordinary(repo, p) for p in paths}, 'unchanged signed inputs')

    lock = None if args.verify else scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    campaign = scalar.Campaign(output, identity, names, base, legacy.same, validate, args.verify or args.resume)
    if args.verify:
        need(len(campaign.accepted) == len(names), 'complete campaign')
        need(legacy.same(base.unique_json(old.ordinary(output, Path('inputs-after.json')).decode()), inputs), 'closing inputs')
        need(legacy.same(base.unique_json(old.ordinary(output, Path('results.json')).decode()), campaign.proofs), 'proof summary')
        with tempfile.TemporaryDirectory(prefix='completion-effect-replay-') as temporary:
            stage = Path(temporary)
            for change in [None, *changes.values()]:
                owner.materialize(stage, candidate | (change[0] if change else {}))
                scan(stage, deps)
        for family in ('effects', 'control'):
            need(legacy.same(campaign.proofs[family + '-positive-before'], campaign.proofs[family + '-positive-after']), 'positive brackets')
        unchanged()
        print('PASS: completion journal effects replay')
        return
    for name in names:
        unchanged()
        if name in campaign.rows:
            print(name, 'replayed', flush=True)
        elif name == 'checker-tests' or name.startswith('closure-'):
            campaign.run(name, tests if name == 'checker-tests' else closure, 120, env)
        else:
            stage = Path(tempfile.mkdtemp(prefix='sources-', dir=output))
            data = owner_data if name == 'owner-regression' else candidate
            change = changes.get(name)
            owner.materialize(stage, data | (change[0] if change else {}))
            if name != 'owner-regression':
                scan(stage, deps)
            active[stage] = old.stage_map(stage)
            command = proof_command(loaded, verus, stage, name, changes)
            campaign.run(name, command, int(command[4]) + 10, env)
            old.stage_gate(stage, active[stage], campaign.rows[name][0], base)
            shutil.rmtree(stage)
            need(not os.path.lexists(stage), 'owned stage absence')
            del active[stage]
        unchanged()
        print(name, 'PASS', flush=True)
        if name == args.stop_after and name != names[-1]:
            print('CHECKPOINT: ' + name + '; qualification incomplete', flush=True)
            return
    for family in ('effects', 'control'):
        need(legacy.same(campaign.proofs[family + '-positive-before'], campaign.proofs[family + '-positive-after']), 'positive brackets')
    (output / 'inputs-after.json').write_text(json.dumps(inputs, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')
    lock.close()
    print('PASS: completion journal effects campaign')


if __name__ == '__main__':
    main()
