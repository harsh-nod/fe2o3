#!/usr/bin/env python3
"""Fresh-build CPU experiment, not native or universal performance acceptance."""
import argparse
import csv
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import statistics
import subprocess
import tarfile

BASELINE = 'ee6489242a5f9976847193fada2d5fd966b25353'
HEADER = ['capacity', 'occupied', 'batch', 'order', 'sample', 'operations', 'elapsed_ns', 'floor_ns']


def need(condition, message):
    if not condition:
        raise ValueError(message)


def expected_rows():
    rows = {}
    for capacity in (64, 1024, 4096):
        for occupied in (0, capacity // 2):
            for count in sorted({1, 16, 63, capacity - occupied}):
                if count > capacity - occupied:
                    continue
                for order in range(3):
                    for sample in range(5):
                        rows[capacity, occupied, count, order, sample] = max(16, min(256, 65536 // capacity))
    return rows


def parse(text):
    reader = csv.DictReader(io.StringIO(text))
    need(reader.fieldnames == HEADER, 'exact CSV header')
    expected = expected_rows()
    rows = {}
    for item in reader:
        need(set(item) == set(HEADER) and all(isinstance(x, str) and x.isascii() and x.isdecimal() for x in item.values()), 'decimal CSV fields')
        row = {k: int(v) for k, v in item.items()}
        key = tuple(row[k] for k in HEADER[:5])
        need(key in expected and key not in rows, 'unique expected case/sample')
        need(row['operations'] == expected[key], 'exact operation count')
        need(0 < row['elapsed_ns'] < 2**64 and 0 <= row['floor_ns'] < 2**64, 'duration bounds')
        rows[key] = row
    need(rows.keys() == expected.keys(), 'complete case/sample matrix')
    return rows


def self_test():
    rows = [list(k) + [v, 100000, 10] for k, v in expected_rows().items()]
    def encode(values, header=HEADER):
        stream = io.StringIO()
        writer = csv.writer(stream)
        writer.writerow(header)
        writer.writerows(values)
        return stream.getvalue()
    good = encode(rows)
    need(len(parse(good)) == 345, 'valid matrix')
    invalid = [encode(rows[:-1]), encode(rows + [rows[0]]), encode([rows[0]] + rows[:-1]),
               encode(rows, HEADER[::-1]), good + 'garbage\n']
    for index, value in [(0, 0), (4, 9), (5, 0), (6, 0), (6, -1), (7, 2**64), (6, 'nan'), (6, True)]:
        changed = [r[:] for r in rows]
        changed[0][index] = value
        invalid.append(encode(changed))
    for text in invalid:
        try:
            parse(text)
        except (ValueError, TypeError):
            pass
        else:
            raise ValueError('accepted adverse CSV')
    return len(invalid)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    rejected = self_test()
    if args.self_test:
        print(f'PASS: {rejected} adverse CSV cases rejected')
        return
    repo, output = args.repo.resolve(), args.output.resolve()
    here = Path(__file__).resolve().parent
    recorder_path = repo / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
    recorder_data = recorder_path.read_bytes()
    need(hashlib.sha256(recorder_data).hexdigest() == 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7', 'recorder identity')
    spec = importlib.util.spec_from_file_location('transaction_cpu_recorder', recorder_path)
    module = importlib.util.module_from_spec(spec)
    exec(compile(recorder_data, str(recorder_path), 'exec'), module.__dict__)
    for signum in module.MANAGED:
        signal.signal(signum, module.interrupted)
    output.mkdir()
    recorder = module.Recorder(output / 'commands', output)
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo'}
    inputs = [Path(__file__).resolve(), here / 'bench.rs', here / 'candidate-allocation-lifecycle.rs', recorder_path,
              repo / 'crates/fe2o3-runtime-model/src/context_version_journal/allocation_lifecycle_tests.rs']
    identity = {str(p): module.sha(p) for p in inputs}
    module.write_json(output / 'inputs-before.json', identity)
    archive = subprocess.check_output(['git', 'archive', BASELINE, 'crates/fe2o3-runtime-model/src',
        'crates/fe2o3-runtime/src/context.rs'], cwd=repo)
    for variant in ('baseline', 'candidate'):
        folder = output / variant
        folder.mkdir()
        with tarfile.open(fileobj=io.BytesIO(archive)) as source:
            for item in source:
                if item.isdir():
                    continue
                need(item.isfile() and (item.name.startswith('crates/fe2o3-runtime-model/src/')
                     or item.name == 'crates/fe2o3-runtime/src/context.rs')
                     and '..' not in Path(item.name).parts, 'regular bounded source archive')
                destination = folder / item.name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.extractfile(item).read())
        src = folder / 'crates/fe2o3-runtime-model/src/context_version_journal'
        (src / 'allocation_lifecycle_tests.rs').write_bytes(inputs[-1].read_bytes())
        if variant == 'candidate':
            (src / 'allocation_lifecycle.rs').write_bytes((here / 'candidate-allocation-lifecycle.rs').read_bytes())
        (folder / 'bench.rs').write_bytes((here / 'bench.rs').read_bytes())
        (folder / 'Cargo.toml').write_text(f'''[package]
name = "enrollment-{variant}"
version = "0.1.0"
edition = "2024"
[workspace]
[lib]
name = "fe2o3_runtime_model"
path = "crates/fe2o3-runtime-model/src/lib.rs"
[[bin]]
name = "enrollment-{variant}"
path = "bench.rs"
[dependencies]
spin = {{ version = "=0.12.3", default-features = false, features = ["once"] }}
[profile.release]
opt-level = 3
debug = false
codegen-units = 1
lto = false
''', encoding='ascii')
        recorder.run(f'lock-{variant}', ['cargo', 'generate-lockfile', '--offline', '--manifest-path', str(folder / 'Cargo.toml')], 30, env=env)
    sources = {str(p.relative_to(output)): module.sha(p) for v in ('baseline', 'candidate') for p in (output / v).rglob('*') if p.is_file()}
    module.write_json(output / 'sources-before.json', sources)
    allowed = sorted(os.sched_getaffinity(0))
    module.write_json(output / 'environment.json', {'cpu': allowed[0], 'allowed_cpus': allowed,
        'load_average': os.getloadavg(), 'uname': list(os.uname()), 'exclusive_cpu_reservation': False,
        'baseline_git_commit': BASELINE, 'csv_adverse_cases': rejected})
    for name, cmd in [('compiler', ['rustc', '-Vv']), ('cpu', ['lscpu'])]:
        recorder.run(name, cmd, 30, env=env)
    for variant in ('baseline', 'candidate'):
        manifest = str(output / variant / 'Cargo.toml')
        recorder.run(f'test-{variant}', ['cargo', 'test', '--offline', '--locked', '--manifest-path', manifest, '--lib'], 180, env=env)
        recorder.run(f'build-{variant}', ['cargo', 'build', '--offline', '--locked', '--release', '--manifest-path', manifest], 180, env=env)
    binaries = {v: output / v / 'target/release' / f'enrollment-{v}' for v in ('baseline', 'candidate')}
    elf_identity = {v: module.sha(p) for v, p in binaries.items()}
    module.write_json(output / 'executables-before.json', elf_identity)
    observations = {}
    for run, variant in enumerate(('baseline', 'candidate', 'candidate', 'baseline')):
        need({v: module.sha(p) for v, p in binaries.items()} == elf_identity, 'executable drift before run')
        receipt = recorder.run(f'run-{run}-{variant}', ['taskset', '-c', str(allowed[0]), str(binaries[variant])], 300, env=env)
        rows = parse((receipt / 'stdout').read_text())
        observations[run] = rows
        need({v: module.sha(p) for v, p in binaries.items()} == elf_identity, 'executable drift after run')
        need({p: module.sha(Path(p)) for p in identity} == identity, 'input drift')
        need({p: module.sha(output / p) for p in sources} == sources, 'source drift')
    summary = []
    for case in sorted({k[:4] for k in expected_rows()}):
        values = {r: [row['elapsed_ns'] / row['operations'] for key, row in rows.items() if key[:4] == case]
                  for r, rows in observations.items()}
        medians = {r: statistics.median(v) for r, v in values.items()}
        baseline = statistics.median(values[0] + values[3])
        candidate = statistics.median(values[1] + values[2])
        summary.append({'case': case, 'run_medians_ns': medians, 'baseline_median_ns': baseline,
            'candidate_median_ns': candidate, 'baseline_over_candidate': baseline / candidate,
            'paired_ratios': [medians[0] / medians[1], medians[3] / medians[2]],
            'run_min_max_ns': {r: [min(v), max(v)] for r, v in values.items()}})
    module.write_json(output / 'summary.json', summary)
    module.write_json(output / 'executables-after.json', {v: module.sha(p) for v, p in binaries.items()})
    module.write_json(output / 'sources-after.json', {p: module.sha(output / p) for p in sources})
    module.write_json(output / 'inputs-after.json', {p: module.sha(Path(p)) for p in identity})
    print(json.dumps({'cases': len(summary), 'timed_blocks': sum(map(len, observations.values())),
        'ratio_range': [min(x['baseline_over_candidate'] for x in summary), max(x['baseline_over_candidate'] for x in summary)],
        'accepted_native_or_performance_milestone': False}), flush=True)


if __name__ == '__main__':
    main()
