"""Serialize restored construction, full-source and focused audit gates."""
import subprocess

root = '/home/harsh/.codex-tmp'
commands = [
    ['python3', '-B', root + '/r103-run.py', 'r103-restored-construction',
        'cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd',
        '--all-features', '--lib', 'queue::live::construction'],
    ['python3', '-B', root + '/r103-source-gate.py', 'r103-final'],
    ['python3', '-B', root + '/r103-auxiliary-gates.py'],
]
for command in commands:
    subprocess.run(command, cwd=root + '/fe2o3-r61-execution', check=True)
