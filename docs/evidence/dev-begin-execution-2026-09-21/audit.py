#!/usr/bin/env python3
"""Offline audit of this development packet; requires its Git source objects."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True

PACKET = Path('docs/evidence/dev-begin-execution-2026-09-21')
PARENT = Path('docs/evidence/dev-enrollment-execution-verus-2026-09-21/archive.py')
PARENT_SHA = 'ef98aa34753965a80a9d79ca6daab042ce2238fa2bfb887f3aec654402eef1ea'
ORIGINAL = Path('/home/harsh/.codex-tmp/fe2o3-begin-execution-20260921-proof-complete')
ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
PROOF = Path('crates/fe2o3-runtime-model/verus')
ENV = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
       'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo'}
STATIC = {'README.md', 'audit.py', 'check.py', 'selftest.py'}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(name, path, data):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def blob(repo, source, path):
    return subprocess.check_output(['git', 'cat-file', 'blob', f'{source}:{path}'], cwd=repo)


def manifest(packet):
    rows = {}
    for line in (packet / 'SHA256SUMS').read_text().splitlines():
        digest, path = line.split('  ', 1)
        need(re.fullmatch('[0-9a-f]{64}', digest) and path not in rows
             and not Path(path).is_absolute() and '..' not in Path(path).parts, 'manifest row')
        rows[path] = digest
    files = [p for p in packet.rglob('*') if not p.is_dir()]
    need(all(p.is_file() and not p.is_symlink() for p in files)
         and not any(p.is_symlink() for p in packet.rglob('*')), 'regular packet files')
    actual = {str(p.relative_to(packet)): sha(p.read_bytes()) for p in files if p.name != 'SHA256SUMS'}
    need(rows == actual, 'complete manifest')


def proof_checks(packet, parent, helper, model, checker, contents, temporary):
    campaign = packet / 'proof'
    inputs = parent.expected_inputs(contents)
    inputs[str(ROOT / PROOF / checker.SOURCE)] = checker.SOURCE_SHA
    inputs[str(ROOT / PACKET / 'check.py')] = sha((temporary / PACKET / 'check.py').read_bytes())
    need(helper.unique_json(campaign / 'inputs-before.json') == inputs
         and helper.unique_json(campaign / 'inputs-after.json') == inputs, 'proof input identities')
    need(helper.unique_json(campaign / 'environment.json') == {**ENV, 'VERUS_Z3_PATH': str(parent.VERUS.parent / 'z3')}, 'proof environment')
    expected_report = {'development_checks_passed': True, 'positive_obligations': 289,
        'inherited_obligations': 263, 'negative_cases': 8, 'general_custody_preservation_proved': False,
        'rust_source_refinement_proved': False, 'native_or_performance_acceptance': False}
    report = helper.unique_json(campaign / 'report.json')
    need(report == expected_report and all(type(report[k]) is type(v) for k, v in expected_report.items()), 'proof report')
    base = model.inherited().BASE
    cases = [('positive_before', None), *[(m.name, m) for m in checker.mutations(base)], ('positive_after', None)]
    need(helper.entries(campaign) == {'inputs-before.json', 'inputs-after.json', 'environment.json',
        'completed-cases.json', 'report.json', 'closure-before', 'closure-after', *(n for n, _ in cases)}, 'proof roster')
    source = (temporary / PROOF / checker.SOURCE).read_text()
    need(sha(source.encode()) == checker.SOURCE_SHA, 'begin source identity')
    completed = {}
    for name, mutation in cases:
        folder = campaign / name
        original = ORIGINAL / name
        current = base.mutate(source, mutation) if mutation else source
        generated = {checker.SOURCE: current.encode(), **{n: contents[PROOF / n] for n in model.DEPENDENCIES},
            'context_version_journal_enrollment_v1.rs': contents[PROOF / 'context_version_journal_enrollment_v1.rs']}
        audit = temporary / 'audits' / name
        audit.mkdir(parents=True)
        for filename, data in generated.items():
            (audit / filename).write_bytes(data)
        model.audit_source(model.inherited(), generated['context_version_journal_enrollment_v1.rs'].decode(), audit)
        (audit / 'audited-begin-body.rs').write_text('use vstd::prelude::*;\n' + current[len(checker.PREFIX):])
        model.inherited().POLICY.scan(audit / 'audited-begin-body.rs')
        generated = {p.name: p.read_bytes() for p in audit.glob('*.rs')}
        need(helper.entries(folder) == {*generated, 'sources.json', 'solver'}, 'case file roster')
        need(all(helper.read(folder / n) == data for n, data in generated.items()), 'exact mutated and inherited sources')
        need(helper.unique_json(folder / 'sources.json') == {str(original / n): sha(data) for n, data in generated.items()}, 'case identities')
        path = original / checker.SOURCE
        command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(parent.VERUS),
            '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json', '--error-format=json',
            '--num-threads', '4', str(path)]
        record = helper.solver_record(folder / 'solver', command, 1 if mutation else 0)
        checker.check_result(model, record['status'], helper.read(folder / 'solver/stdout.log').decode(),
            helper.read(folder / 'solver/stderr.log').decode(), current, path, mutation)
        completed[name] = {str(original / p.relative_to(folder)): sha(p.read_bytes()) for p in folder.rglob('*') if p.is_file()}
    need(helper.unique_json(campaign / 'completed-cases.json') == completed, 'frozen completed cases')
    for phase in ('before', 'after'):
        folder = campaign / ('closure-' + phase)
        command = ['/bin/sh', str(ROOT / parent.CLOSURE), str(parent.VERUS.parent), str(ROOT / PROOF / 'pins/VERUS_CLOSURE_MANIFEST')]
        helper.solver_record(folder, command, 0)
        need(helper.read(folder / 'stdout.log') == b'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n'
             and not helper.read(folder / 'stderr.log'), 'Verus closure receipts')


def command_receipt(helper, folder, command, cwd, timeout):
    need(helper.entries(folder) == {'stdout', 'stderr', 'receipt.json'}, 'command files')
    record = helper.unique_json(folder / 'receipt.json')
    need(set(record) == {'command', 'cwd', 'started_ns', 'timeout_seconds', 'pid', 'exit', 'error',
        'group_absent', 'environment', 'stdin_sha256', 'finished_ns', 'stdout_sha256', 'stderr_sha256'}, 'receipt fields')
    need(record['command'] == command and record['cwd'] == str(cwd) and record['environment'] == ENV
         and type(record['timeout_seconds']) is int and record['timeout_seconds'] == timeout
         and record['stdin_sha256'] is None, 'exact command invocation')
    need(type(record['exit']) is int and record['exit'] == 0 and record['error'] is None
         and record['group_absent'] is True and type(record['pid']) is int and record['pid'] > 0, 'owned command completion')
    helper.check_times(record)
    need(sha(helper.read(folder / 'stdout')) == record['stdout_sha256']
         and sha(helper.read(folder / 'stderr')) == record['stderr_sha256'], 'command streams')
    return record


def unit_summary(data, docs=False):
    summaries = [line for line in data.decode().splitlines() if line.startswith('test result:')]
    expected = [r'test result: ok\. 821 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s']
    if docs:
        expected.append(r'test result: ok\. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s')
    need(len(summaries) == len(expected) and all(re.fullmatch(pattern, row) for pattern, row in zip(expected, summaries)), 'exact terminal test summaries')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    repo, packet = args.repo.resolve(), args.packet.resolve()
    manifest(packet)
    source = (packet / 'SOURCE').read_text().strip()
    need(re.fullmatch('[0-9a-f]{40}', source), 'source commit')
    need({p.name for p in packet.iterdir()} == STATIC | {'SOURCE', 'SHA256SUMS', 'proof', 'cargo'}, 'packet root roster')
    need(all((packet / name).read_bytes() == blob(repo, source, PACKET / name) for name in STATIC), 'static packet source binding')
    with tempfile.TemporaryDirectory(prefix='fe2o3-begin-audit-') as scratch:
        temporary = Path(scratch)
        data = blob(repo, source, PARENT)
        need(sha(data) == PARENT_SHA, 'pinned archive helpers')
        parent = load('begin_archive_parent', temporary / 'parent.py', data)
        contents, model = parent.signed_sources(repo, temporary)
        helper = model.archive_helpers
        for path in [PROOF / 'context_version_journal_begin_v1.rs', PACKET / 'check.py']:
            target = temporary / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(blob(repo, source, path))
        checker = load('begin_checked_source', temporary / PACKET / 'check.py', (temporary / PACKET / 'check.py').read_bytes())
        proof_checks(packet, parent, helper, model, checker, contents, temporary)
        commands = {'compiler': ['rustc', '-Vv'],
            'format': ['cargo', 'fmt', '--check', '-p', 'fe2o3-runtime-model'],
            'test': ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model'],
            'clippy': ['cargo', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']}
        need(helper.entries(packet / 'cargo') == {*commands, 'inputs-before.json', 'inputs-after.json'}, 'Cargo command roster')
        for name, command in commands.items():
            command_receipt(helper, packet / 'cargo' / name, command, ROOT, 300)
        unit_summary(helper.read(packet / 'cargo/test/stdout'), docs=True)
        need(helper.unique_json(packet / 'cargo/inputs-before.json') == helper.unique_json(packet / 'cargo/inputs-after.json'), 'Cargo source brackets')
        cargo_inputs = helper.unique_json(packet / 'cargo/inputs-before.json')
        source_names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', source,
            'crates/fe2o3-runtime-model/src'], cwd=repo, text=True).splitlines()
        paths = [*source_names, 'Cargo.toml', 'Cargo.lock', 'crates/fe2o3-runtime-model/Cargo.toml',
            'crates/fe2o3-runtime/src/context.rs',
            'docs/evidence/dev-enrollment-transaction-2026-09-21/cargo-checks.py', 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py']
        need(set(cargo_inputs) == {str(ROOT / p) for p in paths}, 'exact Cargo input roster')
        need(all(Path(p).is_relative_to(ROOT) and h == sha(blob(repo, source, Path(p).relative_to(ROOT))) for p, h in cargo_inputs.items()), 'Cargo input Git identities')
    print('PASS: development Begin proof, Cargo receipts and packet manifest')


if __name__ == '__main__':
    main()
