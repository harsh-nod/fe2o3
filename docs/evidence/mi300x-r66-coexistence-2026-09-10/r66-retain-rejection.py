"""Preserve rejected qualification without promoting it to accepted evidence."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import subprocess
import tarfile

temporary = Path('/home/harsh/.codex-tmp')
repo = temporary / 'fe2o3-r61-execution'
root = repo / 'docs/evidence/mi300x-r66-coexistence-2026-09-10'
capture = temporary / 'r66-bd8aa3de-capture.tar'
controller = json.loads((temporary / 'r66-remote-controller.json').read_text())
assert controller['returncode'] == 2
assert capture.stat().st_size < 512 << 20
with tarfile.open(capture, mode='r:') as archive:
    members = archive.getmembers()
    assert len(members) < 512 and len({m.name for m in members}) == len(members)
    assert all(m.isfile() or m.isdir() for m in members)
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
commit = controller['source_commit']
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
assert len(commands) == 25 and all(c['returncode'] == 0 for c in commands[:-1])
assert commands[-1]['returncode'] == 2
groups = [c['process_group'] for c in commands if 'process_group' in c]
pids = [int(line.split()[1]) for line in outer['process-identities.txt'].decode().splitlines()]
request = dict(stage=controller['stage'], pids=sorted(set(pids + groups)), process_groups=groups)
cleanup = subprocess.check_output(['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', 'mi300x',
    'python3 -c ' + shlex.quote((temporary / 'r66-check-remote-cleanup.py').read_text())],
    input=json.dumps(request).encode(), timeout=60)
raw = root / 'raw/musl-rejected'
raw.mkdir(parents=True, exist_ok=False)
for name, value in payload.items():
    if name != 'source.tar':
        (raw / name).write_bytes(value)
(root / 'outer').mkdir()
for name, value in outer.items():
    (root / 'outer' / name).write_bytes(value)
(root / 'cleanup.json').write_bytes(cleanup)
for name in ('r66-remote-controller.json', 'r66-remote-transport.log', 'fe2o3-r66-owner-remote.sh',
             'r66-run-remote.py', 'r66-check-remote-cleanup.py', 'r66-retain-rejection.py'):
    shutil.copyfile(temporary / name, root / name)
summary = dict(source_commit=commit, signed_source_verified=True, source_archive_exact=True,
    source_archive_sha256=hashlib.sha256(payload['source.tar']).hexdigest(), source_files=len(sources),
    capture_sha256=hashlib.sha256(capture.read_bytes()).hexdigest(), capture_bytes=capture.stat().st_size,
    commands=len(commands), successful_commands=24, rejected_command='monitor-owner-0',
    acceptance=False, performance_claim=False, physical_overlap='unmeasured',
    actual_binary_retained=False, independent_actual_elf_audit=False,
    cleanup_verified=True, rejection=json.loads(payload['rejection.json']))
(root / 'rejection-audit.json').write_text(json.dumps(summary, indent=2) + '\n')
(root / 'retained-files.sha256').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(root)}\n'
    for p in sorted(root.rglob('*')) if p.is_file() and p.name != 'retained-files.sha256'))
print(json.dumps(summary, indent=2))
