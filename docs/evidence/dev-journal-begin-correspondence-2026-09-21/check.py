#!/usr/bin/env python3
"""Actual/historical Begin correspondence campaign with scoped controls."""
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
PACKET = Path('docs/evidence/dev-journal-begin-correspondence-2026-09-21')
ROOT = CRATE / 'verus/context_journal_begin_historical_v1.rs'
EXECUTION_ROOT = CRATE / 'verus/context_journal_begin_execution_v1.rs'
VIEWS = CRATE / 'verus/context_journal_begin_value_views_v1.rs'
VALUE_VIEWS = CRATE / 'verus/context_journal_value_views_v1.rs'
BRIDGE = CRATE / 'verus/context_journal_begin_historical_bodies_v1.rs'
GUARDED = CRATE / 'verus/context_journal_begin_guarded_historical_v1.rs'
WITNESS = CRATE / 'verus/context_journal_begin_historical_witnesses_v1.rs'
LIVE = CRATE / 'verus/context_journal_begin_custody_witness_v1.rs'
HISTORICAL = CRATE / 'verus/context_version_journal_begin_v1.rs'
CUSTODY = CRATE / 'verus/context_version_journal_begin_custody_v1.rs'
DECISIONS = CRATE / 'verus/context_journal_begin_decisions_v1.rs'
PROOF = CRATE / 'verus/context_journal_begin_bodies_v1.rs'
BODY = CRATE / 'src/context_version_journal/begin_bodies.rs'
RETAINED = CRATE / 'src/context_version_journal/retained_bodies.rs'
SOURCES = [ROOT, EXECUTION_ROOT, VIEWS, VALUE_VIEWS, BRIDGE, GUARDED, WITNESS, LIVE, HISTORICAL,
           CUSTODY, DECISIONS, PROOF, BODY, RETAINED,
           CRATE / 'src/context_version_journal/declarations.rs',
           CRATE / 'verus/context_journal_begin_witnesses_v1.rs',
           CRATE / 'verus/context_begin_reader_guards_v1.rs',
           CRATE / 'verus/context_journal_error_views_v1.rs',
           CRATE / 'verus/context_journal_content_views_v1.rs',
           CRATE / 'verus/context_version_journal_enrollment_v1.rs',
           CRATE / 'verus/context_producer_journal_issuance_v1.rs',
           CRATE / 'verus/context_producer_read_invariant_v1.rs',
           CRATE / 'verus/context_read_invariant_v1.rs',
           CRATE / 'verus/context_read_commit_v1.rs',
           CRATE / 'verus/context_read_preflight_v1.rs',
           CRATE / 'verus/context_version_journal_issuance_v1.rs']
POLICY_SOURCES = [PROOF, DECISIONS, VIEWS, VALUE_VIEWS, BRIDGE, GUARDED, WITNESS, LIVE,
                  CRATE / 'verus/context_journal_error_views_v1.rs',
                  CRATE / 'verus/context_journal_content_views_v1.rs']
PROJECTIONS = [(BODY, 'audited-begin-bodies.rs', 8),
               (RETAINED, 'audited-retained-bodies.rs', 7)]
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
    ("roster_extent", VIEWS, "fn begin_roster_view(", "production", "begin_destinations_correspondence", "roster.map(|_i, value| allocation_write_view(value))", "roster.map(|_i, value: AllocationWriteV1| logical::AllocationWriteV1 { allocation: allocation_reference_view(value.allocation), device: device_key_view(value.device), byte_extent: 0 })", "projection"),
    ("error_embedding", VIEWS, "fn begin_result_from<T>(", "production", "begin_destination_correspondence", "Err(read_error_embed(error))", "Err(ReadErrorV1::InvalidState)", "projection"),
    ("success_embedding", VIEWS, "fn begin_result_from<T>(", "production", "begin_canonical_correspondence", "Ok(result) => Ok(result)", "Ok(result) => Err(ReadErrorV1::InvalidState)", "projection"),
    ("allocation_presence", VIEWS, "fn begin_allocation_result_from(", "production", "begin_allocation_correspondence", "Ok(entry) => Ok(allocation_entry_from(entry))", "Ok(entry) => Err(ReadErrorV1::InvalidAllocationReference)", "projection"),
    ("reserved_presence", VIEWS, "fn begin_reserved_result_from(", "production", "begin_reserved_correspondence", "Ok(key) => Ok(writer_key_from(key))", "Ok(key) => Err(ReadErrorV1::InvalidReference)", "projection"),
    ("raw_actual_skip", BRIDGE, "fn begin_historical_exec_v1(", "production", "begin_historical_exec_v1", "let result = begin_exec_v1(actual, writer, roster);", "let result = Ok(());", "execution"),
    ("raw_model_skip", BRIDGE, "fn begin_historical_exec_v1(", "production", "begin_historical_exec_v1", "let model_result = logical::begin_exec_v1(model, model_writer, model_roster);", "let model_result = Ok(());", "execution"),
    ("guarded_actual_skip", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "let result = begin_exec_v1(actual, writer, roster);", "let result = Ok(());", "execution"),
    ("guarded_model_skip", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "let model_result = logical::begin_issued_exec_v1(model, model_writer, model_roster, Ghost(storage), Ghost(history));\n    proof {", "let model_result = Ok(());\n    proof {", "execution"),
    ("combined_guard_skip", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "let combined = logical::begin_combined_unread_exec_v1(model, model_roster);", "let combined: Result<(), logical::ReadErrorV1> = Ok(());", "execution"),
    ("executed_result_presence", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "(Some(result), model_result)", "(None, model_result)", "execution"),
    ("guard_rejection_mutation", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "let combined = logical::begin_combined_unread_exec_v1(model, model_roster);", "let combined = logical::begin_combined_unread_exec_v1(model, model_roster);\n    actual.registration_watermark = 0;", "execution"),
    ("rejected_actual_run", GUARDED, "fn begin_guarded_issued_historical_exec_v1(", "production", "begin_guarded_issued_historical_exec_v1", "match combined {\n        Err(_) => {", "match combined {\n        Err(_) => {\n            let ignored = begin_exec_v1(actual, writer, roster);", "execution"),
    ("actual_error_identity", DECISIONS, "fn begin_execution_relation_v1(", "production", "begin_paired_transition", "result == Err(error) && after == before", "result == Err(error)", "contract"),
    ("model_error_identity", HISTORICAL, "fn begin_execution_relation_v1(", "production", "begin_paired_transition", "result == Err(error) && after == before", "result == Err(error)", "contract"),
    ("scratch_frame", DECISIONS, "fn begin_success_relation_v1(", "production", "begin_paired_transition", "&&& after.scratch@ == before.scratch@", "&&& true", "contract"),
    ("free_frame", DECISIONS, "fn begin_commit_frame_v1(", "production", "begin_paired_transition", "&&& after.free == before.free", "&&& true", "contract"),
    ("member_prefix", DECISIONS, "fn begin_members_prefix_v1(", "production", "begin_prefix_correspondence", "else { begin_members_prefix_v1(before, writer, roster, (count - 1) as nat)", "else { begin_members_prefix_v1(before, writer, roster, ((count - 1) / 2) as nat)", "contract"),
    ("stage_epoch", BODY, "macro_rules! begin_stage_body {", "production", "begin_stage_exec_v1", "attempt_epoch: entry.attempt_epoch + 1", "attempt_epoch: entry.attempt_epoch", "execution"),
    ("commit_backlink", BODY, "macro_rules! begin_commit_body {", "production", "begin_commit_exec_v1", "entry.pending_member = Some(plan.member_slot)", "entry.pending_member = None", "execution"),
    ("live_reservation", LIVE, "fn begin_historical_live_custody_witness_v1(", "production", "begin_historical_live_custody_witness_v1", "model.reservations.set(0, Some(reservation));", "model.reservations.set(0, None);", "witness"),
    ("live_count", LIVE, "fn begin_historical_live_custody_witness_v1(", "production", "begin_historical_live_custody_witness_v1", "model.counts.set(0, 1);", "model.counts.set(0, 0);", "witness"),
    ("plan_lineage", VALUE_VIEWS, "fn begin_plan_view(", "production", "begin_plan_round_trip", "prior_lineage: value.prior_lineage", "prior_lineage: 0", "projection"),
    ("issued_storage_frame", CUSTODY, "fn begin_issued_relation_v1(", "production", "begin_historical_live_exercise_v1", "&&& producer_storage_frame_v1(before, after)", "&&& true", "contract"),
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
            'verified': 413, 'errors': 0, 'is-verifying-entire-crate': True,
        }), 'exact whole-root positive')
        need(not observed['diagnostics'], 'clean positive diagnostics')
        need(observed['verus'] == expected['verus'], 'exact Verus version')


def inputs(repo):
    paths = set(SOURCES) | set(PINS) | {
        PACKET / 'check.py', PACKET / 'cargo-checks.py', PACKET / 'negative-expectations.json',
        PACKET / 'audit.py', PACKET / 'README.md', Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), CRATE / 'Cargo.toml',
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
    for root in (ROOT, EXECUTION_ROOT):
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
