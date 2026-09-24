#!/usr/bin/env python3
"""Check the retained CPU command, source, test and cleanup evidence."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import json
from pathlib import Path
import re
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
RUNNER_SHA = "45e1199f677f9bacaa3f336aa183dd05af4286d59647633a2c66a410ed555354"
ITERATIONS = {
    "check-initial.log": "c15b07cbccd106fb895f3264342fccc0ae1fe7eb80577a21037bc51466094bff",
    "clippy-library-initial.log": "a3dbf58f50bbccd89593a03f61279ca2f5c72bef934000306d4bb1f4a71016b1",
    "focused-initial.log": "b7172a03425c0c1e5ab567ee4119d2ce5ca7967a1beda4a443424e3f694585bd",
    "runtime-initial.log": "312c07f4f682abad9d5e3dcfb4818836ebb8fe48d592aa3a584c6cebdc39b05e",
    "focused-final.log": "4b27fac7215928e449c54564d7bbe157c69fa84d901761a81042957090a30cfb",
    "focused-corrected.log": "00d91d8d2e388ad0fef6e9a81bc992fce90d763c9469ce1ec5587984c8289be5",
}
FAILED_COLLECTION = {
    "collect.py": "3be22b5ad7ed86cc9e5d7ed76063edee16b41b39c4f71a7c7d6245730cd80f24",
    "verify.py": "eb855d4cddba181c3edbe926e344a2b5dc671d2d9baf4a1542f4150c1d3e1d24",
    "test_verify.py": "bd8ba4a0557371f22ea7daba6776c977448b93f2aa9a40845aa9763da1bb001d",
}
INITIAL_CLEANUP_SHA = "68780043a34b188ae7610b0c8282f7413c01fb60fc089f12a22fd09998387ea3"


def authenticated_module(path, digest, name):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise RuntimeError("ordinary helper required")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("helper identity mismatch")
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


R = authenticated_module(HERE / "runner.py", RUNNER_SHA, "producer_launch_runner")
SUMMARY = r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;"
FIELDS = {"command", "cwd", "started_ns", "timeout_seconds", "pid", "exit", "error", "group_absent",
          "environment", "stdin_sha256", "finished_ns", "stdout_sha256", "stderr_sha256"}
DELTA = {
    "async_engine/snapshot.rs", "async_engine/tests/owned_tests/snapshot_tests.rs",
    "async_engine/tests/owned_tests/snapshot_tests/producer_launch.rs", "context.rs",
    "context/generated_issue.rs", "context/peer_custody.rs", "context/peer_directed_context.rs",
    "context/peer_reconciliation.rs", "context/peer_segments.rs", "context/producer_launch.rs",
    "context/tests/peer_directed_tests.rs", "context/tests/producer_launch_tests.rs", "context/versions.rs",
    "context/versions/generated.rs", "context/versions/producer_readers.rs", "context/versions/readers.rs",
    "context/versions/submissions.rs", "kfd_backend.rs", "kfd_backend/compute_dispatch.rs",
    "kfd_backend/producer_launch_tests.rs",
}


def need(condition, description):
    if not condition:
        raise RuntimeError(description)


def read(path):
    need(path.is_file() and not path.is_symlink(), "ordinary JSON record")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            need(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    def constant(value):
        raise RuntimeError("nonfinite JSON value: " + value)
    return json.loads(path.read_text(), object_pairs_hook=unique, parse_constant=constant)


def environment():
    return {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
            "LC_ALL": "C", "CARGO_TARGET_DIR": str(R.PRIVATE / "target"), "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "CARGO_PROFILE_TEST_OPT_LEVEL": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}


def receipt(row, command, seconds, previous):
    need(set(row) == FIELDS, "exact receipt fields")
    need(row["command"] == command and row["cwd"] == str(R.REPO) and row["environment"] == environment()
         and type(row["timeout_seconds"]) is int and row["timeout_seconds"] == seconds
         and row["stdin_sha256"] is None, "exact command controls")
    need(type(row["exit"]) is int and row["exit"] == 0 and row["error"] is None
         and row["group_absent"] is True, "successful reaped command")
    need(type(row["pid"]) is int and row["pid"] > 0 and type(row["started_ns"]) is int
         and type(row["finished_ns"]) is int and previous <= row["started_ns"] < row["finished_ns"],
         "ordered terminal receipt")
    need(row["finished_ns"] - row["started_ns"] <= (seconds + 15) * 10**9, "command time bound")
    return row["finished_ns"]


def test_roster(text, expected=None):
    summaries = re.findall(SUMMARY, text, re.MULTILINE)
    need(len(summaries) == 1, "one passing test summary")
    counts = list(map(int, summaries[0]))
    need(counts[0] > 0 and counts[1] == counts[3] == 0, "nonempty tests without failures or measurements")
    if expected is not None:
        need(counts == expected, "exact test counts")
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored(?:, [^\n]*)?)$", text, re.MULTILINE)
    roster = {name: result.split(",", 1)[0] for name, result in rows}
    need(len(roster) == len(rows) == counts[0] + counts[2] and "FAILED" not in roster.values(),
         "unique complete named test roster")
    need(list(roster.values()).count("ok") == counts[0]
         and list(roster.values()).count("ignored") == counts[2], "named outcomes match summary")
    return roster, counts


def inventory(root):
    result = {}
    for path in sorted(root.rglob("*")):
        need(not path.is_symlink(), "ordinary evidence tree")
        if path.is_file():
            result[str(path.relative_to(root))] = R.sha(path)
        else:
            need(path.is_dir(), "ordinary evidence entry")
    return result


def iteration_inventory(raw):
    folder = raw / "iterations"
    for path in (raw, folder):
        need(path.is_dir() and not path.is_symlink() and path.resolve() == path,
             "ordinary resolved evidence directory")
    need({path.name for path in folder.iterdir()} == set(ITERATIONS), "exact development iteration roster")
    need(inventory(folder) == ITERATIONS, "pinned development iteration contents")
    return {"iterations/" + name: digest for name, digest in ITERATIONS.items()}


def failed_collection_inventory(raw):
    folder = raw / "collection-failure"
    need(folder.is_dir() and not folder.is_symlink() and folder.resolve() == folder,
         "ordinary original collector directory")
    need({path.name for path in folder.iterdir()} == set(FAILED_COLLECTION)
         and inventory(folder) == FAILED_COLLECTION, "exact original collection sources")


def cleanup_recovery(initial, final, recovery):
    target = R.PRIVATE / "target"
    need(set(initial) == {"path", "allocated_bytes", "absent"} and initial["path"] == str(target)
         and type(initial["allocated_bytes"]) is int and initial["allocated_bytes"] > 0
         and initial["absent"] is False, "original provisional cleanup receipt")
    need(final == initial | {"absent": True} and final["absent"] is True
         and type(final["allocated_bytes"]) is int,
         "independent final cache absence")
    need(recovery == {
        "cache_path": str(target), "initial_receipt_sha256": INITIAL_CLEANUP_SHA,
        "original_collector_sha256": FAILED_COLLECTION["collect.py"],
        "original_collection": "failed-final-receipt-exclusive-create",
        "observation": "owned-cache-absent", "deletion_repeated": False,
        "command_groups_absent": True,
    } and recovery["deletion_repeated"] is False and recovery["command_groups_absent"] is True,
         "explicit non-destructive cleanup recovery")


def validate_campaign(campaign, *, retained):
    expected_files = {"commands", "inputs-before.json", "inputs-after.json"}
    if retained:
        expected_files.add("runner.py")
    need({path.name for path in campaign.iterdir()} == expected_files, "exact campaign artifacts")
    before = read(campaign / "inputs-before.json")
    need(before == read(campaign / "inputs-after.json"), "unchanged source bracket")
    need(before == R.inputs(R.helpers()), "exact current runner and source inputs")
    need(before["runner"] == RUNNER_SHA, "qualified runner identity")
    if retained:
        need(R.sha(campaign / "runner.py") == RUNNER_SHA, "retained runner identity")
    baseline = R.REPO / "docs/evidence/dev-topology-link-directory-cpu-2026-09-24/raw/cpu2/inputs-before.json"
    need(R.sha(baseline) == "c1ea75c7e038a0ba120b199b8705673b8e4381155371f3cb80a7ea953f82f49e", "baseline source map")
    prior, source = read(baseline)["source"], before["source"]
    changed = {name for name in prior.keys() | source.keys() if prior.get(name) != source.get(name)}
    need(changed == {"crates/fe2o3-runtime/src/" + name for name in DELTA}, "exact twenty-path source delta")
    stages = list(R.stages())
    commands = campaign / "commands"
    need({path.name for path in commands.iterdir()} == {name for name, _, _ in stages}, "complete command roster")
    previous = 0
    for name, command, seconds in stages:
        folder = commands / name
        need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "exact stage artifacts")
        row = read(folder / "receipt.json")
        previous = receipt(row, command, seconds, previous)
        for stream in ("stdout", "stderr"):
            need(R.sha(folder / stream) == row[stream + "_sha256"], "captured output identity")
    for tool in ("cargo", "rustc"):
        for stream in ("stdout", "stderr"):
            need(R.sha(commands / tool / stream) == R.sha(commands / ("after-" + tool) / stream), "tool continuity")
    rosters = {}
    counts = {}
    for target in ("gnu", "musl"):
        rosters[target] = {}
        for package, expected in (("focused", [27, 0, 0, 0, 1385]),
                                  ("fe2o3-runtime", [1392, 0, 20, 0, 0]),
                                  ("fe2o3-kfd", [1561, 0, 0, 0, 0]),
                                  ("fe2o3-runtime-model", [1021, 0, 18, 0, 0])):
            text = (commands / (target + "-" + package) / "stdout").read_text()
            roster, result = test_roster(text, expected)
            need(package == "focused" or result[4] == 0, "full unfiltered library suite")
            rosters[target][package] = roster
            counts[target + "-" + package] = result
        need(rosters[target]["focused"].items() <= rosters[target]["fe2o3-runtime"].items(), "focused tests in full suite")
        need(all("producer_launch" in name for name in rosters[target]["focused"]), "producer launch focused roster")
    need(rosters["gnu"] == rosters["musl"], "identical named outcomes on both targets")
    prior_commands = baseline.parent / "commands"
    for package, digest, expected in (
        ("fe2o3-runtime", "8b53536e31335e7969d6838389d67a73c75b5e2c85af258bfb79acd168e1e802", [1365, 0, 20, 0, 0]),
        ("fe2o3-kfd", "a0b879a7321f1d68b5ed673f5f954d456d4cb5c57087aa4e6544d0966f721e58", [1561, 0, 0, 0, 0]),
    ):
        path = prior_commands / ("gnu-" + package) / "stdout"
        need(R.sha(path) == digest, "baseline named roster identity")
        prior_roster, _ = test_roster(path.read_text(), expected)
        if package == "fe2o3-runtime":
            need(not prior_roster.keys() & rosters["gnu"]["focused"].keys(), "new focused tests")
            prior_roster.update(rosters["gnu"]["focused"])
        need(prior_roster == rosters["gnu"][package], "complete baseline plus new named tests")
    model_log = R.REPO / "docs/evidence/dev-owner-lifecycle-2026-09-23/records/test/stdout.log"
    need(R.sha(model_log) == "c0271977d6de2ed722952a5100008fb3536a1e454723220c6389540baa160a6a", "model baseline identity")
    model_text = model_log.read_text()
    model_summary = re.search(SUMMARY, model_text, re.MULTILINE)
    need(model_summary is not None, "model baseline unit summary")
    model_roster, _ = test_roster(model_text[:model_summary.end()], [1021, 0, 18, 0, 0])
    need(model_roster == rosters["gnu"]["fe2o3-runtime-model"], "complete unchanged model named roster")
    docs = re.findall(SUMMARY, (commands / "docs/stdout").read_text(), re.MULTILINE)
    need([list(map(int, row)) for row in docs] == [[27, 0, 0, 0, 0], [4, 0, 0, 0, 0],
                                                [42, 0, 0, 0, 0], [27, 0, 0, 0, 0]], "exact doctest groups")
    return {"commands": len(stages), "source_inputs": len(before["source"]), "tests": counts,
            "doctests": sum(int(row[0]) for row in docs), "native_execution": False,
            "formal_refinement": False, "record_replay": "pass"}


def main():
    raw = HERE / "raw"
    iteration_inventory(raw)
    failed_collection_inventory(raw)
    need({path.name for path in raw.iterdir()} == {
        "iterations", "campaign1", "campaign1.log", "cleanup.json", "cleanup-result.json",
        "cleanup-recovery.json", "collection-failure",
    },
         "exact raw evidence roster")
    need(inventory(raw) == read(HERE / "artifacts.json"), "complete artifact inventory")
    result = validate_campaign(raw / "campaign1", retained=True)
    need(R.sha(raw / "cleanup.json") == INITIAL_CLEANUP_SHA, "unchanged original cleanup receipt")
    cleanup_recovery(read(raw / "cleanup.json"), read(raw / "cleanup-result.json"), read(raw / "cleanup-recovery.json"))
    target = R.PRIVATE / "target"
    need(not target.exists() and not target.is_symlink(), "owned Cargo cache absent")
    result["collection_recovery"] = "final-receipt-only; deletion not repeated"
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
