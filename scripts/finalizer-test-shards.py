#!/usr/bin/env python3
"""Discover the complete default finalizer test inventory and run one exact shard.

The caller's existing CI step deadline supervises this process and its children.
No new process group, test timeout, feature selection or ignored-test policy is
introduced here. Cargo remains the test launcher so its runtime environment is
the same as an ordinary package test invocation.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "fe2o3-hsaco-finalize"
SHARDS = 4
LIMIT = 64 * 1024 * 1024
KINDS = {"lib": "--lib", "test": "--test", "bin": "--bin", "example": "--example"}


class ShardError(Exception):
    pass


def need(value, message):
    if not value:
        raise ShardError(message)


def unique_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate JSON field")
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def target_key(target):
    need(type(target) is dict and type(target.get("name")) is str
         and re.fullmatch(r"[A-Za-z0-9_][A-Za-z0-9_-]*", target["name"]), "ordinary Cargo target name")
    kinds = target.get("kind")
    need(type(kinds) is list and len(kinds) == 1 and type(kinds[0]) is str, "single Cargo target kind")
    return kinds[0], target["name"]


def inventory(metadata, messages, root=ROOT):
    need(type(metadata) is dict and metadata.get("version") == 1, "Cargo metadata version 1")
    packages = [p for p in metadata["packages"] if p["name"] == PACKAGE]
    need(len(packages) == 1, "one finalizer package")
    package = packages[0]
    need(Path(package["manifest_path"]).resolve() == root / "crates" / PACKAGE / "Cargo.toml",
         "exact finalizer manifest")
    declared = {}
    for target in package["targets"]:
        key = target_key(target)
        need(key not in declared and type(target.get("test")) is bool, "unique typed Cargo target")
        declared[key] = target
    artifacts, features, finishes = {}, set(), []
    for message in messages:
        reason = message.get("reason")
        need(reason in ("compiler-artifact", "compiler-message", "build-script-executed", "build-finished"),
             "recognized Cargo JSON build event")
        if reason == "build-finished":
            finishes.append(message.get("success"))
        if reason != "compiler-artifact" or message.get("package_id") != package["id"]:
            continue
        key = target_key(message["target"])
        need(key in declared and message["target"]["src_path"] == declared[key]["src_path"],
             "compiled target matches original metadata")
        enabled = message.get("features")
        need(type(enabled) is list and all(type(f) is str for f in enabled), "Cargo feature roster")
        features.update(enabled)
        profile = message.get("profile")
        need(type(profile) is dict and type(profile.get("test")) is bool, "Cargo test profile")
        if not profile["test"]:
            continue
        executable = message.get("executable")
        need(key[0] in KINDS and key not in artifacts and type(executable) is str
             and Path(executable).is_absolute(), "one supported executable per default test target")
        artifacts[key] = executable
    need(finishes == [True], "one successful complete Cargo build")
    expected = set()
    for key, target in declared.items():
        required = target.get("required-features", [])
        need(type(required) is list and all(type(f) is str for f in required), "target required features")
        if target["test"] and set(required) <= features:
            need(key[0] in KINDS, "unsupported enabled default test harness target")
            expected.add(key)
    need(artifacts and artifacts.keys() == expected, "complete default package test target inventory")
    return artifacts


def listing(raw):
    cases, totals = {}, []
    for line in raw.splitlines():
        if not line:
            continue
        summary = re.fullmatch(r"(\d+) tests?, (\d+) benchmarks?", line)
        if summary:
            totals.append(tuple(map(int, summary.groups())))
            continue
        name, separator, kind = line.rpartition(": ")
        need(separator and kind in ("test", "benchmark") and name and not name.startswith("-")
             and len(name.encode()) <= 2048 and all(ord(c) >= 32 and ord(c) != 127 for c in name),
             "exact ordinary libtest listing entry")
        need(name not in cases, "duplicate libtest name")
        cases[name] = kind
    need(totals == [(sum(k == "test" for k in cases.values()), sum(k == "benchmark" for k in cases.values()))],
         "complete libtest list summary")
    return cases


def partition(roster):
    universe = [(kind, target, name) for (kind, target), cases in sorted(roster.items())
                for name in sorted(cases)]
    need(universe and len(universe) == len(set(universe)), "nonempty unique target/name inventory")
    shards = [universe[index::SHARDS] for index in range(SHARDS)]
    flattened = [case for shard in shards for case in shard]
    need(len(flattened) == len(set(flattened)) and set(flattened) == set(universe),
         "complete disjoint four-way test partition")
    return shards


def validate_manifest(value):
    need(type(value) is dict and set(value) == {"schema", "package", "commit", "shards", "targets"}
         and type(value["schema"]) is int and value["schema"] == 1 and value["package"] == PACKAGE
         and type(value["commit"]) is str and re.fullmatch(r"[0-9a-f]{40}", value["commit"]), "exact shard inventory schema")
    roster, ignored = {}, {}
    for target in value["targets"]:
        need(type(target) is dict and set(target) == {"kind", "name", "executable", "cases", "ignored"}, "exact target receipt")
        key = target_key(dict(name=target["name"], kind=[target["kind"]]))
        need(key[0] in KINDS and key not in roster, "unique supported target receipt")
        cases = target["cases"]
        need(type(cases) is dict and all(type(name) is str and name and kind in ("test", "benchmark")
             for name, kind in cases.items()), "typed complete case roster")
        skipped = target["ignored"]
        need(type(skipped) is list and skipped == sorted(set(skipped)) and set(skipped) <= cases.keys(), "exact ignored roster")
        roster[key], ignored[key] = cases, set(skipped)
    expected = [[list(case) for case in shard] for shard in partition(roster)]
    need(value["shards"] == expected, "inventory uses the complete deterministic partition")
    return roster, ignored


def aggregate(directory, commit):
    need(directory.is_dir() and not directory.is_symlink() and re.fullmatch(r"[0-9a-f]{40}", commit), "aggregate source and directory")
    files = []
    for path in directory.rglob("*"):
        need(not path.is_symlink(), "ordinary aggregate artifacts")
        if path.is_file():
            need(path.name in ("inventory.json", "result.json") and path.stat().st_size <= LIMIT, "bounded exact receipt files")
            files.append(path)
    inventories = sorted(path for path in files if path.name == "inventory.json")
    need(len(inventories) == 4 and len(files) == 8, "four complete shard receipt pairs")
    seen, common, executed = set(), None, []
    for path in inventories:
        raw = path.read_bytes()
        manifest = unique_json(raw)
        roster, ignored = validate_manifest(manifest)
        need(manifest["commit"] == commit, "shard belongs to the exact workflow checkout")
        canonical = {**manifest, "targets": [{k: v for k, v in row.items() if k != "executable"}
                                             for row in manifest["targets"]]}
        need(common is None or common == canonical, "all shards discovered the identical test inventory")
        common = canonical
        result = unique_json(path.with_name("result.json").read_bytes())
        need(set(result) == {"shard", "inventory_sha256", "discovery_only", "selected", "targets", "docs", "complete"},
             "exact shard result schema")
        shard = result["shard"]
        need(type(shard) is int and 0 <= shard < SHARDS and shard not in seen and result["complete"] is True
             and result["discovery_only"] is False and result["inventory_sha256"] == hashlib.sha256(raw).hexdigest()
             and result["docs"] is (shard == 0) and result["selected"] == manifest["shards"][shard],
             "one actual completed result for each shard; docs exactly once")
        seen.add(shard)
        selected = result["selected"]
        expected = {key: [name for kind, target, name in selected if (kind, target) == key] for key in roster}
        expected = {key: names for key, names in expected.items() if names}
        actual = {}
        for row in result["targets"]:
            need(set(row) == {"kind", "name", "selected", "passed", "ignored", "measured", "filtered"}, "exact target execution receipt")
            key = row["kind"], row["name"]
            need(key in expected and key not in actual and row["selected"] == expected[key], "exact selected target/name coverage")
            need(all(type(row[field]) is int and row[field] >= 0 for field in ("passed", "ignored", "measured", "filtered"))
                 and row["passed"] + row["ignored"] + row["measured"] == len(expected[key])
                 and row["ignored"] == len(set(expected[key]) & ignored[key])
                 and row["filtered"] == len(roster[key]) - len(expected[key]), "complete selected outcomes")
            actual[key] = row
        need(actual.keys() == expected.keys(), "no omitted nonempty target selection")
        executed.extend(tuple(case) for case in selected)
    universe = [tuple(case) for shard in common["shards"] for case in shard]
    need(seen == set(range(SHARDS)) and len(executed) == len(set(executed))
         and set(executed) == set(universe), "actual four-shard union is complete and disjoint")
    print(f"FINALIZER_SHARDS_COMPLETE {commit}: {len(executed)} cases, docs once", flush=True)


def cargo_target(key):
    kind, name = key
    need(kind in KINDS, "supported Cargo test selector")
    return ["cargo", "test", "--locked", "-p", PACKAGE, KINDS[kind], *([] if kind == "lib" else [name])]


def test_command(key, names):
    need(names and len(names) == len(set(names)) and all(name and not name.startswith("-") for name in names),
         "nonempty distinct exact filters; never run all on an empty selection")
    return cargo_target(key) + ["--", "--exact", "--format=pretty", "--color=never", *names]


def outcome(raw, cases, ignored, selected):
    summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
                           r"(\d+) measured; (\d+) filtered out; finished in [^\n]+$", raw, re.MULTILINE)
    need(len(summaries) == 1, "one successful exact-target libtest summary")
    passed, failed, skipped, measured, filtered = map(int, summaries[0])
    need(failed == 0 and passed + skipped + measured == len(selected)
         and skipped == len(set(selected) & ignored) and filtered == len(cases) - len(selected),
         "selected tests all accounted for with original ignored semantics")
    rows = re.findall(r"^test (.+) \.\.\. (ok|ignored(?:, [^\n]*)?|bench: [^\n]*)$", raw, re.MULTILINE)
    rows = [(name.removesuffix(" - should panic") if name not in selected else name, status)
            for name, status in rows]
    need(len(rows) == len(selected) and {name for name, _ in rows} == set(selected),
         "each exact selected name appeared once")
    need(all(status.startswith("ignored") == (name in ignored) for name, status in rows),
         "ignored tests were not executed or silently omitted")
    return dict(passed=passed, ignored=skipped, measured=measured, filtered=filtered)


class Commands:
    def __init__(self, output):
        self.output = output
        self.count = 0

    def run(self, label, command):
        self.count += 1
        directory = self.output / f"{self.count:03d}-{label}"
        directory.mkdir()
        print("finalizer shard:", " ".join(command), flush=True)
        with (directory / "stdout.log").open("xb") as stdout, (directory / "stderr.log").open("xb") as stderr:
            result = subprocess.run(command, cwd=ROOT, stdout=stdout, stderr=stderr, check=False)
        (directory / "command.json").write_text(json.dumps(dict(argv=command, status=result.returncode), indent=2) + "\n")
        for name in ("stdout.log", "stderr.log"):
            path = directory / name
            need(path.stat().st_size <= LIMIT, "bounded captured command output")
        raw = (directory / "stdout.log").read_text()
        diagnostic = (directory / "stderr.log").read_text()
        if result.returncode:
            print(raw[-8192:] + diagnostic[-8192:], file=sys.stderr, flush=True)
        need(result.returncode == 0, f"{label} failed with status {result.returncode}; complete logs in {directory}")
        return raw


def execute(shard, output, discover_only=False):
    need(type(shard) is int and 0 <= shard < SHARDS, "shard must be 0 through 3")
    need(output.is_absolute() and output.resolve() == output and not output.exists(), "fresh absolute output directory")
    output.mkdir(mode=0o700)
    commands = Commands(output)
    commit = commands.run("commit", ["git", "rev-parse", "HEAD"]).strip()
    need(re.fullmatch(r"[0-9a-f]{40}", commit), "exact source commit")
    metadata = unique_json(commands.run("metadata", ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"]))
    raw = commands.run("build", ["cargo", "test", "--locked", "-p", PACKAGE, "--no-run", "--message-format=json"])
    targets = inventory(metadata, [unique_json(line) for line in raw.splitlines()])
    roster, ignored = {}, {}
    for index, key in enumerate(sorted(targets)):
        roster[key] = listing(commands.run(f"list-{index}", cargo_target(key) + ["--", "--list", "--format=pretty"]))
        ignored_cases = listing(commands.run(f"ignored-{index}", cargo_target(key) + ["--", "--list", "--ignored", "--format=pretty"]))
        need(all(roster[key].get(name) == kind for name, kind in ignored_cases.items()), "ignored listing is an exact subset")
        ignored[key] = set(ignored_cases)
    shards = partition(roster)
    manifest = dict(schema=1, package=PACKAGE, commit=commit, shards=shards, targets=[dict(kind=key[0], name=key[1],
        executable=targets[key], cases=roster[key], ignored=sorted(ignored[key])) for key in sorted(targets)])
    encoded = json.dumps(manifest, sort_keys=True, indent=2) + "\n"
    validate_manifest(unique_json(encoded))
    (output / "inventory.json").write_text(encoded)
    report = dict(shard=shard, inventory_sha256=hashlib.sha256(encoded.encode()).hexdigest(),
                  discovery_only=discover_only, selected=shards[shard], targets=[], docs=False, complete=False)
    if not discover_only:
        for index, key in enumerate(sorted(targets)):
            selected = [name for kind, target, name in shards[shard] if (kind, target) == key]
            if not selected:
                continue
            raw = commands.run(f"run-{index}", test_command(key, selected))
            summary = outcome(raw, roster[key], ignored[key], selected)
            current = listing(commands.run(f"relist-{index}", cargo_target(key) + ["--", "--list", "--format=pretty"]))
            need(current == roster[key], "test names unchanged after exact execution")
            report["targets"].append(dict(kind=key[0], name=key[1], selected=selected, **summary))
        if shard == 0:
            commands.run("docs", ["cargo", "test", "--locked", "-p", PACKAGE, "--doc"])
            report["docs"] = True
    report["complete"] = True
    (output / "result.json").write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    mode = "DISCOVERED" if discover_only else "PASSED"
    print(f"FINALIZER_SHARD_{mode} {shard}/{SHARDS}: {len(shards[shard])} selected, "
          f"{sum(map(len, shards))} total, {len(targets)} targets", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--shard", type=int, choices=range(SHARDS))
    mode.add_argument("--aggregate", type=Path)
    parser.add_argument("--expected-commit")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--discover-only", action="store_true")
    args = parser.parse_args()
    try:
        if args.aggregate is not None:
            need(args.expected_commit and args.output is None and not args.discover_only, "aggregate arguments")
            aggregate(args.aggregate, args.expected_commit)
        else:
            need(args.output is not None and args.expected_commit is None, "shard arguments")
            execute(args.shard, args.output, args.discover_only)
    except (ShardError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"finalizer shard refused: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
