#!/usr/bin/env python3
"""Record CPU regression checks for actual producer-read release."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    sys.modules[name] = result
    spec.loader.exec_module(result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--target', type=Path, required=True)
    args = parser.parse_args()
    repo, output = args.repo.resolve(), args.output.resolve()
    check = module(Path(__file__).with_name('check.py'), 'ordering_check')
    path = repo / 'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py'
    if hashlib.sha256(path.read_bytes()).hexdigest() != 'df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7':
        raise ValueError('recorder identity')
    recorder = module(path, 'ordering_cpu_recorder')
    for number in recorder.MANAGED:
        signal.signal(number, recorder.interrupted)
    runner = recorder.Recorder(output, repo)
    before = check.snapshot(repo, False)
    recorder.write_json(output / 'inputs-before.json', before)
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo',
           'CARGO_TARGET_DIR': str(args.target.resolve())}
    for name, command in commands():
        runner.run(name, command, 300, env=env)
    after = check.snapshot(repo, False)
    recorder.need(before == after, 'source drift')
    recorder.write_json(output / 'inputs-after.json', after)


def commands():
    return [
        ('host', ['uname', '-a']),
        ('cpu', ['lscpu', '--json']),
        ('compiler', ['rustc', '-Vv']),
        ('format', ['cargo', 'fmt', '--check', '-p', 'fe2o3-runtime-model']),
        ('test', ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model']),
        ('clippy', ['cargo', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']),
        ('release-build', ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--no-run']),
        ('performance', ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model', '--release', '--lib',
                         '::tests::release_shared::performance::shared_producer_release_performance',
                         '--', '--ignored', '--nocapture', '--test-threads=1']),
    ]


if __name__ == '__main__':
    main()
