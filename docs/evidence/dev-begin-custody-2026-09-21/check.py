#!/usr/bin/env python3
"""Qualify logical Begin custody and ordered readers, not native admission."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 321
SOURCE = 'context_version_journal_begin_custody_v1.rs'
SOURCE_SHA = '27b87b4140f85a8d7cdb5807407e642884dc9f1cef2d7f990c03992d3aa67cac'
GUARDS = 'context_begin_reader_guards_v1.rs'
LIFECYCLE = 'context_producer_read_lifecycle_v1.rs'
LIFECYCLE_SHA = '79433d1e07165cd11be717219c901726491cc8310d892aa8dd0068aba5135005'
RAW_CHECK = Path('docs/evidence/dev-begin-execution-2026-09-21/check.py')
RAW_CHECK_SHA = '14a33a659666986293cac96167b04f395b97122dd0b3f7388cc980701bccca39'
PREFIX = ('// Begin custody composition candidate; production/native refinement remains separate.\n'
          'include!("context_version_journal_begin_v1.rs");\n'
          '#[path = "context_begin_reader_guards_v1.rs"]\n'
          'mod begin_reader_guards;\nuse begin_reader_guards::*;\n')
GUARD_PREFIX = ('// Exact lifecycle guard projection, reverified with the importing root\'s types.\n'
                '// The standalone checker authenticates these declarations against the pinned lifecycle source.\n'
                'use super::*;\n\nverus! {\n\n')
DECLARATIONS = (
    'pub proof fn reader_count_addition_fits_usize_v1(',
    'pub open spec fn producer_reader_count_decision_v1(',
    'pub fn producer_reader_count_exec_v1(',
    'pub open spec fn producer_unread_scan_v1(',
    'pub fn producer_require_unread_exec_v1(',
)


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / RAW_CHECK
    data = path.read_bytes()
    need(digest(data) == RAW_CHECK_SHA, 'pinned raw Begin checker')
    spec = importlib.util.spec_from_file_location('custody_raw_begin', path)
    raw = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = raw
    exec(compile(data, str(path), 'exec'), raw.__dict__)
    # Reuse its strict whole-crate diagnostic classifier with this root's count.
    raw.COUNT = COUNT
    module = raw.load(repo)
    module.raw_checker = raw
    return module


def projection(lifecycle):
    need(digest(lifecycle) == LIFECYCLE_SHA, 'pinned lifecycle source')
    source = lifecycle.decode('ascii')
    parts = []
    for declaration in DECLARATIONS:
        marker = '\n' + declaration
        need(source.count(marker) == 1, 'unique lifecycle declaration')
        begin = source.index(marker) + 1
        end = source.index('\n}\n', begin) + 2
        parts.append(source[begin:end])
    return (GUARD_PREFIX + '\n\n'.join(parts) + '\n\n}\n').encode('ascii')


def mutations(base):
    rows = [
        ('combined_busy', 'begin_combined_unread_exec_v1',
         'if count != 0 { return Err(ReadErrorV1::AllocationBusy); }',
         'if count != 0 { return Ok(()); }', 'result == producer_unread_scan_v1'),
        ('combined_error', 'begin_combined_unread_exec_v1',
         'Ok(count) => count, Err(error) => return Err(error),',
         'Ok(count) => count, Err(_) => return Err(ReadErrorV1::AllocationBusy),', 'result == producer_unread_scan_v1'),
        ('final_descriptor', 'begin_combined_unread_exec_v1',
         '        let count = match producer_reader_count_exec_v1',
         '        if index + 1 == roster.len() { return Ok(()); }\n        let count = match producer_reader_count_exec_v1',
         'result == producer_unread_scan_v1'),
        ('stable_busy', 'begin_stable_unread_exec_v1',
         'if contents.readers[reference.slot] != 0 { return Err(ReadErrorV1::AllocationBusy); }',
         'if contents.readers[reference.slot] != 0 { return Ok(()); }', 'result == unread_scan_v1'),
        ('writer_precedence', 'begin_issued_exec_v1',
         '    let ghost before = *contents;',
         '    let ghost before = *contents;\n    if let Err(error) = begin_preflight_exec_v1(&contents.stable.journal, writer, roster) { return Err(error); }',
         'begin_issued_relation_v1'),
        ('stable_precedence', 'begin_issued_exec_v1',
         '    let ghost before = *contents;',
         '    let ghost before = *contents;\n    if let Err(error) = begin_stable_unread_exec_v1(&contents.stable, roster) { return Err(error); }',
         'begin_issued_relation_v1'),
        ('guard_error_frame', 'begin_issued_exec_v1',
         'match begin_combined_unread_exec_v1(contents, roster) {\n        Err(error) => return Err(error),',
         'match begin_combined_unread_exec_v1(contents, roster) {\n        Err(error) => { contents.next_incarnation = 0; return Err(error); },',
         'begin_issued_relation_v1'),
        ('result_substitution', 'begin_issued_exec_v1',
         '    result\n}',
         '    if result.is_ok() { return Err(ReadErrorV1::InvalidState); }\n    result\n}', 'begin_issued_relation_v1'),
    ]
    return [base.Mutation(name, function, before, after, post, COUNT - 1)
            for name, function, before, after, post in rows]


def check_result(module, status, stdout, stderr, source, path, mutation):
    module.raw_checker.check_result(module, status, stdout, stderr, source, path, mutation)


def audit_source(module, candidate, folder):
    inherited = module.inherited()
    raw = module.raw_checker
    need(candidate.isascii() and candidate.startswith(PREFIX), 'exact custody include and module')
    need(candidate.count('include!') == 1 and candidate.count('#[path') == 1, 'single inherited root and projection')
    need((folder / GUARDS).read_bytes() == projection((folder / LIFECYCLE).read_bytes()), 'exact guard projection')
    raw_source = (folder / raw.SOURCE).read_text()
    need(digest(raw_source.encode()) == raw.SOURCE_SHA, 'pinned raw Begin source')
    module.audit_source(inherited, (folder / 'context_version_journal_enrollment_v1.rs').read_text(), folder)
    bodies = {
        'audited-begin-body.rs': raw_source[len(raw.PREFIX):],
        'audited-custody-body.rs': candidate[len(PREFIX):],
        'audited-guards-body.rs': (folder / GUARDS).read_text()[len(GUARD_PREFIX) - len('verus! {\n\n'):],
    }
    for name, body in bodies.items():
        path = folder / name
        path.write_text('use vstd::prelude::*;\n' + body, encoding='ascii')
        inherited.POLICY.scan(path)


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT,
            'inherited_obligations': 289, 'negative_cases': 8,
            'general_begin_custody_and_readers_preserved': True,
            'exact_ordered_begin_wrapper': True, 'rust_source_refinement_proved': False,
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
    raw = module.raw_checker
    for path in [here / SOURCE, here / GUARDS, here / LIFECYCLE, here / raw.SOURCE,
                 repo / RAW_CHECK, Path(__file__).resolve(), verus.parent / 'rust_verify', verus.parent / 'z3']:
        snapshots[path] = path.read_bytes()
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact custody source')
    need(snapshots[here / GUARDS] == projection(snapshots[here / LIFECYCLE]), 'exact guard declarations')
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
        path = case / SOURCE
        path.write_text(candidate, encoding='ascii')
        for dependency in [*module.DEPENDENCIES, 'context_version_journal_enrollment_v1.rs', raw.SOURCE, GUARDS, LIFECYCLE]:
            (case / dependency).write_bytes(snapshots[here / dependency])
        audit_source(module, candidate, case)
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
    (output / 'report.json').write_text(json.dumps(report(), indent=2) + '\n')
    print(json.dumps(report()), flush=True)


if __name__ == '__main__':
    main()
