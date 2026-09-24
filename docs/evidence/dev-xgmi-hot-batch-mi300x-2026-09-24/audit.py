#!/usr/bin/env python3
"""Capture bounded completed-packet replay, mutations and cleanup controls."""

import argparse
import hashlib
import importlib.util
from pathlib import Path
import signal

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("hot_native_audit", ROOT / "campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)


def inputs():
    names = (*C.PROTOCOL, "README.md", "package.py", "test_package.py", "verify.py", "verify_recovery.py",
             "recover.py", "test_verify.py", "audit.py", "artifacts.json")
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in names}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    before = inputs()
    C.B.write_json(args.output / "inputs-before.json", before)
    rec = C.B.Recorder(args.output / "commands", C.REPO)
    for number in C.B.MANAGED:
        signal.signal(number, C.B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        for name in ("verify_recovery", "test_verify", "test_package"):
            rec.run(name, ["/usr/bin/python3", "-I", "-B", str(ROOT / (name + ".py"))], 900, env=env)
    finally:
        after = inputs()
        C.B.write_json(args.output / "inputs-after.json", after)
        C.H.need(before == after, "unchanged packet audit inputs")


if __name__ == "__main__":
    main()
