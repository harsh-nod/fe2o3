#!/usr/bin/env python3
"""Read-only qualification of exact capacity, backing and live-credit accounting."""

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
PREVIOUS = ARCHIVE.parent / "dev-copy-accounting-corrected-2026-09-18"
FIXTURE = (
    "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs"
)
BASE = "b87f30d1b87b2dca29e9f03e8b00f99a65b04391"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def verify():
    parser = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
    assert (
        digest(parser)
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    )
    spec = importlib.util.spec_from_file_location("accounting_harness", parser)
    helper = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = helper
    spec.loader.exec_module(helper)
    before = json.loads(raw("source-before"))
    assert before == json.loads(raw("source-after"))
    assert before["base"] == BASE and len(before["files"]) == 5543
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert current["files"] == before["files"]
    assert (
        digest(PREVIOUS / "SHA256SUMS")
        == "6017b2350f05f4f4d37ea085ef9a45341509d35e81b7e3cfcd85adecc8d01e93"
    )
    subprocess.run(
        ["sha256sum", "--check", "--quiet", "SHA256SUMS"], cwd=PREVIOUS, check=True
    )
    assert (
        before["files"][FIXTURE]
        == "b42daaf3293afc6da164d3c0c6c5a354b12d33fb795e4317789a7938ac79a4b9"
    )
    previous = json.loads((PREVIOUS / "raw/source-after.log").read_text())
    assert previous["files"].keys() == before["files"].keys()
    assert {
        p for p in before["files"] if previous["files"][p] != before["files"][p]
    } == {FIXTURE}
    binaries = json.loads(raw("binaries-before"))
    assert binaries == json.loads(raw("binaries-after")) and set(binaries) == {
        "gnu",
        "musl",
    }
    assert (
        binaries["musl"]["sha256"]
        == "f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe"
    )
    prior_roster = helper.parse((PREVIOUS / "raw/gnu-tests.log").read_text(), 1101, 18)
    for target in ("gnu", "musl"):
        environment = [
            "env",
            "CARGO_INCREMENTAL=0",
            "CARGO_PROFILE_DEV_DEBUG=0",
            "CARGO_PROFILE_TEST_DEBUG=0",
            "CARGO_BUILD_JOBS=2",
            "CARGO_TERM_COLOR=never",
        ]
        target_env = ["FE2O3_HIP_SYS_DISABLE=1"] if target == "musl" else []
        flags = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        cargo = [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-features",
            "--lib",
        ]
        assert shlex.split(
            (ARCHIVE / f"raw/{target}-build.command").read_text()
        ) == environment + target_env + cargo + flags + ["--no-run"]
        assert (
            shlex.split((ARCHIVE / f"raw/{target}-tests.command").read_text())
            == environment + ["RUST_TEST_THREADS=1"] + target_env + cargo + flags
        )
        transcript = raw(target + "-tests")
        assert helper.parse(transcript, 1101, 18) == prior_roster
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$",
            raw(target + "-build"),
            re.MULTILINE,
        )
        executed = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$", transcript, re.MULTILINE
        )
        assert built == executed == [binaries[target]["path"]]
        assert digest(ROOT / binaries[target]["path"]) == binaries[target]["sha256"]
    assert "INTERP" not in raw("musl-elf") and "(NEEDED)" not in raw("musl-elf")
    assert len(helper.parse(raw("unsafe-source"), 5, 1)) == 6
    names = {p.stem for p in (ARCHIVE / "raw").iterdir()}
    assert names == {
        "source-before",
        "musl-build",
        "gnu-build",
        "binaries-before",
        "musl-elf",
        "gnu-tests",
        "musl-tests",
        "binaries-after",
        "clippy",
        "no-default",
        "fmt",
        "unsafe-source",
        "source-after",
        "scripts",
        "verify-final",
    }
    suffixes = {"command", "started", "finished", "exit", "log"}
    expected = {f"{name}.{suffix}" for name in names for suffix in suffixes}
    pending = not (ARCHIVE / "raw/verify-final.exit").exists()
    if pending:
        expected -= {"verify-final.exit", "verify-final.finished"}
    assert {p.name for p in (ARCHIVE / "raw").iterdir()} == expected
    for name in names:
        if name == "verify-final" and pending:
            continue
        assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n", name
        assert shlex.split((ARCHIVE / f"raw/{name}.command").read_text())
        times = [
            (ARCHIVE / f"raw/{name}.{suffix}").read_text().strip()
            for suffix in ("started", "finished")
        ]
        assert all(
            re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", t)
            for t in times
        )
        assert times[0] <= times[1]
    print(
        json.dumps(
            {
                "audit": "PASS",
                "source_files": 5543,
                "changed_source_since_initial": FIXTURE,
                "runtime_each": {"passed": 1101, "ignored": 18},
                "unsafe_policy": {"passed": 5, "ignored": 1},
                "unchanged_test_binaries": 2,
                "closed_receipts": len(names),
                "native_acceptance": "separate-packet",
                "performance_accepted": False,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
