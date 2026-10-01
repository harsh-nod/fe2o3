#!/usr/bin/env python3
"""Portable offline diagnostic replay only; no subprocesses or fresh proof."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tarfile

PINS = {
    'final.tar.gz': '7681239d4852c20aa8a5bc59dc871a36e1302c743a72abe325eb2f7b38ce3a95',
    'manifest.json': '95c3466304665d9aa8819edc718f98e0f47845b28e3e6e0f154c8f3a28101a22',
    'result.json': 'e3721c26d9eee0812cae2019d47cc8e8ba02363520b07bc76e01503878bb5f5b',
    'helpers/diagnostics.py': 'aa4ab2d4626837317a9a52cecfdf8f91655ceff670a7c23882135bf3a6fe2ee8',
    'helpers/mutations.py': 'c111410b8fa3fff49dc60b40d276575326ec9ff99fb1f0ec782724ba5308c9c2',
    'helpers/producer-input-diagnostics-v1.py': '318a29f9067618c2168a8e1d3585b1e8e3f8c78abe40359d52b0a20458238be4',
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def module(root, name):
    path = root / name
    need(path.resolve() == path and path.is_file() and not path.is_symlink(), 'ordinary bundled helper')
    need(sha(path.read_bytes()) == PINS[name], 'exact bundled helper')
    spec = importlib.util.spec_from_file_location('recorded_' + path.stem.replace('-', '_'), path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def read_packet(root, parse):
    for name, expected in PINS.items():
        path = root / name
        need(path.resolve() == path and path.is_file() and not path.is_symlink(), 'ordinary packet input')
        need(sha(path.read_bytes()) == expected, 'exact published packet input: ' + name)
    manifest = parse((root / 'manifest.json').read_text())
    need(manifest['archive_sha256'] == PINS['final.tar.gz'] and len(manifest['files']) == 190,
         'exact complete archived roster')
    files = {}
    with tarfile.open(root / 'final.tar.gz', 'r:gz') as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not member.issym() and not member.islnk() and name not in files,
                 'unique ordinary archive member')
            need(name in manifest['files'] and not name.startswith('/') and
                 '\\' not in name and all(part not in ('', '.', '..') for part in name.split('/')),
                 'closed relative archive path')
            need(0 <= member.size <= 4 * 1024**2, 'bounded recorded member')
            data = archive.extractfile(member).read()
            need({'bytes': len(data), 'mode': member.mode, 'sha256': sha(data)} == manifest['files'][name],
                 'exact archive byte/mode/hash identity')
            files[name] = data
    need(set(files) == set(manifest['files']) and sum(map(len, files.values())) <= 4 * 1024**2,
         'complete bounded packet, without filesystem extraction')
    return files


def check(root):
    root = root.resolve()
    diag = module(root, 'helpers/diagnostics.py')
    parser = module(root, 'helpers/producer-input-diagnostics-v1.py')
    mutations = module(root, 'helpers/mutations.py')
    parse = lambda text: diag.parse(parser, text)
    files = read_packet(root, parse)
    prepared = parse(files['owner/prepared/record.json'].decode())
    original = {path: files['owner/prepared/case/' + str(path)] for path in mutations.PINS}
    mutations.checked_sources(original)
    projections = {'case': original, 'relocated': original}
    negatives, equivalents = {}, {}
    for case in (*mutations.CASES, *mutations.EQUIVALENTS):
        row, projection = mutations.project(original, case)
        projections[row['name']] = projection
        (equivalents if row['result_equivalent_control'] else negatives)[row['name']] = row
    need(prepared['plan']['negative_candidates'] == negatives and
         prepared['plan']['equivalent_controls'] == equivalents, 'exact reviewed mutation roster')
    need(set(prepared['projected_inputs']) == set(projections), 'all twenty two-input projections')
    for case, projection in projections.items():
        for path, expected in projection.items():
            actual = files['owner/prepared/' + case + '/' + str(path)]
            need(actual == expected and sha(actual) == prepared['projected_inputs'][case][str(path)],
                 'actual complete source projection')
    for name, case in negatives.items():
        prefix = 'owner/capture-attempt-1/commands/' + name + '/'
        receipt = parse(files[prefix + 'receipt.json'].decode())
        proof = Path(receipt['command'][-1])
        need(proof.is_absolute() and tuple(proof.parts[-len(mutations.PROOF.parts):]) == mutations.PROOF.parts,
             'original diagnostic proof-root spelling')
        original_root = proof.parents[len(mutations.PROOF.parts) - 1]
        result = diag.negative(root / 'helpers/producer-input-diagnostics-v1.py',
            root / 'helpers/mutations.py', prepared['plan']['verifier_identity'], original,
            projections[name], original_root, case, receipt['exit'],
            files[prefix + 'stdout'].decode(), files[prefix + 'stderr'].decode())
        recorded = parse(files['owner/capture-attempt-1/' + name + '-result.json'].decode())
        need(recorded['observation']['calibration'] == result, 'same recorded strict diagnostic classification')
    return {'recorded_negative_diagnostics_valid': True, 'negative_cases': len(negatives),
            'equivalent_source_controls': len(equivalents), 'two_input_projections': len(projections),
            'archive_members': len(files), 'fresh_proof_run': False, 'process_custody_replayed': False,
            'cpu_qualification_replayed': False, 'std_adapter_proved': False, 'performance_acceptance': False}


if __name__ == '__main__':
    print(json.dumps(check(Path(__file__).resolve().parent), sort_keys=True, indent=2))
