#!/usr/bin/env python3
"""Offline Git-bound typed retained-admission proof and CPU receipt audit."""
import argparse
import hashlib
import importlib.util
from pathlib import Path
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
PACKET = Path('docs/evidence/dev-journal-retained-execution-2026-09-21')
PARENT = Path('docs/evidence/dev-journal-representation-2026-09-21/audit.py')
PARENT_SHA = 'f7d7528ec7a8ac23f8493864d156a49a9f8597b5f20e625919ab5344d590474f'
ORIGINAL = Path('/home/harsh/.codex-tmp/fe2o3-journal-retained-20260921.Z225N2pV/proof-final')
STATIC = {'README.md', 'audit.py', 'check.py', 'selftest.py', 'cargo-checks.py'}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(name, path, data):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def blob(repo, source, path):
    return subprocess.check_output(['git', 'cat-file', 'blob', f'{source}:{path}'], cwd=repo)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--packet', type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    repo, packet = args.repo.resolve(), args.packet.resolve()
    source = (packet / 'SOURCE').read_text().strip()
    need(re.fullmatch('[0-9a-f]{40}', source), 'source commit')
    data = blob(repo, source, PARENT)
    need(sha(data) == PARENT_SHA, 'pinned representation auditor')
    representation = load('typed_admission_representation_auditor', PARENT, data)
    # Reuse its parameterized proof checker with this packet's recorded path identity.
    representation.ORIGINAL = ORIGINAL
    data = blob(repo, source, representation.PARENT)
    need(sha(data) == representation.PARENT_SHA, 'pinned raw Begin auditor')
    old = load('typed_admission_begin_auditor', representation.PARENT, data)
    old.manifest(packet)
    need({p.name for p in packet.iterdir()} == STATIC | {'SOURCE', 'SHA256SUMS', 'proof', 'cargo'}, 'packet root roster')
    need(all((packet / name).read_bytes() == blob(repo, source, PACKET / name) for name in STATIC), 'static packet source binding')
    with tempfile.TemporaryDirectory(prefix='fe2o3-typed-admission-audit-') as scratch:
        temporary = Path(scratch)
        data = blob(repo, source, old.PARENT)
        need(sha(data) == old.PARENT_SHA, 'pinned enrollment archive helpers')
        parent = load('typed_admission_enrollment_parent', temporary / 'parent.py', data)
        contents, inherited_model = parent.signed_sources(repo, temporary)
        check_path = PACKET / 'check.py'
        checker = load('typed_admission_checked_source', temporary / check_path, blob(repo, source, check_path))
        extra = {}
        def materialize(path):
            extra[path] = blob(repo, source, path)
            target = temporary / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(extra[path])
        for path in [check_path, checker.PRIOR,
                     Path('docs/evidence/dev-shared-settlement-commit-2026-09-21/check.py'),
                     Path('docs/evidence/dev-shared-settlement-scratch-2026-09-21/check.py'),
                     Path('docs/evidence/dev-shared-retained-2026-09-21/check.py'),
                     Path('docs/evidence/dev-shared-settlement-storage-2026-09-21/check.py'),
                     Path('docs/evidence/dev-settlement-commit-2026-09-21/check.py'),
                     Path('docs/evidence/dev-settlement-admission-2026-09-21/check.py'),
                     Path('docs/evidence/dev-begin-custody-2026-09-21/check.py'),
                     Path('docs/evidence/dev-begin-execution-2026-09-21/check.py')]:
            materialize(path)
        model = checker.load(temporary)
        for path in checker.extra_paths(model):
            materialize(path)
        helper = inherited_model.archive_helpers
        representation.proof_checks(packet, old, parent, helper, model, checker, contents, extra, temporary)
        commands = {'compiler': ['rustc', '-Vv'],
            'format': ['cargo', 'fmt', '--check', '-p', 'fe2o3-runtime-model'],
            'test': ['cargo', 'test', '--locked', '-p', 'fe2o3-runtime-model'],
            'clippy': ['cargo', 'clippy', '--locked', '-p', 'fe2o3-runtime-model', '--all-targets', '--', '-D', 'warnings']}
        need(helper.entries(packet / 'cargo') == {*commands, 'inputs-before.json', 'inputs-after.json'}, 'Cargo command roster')
        for name, command in commands.items():
            old.command_receipt(helper, packet / 'cargo' / name, command, old.ROOT, 300)
        summaries = [line for line in helper.read(packet / 'cargo/test/stdout').decode().splitlines() if line.startswith('test result:')]
        patterns = [r'test result: ok\. 855 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s',
                    r'test result: ok\. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s']
        need(len(summaries) == len(patterns) and all(re.fullmatch(p, s) for p, s in zip(patterns, summaries)), 'exact terminal test summaries')
        cargo_inputs = helper.unique_json(packet / 'cargo/inputs-before.json')
        need(cargo_inputs == helper.unique_json(packet / 'cargo/inputs-after.json'), 'Cargo source brackets')
        source_names = subprocess.check_output(['git', 'ls-tree', '-r', '--name-only', source,
            'crates/fe2o3-runtime-model/src'], cwd=repo, text=True).splitlines()
        paths = [*source_names, 'Cargo.toml', 'Cargo.lock', 'crates/fe2o3-runtime-model/Cargo.toml',
            'crates/fe2o3-runtime/src/context.rs', str(PACKET / 'cargo-checks.py'),
            'docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py',
            'crates/fe2o3-runtime-model/verus/context_journal_representation_v1.rs',
            'crates/fe2o3-runtime-model/verus/context_journal_retained_execution_v1.rs',
            'crates/fe2o3-runtime-model/verus/context_journal_retained_bodies_v1.rs']
        need(set(cargo_inputs) == {str(old.ROOT / p) for p in paths}, 'exact Cargo input roster')
        need(all(Path(p).is_relative_to(old.ROOT) and h == sha(blob(repo, source, Path(p).relative_to(old.ROOT)))
                 for p, h in cargo_inputs.items()), 'Cargo input Git identities')
    print('PASS: actual typed retained admission, historical correspondence, scoped controls and CPU receipts')


if __name__ == '__main__':
    main()
