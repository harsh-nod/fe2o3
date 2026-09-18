#!/usr/bin/env python3
"""Bind this interrupted diagnostic to its build, receipts, and cleanup."""

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
spec = importlib.util.spec_from_file_location("interrupted", ARCHIVE / "interrupted.py")
interrupted = importlib.util.module_from_spec(spec)
spec.loader.exec_module(interrupted)
s = interrupted.summary
RELATIVE = "docs/evidence/" + ARCHIVE.name
SSH = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]


def remote(script):
    return SSH + ["bash", s.OWNED + "/" + script, s.OWNED]


def python(script):
    return ["python3", "-I", RELATIVE + "/" + script]


COMMANDS = {
    "create": SSH + ["bash", "-s"],
    "stage-build": [
        "scp",
        "-q",
        RELATIVE + "/prepare.sh",
        RELATIVE + "/cleanup.sh",
        "mi300x:" + s.OWNED + "/",
    ],
    "prepare": SSH
    + [
        "timeout",
        "--signal=TERM",
        "--kill-after=10s",
        "900s",
        "bash",
        s.OWNED + "/prepare.sh",
        s.OWNED,
    ],
    "stage-run": [
        "scp",
        "-q",
        RELATIVE + "/guard.sh",
        RELATIVE + "/run.sh",
        RELATIVE + "/inspect.sh",
        "mi300x:" + s.OWNED + "/",
    ],
    "stage-guard": ["scp", "-q", RELATIVE + "/guard.sh", "mi300x:" + s.OWNED + "/"],
    "occupancy-before": SSH + ["bash", s.OWNED + "/guard.sh"],
    "shell-lint": ["bash", RELATIVE + "/lint-shell.sh"],
    "inspect-before": remote("inspect.sh"),
    "benchmark": SSH
    + [
        "timeout",
        "--signal=TERM",
        "--kill-after=10s",
        "1500s",
        "bash",
        s.OWNED + "/run.sh",
        s.OWNED,
    ],
    "inspect-after": remote("inspect.sh"),
    "collect-results": [
        "scp",
        "-r",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "mi300x:" + s.OWNED + "/results",
        RELATIVE + "/",
    ],
    "cleanup": remote("cleanup.sh"),
    "summary-refusal": python("summarize.py"),
    "parser-tests": python("test_reports.py") + ["-v"],
    "interruption-report": python("interrupted.py"),
    "summary-refusal-final": python("summarize.py"),
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
    receipts, last_end = {}, ""
    for name, command in COMMANDS.items():
        paths = {
            suffix: ARCHIVE / "raw" / f"{name}.{suffix}"
            for suffix in ("command", "started", "finished", "exit", "log")
        }
        assert all(path.is_file() for path in paths.values()), name
        expected_exit = (
            1
            if name in ("benchmark", "summary-refusal", "summary-refusal-final")
            else 0
        )
        assert paths["exit"].read_text() == f"{expected_exit}\n", name
        assert shlex.split(paths["command"].read_text()) == command, name
        start, end = (paths[key].read_text().strip() for key in ("started", "finished"))
        assert datetime.fromisoformat(start) <= datetime.fromisoformat(end)
        assert last_end <= start, name
        last_end = end
        receipts[name] = {"exit": expected_exit, "started": start, "finished": end}
    assert (ARCHIVE / "raw/create.log").read_text() == s.OWNED + "\n"
    s.base.validate_guards((ARCHIVE / "raw/occupancy-before.log").read_text(), 1)
    harness = {}
    for name in ("prepare.sh", "guard.sh", "run.sh", "cleanup.sh"):
        digest = hashlib.sha256((ARCHIVE / name).read_bytes()).hexdigest()
        for phase in ("before", "after"):
            inspected = (ARCHIVE / f"raw/inspect-{phase}.log").read_text().splitlines()
            assert inspected.count(f"{digest}  {name}") == 1
        harness[name] = digest
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
    assert set(binaries) == set(s.BINARIES)
    benchmark = (ARCHIVE / "raw/benchmark.log").read_text()
    for binary in binaries:
        assert benchmark.splitlines().count(binary + ": OK") == 2
    report = interrupted.parse(benchmark)
    assert json.loads((ARCHIVE / "raw/interruption-report.log").read_text()) == report
    for name in ("summary-refusal", "summary-refusal-final"):
        refused = (ARCHIVE / f"raw/{name}.log").read_text()
        assert "AssertionError" in refused and "raw/benchmark.exit" in refused
    tests = (ARCHIVE / "raw/parser-tests.log").read_text()
    assert re.search(r"Ran 8 tests in [0-9.]+s\n\nOK\n\Z", tests)
    assert (ARCHIVE / "raw/python-lint.log").read_text() == "All checks passed!\n"
    assert (ARCHIVE / "raw/shell-lint-final.log").read_text().splitlines() == [
        "parsed=" + path.name for path in sorted(ARCHIVE.glob("*.sh"))
    ]
    cleanup = (ARCHIVE / "raw/cleanup.log").read_text().splitlines()
    assert len(cleanup) == 4 and cleanup[1] == "409M\t" + s.OWNED
    assert cleanup[2] == "removed_owned_directory=" + s.OWNED + " absent=yes"
    assert all(re.fullmatch(s.TIMESTAMP, cleanup[index]) for index in (0, 3))
    return {
        "source": s.COMMIT,
        "source_files": len(sources),
        "harness_sha256": harness,
        "binary_sha256": binaries,
        "receipts": receipts,
        "interruption": report,
        "parser_tests": 8,
        "remote_owned_directory_removed": True,
        "scope": "Integrity and interrupted-run audit, not successful hardware qualification or proof.",
    }


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
