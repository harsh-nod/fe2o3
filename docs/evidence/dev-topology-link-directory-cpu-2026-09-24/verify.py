#!/usr/bin/env python3
"""Replay source, tool, test, command and cleanup records for the CPU candidate."""

import importlib.util
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("directory_cpu_collect", ROOT / "collect.py")
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
R, C, P = M.R, M.C, M.P
FIRST_RUNNER = "5f01a2500a0d1db3438e53acb8ebad8702b8f7ffd3dd1c50b799c099cd2ab9dc"
BASELINE_COMMIT = "8dc128357ecd55e1ba4f2866eb075899481f9aa0"
BASELINE_STDOUT = "docs/evidence/dev-xgmi-backing-budgets-cpu-2026-09-24/cpu3/gnu/stdout.log"
TOPOLOGY_STDOUT = "docs/evidence/dev-topology-link-scratch-cpu-2026-09-24/raw/cpu1/gnu-topology/stdout.log"
SUMMARY = r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;"
FOCUSED = {"topology::tests::prechecked_reads::link_directories::" + name: "ok" for name in (
    "every_discovery_checks_each_link_directory_once_and_reads_all_properties",
    "single_directory_check_preserves_stable_contents_and_type_refusals",
    "listing_and_property_validation_remain_fail_closed",
)}


def environment(attempt):
    return {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
            "LC_ALL": "C", "CARGO_TARGET_DIR": str(R.PRIVATE / attempt / "target"), "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "CARGO_PROFILE_TEST_OPT_LEVEL": "1",
            "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}


def named_results(text):
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored(?:, [^\n]*)?)$", text, re.MULTILINE)
    result = {name: status.split(",", 1)[0] for name, status in rows}
    P.need(len(result) == len(rows) and "FAILED" not in result.values(), "unique successful named tests")
    return result


def baseline_rosters():
    P.need(P.sha(Path(C.C.SIGNERS)) == C.C.SIGNERS_SHA, "trusted baseline signer")
    def git(*args):
        return subprocess.check_output([*C.K.GIT, *args], cwd=R.REPO, env=C.K.GIT_ENV, timeout=120)
    git("-c", "gpg.ssh.allowedSignersFile=" + C.C.SIGNERS, "verify-commit", BASELINE_COMMIT)
    text = git("show", BASELINE_COMMIT + ":" + BASELINE_STDOUT).decode()
    summaries = list(re.finditer(SUMMARY, text, re.MULTILINE))
    P.need([list(map(int, match.groups())) for match in summaries]
           == [[1552, 0, 0, 0, 0], [1365, 0, 20, 0, 0]], "signed baseline result counts")
    kfd = named_results(text[:summaries[0].start()])
    runtime = named_results(text[summaries[0].end():summaries[1].start()])
    P.need(len(kfd) == 1552 and set(kfd.values()) == {"ok"} and len(runtime) == 1385
           and list(runtime.values()).count("ignored") == 20, "signed baseline named rosters")
    topology_text = git("show", BASELINE_COMMIT + ":" + TOPOLOGY_STDOUT).decode()
    P.need([list(map(int, row)) for row in re.findall(SUMMARY, topology_text, re.MULTILINE)]
           == [[83, 0, 0, 0, 1475]], "signed newer topology result counts")
    topology = named_results(topology_text)
    old_topology = {name: status for name, status in kfd.items() if name.startswith("topology::")}
    P.need(len(old_topology) == 77 and len(topology) == 83 and set(topology.values()) == {"ok"}
           and all(name.startswith("topology::") for name in topology)
           and old_topology.items() <= topology.items(), "complete signed newer topology roster")
    kfd.update(topology)
    P.need(len(kfd) == 1558, "complete signed baseline KFD roster")
    P.need(not kfd.keys() & FOCUSED.keys(), "new focused test names")
    return kfd | FOCUSED, runtime


def test_summary(folder, expected, ignored, filtered, roster):
    text = (folder / "stdout").read_text()
    matches = re.findall(SUMMARY, text, re.MULTILINE)
    P.need(len(matches) == 1 and list(map(int, matches[0])) == [expected, 0, ignored, 0, filtered], "test result count")
    P.need(named_results(text) == roster, "exact named test outcomes")


def main():
    raw = ROOT / "raw"
    P.need(P.inventory(raw) == P.read(ROOT / "artifacts.json"), "exact artifact inventory")
    current = R.inputs(C)
    prior = R.prior_sources()
    commands = list(R.stages())
    for attempt in ("cpu1", "cpu2"):
        folder = raw / attempt
        before = P.read(folder / "inputs-before.json")
        P.need(before == P.read(folder / "inputs-after.json"), "unchanged attempt inputs")
        P.need(before["source"] == current["source"], "exact candidate source")
        P.need(P.sha(folder / "runner.py") == before["runner"]
               == (FIRST_RUNNER if attempt == "cpu1" else current["runner"]), "frozen attempt runner")
        changed = {name for name in prior.keys() | current["source"].keys() if prior.get(name) != current["source"].get(name)}
        P.need(changed == R.DELTA and P.read(folder / "prior-delta.json") == {
            "prior_sha256": R.PRIOR_SHA, "changed": sorted(R.DELTA)}, "exact authenticated baseline delta")
        stages = commands[:4] if attempt == "cpu1" else commands
        P.need({path.name for path in (folder / "commands").iterdir()} == {row[0] for row in stages}, "command roster")
        previous = 0
        for name, command, seconds in stages:
            stage = folder / "commands" / name
            row = P.read(stage / "receipt.json")
            interrupted = attempt == "cpu1" and name == "gnu-focused"
            P.need(set(row) == {"command", "cwd", "started_ns", "timeout_seconds", "pid", "exit", "error",
                               "group_absent", "environment", "stdin_sha256", "finished_ns", "stdout_sha256", "stderr_sha256"}, "receipt fields")
            P.need(row["command"] == command and row["cwd"] == str(R.REPO) and row["environment"] == environment(attempt)
                   and type(row["timeout_seconds"]) is int and row["timeout_seconds"] == seconds
                   and row["stdin_sha256"] is None, "exact command controls")
            P.need(type(row["exit"]) is int and row["exit"] == (-15 if interrupted else 0)
                   and row["error"] == ("RuntimeError: interrupted by signal 15" if interrupted else None)
                   and row["group_absent"] is True, "terminal command disposition")
            P.need(type(row["pid"]) is int and row["pid"] > 0 and type(row["started_ns"]) is int
                   and type(row["finished_ns"]) is int and previous <= row["started_ns"] < row["finished_ns"], "ordered command receipts")
            P.need(row["finished_ns"] - row["started_ns"] <= (seconds + 15) * 10**9, "bounded command elapsed time")
            previous = row["finished_ns"]
            for stream in ("stdout", "stderr"):
                P.need(P.sha(stage / stream) == row[stream + "_sha256"], "captured output identity")
        cleanup = P.read(raw / (attempt + "-cleanup.json"))
        P.need(type(cleanup) is list and len(cleanup) == 1, "one owned cache cleanup")
        row = cleanup[0]
        target = R.PRIVATE / attempt / "target"
        P.need(set(row) == {"path", "allocated_bytes", "absent"} and row["path"] == str(target)
               and type(row["allocated_bytes"]) is int and row["allocated_bytes"] >= 0 and row["absent"] is True,
               "exact owned cleanup record")
        P.need(not target.exists() and not target.is_symlink(), "owned cache still exists")
    accepted = raw / "cpu2/commands"
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            P.need(P.sha(accepted / tool / stream) == P.sha(accepted / ("after-" + tool) / stream), "tool continuity")
    kfd, runtime = baseline_rosters()
    for target in ("gnu", "musl"):
        test_summary(accepted / (target + "-focused"), 3, 0, 1558, FOCUSED)
        test_summary(accepted / (target + "-fe2o3-kfd"), 1561, 0, 0, kfd)
        test_summary(accepted / (target + "-fe2o3-runtime"), 1365, 20, 0, runtime)
    docs = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
                      (accepted / "docs/stdout").read_text(), re.MULTILINE)
    P.need(list(map(int, docs)) == [27, 4, 42], "complete doctest result counts")
    print(json.dumps({"cpu_commands": len(commands), "source_inputs": len(current["source"]), "new_tests": 3,
                      "kfd_per_target": 1561, "runtime_per_target": 1365, "runtime_ignored_per_target": 20,
                      "doctests": 73, "record_replay": "pass", "native_execution": False, "formal_refinement": False}))


if __name__ == "__main__":
    main()
