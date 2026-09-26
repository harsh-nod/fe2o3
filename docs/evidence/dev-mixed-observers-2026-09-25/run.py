#!/usr/bin/env python3
"""Cold signed-source CPU qualification of observer/backpressure regressions."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import signal
import sys
import tarfile
from types import ModuleType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
HELPER = HERE.parent / "dev-mixed-duration-2026-09-25/run.py"
HELPER_SHA = "3aab8b677aa0ff87ac27898970d263f3e26658b05332e42bfe0b942ce3152258"
CPU_NAMES = (
    "recovered_timeout_rearms_pending_operation_without_reissuing_or_waking_old_observer",
    "frozen_tracked_launch_queue_rejection_refunds_both_budgets_without_submission",
)
NATIVE_NAMES = (
    "native_owned_timeout_recovers_exact_published_operation_and_full_output",
    "native_owned_dropped_observer_keeps_autonomous_progress_and_full_output",
    "native_owned_command_backpressure_refunds_rejected_launch_and_recovers",
)


def load(path, digest, name):
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("authenticated helper: " + str(path))
    module = ModuleType(name)
    module.__file__ = str(path)
    sys.modules[name] = module
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


H = load(HELPER, HELPER_SHA, "observer_cpu_helpers")
H.INPUTS += [str(path.relative_to(ROOT)) for path in (HERE / "run.py", HERE / "test_run.py")]


def roster(stdout):
    rows = [line.removesuffix(": test") for line in stdout.splitlines() if line.endswith(": test")]
    H.need(len(rows) == len(set(rows)) == 1452 and stdout.rstrip().endswith("1452 tests, 0 benchmarks"), "complete unique test roster")
    for suffix in (*CPU_NAMES, *NATIVE_NAMES):
        H.need(sum(row.endswith("::" + suffix) for row in rows) == 1, "new exact regression: " + suffix)
    return rows


def main():
    H.need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", type=Path, required=True)
    args = parser.parse_args()
    target = args.target
    H.need(target.is_absolute() and target.resolve() == target and target.is_dir() and not target.is_symlink()
           and not any(target.iterdir()) and not target.is_relative_to(ROOT), "fresh canonical private build target")
    output = HERE / "raw"
    output.mkdir()
    before = H.snapshot()
    H.save(output / "source-before.json", before)
    source = target / "source"
    source.mkdir()
    cargo_home = target / "cargo-home"
    cargo_home.mkdir()
    for name in ("registry", "git"):
        cache = Path("/home/harsh/.cargo") / name
        if cache.is_dir():
            (cargo_home / name).symlink_to(cache, target_is_directory=True)
    for ancestor in (source, *source.parents):
        H.need(not any((ancestor / ".cargo" / name).exists() for name in ("config", "config.toml")), "no ambient Cargo config")
    environment = dict(HOME="/home/harsh", USER="harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin", LC_ALL="C",
                       CARGO_HOME=str(cargo_home), CARGO_TARGET_DIR=str(target / "build"), CARGO_BUILD_JOBS="2",
                       CARGO_INCREMENTAL="0", CARGO_TERM_COLOR="never", CARGO_PROFILE_TEST_DEBUG="0",
                       CARGO_PROFILE_DEV_DEBUG="0", RUSTFLAGS="-Clink-arg=-Wl,--threads=1",
                       GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
    H.save(output / "environment.json", environment)
    signer = HERE.parent / "dev-mixed-duration-2026-09-25/retained/signed-cpu-v3/allowed-signers"
    H.need(hashlib.sha256(signer.read_bytes()).hexdigest() == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b", "trusted signer")
    shutil.copyfile(signer, output / "allowed-signers")
    controller = load(H.CONTROLLER, H.CONTROLLER_SHA, "observer_cpu_controller")
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    results = {}

    def command(name, argv, expected=None, executable_digest=None):
        print("RUN: " + name, flush=True)
        status, stdout, _ = controller.run_owned(argv, 1200, output / name, environment)
        result = dict(status=status, passed=H.accepted(status, stdout, expected), counts=H.counts(stdout))
        if executable_digest is not None:
            result["sha256"] = executable_digest
        H.save(output / (name + ".json"), result)
        results[name] = result
        H.need(result["passed"], "failed phase: " + name)
        print("PASS: " + name, flush=True)
        return stdout

    os.chdir(ROOT)
    command("signature", ["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen",
                          "-c", "gpg.ssh.allowedSignersFile=" + str(output / "allowed-signers"), "verify-commit", before["commit"]])
    archive = output / "source.tar.gz"
    prefixes = [prefix for prefix in H.INPUTS if any(name == prefix or name.startswith(prefix + "/") for name in before["inputs"])]
    command("source-archive", ["/usr/bin/git", "--no-replace-objects", "archive", "--format=tar.gz", "--output=" + str(archive), before["commit"], *prefixes])
    with tarfile.open(archive, "r:gz") as packed:
        H.need(all(entry.isdir() or entry.isfile() for entry in packed.getmembers()), "ordinary source archive")
        packed.extractall(source, filter="data")
    H.need(H.snapshot(source) == before, "exact signed archive inputs")
    H.save(output / "archive.json", dict(sha256=hashlib.sha256(archive.read_bytes()).hexdigest(), source=str(source)))
    os.chdir(source)
    try:
        command("runner-tests", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_run.py")])
        command("rustc", ["rustc", "-Vv"])
        cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
        rosters = []
        for platform in ("gnu", "musl"):
            H.need(H.snapshot() == H.snapshot(source) == before, "signed source continuity")
            extra = [] if platform == "gnu" else ["--target", "x86_64-unknown-linux-musl"]
            stdout = command(platform + "-build", [*cargo, "--all-features", "--lib", *extra, "--no-run", "--message-format=json"])
            executable = H.select_executable(stdout, source, target / "build")
            digest = H.retain_executable(executable, output / (platform + "-runtime-tests.gz"))
            H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained executable before roster")
            rosters.append(roster(command(platform + "-roster", [str(executable), "--list"])))
            H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained executable before tests")
            command(platform, [str(executable), "--test-threads=2"], (1425, 0, 27, 0, 0), digest)
            H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "unchanged direct executable")
        H.need(rosters[0] == rosters[1], "identical cross-target roster")
        command("doctests", [*cargo, "--all-features", "--doc"], [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)])
        command("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"])
        command("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"])
        command("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"])
    finally:
        after = H.snapshot()
        H.save(output / "source-after.json", after)
        H.save(output / "results.json", results)
        H.need(before == after == H.snapshot(source), "closed source continuity")


if __name__ == "__main__":
    main()
