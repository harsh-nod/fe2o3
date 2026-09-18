#!/usr/bin/env python3
"""Fail-closed receipt envelope; ratios require all sixteen guarded processes."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
PINNED = HERE / "source" if (HERE / "source").is_dir() else HERE / "pinned"
OWNED = "/tmp/fe2o3-kfd-matched-9b9265c69-20260918.EOQ4SkyD"
SOURCE = OWNED + "/source"
COMMIT = "9b9265c6919cb8dff9506f2c6ffa7b7f2538905f"
ORDERS = ("ABDC", "BCAD", "CDBA", "DACB")
EXPECTED = [f"{rep}-{cell}" for rep, order in enumerate(ORDERS, 1) for cell in order]
VISIBILITY = [
    "HIP_VISIBLE_DEVICES",
    "ROCR_VISIBLE_DEVICES",
    "CUDA_VISIBLE_DEVICES",
    "GPU_DEVICE_ORDINAL",
]
UID, BDF = "0x54f88318ca05093d", "0000:85:00.0"
OBSERVER = [
    "/usr/bin/python3",
    "-B",
    SOURCE + "/benchmarks/runtime_gfx942/copy-host-observe.py",
    "--gpu-index",
    "4",
    "--pci-bdf",
    BDF,
    "--unique-id",
    UID,
    "--samples",
    "1",
]
PREFIX = [
    "/usr/bin/timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "180s",
    "/usr/bin/prlimit",
    "--core=0:0",
    "--",
    "/usr/bin/numactl",
    "--physcpubind=48-95",
    "--membind=1",
]
MODES = {
    "A": "diagnostic-slice50us",
    "B": "diagnostic-native-sleep1ms",
    "C": "diagnostic-native-sleep25us",
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


need(__debug__, "pinned payload assertions require non-optimized Python")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique(pairs):
    row = {}
    for key, value in pairs:
        need(key not in row, "duplicate JSON key")
        row[key] = value
    return row


def loads(text):
    return json.loads(text, object_pairs_hook=unique)


def imported(name, relative, expected):
    path = PINNED / relative
    need(digest(path) == expected, "pinned helper identity: " + relative)
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


protocol = imported(
    "pinned_payload",
    "docs/evidence/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py",
    "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9",
)
observer = imported(
    "pinned_observer",
    "benchmarks/runtime_gfx942/copy-host-observe.py",
    "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51",
)


def clock(value):
    need(set(value) == {"utc", "monotonic_ns"}, "clock fields")
    need(
        re.fullmatch(r"2026-09-18T\d{2}:\d{2}:\d{2}\.\d{9}Z", value["utc"]) is not None,
        "UTC timestamp",
    )
    need(
        type(value["monotonic_ns"]) is int and value["monotonic_ns"] > 0,
        "monotonic timestamp",
    )
    return value["monotonic_ns"]


def ordered(first, second):
    need(
        clock(first) <= clock(second) and first["utc"] <= second["utc"],
        "timestamp order",
    )


def payload(cell, stdout, stderr):
    need(stderr == "", "native stderr must be empty")
    lines = stdout.splitlines()
    need(
        stdout.endswith("\n") and all(line for line in lines),
        "complete nonempty payload lines",
    )
    if cell == "A":
        return protocol.base.validate_kfd(lines, "A")
    if cell in ("B", "C"):
        return protocol.validate_native(lines, cell)
    need(cell == "D", "unknown cell")
    return protocol.base.validate_hsa(lines, "D")


def topology(stdout, placement):
    need(stdout == protocol.TOPOLOGY + "\n", "exact UID/BDF/KFD/NUMA/CPU topology")
    expected = [
        "policy: bind",
        "preferred node: 1",
        "physcpubind: " + " ".join(str(cpu) for cpu in range(48, 96)),
        "cpubind: 1",
        "nodebind: 1",
        "membind: 1",
        "preferred: 1",
    ]
    need(
        [line.rstrip() for line in placement.splitlines()] == expected,
        "effective NUMA/CPU placement",
    )


def endpoint(stdout, receipt):
    lines = stdout.splitlines()
    need(len(lines) == 2 and stdout.endswith("\n"), "complete observer envelope")
    row, end = map(loads, lines)
    need(
        set(row)
        == {
            "schema",
            "record",
            "started",
            "finished",
            "gpu_index",
            "pci_bdf",
            "unique_id",
            "vram_limit_exclusive",
            "visibility_filters",
            "sysfs",
            "status",
            "pids",
            "selected_pids",
            "endpoint_admitted",
            "reasons",
            "scope",
            "index",
        },
        "observation field roster",
    )
    need(
        row["schema"] == "fe2o3.copy-host-observation.v1"
        and row["record"] == "observation",
        "observation schema",
    )
    need(
        (row["gpu_index"], row["pci_bdf"], row["unique_id"], row["index"])
        == (4, BDF, UID, 0),
        "observer selection",
    )
    need(
        row["vram_limit_exclusive"] == 536870912
        and row["visibility_filters"] == "removed-for-cli",
        "guard bounds",
    )
    need(
        row["scope"]
        == "sequential-endpoint-observations-not-continuous-monitoring-or-reservation",
        "observer scope",
    )
    ordered(receipt["started"], row["started"])
    ordered(row["finished"], receipt["finished"])
    need(len(row["sysfs"]) == 3, "all three direct sysfs observations")
    spans = [
        row["sysfs"][0],
        row["status"],
        row["sysfs"][1],
        row["pids"],
        row["sysfs"][2],
    ]
    previous = row["started"]
    for span in spans:
        ordered(previous, span["started"])
        ordered(span["started"], span["finished"])
        previous = span["finished"]
    ordered(previous, row["finished"])
    reasons = []
    for label, snap in zip(("before", "between", "after"), row["sysfs"]):
        need(
            set(snap) == {"started", "finished", "path", "values", "errors"},
            "sysfs fields",
        )
        need(snap["path"] == "/sys/bus/pci/devices/" + BDF, "direct BDF sysfs binding")
        try:
            values = snap["values"]
            if snap["errors"] or values["unique_id"].lower() != UID[2:]:
                raise ValueError("invalid sysfs capture")
            need(set(values) == set(observer.METRICS), "complete sysfs metric roster")
            parsed = {
                name: observer.decimal(
                    values[name], 100 if name.endswith("percent") else 2**64 - 1
                )
                for name in observer.METRICS
                if name != "unique_id"
            }
            if parsed["gpu_busy_percent"]:
                reasons.append(f"sysfs-{label}-busy")
            if parsed["mem_info_vram_used"] >= 536870912:
                reasons.append(f"sysfs-{label}-vram")
        except (ValueError, KeyError, TypeError, AttributeError):
            reasons.append(f"sysfs-{label}-invalid")
    selected = None
    for label in ("status", "pids"):
        capture = row[label]
        need(
            set(capture)
            == {"command", "started", "finished", "exit", "error", "stdout", "stderr"},
            "CLI capture fields",
        )
        suffix = (
            [
                "--showuse",
                "--showmeminfo",
                "vram",
                "--showuniqueid",
                "--showbus",
                "--json",
            ]
            if label == "status"
            else ["--showpidgpus"]
        )
        need(
            capture["command"]
            == ["/usr/bin/timeout", "--kill-after=5s", "20s", "/opt/rocm/bin/rocm-smi"]
            + suffix,
            "exact CLI command",
        )
        if capture["exit"] != 0 or capture["error"] or capture["stderr"].strip():
            reasons.append(f"{label}-capture-failed")
            continue
        try:
            if label == "status":
                metrics = observer.parse_status(capture["stdout"], 4, BDF, UID)
                if metrics["busy_percent"]:
                    reasons.append("smi-busy")
                if metrics["vram_bytes"] >= 536870912:
                    reasons.append("smi-vram")
            else:
                selected = sorted(
                    pid
                    for pid, devices in observer.parse_pids(capture["stdout"]).items()
                    if 4 in devices
                )
                if selected:
                    reasons.append("selected-gpu-attachments")
        except (ValueError, KeyError, TypeError, AttributeError):
            reasons.append(f"{label}-invalid")
    need(
        row["reasons"] == reasons and row["selected_pids"] == selected,
        "independently derived refusal and PID list",
    )
    need(row["endpoint_admitted"] is (not reasons), "sticky admission")
    need(
        end
        == {
            "schema": "fe2o3.copy-host-observation.v1",
            "record": "complete",
            "observations": 1,
            "refused": int(bool(reasons)),
            "all_endpoints_admitted": not reasons,
            "performance_accepted": False,
        },
        "observer terminal record",
    )
    need(receipt["exit"] == int(bool(reasons)), "observer process status")
    return {
        "admitted": not reasons,
        "reasons": reasons,
        "selected_pids": selected,
        "sysfs_vram": [
            int(item["values"]["mem_info_vram_used"]) for item in row["sysfs"]
        ],
    }


def manifest(path):
    rows = {}
    for line in path.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        need(
            match is not None and match[2] not in rows,
            "unique canonical manifest paths",
        )
        rows[match[2]] = match[1]
    need(bool(rows), "nonempty manifest")
    return rows


def audit(results, require_full):
    need(
        digest(HERE / "source-files.sha256")
        == "9d15e3613cc69f3a2f1437ec23c48788248b9244e81f2953da6d9db46e86c16d",
        "frozen source manifest identity",
    )
    need(
        manifest(results / "binaries.sha256")
        == {
            OWNED
            + "/target/release/examples/gfx942-runtime-directional-window-benchmark": "8cc0a27ff79a55a6141f61d9fd7e52476ef39421dbc7d6dbd1b8f8aafc14a182",
            OWNED
            + "/hsa-copy-pool-engine": "5d7e0578ea8078bf10066bbd7f36fe78608449af6fb6d647f2ec4503e86d9e33",
        },
        "exact prepared executable identities",
    )
    need(
        digest(results / "platform.sha256")
        == "697788432d2a18e3a7947b0473b542fd7f8c99d56f9510d27d820763c91c3424",
        "exact prepared library/header/tool identities",
    )
    for remote, expected in manifest(HERE / "scripts.sha256").items():
        need(
            remote.startswith(OWNED + "/") and "/" not in remote[len(OWNED) + 1 :],
            "owned script manifest path",
        )
        need(
            digest(HERE / Path(remote).name) == expected,
            "current reviewed script identity",
        )
    root = results / "campaign"
    state = loads((results / "campaign.json").read_text())
    need(
        set(state)
        == {
            "schema",
            "source_commit",
            "orders",
            "started",
            "cells",
            "failure",
            "scope",
            "finished",
            "records",
            "exit",
        },
        "campaign fields",
    )
    need(
        state["schema"] == "fe2o3.matched-campaign.v1"
        and state["source_commit"] == COMMIT
        and state["orders"] == list(ORDERS),
        "campaign identity",
    )
    need(
        state["scope"] == "guarded-shared-host-endpoints-not-reservation",
        "campaign scope",
    )
    ordered(state["started"], state["finished"])
    names = state["records"]
    need(len(set(names)) == len(names), "duplicate receipt name")
    need(
        set(item.name for item in root.iterdir())
        == {
            name + suffix
            for name in names
            for suffix in (".json", ".stdout", ".stderr")
        },
        "closed receipt directory",
    )
    rows, outputs, previous = {}, {}, state["started"]
    for index, name in enumerate(names):
        need(name.startswith(f"{index:03d}-"), "receipt ordinal")
        row = loads((root / f"{name}.json").read_text())
        need(
            set(row)
            == {
                "schema",
                "name",
                "argv",
                "cwd",
                "environment_override",
                "visibility_unset",
                "started",
                "pid",
                "exit",
                "error",
                "group_absent",
                "outer_bound_seconds",
                "finished",
                "stdout_sha256",
                "stderr_sha256",
            },
            "receipt fields",
        )
        need(
            row["schema"] == "fe2o3.matched-command.v1" and row["name"] == name,
            "receipt binding",
        )
        need(
            row["cwd"] == SOURCE and row["visibility_unset"] == VISIBILITY,
            "cwd/environment binding",
        )
        need(
            row["outer_bound_seconds"] == 200
            and type(row["pid"]) is int
            and row["pid"] > 0,
            "bounded owned process",
        )
        need(
            row["error"] is None and row["group_absent"] is True,
            "normal exit and no remaining owned process group",
        )
        need(
            type(row["exit"]) is int and 0 <= row["exit"] <= 255, "normal process exit"
        )
        ordered(previous, row["started"])
        ordered(row["started"], row["finished"])
        previous = row["finished"]
        for stream in ("stdout", "stderr"):
            path = root / f"{name}.{stream}"
            need(digest(path) == row[stream + "_sha256"], "raw output identity")
        rows[name] = row
        outputs[name] = (
            (root / f"{name}.stdout").read_text(),
            (root / f"{name}.stderr").read_text(),
        )
    ordered(previous, state["finished"])
    cursor = 0

    def take(tag, argv, environment=None, exit_code=0):
        nonlocal cursor
        name = f"{cursor:03d}-{tag}"
        need(
            cursor < len(names) and names[cursor] == name,
            "exact serial command roster: " + name,
        )
        row = rows[name]
        need(
            row["argv"] == argv and row["environment_override"] == (environment or {}),
            "exact command/environment: " + name,
        )
        if exit_code is not None:
            need(row["exit"] == exit_code, "required command success: " + name)
        cursor += 1
        return row, outputs[name]

    for phase in ("before",):
        for tag, remote, local in (
            ("source", OWNED + "/source-files.sha256", HERE / "source-files.sha256"),
            (
                "binaries",
                OWNED + "/results/binaries.sha256",
                results / "binaries.sha256",
            ),
            (
                "platform",
                OWNED + "/results/platform.sha256",
                results / "platform.sha256",
            ),
            ("scripts", OWNED + "/scripts.sha256", HERE / "scripts.sha256"),
        ):
            _, streams = take(tag + "-" + phase, ["/usr/bin/sha256sum", "-c", remote])
            need(
                streams == ("".join(path + ": OK\n" for path in manifest(local)), ""),
                "complete identity check output",
            )
    _, topology_out = take(
        "topology",
        [
            "/usr/bin/python3",
            "-B",
            SOURCE + "/benchmarks/runtime_gfx942/r26-host-guard.py",
            "topology",
            "--gpu-index",
            "4",
            "--pci-bdf",
            BDF,
            "--unique-id",
            UID,
        ],
    )
    _, placement_out = take(
        "placement",
        [
            "/usr/bin/numactl",
            "--physcpubind=48-95",
            "--membind=1",
            "/usr/bin/numactl",
            "--show",
        ],
    )
    need(not topology_out[1] and not placement_out[1], "clean topology stdout")
    topology(topology_out[0], placement_out[0])
    take(
        "topology-check",
        [
            "/usr/bin/python3",
            "-B",
            OWNED + "/check.py",
            "--topology",
            OWNED + "/results/campaign/004-topology.stdout",
            OWNED + "/results/campaign/005-placement.stdout",
        ],
    )
    need(
        [item["key"] for item in state["cells"]] == EXPECTED[: len(state["cells"])]
        and 1 <= len(state["cells"]) <= 16,
        "exact cell prefix",
    )
    observed, metrics, failed = [], {}, False
    for item in state["cells"]:
        need(
            set(item) == {"key", "records", "admitted"} and not failed,
            "no phase after sticky failure",
        )
        key, cell = item["key"], item["key"][-1]
        start = cursor
        pre, streams = take(key + "-pre", OBSERVER, exit_code=None)
        need(streams[1] == "", "observer stderr")
        pre_result = endpoint(streams[0], pre)
        observed.append({"key": key, "phase": "pre", **pre_result})
        need(item["admitted"] is pre_result["admitted"], "native launch admission")
        if not pre_result["admitted"]:
            failed = True
            need(
                item["records"] == names[start:cursor],
                "refused cell contains only preflight",
            )
            continue
        if cell in MODES:
            command = PREFIX + [
                OWNED
                + "/target/release/examples/gfx942-runtime-directional-window-benchmark",
                UID,
                "268435456",
                "3",
                "10",
                MODES[cell],
            ]
            environment = {}
        else:
            command = PREFIX + [
                OWNED + "/hsa-copy-pool-engine",
                "0",
                "1",
                "268435456",
                "3",
                "10",
                UID,
                "fine",
                "engine1",
            ]
            environment = {"HSA_XNACK": "0", "ROCR_VISIBLE_DEVICES": "4"}
        native, native_out = take(key + "-native", command, environment, exit_code=None)
        immediate, streams = take(key + "-immediate", OBSERVER, exit_code=None)
        need(streams[1] == "", "observer stderr")
        immediate_result = endpoint(streams[0], immediate)
        observed.append({"key": key, "phase": "immediate", **immediate_result})
        delay, streams = take(key + "-delay", ["/usr/bin/sleep", "20"])
        need(
            streams == ("", "")
            and clock(delay["finished"]) - clock(delay["started"]) >= 20_000_000_000,
            "actual twenty-second delay",
        )
        delayed, streams = take(key + "-delayed", OBSERVER, exit_code=None)
        need(streams[1] == "", "observer stderr")
        delayed_result = endpoint(streams[0], delayed)
        observed.append({"key": key, "phase": "delayed", **delayed_result})
        validation, _ = take(
            key + "-payload",
            [
                "/usr/bin/python3",
                "-B",
                OWNED + "/check.py",
                "--payload",
                cell,
                OWNED + "/results/campaign/" + native["name"] + ".stdout",
                OWNED + "/results/campaign/" + native["name"] + ".stderr",
            ],
            exit_code=None,
        )
        if native["exit"] == 0 and validation["exit"] == 0:
            metrics[(key[0], cell)] = payload(cell, *native_out)
        failed = (
            native["exit"] != 0
            or validation["exit"] != 0
            or not immediate_result["admitted"]
            or not delayed_result["admitted"]
        )
        need(item["records"] == names[start:cursor], "exact per-cell records")
    for tag, remote, local in (
        ("source", OWNED + "/source-files.sha256", HERE / "source-files.sha256"),
        ("binaries", OWNED + "/results/binaries.sha256", results / "binaries.sha256"),
        ("platform", OWNED + "/results/platform.sha256", results / "platform.sha256"),
        ("scripts", OWNED + "/scripts.sha256", HERE / "scripts.sha256"),
    ):
        _, streams = take(tag + "-after", ["/usr/bin/sha256sum", "-c", remote])
        need(
            streams == ("".join(path + ": OK\n" for path in manifest(local)), ""),
            "complete after identity check",
        )
    need(cursor == len(names), "no extra commands after final identities")
    need(
        state["exit"] == int(failed) and (state["failure"] is not None) == failed,
        "campaign terminal status",
    )
    need(failed or len(state["cells"]) == 16, "complete sixteen-cell success required")
    if require_full:
        need(
            not failed and len(metrics) == 16,
            "refuse all ratios for incomplete or rejected campaign",
        )
        protocol.COMMIT = COMMIT
        summary = protocol.summarize(metrics)
        summary["endpoint_count"] = len(observed)
        summary["scope"] += (
            "; no HIP cell; paired process medians only, no pooled-round estimates"
        )
        return summary
    return {
        "source": COMMIT,
        "campaign_exit": state["exit"],
        "complete_matched_campaign": not failed,
        "attempted_cells": [item["key"] for item in state["cells"]],
        "native_processes": sum(item["admitted"] for item in state["cells"]),
        "failure": state["failure"],
        "endpoints": observed,
        "comparisons_emitted": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--payload", nargs=3, metavar=("CELL", "STDOUT", "STDERR"))
    mode.add_argument("--topology", nargs=2, metavar=("TOPOLOGY", "PLACEMENT"))
    mode.add_argument("--audit", type=Path)
    mode.add_argument("--summary", type=Path)
    args = parser.parse_args()
    if args.payload:
        cell, stdout, stderr = args.payload
        payload(cell, Path(stdout).read_text(), Path(stderr).read_text())
        print("payload_valid=true cell=" + cell)
    elif args.topology:
        topology(*(Path(path).read_text() for path in args.topology))
        print("topology_valid=true gpu=4 cpu=48-95 numa=1")
    else:
        print(
            json.dumps(
                audit(args.audit or args.summary, args.summary is not None), indent=2
            )
        )


if __name__ == "__main__":
    main()
