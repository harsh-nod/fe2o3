#!/usr/bin/env python3
"""In-memory adverse-record checks for the one-off diagnostic summarizer."""

from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import runpy
import sys
from unittest.mock import patch

archive = Path(__file__).resolve().parent
script = archive / "summarize.py"
log_path = archive / "raw/benchmark-settled.log"
exit_path = archive / "raw/benchmark-settled.exit"
original = log_path.read_text()
read_text, read_bytes = Path.read_text, Path.read_bytes


def run(log=original, status="0", changed_helper=False):
    def text(path, *args, **kwargs):
        if path == log_path:
            return log
        if path == exit_path:
            return status
        return read_text(path, *args, **kwargs)

    def data(path, *args, **kwargs):
        value = read_bytes(path, *args, **kwargs)
        return value + b"\n" if changed_helper and path.name == "check-parity.py" else value

    output = io.StringIO()
    with patch.object(Path, "read_text", text), patch.object(Path, "read_bytes", data), \
            patch.object(sys, "argv", [str(script), "benchmark-settled"]), redirect_stdout(output):
        runpy.run_path(str(script), run_name="__main__")
    return json.loads(output.getvalue())


assert len(run()["rows"]) == 9
context = next(line for line in original.splitlines() if line.startswith("context "))
row = next(line for line in original.splitlines() if line.startswith("backend="))
postflight = next(line for line in original.splitlines() if line.startswith("postflight "))
finished = next(line for line in original.splitlines() if line.startswith("finished "))
mutations = [
    original.replace("gpu=1 ", "gpu=2 ", 1),
    original.replace("git_commit=ed5b5d64", "git_commit=00000000", 1),
    original.replace(context, context + "\n" + context, 1),
    original.replace("profile=directional", "profile=generic", 1),
    original.replace("packets_per_transfer=65", "packets_per_transfer=64", 1),
    original.replace("completion=facade-wait-and-explicit-continuation-flush", "completion=other", 1),
    original.replace("target=gfx942:sramecc+:xnack-", "target=gfx950", 1),
    original.replace("gpu_index=0", "gpu_index=1", 1),
    original.replace(postflight + "\n", "", 1),
    original.replace(row, row + "\n" + row, 1),
    original.replace("samples=10", "samples=11", 1),
    original.replace(row, row.replace("h2d_p50_ns=6061707", "h2d_p50_ns=0"), 1),
    original.replace(row, row.replace("h2d_p95_ns=6163008", "h2d_p95_ns=1"), 1),
    original.replace(finished + "\n", "", 1),
]
for candidate in mutations:
    assert candidate != original
cases = [(candidate, "0", False) for candidate in mutations] + [(original, "1", False), (original, "0", True)]
for case in cases:
    try:
        run(*case)
    except Exception as error:
        assert type(error).__name__ in {"AssertionError", "KeyError", "CheckError"}, error
    else:
        raise AssertionError("adverse record accepted")
print(f"PASS: diagnostic summary positive and {len(cases)} adverse records")
