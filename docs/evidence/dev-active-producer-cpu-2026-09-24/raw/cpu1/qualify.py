#!/usr/bin/env python3
"""Record or replay the active-producer CPU qualification, never native evidence."""

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
PRIVATE = Path("/home/harsh/.codex-tmp/fe2o3-active-producer-20260924-b0xiLk")
BASE = REPO / "docs/evidence/dev-producer-aware-launch-cpu-2026-09-24"


def need(condition, detail):
    if not condition:
        raise RuntimeError(detail)


def authenticated_module(path, digest, name):
    need(stat.S_ISREG(path.lstat().st_mode), "ordinary authenticated helper")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == digest, "helper identity")
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


V = authenticated_module(BASE / "verify.py",
    "8aacaff8fb1e8880dfa74067e8bf9d5f010add1c74039db09ae34e57d3951f4f", "active_producer_baseline")
R = V.R


def inputs():
    return {"runner": R.sha(Path(__file__)), "source": R.inputs(R.helpers())["source"]}


def environment():
    return V.environment() | {"CARGO_TARGET_DIR": str(PRIVATE / "target")}


def stages():
    cargo = ["cargo", "--offline", "--locked"]
    yield "rustc", ["rustc", "-vV"], 30
    yield "cargo", ["cargo", "-V"], 30
    yield "format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], 120
    yield "clippy", [*cargo, "clippy", "-p", "fe2o3-runtime", "--all-features", "--all-targets",
                     "--", "-D", "warnings"], 1800
    for label, target in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
        yield label + "-default", [*cargo, "check", "-p", "fe2o3-runtime", "--all-targets", *target], 1800
        yield label + "-focused", [*cargo, "test", "-p", "fe2o3-runtime", "--all-features", "--lib",
                                  *target, "producer_launch", "--", "--test-threads=4"], 1800
        yield label + "-runtime", [*cargo, "test", "-p", "fe2o3-runtime", "--all-features", "--lib",
                                  *target, "--", "--test-threads=4"], 1800
    yield "docs", [*cargo, "test", "-p", "fe2o3-runtime", "--all-features", "--doc"], 1800
    yield "after-rustc", ["rustc", "-vV"], 30
    yield "after-cargo", ["cargo", "-V"], 30


def verify(output):
    need({path.name for path in output.iterdir()} == {"inputs-before.json", "inputs-after.json", "commands"},
         "exact campaign tree")
    before = V.read(output / "inputs-before.json")
    need(before == V.read(output / "inputs-after.json") == inputs(), "unchanged current source and runner")
    baseline = BASE / "raw/campaign1/inputs-before.json"
    prior = V.read(baseline)["source"]
    need(prior == V.read(BASE / "raw/campaign1/inputs-after.json")["source"], "baseline source bracket")
    changed = {name for name in prior.keys() | before["source"].keys()
               if prior.get(name) != before["source"].get(name)}
    need(changed == {"crates/fe2o3-runtime/src/kfd_backend.rs",
                     "crates/fe2o3-runtime/src/kfd_backend/compute_dispatch.rs"}, "exact two-path source delta")
    commands = output / "commands"
    need({p.name for p in commands.iterdir()} == {name for name, _, _ in stages()}, "exact stage roster")
    previous = 0
    for name, command, bound in stages():
        folder = commands / name
        need({p.name for p in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "exact command files")
        row = V.read(folder / "receipt.json")
        need(set(row) == V.FIELDS, "exact receipt fields")
        need(row["command"] == command and row["cwd"] == str(REPO)
             and row["environment"] == environment() and type(row["timeout_seconds"]) is int
             and row["timeout_seconds"] == bound and row["stdin_sha256"] is None, "command controls")
        need(type(row["exit"]) is int and row["exit"] == 0 and row["error"] is None
             and row["group_absent"] is True, "successful reaped command")
        need(type(row["pid"]) is int and row["pid"] > 0
             and type(row["started_ns"]) is int and type(row["finished_ns"]) is int
             and previous <= row["started_ns"] < row["finished_ns"]
             and row["finished_ns"] - row["started_ns"] <= (bound + 15) * 10**9, "ordered bounded command")
        previous = row["finished_ns"]
        for stream in ("stdout", "stderr"):
            need(R.sha(folder / stream) == row[stream + "_sha256"], "captured output identity")
    for tool in ("cargo", "rustc"):
        for stream in ("stdout", "stderr"):
            need(R.sha(commands / tool / stream) == R.sha(commands / ("after-" + tool) / stream), "tool continuity")
    rosters = {}
    for label in ("gnu", "musl"):
        focused, _ = V.test_roster((commands / (label + "-focused") / "stdout").read_text(), [35, 0, 0, 0, 1385])
        full, _ = V.test_roster((commands / (label + "-runtime") / "stdout").read_text(), [1400, 0, 20, 0, 0])
        need(focused.items() <= full.items() and all("producer_launch" in name for name in focused), "focused roster")
        old, _ = V.test_roster((BASE / ("raw/campaign1/commands/" + label + "-fe2o3-runtime/stdout")).read_text(),
                               [1392, 0, 20, 0, 0])
        need(old.items() <= full.items() and full.keys() - old.keys() == {
            "kfd_backend::tests::producer_launch_active_three_binding_" + suffix for suffix in (
                "inputs_wait_without_materialization", "cancellation_keeps_parent_owners",
                "rejects_missing_or_foreign_authority", "requires_restored_ready_backing",
                "public_rejections_preserve_custody", "rechecks_final_authority",
                "parent_terminal_retains_both_rosters", "failed_receipts_never_publish_child")}, "exact added tests")
        rosters[label] = (focused, full)
    need(rosters["gnu"] == rosters["musl"], "identical target rosters")
    docs = re.findall(V.SUMMARY, (commands / "docs/stdout").read_text(), re.MULTILINE)
    need([list(map(int, row)) for row in docs] == [[27, 0, 0, 0, 0], [27, 0, 0, 0, 0]], "54 runtime doctests")
    print("active-producer CPU replay: pass; 13 stages; 35 focused; 1400 runtime +20 hardware ignores per target; "
          "54 doctests; native/formal/performance: not established", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    output = args.output.absolute()
    need(output.parent == HERE / "raw" and output.resolve() == output
         and re.fullmatch(r"cpu[1-9][0-9]*", output.name) is not None, "exact new evidence directory")
    if args.verify:
        verify(output)
        return
    target = PRIVATE / "target"
    need(target.resolve() == target and target.is_dir() and not target.is_symlink(), "owned target directory")
    output.mkdir(parents=True)
    before = inputs()
    c = R.helpers()
    c.B.write_json(output / "inputs-before.json", before)
    for number in c.B.MANAGED:
        signal.signal(number, c.B.interrupted)
    recorder = c.B.Recorder(output / "commands", REPO)

    def finish():
        after = inputs()
        c.B.write_json(output / "inputs-after.json", after)
        need(before == after, "unchanged campaign inputs")

    try:
        for name, command, bound in stages():
            recorder.run(name, command, bound, env=environment())
    except BaseException as primary:
        try:
            finish()
        except BaseException as secondary:
            raise primary from secondary
        raise
    finish()
    verify(output)


if __name__ == "__main__":
    main()
