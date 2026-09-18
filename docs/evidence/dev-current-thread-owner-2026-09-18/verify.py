#!/usr/bin/env python3
"""Verify full CPU harnesses and exact source/executable qualification identity."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
assert (
    hashlib.sha256(HELPER.read_bytes()).hexdigest()
    == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
)
spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
ENV = [
    "env",
    "CARGO_INCREMENTAL=0",
    "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0",
    "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never",
    "RUST_TEST_THREADS=1",
]
BASE = "992dc9d4cafda47b93d8a698b42ce84e86cb8fb0"
MUSL_HIP_WARNING = (
    "warning: fe2o3-hip-sys@0.1.0: HIP headers not found; "
    "device-property discovery will return HIP_ERROR_NOT_SUPPORTED\n"
)


def receipt(name, command, code=0):
    paths = {
        suffix: ARCHIVE / f"raw/{name}.{suffix}"
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), (
        name
    )
    assert paths["exit"].read_text() == f"{code}\n", name
    assert shlex.split(paths["command"].read_text()) == command, name
    times = [paths[key].read_text().strip() for key in ("started", "finished")]
    assert all(
        re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value)
        for value in times
    )
    assert times[0] <= times[1]
    return times


def docs(text):
    lines, index, crate = text.splitlines(), 0, None
    rosters, counts = {}, {}
    while index < len(lines):
        line = lines[index]
        index += 1
        if not line.strip():
            continue
        header = re.fullmatch(r"   Doc-tests (fe2o3_host|fe2o3_runtime)", line)
        if header:
            crate = header[1]
            assert crate not in rosters
            rosters[crate], counts[crate] = {}, []
            continue
        running = re.fullmatch(r"running ([0-9]+) tests?", line)
        if running:
            assert crate is not None
            count = int(running[1])
            assert count > 0
            counts[crate].append(count)
            for _ in range(count):
                row = re.fullmatch(
                    r"test (crates/[^ ]+ - .+ \(line [0-9]+\)) - (compile|compile fail) \.\.\. ok",
                    lines[index],
                )
                index += 1
                assert row and row[1] not in rosters[crate]
                rosters[crate][row[1]] = row[2]
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
    assert counts == {"fe2o3_host": [4, 21], "fe2o3_runtime": [2, 42]}, counts
    bundle = [
        kind
        for name, kind in rosters["fe2o3_host"].items()
        if "/bundle_completion.rs " in name
    ]
    assert sorted(bundle) == ["compile"] + ["compile fail"] * 5
    return rosters


def verify():
    commands = {
        "rustc": ["rustc", "--version", "--verbose"],
        "cargo": ["cargo", "--version", "--verbose"],
        "source-before": ["python3", "-I", str(ARCHIVE / "source.py")],
    }
    tests = {}
    for target in ("gnu", "musl"):
        prefix = ENV + (["FE2O3_HIP_SYS_DISABLE=1"] if target == "musl" else [])
        flags = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        for crate in ("host", "runtime"):
            name = target + "-" + crate
            tests[name] = (
                prefix
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "fe2o3-" + crate,
                    "--all-features",
                    "--lib",
                ]
                + flags
            )
            commands[name + "-build"] = tests[name] + ["--no-run"]
    commands["binaries-before"] = ["python3", "-I", str(ARCHIVE / "binaries.py")]
    commands.update(tests)
    commands.update(
        {
            "binaries-after": ["python3", "-I", str(ARCHIVE / "binaries.py")],
            "doctests": ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--all-features",
                "--doc",
            ],
            "no-default": ENV
            + [
                "cargo",
                "check",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--no-default-features",
            ],
            "clippy": ENV
            + [
                "cargo",
                "clippy",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
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
            "fmt": ENV + ["cargo", "fmt", "--all", "--", "--check"],
            "source-after": ["python3", "-I", str(ARCHIVE / "source.py")],
        }
    )
    times = {name: receipt(name, command) for name, command in commands.items()}
    for before, after in zip(list(commands), list(commands)[1:]):
        assert times[before][1] <= times[after][0], (before, after)
    rosters = {}
    for target in ("gnu", "musl"):
        for crate, passed, ignored in (("host", 282, 4), ("runtime", 1088, 17)):
            name = target + "-" + crate
            transcript = (ARCHIVE / f"raw/{name}.log").read_text()
            if name == "musl-host":
                assert transcript.startswith(MUSL_HIP_WARNING)
                transcript = transcript[len(MUSL_HIP_WARNING) :]
            rosters[name] = helper.parse(transcript, passed, ignored)
    for crate in ("host", "runtime"):
        assert rosters["gnu-" + crate] == rosters["musl-" + crate]
    additions = {
        name
        for name in rosters["gnu-runtime"]
        if "::current_thread_tests::" in name
        or "::scheduler::tests::current_thread_" in name
    }
    assert len(additions) == 21 and all(
        rosters["gnu-runtime"][name] == "ok" for name in additions
    )
    baselines = {
        "host": (
            "44e328f663a18742adf85e330088aa1319724394ddc483c3af06fd340555873b",
            282,
            4,
        ),
        "runtime": (
            "a75cfd85f38cc3c12e129ff0bdfe17ea216178e7fb9c52f2d53154c960044c56",
            1067,
            17,
        ),
    }
    for crate, (digest, passed, ignored) in baselines.items():
        raw = (
            ARCHIVE.parent / f"dev-c5-typed-bundle-2026-09-18/raw/gnu-{crate}.log"
        ).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == digest
        previous = helper.parse(raw.decode(), passed, ignored)
        current_roster = rosters["gnu-" + crate]
        added = additions if crate == "runtime" else set()
        assert current_roster.keys() - previous.keys() == added
        assert {k: v for k, v in current_roster.items() if k not in added} == previous
    unsafe = helper.parse((ARCHIVE / "raw/unsafe-source.log").read_text(), 5, 1)
    doc_rosters = docs((ARCHIVE / "raw/doctests.log").read_text())
    before = json.loads((ARCHIVE / "raw/source-before.log").read_text())
    after = json.loads((ARCHIVE / "raw/source-after.log").read_text())
    assert before == after and before["base"] == BASE
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert current["files"] == after["files"] and len(after["files"]) == 5538
    binaries = json.loads((ARCHIVE / "raw/binaries-before.log").read_text())
    assert binaries == json.loads((ARCHIVE / "raw/binaries-after.log").read_text())
    assert set(binaries) == set(tests)
    for name, data in binaries.items():
        assert set(data) == {"path", "sha256"}
        assert (
            hashlib.sha256((ROOT / data["path"]).read_bytes()).hexdigest()
            == data["sha256"]
        )
        paths = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$",
            (ARCHIVE / f"raw/{name}.log").read_text(),
            re.MULTILINE,
        )
        assert paths == [data["path"]], name
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$",
            (ARCHIVE / f"raw/{name}-build.log").read_text(),
            re.MULTILINE,
        )
        assert built == paths
    # Every retained command is closed, including failed development attempts.
    suffixes = {"command", "started", "finished", "exit", "log"}
    names = {p.stem for p in (ARCHIVE / "raw").iterdir()}
    expected = {f"{name}.{suffix}" for name in names for suffix in suffixes}
    actual = {p.name for p in (ARCHIVE / "raw").iterdir()}
    pending_final = (
        "verify-final" in names and not (ARCHIVE / "raw/verify-final.exit").exists()
    )
    if pending_final:
        expected -= {"verify-final.exit", "verify-final.finished"}
    assert actual == expected, actual ^ expected
    failures = {
        "exploratory-current-thread",
        "exploratory-current-thread-expanded",
        "exploratory-clippy",
        "exploratory-clippy-corrected",
    }
    for name in names:
        if name == "verify-final" and pending_final:
            continue
        command = shlex.split((ARCHIVE / f"raw/{name}.command").read_text())
        receipt(name, command, 101 if name in failures else 0)
    print(
        json.dumps(
            {
                "source_files": len(after["files"]),
                "library_rosters": {k: len(v) for k, v in rosters.items()},
                "new_runtime_tests": len(additions),
                "doctests": {k: len(v) for k, v in doc_rosters.items()},
                "unsafe_source": len(unsafe),
                "receipts": len(names),
                "source_and_binaries_unchanged": True,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
