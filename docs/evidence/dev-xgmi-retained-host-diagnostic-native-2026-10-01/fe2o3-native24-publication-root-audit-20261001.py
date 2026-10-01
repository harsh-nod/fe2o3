#!/usr/bin/env python3
"""Read back the compact native24 publication against its retained originals."""

import hashlib
import json
from pathlib import Path
import sys
import tarfile

BASE = Path('/home/harsh/.codex-tmp')
PACKET = BASE / 'fe2o3-native24-publication-20261001-revision-2'
OUTPUT = BASE / 'fe2o3-native24-publication-root-audit-20261001.json'
PINS = {
    'publication-index.json': 'f29c02ac47517e76cd142e74475808e0cd1bb145dd54dfce1749e187152d5b80',
    'summary.json': '2961d5b50994d81c3ebf724eb528b505bce58d5c37cb49975b3c46fa56528f7b',
    'raw.tar.xz': '02c247a483e66625527cf35edd575eb53377a42f2a21b7c1006106f1c3fa2342',
    'signed-source.bundle': '0014112ca92fac62ccae042334b05dbb4faa3a35868d398114a9726bf0454223',
    'package.py': 'd688e46dcb1c7d083454d3df3fb86ccebfde405478fb491320a06d4cfd069511',
    'analysis.json': 'ed6cd5bcb90fd32b4a5c61e80180a3010cdb49a01d025fb87439aee887e07d30',
    'root-readback.json': '3bd1fdabdcd8a65bc726835f9184a6ffaac63118a1bf1ac75d46bb015d9f9f84',
}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def row(path):
    need(path.is_file() and not path.is_symlink(), 'ordinary file')
    return {'sha256': sha(path), 'bytes': path.stat().st_size,
            'mode': path.stat().st_mode & 0o777}


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize,
         'use python3 -I -B')
    need(not OUTPUT.exists(), 'fresh audit output')
    for name, digest in PINS.items():
        need(sha(PACKET / name) == digest, 'reviewed pin: ' + name)
    index = load(PACKET / 'publication-index.json')
    actual = {p.relative_to(PACKET).as_posix(): row(p)
              for p in PACKET.rglob('*') if p.is_file()
              and p.name != 'publication-index.json'}
    need(actual == index, 'complete public file roster, bytes, lengths and modes')
    raw = load(PACKET / 'raw-index.json')
    need(len(raw) == 79, 'outer raw roster')
    seen = set()
    with tarfile.open(PACKET / 'raw.tar.xz', 'r:xz') as archive:
        for member in archive:
            need(member.isfile() and member.name in raw and member.name not in seen,
                 'unique ordinary archive member')
            expected = raw[member.name]
            original = Path(expected['original_path'])
            need(original.is_absolute() and original.resolve() == original, 'original path')
            need(row(original) == {k: expected[k] for k in ('sha256', 'bytes', 'mode')},
                 'unchanged original: ' + member.name)
            need(member.size == expected['bytes'] and member.mode == expected['mode'],
                 'archive length and mode')
            with archive.extractfile(member) as stream:
                need(hashlib.file_digest(stream, 'sha256').hexdigest() == expected['sha256'],
                     'archive bytes')
            seen.add(member.name)
    need(seen == set(raw), 'archive completeness')
    native_member = 'native24/remote-commands/pull/stdout'
    need(raw[native_member]['sha256'] ==
         '1375a5d7dd8968bd905a0a1ce74add0b3a2c38bf5336f44f63dc28a0592f2227',
         'unchanged nested native archive, including all four ELFs')
    census = load(PACKET / 'signature-census.json')
    names = ['signature', 'commit', 'source-head', 'bundle-create', 'bundle-verify', 'bundle-heads']
    need(census['attempt_order'] == names, 'six bounded Git stages')
    need(census['scope'] == 'fresh-recorder-closed-groups-no-historical-pid-probes', 'census scope')
    records = {r['name']: r for r in census['records']}
    need(len(records) == 6 and set(records) == set(names), 'exact census records')
    previous = 0
    for name in names:
        directory = PACKET / 'signature-records' / name
        for leaf, digest in census['raw_sha256'][name].items():
            need(sha(directory / leaf) == digest, 'census raw hash')
        receipt = load(directory / 'receipt.json')
        need(sha(directory / 'receipt.json') == records[name]['receipt_sha256'] and
             receipt['pid'] == records[name]['pid'], 'receipt identity')
        need(receipt['exit'] == 0 and receipt['error'] is None and receipt['group_absent'] is True,
             'successful owned closure')
        need(receipt['started_ns'] >= previous and receipt['finished_ns'] >= receipt['started_ns'],
             'ordered monotonic wall records')
        need(receipt['finished_ns'] - receipt['started_ns'] <=
             receipt['timeout_seconds'] * 1_000_000_000, 'command deadline')
        previous = receipt['finished_ns']
        for leaf in ('stdout', 'stderr'):
            need(sha(directory / leaf) == receipt[leaf + '_sha256'], 'receipt output hash')
    commit = 'a26dbebb57f948439a2e813c7a17c1142014ba1e'
    parent = '4ad64047b0887a747a41aa7b0b9fa2f4206279c1'
    with (PACKET / 'signed-source.bundle').open('rb') as stream:
        need(stream.readline() == b'# v2 git bundle\n', 'bundle format')
        header = []
        while True:
            line = stream.readline()
            need(line, 'terminated bundle header')
            if line == b'\n':
                break
            header.append(line.decode('ascii').rstrip('\n'))
    need(len(header) == 2 and header[0].startswith('-' + parent + ' ') and
         header[1] == commit + ' HEAD', 'single exact bundle prerequisite and head')
    summary = load(PACKET / 'summary.json')
    need(summary['source_commit'] == commit and summary['bundle_prerequisite'] == parent,
         'summary source binding')
    need(summary['native_collection']['files'] == 1040 and
         summary['native_collection']['archive_member'] == native_member, 'native collection count')
    for flag in ('originals_deleted', 'self_contained_full_replay', 'new_gpu_execution',
                 'runtime_source_changes', 'performance_acceptance'):
        need(summary[flag] is False, 'bounded claims: ' + flag)
    result = {'accepted': True, 'publication_pins': PINS, 'public_files': len(index),
              'outer_raw_members': len(raw), 'signature_groups_closed': 6,
              'native_archive_unchanged': True, 'native_workload_rerun': False,
              'historical_pid_probe': False, 'originals_deleted': False}
    with OUTPUT.open('x', encoding='ascii') as stream:
        stream.write(json.dumps(result, sort_keys=True, indent=2) + '\n')
    print(json.dumps({'output': str(OUTPUT), 'sha256': sha(OUTPUT), **result}, sort_keys=True))


if __name__ == '__main__':
    main()
