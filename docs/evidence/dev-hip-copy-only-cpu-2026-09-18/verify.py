#!/usr/bin/env python3
"""Read-only audit of the scoped HIP comparator CPU campaign, not native timing."""

import json
from pathlib import Path
import re
import shlex
import subprocess

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def python_tests(name, count):
    text = raw(name)
    rows = re.findall(r"^test_[^\n]+ \.\.\. ok$", text, re.MULTILINE)
    assert len(rows) == len(set(rows)) == count
    assert re.search(rf"\nRan {count} tests in [0-9.]+s\n\nOK\n$", text)


before = json.loads(raw("inputs-before"))
assert before == json.loads(raw("inputs-after"))
assert before["base"] == "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f"
current = json.loads(
    subprocess.check_output(["python3", "-I", str(ARCHIVE / "inputs.py")], text=True)
)
assert current["files"] == before["files"]
assert current["selected_external_inputs"] == before["selected_external_inputs"]
python_tests("hip-tests", 10)
python_tests("argument-tests", 2)
assert shlex.split((ARCHIVE / "raw/hip-tests.command").read_text()) == [
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
    "test_hip_copy_diagnostic.py",
    "-v",
]
assert shlex.split((ARCHIVE / "raw/argument-tests.command").read_text()) == [
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
]
names = {
    "base",
    "compiler",
    "inputs-before",
    "hip-tests",
    "argument-tests",
    "lint",
    "format",
    "shellcheck",
    "inputs-after",
    "verify-final",
}
suffixes = {"command", "started", "finished", "exit", "log"}
expected = {f"{name}.{suffix}" for name in names for suffix in suffixes}
pending = not (ARCHIVE / "raw/verify-final.exit").exists()
if pending:
    expected -= {"verify-final.exit", "verify-final.finished"}
assert {p.name for p in (ARCHIVE / "raw").iterdir()} == expected
assert raw("base").strip() == before["base"]
for name in names:
    if pending and name == "verify-final":
        continue
    assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n", name
    assert shlex.split((ARCHIVE / f"raw/{name}.command").read_text())
    times = [
        (ARCHIVE / f"raw/{name}.{suffix}").read_text().strip()
        for suffix in ("started", "finished")
    ]
    assert all(
        re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", t) for t in times
    )
    assert times[0] <= times[1]
print(
    json.dumps(
        {
            "audit": "PASS",
            "benchmark_source_files": len(before["files"]),
            "hip_mock_tests": 10,
            "argument_tests": 2,
            "closed_receipts": len(names),
            "gpu_used": False,
            "native_hip_qualification": False,
            "performance_accepted": False,
        },
        indent=2,
    )
)
