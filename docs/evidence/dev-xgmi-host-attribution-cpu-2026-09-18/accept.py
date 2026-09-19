#!/usr/bin/env python3
"""Authoritative, isolated offline acceptance for this CPU-only evidence packet."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run this verifier with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = "8b1ab89f48dd9dfedefef2c4b9c344e97e2c1e65"
SOURCE_SHA = "c25a9d1fb3046510f06034b414be379bcaa702a1167ef044f68308e8749cff3a"
DOC = "docs/runtime-xgmi-copy-diagnostics-v1.md"
DOC_SHA = "7bee4fa3ab0ad891077c1c07b61ee96eead24b5022330ea2a2ff0bc8324ee78b"
PREFIX = "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/"
TOOLS = {
    "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py": "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
    PREFIX
    + "qualify.py": "43397a7a269a21dd317ba0e0d8137943800441f6e17eacbeee4e3766c684e7a3",
    PREFIX
    + "verify.py": "32ac95aef382e2fa613010c4f9f89f41caa723b3e151bf2485b5029a0f48a0c4",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
}
ROSTERS = {
    "kfd": (34, "e1756b2ed4e82270cd25dcd7cbd536f93c0846c2500aecb08adec4207edfcd64"),
    "runtime": (25, "064beb6f88546f62839265f36a662d447eaad84863a2872d3a1b3236f0b578f6"),
}
EXAMPLE_TESTS = sorted(
    [
        "tests::canaries_bind_the_exact_inner_copy_region",
        "tests::diagnostic_controls_are_explicit_and_bounded_before_native_open",
        "tests::patterns_distinguish_round_slot_and_direction",
        "tests::percentile_uses_nearest_rank",
    ]
)
RECEIPT_KEYS = {
    "command",
    "cwd",
    "started_ns",
    "timeout_seconds",
    "pid",
    "exit",
    "error",
    "group_absent",
    "environment",
    "stdin_sha256",
    "finished_ns",
    "stdout_sha256",
    "stderr_sha256",
}


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def object_pairs(pairs):
    value = {}
    for key, item in pairs:
        need(key not in value, "duplicate JSON key")
        value[key] = item
    return value


def read(path):
    need(path.is_file() and not path.is_symlink(), "ordinary JSON file")
    return json.loads(path.read_text(), object_pairs_hook=object_pairs)


def authenticated_commands():
    # No repository helper executes before every transitive local helper is pinned.
    for name, digest in TOOLS.items():
        need(sha(ROOT / name) == digest, "pinned helper: " + name)
    spec = importlib.util.spec_from_file_location(
        "qualified_xgmi_commands", HERE / "qualify.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.commands(), module.SELECTOR


def inventory(archive):
    need(archive.is_dir() and not archive.is_symlink(), "ordinary archive directory")
    files, directories = {}, set()
    for path in sorted(archive.rglob("*")):
        need(not path.is_symlink(), "no archive symlinks")
        name = path.relative_to(archive).as_posix()
        if path.is_dir():
            directories.add(name)
        else:
            files[name] = sha(path)
    return files, directories


def receipt(folder, command, seconds, previous):
    row = read(folder / "receipt.json")
    need(set(row) == RECEIPT_KEYS, "exact receipt keyset")
    need(
        row["command"] == command
        and row["cwd"] == str(ROOT)
        and type(row["timeout_seconds"]) is int
        and row["timeout_seconds"] == seconds
        and row["environment"] is None
        and row["stdin_sha256"] is None
        and type(row["exit"]) is int
        and row["exit"] == 0
        and row["error"] is None
        and row["group_absent"] is True
        and type(row["pid"]) is int
        and row["pid"] > 1
        and type(row["started_ns"]) is int
        and type(row["finished_ns"]) is int
        and previous <= row["started_ns"] < row["finished_ns"],
        "successful exact chronological receipt",
    )
    for stream in ("stdout", "stderr"):
        need(sha(folder / stream) == row[stream + "_sha256"], "output digest")
    return row["finished_ns"]


def passing_tests(output, expected):
    names = re.findall(r"^test (\S+) \.\.\. ok$", output, re.MULTILINE)
    need(
        len(names) == len(set(names)) and sorted(names) == expected,
        "exact passing test identities",
    )
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        output,
        re.MULTILINE,
    )
    need(summaries == [(str(len(expected)), "0", "0")], "one exact harness closure")


def unsafe_policy(output):
    expected = sorted(
        [
            "additions_removals_and_moves_require_baseline_review",
            "all_constructs_and_macro_templates_are_counted",
            "comments_literals_and_documentation_do_not_count",
            "malformed_tokens_fail_closed",
            "unsafe_source_matches_reviewed_inventory",
        ]
    )
    names = re.findall(r"^test (\S+) \.\.\. ok$", output, re.MULTILINE)
    ignored = re.findall(r"^test (\S+) \.\.\. ignored, (.+)$", output, re.MULTILINE)
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        output,
        re.MULTILINE,
    )
    need(
        sorted(names) == expected
        and len(names) == len(set(names))
        and ignored
        == [
            (
                "refresh_reviewed_unsafe_inventory",
                "explicit maintenance command; review the resulting baseline diff",
            )
        ]
        and summaries == [("5", "0", "1")],
        "exact unsafe-policy names/statuses/closure",
    )


def verify_bundle(archive=HERE, live=False):
    commands, selector = authenticated_commands()
    files, directories = inventory(archive)
    top = {
        "qualify.py",
        "verify.py",
        "accept.py",
        "test_accept.py",
        "README.md",
        "tools.json",
    }
    expected_files = top | {
        "raw/" + name + "/" + item
        for name, _, _ in commands
        for item in ("receipt.json", "stdout", "stderr")
    }
    need(set(files) - {"SHA256SUMS"} == expected_files, "exact archive file roster")
    need(
        directories == {"raw"} | {"raw/" + name for name, _, _ in commands},
        "exact archive directory roster",
    )
    need(read(archive / "tools.json") == TOOLS, "exact frozen run tools")
    for name in ("qualify.py", "verify.py"):
        need(sha(archive / name) == TOOLS[PREFIX + name], "frozen launch tool copy")
    need(sha(ROOT / DOC) == DOC_SHA, "separately bound descriptive document")
    previous = 0
    rosters = {}
    for name, command, seconds in commands:
        folder = archive / "raw" / name
        previous = receipt(folder, command, seconds, previous)
        output = (folder / "stdout").read_text()
        if name.endswith("-roster"):
            target, package, _ = name.split("-")
            names = re.findall(r"^(\S+): test$", output, re.MULTILINE)
            count, digest = ROSTERS[package]
            canonical = "\n".join(sorted(names)) + "\n"
            need(
                len(names) == len(set(names)) == count
                and hashlib.sha256(canonical.encode()).hexdigest() == digest
                and output.endswith(f"{count} tests, 0 benchmarks\n"),
                "exact pinned test roster",
            )
            rosters[(target, package)] = sorted(names)
        elif name in ("gnu-kfd", "musl-kfd", "gnu-runtime", "musl-runtime"):
            passing_tests(output, rosters[tuple(name.split("-"))])
        elif name in ("gnu-example", "musl-example", "example-feature-off"):
            passing_tests(output, EXAMPLE_TESTS)
    for package in ROSTERS:
        need(
            rosters[("gnu", package)] == rosters[("musl", package)],
            "identical GNU/musl test membership",
        )
    need(
        (archive / "raw/rustc/stdout").read_text()
        == (
            "rustc 1.96.0-nightly (55e86c996 2026-04-02)\n"
            "binary: rustc\ncommit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9\n"
            "commit-date: 2026-04-02\nhost: x86_64-unknown-linux-gnu\n"
            "release: 1.96.0-nightly\nLLVM version: 22.1.2\n"
        ),
        "exact reported Rust toolchain",
    )
    need(
        (archive / "raw/cargo/stdout").read_text()
        == "cargo 1.96.0-nightly (888f67534 2026-03-30)\n",
        "exact reported Cargo toolchain",
    )
    before = archive / "raw/source-before/stdout"
    after = archive / "raw/source-after/stdout"
    need(sha(before) == sha(after) == SOURCE_SHA, "exact unchanged source snapshot")
    source = read(before)
    need(
        source["base"] == BASE and len(source["files"]) == 5564,
        "exact source base/cohort",
    )
    for name in (
        "crates/fe2o3-kfd/src/sdma/xgmi_diagnostic.rs",
        "crates/fe2o3-runtime/src/kfd_backend/xgmi_diagnostic.rs",
    ):
        need(name in source["files"], "diagnostic module included")
    if live:
        current = json.loads(
            subprocess.check_output(["python3", "-I", str(selector)], cwd=ROOT),
            object_pairs_hook=object_pairs,
        )
        need(current["files"] == source["files"], "live qualified source equality")
    policy = (archive / "raw/unsafe-source/stdout").read_text()
    unsafe_policy(policy)
    return {
        "cpu_qualification": True,
        "source_base": BASE,
        "source_snapshot_sha256": SOURCE_SHA,
        "source_files": 5564,
        "commands": len(commands),
        "native_execution": False,
        "formal_refinement": False,
        "performance_acceptance": False,
    }


def seal(archive, create):
    files, _ = inventory(archive)
    files.pop("SHA256SUMS", None)
    manifest = "".join(digest + "  " + name + "\n" for name, digest in files.items())
    path = archive / "SHA256SUMS"
    if create:
        with path.open("x") as output:
            output.write(manifest)
    else:
        need(path.read_text() == manifest, "complete archive seal")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    report = verify_bundle(live=args.live)
    if not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
        seal(HERE, args.seal)
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
