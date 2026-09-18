#!/usr/bin/env python3
"""Read-only GPU2 historical audit; never invokes SSH, devices, Cargo or ELF."""

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
PRIOR_SEAL = "2b1ab6fb62590ec926a550a3a78281f0fc5e126babc4c5f9edb1054d6e9eb95f"
OWNED = "/home/harsh/fe2o3-combined-sdma-20260918.2cf1a6cf"
PAYLOAD = "af0725e9424354c52cbef6a73eff917d54a0c599fb2047a4a559797d84e93abf"
CASES = [f"combined-{count}" for count in (2, 4, 6, 8, 10, 12, 14)]
COLLECTED = Path("raw/native/collected")
LOCAL_BUNDLE = "/home/harsh/.codex-tmp/fe2o3-combined-sdma-native-bundle-v3-20260918"
LOCAL_CONTROL = "/home/harsh/.codex-tmp/fe2o3-combined-sdma-control-v3-20260918"
LOCAL_OUTPUT = "/home/harsh/.codex-tmp/fe2o3-combined-sdma-native-v3-20260918"
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


def protocol_delta(protocol_text, prior_text, fixture_text, prior_fixture_text):
    current, prior = ast.parse(protocol_text), ast.parse(prior_text)
    need(len(current.body) == len(prior.body), "unchanged protocol statement roster")
    changed = set()
    for new, old in zip(current.body, prior.body):
        if not isinstance(old, ast.Assign) or len(old.targets) != 1:
            continue
        target = old.targets[0]
        identity = isinstance(target, ast.Tuple) and all(isinstance(item, ast.Name) for item in target.elts) and [item.id for item in target.elts] == ["GPU", "UID", "BDF"]
        topology = isinstance(target, ast.Name) and target.id == "TOPOLOGY"
        if identity or topology:
            need(isinstance(new, ast.Assign) and ast.dump(new.targets[0]) == ast.dump(target), "unchanged metadata assignment target")
            value = ast.literal_eval(new.value)
            if identity:
                need(value == (2, "0xd2e26fef80cf5c33", "0000:46:00.0"), "exact new GPU metadata")
                changed.add("identity")
            else:
                need(value == "topology schema=fe2o3.r26-host-topology.v1 placement=taskset-cpulist-then-numactl-physcpubind-membind-v1 gpu_index=2 pci_bdf=0000:46:00.0 unique_id=0xd2e26fef80cf5c33 numa_node=0 device_local_cpu_list=0-47 allowed_cpu_list=0-95 allowed_mem_node_list=0-1 measurement_cpu_list=0-47 observer_cpu=95 kfd_node=4 kfd_gpu_id=29122 topology_sha256=fe3f37d829f89661660b9ab1e80d453237580a84f7676bad67bf0a70158e07e6\n", "exact new topology metadata")
                changed.add("topology")
            new.value = old.value
    need(changed == {"identity", "topology"} and ast.dump(current) == ast.dump(prior), "all protocol semantics unchanged outside selected device metadata")
    expected_fixture = ast.parse(prior_fixture_text)
    replacements = {
        "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\nPID 123 is using 1 DRM device(s):\n2\n===\n=== End of ROCm SMI Log ===\n": "=== ROCm System Management Interface ===\n=== GPUs Indexed by PID ===\nPID 123 is using 1 DRM device(s):\n0\n===\n=== End of ROCm SMI Log ===\n",
        "\n2\n": "\n0\n",
    }
    found = []
    for node in ast.walk(expected_fixture):
        if isinstance(node, ast.Constant) and isinstance(node.value, str) and node.value in replacements:
            found.append(node.value)
            node.value = replacements[node.value]
    need(len(found) == 2 and set(found) == set(replacements), "exact two prior nonselected-PID fixture constants")
    need(ast.dump(ast.parse(fixture_text)) == ast.dump(expected_fixture), "only nonselected-PID fixture identity changed")


def source_binding(payload, prior_payload):
    need(sha(payload / "payload.json") == PAYLOAD, "frozen GPU2 payload")
    manifest = V.load(payload / "payload.json")
    prior = V.load(prior_payload / "payload.json")
    need(set(manifest) == set(prior) and len(manifest) == 25, "exact unchanged payload roster")
    mutable = {"PLAN.md", "protocol.py", "test_protocol.py", "binding.json"}
    for name, digest in manifest.items():
        if name != "queue-example":
            need(sha(payload / name) == digest, "frozen payload bytes: " + name)
        if name not in mutable:
            need(digest == prior[name], "unchanged qualified input: " + name)
    protocol_delta((payload / "protocol.py").read_text(), (prior_payload / "protocol.py").read_text(), (payload / "test_protocol.py").read_text(), (prior_payload / "test_protocol.py").read_text())
    P = V.module(payload / "protocol.py")
    old = V.module(prior_payload / "protocol.py")
    for key in ("COMMIT", "BINARY_SHA", "CPU_SEAL", "COHORT_SHA", "COHORT_FILE_COUNT", "SIGNERS_SHA", "OBSERVER_SHA", "TOPOLOGY_SHA", "PROFILE_SHA"):
        need(getattr(P, key) == getattr(old, key), "unchanged qualified binding: " + key)
    need(P.TESTS == {name: int(name.split("-")[1]) for name in CASES} and list(P.TESTS) == CASES, "exact ordered seven-case plan")
    need((P.GPU, P.BDF, P.UID) == (2, "0000:46:00.0", "0xd2e26fef80cf5c33"), "new selected device identity")
    binding = V.load(payload / "binding.json")
    expected = {
        "commit": P.COMMIT,
        "source_files_matched": P.COHORT_FILE_COUNT,
        "cpu_manifest_sha256": P.CPU_SEAL,
        "binary_sha256": P.BINARY_SHA,
        "cohort_sha256": P.COHORT_SHA,
        "signature_exit": 0,
        "cohort_exit": 0,
        "source_base_field_ignored_only": True,
        "native_authorized": False,
    }
    need(all(binding.get(key) == value for key, value in expected.items()), "new signed export binding")
    need(0 < binding["started_ns"] <= binding["finished_ns"], "ordered export receipt")
    return P, manifest


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
    need(V.load(native / "local-payload-verify/stdout.log") == {"payload_sha256": PAYLOAD, "files": 25, "verified": True}, "exact local payload verifier result")
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
    need(sha(PRIOR / "SHA256SUMS") == PRIOR_SEAL, "unchanged earlier rejected packet")
    earlier = V.audit(PRIOR)
    V.supplementary(PRIOR)
    need(earlier["native_campaign"] == "rejected-delayed-shared-host-observation", "prior refusal is not promoted")
    payload = root / COLLECTED
    P, payload_map = source_binding(payload, PRIOR / V.COLLECTED)
    V.creation(root / "raw/create", root / "control/create.py", OWNED, PAYLOAD)
    need(V.literal_assignment(root / "control/controller.py", "OWNED") == OWNED, "archived controller path")
    for file in ("controller.py", "remote_control.py"):
        need(V.literal_assignment(root / "control" / file, "PAYLOAD") == PAYLOAD, "archived control payload")
    native = root / "raw/native"
    inventory, observed = V.lines(native / "remote-inventory/stdout.log")
    need(inventory["record"] == "complete-owned-inventory" and inventory["owned"] == OWNED, "complete inventory identity")
    need(len(inventory["files"]) == len(payload_map) + 11 + 12 * len(CASES), "exact seven-case file count")
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
        need(test["command"] == V.native_command(P, case, OWNED) and test["outer_bound_seconds"] == 200, "exact native profile argv and bounds")
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
    need(state["commit"] == P.COMMIT and state["failure"] is None and state["payload_after"] == "matched" and state["native_ioctl_failure_claim"] is False and state["performance_claim"] is False, "successful campaign with bounded claims")
    need(state["cases"] == [{"case": case, "test_record": f"{case}-test/record.json", "failures": [], "post_observations": {"immediate": "strict_pass", "delayed": "strict_pass"}, "transcript": transcripts[case]} for case in CASES], "exact seven complete case records")
    need(state["started"]["monotonic_ns"] <= rows["topology"]["started"]["monotonic_ns"] and rows[CASES[-1] + "-delayed"]["finished"]["monotonic_ns"] <= state["finished"]["monotonic_ns"], "entire native chain bracketed by campaign")
    local = V.load(native / "controller.json")
    controller(local)
    local_names = LOCAL_NAMES
    need(local["records"] == [{"name": name, "status": 0, "error": None, "group_absent": True} for name in local_names], "exact successful local controller roster")
    local_rows = {name: V.record(native / name, 0) for name in local_names}
    V.serial_chain(local_rows, local_names, native=False)
    controller_provenance(root, local_rows, payload, P)
    for tag, source, suite in (("local-protocol-tests", payload / "test_protocol.py", "ProtocolTests"), ("local-controller-wiring-tests", root / "control/test_controller.py", "ControllerTests")):
        need(not (native / tag / "stdout.log").read_bytes(), "clean calibration stdout")
        V.unittest_transcript((native / tag / "stderr.log").read_text(), source, suite, 4)
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
    return {"archive_integrity_sealed": sealed, "historical_audit": "passed", "native_campaign": "passed", "profiles": CASES, "native_commands_passed": len(CASES), "strict_endpoints": 3 * len(CASES), "post_observation_offsets": offsets, "refused_endpoints": 0, "recorded_native_pids_groups_absent": len(pids), "remote_files_collected": len(inventory["files"]), "remote_files_retained": len(inventory["files"]) - 1, "cleanup": "closed", "prior_campaign": earlier["native_campaign"], "current_source_files_revalidated": len(cohort) if source_root is not None else None, "current_binary_revalidated": binary is not None, "formal_refinement": False, "performance_claim": False, "native_fault_injection": False, "cursor_scope": "striped-initial-0-no-advance-source-qualified"}


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
