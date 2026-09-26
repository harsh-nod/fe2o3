#!/usr/bin/env python3
"""Correct the musl linker flag and finish checks without replacing prior failures."""
import hashlib
import importlib.util
import os
from pathlib import Path
import signal
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("observer_continuation", HERE / "finish.py")
F = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(F)
R = F.R


def main():
    R.H.need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    original, prior, output = HERE / "raw", HERE / "continuation", HERE / "validation"
    before = F.read(original / "source-before.json")
    R.H.need(before == F.read(original / "source-after.json") == F.read(prior / "source-after.json") == R.H.snapshot(), "unchanged signed source after terminal attempts")
    R.H.need(F.read(prior / "results.json")["musl-build"]["status"] == 101, "retain previous linker failure")
    source = Path(F.read(original / "archive.json")["source"])
    R.H.need(R.H.snapshot(source) == before, "unchanged archived signed source")
    output.mkdir()
    helpers = {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
               for name in ("resume.py", "finish.py", "socket_probe.py", "run.py", "test_run.py")}
    R.H.save(output / "helpers-before.json", helpers)
    R.H.save(output / "source-before.json", before)
    environment = F.read(original / "environment.json")
    musl_environment = {name: value for name, value in environment.items() if name != "RUSTFLAGS"}
    R.H.save(output / "environment.json", environment)
    R.H.save(output / "musl-environment.json", musl_environment)
    controller = R.load(R.H.CONTROLLER, R.H.CONTROLLER_SHA, "observer_resume_controller")
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    results = {}

    def command(name, argv, expected=None, digest=None, required=False):
        R.H.need(before == R.H.snapshot() == R.H.snapshot(source), "source continuity before " + name)
        print("RUN: " + name, flush=True)
        status, stdout, _ = controller.run_owned(argv, 1200, output / name, musl_environment if name.startswith("musl") else environment)
        result = dict(status=status, passed=R.H.accepted(status, stdout, expected), counts=R.H.counts(stdout))
        if digest is not None:
            result["sha256"] = digest
        R.H.save(output / (name + ".json"), result)
        results[name] = result
        print(("PASS: " if result["passed"] else "FAIL: ") + name, flush=True)
        if required:
            R.H.need(result["passed"], "failed prerequisite: " + name)
        return stdout

    os.chdir(source)
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    try:
        # Capture the delayed GNU identity check separately; the failed original
        # command never reached its immediate post-execution digest check.
        gnu = F.read(original / "gnu-runtime-tests.json")
        gnu_digest = hashlib.sha256(Path(gnu["path"]).read_bytes()).hexdigest()
        R.H.need(gnu_digest == gnu["sha256"], "delayed GNU executable continuity")
        R.H.save(output / "gnu-delayed-digest.json", dict(sha256=gnu_digest, immediate_post_execution=False))
        stdout = command("musl-build", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"], required=True)
        executable = R.H.select_executable(stdout, source, Path(environment["CARGO_TARGET_DIR"]))
        digest = R.H.retain_executable(executable, output / "musl-runtime-tests.gz")
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained ELF before roster")
        roster = R.roster(command("musl-roster", [str(executable), "--list"], required=True))
        R.H.need(roster == R.roster((original / "gnu-roster/stdout.log").read_text()), "identical target rosters")
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained ELF before execution")
        command("musl", [str(executable), "--test-threads=2"], (1425, 0, 27, 0, 0), digest)
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "unchanged executed ELF")
        command("doctests", [*cargo, "--all-features", "--doc"], [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)])
        command("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"])
        command("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"])
        command("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"])
    finally:
        R.H.save(output / "results.json", results)
        after = R.H.snapshot()
        R.H.save(output / "source-after.json", after)
        helpers_after = {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest() for name in helpers}
        R.H.save(output / "helpers-after.json", helpers_after)
        R.H.need(helpers == helpers_after and before == after == R.H.snapshot(source), "closed source and helper continuity")
    return 1


if __name__ == "__main__":
    sys.exit(main())
