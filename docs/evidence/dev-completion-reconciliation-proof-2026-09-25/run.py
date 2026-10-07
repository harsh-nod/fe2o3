#!/usr/bin/env python3
"""Signed-source planner development checks; not the final mutation campaign."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
V = Path("crates/fe2o3-runtime-model/verus")
CONTROLLER = V / "check-journal-issuance.py"
CONTROLLER_SHA = "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480"
SOURCE_CHECK = V / "check-completion-reconciliation-source.py"
SOURCE_CHECK_SHA = "c9eedb8946cc635776e073df83803391a9909915b894cefc3ee51ba3eab7286f"
SOURCE_TOOLS = None
SSH_KEYGEN = Path("/usr/bin/ssh-keygen")
SSH_KEYGEN_SHA = "5175ddce2146fc8a03ab8e1ef25a1b0382dd3cb209484f7ae11ac782171c0d04"
SIGNATURE = 'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n'
SIGNER = "harmenon@amd.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICITzoV64zd4tYeZhvOi+mnwQaxEI4rFvXeC3HxileBS\n"
BASELINE = "53aba0d658f0d874f3c4f7ef18ff35f971504375"
INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests",
          str(Path(__file__).relative_to(ROOT)), "docs/runtime-completion-reconciliation-v1.md"]
PROOF_RESULT = {"encountered-error": False, "encountered-vir-error": False,
                "errors": 0, "is-verifying-entire-crate": True, "success": True, "verified": 49}
VERIFIER = {"profile": "release", "version": "0.2026.08.09.92f466f",
            "platform": {"os": "linux", "arch": "x86_64"}, "toolchain": "1.97.1-x86_64-unknown-linux-gnu",
            "commit": "92f466f247f45128c630d1c843fd6e27d2115587"}


def need(value, message):
    if not value:
        raise ValueError(message)


def source_tools():
    global SOURCE_TOOLS
    if SOURCE_TOOLS is None:
        raw = (ROOT / SOURCE_CHECK).read_bytes()
        need(hashlib.sha256(raw).hexdigest() == SOURCE_CHECK_SHA, "source-tool helper identity")
        module = types.ModuleType("reconciliation_source_tools")
        module.__file__ = str(ROOT / SOURCE_CHECK)
        exec(compile(raw, module.__file__, "exec"), module.__dict__)
        SOURCE_TOOLS = module
    return SOURCE_TOOLS


def git(*args):
    return source_tools().git(*args)


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def snapshot():
    paths = git("ls-files", "-z", "--", *INPUTS).split(b"\0")
    return {os.fsdecode(p): hashlib.sha256((ROOT / os.fsdecode(p)).read_bytes()).hexdigest()
            for p in sorted(set(paths)) if p}


def clean_source():
    git("diff", "--no-ext-diff", "--no-textconv", "--exit-code", "HEAD", "--", *INPUTS)
    need(not git("ls-files", "--others", "--exclude-standard", "--", *INPUTS), "no untracked source input")


def signature_tool():
    need(SSH_KEYGEN.is_file() and not SSH_KEYGEN.is_symlink(), "ordinary SSH signature tool")
    need(hashlib.sha256(SSH_KEYGEN.read_bytes()).hexdigest() == SSH_KEYGEN_SHA, "SSH signature tool identity")


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--verus", type=Path, required=True)
    args = parser.parse_args()
    for path in (args.output, args.target):
        need(path.is_absolute() and path.resolve() == path, "canonical output/target")
        need(not path.is_relative_to(ROOT) and not ROOT.is_relative_to(path), "output/target outside source tree")
    need(not args.output.is_relative_to(args.target) and not args.target.is_relative_to(args.output), "disjoint output/target")
    need(not args.output.exists(), "new output directory")
    need(args.target.is_dir() and not args.target.is_symlink(), "owned target")
    need(args.verus.is_file() and args.verus.is_absolute(), "absolute verifier")
    os.chdir(ROOT)
    clean_source()
    raw = (ROOT / CONTROLLER).read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CONTROLLER_SHA, "owned-process controller identity")
    controller = types.ModuleType("reconciliation_proof_owned_process")
    controller.__file__ = str(ROOT / CONTROLLER)
    sys.modules[controller.__name__] = controller
    exec(compile(raw, str(ROOT / CONTROLLER), "exec"), controller.__dict__)
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    args.output.mkdir(parents=True, exist_ok=False)
    before = dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot())
    save(args.output / "source-before.json", before)
    signer = args.output / "allowed-signers"
    signer.write_text(SIGNER)
    environment = dict(os.environ, CARGO_TARGET_DIR=str(args.target.resolve()), CARGO_BUILD_JOBS="4", CARGO_INCREMENTAL="0",
                       VERUS_Z3_PATH=str(args.verus.parent / "z3"), TMPDIR=str(args.output))
    save(args.output / "environment.json", {key: environment.get(key) for key in (
        "PATH", "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "RUSTFLAGS", "RUSTDOCFLAGS",
        "RUSTUP_TOOLCHAIN", "LD_LIBRARY_PATH", "VERUS_Z3_PATH", "TMPDIR")})
    source = source_tools()
    signature = [str(source.GIT), "--no-replace-objects", "--no-pager", "-c", "gpg.format=ssh",
                 "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(signer), "verify-commit"]
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    closure = ["/bin/sh", str(ROOT / "examples/row_softmax_v1/verify-verus-closure.sh"), str(args.verus.parent),
               str(ROOT / V / "pins/VERUS_CLOSURE_MANIFEST")]
    proof = ["/usr/bin/timeout", "--foreground", "--signal=TERM", "--kill-after=5", "120", str(args.verus),
             "--crate-type", "lib", "--triggers-mode", "silent", "--no-cheating", "--output-json", "--error-format=json",
             "--no-report-long-running", "--num-threads", "4", str(ROOT / V / "context_completion_reconciliation_v1.rs")]
    phases = [
        ("baseline-signature", [*signature, BASELINE], None),
        ("source-signature", [*signature, before["commit"]], None),
        ("source-tests", [sys.executable, "-I", "-B", str(ROOT / V / "test-completion-reconciliation-source.py")], None),
        ("source-body", [sys.executable, "-I", "-B", str(ROOT / V / "check-completion-reconciliation-source.py"),
                         "--output", str(args.output / "body-comparison")], None),
        ("closure-before", closure, None),
        ("proof-before", proof, None),
        ("gnu", [*cargo, "--all-features", "--lib", "--", "--test-threads=2"], 1416),
        ("musl", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--", "--test-threads=2"], 1416),
        ("doctests", [*cargo, "--all-features", "--doc"], 46),
        ("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"], None),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "--all", "--check"], None),
        ("proof-after", proof, None),
        ("closure-after", closure, None),
    ]
    results = {}
    for name, command, expected in phases:
        need(before == dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot()), "source changed before " + name)
        source.authenticate(source.GIT)
        if name.endswith("-signature"):
            signature_tool()
        status, stdout, stderr = controller.run_owned(command, 130 if name.startswith("proof-") else 1200,
                                                     args.output / name, source.TOOL_ENV if name.endswith("-signature") else environment)
        source.authenticate(source.GIT)
        if name.endswith("-signature"):
            signature_tool()
        counts = [tuple(map(int, row)) for row in re.findall(
            r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out;", stdout, re.M)]
        passed = status == 0
        if name.endswith("-signature"):
            passed = passed and not stdout and stderr == SIGNATURE
        if name.startswith("proof-") and passed:
            observed = json.loads(stdout)
            passed = (json.dumps(observed["verification-results"], sort_keys=True) == json.dumps(PROOF_RESULT, sort_keys=True)
                      and json.dumps(observed["verus"], sort_keys=True) == json.dumps(VERIFIER, sort_keys=True) and not stderr)
        if name.startswith("closure-"):
            passed = passed and not stderr and stdout == "PASS: pinned Verus release closure matched at this measurement (190 files, 129019839 bytes)\n"
        if name == "source-tests":
            passed = passed and not stderr and stdout == "PASS: completion planner source calibration (8 groups)\n"
        if name == "source-body":
            passed = passed and not stderr and stdout == "PASS: allowlisted completion planner syntax correspondence\n"
        if expected is not None:
            passed = passed and bool(counts) and sum(row[0] for row in counts) == expected and all(row[2] == 0 for row in counts)
        if name in ("gnu", "musl"):
            passed = passed and counts == [(1416, 22, 0)] and stdout.count("peer_directed_tests::reconciliation_tests::") == 3
        need(before == dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot()), "source changed after " + name)
        results[name] = dict(passed=passed, status=status, counts=counts)
        save(args.output / (name + ".json"), results[name])
        print(name + (": PASS" if passed else ": FAIL"), flush=True)
        need(passed, "failed phase: " + name)
    save(args.output / "results.json", results)
    clean_source()
    after = dict(commit=git("rev-parse", "HEAD").decode().strip(), inputs=snapshot())
    need(after == before, "final measured source continuity")
    save(args.output / "source-after.json", after)


if __name__ == "__main__":
    main()
