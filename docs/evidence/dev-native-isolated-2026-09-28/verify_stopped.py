#!/usr/bin/env python3
"""Replay six successes and the final depth-setup failure; never qualify the matrix."""
import json
from pathlib import Path
import re
import runpy
import shlex
import sys
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent


def verify(campaign):
    trusted = SimpleNamespace(**runpy.run_path(str(HERE / "verify.py")))
    s, p, n, b, c, v = trusted.helpers(campaign / "protocol")
    initial = v.read(campaign / "protocol-before.json")
    p.need(initial == v.read(campaign / "protocol-after.json") == b.inventory(campaign / "protocol"), "helper continuity")
    build = s.module(campaign / "protocol/verify_build.py", initial["verify_build.py"])
    build.REPO = trusted.REPO
    qualified = build.verify(campaign.parent)
    p.need(qualified == v.read(campaign / "build-binding.json"), "actual signed-source CPU build")
    binding, marker = v.read(campaign / "binding.json"), v.read(campaign / "owner.json")
    p.need(set(marker) == {"path", "commit", "binding_sha256"}
           and re.fullmatch(re.escape(p.PREFIX) + "[0-9a-f]{16}", marker["path"])
           and marker["commit"] == p.COMMIT and marker["binding_sha256"] == p.sha(campaign / "binding.json"), "owned marker")
    root, remote = Path(marker["path"]), campaign / "remote"
    p.need(b.inventory(remote) == v.read(campaign / "remote-inventory.json"), "complete remote inventory")
    payload = b.inventory(campaign / "payload")
    p.need(payload == b.inventory(remote / "artifacts") and set(payload) == n.PAYLOAD, "whole retained payload")
    p.need(payload["runtime-tests"] == qualified["elf_sha256"]
           and all(payload[name] == initial[name] for name in n.PAYLOAD - {"runtime-tests"}), "ELF and helper joins")
    expected = {"commit": p.COMMIT, "payload": payload, "order": [list(row) for row in p.CASE_NAMES],
                "outer_seconds": n.REMOTE_SECONDS, "protocol": initial, "build_sha256": p.sha(campaign / "build-binding.json"),
                "source_files": v.read(campaign.parent / "build-inventory.json")["source"],
                "device": [p.GPU, p.BDF, p.UID], "target": qualified["target"], "features": qualified["features"],
                "build_environment": qualified["environment"]}
    p.need(p.same(binding, expected), "exact campaign binding")
    ordered = []
    def native(name, argv, env, bound, status=0):
        row = v.native_command(remote / name, argv, root, env, bound, status)
        if ordered:
            p.need(ordered[-1]["finished"]["monotonic_ns"] <= row["started"]["monotonic_ns"], "serialized commands")
        ordered.append(row)
        return row
    observer = p.load_module(remote / "artifacts/observer.py", p.OBSERVER_SHA, "stopped_observer")
    def observe(name, t0=None, offset=None):
        row = native(name, ["/usr/bin/python3", "-I", "-B", str(root / "observer.py"), "--gpu-index", str(p.GPU),
                            "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), n.OBSERVE_SECONDS)
        p.need((remote / name / "stderr.log").read_bytes() == b"", "empty observer stderr")
        value = p.endpoint((remote / name / "stdout.log").read_bytes(), observer, t0=t0, offset=offset)
        p.need(row["spawned"]["monotonic_ns"] <= p.stamp(value["started"]) <= p.stamp(value["finished"])
               <= row["t0"]["monotonic_ns"], "enclosed observation")
        return value
    native("topology", ["/usr/bin/python3", "-I", "-B", str(root / "topology.py"), "topology", "--gpu-index", str(p.GPU),
                        "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), 100)
    n.topology((remote / "topology/stdout.log").read_text(), p)
    native("placement", ["/usr/bin/numactl", "--physcpubind=0-47", "--membind=0", "/usr/bin/numactl", "--show"],
           p.environment(root), 30)
    n.placement((remote / "placement/stdout.log").read_text(), p)
    for name in ("topology", "placement"):
        p.need((remote / name / "stderr.log").read_bytes() == b"", "clean topology/placement")
    cases = []
    for ordinal, (case, _) in enumerate(p.CASE_NAMES, 1):
        name = f"{ordinal:02}-{case}"
        resources = v.read(remote / (name + "-resources.json"))
        p.need(set(resources) == {"global", "node0", "disk_free"}, "resource roster")
        for key, label in (("global", "MemAvailable"), ("node0", "MemFree")):
            values = re.findall(r"(?:^| )" + label + r":\s+([0-9]+) kB$", resources[key], re.M)
            p.need(len(values) == 1 and int(values[0]) * 1024 >= 4 * 1024**3, "memory headroom")
        p.need(type(resources["disk_free"]) is int and resources["disk_free"] >= 1024**3, "disk headroom")
        before = observe(name + "-before")
        row = native(name + "-test", p.command(root, case), p.environment(root, case),
                     p.test_seconds(case), 101 if case == "depth" else 0)
        p.need(0 <= row["spawned"]["monotonic_ns"] - p.stamp(before["finished"]) <= 10**9, "fresh admitted launch")
        observe(name + "-immediate", row["t0"]["monotonic_ns"], 0)
        observe(name + "-delayed", row["t0"]["monotonic_ns"], 20)
        stdout = (remote / (name + "-test/stdout.log")).read_text()
        stderr = (remote / (name + "-test/stderr.log")).read_text()
        transcript = None
        if case == "depth":
            test = re.escape(p.TESTS[case])
            p.need(re.fullmatch(
                r"\nrunning 1 test\ntest " + test + r" \.\.\. FAILED\n\nfailures:\n\nfailures:\n    "
                + test + r"\n\ntest result: FAILED\. 0 passed; 1 failed; 0 ignored; 0 measured; "
                + str(p.FILTERED) + r" filtered out; finished in [0-9]+\.[0-9]+s\n\n", stdout),
                "exact failed depth harness without publication/output markers")
            p.need(re.fullmatch(
                r"\nthread '" + test + r"' \([0-9]+\) panicked at "
                r"crates/fe2o3-runtime/src/kfd_backend/scale_capacity/native_depth.rs:261:75:\n"
                r"called `Option::unwrap\(\)` on a `None` value\n"
                r"note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n",
                stderr), "exact preserved depth budget setup failure")
        else:
            transcript = p.transcript(case, stdout, stderr)
        cases.append({"case": case, "ordinal": ordinal,
                      "failures": ["test: ValueError('native command passed')"] if case == "depth" else [],
                      "transcript": transcript,
                      "post_observations": {"immediate": "strict_pass", "delayed": "strict_pass"}})
    p.need(p.same(v.read(remote / "finished.json"), {"commit": p.COMMIT, "cases": cases, "spawn_failures": [],
        "failures": [repr(ValueError("case rejected; no subsequent case: " + repr(cases[-1])))], "complete_matrix": False,
        "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}), "stopped disposition")
    p.need(len({row["process_group"] for row in ordered}) == 30, "thirty distinct reaped groups")
    for index, row in enumerate(ordered, 1):
        active = v.read(remote / f"active-{index:03}/receipt.json")
        p.need(set(active) == {"pid", "command", "started", "purpose"} and type(active["pid"]) is int
               and active["pid"] == row["process_group"] and active["command"] == row["command"]
               and active["purpose"] == "record group before unblocking managed signals"
               and row["started"]["monotonic_ns"] <= active["started"]["monotonic_ns"] <= row["spawned"]["monotonic_ns"], "active custody")
    names = {row["name"] for row in ordered} | {f"active-{i:03}" for i in range(1, 31)}
    p.need({path.name for path in remote.iterdir()} == names | {"outer", "finished.json", "artifacts"} | {f"{i:02}-{case}-resources.json" for i, (case, _) in enumerate(p.CASE_NAMES, 1)},
           "no later cell or omitted remote file")
    outer = v.read(remote / "outer/receipt.json")
    p.need(set(outer) == {"pid", "controller_pid", "purpose"} and all(type(outer[key]) is int and outer[key] > 0 for key in ("pid", "controller_pid"))
           and outer["purpose"] == "remote timeout process group", "outer custody")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = c.control_bytes(p, campaign / "payload/base.py")
    p.need((campaign / "control.py").read_bytes() == wire, "exact owned cleanup controller")
    def control(name):
        return ["ssh", "-T", *c.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])]
    remote_argv = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", "4500s", "/usr/bin/python3", "-I", "-B", "-c", c.BOOTSTRAP, serialized]
    specs = [("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(trusted.ORIGINAL_HERE / "test_protocol.py")], 180, None, 0),
             ("create", control("create"), 120, wire, 0),
             ("upload", ["scp", "-q", *c.SSH, "--", *(str(trusted.ORIGINAL / "payload" / name) for name in sorted(n.PAYLOAD)),
                         str(trusted.ORIGINAL / "binding.json"), "mi300x:" + str(root) + "/"], 120, None, 0),
             ("native", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(remote_argv)], 4620, None, 1),
             ("inventory", control("inventory"), 120, wire, 0),
             ("collect", ["scp", "-q", "-r", *c.SSH, "--", "mi300x:" + str(root) + "/results", str(trusted.ORIGINAL / "remote")], 180, None, 0),
             ("cleanup", control("cleanup"), 120, wire, 0), ("absence", control("absence"), 120, wire, 0)]
    p.need({path.name for path in (campaign / "commands").iterdir()} == {row[0] for row in specs}, "exact local roster")
    previous = 0
    for name, argv, bound, stdin, status in specs:
        row = v.command(campaign / "commands" / name, argv, trusted.ORIGINAL_REPO, trusted.ENV, bound, stdin, status)
        p.need(previous <= row["started_ns"], "collection precedes cleanup and absence")
        previous = row["finished_ns"]
    p.need(v.read(campaign / "commands/create/stdout") == marker, "owned creation")
    p.need(v.read(campaign / "commands/inventory/stdout") == v.read(campaign / "remote-inventory.json"), "complete collection")
    p.need(v.read(campaign / "commands/cleanup/stdout") == {"removed": str(root)}, "owned removal")
    p.need(p.same(v.read(campaign / "commands/absence/stdout"), {"path_absent": True, "processes_absent": True}), "independent absence")
    p.need(p.same(v.read(campaign / "collection.json"), {"failures": ["RuntimeError('native failed')"], "owned_cleanup": True,
        "collected": True, "exclusive_reservation": False, "performance_acceptance": False}), "failed campaign retained and cleaned")
    return {"qualified": False, "attempted_cells": 7, "harness_passes": 6, "failed_cells": 1, "unrun_cells": 0,
            "strict_endpoints": 21, "remote_commands": 30, "owned_cleanup": True,
            "failure": "depth fixture exceeds the 256 backing-record session limit",
            "cpu_passes": qualified["cpu_passes"], "elf_sha256": qualified["elf_sha256"],
            "formal_refinement": False, "performance_acceptance": False}



if __name__ == "__main__":
    if not sys.flags.isolated or not sys.flags.dont_write_bytecode or sys.flags.optimize:
        raise ValueError("use python3 -I -B")
    print(json.dumps(verify(Path(sys.argv[1])), sort_keys=True))
