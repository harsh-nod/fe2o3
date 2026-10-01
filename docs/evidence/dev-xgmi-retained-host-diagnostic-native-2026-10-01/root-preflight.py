#!/usr/bin/env python3
"""Read back the exact native payload before one bounded remote campaign."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

BASE = Path('/home/harsh/.codex-tmp')
PREP = BASE / 'fe2o3-xgmi-retained-host-diagnostic-native24-20260930-attempt-1'
REPO = BASE / 'fe2o3-xgmi-retained-host-diagnostic-native-20260930'
COMMIT = 'a26dbebb57f948439a2e813c7a17c1142014ba1e'
OUT = BASE / 'fe2o3-native24-preflight-20261001.json'
V = 'benchmarks/runtime_gfx942/'
STAGES = ['source-head', 'signature-before', 'selected-tree', 'filter-capability', 'init',
          'filtered-fetch', 'initial-objects', 'detached-head', 'sparse-enable',
          'sparse-noncone', 'selected-checkout', 'populated-objects',
          'signature-relocated', 'relocated-tree']


def sha(path):
    assert path.resolve() == path and path.is_file() and not path.is_symlink()
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    assert not OUT.exists()
    assert sha(PREP / 'binding.json') == 'b4d89265c8861e3bcf39517e4c96cd2a7885e0753f688051451082eb0bf38e82'
    binding = json.loads((PREP / 'binding.json').read_bytes())
    assert binding['commit'] == COMMIT and len(binding['selected_objects']) == 6482
    assert len(binding['files']) == 6497
    assert binding['devices'] == [
        dict(physical_index=1, unique_id='ab83d2ffef0d3cdf', pci_bdf='0000:26:00.0'),
        dict(physical_index=2, unique_id='d2e26fef80cf5c33', pci_bdf='0000:46:00.0')]
    marker = json.loads((PREP / 'owner.json').read_bytes())
    assert marker == dict(commit=COMMIT, binding_sha256=sha(PREP / 'binding.json'),
                         path='/home/harsh/fe2o3-xgmi-series-20260930.0b26c1f6eb613ac2')
    assert sha(PREP / 'source.tar.gz') == binding['payload_sha256'] == '105053772f7c8b0081ff3975c879e5ad9fbcbb0d5e8b9dff591c82ef60fd7922'
    assert (PREP / 'source.tar.gz').stat().st_size == binding['payload_bytes'] == 33171355
    source = PREP / 'source'
    for name, oid in binding['selected_objects'].items():
        path = source / name
        data = path.read_bytes()
        assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == oid
        assert sha(path) == binding['files']['source/' + name]['sha256']
        assert data == (REPO / name).read_bytes()
    helper = source / V / 'xgmi_peer_series_transport.py'
    spec = importlib.util.spec_from_file_location('native24_preflight_transport', helper)
    transport = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(transport)
    assert transport.read_binding(PREP, marker) == binding
    transport.verify_source(PREP, binding)
    transport.validate_archive(PREP / 'source.tar.gz', binding['files'])
    signature = subprocess.run(['/usr/bin/git', '--no-replace-objects', '-c',
        'gpg.ssh.allowedSignersFile=' + str(PREP / 'allowed-signers'), 'verify-commit', COMMIT],
        cwd=source, check=True, capture_output=True, timeout=60)
    assert signature.stdout == b'' and signature.stderr == (
        b'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n')
    census = json.loads((PREP / 'prepare-census.json').read_bytes())
    assert census['attempt_order'] == STAGES and len(census['records']) == 14
    assert {row['name'] for row in census['records']} == set(STAGES)
    assert len({row['pid'] for row in census['records']}) == 14
    for name in STAGES:
        directory = PREP / 'commands' / name
        for filename, digest in census['raw_sha256'][name].items():
            assert sha(directory / filename) == digest
        receipt = json.loads((directory / 'receipt.json').read_bytes())
        assert type(receipt['exit']) is int and receipt['exit'] == 0
        assert receipt['error'] is None and receipt['group_absent'] is True
        assert receipt['stdout_sha256'] == sha(directory / 'stdout')
        assert receipt['stderr_sha256'] == sha(directory / 'stderr')
        entry = next(row for row in census['records'] if row['name'] == name)
        assert entry['pid'] == receipt['pid'] and entry['receipt_sha256'] == sha(directory / 'receipt.json')
    transport.verify_source(PREP, binding)
    result = dict(commit=COMMIT, marker=marker, binding_sha256=sha(PREP / 'binding.json'),
                  payload_sha256=sha(PREP / 'source.tar.gz'), signed_source_files=6482,
                  archive_files=6497, source_archive_and_signature_match=True,
                  preparation_stages=14, historical_pid_probes=False,
                  remote_execution=False, cpu_or_solver_execution=False,
                  auditor_sha256=sha(Path(__file__).resolve()))
    with OUT.open('x') as handle:
        json.dump(result, handle, indent=2, sort_keys=True)
        handle.write('\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
