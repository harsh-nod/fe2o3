#!/usr/bin/env python3
"""Offline audit of this development packet; requires its Git source objects."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import re
import statistics
import subprocess
import sys
import tempfile
import tomllib

PACKET = Path('docs/evidence/dev-enrollment-transaction-2026-09-21')
PARENT = Path('docs/evidence/dev-enrollment-execution-verus-2026-09-21/archive.py')
PARENT_SHA = 'ef98aa34753965a80a9d79ca6daab042ce2238fa2bfb887f3aec654402eef1ea'
ORIGINAL = Path('/home/harsh/.codex-tmp/fe2o3-enrollment-nosort-20260921')
CPU_ORIGINAL = ORIGINAL / 'cpu-complete-b'
ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
PROOF = Path('crates/fe2o3-runtime-model/verus')
ENV = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
       'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo'}
STATIC = {'README.md', 'audit.py', 'bench.rs', 'candidate-allocation-lifecycle.rs',
          'cargo-checks.py', 'check.py', 'compare.py', 'selftest.py'}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def typed_equal(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(typed_equal(actual[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(typed_equal(a, b) for a, b in zip(actual, expected))
    return actual == expected


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
    expected_report = {'development_checks_passed': True, 'positive_obligations': 275,
        'inherited_obligations': 263, 'negative_cases': 6, 'historical_error_contract_proved': False,
        'production_promoted': False, 'native_or_performance_acceptance': False}
    report = helper.unique_json(campaign / 'report.json')
    need(report == expected_report and all(type(report[k]) is type(v) for k, v in expected_report.items()), 'proof report')
    base = model.inherited().BASE
    cases = [('positive_before', None), *[(m.name, m) for m in checker.mutations(base)], ('positive_after', None)]
    need(helper.entries(campaign) == {'inputs-before.json', 'inputs-after.json', 'environment.json',
        'completed-cases.json', 'report.json', 'closure-before', 'closure-after', *(n for n, _ in cases)}, 'proof roster')
    source = (temporary / PROOF / checker.SOURCE).read_text()
    need(sha(source.encode()) == checker.SOURCE_SHA, 'transaction source identity')
    completed = {}
    for name, mutation in cases:
        folder = campaign / name
        original = ORIGINAL / 'proof-checks-complete' / name
        current = base.mutate(source, mutation) if mutation else source
        generated = {checker.SOURCE: current.encode(), **{n: contents[PROOF / n] for n in model.DEPENDENCIES},
            'context_version_journal_enrollment_v1.rs': contents[PROOF / 'context_version_journal_enrollment_v1.rs']}
        audit = temporary / 'audits' / name
        audit.mkdir(parents=True)
        for filename, data in generated.items():
            (audit / filename).write_bytes(data)
        model.audit_source(model.inherited(), generated['context_version_journal_enrollment_v1.rs'].decode(), audit)
        (audit / 'audited-transaction-body.rs').write_text('use vstd::prelude::*;\n' + current[len(checker.PREFIX):])
        model.inherited().POLICY.scan(audit / 'audited-transaction-body.rs')
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
    expected = [r'test result: ok\. 817 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s']
    if docs:
        expected.append(r'test result: ok\. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s')
    need(len(summaries) == len(expected) and all(re.fullmatch(pattern, row) for pattern, row in zip(expected, summaries)), 'exact terminal test summaries')


def cpu_checks(repo, source, packet, helper, compare):
    cpu = packet / 'cpu'
    need(helper.entries(cpu) == {'commands', 'baseline', 'candidate', 'inputs-before.json', 'inputs-after.json',
        'sources-before.json', 'sources-after.json', 'executables-before.json', 'executables-after.json',
        'environment.json', 'summary.json'}, 'CPU packet roster')
    before = helper.unique_json(cpu / 'sources-before.json')
    need(before == helper.unique_json(cpu / 'sources-after.json'), 'CPU source brackets')
    names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', compare.BASELINE,
        'crates/fe2o3-runtime-model/src', 'crates/fe2o3-runtime/src/context.rs'], cwd=repo, text=True).splitlines()
    originals = {n: blob(repo, compare.BASELINE, n) for n in names}
    baseline_lock = tomllib.loads(blob(repo, compare.BASELINE, 'Cargo.lock').decode())
    spin = [p for p in baseline_lock['package'] if p['name'] == 'spin' and p['version'] == '0.12.3']
    need(len(spin) == 1, 'baseline spin dependency')
    expected = {}
    for variant in ('baseline', 'candidate'):
        need(helper.entries(cpu / variant) == {'Cargo.toml', 'Cargo.lock'}, 'CPU manifest roster')
        for path, data in originals.items():
            if path.endswith('/allocation_lifecycle_tests.rs'):
                data = blob(repo, source, path)
            elif variant == 'candidate' and path.endswith('/allocation_lifecycle.rs'):
                data = blob(repo, source, PACKET / 'candidate-allocation-lifecycle.rs')
            expected[f'{variant}/{path}'] = sha(data)
        expected[f'{variant}/bench.rs'] = sha(blob(repo, source, PACKET / 'bench.rs'))
        for name in ('Cargo.toml', 'Cargo.lock'):
            expected[f'{variant}/{name}'] = sha(helper.read(cpu / variant / name))
        config = tomllib.loads(helper.read(cpu / variant / 'Cargo.toml').decode())
        need(typed_equal(config, {'package': {'name': f'enrollment-{variant}', 'version': '0.1.0', 'edition': '2024'},
            'workspace': {}, 'lib': {'name': 'fe2o3_runtime_model', 'path': 'crates/fe2o3-runtime-model/src/lib.rs'},
            'bin': [{'name': f'enrollment-{variant}', 'path': 'bench.rs'}],
            'dependencies': {'spin': {'version': '=0.12.3', 'default-features': False, 'features': ['once']}},
            'profile': {'release': {'opt-level': 3, 'debug': False, 'codegen-units': 1, 'lto': False}}}), 'exact build manifest')
        lock = tomllib.loads(helper.read(cpu / variant / 'Cargo.lock').decode())
        need(typed_equal(lock, {'version': 4, 'package': [{'name': f'enrollment-{variant}', 'version': '0.1.0', 'dependencies': ['spin']}, *spin]}), 'exact lock semantics')
    need(before == expected, 'source-to-comparison binding')
    for name in ('inputs', 'executables'):
        need(helper.unique_json(cpu / f'{name}-before.json') == helper.unique_json(cpu / f'{name}-after.json'), f'CPU {name} brackets')
    inputs = helper.unique_json(cpu / 'inputs-before.json')
    expected_inputs = {str(ROOT / path) for path in [PACKET / 'compare.py', PACKET / 'bench.rs',
        PACKET / 'candidate-allocation-lifecycle.rs',
        Path('docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'),
        Path('crates/fe2o3-runtime-model/src/context_version_journal/allocation_lifecycle_tests.rs')]}
    need(set(inputs) == expected_inputs, 'exact CPU input roster')
    need(all(Path(p).is_relative_to(ROOT) and h == sha(blob(repo, source, Path(p).relative_to(ROOT))) for p, h in inputs.items()), 'CPU input Git identities')
    elf = helper.unique_json(cpu / 'executables-before.json')
    need(set(elf) == {'baseline', 'candidate'} and all(re.fullmatch('[0-9a-f]{64}', x) for x in elf.values()), 'ELF identities')
    environment = helper.unique_json(cpu / 'environment.json')
    need(environment['baseline_git_commit'] == compare.BASELINE and environment['exclusive_cpu_reservation'] is False
         and environment['csv_adverse_cases'] == compare.self_test(), 'CPU scope')
    expected_commands = {'lock-baseline', 'lock-candidate', 'compiler', 'cpu', 'test-baseline', 'test-candidate',
        'build-baseline', 'build-candidate', 'run-0-baseline', 'run-1-candidate', 'run-2-candidate', 'run-3-baseline'}
    need(helper.entries(cpu / 'commands') == expected_commands, 'CPU command roster')
    original_cpu = CPU_ORIGINAL
    commands = {'compiler': (['rustc', '-Vv'], 30), 'cpu': (['lscpu'], 30)}
    for variant in ('baseline', 'candidate'):
        manifest_path = str(original_cpu / variant / 'Cargo.toml')
        commands[f'lock-{variant}'] = (['cargo', 'generate-lockfile', '--offline', '--manifest-path', manifest_path], 30)
        commands[f'test-{variant}'] = (['cargo', 'test', '--offline', '--locked', '--manifest-path', manifest_path, '--lib'], 180)
        commands[f'build-{variant}'] = (['cargo', 'build', '--offline', '--locked', '--release', '--manifest-path', manifest_path], 180)
    for name, (command, timeout) in commands.items():
        command_receipt(helper, cpu / 'commands' / name, command, original_cpu, timeout)
    observations = {}
    for run, variant in enumerate(('baseline', 'candidate', 'candidate', 'baseline')):
        folder = cpu / 'commands' / f'run-{run}-{variant}'
        command = ['taskset', '-c', str(environment['cpu']), str(CPU_ORIGINAL / variant / 'target/release' / f'enrollment-{variant}')]
        command_receipt(helper, folder, command, original_cpu, 300)
        need(not helper.read(folder / 'stderr'), 'no benchmark diagnostics')
        observations[run] = compare.parse(helper.read(folder / 'stdout').decode())
    derived = []
    for case in sorted({k[:4] for k in compare.expected_rows()}):
        values = {r: [row['elapsed_ns'] / row['operations'] for key, row in rows.items() if key[:4] == case] for r, rows in observations.items()}
        medians = {str(r): statistics.median(v) for r, v in values.items()}
        baseline, candidate = statistics.median(values[0] + values[3]), statistics.median(values[1] + values[2])
        derived.append({'case': list(case), 'run_medians_ns': medians, 'baseline_median_ns': baseline,
            'candidate_median_ns': candidate, 'baseline_over_candidate': baseline / candidate,
            'paired_ratios': [medians['0'] / medians['1'], medians['3'] / medians['2']],
            'run_min_max_ns': {str(r): [min(v), max(v)] for r, v in values.items()}})
    need(typed_equal(helper.unique_json(cpu / 'summary.json'), derived), 'derived CPU summary')
    for variant in ('baseline', 'candidate'):
        unit_summary(helper.read(cpu / 'commands' / f'test-{variant}' / 'stdout'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    repo, packet = args.repo.resolve(), args.packet.resolve()
    manifest(packet)
    source = (packet / 'SOURCE').read_text().strip()
    need(re.fullmatch('[0-9a-f]{40}', source), 'source commit')
    need({p.name for p in packet.iterdir()} == STATIC | {'SOURCE', 'SHA256SUMS', 'proof', 'cpu', 'cargo'}, 'packet root roster')
    need(all((packet / name).read_bytes() == blob(repo, source, PACKET / name) for name in STATIC), 'static packet source binding')
    with tempfile.TemporaryDirectory(prefix='fe2o3-enrollment-audit-') as scratch:
        temporary = Path(scratch)
        data = blob(repo, source, PARENT)
        need(sha(data) == PARENT_SHA, 'pinned archive helpers')
        parent = load('transaction_archive_parent', temporary / 'parent.py', data)
        contents, model = parent.signed_sources(repo, temporary)
        helper = model.archive_helpers
        for path in [PROOF / 'context_version_journal_enrollment_transaction_v1.rs', PACKET / 'check.py', PACKET / 'compare.py']:
            target = temporary / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(blob(repo, source, path))
        checker = load('transaction_checked_source', temporary / PACKET / 'check.py', (temporary / PACKET / 'check.py').read_bytes())
        compare = load('transaction_cpu_source', temporary / PACKET / 'compare.py', (temporary / PACKET / 'compare.py').read_bytes())
        proof_checks(packet, parent, helper, model, checker, contents, temporary)
        cpu_checks(repo, source, packet, helper, compare)
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
            str(PACKET / 'cargo-checks.py'), 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py']
        need(set(cargo_inputs) == {str(ROOT / p) for p in paths}, 'exact Cargo input roster')
        need(all(Path(p).is_relative_to(ROOT) and h == sha(blob(repo, source, Path(p).relative_to(ROOT))) for p, h in cargo_inputs.items()), 'Cargo input Git identities')
    print('PASS: development proof, CPU comparison, Cargo receipts and packet manifest')


if __name__ == '__main__':
    main()
