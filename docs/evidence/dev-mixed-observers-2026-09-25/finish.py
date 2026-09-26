#!/usr/bin/env python3
"""Continue independent checks after a terminal failure; never relabel it a pass."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("observer_runner", HERE / "run.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)


def read(path):
    return json.loads(path.read_text())


def main():
    R.H.need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    original = HERE / "raw"
    before = read(original / "source-before.json")
    R.H.need(before == read(original / "source-after.json") == R.H.snapshot(), "unchanged signed source after terminal original run")
    source = Path(read(original / "archive.json")["source"])
    R.H.need(R.H.snapshot(source) == before, "unchanged archived signed source")
    original_results = read(original / "results.json")
    R.H.need(original_results["gnu"]["status"] == 101 and original_results["gnu"]["passed"] is False, "retain failed GNU suite")
    output = HERE / "continuation"
    output.mkdir()
    helpers = {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
               for name in ("finish.py", "socket_probe.py", "run.py", "test_run.py")}
    R.H.save(output / "helpers-before.json", helpers)
    R.H.save(output / "source-before.json", before)
    environment = read(original / "environment.json")
    R.H.save(output / "environment.json", environment)
    controller = R.load(R.H.CONTROLLER, R.H.CONTROLLER_SHA, "observer_continuation_controller")
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    results = {}

    def command(name, argv, expected=None, executable_digest=None, required=True):
        R.H.need(before == R.H.snapshot() == R.H.snapshot(source), "source continuity before " + name)
        print("RUN: " + name, flush=True)
        status, stdout, _ = controller.run_owned(argv, 1200, output / name, environment)
        result = dict(status=status, passed=R.H.accepted(status, stdout, expected), counts=R.H.counts(stdout))
        if executable_digest is not None:
            result["sha256"] = executable_digest
        R.H.save(output / (name + ".json"), result)
        results[name] = result
        print(("PASS: " if result["passed"] else "FAIL: ") + name, flush=True)
        if required:
            R.H.need(result["passed"], "failed prerequisite: " + name)
        return stdout

    os.chdir(source)
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    try:
        command("socket-probe", ["/usr/bin/python3", "-I", "-B", str(HERE / "socket_probe.py")])
        stdout = command("musl-build", [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"])
        executable = R.H.select_executable(stdout, source, Path(environment["CARGO_TARGET_DIR"]))
        digest = R.H.retain_executable(executable, output / "musl-runtime-tests.gz")
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained executable before roster")
        roster = R.roster(command("musl-roster", [str(executable), "--list"]))
        R.H.need(roster == R.roster((original / "gnu-roster/stdout.log").read_text()), "identical target rosters")
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "retained executable before execution")
        command("musl", [str(executable), "--test-threads=2"], (1425, 0, 27, 0, 0), digest, required=False)
        R.H.need(hashlib.sha256(executable.read_bytes()).hexdigest() == digest, "unchanged executed ELF")
        command("doctests", [*cargo, "--all-features", "--doc"], [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)], required=False)
        command("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"], required=False)
        command("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], required=False)
        command("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], required=False)
    finally:
        R.H.save(output / "results.json", results)
        after = R.H.snapshot()
        R.H.save(output / "source-after.json", after)
        helpers_after = {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest() for name in helpers}
        R.H.save(output / "helpers-after.json", helpers_after)
        R.H.need(helpers == helpers_after and before == after == R.H.snapshot(source), "closed source and helper continuity")
    # The full GNU suite failed. Successful independent checks cannot erase it.
    return 1


if __name__ == "__main__":
    sys.exit(main())
