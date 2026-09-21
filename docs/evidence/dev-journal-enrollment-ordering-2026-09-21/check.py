#!/usr/bin/env python3
"""Standalone actual-type ordering campaign; no inherited logical proof staging."""
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
PACKET = Path('docs/evidence/dev-journal-enrollment-ordering-2026-09-21')
ROOT = CRATE / 'verus/context_journal_enrollment_ordering_v1.rs'
PROOF = CRATE / 'verus/context_journal_enrollment_ordering_bodies_v1.rs'
BODY = CRATE / 'src/context_version_journal/enrollment_ordering_bodies.rs'
SOURCES = [ROOT, PROOF, BODY, CRATE / 'src/context_version_journal/declarations.rs',
           CRATE / 'src/context_version_journal/enrollment_declarations.rs']
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
# name, directly expanded macro, selected function, unique executable replacement
MUTATIONS = [
    ('swap_payload', 'enrollment_swap_body', 'enrollment_swap',
     '$values[$right] = a;', '$values[$right] = match a { Some(mut value) => { value.key.local = 0; Some(value) }, None => None };'),
    ('sorted_direction', 'enrollment_sorted_body', 'enrollment_sorted',
     'unwrap().slot > $values[$index].unwrap().slot', 'unwrap().slot < $values[$index].unwrap().slot'),
    ('sift_child', 'enrollment_sift_body', 'enrollment_sift',
     'unwrap().slot < $values[$left + 1].unwrap().slot', 'unwrap().slot > $values[$left + 1].unwrap().slot'),
    ('heap_build', 'enrollment_heapsort_body', 'enrollment_heapsort',
     'let mut $start = $len / 2;', 'let mut $start = 0usize;'),
    ('heap_extract', 'enrollment_heapsort_body', 'enrollment_heapsort',
     '$end -= 1;', '$end = 1;'),
    ('sort_skip', 'enrollment_sort_body', 'sort_slots',
     'if !enrollment_sorted($values)', 'if false'),
    ('key_order', 'enrollment_less_body', 'enrollment_less',
     '$left.context_generation < $right.context_generation', '$left.context_generation > $right.context_generation'),
    ('key_result', 'enrollment_contains_key_body', 'contains_key',
     '&& entry.key.local == $key.local;', '|| entry.key.local == $key.local;'),
    ('slot_result', 'enrollment_contains_slot_body', 'contains_slot',
     'let $found = $values[$lo].unwrap().slot == $slot;', 'let $found = $values[$lo].unwrap().slot != $slot;'),
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
    name, macro, function, before, after = mutation
    source = data.decode('ascii')
    anchor = 'macro_rules! ' + macro + ' {'
    need(source.count(anchor) == 1, 'unique mutation macro')
    start = source.index(anchor)
    end = source.index('\n}\n', start) + 2
    fragment = source[start:end]
    need(fragment.count(before) == 1 and before != after, 'unique executable mutation')
    return (source[:start] + fragment.replace(before, after, 1) + source[end:]).encode('ascii')


def command(verus, source, mutation):
    result = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
              '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
              '--error-format=json', '--num-threads', '4']
    if mutation:
        result += ['--verify-root', '--verify-function', mutation[2]]
    return [*result, str(source)]


def normalized(base, stdout, stderr, folder):
    report = base.unique_json(stdout)
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    # Paths are the only normalized fields. All bytes, spans, expansion trees,
    # diagnostic text and JSON value types remain part of the expected result.
    diagnostics = base.unique_json(json.dumps(diagnostics).replace(str(folder), '<CASE>'))
    return {'result': report['verification-results'], 'diagnostics': diagnostics, 'verus': report['verus']}


def check_result(base, status, stdout, stderr, folder, mutation, expected):
    need(type(status) is int and status == (1 if mutation else 0), 'normal expected exit')
    observed = normalized(base, stdout, stderr, folder)
    if mutation:
        need(same(observed, expected[mutation[0]]), 'exact intended failure: ' + mutation[0])
    else:
        need(same(observed['result'], {
            'encountered-error': False, 'encountered-vir-error': False, 'success': True,
            'verified': 32, 'errors': 0, 'is-verifying-entire-crate': True,
        }), 'exact whole-root positive')
        need(not observed['diagnostics'], 'clean positive diagnostics')
        need(observed['verus'] == expected['verus'], 'exact Verus version')


def inputs(repo):
    paths = set(SOURCES) | set(PINS) | {
        PACKET / 'check.py', PACKET / 'cargo-checks.py', PACKET / 'negative-expectations.json',
        PACKET / 'audit.py', Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), CRATE / 'Cargo.toml',
        Path('crates/fe2o3-runtime/src/context.rs'),
        Path('docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'),
    }
    paths.update(p.relative_to(repo) for p in (repo / CRATE / 'src').rglob('*') if p.is_file())
    for name in ('context_journal_representation_v1.rs', 'context_journal_retained_execution_v1.rs',
                 'context_journal_retained_bodies_v1.rs', 'context_journal_unknown_execution_v1.rs',
                 'context_journal_unknown_body_v1.rs', 'context_journal_settlement_execution_v1.rs',
                 'context_journal_settlement_bodies_v1.rs'):
        paths.add(CRATE / 'verus' / name)
    return sorted(paths)


def snapshot(repo, probe):
    paths = set(SOURCES) | set(PINS) if probe else inputs(repo)
    return {str(path): sha((repo / path).read_bytes()) for path in sorted(paths)}


def policy_check(repo, policy, folder):
    policy.scan(folder / PROOF)
    source = (folder / BODY).read_text(encoding='ascii')
    need(source.count('macro_rules!') == 8, 'exact shared body macro roster')
    # The fixed include/declaration envelopes are replayed byte-for-byte; scan
    # all shared executable tokens after removing only macro declaration words.
    stripped = folder / 'audited-shared-bodies.rs'
    stripped.write_text(source.replace('macro_rules!', ''), encoding='ascii')
    policy.scan(stripped)
    need((folder / ROOT).read_bytes() == (repo / ROOT).read_bytes(), 'exact include envelope')


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
        for name, mutation in [('positive-before', None), *[(m[0], m) for m in MUTATIONS], ('positive-after', None)]:
            folder = output / name
            for path in SOURCES:
                target = folder / path
                target.parent.mkdir(parents=True, exist_ok=True)
                data = (repo / path).read_bytes()
                target.write_bytes(mutated(data, mutation) if mutation and path == BODY else data)
            policy_check(repo, policy, folder)
            frozen = {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}
            status, stdout, stderr = base.run_owned(command(verus, folder / ROOT, mutation), 190, folder / 'solver', env)
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
    parser.add_argument('--probe', action='store_true', help='development only; never accepted by the auditor')
    campaign(parser.parse_args())


if __name__ == '__main__':
    main()
