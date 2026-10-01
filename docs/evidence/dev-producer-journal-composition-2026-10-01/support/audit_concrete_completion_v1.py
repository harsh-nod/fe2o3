#!/usr/bin/env python3
"""Read-only109+3 composition using the exact independently audited prefix."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import types

BASE = Path(__file__).resolve().parent
OWNER_SHA = "606c17a081fb9b6ae0449aa66c07f72b9c7b900e45f006c495e0cbd33d826716"
CONTROLS_SHA = "00fa4dffabcdfe733c08b51957badfa884897e729d2e9c85825485fe828f8fe4"
PREPARED_SHA = "7246c4c282c2f6502e23f35122b56b77ffd8f05a0bb35e0e323b0e70a7a16519"
HELPER_SHA = "46177aa0e4be6f43c2d4b34504016daccf933bdd7390643f533e4f18cb51e4be"
TERMINAL_PINS = {
    "/run/shm/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/results.json": "51361891692a19dbbc9fe43ce3c4f58e28149931687fc9f3c91f34f88512ca91",
    "/run/shm/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/fresh-process-closure.json": "bffb1751b50b71538e82659240ff382997dd3ca122f4674c9baf5f72bd287271",
    "/run/shm/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/closing-inputs.json": "7246c4c282c2f6502e23f35122b56b77ffd8f05a0bb35e0e323b0e70a7a16519",
    "/mnt/c/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/packet.tar": "9694c4cefdc4dd4c6267379bfa98ca587f3598e2f9e166156894995c4ea9227b",
    "/mnt/c/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/members.json": "dc44f452fc0a942c33856182d08cbb4b7093fa6ddf3607eb1b62cbd621989af9",
    "/mnt/c/fe2o3-a2-signed-concrete-completion-20261001-attempt-1/receipt.json": "e29bab9010f13d64e75381384cfc65f031bd1adaa28e408f32e6ce0ae76f4dc9",
}


def need(value, message):
    if not value:
        raise ValueError(message)


def module(path, digest, name):
    need(path.resolve() == path and not path.is_symlink(), "ordinary canonical readback helper")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == digest, "exact reviewed helper")
    result = types.ModuleType(name)
    result.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), result.__dict__)
    return result


def terminal(pins, packet, durable, sha):
    expected = {packet / name for name in ("results.json", "fresh-process-closure.json", "closing-inputs.json")}
    expected |= {durable / name for name in ("packet.tar", "members.json", "receipt.json")}
    need(set(pins) == {str(path) for path in expected} and len(pins) == 6, "six terminal pins; never an unfinished packet")
    need(all(sha(Path(path)) == digest for path, digest in pins.items()), "exact completed bytes")


def admission(row, before, previous_end):
    budget, floor = 300 * 10**9, 16 * 1024**3
    need(row["accepted"] is True and "error" not in row and row["budget_ns"] == budget and row["floor_bytes"] == floor,
         "accepted exact-budget/floor admission")
    need(all(type(row[key]) is int for key in ("spent_ns_before", "spent_ns_after", "started_monotonic_ns"))
         and row["spent_ns_before"] == before and 0 <= before <= row["spent_ns_after"] < budget
         and row["started_monotonic_ns"] >= previous_end, "one cumulative monotonic budget")
    samples = row["samples"]
    need(type(samples) is list and samples, "nonempty real admission observations")
    elapsed = -1
    for index, sample in enumerate(samples):
        need(set(sample) == {"elapsed_ns", "available_bytes"}
             and all(type(value) is int and value >= 0 for value in sample.values())
             and elapsed <= sample["elapsed_ns"] <= row["spent_ns_after"] - before,
             "typed ordered bounded admission observations")
        elapsed = sample["elapsed_ns"]
        need((sample["available_bytes"] >= floor) == (index == len(samples) - 1),
             "first recovered sample is the admitting one")
    return row["spent_ns_after"], row["started_monotonic_ns"] + row["spent_ns_after"] - before


def audit():
    p = module(BASE / "post_audit_v1.py", HELPER_SHA, "completion_readback_helpers")
    adapter = module(BASE / "complete_signed_concrete_v1.py", OWNER_SHA, "completion_readback_adapter")
    need(p.sha(BASE / "test_complete_signed_concrete_v1.py") == CONTROLS_SHA
         and p.sha(adapter.PREPARED) == PREPARED_SHA, "exact controls and preparation")
    terminal(TERMINAL_PINS, adapter.OUTPUT, adapter.DURABLE, p.sha)
    w = adapter.build()
    x = w.components()
    frozen = w.prepare(x)
    need(p.load(w.PREPARED) == frozen, "complete current source/tools/signed blobs/prefix reconstruction")
    packet, c, h = w.OUTPUT, x.c, x.h
    tree = c.prior.tree(packet, h)
    prefix_tree = c.prior.tree(x.audit.PACKET, h)
    opening = p.load(packet / "inputs.json")
    namespace, env = opening.pop("namespace"), opening.pop("environment")
    need(opening == frozen and p.load(packet / "closing-inputs.json") == frozen and env == c.d.environment(packet),
         "exact complete opening/closing bindings and explicit environment")
    report = p.load(adapter.REPORT)
    need(p.tree_digest(prefix_tree) == report["packet_tree_sha256"] and report["qualified_fresh_negative_cases"] == 0,
         "unchanged root-matched rejected-prefix readback, not a new prefix classification")
    result = p.load(packet / "results.json")
    need(result["composed_completion_checks_passed"] is True and result["original_campaign_qualified"] is False
         and result["durable_custody_required"] is True and result["scope"] == w.SCOPE and result["errors"] == []
         and result["prior_captures_promoted"] is False and result["fresh_cpu_execution"] is False
         and result["native_live_credit_arc_or_hardware_qualified"] is False, "exact composed-only scope")
    commands, rows = frozen["commands"], result["stages"]
    need(len(commands) == len(rows) == 3 and [row["name"] for row in rows] == [spec["name"] for spec in commands],
         "exact three fresh closing stages")
    groups, details, spent, end = [], [], 0, 0
    for spec, row in zip(commands, rows, strict=True):
        folder = packet / spec["name"]
        need(p.load(folder / "command.json") == dict(spec, cwd=str(w.REPO), environment=env)
             and p.load(folder / "result.json") == row and row["kind"] == spec["kind"]
             and row["accepted"] is True and row["owned_launch_attempted"] is True and row["group_absent"] is True
             and type(row["returncode"]) is int and row["returncode"] == 0, "exact executed fresh stage and result")
        admitted = p.load(folder / "admission.json")
        spent, end = admission(admitted, spent, end)
        record = p.load(folder / "owned/record.json")
        c.base.owned_record(record, 0, spec["argv"])
        groups.append(record["process_group"])
        stdout, stderr = [(folder / "owned" / name).read_bytes().decode() for name in ("stdout.log", "stderr.log")]
        texts = w.projection(x, spec)[0] if "root" in spec else None
        classified = w.classify(x, spec, texts, 0, stdout, stderr)
        need(classified["accepted"] is True and all(row[key] == value for key, value in classified.items()),
             "independent fresh214 positive/release/signature raw classification")
        details.append({"name": spec["name"], "classification": classified, "admission": admitted,
            "stdout_sha256": p.sha(folder / "owned/stdout.log"), "stderr_sha256": p.sha(folder / "owned/stderr.log"),
            "record_sha256": p.sha(folder / "owned/record.json"), "elapsed_ns": record["finished_ns"] - record["started_ns"]})
    p.saved_census(p.load(packet / "fresh-process-closure.json"), groups, namespace,
        p.load(packet / "fresh-census-inputs.json"), [row["name"] for row in rows])
    need(p.load(packet / "generated-inputs.json") == frozen["projections"] == {}
         and w.qualified(commands, rows, [], True) is True, "exact109+3 qualification without new negative cases")
    pins = dict(frozen["raw_pins"], **{str(w.PREPARED): PREPARED_SHA})
    index = p.load(packet / "raw-archive-index.json")
    p.archive_join(index, pins)
    h.validate_raw_archive(packet, index)
    durable = p.durable_packet(x.resources, packet, w.DURABLE)
    adapter.prefix_unchanged(x)
    need(c.prior.tree(packet, h) == tree and c.prior.tree(x.audit.PACKET, h) == prefix_tree,
         "both original packets unchanged by readback")
    terminal(TERMINAL_PINS, packet, w.DURABLE, p.sha)
    return {"composed_readback_passed": True, "original_campaign_qualified": False,
        "prefix_report_sha256": adapter.REPORT_SHA, "prefix_stages": 109, "fresh_closing_stages": 3,
        "qualified_unique_prefix_negative_cases": 89, "new_negative_cases": 0,
        "positive_counts": {"leaf": [42] * 3, "conditional": [64] * 3, "concrete": [214] * 3},
        "prefix_packet_tree_sha256": p.tree_digest(prefix_tree), "completion_packet_tree_sha256": p.tree_digest(tree),
        "completion_packet_files": len(tree), "saved_original_prefix_groups": 109, "saved_new_groups": len(groups),
        "fresh_stages": details, "total_admission_ns": spent, "durable": durable,
        "admission_and_owned_receipts_use_different_clock_domains": True,
        "admission_before_launch_is_source_bound_not_cross_clock_arithmetic": True,
        "prefix_readback_reused_by_exact_immutable_binding": True, "historical_pid_probes": False,
        "solver_or_cpu_rerun": False, "original_packets_changed": False, "scope": w.SCOPE,
        "acceptance_requires_original_completion_terminal_directory_fsync_acknowledgment": True}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    need(sys.flags.isolated and sys.dont_write_bytecode and not sys.flags.optimize, "python3 -I -B")
    need(args.output.is_absolute() and args.output.parent == BASE and not args.output.exists(), "fresh external readback report")
    result = audit()
    raw = (json.dumps(result, sort_keys=True, indent=2) + "\n").encode()
    need(len(raw) <= 2 * 1024**2, "bounded readback report")
    with args.output.open("xb") as stream:
        stream.write(raw)
    print(json.dumps({"readback_passed": True, "sha256": hashlib.sha256(raw).hexdigest(),
                      "packet_tree_sha256": result["completion_packet_tree_sha256"]}))
