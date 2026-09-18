#!/usr/bin/env python3
"""Verify the late-selection CPU qualification and exact source delta."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
assert __debug__, "verification requires Python assertions"
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
BASE = "8b0021ba740c337b2641ecd26ed6c9375613ed32"
CHANGED = (
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/primary_envelope.rs"
)
BASE_HASH = "85aae42b342167c40634f032998276c1897e88eea21577dd50a39358a9081e4a"
QUALIFIED_HASH = "6fad1d1a167da1f5aa3ef5932d25cd92c91c105b9ac39fa8e8e7234790da2945"
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
OLD_ARCHIVE = ARCHIVE.parent / "dev-primary-envelope-cpu-2026-09-18"
OLD_SOURCE = OLD_ARCHIVE / "raw/source-after-final.log"
OLD_GNU = OLD_ARCHIVE / "raw/gnu-runtime-final.log"
OLD_MUSL = OLD_ARCHIVE / "raw/musl-runtime-final.log"
ENV = [
    "env",
    "CARGO_INCREMENTAL=0",
    "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0",
    "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never",
    "RUST_TEST_THREADS=1",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def receipt(name, command):
    paths = {
        suffix: ARCHIVE / f"raw/{name}.{suffix}"
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), (
        name
    )
    assert shlex.split(paths["command"].read_text()) == command, name
    assert paths["exit"].read_text() == "0\n", name
    times = [paths[field].read_text().strip() for field in ("started", "finished")]
    assert all(
        re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z", value) for value in times
    ), name
    assert times[0] <= times[1], name
    return times


def parse_doctests(text):
    lines = text.splitlines()
    index = 0
    rosters = []
    counts = []
    while index < len(lines):
        line = lines[index]
        index += 1
        if not line.strip() or line == "   Doc-tests fe2o3_runtime":
            continue
        running = re.fullmatch(r"running ([0-9]+) tests?", line)
        if running:
            count = int(running[1])
            assert count > 0
            counts.append(count)
            roster = {}
            for _ in range(count):
                row = re.fullmatch(
                    r"test (crates/[^ ]+ - .+ \(line [0-9]+\)) - (compile|compile fail) \.\.\. ok",
                    lines[index],
                )
                index += 1
                assert row and row[1] not in roster
                roster[row[1]] = row[2]
            rosters.append(roster)
            assert lines[index] == ""
            index += 1
            assert re.fullmatch(
                rf"test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s",
                lines[index],
            )
            index += 1
            continue
        assert re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|all doctests ran in [0-9.]+s; merged doctests compilation took [0-9.]+s",
            line,
        ), line
    assert counts == [4, 42]
    assert sum(map(len, rosters)) == 46
    return rosters


def commands():
    runtime = [
        "cargo",
        "test",
        "--frozen",
        "-p",
        "fe2o3-runtime",
        "--all-features",
        "--lib",
    ]
    musl_env = ENV + ["FE2O3_HIP_SYS_DISABLE=1"]
    musl_target = ["--target", "x86_64-unknown-linux-musl"]
    scripts = [
        str(ARCHIVE / name) for name in ("source.py", "binaries.py", "verify.py")
    ]
    return {
        "source-before": ["python3", "-I", str(ARCHIVE / "source.py")],
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
        "gnu-runtime-build": ENV + runtime + ["--no-run"],
        "musl-runtime-build": musl_env + runtime + musl_target + ["--no-run"],
        "binaries-before": ["python3", "-I", str(ARCHIVE / "binaries.py")],
        "gnu-runtime": ENV + runtime,
        "musl-runtime": musl_env + runtime + musl_target,
        "binaries-after": ["python3", "-I", str(ARCHIVE / "binaries.py")],
        "doctests": ENV
        + [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-features",
            "--doc",
        ],
        "clippy": ENV
        + [
            "cargo",
            "clippy",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        "no-default": ENV
        + [
            "cargo",
            "check",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--no-default-features",
        ],
        "fmt": ENV + ["cargo", "fmt", "--all", "--", "--check"],
        "unsafe-source": ENV
        + [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "cargo-fe2o3",
            "--test",
            "unsafe_source_policy",
        ],
        "source-after": ["python3", "-I", str(ARCHIVE / "source.py")],
        "python-lint": ["ruff", "check", "--no-cache"] + scripts,
        "python-format": ["ruff", "format", "--check", "--no-cache"] + scripts,
        "shell-syntax": [
            "bash",
            "-n",
            str(ARCHIVE / "record.sh"),
            str(ARCHIVE / "qualify.sh"),
            str(ARCHIVE / "seal.sh"),
        ],
        "shellcheck": [
            "shellcheck",
            str(ARCHIVE / "record.sh"),
            str(ARCHIVE / "qualify.sh"),
            str(ARCHIVE / "seal.sh"),
        ],
        "verify": ["python3", "-I", str(ARCHIVE / "verify.py")],
    }


def signed_source_matches(source):
    process = subprocess.Popen(
        ["git", "cat-file", "--batch"],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    different = set()
    try:
        for name, observed in source.items():
            process.stdin.write(f"{BASE}:{name}\n".encode())
            process.stdin.flush()
            header = process.stdout.readline().decode().split()
            assert len(header) == 3 and header[1] == "blob", name
            remaining = int(header[2])
            hasher = hashlib.sha256()
            while remaining:
                data = process.stdout.read(min(remaining, 1024 * 1024))
                assert data
                hasher.update(data)
                remaining -= len(data)
            assert process.stdout.read(1) == b"\n"
            if hasher.hexdigest() != observed:
                different.add(name)
        process.stdin.close()
        assert process.wait(timeout=30) == 0
        assert not process.stderr.read()
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
    return different


def verify():
    assert (
        digest(HELPER)
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    )
    assert (
        digest(OLD_SOURCE)
        == "d9548dafe70e715598e2d103e26f6fd568070a279f7b8b2d5395f4a19a083f4d"
    )
    assert (
        digest(OLD_GNU)
        == "376d52437f41c9ce02357a504206091ba0b6975c569aa563085300b763a0d945"
    )
    assert (
        digest(OLD_MUSL)
        == "43f0f72cbcc9bc83dbfd2f8a95d18d9fec16df40d9b85b7d36f8fe73fddb9a1f"
    )
    spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)

    expected_commands = commands()
    expected_raw = {
        f"{name}.{suffix}"
        for name in expected_commands
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    pending = not (ARCHIVE / "raw/verify.exit").exists()
    if pending:
        expected_raw -= {"verify.finished", "verify.exit"}
        assert (
            shlex.split((ARCHIVE / "raw/verify.command").read_text())
            == expected_commands["verify"]
        )
    times = []
    for name, command in expected_commands.items():
        if name == "verify" and pending:
            continue
        times.append(receipt(name, command))
    assert all(left[1] <= right[0] for left, right in zip(times, times[1:]))
    assert {path.name for path in (ARCHIVE / "raw").iterdir()} == expected_raw

    current = json.loads(raw("source-before"))
    after = json.loads(raw("source-after"))
    assert current["files"] == after["files"]
    assert len(current["files"]) == 5553
    assert re.fullmatch(r"[0-9a-f]{40}", current["base"])
    assert re.fullmatch(r"[0-9a-f]{40}", after["base"])
    old = json.loads(OLD_SOURCE.read_text())
    assert current["files"].keys() == old["files"].keys()
    changed_from_old = {
        name
        for name in current["files"]
        if current["files"][name] != old["files"][name]
    }
    assert changed_from_old == {CHANGED}
    assert old["files"][CHANGED] == BASE_HASH
    assert current["files"][CHANGED] == QUALIFIED_HASH
    assert signed_source_matches(current["files"]) == {CHANGED}
    assert (
        hashlib.sha256(
            subprocess.check_output(["git", "show", f"{BASE}:{CHANGED}"], cwd=ROOT)
        ).hexdigest()
        == BASE_HASH
    )
    live = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert live["files"] == current["files"]

    rosters = {
        target: helper.parse(raw(f"{target}-runtime"), 1102, 20)
        for target in ("gnu", "musl")
    }
    assert rosters["gnu"] == rosters["musl"]
    old_rosters = {
        "gnu": helper.parse(OLD_GNU.read_text(), 1102, 20),
        "musl": helper.parse(OLD_MUSL.read_text(), 1102, 20),
    }
    assert old_rosters["gnu"] == old_rosters["musl"] == rosters["gnu"]
    assert (
        rosters["gnu"][
            "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_error_retains_installed_root"
        ]
        == "ignored"
    )
    assert (
        rosters["gnu"][
            "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload"
        ]
        == "ignored"
    )
    assert len(helper.parse(raw("unsafe-source"), 5, 1)) == 6
    docs = parse_doctests(raw("doctests"))

    binaries = json.loads(raw("binaries-before"))
    assert binaries == json.loads(raw("binaries-after"))
    assert set(binaries) == {"gnu-runtime", "musl-runtime"}
    for name, data in binaries.items():
        assert set(data) == {"path", "sha256"}
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$", raw(name + "-build"), re.MULTILINE
        )
        run = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$", raw(name), re.MULTILINE
        )
        assert built == run == [data["path"]], name
        assert digest(ROOT / data["path"]) == data["sha256"]
    assert raw("rustc").startswith("rustc 1.96.0-nightly (55e86c996 2026-04-02)\n")
    assert raw("cargo") == "cargo 1.96.0-nightly (888f67534 2026-03-30)\n"

    print(
        json.dumps(
            {
                "audit": "PASS",
                "scope": "CPU only; native primary-release probes remain ignored",
                "selected_source_files": len(current["files"]),
                "signed_source_deltas": sorted(signed_source_matches(current["files"])),
                "runtime_each": {"passed": 1102, "ignored": 20},
                "doctests": sum(map(len, docs)),
                "unsafe_policy": {"passed": 5, "ignored": 1},
                "test_binaries": len(binaries),
                "receipt_roster": len(expected_commands),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
