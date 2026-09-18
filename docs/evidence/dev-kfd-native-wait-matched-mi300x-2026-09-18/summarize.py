#!/usr/bin/env python3
"""Reuse the frozen four-cell protocol with this campaign's exact identities."""

import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
PRIOR = ARCHIVE.parent / "dev-kfd-native-wait-mi300x-2026-09-18"
COMMIT = "ecad7245fa9fa051daf3a9cb3cd82724da5446eb"
OWNED = "/tmp/fe2o3-kfd-native-wait-matched-20260918.tS8fnkzm"


def load(name, path, digest):
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


protocol = load(
    "native_wait_protocol",
    PRIOR / "summarize.py",
    "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9",
)
# Only campaign source/storage identities change; policies, workload, ordering,
# guard thresholds and whole-transcript acceptance stay pinned to the protocol.
old_binaries = protocol.BINARIES
protocol.BINARIES = tuple(path.replace(protocol.OWNED, OWNED) for path in old_binaries)
protocol.RUNNER_METADATA -= {path + ": OK" for path in old_binaries}
protocol.RUNNER_METADATA |= {path + ": OK" for path in protocol.BINARIES}
protocol.OWNED = OWNED
protocol.COMMIT = COMMIT
protocol.CONTEXT = dict(protocol.CONTEXT, git_commit=COMMIT)


def summarize():
    assert (ARCHIVE / "raw/benchmark.exit").read_text() == "0\n"
    return protocol.summarize(
        protocol.parse((ARCHIVE / "raw/benchmark.log").read_text())
    )


if __name__ == "__main__":
    print(json.dumps(summarize(), indent=2))
