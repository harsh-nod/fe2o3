#!/usr/bin/env python3
"""Record replay/rejection tests and optionally remove only this packet's cache."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import os
from pathlib import Path
import re
import signal
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "qualify.py"
RAW = SCRIPT.read_bytes()
if not stat.S_ISREG(SCRIPT.lstat().st_mode) or hashlib.sha256(RAW).hexdigest() != \
        "4637727913136890e823e6f9a475077fbef216da4502d5899fcf569e927543b8":
    raise RuntimeError("authenticated qualification helper required")
Q = ModuleType("active_producer_final_qualification")
Q.__file__ = str(SCRIPT)
exec(compile(RAW, str(SCRIPT), "exec"), Q.__dict__)

CPU1_RECEIPTS = {
    "cargo": "d27c86cdd0963f9603caf02630ef52a07568c532b8611a5859e4ee82c842508c",
    "clippy": "d2c3c0b6a81c4a9cb3e9d4a55237509ad87df65d5417f60c473d40b1862641d8",
    "format": "1868e8e364e7d63a52c83070aaf687ef394af30e155e9122b036669b07251840",
    "gnu-default": "529945e584f24dcd3fd9c55483df832c2706b7366eba65caaa0b087a17edd930",
    "gnu-focused": "59349bb091c2ef26adf09e10ceb0b1cac17c2215c6f7a7ad372ab86eb0b30f0b",
    "rustc": "ae0a2605616fa2545a83c1a0ad6ede971795b8c870ebc0f499f0dfc1c9041cbe",
}
REPRO_RECEIPTS = {"active-serial": "3270a9ddb4432bf1278cc1b40a4c5b81c903e375834e92493bb38e20ec11d552"}


def ordinary_tree(root):
    Q.need(root.is_dir() and not root.is_symlink() and root.resolve() == root, "ordinary evidence root")
    Q.V.inventory(root)


def terminal_receipts(root, names, pins=None):
    ordinary_tree(root)
    Q.need({p.name for p in root.iterdir()} == set(names), "exact cleanup command roster")
    rows = []
    for name in sorted(names):
        folder = root / name
        Q.need({p.name for p in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "complete cleanup command")
        receipt = folder / "receipt.json"
        if pins is not None:
            Q.need(Q.R.sha(receipt) == pins[name], "pinned original command receipt")
        row = Q.V.read(receipt)
        Q.need(set(row) == Q.V.FIELDS and type(row["pid"]) is int and row["pid"] > 0
               and type(row["exit"]) is int and row["group_absent"] is True, "terminal cleanup command")
        if pins is None:
            Q.need(row["exit"] == 0 and row["error"] is None, "successful cleanup prerequisite")
        for stream in ("stdout", "stderr"):
            Q.need(Q.R.sha(folder / stream) == row[stream + "_sha256"], "cleanup command output identity")
        rows.append(row)
    return rows


def inputs():
    return {"qualification": Q.inputs(), "packet": {name: Q.R.sha(HERE / name) for name in (
        "qualify.py", "test_qualify.py", "finalize.py", "README.md")}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cleanup", action="store_true")
    args = parser.parse_args()
    output = args.output.absolute()
    Q.need(output.parent == HERE and output.resolve() == output
           and re.fullmatch(r"audit[1-9][0-9]*", output.name) is not None, "exact new audit directory")
    before = inputs()
    Q.need(before["packet"]["test_qualify.py"] ==
           "bb07eca042ef404d436e00a10eb12b5597c3fe50fc126677b8aeba95b5377cc6", "reviewed rejection tests")
    output.mkdir()
    c = Q.R.helpers()
    c.B.write_json(output / "inputs-before.json", before)
    recorder = c.B.Recorder(output / "commands", Q.REPO)
    for number in c.B.MANAGED:
        signal.signal(number, c.B.interrupted)
    env = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}

    def finish():
        after = inputs()
        c.B.write_json(output / "inputs-after.json", after)
        Q.need(before == after, "unchanged audit inputs")

    try:
        recorder.run("replay", ["/usr/bin/python3", "-I", "-B", str(SCRIPT), "--output",
                     str(HERE / "raw/cpu2"), "--verify"], 900, env=env)
        recorder.run("rejections", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_qualify.py")], 900, env=env)
        if args.cleanup:
            C = Q.authenticated_module(Q.BASE / "collect.py",
                "7d1c9ef12a62da30de7c2a43d823a77e0977ccc60a52df40f267ec42f61023e9", "active_producer_cleanup")
            target = Q.PRIVATE / "target"
            for path in (Q.PRIVATE, target):
                Q.need(path.is_dir() and path.resolve() == path and not path.is_symlink()
                       and path.stat().st_uid == os.getuid(), "exact owned cache directory")
            raw = HERE / "raw"
            ordinary_tree(raw)
            Q.need({p.name for p in raw.iterdir()} == {"cpu1", "cpu2", "repro1"}, "exact retained attempts")
            Q.need({p.name for p in (raw / "cpu1").iterdir()} == {
                "commands", "inputs-before.json", "inputs-after.json", "qualify.py", "source.patch"},
                "exact failed campaign artifacts")
            rows = terminal_receipts(raw / "cpu1/commands", CPU1_RECEIPTS, CPU1_RECEIPTS)
            rows += terminal_receipts(raw / "cpu2/commands", {name for name, _, _ in Q.stages()})
            rows += terminal_receipts(raw / "repro1", REPRO_RECEIPTS, REPRO_RECEIPTS)
            rows += terminal_receipts(output / "commands", {"replay", "rejections"})
            Q.need(len(rows) == 22, "complete campaign, reproduction and audit commands")
            for row in rows:
                Q.need(not c.B.group_exists(row["pid"]), "recorded command group remains")
            allocated = C.cache_bytes(target)
            Q.need(inputs() == before, "unchanged inputs before cleanup")
            result = C.remove_cache(target, HERE / "raw/cleanup-before.json",
                                    HERE / "raw/cleanup-after.json", allocated, c.B.write_json)
            Q.need(not target.exists() and not target.is_symlink(), "owned cache absent")
            print({"cache_removed_bytes": result["allocated_bytes"], "cache_absent": True}, flush=True)
    except BaseException as primary:
        try:
            finish()
        except BaseException as secondary:
            raise primary from secondary
        raise
    finish()


if __name__ == "__main__":
    main()
