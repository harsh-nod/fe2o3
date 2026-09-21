"""Import completed receipts or validate archived evidence without scratch paths.

Offline validation needs a Git repository containing SOURCE, but neither the
original worktree nor the original Verus installation. It does not rerun Verus.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
LOCAL = Path(__file__).resolve().parent
CAMPAIGN = Path('/home/harsh/.codex-tmp/fe2o3-producer-custody-verus-20260921-a2')
ARCHIVE = ROOT / 'docs/evidence/dev-producer-read-custody-verus-2026-09-21'
SOURCE = '237f02c6997eff73ee4d0a17d77c2b9019376715'
PROOF = Path('crates/fe2o3-runtime-model/verus')
VERUS = Path('/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus')
CLOSURE = Path('examples/row_softmax_v1/verify-verus-closure.sh')
QUALIFY_SHA = '995ec68a97c06ed2ed9582a6f3fcf9e8736349b5b7a18f1deb440f39fe37e708'
PINNED = {
    PROOF / 'context_producer_read_invariant_v1.rs': 'CONTEXT_PRODUCER_READ_INVARIANT_SHA256',
    PROOF / 'context_read_invariant_v1.rs': 'CONTEXT_READ_INVARIANT_SHA256',
    PROOF / 'context_read_commit_v1.rs': 'CONTEXT_READ_COMMIT_SHA256',
    PROOF / 'context_read_preflight_v1.rs': 'CONTEXT_READ_PREFLIGHT_SHA256',
    PROOF / 'context_version_journal_issuance_v1.rs': 'CONTEXT_VERSION_JOURNAL_ISSUANCE_SHA256',
    PROOF / 'check-producer-read-invariant.py': 'PRODUCER_READ_INVARIANT_CHECKER_SHA256',
    PROOF / 'check-read-invariant.py': 'READ_INVARIANT_CHECKER_SHA256',
    PROOF / 'check-read-commit.py': 'READ_COMMIT_CHECKER_SHA256',
    PROOF / 'check-read-preflight.py': 'READ_PREFLIGHT_CHECKER_SHA256',
    PROOF / 'check-journal-issuance.py': 'JOURNAL_ISSUANCE_CHECKER_SHA256',
    Path('examples/wave64_collectives_v1/check-proof-source.py'): 'PROOF_SOURCE_CHECKER_SHA256',
    PROOF / 'pins/VERUS_CLOSURE_MANIFEST': 'VERUS_CLOSURE_MANIFEST_SHA256',
}
FINISHED = {'source_commit': SOURCE, 'qualified': True,
            'conditional_producer_custody': True, 'wrapper_lifecycle_refinement': False,
            'context_integration': False, 'native_execution': False,
            'performance_acceptance': False}
ENVIRONMENT = {
    'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
    'LANG': 'C', 'LC_ALL': 'C', 'RUSTUP_HOME': '/home/harsh/.rustup',
    'CARGO_HOME': '/home/harsh/.cargo', 'VERUS': str(VERUS), 'VERUS_TIMEOUT_SECONDS': '180',
}
COMMANDS = [
    ('head-before', ['git', 'rev-parse', 'HEAD'], 30),
    ('status-before', ['git', 'status', '--porcelain'], 30),
    ('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30),
    ('syntax', ['bash', '-n', str(PROOF / 'verify-verus.sh')], 30),
    ('lint', ['/home/harsh/.local/bin/ruff', 'check', str(PROOF / 'check-producer-read-invariant.py'), str(PROOF / 'check-negative-quality.py')], 60),
    ('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30),
    ('global-verus', ['bash', str(PROOF / 'verify-verus.sh')], 14400),
    ('head-after', ['git', 'rev-parse', 'HEAD'], 30),
    ('status-after', ['git', 'status', '--porcelain'], 30),
]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    need(path.is_file() and not path.is_symlink(), f'regular file: {path}')
    return path.read_bytes()


def entries(path):
    need(path.is_dir() and not path.is_symlink(), f'regular directory: {path}')
    return {child.name for child in path.iterdir()}


def unique_json(path):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result

    def invalid(value):
        raise ValueError(f'invalid JSON constant: {value}')

    return json.loads(read(path), object_pairs_hook=pairs, parse_constant=invalid)


def signed_sources(repo, temporary):
    files = {*PINNED, *(PROOF / 'pins' / pin for pin in PINNED.values()),
             PROOF / 'pins/VERUS_SHA256', PROOF / 'pins/TRANSCRIPT_SHA256', CLOSURE}
    contents = {}
    for name in files:
        data = subprocess.check_output(['git', 'cat-file', 'blob', f'{SOURCE}:{name}'], cwd=repo)
        target = temporary / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        contents[name] = data
    for name, pin in PINNED.items():
        need(sha(contents[name]) == contents[PROOF / 'pins' / pin].decode().strip(), 'signed source pin')
    checker = temporary / PROOF / 'check-producer-read-invariant.py'
    spec = importlib.util.spec_from_file_location('archive_producer_checker', checker)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    exec(compile(contents[PROOF / checker.name], str(checker), 'exec'), module.__dict__)
    return contents, module


def expected_inputs(contents):
    keys = {*PINNED, *(PROOF / 'pins' / pin for pin in PINNED.values()),
            PROOF / 'pins/VERUS_SHA256', CLOSURE}
    result = {str(ROOT / name): sha(contents[name]) for name in keys}
    manifest = contents[PROOF / 'pins/VERUS_CLOSURE_MANIFEST'].decode('ascii')
    for tool in ['verus', 'rust_verify', 'z3']:
        rows = [line.split('|') for line in manifest.splitlines() if line.startswith(f'required={tool}|')]
        need(len(rows) == 1 and len(rows[0]) == 4, 'unique manifest tool')
        result[str(VERUS.parent / tool)] = rows[0][3]
    need(result[str(VERUS)] == contents[PROOF / 'pins/VERUS_SHA256'].decode().strip(), 'launcher pin')
    return result


def check_times(record):
    start, finish = record['started_ns'], record['finished_ns']
    # These legacy receipts use wall time, which stepped backwards between cases.
    need(type(start) is int and type(finish) is int and 0 < start <= finish, 'receipt times')


def solver_record(folder, command, status):
    need(entries(folder) == {'record.json', 'stdout.log', 'stderr.log'}, 'exact solver files')
    record = unique_json(folder / 'record.json')
    need(set(record) == {'command', 'started_ns', 'process_group', 'group_absent', 'finished_ns', 'status'}, 'solver receipt fields')
    need(record['command'] == command and type(record['status']) is int
         and record['status'] == status and record['group_absent'] is True
         and type(record['process_group']) is int and record['process_group'] > 0, 'owned solver completion')
    check_times(record)
    return record


def validate_campaign(campaign, module, contents, temporary):
    inputs = unique_json(campaign / 'inputs.json')
    need(inputs == expected_inputs(contents), 'exact campaign identity roster')
    result = unique_json(campaign / 'finished.json')
    need(result == {'qualified': True, 'obligations': 180, 'inherited': 155,
                    'test_obligations': 1, 'invariant_mutations': 12, 'inputs': inputs}, 'exact dedicated result')
    source = contents[PROOF / 'context_producer_read_invariant_v1.rs'].decode('ascii') + module.SUBJECT
    cases = [('positive_before', None), *[(item.name, item) for item in module.mutations()], ('positive_after', None)]
    need(entries(campaign) == {'inputs.json', 'finished.json', 'closure-before', 'closure-after', *(name for name, _ in cases)}, 'exact campaign entries')
    closure_command = ['/bin/sh', str(ROOT / CLOSURE), str(VERUS.parent), str(ROOT / PROOF / 'pins/VERUS_CLOSURE_MANIFEST')]
    rows = []
    for phase in ['before', 'after']:
        folder = campaign / f'closure-{phase}'
        solver_record(folder, closure_command, 0)
        need(read(folder / 'stdout.log') == b'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n'
             and not read(folder / 'stderr.log'), 'exact closure output')
        if phase == 'after':
            break
        for name, mutation in cases:
            folder = campaign / name
            original_folder = CAMPAIGN / name
            candidate = original_folder / 'producer.rs'
            expected = module.BASE.mutate(source, mutation) if mutation else source
            generated = {'producer.rs': expected.encode('ascii'),
                         **{dependency: contents[PROOF / dependency] for dependency in module.DEPENDENCIES}}
            audit = temporary / 'audit' / name
            audit.mkdir(parents=True)
            for filename, data in generated.items():
                (audit / filename).write_bytes(data)
            module.audit_source(expected, audit)
            for part in ['producer', 'invariant', 'commit', 'preflight']:
                filename = f'audited-{part}-body.rs'
                generated[filename] = read(audit / filename)
            sources = {str(original_folder / filename): sha(data) for filename, data in generated.items()}
            need(unique_json(folder / 'sources.json') == sources, 'exact generated identity roster')
            need(entries(folder) == {*generated, 'sources.json', 'solver'}, 'exact case files')
            need(all(read(folder / filename) == data for filename, data in generated.items()), 'regenerated solver input closure')
            command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180',
                       str(VERUS), '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating',
                       '--output-json', '--error-format=json', '--num-threads', '4', str(candidate)]
            record = solver_record(folder / 'solver', command, 1 if mutation else 0)
            module.PREFLIGHT.check_result(record['status'], read(folder / 'solver/stdout.log').decode(),
                                          read(folder / 'solver/stderr.log').decode(), expected, candidate, mutation)
            rows.append({'case': name, 'exit': record['status'], 'verified': 180 if mutation else 181,
                         'errors': 1 if mutation else 0,
                         'seconds': (record['finished_ns'] - record['started_ns']) / 1e9})
    return rows


def validate_qualification(local, contents):
    finished = unique_json(local / 'finished.json')
    need(finished == FINISHED and all(type(finished[key]) is type(value) for key, value in FINISHED.items()), 'complete signed-source qualification')
    need(sha(read(local / 'qualify.py')) == QUALIFY_SHA, 'original qualification driver')
    qualification = local / 'qualification'
    need(entries(qualification) == {name for name, _, _ in COMMANDS}, 'exact qualification command roster')
    for name, command, timeout in COMMANDS:
        folder = qualification / name
        need(entries(folder) == {'receipt.json', 'stdout', 'stderr'}, 'exact command files')
        record = unique_json(folder / 'receipt.json')
        need(set(record) == {'command', 'cwd', 'environment', 'error', 'exit', 'finished_ns', 'group_absent',
                             'pid', 'started_ns', 'stderr_sha256', 'stdin_sha256', 'stdout_sha256', 'timeout_seconds'}, 'receipt fields')
        need(record['command'] == command and record['cwd'] == str(ROOT)
             and record['environment'] == ENVIRONMENT and record['stdin_sha256'] is None
             and type(record['timeout_seconds']) is int and record['timeout_seconds'] == timeout
             and type(record['pid']) is int and record['pid'] > 0
             and type(record['exit']) is int and record['exit'] == 0
             and record['error'] is None and record['group_absent'] is True, 'qualification command completion')
        check_times(record)
        for stream in ['stdout', 'stderr']:
            need(sha(read(folder / stream)) == record[stream + '_sha256'], 'raw stream checksum')
    for phase in ['before', 'after']:
        need(read(qualification / f'head-{phase}/stdout') == (SOURCE + '\n').encode(), 'recorded source HEAD')
        need(not read(qualification / f'status-{phase}/stdout'), 'recorded clean tree')
    need(not read(qualification / 'signature/stdout')
         and read(qualification / 'signature/stderr') == b'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n', 'recorded source signing identity')
    transcript = read(qualification / 'global-verus/stdout').splitlines()[-1] + b'\n'
    need(sha(transcript) == contents[PROOF / 'pins/TRANSCRIPT_SHA256'].decode().strip(), 'complete registered transcript')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-archive', type=Path)
    parser.add_argument('--repo', type=Path, default=ROOT)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='fe2o3-producer-evidence-') as owned:
        temporary = Path(owned)
        contents, module = signed_sources(args.repo, temporary / 'source')
        if args.verify_archive:
            local, campaign = args.verify_archive, args.verify_archive / 'producer-campaign'
        else:
            need(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == SOURCE, 'source HEAD')
            need(not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT), 'clean source')
            local, campaign = LOCAL, CAMPAIGN
            need(all(sha(read(Path(path))) == digest for path, digest in expected_inputs(contents).items()), 'current source/tool identities')
        validate_qualification(local, contents)
        rows = validate_campaign(campaign, module, contents, temporary)
        if args.verify_archive:
            need(unique_json(local / 'campaign-summary.json') == rows, 'derived campaign summary')
        else:
            ARCHIVE.mkdir()
            for name in ['qualify.py', 'archive.py', 'archive-selftest.py', 'finished.json']:
                shutil.copyfile(LOCAL / name, ARCHIVE / name)
            shutil.copytree(LOCAL / 'qualification', ARCHIVE / 'qualification')
            shutil.copytree(CAMPAIGN, ARCHIVE / 'producer-campaign')
            (ARCHIVE / 'campaign-summary.json').write_text(json.dumps(rows, indent=2) + '\n')
        print(json.dumps({'archive': str(args.verify_archive or ARCHIVE), 'source_commit': SOURCE,
                          'qualification_commands': len(COMMANDS), 'dedicated_cases': len(rows), 'validated': True}))


if __name__ == '__main__':
    main()
