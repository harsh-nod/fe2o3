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
import tarfile
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
    env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
    return subprocess.check_output(["/usr/bin/git", "--no-replace-objects", *args], cwd=ROOT, env=env)


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, sort_keys=True, indent=2)
        output.write("\n")


def bind_blob(raw, oid):
    need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid,
         "measured bytes differ from signed Git blob")
    return hashlib.sha256(raw).hexdigest()


def snapshot(source_root=ROOT):
    need(git("rev-parse", "--show-object-format") == b"sha1\n", "Git SHA1 object format")
    commit = git("rev-parse", "HEAD").decode().strip()
    need(not git("ls-files", "--others", "--exclude-standard", "--", *INPUTS), "untracked source")
    need(not git("diff", "--cached", "--name-only", commit, "--", *INPUTS), "unsigned index changes")
    inputs = {}
    for entry in git("ls-tree", "-r", "-z", "--full-tree", commit, "--", *INPUTS).split(b"\0"):
        if not entry:
            continue
        header, path = entry.split(b"\t", 1)
        mode, kind, oid = header.split()
        name = os.fsdecode(path)
        source = source_root / name
        need(mode in (b"100644", b"100755") and kind == b"blob", "ordinary source blob")
        need(source.is_file() and not source.is_symlink(), "ordinary measured source")
        inputs[name] = {"git_blob": oid.decode(), "sha256": bind_blob(source.read_bytes(), oid.decode())}
    return dict(commit=commit, inputs=inputs)


def counts(stdout):
    return [tuple(map(int, row)) for row in re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
        stdout, re.M)]


def accepted(status, stdout, expected):
    return status == 0 and (expected is None or counts(stdout) == (expected if isinstance(expected, list) else [expected]))


def select_executable(stdout, source_root, target):
    paths = set()
    for line in stdout.splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        if (row.get("reason") == "compiler-artifact" and row.get("target", {}).get("name") == "fe2o3_runtime"
                and row.get("profile", {}).get("test") and row.get("executable")):
            need(row["target"].get("kind") == ["lib"] and row["target"].get("src_path") ==
                 str(source_root / "crates/fe2o3-runtime/src/lib.rs"), "exact runtime library source")
            need(row.get("package_id") == "path+file://" + str(source_root / "crates/fe2o3-runtime") + "#0.1.0",
                 "exact runtime package identity")
            paths.add(Path(row["executable"]))
    need(len(paths) == 1, "one exact runtime test executable")
    path = paths.pop()
    need(path.resolve().is_relative_to(target.resolve()) and path.is_file() and not path.is_symlink(), "owned test ELF")
    need(0 < path.stat().st_size <= 512 * 1024 * 1024, "bounded ELF extent")
    return path


def retain_executable(path, output):
    raw = path.read_bytes()
    need(raw.startswith(b"\x7fELF"), "ELF executable")
    with output.open("xb") as destination, gzip.GzipFile(fileobj=destination, mode="wb", filename="", mtime=0) as compressed:
        compressed.write(raw)
    save(output.with_suffix(".json"), dict(path=str(path), bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest(),
         gzip_sha256=hashlib.sha256(output.read_bytes()).hexdigest()))
    return hashlib.sha256(raw).hexdigest()


def check_roster(stdout):
    rows = [line.removesuffix(": test") for line in stdout.splitlines() if line.endswith(": test")]
    need(len(rows) == 1447 and len(set(rows)) == len(rows), "exact unique test roster")
    need(stdout.rstrip().endswith("1447 tests, 0 benchmarks"), "complete roster summary")
    need(sum("qualification_gfx942_mixed_duration_v1::tests::" in row for row in rows) == 7,
         "seven fixture tests")
    for suffix in ("native_mixed_duration_profiles_preserve_full_output_and_refund_backing",
                   "native_owned_later_short_completes_while_earlier_long_signal_is_pending"):
        need(sum(row.endswith("::" + suffix) for row in rows) == 1, "exact native canary roster")
    return rows


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--target", required=True, type=Path)
    parser.add_argument("--allowed-signers", required=True, type=Path)
    args = parser.parse_args()
    for path in (args.target, args.output):
        need(path.is_absolute() and path == path.resolve() and not path.is_relative_to(ROOT), "canonical path outside repository")
    need(not args.target.is_relative_to(args.output) and not args.output.is_relative_to(args.target), "disjoint owned paths")
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
    source_root = args.target / "source"
    cargo_home = args.target / "cargo-home"
    source_root.mkdir()
    cargo_home.mkdir()
    # Share only offline dependency caches, never ambient Cargo configuration.
    for name in ("registry", "git"):
        cache = Path("/home/harsh/.cargo") / name
        if cache.is_dir():
            (cargo_home / name).symlink_to(cache, target_is_directory=True)
    for ancestor in (source_root, *source_root.parents):
        need(not any((ancestor / ".cargo" / name).exists() for name in ("config", "config.toml")), "no ambient ancestor Cargo config")
    environment = dict(HOME="/home/harsh", USER="harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin",
        LC_ALL="C", CARGO_HOME=str(cargo_home), CARGO_TARGET_DIR=str(args.target / "build"), CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0",
        GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null",
        CARGO_TERM_COLOR="never", CARGO_PROFILE_TEST_DEBUG="0", CARGO_PROFILE_DEV_DEBUG="0")
    save(args.output / "environment.json", environment)
    shutil.copyfile(args.allowed_signers, args.output / "allowed-signers")
    signature = ["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=ssh", "-c",
                 "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" +
                 str(args.output / "allowed-signers"), "verify-commit", before["commit"]]
    status, _, _ = controller.run_owned(signature, 60, args.output / "signature", environment)
    need(status == 0, "source signature")
    archive = args.output / "source.tar.gz"
    prefixes = [prefix for prefix in INPUTS if any(path == prefix or path.startswith(prefix + "/") for path in before["inputs"])]
    status, _, _ = controller.run_owned(["/usr/bin/git", "--no-replace-objects", "archive", "--format=tar.gz",
        "--output=" + str(archive), before["commit"], *prefixes], 120, args.output / "source-archive", environment)
    need(status == 0, "signed source archive")
    with tarfile.open(archive, "r:gz") as packed:
        need(all(member.isdir() or member.isfile() for member in packed.getmembers()), "ordinary archive entries")
        packed.extractall(source_root, filter="data")
    need(snapshot(source_root) == before, "archive exactly matches signed blobs")
    save(args.output / "archive.json", dict(sha256=hashlib.sha256(archive.read_bytes()).hexdigest(), source=str(source_root)))
    save(args.output / "selected-tools.json", {path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
        for path in ("/usr/bin/git", "/usr/bin/ssh-keygen", "/opt/rocm/llvm/bin/clang", "/opt/rocm/llvm/bin/ld.lld", "/opt/rocm/llvm/bin/llvm-objdump")})
    os.chdir(source_root)
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    phases = [
        ("runner-tests", [sys.executable, "-I", "-B", str(HERE / "test_run.py")], None),
        ("fixture-rebuild", ["bash", FIXTURES + "/build-and-verify.sh"], None),
        ("short-disassembly", ["/opt/rocm/llvm/bin/llvm-objdump", "--disassemble", "--mcpu=gfx942", FIXTURES + "/short.hsaco"], None),
        ("long-disassembly", ["/opt/rocm/llvm/bin/llvm-objdump", "--disassemble", "--mcpu=gfx942", FIXTURES + "/long.hsaco"], None),
        ("rustc", ["rustc", "-Vv"], None),
        ("gnu-build", [*cargo, "--all-features", "--lib", "--no-run", "--message-format=json"], None),
        ("musl-build", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"], None),
        ("doctests", [*cargo, "--all-features", "--doc"], [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)]),
        ("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"], None),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], None),
    ]
    results = {}
    try:
        for name, command, expected in phases:
            need(snapshot() == snapshot(source_root) == before, "source changed before " + name)
            print("RUN: " + name, flush=True)
            status, stdout, _ = controller.run_owned(command, 1200, args.output / name, environment)
            passed = accepted(status, stdout, expected)
            results[name] = dict(passed=passed, status=status, counts=counts(stdout))
            save(args.output / (name + ".json"), results[name])
            need(passed, "failed phase: " + name)
            if name in ("gnu-build", "musl-build"):
                platform = name.removesuffix("-build")
                target = args.target / "build"
                if platform == "musl":
                    target /= "x86_64-unknown-linux-musl"
                executable = select_executable(stdout, source_root, target)
                digest = retain_executable(executable, args.output / (platform + "-runtime-tests.gz"))
                need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained ELF before roster")
                status, roster, _ = controller.run_owned([str(executable), "--list"], 60, args.output / (platform + "-roster"), environment)
                need(status == 0, "test roster")
                check_roster(roster)
                need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained ELF before execution")
                status, stdout, _ = controller.run_owned([str(executable), "--test-threads=2"], 1200, args.output / platform, environment)
                passed = accepted(status, stdout, (1423, 0, 24, 0, 0))
                results[platform] = dict(passed=passed, status=status, counts=counts(stdout), sha256=digest)
                save(args.output / (platform + ".json"), results[platform])
                need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "executed ELF continuity")
                need(passed, "direct executable tests: " + platform)
            need(snapshot() == snapshot(source_root) == before, "source changed after " + name)
            print("PASS: " + name, flush=True)
    finally:
        after = snapshot()
        need(snapshot(source_root) == after, "archive source continuity")
        save(args.output / "source-after.json", after)
        save(args.output / "results.json", results)
        need(before == after, "source continuity")


if __name__ == "__main__":
    main()
