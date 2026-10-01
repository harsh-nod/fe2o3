#!/usr/bin/env python3
"""Read-only reconstruction of the rejected signed112 campaign's closed109 prefix."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import types

BASE = Path(__file__).resolve().parent
OWNER_SHA = "54c7951f454d3463c8eab232b602e9584b1928ea3a06f7a887a57b44903c6ba8"
CONTROLS_SHA = "f44df9b984a3b05817e54b4bf591e7cb2257f8e9b5773c5e5e795c7012cd3c48"
PREPARED_SHA = "3ce4ea80523dd394647b471289696444b4b2ae1d6537b8eccd4102696865c92e"
HELPER_SHA = "46177aa0e4be6f43c2d4b34504016daccf933bdd7390643f533e4f18cb51e4be"
PACKET = Path("/run/shm/fe2o3-a2-signed-concrete-qualification-20261001-attempt-1")
DURABLE = Path("/mnt/c") / PACKET.name
TERMINAL_PINS = {
    str(PACKET / "results.json"): "75cfdffbeec751b4c6a069858ae9457cbba3ef85f6b7d54295712c1552c83876",
    str(PACKET / "fresh-process-closure.json"): "eb37801a7106d4b56fadd004a1145a7d4b2cdc4d7a479248b59c9023d5f84b8c",
    str(DURABLE / "packet.tar"): "cc3135a54ac6fa7271641a8fb9d7052075eba107070c25cee4bd754b4bc43ff1",
    str(DURABLE / "members.json"): "87c0eab6bebfea56289cb338d4f0469d663709a8e42c68e43361411ae0e54a7e",
    str(DURABLE / "receipt.json"): "20c3b2e314ee4e566fe18f7dac53798143f448d9922fdf407685a2be8a8028c7",
}
ERRORS = [
    "ValueError: at least 16 GiB available local RAM",
    "111-tool-release-after: ValueError: at least 16 GiB available local RAM",
    "112-signature-after: ValueError: at least 16 GiB available local RAM",
    "closing source/archive: ValueError: at least 16 GiB available local RAM",
    "closing census: ValueError: at least 16 GiB available local RAM",
]


def need(value, message):
    if not value:
        raise ValueError(message)


def module(path, digest, name):
    need(path.resolve() == path and not path.is_symlink(), "ordinary canonical readback helper")
    raw = path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == digest, "exact reviewed helper bytes")
    result = types.ModuleType(name)
    result.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), result.__dict__)
    return result


def terminal(pins, packet, durable, sha):
    expected = {packet / name for name in ("results.json", "fresh-process-closure.json")}
    expected |= {durable / name for name in ("packet.tar", "members.json", "receipt.json")}
    need(set(pins) == {str(path) for path in expected} and len(pins) == 5, "five exact rejected terminal pins, never an active packet")
    need(all(sha(Path(path)) == digest for path, digest in pins.items()), "exact original terminal bytes")


def roster(commands, rows):
    need(len(commands) == 112 and len(rows) == 109
         and [row["name"] for row in rows] == [spec["name"] for spec in commands[:109]]
         and all(row["kind"] == spec["kind"] and row["accepted"] is True
                 and row["owned_launch_attempted"] is True and row["group_absent"] is True
                 for spec, row in zip(commands[:109], rows, strict=True)), "exact accepted109 prefix of rejected112")


def rejection(result):
    need(result["signed_campaign_checks_passed"] is False and result["errors"] == ERRORS,
         "original rejection remains RAM-only with zero whole-campaign qualification")


def boundary_counts(commands, details):
    counts = {family: 0 for family in ("leaf", "conditional", "concrete")}
    forwarding = []
    for spec, result in zip(commands, details, strict=True):
        if spec["kind"] != "negative":
            continue
        counts[spec["family"]] += 1
        classified = result["classification"]
        need(classified["logical_diagnostic_accepted"] is True and classified["qualified_kill"] is False
             and classified["historical_capture_is_qualified_kill"] is False
             and classified["signed_campaign_qualified"] is False, "predicate distinct from whole-campaign qualification")
        if "forwarding" in classified:
            forwarding.append(classified["forwarding"])
    need(counts == {"leaf": 38, "conditional": 21, "concrete": 30}, "all89 fresh actual-body negatives")
    semantic = [row for row in forwarding if row["boundary"] == "actual-journal-result-equality"]
    ghost = [row for row in forwarding if row["boundary"] == "wrapper-ghost-trace-only"]
    need(len(semantic) == 8 and len(ghost) == 1
         and all(row["actual_result_failure_observed"] is True for row in semantic)
         and ghost[0]["actual_result_failure_observed"] is False
         and all(row["inner_query_call_count_proved"] is False for row in forwarding),
         "eight actual result-equality and one ghost-only boundary; no inner call-count proof")
    return counts


def audit():
    p = module(BASE / "post_audit_v1.py", HELPER_SHA, "signed_concrete_readback_primitives")
    w = module(BASE / "signed_concrete_campaign_v1.py", OWNER_SHA, "signed_concrete_readback_owner")
    need(p.sha(BASE / "test_signed_concrete_campaign_v1.py") == CONTROLS_SHA
         and p.sha(w.PREPARED) == PREPARED_SHA, "exact reviewed owner controls and preparation")
    terminal(TERMINAL_PINS, w.OUTPUT, w.DURABLE, p.sha)
    x = w.components()
    frozen = w.prepare(x)
    need(p.load(w.PREPARED) == frozen, "independent complete signed source/blob/tool/history/old CPU reconstruction")
    packet, c, h = w.OUTPUT, x.c, x.h
    before = c.prior.tree(packet, h)
    opening = p.load(packet / "inputs.json")
    namespace, env = opening.pop("namespace"), opening.pop("environment")
    need(opening == frozen and env == c.d.environment(packet) and not (packet / "closing-inputs.json").exists(),
         "exact opening binding; original closing source reconstruction was never completed")
    result = p.load(packet / "results.json")
    rejection(result)
    need(result["durable_custody_required"] is True and result["scope"] == w.SCOPE
         and result["prior_captures_promoted"] is False and result["fresh_cpu_execution"] is False
         and result["native_live_credit_arc_or_hardware_qualified"] is False, "exact completed campaign scope")
    commands, rows = frozen["commands"], result["stages"]
    roster(commands, rows)
    need([spec["name"] for spec in commands[109:]] == ["110-concrete-after", "111-tool-release-after", "112-signature-after"]
         and all(not (packet / spec["name"]).exists() for spec in commands[109:]), "exact three wholly unlaunched closing stages")
    groups, details, stages, projections = [], [], [], {}
    positives = {family: [] for family in w.COUNTS}
    for spec, row in zip(commands[:109], rows, strict=True):
        folder = packet / spec["name"]
        need(p.load(folder / "command.json") == dict(spec, cwd=str(w.REPO), environment=env)
             and p.load(folder / "result.json") == row, "exact executed command/environment and result join")
        status = 1 if spec["kind"] == "negative" else 0
        need(type(row["returncode"]) is int and row["returncode"] == status, "exact original typed exit")
        record = p.load(folder / "owned/record.json")
        c.base.owned_record(record, status, spec["argv"])
        groups.append(record["process_group"])
        texts = None
        if "root" in spec:
            texts, pins = w.projection(x, spec)
            root = Path(spec["root"])
            if root != w.REPO:
                need(c.prior.tree(root, h) == pins == frozen["projections"][str(root)], "complete actual mutated/relocated source projection")
                projections[str(root)] = pins
        stdout, stderr = [(folder / "owned" / name).read_bytes().decode() for name in ("stdout.log", "stderr.log")]
        detail = w.classify(x, spec, texts, status, stdout, stderr)
        need(detail["accepted"] is True and all(row[key] == value for key, value in detail.items()),
             "independent strict raw classification of every original stage")
        details.append(detail)
        if spec["kind"] == "positive":
            positives[spec["family"]].append(detail["classification"]["candidate_verified"])
        stages.append({"name": spec["name"], "kind": spec["kind"], "returncode": status, "classification": detail,
            "stdout_sha256": p.sha(folder / "owned/stdout.log"), "stderr_sha256": p.sha(folder / "owned/stderr.log"),
            "record_sha256": p.sha(folder / "owned/record.json"), "elapsed_ns": record["finished_ns"] - record["started_ns"]})
    need(projections == frozen["projections"] == p.load(packet / "generated-inputs.json") and len(projections) == 92,
         "all89 actual-body and three distinct relocated positive projections")
    need(positives == {"leaf": [42] * 3, "conditional": [64] * 3, "concrete": [214] * 2},
         "eight actual positives; closing concrete positive is missing")
    counts = boundary_counts(commands[:109], details)
    p.saved_census(p.load(packet / "fresh-process-closure.json"), groups, namespace,
        p.load(packet / "fresh-census-inputs.json"), [row["name"] for row in rows])
    need(len(groups) == 109, "all109 original fresh process groups closed; original final RAM check still failed")
    pins = dict(frozen["raw_pins"], **{str(w.PREPARED): PREPARED_SHA})
    index = p.load(packet / "raw-archive-index.json")
    p.archive_join(index, pins)
    h.validate_raw_archive(packet, index)
    durable = p.durable_packet(x.resources, packet, w.DURABLE)
    need(c.prior.tree(packet, h) == before, "readback leaves all original evidence unchanged")
    terminal(TERMINAL_PINS, packet, w.DURABLE, p.sha)
    return {"rejected_signed_concrete_readback_passed": True, "signed_campaign_qualified": False,
        "qualified_fresh_negative_cases": 0, "original_errors": ERRORS,
        "unlaunched_stages": [spec["name"] for spec in commands[109:]],
        "original_closing_input_binding_missing": True, "current_readback_source_reconstruction_matches": True,
        "signed_commit": w.SIGNED, "pre_sign_sha256": w.REVIEW_SHA,
        "prepared_sha256": PREPARED_SHA, "packet_tree_sha256": p.tree_digest(before), "packet_files": len(before),
        "stages": stages, "source_files": len(x.source), "signed_blobs": len(x.blobs),
        "proof_closures": {family: len(paths) for family, paths in x.review["proof_closures"].items()},
        "observed_fresh_negative_cases": counts, "positive_counts": positives, "saved_original_groups": len(groups),
        "durable": durable, "old_cpu_custody_only": True, "historical_pid_probes": False,
        "solver_or_cpu_rerun": False, "original_packet_changed": False, "scope": w.SCOPE,
        "durable_custody_requires_original_terminal_directory_fsync_acknowledgment": True}


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
                      "packet_tree_sha256": result["packet_tree_sha256"], "packet_files": result["packet_files"]}))
