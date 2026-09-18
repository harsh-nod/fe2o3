#!/usr/bin/env python3
"""Check complete CPU payload/mock harnesses, source identity and closed receipts."""

import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess

assert __debug__, "verification requires Python assertions"
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
INPUTS = ARCHIVE.parent / "dev-hip-copy-only-cpu-2026-09-18/inputs.py"
assert (
    hashlib.sha256(INPUTS.read_bytes()).hexdigest()
    == "88ca79259a10223fe8240bef83c25536320c514ea069960538c23164e86517ee"
)


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def whole_tests(name, counts):
    lines = raw(name).splitlines()
    total = sum(counts.values())
    assert len(lines) == total + 5 and lines[total] == ""
    assert lines[total + 1] == "-" * 70
    assert re.fullmatch(rf"Ran {total} tests in [0-9.]+s", lines[total + 2])
    assert lines[total + 3 :] == ["", "OK"]
    rows = {}
    for line in lines[:total]:
        match = re.fullmatch(
            r"(test_[a-z0-9_]+) \(([a-zA-Z0-9_.]+)\.([a-zA-Z0-9_]+)\) \.\.\. ok", line
        )
        assert match and match[1] == match[3], line
        key = (match[2], match[1])
        assert key not in rows
        rows[key] = True
    assert {
        class_name: sum(k[0] == class_name for k in rows)
        for class_name in {k[0] for k in rows}
    } == counts


before = json.loads(raw("inputs-before"))
assert before == json.loads(raw("inputs-after"))
assert before["base"] == "1890a64e1911a2a346c5a9e70a8ff5ad45b19231"
current = json.loads(subprocess.check_output(["python3", "-I", str(INPUTS)], text=True))
assert current["files"] == before["files"]
assert current["selected_external_inputs"] == before["selected_external_inputs"]
assert (
    before["files"]["benchmarks/runtime_gfx942/hip_copy_diagnostic.py"]
    == "d5a2ad3dc7c00e58e7666973f2fd14d1343d7dc13edb96e738c6710eb690c561"
)
whole_tests(
    "tests",
    {
        "test_hip_copy_diagnostic.HipCopyDiagnosticTests": 10,
        "test_hip_copy_payload.HipCopyPayloadTests": 11,
    },
)
whole_tests("arguments", {"test_native_benchmark_args.NativeBenchmarkArgumentTests": 2})
python_files = [
    "benchmarks/runtime_gfx942/hip_copy_diagnostic.py",
    "benchmarks/runtime_gfx942/test_hip_copy_payload.py",
    "benchmarks/runtime_gfx942/test_hip_copy_diagnostic.py",
    str(ARCHIVE / "verify.py"),
]
commands = {
    "inputs-before": ["python3", "-I", str(INPUTS)],
    "tests": [
        "env",
        "ROCM_PATH=/opt/rocm",
        "python3",
        "-B",
        "-m",
        "unittest",
        "discover",
        "-s",
        "benchmarks/runtime_gfx942",
        "-p",
        "test_hip_copy*.py",
        "-v",
    ],
    "arguments": [
        "python3",
        "-B",
        "-m",
        "unittest",
        "discover",
        "-s",
        "benchmarks/runtime_gfx942",
        "-p",
        "test_native_benchmark_args.py",
        "-v",
    ],
    "lint": ["ruff", "check", "--no-cache"] + python_files,
    "format": ["ruff", "format", "--check", "--no-cache"] + python_files,
    "shellcheck": ["shellcheck", str(ARCHIVE / "qualify.sh"), str(ARCHIVE / "seal.sh")],
    "inputs-after": ["python3", "-I", str(INPUTS)],
    "verify-final": ["python3", "-I", str(ARCHIVE / "verify.py")],
}
expected = {
    f"{name}.{suffix}"
    for name in commands
    for suffix in ("command", "started", "finished", "exit", "log")
}
pending = not (ARCHIVE / "raw/verify-final.exit").exists()
if pending:
    expected -= {"verify-final.exit", "verify-final.finished"}
assert {p.name for p in (ARCHIVE / "raw").iterdir()} == expected
previous = None
for name, command in commands.items():
    assert shlex.split((ARCHIVE / f"raw/{name}.command").read_text()) == command
    started = (ARCHIVE / f"raw/{name}.started").read_text().strip()
    assert re.fullmatch(r"2026-09-18T\d\d:\d\d:\d\d\.\d{9}Z", started)
    if previous is not None:
        assert previous <= started
    if pending and name == "verify-final":
        continue
    assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n"
    finished = (ARCHIVE / f"raw/{name}.finished").read_text().strip()
    assert re.fullmatch(r"2026-09-18T\d\d:\d\d:\d\d\.\d{9}Z", finished)
    assert started <= finished
    previous = finished
print(
    json.dumps(
        {
            "audit": "PASS",
            "source_files": len(before["files"]),
            "payload_calibration_groups": 11,
            "hip_mock_groups": 10,
            "argument_tests": 2,
            "closed_receipts": len(commands),
            "gpu_used": False,
            "performance_accepted": False,
        },
        indent=2,
    )
)
