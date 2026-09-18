#!/usr/bin/env python3
"""Strict pinned matched-protocol parser; no incomplete-campaign ratios."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
STAGE = Path(__file__).resolve().parent
PRIOR = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917/docs/evidence/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py")
assert hashlib.sha256(PRIOR.read_bytes()).hexdigest() == "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9"
spec = importlib.util.spec_from_file_location("pinned_native_wait_protocol", PRIOR)
protocol = importlib.util.module_from_spec(spec)
spec.loader.exec_module(protocol)
owned = "/tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS"
commit = "5d70cb0a6e16fb265fe224690274fdb0be2b0055"
old_binaries = protocol.BINARIES
protocol.BINARIES = tuple(path.replace(protocol.OWNED, owned) for path in old_binaries)
protocol.RUNNER_METADATA -= {path + ": OK" for path in old_binaries}
protocol.RUNNER_METADATA |= {path + ": OK" for path in protocol.BINARIES}
protocol.OWNED = owned
protocol.COMMIT = commit
protocol.CONTEXT = dict(protocol.CONTEXT, git_commit=commit)
assert (STAGE / "raw/benchmark.exit").read_text() == "0\n"
print(json.dumps(protocol.summarize(protocol.parse((STAGE / "raw/benchmark.log").read_text())), indent=2))
