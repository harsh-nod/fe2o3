#!/usr/bin/env python3
"""Bounded, source-bracketed CPU qualification of matched hot batch producers."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys


REPO = Path(__file__).resolve().parents[3]
BASE_PACKET = REPO / "docs/evidence/dev-topology-link-scratch-cpu-2026-09-24/raw/cpu1"
EXAMPLE = "gfx942-runtime-xgmi-peer-benchmark"
EXAMPLE_PATH = f"crates/fe2o3-runtime/examples/{EXAMPLE}.rs"
PREFIX = "benchmarks/runtime_gfx942/"
ALLOWED_DELTA = {EXAMPLE_PATH} | {PREFIX + name for name in (
    "xgmi_peer_benchmark_common.hpp", "xgmi_peer_benchmark_common_test.cpp",
    "test_xgmi_peer_benchmark_common.py", "xgmi_peer_hip.cpp", "xgmi_peer_hsa.cpp",
    "xgmi_peer_hot_results.py", "test_xgmi_peer_hot_results.py",
    "xgmi_peer_hot_callbacks_test.cpp", "test_xgmi_peer_hot_callbacks.py",
)}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    output = Path(sys.argv[1]).resolve()
    output.mkdir()
    target = output / "target"
    target.mkdir()
    os.chdir(REPO)
    spec = importlib.util.spec_from_file_location(
        "owner_check", REPO / "crates/fe2o3-runtime-model/verus/check-owner-lifecycle.py"
    )
    check = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = check
    spec.loader.exec_module(check)
    base = check.dependencies(REPO)["base"]
    for number in base.SIGNALS:
        signal.signal(number, base.interrupted)

    def inputs():
        paths = subprocess.check_output([
            "/usr/bin/git", "ls-files", "--cached", "--others", "--exclude-standard", "-z",
            "crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", PREFIX,
        ], env=check.git_environment()).decode().split("\0")
        paths = sorted(set(path for path in paths if path and (
            path.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in path
        )))
        return {"runner": digest(Path(__file__)), "source": {path: digest(REPO / path) for path in paths}}

    before = inputs()
    (output / "inputs-before.json").write_text(json.dumps(before, indent=2) + "\n")
    old = json.loads((BASE_PACKET / "inputs-before.json").read_text())["source"]
    changed = sorted(path for path in set(old) | set(before["source"])
                     if old.get(path) != before["source"].get(path))
    check.need(set(changed) == ALLOWED_DELTA, "unexpected delta from prior production CPU qualification")
    (output / "prior-qualification-delta.json").write_text(json.dumps({
        "prior_inputs_sha256": digest(BASE_PACKET / "inputs-before.json"),
        "changed": changed, "allowed": sorted(ALLOWED_DELTA),
    }, indent=2) + "\n")
    env = {
        "HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
        "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2",
        "CARGO_TERM_COLOR": "never", "CARGO_PROFILE_TEST_OPT_LEVEL": "1", "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_PROFILE_DEV_DEBUG": "0", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
    }
    tools = [("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"]),
             ("g++", ["/usr/bin/g++", "--version"]), ("hipcc", ["/opt/rocm/bin/hipcc", "--version"])]
    commands = [(name, command, 30) for name, command in tools]
    for name in ("native_benchmark_args", "xgmi_peer_benchmark_common", "xgmi_peer_hot_callbacks",
                 "xgmi_peer_hot_results", "xgmi_peer_segments"):
        commands.append((name, ["/usr/bin/python3", "-I", "-B", PREFIX + "test_" + name + ".py"], 900))
    commands += [
        ("build-hip", ["/opt/rocm/bin/hipcc", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "--offload-arch=gfx942", PREFIX + "xgmi_peer_hip.cpp", "-o", str(output / "peer-hip")], 300),
        ("build-hsa", ["/usr/bin/g++", "-std=c++17", "-O3", "-Wall", "-Wextra", "-Werror",
                       "-I/opt/rocm/include", PREFIX + "xgmi_peer_hsa.cpp", "-L/opt/rocm/lib",
                       "-Wl,-rpath,/opt/rocm/lib", "-lhsa-runtime64", "-o", str(output / "peer-hsa")], 300),
    ]
    cargo = ["cargo", "--offline", "--locked"]
    for label, flags in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
        for feature, feature_flags in (("default", []), ("all", ["--all-features"])):
            commands.append((label + "-" + feature, cargo + ["test", "-p", "fe2o3-runtime", "--example", EXAMPLE]
                             + flags + feature_flags + ["--", "--test-threads=2"], 3600))
    commands += [
        ("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], 120),
        ("clippy", cargo + ["clippy", "-p", "fe2o3-runtime", "--all-features", "--example", EXAMPLE,
                            "--", "-D", "warnings"], 1200),
    ]
    commands += [("after-" + name, command, 30) for name, command in tools]
    try:
        for name, command, bound in commands:
            print("RUN: " + name, flush=True)
            status, stdout, stderr = base.run_owned(command, bound, output / name, env)
            print("\n".join(line for line in stdout.splitlines() if line.startswith("test result:")), flush=True)
            check.need(status == 0, "failed command: " + name)
            print("PASS: " + name, flush=True)
        for name, _ in tools:
            for stream in ("stdout.log", "stderr.log"):
                check.need((output / name / stream).read_bytes() == (output / ("after-" + name) / stream).read_bytes(),
                           "tool continuity: " + name)
        (output / "binaries.json").write_text(json.dumps({
            name: digest(output / name) for name in ("peer-hip", "peer-hsa")
        }, indent=2) + "\n")
    finally:
        after = inputs()
        (output / "inputs-after.json").write_text(json.dumps(after, indent=2) + "\n")
        check.need(before == after, "source changed during qualification")


if __name__ == "__main__":
    main()
