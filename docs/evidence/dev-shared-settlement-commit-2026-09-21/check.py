#!/usr/bin/env python3
"""Qualify the shared production commit; allocator and whole-runtime refinement remain separate."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 398
SOURCE = 'context_shared_settlement_commit_v1.rs'
SOURCE_SHA = '570000f0fae33620427225d5888eeb113756c82227db231577a1c82886b6a49d'
BODY = Path('src/context_version_journal/settlement_commit_body.rs')
BODY_SHA = '0491e8c5c4405a26685b1e449ad5e1a81d464d57e44afcbba5b123b9dfdefa6d'
ADAPTER = Path('src/context_version_journal/settlement_commit.rs')
ADAPTER_SHA = 'd8283836b974a6dd06b622b26099adda300ebf3e1f57dee1c168b5e8fa2bfc97'
PRIOR = Path('docs/evidence/dev-shared-settlement-scratch-2026-09-21/check.py')
PRIOR_SHA = '6f300cc65a1c44cc29b1b80aa303fc39be3b073d3aebcb58f0e781a5c9cea43d'
PREFIX = ('// Shared settlement commit; physical storage and whole-wrapper refinement remain separate.\n'
          'include!("context_shared_settlement_scratch_v1.rs");\n'
          'include!("../src/context_version_journal/settlement_commit_body.rs");\n')
MACRO = 'settlement_commit_body'
FUNCTION = 'shared_settlement_commit_v1'
BODY_PREFIX = '// Shared executable settlement commit. Loop annotations carry no runtime code.\n'
PARAMETERS = ('($syntax:ident, $journal:ident, $writer:ident, $count:ident, $success:ident,\n'
              '     $index:ident, [$($annotations:tt)*])')
SCRATCH = 'journal.scratch@ =~= settlement_scratch_v1(before, head, count, count as nat, index as nat)'
ALLOCATIONS = 'journal.allocations@ == settlement_allocations_prefix_v1(before, head, success, index as nat)'
MEMBERS = 'journal.members@ == settlement_members_prefix_v1(before, head, index as nat)'
RETURNS = 'journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, index as nat)'
CALL = '''settlement_commit_body!(verus_exec_expr, journal, writer, count, success, index, [
        invariant index <= count, settlement_storage_ready_v1(before, head, count), writer.slot < before.writers@.len(),
            settlement_commit_frame_v1(before, *journal), journal.writers == before.writers, journal.free == before.free,
            journal.scratch@ =~= settlement_scratch_v1(before, head, count, count as nat, index as nat),
            journal.allocations@ == settlement_allocations_prefix_v1(before, head, success, index as nat),
            journal.members@ == settlement_members_prefix_v1(before, head, index as nat),
            journal.member_free@ =~= before.member_free@ + settlement_slots_v1(before, head, index as nat),
            journal.members@.len() == before.members@.len(), journal.allocations@.len() == before.allocations@.len(),
            forall|a: int| 0 <= a < before.allocations@.len() ==>
                (#[trigger] journal.allocations@[a]).is_some() == before.allocations@[a].is_some(),
            index < count ==> settlement_plan_ready_v1(before, head, index as nat),
        decreases count - index,
    ])'''
RUST = [Path('crates/fe2o3-runtime-model') / p for p in (
    BODY, ADAPTER, 'src/context_version_journal/settlement_commit_tests.rs')]
Mutation = namedtuple('Mutation', 'name macro function before after postcondition invariant', defaults=[None])


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned shared scratch checker')
    spec = importlib.util.spec_from_file_location('shared_commit_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.shared_commit_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.shared_commit_parent.extra_paths(module), PRIOR,
            Path('crates/fe2o3-runtime-model/verus') / SOURCE, *RUST]


def macro_bounds(body):
    prefix = BODY_PREFIX + 'macro_rules! ' + MACRO + ' {\n    ' + PARAMETERS + ' => {\n        $syntax!({\n'
    suffix = '        })\n    };\n}\n'
    need(body.isascii() and body.startswith(prefix) and body.endswith(suffix)
         and body.count(suffix) == 1, 'exact commit macro envelope')
    return {'definition': len(BODY_PREFIX), 'start': len(prefix), 'end': len(body) - len(suffix),
            'block_start': len(prefix) - 2, 'block_end': len(body) - len(suffix) + 9}


def mutations(_base=None):
    changes = [
        ('lineage', 'allocation.content_lineage = plan.attempt_epoch;',
         'allocation.content_lineage = plan.prior_lineage;', ALLOCATIONS),
        ('no_effect', 'if $success {', 'if true {', ALLOCATIONS),
        ('backlink', 'allocation.pending_member = None;', 'let _ = allocation.pending_member;', ALLOCATIONS),
        ('allocation_frame', 'allocation.pending_member = None;',
         'allocation.pending_member = None; allocation.attempt_epoch = 0;', ALLOCATIONS),
        ('member_clear', '$journal.members[plan.member_slot] = None;', 'let _ = plan.member_slot;', MEMBERS),
        ('member_return', '$journal.member_free.push(plan.member_slot);', 'let _ = plan.member_slot;', RETURNS),
        ('scratch_clear', '.take()', '', SCRATCH),
        ('writer_clear', '$journal.writers[$writer.slot] = None;', 'let _ = $writer.slot;', None),
        ('writer_return', '$journal.free.push($writer.slot);', '$journal.free.push(0);', None),
        ('journal_frame', '$journal.free.push($writer.slot);',
         '$journal.free.push($writer.slot); $journal.reserved_count = 0;', None),
    ]
    return [Mutation(name, MACRO, FUNCTION, before, after, 'settlement_raw_success_relation_v1', invariant)
            for name, before, after, invariant in changes]


def candidate(body, mutation):
    if mutation is None:
        return body
    bounds = macro_bounds(body)
    begin, end = bounds['start'], bounds['end']
    inner = body[begin:end]
    need(mutation.before and mutation.before != mutation.after and inner.count(mutation.before) == 1, 'unique commit mutation')
    offset = inner.index(mutation.before)
    changed = inner[:offset] + mutation.after + inner[offset + len(mutation.before):]
    need(changed[:offset] + mutation.before + changed[offset + len(mutation.after):] == inner,
         'reversible commit mutation at exact offset')
    return body[:begin] + changed + body[end:]


def audit_adapter(adapter):
    need(digest(adapter) == ADAPTER_SHA, 'exact commit Rust identity adapter and empty annotations')


def audit_sources(module, source, body, folder):
    prior = module.shared_commit_parent
    inherited_source = (folder / 'verus' / prior.SOURCE).read_text(encoding='ascii')
    inherited_body = (folder / prior.BODY).read_text(encoding='ascii')
    need(digest(inherited_source.encode()) == prior.SOURCE_SHA and digest(inherited_body.encode()) == prior.BODY_SHA,
         'pinned inherited scratch source bodies')
    prior.audit_sources(module, inherited_source, inherited_body, folder)
    need(digest(source.encode()) == SOURCE_SHA and source.startswith(PREFIX)
         and source.count('include!') == 2 and source.count(CALL) == 1, 'exact commit root and proof-only annotations')
    policy = module.inherited().POLICY
    path = folder / 'verus/audited-shared-commit-root.rs'
    path.write_text(source[len(PREFIX):], encoding='ascii')
    policy.scan(path)
    bounds = macro_bounds(body)
    path = folder / 'verus/audited-settlement_commit_body.rs'
    path.write_text(body[bounds['start']:bounds['end']], encoding='ascii')
    policy.scan(path)


def stage(module, snapshots, folder, mutation):
    here = module.HERE
    audit_adapter(snapshots[here.parent / ADAPTER])
    module.shared_commit_parent.stage(module, snapshots, folder, None)
    source = snapshots[here / SOURCE].decode('ascii')
    body = candidate(snapshots[here.parent / BODY].decode('ascii'), mutation)
    (folder / 'verus' / SOURCE).write_text(source, encoding='ascii')
    (folder / BODY).write_text(body, encoding='ascii')
    audit_sources(module, source, body, folder)
    return source, body, {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def check_result(module, status, stdout, stderr, source, body, path, mutation):
    base, retained, spans_helper = module.inherited().BASE, module.scratch_parent, module.retained_parent
    if mutation is None:
        module.raw_checker.check_result(module, status, stdout, stderr, source, path, None)
        return
    need(type(status) is int and status == 1, 'expected normal commit negative exit')
    expected = {'encountered-error': True, 'encountered-vir-error': False, 'success': False,
                'verified': COUNT - 1, 'errors': 1, 'is-verifying-entire-crate': True}
    need(same(base.unique_json(stdout)['verification-results'], expected), 'exact whole-crate commit negative result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    need(len(diagnostics) in (1, 2), 'one intended commit negative and optional footer')
    if len(diagnostics) == 2:
        footer = diagnostics[1]
        need(footer['$message_type'] == 'diagnostic' and footer['level'] == 'error'
             and footer['message'] == 'aborting due to 1 previous error' and footer['code'] is None
             and footer['spans'] == [] and footer['children'] == [], 'exact commit negative footer')
    error = diagnostics[0]
    message = 'invariant not satisfied at end of loop body' if mutation.invariant else 'postcondition not satisfied'
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error' and error['message'] == message
         and error['code'] is None and error['children'] == [], 'intended commit obligation failure')
    if mutation.invariant:
        begin, end = retained.invocation(source, mutation.macro)
        need(source[begin:end].count(mutation.invariant) == 1, 'unique intended commit invariant')
        start = source.index(mutation.invariant, begin)
        wanted = spans_helper.exact_span(source, path, start, start + len(mutation.invariant), True)
        need(same(error['spans'], [wanted]), 'exact commit loop invariant span')
        return
    begin, end = module.postcondition_bounds(base, source, mutation)
    primary = spans_helper.exact_span(source, path, begin, end, True, 'failed this postcondition')
    _, begin, end = base.function_bounds(source, mutation.function)
    exit_span = spans_helper.exact_span(source, path, begin, end, False, 'at the end of the function body')
    spans = error['spans']
    need(len(spans) == 2, 'one commit postcondition and one wrapper exit')
    primaries = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(same(primaries, [primary]) and same(exits, [exit_span]), 'exact commit postcondition and wrapper exit')


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 392,
            'negative_cases': 10, 'loop_invariant_negatives': 7, 'postcondition_negatives': 3,
            'shared_production_commit': True, 'unchanged_raw_commit_precondition': True,
            'logical_settlement_composition': True, 'issued_custody_preservation': True,
            'allocator_or_machine_refinement': False, 'full_runtime_refinement': False,
            'native_or_performance_acceptance': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    repo, verus = args.repo.resolve(), args.verus.resolve()
    module = load(repo)
    inherited = module.inherited()
    base, here = inherited.BASE, module.HERE
    for signum in base.SIGNALS:
        signal.signal(signum, base.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, base.SIGNALS)
    pins = {here / name: pin for name, pin in module.DEPENDENCIES.items()}
    pins[here / 'context_version_journal_enrollment_v1.rs'] = 'CONTEXT_VERSION_JOURNAL_ENROLLMENT_SHA256'
    pins[verus] = 'VERUS_SHA256'
    pins[here / 'pins/VERUS_CLOSURE_MANIFEST'] = 'VERUS_CLOSURE_MANIFEST_SHA256'
    for helper, pin in [(module, 'JOURNAL_ENROLLMENT_CHECKER_SHA256'), (inherited, 'PRODUCER_JOURNAL_ISSUANCE_CHECKER_SHA256'),
        (inherited.PRODUCER, 'PRODUCER_READ_INVARIANT_CHECKER_SHA256'), (inherited.INVARIANT, 'READ_INVARIANT_CHECKER_SHA256'),
        (inherited.COMMIT, 'READ_COMMIT_CHECKER_SHA256'), (inherited.PREFLIGHT, 'READ_PREFLIGHT_CHECKER_SHA256'),
        (base, 'JOURNAL_ISSUANCE_CHECKER_SHA256'), (inherited.POLICY, 'PROOF_SOURCE_CHECKER_SHA256')]:
        pins[Path(helper.__file__)] = pin
    snapshots = inherited.PRODUCER.pinned_snapshots(pins)
    for path in [*(repo / p for p in extra_paths(module)), Path(__file__).resolve(), verus.parent / 'rust_verify', verus.parent / 'z3']:
        snapshots[path] = path.read_bytes()
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA and digest(snapshots[here.parent / BODY]) == BODY_SHA, 'exact commit sources')
    audit_adapter(snapshots[here.parent / ADAPTER])
    closure = repo / 'examples/row_softmax_v1/verify-verus-closure.sh'
    snapshots[closure] = closure.read_bytes()
    need(digest(snapshots[closure]) == 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c', 'closure checker')
    identities = {str(p): digest(data) for p, data in snapshots.items()}
    output = args.output.resolve()
    output.mkdir()
    (output / 'inputs-before.json').write_text(json.dumps(identities, indent=2) + '\n')
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo', 'VERUS_Z3_PATH': str(verus.parent / 'z3')}
    (output / 'environment.json').write_text(json.dumps(env, indent=2) + '\n')
    command = ['/bin/sh', str(closure), str(verus.parent), str(here / 'pins/VERUS_CLOSURE_MANIFEST')]
    need(base.run_owned(command, 120, output / 'closure-before', env)[0] == 0, 'closure before')
    cases = [('positive_before', None), *[(m.name, m) for m in mutations(base)], ('positive_after', None)]
    completed = {}
    for name, mutation in cases:
        case = output / name
        case.mkdir()
        source, body, generated = stage(module, snapshots, case, mutation)
        generated_hashes = {str(case / p): digest(data) for p, data in generated.items()}
        (case / 'sources.json').write_text(json.dumps(generated_hashes, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        path = case / 'verus' / SOURCE
        cmd = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4', str(path)]
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in generated_hashes.items()), 'generated source drift')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift after solver')
        check_result(module, status, stdout, stderr, source, body, path, mutation)
        completed[name] = {str(p): digest(p.read_bytes()) for p in case.rglob('*') if p.is_file()}
        print(name + ': PASS', flush=True)
    need(base.run_owned(command, 120, output / 'closure-after', env)[0] == 0, 'closure after')
    after = {p: digest(Path(p).read_bytes()) for p in identities}
    need(after == identities, 'final identities')
    for name, frozen in completed.items():
        need({str(p): digest(p.read_bytes()) for p in (output / name).rglob('*') if p.is_file()} == frozen, 'completed case drift')
    (output / 'completed-cases.json').write_text(json.dumps(completed, indent=2) + '\n')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'report.json').write_text(json.dumps(report(), indent=2) + '\n')
    print(json.dumps(report()), flush=True)


if __name__ == '__main__':
    main()
