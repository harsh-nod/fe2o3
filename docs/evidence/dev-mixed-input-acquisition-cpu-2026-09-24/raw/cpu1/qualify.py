#!/usr/bin/env python3
"""Source-bound CPU qualification of atomic mixed launch-input acquisition."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
from pathlib import Path
import re
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PRIVATE = Path("/home/harsh/.codex-tmp/fe2o3-mixed-inputs-20260924-Ta5gI69r")
BASE = REPO / "docs/evidence/dev-native-producer-witness-cpu-2026-09-24/raw/cpu4"
MODEL_BASE = REPO / "docs/evidence/dev-producer-aware-launch-cpu-2026-09-24/raw/campaign1"
HELPER = REPO / "docs/evidence/dev-active-producer-cpu-2026-09-24/qualify.py"
raw = HELPER.read_bytes()
if not stat.S_ISREG(HELPER.lstat().st_mode) or hashlib.sha256(raw).hexdigest() != \
        "4637727913136890e823e6f9a475077fbef216da4502d5899fcf569e927543b8":
    raise RuntimeError("authenticated CPU controller")
Q = ModuleType("mixed_input_cpu_controller")
Q.__file__ = str(HELPER)
exec(compile(raw, Q.__file__, "exec"), Q.__dict__)
V, R, need = Q.V, Q.R, Q.need

BASELINE = {
    "inputs-before.json": "41b41de7b9a3cbfd56464ff6b2d18f4754a045638bfea4db868d7bb3a4667976",
    "commands/gnu-runtime/stdout": "9695862ecd263ebd76f52be12c3a0a08a9d1dcd4e511e704484958a9755ba261",
    "commands/musl-runtime/stdout": "6ba0bf8b0716215c128798b20f5ab4b944680fdc0596a20ba2a1801317b26c70",
}
MODEL_BASELINE = {
    "gnu": "4a57cedbae418219e78b08dc95413f54e48994b84c6bb9563f0a1290b4e25f32",
    "musl": "6888260cfe4d6a08358db95d0ea1de3a0e86cbe442235a9ca8e23211f1b8ca01",
}
CHANGED = {
    "crates/fe2o3-runtime-model/src/context_producer_reads.rs",
    "crates/fe2o3-runtime-model/src/context_producer_reads/tests.rs",
    "crates/fe2o3-runtime-model/src/context_producer_reads/mixed_acquire_bodies.rs",
    "crates/fe2o3-runtime-model/src/context_producer_reads/tests/mixed_acquire.rs",
    "crates/fe2o3-runtime-model/src/context_read_leases.rs",
    "crates/fe2o3-runtime-model/src/context_read_leases/acquire.rs",
    "crates/fe2o3-runtime-model/verus/context_mixed_acquire_execution_v1.rs",
    "crates/fe2o3-runtime-model/verus/context_mixed_acquire_paired_v1.rs",
    "crates/fe2o3-runtime/src/context/tests/producer_launch_tests.rs",
    "crates/fe2o3-runtime/src/context/versions.rs",
    "crates/fe2o3-runtime/src/context/versions/producer_readers.rs",
    "crates/fe2o3-runtime/src/context/versions/readers.rs",
    "crates/fe2o3-runtime/src/context/versions/submissions.rs",
}
ADDED = {"context::tests::producer_launch_tests::producer_launch_" + suffix for suffix in (
    "mixed_acquisition_handles_each_empty_side_and_no_inputs",
    "acquire_error_retains_both_unmarked_original_input_roots",
    "finalization_panic_retains_all_leases_without_publishing_markers",
)}
MODEL_ADDED = {"context_producer_reads::tests::mixed_acquire::mixed_acquire_" + suffix for suffix in (
    "exact_rosters_share_one_budget_without_storage_growth",
    "late_rejection_on_either_side_preserves_every_owner_and_output",
    "checks_combined_headroom_before_either_commit",
    "independent_incarnation_exhaustion_is_atomic",
    "empty_sides_skip_only_the_unused_arena",
    "validates_consumer_empty_output_shapes_and_occupied_outputs",
    "canonical_rosters_and_cross_kind_aliases_reject_atomically",
)}


def baseline(name):
    path = BASE / name
    need(R.sha(path) == BASELINE[name], "qualified baseline identity")
    return path


def environment():
    return V.environment() | {"CARGO_TARGET_DIR": str(PRIVATE / "target"), "CARGO_PROFILE_TEST_OPT_LEVEL": "1"}


def stages():
    cargo = ["cargo", "--offline", "--locked"]
    packages = ["-p", "fe2o3-runtime", "-p", "fe2o3-runtime-model"]
    yield "rustc", ["rustc", "-vV"], 30
    yield "cargo", ["cargo", "-V"], 30
    yield "format", ["cargo", "fmt", *packages, "--", "--check"], 120
    yield "clippy", [*cargo, "clippy", *packages, "--all-features", "--all-targets", "--", "-D", "warnings"], 1800
    for label, target in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
        yield label + "-default", [*cargo, "check", *packages, "--all-targets", *target], 1800
        yield label + "-focused", [*cargo, "test", "-p", "fe2o3-runtime", "--all-features", "--lib",
                                  *target, "producer_launch", "--", "--test-threads=4"], 1800
        for name, package in (("runtime", "fe2o3-runtime"), ("model", "fe2o3-runtime-model")):
            yield label + "-" + name, [*cargo, "test", "-p", package, "--all-features", "--lib",
                                       *target, "--", "--test-threads=4"], 1800
    yield "docs", [*cargo, "test", *packages, "--all-features", "--doc"], 1800
    yield "after-rustc", ["rustc", "-vV"], 30
    yield "after-cargo", ["cargo", "-V"], 30


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
    need(changed == CHANGED, "exact thirteen-path source delta")
    need(len(before["source"]) == 3965, "complete source inventory")
    commands = output / "commands"
    need({path.name for path in commands.iterdir()} == {name for name, _, _ in stages()}, "exact stages")
    previous = 0
    for name, command, bound in stages():
        folder = commands / name
        need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "command files")
        row = V.read(folder / "receipt.json")
        need(set(row) == V.FIELDS and row["command"] == command and row["cwd"] == str(REPO)
             and row["environment"] == environment() and type(row["timeout_seconds"]) is int
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
        full, _ = V.test_roster((commands / (label + "-runtime") / "stdout").read_text(), [1403, 0, 22, 0, 0])
        focused, _ = V.test_roster((commands / (label + "-focused") / "stdout").read_text(), [38, 0, 2, 0, 1385])
        old, _ = V.test_roster(baseline("commands/" + label + "-runtime/stdout").read_text(), [1400, 0, 22, 0, 0])
        need(old.items() <= full.items() and full.keys() - old.keys() == ADDED
             and all(full[name] == "ok" for name in ADDED), "exact three runtime additions")
        need(focused == {name: status for name, status in full.items() if "producer_launch" in name}, "focus roster")
        model, _ = V.test_roster((commands / (label + "-model") / "stdout").read_text(), [1028, 0, 18, 0, 0])
        baseline_model = MODEL_BASE / ("commands/" + label + "-fe2o3-runtime-model/stdout")
        need(R.sha(baseline_model) == MODEL_BASELINE[label], "qualified model baseline identity")
        old_model, _ = V.test_roster(baseline_model.read_text(), [1021, 0, 18, 0, 0])
        need(old_model.items() <= model.items() and model.keys() - old_model.keys() == MODEL_ADDED
             and all(model[name] == "ok" for name in MODEL_ADDED), "exact seven model additions")
        rosters.append((full, focused, model))
    need(rosters[0] == rosters[1], "same target rosters")
    docs = re.findall(V.SUMMARY, (commands / "docs/stdout").read_text(), re.MULTILINE)
    need([list(map(int, row)) for row in docs] == [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0], [27, 0, 0, 0, 0]], "73 doctests")
    print("mixed-input CPU replay: PASS; 15 stages; GNU/musl each 1403 runtime +22 ignored, "
          "1028 model +18 ignored; 73 doctests; 3965 unchanged inputs; no native or authenticated proof claim", flush=True)


Q.HERE, Q.PRIVATE, Q.__file__ = HERE, PRIVATE, __file__
Q.__doc__ = __doc__
Q.verify, Q.environment, Q.stages = verify, environment, stages

if __name__ == "__main__":
    Q.main()
