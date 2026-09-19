#!/usr/bin/env python3
"""Record and verify bounded CPU coverage for XGMI peer hot controls."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import signal
import subprocess
import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("run with python3 -I -B")

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
NATIVE = "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
SELECTOR = "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
HELPERS = {
    NATIVE: "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
    SELECTOR: "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
}
STAGES = tuple("""source-before rustc cargo common-tests argument-tests gnu-example
musl-example feature-off-example fmt source-after""".split())
ENV = """env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never
RUST_TEST_THREADS=2 CARGO_PROFILE_TEST_OPT_LEVEL=1
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true""".split()
EXAMPLE = "gfx942-runtime-xgmi-peer-benchmark"
SOURCE_FILE = "crates/fe2o3-runtime/examples/" + EXAMPLE + ".rs"
RUST_TESTS = set(
    """tests::aggregate_and_ordinary_depth_bounds_are_distinct
tests::aggregate_classification_includes_both_aggregate_modes
tests::aggregate_hot_only_requires_depth_one tests::canaries_bind_the_exact_inner_copy_region
tests::diagnostic_submission_roster_is_bounded_before_native_open
tests::existing_modes_preserve_both_phases_and_report_schemas
tests::patterns_distinguish_round_slot_and_direction tests::percentile_uses_nearest_rank
tests::progress_flags_are_explicit_and_mutually_exclusive_before_native_open""".split()
)
COMMON_TESTS = set(
    """test_both_comparators_use_shared_hot_lifecycle
test_checked_controls_guard_mutations_and_exact_lifecycle
test_undefined_behavior_sanitizer""".split()
)
ARGUMENT_TESTS = set(
    """test_legacy_comparators_use_checked_arguments
test_strict_parsing_and_checked_arithmetic""".split()
)


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


for name, digest in HELPERS.items():
    need(sha(ROOT / name) == digest, "unauthenticated helper: " + name)
spec = importlib.util.spec_from_file_location("peer_hot_recorder", ROOT / NATIVE)
N = importlib.util.module_from_spec(spec)
spec.loader.exec_module(N)


def commands(root):
    root = Path(root)
    python = ["python3", "-I", "-B"]
    common = "benchmarks/runtime_gfx942/test_xgmi_peer_benchmark_common.py"
    arguments = "benchmarks/runtime_gfx942/test_native_benchmark_args.py"
    cargo = ["cargo", "test", "--frozen", "-p", "fe2o3-runtime"]
    example = ["--example", EXAMPLE]
    return [
        ("source-before", python + [str(root / SELECTOR)], 60),
        ("rustc", ["rustc", "-vV"], 30),
        ("cargo", ["cargo", "-V"], 30),
        ("common-tests", python + [common, "-v"], 300),
        ("argument-tests", python + [arguments, "-v"], 300),
        ("gnu-example", ENV + cargo + ["--all-features"] + example, 1200),
        (
            "musl-example",
            ENV
            + cargo
            + ["--all-features", "--target", "x86_64-unknown-linux-musl"]
            + example,
            1200,
        ),
        (
            "feature-off-example",
            ENV + cargo + ["--no-default-features"] + example,
            1200,
        ),
        ("fmt", ["rustfmt", "--edition", "2024", "--check", SOURCE_FILE], 180),
        ("source-after", python + [str(root / SELECTOR)], 60),
    ]


def source_snapshot(data):
    need(set(data) == {"base", "files"}, "source snapshot keys")
    need(
        isinstance(data["base"], str) and re.fullmatch(r"[0-9a-f]{40}", data["base"]),
        "source base",
    )
    files = data["files"]
    need(type(files) is dict and files, "source file map")
    for name, digest in files.items():
        need(
            type(name) is str
            and name
            and not name.startswith("/")
            and ".." not in name.split("/"),
            "relative source path",
        )
        need(
            type(digest) is str and re.fullmatch(r"[0-9a-f]{64}", digest),
            "source digest",
        )
    required = """benchmarks/runtime_gfx942/xgmi_peer_hip.cpp
benchmarks/runtime_gfx942/xgmi_peer_hsa.cpp benchmarks/runtime_gfx942/xgmi_peer_benchmark_common.hpp
benchmarks/runtime_gfx942/xgmi_peer_benchmark_common_test.cpp
benchmarks/runtime_gfx942/test_xgmi_peer_benchmark_common.py""".split()
    for name in [SOURCE_FILE, *required]:
        need(name in files, "required source: " + name)
    return data


def receipt(archive, stage, expected_command, timeout, previous):
    folder = archive / "raw" / stage
    row = json.loads((folder / "receipt.json").read_text())
    keys = """command cwd started_ns timeout_seconds pid exit error group_absent
environment stdin_sha256 finished_ns stdout_sha256 stderr_sha256""".split()
    need(set(row) == set(keys), "receipt keys: " + stage)
    need(
        row["command"] == expected_command and row["timeout_seconds"] == timeout,
        "exact command: " + stage,
    )
    need(type(row["cwd"]) is str and row["cwd"], "receipt cwd")
    need(type(row["started_ns"]) is int and row["started_ns"] > 0, "start time")
    need(type(row["finished_ns"]) is int and row["finished_ns"] >= row["started_ns"], "finish time")
    need(type(row["pid"]) is int and row["pid"] > 0, "pid")
    need(row["exit"] == 0 and type(row["exit"]) is int, "zero exit: " + stage)
    need(row["error"] is None and row["group_absent"] is True, "closed stage: " + stage)
    need(
        row["environment"] is None and row["stdin_sha256"] is None,
        "ambient/stdin: " + stage,
    )
    need(previous is None or row["started_ns"] >= previous, "ordered stages")
    for stream in ("stdout", "stderr"):
        need(row[stream + "_sha256"] == sha(folder / stream), "stream hash: " + stage)
    return row


def python_tests(stderr, expected):
    names = set(re.findall(r"^(test_[a-z0-9_]+) \([^\n]+\) \.\.\. ok$", stderr, re.M))
    need(names == expected and f"Ran {len(expected)} tests" in stderr
         and re.search(r"^OK$", stderr, re.M), "exact successful Python test roster")


def rust_tests(stdout):
    names = set(re.findall(r"^test (tests::[a-z0-9_]+) \.\.\. ok$", stdout, re.M))
    need(names == RUST_TESTS, "exact Rust example test roster")
    need("9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out" in stdout,
         "exact Rust example result")


def derive(archive=HERE, binding=None, live=False):
    archive = Path(archive)
    execution_root = Path(binding["execution_root"]) if binding else ROOT
    expected = commands(execution_root)
    previous = None
    cwd = None
    rows = {}
    for stage, command, timeout in expected:
        row = receipt(archive, stage, command, timeout, previous)
        cwd = row["cwd"] if cwd is None else cwd
        need(row["cwd"] == cwd == str(execution_root), "one execution root")
        previous, rows[stage] = row["finished_ns"], row
    before = archive / "raw/source-before/stdout"
    after = archive / "raw/source-after/stdout"
    need(before.read_bytes() == after.read_bytes(), "source snapshot unchanged")
    source = source_snapshot(json.loads(before.read_text()))
    python_tests((archive / "raw/common-tests/stderr").read_text(), COMMON_TESTS)
    python_tests((archive / "raw/argument-tests/stderr").read_text(), ARGUMENT_TESTS)
    for stage in ("gnu-example", "musl-example", "feature-off-example"):
        rust_tests((archive / "raw" / stage / "stdout").read_text())
    result = {
        "schema": "fe2o3.xgmi-peer-hot-controls-cpu.v1",
        "commands": len(STAGES),
        "execution_root": str(execution_root),
        "source_base": source["base"],
        "source_files": source["files"],
        "source_snapshot_sha256": sha(before),
        "toolchain": {
            "rustc_stdout_sha256": rows["rustc"]["stdout_sha256"],
            "cargo_stdout_sha256": rows["cargo"]["stdout_sha256"],
        },
        "tests": {"rust_each_configuration": 9, "common": 3, "arguments": 2},
        "tools": dict(HELPERS, **{
            "docs/evidence/dev-xgmi-peer-hot-controls-cpu-2026-09-19/" + name:
                sha(archive / name) for name in ("cpu.py", "test_cpu.py")}),
    }
    if binding is not None:
        need(type(binding) is dict and binding == result, "exact binding")
    if live:
        output = subprocess.check_output(["python3", "-I", "-B", str(ROOT / SELECTOR)], cwd=ROOT)
        need(source_snapshot(json.loads(output))["files"] == source["files"],
             "live selected source map")
    return result


def expected_paths():
    files = {"cpu.py", "test_cpu.py", "binding.json"}
    directories = {"raw"}
    for stage in STAGES:
        directories.add("raw/" + stage)
        files |= {
            f"raw/{stage}/{name}" for name in ("receipt.json", "stdout", "stderr")
        }
    return files, directories


def inventory(archive, sealed):
    files, directories = set(), set()
    for path in Path(archive).rglob("*"):
        need(not path.is_symlink(), "no archive symlinks")
        relative = path.relative_to(archive).as_posix()
        (directories if path.is_dir() else files).add(relative)
        need(path.is_dir() or path.is_file(), "ordinary archive node")
    expected_files, expected_directories = expected_paths()
    if sealed:
        expected_files.add("SHA256SUMS")
    need(files == expected_files and directories == expected_directories, "exact archive closure")


def seal(archive=HERE, create=False):
    archive = Path(archive)
    if create:
        inventory(archive, False)
        lines = [f"{sha(archive / name)}  {name}\n" for name in sorted(expected_paths()[0])]
        (archive / "SHA256SUMS").open("x", encoding="utf-8").writelines(lines)
    inventory(archive, True)
    lines = (archive / "SHA256SUMS").read_text().splitlines()
    parsed = {}
    for line in lines:
        match = re.fullmatch(r"([0-9a-f]{64})  ([^\n]+)", line)
        need(match is not None and match[2] not in parsed, "seal syntax")
        parsed[match[2]] = match[1]
    need(set(parsed) == expected_paths()[0], "seal roster")
    for name, digest in parsed.items():
        need(sha(archive / name) == digest, "sealed digest: " + name)


def verify(archive=HERE, live=False, allow_unsealed=False):
    archive = Path(archive)
    binding = json.loads((archive / "binding.json").read_text())
    report = derive(archive, binding=binding, live=live)
    if not allow_unsealed or (archive / "SHA256SUMS").exists():
        seal(archive)
    else:
        inventory(archive, False)
    return report


def record():
    recorder = N.Recorder(HERE / "raw", ROOT)
    for number in N.MANAGED:
        signal.signal(number, N.interrupted)
    for name, command, timeout in commands(ROOT):
        recorder.run(name, command, timeout)
    N.write_json(HERE / "binding.json", derive(HERE))
    verify(HERE, live=True, allow_unsealed=True)
    seal(HERE, create=True)
    return verify(HERE, live=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", action="store_true")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--allow-unsealed", action="store_true")
    args = parser.parse_args()
    need(not (args.record and (args.live or args.allow_unsealed)), "record has fixed verification")
    report = record() if args.record else verify(HERE, args.live, args.allow_unsealed)
    print(json.dumps({"schema": report["schema"], "commands": report["commands"],
                      "source_files": len(report["source_files"]), "verified": True}, sort_keys=True))


if __name__ == "__main__":
    main()
