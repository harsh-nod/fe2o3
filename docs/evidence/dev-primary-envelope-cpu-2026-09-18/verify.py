#!/usr/bin/env python3
"""Verify both CPU campaigns and the final primary-envelope source cohort."""

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
BASE = "1890a64e1911a2a346c5a9e70a8ff5ad45b19231"
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
OLD_GNU = (
    ARCHIVE.parent / "dev-worker-generated-bootstrap-2026-09-18/raw/gnu-runtime.log"
)
OLD_MUSL = (
    ARCHIVE.parent / "dev-worker-generated-bootstrap-2026-09-18/raw/musl-runtime.log"
)
NEW_TESTS = {
    "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_error_retains_installed_root",
    "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload",
}
COMMON_SOURCE_HASHES = {
    "crates/fe2o3-runtime/src/kfd_backend/compute_dispatch.rs": "4966cd8b76cfbdb4edbd39a8c1117df44d3c3d70d2178b61799090b403758f26",
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests.rs": "7c0d7fceac0a66e57af238e6e1d19dc1ba94e8c9a7f1d58fb99b853c3b6f004d",
    "scripts/unsafe-source-baseline.json": "0d190337179295fe5e9e62eaec14f55b1e756c4907e5c6c27fa865411c5e8f3c",
}
INITIAL_CHANGED_HASHES = {
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/primary_envelope.rs": "44096119050975f4c70c2320dcb59a853c575cd906d01b15b20b670412fe62ab",
    "docs/runtime-primary-queue-release-v1.md": "6f1a39232cce49b19a305f060295a916e0e407f58aab86755f89cbe37c296605",
}
FINAL_CHANGED_HASHES = {
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/primary_envelope.rs": "85aae42b342167c40634f032998276c1897e88eea21577dd50a39358a9081e4a",
    "docs/runtime-primary-queue-release-v1.md": "add69f4760de9ce61c9c4c630a8dbce95e2e37499d924af644dc16a66f38f773",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def receipt(name, command, code=0):
    paths = {
        suffix: ARCHIVE / f"raw/{name}.{suffix}"
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), (
        name
    )
    assert shlex.split(paths["command"].read_text()) == command, name
    assert paths["exit"].read_text() == f"{code}\n", name
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
        if not line.strip():
            continue
        if line == "   Doc-tests fe2o3_runtime":
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


def campaign_commands(suffix):
    tag = f"-{suffix}" if suffix else ""
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
    binary_args = ["python3", "-I", str(ARCHIVE / "binaries.py")]
    if suffix:
        binary_args.append("final")
    return {
        f"source-before{tag}": ["python3", "-I", str(ARCHIVE / "source.py")],
        f"rustc{tag}": ["rustc", "-vV"],
        f"cargo{tag}": ["cargo", "-V"],
        f"gnu-runtime-build{tag}": ENV + runtime + ["--no-run"],
        f"musl-runtime-build{tag}": musl_env + runtime + musl_target + ["--no-run"],
        f"binaries-before{tag}": binary_args,
        f"gnu-runtime{tag}": ENV + runtime,
        f"musl-runtime{tag}": musl_env + runtime + musl_target,
        f"binaries-after{tag}": binary_args,
        f"doctests{tag}": ENV
        + [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-features",
            "--doc",
        ],
        f"clippy{tag}": ENV
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
        f"no-default{tag}": ENV
        + [
            "cargo",
            "check",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--no-default-features",
        ],
        f"fmt{tag}": ENV + ["cargo", "fmt", "--all", "--", "--check"],
        f"unsafe-source{tag}": ENV
        + [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "cargo-fe2o3",
            "--test",
            "unsafe_source_policy",
        ],
        f"source-after{tag}": ["python3", "-I", str(ARCHIVE / "source.py")],
    }


def verify_campaign(helper, suffix):
    tag = f"-{suffix}" if suffix else ""
    rosters = {
        target: helper.parse(raw(f"{target}-runtime{tag}"), 1102, 20)
        for target in ("gnu", "musl")
    }
    assert rosters["gnu"] == rosters["musl"]
    assert len(helper.parse(raw(f"unsafe-source{tag}"), 5, 1)) == 6
    docs = parse_doctests(raw(f"doctests{tag}"))
    binaries = json.loads(raw(f"binaries-before{tag}"))
    assert binaries == json.loads(raw(f"binaries-after{tag}"))
    assert set(binaries) == {"gnu-runtime", "musl-runtime"}
    for name, data in binaries.items():
        assert set(data) == {"path", "sha256"}
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$",
            raw(name + "-build" + tag),
            re.MULTILINE,
        )
        run = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$",
            raw(name + tag),
            re.MULTILINE,
        )
        assert built == run == [data["path"]], name + tag
        if suffix:
            assert digest(ROOT / data["path"]) == data["sha256"]
    assert raw(f"rustc{tag}").startswith(
        "rustc 1.96.0-nightly (55e86c996 2026-04-02)\n"
    )
    assert raw(f"cargo{tag}") == "cargo 1.96.0-nightly (888f67534 2026-03-30)\n"
    return rosters["gnu"], binaries, docs


def verify():
    assert (
        digest(HELPER)
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    )
    assert (
        digest(OLD_GNU)
        == "44504c4d9056e000e0a68fc26294eed6b1662c85876b7575fa759dc3a94692fd"
    )
    assert (
        digest(OLD_MUSL)
        == "0bca6034e8cfbb0542a9a7f3f28dcae4e747ae81c8cc7d918d4c5c8af23ae62e"
    )
    spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)

    initial_commands = campaign_commands("")
    scripts = [
        str(ARCHIVE / name) for name in ("source.py", "binaries.py", "verify.py")
    ]
    preliminary = {
        "python-lint": (["ruff", "check", "--no-cache"] + scripts, 0),
        "python-format": (
            ["ruff", "format", "--check", "--no-cache"] + scripts,
            1,
        ),
        "shellcheck": (
            ["shellcheck"]
            + [str(ARCHIVE / name) for name in ("record.sh", "qualify.sh", "seal.sh")],
            0,
        ),
        "verify-final": (["python3", "-I", str(ARCHIVE / "verify.py")], 1),
    }
    final_commands = campaign_commands("final")
    final_scripts = {
        "python-lint-final": ["ruff", "check", "--no-cache"] + scripts,
        "python-format-final": ["ruff", "format", "--check", "--no-cache"] + scripts,
        "shellcheck-final": ["shellcheck"]
        + [
            str(ARCHIVE / name)
            for name in ("record.sh", "qualify.sh", "qualify-final.sh", "seal.sh")
        ],
        "verify-final-v2": ["python3", "-I", str(ARCHIVE / "verify.py")],
    }

    times = [receipt(name, command) for name, command in initial_commands.items()]
    for name, (command, code) in preliminary.items():
        times.append(receipt(name, command, code))
    times.extend(receipt(name, command) for name, command in final_commands.items())
    for name, command in final_scripts.items():
        if name != "verify-final-v2":
            times.append(receipt(name, command))

    names = (
        set(initial_commands)
        | set(preliminary)
        | set(final_commands)
        | set(final_scripts)
    )
    expected = {
        f"{name}.{suffix}"
        for name in names
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    pending = not (ARCHIVE / "raw/verify-final-v2.exit").exists()
    if pending:
        expected -= {"verify-final-v2.exit", "verify-final-v2.finished"}
        assert (
            shlex.split((ARCHIVE / "raw/verify-final-v2.command").read_text())
            == final_scripts["verify-final-v2"]
        )
    else:
        times.append(receipt("verify-final-v2", final_scripts["verify-final-v2"]))
    assert all(left[1] <= right[0] for left, right in zip(times, times[1:]))
    assert {path.name for path in (ARCHIVE / "raw").iterdir()} == expected

    initial_roster, initial_binaries, initial_docs = verify_campaign(helper, "")
    final_roster, final_binaries, final_docs = verify_campaign(helper, "final")
    assert initial_roster == final_roster
    old = helper.parse(OLD_GNU.read_text(), 1102, 18)
    assert old == helper.parse(OLD_MUSL.read_text(), 1102, 18)
    assert final_roster.keys() - old.keys() == NEW_TESTS
    assert all(final_roster[name] == "ignored" for name in NEW_TESTS)
    assert {
        name: status for name, status in final_roster.items() if name not in NEW_TESTS
    } == old

    initial_source = json.loads(raw("source-before"))
    assert initial_source == json.loads(raw("source-after"))
    final_source = json.loads(raw("source-before-final"))
    assert final_source == json.loads(raw("source-after-final"))
    assert initial_source["base"] == final_source["base"] == BASE
    assert len(initial_source["files"]) == len(final_source["files"]) == 5553
    assert initial_source["files"].keys() == final_source["files"].keys()
    for name, expected_hash in COMMON_SOURCE_HASHES.items():
        assert (
            initial_source["files"][name]
            == final_source["files"][name]
            == expected_hash
        )
    assert {
        name: initial_source["files"][name] for name in INITIAL_CHANGED_HASHES
    } == INITIAL_CHANGED_HASHES
    assert {
        name: final_source["files"][name] for name in FINAL_CHANGED_HASHES
    } == FINAL_CHANGED_HASHES
    changed = {
        name
        for name in initial_source["files"]
        if initial_source["files"][name] != final_source["files"][name]
    }
    assert changed == set(FINAL_CHANGED_HASHES)
    live = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert live["files"] == final_source["files"]

    print(
        json.dumps(
            {
                "audit": "PASS",
                "scope": "CPU only; native primary-release probes remain ignored",
                "initial_source_files": len(initial_source["files"]),
                "final_source_files": len(final_source["files"]),
                "source_changes_between_campaigns": len(changed),
                "initial_test_binaries": len(initial_binaries),
                "final_test_binaries": len(final_binaries),
                "runtime_each": {"passed": 1102, "ignored": 20},
                "new_ignored_tests": len(NEW_TESTS),
                "doctests_each": sum(map(len, final_docs)),
                "initial_doctests": sum(map(len, initial_docs)),
                "unsafe_policy_each": {"passed": 5, "ignored": 1},
                "closed_receipts": len(names),
                "successful_receipts": len(names) - 2,
                "preserved_preliminary_failures": 2,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
