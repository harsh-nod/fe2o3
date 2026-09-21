#!/usr/bin/env python3
"""Qualify settlement admission and Unknown execution, not settlement commit/native refinement."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 338
SOURCE = 'context_version_journal_settlement_v1.rs'
SOURCE_SHA = 'ac6ed6a54da810c66a873f241050cfb837a3b3a7be260afe15956fdec1a72ea4'
PRIOR = Path('docs/evidence/dev-begin-custody-2026-09-21/check.py')
PRIOR_SHA = '8e002812aec34985fb93b06cebff3e55c84393840c4ba8260a9f025aed084159'
PREFIX = ('// Settlement admission and issuance composition; physical/native refinement remains separate.\n'
          'include!("context_version_journal_begin_custody_v1.rs");\n')


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned Begin custody checker')
    spec = importlib.util.spec_from_file_location('settlement_begin_custody', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.custody_checker = prior
    module.raw_checker.COUNT = COUNT
    return module


def source_names(module):
    prior = module.custody_checker
    return [*module.DEPENDENCIES, 'context_version_journal_enrollment_v1.rs', module.raw_checker.SOURCE,
            prior.SOURCE, prior.GUARDS, prior.LIFECYCLE]


def extra_paths(module):
    here = Path('crates/fe2o3-runtime-model/verus')
    prior = module.custody_checker
    return [here / n for n in (SOURCE, module.raw_checker.SOURCE, prior.SOURCE, prior.GUARDS, prior.LIFECYCLE)] + [PRIOR, prior.RAW_CHECK]


def mutations(base):
    rows = [
        ('header_context', 'retained_header_exec_v1',
         'if !same_key_exec_v1(key, writer.key) || key.context_generation != journal.context_generation',
         'if !same_key_exec_v1(key, writer.key)', 'result == retained_header_decision_v1'),
        ('member_lineage', 'retained_member_exec_v1',
         '|| allocation.content_lineage != member.prior_lineage || member.prior_lineage >= member.attempt_epoch',
         '|| allocation.content_lineage != member.prior_lineage', 'result == retained_member_decision_v1'),
        ('trailing_chain', 'retained_chain_exec_v1',
         'if head.is_some() { return Err(ReadErrorV1::InvalidState); }',
         'if head.is_some() { return Ok(()); }', 'result == retained_chain_decision_v1'),
        ('physical_headroom', 'settlement_return_exec_v1',
         'if writers > journal.writer_capacity || writers > free_storage',
         'if writers > free_storage && writers <= journal.writer_capacity && members <= journal.allocation_capacity\n'
         '        && members <= member_free_storage && count == 0 { return Ok(()); }\n'
         '    if writers > journal.writer_capacity || writers > free_storage', 'result == settlement_return_decision_v1'),
        ('scratch_busy', 'settlement_return_exec_v1',
         'if journal.scratch[index].is_some() { return Err(ReadErrorV1::InvalidState); }',
         'if journal.scratch[index].is_some() { return Ok(()); }', 'result == settlement_return_decision_v1'),
        ('evidence_identity', 'settlement_preflight_exec_v1',
         'return Err(ReadErrorV1::SettlementEvidenceMismatch);', 'return Err(ReadErrorV1::InvalidReference);',
         'result == settlement_preflight_decision_v1'),
        ('return_precedence', 'settlement_preflight_exec_v1',
         '    let (head, count, _) = match',
         '    if free_storage == 0 { return Err(ReadErrorV1::InvalidState); }\n    let (head, count, _) = match',
         'result == settlement_preflight_decision_v1'),
        ('unknown_error_frame', 'unknown_exec_v1',
         'Ok(value) => value, Err(error) => return Err(error),',
         'Ok(value) => value, Err(error) => { journal.reserved_count = 0; return Err(error); },',
         'unknown_execution_relation_v1'),
        ('unknown_result', 'unknown_exec_v1',
         '    Ok(())\n}', '    Err(ReadErrorV1::InvalidState)\n}', 'unknown_execution_relation_v1'),
        ('issued_result', 'unknown_issued_exec_v1',
         '    result\n}', '    if result.is_ok() { return Err(ReadErrorV1::InvalidState); }\n    result\n}',
         'unknown_issued_relation_v1'),
    ]
    return [base.Mutation(name, function, before, after, post, COUNT - 1)
            for name, function, before, after, post in rows]


def check_result(module, status, stdout, stderr, source, path, mutation):
    module.raw_checker.check_result(module, status, stdout, stderr, source, path, mutation)


def audit_source(module, candidate, folder):
    prior = module.custody_checker
    need(candidate.isascii() and candidate.startswith(PREFIX) and candidate.count('include!') == 1, 'exact settlement import')
    inherited = (folder / prior.SOURCE).read_bytes()
    need(digest(inherited) == prior.SOURCE_SHA, 'pinned Begin custody root')
    prior.audit_source(module, inherited.decode('ascii'), folder)
    path = folder / 'audited-settlement-body.rs'
    path.write_text('use vstd::prelude::*;\n' + candidate[len(PREFIX):], encoding='ascii')
    module.inherited().POLICY.scan(path)


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 321,
            'negative_cases': 10, 'conditional_settlement_issuance_preserved': True,
            'exact_settlement_preflight': True, 'exact_unknown_execution': True,
            'settlement_commit_loop_refined': False, 'physical_capacity_binding_proved': False,
            'rust_source_refinement_proved': False, 'native_or_performance_acceptance': False}


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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact settlement source')
    closure = repo / 'examples/row_softmax_v1/verify-verus-closure.sh'
    snapshots[closure] = closure.read_bytes()
    need(digest(snapshots[closure]) == 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c', 'closure checker')
    identities = {str(p): digest(data) for p, data in snapshots.items()}
    source = snapshots[here / SOURCE].decode('ascii')
    for mutation in mutations(base):
        module.postcondition_bounds(base, base.mutate(source, mutation), mutation)
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
        candidate = base.mutate(source, mutation) if mutation else source
        expected = {SOURCE: candidate.encode('ascii'), **{n: snapshots[here / n] for n in source_names(module)}}
        for filename, data in expected.items():
            (case / filename).write_bytes(data)
        audit_source(module, candidate, case)
        need(all((case / n).read_bytes() == data for n, data in expected.items()), 'exact generated input bytes')
        generated = {str(p): digest(p.read_bytes()) for p in case.glob('*.rs')}
        (case / 'sources.json').write_text(json.dumps(generated, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        path = case / SOURCE
        cmd = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4', str(path)]
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in generated.items()), 'generated source drift')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift after solver')
        check_result(module, status, stdout, stderr, candidate, path, mutation)
        completed[case.name] = {str(p): digest(p.read_bytes()) for p in case.rglob('*') if p.is_file()}
        print(case.name + ': PASS', flush=True)
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
