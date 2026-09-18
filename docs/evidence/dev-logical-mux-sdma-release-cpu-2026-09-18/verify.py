#!/usr/bin/env python3
"""Audit exact CPU receipts and source identity without executing archived commands."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


HELPER = ROOT / "docs/evidence/dev-combined-sdma-release-cpu-2026-09-18/verify.py"
need(sha(HELPER) == "42b19f24bd001bb259107013622ffde4a7f2c0d5fae44f7654e97b9797373452", "pinned receipt helper")
spec = importlib.util.spec_from_file_location("combined_cpu_receipts", HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
helper.ARCHIVE = ARCHIVE
FILTERS = ["queue::live::construction_primary::integration_tests::release_cases::", "queue::live::primary_release::tests::", "sdma_cleanup"]
ROSTER_SHA = "4918ab953124878a399a9f52a65bff5246b2152770f43cfc050ed269ba5ee49b"
UNSAFE = dict.fromkeys([
    "additions_removals_and_moves_require_baseline_review",
    "all_constructs_and_macro_templates_are_counted",
    "comments_literals_and_documentation_do_not_count",
    "malformed_tokens_fail_closed",
    "unsafe_source_matches_reviewed_inventory",
], "ok") | {"refresh_reviewed_unsafe_inventory": "ignored"}


def parse_roster(text):
    lines = text.splitlines()
    while lines and not lines[0].endswith(": test"):
        line = lines.pop(0)
        need(not line.strip() or re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|     Running unittests .+", line
        ), "roster prelude")
    need(len(lines) == 138 and lines[-2:] == ["", "136 tests, 0 benchmarks"], "complete roster")
    need(all(re.fullmatch(r"\S+: test", line) for line in lines[:-2]), "roster rows")
    names = [line.removesuffix(": test") for line in lines[:-2]]
    need(names == sorted(set(names)), "unique sorted roster")
    need(hashlib.sha256(("\n".join(names) + "\n").encode()).hexdigest() == ROSTER_SHA, "exact named roster")
    return set(names)


def commands():
    specs = helper.commands()
    env = specs["clippy"][:specs["clippy"].index("cargo")]
    specs["clippy"] = env + ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"]
    specs["no-default"] = env + ["cargo", "check", "--frozen", "-p", "fe2o3-kfd", "-p", "fe2o3-runtime", "--no-default-features"]
    for target in ("gnu", "musl"):
        base = env + ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", "--all-features"]
        if target == "musl":
            base += ["--target", "x86_64-unknown-linux-musl"]
        specs[target + "-roster"] = base + ["--lib", "--", "--list"] + FILTERS
        specs[target + "-release"] = base + ["--lib", "--"] + FILTERS
    specs["unsafe-source"] = env + ["cargo", "test", "--frozen", "-p", "cargo-fe2o3", "--test", "unsafe_source_policy"]
    order = ["source-before", "rustc", "cargo", "clippy", "no-default", "gnu-roster", "gnu-release", "musl-roster", "musl-release", "runtime", "unsafe-source", "fmt", "diff", "source-after", "source-unchanged", "calibration"]
    return {name: specs[name] for name in order}


def qualify(live=False):
    need(sha(helper.SOURCE) == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953", "pinned source selector")
    last = None
    for name, command in commands().items():
        start, end = helper.receipt(name, command)
        need(last is None or last <= start, "qualification chronology")
        last = end
    for target in ("gnu", "musl"):
        names = parse_roster((ARCHIVE / f"raw/{target}-roster.log").read_text())
        helper.parse_harness((ARCHIVE / f"raw/{target}-release.log").read_text(), names, 1302)
    helper.parse_harness((ARCHIVE / "raw/runtime.log").read_text(), helper.RUNTIME, 1121)
    # The pinned parser checks ignored outcomes explicitly for the maintenance-only test.
    parser_spec = importlib.util.spec_from_file_location("strict_harness", helper.PARSER)
    parser = importlib.util.module_from_spec(parser_spec)
    parser_spec.loader.exec_module(parser)
    need(parser.parse((ARCHIVE / "raw/unsafe-source.log").read_text(), 5, 1) == UNSAFE, "exact unsafe-policy outcomes")
    for name in ("fmt", "diff", "source-unchanged"):
        need((ARCHIVE / f"raw/{name}.log").read_bytes() == b"", "empty successful check")
    before = (ARCHIVE / "raw/source-before.log").read_bytes()
    need(before == (ARCHIVE / "raw/source-after.log").read_bytes(), "unchanged complete source")
    source = json.loads(before)
    need(source["base"] == "27843725eda2f13198b865643b7739a89433e501" and len(source["files"]) == 5558, "source base and count")
    need(re.fullmatch(r"\.{5}\n-+\nRan 5 tests in [0-9.]+s\n\nOK\n", (ARCHIVE / "raw/calibration.log").read_text()), "complete calibration harness")
    if live:
        current = json.loads(subprocess.check_output(["python3", "-I", str(helper.SOURCE)], text=True))
        need(current["files"] == source["files"], "live source identity")
    return {"kfd_tests_per_target": 136, "gnu_runtime_smoke_tests": 4,
            "unsafe_policy_passed": 5, "unsafe_maintenance_ignored": 1,
            "source_files": 5558, "native_execution": False, "formal_refinement": False}


def manifest():
    paths = {}
    for path in sorted(ARCHIVE.rglob("*")):
        need(not path.is_symlink(), "ordinary archive paths")
        if path.is_file() and path != ARCHIVE / "SHA256SUMS":
            paths[path.relative_to(ARCHIVE).as_posix()] = sha(path)
    expected = {".gitattributes", "README.md", "record.sh", "qualify.sh", "verify.py", "test_verify.py"}
    expected.update(f"raw/{name}.{suffix}" for name in commands()
                    for suffix in ("command", "exit", "started", "finished", "log"))
    need(set(paths) == expected, "exact archive membership")
    return "".join(f"{digest}  {name}\n" for name, digest in paths.items())


def verify_seal(seal, expected, create=False, allow_absent=False):
    if create:
        with seal.open("x") as stream:
            stream.write(expected)
    elif seal.exists():
        need(seal.read_text() == expected, "exact sealed archive")
    else:
        need(allow_absent, "missing archive seal")


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    result = qualify(args.live)
    expected = manifest()
    seal = ARCHIVE / "SHA256SUMS"
    verify_seal(seal, expected, args.seal, args.allow_unsealed)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
