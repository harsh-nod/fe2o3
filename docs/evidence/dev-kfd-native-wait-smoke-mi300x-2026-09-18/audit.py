#!/usr/bin/env python3
"""Bind native smoke acceptance to the fixed source and closed receipts."""

from datetime import datetime
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
spec = importlib.util.spec_from_file_location("smoke_report", ARCHIVE / "report.py")
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)
s = report.s
RELATIVE = "docs/evidence/" + ARCHIVE.name
PRIOR_RELATIVE = "docs/evidence/" + report.PRIOR.name
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]


def remote(script):
    return SSH + ["bash", report.OWNED + "/" + script, report.OWNED]


def python(script):
    return ["python3", "-I", RELATIVE + "/" + script]


COMMANDS = {
    "create": SSH + ["bash", "-s"],
    "stage-build": [
        "scp",
        "-q",
        PRIOR_RELATIVE + "/prepare.sh",
        PRIOR_RELATIVE + "/guard.sh",
        RELATIVE + "/cleanup.sh",
        "mi300x:" + report.OWNED + "/",
    ],
    "prepare": SSH
    + [
        "timeout",
        "--signal=TERM",
        "--kill-after=10s",
        "900s",
        "bash",
        report.OWNED + "/prepare.sh",
        report.OWNED,
    ],
    "stage-run": [
        "scp",
        "-q",
        RELATIVE + "/run.sh",
        RELATIVE + "/inspect.sh",
        "mi300x:" + report.OWNED + "/",
    ],
    "shell-lint": ["bash", RELATIVE + "/lint-shell.sh"],
    "inspect-before": remote("inspect.sh"),
    "occupancy-before": SSH + ["bash", report.OWNED + "/guard.sh"],
    "native": SSH
    + [
        "timeout",
        "--signal=TERM",
        "--kill-after=10s",
        "480s",
        "bash",
        report.OWNED + "/run.sh",
        report.OWNED,
    ],
    "parser-tests": python("test_report.py") + ["-v"],
    "native-report": python("report.py"),
    "inspect-after": remote("inspect.sh"),
    "collect-results": [
        "scp",
        "-r",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "mi300x:" + report.OWNED + "/results",
        RELATIVE + "/",
    ],
    "parser-tests-fixed": python("test_report.py") + ["-v"],
    "cleanup": remote("cleanup.sh"),
    "python-lint": ["ruff", "check", RELATIVE],
    "shell-lint-final": ["bash", RELATIVE + "/lint-shell.sh"],
}


def manifest(path):
    entries = {}
    for line in path.read_text().splitlines():
        digest, name = line.split("  ", 1)
        assert re.fullmatch(r"[0-9a-f]{64}", digest) and name not in entries
        entries[name] = digest
    assert entries
    return entries


def audit():
    receipts = {}
    for name, command in COMMANDS.items():
        paths = {
            suffix: ARCHIVE / "raw" / f"{name}.{suffix}"
            for suffix in ("command", "started", "finished", "exit", "log")
        }
        assert all(path.is_file() for path in paths.values()), name
        expected_exit = 1 if name == "parser-tests" else 0
        assert paths["exit"].read_text() == f"{expected_exit}\n", name
        assert shlex.split(paths["command"].read_text()) == command, name
        start, end = (paths[key].read_text().strip() for key in ("started", "finished"))
        assert datetime.fromisoformat(start) <= datetime.fromisoformat(end)
        receipts[name] = {"exit": expected_exit, "started": start, "finished": end}
    ordered = (
        "create",
        "stage-build",
        "prepare",
        "shell-lint",
        "inspect-before",
        "occupancy-before",
        "native",
        "native-report",
        "inspect-after",
        "collect-results",
        "cleanup",
        "python-lint",
        "shell-lint-final",
    )
    for before, after in zip(ordered, ordered[1:]):
        assert receipts[before]["finished"] <= receipts[after]["started"], (
            before,
            after,
        )
    for before, after in (
        ("stage-build", "stage-run"),
        ("stage-run", "inspect-before"),
        ("parser-tests", "parser-tests-fixed"),
    ):
        assert receipts[before]["finished"] <= receipts[after]["started"]
    assert (ARCHIVE / "raw/create.log").read_text() == report.OWNED + "\n"
    s.base.validate_guards((ARCHIVE / "raw/occupancy-before.log").read_text(), 1)
    harness = {}
    for name in ("prepare.sh", "guard.sh", "run.sh", "cleanup.sh", "inspect.sh"):
        path = (
            report.PRIOR / name
            if name in ("prepare.sh", "guard.sh")
            else ARCHIVE / name
        )
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        for phase in ("before", "after"):
            inspected = (ARCHIVE / f"raw/inspect-{phase}.log").read_text().splitlines()
            assert inspected.count(f"{digest}  {name}") == 1
        harness[name] = digest
    assert (
        harness["prepare.sh"]
        == "229d79785fabcec6178d871863c154caa47534a083d7eb85e19cf0e10ebc0ba1"
    )
    assert (
        harness["guard.sh"]
        == "3c9c1408c8afcd6bdb1a85f208ed9af6a96ba363f9a2f069529fa047d1ebfa89"
    )
    owner_digest = hashlib.sha256(
        ("fe2o3-native-wait-" + s.COMMIT + "\n").encode()
    ).hexdigest()
    for phase in ("before", "after"):
        inspected = (ARCHIVE / f"raw/inspect-{phase}.log").read_text().splitlines()
        assert inspected.count(owner_digest + "  owner") == 1
        for variable in (
            "HSA_XNACK",
            "HSA_ENABLE_SDMA",
            "HSA_ENABLE_PEER_SDMA",
            "HSA_OVERRIDE_GFX_VERSION",
            "HSA_TOOLS_LIB",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
        ):
            assert inspected.count(f"inherited_environment {variable}=<unset>") == 1

    sources = manifest(ARCHIVE / "results/source-files.sha256")
    checked = {}
    process = subprocess.Popen(
        [
            "git",
            "archive",
            s.COMMIT,
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates",
            "examples",
            "benchmarks/runtime_gfx942",
        ],
        cwd=ROOT,
        stdout=subprocess.PIPE,
    )
    with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
        for entry in archive:
            if entry.isfile():
                assert entry.name in sources
                digest = hashlib.sha256(archive.extractfile(entry).read()).hexdigest()
                assert digest == sources[entry.name], entry.name
                checked[entry.name] = digest
            else:
                assert entry.isdir(), entry.name
    assert process.wait() == 0 and checked == sources
    for phase in ("before", "after"):
        assert (ARCHIVE / f"results/source-{phase}.log").read_text().splitlines() == [
            name + ": OK" for name in sources
        ]
    binaries = manifest(ARCHIVE / "results/binaries.sha256")
    assert set(binaries) == set(report.BINARIES)
    native = (ARCHIVE / "raw/native.log").read_text()
    for binary in binaries:
        assert native.splitlines().count(binary + ": OK") == 2
    assert "post_run_porcelain=\n" in native
    observations = report.report(report.parse(native))
    assert json.loads((ARCHIVE / "raw/native-report.log").read_text()) == observations
    for cell, ceiling in (("B", 1000000), ("C", 25000)):
        result = observations["cells"][cell]
        assert result["cpu_status_counts"] == {"available": 52}
        assert result["maximum_requested_sleep_ns"] == ceiling
    failed_tests = (ARCHIVE / "raw/parser-tests.log").read_text()
    assert 'field=\'"GPU use (%)": "0"\'' in failed_tests
    assert "AssertionError: AssertionError not raised" in failed_tests
    assert re.search(
        r"Ran 4 tests in [0-9.]+s\n\nFAILED \(failures=1\)\n\Z", failed_tests
    )
    tests = (ARCHIVE / "raw/parser-tests-fixed.log").read_text()
    assert re.search(r"Ran 5 tests in [0-9.]+s\n\nOK\n\Z", tests)
    assert (ARCHIVE / "raw/python-lint.log").read_text() == "All checks passed!\n"
    assert (ARCHIVE / "raw/shell-lint-final.log").read_text().splitlines() == [
        "parsed=" + path.name for path in sorted(ARCHIVE.glob("*.sh"))
    ]
    cleanup = (ARCHIVE / "raw/cleanup.log").read_text().splitlines()
    assert len(cleanup) == 4 and cleanup[1] == "409M\t" + report.OWNED
    assert cleanup[2] == "removed_owned_directory=" + report.OWNED + " absent=yes"
    assert all(re.fullmatch(s.TIMESTAMP, cleanup[index]) for index in (0, 3))
    return {
        "source": s.COMMIT,
        "source_files": len(sources),
        "harness_sha256": harness,
        "binary_sha256": binaries,
        "receipts": receipts,
        "native_smoke": observations,
        "parser_tests_passed": 5,
        "preliminary_failed_fixture_retained": True,
        "remote_owned_directory_removed": True,
        "scope": "Two native diagnostic success paths, not fault coverage, a matched performance campaign, or formal refinement.",
    }


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
