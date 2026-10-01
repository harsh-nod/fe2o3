#!/usr/bin/env python3
"""Copy audited A2 evidence; gzip wraps original tar bytes without rewriting them."""
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys

ROOT = Path('/run/shm/fe2o3-runtime-integration-20261001-99c07511e')
DEST = ROOT / 'docs/evidence/dev-producer-journal-composition-2026-10-01'
AUDIT = Path('/run/shm/fe2o3-a2-concrete-opacity-draft-20261001-audit')
SCANNER = Path('/run/shm/fe2o3-evidence-sensitive-scan-20261001-v2.py')
SCAN = Path('/run/shm/fe2o3-sensitive-a2-composed-custody-scan-20261001.json')
PACKETS = {
    'prefix': ('qualification', {
        'packet.tar': 'cc3135a54ac6fa7271641a8fb9d7052075eba107070c25cee4bd754b4bc43ff1',
        'members.json': '87c0eab6bebfea56289cb338d4f0469d663709a8e42c68e43361411ae0e54a7e',
        'receipt.json': '20c3b2e314ee4e566fe18f7dac53798143f448d9922fdf407685a2be8a8028c7',
    }),
    'completion': ('completion', {
        'packet.tar': '9694c4cefdc4dd4c6267379bfa98ca587f3598e2f9e166156894995c4ea9227b',
        'members.json': 'dc44f452fc0a942c33856182d08cbb4b7093fa6ddf3607eb1b62cbd621989af9',
        'receipt.json': 'e29bab9010f13d64e75381384cfc65f031bd1adaa28e408f32e6ce0ae76f4dc9',
    }),
}
SUPPORT = {
    'audit_rejected_signed_concrete_v1.py': '5635ffe856c94dd01e1f97fe4c43fc52012f8313b078abdf53533d542ff2a998',
    'test_audit_rejected_signed_concrete_v1.py': 'ebd78514beeb5fb57d48934a959d74df1fabb4ce4585e5c1c3d9fa6ee8758a3a',
    'rejected-signed-concrete-post-audit-v1.json': 'c5b3bda8d16123440f8a960a6fd8df25222be1d9fa9f616381349515e315ee1d',
    'complete_signed_concrete_v1.py': '606c17a081fb9b6ae0449aa66c07f72b9c7b900e45f006c495e0cbd33d826716',
    'test_complete_signed_concrete_v1.py': '00fa4dffabcdfe733c08b51957badfa884897e729d2e9c85825485fe828f8fe4',
    'prepared-signed-concrete-completion-v1.json': '7246c4c282c2f6502e23f35122b56b77ffd8f05a0bb35e0e323b0e70a7a16519',
    'audit_concrete_completion_v1.py': 'd2dc6596ab60561808f2ce9bc82b12019c8551c7a90437a1f1c549e50c53606f',
    'test_audit_concrete_completion_v1.py': '23fffdb9e5528d37a983b93069152fd0587f36ee43edb2914e5100637980f199',
    'concrete-completion-post-audit-v1.json': 'b120471f71930fecff00108532d9d5f1919639cd17b1b32f1037f9289f8ccec4',
}


def raw(path):
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or path.resolve() != path:
        raise ValueError('ordinary original file required')
    return path.read_bytes()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def need(condition, message):
    if not condition:
        raise ValueError(message)


def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize,
         'python3 -I -B required')
    need(shutil.disk_usage(ROOT).free > 4 * 1024**3, 'publication disk floor')
    need(digest(raw(SCANNER)) == 'd67d508215343edce6b34185927c9efb93dda5c0ab68f84a82be75538f35f99f',
         'reviewed scanner')
    scan = json.loads(raw(SCAN))
    need(scan['accepted'] and not scan['hits'] and not scan['errors'], 'accepted archive scan')
    need(scan['scanner_sha256'] == digest(raw(SCANNER)), 'scanner report identity')
    audit = json.loads(raw(AUDIT / 'concrete-completion-post-audit-v1.json'))
    need(audit['composed_readback_passed'] and not audit['original_campaign_qualified'], 'composed scope')
    need(audit['prefix_stages'] == 109 and audit['fresh_closing_stages'] == 3
         and audit['qualified_unique_prefix_negative_cases'] == 89
         and audit['positive_counts'] == {'leaf': [42]*3, 'conditional': [64]*3, 'concrete': [214]*3},
         'audited qualification roster')
    spec = importlib.util.spec_from_file_location('publication_scan', SCANNER)
    scanner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(scanner)
    needles = scanner.needles()
    need(bool(needles), 'sensitive environment control')

    def clean(data):
        need(not any(value in data for value in needles), 'known sensitive value')
        for match in scanner.JSON_FIELD.finditer(data):
            value = json.loads(match[2])
            need(len(value) < 12 or any(word in value.lower() for word in
                 ('redacted', 'placeholder', 'dummy', 'not-a-real', '${')), 'credential-shaped JSON field')

    # Retain exact verified buffers through writes, avoiding a hash/copy race.
    entries = {}
    for label, (kind, pins) in PACKETS.items():
        directory = Path(f'/mnt/c/fe2o3-a2-signed-concrete-{kind}-20261001-attempt-1')
        need(str(directory) in scan['scope'], 'archive scan scope')
        for name, expected in pins.items():
            path = directory / name
            data = raw(path)
            need(digest(data) == expected, 'durable original pin')
            clean(data)
            output = f'{label}-{name}' + ('.gz' if name == 'packet.tar' else '')
            entries[output] = (path, data, name == 'packet.tar')
    for name, expected in SUPPORT.items():
        path = AUDIT / name
        data = raw(path)
        need(digest(data) == expected, 'auditor/support pin')
        clean(data)
        entries['support/' + name] = (path, data, False)
    bundle = Path('/run/shm/fe2o3-a2-composed-signed-source-20261001.bundle')
    data = raw(bundle)
    need(digest(data) == 'd70ebb7934df63d123b62e76dc4fb7d22d3bc09ba7afce935054a0f3761ab736', 'signed bundle')
    entries['signed-source.bundle'] = (bundle, data, False)
    for name, path in (('sensitive-scan.py', SCANNER), ('sensitive-scan.json', SCAN),
                       ('package.py', Path(__file__).resolve())):
        data = raw(path)
        clean(data)
        entries['support/' + name] = (path, data, False)
    need(not DEST.exists(), 'fresh publication destination')
    DEST.mkdir(parents=True)
    (DEST / 'support').mkdir()
    manifest = {}
    for name, (original, data, compress) in entries.items():
        output = DEST / name
        with output.open('xb') as stream:
            if compress:
                with gzip.GzipFile(fileobj=stream, mode='wb', filename='', mtime=0) as compressed:
                    compressed.write(data)
            else:
                stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        published = raw(output)
        need((gzip.decompress(published) if compress else published) == data, 'exact output readback')
        need(raw(original) == data, 'original continuity')
        need(len(published) < 90 * 1024**2, 'Git artifact bound')
        manifest[name] = {'original': str(original), 'original_sha256': digest(data),
            'original_bytes': len(data), 'sha256': digest(published), 'bytes': len(published),
            'lossless_gzip_wrapper': compress}
    value = {'schema': 1, 'signed_candidate': 'fef0f92cb6385c57d1b77c2eff41db967490beb1',
        'bundle_requires_parent': '6793b910c2aa662df20763fbbd349cec0994677f',
        'composed_qualification': True, 'original_campaign_qualified': False,
        'prefix_stages': 109, 'fresh_stages': 3, 'unique_negative_cases': 89,
        'originals_unchanged': True, 'files': manifest,
        'replay_self_contained': False, 'scanner_decodes_git_pack': False}
    encoded = (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()
    clean(encoded)
    with (DEST / 'manifest.json').open('xb') as stream:
        stream.write(encoded)
        stream.flush()
        os.fsync(stream.fileno())
    need(raw(DEST / 'manifest.json') == encoded, 'manifest readback')
    for directory in (DEST / 'support', DEST, DEST.parent):
        fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(fd)
        finally:
            os.close(fd)
    print(json.dumps({'published': str(DEST), 'files': len(manifest),
        'manifest_sha256': digest(encoded), 'originals_unchanged': True}), flush=True)


if __name__ == '__main__':
    main()
