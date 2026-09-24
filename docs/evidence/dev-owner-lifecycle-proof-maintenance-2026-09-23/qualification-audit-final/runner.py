#!/usr/bin/env python3
"""Capture the bounded lifecycle packet audit and unchanged input bracket."""
import hashlib
import importlib.util
import json
import signal
import sys
from pathlib import Path

sys.dont_write_bytecode = True
REPO = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
PACKET = REPO / 'docs/evidence/dev-owner-lifecycle-2026-09-23'
OUTPUT = Path(__file__).parent / 'audit-2'
CHECK = REPO / 'crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    OUTPUT.mkdir()
    spec = importlib.util.spec_from_file_location('lifecycle_audit_owner', CHECK)
    check = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = check
    spec.loader.exec_module(check)
    base = check.dependencies(REPO)['base']
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)

    def identity():
        return {
            'commit': check.git_output(REPO, 'rev-parse', 'HEAD', text=True).strip(),
            'runner': sha(Path(__file__)),
            'python': sha(Path(sys.executable).resolve()),
            'packet': {str(path.relative_to(PACKET)): sha(path)
                       for path in sorted(PACKET.rglob('*')) if path.is_file()},
        }

    before = identity()
    check.need(len(before['packet']) == 194, 'exact manifested lifecycle packet')
    (OUTPUT / 'inputs-before.json').write_text(json.dumps(before, indent=2) + '\n')
    command = [sys.executable, '-I', '-B', str(PACKET / 'audit.py'), '--repo', str(REPO), '--selftest']
    env = {**check.git_environment(), 'HOME': '/home/harsh', 'TMPDIR': str(OUTPUT)}
    try:
        status, stdout, stderr = base.run_owned(command, 600, OUTPUT / 'command', env)
    finally:
        after = identity()
        (OUTPUT / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
        check.need(before == after, 'audit packet/runner/Python/HEAD drift')
    check.need(status == 0 and not stderr and stdout ==
               'PASS: 449 Git source identities, 62 terminal commands, 49 scoped lifecycle controls, and CPU receipts\n'
               'PASS: 39 rehashed malformed record sets and 2 source-policy changes rejected; bare Git audit passed\n',
               'exact lifecycle auditor and corruption-control transcript')
    print('PASS: complete lifecycle packet audit; 39 record controls and 2 source controls; input bracket and owned cleanup')


if __name__ == '__main__':
    main()
