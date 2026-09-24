#!/usr/bin/env python3
"""Record final packet replay and rejection tests against unchanged inputs."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
from pathlib import Path
import re
import signal
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
VERIFY = HERE / "verify.py"
if not stat.S_ISREG(VERIFY.lstat().st_mode):
    raise RuntimeError("ordinary verifier required")
RAW = VERIFY.read_bytes()
if hashlib.sha256(RAW).hexdigest() != "8aacaff8fb1e8880dfa74067e8bf9d5f010add1c74039db09ae34e57d3951f4f":
    raise RuntimeError("verifier identity mismatch")
V = ModuleType("producer_launch_audit_verify")
V.__file__ = str(VERIFY)
exec(compile(RAW, str(VERIFY), "exec"), V.__dict__)
R = V.R


def inputs():
    return {name: R.sha(HERE / name) for name in (
        "runner.py", "collect.py", "verify.py", "test_verify.py", "audit.py", "recover.py", "README.md", "artifacts.json")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.absolute()
    V.need(output.parent == HERE and output.resolve() == output
           and re.fullmatch(r"audit[1-9][0-9]*", output.name) is not None, "exact new audit directory")
    before = inputs()
    output.mkdir()
    c = R.helpers()
    c.B.write_json(output / "inputs-before.json", before)
    recorder = c.B.Recorder(output / "commands", R.REPO)
    for number in c.B.MANAGED:
        signal.signal(number, c.B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}

    def finish():
        after = inputs()
        c.B.write_json(output / "inputs-after.json", after)
        V.need(before == after, "unchanged packet audit inputs")

    try:
        for name in ("verify", "test_verify"):
            recorder.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / (name + ".py"))], 900, env=env)
    except BaseException as primary:
        try:
            finish()
        except BaseException as secondary:
            raise primary from secondary
        raise
    finish()


if __name__ == "__main__":
    main()
