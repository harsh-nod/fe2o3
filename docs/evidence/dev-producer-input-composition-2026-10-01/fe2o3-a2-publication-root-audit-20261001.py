#!/usr/bin/env python3
"""Independently compare the public composition archive with retained originals."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import tarfile

BASE = Path('/home/harsh/.codex-tmp')
RECORDS = BASE / 'fe2o3-a2-producer-composition-records-20260930-audit'
PACKET = RECORDS / 'compact-publication-attempt-2/public'
REPO = BASE / 'fe2o3-a2-producer-composition-20260930-qualified'
OUTPUT = BASE / 'fe2o3-a2-publication-root-audit-20261001.json'
COMMITS = ['10013c06a14e876fde141c769b5baf8bc42b3203',
           '6b9e5d87c4386692f7c213a95dd786ccdf42cb45']
COMMON = 'ee849c0fc5bf6d7f7bc21e4044b2a152dbfab1f3'
PINS = {
    'summary.json': '9d4dec0c674c85aa21f36c2baf8421e49fbc53174bd8efc8ee27b0ec2848c61b',
    'campaign.tar.xz': '5dcbe2ad5cc49e4b60cf33acad5a38f97f27904dd9302a38a06c3fadf9bb8455',
    'signed-candidate.bundle': '092cceb5fd39504f124a5bb9175a1048bd46b28da625a6ecc38e1224832ea5b2',
    'publish.py': '6e6c2f70e6e0b9bb227eb92471c36dceb64f247f94880d11d9d8f99b56c59af8',
    'test-publish.py': '4667d87733bdd90f2c7ec631df7f8c2d1d270b655667e2ae9deeb8dd9d269118',
}

def need(value, message):
    if not value:
        raise ValueError(message)

def sha(path):
    need(path.resolve(strict=True) == path and path.is_file() and not path.is_symlink(), 'canonical regular original')
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def load(path):
    return json.loads(path.read_bytes())

def main():
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, 'python3 -I -B required')
    need(not OUTPUT.exists(), 'fresh output')
    for name, digest in PINS.items():
        need(sha(PACKET / name) == digest, 'reviewed public pin: ' + name)
    summary = load(PACKET / 'summary.json')
    expected_files = dict(summary['public_files_before_summary'], **{'summary.json': PINS['summary.json']})
    need({str(p.relative_to(PACKET)): sha(p) for p in PACKET.rglob('*') if p.is_file()} == expected_files,
         'complete exact public roster')
    prepared_path = RECORDS / 'prepared-publication-v2.json'
    need(sha(prepared_path) == 'c86ca3a2751334df5cdb4a1d6b3c86e00a67ab72d32b94301c90e4e23c2698b8', 'reviewed preparation')
    prepared = load(prepared_path)
    entries = load(PACKET / 'archive-index.json')
    need(entries == prepared['archive_entries'] and len(entries) == 1240, 'exact prepared archive roster')
    seen = {}
    with tarfile.open(PACKET / 'campaign.tar.xz', 'r:xz') as archive:
        for member in archive:
            path = PurePosixPath(member.name)
            need(str(path) == member.name and not path.is_absolute() and '..' not in path.parts,
                 'canonical contained archive member')
            need(member.name in entries and member.name not in seen, 'unique known archive member')
            row = entries[member.name]
            original = Path(row['origin'])
            need(sha(original) == row['sha256'] and original.stat().st_size == row['bytes'], 'original content identity')
            mode = 0o755 if original.stat().st_mode & 0o111 else 0o644
            need(member.mode == row['mode'] == mode and member.uid == member.gid == member.mtime == 0
                 and member.uname == member.gname == '' and set(member.pax_headers) <= {'path', 'linkpath'},
                 'normalized exact archive mode and metadata')
            if row['hardlink'] is not None:
                need(member.islnk() and member.size == 0 and member.linkname == row['hardlink']
                     and member.linkname in seen, 'backward internal exact hardlink')
                observed = seen[member.linkname]
            else:
                need(member.isfile() and not member.linkname and member.size == row['bytes'], 'exact ordinary archive entry')
                with archive.extractfile(member) as stream:
                    observed = (hashlib.file_digest(stream, 'sha256').hexdigest(), member.size, member.mode)
            need(observed == (row['sha256'], row['bytes'], row['mode']), 'decoded payload identity')
            seen[member.name] = observed
    need(list(seen) == list(entries), 'ordered complete archive')
    campaign = {name.removeprefix('campaign102/'): row['sha256'] for name, row in entries.items() if name.startswith('campaign102/')}
    digest = hashlib.sha256(json.dumps(campaign, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    need(len(campaign) == 1218 and digest == 'c76eec6b642205a2d70517c96bbea14d8fc0c813fd56c8fbbec02d233260bfdd', 'entire original accepted campaign')
    need(entries['root-readback/result.json']['sha256'] ==
         entries['independent-readback/001-audit/owned/stdout.log']['sha256'] ==
         '0aa2bda66cccacc40c57627a8f0bc09a35904c6b97e7e76cba2bee9e81302d70', 'independent readback agreement')
    bundle_record = load(PACKET / 'source-bundle-verification.json')
    need(bundle_record['commits'] == COMMITS and bundle_record['header'] ==
         {'format': '# v2 git bundle', 'heads': [[COMMITS[-1], 'HEAD']], 'prerequisite': COMMON}, 'bundle record identities')
    previous, groups = 0, []
    for index, (stage, name) in enumerate(zip(bundle_record['stages'], ('create', 'verify', 'heads'), strict=True), 1):
        folder = PACKET.parent / f'{index:02d}-bundle-{name}'
        record = stage['record']
        need(stage['name'] == name and load(folder / 'owned/record.json') == record
             and load(folder / 'command.json')['argv'] == stage['command'] == record['command'], 'original bundle command/receipt join')
        need(record['status'] == 0 and record['group_absent'] is True
             and record['started_ns'] >= previous and record['finished_ns'] >= record['started_ns']
             and record['finished_ns'] - record['started_ns'] <= 130_000_000_000, 'successful bounded closed bundle command')
        for stream in ('stdout', 'stderr'):
            need((folder / f'owned/{stream}.log').read_text() == stage[stream], 'bundle raw stream join')
        groups.append(record['process_group'])
        previous = record['finished_ns']
    need(len(set(groups)) == 3, 'three distinct originally closed groups; no historical PID probes')
    with (PACKET / 'signed-candidate.bundle').open('rb') as stream:
        need(stream.readline() == b'# v2 git bundle\n', 'actual bundle format')
        need(stream.readline().decode().startswith('-' + COMMON + ' '), 'actual public prerequisite')
        need(stream.readline() == (COMMITS[-1] + ' HEAD\n').encode() and stream.readline() == b'\n', 'actual single bundle head')
    git_records = []
    signers = BASE / 'fe2o3-preflight-repeat-20260929-nnNtgZnS/preflight-final/allowed-signers'
    commands = [
        ['-c', 'gpg.ssh.allowedSignersFile=' + str(signers), 'verify-commit', commit] for commit in COMMITS]
    commands.append(['bundle', 'verify', str(PACKET / 'signed-candidate.bundle')])
    for args in commands:
        argv = ['/usr/bin/git', '--no-replace-objects', '-c', 'gc.auto=0', *args]
        result = subprocess.run(argv, cwd=REPO, capture_output=True, timeout=30)
        need(result.returncode == 0, 'fresh read-only Git signature/bundle verification')
        git_records.append({'argv': argv, 'status': result.returncode, 'stdout': result.stdout.decode(), 'stderr': result.stderr.decode()})
    need(summary['accepted'] is True and summary['commits'] == COMMITS and summary['public_prerequisite'] == COMMON
         and summary['campaign_stages'] == 102 and summary['fresh_negative_cases'] == 81
         and summary['self_contained_full_replay'] is False and summary['cpu_elf_included'] is False
         and summary['new_solver_or_cpu_or_gpu_run'] is False, 'qualified limited summary')
    external = load(PACKET / 'external-replay-prerequisites.json')
    need(external['self_contained_full_replay'] is False and external['cpu_elf_included'] is False, 'external replay boundary explicit')
    cpu = external['retained_cpu_artifact']
    need(all(sha(Path(cpu[key])) == cpu['sha256'] for key in ('source', 'retained')), 'both original CPU artifacts retained unchanged')
    result = {'accepted': True, 'publication_pins': PINS, 'archive_members': len(entries),
              'original_campaign_files': len(campaign), 'campaign_tree_sha256': digest,
              'original_bundle_groups_closed': groups, 'fresh_git_checks': git_records,
              'originals_unchanged': True, 'new_solver_cpu_gpu_execution': False,
              'historical_pid_probes': False, 'self_contained_full_replay': False}
    with OUTPUT.open('x', encoding='ascii') as stream:
        stream.write(json.dumps(result, sort_keys=True, indent=2) + '\n')
    print(json.dumps({'output': str(OUTPUT), 'sha256': sha(OUTPUT), 'accepted': True}))

if __name__ == '__main__':
    main()
