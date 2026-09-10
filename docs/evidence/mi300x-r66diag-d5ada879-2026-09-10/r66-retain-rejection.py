"""Preserve rejected qualification without promoting it to accepted evidence."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
commit = sys.argv[1]
assert re.fullmatch(r'[0-9a-f]{40}', commit)
label = 'r66diag-' + commit[:8]
root = repo / ('docs/evidence/mi300x-' + label + '-2026-09-10')
capture = temporary / (label + '-capture.tar')
controller = json.loads((temporary / (label + '-controller.json')).read_text())
assert controller['source_commit'] == commit
assert controller['returncode'] == 2
assert capture.stat().st_size < 512 << 20
with tarfile.open(capture, mode='r:') as archive:
    members = archive.getmembers()
    assert len(members) < 512 and len({m.name for m in members}) == len(members)
    assert all(m.isfile() or m.isdir() for m in members)
    assert sum(m.size for m in members) <= 512 << 20
    assert all(not PurePosixPath(m.name).is_absolute()
               and '..' not in PurePosixPath(m.name).parts for m in members)
    prefixes = {m.name + '/' for m in members if m.isdir()
        and re.fullmatch(r'output/r66-rejected-[0-9a-f]{32}', m.name)}
    assert len(prefixes) == 1
    prefix = prefixes.pop()
    payload = {m.name[len(prefix):]: archive.extractfile(m).read() for m in members
        if m.isfile() and m.name.startswith(prefix)}
    outer = {name: archive.extractfile(name).read() for name in
        ('run.log', 'clone.log', 'process-identities.txt', 'exit-status.txt')}
assert outer['exit-status.txt'] == b'2\n'
assert all(PurePosixPath(name).name == name for name in payload)
git = ['git', '-C', str(repo)]
subprocess.run(git + ['-c', 'gpg.ssh.allowedSignersFile=' + str(temporary / 'fe2o3-r60-trusted-allowed-signers'),
                     'verify-commit', commit], check=True)
assert payload['signed-commit.txt'] == subprocess.check_output(git + ['cat-file', 'commit', commit])
assert payload['source.tar'] == subprocess.check_output(git + ['archive', '--format=tar', commit])
with tarfile.open(fileobj=io.BytesIO(payload['source.tar'])) as archive:
    sources = {m.name: hashlib.sha256(archive.extractfile(m).read()).hexdigest()
               for m in archive.getmembers() if m.isfile()}
assert sources == json.loads(payload['source-files.json'])
commands = json.loads(payload['commands.json'])
prelaunch_busy = len(commands) == 24
if prelaunch_busy:
    assert all(c['returncode'] == 0 for c in commands)
    assert commands[-1]['stdout'] == 'evidence/023-telemetry-owner-0-start.stdout'
    assert json.loads(payload['rejection.json'])['error'] == 'selected GPU identity changed or GPU is busy'
    card = json.loads(payload['023-telemetry-owner-0-start.stdout'])['card1']
    assert card['Unique ID'] == '0xab83d2ffef0d3cdf' and card['PCI Bus'].lower() == '0000:26:00.0'
    assert int(card['GPU use (%)']) > 5 or int(card['GPU Memory Allocated (VRAM%)']) != 0
    assert not any('-monitor-owner-' in c['stdout'] for c in commands)
else:
    assert 25 <= len(commands) <= 32 and all(c['returncode'] == 0 for c in commands[:-1])
    assert commands[-1]['returncode'] == 2
assert 0 < len(payload['owner-binary']) <= 64 << 20
for relative in ('benchmarks/runtime_gfx942/run-r61-owner-mi300x.py',
                 'benchmarks/runtime_gfx942/run-r60-pipeline-mi300x.py',
                 'scripts/runtime_pure_rust_audit.py',
                 'scripts/runtime-pure-rust-policy.json'):
    assert hashlib.sha256((repo / relative).read_bytes()).hexdigest() == sources[relative]
spec = importlib.util.spec_from_file_location('r66diag_owner_audit',
    repo / 'benchmarks/runtime_gfx942/run-r61-owner-mi300x.py')
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)
policy = json.loads((repo / 'scripts/runtime-pure-rust-policy.json').read_text())
with tempfile.TemporaryDirectory(prefix='fe2o3-r66diag-audit-') as directory:
    binary = Path(directory) / 'owner-binary'
    binary.write_bytes(payload['owner-binary'])
    symbols = subprocess.check_output(['/usr/bin/nm', '--format=posix', '--no-demangle', binary], text=True)
    headers = subprocess.check_output(['/usr/bin/readelf', '--program-headers', '--dynamic', '--wide', binary], text=True)
    count = owner.validate_static_symbols(symbols, policy)
    owner.validate_static_headers(headers)
    assert symbols == payload['018-full-symbols.stdout'].decode()
    assert headers == payload['017-static-headers.stdout'].decode()
    assert b'__pthread_get_minstack' not in payload['owner-binary'] and b'tokio' not in payload['owner-binary']
    assert count == json.loads(payload['static-closure.json'])['full_symbol_names']
    audit = [sys.executable, str(repo / 'scripts/runtime_pure_rust_audit.py'),
             '--policy', str(repo / 'scripts/runtime-pure-rust-policy.json')]
    elf = subprocess.check_output(audit + ['elf', '--input', str(binary)], text=True)
    metadata_path = Path(directory) / 'cargo-metadata.json'
    metadata_path.write_bytes(payload['cargo-metadata.json'])
    metadata = subprocess.check_output(audit + ['metadata', '--input', str(metadata_path),
                                               '--root', 'fe2o3-runtime'], text=True)
groups = [c['process_group'] for c in commands if 'process_group' in c]
pids = [int(line.split()[1]) for line in outer['process-identities.txt'].decode().splitlines()]
request = dict(stage=controller['stage'], pids=sorted(set(pids + groups)), process_groups=groups,
               require_selected_idle=not prelaunch_busy)
cleanup = subprocess.check_output(['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'mi300x',
    'python3 -c ' + shlex.quote((temporary / 'r66-check-remote-cleanup.py').read_text())],
    input=json.dumps(request).encode(), timeout=60)
raw = root / 'raw/musl-rejected'
raw.mkdir(parents=True, exist_ok=False)
for name, value in payload.items():
    if name not in ('source.tar', 'owner-binary'):
        (raw / name).write_bytes(value)
(root / 'outer').mkdir()
for name, value in outer.items():
    (root / 'outer' / name).write_bytes(value)
(root / 'cleanup.json').write_bytes(cleanup)
for name in (label + '-controller.json', label + '-transport.log', 'fe2o3-r66-owner-remote.sh',
             'r66-run-remote.py', 'r66-check-remote-cleanup.py', 'r66-retain-rejection.py'):
    shutil.copyfile(temporary / name, root / name)
summary = dict(source_commit=commit, signed_source_verified=True, source_archive_exact=True,
    source_archive_sha256=hashlib.sha256(payload['source.tar']).hexdigest(), source_files=len(sources),
    capture_sha256=hashlib.sha256(capture.read_bytes()).hexdigest(), capture_bytes=capture.stat().st_size,
    commands=len(commands), successful_commands=sum(c['returncode'] == 0 for c in commands),
    last_command=commands[-1], qualifier_started=not prelaunch_busy,
    rejection_stage='prelaunch-shared-gpu-busy' if prelaunch_busy else 'qualifier',
    acceptance=False, performance_claim=False, physical_overlap='unmeasured',
    actual_binary_retained=True, independent_actual_elf_audit=True,
    binary_sha256=hashlib.sha256(payload['owner-binary']).hexdigest(),
    binary_bytes=len(payload['owner-binary']), static_symbol_names=count,
    elf_audit=elf.strip(), metadata_audit=metadata.strip(),
    cleanup_verified=True, rejection=json.loads(payload['rejection.json']))
(root / 'rejection-audit.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
