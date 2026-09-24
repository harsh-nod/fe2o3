#!/usr/bin/env python3
"""Bracket the completed CPU packet's replay and negative tests."""

import argparse
import importlib.util
from pathlib import Path
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("directory_cpu_audit", HERE / "runner.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
C = R.helpers()


def inputs():
    return {name: R.sha(HERE / name) for name in (
        "runner.py", "collect.py", "verify.py", "test_verify.py", "audit.py", "README.md", "artifacts.json")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    before = inputs()
    C.B.write_json(args.output / "inputs-before.json", before)
    recorder = C.B.Recorder(args.output / "commands", R.REPO)
    for number in C.B.MANAGED:
        signal.signal(number, C.B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        for name in ("verify", "test_verify"):
            recorder.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / (name + ".py"))], 900, env=env)
    finally:
        after = inputs()
        C.B.write_json(args.output / "inputs-after.json", after)
        C.H.need(before == after, "unchanged CPU audit inputs")


if __name__ == "__main__":
    main()
