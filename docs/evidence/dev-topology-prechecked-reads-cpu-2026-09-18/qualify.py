#!/usr/bin/env python3
"""Non-overwriting CPU qualification of prechecked topology property reads."""

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
PRIOR = "docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/qualify.py"
PRIOR_SHA = "06f5d1e8ba5f8e8afee3c7a3dce2373a64b9c1cb9621314613ef2eaa7da38686"
path = ROOT / PRIOR
if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != PRIOR_SHA:
    raise RuntimeError("unauthenticated qualification recipe")
spec = importlib.util.spec_from_file_location("prechecked_cpu_recipe", path)
R = importlib.util.module_from_spec(spec)
spec.loader.exec_module(R)
Q = R.Q
Q.HERE = HERE
SELECTOR = Q.SELECTOR
HELPERS = {**R.HELPERS, PRIOR: PRIOR_SHA}


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
