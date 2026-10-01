#!/usr/bin/env python3
"""Compact exact immutable campaign bytes, retaining explicit external dependencies."""
import hashlib
import io
import json
from pathlib import Path
import stat
import subprocess
import sys
import tarfile

BASE = Path('/home/harsh/.codex-tmp')
RECORDS = BASE / 'fe2o3-distributed-codec-primitives-records-20260930-cursor'
SOURCE = BASE / 'fe2o3-distributed-codec-primitives-20260930-argument'
OUTPUT = BASE / 'fe2o3-codec-publication-20261001'
SIGNED = 'e02b7229ab6b85c2c1697d239d6f82e96ff04d45'
PARENT = '0c38a20c65452ee34cd97f2015b1f70da8211b18'

def need(value, message):
    if not value:
        raise ValueError(message)

def digest(data):
    return hashlib.sha256(data).hexdigest()

def raw(path):
    need(path.resolve() == path and path.is_file() and not path.is_symlink(), 'ordinary canonical input')
    return path.read_bytes()

def save(path, value):
    with path.open('x') as stream:
        stream.write(json.dumps(value, sort_keys=True, indent=2) + '\n')

need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, 'python -I -B required')
need(digest(raw(BASE / 'fe2o3-codec-primitives-root-readback-20261001.json')) ==
     'ba066fac656b875ef7138db35b8832f655dd755e03e2f4eafdca545dc6b46e31', 'accepted independent root readback')
need(json.loads(raw(BASE / 'fe2o3-codec-integration-ci-results-20261001/result.json'))['all_18_pass'] is True,
     'complete local CI replay')
files = {}
for index in (1, 2, 3):
    packet = RECORDS / ('signed-primitive-campaign-attempt-' + str(index))
    for path in sorted(packet.rglob('*')):
        need(not path.is_symlink(), 'no archive symlinks')
        if path.is_file():
            files[str(path.relative_to(RECORDS))] = path
for name in ('signed_primitive_campaign_v2.py', 'test_signed_primitive_campaign_v2.py',
             'signed-primitive-prepared-2.json', 'audit_signed_primitive_attempt3.py',
             'signed-primitive-post-audit-3.json', 'integration-guard-audit-1.json'):
    files['controllers/' + name] = RECORDS / name
for name in ('fe2o3-codec-primitives-root-readback-20261001.py',
             'fe2o3-codec-primitives-root-readback-20261001.json',
             'fe2o3-codec-integration-audit-20261001.py', 'fe2o3-codec-integration-audit-20261001.json',
             'fe2o3-codec-integration-ci-20261001.py'):
    files['integration/' + name] = BASE / name
ci = BASE / 'fe2o3-codec-integration-ci-results-20261001'
for path in sorted(ci.iterdir()):
    files['local-ci/' + path.name] = path
git = ['/usr/bin/git', '--no-replace-objects', '-c', 'gc.auto=0']
names = subprocess.check_output(git + ['diff', '--name-only', PARENT, SIGNED], cwd=SOURCE).decode().splitlines()
need(len(names) == 9, 'exact nine-path signed implementation')
for name in names:
    need(raw(SOURCE / name) == subprocess.check_output(git + ['show', SIGNED + ':' + name], cwd=SOURCE),
         'signed candidate bytes')
    files['candidate/' + name] = SOURCE / name
OUTPUT.mkdir(mode=0o700)
archive = OUTPUT / 'campaigns.tar.xz'
index = {}
with tarfile.open(archive, 'x:xz', preset=3, format=tarfile.PAX_FORMAT) as tar:
    for name, path in sorted(files.items()):
        data = raw(path)
        need(not data.startswith(b'\x7fELF'), 'no binary executables in evidence archive')
        mode = stat.S_IMODE(path.stat().st_mode)
        entry = tarfile.TarInfo(name)
        entry.size, entry.mode, entry.mtime = len(data), mode, 0
        tar.addfile(entry, io.BytesIO(data))
        index[name] = {'sha256': digest(data), 'bytes': len(data), 'mode': mode}
with tarfile.open(archive, 'r:xz') as tar:
    members = tar.getmembers()
    need(len(members) == len(index) and {m.name for m in members} == set(index), 'complete unique archive roster')
    for member in members:
        need(member.isfile() and member.size == index[member.name]['bytes'] and member.mode == index[member.name]['mode'],
             'ordinary exact member metadata')
        data = tar.extractfile(member).read()
        need(digest(data) == index[member.name]['sha256'] and data == raw(files[member.name]),
             'archive equals unchanged original bytes')
save(OUTPUT / 'archive-index.json', index)
bundle = OUTPUT / 'signed-candidate.bundle'
subprocess.run(git + ['bundle', 'create', str(bundle), 'HEAD', '^' + PARENT], cwd=SOURCE, check=True, timeout=120)
verified = subprocess.run(git + ['bundle', 'verify', str(bundle)], cwd=SOURCE, capture_output=True, check=True, timeout=60)
need(subprocess.check_output(git + ['rev-parse', 'HEAD'], cwd=SOURCE).decode().strip() == SIGNED, 'unchanged signed source')
summary = {'signed_commit': SIGNED, 'bundle_prerequisite': PARENT, 'archive_files': len(index),
           'archive_logical_bytes': sum(row['bytes'] for row in index.values()),
           'archive_sha256': digest(raw(archive)), 'archive_bytes': archive.stat().st_size,
           'archive_index_sha256': digest(raw(OUTPUT / 'archive-index.json')),
           'bundle_sha256': digest(raw(bundle)), 'bundle_bytes': bundle.stat().st_size,
           'bundle_verification_stdout': verified.stdout.decode(), 'bundle_verification_stderr': verified.stderr.decode(),
           'qualification': {'accepted_attempt': 3, 'stages': 38, 'closed_groups': 38, 'full_proofs': 3,
                             'verified_each': 54, 'logical_negatives': 31},
           'earlier_attempts_preserved': [1, 2], 'local_ci_commands_passed': 18,
           'source_inputs': 6500, 'self_contained_evidence': False,
           'scope': 'Exact raw publication and signed source custody, not new qualification. CPU/static evidence is explicitly reused. '
                    'Prior discovery, tool binaries, original/retained CPU executables and external helper history remain local dependencies. '
                    'No whole codec, authentication, Context, GPU, performance, parity or milestone closure.'}
save(OUTPUT / 'summary.json', summary)
print(json.dumps({key: summary[key] for key in ('archive_files', 'archive_bytes', 'archive_logical_bytes', 'archive_sha256', 'bundle_sha256')}))
