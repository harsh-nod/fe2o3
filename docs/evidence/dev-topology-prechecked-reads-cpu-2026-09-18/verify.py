#!/usr/bin/env python3
"""Isolated acceptance of the frozen CPU-only prechecked topology-read run."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run verification with python3 -I")

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
EXECUTION_ROOT = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
PREFIX = "docs/evidence/dev-topology-prechecked-reads-cpu-2026-09-18/"
PRIOR = "docs/evidence/dev-xgmi-host-attribution-cpu-2026-09-18/"
BASE = "fa9413425531458f9ac6b1ceb41d96912a9ae2d4"
SOURCE_SHA = "9cf73b317f1e2bc3a357d6f8f156f1792cdface87b0ab8ec94b12eb3e79a0a2b"
SOURCE_FILES = 5573
DOC = "docs/runtime-topology-prechecked-reads-v1.md"
DOC_SHA = "3cbd9cdbb429e6437058ba8dbfe69b835ab2cfa3283aec51dc0361bffe3c68b1"
ROSTERS = {
    "kfd": (162, "de6cf9ea663771cda05626b3aa62493b28e669db4532d61b075d39175e6f5592"),
    "runtime": (30, "2858b60801386a36bd09a18f0423d6952cbee7bf32f072a9490320ead43f8969"),
}
TOOLS = {
    PRIOR
    + "qualify.py": "43397a7a269a21dd317ba0e0d8137943800441f6e17eacbeee4e3766c684e7a3",
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py": "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
    "docs/evidence/dev-xgmi-pair-currentness-cpu-2026-09-18/qualify.py": "06f5d1e8ba5f8e8afee3c7a3dce2373a64b9c1cb9621314613ef2eaa7da38686",
    PREFIX
    + "qualify.py": "77b55ec25a4a779053812ff9bd691f1c0a4081f8dc8b771f359cec2e4b385a4b",
}
ACCEPT = PRIOR + "accept.py"
ACCEPT_SHA = "be5998bbd3bde933c48d4ae3d6e12ae8231da62cffd254abdb7c7ebe3f7dc8bd"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


# Reuse the separately sealed receipt/roster parsers, never the old packet's
# acceptance policy. Authenticate the helper before executing any of its code.
need(sha(ROOT / ACCEPT) == ACCEPT_SHA, "pinned acceptance helper")
spec = importlib.util.spec_from_file_location("retirement_cpu_parsers", ROOT / ACCEPT)
A = importlib.util.module_from_spec(spec)
spec.loader.exec_module(A)
# Receipts bind the original executor, not the checkout running this verifier.
A.ROOT = EXECUTION_ROOT
read = A.read
unsafe_policy = A.unsafe_policy
seal = A.seal


def authenticated_commands():
    for name, digest in TOOLS.items():
        need(sha(ROOT / name) == digest, "pinned helper: " + name)
    spec = importlib.util.spec_from_file_location(
        "retirement_cpu_driver", HERE / "qualify.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.Q.HERE = EXECUTION_ROOT / PREFIX
    module.Q.SELECTOR = EXECUTION_ROOT / module.SELECTOR.relative_to(ROOT)
    return module.commands(), module.SELECTOR


def verify_bundle(archive=HERE, live=False):
    commands, selector = authenticated_commands()
    need(sha(ROOT / DOC) == DOC_SHA, "separately bound descriptive document")
    files, directories = A.inventory(archive)
    top = {"qualify.py", "verify.py", "test_verify.py", "README.md", "tools.json"}
    expected = top | {
        "raw/" + name + "/" + item
        for name, _, _ in commands
        for item in ("receipt.json", "stdout", "stderr")
    }
    need(set(files) - {"SHA256SUMS"} == expected, "exact archive file roster")
    need(
        directories == {"raw"} | {"raw/" + name for name, _, _ in commands},
        "exact archive directory roster",
    )
    need(read(archive / "tools.json") == TOOLS, "exact frozen run tools")
    need(
        sha(archive / "qualify.py") == TOOLS[PREFIX + "qualify.py"],
        "frozen driver copy",
    )
    previous = 0
    rosters = {}
    for name, command, seconds in commands:
        folder = archive / "raw" / name
        previous = A.receipt(folder, command, seconds, previous)
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
            A.passing_tests(output, rosters[tuple(name.split("-"))])
        elif name in ("gnu-example", "musl-example", "example-feature-off"):
            A.passing_tests(output, A.EXAMPLE_TESTS)
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
        "exact Rust toolchain",
    )
    need(
        (archive / "raw/cargo/stdout").read_text()
        == "cargo 1.96.0-nightly (888f67534 2026-03-30)\n",
        "exact Cargo toolchain",
    )
    before, after = (
        archive / "raw/source-before/stdout",
        archive / "raw/source-after/stdout",
    )
    need(sha(before) == sha(after) == SOURCE_SHA, "exact unchanged source snapshot")
    source = read(before)
    need(
        source["base"] == BASE and len(source["files"]) == SOURCE_FILES,
        "exact source base/cohort",
    )
    for name in (
        "crates/fe2o3-kfd/src/topology.rs",
        "crates/fe2o3-kfd/src/topology/tests/prechecked_reads.rs",
        "crates/fe2o3-kfd/src/currentness/full.rs",
        "crates/fe2o3-kfd/src/currentness/full/tests.rs",
        "crates/fe2o3-kfd/src/shared_memory/pair_currentness.rs",
        "crates/fe2o3-kfd/src/shared_memory/pair_currentness/tests.rs",
        "crates/fe2o3-kfd/src/sdma/owner_release.rs",
        "crates/fe2o3-kfd/src/sdma/xgmi_retirement.rs",
        "crates/fe2o3-kfd/src/sdma/xgmi_retirement/tests.rs",
        "crates/fe2o3-runtime/src/kfd_backend/tests/native_xgmi_retirement_tests.rs",
    ):
        need(
            name in source["files"],
            "required prechecked/currentness/retirement source included",
        )
    if live:
        current = json.loads(
            subprocess.check_output(["python3", "-I", str(selector)], cwd=ROOT),
            object_pairs_hook=A.object_pairs,
        )
        need(current["files"] == source["files"], "live qualified source equality")
    unsafe_policy((archive / "raw/unsafe-source/stdout").read_text())
    return {
        "cpu_qualification": True,
        "source_base": BASE,
        "source_snapshot_sha256": SOURCE_SHA,
        "source_files": SOURCE_FILES,
        "commands": len(commands),
        "native_execution": False,
        "formal_refinement": False,
        "performance_acceptance": False,
    }


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
