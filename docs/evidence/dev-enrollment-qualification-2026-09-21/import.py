"""Import unchanged CPU/prerequisite receipts; not an offline proof validator."""

import hashlib
import json
from pathlib import Path
import shutil

DESTINATION = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution/docs/evidence/dev-enrollment-qualification-2026-09-21')
WORKTREE = '/home/harsh/.codex-tmp/fe2o3-enrollment-linear-output-20260921'
RUNS = [
    ('cpu', Path('/home/harsh/.codex-tmp/fe2o3-enrollment-linear-qualification-20260921'),
     '2202036a93c2c340536b80f3d2e907b201833789',
     '4610a153b15bc8a1d8c8cbdf308f7a966738f830b64c59a5313431dba3a83ce2',
     {'head-before', 'status-before', 'signature', 'format', 'tests', 'clippy',
      'diff-check', 'head-after', 'status-after'}),
    ('prerequisites', Path('/home/harsh/.codex-tmp/fe2o3-enrollment-prereq-qualification-20260921'),
     '3278dbb91b77d0417b32241de0c5e41190f44ae4',
     '1544428a1f97c1aed4551a309ce76720fc553c59a49988b59a0efe65f99e024e',
     {'head-before', 'status-before', 'signature', 'lint', 'diff-check',
      'enrollment-campaign', 'head-after', 'status-after'}),
]


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, root, source, driver_sha, phases in RUNS:
    need(sha(root / 'qualify.py') == driver_sha, 'original driver')
    finished = json.loads((root / 'finished.json').read_text())
    need(finished['source_commit'] == source and finished['qualified'] is True
         and finished['native_execution'] is False
         and finished['performance_acceptance'] is False, 'completed narrow qualification')
    qualification = root / 'qualification'
    need({path.name for path in qualification.iterdir()} == phases, 'exact receipt phases')
    for phase in phases:
        folder = qualification / phase
        need({path.name for path in folder.iterdir()} == {'receipt.json', 'stdout', 'stderr'},
             'exact command files')
        receipt = json.loads((folder / 'receipt.json').read_text())
        need(type(receipt['exit']) is int and receipt['exit'] == 0
             and receipt['error'] is None and receipt['group_absent'] is True
             and receipt['cwd'] == WORKTREE and receipt['stdin_sha256'] is None,
             'completed owned command')
        for stream in ['stdout', 'stderr']:
            need(sha(folder / stream) == receipt[stream + '_sha256'], 'unmodified raw stream')
    for phase in ['before', 'after']:
        need((qualification / ('head-' + phase) / 'stdout').read_text().strip() == source,
             'source bracket')
        need(not (qualification / ('status-' + phase) / 'stdout').read_bytes(), 'clean bracket')
    if name == 'prerequisites':
        campaign = root / 'campaign'
        need((campaign / 'inputs-before.json').read_bytes()
             == (campaign / 'inputs-after.json').read_bytes(), 'unchanged campaign identities')
        for receipt_path in campaign.glob('**/record.json'):
            receipt = json.loads(receipt_path.read_text())
            need(receipt['group_absent'] is True, 'campaign group absent')
    need(all(not path.is_symlink() for path in root.rglob('*')), 'no source symlinks')

DESTINATION.mkdir()
for name, root, source, driver_sha, phases in RUNS:
    destination = DESTINATION / name
    destination.mkdir()
    for file in ['qualify.py', 'finished.json']:
        shutil.copyfile(root / file, destination / file)
    shutil.copytree(root / 'qualification', destination / 'qualification')
    if name == 'prerequisites':
        shutil.copytree(root / 'campaign', destination / 'campaign')
shutil.copyfile(__file__, DESTINATION / 'import.py')
print(json.dumps({'archive': str(DESTINATION), 'raw_receipts_imported': True,
                  'offline_solver_revalidation': False}))
