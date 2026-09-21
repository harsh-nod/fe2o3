#!/usr/bin/env python3
"""Qualify exact logical settlement execution and issued custody, not native refinement."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 360
SOURCE = 'context_version_journal_settlement_custody_v1.rs'
COMMIT = 'context_version_journal_settlement_commit_v1.rs'
SOURCE_SHA = 'a20e4ebfc1928e75e6ed9aa39ba2c0f655bfd3f861a8e3ae98c490f1bbb7100b'
COMMIT_SHA = '2fb2468064deef39bc6727f340611b3d6b716a7050b1f9bb0fac0ce4d37b61ed'
PRIOR = Path('docs/evidence/dev-settlement-admission-2026-09-21/check.py')
PRIOR_SHA = '1516f53f2d9d1d22bf17eece6a4103b606038a7b085845607520c4f2e93badae'
PREFIXES = {
    SOURCE: '// Compose exact logical settlement with issued custody; native admission remains separate.\ninclude!("context_version_journal_settlement_commit_v1.rs");\n',
    COMMIT: '// Exact logical settlement staging/commit; Rust storage and native refinement remain separate.\ninclude!("context_version_journal_settlement_v1.rs");\n',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned settlement admission checker')
    spec = importlib.util.spec_from_file_location('settlement_admission_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.commit_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def source_names(module):
    return [*module.commit_parent.source_names(module), module.commit_parent.SOURCE, COMMIT]


def extra_paths(module):
    here = Path('crates/fe2o3-runtime-model/verus')
    return [*module.commit_parent.extra_paths(module), PRIOR, here / SOURCE, here / COMMIT]


def target(mutation):
    return SOURCE if mutation is None or mutation.name == 'issued_result' else COMMIT


def mutations(base):
    end = '    proof { assert(journal.scratch@ =~= before.scratch@); }\n}'
    rows = [
        ('stage_plan', 'settlement_stage_exec_v1', '    }\n}',
         '    }\n    if count > 0 { journal.scratch.set(0, None); }\n    return;\n}',
         'final(journal).scratch@ == settlement_scratch_v1'),
        ('scratch_tail', 'settlement_commit_exec_v1', end,
         end[:-1] + '    if count < journal.scratch.len() { journal.scratch.set(count, None); }\n    return;\n}',
         'settlement_raw_success_relation_v1'),
        ('unlinked_member', 'settlement_commit_exec_v1', end,
         end[:-1] + '    if count == 0 && journal.members.len() > 0 { journal.members.set(0, None); }\n    return;\n}',
         'settlement_raw_success_relation_v1'),
        ('writer_retained', 'settlement_commit_exec_v1', '    journal.writers.set(writer.slot, None);',
         '    if count > 0 { journal.writers.set(writer.slot, None); }', 'settlement_raw_success_relation_v1'),
        ('writer_return', 'settlement_commit_exec_v1', '    journal.free.push(writer.slot);',
         '    journal.free.push(usize::MAX);', 'settlement_raw_success_relation_v1'),
        ('free_prefix', 'settlement_commit_exec_v1', end,
         end[:-1] + '    if journal.free.len() > 1 { journal.free.set(0, usize::MAX); }\n    return;\n}',
         'settlement_raw_success_relation_v1'),
        ('scalar_frame', 'settlement_commit_exec_v1', end,
         end[:-1] + '    journal.reserved_count = 0;\n    return;\n}', 'settlement_raw_success_relation_v1'),
        ('wrong_outcome', 'settlement_exec_v1',
         'settlement_commit_exec_v1(journal, writer, head, count, success, Ghost(before));',
         'settlement_commit_exec_v1(journal, writer, head, count, !success, Ghost(before));', 'settlement_execution_relation_v1'),
        ('rejection_frame', 'settlement_exec_v1', 'Ok(value) => value, Err(error) => return Err(error),',
         'Ok(value) => value, Err(error) => { journal.registration_watermark = 0; return Err(error); },',
         'settlement_execution_relation_v1'),
        ('issued_result', 'settlement_issued_exec_v1', '    result\n}',
         '    if result.is_ok() { return Err(ReadErrorV1::InvalidState); }\n    result\n}', 'settlement_issued_execution_v1'),
    ]
    return [base.Mutation(name, function, before, after, post, COUNT - 1)
            for name, function, before, after, post in rows]


def check_result(module, status, stdout, stderr, source, path, mutation):
    module.raw_checker.check_result(module, status, stdout, stderr, source, path, mutation)


def audit_sources(module, sources, folder):
    prior = module.commit_parent
    inherited = (folder / prior.SOURCE).read_bytes()
    need(digest(inherited) == prior.SOURCE_SHA, 'pinned settlement admission root')
    prior.audit_source(module, inherited.decode('ascii'), folder)
    for name, prefix in PREFIXES.items():
        source = sources[name]
        need(source.isascii() and source.startswith(prefix) and source.count('include!') == 1, 'exact settlement execution import')
        path = folder / ('audited-' + name)
        path.write_text('use vstd::prelude::*;\n' + source[len(prefix):], encoding='ascii')
        module.inherited().POLICY.scan(path)


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 338,
            'negative_cases': 10, 'exact_raw_settlement_execution': True,
            'settlement_commit_loop_refined': True, 'issued_producer_preservation': True,
            'constructor_settlement_witness': True, 'mixed_reader_constructor_witness': False,
            'physical_capacity_binding_proved': False, 'rust_source_refinement_proved': False,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA and digest(snapshots[here / COMMIT]) == COMMIT_SHA, 'exact settlement roots')
    closure = repo / 'examples/row_softmax_v1/verify-verus-closure.sh'
    snapshots[closure] = closure.read_bytes()
    need(digest(snapshots[closure]) == 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c', 'closure checker')
    identities = {str(p): digest(data) for p, data in snapshots.items()}
    sources = {name: snapshots[here / name].decode('ascii') for name in PREFIXES}
    for mutation in mutations(base):
        module.postcondition_bounds(base, base.mutate(sources[target(mutation)], mutation), mutation)
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
        candidates = dict(sources)
        if mutation:
            candidates[target(mutation)] = base.mutate(candidates[target(mutation)], mutation)
        expected = {n: snapshots[here / n] for n in source_names(module)}
        expected.update({n: s.encode('ascii') for n, s in candidates.items()})
        for filename, data in expected.items():
            (case / filename).write_bytes(data)
        audit_sources(module, candidates, case)
        need(all((case / n).read_bytes() == data for n, data in expected.items()), 'exact generated input bytes')
        generated = {str(p): digest(p.read_bytes()) for p in case.glob('*.rs')}
        (case / 'sources.json').write_text(json.dumps(generated, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        cmd = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4', str(case / SOURCE)]
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in generated.items()), 'generated source drift')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift after solver')
        check_result(module, status, stdout, stderr, candidates[target(mutation)], case / target(mutation), mutation)
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
