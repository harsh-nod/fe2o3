#!/usr/bin/env python3
"""Snapshot the benchmark source scope and selected local compiler/HIP headers."""

import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
paths = (
    subprocess.check_output(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "benchmarks/runtime_gfx942",
        ],
        cwd=ROOT,
    )
    .decode()
    .split("\0")
)
external = [
    "/usr/bin/g++",
    "/opt/rocm/include/hip/hip_runtime_api.h",
    "/opt/rocm/include/hip/hip_runtime.h",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


print(
    json.dumps(
        {
            "base": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
            ).strip(),
            "files": {name: digest(ROOT / name) for name in sorted(set(paths) - {""})},
            "selected_external_inputs": {name: digest(Path(name)) for name in external},
        },
        indent=2,
    )
)
