#!/usr/bin/env python3
"""Actual-type Begin execution campaign with scoped controls."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

sys.dont_write_bytecode = True
CRATE = Path('crates/fe2o3-runtime-model')
PACKET = Path('docs/evidence/dev-journal-begin-execution-2026-09-21')
ROOT = CRATE / 'verus/context_journal_begin_execution_v1.rs'
PROOF = CRATE / 'verus/context_journal_begin_bodies_v1.rs'
DECISIONS = CRATE / 'verus/context_journal_begin_decisions_v1.rs'
WITNESS = CRATE / 'verus/context_journal_begin_witnesses_v1.rs'
BODY = CRATE / 'src/context_version_journal/begin_bodies.rs'
RETAINED = CRATE / 'src/context_version_journal/retained_bodies.rs'
SOURCES = [ROOT, PROOF, DECISIONS, WITNESS, BODY, RETAINED,
           CRATE / 'src/context_version_journal/declarations.rs']
POLICY_SOURCES = [PROOF, DECISIONS, WITNESS]
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
    ("reserved_key", BODY, "macro_rules! begin_reserved_body {", "", "begin_reserved_exec_v1", "shared_retained_writer_key_v1(key, $writer.key)", "true", "execution"),
    ("reserved_context", BODY, "macro_rules! begin_reserved_body {", "", "begin_reserved_exec_v1", "&& key.context_generation == $journal.context_generation", "&& true", "execution"),
    ("canonical", BODY, "macro_rules! begin_canonical_body {", "", "begin_canonical_exec_v1", "if !shared_retained_allocation_less_v1(", "if shared_retained_allocation_less_v1(", "execution"),
    ("device", BODY, "macro_rules! begin_destinations_body {", "", "begin_destinations_exec_v1", "entry.device.local != destination.device.local", "false", "execution"),
    ("extent", BODY, "macro_rules! begin_destinations_body {", "", "begin_destinations_exec_v1", "if entry.byte_extent != destination.byte_extent", "if false", "execution"),
    ("busy", BODY, "macro_rules! begin_destinations_body {", "", "begin_destinations_exec_v1", "if entry.pending_member.is_some()", "if false", "execution"),
    ("epoch", BODY, "macro_rules! begin_destinations_body {", "", "begin_destinations_exec_v1", "checked_add(1)", "checked_add(0)", "execution"),
    ("slot_bound", BODY, "macro_rules! begin_slots_body {", "", "begin_slots_exec_v1", "member >= $journal.members.len()", "member > $journal.members.len()", "execution"),
    ("slot_vacancy", BODY, "macro_rules! begin_slots_body {", "", "begin_slots_exec_v1", "$journal.members[member].is_some()", "false", "execution"),
    ("slot_scratch", BODY, "macro_rules! begin_slots_body {", "", "begin_slots_exec_v1", "if $journal.scratch[$index].is_some()", "if false", "execution"),
    ("reserved_count", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "checked_sub(1)", "checked_sub(0)", "execution"),
    ("roster_capacity", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if count > $journal.allocation_capacity", "if false", "execution"),
    ("canonical_skip", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if let Err(error) = begin_canonical_exec_v1($roster) { return Err(error); }", "", "execution"),
    ("member_capacity", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if count > $journal.member_free.len()", "if false", "execution"),
    ("scratch_length", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if count > $journal.scratch.len()", "if false", "execution"),
    ("stage_epoch", BODY, "macro_rules! begin_stage_body {", "", "begin_stage_exec_v1", "attempt_epoch: entry.attempt_epoch + 1", "attempt_epoch: entry.attempt_epoch", "execution"),
    ("stage_lineage", BODY, "macro_rules! begin_stage_body {", "", "begin_stage_exec_v1", "prior_lineage: entry.content_lineage", "prior_lineage: 0", "execution"),
    ("stage_free_order", BODY, "macro_rules! begin_stage_body {", "", "begin_stage_exec_v1", "$journal.member_free[$journal.member_free.len() - 1 - $index]", "$journal.member_free[$index]", "execution"),
    ("commit_clear", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "$journal.scratch[$index].take()", "$journal.scratch[$index]", "execution"),
    ("commit_next", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "if $index + 1 == $count", "if true", "execution"),
    ("commit_pop", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "let _ = $journal.member_free.pop();", "let _ = ();", "execution"),
    ("commit_epoch", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "entry.attempt_epoch = plan.attempt_epoch", "entry.attempt_epoch = 0", "execution"),
    ("commit_backlink", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "entry.pending_member = Some(plan.member_slot)", "entry.pending_member = None", "execution"),
    ("commit_count", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "$journal.reserved_count = $reserved_count", "$journal.reserved_count = 0", "execution"),
    ("stage_skip", BODY, "macro_rules! begin_execution_body {", "", "begin_exec_v1", "begin_stage_exec_v1($journal, $roster);", "", "execution"),
    ("commit_skip", BODY, "macro_rules! begin_execution_body {", "", "begin_exec_v1", "begin_commit_exec_v1($journal, $writer, $roster, $reserved_count $($ghost_before)*);", "", "execution"),
    ("canonical_precedence", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if let Err(error) = begin_canonical_exec_v1($roster) { return Err(error); }\n        if let Err(error) = begin_destinations_exec_v1($journal, $roster) { return Err(error); }", "if let Err(error) = begin_destinations_exec_v1($journal, $roster) { return Err(error); }\n        if let Err(error) = begin_canonical_exec_v1($roster) { return Err(error); }", "execution"),
    ("capacity_precedence", BODY, "macro_rules! begin_preflight_body {", "", "begin_preflight_exec_v1", "if count > $journal.member_free.len() { return Err(ReadErrorV1::MemberCapacity); }\n        if count > $journal.scratch.len() { return Err(ReadErrorV1::InvalidState); }", "if count > $journal.scratch.len() { return Err(ReadErrorV1::InvalidState); }\n        if count > $journal.member_free.len() { return Err(ReadErrorV1::MemberCapacity); }", "execution"),
    ("raw_alias_rejection", BODY, "macro_rules! begin_slots_body {", "", "begin_slots_exec_v1", "let member = $journal.member_free[$journal.member_free.len() - 1 - $index];", "let member = $journal.member_free[$journal.member_free.len() - 1 - $index];\n                if $index > 0 && member == $journal.member_free[$journal.member_free.len() - $index] { return Err(ReadErrorV1::InvalidState); }", "execution"),
    ("dirty_tail", BODY, "macro_rules! begin_commit_body {", "", "begin_commit_exec_v1", "$journal.reserved_count = $reserved_count;", "$journal.reserved_count = $reserved_count;\n            if $count < $journal.scratch.len() { $journal.scratch[$count] = None; }", "execution"),
    ("reject_mutation", BODY, "macro_rules! begin_execution_body {", "", "begin_exec_v1", "Ok(count) => count, Err(error) => return Err(error),", "Ok(count) => count, Err(error) => { $journal.registration_watermark = 0; return Err(error); },", "execution"),
    ("writer_frame", DECISIONS, "fn begin_stage_frame_v1(", "", "begin_commit_exec_v1", "&&& after.writers == before.writers", "&&& true", "contract"),
    ("error_identity", DECISIONS, "fn begin_execution_relation_v1(", "", "begin_raw_alias_witness_v1", "result == Err(error) && after == before", "result == Err(error)", "contract"),
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
        need(same(observed, expected[mutation[0]]), 'exact intended failure: ' + mutation[0])
    else:
        need(same(observed['result'], {
            'encountered-error': False, 'encountered-vir-error': False, 'success': True,
            'verified': 45, 'errors': 0, 'is-verifying-entire-crate': True,
        }), 'exact whole-root positive')
        need(not observed['diagnostics'], 'clean positive diagnostics')
        need(observed['verus'] == expected['verus'], 'exact Verus version')


def inputs(repo):
    paths = set(SOURCES) | set(PINS) | {
        PACKET / 'check.py', PACKET / 'cargo-checks.py', PACKET / 'negative-expectations.json',
        PACKET / 'audit.py', PACKET / 'performance.py', Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), CRATE / 'Cargo.toml',
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
    for root in (ROOT,):
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
