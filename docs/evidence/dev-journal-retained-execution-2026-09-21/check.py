#!/usr/bin/env python3
"""Qualify shared retained admission bodies on the actual production declarations."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 465
SOURCE = 'context_journal_retained_execution_v1.rs'
PRIOR = Path('docs/evidence/dev-journal-representation-2026-09-21/check.py')
PRIOR_SHA = 'bf01de36a60158fd7786367610f8605ef32cdea229d516515af580988d1bbafe'
CRATE = Path('crates/fe2o3-runtime-model')
BODY = Path('src/context_version_journal/retained_bodies.rs')
EXEC = Path('verus/context_journal_retained_bodies_v1.rs')
PINS = {
    'verus/' + SOURCE: '35ec968a60bb7db0a09409b57a401de2bf201724ac02c2b2126ce6ce6abb8e9c',
    'verus/context_journal_retained_decisions_v1.rs': '89768b90628155e4c74de341b58a99c6ddb6f4386a3c1ed6d5212b32a4751200',
    str(EXEC): '4407ba3b7a53bf6bd1965d1d2baba0d3913b2a6b7e73d8b9d60f9fabd3dd54e8',
    'src/context_version_journal/retained_tests.rs': '52ca1a7bb1714f226939824273bd30a5b4ee576384e447920f891f25b9a99771',
}
SOURCE_SHA = PINS['verus/' + SOURCE]
Mutation = namedtuple('Mutation', 'name macro function before after postcondition')


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
    need(digest(data) == PRIOR_SHA, 'pinned representation checker')
    spec = importlib.util.spec_from_file_location('typed_retained_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.typed_retained_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.typed_retained_parent.extra_paths(module), PRIOR, *(CRATE / p for p in PINS)]


def mutations(_base=None):
    posts = {
        'allocation': 'result == allocation_result_from(allocation_decision(journal_view(*journal), allocation_reference_view(reference)))',
        'header': 'result == read_result_from(header_decision(journal_view(*journal), writer_reference_view(writer), allow_unknown))',
        'member': 'result == member_result_from(member_decision(journal_view(*journal), writer_reference_view(writer), head, previous_view(previous)))',
        'chain': 'result == read_result_from(chain_decision(journal_view(*journal), writer_reference_view(writer), initial, count))',
    }
    rows = [
        ('allocation_context', 'allocation', '\n            || entry.key.context_generation != $journal.context_generation', ''),
        ('allocation_error', 'allocation',
         'if $reference.slot >= $journal.allocations.len() {\n            return Err(ReadErrorV1::InvalidAllocationReference);',
         'if $reference.slot >= $journal.allocations.len() {\n            return Err(ReadErrorV1::InvalidState);'),
        ('header_context', 'header', '\n            || key.context_generation != $journal.context_generation', ''),
        ('header_unknown', 'header', 'if $allow_unknown =>', 'if true =>'),
        ('member_writer', 'member', 'member.writer.slot != $writer.slot', 'false'),
        ('member_backlink', 'member', 'allocation.pending_member != Some(slot)', 'false'),
        ('member_epoch', 'member', 'allocation.attempt_epoch != member.attempt_epoch', 'false'),
        ('member_lineage', 'member', 'allocation.content_lineage != member.prior_lineage', 'false'),
        ('chain_error', 'chain',
         'if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {\n                return Err(ReadErrorV1::InvalidState);',
         'if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {\n                return Err(ReadErrorV1::InvalidReference);'),
        ('chain_tail', 'chain', 'if $head.is_some() {', 'if false {'),
    ]
    return [Mutation(name, 'retained_' + family + '_body', 'shared_retained_' + family + '_v1', before, after, posts[family])
            for name, family, before, after in rows]


def stage(module, snapshots, folder, mutation):
    for path, pin in PINS.items():
        need(digest(snapshots[module.HERE.parent / path]) == pin, 'pinned typed admission source: ' + path)
    module.typed_retained_parent.stage(module, snapshots, folder, None)
    for path in PINS:
        target = folder / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(snapshots[module.HERE.parent / path])
        if path.startswith('verus/') and path != 'verus/' + SOURCE:
            module.inherited().POLICY.scan(target)
    target = folder / BODY
    target.write_text(module.scratch_parent.candidate(target.read_text(encoding='ascii'), mutation), encoding='ascii')
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
    negative_verified = int(mutation is not None and mutation.macro == 'retained_chain_body')
    expected = {'encountered-error': bool(mutation), 'encountered-vir-error': False,
                'verified': negative_verified if mutation else COUNT, 'errors': 1 if mutation else 0,
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
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error'
         and error['message'] == 'postcondition not satisfied' and error['code'] is None
         and error['children'] == [], 'intended postcondition failure')
    proof, body = generated[EXEC].decode('ascii'), generated[BODY].decode('ascii')
    proof_path, macro_path = path.parent.parent / EXEC, path.parent / '..' / BODY
    need(proof.count(mutation.postcondition) == 1, 'unique typed postcondition')
    begin = proof.index(mutation.postcondition)
    helper, retained = module.retained_parent, module.scratch_parent
    primary = helper.exact_span(proof, proof_path, begin, begin + len(mutation.postcondition), True, 'failed this postcondition')
    spans = error['spans']
    need(len(spans) == 2 and all(type(s['is_primary']) is bool for s in spans), 'exact typed span roster')
    primaries = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(same(primaries, [primary]) and len(exits) == 1, 'exact typed postcondition')
    actual = exits[0]
    first, last, label = actual['byte_start'], actual['byte_end'], actual['label']
    need(type(first) is int and type(last) is int, 'integer exit offsets')
    if actual['file_name'] == str(proof_path):
        _, start, end = base.function_bounds(proof, mutation.function)
        need((first, last, label) == (start, end, 'at the end of the function body'), 'exact intended wrapper exit')
        wanted = helper.exact_span(proof, proof_path, first, last, False, label)
    else:
        bounds = retained.macro_bounds(body)[mutation.macro]
        need((first, last, label) in retained.exits_in(body, bounds), 'exact intended shared exit')
        begin, end = retained.invocation(proof, mutation.macro)
        definition = 'macro_rules! ' + mutation.macro
        expansion = {'span': helper.exact_span(proof, proof_path, begin, end),
                     'macro_decl_name': mutation.macro + '!',
                     'def_site_span': helper.exact_span(body, macro_path, bounds['definition'], bounds['definition'] + len(definition))}
        wanted = helper.exact_span(body, macro_path, first, last, False, label, expansion)
    need(same(actual, wanted), 'exact typed exit and expansion tree')


def report():
    return {'development_checks_passed': True, 'whole_crate_positive_obligations': COUNT,
            'targeted_negative_controls': 10, 'negative_verification_scope': 'one named production-module function per control',
            'actual_typed_retained_admission': True, 'unconditional_sequence_decisions': True,
            'historical_decision_correspondence': True, 'physical_storage_refinement': False,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact typed admission root')
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
