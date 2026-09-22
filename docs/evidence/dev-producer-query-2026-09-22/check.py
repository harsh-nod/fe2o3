#!/usr/bin/env python3
"""Actual producer-read query correspondence campaign with scoped controls."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
PACKET = Path('docs/evidence/dev-producer-query-2026-09-22')
ROOT = CRATE / 'verus/context_producer_query_historical_v1.rs'
EXECUTION_ROOT = CRATE / 'verus/context_producer_query_execution_v1.rs'
RAW_ROOT = CRATE / 'verus/context_journal_begin_execution_v1.rs'
VIEWS = CRATE / 'verus/context_reader_owner_views_v1.rs'
BRIDGE = CRATE / 'verus/context_producer_query_correspondence_v1.rs'
LIVE = CRATE / 'verus/context_producer_query_witnesses_v1.rs'
DECISIONS = CRATE / 'verus/context_producer_query_decisions_v1.rs'
PROOF = CRATE / 'verus/context_producer_query_bodies_v1.rs'
BODY = CRATE / 'src/context_producer_reads/query_bodies.rs'
WRITER = CRATE / 'src/context_version_journal/writer_lookup_bodies.rs'
HISTORICAL = CRATE / 'verus/context_producer_query_historical_bodies_v1.rs'
LIFECYCLE = CRATE / 'verus/context_producer_read_lifecycle_v1.rs'
LOOKUP = CRATE / 'src/context_version_journal/lookup_bodies.rs'
RETAINED = CRATE / 'src/context_version_journal/retained_bodies.rs'
BEGIN = CRATE / 'src/context_version_journal/begin_bodies.rs'
SOURCES = [
    WRITER, BODY, HISTORICAL,
    CRATE / 'verus/context_stable_release_execution_v1.rs',
    CRATE / 'verus/context_stable_release_decisions_v1.rs',
    CRATE / 'verus/context_stable_release_bodies_v1.rs',
    CRATE / 'verus/context_stable_release_correspondence_v1.rs',
    CRATE / 'verus/context_stable_release_witnesses_v1.rs',
    CRATE / 'src/context_read_leases/acquire_declarations.rs',
    CRATE / 'src/context_read_leases/acquire_bodies.rs',
    CRATE / 'verus/context_stable_acquire_execution_v1.rs',
    CRATE / 'verus/context_stable_acquire_decisions_v1.rs',
    CRATE / 'verus/context_stable_acquire_bodies_v1.rs',
    CRATE / 'verus/context_stable_acquire_correspondence_v1.rs',
    CRATE / 'verus/context_stable_acquire_witnesses_v1.rs',
    CRATE / 'src/context_read_leases/release_declarations.rs',
    CRATE / 'src/context_read_leases/release_bodies.rs',
    ROOT, EXECUTION_ROOT, BRIDGE, LIVE, DECISIONS, PROOF,
    CRATE / 'src/context_producer_reads/declarations.rs',
    CRATE / 'src/context_read_leases/declarations.rs',
    CRATE / 'src/context_read_leases/guard_bodies.rs',
    CRATE / 'src/context_version_journal/begin_bodies.rs',
    CRATE / 'src/context_version_journal/declarations.rs',
    CRATE / 'src/context_version_journal/lookup_bodies.rs',
    CRATE / 'src/context_version_journal/retained_bodies.rs',
    CRATE / 'verus/context_begin_reader_guards_v1.rs',
    CRATE / 'verus/context_journal_begin_bodies_v1.rs',
    CRATE / 'verus/context_journal_begin_decisions_v1.rs',
    CRATE / 'verus/context_journal_begin_execution_v1.rs',
    CRATE / 'verus/context_journal_begin_historical_bodies_v1.rs',
    CRATE / 'verus/context_journal_begin_value_views_v1.rs',
    CRATE / 'verus/context_journal_begin_witnesses_v1.rs',
    CRATE / 'verus/context_journal_content_views_v1.rs',
    CRATE / 'verus/context_journal_error_views_v1.rs',
    CRATE / 'verus/context_journal_value_views_v1.rs',
    CRATE / 'verus/context_producer_journal_issuance_v1.rs',
    CRATE / 'verus/context_producer_read_invariant_v1.rs',
    CRATE / 'verus/context_read_commit_v1.rs',
    CRATE / 'verus/context_read_invariant_v1.rs',
    CRATE / 'verus/context_read_preflight_v1.rs',
    CRATE / 'verus/context_reader_guards_bodies_v1.rs',
    CRATE / 'verus/context_reader_guards_correspondence_v1.rs',
    CRATE / 'verus/context_reader_guards_decisions_v1.rs',
    CRATE / 'verus/context_reader_guards_execution_v1.rs',
    CRATE / 'verus/context_reader_guards_witness_v1.rs',
    CRATE / 'verus/context_reader_owner_views_v1.rs',
    CRATE / 'verus/context_version_journal_begin_custody_v1.rs',
    CRATE / 'verus/context_version_journal_begin_v1.rs',
    CRATE / 'verus/context_version_journal_enrollment_v1.rs',
    CRATE / 'verus/context_version_journal_issuance_v1.rs',
]
POLICY_SOURCES = [
    HISTORICAL,
    CRATE / 'verus/context_stable_release_decisions_v1.rs',
    CRATE / 'verus/context_stable_release_bodies_v1.rs',
    CRATE / 'verus/context_stable_release_correspondence_v1.rs',
    CRATE / 'verus/context_stable_release_witnesses_v1.rs',
    CRATE / 'src/context_read_leases/acquire_declarations.rs',
    CRATE / 'verus/context_stable_acquire_decisions_v1.rs',
    CRATE / 'verus/context_stable_acquire_bodies_v1.rs',
    CRATE / 'verus/context_stable_acquire_correspondence_v1.rs',
    CRATE / 'verus/context_stable_acquire_witnesses_v1.rs',
    BRIDGE, LIVE, DECISIONS, PROOF,
    CRATE / 'src/context_read_leases/release_declarations.rs',
    CRATE / 'verus/context_journal_begin_bodies_v1.rs',
    CRATE / 'verus/context_journal_begin_decisions_v1.rs',
    CRATE / 'verus/context_journal_begin_value_views_v1.rs',
    CRATE / 'verus/context_journal_value_views_v1.rs',
    CRATE / 'verus/context_journal_begin_historical_bodies_v1.rs',
    CRATE / 'verus/context_journal_error_views_v1.rs',
    CRATE / 'verus/context_journal_content_views_v1.rs',
    CRATE / 'verus/context_reader_guards_bodies_v1.rs',
    CRATE / 'verus/context_reader_guards_decisions_v1.rs',
    CRATE / 'verus/context_reader_guards_correspondence_v1.rs',
    CRATE / 'verus/context_reader_guards_witness_v1.rs',
    CRATE / 'verus/context_reader_owner_views_v1.rs',
    CRATE / 'src/context_read_leases/declarations.rs',
    CRATE / 'src/context_producer_reads/declarations.rs',
]
PROJECTIONS = [(BEGIN, 'audited-begin-bodies.rs', 8),
               (RETAINED, 'audited-retained-bodies.rs', 7),
               (LOOKUP, 'audited-lookup-bodies.rs', 1),
               (CRATE / 'src/context_read_leases/guard_bodies.rs', 'audited-guard-bodies.rs', 5),
               (CRATE / 'src/context_read_leases/acquire_bodies.rs', 'audited-acquire-bodies.rs', 9),
               (CRATE / 'src/context_read_leases/release_bodies.rs', 'audited-release-bodies.rs', 8),
               (WRITER, 'audited-writer-lookup-bodies.rs', 1),
               (BODY, 'audited-producer-query-bodies.rs', 7)]
BASE = CRATE / 'verus/check-journal-issuance.py'
POLICY = Path('examples/wave64_collectives_v1/check-proof-source.py')
CLOSURE = Path('examples/row_softmax_v1/verify-verus-closure.sh')
MANIFEST = CRATE / 'verus/pins/VERUS_CLOSURE_MANIFEST'
PINS = {
    BASE: '36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480',
    POLICY: 'a3071ad8f0025a59d0c70dcf2427c1eb43b0c02fe484474a84d0377d99ccb887',
    CLOSURE: 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c',
    MANIFEST: 'f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01',
    CRATE / 'src/context_version_journal/declarations.rs':
        'de362edd368eda151aa2a0a112cf113af91d42a2fb03259707db77c0581e5c16',
}
# name, source, unique declaration anchor, module, selected function, before, after, kind
MUTATIONS = [
    ("writer_key", WRITER, "macro_rules! writer_lookup_body {", "production", "ContextVersionJournalV1::lookup_writer", "!$same(key, $reference.key)", "false", "execution"),
    ("writer_context", WRITER, "macro_rules! writer_lookup_body {", "production", "ContextVersionJournalV1::lookup_writer", "key.context_generation != $contents.context_generation", "false", "execution"),
    ("writer_payload", WRITER, "macro_rules! writer_lookup_body {", "production", "ContextVersionJournalV1::lookup_writer", "ContextWriterStateV1::Unknown { member_count: count }", "ContextWriterStateV1::Unknown { member_count: 0 }", "execution"),
    ("producer_slot", BODY, "macro_rules! producer_writer_same_body {", "production", "producer_writer_same_exec_v1", "$left.slot == $right.slot", "true", "execution"),
    ("reference_incarnation", BODY, "macro_rules! producer_reference_same_body {", "production", "producer_reference_same_exec_v1", "$left.incarnation == $right.incarnation", "true", "execution"),
    ("reference_consumer", BODY, "macro_rules! producer_reference_same_body {", "production", "producer_reference_same_exec_v1", "producer_query_consumer_same_exec_v1($left.consumer, $right.consumer)", "true", "execution"),
    ("device_local", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "state.device.local != read.device.local", "false", "execution"),
    ("range_endpoint", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "if end > read.byte_extent", "if end >= read.byte_extent", "execution"),
    ("producer_kind", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "!matches!($request.producer.key.kind, ContextWriterKindV1::Submission)", "false", "execution"),
    ("attempt_epoch", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "state.attempt_epoch != read.attempt_epoch", "false", "execution"),
    ("pending_identity", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "!$same(writer, $request.producer)", "false", "execution"),
    ("unknown_status", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "ContextWriterStateV1::Unknown { .. } => Ok(ContextProducerReadStatusV1::Unknown)", "ContextWriterStateV1::Unknown { .. } => Ok(ContextProducerReadStatusV1::Pending)", "execution"),
    ("resolved_slot", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "match state.pending_writer {", "match journal.lookup_writer($request.producer) { Ok(_) => {}, Err(error) => return Err(error) };\n        match state.pending_writer {", "execution"),
    ("resolved_lineage", BODY, "macro_rules! producer_status_body {", "production", "ContextProducerReadJournalV1::status", "None if state.content_lineage == read.content_lineage", "None if true", "execution"),
    ("validate_pending", BODY, "macro_rules! producer_validate_body {", "production", "ContextProducerReadJournalV1::validate_producer_read", "Ok(ContextProducerReadStatusV1::Pending) => Ok(())", "Ok(ContextProducerReadStatusV1::Unknown) => Ok(())", "execution"),
    ("inspect_status_error", BODY, "macro_rules! producer_inspect_body {", "production", "ContextProducerReadJournalV1::inspect_producer_read", "Err(error) => Err(error)", "Err(_error) => Ok((entry.request, ContextProducerReadStatusV1::Pending))", "execution"),
    ("query_status", BODY, "macro_rules! producer_query_body {", "production", "ContextProducerReadJournalV1::producer_read_status", "Ok((_, status)) => Ok(status)", "Ok((_, _status)) => Ok(ContextProducerReadStatusV1::Pending)", "execution"),
    ("raw_domain", PROOF, "impl ContextProducerReadJournalV1 {", "production", "producer_status_historical_exec_v1", "ensures result == producer_status_decision_v1(self.stable.journal, *request),", "requires self.next_incarnation > 0,\n        ensures result == producer_status_decision_v1(self.stable.journal, *request),", "contract"),
    ("skip_actual_status", BRIDGE, "fn producer_status_historical_exec_v1(", "production", "producer_status_historical_exec_v1", "let result = actual.status(request);", "let result: Result<ContextProducerReadStatusV1, ReadErrorV1> = Err(ReadErrorV1::InvalidReference);", "execution"),
    ("skip_historical_status", BRIDGE, "fn producer_status_historical_exec_v1(", "production", "producer_status_historical_exec_v1", "let model_result = logical::producer_status_exec_v1(&model.stable.journal, model_request);", "let model_result: Result<logical::ProducerStatusV1, logical::ReadErrorV1> = Err(logical::ReadErrorV1::InvalidReference);", "execution"),
    ("skip_actual_validate", BRIDGE, "fn producer_validate_historical_exec_v1(", "production", "producer_validate_historical_exec_v1", "let result = actual.validate_producer_read(request);", "let result: Result<(), ReadErrorV1> = Err(ReadErrorV1::InvalidReference);", "execution"),
    ("skip_historical_validate", BRIDGE, "fn producer_validate_historical_exec_v1(", "production", "producer_validate_historical_exec_v1", "let model_result = logical::producer_validate_exec_v1(&model.stable.journal, model_request);", "let model_result: Result<(), logical::ReadErrorV1> = Err(logical::ReadErrorV1::InvalidReference);", "execution"),
    ("skip_actual_lookup", BRIDGE, "fn producer_lookup_historical_exec_v1(", "production", "producer_lookup_historical_exec_v1", "let result = actual.lookup_producer_read(reference);", "let result: Result<ContextProducerReadV1, ReadErrorV1> = Err(ReadErrorV1::InvalidReference);", "execution"),
    ("skip_historical_lookup", BRIDGE, "fn producer_lookup_historical_exec_v1(", "production", "producer_lookup_historical_exec_v1", "let model_result = logical::producer_lookup_exec_v1(model, model_reference);", "let model_result: Result<logical::ProducerReadV1, logical::ReadErrorV1> = Err(logical::ReadErrorV1::InvalidReference);", "execution"),
    ("skip_actual_query", BRIDGE, "fn producer_query_historical_exec_v1(", "production", "producer_query_historical_exec_v1", "let result = actual.producer_read_status(reference);", "let result: Result<ContextProducerReadStatusV1, ReadErrorV1> = Err(ReadErrorV1::InvalidReference);", "execution"),
    ("skip_historical_query", BRIDGE, "fn producer_query_historical_exec_v1(", "production", "producer_query_historical_exec_v1", "let model_result = historical_producer_query_exec_v1(model, model_reference);", "let model_result: Result<logical::ProducerStatusV1, logical::ReadErrorV1> = Err(logical::ReadErrorV1::InvalidReference);", "execution"),
    ("status_projection", BRIDGE, "fn producer_status_from(", "production", "producer_status_correspondence", "logical::ProducerStatusV1::NoEffect => ContextProducerReadStatusV1::NoEffect", "logical::ProducerStatusV1::NoEffect => ContextProducerReadStatusV1::Success", "projection"),
    ("unknown_witness", LIVE, "fn producer_query_raw_witness_v1(", "production", "producer_query_raw_witness_v1", "else if phase == 1 { ContextProducerReadStatusV1::Unknown }", "else if phase == 1 { ContextProducerReadStatusV1::Pending }", "witness"),
    ("precedence_witness", LIVE, "fn producer_query_raw_witness_v1(", "production", "producer_query_raw_witness_v1", "let stale = ContextProducerReadReferenceV1 { incarnation: 2, ..reference };\n    let model_stale = logical::ProducerReadReferenceV1 { incarnation: 2, ..model_reference };", "let stale = ContextProducerReadReferenceV1 { incarnation: 1, ..reference };\n    let model_stale = logical::ProducerReadReferenceV1 { incarnation: 1, ..model_reference };", "witness"),
]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def same(left, right):
    return json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)


def load(repo, path, name):
    data = (repo / path).read_bytes()
    need(sha(data) == PINS[path], 'pinned leaf utility: ' + str(path))
    spec = importlib.util.spec_from_file_location(name, repo / path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(data, str(repo / path), 'exec'), module.__dict__)
    return module


def mutated(data, mutation):
    name, path, anchor, module, function, before, after, kind = mutation
    source = data.decode('ascii')
    need(source.count(anchor) == 1, 'unique mutation declaration')
    start = source.index(anchor)
    end = source.index('\n}\n', start) + 2
    fragment = source[start:end]
    need(fragment.count(before) == 1 and before != after, 'unique scoped mutation')
    return (source[:start] + fragment.replace(before, after, 1) + source[end:]).encode('ascii')


def command(verus, source, mutation):
    result = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '600', str(verus),
              '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
              '--error-format=json', '--no-report-long-running', '--num-threads', '4']
    if mutation:
        result += ['--verify-only-module', mutation[3]] if mutation[3] else ['--verify-root']
        result += ['--verify-function', mutation[4]]
    return [*result, str(source)]


def normalized(base, stdout, stderr, folder):
    report = base.unique_json(stdout)
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    # Paths are the only normalized fields. Canonicalize top-level emission
    # order, retaining every complete record, nested order and duplicate count.
    diagnostics = base.unique_json(json.dumps(diagnostics).replace(str(folder), '<CASE>'))
    diagnostics.sort(key=lambda row: json.dumps(row, sort_keys=True))
    return {'result': report['verification-results'], 'diagnostics': diagnostics, 'verus': report['verus']}


def check_result(base, status, stdout, stderr, folder, mutation, expected):
    need(type(status) is int and status == (1 if mutation else 0), 'normal expected exit')
    observed = normalized(base, stdout, stderr, folder)
    if mutation:
        result = observed['result']
        need(result['encountered-vir-error'] is False and result.get('success', False) is False
             and type(result['errors']) is int and result['errors'] > 0
             and type(result['verified']) is int and result['is-verifying-entire-crate'] is False,
             'scoped verification failure, not a frontend error')
        errors = [row for row in observed['diagnostics'] if row['level'] == 'error']
        logical_errors = {
            'assertion failed', 'postcondition not satisfied', 'precondition not satisfied',
            'invariant not satisfied at end of loop body',
        }
        need(any(row['message'] in logical_errors for row in errors)
             and all(row['message'] in logical_errors or re.fullmatch(
                 r'aborting due to \d+ previous errors?', row['message']) for row in errors),
             'logical failure, not a syntax error or solver resource limit')
        need(same(observed, expected[mutation[0]]), 'exact intended failure: ' + mutation[0])
    else:
        need(same(observed['result'], {
            'encountered-error': False, 'encountered-vir-error': False, 'success': True,
            'verified': 546, 'errors': 0, 'is-verifying-entire-crate': True,
        }), 'exact whole-root positive')
        need(not observed['diagnostics'], 'clean positive diagnostics')
        need(observed['verus'] == expected['verus'], 'exact Verus version')


def inputs(repo):
    paths = set(SOURCES) | set(PINS) | {LIFECYCLE,
        PACKET / 'check.py', PACKET / 'cargo-checks.py', PACKET / 'negative-expectations.json',
        PACKET / 'audit.py', PACKET / 'README.md', PACKET / 'performance.py', Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), CRATE / 'Cargo.toml',
        Path('crates/fe2o3-runtime/src/context.rs'),
        Path('docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'),
    }
    paths.update(p.relative_to(repo) for p in (repo / CRATE / 'src').rglob('*') if p.is_file())
    return sorted(paths)


def snapshot(repo, probe):
    paths = set(SOURCES) | set(PINS) if probe else inputs(repo)
    return {str(path): sha((repo / path).read_bytes()) for path in sorted(paths)}


def policy_check(repo, policy, folder):
    for source in POLICY_SOURCES:
        policy.scan(folder / source)
    # The fixed include/declaration envelopes are replayed byte-for-byte; scan
    # all shared executable tokens after removing only macro declaration words.
    for path, projection, count in PROJECTIONS:
        source = (folder / path).read_text(encoding='ascii')
        need(source.count('macro_rules!') == count, 'exact shared body macro roster')
        stripped = folder / projection
        stripped.write_text(source.replace('macro_rules!', ''), encoding='ascii')
        policy.scan(stripped)
    for root in (ROOT, EXECUTION_ROOT, RAW_ROOT):
        need((folder / root).read_bytes() == (repo / root).read_bytes(), 'exact include envelope')


def campaign(args):
    repo, output, verus = args.repo.resolve(), args.output.resolve(), args.verus.resolve(strict=True)
    base = load(repo, BASE, 'ordering_recorder')
    policy = load(repo, POLICY, 'ordering_policy')
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    for path, pin in PINS.items():
        need(sha((repo / path).read_bytes()) == pin, 'pinned source/closure: ' + str(path))
    output.mkdir()
    before = snapshot(repo, args.probe)
    source_commit = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
    if not args.probe:
        for path, digest in before.items():
            data = subprocess.check_output(['git', '-C', str(repo), 'show', source_commit + ':' + path])
            need(sha(data) == digest, 'signed source/worktree correspondence: ' + path)
    (output / 'inputs-before.json').write_text(json.dumps(before, indent=2) + '\n')
    (output / 'source.json').write_text(json.dumps({'commit': source_commit, 'probe': args.probe, 'verus': str(verus)}, indent=2) + '\n')
    expected = None if args.probe else base.unique_json((repo / PACKET / 'negative-expectations.json').read_text())
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'CARGO_HOME': '/home/harsh/.cargo', 'RUSTUP_HOME': '/home/harsh/.rustup', 'VERUS_Z3_PATH': str(verus.parent / 'z3')}
    for phase in ['before', 'after']:
        status, _, _ = base.run_owned(['/bin/sh', str(repo / CLOSURE), str(verus.parent), str(repo / MANIFEST)],
                                      120, output / ('closure-' + phase), env)
        need(status == 0, 'authenticated Verus distribution')
        if phase == 'after':
            break
        cases = [(m[0], m) for m in MUTATIONS]
        if not args.probe:
            cases = [('positive-before', None), *cases, ('positive-after', None)]
        for name, mutation in cases:
            folder = output / name
            for path in SOURCES:
                target = folder / path
                target.parent.mkdir(parents=True, exist_ok=True)
                data = (repo / path).read_bytes()
                target.write_bytes(mutated(data, mutation) if mutation and path == mutation[1] else data)
            policy_check(repo, policy, folder)
            frozen = {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}
            status, stdout, stderr = base.run_owned(command(verus, folder / ROOT, mutation), 610, folder / 'solver', env)
            need(frozen == {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}, 'staged source drift')
            if not args.probe:
                check_result(base, status, stdout, stderr, folder, mutation, expected)
            print(name, status, base.unique_json(stdout)['verification-results'], flush=True)
    after = snapshot(repo, args.probe)
    need(before == after, 'source drift during campaign')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--probe', action='store_true', help='development controls only; never accepted by the auditor')
    campaign(parser.parse_args())


if __name__ == '__main__':
    main()
