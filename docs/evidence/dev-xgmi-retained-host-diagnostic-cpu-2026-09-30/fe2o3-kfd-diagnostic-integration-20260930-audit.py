#!/usr/bin/env python3
"""Check the exact CPU-qualified delta and unchanged executable proof closures."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
BEFORE = 'bff0a1728e6a87119d4cd72025246273464cf29e'
MERGED = '841c06b7355c5110d4641cdeda1c332623de1ed8'
CANDIDATE = 'd6b1906d0d24b366ea4222fbca4ae4a6966fad6c'
CPU = Path('/home/harsh/.codex-tmp/fe2o3-xgmi-retained-host-diagnostic-cpu-20260930-continuation-2')
V = Path('crates/fe2o3-runtime-model/verus')
EXPECTED = {
    'retained-pair-post-catch': ('c41d4d9111ce5418de7725a1b4e63773a064ae4436b84b19e6f120d9cbf1052a', 2, 14, 29),
    'retained-pair-owned-storage': ('c41d4d9111ce5418de7725a1b4e63773a064ae4436b84b19e6f120d9cbf1052a', 3, 8, 21),
    'retained-credit-dispatch': ('e12eb39e143bc32dcdc0e4ad90dc5bc9d2c7f274a734d1f486673909caae6bb4', 14, 41, 25),
    'request-charge': ('6a350a6e04e95cfe0f3694f07f5710ff41e17424df134078fbdc983535f11996', 3, 3, 21),
}


def git(*args):
    return subprocess.check_output(['/usr/bin/git', '--no-replace-objects', '-c', 'gc.auto=0', *args], cwd=ROOT, timeout=60)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    assert sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize
    assert sys.argv[1:] in ([], ['--refreshed'])
    refreshed = bool(sys.argv[1:])
    assert git('rev-parse', 'HEAD').decode().strip() == MERGED
    source_raw = (CPU / 'source-before.json').read_bytes()
    assert sha(source_raw) == '73ef7e432dea941606caa96ab815a91b326dca33dc0a2126a402dba1989af4f7'
    source = json.loads(source_raw)
    paths = [p.decode() for p in git('diff', '--name-only', '-z', BEFORE, MERGED).split(b'\0') if p]
    assert len(paths) == 12
    for name in paths:
        data = (ROOT / name).read_bytes()
        assert sha(data) == source[name]['sha256']
        assert data == git('show', CANDIDATE + ':' + name)
        assert source[name]['mode'] == '100644'
    rows = {}
    for name, (expected, files, verified, mutants) in EXPECTED.items():
        path = ROOT / V / ('check-' + name + '.py')
        spec = importlib.util.spec_from_file_location(name.replace('-', '_'), path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        snapshot = module.snapshot()
        inputs = {p: t for p, t in snapshot.items() if p.is_relative_to(module.SRC)} if name.startswith('retained-pair') else {p: t for p, t in snapshot.items() if p != module.PROOF}
        assert module.tree_hash(inputs) == expected
        assert len(module.FILES) == files and module.EXPECTED_VERIFIED == verified
        closure = {}
        for p in module.FILES:
            data = (ROOT / p).read_bytes()
            assert data == git('show', BEFORE + ':' + str(p))
            closure[str(p)] = sha(data)
        mutations = module.mutations(snapshot if name == 'retained-credit-dispatch' else snapshot[module.BODY])
        assert len(mutations) == mutants
        if refreshed:
            assert module.SOURCE_TREE_SHA == expected
            module.audit(snapshot)
        else:
            assert module.SOURCE_TREE_SHA != expected
            try:
                module.audit(snapshot)
            except ValueError:
                pass
            else:
                raise AssertionError('unrefreshed guard must reject')
        rows[name] = {'implementation_inputs': len(inputs), 'source_tree_sha256': expected,
                      'unchanged_proof_closure': closure, 'verified_count_unchanged': verified,
                      'mutations_unchanged': mutants}
    print(json.dumps({'merged': MERGED, 'candidate': CANDIDATE, 'cpu_paths': paths,
                      'refreshed': refreshed, 'guards': rows, 'new_solver_execution': False}, sort_keys=True))


if __name__ == '__main__':
    main()
