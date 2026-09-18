#!/usr/bin/env python3
"""Audit the GPU4 refusal without promoting it to native runtime evidence."""

from pathlib import Path
import re

import verify as V

OWNED = "/tmp/fe2o3-striped-sdma-20260918.q34ifzz7"
PAYLOAD = "fa7711d1a8e2a822de1d93c403adb3a046e16ffbfcfb7b0f6f51cc15e7516cd3"
REASONS = [f"sysfs-{phase}-{reason}" for phase in ("before", "between", "after") for reason in ("busy", "vram")] + ["smi-busy", "smi-vram", "selected-gpu-attachments"]


def endpoint(P, value):
    V.need((P.GPU, P.BDF, P.UID) == (4, "0000:85:00.0", "0x54f88318ca05093d"), "rejected GPU identity")
    V.need(value["endpoint_admitted"] is False and value["reasons"] == REASONS and value["selected_pids"] == [4127510], "exact shared-host refusal")
    V.need(V.sha(P.OBSERVER_SOURCE) == P.OBSERVER_SHA, "pinned rejection observer")
    observer = V.module(P.OBSERVER_SOURCE)
    snapshots = iter(value["sysfs"])
    stamps = iter((value["started"], value["finished"]))
    captures = iter((value["status"], value["pids"]))

    def sysfs(root, bdf):
        sample = next(snapshots)
        V.need(sample["path"] == str(root / bdf), "raw sysfs capture identity")
        return sample

    def command(argv):
        captured = next(captures)
        V.need(captured["command"] == argv and captured["exit"] == 0 and captured["error"] is None and captured["stderr"] == "", "complete raw capture and exact command")
        return captured

    # Replay only archived captures through the pinned derivation, never devices.
    observer.stamp = lambda: next(stamps)
    observer.capture_sysfs = sysfs
    derived = observer.observe(P.GPU, P.BDF, P.UID, Path("/opt/rocm/bin/rocm-smi"), run=command)
    V.need(value == {**derived, "index": 0}, "entire refusal rederived from original raw captures")
    V.need(all(list(iterator) == [] for iterator in (snapshots, stamps, captures)), "all original captures consumed")
    metrics = [(int(row["values"]["gpu_busy_percent"]), int(row["values"]["mem_busy_percent"]), int(row["values"]["mem_info_vram_used"])) for row in value["sysfs"]]
    V.need(metrics == [(97, 4, 36341137408), (96, 4, 36341137408), (92, 7, 36341137408)], "exact rejected sysfs telemetry")
    V.need(observer.parse_status(value["status"]["stdout"], P.GPU, P.BDF, P.UID) == {"busy_percent": 95, "vram_bytes": 36341137408}, "exact rejected SMI telemetry")
    timeline = [value["started"]]
    for row in (value["sysfs"][0], value["status"], value["sysfs"][1], value["pids"], value["sysfs"][2]):
        timeline.extend((row["started"], row["finished"]))
    timeline.append(value["finished"])
    V.need(all(type(stamp["monotonic_ns"]) is int and stamp["monotonic_ns"] > 0 and re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", stamp["utc"]) for stamp in timeline), "typed rejected endpoint clocks")
    V.need(all(a["monotonic_ns"] <= b["monotonic_ns"] for a, b in zip(timeline, timeline[1:])), "ordered rejected endpoint clocks")


def audit(root):
    native = root / "raw/native"
    payload = native / "collected"
    V.need(V.sha(payload / "payload.json") == PAYLOAD, "rejected frozen payload")
    manifest = V.load(payload / "payload.json")
    P = V.module(payload / "protocol.py")
    V.creation(root / "raw/create", root / "control/create.py", OWNED, PAYLOAD)
    V.need(len(manifest) == 23 and manifest["queue-example"] == P.BINARY_SHA, "rejected payload closure")
    for name, digest in manifest.items():
        if name != "queue-example":
            V.need(V.sha(payload / name) == digest, "rejected payload bytes")
    accepted_payload = root.parent / V.COLLECTED
    for name in ("cpu/source-before.log", "cpu/source-after.log", "cpu/SHA256SUMS", "cpu/binary.log", "allowed-signers"):
        V.need(V.sha(payload / name) == V.sha(accepted_payload / name), "same qualified CPU/source/binary/signer cohort")
    binding = V.load(payload / "binding.json")
    V.need(binding["commit"] == P.COMMIT == "602fda830307f9818cbff5d57d4be68a897e75b7" and binding["source_files_matched"] == 5556 and binding["signature_exit"] == binding["cohort_exit"] == 0 and binding["binary_sha256"] == P.BINARY_SHA, "rejected signed containing source binding")
    for name in manifest:
        if name.startswith("source/"):
            V.need(V.sha(payload / name) == V.sha(accepted_payload / name), "same signed source snapshots")
    signature = (payload / "commit-signature.stderr").read_text()
    V.need("harmenon@amd.com" in signature and "SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg" in signature, "rejected historical signature receipt")
    inventory, observed = V.lines(native / "remote-inventory/stdout.log")
    V.need(V.load(native / "collection-verified.json") == {key: inventory[key] for key in ("owned", "files", "recorded_pids")}, "rejected collection verification matches remote inventory")
    V.need(inventory["record"] == "complete-owned-inventory" and inventory["owned"] == OWNED and len(inventory["files"]) == 37, "exact rejected remote inventory")
    V.verify_inventory(payload, inventory, P.BINARY_SHA)
    pids = inventory["recorded_pids"]
    V.need(len(pids) == len(set(pids)) == 4 and all(type(pid) is int and pid > 1 for pid in pids), "four rejected owned groups")
    V.absence(observed, OWNED, pids)
    results = payload / "results"
    names = ["topology", "placement", "striped-2-preflight"]
    V.need({path.name for path in results.iterdir()} == set(names) | {"campaign.json", "controller-launch.json"}, "no runtime test or post-observation launched")
    rows = {name: V.record(results / name, 1 if name.endswith("preflight") else 0) for name in names}
    for name, row in rows.items():
        V.need({p.name for p in (results / name).iterdir()} == {"record.json", "stdout.log", "stderr.log"}, "exact rejected command triplet")
        V.need(row["cwd"] == OWNED and row["environment"] == V.environment(P.UID) and not (results / name / "stderr.log").read_bytes(), "rejected command context")
        V.need(row["started"]["monotonic_ns"] <= row["spawned"]["monotonic_ns"] <= row["t0"]["monotonic_ns"] <= row["finished"]["monotonic_ns"], "rejected command timing")
    V.serial_chain(rows, names, native=True)
    launch = V.load(results / "controller-launch.json")
    V.outer_command(launch, OWNED)
    V.need(launch["status"] == 1 and launch["error"] is None and launch["group_absent"] is True, "rejected outer closure")
    V.need(sorted([launch["process_group"]] + [row["process_group"] for row in rows.values()]) == pids, "derived rejected process roster")
    V.need((results / "topology/stdout.log").read_text() == P.TOPOLOGY, "rejected exact topology")
    P.placement((results / "placement/stdout.log").read_text())
    value, completion = V.lines(results / "striped-2-preflight/stdout.log")
    V.observer_command(rows["striped-2-preflight"], P, OWNED)
    V.endpoint_bracket(rows["striped-2-preflight"], value)
    endpoint(P, value)
    V.need(completion == {"schema": "fe2o3.copy-host-observation.v1", "record": "complete", "observations": 1, "refused": 1, "all_endpoints_admitted": False, "performance_accepted": False}, "rejected observer completion")
    state = V.load(results / "campaign.json")
    V.need(state["commit"] == P.COMMIT and state["cases"] == [] and state["failure"] == "ValueError: fresh preflight command passed" and state["payload_after"] == "matched" and state["native_ioctl_failure_claim"] is False and state["performance_claim"] is False, "rejected before runtime binary launch")
    V.outer_transcript(native, launch, state, OWNED)
    local = V.load(native / "controller.json")
    V.need(local["owned"] == OWNED and local["payload_sha256"] == PAYLOAD and local["native_outer_passed"] is False and local["failure"] == "RuntimeError: native campaign rejected; receipts and cleanup retained", "controller retains rejection")
    V.need(all(local[key] is True for key in ("native_attempted", "collected_verified", "cleanup_closed", "independent_absence_closed")) and local["local_only"] is False, "rejected collection and cleanup close")
    local_names = ["local-protocol-tests", "local-payload-verify", "local-controller-wiring-tests", "remote-empty", "upload", "remote-approve", "native-outer", "remote-inventory", "collect", "remote-cleanup", "remote-independent-absence"]
    V.need(local["records"] == [{"name": name, "status": int(name == "native-outer"), "error": None, "group_absent": True} for name in local_names], "exact rejected controller roster")
    local_rows = {name: V.record(native / name, int(name == "native-outer")) for name in local_names}
    V.serial_chain(local_rows, local_names, native=False)
    for name, mode in (("remote-cleanup", "cleanup"), ("remote-independent-absence", "absence")):
        V.cleanup_command(local_rows[name], mode, pids, V.digest_map(inventory["files"]), V.sha(root / "control/remote_control.py"), OWNED)
    before, removed, after = V.lines(native / "remote-cleanup/stdout.log")
    V.absence(before, OWNED, pids)
    V.absence(after, OWNED, pids)
    V.need(removed == {"record": "exact-owned-directory-removed", "owned": OWNED, "removed_regular_files": 37, "inventory_sha256": V.digest_map(inventory["files"])}, "exact rejected collected files removed")
    final, absent = V.lines(native / "remote-independent-absence/stdout.log")
    V.absence(final, OWNED, pids)
    V.need(absent == {"absent": True, "owned": OWNED, "record": "independent-path-absence"}, "rejected independent path absence")
    return {"disposition": "rejected-before-runtime-binary-launch", "native_commands": 0, "refused_endpoints": 1, "remote_files_collected": 37, "owned_groups_absent": 4, "cleanup": "closed"}
