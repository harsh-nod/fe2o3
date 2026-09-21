#!/usr/bin/env python3
"""Qualify shared scratch scanning/staging, not allocator or whole-runtime refinement."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 392
SOURCE = 'context_shared_settlement_scratch_v1.rs'
SOURCE_SHA = '811cf580a09093a45a2556e1df19157d52842e7ff2424d4eda5eb07d6b07a0ab'
BODY = Path('src/context_version_journal/settlement_scratch_bodies.rs')
BODY_SHA = '82601afcffa905c51ba44ba06bd01c39bc7ad91ca8d6d9c613c21ada8b97e3f7'
ADAPTER = Path('src/context_version_journal/settlement_scratch.rs')
ADAPTER_SHA = '6254468b91a4412dc8ac6e57463e5274187c4fac33230c5e3a573036e8712019'
PRIOR = Path('docs/evidence/dev-shared-retained-2026-09-21/check.py')
PRIOR_SHA = '0143ae9909a12f420946ce034d483a9cde343fc4689b3daa8b686ef4b953451e'
PREFIX = ('// Shared scratch scanning/staging; physical storage and commit refinement remain separate.\n'
          'include!("context_shared_retained_v1.rs");\n'
          'include!("../src/context_version_journal/settlement_scratch_bodies.rs");\n')
BODY_PREFIX = '// Shared executable scratch scan/staging. Loop annotations carry no runtime code.\n'
PARAMETERS = {
    'settlement_scratch_scan_body': '($syntax:ident, $journal:ident, $count:ident, $index:ident, [$($annotations:tt)*])',
    'settlement_scratch_stage_body': ('($syntax:ident, $journal:ident, $initial:ident, $count:ident, $head:ident,\n'
                                    '     $index:ident, [$($annotations:tt)*])'),
}
SCAN_INVARIANT = 'settlement_scratch_scan_v1(*journal, count, 0) == settlement_scratch_scan_v1(*journal, count, index as nat)'
STAGE_INVARIANT = 'journal.scratch@ =~= settlement_scratch_v1(before, initial, count, index as nat, 0)'
SCAN_CALL = ('settlement_scratch_scan_body!(verus_exec_expr, journal, count, index, [\n'
             '        invariant index <= count, count <= journal.scratch@.len(),\n'
             '            ' + SCAN_INVARIANT + ',\n'
             '        decreases count - index,\n'
             '    ])')
STAGE_CALL = ('settlement_scratch_stage_body!(verus_exec_expr, journal, initial, count, head, index, [\n'
              '        invariant index <= count, before == *old(journal), settlement_storage_ready_v1(before, initial, count),\n'
              '            begin_stage_frame_v1(before, *journal), head == settlement_cursor_v1(before, initial, index as nat),\n'
              '            index < count ==> settlement_plan_ready_v1(before, initial, index as nat),\n'
              '            ' + STAGE_INVARIANT + ',\n'
              '        decreases count - index,\n'
              '    ])')
RUST = [Path('crates/fe2o3-runtime-model') / p for p in (
    BODY, ADAPTER, 'src/context_version_journal/settlement_scratch_tests.rs')]
Mutation = namedtuple('Mutation', 'name macro function before after postcondition invariant', defaults=[None])


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned shared retained checker')
    spec = importlib.util.spec_from_file_location('shared_scratch_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.scratch_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.scratch_parent.extra_paths(module), PRIOR,
            Path('crates/fe2o3-runtime-model/verus') / SOURCE, *RUST]


def macro_bounds(body):
    need(body.isascii() and body.startswith(BODY_PREFIX), 'ASCII scratch macro preamble')
    cursor, result = len(BODY_PREFIX), {}
    for name, parameters in PARAMETERS.items():
        prefix = 'macro_rules! ' + name + ' {\n    ' + parameters + ' => {\n        $syntax!({\n'
        suffix = '        })\n    };\n}\n'
        need(body.startswith(prefix, cursor), 'exact scratch macro envelope: ' + name)
        start = cursor + len(prefix)
        end = body.index(suffix, start)
        result[name] = {'definition': cursor, 'start': start, 'end': end,
                        'block_start': start - 2, 'block_end': end + 9}
        cursor = end + len(suffix)
        if name == 'settlement_scratch_scan_body':
            need(body[cursor:cursor + 1] == '\n', 'single scratch inter-macro separator')
            cursor += 1
    need(cursor == len(body), 'no extra scratch source')
    return result


def mutations(_base=None):
    scan = ('settlement_scratch_scan_body', 'shared_settlement_scratch_scan_v1')
    stage = ('settlement_scratch_stage_body', 'shared_settlement_scratch_stage_v1')
    scan_post = 'result == settlement_scratch_scan_v1'
    stage_post = 'final(journal).scratch@ == settlement_scratch_v1'
    result = [
        Mutation('scan_bypass', *scan, '$journal.scratch[$index].is_some()', 'false', scan_post, SCAN_INVARIANT),
        Mutation('scan_error', *scan, 'return Err(ReadErrorV1::InvalidState);', 'return Err(ReadErrorV1::InvalidReference);', scan_post),
        Mutation('scan_result', *scan, '            Ok(())', '            Err(ReadErrorV1::InvalidState)', scan_post),
    ]
    for name, before, after in [
        ('stage_prior_lineage', 'prior_lineage: member.prior_lineage', 'prior_lineage: 0'),
        ('stage_attempt_epoch', 'attempt_epoch: member.attempt_epoch', 'attempt_epoch: 0'),
        ('stage_member_slot', 'member_slot: slot', 'member_slot: $index'),
        ('stage_allocation_key', 'allocation: member.allocation',
         'allocation: { let mut allocation = member.allocation; allocation.key.local = 0; allocation }'),
    ]:
        result.append(Mutation(name, *stage, before, after, stage_post, STAGE_INVARIANT))
    anchor = '                $index += 1;\n            }\n'
    for name, appended, post in [
        ('stage_clear_plan', '            if $count > 0 { $journal.scratch[0] = None; }\n', stage_post),
        ('stage_clear_tail', '            if $count < $journal.scratch.len() { $journal.scratch[$count] = None; }\n', stage_post),
        ('stage_frame', '            $journal.reserved_count = 0;\n', 'begin_stage_frame_v1'),
    ]:
        result.append(Mutation(name, *stage, anchor, anchor + appended, post))
    return result


def candidate(body, mutation):
    if mutation is None:
        return body
    bounds = macro_bounds(body)[mutation.macro]
    begin, end = bounds['start'], bounds['end']
    inner = body[begin:end]
    need(mutation.before and mutation.before != mutation.after and inner.count(mutation.before) == 1, 'unique scratch mutation')
    offset = inner.index(mutation.before)
    changed = inner[:offset] + mutation.after + inner[offset + len(mutation.before):]
    need(changed[:offset] + mutation.before + changed[offset + len(mutation.after):] == inner,
         'reversible scratch mutation at exact offset')
    return body[:begin] + changed + body[end:]


def audit_adapter(adapter):
    need(digest(adapter) == ADAPTER_SHA, 'exact scratch Rust identity adapter and empty annotations')


def audit_sources(module, source, body, folder):
    prior = module.scratch_parent
    inherited_source = (folder / 'verus' / prior.SOURCE).read_text(encoding='ascii')
    inherited_body = (folder / prior.BODY).read_text(encoding='ascii')
    need(digest(inherited_source.encode()) == prior.SOURCE_SHA and digest(inherited_body.encode()) == prior.BODY_SHA,
         'pinned inherited retained source bodies')
    prior.audit_sources(module, inherited_source, inherited_body, folder)
    need(source.isascii() and source.startswith(PREFIX) and source.count('include!') == 2, 'exact scratch imports')
    need(source.count(SCAN_CALL) == source.count(STAGE_CALL) == 1, 'exact scratch proof-only loop annotations and adapters')
    policy = module.inherited().POLICY
    path = folder / 'verus/audited-scratch-root.rs'
    path.write_text(source[len(PREFIX):], encoding='ascii')
    policy.scan(path)
    for name, bounds in macro_bounds(body).items():
        path = folder / 'verus' / ('audited-' + name + '.rs')
        path.write_text(body[bounds['start']:bounds['end']], encoding='ascii')
        policy.scan(path)


def stage(module, snapshots, folder, mutation):
    here = module.HERE
    audit_adapter(snapshots[here.parent / ADAPTER])
    module.scratch_parent.stage(module, snapshots, folder, None)
    source = snapshots[here / SOURCE].decode('ascii')
    body = candidate(snapshots[here.parent / BODY].decode('ascii'), mutation)
    (folder / 'verus' / SOURCE).write_text(source, encoding='ascii')
    (folder / BODY).write_text(body, encoding='ascii')
    audit_sources(module, source, body, folder)
    return source, body, {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def exits_in(body, bounds):
    first, last = bounds['start'], bounds['end']
    inner = body[first:last]
    need('//' not in inner and '/*' not in inner, 'comment-free scratch exit grammar')
    literals = re.findall(r'"[^"\n]*"', inner)
    need(all(s in ('"validated complete retained chain"', '"validated retained member"') for s in literals), 'fixed expect literals')
    matches = list(re.finditer(r'\breturn Err\(ReadErrorV1::[A-Za-z]+\)(?=;)', inner))
    need(len(matches) == len(re.findall(r'\breturn\b', inner)), 'all scratch returns recognized')
    return {(first + m.start(), first + m.end(), 'at this exit') for m in matches} | {
        (bounds['block_start'], bounds['block_end'], 'at the end of the function body')}


def check_result(module, status, stdout, stderr, source, body, path, mutation):
    base, retained, spans_helper = module.inherited().BASE, module.scratch_parent, module.retained_parent
    if mutation is None:
        module.raw_checker.check_result(module, status, stdout, stderr, source, path, None)
        return
    need(type(status) is int and status == 1, 'expected normal scratch negative exit')
    actual = base.unique_json(stdout)['verification-results']
    expected = {'encountered-error': True, 'encountered-vir-error': False, 'success': False,
                'verified': COUNT - 1, 'errors': 1, 'is-verifying-entire-crate': True}
    need(same(actual, expected), 'exact whole-crate scratch negative result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    need(len(diagnostics) in (1, 2), 'one intended scratch negative and optional footer')
    if len(diagnostics) == 2:
        footer = diagnostics[1]
        need(footer['$message_type'] == 'diagnostic' and footer['level'] == 'error'
             and footer['message'] == 'aborting due to 1 previous error' and footer['code'] is None
             and footer['spans'] == [] and footer['children'] == [], 'exact scratch negative footer')
    error = diagnostics[0]
    message = 'invariant not satisfied at end of loop body' if mutation.invariant else 'postcondition not satisfied'
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error' and error['message'] == message
         and error['code'] is None and error['children'] == [], 'intended scratch obligation failure')
    spans = error['spans']
    if mutation.invariant:
        begin, end = retained.invocation(source, mutation.macro)
        need(source[begin:end].count(mutation.invariant) == 1, 'unique intended loop invariant')
        start = source.index(mutation.invariant, begin)
        wanted = spans_helper.exact_span(source, path, start, start + len(mutation.invariant), True)
        need(same(spans, [wanted]), 'exact scratch loop invariant span')
        return
    need(len(spans) == 2, 'one scratch postcondition and one exit')
    primary = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(len(primary) == len(exits) == 1, 'typed scratch primary and exit spans')
    begin, end = module.postcondition_bounds(base, source, mutation)
    need(same(primary[0], spans_helper.exact_span(source, path, begin, end, True, 'failed this postcondition')), 'exact scratch postcondition')
    exit_span = exits[0]
    begin, end, label = exit_span['byte_start'], exit_span['byte_end'], exit_span['label']
    need(type(begin) is int and type(end) is int, 'integer scratch exit offsets')
    if exit_span['file_name'] == str(path):
        _, first, last = base.function_bounds(source, mutation.function)
        need((begin, end, label) == (first, last, 'at the end of the function body'), 'exact scratch wrapper exit')
        wanted = spans_helper.exact_span(source, path, begin, end, False, label)
    else:
        macro_path = path.parent / '..' / BODY
        bounds = macro_bounds(body)[mutation.macro]
        need((begin, end, label) in exits_in(body, bounds), 'exact scratch return or block exit')
        call_begin, call_end = retained.invocation(source, mutation.macro)
        definition = 'macro_rules! ' + mutation.macro
        expansion = {'span': spans_helper.exact_span(source, path, call_begin, call_end),
                     'macro_decl_name': mutation.macro + '!',
                     'def_site_span': spans_helper.exact_span(body, macro_path, bounds['definition'], bounds['definition'] + len(definition))}
        wanted = spans_helper.exact_span(body, macro_path, begin, end, False, label, expansion)
    need(same(exit_span, wanted), 'exact scratch exit and expansion tree')


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 382,
            'negative_cases': 10, 'loop_invariant_negatives': 5, 'postcondition_negatives': 5,
            'shared_scratch_scan_and_staging': True, 'unchanged_raw_staging_precondition': True,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA and digest(snapshots[here.parent / BODY]) == BODY_SHA, 'exact scratch sources')
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
