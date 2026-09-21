#!/usr/bin/env python3
"""Development transaction checks; not historical-contract or native acceptance."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

COUNT = 275
SOURCE = "context_version_journal_enrollment_transaction_v1.rs"
SOURCE_SHA = "704b0d623bcbc9b06b4623e305e6035d29e2e12d5916898ad2d2ac9baf3e557a"
PREFIX = ('// No-sort development candidate. Logical contents, not opaque Vec identity.\n'
          'include!("context_version_journal_enrollment_v1.rs");\n')


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    here = repo / 'crates/fe2o3-runtime-model/verus'
    path = here / 'check-journal-enrollment.py'
    data = path.read_bytes()
    need(digest(data) == 'f5aaec2ac9a6109b93a0cf4ea0d71282215dbeb488b3c0b67d2857433d8123f6', 'inherited checker identity')
    spec = importlib.util.spec_from_file_location('transaction_enrollment', path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def mutations(base):
    function = 'enrollment_transaction_exec_v1'
    undo = '        enrollment_unstage_exec_v1(&mut contents.allocations, entries, output, installed, Ghost(before.allocations@));\n'
    duplicate = '    if installed < entries.len() {\n'
    overlap = '    if !enrollment_staged_retained_clear_exec_v1(contents, entries, output, remaining, Ghost(before)) {\n'
    rows = [
        ('duplicate_undo', duplicate + undo, duplicate),
        ('overlap_undo', overlap + undo, overlap),
        ('output_restoration', duplicate + undo + '        enrollment_clear_output_exec_v1(output);\n        proof { assert(output@ =~= original); }',
         duplicate + undo + '        enrollment_clear_output_exec_v1(output);\n        proof { assert(output@ =~= original); }\n'
         '        if output.len() > 0 { output[0] = Some(AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 0, local: 0 } }); }'),
        ('retained_prefix', '    contents.allocation_free.truncate(remaining);', '    contents.allocation_free.truncate(0);'),
        ('success_result', '    Ok(())\n}', '    Err(EnrollmentErrorV1::AllocationReplay)\n}'),
    ]
    cases = [base.Mutation(name, function, before, after, 'enrollment_transaction_relation_v1', COUNT - 1)
             for name, before, after in rows]
    cases.append(base.Mutation('retained_overlap_result', 'enrollment_staged_retained_clear_exec_v1',
        '    true\n}', '    false\n}', 'clear == enrollment_retained_clear_v1', COUNT - 1))
    return cases


def check_result(module, status, stdout, stderr, source, path, mutation):
    base = module.inherited().BASE
    need(type(status) is int and status == (1 if mutation else 0), 'expected normal exit')
    actual = base.unique_json(stdout)['verification-results']
    expected = {'encountered-error': mutation is not None, 'encountered-vir-error': False,
                'success': mutation is None, 'verified': COUNT - 1 if mutation else COUNT,
                'errors': 1 if mutation else 0, 'is-verifying-entire-crate': True}
    need(actual == expected and all(type(actual[k]) is type(v) for k, v in expected.items()), 'exact whole-crate result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        need(not diagnostics, 'positive diagnostics')
        return
    need(len(diagnostics) in (1, 2), 'one intended error and optional footer')
    if len(diagnostics) == 2:
        footer = diagnostics.pop()
        need(footer['$message_type'] == 'diagnostic' and footer['level'] == 'error'
             and footer['message'] == 'aborting due to 1 previous error'
             and footer['code'] is None and not footer['spans'] and not footer['children'], 'exact footer')
    error = diagnostics[0]
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error'
         and error['message'] == 'postcondition not satisfied' and error['code'] is None
         and not error['children'] and len(error['spans']) == 2, 'intended postcondition error')
    for span in error['spans']:
        base.check_span(span, source, path)
    primary = [s for s in error['spans'] if s['is_primary'] is True]
    exits = [s for s in error['spans'] if s['is_primary'] is False]
    need(len(primary) == len(exits) == 1, 'one primary and exit')
    begin, end = module.postcondition_bounds(base, source, mutation)
    _, body, finish = base.function_bounds(source, mutation.function)
    need((primary[0]['byte_start'], primary[0]['byte_end']) == (begin, end)
         and primary[0]['label'] == 'failed this postcondition', 'exact primary clause')
    need(body <= exits[0]['byte_start'] < exits[0]['byte_end'] <= finish
         and exits[0]['label'] in ('at this exit', 'at the end of the function body'), 'executable exit')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    repo, verus = args.repo.resolve(), args.verus.resolve()
    module = load(repo)
    inherited = module.inherited()
    base = inherited.BASE
    for signum in base.SIGNALS:
        signal.signal(signum, base.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, base.SIGNALS)
    here = module.HERE
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
    source_path = here / SOURCE
    snapshots[source_path] = source_path.read_bytes()
    need(digest(snapshots[source_path]) == SOURCE_SHA, 'exact transaction source')
    snapshots[Path(__file__).resolve()] = Path(__file__).read_bytes()
    for executable in (verus.parent / 'rust_verify', verus.parent / 'z3'):
        snapshots[executable] = executable.read_bytes()
    closure = repo / 'examples/row_softmax_v1/verify-verus-closure.sh'
    snapshots[closure] = closure.read_bytes()
    need(digest(snapshots[closure]) == 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c', 'closure checker')
    identities = {str(p): digest(data) for p, data in snapshots.items()}
    source = snapshots[source_path].decode('ascii')
    need(source.startswith(PREFIX) and source.count('include!') == 1, 'single exact inherited include')
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
        path = case / SOURCE
        path.write_text(candidate, encoding='ascii')
        for dependency in [*module.DEPENDENCIES, 'context_version_journal_enrollment_v1.rs']:
            (case / dependency).write_bytes(snapshots[here / dependency])
        module.audit_source(inherited, snapshots[here / 'context_version_journal_enrollment_v1.rs'].decode('ascii'), case)
        audited = case / 'audited-transaction-body.rs'
        audited.write_text('use vstd::prelude::*;\n' + candidate[len(PREFIX):], encoding='ascii')
        inherited.POLICY.scan(audited)
        generated = {str(p): digest(p.read_bytes()) for p in case.glob('*.rs')}
        (case / 'sources.json').write_text(json.dumps(generated, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        cmd = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
               '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
               '--error-format=json', '--num-threads', '4', str(path)]
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in generated.items()), 'generated source drift')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift after solver')
        check_result(module, status, stdout, stderr, candidate, path, mutation)
        completed[name] = {str(p): digest(p.read_bytes()) for p in case.rglob('*') if p.is_file()}
        print(name + ': PASS', flush=True)
    need(base.run_owned(command, 120, output / 'closure-after', env)[0] == 0, 'closure after')
    after = {p: digest(Path(p).read_bytes()) for p in identities}
    need(after == identities, 'final identities')
    for name, frozen in completed.items():
        need({str(p): digest(p.read_bytes()) for p in (output / name).rglob('*') if p.is_file()} == frozen,
             'completed case artifact drift')
    (output / 'completed-cases.json').write_text(json.dumps(completed, indent=2) + '\n')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    report = {'development_checks_passed': True, 'positive_obligations': COUNT,
              'inherited_obligations': 263, 'negative_cases': 6,
              'historical_error_contract_proved': False, 'production_promoted': False,
              'native_or_performance_acceptance': False}
    (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report), flush=True)


if __name__ == '__main__':
    main()
