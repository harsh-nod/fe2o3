#!/usr/bin/env python3
"""Record a post-trial, byte-identical build and CPU qualification replay."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = "a1301779d5536723cbbb5693823ce7f652d91129"
TARGET = "/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target"
BINARY = Path(TARGET) / "x86_64-unknown-linux-musl/debug/examples/gfx942-runtime-xgmi-segments-smoke"
BUILD = ["cargo", "build", "--locked", "-q", "-p", "fe2o3-runtime",
         "--all-features", "--target", "x86_64-unknown-linux-musl",
         "--example", "gfx942-runtime-xgmi-segments-smoke"]
ENV = {
    "HOME": "/home/harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
    "LANG": "C", "LC_ALL": "C", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
    "CARGO_TARGET_DIR": TARGET, "CARGO_INCREMENTAL": "0",
    "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUST_TEST_THREADS": "2",
    "CARGO_PROFILE_DEV_OPT_LEVEL": "1", "CARGO_PROFILE_DEV_DEBUG": "0",
    "CARGO_PROFILE_DEV_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_DEV_OVERFLOW_CHECKS": "true",
    "CARGO_PROFILE_TEST_OPT_LEVEL": "1", "CARGO_PROFILE_TEST_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true",
}


def capture():
    out = HERE / "local"
    out.mkdir()
    result = json.loads((HERE / "raw/result.json").read_bytes())
    records = []

    def run(label, command, timeout=600):
        started = time.time_ns()
        cp = subprocess.run(command, cwd=ROOT, env=ENV, capture_output=True, timeout=timeout)
        (out / (label + ".stdout")).write_bytes(cp.stdout)
        (out / (label + ".stderr")).write_bytes(cp.stderr)
        record = {"label": label, "command": command, "exit": cp.returncode,
                  "started_unix_ns": started, "finished_unix_ns": time.time_ns()}
        records.append(record)
        if cp.returncode:
            raise RuntimeError(label + " failed")
        return cp.stdout

    paths = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates"]
    run("source-before", ["git", "diff", "--exit-code", SOURCE, "--", *paths])
    run("rustc", ["rustc", "-vV"])
    run("cargo", ["cargo", "-V"])
    run("build", BUILD)
    if BINARY.is_symlink() or hashlib.sha256(BINARY.read_bytes()).hexdigest() != result["binary_sha256"]:
        raise RuntimeError("replayed build is not byte-identical to native payload")
    test = ["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime", "--all-features"]
    run("gnu-runtime", test + ["--lib"])
    run("musl-runtime", test + ["--target", "x86_64-unknown-linux-musl", "--lib",
                               "--example", "gfx942-runtime-xgmi-segments-smoke"])
    run("r74-model", ["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime-model",
                      "--lib", "r74_ordered_peer_copy"])
    run("clippy", ["cargo", "clippy", "--locked", "-q", "-p", "fe2o3-runtime",
                   "--all-features", "--lib", "--tests", "--example",
                   "gfx942-runtime-xgmi-segments-smoke", "--", "-D", "warnings"])
    run("source-after", ["git", "diff", "--exit-code", SOURCE, "--", *paths])
    report = {"schema": "fe2o3.ordered-peer-cpu-replay.v1", "source_commit": SOURCE,
              "cwd": str(ROOT), "environment": ENV, "records": records,
              "binary_sha256": result["binary_sha256"], "byte_identical_post_trial_build": True,
              "clean_target_rebuild": False}
    report["files"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(out.iterdir())}
    (out / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="ascii")
    print("ordered_peer_cpu_replay=passed", flush=True)


if __name__ == "__main__":
    if sys.argv[1:] != ["--capture"]:
        raise SystemExit("use --capture once, before sealing this archive")
    capture()
