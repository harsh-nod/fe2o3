#!/usr/bin/env python3
"""Retained extraction/CPU development, not planner proof or native qualification."""

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
CONTEXT = Path("crates/fe2o3-runtime/src/context/peer_reconciliation.rs")
BODY = CONTEXT.with_name("completion_reconciliation_body.rs")
BASELINE = "b8804de3ec7513d7bb41be52bbaa7d25570ba9c3"
CONTROLLER = ROOT / "crates/fe2o3-runtime-model/verus/check-journal-issuance.py"
CONTROLLER_SHA = "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480"
INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
          str(Path(__file__).relative_to(ROOT)), str(Path(__file__).with_name("test_run.py").relative_to(ROOT))]
HEADER = """    fn plan_completion_step_v1(
        &mut self,
        requested: RuntimeSubmissionIdV1,
    ) -> Result<CompletionStepV1, RuntimeValidationErrorV1> {
"""
PREFIX = """// One executable planner body; custody, journal and settlement effects stay in its adapters.
macro_rules! completion_reconciliation_body {
    ($syntax:ident, $context:ident, $requested:ident, [$($on_step:tt)*]) => {
        $syntax!({
"""
SUFFIX = "\n        })\n    };\n}\n"
INVOCATION = "        completion_reconciliation_body!(completion_settlement_rust_expr, self, requested, [])"
STEP = "for _ in 0..(2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1) {\n                $($on_step)*\n"


def need(value, message):
    if not value:
        raise ValueError(message)


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def function_body(text, suffix):
    need(text.count(HEADER) == 1, "unique planner header")
    tail = text.split(HEADER)[1]
    need(tail.count(suffix) == 1, "unique planner suffix")
    return tail.split(suffix)[0]


def project_body(text):
    need(text.startswith(PREFIX) and text.endswith(SUFFIX), "exact shared macro wrapper")
    body = text[len(PREFIX):-len(SUFFIX)]
    need(body.count(STEP) == 1 and body.count("$($on_step)*") == 1, "exact loop counter hook")
    return body.replace("$($on_step)*\n", "").replace("$context", "self").replace("$requested", "requested")


def canonical(body):
    source = "impl Witness {\n" + HEADER + body + "\n    }\n}\n"
    result = subprocess.run(["rustfmt", "--edition", "2024", "--emit", "stdout"], cwd=ROOT,
                            input=source, text=True, capture_output=True, check=True)
    need(not result.stderr, "clean Rustfmt parsing")
    return result.stdout


def compare_sources(previous, shared, production):
    old = function_body(previous, "\n    }\n\n    pub(super) fn reconcile_directed_success_v1(")
    current = function_body(production, "\n    }\n\n    #[cfg(test)]\n    pub(super) fn reconcile_directed_counted_for_test_v1(")
    need(current == INVOCATION, "exact empty-hook production invocation")
    old, new = canonical(old), canonical(project_body(shared))
    need(old == new, "planner statements changed")
    return old, new


def source_comparison(output):
    old, new = compare_sources(git("show", BASELINE + ":" + str(CONTEXT)).decode(),
                               (ROOT / BODY).read_text(), (ROOT / CONTEXT).read_text())
    output.mkdir(parents=True, exist_ok=False)
    (output / "previous.rs").write_text(old)
    (output / "shared.rs").write_text(new)
    save(output / "comparison.json", dict(baseline=BASELINE, formatted_equal=True,
        previous_sha256=hashlib.sha256(old.encode()).hexdigest(), shared_sha256=hashlib.sha256(new.encode()).hexdigest()))


def snapshot():
    paths = git("ls-files", "-z", "--", *INPUTS).split(b"\0")
    return {os.fsdecode(p): hashlib.sha256((ROOT / os.fsdecode(p)).read_bytes()).hexdigest()
            for p in sorted(set(paths)) if p}


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", type=Path)
    parser.add_argument("--compare-body", action="store_true")
    args = parser.parse_args()
    if args.compare_body:
        source_comparison(args.output)
        return
    need(args.target is not None and args.target.is_dir() and not args.target.is_symlink(), "owned target")
    os.chdir(ROOT)
    git("diff", "--exit-code", "HEAD", "--", *INPUTS)
    need(not git("ls-files", "--others", "--exclude-standard", "--", *INPUTS), "no untracked source input")
    raw = CONTROLLER.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CONTROLLER_SHA, "owned-process controller identity")
    controller = types.ModuleType("reconciliation_owned_process")
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
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    phases = [
        ("runner-tests", [sys.executable, "-I", "-B", str(Path(__file__).with_name("test_run.py"))], None),
        ("source-body", [sys.executable, "-I", "-B", str(Path(__file__)), "--compare-body", "--output", str(args.output / "body-comparison")], None),
        ("rustc", ["rustc", "-Vv"], None),
        ("gnu", [*cargo, "--all-features", "--lib", "--", "--test-threads=2"], 1416),
        ("musl", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--", "--test-threads=2"], 1416),
        ("doctests", [*cargo, "--all-features", "--doc"], 46),
        ("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"], None),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "--all", "--check"], None),
    ]
    results = {}
    for name, command, expected in phases:
        need(before == dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot()), "source changed before " + name)
        status, stdout, _ = controller.run_owned(command, 1200, args.output / name, environment)
        counts = [tuple(map(int, row)) for row in re.findall(
            r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out;", stdout, re.M)]
        passed = status == 0
        if expected is not None:
            passed = passed and bool(counts) and sum(row[0] for row in counts) == expected and all(row[2] == 0 for row in counts)
        if name in ("gnu", "musl"):
            passed = passed and counts == [(1416, 22, 0)] and stdout.count("peer_directed_tests::reconciliation_tests::") == 3
        need(before == dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot()), "source changed after " + name)
        results[name] = dict(passed=passed, status=status, counts=counts)
        save(args.output / (name + ".json"), results[name])
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        need(passed, "failed phase: " + name)
    save(args.output / "source-after.json", before)
    save(args.output / "results.json", results)


if __name__ == "__main__":
    main()
