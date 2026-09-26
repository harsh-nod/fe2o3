#!/usr/bin/env python3
"""Signed-source CPU development checks; never a native/performance acceptance."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import types

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
CONTROLLER = ROOT / "crates/fe2o3-runtime-model/verus/check-journal-issuance.py"
CONTROLLER_SHA = "36d5e0641300bf2568884ec4c440c668477a4b24a07ee95273a91eab34659480"
INPUTS = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "examples", "tests",
          str(Path(__file__).relative_to(ROOT)), str(HERE.relative_to(ROOT) / "test_run.py")]
FIXTURES = "crates/fe2o3-runtime/fixtures/trusted-gfx942-mixed-duration-v1"


def need(value, message):
    if not value:
        raise ValueError(message)


def git(*args):
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
    return subprocess.check_output(["git", "--no-replace-objects", *args], cwd=ROOT, env=env)


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, sort_keys=True, indent=2)
        output.write("\n")


def bind_blob(raw, oid):
    need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid,
         "measured bytes differ from signed Git blob")
    return hashlib.sha256(raw).hexdigest()


def snapshot():
    need(git("rev-parse", "--show-object-format") == b"sha1\n", "Git SHA1 object format")
    commit = git("rev-parse", "HEAD").decode().strip()
    need(not git("ls-files", "--others", "--exclude-standard", "--", *INPUTS), "untracked source")
    inputs = {}
    for entry in git("ls-tree", "-r", "-z", "--full-tree", commit, "--", *INPUTS).split(b"\0"):
        if not entry:
            continue
        header, path = entry.split(b"\t", 1)
        mode, kind, oid = header.split()
        name = os.fsdecode(path)
        source = ROOT / name
        need(mode in (b"100644", b"100755") and kind == b"blob", "ordinary source blob")
        need(source.is_file() and not source.is_symlink(), "ordinary measured source")
        inputs[name] = {"git_blob": oid.decode(), "sha256": bind_blob(source.read_bytes(), oid.decode())}
    return dict(commit=commit, inputs=inputs)


def counts(stdout):
    return [tuple(map(int, row)) for row in re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
        stdout, re.M)]


def accepted(status, stdout, expected):
    return status == 0 and (expected is None or counts(stdout) == [expected])


def retain_executable(stdout, target, output):
    paths = set()
    for line in stdout.splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        if (row.get("reason") == "compiler-artifact" and row.get("target", {}).get("name") == "fe2o3_runtime"
                and row.get("profile", {}).get("test") and row.get("executable")):
            paths.add(Path(row["executable"]))
    need(len(paths) == 1, "one exact runtime test executable")
    path = paths.pop()
    need(path.resolve().is_relative_to(target.resolve()) and path.is_file() and not path.is_symlink(), "owned test ELF")
    raw = path.read_bytes()
    need(raw.startswith(b"\x7fELF"), "ELF executable")
    with output.open("xb") as destination, gzip.GzipFile(fileobj=destination, mode="wb", filename="", mtime=0) as compressed:
        compressed.write(raw)
    save(output.with_suffix(".json"), dict(path=str(path), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
         gzip_sha256=hashlib.sha256(output.read_bytes()).hexdigest()))
    return path


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--target", required=True, type=Path)
    parser.add_argument("--allowed-signers", required=True, type=Path)
    args = parser.parse_args()
    need(args.target.is_dir() and not args.target.is_symlink() and not any(args.target.iterdir()), "fresh owned target")
    args.output.mkdir(parents=True, exist_ok=False)
    os.chdir(ROOT)
    before = snapshot()
    save(args.output / "source-before.json", before)
    raw = CONTROLLER.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == CONTROLLER_SHA, "owned-process helper identity")
    controller = types.ModuleType("mixed_duration_owned")
    controller.__file__ = str(CONTROLLER)
    sys.modules[controller.__name__] = controller
    exec(compile(raw, str(CONTROLLER), "exec"), controller.__dict__)
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    environment = dict(HOME="/home/harsh", USER="harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin",
        LC_ALL="C", CARGO_TARGET_DIR=str(args.target.resolve()), CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0",
        CARGO_TERM_COLOR="never", CARGO_PROFILE_TEST_DEBUG="0", CARGO_PROFILE_DEV_DEBUG="0")
    save(args.output / "environment.json", environment)
    shutil.copyfile(args.allowed_signers, args.output / "allowed-signers")
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    phases = [
        ("signature", ["git", "--no-replace-objects", "-c", "gpg.format=ssh", "-c",
                       "gpg.ssh.allowedSignersFile=" + str((args.output / "allowed-signers").resolve()),
                       "verify-commit", before["commit"]], None),
        ("runner-tests", [sys.executable, "-I", "-B", str(HERE / "test_run.py")], None),
        ("fixture-rebuild", ["bash", FIXTURES + "/build-and-verify.sh"], None),
        ("short-disassembly", ["/opt/rocm/llvm/bin/llvm-objdump", "--disassemble", "--mcpu=gfx942", FIXTURES + "/short.hsaco"], None),
        ("long-disassembly", ["/opt/rocm/llvm/bin/llvm-objdump", "--disassemble", "--mcpu=gfx942", FIXTURES + "/long.hsaco"], None),
        ("rustc", ["rustc", "-Vv"], None),
        ("gnu", [*cargo, "--all-features", "--lib", "--message-format=json", "--", "--test-threads=2"], (1423, 0, 24, 0, 0)),
        ("musl", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--message-format=json", "--", "--test-threads=2"], (1423, 0, 24, 0, 0)),
        ("doctests", [*cargo, "--all-features", "--doc"], (46, 0, 0, 0, 0)),
        ("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"], None),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], None),
    ]
    results = {}
    try:
        for name, command, expected in phases:
            need(snapshot() == before, "source changed before " + name)
            print("RUN: " + name, flush=True)
            status, stdout, _ = controller.run_owned(command, 1200, args.output / name, environment)
            passed = accepted(status, stdout, expected)
            results[name] = dict(passed=passed, status=status, counts=counts(stdout))
            save(args.output / (name + ".json"), results[name])
            need(passed, "failed phase: " + name)
            if name in ("gnu", "musl"):
                executable = retain_executable(stdout, args.target, args.output / (name + "-runtime-tests.gz"))
                status, _, _ = controller.run_owned([str(executable), "--list"], 60, args.output / (name + "-roster"), environment)
                need(status == 0, "test roster")
            need(snapshot() == before, "source changed after " + name)
            print("PASS: " + name, flush=True)
    finally:
        after = snapshot()
        save(args.output / "source-after.json", after)
        save(args.output / "results.json", results)
        need(before == after, "source continuity")


if __name__ == "__main__":
    main()
