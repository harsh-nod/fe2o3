#!/usr/bin/env python3
"""Authenticate the constructor-origin mixed-reader lifecycle extension.

Historical checkers and source pins are replayed on their frozen inputs, never
repinned to accept this extension. This is not Context or native qualification.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import signal
import sys
import tempfile
import types
from pathlib import Path

sys.dont_write_bytecode = True
V = Path('crates/fe2o3-runtime-model/verus')
CHECK = V / 'check-owner-mixed-lifecycle.py'
TEST = V / 'test-owner-mixed-lifecycle.py'
PREVIOUS = V / 'check-mixed-acquire.py'
PREVIOUS_SHA = '6ab16853e5f3fa9630ab740b797750ee36c52bff7d93231b69b0d84ba786073f'
BASELINE = '3db455f03f6ff06a65d5603a59b96ddac225f503'
ROOT = V / 'context_owner_lifecycle_paired_v1.rs'
ALIAS = V / 'context_mixed_acquire_paired_v1.rs'
MODEL = V / 'context_owner_lifecycle_model_v1.rs'
BRIDGE = V / 'context_owner_lifecycle_correspondence_v1.rs'
DOMAIN = V / 'context_owner_lifecycle_domain_v1.rs'
READER = V / 'context_owner_lifecycle_reader_witnesses_v1.rs'
WITNESS = V / 'context_owner_lifecycle_mixed_witnesses_v1.rs'
MODIFIED = {ROOT, ALIAS, MODEL, BRIDGE, DOMAIN, READER,
            V / 'context_owner_lifecycle_history_v1.rs', V / 'context_owner_lifecycle_preservation_v1.rs'}
# Measured after review. This program never rewrites its own acceptance policy.
SOURCE_PINS = {
    V / 'context_mixed_acquire_paired_v1.rs': '04a6eb246b2d65085509eba4abfb5cd5cfcf412c783736495eca5dfdd5cc17d3',
    V / 'context_owner_lifecycle_correspondence_v1.rs': 'e399cd94ab2c3d951976fe567c85e5e7b40659c2092cc5fbab5524b32de1630d',
    V / 'context_owner_lifecycle_domain_v1.rs': '3fbafa2257fce54ac412be7ec6a87600af42749c39d582142586e41ba6732f4c',
    V / 'context_owner_lifecycle_history_v1.rs': 'a4931b9081e3e78d69b7a89b84ee0341026ad4a30ed4a3d1f376bd88c1d2738e',
    V / 'context_owner_lifecycle_mixed_witnesses_v1.rs': 'c9cccf205602a43fcca67346ddf39cbaa6f01db3f38de5b63d99595a5c8643a7',
    V / 'context_owner_lifecycle_model_v1.rs': '0aef6fa01d59c8ecf9d41f5ca8b7f7b3cb51e6327b2e87dd8558d0cdad747561',
    V / 'context_owner_lifecycle_paired_v1.rs': '07a28327cfe028b4f643622a2e76f8741aa842974b2b50578e919b97f11fe4f6',
    V / 'context_owner_lifecycle_preservation_v1.rs': '0d0a4d37d61d8c185d7360b160b2bc5c0c72d9412572631f3ea72c5aa8c20504',
    V / 'context_owner_lifecycle_reader_witnesses_v1.rs': '429a7f6d38bbd82d5282f2befa95e25451dee2c028ff14473dc8f19cdd011a53',
}
CONTRACT_PINS = {
    (V / 'context_owner_lifecycle_correspondence_v1.rs', 'lifecycle_mutation_correspondence_v1'): 'db418aedabe755a0a5fc9341c5dc81aa23dec050d41eb7ed66e59f6e9bf225ce',
    (V / 'context_owner_lifecycle_correspondence_v1.rs', 'lifecycle_mixed_answers_projection_v1'): '5e150358875d07d607a07488fccb5a687cfe303f69935703edcc9b0c0de15802',
    (V / 'context_owner_lifecycle_domain_v1.rs', 'lifecycle_mutation_domain_from_reached_v1'): '51f90d7293948a60a65981a7a936568f8088d8fb1611cb8025b287bd8cafc948',
    (V / 'context_owner_lifecycle_domain_v1.rs', 'lifecycle_mixed_domain_projection_v1'): '801fb002df532c80ba33ba0e3b88b6684b91426cb4229fe2ee798d594229e956',
    (V / 'context_owner_lifecycle_history_v1.rs', 'lifecycle_mixed_acquire_history_v1'): 'fe0f4c55cbbf368537ff4ac5b5d337993488d4a5101af17559db9a4c10e565db',
    (V / 'context_owner_lifecycle_preservation_v1.rs', 'lifecycle_mixed_acquire_frame_v1'): '291eccdc3410f04eced1326e45aaa90fafdde675560a3942cbecdf819fe21649',
    (V / 'context_owner_lifecycle_preservation_v1.rs', 'lifecycle_step_issued_v1'): 'e900553d57e0c22c41800314fe2ab2f161f3fbc458df3dd40fe73dc960526dbb',
    (V / 'context_owner_lifecycle_preservation_v1.rs', 'lifecycle_step_histories_v1'): '7b84eeb4d1fb0e65bf3c46b4c0e4de673f1dd2adf43901b091dd96b9e6e3c9a3',
    (V / 'context_owner_lifecycle_preservation_v1.rs', 'lifecycle_step_shape_v1'): 'c9f08da8c8d2e5038173062f1d30eac0164454584aa6880e7da1eca71afb2da2',
    (V / 'context_owner_lifecycle_reader_witnesses_v1.rs', 'lifecycle_reader_append_event_v1'): 'ccc2ce12b94c0272bd0bd66aa570ed1105be11bc4646e625230a37bbefa29eef',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_reused_prefix_v1'): '1f04ce6f5df7d61e15bb1d58cbdd44d87e3519966b16f13319235f3c7622a689',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_fixture_v1'): '3a3a7cb44b6f0dcaa4e9d8676015da88e78ba5537eea0a10ade6f601945c4052',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_requests_v1'): '09f81ab733a464ad287e61e7c230312bee446c87247ac545162bfc98474600fb',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_record_v1'): 'a227fa0bad37a5e942447c5c86205a7296e991faaa109d4059479fdf0d494ec5',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_acquired_fixture_v1'): '861b7f568fe5fadf6dc9bbf506f2a894e7a44d112aa8210c72f92da6d8a4ec39',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_trace_audit_v1'): 'bd607dce135c7cfefe558cfc9d5bfc45e4c40beb109c3a91d693e59a56dc7d2c',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_late_failure_witness_v1'): 'cb389b648afe8d768d30c67295bf0c033007d966d0cee3f46278424c43ed02e5',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_capacity_witness_v1'): '19fb432f97bf85028c556e44453dc9a5da13d99db5b3ae4fd4560ccea4e7ebb4',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_terminal_fixture_v1'): 'df2cf0ae67aa5d8c4bf1b1a19ea4a024701c7f24d3276c2ddd348d51367a0ef2',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_stable_release_v1'): '03e08160deab8a1a27c7e341491dfd681144f84ff7e8b604d352cac78c2dffff',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_settlement_witness_v1'): '0f2030aa0060bde8adeef48a693a83db044dcb4275e95313348368df7f0e5db1',
    (V / 'context_owner_lifecycle_mixed_witnesses_v1.rs', 'lifecycle_mixed_unknown_witness_v1'): 'c791af479d61151e482381febbd44826ce5411f5967759ed99a26a14290a2a2a',
}
CONTRACT_NAMES = {
    BRIDGE: ('lifecycle_mutation_correspondence_v1', 'lifecycle_mixed_answers_projection_v1'),
    DOMAIN: ('lifecycle_mutation_domain_from_reached_v1', 'lifecycle_mixed_domain_projection_v1'),
    V / 'context_owner_lifecycle_history_v1.rs': ('lifecycle_mixed_acquire_history_v1',),
    V / 'context_owner_lifecycle_preservation_v1.rs': ('lifecycle_mixed_acquire_frame_v1',
        'lifecycle_step_issued_v1', 'lifecycle_step_histories_v1', 'lifecycle_step_shape_v1'),
    READER: ('lifecycle_reader_append_event_v1',),
    WITNESS: ('lifecycle_mixed_reused_prefix_v1', 'lifecycle_mixed_fixture_v1', 'lifecycle_mixed_requests_v1',
        'lifecycle_mixed_record_v1', 'lifecycle_mixed_acquired_fixture_v1', 'lifecycle_mixed_trace_audit_v1',
        'lifecycle_mixed_late_failure_witness_v1', 'lifecycle_mixed_capacity_witness_v1',
        'lifecycle_mixed_terminal_fixture_v1', 'lifecycle_mixed_stable_release_v1',
        'lifecycle_mixed_settlement_witness_v1', 'lifecycle_mixed_unknown_witness_v1'),
}
VERIFIED = 1271


def need(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def previous(repo):
    path = repo / PREVIOUS
    need(path.is_file() and not any(p.is_symlink() for p in (path, *path.parents)), 'ordinary inherited checker')
    data = path.read_bytes()
    need(digest(data) == PREVIOUS_SHA, 'authenticate inherited checker before import')
    module = types.ModuleType('mixed_lifecycle_previous')
    module.__file__ = str(path)
    sys.modules[module.__name__] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    need(module.ordinary(repo, PREVIOUS) == module.git(repo, 'show', BASELINE + ':' + str(PREVIOUS)),
         'inherited checker at signed baseline')
    lifecycle, deps, old_paths = module.inherited(repo)
    paths = old_paths | module.PRIOR_ADDITIONS | module.NEW | {module.CHECK, module.TEST}
    need(len(paths) == 459, 'exact inherited mixed closure')
    return module, lifecycle, deps, old_paths, paths


def materialize(stage, data):
    for path, payload in sorted(data.items()):
        target = stage / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)


def authenticate_baseline(repo, old, lifecycle, deps, old_paths, baseline, parent):
    historical = {p: old.git(repo, 'show', old.HISTORICAL + ':' + str(p)) for p in old_paths}
    prior = {p: old.git(repo, 'show', old.BASELINE + ':' + str(p)) for p in old_paths | old.PRIOR_ADDITIONS}
    with tempfile.TemporaryDirectory(prefix='mixed-lifecycle-history-', dir=parent) as temporary:
        stage = Path(temporary)
        materialize(stage, historical)
        lifecycle.authenticate(stage, old_paths - set(lifecycle.NEW) - {old.OLD_CHECK}, deps, git_repo=repo)
        lifecycle.scan_sources(stage, deps)
    with tempfile.TemporaryDirectory(prefix='mixed-lifecycle-baseline-', dir=parent) as temporary:
        stage = Path(temporary)
        materialize(stage, baseline)
        old.authenticate_current(stage, deps, old_paths, historical, prior)
        old.scan_new(stage, deps)


def check_delta(candidate, baseline):
    need(set(candidate) == set(baseline) | {WITNESS, CHECK, TEST}, 'closed 462-input candidate')
    need(len(candidate) == 462, 'exact extension closure size')
    need({p for p in baseline if candidate[p] != baseline[p]} == MODIFIED, 'exact eight-file reviewed delta')
    need(set(SOURCE_PINS) == MODIFIED | {WITNESS}, 'closed reviewed source pins')
    for path, pin in SOURCE_PINS.items():
        need(re.fullmatch('[0-9a-f]{64}', pin) and digest(candidate[path]) == pin, 'reviewed source: ' + str(path))


def contracts(candidate, baseline, old, lifecycle, deps):
    policy = deps['policy']
    need(set(CONTRACT_PINS) == {(p, n) for p, names in CONTRACT_NAMES.items() for n in names}, 'closed reviewed contract roster')
    for (path, name), pin in CONTRACT_PINS.items():
        block = lifecycle.contract(candidate[path].decode(), name, policy)
        need(digest(block.encode()) == pin, 'reviewed contract: ' + name)
    for name in lifecycle.READER_CONTRACT_PINS:
        need(lifecycle.contract(candidate[READER].decode(), name, policy)
             == lifecycle.contract(baseline[READER].decode(), name, policy), 'unchanged inherited reader contract: ' + name)
    expected = baseline[ROOT].decode()
    expected = old.once(expected, 'use owner_lifecycle_model::*;',
        'use owner_lifecycle_model::*;\n#[path = "context_mixed_acquire_model_v1.rs"]\nmod mixed_acquire_model;\nuse mixed_acquire_model::*;')
    expected = old.once(expected, '    include!("context_producer_stable_witnesses_v1.rs");',
        '    include!("context_producer_stable_witnesses_v1.rs");\n    include!("context_mixed_acquire_execution_v1.rs");\n    include!("context_mixed_acquire_correspondence_v1.rs");')
    expected = old.once(expected, '    include!("context_owner_lifecycle_reader_witnesses_v1.rs");',
        '    include!("context_owner_lifecycle_reader_witnesses_v1.rs");\n    include!("context_owner_lifecycle_mixed_witnesses_v1.rs");')
    need(candidate[ROOT] == expected.encode(), 'one canonical lifecycle root extension')
    need(candidate[ALIAS] == b'include!("context_owner_lifecycle_paired_v1.rs");\n', 'development root is only a canonical alias')
    for name in ('lifecycle_mixed_reused_prefix_v1', 'lifecycle_mixed_fixture_v1',
                 'lifecycle_mixed_acquired_fixture_v1', 'lifecycle_mixed_late_failure_witness_v1',
                 'lifecycle_mixed_capacity_witness_v1', 'lifecycle_mixed_terminal_fixture_v1',
                 'lifecycle_mixed_settlement_witness_v1', 'lifecycle_mixed_unknown_witness_v1'):
        need(not re.search(r'\brequires\b', lifecycle.contract(candidate[WITNESS].decode(), name, policy)),
             'constructor-origin witness has no admission premise: ' + name)


def scan(stage, deps):
    for path in sorted((MODIFIED | {WITNESS}) - {ROOT, ALIAS}):
        deps['policy'].scan(stage / path)
    for path in (MODEL, V / 'context_owner_lifecycle_history_v1.rs', V / 'context_owner_lifecycle_preservation_v1.rs'):
        deps['inspection'].independent_model((stage / path).read_text(), deps['policy'])


def receipt_budget(row, seconds):
    # The inherited outer watchdog has 10 seconds of headroom; allow five more
    # for receipt recording/reaping, without accepting an unbounded replay.
    need(type(row['started_ns']) is int and type(row['finished_ns']) is int
         and 0 < row['started_ns'] <= row['finished_ns'], 'receipt clock types/order')
    need(row['finished_ns'] - row['started_ns'] <= (seconds + 15) * 1_000_000_000, 'receipt execution budget')


def mutations(candidate, old):
    rows = [
        ('stable-history', MODEL, '\n        | LifecycleStepV1::AcquireMixed { stable_output: output, result, .. }', '',
         'owner_lifecycle_preservation', 'lifecycle_step_histories_v1'),
        ('producer-history', MODEL, '\n        | LifecycleStepV1::AcquireMixed { producer_output: output, result, .. }', '',
         'owner_lifecycle_preservation', 'lifecycle_step_histories_v1'),
        ('result-answer', BRIDGE,
         'result == begin_result_from(m) && stable_output_view(stable_output) == so && producer_output_view(producer_output) == po',
         'stable_output_view(stable_output) == so && producer_output_view(producer_output) == po',
         'production', 'lifecycle_mixed_answers_projection_v1'),
        ('stable-answer', BRIDGE, ' && stable_output_view(stable_output) == so', '',
         'production', 'lifecycle_mixed_answers_projection_v1'),
        ('producer-answer', BRIDGE, ' && producer_output_view(producer_output) == po', '',
         'production', 'lifecycle_mixed_answers_projection_v1'),
        ('mixed-domain', DOMAIN, 'LifecycleActualStepV1::AcquireMixed { .. } => mixed_acquire_storage_v1(owner),',
         'LifecycleActualStepV1::AcquireMixed { .. } => true,', 'production', 'lifecycle_mixed_domain_projection_v1'),
        ('stable-original', BRIDGE, ' && stable_output_view(stable_original) == so', '',
         'production', 'lifecycle_mutation_correspondence_v1'),
        ('producer-original', BRIDGE, ' && producer_output_view(producer_original) == po', '',
         'production', 'lifecycle_mutation_correspondence_v1'),
        ('missing-atomic-event', READER,
         'actual: trace.actual.push(after), model: trace.model.push(model_after), events: trace.events.push(event)',
         'actual: trace.actual.push(after), model: trace.model.push(model_after), events: trace.events',
         'production', 'lifecycle_reader_append_event_v1'),
    ]
    for _, path, before, after, _, _ in rows:
        old.once(candidate[path].decode(), before, after)
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verus', type=Path)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--resume', action='store_true')
    modes.add_argument('--verify', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    need(Path(__file__).resolve() == repo / CHECK, 'checker in source checkout')
    if not args.verify:
        output.parent.mkdir(parents=True, exist_ok=True)
    parent = None if args.verify else output.parent
    old, lifecycle, deps, old_paths, paths = previous(repo)
    base, scalar, legacy = (deps[key] for key in ('base', 'scalar', 'legacy'))
    baseline = {p: old.git(repo, 'show', BASELINE + ':' + str(p)) for p in paths}
    candidate = {p: old.ordinary(repo, p) for p in paths | {WITNESS, CHECK, TEST}}
    authenticate_baseline(repo, old, lifecycle, deps, old_paths, baseline, parent)
    check_delta(candidate, baseline)
    contracts(candidate, baseline, old, lifecycle, deps)
    need(type(VERIFIED) is int and VERIFIED > 738, 'measured whole-root count required')
    changes = mutations(candidate, old)
    inputs = {str(p): digest(data) for p, data in sorted(candidate.items())}
    head = old.git(repo, 'rev-parse', 'HEAD').decode().strip()
    recorded = base.unique_json(old.ordinary(output, Path('source.json')).decode()) if args.verify else None
    if recorded is not None:
        need(set(recorded) == {'commit', 'baseline', 'inputs', 'recorded_repo', 'recorded_output', 'recorded_verus'}, 'closed replay identity')
        for key in ('recorded_repo', 'recorded_output', 'recorded_verus'):
            need(type(recorded[key]) is str and Path(recorded[key]).is_absolute()
                 and '..' not in Path(recorded[key]).parts, 'absolute recorded path')
        need(re.fullmatch('[0-9a-f]{40}', recorded['commit']) is not None, 'recorded source commit')
        old.git(repo, 'merge-base', '--is-ancestor', recorded['commit'], head)
        commit, verus = recorded['commit'], Path(recorded['recorded_verus'])
    else:
        need(args.verus is not None, 'pinned verifier required')
        commit, verus = head, args.verus.resolve(strict=True)
    need(verus.name == 'verus', 'named pinned verifier')
    for path, pin in inputs.items():
        need(digest(old.git(repo, 'show', commit + ':' + path)) == pin, 'signed containing source: ' + path)
    with tempfile.TemporaryDirectory(prefix='mixed-lifecycle-signer-', dir=parent) as temporary:
        signer = Path(temporary) / 'allowed-signers'
        signer.write_text(old.SIGNER)
        for revision in (old.HISTORICAL, old.BASELINE, BASELINE, commit):
            old.git(repo, '-c', 'gpg.ssh.allowedSignersFile=' + str(signer), 'verify-commit', revision)
    cases = [('positive-before', None), *[(row[0], row) for row in changes], ('positive-after', None),
             ('mixed-regression', None), ('stable-regression', None)]
    names = ['checker-tests', 'closure-before', *[name for name, _ in cases], 'closure-after']
    need(args.stop_after is None or args.stop_after in names, 'known stop checkpoint')
    recorded_repo = Path(recorded['recorded_repo']) if recorded else repo
    recorded_output = Path(recorded['recorded_output']) if recorded else output
    closure = ['/bin/sh', str(recorded_repo / legacy.CLOSURE), str(verus.parent), str(recorded_repo / legacy.MANIFEST)]
    tests = ['/usr/bin/python3', '-I', '-B', str(recorded_repo / TEST)]
    identity = dict(commit=commit, baseline=BASELINE, inputs=inputs, recorded_repo=str(recorded_repo),
                    recorded_output=str(recorded_output), recorded_verus=str(verus))
    env = dict(HOME='/home/harsh', PATH='/home/harsh/.cargo/bin:/usr/bin:/bin', CARGO_HOME='/home/harsh/.cargo',
               RUSTUP_HOME='/home/harsh/.rustup', VERUS_Z3_PATH=str(verus.parent / 'z3'), TMPDIR=str(output))
    change_map, active = dict(cases), {}

    def root_for(name):
        return old.ROOT if name == 'mixed-regression' else old.REGRESSION if name == 'stable-regression' else ROOT

    def validate(name, row, stdout, stderr):
        budget = 120 if name == 'checker-tests' or name.startswith('closure-') else (
            old.SCOPED_SECONDS if change_map[name] else old.WHOLE_SECONDS)
        receipt_budget(row, budget)
        if name == 'checker-tests':
            need(row['command'] == tests and type(row['status']) is int and row['status'] == 0
                 and stdout == 'PASS: mixed-lifecycle checker calibration (8 groups)\n' and not stderr, 'exact checker calibration')
            return None
        if name.startswith('closure-'):
            need(row['command'] == closure and type(row['status']) is int and row['status'] == 0 and not stderr
                 and stdout == 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned tool closure')
            return None
        root, change = root_for(name), change_map[name]
        recorded_root = Path(row['command'][-1])
        need(recorded_root.is_absolute() and str(recorded_root).endswith('/' + str(root)), 'exact proof root')
        stage = Path(str(recorded_root)[:-len(str(root)) - 1])
        need(stage.parent == recorded_output and stage.name.startswith('sources-'), 'owned source stage')
        need(row['command'] == old.command(legacy, verus, stage / root, change), 'exact proof command')
        if stage in active:
            old.stage_gate(stage, active[stage], row, base)
        else:
            need(args.verify or args.resume, 'live staged continuity required')
        observed = legacy.normalized(base, stdout, stderr, stage)
        scalar.check_verifier(observed['verus'])
        if change:
            lifecycle.check_negative(scalar, name, row['status'], observed)
        else:
            count = old.VERIFIED if name == 'mixed-regression' else old.REGRESSION_VERIFIED if name == 'stable-regression' else VERIFIED
            need(type(row['status']) is int and row['status'] == 0 and not observed['diagnostics'], 'clean whole-root positive')
            need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                'success': True, 'verified': count, 'errors': 0, 'is-verifying-entire-crate': True}), 'exact positive count')
        return observed

    lock = None if args.verify else scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    campaign = scalar.Campaign(output, identity, names, base, legacy.same, validate, args.resume or args.verify)

    def unchanged():
        need(old.git(repo, 'rev-parse', 'HEAD').decode().strip() == head, 'source HEAD unchanged')
        need(candidate == {p: old.ordinary(repo, p) for p in candidate}, 'all source inputs unchanged')

    if args.verify:
        need(len(campaign.accepted) == len(names), 'complete campaign required')
        need(legacy.same(base.unique_json(old.ordinary(output, Path('inputs-after.json')).decode()), inputs), 'retained final inputs')
        need(legacy.same(base.unique_json(old.ordinary(output, Path('results.json')).decode()), campaign.proofs), 'retained proof summaries')
        with tempfile.TemporaryDirectory(prefix='mixed-lifecycle-replay-', dir=parent) as temporary:
            stage = Path(temporary)
            materialize(stage, candidate)
            scan(stage, deps)
            for change in changes:
                target = stage / change[1]
                target.write_text(old.once(candidate[change[1]].decode(), change[2], change[3]))
                scan(stage, deps)
                target.write_bytes(candidate[change[1]])
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching positive bracket')
        unchanged()
        print('PASS: mixed-lifecycle campaign replay', flush=True)
        return

    for name in names:
        unchanged()
        if name in campaign.rows:
            print(name, 'replayed', flush=True)
        elif name == 'checker-tests':
            campaign.run(name, tests, 120, env)
        elif name.startswith('closure-'):
            campaign.run(name, closure, 120, env)
        else:
            stage = Path(tempfile.mkdtemp(prefix='sources-', dir=output))
            data = baseline if name.endswith('-regression') else candidate
            materialize(stage, data)
            change = change_map[name]
            if change:
                (stage / change[1]).write_text(old.once(candidate[change[1]].decode(), change[2], change[3]))
            if not name.endswith('-regression'):
                scan(stage, deps)
            active[stage] = old.stage_map(stage)
            campaign.run(name, old.command(legacy, verus, stage / root_for(name), change),
                         (old.SCOPED_SECONDS if change else old.WHOLE_SECONDS) + 10, env)
            old.stage_gate(stage, active[stage], campaign.rows[name][0], base)
            shutil.rmtree(stage)
            need(not os.path.lexists(stage), 'owned source stage absence')
            del active[stage]
            print(name, campaign.proofs[name]['result'], flush=True)
        unchanged()
        if name == args.stop_after and name != names[-1]:
            print('CHECKPOINT: ' + name + '; qualification incomplete', flush=True)
            return
    need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching whole-root bracket')
    (output / 'inputs-after.json').write_text(json.dumps(inputs, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')
    lock.close()
    print('PASS: constructor-origin mixed-lifecycle developer campaign', flush=True)


if __name__ == '__main__':
    main()
