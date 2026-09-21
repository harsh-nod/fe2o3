#!/usr/bin/env python3
"""Qualify the actual-typed settlement bodies and their exact historical boundary."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 478
SOURCE = 'context_journal_settlement_execution_v1.rs'
PRIOR = Path('docs/evidence/dev-journal-unknown-execution-2026-09-21/check.py')
PRIOR_SHA = '6c37266657cc3fe4d143f41e0f7f122f8bd707a412577246896d315d1038721b'
CRATE = Path('crates/fe2o3-runtime-model')
BODY = Path('src/context_version_journal/settlement_return_body.rs')
SCRATCH = Path('src/context_version_journal/settlement_scratch_bodies.rs')
COMMIT = Path('src/context_version_journal/settlement_commit_body.rs')
EXEC = Path('verus/context_journal_settlement_bodies_v1.rs')
SPEC = Path('verus/context_journal_settlement_historical_v1.rs')
CUSTODY = Path('verus/context_version_journal_begin_custody_v1.rs')
LEGACY_CUSTODY = Path('docs/evidence/dev-begin-custody-2026-09-21/proof/positive_before/context_version_journal_begin_custody_v1.rs')
LEGACY_CUSTODY_SHA = '27b87b4140f85a8d7cdb5807407e642884dc9f1cef2d7f990c03992d3aa67cac'
PINS = {
    str(CUSTODY): "81278cfcb574fa1d03a9e8eed329e792a37b7897843bd586fa766be2fde271d5",
    "verus/context_journal_settlement_execution_v1.rs": "e7c6497dff56f963797ac8df40936b2e253bfc007ad79183c62eb4fb0b353685",
    "verus/context_journal_settlement_decisions_v1.rs": "bc8e11be1222966a100a154b31cad9492d3b9e7ca0a8bd78f444c88475147dba",
    "verus/context_journal_settlement_bodies_v1.rs": "250392902dae7b5e115f7f2c2136bb1c92abb94f54bc13e306b9211b3c88c205",
    "verus/context_journal_settlement_historical_v1.rs": "3c733fa43c320dd51696b953a7171c39cc41ef5f6b30fdb5d89200d258690d15",
    "src/context_version_journal/settlement_storage.rs": "45ad5168130ced418f011166fa112a07cf8f2c171028cf6656d4e80026f829bd",
    "src/context_version_journal/settlement_storage_declarations.rs": "bc7a9d53ea4a7acd0f0bfb96a06b49c9725d5e047bb689608e2fa47f471b8033"
}
SOURCE_SHA = PINS['verus/' + SOURCE]
Mutation = namedtuple('Mutation', 'name path macro function before after clause kind verified anchor', defaults=[None])


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned production Unknown checker')
    spec = importlib.util.spec_from_file_location('settlement_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.settlement_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.settlement_parent.extra_paths(module), PRIOR, LEGACY_CUSTODY, *(CRATE / p for p in PINS)]


def custody_transition(original, current):
    need(digest(original) == LEGACY_CUSTODY_SHA, 'authenticated original custody proof')
    old, new = original.decode('ascii'), current.decode('ascii')
    name = 'pub proof fn begin_preserves_pending_custody_v1('
    marker = '#[verifier::spinoff_prover]\n' + name
    need(old.count(marker) == new.count(marker) == 1, 'unique custody composition theorem')
    first = old.index(marker)
    last = old.index('\n}\n', first) + 2
    prefix, suffix = old[:first], old[last:]
    need(new.startswith(prefix) and new.endswith(suffix), 'unchanged surrounding custody source')
    old_header = old[old.index(name):old.index('\n{', old.index(name))]
    new_header = new[new.index(name):new.index('\n{', new.index(name))]
    need(new_header == old_header, 'unchanged custody theorem contract')
    replacement = new[len(prefix):len(new) - len(suffix)]
    need(replacement.count('pub proof fn ') == 4, 'exact custody proof decomposition roster')
    for kind in ('allocation', 'member', 'writer'):
        need(replacement.count('pub proof fn begin_preserves_' + kind + '_custody_v1(') == 1,
             'named derived custody helper')
    return replacement


def mutations(_base=None):
    stage = ('forall|i: int| 0 <= i < journal.scratch@.len() ==> begin_plan_slot_view(#[trigger] journal.scratch@[i])\n'
             '                == settlement_scratch_v1(before, initial, count, index as nat, 0)[i]')
    scan = ('settlement_scratch_scan_v1(journal_view(*journal), count, 0)\n'
            '                == settlement_scratch_scan_v1(journal_view(*journal), count, index as nat)')
    allocations = 'journal_view(*journal).allocations =~= settlement_allocations_prefix_v1(before, head, success, index as nat)'
    returns = 'journal.member_free@ =~= before.member_free + settlement_slots_v1(before, head, index as nat)'
    scratch = ('forall|i: int| 0 <= i < journal.scratch@.len() ==> begin_plan_slot_view(#[trigger] journal.scratch@[i])\n'
               '                == settlement_scratch_v1(before, head, count, count as nat, index as nat)[i]')
    result = [
        Mutation('writer_storage', BODY, 'settlement_return_admission_body', 'shared_settlement_storage_exec_v1',
            '|| writer_returns > $storage.writer_storage', '|| false', None, 'post', 0),
        Mutation('scratch_reject', SCRATCH, 'settlement_scratch_scan_body', 'shared_settlement_scratch_scan_v1',
            'return Err(ReadErrorV1::InvalidState)', 'return Ok(())', None, 'post', 1),
        Mutation('scratch_skip', SCRATCH, 'settlement_scratch_scan_body', 'shared_settlement_scratch_scan_v1',
            'if $journal.scratch[$index].is_some() {', 'if false {', scan, 'invariant', 1),
    ]
    for name, before, after in [
        ('stage_slot', 'member_slot: slot,', 'member_slot: 0,'),
        ('stage_lineage', 'prior_lineage: member.prior_lineage,', 'prior_lineage: 0,'),
        ('stage_epoch', 'attempt_epoch: member.attempt_epoch,', 'attempt_epoch: 0,'),
    ]:
        result.append(Mutation(name, SCRATCH, 'settlement_scratch_stage_body', 'shared_settlement_scratch_stage_v1',
                               before, after, stage, 'invariant', 1))
    for name, before, after, clause in [
        ('commit_lineage', 'allocation.content_lineage = plan.attempt_epoch;', 'allocation.content_lineage = plan.prior_lineage;', allocations),
        ('commit_no_effect', 'if $success {', 'if true {', allocations),
        ('commit_backlink', 'allocation.pending_member = None;', 'let _ = allocation.pending_member;', allocations),
        ('commit_member_return', '$journal.member_free.push(plan.member_slot);', 'let _ = plan.member_slot;', returns),
        ('commit_scratch_clear', '.take()', '', scratch),
        ('commit_writer_return', '$journal.free.push($writer.slot);', '$journal.free.push(0);', None),
    ]:
        result.append(Mutation(name, COMMIT, 'settlement_commit_body', 'shared_settlement_commit_v1',
                               before, after, clause, 'invariant' if clause else 'post', 1))
    result.extend([
        Mutation('rejection_identity', SPEC, None, 'settlement_historical_factorization',
            'Err(_) => after == before,', 'Err(_) => true,', None, 'post', 0, 'spec fn historical_settlement_identity('),
        Mutation('success_identity', SPEC, None, 'settlement_historical_factorization',
            'Ok(_) => after.allocation_free == before.allocation_free,', 'Ok(_) => true,', None, 'post', 0, 'spec fn historical_settlement_identity('),
    ])
    return result


def bounds(body, mutation):
    anchor = 'macro_rules! ' + mutation.macro + ' {' if mutation.macro else mutation.anchor
    need(body.count(anchor) == 1, 'unique mutation definition')
    start = body.index(anchor)
    end = body.index('\n}\n', start) + 2
    return start, end


def stage(module, snapshots, folder, mutation):
    for path, pin in PINS.items():
        need(digest(snapshots[module.HERE.parent / path]) == pin, 'pinned settlement source: ' + path)
    original = snapshots[module.HERE.parent.parent.parent / LEGACY_CUSTODY]
    addition = custody_transition(original, snapshots[module.HERE.parent / CUSTODY])
    historical = dict(snapshots)
    historical[module.HERE.parent / CUSTODY] = original
    module.settlement_parent.stage(module, historical, folder, None)
    for path in PINS:
        target = folder / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(snapshots[module.HERE.parent / path])
        if path.startswith('verus/') and path not in ('verus/' + SOURCE, str(CUSTODY)):
            module.inherited().POLICY.scan(target)
    transition = folder / 'verus/audited-begin-custody-transition.rs'
    transition.write_text(addition, encoding='ascii')
    module.inherited().POLICY.scan(transition)
    if mutation:
        target = folder / mutation.path
        body = target.read_text(encoding='ascii')
        start, end = bounds(body, mutation)
        fragment = body[start:end]
        need(fragment.count(mutation.before) == 1 and mutation.before != mutation.after, 'unique scoped mutation')
        target.write_text(body[:start] + fragment.replace(mutation.before, mutation.after, 1) + body[end:], encoding='ascii')
    generated = {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}
    return (folder / 'verus' / SOURCE).read_text(encoding='ascii'), generated, generated


def solver_command(verus, path, mutation):
    command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4']
    if mutation:
        command.extend(['--verify-only-module', 'production', '--verify-function', mutation.function])
    return [*command, str(path)]


def check_result(module, status, stdout, stderr, source, generated, path, mutation):
    base = module.inherited().BASE
    expected = {'encountered-error': bool(mutation), 'encountered-vir-error': False,
                'verified': mutation.verified if mutation else COUNT, 'errors': 1 if mutation else 0,
                'is-verifying-entire-crate': not bool(mutation)}
    if mutation is None:
        expected['success'] = True
    need(type(status) is int and status == (1 if mutation else 0), 'normal expected solver exit')
    need(same(base.unique_json(stdout)['verification-results'], expected), 'exact verification result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        need(not diagnostics, 'clean whole-crate positive')
        return
    need(len(diagnostics) == 3, 'exact diagnostic roster')
    note, error, footer = diagnostics
    for row, level, message in [(note, 'note', 'verifying module production (selected functions)'),
                                (footer, 'error', 'aborting due to 1 previous error')]:
        need(row['$message_type'] == 'diagnostic' and row['level'] == level and row['message'] == message
             and row['code'] is None and row['spans'] == [] and row['children'] == [], 'exact scope note and footer')
    message = 'invariant not satisfied at end of loop body' if mutation.kind == 'invariant' else 'postcondition not satisfied'
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error'
         and error['message'] == message and error['code'] is None and error['children'] == [], 'intended obligation failure')
    proof_file = EXEC if mutation.macro else SPEC
    proof = generated[proof_file].decode('ascii')
    proof_path = path.parent.parent / proof_file
    helper = module.retained_parent
    anchor = 'fn ' + mutation.function + '('
    need(proof.count(anchor) == 1, 'unique target proof function')
    function = proof.index(anchor)
    body_start = proof.index('\n{', function) + 1
    body_end = proof.index('\n}\n', body_start) + 2
    if mutation.kind == 'invariant':
        begin, end = module.scratch_parent.invocation(proof, mutation.macro)
        need(proof[begin:end].count(mutation.clause) == 1, 'unique intended invariant')
        first = proof.index(mutation.clause, begin)
        wanted = helper.exact_span(proof, proof_path, first, first + len(mutation.clause), True)
        need(same(error['spans'], [wanted]), 'exact settlement invariant')
        return
    first = proof.index('ensures ', function) + len('ensures ')
    last = proof.rindex(',\n', first, body_start)
    primary = helper.exact_span(proof, proof_path, first, last, True, 'failed this postcondition')
    spans = error['spans']
    need(len(spans) == 2 and all(type(s['is_primary']) is bool for s in spans), 'exact typed span roster')
    primaries = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(same(primaries, [primary]) and len(exits) == 1, 'exact settlement postcondition')
    actual = exits[0]
    if not mutation.macro:
        expression = 'match logical::settlement_preflight_decision_v1(before, writer, evidence, free_storage, member_free_storage) {'
        first = proof.index(expression, body_start)
        last = proof.index('\n    }\n}', first) + len('\n    }')
        wanted = helper.exact_span(proof, proof_path, first, last, False, 'at the end of the function body')
    elif mutation.path == COMMIT:
        wanted = helper.exact_span(proof, proof_path, body_start, body_end, False, 'at the end of the function body')
    else:
        body = generated[mutation.path].decode('ascii')
        macro_path = path.parent / '..' / mutation.path
        begin, end = bounds(body, mutation)
        if mutation.path == BODY:
            first = body.index('=> {{', begin) + len('=> {')
            last = body.index('    }};', first) + 5
            label = 'at the end of the function body'
        else:
            text = 'return Ok(())'
            need(body[begin:end].count(text) == 1, 'unique scratch rejection exit')
            first = body.index(text, begin)
            last = first + len(text)
            label = 'at this exit'
        call_begin, call_end = module.scratch_parent.invocation(proof, mutation.macro)
        definition = 'macro_rules! ' + mutation.macro
        expansion = {'span': helper.exact_span(proof, proof_path, call_begin, call_end),
                     'macro_decl_name': mutation.macro + '!',
                     'def_site_span': helper.exact_span(body, macro_path, begin, begin + len(definition))}
        wanted = helper.exact_span(body, macro_path, first, last, False, label, expansion)
    need(same(actual, wanted), 'exact settlement exit and expansion tree')


def report():
    return {'development_checks_passed': True, 'whole_crate_positive_obligations': COUNT,
            'targeted_negative_controls': 14, 'negative_verification_scope': 'one named production-module function per control',
            'executable_controls': 12, 'identity_specification_controls': 2,
            'actual_typed_settlement_bodies': True, 'ordered_alias_semantics': True,
            'historical_identity_factorization': True, 'physical_storage_refinement': False,
            'public_wrapper_refinement': False, 'full_runtime_refinement': False,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact settlement root')
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
        status, stdout, stderr = base.run_owned(solver_command(verus, path, mutation), 190, case / 'solver', env)
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
