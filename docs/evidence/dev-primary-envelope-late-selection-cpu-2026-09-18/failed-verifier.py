#!/usr/bin/env python3
"""Verify both CPU cohorts and the exact late-selection source delta."""

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
PRELIMINARY_HASH = "6fad1d1a167da1f5aa3ef5932d25cd92c91c105b9ac39fa8e8e7234790da2945"
FINAL_HASH = "2b5adf15ec70a5c039c5a531efb6609386b126909a711fdf81e2a9808c1d7a4e"
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
PRELIMINARY_SCRIPT_HASHES = {
    "binaries.py": "76dd70d1401208a9d581a564e117ab9018da3c90f4489b7ba804a658d1085ee0",
    "qualify.sh": "3de14f014b245b28399f20235aa84b896da59cc04488d7fe868e6a2160b09f36",
    "record.sh": "0c8509a307fb890fcf285e53088281fd53c1bbfd5e2f0a378775c78d8d6c872d",
    "seal.sh": "f718f339a673783f56daaa166f775677c321f5822f9be31e74156ab33f5df882",
    "source.py": "a7a8b8e73aa1a0817e13f9e575369403ece42f4eb702fd1ec04d9bc57fcff953",
    "verify.py": "f3927b2f9bd8f80a44ab117f5688e04945d0ec9a6dcdbbdb684ba0a4a1876f1c",
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
    binaries = ["python3", "-I", str(ARCHIVE / "binaries.py")]
    if suffix:
        binaries.append("final")
    return {
        f"source-before{tag}": ["python3", "-I", str(ARCHIVE / "source.py")],
        f"rustc{tag}": ["rustc", "-vV"],
        f"cargo{tag}": ["cargo", "-V"],
        f"gnu-runtime-build{tag}": ENV + runtime + ["--no-run"],
        f"musl-runtime-build{tag}": musl_env + runtime + musl_target + ["--no-run"],
        f"binaries-before{tag}": binaries,
        f"gnu-runtime{tag}": ENV + runtime,
        f"musl-runtime{tag}": musl_env + runtime + musl_target,
        f"binaries-after{tag}": binaries,
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


def script_commands():
    scripts = [
        str(ARCHIVE / name) for name in ("source.py", "binaries.py", "verify.py")
    ]
    shells = [
        str(ARCHIVE / name)
        for name in ("record.sh", "qualify.sh", "qualify-final.sh", "seal.sh")
    ]
    return {
        "python-lint": ["ruff", "check", "--no-cache"] + scripts,
        "python-format": ["ruff", "format", "--check", "--no-cache"] + scripts,
        "shell-syntax": ["bash", "-n"] + shells,
        "shellcheck": ["shellcheck"] + shells,
        "verify": ["python3", "-I", str(ARCHIVE / "verify.py")],
    }


def signed_source_deltas(source):
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


def verify_campaign(helper, suffix, current_binaries):
    tag = f"-{suffix}" if suffix else ""
    rosters = {
        target: helper.parse(raw(f"{target}-runtime{tag}"), 1102, 20)
        for target in ("gnu", "musl")
    }
    assert rosters["gnu"] == rosters["musl"]
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
        if current_binaries:
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
    snapshots = ARCHIVE / "preliminary-scripts"
    assert {path.name for path in snapshots.iterdir()} == set(PRELIMINARY_SCRIPT_HASHES)
    assert {
        name: digest(snapshots / name) for name in PRELIMINARY_SCRIPT_HASHES
    } == PRELIMINARY_SCRIPT_HASHES
    spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)

    preliminary_all = campaign_commands("")
    preliminary_names = list(preliminary_all)[:13]
    final = campaign_commands("final")
    scripts = script_commands()
    ordered = (
        [
            (name, preliminary_all[name], 1 if name == "fmt" else 0)
            for name in preliminary_names
        ]
        + [(name, command, 0) for name, command in final.items()]
        + [(name, command, 0) for name, command in scripts.items()]
    )
    expected_raw = {
        f"{name}.{suffix}"
        for name, _, _ in ordered
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    pending = not (ARCHIVE / "raw/verify.exit").exists()
    if pending:
        expected_raw -= {"verify.finished", "verify.exit"}
        assert (
            shlex.split((ARCHIVE / "raw/verify.command").read_text())
            == scripts["verify"]
        )
    times = []
    for name, command, code in ordered:
        if name == "verify" and pending:
            continue
        times.append(receipt(name, command, code))
    assert all(left[1] <= right[0] for left, right in zip(times, times[1:]))
    assert {path.name for path in (ARCHIVE / "raw").iterdir()} == expected_raw
    assert "Diff in " in raw("fmt") and "SELECTION" in raw("fmt")

    signed = json.loads(OLD_SOURCE.read_text())
    preliminary = json.loads(raw("source-before"))
    final_before = json.loads(raw("source-before-final"))
    final_after = json.loads(raw("source-after-final"))
    for snapshot in (signed, preliminary, final_before, final_after):
        assert len(snapshot["files"]) == 5553
        assert re.fullmatch(r"[0-9a-f]{40}", snapshot["base"])
    assert (
        signed["files"].keys()
        == preliminary["files"].keys()
        == final_before["files"].keys()
    )
    assert final_before["files"] == final_after["files"]
    assert signed_source_deltas(signed["files"]) == set()
    assert signed_source_deltas(preliminary["files"]) == {CHANGED}
    assert signed_source_deltas(final_before["files"]) == {CHANGED}
    assert signed["files"][CHANGED] == BASE_HASH
    assert preliminary["files"][CHANGED] == PRELIMINARY_HASH
    assert final_before["files"][CHANGED] == FINAL_HASH
    assert {
        name
        for name in preliminary["files"]
        if preliminary["files"][name] != final_before["files"][name]
    } == {CHANGED}
    live = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert live["files"] == final_before["files"]

    preliminary_roster, preliminary_binaries, preliminary_docs = verify_campaign(
        helper, "", False
    )
    final_roster, final_binaries, final_docs = verify_campaign(helper, "final", True)
    assert preliminary_roster == final_roster
    old = helper.parse(OLD_GNU.read_text(), 1102, 20)
    assert old == helper.parse(OLD_MUSL.read_text(), 1102, 20) == final_roster
    for name in (
        "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_error_retains_installed_root",
        "kfd_backend::retained_release_tests::primary_envelope::native_runtime_primary_release_panic_retains_installed_root_and_payload",
    ):
        assert final_roster[name] == "ignored"
    assert len(helper.parse(raw("unsafe-source-final"), 5, 1)) == 6

    print(
        json.dumps(
            {
                "audit": "PASS",
                "scope": "CPU only; native primary-release probes remain ignored",
                "selected_source_files": len(final_before["files"]),
                "signed_source_deltas": sorted(
                    signed_source_deltas(final_before["files"])
                ),
                "preliminary_source_deltas": 1,
                "preliminary_fmt_exit": 1,
                "runtime_each": {"passed": 1102, "ignored": 20},
                "doctests_each": sum(map(len, final_docs)),
                "preliminary_doctests": sum(map(len, preliminary_docs)),
                "unsafe_policy_final": {"passed": 5, "ignored": 1},
                "preliminary_test_binaries": len(preliminary_binaries),
                "final_test_binaries": len(final_binaries),
                "receipt_roster": len(ordered),
                "successful_receipts": len(ordered) - 1,
                "preserved_preliminary_failures": 1,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
