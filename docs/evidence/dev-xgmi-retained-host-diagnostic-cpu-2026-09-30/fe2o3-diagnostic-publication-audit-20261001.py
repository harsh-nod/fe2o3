#!/usr/bin/env python3
"""Independently replay compact CPU evidence, signed blobs and local CI receipts."""
import hashlib
import json
import lzma
from pathlib import Path, PurePosixPath
import stat
import subprocess
import tarfile

BASE = Path('/home/harsh/.codex-tmp')
PUB = BASE / 'fe2o3-xgmi-retained-host-diagnostic-publication-20260930'
REPO = BASE / 'fe2o3-r61-execution'
CI = BASE / 'fe2o3-kfd-diagnostic-ci-results-20260930'
OUT = BASE / 'fe2o3-diagnostic-publication-audit-20261001.json'
COMMIT = 'd6b1906d0d24b366ea4222fbca4ae4a6966fad6c'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def raw(path):
    assert path.is_file() and not path.is_symlink() and path.resolve() == path
    return path.read_bytes()


def pinned(name, expected):
    data = raw(PUB / name)
    assert digest(data) == expected, name
    return data


def archive(path, index, originals=False):
    values = {}
    with tarfile.open(path, 'r:xz') as handle:
        for member in handle:
            name = member.name
            assert member.isfile() and not member.issym() and not member.islnk()
            assert str(PurePosixPath(name)) == name and not name.startswith('/')
            assert '..' not in PurePosixPath(name).parts and name not in values
            assert name in index and member.size == index[name]['size']
            data = handle.extractfile(member).read()
            assert len(data) == member.size and digest(data) == index[name]['sha256']
            if originals:
                assert raw(Path(index[name]['source'])) == data
            values[name] = data
    assert set(values) == set(index)
    return values


def git(*args, data=None):
    return subprocess.run(['/usr/bin/git', '--no-replace-objects', *args], cwd=REPO,
                          input=data, check=True, capture_output=True, timeout=120).stdout


def main():
    assert not OUT.exists()
    summary = json.loads(pinned('summary.json', 'cde46371e62fdf65acf8285bc5a605a8c47cd3e65b50225983abe2409dd0164a'))
    index = json.loads(pinned('raw-index.json', summary['raw_index_sha256']))
    pinned('raw.tar.xz', summary['archive']['sha256'])
    values = archive(PUB / 'raw.tar.xz', index, originals=True)
    assert len(values) == 410 and sum(map(len, values.values())) == 25758844
    source_bytes = values['linked-cpu/source-before.json']
    assert digest(source_bytes) == summary['cpu_source_snapshot_sha256']
    assert source_bytes == values['linked-cpu/source-after.json']
    cpu = json.loads(source_bytes)
    verification = json.loads(pinned('signed-source-verification.json',
                                    summary['signed_source_verification']['verification_sha256']))
    pinned('signed-source-records.tar.xz', verification['source_records_sha256'])
    records = archive(PUB / 'signed-source-records.tar.xz', verification['source_records_members'])
    packed = pinned('signed-source-binding.json.xz', verification['binding_sha256'])
    binding_bytes = lzma.decompress(packed)
    assert digest(binding_bytes) == verification['binding_uncompressed_sha256']
    binding = json.loads(binding_bytes)
    files = binding['files']
    assert binding['commit'] == verification['commit'] == summary['signed_implementation_commit'] == COMMIT
    assert len(cpu) == len(files) == 6477 and set(cpu) == set(files)
    assert git('rev-parse', COMMIT + '^').decode().strip() == binding['parent'] == verification['parent']
    tree = {}
    for entry in git('ls-tree', '-rz', COMMIT, '--', *binding['source_roots']).split(b'\0'):
        if entry:
            header, path = entry.split(b'\t', 1)
            mode, kind, oid = header.decode().split()
            assert kind == 'blob' and mode in ('100644', '100755')
            tree[path.decode()] = (mode, oid)
    assert set(tree) == set(files)
    objects = git('cat-file', '--batch', data=''.join(v[1] + '\n' for v in tree.values()).encode())
    offset = 0
    for name, (mode, oid) in tree.items():
        end = objects.index(b'\n', offset)
        observed, kind, size = objects[offset:end].decode().split()
        offset = end + 1
        data = objects[offset:offset + int(size)]
        offset += int(size)
        assert observed == oid and kind == 'blob' and objects[offset:offset + 1] == b'\n'
        assert hashlib.sha1(b'blob ' + size.encode() + b'\0' + data).hexdigest() == oid
        offset += 1
        assert files[name] == dict(git_blob=oid, mode=mode, sha256=digest(data), size=len(data))
        assert cpu[name] == dict(mode=mode, sha256=digest(data))
        path = Path(binding['source_root']) / name
        assert raw(path) == data and stat.S_IMODE(path.stat().st_mode) == int(mode[-3:], 8)
    assert offset == len(objects)
    bundle = pinned('signed-candidate.bundle', verification['bundle_sha256'])
    assert len(bundle) == verification['bundle_size']
    git('bundle', 'verify', str(PUB / 'signed-candidate.bundle'))
    pinned('allowed-signers', verification['allowed_signers_sha256'])
    verified = subprocess.run(['/usr/bin/git', '-c', 'gpg.ssh.allowedSignersFile=' + str(PUB / 'allowed-signers'),
                               'verify-commit', COMMIT], cwd=REPO, check=True, capture_output=True, timeout=60)
    assert not verified.stdout and verified.stderr.decode() == verification['signature_stderr']
    ci = json.loads(raw(CI / 'result.json'))
    assert ci['all_17_pass'] and len(ci['commands']) == 17
    assert ci['head'] == '4ad64047b0887a747a41aa7b0b9fa2f4206279c1'
    assert digest(raw(REPO / '.github/workflows/runtime-component-source-guards.yml')) == ci['workflow_sha256']
    for n, row in enumerate(ci['commands'], 1):
        assert row['exit'] == 0
        for stream in ('stdout', 'stderr'):
            assert digest(raw(CI / f'{n:02}-{stream}')) == row[stream + '_sha256']
    assert b'Ran 64 tests' in raw(CI / '16-stderr') and b'Ran 40 tests' in raw(CI / '17-stderr')
    result = dict(archive_members=len(values), source_record_members=len(records), signed_source_files=len(files),
                  originals_and_signed_blobs_match=True, signature_and_bundle_verified=True,
                  local_ci_commands=17, new_solver_or_rust_test_or_gpu_execution=False,
                  source_commit=COMMIT, ci_commit=ci['head'],
                  package={p.name: digest(raw(p)) for p in sorted(PUB.iterdir())},
                  auditor_sha256=digest(raw(Path(__file__).resolve())))
    with OUT.open('x') as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write('\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
