#!/usr/bin/env python3
"""Build two signed scratch cohorts, collect matched MI300X trials, clean owned state."""
import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')
import argparse
import hashlib
import json
from pathlib import Path
import secrets
import shlex
import shutil
import signal
import tarfile
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')

def load(path, name, digest):
    raw = path.read_bytes()
    if path.is_symlink() or hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError('authenticated local helper: ' + str(path))
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), value.__dict__)
    return value

# This archived controller authenticates its transitive imports before executing them.
K = load(REPO / 'benchmarks/runtime_gfx942/xgmi_backing_budget_campaign.py', 'scratch_controller',
         '9b90dac696bcebe7a4cf7328d404e9edfc485ff1cd0af63b36ae4795b1dfdf76')
N = load(HERE / 'native.py', 'scratch_native', '4fe0aceb20efd2aeca038a26704348974c1a077b66f419f2debb8b25b325bdb0')
B, H, C = N.B, N.H, K.C
EXAMPLE = 'gfx942-runtime-xgmi-peer-benchmark'
BASELINE = 'a852464aee976a1ff1472f2e477e25b6cf2ac933'
PARSER_DIGESTS = {'diagnostic.py': '294385604f9f8b8494b721dfadd8c2c0f274e3f64c91edba8bff6c29feefb9e4',
                  'results.py': 'fc9355b27ab0329bcd31cf7b55ac529114026d74630eaf7d87998a9409b9b2e6'}
EXPECTED_DELTA = {
    'crates/fe2o3-kfd/src/topology.rs', 'crates/fe2o3-kfd/src/topology/link_properties.rs',
    'crates/fe2o3-kfd/src/topology/tests/link_properties.rs',
    'crates/fe2o3-kfd/src/topology/tests/prechecked_reads.rs',
    'crates/fe2o3-kfd/src/topology/tests/prechecked_reads/link_scratch.rs',
    'crates/fe2o3-kfd/src/topology/tests/directory_entries.rs',
    'crates/fe2o3-kfd/src/topology/tests/directory_entries/allocation_counter.rs',
}

def qualified_cpu(packet, candidate_files):
    before = H.parse_json((packet / 'inputs-before.json').read_bytes())
    after = H.parse_json((packet / 'inputs-after.json').read_bytes())
    H.need(before['runner'] == 'b1e15db084398e4e19cbb0f4f7a0588cf2594b89611d49248a1426243c339b4a', 'qualified CPU runner')
    H.need(H.sha(packet.parent / 'runner.py') == before['runner'], 'actual qualified CPU runner bytes')
    H.need(before == after and EXPECTED_DELTA <= before['source'].keys(), 'complete unchanged CPU source bracket')
    roots = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo', 'crates', 'benchmarks/runtime_gfx942']
    expected_source = {name: digest for name, digest in candidate_files.items()
                       if any(name == root or name.startswith(root + '/') for root in roots)
                       and (name.endswith(('.rs', '.toml', '.lock', '.json', '.py', '.cpp', '.hpp')) or '/fixtures/' in name)}
    H.need(before['source'] == expected_source, 'exact signed candidate CPU source roster')
    cargo = ['cargo', '--locked']
    packages = ['-p', 'fe2o3-kfd', '-p', 'fe2o3-runtime']
    commands = {'rustc': ['rustc', '-vV'], 'cargo': ['cargo', '-V'],
                'format': ['cargo', 'fmt', *packages, '--', '--check'],
                'docs': [*cargo, 'test', *packages, '--all-features', '--doc'],
                'clippy': [*cargo, 'clippy', *packages, '--all-features', '--all-targets', '--', '-D', 'warnings'],
                'after-rustc': ['rustc', '-vV'], 'after-cargo': ['cargo', '-V']}
    for label, target in [('gnu', []), ('musl', ['--target', 'x86_64-unknown-linux-musl'])]:
        for selection in ('topology::', 'currentness::'):
            commands[label + '-' + selection[:-2]] = [*cargo, 'test', '-p', 'fe2o3-kfd', '--all-features', '--lib', *target, selection, '--', '--test-threads=4']
        commands[label + '-runtime'] = [*cargo, 'test', '-p', 'fe2o3-runtime', '--all-features', '--lib', *target, '--', '--test-threads=4']
    H.need({path.name for path in packet.iterdir() if path.is_dir()} == commands.keys(), 'complete CPU command roster')
    for stage, command in commands.items():
        row = H.parse_json((packet / stage / 'record.json').read_bytes())
        H.need(row['command'] == command, 'exact CPU command: ' + stage)
        H.need(type(row['status']) is int and row['status'] == 0 and row['group_absent'] is True and 'exception' not in row, 'successful reaped CPU command: ' + stage)
    for tool in ('rustc', 'cargo'):
        for stream in ('stdout.log', 'stderr.log'):
            H.need((packet / tool / stream).read_bytes() == (packet / ('after-' + tool) / stream).read_bytes(), 'CPU tool continuity')
    return B.inventory(packet)

def source(rec, cohort, commit, output):
    folder = rec.run(cohort + '-tree', [*K.GIT, 'ls-tree', '-rz', commit, '--', *K.SELECTORS], 120, env=K.GIT_ENV)
    entries = {}
    for row in (folder / 'stdout').read_bytes().rstrip(b'\0').split(b'\0'):
        header, raw_name = row.split(b'\t')
        mode, kind, oid = header.decode().split()
        name = raw_name.decode()
        H.need(mode in ('100644', '100755') and kind == 'blob' and name not in entries, 'ordinary unique source')
        entries[name] = (mode, oid)
    selectors = [value for value in K.SELECTORS if any(name == value or name.startswith(value + '/') for name in entries)]
    archive_folder = rec.run(cohort + '-archive', [*K.GIT, 'archive', '--format=tar', commit, *selectors], 120, env=K.GIT_ENV)
    checkout = output / ('source-' + cohort)
    checkout.mkdir()
    with tarfile.open(archive_folder / 'stdout', 'r:') as archive:
        members = archive.getmembers()
        directories = {str(parent) for name in entries for parent in Path(name).parents if str(parent) != '.'}
        observed_directories = set()
        for member in members:
            H.need(member.isfile() or member.isdir(), 'ordinary source archive entries')
            if member.isdir():
                name = member.name.rstrip('/')
                H.need(name in directories and name not in observed_directories, 'exact source parent directory')
                observed_directories.add(name)
        B.validate_members([member for member in members if member.isfile()], entries)
        archive.extractall(checkout, filter='data')
    files = B.inventory(checkout)
    H.need(set(files) == set(entries), 'complete source extraction')
    for name, (mode, oid) in entries.items():
        raw = (checkout / name).read_bytes()
        H.need(hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == oid, 'signed source blob')
        H.need(bool((checkout / name).stat().st_mode & 0o111) == (mode == '100755'), 'source mode')
    B.write_json(output / (cohort + '-source.json'), files)
    return checkout, files

def build_environment(output, cohort):
    H.need(cohort in ('baseline', 'candidate'), 'known build cohort')
    target = output / ('target-' + cohort)
    target.mkdir()
    return {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
            'LANG': 'C', 'LC_ALL': 'C', 'CARGO_TARGET_DIR': str(target), 'CARGO_INCREMENTAL': '0',
            'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never', 'RUSTUP_TOOLCHAIN': 'nightly-2026-04-03'}

def distinct_cohort_binaries(payload):
    H.need(H.sha(payload / 'kfd-baseline') != H.sha(payload / 'kfd-candidate'), 'scratch candidate must not reuse the baseline ELF')

def build_cohort(rec, cohort, commit, output, payload):
    env = build_environment(output, cohort)
    target = Path(env['CARGO_TARGET_DIR'])
    rec.cwd = REPO
    rec.run(cohort + '-signature', [*K.GIT, '-c', 'gpg.ssh.allowedSignersFile=' + C.SIGNERS, 'verify-commit', commit], 30, env=K.GIT_ENV)
    checkout, files = source(rec, cohort, commit, output)
    rec.cwd = checkout
    rec.run(cohort + '-rustc', ['rustc', '-vV'], 30, env=env)
    rec.run(cohort + '-cargo', ['cargo', '-V'], 30, env=env)
    rec.run(cohort + '-build', ['cargo', 'build', '--locked', '--release', '-p', 'fe2o3-runtime',
                              '--target', 'x86_64-unknown-linux-musl', '--features', 'hardware-diagnostic', '--example', EXAMPLE], 1800, env=env)
    binary = target / ('x86_64-unknown-linux-musl/release/examples/' + EXAMPLE)
    shutil.copy2(binary, payload / ('kfd-' + cohort))
    rec.run(cohort + '-tests', ['cargo', 'test', '--locked', '--release', '-p', 'fe2o3-runtime',
                              '--target', 'x86_64-unknown-linux-musl', '--features', 'hardware-diagnostic', '--example', EXAMPLE], 1800, env=env)
    for tool, command in [('rustc', ['rustc', '-vV']), ('cargo', ['cargo', '-V'])]:
        folder = rec.run(cohort + '-after-' + tool, command, 30, env=env)
        for stream in ('stdout', 'stderr'):
            H.need(H.sha(folder / stream) == H.sha(rec.output / (cohort + '-' + tool) / stream), 'unchanged tool')
    H.need(B.inventory(checkout) == files, 'unchanged build inputs')
    return {'commit': commit, 'files': files}, env

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cpu-packet', type=Path, required=True)
    parser.add_argument('--device', action='append', required=True)
    args = parser.parse_args()
    devices = K.devices_from_args(args.device)
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    H.need(args.candidate == K.git('rev-parse', args.candidate).decode().strip(), 'exact candidate commit')
    K.git('merge-base', '--is-ancestor', BASELINE, args.candidate)
    delta = K.git('diff', '--name-only', BASELINE, args.candidate, '--', *K.SELECTORS).decode().splitlines()
    H.need(set(delta) == EXPECTED_DELTA, 'only reviewed scratch source delta')
    output = args.output.resolve()
    output.mkdir()
    rec = B.Recorder(output / 'local', REPO)
    protocol = {name: H.sha(HERE / name) for name in ('campaign.py', 'native.py', 'test_campaign.py', 'test_cpu_binding.py', 'validate_protocol.py')}
    B.write_json(output / 'protocol-before.json', protocol)
    frozen = output / 'protocol'
    frozen.mkdir()
    for name in protocol:
        shutil.copy2(HERE / name, frozen / name)
    payload = output / 'payload'
    payload.mkdir()
    copies = {'native.py': HERE / 'native.py', 'hot.py': Path(H.__file__), 'base.py': Path(B.__file__),
              'diagnostic.py': REPO / 'docs/evidence/dev-xgmi-currentness-attribution-mi300x-2026-09-20/results.py',
              'results.py': REPO / 'docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/results.py'}
    for name, path in copies.items():
        if name in PARSER_DIGESTS:
            H.need(H.sha(path) == PARSER_DIGESTS[name], 'frozen parser: ' + name)
        shutil.copy2(path, payload / name)
    sources, build_environments = {}, {}
    H.need(H.sha(Path(C.SIGNERS)) == C.SIGNERS_SHA, 'pinned signer')
    for cohort, commit in [('baseline', BASELINE), ('candidate', args.candidate)]:
        sources[cohort], build_environments[cohort] = build_cohort(rec, cohort, commit, output, payload)
    for tool in ('rustc', 'cargo'):
        H.need(H.sha(rec.output / ('baseline-' + tool) / 'stdout') == H.sha(rec.output / ('candidate-' + tool) / 'stdout'), 'matched build tools')
    distinct_cohort_binaries(payload)
    cpu_files = qualified_cpu(args.cpu_packet, sources['candidate']['files'])
    shutil.copytree(args.cpu_packet, output / 'cpu-qualification')
    shutil.copy2(args.cpu_packet.parent / 'runner.py', output / 'cpu-runner.py')
    H.need(B.inventory(output / 'cpu-qualification') == cpu_files, 'byte-exact CPU qualification copy')
    remote_files = {name: sources['candidate']['files'][name] for name in C.SOURCE_FILES}
    with tarfile.open(payload / 'source.tar.gz', 'w:gz') as archive:
        for name in remote_files:
            H.need(sources['baseline']['files'][name] == remote_files[name], 'unchanged comparators/observer')
            archive.add(output / 'source-candidate' / name, arcname=name, recursive=False)
    H.need({name: H.sha(HERE / name) for name in protocol} == protocol, 'unchanged protocol before remote execution')
    binding = {'commit': args.candidate, 'cohorts': sources, 'source_files': remote_files, 'protocol_files': protocol,
               'cpu_qualification_files': cpu_files,
               'cpu_runner_sha256': H.sha(output / 'cpu-runner.py'),
               'payload': B.inventory(payload), 'devices': devices, 'controls': N.CONTROLS,
               'order': N.ORDER, 'target': 'x86_64-unknown-linux-musl', 'profile': 'release default',
               'features': 'default,hardware-diagnostic', 'local_build_environments': build_environments}
    B.write_json(output / 'binding.json', binding)
    marker = {'path': N.PREFIX + secrets.token_hex(8), 'commit': args.candidate, 'binding_sha256': H.sha(output / 'binding.json')}
    B.write_json(output / 'owner.json', marker)
    serialized = json.dumps(marker, sort_keys=True, separators=(',', ':'))
    rec.cwd = REPO
    attempted, collected, cleaned, failures = False, False, False, []
    def control(name):
        return rec.run(name, ['ssh', '-T', *C.SSH, 'mi300x', shlex.join(['/usr/bin/python3', '-I', '-B', '-', name, serialized])],
                       120, stdin=C.control_bytes(N.PREFIX))
    try:
        control('create')
        rec.run('upload', ['scp', '-q', *C.SSH, '--', *(str(payload / name) for name in sorted(N.PAYLOAD)),
                          str(output / 'binding.json'), 'mi300x:' + marker['path'] + '/'], 120)
        attempted = True
        rec.run('native', K.native_command(serialized), 12000)
    except BaseException as error:
        failures.append(repr(error))
    finally:
        try:
            folder = control('inventory')
            expected = H.parse_json((folder / 'stdout').read_bytes())
            rec.run('collect', ['scp', '-q', '-r', *C.SSH, '--', 'mi300x:' + marker['path'] + '/results', str(output / 'remote')], 120)
            H.need(B.inventory(output / 'remote') == expected, 'byte-exact collection')
            B.write_json(output / 'remote-inventory.json', expected)
            collected = True
        except BaseException as error:
            failures.append('collection: ' + repr(error))
        cleaned, errors = C.settle_remote(control, collected=collected, native_attempted=attempted)
        failures.extend(errors)
        H.need(B.inventory(payload) == binding['payload'], 'unchanged retained payload')
        after = {name: H.sha(HERE / name) for name in protocol}
        B.write_json(output / 'protocol-after.json', after)
        H.need(after == protocol, 'unchanged protocol after remote execution')
        B.write_json(output / 'collection.json', {'failures': failures, 'owned_cleanup': cleaned,
                     'exclusive_reservation': False, 'performance_acceptance': False})
    H.need(not failures and cleaned, 'campaign did not qualify')

if __name__ == '__main__':
    main()
