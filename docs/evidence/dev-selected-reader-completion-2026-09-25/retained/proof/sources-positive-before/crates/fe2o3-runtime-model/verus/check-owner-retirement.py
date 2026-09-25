#!/usr/bin/env python3
"""Qualify shared allocation retirement and independent logical execution.

Normal contents and conditional custody only: not physical storage, unwind,
native disposal authority, pending-consumer admission, or performance evidence.
"""
import argparse
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
ROOT = CRATE / 'verus/context_owner_retirement_paired_v1.rs'
RAW = CRATE / 'verus/context_owner_retirement_execution_v1.rs'
BODY = CRATE / 'src/context_version_journal/retirement_bodies.rs'
BRIDGE = CRATE / 'verus/context_owner_retirement_correspondence_v1.rs'
PREVIOUS = CRATE / 'verus/check-owner-scalar-enrollment.py'
FROZEN = '3d686c5ca4bbf8a3a58cb9de357f00aab2db7116'
FROZEN_INPUTS = Path('docs/evidence/dev-owner-scalar-enrollment-2026-09-23/records/source.json')
POLICY_NEW = [CRATE / ('verus/context_owner_retirement_' + suffix + '_v1.rs')
              for suffix in ('decisions', 'bodies', 'model', 'invariant', 'correspondence', 'witnesses')]
OWNERS = [('context_version_journal', 'ContextVersionJournalV1', None),
          ('context_read_leases', 'ContextReadLeasedJournalV1', 'journal'),
          ('context_producer_reads', 'ContextProducerReadJournalV1', 'stable')]
NEW = [ROOT, RAW, BODY, *POLICY_NEW,
       *[CRATE / ('src/' + owner + '/retirement_baseline.rs') for owner, _, _ in OWNERS],
       CRATE / 'src/context_version_journal/retirement_shared_tests.rs',
       CRATE / 'src/context_producer_reads/tests/retirement_shared.rs']
PROJECTIONS = [(BODY, 'audited-retirement.rs', 5)]
PAIRED_VERIFIED = 999

# Whole-file pins bind the reviewed adapters, lazy capacity expression, unchanged
# generic guards, test registration, and touched-only same-owner reset helpers.
# Frozen method bodies below are independently reconstructed from the prior commit.
ADAPTER_PINS = {
    'context_version_journal.rs': 'd878e74e78ce9321320171d166d1ec1a5ab92db9042d7e0c92e51e4c92379ef5',
    'context_version_journal/allocation_lifecycle.rs': 'f7eb4b75509741d67b987f120f59ee7c752b3d04862d76997a28dfcffc12f84e',
    'context_read_leases.rs': '323fd8957db9d0881a18732ff1e9d78b4ee6f3514091407be45d6aca0cf19100',
    'context_producer_reads.rs': 'ecfef18361c612f4a2d343373d8b788d22aa002edea26be8d5beff2354c4ff3b',
    'context_version_journal/guard_test_support.rs': '95b28ec694c4e38725ccc03bc936548eb2d95dc106a0fad80ffadc3fd4702c07',
    'context_read_leases/guard_test_support.rs': 'fedc047c3374223bbdb17043a9b20208d1fe7648c056ca772d56d60103691941',
    'context_producer_reads/guard_test_support.rs': '74103a7ae53b222cc3977c5fed7d88842d67f1efa7cc945ada41ae7465d998e5',
    'context_version_journal/tests.rs': 'cb19971e3038179b2fd5ceba671a7d60aeebc7ab43e08dde11a7c650aff7f122',
    'context_producer_reads/tests.rs': 'c621ee5736d007f22d9d1f026f4a7c3f8f9b93f1554dd0d9cf702eb5e6b31805',
}


def mutation(name, macro, owner, function, before, after):
    return (name, BODY, 'macro_rules! retirement_' + macro + '_body {',
            'production', owner + '::' + function, before, after, 'execution')


JOURNAL = 'ContextVersionJournalV1'
VALIDATE = 'validate_allocation_retirement_observed_v1'
RETIRE = 'retire_allocations_observed_v1'
MUTATIONS = [
    mutation('roster_bound', 'preflight', JOURNAL, VALIDATE,
             '$roster.len() > $journal.allocation_capacity', '$roster.len() >= $journal.allocation_capacity'),
    mutation('exact_reference', 'preflight', JOURNAL, VALIDATE,
             '$exact($journal, reference)', '$exact($journal, $roster[0])'),
    mutation('canonical_order', 'preflight', JOURNAL, VALIDATE,
             '!$less(key, reference.key)', '!$less(reference.key, key)'),
    mutation('identity_before_order', 'preflight', JOURNAL, VALIDATE,
             'let entry = match $exact($journal, reference)',
             'if let Some(key) = $previous { if !$less(key, reference.key) { '
             'return Err(ContextVersionJournalErrorV1::NonCanonicalRoster); } } '
             'let entry = match $exact($journal, reference)'),
    mutation('order_before_pending', 'preflight', JOURNAL, VALIDATE,
             'if let Some(key) = $previous {',
             'if entry.pending_member.is_some() { return Err(ContextVersionJournalErrorV1::AllocationBusy); } '
             'if let Some(key) = $previous {'),
    mutation('pending_guard', 'preflight', JOURNAL, VALIDATE,
             'entry.pending_member.is_some()', 'false'),
    mutation('empty_admission', 'preflight', JOURNAL, VALIDATE,
             'let mut $previous = None;', 'if $roster.len() == 0 { return Ok(()); } let mut $previous = None;'),
    mutation('logical_headroom', 'preflight', JOURNAL, VALIDATE,
             'returned > $journal.allocation_capacity || ', ''),
    mutation('physical_headroom', 'preflight', JOURNAL, VALIDATE,
             ' || returned > $capacity', ''),
    mutation('returned_count', 'preflight', JOURNAL, VALIDATE,
             '.checked_add($roster.len())', '.checked_add(0)'),
    mutation('unchecked_return', 'preflight', JOURNAL, VALIDATE,
             'let returned = match $journal.allocation_free.len().checked_add($roster.len()) {\n'
             '                Some(count) => count,\n'
             '                None => return Err(ContextVersionJournalErrorV1::InvalidState),\n'
             '            };',
             'let returned = $journal.allocation_free.len() + $roster.len();'),
    mutation('clear_slot', 'execute', JOURNAL, RETIRE, '$journal.allocations[reference.slot] = None;', ''),
    mutation('append_slot', 'execute', JOURNAL, RETIRE, '$journal.allocation_free.push(reference.slot);', ''),
    mutation('slot_order', 'execute', JOURNAL, RETIRE,
             '$journal.allocation_free.push(reference.slot);', '$journal.allocation_free.push($index);'),
    mutation('journal_frame', 'execute', JOURNAL, RETIRE,
             '$journal.allocation_free.push(reference.slot);',
             '$journal.allocation_free.push(reference.slot); $journal.registration_watermark = 0;'),
]
for prefix, owner in [('stable', 'ContextReadLeasedJournalV1'), ('producer', 'ContextProducerReadJournalV1')]:
    MUTATIONS.extend([
        mutation(prefix + '_busy', 'unread', owner, 'require_unread_allocations', 'count != 0', 'count > 1'),
        mutation(prefix + '_lookup_error', 'unread', owner, 'require_unread_allocations',
                 'Err(error) => return Err(error),', 'Err(_) => return Ok(()),'),
        mutation(prefix + '_guard', 'owner_validate', owner, VALIDATE,
                 'match $owner.require_unread_allocations($roster) {\n            Ok(()) => {},\n'
                 '            Err(error) => return Err(error),\n        }', ''),
        mutation(prefix + '_frame', 'owner_execute', owner, RETIRE,
                 '$owner.$field.$retire($roster $($observations)*)',
                 '{ $owner.next_incarnation = 0; $owner.$field.$retire($roster $($observations)*) }'),
    ])
MUTATIONS.extend([
    ('skip_actual', BRIDGE, 'fn owner_retirement_paired_exec_v1(', 'production', 'owner_retirement_paired_exec_v1',
     'let result = actual.retire_allocations_observed_v1(roster, capacity);',
     'let result: Result<(), ReadErrorV1> = Err(ReadErrorV1::AllocationBusy);', 'execution'),
    ('skip_model', BRIDGE, 'fn owner_retirement_paired_exec_v1(', 'production', 'owner_retirement_paired_exec_v1',
     'let model_result = logical::retirement_producer_exec_v1(model, model_roster, capacity);',
     'let model_result: Result<(), logical::ReadErrorV1> = Err(logical::ReadErrorV1::AllocationBusy);', 'execution'),
])


def need(condition, message):
    if not condition:
        raise ValueError(message)


def check_negative(scalar, name, status, observed):
    need(type(status) is int and status == 1, 'normal negative exit')
    scalar.negative_report(observed['result'])
    errors = [row for row in observed['diagnostics'] if row['level'] == 'error']
    allowed = {'assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
               'invariant not satisfied at end of loop body'}
    if name == 'unchecked_return':
        allowed.add('possible arithmetic underflow/overflow')
    need(any(row['message'] in allowed for row in errors)
         and all(row['message'] in allowed or re.fullmatch(r'aborting due to \d+ previous errors?', row['message'])
                 for row in errors), 'logical failure, not syntax, timeout, or solver resource exhaustion')


def recorder_selftest():
    import copy
    scalar = module(Path(__file__).resolve().parents[3], PREVIOUS, 'retirement_recorder_selftest')
    scalar.recorder_selftest()
    valid = {'result': {'encountered-error': True, 'encountered-vir-error': False, 'verified': 0,
                        'errors': 1, 'is-verifying-entire-crate': False},
             'diagnostics': [{'level': 'error', 'message': 'possible arithmetic underflow/overflow'}]}
    check_negative(scalar, 'unchecked_return', 1, valid)
    assertion = copy.deepcopy(valid)
    assertion['diagnostics'][0]['message'] = 'assertion failed'
    check_negative(scalar, 'clear_slot', 1, assertion)
    rejected = 0
    for fault in ('wrong_case', 'resource', 'frontend_message', 'abort_only', 'status', 'bool_status', 'frontend_flag', 'count'):
        row, name, status = copy.deepcopy(valid), 'unchecked_return', 1
        if fault == 'wrong_case':
            name = 'clear_slot'
        elif fault in ('resource', 'frontend_message', 'abort_only'):
            row['diagnostics'][0]['message'] = {'resource': 'function body check: Resource limit (rlimit) exceeded',
                'frontend_message': 'unexpected token', 'abort_only': 'aborting due to 1 previous error'}[fault]
        elif fault == 'status':
            status = 0
        elif fault == 'bool_status':
            status = True
        elif fault == 'frontend_flag':
            row['result']['encountered-vir-error'] = True
        else:
            row['result']['errors'] = 0
        try:
            check_negative(scalar, name, status, row)
        except ValueError:
            rejected += 1
        else:
            raise ValueError('accepted invalid negative diagnostic: ' + fault)
    print(f'PASS: retirement negative diagnostic policy; {rejected} malformed cases rejected')


def module(repo, path, name):
    spec = importlib.util.spec_from_file_location(name, repo / path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def source_roster(repo, inherited):
    expected = {p for p in inherited | set(NEW) if p.is_relative_to(CRATE / 'src')}
    paths = list((repo / CRATE / 'src').rglob('*'))
    need(not any(p.is_symlink() for p in paths), 'no runtime source symlinks')
    need({p.relative_to(repo) for p in paths if p.is_file()} == expected, 'closed runtime source roster')
    need({p.name for p in (repo / CRATE).iterdir()} == {'Cargo.toml', 'src', 'verus'}, 'closed Cargo discovery roster')


def authenticate(repo, inherited, scalar, git_repo=None):
    def old(path):
        return subprocess.check_output(['git', '-C', str(git_repo or repo), 'show', FROZEN + ':' + str(path)])

    def method(source, name):
        start = source.index('    pub fn ' + name + '(')
        return source[start:source.index('\n    }\n', start) + len('\n    }\n')]

    changed = {CRATE / 'src' / path for path in ADAPTER_PINS}
    need(changed <= inherited, 'all reviewed adapter paths inherited')
    for path in inherited:
        current = (repo / path).read_bytes()
        if path in changed:
            need(scalar.digest(current) == ADAPTER_PINS[str(path.relative_to(CRATE / 'src'))], 'exact reviewed adapter: ' + str(path))
        else:
            need(current == old(path), 'unchanged inherited input: ' + str(path))
    for owner, ty, field in OWNERS:
        source = CRATE / ('src/' + owner + '.rs') if field else CRATE / 'src/context_version_journal/allocation_lifecycle.rs'
        previous = old(source).decode()
        methods = []
        for name in ('validate_allocation_retirement', 'retire_allocations'):
            body = method(previous, name).replace('pub fn ' + name + '(', 'pub(crate) fn baseline_' + name + '_v1(')
            for callee in ('validate_allocation_retirement', 'retire_allocations'):
                body = body.replace('.' + callee + '(', '.baseline_' + callee + '_v1(')
            if field:
                body = body.replace('self.' + field + '.baseline_validate_allocation_retirement_v1(',
                                    'self.' + field + '\n            .baseline_validate_allocation_retirement_v1(')
            methods.append(body)
        expected = ('// Frozen at ' + FROZEN + '; only names and visibility are redirected.\n'
                    'use super::*;\n\nimpl ' + ty + ' {\n' + '\n'.join(methods) + '}\n')
        need((repo / CRATE / ('src/' + owner + '/retirement_baseline.rs')).read_text() == expected,
             'independent frozen methods: ' + owner)
    expected = old(scalar.ROOT).decode().replace('mod production {',
        '#[path = "context_owner_retirement_model_v1.rs"]\nmod owner_retirement_model;\nuse owner_retirement_model::*;\n\n'
        '#[path = "context_owner_retirement_invariant_v1.rs"]\nmod owner_retirement_invariant;\nuse owner_retirement_invariant::*;\n\nmod production {')
    expected = expected.replace('include!("context_owner_scalar_enrollment_execution_v1.rs");',
                                'include!("context_owner_retirement_execution_v1.rs");')
    expected = expected.replace('    include!("context_owner_scalar_enrollment_witnesses_v1.rs");',
        '    include!("context_owner_scalar_enrollment_witnesses_v1.rs");\n'
        '    include!("context_owner_retirement_correspondence_v1.rs");\n'
        '    include!("context_owner_retirement_witnesses_v1.rs");')
    need((repo / ROOT).read_text() == expected, 'exact retirement paired root')
    expected = ('// Extend the same actual owner declarations; Vec capacity is only observed.\n'
                'include!("context_owner_scalar_enrollment_execution_v1.rs");\n'
                'include!("../src/context_version_journal/retirement_bodies.rs");\n'
                'include!("context_owner_retirement_decisions_v1.rs");\n'
                'include!("context_owner_retirement_bodies_v1.rs");\n')
    need((repo / RAW).read_text() == expected, 'exact retirement actual root')
    source_roster(repo, inherited)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--probe', action='store_true')
    parser.add_argument('--resume', action='store_true')
    parser.add_argument('--stop-after')
    args = parser.parse_args()
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    os.chdir(repo)
    scalar = module(repo, PREVIOUS, 'retirement_scalar_check')
    lock = scalar.campaign_lock(output)
    history = module(repo, scalar.PREVIOUS, 'retirement_history_check')
    settlement = module(repo, history.PREVIOUS, 'retirement_settlement_check')
    writer = module(repo, settlement.PREVIOUS, 'retirement_writer_check')
    previous = module(repo, writer.PREVIOUS, 'retirement_enrollment_check')
    ancestor = module(repo, previous.PREVIOUS, 'retirement_unknown_check')
    legacy = module(repo, settlement.LEGACY, 'retirement_legacy_check')
    base = module(repo, legacy.BASE, 'retirement_process_recorder')
    policy = module(repo, legacy.POLICY, 'retirement_proof_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    inherited = {Path(p) for p in base.unique_json((repo / FROZEN_INPUTS).read_text())['inputs']} | {FROZEN_INPUTS}
    authenticate(repo, inherited, scalar)
    sources = sorted(inherited | set(NEW))
    inputs = set(sources) | {Path(__file__).resolve().relative_to(repo)}
    before = {str(p): scalar.digest((repo / p).read_bytes()) for p in sorted(inputs)}
    commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    if not args.probe:
        for path, expected in before.items():
            data = subprocess.check_output(['git', '-C', str(repo), 'show', commit + ':' + path])
            need(scalar.digest(data) == expected, 'source commit mismatch: ' + path)
    for path, pin in legacy.PINS.items():
        need(scalar.digest((repo / path).read_bytes()) == pin, 'source pin: ' + str(path))
    source = {'commit': commit, 'probe': args.probe, 'inputs': before}
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup',
           'VERUS_Z3_PATH': str(verus.parent / 'z3'), 'CARGO_TARGET_DIR': str(args.target.resolve())}
    cases = [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None),
             ('raw-regression', None), ('scalar-regression', None)]
    cpu = [
        ('recorder-test', ['python3', '-B', str(Path(__file__).resolve()), '--selftest']),
        ('compiler', ['rustc', '+1.97.1', '-Vv']),
        ('format', ['cargo', '+1.97.1', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', '+1.97.1', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', '+1.97.1', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
    ]
    names = ['closure-before', *[name for name, _ in cases], 'closure-after', *[name for name, _ in cpu]]
    need(args.stop_after is None or args.stop_after in names, 'known checkpoint name')
    closure_command = ['/bin/sh', str(repo / legacy.CLOSURE), str(verus.parent), str(repo / legacy.MANIFEST)]
    cpu_commands, mutations = dict(cpu), dict(cases)
    cpu_parser = module(repo, settlement.CPU_PARSER, 'retirement_cpu_parser').test_results

    def proof_root(name):
        return RAW if name == 'raw-regression' else scalar.ROOT if name == 'scalar-regression' else ROOT

    def validate(name, row, stdout, stderr):
        if name in ('closure-before', 'closure-after'):
            need(row['command'] == closure_command and row['status'] == 0 and not stderr and stdout ==
                 'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n', 'pinned Verus distribution')
        elif name in cpu_commands:
            need(row['command'] == cpu_commands[name] and row['status'] == 0, 'CPU check: ' + name)
            if name == 'test':
                need(cpu_parser(stdout) == [(980, 0, 18, 0, 0), (27, 0, 0, 0, 0)], 'exact CPU test inventory')
            if name == 'recorder-test':
                need(stdout == 'PASS: receipt and checkpoint replay checks; 29 malformed or conflicting operations rejected\n'
                     'PASS: retirement negative diagnostic policy; 8 malformed cases rejected\n'
                     and not stderr, 'recorder regression inventory')
        else:
            root, mutation = proof_root(name), mutations[name]
            recorded_root = Path(row['command'][-1])
            need(recorded_root.is_absolute() and str(recorded_root).endswith('/' + str(root)), 'resumed proof root')
            stage = Path(str(recorded_root)[:-len(str(root)) - 1])
            need(stage.parent == output and stage.name.startswith('sources-'), 'owned recorded source stage')
            need(row['command'] == legacy.command(verus, stage / root, mutation), 'exact proof command')
            observed = legacy.normalized(base, stdout, stderr, stage)
            scalar.check_verifier(observed['verus'])
            if mutation:
                check_negative(scalar, name, row['status'], observed)
            else:
                need(row['status'] == 0 and not observed['diagnostics'], 'clean terminal positive')
                need(legacy.same(observed['result'], {'encountered-error': False, 'encountered-vir-error': False,
                     'success': True, 'verified': 331 if name == 'raw-regression' else 929 if name == 'scalar-regression' else PAIRED_VERIFIED,
                     'errors': 0, 'is-verifying-entire-crate': True}), 'exact whole-root positive')
            return observed

    campaign = scalar.Campaign(output, source, names, base, legacy.same, validate, args.resume)

    def stop(name):
        if args.stop_after != name or name == 'release-build':
            return False
        source_roster(repo, inherited)
        need(before == {str(p): scalar.digest((repo / p).read_bytes()) for p in sorted(inputs)}, 'checkpoint source drift')
        print('CHECKPOINT: ' + name + ' checked; campaign incomplete', flush=True)
        return True

    for phase in ['before', 'after']:
        campaign.run('closure-' + phase, closure_command, 120, env)
        if stop('closure-' + phase):
            return
        if phase == 'after':
            break
        for name, mutation in cases:
            if name in campaign.rows:
                print(name, campaign.proofs[name]['result'], '(reused)', flush=True)
                if stop(name):
                    return
                continue
            with tempfile.TemporaryDirectory(prefix='sources-', dir=output) as temporary:
                stage = Path(temporary)
                for path in sources:
                    target = stage / path
                    target.parent.mkdir(parents=True, exist_ok=True)
                    data = (repo / path).read_bytes()
                    need(scalar.digest(data) == before[str(path)], 'staging source drift: ' + str(path))
                    target.write_bytes(legacy.mutated(data, mutation) if mutation and path == mutation[1] else data)
                policies = (set(legacy.POLICY_SOURCES) | (set(ancestor.NEW) - {ancestor.ROOT, ancestor.RAW, ancestor.BODY})
                            | set(previous.POLICY_NEW) | set(writer.POLICY_NEW) | set(settlement.POLICY_NEW)
                            | set(history.POLICY_NEW) | set(scalar.POLICY_NEW) | set(POLICY_NEW))
                for path in policies:
                    policy.scan(stage / path)
                for path, projection, count in [*legacy.PROJECTIONS, (ancestor.BODY, 'audited-unknown-wrappers.rs', 3),
                                                *previous.PROJECTIONS, *writer.PROJECTIONS, *settlement.PROJECTIONS,
                                                *scalar.PROJECTIONS, *PROJECTIONS]:
                    data = (stage / path).read_text()
                    need(data.count('macro_rules!') == count, 'macro roster')
                    stripped = stage / projection
                    stripped.write_text(data.replace('macro_rules!', ''))
                    policy.scan(stripped)
                campaign.run(name, legacy.command(verus, stage / proof_root(name), mutation), 610, env)
                print(name, campaign.proofs[name]['result'], flush=True)
            if stop(name):
                return
        need(legacy.same(campaign.proofs['positive-before'], campaign.proofs['positive-after']), 'matching whole-root positives')
    for name, command in cpu:
        campaign.run(name, command, 600, env)
        print(name, 'passed', flush=True)
        if stop(name):
            return
    after = {str(p): scalar.digest((repo / p).read_bytes()) for p in sorted(inputs)}
    source_roster(repo, inherited)
    need(before == after, 'source drift')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'results.json').write_text(json.dumps(campaign.proofs, indent=2) + '\n')


if __name__ == '__main__':
    if sys.argv[1:] == ['--selftest']:
        recorder_selftest()
    else:
        main()
