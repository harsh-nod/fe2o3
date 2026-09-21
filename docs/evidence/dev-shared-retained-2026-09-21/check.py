#!/usr/bin/env python3
"""Qualify shared retained admission/Unknown bodies and their logical composition."""
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
COUNT = 382
SOURCE = 'context_shared_retained_v1.rs'
SOURCE_SHA = '539addf418fd96160725e1df1a223e1d347eb171e0d2b675617c4a9502cabbf0'
BODY = Path('src/context_version_journal/retained_bodies.rs')
BODY_SHA = '87b49950e6b5873772c713b448860a336011ea3a1b2be40790f3ed4db8e50e44'
ADAPTER = Path('src/context_version_journal/retained.rs')
ADAPTER_SHA = '7d369b0c1803d99a699ebc26fb330f59b61754cf5004d27033f155df27976dde'
PRIOR = Path('docs/evidence/dev-shared-settlement-storage-2026-09-21/check.py')
PRIOR_SHA = '8a19a1c650c255adad0e534dda7f5cf3614292453573289049e1249603a13b62'
PREFIX = ('// Shared retained admission and Unknown execution; allocation/Context gates remain separate.\n'
          'include!("context_settlement_shared_storage_v1.rs");\n'
          'include!("../src/context_version_journal/retained_bodies.rs");\n')
BODY_PREFIX = '// Shared executable admission/Unknown bodies. Loop annotations carry no runtime code.\n'
PARAMETERS = {
    'retained_writer_key_body': '($left:ident, $right:ident)',
    'retained_allocation_less_body': '($left:ident, $right:ident)',
    'retained_allocation_body': '($journal:ident, $reference:ident)',
    'retained_header_body': '($journal:ident, $writer:ident, $allow_unknown:ident)',
    'retained_member_body': '($journal:ident, $writer:ident, $head:ident, $previous:ident)',
    'retained_chain_body': ('($syntax:ident, $journal:ident, $writer:ident, $initial:ident, $count:ident,\n'
                            '     $head:ident, $previous:ident, $index:ident, [$($annotations:tt)*])'),
    'retained_unknown_body': '($journal:ident, $writer:ident)',
}
ANNOTATIONS = ('invariant index <= count, count <= journal.allocation_capacity, (count == 0) == initial.is_none(),\n'
               '            retained_scan_v1(*journal, writer, initial, count as nat, None)\n'
               '                == retained_scan_v1(*journal, writer, head, (count - index) as nat, previous),\n'
               '        decreases count - index,')
CHAIN_CALL = ('retained_chain_body!(verus_exec_expr, journal, writer, initial, count, head, previous, index, [\n'
              '        ' + ANNOTATIONS + '\n    ])')
RUST = [Path('crates/fe2o3-runtime-model') / p for p in (
    BODY, 'src/context_version_journal/retained.rs', 'src/context_version_journal/retained_tests.rs')]
Mutation = namedtuple('Mutation', 'name macro function before after postcondition assertion', defaults=[None])


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned shared storage checker')
    spec = importlib.util.spec_from_file_location('shared_retained_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.retained_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.retained_parent.extra_paths(module), PRIOR,
            Path('crates/fe2o3-runtime-model/verus') / SOURCE, *RUST]


def macro_bounds(body):
    need(body.isascii() and body.startswith(BODY_PREFIX), 'ASCII retained macro preamble')
    cursor, result = len(BODY_PREFIX), {}
    for name, parameters in PARAMETERS.items():
        chain = name == 'retained_chain_body'
        prefix = 'macro_rules! ' + name + ' {\n    ' + parameters + ' => '
        prefix += '{\n        $syntax!({\n' if chain else '{{\n'
        suffix = '        })\n    };\n}\n' if chain else '    }};\n}\n'
        need(body.startswith(prefix, cursor), 'exact macro envelope: ' + name)
        start = cursor + len(prefix)
        end = body.index(suffix, start)
        result[name] = {'definition': cursor, 'start': start, 'end': end,
                        'block_start': start - 2, 'block_end': end + (9 if chain else 5)}
        cursor = end + len(suffix)
        if name != 'retained_unknown_body':
            need(body[cursor:cursor + 1] == '\n', 'single inter-macro separator')
            cursor += 1
    need(cursor == len(body), 'no extra retained source')
    return result


def mutations(_base=None):
    rows = [
        ('writer_context', 'writer_key', '$left.context_generation == $right.context_generation', 'true', 'result == (left == right)'),
        ('writer_kind', 'writer_key', '_ => false,', '_ => true,', 'result == (left == right)'),
        ('strict_order', 'allocation_less', '$left.local < $right.local', '$left.local <= $right.local', 'result == enrollment_key_less_v1'),
        ('allocation_context', 'allocation', '\n            || entry.key.context_generation != $journal.context_generation', '', 'result == begin_exact_allocation_v1'),
        ('allocation_bounds_error', 'allocation',
         'if $reference.slot >= $journal.allocations.len() {\n            return Err(ReadErrorV1::InvalidAllocationReference);',
         'if $reference.slot >= $journal.allocations.len() {\n            return Err(ReadErrorV1::InvalidState);', 'result == begin_exact_allocation_v1'),
        ('header_context', 'header', '\n            || key.context_generation != $journal.context_generation', '', 'result == retained_header_decision_v1'),
        ('header_unknown', 'header', 'if $allow_unknown =>', 'if true =>', 'result == retained_header_decision_v1'),
        ('member_writer', 'member', 'member.writer.slot != $writer.slot', 'false', 'result == retained_member_decision_v1'),
        ('member_backlink', 'member', 'allocation.pending_member != Some(slot)', 'false', 'result == retained_member_decision_v1'),
        ('member_epoch', 'member', 'allocation.attempt_epoch != member.attempt_epoch', 'false', 'result == retained_member_decision_v1'),
        ('member_lineage', 'member', 'allocation.content_lineage != member.prior_lineage', 'false', 'result == retained_member_decision_v1'),
        ('chain_guard', 'chain',
         'if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {\n                return Err(ReadErrorV1::InvalidState);',
         'if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {\n                return Err(ReadErrorV1::InvalidReference);', 'result == retained_chain_decision_v1'),
        ('chain_tail', 'chain', 'if $head.is_some() {', 'if false {', 'result == retained_chain_decision_v1'),
    ]
    result = [Mutation(name, 'retained_' + family + '_body', 'shared_retained_' + family + '_v1', before, after, post)
              for name, family, before, after, post in rows]
    result.extend([
        Mutation('unknown_rejection', 'retained_unknown_body', 'shared_retained_unknown_v1',
                 'if result.is_err() {', 'if false {', 'unknown_execution_relation_v1',
                 'result.is_err() ==> *journal == before'),
        Mutation('unknown_variant', 'retained_unknown_body', 'shared_retained_unknown_v1',
                 'Some(WriterEntryV1::Unknown {', 'Some(WriterEntryV1::Pending {', 'unknown_execution_relation_v1',
                 'journal.writers@ =~= before.writers@.update(writer.slot as int,\n'
                 '                Some(unknown_writer_v1(before.writers@[writer.slot as int].unwrap())))'),
    ])
    return result


def candidate(body, mutation):
    if mutation is None:
        return body
    bounds = macro_bounds(body)[mutation.macro]
    begin, end = bounds['start'], bounds['end']
    inner = body[begin:end]
    need(mutation.before and mutation.before != mutation.after and inner.count(mutation.before) == 1, 'unique retained mutation')
    offset = inner.index(mutation.before)
    changed = inner[:offset] + mutation.after + inner[offset + len(mutation.before):]
    need(changed[:offset] + mutation.before + changed[offset + len(mutation.after):] == inner,
         'reversible retained mutation at exact offset')
    return body[:begin] + changed + body[end:]


def audit_adapter(adapter):
    # Pin the reviewed complete adapter, not comment-sensitive token substrings.
    need(digest(adapter) == ADAPTER_SHA, 'exact Rust identity adapter and empty loop annotations')


def audit_sources(module, source, body, folder):
    prior = module.retained_parent
    inherited_source = (folder / 'verus' / prior.SOURCE).read_text(encoding='ascii')
    inherited_body = (folder / prior.BODY).read_text(encoding='ascii')
    need(digest(inherited_source.encode()) == prior.SOURCE_SHA and digest(inherited_body.encode()) == prior.BODY_SHA,
         'pinned inherited shared source bodies')
    prior.audit_sources(module, inherited_source, inherited_body, folder)
    need(source.isascii() and source.startswith(PREFIX) and source.count('include!') == 2, 'exact retained imports')
    need(source.count(CHAIN_CALL) == 1, 'exact proof-only loop annotations and syntax adapter')
    policy = module.inherited().POLICY
    path = folder / 'verus/audited-retained-root.rs'
    path.write_text(source[len(PREFIX):], encoding='ascii')
    policy.scan(path)
    for name, bounds in macro_bounds(body).items():
        path = folder / 'verus' / ('audited-' + name + '.rs')
        path.write_text(body[bounds['start']:bounds['end']], encoding='ascii')
        policy.scan(path)


def stage(module, snapshots, folder, mutation):
    prior, here = module.retained_parent, module.HERE
    audit_adapter(snapshots[here.parent / ADAPTER])
    prior.stage(module, snapshots, folder, None)
    source = snapshots[here / SOURCE].decode('ascii')
    body = candidate(snapshots[here.parent / BODY].decode('ascii'), mutation)
    (folder / 'verus' / SOURCE).write_text(source, encoding='ascii')
    (folder / BODY).write_text(body, encoding='ascii')
    audit_sources(module, source, body, folder)
    return source, body, {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def invocation(source, name):
    anchor = name + '!('
    need(source.count(anchor) == 1, 'unique retained macro invocation')
    begin = source.index(anchor)
    cursor, depth = begin + len(anchor), 1
    while depth:
        need(cursor < len(source), 'complete macro invocation')
        depth += (source[cursor] == '(') - (source[cursor] == ')')
        cursor += 1
    return begin, cursor


def exits_in(body, bounds):
    """Enumerate complete returns in the fixed, comment-free shared-body grammar."""
    first, last = bounds['start'], bounds['end']
    inner = body[first:last]
    need('//' not in inner and '/*' not in inner and '"' not in inner, 'simple retained exit grammar')
    matches = list(re.finditer(r'\breturn (?:Err\((?:ReadErrorV1::[A-Za-z]+|error)\)|result)(?=[;,])', inner))
    need(len(matches) == len(re.findall(r'\breturn\b', inner)), 'all retained returns recognized')
    return {(first + m.start(), first + m.end(), 'at this exit') for m in matches} | {
        (bounds['block_start'], bounds['block_end'], 'at the end of the function body')}


def check_result(module, status, stdout, stderr, source, body, path, mutation):
    base, prior = module.inherited().BASE, module.retained_parent
    if mutation is None:
        module.raw_checker.check_result(module, status, stdout, stderr, source, path, None)
        return
    need(type(status) is int and status == 1, 'expected normal negative exit')
    actual = base.unique_json(stdout)['verification-results']
    expected = {'encountered-error': True, 'encountered-vir-error': False, 'success': False,
                'verified': COUNT - 1, 'errors': 1, 'is-verifying-entire-crate': True}
    need(same(actual, expected), 'exact whole-crate negative result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    need(len(diagnostics) in (1, 2), 'one intended negative and optional footer')
    if len(diagnostics) == 2:
        footer = diagnostics[1]
        need(footer['$message_type'] == 'diagnostic' and footer['level'] == 'error'
             and footer['message'] == 'aborting due to 1 previous error' and footer['code'] is None
             and footer['spans'] == [] and footer['children'] == [], 'exact negative footer')
    error = diagnostics[0]
    message = 'assertion failed' if mutation.assertion else 'postcondition not satisfied'
    need(error['$message_type'] == 'diagnostic' and error['level'] == 'error' and error['message'] == message
         and error['code'] is None and error['children'] == [], 'intended obligation failure')
    spans = error['spans']
    if mutation.assertion:
        needle = 'assert(' + mutation.assertion + ');'
        _, begin, end = base.function_bounds(source, mutation.function)
        need(source[begin:end].count(needle) == 1, 'unique intended supporting assertion')
        start = source.index(needle, begin) + len('assert(')
        expected_span = prior.exact_span(source, path, start, start + len(mutation.assertion), True, 'assertion failed')
        need(same(spans, [expected_span]), 'exact supporting assertion span')
        return
    need(len(spans) == 2, 'one postcondition and one exit')
    primary = [s for s in spans if s['is_primary'] is True]
    exits = [s for s in spans if s['is_primary'] is False]
    need(len(primary) == len(exits) == 1, 'typed primary and exit spans')
    begin, end = module.postcondition_bounds(base, source, mutation)
    need(same(primary[0], prior.exact_span(source, path, begin, end, True, 'failed this postcondition')), 'exact retained postcondition')
    exit_span = exits[0]
    begin, end = exit_span['byte_start'], exit_span['byte_end']
    need(type(begin) is int and type(end) is int, 'integer exit offsets')
    label = exit_span['label']
    need(label in ('at this exit', 'at the end of the function body'), 'executable exit label')
    if exit_span['file_name'] == str(path):
        _, first, last = base.function_bounds(source, mutation.function)
        need((begin, end, label) == (first, last, 'at the end of the function body'), 'exact intended wrapper exit')
        wanted = prior.exact_span(source, path, begin, end, False, label)
    else:
        macro_path = path.parent / '..' / BODY
        bounds = macro_bounds(body)[mutation.macro]
        need((begin, end, label) in exits_in(body, bounds), 'exact intended shared return or block exit')
        call_begin, call_end = invocation(source, mutation.macro)
        definition = 'macro_rules! ' + mutation.macro
        expansion = {'span': prior.exact_span(source, path, call_begin, call_end),
                     'macro_decl_name': mutation.macro + '!',
                     'def_site_span': prior.exact_span(body, macro_path, bounds['definition'], bounds['definition'] + len(definition))}
        wanted = prior.exact_span(body, macro_path, begin, end, False, label, expansion)
    need(same(exit_span, wanted), 'exact retained exit and expansion tree')


def report():
    return {'development_checks_passed': True, 'positive_obligations': COUNT, 'inherited_obligations': 367,
            'negative_cases': 15, 'shared_retained_admission_and_unknown': True, 'logical_settlement_composition': True,
            'issued_custody_preservation': True, 'repeated_unknown_identity': True,
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
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA and digest(snapshots[here.parent / BODY]) == BODY_SHA, 'exact retained sources')
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
