#!/usr/bin/env python3
"""Verify fresh-path/build-identity-only changes against the sealed reviewed runner."""

import difflib
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
OLD = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917/docs/evidence/dev-hip-native-copy-smoke-1890a64e1-2026-09-18")
OLD_PATH = "/tmp/fe2o3-hip-smoke-1890a64e1-20260918.JA7uyMoS"
NEW_PATH = "/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.EvupxcTh"
NAMES = ["run.py", "check.py", "protocol.py", "cleanup.py", "topology.py", "prepare.sh",
         "dependency-snapshot.py", "create.sh", "record.sh", "freeze-source.py",
         "freeze-scripts.py", "test_check.py", "hip_copy_diagnostic.py"]
assert hashlib.sha256((OLD / "SHA256SUMS").read_bytes()).hexdigest() == "cbb59665c7f9e3199d67e82662d8f02fbff5caccce2bcf6a3e81b20317bb0adf"
old_manifest = {line[66:]: line[:64] for line in (OLD / "SHA256SUMS").read_text().splitlines()}
changed = []
for name in NAMES:
    original, current = (OLD / name).read_text(), (HERE / name).read_text()
    assert hashlib.sha256(original.encode()).hexdigest() == old_manifest[name]
    expected = original.replace(OLD_PATH, NEW_PATH)
    if name == "create.sh":
        expected = expected.replace("/tmp/fe2o3-hip-smoke-1890a64e1-20260918.XXXXXXXX", "/tmp/fe2o3-hip-smoke-1890a64e1-new-20260918.XXXXXXXX")
    if name == "protocol.py":
        expected = expected.replace("d2c012774e2321266d2708d7d7f40745d332ebbebf9d26c9c5f61c4bc57ad976", "207c65b8c540e28fc0c960bd962e23e6e30802132f4a68be058d22912af076e1")
    if name == "check.py":
        expected = expected.replace("ee22799c58bc45efddc94ed79de486bb35e396f5f014fe9fbba137297eb79d9b", "5911229d3fce10aceef828b65f4d5b21066d2fb173759bd2a178dc7acb024d2a")
    assert current == expected, name + ": unexpected semantic change"
    if original != current:
        changed.append(name)
        print("".join(difflib.unified_diff(original.splitlines(True), current.splitlines(True), fromfile="sealed/" + name, tofile="fresh/" + name)), end="")
for name in ("source-files.sha256", "source-export.json"):
    assert (OLD / name).read_bytes() == (HERE / name).read_bytes()
print(json.dumps({"reviewed_scripts": len(NAMES), "changed_only_path_or_build_identity": changed,
                  "committed_export_equal": True, "old_results_reused": False}, sort_keys=True))
