#!/usr/bin/env python3
"""Source-bound developer qualification of independent mixed acquisition.

This extension leaves inherited lifecycle pins unchanged. It is not registered
in the global proof inventory and does not qualify Context or native execution.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import types
from pathlib import Path

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
V = CRATE / 'verus'
CHECK = V / 'check-mixed-acquire.py'
TEST = V / 'test-mixed-acquire.py'
OLD_CHECK = V / 'check-owner-lifecycle.py'
OLD_SHA = '7e442ed7c87078f947e550fe17f7387a90dd6ace6557bb5b64241de88bc59ea6'
HISTORICAL = '0da85d666e17988d65cce7a30c7ee67319487e45'
BASELINE = 'c2102783964fa405bef28b757f6b834f09c56181'
SIGNER = 'harmenon@amd.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICITzoV64zd4tYeZhvOi+mnwQaxEI4rFvXeC3HxileBS\n'
ROOT = V / 'context_mixed_acquire_historical_v1.rs'
REGRESSION = V / 'context_producer_stable_historical_v1.rs'
MODEL = V / 'context_mixed_acquire_model_v1.rs'
BRIDGE = V / 'context_mixed_acquire_correspondence_v1.rs'
WITNESS = V / 'context_mixed_acquire_witness_v1.rs'
EXECUTION = V / 'context_mixed_acquire_execution_v1.rs'
BODY = CRATE / 'src/context_producer_reads/mixed_acquire_bodies.rs'
NEW = {ROOT, MODEL, BRIDGE, WITNESS}
PRIOR_ADDITIONS = {BODY, EXECUTION, V / 'context_mixed_acquire_paired_v1.rs',
                   CRATE / 'src/context_producer_reads/tests/mixed_acquire.rs'}
PRIOR_CHANGES = {CRATE / ('src/' + name) for name in (
    'context_producer_reads.rs', 'context_producer_reads/tests.rs',
    'context_read_leases.rs', 'context_read_leases/acquire.rs')}
# Historical evidence also captured this Context input. It is source-bound to
# the CPU-qualified baseline, but is not compiled into the bounded proof root.
PRIOR_INPUT_CHANGES = PRIOR_CHANGES | {Path('crates/fe2o3-runtime/src/context.rs')}
# Filled only after source review and measurement, never by this checker.
NEW_PINS = {
    MODEL: 'e63acfa4bd41cc7b451c5213ab3e1cb692b43b30ef126d3ef8e555313baf0456',
    BRIDGE: 'ea8139b508fb2325e8cd282228e54da8d50a0f207de6433a569cf73d31b13ad1',
    ROOT: '48ce1cadef3d59656287d9bf56b8cdf9506d4d507185dd2bfcf2abcf41cb788f',
    WITNESS: '8c63cedd65afaf8d11ec095a72e3baabe1db43fb9afada64dba9230420114cae',
}
VERIFIED = 738
REGRESSION_VERIFIED = 720
WHOLE_SECONDS = 1200
SCOPED_SECONDS = 600


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(repo, *args):
    return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
        '-c', 'gpg.ssh.program=/usr/bin/ssh-keygen', '-C', str(repo), *args], env={
        'PATH': '/usr/bin:/bin', 'LC_ALL': 'C', 'GIT_NO_REPLACE_OBJECTS': '1',
        'GIT_OPTIONAL_LOCKS': '0', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull})


def ordinary(repo, path):
    need(not path.is_absolute() and '..' not in path.parts, 'relative input path')
    need(not any((repo / p).is_symlink() for p in (path, *path.parents)), 'no input symlinks: ' + str(path))
    need(stat.S_ISREG((repo / path).lstat().st_mode), 'ordinary input: ' + str(path))
    return (repo / path).read_bytes()


def inherited(repo):
    data = ordinary(repo, OLD_CHECK)
    need(sha(data) == OLD_SHA and data == git(repo, 'show', HISTORICAL + ':' + str(OLD_CHECK)),
         'authenticated historical lifecycle checker before import')
    module = types.ModuleType('mixed_inherited_lifecycle')
    module.__file__ = str(repo / OLD_CHECK)
    sys.modules[module.__name__] = module
    exec(compile(data, module.__file__, 'exec'), module.__dict__)
    deps = module.dependencies(repo)
    paths = module.inherited_sources(repo, deps['base']) | set(module.NEW) | {OLD_CHECK}
    need(len(paths) == 449, 'exact historical lifecycle closure')
    return module, deps, paths


def once(data, before, after):
    need(data.count(before) == 1 and before != after, 'unique mutation/insertion anchor')
    return data.replace(before, after, 1)


def root_contract(repo):
    expected = ordinary(repo, REGRESSION).decode()
    expected = once(expected, 'mod production {',
        '#[path = "context_mixed_acquire_model_v1.rs"]\nmod mixed_acquire_model;\nuse mixed_acquire_model::*;\n\nmod production {')
    anchor = '    include!("context_producer_stable_witnesses_v1.rs");'
    expected = once(expected, anchor, anchor + ''.join('\n    include!("' + p.name + '");'
        for p in (EXECUTION, BRIDGE, WITNESS)))
    need(ordinary(repo, ROOT) == expected.encode(), 'exact independent mixed root extension')


def independent_model(source, deps):
    deps['inspection'].independent_model(source, deps['policy'])
    need(re.search(r'\bContext[A-Za-z0-9_]*\b|\b(?:producer|stable)_represents\b',
                   deps['policy'].code_only(source)) is None, 'logical model cannot assume actual representation')


def authenticate(repo, lifecycle, deps, old_paths, scratch_parent):
    need(set(NEW_PINS) == NEW and all(re.fullmatch('[0-9a-f]{64}', p) for p in NEW_PINS.values()), 'reviewed new source pins required')
    need(type(VERIFIED) is int and type(REGRESSION_VERIFIED) is int
         and VERIFIED > REGRESSION_VERIFIED > 0, 'measured whole-root counts required')
    # Audit the old closure in isolation, never repinning it to accept the new implementation.
    with tempfile.TemporaryDirectory(prefix='mixed-historical-', dir=scratch_parent) as temporary:
        historical = Path(temporary)
        for path in sorted(old_paths):
            target = historical / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(git(repo, 'show', HISTORICAL + ':' + str(path)))
        lifecycle.authenticate(historical, old_paths - set(lifecycle.NEW) - {OLD_CHECK}, deps, git_repo=repo)
        lifecycle.scan_sources(historical, deps)
    historical_data = {p: git(repo, 'show', HISTORICAL + ':' + str(p)) for p in old_paths}
    baseline_data = {p: git(repo, 'show', BASELINE + ':' + str(p)) for p in old_paths | PRIOR_ADDITIONS}
    authenticate_current(repo, deps, old_paths, historical_data, baseline_data)


def authenticate_current(repo, deps, old_paths, historical_data, baseline_data):
    need(set(historical_data) == old_paths and set(baseline_data) == old_paths | PRIOR_ADDITIONS, 'exact frozen source maps')
    changed = {p for p in old_paths if ordinary(repo, p) != historical_data[p]}
    need(changed == PRIOR_INPUT_CHANGES, 'exact inherited baseline delta')
    for path in old_paths | PRIOR_ADDITIONS:
        need(ordinary(repo, path) == baseline_data[path], 'unchanged signed mixed baseline: ' + str(path))
    for path, pin in NEW_PINS.items():
        need(sha(ordinary(repo, path)) == pin, 'reviewed independent source: ' + str(path))
    root_contract(repo)
    independent_model(ordinary(repo, MODEL).decode(), deps)
    expected = {p for p in old_paths | PRIOR_ADDITIONS if p.is_relative_to(CRATE / 'src')}
    discovered = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in discovered), 'no runtime source links')
    need({p.relative_to(repo) for p in discovered if p.is_file()} == expected, 'closed runtime source roster')


def mutations(repo):
    data = ordinary(repo, BODY).decode()
    stable_commit = '            if !$stable_requests.is_empty() {\n                $stable_commit(&mut $contents.stable, $consumer, $stable_requests, $stable_output);\n            }\n'
    pending_preflight = '            if !$producer_requests.is_empty() {\n                match $producer_preflight($contents, $consumer, $producer_requests, $producer_output) {\n                    Ok($value) => { $($producer_passed)* },\n                    Err(error) => return Err(error),\n                }\n            }\n'
    producer_commit = '            if !$producer_requests.is_empty() {\n                $producer_commit($contents, $consumer, $producer_requests, $producer_output);\n            }\n'
    rows = [
        ('combined-budget', BODY, 'if count > $contents.remaining_read_slots()', 'if false', 'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'),
        ('pending-preflight', BODY, pending_preflight, '', 'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'),
        ('stable-commit', BODY, stable_commit, '', 'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'),
        ('producer-commit', BODY, producer_commit, '', 'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'),
        ('early-error-frame', BODY, '                return Err(ContextVersionJournalErrorV1::ForeignContext);',
         '                if !$stable_output.is_empty() { $stable_output[0] = None; }\n                if !$producer_output.is_empty() { $producer_output[0] = None; }\n                return Err(ContextVersionJournalErrorV1::ForeignContext);',
         'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'),
        ('pending-headroom', MODEL, '        requests.len() <= after.reservations@.len() - producer_live_reads_v1(after),\n', '',
         'mixed_acquire_model', 'mixed_pending_decision_frame_v1'),
        ('bridge-actual-executor', BRIDGE,
         '    let result = actual.acquire_mixed_reads(consumer, stable, stable_output, pending, producer_output);',
         '    let result: Result<(), ReadErrorV1> = Err(ReadErrorV1::InvalidState);',
         'production', 'mixed_acquire_paired_exec_v1'),
        ('bridge-logical-executor', BRIDGE,
         '    let model_result = logical::mixed_acquire_exec_v1(model, model_consumer, model_stable, model_stable_output,\n        model_pending, model_producer_output);',
         '    let model_result: Result<(), logical::ReadErrorV1> = Err(logical::ReadErrorV1::InvalidState);',
         'production', 'mixed_acquire_paired_exec_v1'),
    ]
    moved = once(once(data, stable_commit, ''), pending_preflight, stable_commit + pending_preflight)
    rows.insert(0, ('stable-before-preflight', BODY, data, moved, 'production', 'ContextProducerReadJournalV1::acquire_mixed_reads'))
    for _, path, before, after, _, _ in rows:
        once(ordinary(repo, path).decode(), before, after)
    return rows


def scan_new(stage, deps):
    for path in (MODEL, BRIDGE, WITNESS):
        deps['policy'].scan(stage / path)
    independent_model((stage / MODEL).read_text(), deps)
    # The single reviewed macro/include wrappers are removed only for lexical auditing.
    for path, replacements in ((BODY, [('macro_rules!', '')]),
                               (EXECUTION, [('include!("../src/context_producer_reads/mixed_acquire_bodies.rs");', '')])):
        data = (stage / path).read_text()
        for before, after in replacements:
            data = once(data, before, after)
        audit = stage / ('audit-' + path.name)
        audit.write_text(data)
        deps['policy'].scan(audit)


def stage_map(stage):
    paths = list(stage.rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no staged source links')
    return {str(p.relative_to(stage)): sha(ordinary(stage, p.relative_to(stage)))
            for p in sorted(paths) if not p.is_dir()}


def stage_gate(stage, expected, row, base):
    need(stage_map(stage) == expected, 'staged input continuity before acceptance')
    need(row['group_absent'] is True and not base.group_exists(row['process_group']),
         'terminal owned proof group before acceptance')


def command(legacy, verus, root, change):
    selection = None if change is None else (change[0], change[1], '', change[4], change[5], '', '', 'logical')
    cmd = legacy.command(verus, root, selection)
    need(cmd[:5] == ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '600'], 'inherited command prefix')
    cmd[4] = str(WHOLE_SECONDS if change is None else SCOPED_SECONDS)
    return cmd


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verus', type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--resume', action='store_true')
    mode.add_argument('--verify', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    need(Path(__file__).resolve() == repo / CHECK, 'checker in source checkout')
    if not args.verify:
        output.parent.mkdir(parents=True, exist_ok=True)
    scratch_parent = None if args.verify else output.parent
    lifecycle, deps, old_paths = inherited(repo)
    base, scalar, legacy = (deps[n] for n in ('base', 'scalar', 'legacy'))
    recorded = base.unique_json(ordinary(output, Path('source.json')).decode()) if args.verify else None
    if recorded is not None:
        need(set(recorded) == {'commit', 'baseline', 'historical', 'inputs', 'recorded_repo', 'recorded_output', 'recorded_verus'},
             'closed replay identity')
        for key in ('recorded_repo', 'recorded_output', 'recorded_verus'):
            need(type(recorded[key]) is str and Path(recorded[key]).is_absolute()
                 and '..' not in Path(recorded[key]).parts, 'absolute recorded path')
        need(re.fullmatch('[0-9a-f]{40}', recorded['commit']) is not None, 'recorded source commit')
        verus = Path(recorded['recorded_verus'])
    else:
        need(args.verus is not None, 'pinned verifier required for execution')
        verus = args.verus.resolve(strict=True)
    need(verus.name == 'verus', 'named pinned Verus executable')
    authenticate(repo, lifecycle, deps, old_paths, scratch_parent)
    sources = old_paths | PRIOR_ADDITIONS | NEW
    inputs = sources | {CHECK, TEST}
    before = {str(p): sha(ordinary(repo, p)) for p in sorted(inputs)}
    head = git(repo, 'rev-parse', 'HEAD').decode().strip()
    commit = recorded['commit'] if recorded is not None else head
    if recorded is not None:
        git(repo, 'merge-base', '--is-ancestor', commit, head)
    for path, pin in before.items():
        need(sha(git(repo, 'show', commit + ':' + path)) == pin, 'signed containing source required: ' + path)
    with tempfile.TemporaryDirectory(prefix='mixed-signer-', dir=scratch_parent) as temporary:
        signer = Path(temporary) / 'allowed-signers'
        signer.write_text(SIGNER)
        for revision in (HISTORICAL, BASELINE, commit):
            git(repo, '-c', 'gpg.ssh.allowedSignersFile=' + str(signer), 'verify-commit', revision)
    changes = mutations(repo)
    cases = [('positive-before', None), *[(r[0], r) for r in changes],
             ('positive-after', None), ('stable-regression', None)]
    names = ['checker-tests', 'closure-before', *[n for n, _ in cases], 'closure-after']
    need(args.stop_after is None or args.stop_after in names, 'known stop checkpoint')
    recorded_repo = Path(recorded['recorded_repo']) if recorded is not None else repo
    recorded_output = Path(recorded['recorded_output']) if recorded is not None else output
    closure = ['/bin/sh', str(recorded_repo / legacy.CLOSURE), str(verus.parent), str(recorded_repo / legacy.MANIFEST)]
    tests = ['/usr/bin/python3', '-I', '-B', str(recorded_repo / TEST)]
    for path, pin in legacy.PINS.items():
        need(sha(ordinary(repo, path)) == pin, 'inherited pinned tool/source input')
    identity = {'commit': commit, 'baseline': BASELINE, 'historical': HISTORICAL, 'inputs': before,
                'recorded_repo': str(recorded_repo), 'recorded_output': str(recorded_output), 'recorded_verus': str(verus)}
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'TMPDIR': str(output)}
    change_map = dict(cases)
    active_stages = {}

    def validate(name, row, stdout, stderr):
        if name == 'checker-tests':
            need(row['command'] == tests and type(row['status']) is int and row['status'] == 0,
                 'checker calibration tests')
            need(stdout == 'PASS: mixed-acquisition checker calibration (11 groups)\n' and not stderr,
                 'exact calibration inventory')
            return None
        if name.startswith('closure-'):
            need(row['command'] == closure and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned verifier distribution')
            return None
        change = change_map[name]
        root = REGRESSION if name == 'stable-regression' else ROOT
        recorded = Path(row['command'][-1])
        need(recorded.is_absolute() and str(recorded).endswith('/' + str(root)), 'exact recorded proof root')
        stage = Path(str(recorded)[:-len(str(root)) - 1])
        need(stage.parent == recorded_output and stage.name.startswith('sources-'), 'owned proof stage')
        need(row['command'] == command(legacy, verus, stage / root, change), 'exact proof command')
        if stage in active_stages:
            stage_gate(stage, active_stages[stage], row, base)
        else:
            need(args.resume or args.verify, 'live proof requires staged continuity bracket')
        observed = legacy.normalized(base, stdout, stderr, stage)
        scalar.check_verifier(observed['verus'])
        if change:
            lifecycle.check_negative(scalar, name, row['status'], observed)
        else:
            need(type(row['status']) is int and row['status'] == 0 and not observed['diagnostics'], 'clean whole-root positive')
            need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                'success': True, 'verified': REGRESSION_VERIFIED if name == 'stable-regression' else VERIFIED,
                'errors': 0, 'is-verifying-entire-crate': True}), 'exact whole-root count')
        return observed

    lock = None if args.verify else scalar.campaign_lock(output)
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    campaign = scalar.Campaign(output, identity, names, base, legacy.same, validate, args.resume or args.verify)

    if args.verify:
        need(len(campaign.accepted) == len(names), 'complete campaign required for replay')
        need(ordinary(output, Path('inputs-after.json')) and ordinary(output, Path('results.json')), 'complete replay summaries')
        # Reconstruct and scan every mutation from the authenticated source, without executing it.
        with tempfile.TemporaryDirectory(prefix='mixed-replay-', dir=scratch_parent) as temporary:
            stage = Path(temporary)
            for path in sources:
                target = stage / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(ordinary(repo, path))
            scan_new(stage, deps)
            for change in changes:
                target = stage / change[1]
                original = ordinary(repo, change[1])
                target.write_bytes(once(original.decode(), change[2], change[3]).encode())
                scan_new(stage, deps)
                target.write_bytes(original)
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching replay positive bracket')
        need(before == {str(p): sha(ordinary(repo, p)) for p in sorted(inputs)}, 'replay source continuity')
        print('PASS: independent mixed-acquisition campaign replay', flush=True)
        return

    def unchanged():
        need(git(repo, 'rev-parse', 'HEAD').decode().strip() == head, 'source HEAD unchanged')
        need(before == {str(p): sha(ordinary(repo, p)) for p in sorted(inputs)}, 'complete source inputs unchanged')

    for name in names:
        unchanged()
        if name in campaign.rows:
            print(name, 'replayed', flush=True)
        elif name == 'checker-tests':
            campaign.run(name, tests, 120, env)
        elif name.startswith('closure-'):
            campaign.run(name, closure, 120, env)
        else:
            change = change_map[name]
            root = REGRESSION if name == 'stable-regression' else ROOT
            stage = Path(tempfile.mkdtemp(prefix='sources-', dir=output))
            for path in sorted(sources):
                data = ordinary(repo, path)
                need(sha(data) == before[str(path)], 'staging source continuity')
                if change and path == change[1]:
                    data = once(data.decode(), change[2], change[3]).encode()
                target = stage / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            scan_new(stage, deps)
            staged_before = stage_map(stage)
            active_stages[stage] = staged_before
            campaign.run(name, command(legacy, verus, stage / root, change),
                (WHOLE_SECONDS if change is None else SCOPED_SECONDS) + 10, env)
            need(stage_map(stage) == staged_before, 'staged input continuity')
            row = campaign.rows[name][0]
            need(row['group_absent'] is True and not base.group_exists(row['process_group']), 'terminal owned proof group')
            # Retain failed stages; delete only after the accepted command and exact source recheck.
            shutil.rmtree(stage)
            need(not os.path.lexists(stage), 'owned stage absence')
            del active_stages[stage]
            print(name, campaign.proofs[name]['result'], flush=True)
        unchanged()
        if name == args.stop_after and name != names[-1]:
            print('CHECKPOINT: ' + name + '; qualification incomplete', flush=True)
            return
    need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching full positive bracket')
    (output / 'inputs-after.json').write_text(json.dumps(before, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')
    lock.close()
    print('PASS: independent mixed-acquisition developer campaign', flush=True)


if __name__ == '__main__':
    main()
