#!/usr/bin/env python3
"""Source-bound GNU/musl qualification of one topology-directory check removal."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import json
from pathlib import Path
import signal
import subprocess
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PRIVATE = Path("/home/harsh/.codex-tmp/fe2o3-link-directory-20260924-Lcnqgul1")
PRIOR = REPO / "docs/evidence/dev-xgmi-hot-batch-cpu-2026-09-24/raw/cpu2/inputs-before.json"
PRIOR_SHA = "f1254dd5ada2bc39e30f557aaf130bd3df6618a5e5af5653714c1abdbdc20b57"
DELTA = {
    "crates/fe2o3-kfd/src/topology.rs",
    "crates/fe2o3-kfd/src/topology/tests/prechecked_reads.rs",
    "crates/fe2o3-kfd/src/topology/tests/prechecked_reads/link_directories.rs",
}
PATHS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "benchmarks/runtime_gfx942"]


def sha(path):
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("ordinary input: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def helpers():
    path = REPO / "docs/evidence/dev-xgmi-hot-batch-mi300x-2026-09-24/campaign.py"
    if not path.is_file() or path.is_symlink():
        raise RuntimeError("ordinary recorder helper")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != "a6b2f42beb687b82bc9f8207acc034df6003c598eb7dc8308447024ef421f940":
        raise RuntimeError("authenticated recorder helpers")
    module = ModuleType("link_directory_capture")
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


def prior_sources():
    if not PRIOR.is_file() or PRIOR.is_symlink():
        raise RuntimeError("ordinary prior qualification")
    raw = PRIOR.read_bytes()
    if hashlib.sha256(raw).hexdigest() != PRIOR_SHA:
        raise RuntimeError("authenticated prior qualification")
    return json.loads(raw)["source"]


def inputs(c):
    raw = subprocess.check_output([*c.K.GIT, "ls-files", "--cached", "--others", "--exclude-standard", "-z", *PATHS],
                                  cwd=REPO, env=c.K.GIT_ENV, timeout=120)
    names = sorted({name for name in raw.decode().split("\0") if name and
                    (name.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in name)})
    return {"runner": sha(Path(__file__)), "source": {name: sha(REPO / name) for name in names}}


def stages():
    cargo = ["cargo", "--offline", "--locked"]
    packages = ["-p", "fe2o3-kfd", "-p", "fe2o3-runtime"]
    yield "rustc", ["rustc", "-vV"], 30
    yield "cargo", ["cargo", "-V"], 30
    yield "format", ["cargo", "fmt", *packages, "--", "--check"], 120
    for label, target in (("gnu", []), ("musl", ["--target", "x86_64-unknown-linux-musl"])):
        yield label + "-focused", [*cargo, "test", "-p", "fe2o3-kfd", "--all-features", "--lib", *target,
                                  "prechecked_reads::link_directories::", "--", "--test-threads=4"], 7200
        for package in ("fe2o3-kfd", "fe2o3-runtime"):
            yield label + "-" + package, [*cargo, "test", "-p", package, "--all-features", "--lib", *target,
                                          "--", "--test-threads=4"], 7200
    yield "docs", [*cargo, "test", *packages, "--all-features", "--doc"], 1200
    yield "clippy", [*cargo, "clippy", *packages, "--all-features", "--all-targets", "--", "-D", "warnings"], 1200
    yield "after-rustc", ["rustc", "-vV"], 30
    yield "after-cargo", ["cargo", "-V"], 30


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output
    if output.parent != PRIVATE or output.resolve() != output or output.is_symlink():
        raise RuntimeError("exact owned campaign output")
    output.mkdir()
    target = output / "target"
    target.mkdir()
    c = helpers()
    for number in c.B.MANAGED:
        signal.signal(number, c.B.interrupted)
    before = inputs(c)
    c.B.write_json(output / "inputs-before.json", before)
    old = prior_sources()
    changed = {name for name in old.keys() | before["source"].keys() if old.get(name) != before["source"].get(name)}
    c.H.need(changed == DELTA, "exact three-path source delta")
    c.B.write_json(output / "prior-delta.json", {"prior_sha256": PRIOR_SHA, "changed": sorted(changed)})
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
           "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2",
           "CARGO_TERM_COLOR": "never", "CARGO_PROFILE_TEST_OPT_LEVEL": "1", "CARGO_PROFILE_TEST_DEBUG": "0",
           "CARGO_PROFILE_DEV_DEBUG": "0", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    recorder = c.B.Recorder(output / "commands", REPO)
    try:
        for name, command, bound in stages():
            recorder.run(name, command, bound, env=env)
        for tool in ("rustc", "cargo"):
            for stream in ("stdout", "stderr"):
                c.H.need(sha(recorder.output / tool / stream) == sha(recorder.output / ("after-" + tool) / stream),
                         "unchanged tool identity")
    finally:
        after = inputs(c)
        c.B.write_json(output / "inputs-after.json", after)
        c.H.need(before == after, "unchanged qualification inputs")


if __name__ == "__main__":
    main()
