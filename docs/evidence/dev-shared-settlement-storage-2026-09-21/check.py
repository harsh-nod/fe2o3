#!/usr/bin/env python3
"""Qualify a shared scalar Rust/Verus body, not allocator or full Rust refinement."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 367
SOURCE = 'context_settlement_shared_storage_v1.rs'
SOURCE_SHA = '69139e189c4cf3cc3cc252e522bf73f1d5d19aa8979f9423a2b96e86fc943809'
BODY = Path('src/context_version_journal/settlement_return_body.rs')
BODY_SHA = '4b8cf3f0e8e0282a9bdf2ae7e53713f752207e607d0302b35fe55bc34a9bb89f'
PRIOR = Path('docs/evidence/dev-settlement-commit-2026-09-21/check.py')
PRIOR_SHA = '9180e86e084c4e898942375028669d85893b576c1ead4c5cd7425f2b430d7a4b'
PREFIX = ('// Shared executable storage admission; allocator and full Rust refinement remain separate.\n'
          'include!("context_version_journal_settlement_custody_v1.rs");\n'
          'include!("../src/context_version_journal/settlement_return_body.rs");\n')
MACRO_PREFIX = ('// Expanded unchanged by ordinary Rust and the settlement storage Verus proof.\n'
                'macro_rules! settlement_return_admission_body {\n'
                '    ($storage:ident, $count:ident, $invalid:path) => {{\n')
MACRO_SUFFIX = '    }};\n}\n'
INVOCATION = 'settlement_return_admission_body!(storage, count, ReadErrorV1::InvalidState)'
RUST = [Path('crates/fe2o3-runtime-model') / p for p in (
    BODY, 'src/context_version_journal.rs', 'src/context_version_journal/settlement.rs',
    'src/context_version_journal/settlement_storage.rs', 'src/context_version_journal/settlement_tests.rs')]


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned settlement execution checker')
    spec = importlib.util.spec_from_file_location('shared_storage_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.storage_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.storage_parent.extra_paths(module), PRIOR,
            Path('crates/fe2o3-runtime-model/verus') / SOURCE, *RUST]


def mutations(base):
    scalar = 'shared_settlement_storage_exec_v1'
    post = 'result == shared_settlement_storage_decision_v1'
    rows = [('writer_add', scalar, 'writer_free_len.checked_add(1)', 'writer_free_len.checked_add(0)', post),
            ('member_add', scalar, 'member_free_len.checked_add($count)', 'member_free_len.checked_add(0)', post)]
    for name, before in [('writer_limit', 'writer_returns > $storage.writer_limit'),
                         ('writer_storage', 'writer_returns > $storage.writer_storage'),
                         ('member_limit', 'member_returns > $storage.member_limit'),
                         ('member_storage', 'member_returns > $storage.member_storage'),
                         ('scratch_len', '$count > $storage.scratch_len')]:
        rows.append((name, scalar, before, 'false', post))
    rows.append(('success_result', scalar, '        Ok(())', '        Err($invalid)', post))
    anchor = '    match shared_settlement_storage_exec_v1(&storage, count) {'
    for name, writer, member in [('writer_observation', 'member_free_storage', 'member_free_storage'),
                                 ('member_observation', 'free_storage', 'free_storage')]:
        # Isolate spurious rejection so the unchanged normal path still establishes
        # the scratch-loop invariant; a direct swap also fails that invariant.
        wrong = ('    let wrong_observation = SettlementReturnStorageV1 {\n'
                 '        writer_free_len: journal.free.len(), member_free_len: journal.member_free.len(),\n'
                 f'        writer_limit: journal.writer_capacity, writer_storage: {writer},\n'
                 f'        member_limit: journal.allocation_capacity, member_storage: {member},\n'
                 '        scratch_len: journal.scratch.len(),\n'
                 '    };\n'
                 '    match shared_settlement_storage_exec_v1(&wrong_observation, count) {\n'
                 '        Ok(_) => {}, Err(error) => return Err(error),\n'
                 '    };\n')
        rows.append((name, 'shared_settlement_return_exec_v1', anchor, wrong + anchor,
                     'result == settlement_return_decision_v1'))
    return [base.Mutation(name, function, before, after, post, COUNT - 1)
            for name, function, before, after, post in rows]


def macro_mutation(mutation):
    return mutation is not None and mutation.function == 'shared_settlement_storage_exec_v1'


def candidates(base, source, body, mutation):
    if macro_mutation(mutation):
        need(body.startswith(MACRO_PREFIX) and body.endswith(MACRO_SUFFIX), 'single exact macro envelope')
        inner = body[len(MACRO_PREFIX):-len(MACRO_SUFFIX)]
        need(inner.count(mutation.before) == 1 and mutation.before != mutation.after, 'unique macro mutation')
        changed = inner.replace(mutation.before, mutation.after)
        need(changed.count(mutation.after) == 1 and changed.replace(mutation.after, mutation.before) == inner,
             'reversible macro mutation')
        body = MACRO_PREFIX + changed + MACRO_SUFFIX
    elif mutation:
        source = base.mutate(source, mutation)
    return source, body


def audit_sources(module, source, body, folder):
    prior = module.storage_parent
    inherited = {n: (folder / 'verus' / n).read_text(encoding='ascii') for n in prior.PREFIXES}
    need(digest(inherited[prior.SOURCE].encode()) == prior.SOURCE_SHA
         and digest(inherited[prior.COMMIT].encode()) == prior.COMMIT_SHA, 'pinned execution roots')
    prior.audit_sources(module, inherited, folder / 'verus')
    need(source.isascii() and source.startswith(PREFIX) and source.count('include!') == 2, 'exact shared imports')
    need(body.isascii() and body.startswith(MACRO_PREFIX) and body.endswith(MACRO_SUFFIX), 'exact macro envelope')
    for name, text in [('audited-shared-storage.rs', source[len(PREFIX):]),
                       ('audited-shared-body.rs', body[len(MACRO_PREFIX):-len(MACRO_SUFFIX)])]:
        path = folder / 'verus' / name
        path.write_text(text, encoding='ascii')
        module.inherited().POLICY.scan(path)


def stage(module, snapshots, folder, mutation):
    here, prior = module.HERE, module.storage_parent
    source, body = candidates(module.inherited().BASE, snapshots[here / SOURCE].decode('ascii'),
                              snapshots[here.parent / BODY].decode('ascii'), mutation)
    generated = {Path('verus') / n: snapshots[here / n]
                 for n in [*prior.source_names(module), prior.SOURCE]}
    generated.update({Path('verus') / SOURCE: source.encode('ascii'), BODY: body.encode('ascii')})
    for path, data in generated.items():
        target = folder / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    audit_sources(module, source, body, folder)
    need(all((folder / p).read_bytes() == data for p, data in generated.items()), 'exact included bytes')
    return source, body, {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}


def exact_span(source, path, begin, end, primary=False, label=None, expansion=None):
    need(source.isascii() and 0 <= begin < end <= len(source), 'expected ASCII span')
    first, last = source.count('\n', 0, begin) + 1, source.count('\n', 0, end) + 1
    left, right = begin - source.rfind('\n', 0, begin), end - source.rfind('\n', 0, end)
    lines = source.splitlines()[first - 1:last]
    return {'file_name': str(path), 'byte_start': begin, 'byte_end': end, 'line_start': first, 'line_end': last,
            'column_start': left, 'column_end': right, 'is_primary': primary,
            'text': [{'text': line, 'highlight_start': left if i == 0 else 1,
                      'highlight_end': right if i == len(lines) - 1 else len(line) + 1}
                     for i, line in enumerate(lines)], 'label': label, 'suggested_replacement': None,
            'suggestion_applicability': None, 'expansion': expansion}


def check_result(module, status, stdout, stderr, source, body, path, mutation):
    if not macro_mutation(mutation):
        module.raw_checker.check_result(module, status, stdout, stderr, source, path, mutation)
        return
    base = module.inherited().BASE
    # Reuse the strict summary/error/primary-clause checks, replacing only the
    # authenticated macro exit with the corresponding wrapper body for that check.
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    need(len(diagnostics) in (1, 2), 'macro diagnostic count')
    spans = diagnostics[0]['spans']
    need(len(spans) == 2, 'macro postcondition and exit')
    primary = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(len(primary) == len(exits) == 1, 'typed macro primary and exit')
    begin, end = module.postcondition_bounds(base, source, mutation)
    wanted_primary = exact_span(source, path, begin, end, True, 'failed this postcondition')
    macro_path = path.parent / '..' / BODY
    call = source.index(INVOCATION)
    decl = body.index('macro_rules!')
    definition = 'macro_rules! settlement_return_admission_body'
    expansion = {'span': exact_span(source, path, call, call + len(INVOCATION)),
                 'macro_decl_name': 'settlement_return_admission_body!',
                 'def_site_span': exact_span(body, macro_path, decl, decl + len(definition))}
    begin = len(MACRO_PREFIX) - 2
    end = len(body) - len(MACRO_SUFFIX) + len('    }')
    wanted_exit = exact_span(body, macro_path, begin, end, False, 'at the end of the function body', expansion)
    same = lambda a, b: json.dumps(a, sort_keys=True) == json.dumps(b, sort_keys=True)
    need(same(primary[0], wanted_primary), 'exact macro postcondition span')
    need(same(exits[0], wanted_exit), 'exact macro exit, expansion, invocation and definition')
    changed = copy.deepcopy(diagnostics)
    _, begin, end = base.function_bounds(source, mutation.function)
    changed[0]['spans'] = [wanted_primary, exact_span(source, path, begin, end, False, 'at the end of the function body')]
    module.raw_checker.check_result(module, status, stdout, '\n'.join(map(json.dumps, changed)), source, path, mutation)


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 360,
            'negative_cases': 10, 'shared_scalar_body_verified': True, 'logical_settlement_composition': True,
            'actual_vec_observations_in_rust_callsite': True, 'allocator_behavior_proved': False,
            'physical_capacity_binding_proved': False, 'full_rust_source_refinement_proved': False,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA and digest(snapshots[here.parent / BODY]) == BODY_SHA, 'exact shared sources')
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
        identities_generated = {str(case / p): digest(data) for p, data in generated.items()}
        (case / 'sources.json').write_text(json.dumps(identities_generated, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        path = case / 'verus' / SOURCE
        cmd = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4', str(path)]
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities_generated.items()), 'generated source drift')
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
