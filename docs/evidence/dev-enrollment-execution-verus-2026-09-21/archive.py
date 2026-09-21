"""Import completed enrollment evidence, or validate it without scratch/tool paths.

Offline validation requires a Git repository containing SOURCE. It reconstructs
the pinned source/checker closure and rechecks receipts; it does not rerun Verus.
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
ORIGINAL = Path('/home/harsh/.codex-tmp/fe2o3-enrollment-execution-20260921-a')
CAMPAIGN = ORIGINAL / 'campaign'
ARCHIVE = ROOT / 'docs/evidence/dev-enrollment-execution-verus-2026-09-21'
SOURCE = '97ea2f97a2473bb8ebc1944a8efff18866d3b72a'
PROOF = Path('crates/fe2o3-runtime-model/verus')
VERUS = Path('/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus')
LEGACY = Path('docs/evidence/dev-producer-journal-issuance-verus-2026-09-21/archive.py')
LEGACY_SHA = 'a2af11fb15824a9f341a6992bcb77599a95ead5355a592acdca538082cd2a297'
QUALIFY_SHA = 'bc5ccbd17241751951a6e2ff2d1f98b8d2a96b2c50dece4cb9178bb0461412c0'
CLOSURE = Path('examples/row_softmax_v1/verify-verus-closure.sh')
FINISHED = {'source_commit': SOURCE, 'qualified': True, 'complete_logical_enrollment': True,
            'producer_custody_and_issuance_preserved': True, 'constructor_enroll_register_witness': True,
            'constructor_to_pending_reachability': False, 'rust_sorting_search_refined': False,
            'production_integration': False, 'native_execution': False, 'performance_acceptance': False}
ENVIRONMENT = {'HOME': '/home/harsh', 'USER': 'harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
               'LANG': 'C', 'LC_ALL': 'C', 'RUSTUP_HOME': '/home/harsh/.rustup',
               'CARGO_HOME': '/home/harsh/.cargo', 'VERUS': str(VERUS)}
COMMANDS = [
    ('head-before', ['git', 'rev-parse', 'HEAD'], 30),
    ('status-before', ['git', 'status', '--porcelain'], 30),
    ('signature', ['git', '-c', 'gpg.ssh.allowedSignersFile=/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers', 'verify-commit', SOURCE], 30),
    ('diff-check', ['git', 'diff', '--check', SOURCE + '^', SOURCE], 30),
    ('lint', ['/home/harsh/.local/bin/ruff', 'check', '--no-cache', str(PROOF / 'check-journal-enrollment.py')], 60),
    ('self-test', ['python3', '-I', '-B', str(PROOF / 'check-journal-enrollment.py'), '--self-test'], 120),
    ('campaign', ['python3', '-I', '-B', str(PROOF / 'check-journal-enrollment.py'), '--verus', str(VERUS),
                  '--timeout', '180', '--output', str(CAMPAIGN)], 7200),
    ('head-after', ['git', 'rev-parse', 'HEAD'], 30),
    ('status-after', ['git', 'status', '--porcelain'], 30),
]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(name, path, data):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def git_blob(repo, path):
    return subprocess.check_output(['git', 'cat-file', 'blob', f'{SOURCE}:{path}'], cwd=repo)


def signed_sources(repo, temporary):
    helper_data = git_blob(repo, LEGACY)
    need(sha(helper_data) == LEGACY_SHA, 'pinned receipt helpers')
    helper = load('enrollment_archive_helpers', temporary / 'helpers.py', helper_data)
    pinned = {**helper.PINNED,
              PROOF / 'context_version_journal_enrollment_v1.rs': 'CONTEXT_VERSION_JOURNAL_ENROLLMENT_SHA256',
              PROOF / 'check-journal-enrollment.py': 'JOURNAL_ENROLLMENT_CHECKER_SHA256'}
    files = {*pinned, *(PROOF / 'pins' / pin for pin in pinned.values()), PROOF / 'pins/VERUS_SHA256', CLOSURE}
    contents = {}
    for path in files:
        data = git_blob(repo, path)
        target = temporary / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        contents[path] = data
    for path, pin in pinned.items():
        need(sha(contents[path]) == contents[PROOF / 'pins' / pin].decode().strip(), 'signed source pin')
    checker = temporary / PROOF / 'check-journal-enrollment.py'
    module = load('archive_enrollment_checker', checker, contents[PROOF / checker.name])
    module.archive_helpers = helper
    return contents, module


def expected_inputs(contents):
    result = {str(ROOT / path): sha(data) for path, data in contents.items()}
    manifest = contents[PROOF / 'pins/VERUS_CLOSURE_MANIFEST'].decode('ascii')
    for tool in ('verus', 'rust_verify', 'z3'):
        rows = [line.split('|') for line in manifest.splitlines() if line.startswith(f'required={tool}|')]
        need(len(rows) == 1 and len(rows[0]) == 4, 'unique manifest tool')
        result[str(VERUS.parent / tool)] = rows[0][3]
    need(result[str(VERUS)] == contents[PROOF / 'pins/VERUS_SHA256'].decode().strip(), 'launcher pin')
    return result


def validate_campaign(campaign, module, contents, temporary):
    h = module.archive_helpers
    inputs = expected_inputs(contents)
    need(h.unique_json(campaign / 'inputs-before.json') == inputs
         and h.unique_json(campaign / 'inputs-after.json') == inputs, 'exact input identity brackets')
    result = h.unique_json(campaign / 'report.json')
    expected = {'qualified': True, 'scope': 'complete logical enrollment with producer custody and issuance',
                'positive_obligations': 263, 'inherited_obligations': 201, 'negative_cases': 21,
                'full_logical_batch_enrollment_verified': True, 'rust_sorting_search_refined': False,
                'native_or_performance_acceptance': False}
    need(result == expected and all(type(result[key]) is type(value) for key, value in expected.items()), 'exact campaign result')
    environment = {'HOME': '/home/harsh', 'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo',
                   'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin', 'VERUS_Z3_PATH': str(VERUS.parent / 'z3')}
    need(h.unique_json(campaign / 'environment.json') == environment, 'solver environment')
    base = module.inherited().BASE
    mutations = module.mutations(base)
    need(len(mutations) == len({item.name for item in mutations}) == 21, 'exact unique mutations')
    cases = [('positive_before', None), *[(item.name, item) for item in mutations], ('positive_after', None)]
    need(h.entries(campaign) == {'inputs-before.json', 'inputs-after.json', 'environment.json', 'report.json',
                                 'closure-before', 'closure-after', *(name for name, _ in cases)}, 'exact campaign files')
    source = contents[PROOF / 'context_version_journal_enrollment_v1.rs'].decode('ascii')
    rows = []
    for phase in ('before', 'after'):
        folder = campaign / ('closure-' + phase)
        command = ['/bin/sh', str(ROOT / CLOSURE), str(VERUS.parent), str(ROOT / PROOF / 'pins/VERUS_CLOSURE_MANIFEST')]
        h.solver_record(folder, command, 0)
        need(h.read(folder / 'stdout.log') == b'PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n'
             and not h.read(folder / 'stderr.log'), 'exact closure output')
        if phase == 'after':
            break
        for name, mutation in cases:
            folder, original = campaign / name, CAMPAIGN / name
            candidate = original / 'context_version_journal_enrollment_v1.rs'
            current = base.mutate(source, mutation) if mutation else source
            generated = {'context_version_journal_enrollment_v1.rs': current.encode('ascii'),
                         **{name: contents[PROOF / name] for name in module.DEPENDENCIES}}
            audit = temporary / name
            audit.mkdir(parents=True)
            for filename, data in generated.items():
                (audit / filename).write_bytes(data)
            module.audit_source(module.inherited(), current, audit)
            generated = {path.name: h.read(path) for path in audit.glob('*.rs')}
            need(h.unique_json(folder / 'sources.json') == {str(original / name): sha(data) for name, data in generated.items()},
                 'exact regenerated identities')
            need(h.entries(folder) == {*generated, 'sources.json', 'solver'}, 'exact case files')
            need(all(h.read(folder / name) == data for name, data in generated.items()), 'exact regenerated inputs')
            command = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(VERUS),
                       '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
                       '--error-format=json', '--num-threads', '4', str(candidate)]
            record = h.solver_record(folder / 'solver', command, 1 if mutation else 0)
            module.check_result(base, record['status'], h.read(folder / 'solver/stdout.log').decode(),
                                h.read(folder / 'solver/stderr.log').decode(), current, candidate, mutation)
            rows.append({'case': name, 'exit': record['status'], 'verified': 262 if mutation else 263,
                         'errors': 1 if mutation else 0, 'seconds': (record['finished_ns'] - record['started_ns']) / 1e9})
    return rows


def validate_qualification(local, module):
    h = module.archive_helpers
    result = h.unique_json(local / 'finished.json')
    need(result == FINISHED and all(type(result[key]) is type(value) for key, value in FINISHED.items()), 'exact qualification scope')
    need(sha(h.read(local / 'qualify.py')) == QUALIFY_SHA, 'original qualification driver')
    qualification = local / 'qualification'
    need(h.entries(qualification) == {name for name, _, _ in COMMANDS}, 'exact outer command roster')
    for name, command, timeout in COMMANDS:
        folder = qualification / name
        need(h.entries(folder) == {'receipt.json', 'stdout', 'stderr'}, 'exact command files')
        record = h.unique_json(folder / 'receipt.json')
        need(set(record) == {'command', 'cwd', 'environment', 'error', 'exit', 'finished_ns', 'group_absent', 'pid',
                             'started_ns', 'stderr_sha256', 'stdin_sha256', 'stdout_sha256', 'timeout_seconds'}, 'receipt fields')
        need(record['command'] == command and record['cwd'] == str(ROOT) and record['environment'] == ENVIRONMENT
             and record['stdin_sha256'] is None and type(record['timeout_seconds']) is int and record['timeout_seconds'] == timeout
             and type(record['pid']) is int and record['pid'] > 0 and type(record['exit']) is int and record['exit'] == 0
             and record['error'] is None and record['group_absent'] is True, 'owned command completion')
        h.check_times(record)
        for stream in ('stdout', 'stderr'):
            need(sha(h.read(folder / stream)) == record[stream + '_sha256'], 'raw stream checksum')
    for phase in ('before', 'after'):
        need(h.read(qualification / f'head-{phase}/stdout') == (SOURCE + '\n').encode(), 'recorded source HEAD')
        need(not h.read(qualification / f'status-{phase}/stdout'), 'recorded clean tree')
    need(not h.read(qualification / 'signature/stdout')
         and h.read(qualification / 'signature/stderr') == b'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n', 'recorded source signing identity')


def validate_packet(local, module, rows):
    h = module.archive_helpers
    need(h.entries(local) == {'README.md', 'SHA256SUMS', 'qualify.py', 'finished.json', 'archive.py',
                              'archive-selftest.py', 'case-summary.json', 'adverse-results.json',
                              'campaign', 'qualification'}, 'exact published packet roster')
    need(json.dumps(h.unique_json(local / 'case-summary.json'), sort_keys=True) == json.dumps(rows, sort_keys=True),
         'exact regenerated case summary')
    adverse = h.unique_json(local / 'adverse-results.json')
    expected = {'adverse_cases_rejected': 26, 'source_commit': SOURCE, 'passed': True}
    need(adverse == expected and all(type(adverse[key]) is type(value) for key, value in expected.items()),
         'recorded adverse-test result')
    observed = {}
    for path in sorted(local.rglob('*')):
        need(not path.is_symlink(), 'no packet symlinks')
        if path.is_file() and path != local / 'SHA256SUMS':
            observed[path.relative_to(local).as_posix()] = sha(h.read(path))
    sealed = {}
    for line in h.read(local / 'SHA256SUMS').decode('ascii').splitlines():
        digest, name = line.split('  ', 1)
        need(name not in sealed and len(digest) == 64 and all(c in '0123456789abcdef' for c in digest),
             'unique canonical manifest row')
        sealed[name] = digest
    need(sealed == observed, 'complete packet manifest')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=ROOT)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--import-evidence', action='store_true')
    args = parser.parse_args()
    local = args.archive or LOCAL
    with tempfile.TemporaryDirectory(prefix='fe2o3-enrollment-evidence-audit-') as owned:
        temporary = Path(owned)
        contents, module = signed_sources(args.repo, temporary / 'source')
        validate_qualification(local, module)
        rows = validate_campaign(local / 'campaign', module, contents, temporary / 'regenerated')
        if args.archive is not None:
            validate_packet(local, module, rows)
    if args.import_evidence:
        need(args.archive is None and not ARCHIVE.exists(), 'new owned archive only')
        ARCHIVE.mkdir()
        for name in ('qualify.py', 'finished.json', 'archive.py', 'archive-selftest.py'):
            shutil.copyfile(LOCAL / name, ARCHIVE / name)
        shutil.copytree(LOCAL / 'qualification', ARCHIVE / 'qualification')
        shutil.copytree(LOCAL / 'campaign', ARCHIVE / 'campaign')
        (ARCHIVE / 'case-summary.json').write_text(json.dumps(rows, indent=2) + '\n')
    print(json.dumps({'validated': True, 'source_commit': SOURCE, 'whole_crate_cases': len(rows),
                      'native_or_performance_acceptance': False}))


if __name__ == '__main__':
    main()
