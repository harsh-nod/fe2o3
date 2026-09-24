#!/usr/bin/env python3
"""Record protocol/cleanup calibration or retained native evidence replay."""

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
REPO = HERE.parents[2]
HELPER = REPO / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
raw = HELPER.read_bytes()
if not stat.S_ISREG(HELPER.lstat().st_mode) or hashlib.sha256(raw).hexdigest() != \
        "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7":
    raise RuntimeError("authenticated recorder")
B = ModuleType("producer_native_audit_recorder")
B.__file__ = str(HELPER)
exec(compile(raw, str(HELPER), "exec"), B.__dict__)


def inputs():
    return {name: B.sha(HERE / name) for name in (
        "prepare.py", "protocol.py", "native.py", "campaign.py", "test_protocol.py", "PROTOCOL.md",
        "verify.py", "test_verify.py", "collect.py", "audit.py", "README.md")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--preflight", action="store_true")
    args = parser.parse_args()
    output = args.output
    B.need(output.resolve() == output and output.parent == HERE
           and re.fullmatch(r"audit[1-9][0-9]*", output.name), "fresh audit output")
    output.mkdir()
    before = inputs()
    B.write_json(output / "inputs-before.json", before)
    recorder = B.Recorder(output / "commands", REPO)
    for number in B.MANAGED:
        signal.signal(number, B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        commands = [("protocol", "test_protocol.py", [], 18), ("cleanup", "test_verify.py", ["CleanupTests"], 5)] \
            if args.preflight else [("replay", "verify.py", [], None), ("rejections", "test_verify.py", [], 16)]
        for name, script, arguments, count in commands:
            folder = recorder.run(name, ["/usr/bin/python3", "-I", "-B", str(HERE / script), *arguments], 900, env=env)
            if count is not None:
                text = (folder / "stderr").read_text()
                B.need(re.findall(r"^Ran (\d+) tests? in [0-9.]+s$", text, re.MULTILINE) == [str(count)]
                       and text.endswith("\nOK\n") and "skipped" not in text, "exact passing audit test groups")
        B.write_json(output / "result.json", {"mode": "preflight" if args.preflight else "retained-replay", "result": "pass"})
    finally:
        after = inputs()
        B.write_json(output / "inputs-after.json", after)
        B.need(before == after, "unchanged audit controls")


if __name__ == "__main__":
    main()
