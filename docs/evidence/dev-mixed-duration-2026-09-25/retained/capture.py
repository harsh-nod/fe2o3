import hashlib
import os
from pathlib import Path
import signal
import sys
import types

root = Path('/home/harsh/.codex-tmp/fe2o3-r61-execution')
path = root / 'crates/fe2o3-runtime-model/verus/check-journal-issuance.py'
raw = path.read_bytes()
if hashlib.sha256(raw).hexdigest() != '36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480':
    raise ValueError('owned process helper identity')
module = types.ModuleType('mixed_owned')
module.__file__ = str(path)
sys.modules[module.__name__] = module
exec(compile(raw, str(path), 'exec'), module.__dict__)
for number in module.SIGNALS:
    signal.signal(number, module.interrupted)
os.chdir(root)
status, stdout, stderr = module.run_owned(sys.argv[2:], 1200, Path(__file__).resolve().parent / sys.argv[1],
    dict(os.environ, CARGO_TARGET_DIR='/dev/shm/fe2o3-mixed-duration-target-20260925-NohI5JUH',
         CARGO_BUILD_JOBS='4', CARGO_INCREMENTAL='0'))
print(stdout[-8000:])
print(stderr[-8000:])
print('exit=' + str(status))
sys.exit(status)
