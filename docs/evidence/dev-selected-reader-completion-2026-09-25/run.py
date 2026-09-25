#!/usr/bin/env python3
"""Retain CPU development checks; not a hardware or Verus qualification gate."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[3]
INPUTS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples", "tests"]
INPUTS += ["docs/evidence/dev-selected-reader-completion-2026-09-25/run.py", "docs/evidence/dev-selected-reader-completion-2026-09-25/test_run.py"]
INPUTS += ["docs/evidence/dev-selected-reader-completion-2026-09-25/proof.py", "docs/evidence/dev-selected-reader-completion-2026-09-25/test_proof.py"]
GENERATED = "context::generated_issue::tests::journal_tests::reader_tests::generated_reader_release_validates_before_selected_root_effect"
FOCUSED = "context::tests::producer_launch_tests::completion_faults::"
NAMES = {
    FOCUSED + name
    for name in (
        "completion_context_boundary_errors_preserve_exact_committed_prefix",
        "completion_context_before_effect_panics_preserve_exact_committed_prefix",
        "completion_context_after_effect_panics_preserve_commit_without_retiring_roots",
        "completion_context_prevalidation_rejects_before_the_first_effect_boundary",
        "completion_context_late_writer_identity_error_preserves_released_inputs",
    )
}
CANCEL_SIGNAL = None


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def save(path, value):
    with path.open("x") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def snapshot():
    files = git("ls-files", "-z", "--", *INPUTS).split(b"\0")
    return {
        os.fsdecode(name): hashlib.sha256((ROOT / os.fsdecode(name)).read_bytes()).hexdigest()
        for name in sorted(set(files)) if name
    }


def interrupted(signum, _frame):
    global CANCEL_SIGNAL
    CANCEL_SIGNAL = signum


def group_exists(pgid):
    try:
        os.killpg(pgid, 0)
        return True
    except ProcessLookupError:
        return False


def main():
    if sys.flags.optimize:
        raise RuntimeError("optimized Python is not supported")
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGINT, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    args = parser.parse_args()
    if not args.target.is_dir() or args.target.is_symlink():
        raise RuntimeError("target must be an existing nonsymlink directory")
    git("diff", "--exit-code", "HEAD", "--", *INPUTS)
    if git("ls-files", "--others", "--exclude-standard", "--", *INPUTS):
        raise RuntimeError("untracked source inputs")
    args.output.mkdir(parents=True, exist_ok=False)
    source = snapshot()
    save(args.output / "source-before.json", source)
    environment = dict(os.environ, CARGO_TARGET_DIR=str(args.target.resolve()), CARGO_BUILD_JOBS="4", CARGO_INCREMENTAL="0")
    base = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features"]
    phases = [
        ("runner-tests", [sys.executable, "-I", "-B", str(Path(__file__).with_name("test_run.py"))], None),
        ("proof-runner-tests", [sys.executable, "-I", "-B", str(Path(__file__).with_name("test_proof.py"))], None),
        ("rustc", ["rustc", "-Vv"], None),
        ("focused", [*base, "--lib", FOCUSED], [(5, 0, 1430)]),
        ("generated", [*base, "--lib", "generated_reader_release_validates_before_selected_root_effect"], [(1, 0, 1434)]),
        ("gnu", [*base, "--lib"], [(1413, 22, 0)]),
        ("musl", [*base, "--lib", "--target", "x86_64-unknown-linux-musl"], [(1413, 22, 0)]),
        ("doctests", [*base, "--doc"], [(4, 0, 0), (42, 0, 0)]),
        ("default", ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime", "--no-default-features"], None),
        ("clippy", ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "--all", "--check"], None),
    ]
    for name, command, expected in phases:
        if CANCEL_SIGNAL is not None:
            raise RuntimeError(f"interrupted by signal {CANCEL_SIGNAL}")
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        start = time.monotonic()
        timed_out = False
        code = None
        interruption = None
        cleanup_required = False
        with (args.output / f"{name}.stdout").open("xb") as out, (args.output / f"{name}.stderr").open("xb") as err:
            process = None
            try:
                process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=out, stderr=err, start_new_session=True)
                while True:
                    if CANCEL_SIGNAL is not None:
                        interruption = f"signal {CANCEL_SIGNAL}"
                        break
                    remaining = 1200 - (time.monotonic() - start)
                    if remaining <= 0:
                        timed_out = True
                        break
                    try:
                        code = process.wait(timeout=min(1, remaining))
                        break
                    except subprocess.TimeoutExpired:
                        continue
            except BaseException as error:
                interruption = repr(error)
            finally:
                if process is not None:
                    cleanup_required = group_exists(process.pid)
                    if cleanup_required:
                        # Managed signals only set a flag; none can unwind cleanup.
                        try:
                            os.killpg(process.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        process.wait()
                    code = process.returncode
        if CANCEL_SIGNAL is not None:
            interruption = f"signal {CANCEL_SIGNAL}"
        group_absent = process is not None and not group_exists(process.pid)
        stdout = (args.output / f"{name}.stdout").read_text()
        counts = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out;", stdout, re.M)
        names = set(re.findall(r"^test (\S+) \.\.\. ok$", stdout, re.M))
        roster_ok = name not in ("focused", "gnu", "musl") or NAMES <= names
        roster_ok = roster_ok and (name not in ("generated", "gnu", "musl") or GENERATED in names)
        counts_ok = expected is None or counts == [tuple(map(str, entry)) for entry in expected]
        after = snapshot()
        passed = code == 0 and not timed_out and interruption is None and not cleanup_required and group_absent and roster_ok and counts_ok and source == after
        save(args.output / f"{name}.json", dict(
            command=command, cwd=str(ROOT), started=started, elapsed_seconds=time.monotonic() - start,
            source_commit=git("rev-parse", "HEAD").decode().strip(), returncode=code, timed_out=timed_out,
            pgid=None if process is None else process.pid, process_group_absent=group_absent,
            interruption=interruption, cleanup_required=cleanup_required, counts=counts, roster_ok=roster_ok,
            sources_unchanged=source == after, passed=passed,
            environment={key: environment.get(key) for key in ("CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "RUSTFLAGS", "RUSTDOCFLAGS", "RUSTUP_TOOLCHAIN")},
        ))
        print(f"{name}: {'PASS' if passed else 'FAIL'}", flush=True)
        if not passed:
            raise RuntimeError(f"failed phase: {name}")
    save(args.output / "source-after.json", snapshot())


if __name__ == "__main__":
    main()
