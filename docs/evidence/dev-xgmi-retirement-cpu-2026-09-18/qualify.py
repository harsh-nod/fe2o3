#!/usr/bin/env python3
"""Non-overwriting CPU qualification of XGMI retirement and shared SDMA release."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run qualification with python3 -I")

import hashlib
import importlib.util
from pathlib import Path
import signal

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
PRIOR = "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/"
HELPERS = {
    PRIOR
    + "qualify.py": "43397a7a269a21dd317ba0e0d8137943800441f6e17eacbeee4e3766c684e7a3",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py": "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
}
for name, expected in HELPERS.items():
    path = ROOT / name
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise RuntimeError("unauthenticated helper: " + name)
spec = importlib.util.spec_from_file_location(
    "retirement_cpu_commands", ROOT / (PRIOR + "qualify.py")
)
Q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(Q)
Q.HERE = HERE
Q.ENV = [
    "RUST_TEST_THREADS=2" if item == "RUST_TEST_THREADS=1" else item for item in Q.ENV
]
Q.KFD_FILTERS += ["sdma::xgmi_retirement::tests::", "sdma_cases::"]
SELECTOR = Q.SELECTOR


def commands():
    return Q.commands()


def main():
    for number in Q.N.MANAGED:
        signal.signal(number, Q.N.interrupted)
    recorder = Q.N.Recorder(HERE / "raw", ROOT)
    tools = dict(HELPERS)
    tools[str((HERE / "qualify.py").relative_to(ROOT))] = Q.N.sha(HERE / "qualify.py")
    Q.N.write_json(HERE / "tools.json", tools)
    for name, command, seconds in commands():
        recorder.run(name, command, seconds)


if __name__ == "__main__":
    main()
