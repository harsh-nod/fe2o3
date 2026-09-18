#!/usr/bin/env python3
"""Bind the raw observations, harness, source manifest and closed receipts."""

from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import tarfile
import types


ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
SOURCE = "3e12ef82bbb41fb116afbb7ddf7cffdad735ec7d"
OWNED = "/tmp/fe2o3-kfd-copy-progress-20260918.rrF6He9N"
RECORDS = (
    "create",
    "prepare",
    "stage-scripts",
    "stage-audit",
    "inspect-before",
    "benchmark",
    "cpu-audit",
    "inspect-after",
    "hash-binaries",
    "collect",
    "cleanup",
    "occupancy-after-cleanup",
    "partial-observations",
    "summary-refusal",
    "interrupted-tests",
    "python-lint",
    "shell-lint",
    "shell-lint-final",
)


def manifest(path):
    entries = {}
    for line in path.read_text().splitlines():
        digest, name = line.split("  ", 1)
        assert re.fullmatch(r"[0-9a-f]{64}", digest)
        assert name not in entries
        entries[name] = digest
    assert entries
    return entries


def audit():
    summary = types.ModuleType("pool_summary")
    summary.__file__ = str(ARCHIVE / "summarize.py")
    exec(
        compile(Path(summary.__file__).read_bytes(), summary.__file__, "exec"),
        summary.__dict__,
    )
    receipts = {}
    for name in RECORDS:
        raw = ARCHIVE / "raw"
        for suffix in ("command", "started", "finished", "exit", "log"):
            assert (raw / f"{name}.{suffix}").is_file()
        expected_exit = 1 if name in ("benchmark", "summary-refusal") else 0
        assert (raw / f"{name}.exit").read_text() == f"{expected_exit}\n", name
        start = (raw / f"{name}.started").read_text().strip()
        end = (raw / f"{name}.finished").read_text().strip()
        assert datetime.fromisoformat(start) <= datetime.fromisoformat(end)
        receipts[name] = {
            "exit": expected_exit,
            "started": start,
            "finished": end,
            "command": (raw / f"{name}.command").read_text().strip(),
        }
    ordered = (
        "inspect-before",
        "benchmark",
        "inspect-after",
        "hash-binaries",
        "collect",
        "cleanup",
        "occupancy-after-cleanup",
        "partial-observations",
        "summary-refusal",
        "interrupted-tests",
    )
    for before, after in zip(ordered, ordered[1:]):
        assert receipts[before]["finished"] <= receipts[after]["started"], (
            before,
            after,
        )
    for name in ("prepare", "stage-scripts", "stage-audit"):
        assert receipts["create"]["finished"] <= receipts[name]["started"]
        assert receipts[name]["finished"] <= receipts["inspect-before"]["finished"]
    assert (ARCHIVE / "raw/create.log").read_text() == OWNED + "\n"
    relative = "docs/evidence/" + ARCHIVE.name
    ssh = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
    scp = ["scp", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10"]
    expected_commands = {
        "create": ssh + ["bash", "-s"],
        "prepare": ssh
        + [
            "timeout",
            "--signal=TERM",
            "--kill-after=30s",
            "900s",
            "bash",
            "-s",
            "--",
            OWNED,
        ],
        "stage-scripts": scp
        + [relative + "/" + name for name in ("guard.sh", "run.sh")]
        + ["mi300x:" + OWNED + "/"],
        "stage-audit": scp
        + [relative + "/" + name for name in ("prepare.sh", "cleanup.sh", "inspect.sh")]
        + ["mi300x:" + OWNED + "/"],
        "inspect-before": ssh + ["bash", OWNED + "/inspect.sh", OWNED],
        "inspect-after": ssh + ["bash", OWNED + "/inspect.sh", OWNED],
        "benchmark": ssh
        + [
            "/usr/bin/timeout",
            "--signal=TERM",
            "--kill-after=10s",
            "1800s",
            "bash",
            OWNED + "/run.sh",
            OWNED,
        ],
        "hash-binaries": ssh
        + [
            "sha256sum",
            OWNED
            + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
            OWNED + "/hsa-copy-pool-engine",
        ],
        "collect": [
            "scp",
            "-q",
            "-r",
            "-o",
            "BatchMode=yes",
            "mi300x:" + OWNED + "/results",
            relative + "/",
        ],
        "cleanup": ssh + ["bash", OWNED + "/cleanup.sh", OWNED],
        "occupancy-after-cleanup": ssh + ["bash", "-c", '"source /dev/stdin; admit"'],
        "interrupted-tests": ["python3", "-I", relative + "/test-interrupted.py", "-v"],
        "summary-refusal": ["python3", "-I", relative + "/summarize.py"],
        "partial-observations": ["python3", "-I", relative + "/interrupted.py"],
        "cpu-audit": ["python3", "-I", relative + "/audit-cpu.py"],
        "python-lint": ["ruff", "check", relative],
        "shell-lint-final": ["bash", relative + "/lint-shell.sh"],
        "shell-lint": ["bash", "-n"]
        + [
            relative + "/" + name
            for name in (
                "create.sh",
                "prepare.sh",
                "guard.sh",
                "run.sh",
                "inspect.sh",
                "cleanup.sh",
                "record.sh",
                "seal.sh",
            )
        ],
    }
    assert set(expected_commands) == set(RECORDS)
    for name, expected in expected_commands.items():
        assert shlex.split(receipts[name]["command"]) == expected, ("command", name)
    harness = {}
    for name in ("prepare.sh", "guard.sh", "run.sh", "cleanup.sh"):
        digest = hashlib.sha256((ARCHIVE / name).read_bytes()).hexdigest()
        expected = f"{digest}  {name}"
        for phase in ("before", "after"):
            assert (
                ARCHIVE / f"raw/inspect-{phase}.log"
            ).read_text().splitlines().count(expected) == 1
        harness[name] = digest

    sources = manifest(ARCHIVE / "results/source-files.sha256")
    checked = {}
    process = subprocess.Popen(
        [
            "git",
            "archive",
            SOURCE,
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
                assert entry.name in sources, entry.name
                digest = hashlib.sha256(archive.extractfile(entry).read()).hexdigest()
                assert digest == sources[entry.name], entry.name
                checked[entry.name] = digest
            else:
                assert entry.isdir(), ("unexpected source member", entry.name)
    assert process.wait() == 0 and checked == sources
    expected_checks = [f"{name}: OK" for name in sources]
    for phase in ("before", "after"):
        assert (
            ARCHIVE / f"results/source-{phase}.log"
        ).read_text().splitlines() == expected_checks
    binaries = manifest(ARCHIVE / "results/binaries.sha256")
    assert manifest(ARCHIVE / "raw/hash-binaries.log") == binaries
    assert set(binaries) == {
        OWNED + "/hsa-copy-pool-engine",
        OWNED + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
    }
    benchmark = (ARCHIVE / "raw/benchmark.log").read_text()
    for binary in binaries:
        assert benchmark.splitlines().count(binary + ": OK") == 2
    partial = types.ModuleType("interrupted")
    partial.__file__ = str(ARCHIVE / "interrupted.py")
    exec(
        compile(Path(partial.__file__).read_bytes(), partial.__file__, "exec"),
        partial.__dict__,
    )
    observations = partial.parse(benchmark)
    assert not observations["accepted_campaign"]
    expected_summary = json.dumps(observations, indent=2) + "\n"
    assert (ARCHIVE / "raw/partial-observations.log").read_text() == expected_summary
    try:
        summary.parse(benchmark)
    except AssertionError:
        pass
    else:
        raise AssertionError("interrupted campaign accepted as complete")
    refusal = (ARCHIVE / "raw/summary-refusal.log").read_text()
    assert "raw/benchmark.exit" in refusal and refusal.endswith("AssertionError\n")
    summary.validate_guards(
        (ARCHIVE / "raw/occupancy-after-cleanup.log").read_text(), 1
    )
    assert (
        f"removed_owned_directory={OWNED} absent=yes"
        in (ARCHIVE / "raw/cleanup.log").read_text().splitlines()
    )
    cpu_audit = types.ModuleType("cpu_audit")
    cpu_audit.__file__ = str(ARCHIVE / "audit-cpu.py")
    exec(
        compile(Path(cpu_audit.__file__).read_bytes(), cpu_audit.__file__, "exec"),
        cpu_audit.__dict__,
    )
    import contextlib
    import io

    cpu_output = io.StringIO()
    with contextlib.redirect_stdout(cpu_output):
        cpu_audit.main()
    assert cpu_output.getvalue() == (ARCHIVE / "raw/cpu-audit.log").read_text()
    test_log = (ARCHIVE / "raw/interrupted-tests.log").read_text()
    assert len(re.findall(r"^test_.* \.\.\. ok$", test_log, re.M)) == 12
    assert re.search(r"\nRan 12 tests in [0-9.]+s\n\nOK\n$", test_log)
    assert (ARCHIVE / "raw/shell-lint-final.log").read_text().splitlines() == [
        f"syntax_ok={name}"
        for name in (
            "create.sh",
            "prepare.sh",
            "guard.sh",
            "run.sh",
            "inspect.sh",
            "cleanup.sh",
            "record.sh",
            "seal.sh",
            "lint-shell.sh",
        )
    ]
    return {
        "scope": "archived interrupted diagnostic, no accepted performance or parity result",
        "accepted_campaign": False,
        "source_commit": SOURCE,
        "source_files": len(sources),
        "binary_hashes": binaries,
        "harness_hashes": harness,
        "records": receipts,
        "completed_processes": len(observations["rows"]),
        "planned_processes": 16,
        "successful_prefix_guard_observations": 16,
        "failed_guard_observations": 2,
        "owned_remote_directory_removed": OWNED,
    }


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
