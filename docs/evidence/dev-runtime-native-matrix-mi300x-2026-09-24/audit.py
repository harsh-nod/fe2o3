#!/usr/bin/env python3
"""Record replay, parser controls and static checks under an unchanged packet."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import importlib.util
from pathlib import Path
import signal

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("native_matrix_audit", HERE / "verify.py")
V = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(V)
NAMES = ("prepare.py", "protocol.py", "native.py", "campaign.py", "test_protocol.py", "PROTOCOL.md",
         "collect.py", "verify.py", "test_verify.py", "audit.py", "README.md", "artifacts.json")


def inputs():
    return {name: V.sha(HERE / name) for name in NAMES}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir()
    before = inputs()
    b = V.module(HERE / "raw/campaign3/remote/artifacts/base.py",
                 "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7", "native_matrix_audit_recorder")
    b.write_json(args.output / "inputs-before.json", before)
    recorder = b.Recorder(args.output / "commands", V.REPO)
    for number in b.MANAGED:
        signal.signal(number, b.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        for name, seconds in (("verify", 300), ("test_verify", 900), ("test_protocol", 120)):
            recorder.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / (name + ".py"))], seconds, env=env)
        recorder.run("ruff", ["/home/harsh/.local/bin/ruff", "check", "--isolated", "--select", "F",
                              *(str(HERE / name) for name in NAMES if name.endswith(".py"))], 120, env=env)
    finally:
        after = inputs()
        b.write_json(args.output / "inputs-after.json", after)
        V.need(before == after, "unchanged audit inputs")


if __name__ == "__main__":
    main()
