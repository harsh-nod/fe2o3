#!/usr/bin/env python3
"""Read back the exact native36 public packet and retained originals."""
import hashlib
import json
from pathlib import Path
import runpy

BASE = Path('/home/harsh/.codex-tmp')
HELPER = BASE / 'fe2o3-native36-package-20261001.py'
REPORT = BASE / 'fe2o3-native36-publication-root-readback-20261001.json'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def load(path):
    return json.loads(path.read_bytes())


def main():
    assert sha(HELPER) == '7231caf2389d27f5172aae102665bc39ddc34734b9765442956248ffdc57c7fd'
    api = runpy.run_path(str(HELPER), run_name='root_publication_readback')
    extension = api['load_module']('root_packet_transport', api['PREP'] / 'source/benchmarks/runtime_gfx942/xgmi_retained_wait_cadence_transport.py',
                                  'ce88934ea1a52f0a4b91324843a7c1bd5d4255668179b703268cf0fe3f53c644')
    transport, native, hot, ordinary, experiment, planner = extension.helpers()
    historical = api['load_module']('root_packet_census', *api['PINS']['historical-census.py'])
    packet, durable, prep = api['PACKET'], api['DURABLE'], api['PREP']
    result = load(api['OUT'] / 'result.json')
    assert sha(api['OUT'] / 'result.json') == 'f1771c51d1c08e4d74c75074c7ec329cf0526ef1c19e0ee66168d0fb410ec754'
    assert result == load(durable / 'result.json') and result['accepted'] is True and not result['failures']
    receipt = result['durable_packet']
    assert sha(durable / 'final.tar.gz') == receipt['archive_sha256'] == '1aa7dc18d20609fdc65b8fe017cecf251dacdc4220532bac0dd717c5971267f8'
    assert sha(durable / 'final.manifest.json') == receipt['manifest_sha256'] == '40e7f74bd37816fed05f1cd25a71db21b70048ed91947c024c491a7499b767c9'
    files = load(durable / 'final.manifest.json')['files']
    assert len(files) == 42 and transport.inventory(packet) == files
    transport.validate_archive(durable / 'final.tar.gz', files)
    assert load(packet / 'publication-index.json') == {name: value for name, value in files.items() if name != 'publication-index.json'}
    for name, row in load(packet / 'loose-input-index.json').items():
        assert transport.inventory_row(Path(row['original_path'])) == row['original']
        assert transport.inventory_row(packet / name) == row['published']
        assert row['original']['sha256'] == row['published']['sha256']
    assert all(sha(path) == digest == sha(packet / name) for name, (path, digest) in api['PINS'].items())
    selected = {}
    for name, row in load(packet / 'raw-index.json').items():
        original = Path(row['original_path'])
        assert original == prep / name
        expected = {key: value for key, value in row.items() if key != 'original_path'}
        assert transport.inventory_row(original) == expected
        selected[name] = expected
    assert len(selected) == 89
    transport.validate_archive(packet / 'raw.tar.gz', selected)
    collection = load(prep / 'remote-commands/collect/stdout')
    transport.validate_archive(prep / 'remote-commands/pull/stdout', collection['files'])
    assert transport.inventory(prep / 'readback') == collection['files']
    census = historical.census(packet / 'signature-records', load(packet / 'signature-census.json'))
    assert census['count'] == 7
    assert load(api['OUT'] / 'fresh-census.json') == load(packet / 'signature-census.json')
    assert sha(packet / 'signed-source.bundle') == '5c183cbf013c987b48e1c54e8fabe085ec2f9ddf6aba8704afc41d72ce1ef5e5'
    assert (packet / 'signature-records/bundle-heads/stdout').read_text() == api['COMMIT'] + ' ' + api['REF'] + '\n'
    report = {'public_packet_readback_pass': True, 'packet_files': 42, 'raw_files': 89,
        'nested_native_files': len(collection['files']), 'closed_package_groups': 7,
        'controller_sha256': sha(HELPER), 'durable_archive_sha256': sha(durable / 'final.tar.gz'),
        'durable_manifest_sha256': sha(durable / 'final.manifest.json'),
        'signed_bundle_sha256': sha(packet / 'signed-source.bundle'), 'all_originals_preserved': True,
        'full_replay_self_contained': False, 'new_gpu_execution': False, 'historical_pid_probes': False}
    with REPORT.open('x') as stream:
        json.dump(report, stream, sort_keys=True, indent=2)
        stream.write('\n')
    print(json.dumps({'accepted': True, 'report': str(REPORT), 'sha256': sha(REPORT)}))


if __name__ == '__main__':
    main()
