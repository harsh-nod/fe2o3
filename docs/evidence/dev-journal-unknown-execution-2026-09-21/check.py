#!/usr/bin/env python3
"""Qualify the actual-typed Unknown transition and its exact historical boundary."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 472
SOURCE = 'context_journal_unknown_execution_v1.rs'
PRIOR = Path('docs/evidence/dev-journal-retained-execution-2026-09-21/check.py')
PRIOR_SHA = '52739d3c2b9c696c3e84a54d4261e597c71a81aa921447445fe45dd6d370d2db'
CRATE = Path('crates/fe2o3-runtime-model')
BODY = Path('src/context_version_journal/retained_bodies.rs')
EXEC = Path('verus/context_journal_unknown_body_v1.rs')
SPEC = Path('verus/context_journal_unknown_decisions_v1.rs')
PINS = {
    'verus/' + SOURCE: '7cb56fb09b379f03d26e9a25c9b8fb39a54e95f3b68975d513272b6f2bf7d38f',
    str(SPEC): '86fed2fe2ea73c1202573bb66998697c55b657d0da299cc3c501639f0c9c24ba',
    str(EXEC): '77e1bb1666e5989587f68ea2a69b7b6d318740d3c5d7c1cb86668e90e21c42fd',
    'src/context_version_journal/unknown_tests.rs': '5cbda2c44717a2d30aa37ddb4c7ea08b75e2685fc9fdd4eba7e8cc3096da45b5',
    'src/context_version_journal/settlement_tests.rs': '520c3ae5de351433e3c15add13d4ac62811076be1a9dbb7cfdb00a1b38409565',
}
SOURCE_SHA = PINS['verus/' + SOURCE]
Mutation = namedtuple('Mutation', 'name path macro function before after assertion postcondition')
WRITERS = 'journal_view(*journal).writers =~= unknown_after(journal_view(before), writer_reference_view(writer)).writers'
POST = 'unknown_execution_view(journal_view(*old(journal)), journal_view(*final(journal)), writer_reference_view(writer), result)'
FACTOR = ('logical::unknown_execution_relation_v1(before, after, writer, result) <==>\n'
          '        (unknown_execution_view(logical_contents(before), logical_contents(after), writer, read_result_from(result))\n'
          '            && historical_unknown_identity(before, after, writer, result))')


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
    need(digest(data) == PRIOR_SHA, 'pinned typed admission checker')
    spec = importlib.util.spec_from_file_location('unknown_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.unknown_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.unknown_parent.extra_paths(module), PRIOR, *(CRATE / p for p in PINS)]


def mutations(_base=None):
    rows = [
        ('header_error', 'Err(error) => return Err(error),',
         'Err(_) => return Err(ReadErrorV1::InvalidState),', None),
        ('rejection_write', 'if result.is_err() {', 'if false {', WRITERS),
        ('pending_store', 'Some(WriterEntryV1::Unknown {', 'Some(WriterEntryV1::Pending {', WRITERS),
        ('head_store', '                head,', '                head: None,', WRITERS),
        ('count_store', '                count,', '                count: 0,', WRITERS),
        ('unknown_revalidation',
         'let result = shared_retained_chain_v1($journal, $writer, head, count);',
         'let result = if unknown { Ok(()) } else { shared_retained_chain_v1($journal, $writer, head, count) };', None),
        ('scalar_frame', 'if !unknown {', 'if !unknown {\n            $journal.reserved_count = 0;', None),
    ]
    result = [Mutation(name, BODY, 'retained_unknown_body', 'shared_retained_unknown_v1', before, after, assertion, POST)
              for name, before, after, assertion in rows]
    result.extend([
        Mutation('rejection_identity', SPEC, None, 'unknown_historical_factorization',
                 '&&& result.is_err() ==> after == before', '&&& true', None, FACTOR),
        Mutation('repeated_identity', SPEC, None, 'unknown_historical_factorization',
                 'Ok((_, _, true)) => after == before, _ => true,',
                 'Ok((_, _, true)) => true, _ => true,', None, FACTOR),
    ])
    return result


def stage(module, snapshots, folder, mutation):
    for path, pin in PINS.items():
        need(digest(snapshots[module.HERE.parent / path]) == pin, 'pinned Unknown source: ' + path)
    module.unknown_parent.stage(module, snapshots, folder, None)
    for path in PINS:
        target = folder / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(snapshots[module.HERE.parent / path])
        if path.startswith('verus/') and path != 'verus/' + SOURCE:
            module.inherited().POLICY.scan(target)
    if mutation:
        target = folder / mutation.path
        body = target.read_text(encoding='ascii')
        if mutation.macro:
            body = module.scratch_parent.candidate(body, mutation)
        else:
            anchor = 'spec fn historical_unknown_identity('
            need(body.count(anchor) == 1, 'unique historical identity definition')
            start = body.index(anchor)
            end = body.index('\n}\n', start) + 2
            fragment = body[start:end]
            need(fragment.count(mutation.before) == 1 and mutation.before != mutation.after, 'unique identity mutation')
            body = body[:start] + fragment.replace(mutation.before, mutation.after, 1) + body[end:]
        target.write_text(body, encoding='ascii')
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
                'verified': 0 if mutation else COUNT, 'errors': 1 if mutation else 0,
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
    message = 'assertion failed' if mutation.assertion else 'postcondition not satisfied'
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error'
         and error['message'] == message and error['code'] is None and error['children'] == [], 'intended obligation failure')
    proof_file = EXEC if mutation.macro else SPEC
    proof, body = generated[proof_file].decode('ascii'), generated[BODY].decode('ascii')
    proof_path, macro_path = path.parent.parent / proof_file, path.parent / '..' / BODY
    helper, retained = module.retained_parent, module.scratch_parent
    if mutation.assertion:
        needle = 'assert(' + mutation.assertion + ');'
        need(proof.count(needle) == 1, 'unique supporting assertion')
        begin = proof.index(needle) + len('assert(')
        primary = helper.exact_span(proof, proof_path, begin, begin + len(mutation.assertion), True, 'assertion failed')
        need(same(error['spans'], [primary]), 'exact supporting assertion')
        return
    need(proof.count(mutation.postcondition) == 1, 'unique Unknown postcondition')
    begin = proof.index(mutation.postcondition)
    primary = helper.exact_span(proof, proof_path, begin, begin + len(mutation.postcondition), True, 'failed this postcondition')
    spans = error['spans']
    need(len(spans) == 2 and all(type(s['is_primary']) is bool for s in spans), 'exact typed span roster')
    primaries = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(same(primaries, [primary]) and len(exits) == 1, 'exact Unknown postcondition')
    actual = exits[0]
    first, last, label = actual['byte_start'], actual['byte_end'], actual['label']
    need(type(first) is int and type(last) is int, 'integer exit offsets')
    if actual['file_name'] == str(proof_path):
        anchor = 'fn ' + mutation.function + '('
        need(proof.count(anchor) == 1, 'unique Unknown proof function')
        start = proof.index('\n{', proof.index(anchor)) + 1
        end = proof.index('\n}\n', start) + 2
        if mutation.macro is None:
            # This pinned ghost proof reports its final if-let expression as the exit.
            expression = 'if let Ok((head, count, unknown)) = logical::retained_header_decision_v1(before, writer, true) {'
            need(proof[start:end].count(expression) == 1, 'unique historical final expression')
            start = proof.index(expression, start)
            end = proof.index('\n    }\n}', start) + len('\n    }')
        need((first, last, label) == (start, end, 'at the end of the function body'), 'exact intended wrapper exit')
        wanted = helper.exact_span(proof, proof_path, first, last, False, label)
    else:
        need(mutation.macro is not None, 'historical proof has no macro exit')
        bounds = retained.macro_bounds(body)[mutation.macro]
        need((first, last, label) in retained.exits_in(body, bounds), 'exact intended shared exit')
        begin, end = retained.invocation(proof, mutation.macro)
        definition = 'macro_rules! ' + mutation.macro
        expansion = {'span': helper.exact_span(proof, proof_path, begin, end),
                     'macro_decl_name': mutation.macro + '!',
                     'def_site_span': helper.exact_span(body, macro_path, bounds['definition'], bounds['definition'] + len(definition))}
        wanted = helper.exact_span(body, macro_path, first, last, False, label, expansion)
    need(same(actual, wanted), 'exact Unknown exit and expansion tree')


def report():
    return {'development_checks_passed': True, 'whole_crate_positive_obligations': COUNT,
            'targeted_negative_controls': 9, 'negative_verification_scope': 'one named production-module function per control',
            'executable_controls': 7, 'identity_specification_controls': 2,
            'actual_typed_unknown_transition': True, 'exact_sequence_frame': True,
            'historical_identity_factorization': True, 'physical_storage_refinement': False,
            'full_runtime_refinement': False, 'native_or_performance_acceptance': False}


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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact Unknown root')
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
