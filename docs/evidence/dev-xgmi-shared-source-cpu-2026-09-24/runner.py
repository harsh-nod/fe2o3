#!/usr/bin/env python3
"""Bounded CPU qualification of directed XGMI shared-source ownership."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

REPO = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
ROOT = Path(__file__).parent
TARGET = Path('/home/harsh/.codex-tmp/fe2o3-directed-owner-20260924-mamEIcU0/target')
CHECK = REPO / 'crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    os.chdir(REPO)
    output = ROOT / sys.argv[1]
    output.mkdir()
    spec = importlib.util.spec_from_file_location('progress_probe_checker', CHECK)
    check = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = check
    spec.loader.exec_module(check)
    base = check.dependencies(REPO)['base']
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)
    def inputs():
        paths = subprocess.check_output(
            ['/usr/bin/git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z',
             'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo',
             'benchmarks/runtime_gfx942'],
            env=check.git_environment()).decode().split('\0')
        paths = sorted(set(path for path in paths if path and
                           (path.endswith(('.rs', '.toml', '.lock', '.json', '.py')) or '/fixtures/' in path)))
        return {'runner': digest(Path(__file__)),
                'source': {path: digest(REPO / path) for path in paths}}

    before = inputs()
    (output / 'inputs-before.json').write_text(json.dumps(before, indent=2) + '\n')
    env = {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'LC_ALL': 'C', 'CARGO_TARGET_DIR': str(TARGET), 'CARGO_INCREMENTAL': '0',
           'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never',
           'CARGO_PROFILE_TEST_OPT_LEVEL': '1', 'CARGO_PROFILE_TEST_DEBUG': '0',
           'CARGO_PROFILE_DEV_DEBUG': '0', 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-03'}
    cargo = ['cargo', '--locked']
    example = 'gfx942-runtime-xgmi-directed-owner-smoke'
    commands = [
        ('rustc', ['rustc', '-vV'], 30),
        ('cargo', ['cargo', '-V'], 30),
        ('format', ['cargo', 'fmt', '-p', 'fe2o3-runtime', '--', '--check'], 120),
        ('gnu', cargo + ['test', '-p', 'fe2o3-runtime', '--all-features', '--lib'], 1200),
        ('musl', cargo + ['test', '-p', 'fe2o3-runtime', '--all-features', '--lib',
                          '--target', 'x86_64-unknown-linux-musl'], 1200),
        ('docs', cargo + ['test', '-p', 'fe2o3-runtime', '--all-features', '--doc'], 1200),
        ('gnu-example', cargo + ['test', '-p', 'fe2o3-runtime', '--example', example], 1200),
        ('musl-example', cargo + ['test', '-p', 'fe2o3-runtime', '--example', example,
                                  '--target', 'x86_64-unknown-linux-musl'], 1200),
        ('build-cli', cargo + ['build', '-p', 'fe2o3-runtime', '--example', example,
                              '--target', 'x86_64-unknown-linux-musl'], 1200),
        ('clippy', cargo + ['clippy', '-p', 'fe2o3-runtime', '--all-features', '--all-targets',
                           '--', '-D', 'warnings'], 1200),
        ('results-tests', ['/usr/bin/python3', '-I', '-B',
                          str(REPO / 'benchmarks/runtime_gfx942/test_xgmi_directed_owner_results.py')], 120),
        ('campaign-tests', ['/usr/bin/python3', '-I', '-B',
                           str(REPO / 'benchmarks/runtime_gfx942/test_xgmi_directed_owner_campaign.py')], 120),
        ('native-runner-tests', ['/usr/bin/python3', '-I', '-B',
                           str(REPO / 'benchmarks/runtime_gfx942/test_xgmi_directed_owner_native.py')], 120),
        ('after-rustc', ['rustc', '-vV'], 30),
        ('after-cargo', ['cargo', '-V'], 30),
    ]
    env['FE2O3_DIRECTED_OWNER_RUST_BINARY'] = str(TARGET / 'x86_64-unknown-linux-musl/debug/examples' / example)
    if len(sys.argv) > 2 and sys.argv[2] == 'focused':
        commands = [('focused', cargo + ['test', '-p', 'fe2o3-runtime', '--lib', 'shared_sources'], 1200)]
    if len(sys.argv) > 2 and sys.argv[2] == 'lint':
        commands = [row for row in commands if row[0] == 'clippy']
    try:
        for name, command, bound in commands:
            print('RUN: ' + name, flush=True)
            status, stdout, stderr = base.run_owned(command, bound, output / name, env)
            check.need(status == 0, 'failed command: ' + name)
            summaries = [line for line in stdout.splitlines() if line.startswith('test result:')]
            print('PASS: ' + name + ('; ' + '; '.join(summaries) if summaries else ''), flush=True)
        for tool in ('rustc', 'cargo'):
            if (output / ('after-' + tool)).exists():
                for stream in ('stdout.log', 'stderr.log'):
                    check.need((output / tool / stream).read_bytes() ==
                               (output / ('after-' + tool) / stream).read_bytes(), 'toolchain continuity')
    finally:
        after = inputs()
        (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
        check.need(before == after, 'runtime development inputs changed')


if __name__ == '__main__':
    main()
