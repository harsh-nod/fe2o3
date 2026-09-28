#!/usr/bin/env python3
"""Pinned owned-process/endpoint runner with six cases and per-case deadlines."""
import hashlib
from pathlib import Path
import sys
from types import ModuleType

if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
    raise RuntimeError("use python3 -I -B")
HERE = Path(__file__).resolve().parent
raw = (HERE / "native-base.py").read_bytes()
if hashlib.sha256(raw).hexdigest() != "19cfa66a24ecf30d028673ca4d1110cf36f2b51b1bf47734c4c9365e687709a4":
    raise RuntimeError("pinned native process runner")
old = b"p.command(root, case), TEST_SECONDS, p.stamp(before[\"finished\"])"
if raw.count(old) != 1:
    raise RuntimeError("unique per-case recorder deadline site")
raw = raw.replace(old, b"p.command(root, case), p.test_seconds(case), p.stamp(before[\"finished\"])")
runner = ModuleType("native_async_runner")
runner.__file__ = str(HERE / "native-base.py")
exec(compile(raw, runner.__file__, "exec"), runner.__dict__)
runner.CASE_START = 0
runner.EXPECTED_CASES = 6
runner.REMOTE_SECONDS = 4200
runner.PAYLOAD |= {"native-base.py", "protocol-base.py", "oracle.py", "profile.py", "scale.py"}

if __name__ == "__main__":
    runner.main()
