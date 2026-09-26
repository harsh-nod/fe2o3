#!/usr/bin/env python3
"""Retain all leaf-development attempts, then remove this exact owned tree."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import types

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCRATCH = Path("/home/harsh/.codex-tmp/fe2o3-leaf-outcomes-20260925-4CVOfBNd")
SOURCE = "7185cc6fba1ac7b154469881b1e2df17aec25b6f"
PRIOR = ROOT / "docs/evidence/dev-completion-reconciliation-proof-2026-09-25/collect.py"
PRIOR_SHA = "dacc92d8efc3749e5d8630057f4db3382b8a7beeb335be584c306da32932de5f"
NEGATIVES = {"skip-success-settlement", "promote-unknown-input", "omit-failure-quarantine", "reject-valid-custody"}
PHASES = {"source-signature", "runner-tests", "source-tests", "source-body", "closure-before", "proof-before",
          "relocated-proof", "proof-after", "closure-after", "format"} | NEGATIVES
SCOPE = "Exact finite-projection leaf development; not production refinement, final admission or native/performance qualification"
DEVELOPMENT = ["01-exact-adapters", "02-exact-adapters", "03-custody-focused", "04-leaf-outcomes",
               "05-leaf-outcomes", "06-leaf-outcomes", "07-isolated-planner", "08-leaf-lemmas",
               "09-opaque-leaf", "10-scalar-leaf", "11-default-whole-root"]
PARTIAL_PHASES = {"source-signature", "runner-tests", "source-tests", "source-body", "closure-before",
                  "proof-before", "relocated-proof", "skip-success-settlement"}


def main():
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise ValueError("use python3 -I -B")
    raw = PRIOR.read_bytes()
    if hashlib.sha256(raw).hexdigest() != PRIOR_SHA:
        raise ValueError("retention helper identity")
    helper = types.ModuleType("leaf_retention_helpers")
    helper.__file__ = str(PRIOR)
    exec(compile(raw, str(PRIOR), "exec"), helper.__dict__)
    need = helper.need
    final = SCRATCH / "signed-campaign-v3"
    before = json.loads((final / "source-before.json").read_text())
    after = json.loads((final / "source-after.json").read_text())
    need(before == after and before["commit"] == SOURCE, "complete final signed source bracket")
    signed = json.loads((final / "signed-inputs.json").read_text())
    need({path: row["sha256"] for path, row in signed.items()} == before["inputs"], "signed blob binding")
    results = json.loads((final / "results.json").read_text())
    need(results.keys() == PHASES, "exact final phases")
    for phase, row in results.items():
        expected = 1 if phase in NEGATIVES else 0
        need(row == dict(passed=True, status=expected), "accepted terminal final phase")
        record = json.loads((final / phase / "record.json").read_text())
        need(record["status"] == expected and record["group_absent"] is True, "final owned phase terminal")
    records = {}
    groups = []
    expected_records = {name + "/proof/record.json" for name in DEVELOPMENT}
    expected_records |= {campaign + "/" + phase + "/record.json"
                         for campaign in ("signed-campaign", "signed-campaign-v2") for phase in PARTIAL_PHASES}
    expected_records |= {"signed-campaign-v3/" + phase + "/record.json" for phase in PHASES}
    need({str(path.relative_to(SCRATCH)) for path in SCRATCH.rglob("record.json")} == expected_records,
         "all 41 expected development/campaign records present")
    for name in DEVELOPMENT:
        directory = SCRATCH / name
        inputs = json.loads((directory / "inputs.json").read_text())
        need(helper.inventory(directory / "source") == inputs, "exact retained development source")
        json.loads((directory / "result.json").read_text())
    for path in sorted(SCRATCH.rglob("record.json")):
        need((path.parent / "stdout.log").is_file() and (path.parent / "stderr.log").is_file(), "raw phase logs present")
        row = json.loads(path.read_text())
        need(row["group_absent"] is True and type(row["status"]) is int
             and row["finished_ns"] >= row["started_ns"], "all development process groups terminal")
        records[str(path.relative_to(SCRATCH))] = row
        groups.append(row["process_group"])
    helper.groups_absent(groups)
    need(not (HERE / "retained").exists(), "new retained destination")
    owned = helper.snapshot(SCRATCH)
    original = helper.inventory(SCRATCH)
    shutil.copytree(SCRATCH, HERE / "retained")
    need(helper.inventory(HERE / "retained") == original and helper.inventory(SCRATCH) == original,
         "complete byte-exact retention")
    helper.write(HERE / "retention.json", dict(scope=SCOPE, source=str(SCRATCH), files=original))
    identities = {}
    for row in owned.values():
        identities[tuple(row[:2])] = row[-1]
    allocated = sum(identities.values())
    helper.write(HERE / "cleanup-before.json", dict(scope=SCOPE, source_commit=SOURCE,
        source_inputs=len(before["inputs"]), retained_files=len(original), owned_root=str(SCRATCH), inventory=owned,
        allocated_bytes=allocated, terminal_records=records, observed_at=datetime.now(timezone.utc).isoformat()))
    need(helper.snapshot(SCRATCH) == owned, "owned tree unchanged")
    helper.groups_absent(groups)
    need(helper.inventory(HERE / "retained") == original, "retained before deletion")
    shutil.rmtree(SCRATCH)
    need(not os.path.lexists(SCRATCH) and SCRATCH.name not in os.listdir(SCRATCH.parent), "independent owned absence")
    helper.groups_absent(groups)
    need(helper.inventory(HERE / "retained") == original, "retained after deletion")
    helper.write(HERE / "cleanup-after.json", dict(scope=SCOPE, removed_paths=[str(SCRATCH)],
        absent_paths=[str(SCRATCH)], terminal_groups_absent=groups, retained_files=len(original),
        retained_hashes_match=True, path_accounted_allocated_bytes_removed=allocated, remote_resources_created=False,
        observed_at=datetime.now(timezone.utc).isoformat()))
    print(json.dumps(dict(retained_files=len(original), source_inputs=len(before["inputs"]),
                         removed_allocated_bytes=allocated, absent=True), sort_keys=True))


if __name__ == "__main__":
    main()
