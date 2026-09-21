#!/usr/bin/env python3
"""Record production retained-admission regressions and complete model tests."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import signal


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    path = repo / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7':
        raise ValueError('recorder identity')
    spec = importlib.util.spec_from_file_location('retained_cargo_recorder', path)
    recorder = importlib.util.module_from_spec(spec)
    exec(compile(data, str(path), 'exec'), recorder.__dict__)
    for number in recorder.MANAGED:
        signal.signal(number, recorder.interrupted)
    runner = recorder.Recorder(output, repo)
    files = [repo / 'Cargo.toml', repo / 'Cargo.lock', Path(__file__).resolve(), path,
             repo / 'crates/fe2o3-runtime/src/context.rs', repo / 'crates/fe2o3-runtime-model/Cargo.toml']
    for name in ('context_journal_representation_v1.rs', 'context_journal_retained_execution_v1.rs',
                 'context_journal_retained_bodies_v1.rs'):
        files.append(repo / 'crates/fe2o3-runtime-model/verus' / name)
    files.extend(p for p in (repo / 'crates/fe2o3-runtime-model/src').rglob('*') if p.is_file())
    before = {str(p): recorder.sha(p) for p in files}
    recorder.write_json(output / 'inputs-before.json', before)
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo'}
    for name, cmd in [
        ('compiler', ['rustc', '-Vv']),
        ('format', ['cargo', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
    ]:
        runner.run(name, cmd, 300, env=env)
    after = {str(p): recorder.sha(p) for p in files}
    recorder.need(before == after, 'source drift')
    recorder.write_json(output / 'inputs-after.json', after)


if __name__ == '__main__':
    main()
