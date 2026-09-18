#!/usr/bin/env python3
"""Read-only LogicalMux historical audit; never invokes SSH, devices, Cargo or ELF."""

import argparse
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
PRIOR = ROOT.parent / "dev-combined-sdma-release-native-2026-09-18"
HELPER_SHA = "22c38fc74c8a2b6e772f4ee2aa5166fddf60d89040eab6b0b5a9322c55d09d53"
OWNED = "/home/harsh/fe2o3-logical-mux-sdma-20260918.c0d7c87a"
PAYLOAD = "3d19951d78837d794af0e5f4fb4bf64d31d70436816b66825e9ed9ceb3febcba"
CASES = [f"logical-mux-{lanes}" for lanes in (2, 4, 8, 14, 16)]
COLLECTED = Path("raw/native/collected")
LOCAL_BUNDLE = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-native-bundle-v2-20260918"
LOCAL_CONTROL = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-control-v2-20260918"
LOCAL_OUTPUT = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-native-v2-20260918"
LOCAL_PROTOCOL = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918"
LOCAL_NAMES = ["local-protocol-tests", "local-payload-verify", "local-controller-wiring-tests", "remote-empty", "upload", "remote-approve", "native-outer", "remote-inventory", "collect", "remote-cleanup", "remote-independent-absence"]


def sha(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def need(value, message):
    if not value:
        raise ValueError(message)


need(sha(PRIOR / "verify.py") == HELPER_SHA, "pinned shared historical audit helpers")
spec = importlib.util.spec_from_file_location("prior_combined_audit", PRIOR / "verify.py")
V = importlib.util.module_from_spec(spec)
spec.loader.exec_module(V)


def source_binding(payload):
    need(sha(payload / "payload.json") == PAYLOAD, "frozen LogicalMux payload")
    manifest = V.load(payload / "payload.json")
    need(type(manifest) is dict and len(manifest) == 26, "exact payload count")
    for name, digest in manifest.items():
        path = Path(name)
        need(not path.is_absolute() and ".." not in path.parts and path.as_posix() == name, "normalized payload path")
        need(re.fullmatch(r"[0-9a-f]{64}", digest), "payload digest")
        if name != "queue-example":
            need(sha(payload / name) == digest, "frozen payload bytes: " + name)
    P = V.module(payload / "protocol.py")
    pins = {
        "COMMIT": "9afafa4176e8b0c6ff53f6892841924f3c7c9c27",
        "BINARY_SHA": "bcb2a337dd679c31726a326ddfa99e2bb72975b6bff71a7270817148cd32e109",
        "CPU_SEAL": "40715bbee4e76d373862261b76cbb62f94d23808cd5ecf0c457162c881d153d9",
        "COHORT_SHA": "b61c8b42cda67dbdeba649907cf93ca23b862b055f77616fad98fcaab19c405f",
        "COHORT_FILE_COUNT": 5558,
        "SIGNERS_SHA": "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b",
        "OBSERVER_SHA": "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51",
        "TOPOLOGY_SHA": "669a25592e8ba72743bed9c0a3ced39ad392f104746c2ca04a7f2867ed8d7ca3",
    }
    need(all(getattr(P, key) == value for key, value in pins.items()), "exact reviewed source/executable pins")
    need(list(P.TESTS) == CASES and list(P.TESTS.values()) == [2, 4, 8, 14, 16], "ordered five-case plan")
    need((P.GPU, P.BDF, P.UID) == (1, "0000:26:00.0", "0xab83d2ffef0d3cdf"), "selected device identity")
    selected = V.literal_assignment(payload / "prepare.py", "SELECTED")
    need(len(selected) == 12 and selected["crates/fe2o3-kfd/src/sdma/multi_queue/logical_mux.rs"] == "source/logical_mux.rs" and selected["crates/fe2o3-kfd/src/queue_live/construction_primary/integration_release_logical_mux_sdma_tests.rs"] == "source/integration_release_logical_mux_sdma_tests.rs", "LogicalMux source snapshots")
    expected_files = set(selected.values()) | {
        "run.py", "protocol.py", "test_protocol.py", "PLAN.md", "prepare.py",
        "allowed-signers", "queue-example", "binding.json",
        "commit-signature.stdout", "commit-signature.stderr",
        "cpu/SHA256SUMS", "cpu/source-before.log", "cpu/source-after.log", "cpu/binary.log",
    }
    need(set(manifest) == expected_files, "exact payload membership")
    for name, digest in (("queue-example", P.BINARY_SHA), ("allowed-signers", P.SIGNERS_SHA), ("source/copy-host-observe.py", P.OBSERVER_SHA), ("source/r26-host-guard.py", P.TOPOLOGY_SHA), ("cpu/SHA256SUMS", P.CPU_SEAL), ("cpu/source-before.log", P.COHORT_SHA), ("cpu/source-after.log", P.COHORT_SHA)):
        need(manifest[name] == digest, "bound payload identity: " + name)
    cpu_manifest = {}
    for line in (payload / "cpu/SHA256SUMS").read_text().splitlines():
        digest, name = line.split("  ", 1)
        need(name not in cpu_manifest and re.fullmatch(r"[0-9a-f]{64}", digest), "unique typed CPU manifest entry")
        cpu_manifest[name] = digest
    for name in ("binary.log", "source-before.log", "source-after.log"):
        need(cpu_manifest["raw/" + name] == sha(payload / "cpu" / name), "CPU receipt sealed identity")
    V.binary_receipt((payload / "cpu/binary.log").read_text(), P.BINARY_SHA)
    inventory = V.load(payload / "cpu/source-before.log")
    need(inventory["base"] == "95ed0301cb532dd4bd762ec4410f0bd9a61814cc", "qualified source base")
    cohort = inventory["files"]
    need(len(cohort) == P.COHORT_FILE_COUNT, "source cohort count")
    need((payload / "cpu/source-before.log").read_bytes() == (payload / "cpu/source-after.log").read_bytes(), "unchanged source cohort")
    for source, snapshot in selected.items():
        need(sha(payload / snapshot) == cohort[source], "selected source/cohort identity")
    binding = V.load(payload / "binding.json")
    expected = {
        "commit": P.COMMIT, "source_files_matched": P.COHORT_FILE_COUNT,
        "cpu_manifest_sha256": P.CPU_SEAL, "binary_sha256": P.BINARY_SHA,
        "cohort_sha256": P.COHORT_SHA, "signature_exit": 0, "cohort_exit": 0,
        "source_base_field_ignored_only": True, "native_authorized": False,
        "cohort_command": ["git", "cat-file", "--batch"],
        "signature_command": ["git", "-c", "gpg.ssh.allowedSignersFile=" + LOCAL_PROTOCOL + "/allowed-signers", "verify-commit", "--raw", P.COMMIT],
    }
    need(set(binding) == set(expected) | {"started_ns", "finished_ns"} and all(binding.get(key) == value for key, value in expected.items()), "exact signed export binding")
    need(type(binding["started_ns"]) is int and type(binding["finished_ns"]) is int and 0 < binding["started_ns"] <= binding["finished_ns"], "ordered export receipt")
    need(not (payload / "commit-signature.stdout").read_bytes(), "clean signature stdout")
    need((payload / "commit-signature.stderr").read_text() == 'Good "git" signature for harmenon@amd.com with ED25519 key SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg\n', "historical signature receipt")
    V.COMMIT = P.COMMIT
    return P, manifest


def native_command(P, case, owned):
    return [
        "/usr/bin/timeout", "--signal=TERM", "--kill-after=5s", "180s",
        "/usr/bin/prlimit", "--core=0:0", "--fsize=16777216:16777216", "--",
        "/usr/bin/numactl", "--physcpubind=0-47", "--membind=0",
        owned + "/queue-example", "--retained-release-logical-mux-sdma",
        str(P.TESTS[case]), P.UID,
    ]


def controller(value):
    need(value["owned"] == OWNED and value["payload_sha256"] == PAYLOAD, "controller identity")
    need(value["local_only"] is False and value["failure"] is None, "successful native controller disposition")
    need(all(value[key] is True for key in ("native_attempted", "native_outer_passed", "collected_verified", "cleanup_closed", "independent_absence_closed")), "native collection and cleanup closed")


def controller_provenance(root, rows, payload, P):
    options = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3"]
    code = "import json,pathlib,protocol; root=pathlib.Path.cwd(); manifest=protocol.payload(root); print(json.dumps({'payload_sha256':protocol.sha(root/'payload.json'),'files':len(manifest),'verified':True},sort_keys=True))"
    expected = {
        "local-protocol-tests": (["/usr/bin/python3", "-B", LOCAL_BUNDLE + "/test_protocol.py", "-v"], 30, None),
        "local-payload-verify": (["/usr/bin/python3", "-B", "-c", code], 30, None),
        "local-controller-wiring-tests": (["/usr/bin/python3", "-B", LOCAL_CONTROL + "/test_controller.py", "-v"], 30, None),
        "upload": (["scp", "-r", *options, LOCAL_BUNDLE + "/.", "mi300x:" + OWNED + "/"], 180, None),
        "collect": (["scp", "-r", *options, "mi300x:" + OWNED, LOCAL_OUTPUT + "/collected"], 180, None),
    }
    helper = sha(root / "control/remote_control.py")
    for name, mode, bound in (("remote-empty", "empty", 45), ("remote-approve", "approve", 90), ("native-outer", "launch", 3700), ("remote-inventory", "inventory", 120)):
        expected[name] = (["ssh", "-T", *options, "mi300x", shlex.join(["/usr/bin/python3", "-B", "-", mode, OWNED])], bound, helper)
    for name, (command, bound, stdin) in expected.items():
        row = rows[name]
        need(row["command"] == command and row["bound_seconds"] == bound and row["stdin_sha256"] == stdin, "exact controller command provenance: " + name)
    native = root / "raw/native"
    for name, row in rows.items():
        need(row["cwd"] == LOCAL_BUNDLE, "exact local controller cwd")
        if name not in ("local-protocol-tests", "local-controller-wiring-tests"):
            need(not (native / name / "stderr.log").read_bytes(), "clean transport and payload-verifier stderr")
    need(V.load(native / "local-payload-verify/stdout.log") == {"payload_sha256": PAYLOAD, "files": 26, "verified": True}, "exact local payload verifier result")
    need(V.load(native / "remote-empty/stdout.log") == {"record": "fresh-marked-directory", "owned": OWNED}, "fresh remote marker only")
    need(V.load(native / "remote-approve/stdout.log") == {"record": "exact-payload-approved", "owned": OWNED, "payload_sha256": PAYLOAD}, "exact remote approval result")
    for name in ("upload", "collect"):
        need(not (native / name / "stdout.log").read_bytes(), "clean file transfer stdout")
    marker = {"commit": P.COMMIT, "path": OWNED, "payload_sha256": PAYLOAD}
    need(V.load(payload / "owner.json") == marker, "exact collected owner marker")
    need(V.load(payload / "approval.json") == {**marker, "root_reviewed": True, "shared_host_permission": "user-permits-currently-free-gpu", "post_observation_policy": "both-strict-t0-group-closed-immediate-0-1-delayed-20-21-v1"}, "exact prospective authorization policy")


def audit(root, *, allow_unsealed=False, source_root=None, binary=None):
    need(not any(path.is_symlink() for path in root.rglob("*")), "ordinary archive paths")
    sealed = V.manifest(root, allow_unsealed)
    payload = root / COLLECTED
    P, payload_map = source_binding(payload)
    V.creation(root / "raw/create", root / "control/create.py", OWNED, PAYLOAD)
    need(V.literal_assignment(root / "control/controller.py", "OWNED") == OWNED, "archived controller path")
    for file in ("controller.py", "remote_control.py"):
        need(V.literal_assignment(root / "control" / file, "PAYLOAD") == PAYLOAD, "archived control payload")
    native = root / "raw/native"
    inventory, observed = V.lines(native / "remote-inventory/stdout.log")
    need(inventory["record"] == "complete-owned-inventory" and inventory["owned"] == OWNED, "complete inventory identity")
    need(len(inventory["files"]) == len(payload_map) + 11 + 12 * len(CASES), "exact five-case file count")
    V.verify_inventory(payload, inventory, P.BINARY_SHA)
    need(V.load(native / "collection-verified.json") == {key: inventory[key] for key in ("owned", "files", "recorded_pids")}, "exact collection verification")
    pids = inventory["recorded_pids"]
    need(len(pids) == len(set(pids)) == 3 + 4 * len(CASES) and all(type(pid) is int and pid > 1 for pid in pids), "exact distinct owned PID roster")
    V.absence(observed, OWNED, pids)
    results = payload / "results"
    names = ["topology", "placement"] + [f"{case}-{kind}" for case in CASES for kind in ("preflight", "test", "immediate", "delayed")]
    need({p.name for p in results.iterdir()} == set(names) | {"campaign.json", "controller-launch.json"}, "exact native result roster")
    rows = {name: V.record(results / name, 0) for name in names}
    V.serial_chain(rows, names, native=True)
    for name, row in rows.items():
        folder = results / name
        need({p.name for p in folder.iterdir()} == {"record.json", "stdout.log", "stderr.log"}, "exact command receipt triplet")
        need(row["cwd"] == OWNED and row["environment"] == V.environment(P.UID), "exact native execution context")
        need(not (folder / "stderr.log").read_bytes(), "clean native command stderr")
        need(row["started"]["monotonic_ns"] <= row["spawned"]["monotonic_ns"] <= row["t0"]["monotonic_ns"] <= row["finished"]["monotonic_ns"], "complete native command timing")
    launch = V.load(results / "controller-launch.json")
    V.outer_command(launch, OWNED)
    need(launch["status"] == 0 and launch["error"] is None and launch["group_absent"] is True, "outer process successful and absent")
    need(sorted([launch["process_group"]] + [row["process_group"] for row in rows.values()]) == pids, "PID roster derived from every native receipt")
    need(rows["topology"]["command"] == ["/usr/bin/python3", "-B", OWNED + "/source/r26-host-guard.py", "topology", "--gpu-index", str(P.GPU), "--pci-bdf", P.BDF, "--unique-id", P.UID] and rows["topology"]["outer_bound_seconds"] == 75, "exact topology command")
    need((results / "topology/stdout.log").read_text() == P.TOPOLOGY, "exact frozen topology")
    need(rows["placement"]["command"] == ["/usr/bin/numactl", "--physcpubind=0-47", "--membind=0", "/usr/bin/numactl", "--show"] and rows["placement"]["outer_bound_seconds"] == 15, "exact placement command")
    P.placement((results / "placement/stdout.log").read_text())
    transcripts, offsets = {}, []
    for case in CASES:
        test = rows[case + "-test"]
        need(test["command"] == native_command(P, case, OWNED) and test["outer_bound_seconds"] == 200, "exact native profile argv and bounds")
        t0 = test["t0"]["monotonic_ns"]
        for label, offset in (("preflight", None), ("immediate", 0), ("delayed", 20)):
            name = f"{case}-{label}"
            value, completion = V.lines(results / name / "stdout.log")
            V.observer_command(rows[name], P, OWNED)
            V.endpoint_bracket(rows[name], value)
            need(completion == {"schema": "fe2o3.copy-host-observation.v1", "record": "complete", "observations": 1, "refused": 0, "all_endpoints_admitted": True, "performance_accepted": False}, "strict complete observer disposition")
            end = P.endpoint(value, t0=None if offset is None else t0, offset=offset)
            if offset is None:
                need(0 <= test["spawned"]["monotonic_ns"] - end <= 1_000_000_000, "fresh admission before native launch")
            else:
                offsets.append({"case": case, "endpoint": label, "start_offset_ns": value["started"]["monotonic_ns"] - t0})
        transcripts[case] = P.transcript(case, (results / f"{case}-test/stdout.log").read_text(), (results / f"{case}-test/stderr.log").read_text())
    state = V.load(results / "campaign.json")
    V.outer_transcript(native, launch, state, OWNED)
    need(state["commit"] == P.COMMIT and state["failure"] is None and state["payload_after"] == "matched" and state["native_ioctl_failure_claim"] is False and state["performance_claim"] is False and state["logical_lane_execution_claim"] is False and state["cursor_observation_claim"] is False, "successful campaign with bounded claims")
    need(state["cases"] == [{"case": case, "test_record": f"{case}-test/record.json", "failures": [], "post_observations": {"immediate": "strict_pass", "delayed": "strict_pass"}, "transcript": transcripts[case]} for case in CASES], "exact five complete case records")
    need(state["started"]["monotonic_ns"] <= rows["topology"]["started"]["monotonic_ns"] and rows[CASES[-1] + "-delayed"]["finished"]["monotonic_ns"] <= state["finished"]["monotonic_ns"], "entire native chain bracketed by campaign")
    local = V.load(native / "controller.json")
    controller(local)
    local_names = LOCAL_NAMES
    need(local["records"] == [{"name": name, "status": 0, "error": None, "group_absent": True} for name in local_names], "exact successful local controller roster")
    local_rows = {name: V.record(native / name, 0) for name in local_names}
    V.serial_chain(local_rows, local_names, native=False)
    controller_provenance(root, local_rows, payload, P)
    for tag, source, suite, count in (("local-protocol-tests", payload / "test_protocol.py", "ProtocolTests", 5), ("local-controller-wiring-tests", root / "control/test_controller.py", "ControllerTests", 4)):
        need(not (native / tag / "stdout.log").read_bytes(), "clean calibration stdout")
        V.unittest_transcript((native / tag / "stderr.log").read_text(), source, suite, count)
    for name, mode in (("remote-cleanup", "cleanup"), ("remote-independent-absence", "absence")):
        V.cleanup_command(local_rows[name], mode, pids, V.digest_map(inventory["files"]), sha(root / "control/remote_control.py"), OWNED)
    before, removed, after = V.lines(native / "remote-cleanup/stdout.log")
    V.absence(before, OWNED, pids)
    V.absence(after, OWNED, pids)
    need(removed == {"record": "exact-owned-directory-removed", "owned": OWNED, "removed_regular_files": len(inventory["files"]), "inventory_sha256": V.digest_map(inventory["files"])}, "exact collected inventory removed")
    final, absent = V.lines(native / "remote-independent-absence/stdout.log")
    V.absence(final, OWNED, pids)
    need(absent == {"absent": True, "owned": OWNED, "record": "independent-path-absence"}, "separate exact path absence")
    for file in (root / "raw").rglob("record.json"):
        V.record(file.parent, 0)
    cohort = V.load(payload / "cpu/source-before.log")["files"]
    if source_root is not None:
        need(all(sha(source_root / name) == digest for name, digest in cohort.items()), "all live source inputs match")
    if binary is not None:
        need(sha(binary) == P.BINARY_SHA, "supplied ELF matches qualified executable")
    return {"archive_integrity_sealed": sealed, "historical_audit": "passed", "native_campaign": "passed", "profiles": CASES, "native_commands_passed": len(CASES), "strict_endpoints": 3 * len(CASES), "post_observation_offsets": offsets, "refused_endpoints": 0, "recorded_native_pids_groups_absent": len(pids), "remote_files_collected": len(inventory["files"]), "remote_files_retained": len(inventory["files"]) - 1, "cleanup": "closed", "current_source_files_revalidated": len(cohort) if source_root is not None else None, "current_binary_revalidated": binary is not None, "formal_refinement": False, "performance_claim": False, "native_fault_injection": False, "cursor_scope": "initial-0-no-advance-source-qualified"}


def supplementary(root):
    row = {suffix: (root / f"audit/calibration-final.{suffix}").read_text() for suffix in ("command", "started", "finished", "exit", "log")}
    need(row["exit"] == "0\n", "archive calibration passed")
    need(shlex.split(row["command"]) == ["python3", "-B", "docs/evidence/" + root.name + "/test_verify.py", "-v"], "exact archive calibration command")
    need(all(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z\n", row[key]) for key in ("started", "finished")) and row["started"] <= row["finished"], "ordered calibration receipt")
    V.unittest_transcript(row["log"], root / "test_verify.py", "ArchiveTests", 10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--allow-unsealed", action="store_true")
    modes.add_argument("--seal", action="store_true")
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    result = audit(ROOT, allow_unsealed=args.allow_unsealed or args.seal, source_root=args.source_root, binary=args.binary)
    supplementary(ROOT)
    if args.seal:
        contents = "".join(f"{sha(path)}  {path.relative_to(ROOT).as_posix()}\n" for path in sorted(ROOT.rglob("*")) if path.is_file() and path != ROOT / "SHA256SUMS")
        with (ROOT / "SHA256SUMS").open("x") as target:
            target.write(contents)
        result["archive_integrity_sealed"] = V.manifest(ROOT, False)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
