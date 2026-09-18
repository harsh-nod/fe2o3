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
SOURCE = "3da2d25ac965afa9845b8ab1246e5fdf13c5d821"
OWNED = "/tmp/fe2o3-hsa-pool-engine-20260918.wBkvxkrs"
RECORDS = (
    "create",
    "stage",
    "prepare",
    "preflight",
    "stage-cleanup",
    "inspect-before",
    "benchmark",
    "inspect-after",
    "hash-binaries",
    "collect",
    "summary-test",
    "summary",
    "cleanup",
    "occupancy-after-cleanup",
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
        assert (raw / f"{name}.exit").read_text() == "0\n", name
        start = (raw / f"{name}.started").read_text().strip()
        end = (raw / f"{name}.finished").read_text().strip()
        assert datetime.fromisoformat(start) <= datetime.fromisoformat(end)
        receipts[name] = {
            "started": start,
            "finished": end,
            "command": (raw / f"{name}.command").read_text().strip(),
        }
    ordered = (
        "create",
        "stage",
        "prepare",
        "preflight",
        "stage-cleanup",
        "inspect-before",
        "benchmark",
        "inspect-after",
        "hash-binaries",
        "collect",
        "summary-test",
        "summary",
        "cleanup",
        "occupancy-after-cleanup",
    )
    for before, after in zip(ordered, ordered[1:]):
        # Staging and the build overlapped briefly; completion ordering is still required.
        assert receipts[before]["finished"] <= receipts[after]["finished"], (
            before,
            after,
        )
        if (before, after) != ("stage", "prepare"):
            assert receipts[before]["finished"] <= receipts[after]["started"], (
                before,
                after,
            )
    assert (ARCHIVE / "raw/create.log").read_text() == OWNED + "\n"
    relative = "docs/evidence/" + ARCHIVE.name
    ssh = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x"]
    scp = ["scp", "-q", "-o", "BatchMode=yes"]
    expected_commands = {
        "create": ssh + ["bash", "-s"],
        "stage": scp
        + [
            relative + "/" + name
            for name in ("prepare.sh", "guard.sh", "run.sh", "cleanup.sh")
        ]
        + ["mi300x:" + OWNED + "/"],
        "prepare": ssh
        + [
            "/usr/bin/timeout",
            "--signal=TERM",
            "--kill-after=5s",
            "900s",
            "bash",
            OWNED + "/prepare.sh",
            OWNED,
        ],
        "preflight": ssh + ["bash", "-c", '"source ' + OWNED + '/guard.sh; admit"'],
        "stage-cleanup": scp
        + [relative + "/cleanup.sh", relative + "/inspect.sh", "mi300x:" + OWNED + "/"],
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
        "summary-test": ["python3", "-I", relative + "/test-summary.py", "-v"],
        "summary": ["python3", "-I", relative + "/summarize.py"],
        "cleanup": ssh + ["bash", OWNED + "/cleanup.sh", OWNED],
        "occupancy-after-cleanup": ssh + ["bash", "-c", '"source /dev/stdin; admit"'],
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
    rows = summary.parse(benchmark)
    expected_summary = json.dumps(summary.summarize(rows), indent=2) + "\n"
    assert (ARCHIVE / "raw/summary.log").read_text() == expected_summary
    summary.validate_guards((ARCHIVE / "raw/preflight.log").read_text(), 1)
    summary.validate_guards(
        (ARCHIVE / "raw/occupancy-after-cleanup.log").read_text(), 1
    )
    assert (
        f"removed_owned_directory={OWNED} absent=yes"
        in (ARCHIVE / "raw/cleanup.log").read_text().splitlines()
    )
    return {
        "scope": "closed development diagnostic, not parity acceptance or full toolchain closure",
        "source_commit": SOURCE,
        "source_files": len(sources),
        "binary_hashes": binaries,
        "harness_hashes": harness,
        "records": receipts,
        "processes": len(rows),
        "benchmark_guard_observations": 49,
        "owned_remote_directory_removed": OWNED,
    }


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
