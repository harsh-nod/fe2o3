#!/usr/bin/env python3
"""Replay the CPU packet and optionally reclaim only its owned Cargo cache."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import os
from pathlib import Path
import re
import signal
from types import ModuleType

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "qualify.py"
raw = SCRIPT.read_bytes()
if SCRIPT.is_symlink() or hashlib.sha256(raw).hexdigest() != \
        "665ba3ace41859dbe02e36bfa89fe5a331c52ecad00869de9f66c72b8cbf453e":
    raise RuntimeError("authenticated verifier")
M = ModuleType("native_producer_cpu_audit")
M.__file__ = str(SCRIPT)
exec(compile(raw, str(SCRIPT), "exec"), M.__dict__)
FAILED = {
    "cpu1": ("clippy", -15, "RuntimeError: interrupted by signal 15",
             "3e9a96aaa399096f0d50b269fce17a8e5aa3383e04b160787051d73c51553831"),
    "cpu2": ("clippy", 101, None, "e8ee9a78718610e1766e0cdcb18d18f44bac001d11a015a071a4c405e76a3904"),
    "cpu3": ("gnu-default", -15, "RuntimeError: interrupted by signal 15",
             "2fa40f43b5d0f8318f0c95b4d58bfb7d31e0c3970f2be627e59089a831b8483d"),
}


def inputs():
    return {"qualification": M.Q.inputs(), "packet": {name: M.R.sha(HERE / name) for name in
            ("qualify.py", "test_qualify.py", "audit.py", "README.md")}}


def closed_commands(root, names, helper):
    M.V.inventory(root)
    M.need({path.name for path in root.iterdir()} == set(names), "exact terminal command roster")
    for name in names:
        folder = root / name
        M.need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "terminal artifacts")
        row = M.V.read(folder / "receipt.json")
        M.need(set(row) == M.V.FIELDS and type(row["exit"]) is int and row["group_absent"] is True
               and type(row["pid"]) is int and row["pid"] > 0 and not helper.group_exists(row["pid"]),
               "reaped group remains absent")
        for stream in ("stdout", "stderr"):
            M.need(M.R.sha(folder / stream) == row[stream + "_sha256"], "closed output identity")


def failed_attempts(helper):
    for attempt, (last, status, error, digest) in FAILED.items():
        root = HERE / "raw" / attempt
        M.need({path.name for path in root.iterdir()} ==
               {"commands", "inputs-before.json", "inputs-after.json", "qualify.py"}, "exact failed attempt artifacts")
        M.need(M.V.read(root / "inputs-before.json") == M.V.read(root / "inputs-after.json"), "failed source bracket")
        inputs = M.V.read(root / "inputs-before.json")
        source = "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/producer_launch.rs"
        M.need(M.R.sha(HERE / "raw" / (attempt + "-producer_launch.rs")) == inputs["source"][source], "retained rejected source")
        M.need(M.R.sha(root / "qualify.py") == inputs["runner"], "retained failed runner")
        stages = []
        for name, _, _ in M.Q.stages():
            stages.append(name)
            if name == last:
                break
        closed_commands(root / "commands", stages, helper)
        previous = 0
        for name, command, bound in M.Q.stages():
            row = M.V.read(root / "commands" / name / "receipt.json")
            M.need(row["command"] == command and row["cwd"] == str(M.REPO)
                   and row["environment"] == M.Q.environment() and row["stdin_sha256"] is None
                   and type(row["timeout_seconds"]) is int and row["timeout_seconds"] == bound,
                   "exact failed-prefix command controls")
            M.need(type(row["started_ns"]) is int and type(row["finished_ns"]) is int
                   and previous <= row["started_ns"] < row["finished_ns"]
                   and row["finished_ns"] - row["started_ns"] <= (bound + 15) * 10**9,
                   "ordered bounded failed-prefix commands")
            previous = row["finished_ns"]
            if name == last:
                break
            M.need(row["exit"] == 0 and row["error"] is None, "successful prefix before recorded failure")
        receipt = root / "commands" / last / "receipt.json"
        row = M.V.read(receipt)
        M.need(M.R.sha(receipt) == digest and row["exit"] == status and row["error"] == error, "original unsuccessful disposition")


def cleanup_roster():
    M.V.inventory(HERE / "raw")
    M.need({path.name for path in (HERE / "raw").iterdir()} ==
           {"cpu1", "cpu2", "cpu3", "cpu4", "cpu1-producer_launch.rs", "cpu2-producer_launch.rs", "cpu3-producer_launch.rs"},
           "exact attempts before cache deletion")
    M.need({path.name for path in M.PRIVATE.iterdir()} == {"target"}, "sole owned CPU cache")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cleanup", action="store_true")
    args = parser.parse_args()
    output = args.output
    M.need(output.resolve() == output and output.parent == HERE
           and re.fullmatch(r"audit[1-9][0-9]*", output.name), "fresh audit path")
    output.mkdir()
    helper = M.R.helpers().B
    for number in helper.MANAGED:
        signal.signal(number, helper.interrupted)
    before = inputs()
    helper.write_json(output / "inputs-before.json", before)
    recorder = helper.Recorder(output / "commands", M.REPO)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
    try:
        recorder.run("replay", ["/usr/bin/python3", "-I", "-B", str(SCRIPT), "--output",
                               str(HERE / "raw/cpu4"), "--verify"], 900, env=env)
        recorder.run("rejections", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_qualify.py")], 900, env=env)
        failed_attempts(helper)
        closed_commands(HERE / "raw/cpu4/commands", [name for name, _, _ in M.Q.stages()], helper)
        closed_commands(output / "commands", ["replay", "rejections"], helper)
        if args.cleanup:
            cleanup_roster()
            target = M.PRIVATE / "target"
            for path in (M.PRIVATE, target):
                M.need(path.is_dir() and path.resolve() == path and not path.is_symlink()
                       and path.stat().st_uid == os.getuid(), "exact owned cache")
            collector = M.Q.authenticated_module(M.REPO / "docs/evidence/dev-producer-aware-launch-cpu-2026-09-24/collect.py",
                "7d1c9ef12a62da30de7c2a43d823a77e0977ccc60a52df40f267ec42f61023e9", "native_producer_cache_cleanup")
            allocated = collector.cache_bytes(target)
            M.need(inputs() == before, "source and controls unchanged before cleanup")
            cleanup_roster()
            collector.remove_cache(target, HERE / "raw/cleanup-before.json", HERE / "raw/cleanup-after.json",
                                   allocated, helper.write_json)
        helper.write_json(output / "result.json", {"replay": "pass", "failed_attempts_retained": list(FAILED),
                                                    "cleanup": args.cleanup})
    finally:
        after = inputs()
        helper.write_json(output / "inputs-after.json", after)
        M.need(before == after, "unchanged audit inputs")


if __name__ == "__main__":
    main()
