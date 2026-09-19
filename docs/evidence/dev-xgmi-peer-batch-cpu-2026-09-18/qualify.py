#!/usr/bin/env python3
"""Record one non-overwriting CPU qualification of XGMI peer batches."""

import hashlib
import importlib.util
from pathlib import Path
import signal
import sys

if not sys.flags.isolated:
    raise RuntimeError("run qualification with python3 -I")

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PREFIX = "docs/evidence/dev-xgmi-peer-batch-cpu-2026-09-18/"
PAIR_QUALIFY = "docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/qualify.py"
SOURCE = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
FIXED_TOOLS = {
    "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/accept.py": "be5998bbd3bde933c48d4ae3d6e12ae8231da62cffd254abdb7c7ebe3f7dc8bd",
    "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/qualify.py": "43397a7a269a21dd317ba0e0d8137943800441f6e17eacbeee4e3766c684e7a3",
    PAIR_QUALIFY: "06f5d1e8ba5f8e8afee3c7a3dce2373a64b9c1cb9621314613ef2eaa7da38686",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    SOURCE: "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
}


def sha(path):
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("ordinary tool file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, expected in FIXED_TOOLS.items():
    if sha(ROOT / name) != expected:
        raise RuntimeError("unauthenticated helper: " + name)

spec = importlib.util.spec_from_file_location(
    "peer_batch_pair_cpu_commands", ROOT / PAIR_QUALIFY
)
P = importlib.util.module_from_spec(spec)
spec.loader.exec_module(P)
P.Q.KFD_FILTERS += ["sdma::tests::xgmi_batch_wait::"]
P.Q.RUNTIME_FILTERS += ["context::", "kfd_backend::xgmi_batch::"]


def configure(root, here):
    """Bind inherited absolute command paths to the original execution tree."""
    P.Q.HERE = Path(here)
    P.Q.SELECTOR = Path(root) / SOURCE


configure(ROOT, HERE)
SELECTOR = P.Q.SELECTOR


def commands():
    return P.commands()


def tool_manifest():
    result = dict(FIXED_TOOLS)
    for name in ("qualify.py", "verify.py", "test_verify.py"):
        result[PREFIX + name] = sha(HERE / name)
    return result


def main():
    for number in P.Q.N.MANAGED:
        signal.signal(number, P.Q.N.interrupted)
    recorder = P.Q.N.Recorder(HERE / "raw", ROOT)
    P.Q.N.write_json(HERE / "tools.json", tool_manifest())
    for name, command, seconds in commands():
        recorder.run(name, command, seconds)


if __name__ == "__main__":
    main()
