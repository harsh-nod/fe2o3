#!/usr/bin/env python3
"""Retained host-data development checks, not proof or hardware qualification."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
CONTROLLER = ROOT / "crates/fe2o3-runtime-model/verus/check-journal-issuance.py"
CONTROLLER_SHA = "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480"
INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests", str(Path(__file__).relative_to(ROOT)), str(Path(__file__).with_name("test_run.py").relative_to(ROOT))]
FIXTURE = "crates/fe2o3-macros/tests/fixtures/generic-worker-v3-adapter/Cargo.toml"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def snapshot():
    paths = git("ls-files", "-z", "--", *INPUTS).split(b"\0")
    return {os.fsdecode(p): hashlib.sha256((ROOT / os.fsdecode(p)).read_bytes()).hexdigest()
            for p in sorted(set(paths)) if p}


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def compiler_negative(status, stdout, code, phrase, filename):
    try:
        messages = [json.loads(line) for line in stdout.splitlines()]
        errors = [m["message"] for m in messages if m.get("reason") == "compiler-message"
                  and m["message"]["level"] == "error"]
        return (status == 101 and len(errors) == 1
                and errors[0]["code"]["code"] == code
                and phrase in errors[0]["message"]
                and any(span["is_primary"] is True and Path(span["file_name"]).name == filename
                        for span in errors[0]["spans"])
                and set(messages[-1]) == {"reason", "success"}
                and messages[-1]["reason"] == "build-finished"
                and messages[-1]["success"] is False)
    except (ValueError, TypeError, KeyError, IndexError, AttributeError):
        return False


def main():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise RuntimeError("use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    args = parser.parse_args()
    if not args.target.is_dir() or args.target.is_symlink():
        raise RuntimeError("existing ordinary owned target required")
    os.chdir(ROOT)
    git("diff", "--exit-code", "HEAD", "--", *INPUTS)
    if git("ls-files", "--others", "--exclude-standard", "--", *INPUTS):
        raise RuntimeError("untracked source input")
    raw = CONTROLLER.read_bytes()
    if hashlib.sha256(raw).hexdigest() != CONTROLLER_SHA:
        raise RuntimeError("owned-process controller identity")
    controller = types.ModuleType("completed_input_owned_process")
    controller.__file__ = str(CONTROLLER)
    sys.modules[controller.__name__] = controller
    exec(compile(raw, str(CONTROLLER), "exec"), controller.__dict__)
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    args.output.mkdir(parents=True, exist_ok=False)
    before = dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot())
    save(args.output / "source-before.json", before)
    environment = dict(os.environ, CARGO_TARGET_DIR=str(args.target.resolve()), CARGO_BUILD_JOBS="4", CARGO_INCREMENTAL="0")
    save(args.output / "environment.json", {key: environment.get(key) for key in (
        "PATH", "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTUP_TOOLCHAIN", "LD_LIBRARY_PATH")})
    cargo = ["cargo", "test", "--locked", "--offline"]
    fixture = ["cargo", "check", "--locked", "--offline", "--manifest-path", FIXTURE,
               "--target-dir", str(args.target.resolve() / "fixture"), "--message-format=json", "--bin"]
    phases = [
        ("runner-tests", [sys.executable, "-I", "-B", str(Path(__file__).with_name("test_run.py"))], None),
        ("rustc", ["rustc", "-Vv"], None),
        ("host-gnu", [*cargo, "-p", "fe2o3-host", "--lib", "--", "--test-threads=2"], 175),
        ("host-musl", [*cargo, "-p", "fe2o3-host", "--lib", "--target", "x86_64-unknown-linux-musl", "--", "--test-threads=2"], 175),
        ("host-doctests", [*cargo, "-p", "fe2o3-host", "--doc"], "tests"),
        ("accounting", [*cargo, "-p", "fe2o3-resource-accounting", "--lib"], "tests"),
        ("macros", [*cargo, "-p", "fe2o3-macros", "--lib"], "tests"),
        ("runtime", [*cargo, "-p", "fe2o3-runtime", "--all-features", "--lib", "--", "--test-threads=2"], 1413),
        ("fixture-pass", [*fixture, "pass"], None),
        ("fixture-reuse", [*fixture, "charged_result_reuse"], ("E0382", "use of moved value: `result`", "charged_result_reuse.rs")),
        ("fixture-clone", [*fixture, "charged_result_clone"], ("E0599", "no method named `clone`", "charged_result_clone.rs")),
        ("fixture-storage", [*fixture, "charged_result_storage_escape"], ("E0599", "no method named `into_boxed_slice`", "charged_result_storage_escape.rs")),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-host", "-p", "fe2o3-macros", "-p", "fe2o3-resource-accounting", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "--all", "--check"], None),
    ]
    results = {}
    for name, command, expected in phases:
        if before != dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot()):
            raise RuntimeError("source changed before " + name)
        status, stdout, _ = controller.run_owned(command, 1200, args.output / name, environment)
        counts = [tuple(map(int, row)) for row in re.findall(
            r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out;", stdout, re.M)]
        passed = status == 0
        if isinstance(expected, tuple):
            passed = compiler_negative(status, stdout, *expected)
        elif isinstance(expected, int):
            passed = passed and len(counts) == 1 and counts[0][0] == expected and counts[0][2] == 0
        elif expected == "tests":
            passed = passed and bool(counts) and sum(row[0] for row in counts) > 0 and all(row[2] == 0 for row in counts)
        if name.startswith("host-") and name != "host-doctests":
            passed = passed and stdout.count("::completed_input_") == 8 and "invocation_completed_input_reuses_host_data_through_read_only_preparation ... ok" in stdout
        passed = passed and before == dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot())
        results[name] = dict(passed=passed, status=status, counts=counts)
        save(args.output / (name + ".json"), results[name])
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        if not passed:
            raise RuntimeError("failed phase: " + name)
    save(args.output / "source-after.json", before)
    save(args.output / "results.json", results)


if __name__ == "__main__":
    main()
