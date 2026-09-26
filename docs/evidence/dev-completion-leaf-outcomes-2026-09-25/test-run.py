#!/usr/bin/env python3
"""Classification calibration; synthetic inputs are never proof evidence."""
import copy
import json
from pathlib import Path
import runpy

check = runpy.run_path(str(Path(__file__).with_name("run.py")))["logical_negative"]
verifier = {"version": "calibration-only"}
source_paths = {"/snapshot/context_completion_reconciliation_leaf_v1.rs"}
base = {"verus": verifier, "verification-results": {
    "encountered-error": True, "encountered-vir-error": False,
    "is-verifying-entire-crate": False, "errors": 1, "verified": 1,
}}
diagnostic = {"level": "error", "message": "postcondition not satisfied", "spans": [
    {"is_primary": True, "file_name": "/snapshot/context_completion_reconciliation_leaf_v1.rs"},
]}


def require(value):
    if not value:
        raise ValueError("classifier calibration failed")


def accepted(status=1, result=base, messages=None):
    return check(status, json.dumps(result), "\n".join(json.dumps(m) for m in (
        [diagnostic] if messages is None else messages)), verifier, source_paths)


require(accepted())
for status in (0, 2, 124, 137, -9):
    require(not accepted(status=status))
require(not check(1, "not JSON", json.dumps(diagnostic), verifier, source_paths))
for message in ("mismatched types", "unexpected token", "Resource limit (rlimit) exceeded"):
    require(not accepted(messages=[dict(diagnostic, message=message)]))
require(not accepted(messages=[diagnostic, {"level": "warning", "message": "unused variable"}]))
require(not accepted(messages=[]))
require(not accepted(messages=[dict(diagnostic, spans=[])]))
for key, value in (("encountered-error", False), ("encountered-vir-error", True),
                   ("is-verifying-entire-crate", True), ("errors", 0), ("errors", True), ("success", True)):
    changed = copy.deepcopy(base)
    changed["verification-results"][key] = value
    require(not accepted(result=changed))
changed = copy.deepcopy(base)
changed["verus"] = {"version": "different"}
require(not accepted(result=changed))
require(not accepted(messages=[dict(diagnostic, spans=[{"is_primary": True, "file_name": "/other.rs"}])]))
print("PASS: leaf outcome negative classifier (10 groups)")
