#!/usr/bin/env python3
"""Check complete CPU receipts and the unchanged selected source cohort."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
assert __debug__, "the reused strict harness parser requires assertions"
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
ENV = shlex.split("env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1")
FILTERS = ["constructed_generic_release", "constructed_directional_release", "constructed_primary_release", "primary_release::tests", "shared_memory::tests::pristine_abort"]
SOURCE = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"


def receipt(name, command, code=0):
    paths = {suffix: ARCHIVE / f"raw/{name}.{suffix}" for suffix in ("command", "started", "finished", "exit", "log")}
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), name
    assert paths["exit"].read_text() == f"{code}\n", name
    assert shlex.split(paths["command"].read_text()) == command, name
    stamps = [paths[key].read_text() for key in ("started", "finished")]
    assert all(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z\n", stamp) for stamp in stamps), name
    assert stamps[0] <= stamps[1], name
    return stamps


def main():
    assert not sys.argv[1:]
    assert hashlib.sha256((ROOT / SOURCE).read_bytes()).hexdigest() == "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953"
    assert hashlib.sha256(HELPER.read_bytes()).hexdigest() == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    spec = importlib.util.spec_from_file_location("strict_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    commands = {
        "source-before": ["python3", "-I", SOURCE],
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "clippy": ENV + ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"],
    }
    for target in ("gnu", "musl"):
        for crate in ("kfd", "runtime"):
            command = ENV + ["cargo", "test", "--frozen", "-p", f"fe2o3-{crate}", "--all-features"]
            if target == "musl":
                command += ["--target", "x86_64-unknown-linux-musl"]
            command += ["--lib"]
            if crate == "kfd":
                command += ["--", *FILTERS]
            commands[f"{target}-{crate}"] = command
    crates = ["-p", "fe2o3-kfd", "-p", "fe2o3-runtime"]
    commands.update({
        "no-default": ENV + ["cargo", "check", "--frozen", *crates, "--no-default-features"],
        "docs": ENV + ["cargo", "test", "--frozen", *crates, "--all-features", "--doc"],
        "unsafe-policy": ENV + ["cargo", "test", "--frozen", "-p", "cargo-fe2o3", "--test", "unsafe_source_policy"],
        "fmt": ["cargo", "fmt", "--all", "--check"],
        "diff": ["git", "diff", "--check"],
        "source-after": ["python3", "-I", SOURCE],
        "source-unchanged": ["cmp", str(ARCHIVE / "raw/source-before-final.log"), str(ARCHIVE / "raw/source-after-final.log")],
    })
    previous = ""
    for name in ("source-before", "rustc", "cargo", "gnu-kfd", "gnu-runtime", "musl-kfd", "musl-runtime", "clippy"):
        stamps = receipt(name, commands[name], 101 if name == "clippy" else 0)
        assert previous <= stamps[0], name
        previous = stamps[1]
    assert hashlib.sha256((ARCHIVE / "raw/clippy.log").read_bytes()).hexdigest() == "f47b833c60eb9287f9fa110cbcc5b9a9a45b69ffceac2aa07f4cdce24519c755", "reviewed test-local mem::replace diagnostics"
    for name, command in commands.items():
        stamps = receipt(f"{name}-final", command)
        assert previous <= stamps[0], name
        previous = stamps[1]

    rosters = {}
    rejected = 0
    for name, counts in {
        "gnu-kfd": (66, 0, 1351), "musl-kfd": (66, 0, 1351),
        "gnu-runtime": (1105, 20, 0), "musl-runtime": (1105, 20, 0),
        "unsafe-policy": (5, 1, 0),
    }.items():
        output = (ARCHIVE / f"raw/{name}-final.log").read_text()
        rosters[name] = helper.parse(output, *counts)
        if name != "unsafe-policy":
            assert helper.parse((ARCHIVE / f"raw/{name}.log").read_text(), *counts) == rosters[name], "preliminary and final named outcomes"
        for mutated in (
            output + "unexpected trailing payload\n",
            output.replace(" ... ok\n", " ... FAILED\n", 1),
            output.replace(" ... ok\n", " ... ok, unexplained payload\n", 1),
            output.replace("test result: ok.", "test result: FAILED."),
            output[:output.index("test result:")],
        ):
            assert mutated != output
            try:
                helper.parse(mutated, *counts)
            except (AssertionError, IndexError):
                rejected += 1
            else:
                raise AssertionError(f"corrupt harness accepted: {name}")
    for crate in ("kfd", "runtime"):
        assert rosters[f"gnu-{crate}"] == rosters[f"musl-{crate}"], crate
    assert sum("::generic_sdma_cases::" in name for name in rosters["gnu-kfd"]) == 5
    before = (ARCHIVE / "raw/source-before-final.log").read_bytes()
    assert before == (ARCHIVE / "raw/source-after-final.log").read_bytes()
    source = json.loads(before)
    assert source["base"] == "c38b8a23d3ad01ab9ceb2b2ad3fa9fedf8e5aad6"
    assert len(source["files"]) == 5555
    initial = json.loads((ARCHIVE / "raw/source-before.log").read_bytes())
    assert initial["base"] == source["base"]
    assert initial["files"].keys() == source["files"].keys()
    assert {name for name in source["files"] if initial["files"][name] != source["files"][name]} == {
        "crates/fe2o3-kfd/src/sdma/retained_release/generic_fixture.rs"
    }, "only the test-local escrow idiom changed between attempts"
    current = json.loads(subprocess.check_output(["python3", "-I", SOURCE], cwd=ROOT))
    assert source["files"] == current["files"], "source changed after qualification"
    print(json.dumps({
        "source_files": len(source["files"]),
        "kfd_passed_per_target": 66, "kfd_filtered_per_target": 1351,
        "runtime_passed_per_target": 1105, "runtime_ignored_per_target": 20,
        "rejected_harness_mutations": rejected,
        "scope": "CPU development checks; no native, binary-closure, performance or formal-refinement qualification",
    }, sort_keys=True))


if __name__ == "__main__":
    main()
