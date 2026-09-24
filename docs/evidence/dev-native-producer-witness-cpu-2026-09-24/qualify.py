#!/usr/bin/env python3
"""Qualify the native producer witness source without claiming GPU execution."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
from pathlib import Path
import re
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
BASE = REPO / "docs/evidence/dev-active-producer-cpu-2026-09-24"
PRIVATE = Path("/home/harsh/.codex-tmp/fe2o3-native-producer-20260924-BSu4wHKM")
raw = (BASE / "qualify.py").read_bytes()
if (BASE / "qualify.py").is_symlink() or hashlib.sha256(raw).hexdigest() != \
        "4637727913136890e823e6f9a475077fbef216da4502d5899fcf569e927543b8":
    raise RuntimeError("authenticated CPU controller")
Q = ModuleType("native_producer_cpu_controller")
Q.__file__ = str(BASE / "qualify.py")
exec(compile(raw, Q.__file__, "exec"), Q.__dict__)
V, R, need = Q.V, Q.R, Q.need
BASELINE = {
    "inputs-before.json": "86cc3b978d14a24231c68e72d9dc68f2af2911447194e40cd6882dbf3ce26e17",
    "commands/gnu-runtime/stdout": "b3ee76478026d644fe1a39277ea2dffbf229dfac80219dc98ae629ebbaf779f5",
    "commands/musl-runtime/stdout": "d22a8c96ac4c9ca5f2db109b7d5216be174f45ae30a4d478136c77a57009aa13",
}
ADDED = {"kfd_backend::retained_release_tests::producer_launch::native_runtime_producer_launch_" + mode
         for mode in ("queued_chain", "published_chain")}


def baseline(name):
    path = BASE / "raw/cpu2" / name
    need(R.sha(path) == BASELINE[name], "qualified baseline identity")
    return path


def verify(output):
    V.inventory(output)
    need({path.name for path in output.iterdir()} ==
         {"inputs-before.json", "inputs-after.json", "commands", "qualify.py"}, "exact campaign tree")
    before = V.read(output / "inputs-before.json")
    need(before == V.read(output / "inputs-after.json") == Q.inputs(), "unchanged source and runner")
    need(R.sha(output / "qualify.py") == before["runner"], "retained runner identity")
    prior = V.read(baseline("inputs-before.json"))["source"]
    changed = {name for name in prior.keys() | before["source"].keys()
               if prior.get(name) != before["source"].get(name)}
    prefix = "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests"
    need(changed == {prefix + ".rs", prefix + "/cold_allocation.rs", prefix + "/producer_launch.rs"},
         "three test-only source paths")
    need(len(before["source"]) == 3961, "complete source inventory")
    commands = output / "commands"
    need({path.name for path in commands.iterdir()} == {name for name, _, _ in Q.stages()}, "exact stages")
    previous = 0
    for name, command, bound in Q.stages():
        folder = commands / name
        need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "command files")
        row = V.read(folder / "receipt.json")
        need(set(row) == V.FIELDS and row["command"] == command and row["cwd"] == str(REPO)
             and row["environment"] == Q.environment() and type(row["timeout_seconds"]) is int
             and row["timeout_seconds"] == bound and row["stdin_sha256"] is None, "command controls")
        need(type(row["exit"]) is int and row["exit"] == 0 and row["error"] is None
             and row["group_absent"] is True, "reaped successful command")
        need(type(row["pid"]) is int and row["pid"] > 0
             and type(row["started_ns"]) is int and type(row["finished_ns"]) is int
             and previous <= row["started_ns"] < row["finished_ns"]
             and row["finished_ns"] - row["started_ns"] <= (bound + 15) * 10**9, "ordered bounded receipt")
        previous = row["finished_ns"]
        for stream in ("stdout", "stderr"):
            need(R.sha(folder / stream) == row[stream + "_sha256"], "output identity")
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            need(R.sha(commands / tool / stream) == R.sha(commands / ("after-" + tool) / stream), "tool continuity")
    rosters = []
    for label in ("gnu", "musl"):
        full, _ = V.test_roster((commands / (label + "-runtime") / "stdout").read_text(), [1400, 0, 22, 0, 0])
        focused, _ = V.test_roster((commands / (label + "-focused") / "stdout").read_text(), [35, 0, 2, 0, 1385])
        old, _ = V.test_roster(baseline("commands/" + label + "-runtime/stdout").read_text(), [1400, 0, 20, 0, 0])
        need(old.items() <= full.items() and full.keys() - old.keys() == ADDED
             and all(full[name] == "ignored" for name in ADDED), "exact two hardware-only additions")
        need(focused == {name: status for name, status in full.items() if "producer_launch" in name}, "focus roster")
        rosters.append((full, focused))
    need(rosters[0] == rosters[1], "same target rosters")
    docs = re.findall(V.SUMMARY, (commands / "docs/stdout").read_text(), re.MULTILINE)
    need([list(map(int, row)) for row in docs] == [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]], "46 runtime doctests")
    print("native producer witness CPU replay: PASS; 13 stages; GNU/musl 1400 passed +22 ignored; "
          "46 doctests; 3961 unchanged inputs; no native, proof or performance claim", flush=True)


# Reuse the authenticated recorder, bounded command roster and source brackets.
Q.HERE, Q.PRIVATE, Q.__file__, Q.verify = HERE, PRIVATE, __file__, verify

if __name__ == "__main__":
    Q.main()
