#!/usr/bin/env python3
"""Check this interrupted campaign's provenance, custody and refusal to compare."""

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
RELATIVE = str(ARCHIVE.relative_to(ROOT))
spec = importlib.util.spec_from_file_location(
    "interruption", ARCHIVE / "interrupted.py"
)
interruption = importlib.util.module_from_spec(spec)
spec.loader.exec_module(interruption)
s = interruption.summary
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
    "parser-tests": python("test_reports.py") + ["-v"],
    "stage-run": [
        "scp",
        "-q",
        RELATIVE + "/guard.sh",
        RELATIVE + "/run.sh",
        RELATIVE + "/inspect.sh",
        "mi300x:" + s.OWNED + "/",
    ],
    "occupancy-before": SSH + ["bash", s.OWNED + "/guard.sh"],
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
    "postfailure-processes": SSH
    + [
        "/opt/rocm/bin/rocm-smi",
        "--showuse",
        "--showmeminfo",
        "vram",
        "--showpidgpus",
        "--showpids",
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
    "interruption-report": python("interrupted.py"),
    "summary-refusal": python("summarize.py"),
    "parser-tests-final": python("test_reports.py") + ["-v"],
    "python-lint": ["ruff", "check", RELATIVE],
    "python-format": ["ruff", "format", "--check", RELATIVE],
    "shellcheck": ["shellcheck", "-e", "SC1091"]
    + [RELATIVE + "/" + path.name for path in sorted(ARCHIVE.glob("*.sh"))],
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
            suffix: ARCHIVE / f"raw/{name}.{suffix}"
            for suffix in ("command", "started", "finished", "exit", "log")
        }
        assert all(p.is_file() and not p.is_symlink() for p in paths.values()), name
        code = 1 if name in ("benchmark", "summary-refusal") else 0
        assert paths["exit"].read_text() == f"{code}\n", name
        assert shlex.split(paths["command"].read_text()) == command, name
        start, end = [paths[k].read_text().strip() for k in ("started", "finished")]
        assert datetime.fromisoformat(start) <= datetime.fromisoformat(end)
        receipts[name] = {"exit": code, "started": start, "finished": end}
    # Parser work overlapped the remote build; only the native custody chain is ordered.
    chain = [
        "prepare",
        "stage-run",
        "occupancy-before",
        "inspect-before",
        "benchmark",
        "postfailure-processes",
        "inspect-after",
        "collect-results",
        "cleanup",
    ]
    for left, right in zip(chain, chain[1:]):
        assert receipts[left]["finished"] <= receipts[right]["started"], (left, right)
    assert (ARCHIVE / "raw/create.log").read_text() == s.OWNED + "\n"
    s.base.validate_guards((ARCHIVE / "raw/occupancy-before.log").read_text(), 1)
    harness = {}
    for name in ("prepare.sh", "guard.sh", "run.sh", "cleanup.sh"):
        digest = hashlib.sha256((ARCHIVE / name).read_bytes()).hexdigest()
        for phase in ("before", "after"):
            lines = (ARCHIVE / f"raw/inspect-{phase}.log").read_text().splitlines()
            assert lines.count(f"{digest}  {name}") == 1
        harness[name] = digest
    owner_digest = hashlib.sha256(
        ("fe2o3-native-wait-" + s.COMMIT + "\n").encode()
    ).hexdigest()
    for phase in ("before", "after"):
        lines = (ARCHIVE / f"raw/inspect-{phase}.log").read_text().splitlines()
        assert lines.count(owner_digest + "  owner") == 1
        for variable in (
            "HSA_XNACK",
            "HSA_ENABLE_SDMA",
            "HSA_ENABLE_PEER_SDMA",
            "HSA_OVERRIDE_GFX_VERSION",
            "HSA_TOOLS_LIB",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
        ):
            assert lines.count(f"inherited_environment {variable}=<unset>") == 1

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
                digest = hashlib.sha256(archive.extractfile(entry).read()).hexdigest()
                assert digest == sources[entry.name], entry.name
                checked[entry.name] = digest
            else:
                assert entry.isdir(), entry.name
    assert process.wait() == 0 and checked == sources and len(sources) == 5534
    for phase in ("before", "after"):
        assert (ARCHIVE / f"results/source-{phase}.log").read_text().splitlines() == [
            name + ": OK" for name in sources
        ]
    binaries = manifest(ARCHIVE / "results/binaries.sha256")
    assert set(binaries) == set(s.BINARIES)
    text = (ARCHIVE / "raw/benchmark.log").read_text()
    for binary in binaries:
        assert text.splitlines().count(binary + ": OK") == 2
    report = interruption.parse(text)
    assert json.loads((ARCHIVE / "raw/interruption-report.log").read_text()) == report
    refused = (ARCHIVE / "raw/summary-refusal.log").read_text()
    assert "AssertionError" in refused and "raw/benchmark.exit" in refused
    for name, count in (("parser-tests", 9), ("parser-tests-final", 12)):
        log = (ARCHIVE / f"raw/{name}.log").read_text()
        assert re.search(rf"Ran {count} tests in [0-9.]+s\n\nOK\n\Z", log)
    processes = (ARCHIVE / "raw/postfailure-processes.log").read_text()
    assert re.search(
        r"^536718\s+host_queue_publ\s+8\s+5161283584\s+0\s+0\s*$", processes, re.M
    )
    assert "PID 536718 is using 8 DRM device(s):\n2 4 6 0 1 3 5 7 " in processes
    cleanup = (ARCHIVE / "raw/cleanup.log").read_text().splitlines()
    assert len(cleanup) == 4 and cleanup[1] == "410M\t" + s.OWNED
    assert cleanup[2] == "removed_owned_directory=" + s.OWNED + " absent=yes"
    assert all(re.fullmatch(s.TIMESTAMP, cleanup[i]) for i in (0, 3))
    expected_raw = {
        f"{name}.{suffix}"
        for name in COMMANDS
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    actual_raw = {p.name for p in (ARCHIVE / "raw").iterdir()}
    assert (
        actual_raw
        - {f"audit.{s}" for s in ("command", "started", "finished", "exit", "log")}
        == expected_raw
    )
    return {
        "source": s.COMMIT,
        "source_files": len(sources),
        "harness_sha256": harness,
        "binary_sha256": binaries,
        "receipts": receipts,
        "interruption": report,
        "parser_tests": 12,
        "remote_owned_directory_removed": True,
        "scope": "Interrupted-run integrity and cleanup only; no accepted performance comparison.",
    }


if __name__ == "__main__":
    print(json.dumps(audit(), indent=2))
