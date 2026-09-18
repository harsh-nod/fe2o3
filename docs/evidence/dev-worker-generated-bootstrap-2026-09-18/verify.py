#!/usr/bin/env python3
"""Check exact CPU harnesses, commands, source maps and executable identities."""

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
ENV = [
    "env",
    "CARGO_INCREMENTAL=0",
    "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0",
    "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never",
    "RUST_TEST_THREADS=1",
]
BASE = "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f"
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
MUSL_WARNING = (
    "warning: fe2o3-hip-sys@0.1.0: HIP headers not found; "
    "device-property discovery will return HIP_ERROR_NOT_SUPPORTED\n"
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def receipt(name, command):
    assert shlex.split((ARCHIVE / f"raw/{name}.command").read_text()) == command, name
    assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n", name
    times = [
        (ARCHIVE / f"raw/{name}.{field}").read_text().strip()
        for field in ("started", "finished")
    ]
    assert all(re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z", t) for t in times)
    assert times[0] <= times[1]
    return times


def doctests(text):
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
            assert crate is not None and int(running[1]) > 0
            count = int(running[1])
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
    assert counts == {"fe2o3_host": [5, 21], "fe2o3_runtime": [4, 42]}, counts
    for crate, method in (
        ("fe2o3_host", "authenticate_inherited_worker_v3_application_v1"),
        ("fe2o3_runtime", "open_worker_v3_generated_only_v1"),
    ):
        assert [kind for name, kind in rosters[crate].items() if method in name] == [
            "compile"
        ]
    return rosters


def verify():
    assert (
        digest(HELPER)
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    )
    spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    commands = {
        "source-before": ["python3", "-I", str(ARCHIVE / "source.py")],
        "rustc": ["rustc", "-vV"],
        "cargo": ["cargo", "-V"],
    }
    tests = {}
    for target in ("gnu", "musl"):
        prefix = ENV + (["FE2O3_HIP_SYS_DISABLE=1"] if target == "musl" else [])
        flags = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        for crate in ("runtime", "host"):
            name = f"{target}-{crate}"
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
    commands["binaries-after"] = ["python3", "-I", str(ARCHIVE / "binaries.py")]
    packages = ["-p", "fe2o3-runtime", "-p", "fe2o3-host"]
    commands.update(
        {
            "doctests": ENV
            + ["cargo", "test", "--frozen"]
            + packages
            + ["--all-features", "--doc"],
            "public-api": ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-host",
                "--no-default-features",
                "--test",
                "production_descriptor_error_api",
            ],
            "clippy": ENV
            + ["cargo", "clippy", "--frozen"]
            + packages
            + ["--all-targets", "--all-features", "--", "-D", "warnings"],
            "no-default": ENV
            + ["cargo", "check", "--frozen"]
            + packages
            + ["--no-default-features"],
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
        }
    )
    times = [receipt(name, command) for name, command in commands.items()]
    assert all(a[1] <= b[0] for a, b in zip(times, times[1:]))
    rosters = {}
    for target in ("gnu", "musl"):
        for crate, passed, ignored in (("runtime", 1102, 18), ("host", 283, 4)):
            name = f"{target}-{crate}"
            transcript = raw(name)
            if name == "musl-host":
                assert transcript.startswith(MUSL_WARNING)
                transcript = transcript[len(MUSL_WARNING) :]
            rosters[name] = helper.parse(transcript, passed, ignored)
    for crate, archive, sha, passed, ignored, addition in (
        (
            "runtime",
            "dev-copy-accounting-live-credits-2026-09-18/raw/gnu-tests.log",
            "52bdb242f5a8c017493214fad18a28244824a5a1f3a7a3f85025da8db802d3c1",
            1101,
            18,
            "kfd_backend::tests::worker_v3_generated_only_profile_denies_generic_compute_before_encoding",
        ),
        (
            "host",
            "dev-current-thread-owner-2026-09-18/raw/gnu-host.log",
            "d7b9c2dd35b99805624de9c5ffc1b987b1ced40576c967e8264296119fc44626",
            282,
            4,
            "production_application::tests::bootstrap_error_mapping_preserves_public_variants",
        ),
    ):
        baseline = ARCHIVE.parent / archive
        assert digest(baseline) == sha
        old = helper.parse(baseline.read_text(), passed, ignored)
        current = rosters["gnu-" + crate]
        assert current == rosters["musl-" + crate]
        assert current.keys() - old.keys() == {addition} and current[addition] == "ok"
        assert {k: v for k, v in current.items() if k != addition} == old
    assert len(helper.parse(raw("unsafe-source"), 5, 1)) == 6
    assert helper.parse(raw("public-api"), 1, 0) == {
        "production_descriptor_errors_are_public_without_worker_v2": "ok"
    }
    docs = doctests(raw("doctests"))
    before = json.loads(raw("source-before"))
    assert before == json.loads(raw("source-after")) and before["base"] == BASE
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert current["files"] == before["files"]
    binaries = json.loads(raw("binaries-before"))
    assert binaries == json.loads(raw("binaries-after")) and set(binaries) == set(tests)
    for name, data in binaries.items():
        assert (
            set(data) == {"path", "sha256"}
            and digest(ROOT / data["path"]) == data["sha256"]
        )
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$", raw(name + "-build"), re.MULTILINE
        )
        run = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$", raw(name), re.MULTILINE
        )
        assert built == run == [data["path"]]
    scripts = [
        str(ARCHIVE / name) for name in ("source.py", "binaries.py", "verify.py")
    ]
    extra = {
        "python-lint": ["ruff", "check", "--no-cache"] + scripts,
        "python-format": ["ruff", "format", "--check", "--no-cache"] + scripts,
        "shellcheck": ["shellcheck"]
        + [str(ARCHIVE / name) for name in ("record.sh", "qualify.sh", "seal.sh")],
        "verify-final": ["python3", "-I", str(ARCHIVE / "verify.py")],
    }
    names = set(commands) | set(extra)
    expected = {
        f"{name}.{suffix}"
        for name in names
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    pending = not (ARCHIVE / "raw/verify-final.exit").exists()
    if pending:
        expected -= {"verify-final.exit", "verify-final.finished"}
    assert {p.name for p in (ARCHIVE / "raw").iterdir()} == expected
    for name, command in extra.items():
        if name == "verify-final" and pending:
            assert (
                shlex.split((ARCHIVE / "raw/verify-final.command").read_text())
                == command
            )
            continue
        receipt(name, command)
    print(
        json.dumps(
            {
                "audit": "PASS",
                "scope": "CPU only; no native, formal proof or performance acceptance",
                "source_files": len(before["files"]),
                "unchanged_test_binaries": len(binaries),
                "runtime_each": {"passed": 1102, "ignored": 18},
                "host_each": {"passed": 283, "ignored": 4},
                "doctests": {k: len(v) for k, v in docs.items()},
                "public_api_passed": 1,
                "unsafe_policy": {"passed": 5, "ignored": 1},
                "closed_receipts": len(names),
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
