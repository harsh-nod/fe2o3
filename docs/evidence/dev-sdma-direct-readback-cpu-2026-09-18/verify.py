#!/usr/bin/env python3
"""Check fresh CPU receipts and compare the complete cohort to signed source."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
assert __debug__, "the pinned harness parser requires assertions"

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
COMMIT = "fd1cf3dd05e691797533b1beab1f015c4b2d8fad"
ENV = "env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1"
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
TIMES = {}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def receipt(name, command):
    prefix = ARCHIVE / "raw" / name
    need(
        all(
            prefix.with_suffix(suffix).is_file()
            and not prefix.with_suffix(suffix).is_symlink()
            for suffix in (".exit", ".command", ".log", ".started", ".finished")
        ),
        "ordinary receipt files",
    )
    need(prefix.with_suffix(".exit").read_text() == "0\n", f"exit: {name}")
    need(
        shlex.split(prefix.with_suffix(".command").read_text()) == command,
        f"command: {name}",
    )
    stamps = [
        prefix.with_suffix(suffix).read_text() for suffix in (".started", ".finished")
    ]
    need(
        all(
            re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z\n", value)
            for value in stamps
        ),
        "UTC timestamps",
    )
    need(stamps[0] <= stamps[1], "ordered command interval")
    TIMES[name] = stamps
    return prefix.with_suffix(".log").read_text()


def main():
    need(
        sha(HELPER.read_bytes())
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb",
        "pinned strict harness parser",
    )
    spec = importlib.util.spec_from_file_location("strict_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    before = receipt(
        "source-before-final", ["python3", "-I", str(ARCHIVE / "source.py")]
    )
    after = receipt("source-after-final", ["python3", "-I", str(ARCHIVE / "source.py")])
    need(before == after, "unchanged full source inventory")
    cohort = json.loads(before)
    need(
        cohort["base"] == COMMIT and len(cohort["files"]) == 5553,
        "exact containing source cohort",
    )
    selectors = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo",
        "crates",
        "examples",
        "benchmarks/runtime_gfx942",
        "scripts/unsafe-source-baseline.json",
        "docs/runtime-primary-queue-release-v1.md",
    ]
    roster = (
        subprocess.check_output(
            ["git", "ls-tree", "-r", "--name-only", "-z", COMMIT, "--", *selectors],
            cwd=ROOT,
        )
        .decode()
        .split("\0")
    )
    need(set(roster) - {""} == set(cohort["files"]), "complete signed source roster")
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    need(
        current == cohort,
        "current scoped source still matches qualified commit and bytes",
    )
    receipt("rustc-final", ["rustc", "-vV"])
    receipt("cargo-final", ["cargo", "-V"])
    rosters, built = {}, {}
    for target in ("gnu", "musl"):
        command = shlex.split(ENV) + [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-features",
            "--lib",
        ]
        if target == "musl":
            command += ["--target", "x86_64-unknown-linux-musl"]
        build = receipt(f"{target}-runtime-build-final", command + ["--no-run"])
        paths = re.findall(r"^  Executable .+ \((target/[^)]+)\)$", build, re.MULTILINE)
        need(len(paths) == 1, "single runtime executable")
        built[target] = paths[0]
        output = receipt(f"{target}-runtime-final", command)
        rosters[target] = helper.parse(output, 1105, 20)
        running = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$", output, re.MULTILINE
        )
        need(running == paths, "executed build-selected harness")
    need(rosters["gnu"] == rosters["musl"], "identical complete named outcome rosters")
    for kind in ("host", "device"):
        name = f"kfd_backend::retained_release_tests::cold_allocation::native_runtime_cold_{kind}_capacity_refunds_context_and_retries"
        need(
            rosters["musl"].get(name) == "ignored",
            "native case is compiled but not CPU executed",
        )
    binaries = [
        receipt(
            f"binaries-{phase}-final",
            ["python3", "-I", str(ARCHIVE / "binaries.py"), "final"],
        )
        for phase in ("before", "after")
    ]
    need(binaries[0] == binaries[1], "unchanged executable bytes")
    parsed = json.loads(binaries[0])
    need(set(parsed) == {"gnu-runtime", "musl-runtime"}, "exact executable roster")
    for target in ("gnu", "musl"):
        row = parsed[f"{target}-runtime"]
        need(
            set(row) == {"path", "sha256"} and row["path"] == built[target],
            "hashed executable is build-selected and executed",
        )
        path = Path(row["path"])
        need(not path.is_absolute() and ".." not in path.parts, "executable path")
        need(
            sha((ROOT / path).read_bytes()) == row["sha256"],
            "current qualified executable identity",
        )
    chain = [
        "source-before-final",
        "rustc-final",
        "cargo-final",
        "gnu-runtime-build-final",
        "musl-runtime-build-final",
        "binaries-before-final",
        "gnu-runtime-final",
        "musl-runtime-final",
        "binaries-after-final",
        "source-after-final",
    ]
    for left, right in zip(chain, chain[1:]):
        need(
            TIMES[left][1] <= TIMES[right][0],
            f"ordered qualification: {left} then {right}",
        )
    process = subprocess.Popen(
        ["git", "cat-file", "--batch"],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
    )
    try:
        for path, digest in cohort["files"].items():
            need(
                not Path(path).is_absolute()
                and ".." not in Path(path).parts
                and "\n" not in path,
                "source path",
            )
            process.stdin.write(f"{COMMIT}:{path}\n".encode())
            process.stdin.flush()
            header = process.stdout.readline().split()
            need(len(header) == 3 and header[1] == b"blob", "source blob exists")
            data = process.stdout.read(int(header[2]))
            need(
                process.stdout.read(1) == b"\n" and sha(data) == digest,
                f"signed source: {path}",
            )
        process.stdin.close()
        need(process.wait(timeout=30) == 0, "source comparison completed")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
    print(
        json.dumps(
            {
                "commit": COMMIT,
                "source_files_matched": len(cohort["files"]),
                "gnu_passed": 1105,
                "musl_passed": 1105,
                "ignored_each": 20,
                "binary_identities_unchanged": True,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
