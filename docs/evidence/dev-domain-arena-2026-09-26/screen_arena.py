#!/usr/bin/env python3
"""Development-only arena mutation screening; not authenticated qualification."""
import importlib.util
from pathlib import Path
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError('use python3 -I -B')
repo = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('checker',
    repo / 'crates/fe2o3-runtime-model/verus/check-resource-domain.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)
checker.install_handlers()
output, verus = map(lambda value: Path(value).absolute(), sys.argv[1:])
checker.need(not output.exists(), 'fresh development output')
output.mkdir()
captured = {path: checker.ordinary(repo / path) for path in checker.SOURCES}
home = str(Path.home())
env = {'PATH': home + '/.cargo/bin:/usr/bin:/bin', 'HOME': home,
    'RUSTUP_HOME': home + '/.rustup', 'CARGO_HOME': home + '/.cargo',
    'LC_ALL': 'C', 'VERUS_Z3_PATH': str(verus.parent / 'z3')}
cases = [('positive-before', None)] + [
    (name, (path, data)) for name, path, data in checker.mutations(captured)
    if path == checker.ARENA] + [('positive-after', None)]
for name, changed in cases:
    root = checker.stage(output, name, captured, changed)
    command = [str(verus), '--crate-type', 'lib', '--no-cheating', '--num-threads', '2',
        '--triggers-mode', 'silent', str(root / checker.ARENA_ROOT)]
    code, log = checker.execute(command, root, env, output, name)
    checker.classify(code, log, negative=changed is not None, obligations=21)
    print(name + ': classified', flush=True)
print('DEVELOPMENT_ONLY_ARENA_SCREEN_OK', flush=True)
