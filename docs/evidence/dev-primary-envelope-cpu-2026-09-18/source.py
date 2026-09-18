#!/usr/bin/env python3
"""Snapshot the qualified Cargo/source inputs, excluding evidence and target."""

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
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo",
            "crates",
            "examples",
            "benchmarks/runtime_gfx942",
            "scripts/unsafe-source-baseline.json",
            "docs/runtime-primary-queue-release-v1.md",
        ],
        cwd=ROOT,
    )
    .decode()
    .split("\0")
)
print(
    json.dumps(
        {
            "base": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
            ).strip(),
            "files": {
                name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                for name in sorted(set(paths) - {""})
            },
        },
        indent=2,
    )
)
